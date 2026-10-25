//! UZ: Lottie JSON ni bir marta kadrlar varag'iga (sprite sheet) aylantiradi; qatlam ranglarini almashtirish mumkin (RLottie `setLayerColor` kabi).
//! EN: Turns Lottie JSON once into a sprite sheet of frames; layer colors can be replaced (like RLottie's `setLayerColor`).
//! RU: Превращает Lottie JSON один раз в лист кадров; цвета слоёв можно заменять (как `setLayerColor` в RLottie).

use uzcoders_lottie::{Animation, RenderConfig, Renderer, Rgba8};

/// Tayyor varaq / A finished sheet / Готовый лист.
#[derive(Clone)]
pub struct Sheet {
    pub rgba: Vec<u8>,
    pub w: u32,
    pub h: u32,
    pub cols: u32,
    pub count: u32,
    pub fps: f32,
}

const MAX_SIDE: u32 = 4096;

/// `layers`: (qatlam nomi, rang) / (layer name, color) / (имя слоя, цвет). `px` — kadrning uzun tomoni.
pub fn build(raw: &str, px: u32, max_fps: f32, layers: &[(&str, [f32; 3])]) -> Result<Sheet, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| build_inner(raw, px, max_fps, layers))).unwrap_or_else(|_| Err("chizishda kutilmagan xato".to_string()))
}

fn build_inner(raw: &str, px: u32, max_fps: f32, layers: &[(&str, [f32; 3])]) -> Result<Sheet, String> {
    let mut anim = Animation::from_json_str(raw).map_err(|e| format!("parse: {e}"))?;
    for (name, rgb) in layers {
        if *name == "*" {
            anim.set_all_colors(*rgb);
        } else {
            anim.set_layer_color(name, *rgb);
        }
    }
    let r = Renderer::default();
    let prepared = r.prepare(&anim).map_err(|e| format!("{e}"))?;
    let long = anim.width.max(anim.height).max(1) as f32;
    let cfg = RenderConfig::new(Rgba8::TRANSPARENT, px.max(16) as f32 / long);
    let total = (anim.out_point - anim.in_point).max(1.0);
    let fr = if anim.frame_rate > 1.0 { anim.frame_rate } else { 30.0 };
    let secs = (total / fr).max(0.1);
    let fps = fr.min(max_fps.max(1.0));
    let count = ((secs * fps).round() as u32).max(1);
    let first = prepared.render_frame(anim.in_point, cfg).map_err(|e| format!("{e}"))?;
    let (cw, ch) = (first.width, first.height);
    if cw == 0 || ch == 0 || cw > MAX_SIDE || ch > MAX_SIDE {
        return Err("kadr o'lchami noto'g'ri".into());
    }
    let cols = (MAX_SIDE / cw).clamp(1, count).min(((count as f32).sqrt().ceil() as u32).max(1));
    let rows = count.div_ceil(cols);
    if rows * ch > MAX_SIDE {
        return Err("varaq juda katta".into());
    }
    let (w, h) = (cols * cw, rows * ch);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for i in 0..count {
        let f = if i == 0 { first.clone() } else { prepared.render_frame(anim.in_point + total * i as f32 / count as f32, cfg).map_err(|e| format!("{e}"))? };
        if (f.width, f.height) != (cw, ch) {
            return Err("kadr o'lchami o'zgardi".into());
        }
        let (ox, oy) = ((i % cols) * cw, (i / cols) * ch);
        for y in 0..ch {
            let src = (y * cw * 4) as usize;
            let dst = (((oy + y) * w + ox) * 4) as usize;
            rgba[dst..dst + (cw * 4) as usize].copy_from_slice(&f.pixels[src..src + (cw * 4) as usize]);
        }
    }
    unpremultiply(&mut rgba);
    bleed_transparent(&mut rgba, w as usize, h as usize, cw as usize, ch as usize);
    Ok(Sheet { rgba, w, h, cols, count, fps: count as f32 / secs })
}

fn unpremultiply(px: &mut [u8]) {
    for p in px.chunks_exact_mut(4) {
        let a = u32::from(p[3]);
        if a > 0 && a < 255 {
            for c in &mut p[..3] {
                *c = ((u32::from(*c) * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
}

/// To'liq shaffof piksellarga qo'shni ko'rinadigan pikselning rangini beradi (har kadr ichida, kadr chegarasidan chiqmasdan).
fn bleed_transparent(px: &mut [u8], w: usize, h: usize, cw: usize, ch: usize) {
    let src = px.to_vec();
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 4;
            if src[i + 3] != 0 {
                continue;
            }
            let (fx, fy) = ((x / cw) * cw, (y / ch) * ch);
            let mut best: Option<usize> = None;
            for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if nx < fx as i32 || ny < fy as i32 || nx >= (fx + cw) as i32 || ny >= (fy + ch) as i32 {
                    continue;
                }
                let j = (ny as usize * w + nx as usize) * 4;
                if src[j + 3] > 0 && best.is_none_or(|b| src[j + 3] > src[b + 3]) {
                    best = Some(j);
                }
            }
            if let Some(j) = best {
                px[i..i + 3].copy_from_slice(&src[j..j + 3]);
            }
        }
    }
}

