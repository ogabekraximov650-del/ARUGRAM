//! Telegram `.tgs` (Lottie) stikerini bir marta kadrlar varag'iga (sprite sheet) aylantiradi: kadrlar bitta rasmda yonma-yon, `uzcoders-ui`
//! ning `sprite` elementi ularni vaqt bo'yicha almashtirib ko'rsatadi. Chizish — sof Rust (`uzcoders-lottie`); tashqi dekoder yo'q.
//! Varaq — oddiy (straight) alfali RGBA: GL aralashtirishi shuni kutadi. Shaffof piksellar rangi qo'shni piksellardan to'ldiriladi
//! (bilinear namuna olishda qirrada qora hoshiya chiqmasligi uchun).

use uzcoders_lottie::{Animation, RenderConfig, Renderer, Rgba8};

/// Tayyor varaq: `cols` ustun, `count` kadr, har kadr `cw` x `ch` piksel, `fps` — ko'rsatish tezligi.
pub struct AnimSheet {
    pub rgba: Vec<u8>,
    pub w: u32,
    pub h: u32,
    pub cols: u32,
    pub count: u32,
    pub fps: f32,
}

/// GPU tekstura tomoni chegarasi (eng kam umumiy qiymat).
const MAX_SIDE: u32 = 4096;

/// Xavfsiz: chizish kutubxonasidagi kutilmagan panic ilovani yopmasligi uchun (`panic = "unwind"`) xatoga aylantiriladi.
pub fn build_safe(raw: &[u8], px: u32, max_frames: u32, max_fps: f32) -> Result<AnimSheet, String> {
    std::panic::catch_unwind(|| build(raw, px, max_frames, max_fps)).unwrap_or_else(|_| Err("chizishda kutilmagan xato".to_string()))
}

/// `.tgs`/JSON baytlaridan varaq. `px` — kadrning uzun tomoni; `max_frames`/`max_fps` — xotira va tezlik chegarasi.
pub fn build(raw: &[u8], px: u32, max_frames: u32, max_fps: f32) -> Result<AnimSheet, String> {
    let json = uzcoders_lottie::tgs::to_json(raw).ok_or("tgs ochilmadi")?;
    let anim = Animation::from_json_str(&json).map_err(|e| format!("parse: {e}"))?;
    let r = Renderer::default();
    let prepared = r.prepare(&anim).map_err(|e| format!("{e}"))?;
    let long = anim.width.max(anim.height).max(1) as f32;
    let cfg = RenderConfig::new(Rgba8::TRANSPARENT, px.max(16) as f32 / long);
    let total = (anim.out_point - anim.in_point).max(1.0);
    let fr = if anim.frame_rate > 1.0 { anim.frame_rate } else { 30.0 };
    let secs = (total / fr).max(0.1);
    let fps = fr.min(max_fps.max(1.0));
    let count = ((secs * fps).round() as u32).clamp(1, max_frames.max(1));
    let t0 = std::time::Instant::now();
    let first = prepared.render_frame(anim.in_point, cfg).map_err(|e| format!("{e}"))?;
    let (cw, ch) = (first.width, first.height);
    if cw == 0 || ch == 0 || cw > MAX_SIDE || ch > MAX_SIDE {
        return Err("kadr o'lchami noto'g'ri".into());
    }
    // Varaq o'lchami: tomonlar MAX_SIDE dan oshmasin.
    let cols = (MAX_SIDE / cw).clamp(1, count).min(((count as f32).sqrt().ceil() as u32).max(1));
    let rows = count.div_ceil(cols);
    if rows * ch > MAX_SIDE {
        return Err("varaq juda katta".into());
    }
    let (w, h) = (cols * cw, rows * ch);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for i in 0..count {
        // Juda sekin stiker telefonni qizdirmasin: 12 soniyadan oshsa to'xtatiladi (statik rasm qoladi).
        if t0.elapsed().as_secs() > 12 {
            return Err("vaqt chegarasi".into());
        }
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
    Ok(AnimSheet { rgba, w, h, cols, count, fps: count as f32 / secs })
}

/// GIF (`mp4`) baytlaridan varaq: dekodlash Android tizim dekoderida (Java ko'prigi `gifSheet`), yig'ish va saqlash Rustda.
/// Kompyuter testlarida (Androidsiz) xato qaytaradi.
pub fn gif_sheet(raw: &[u8], px: u32, max_frames: u32, max_fps: f32) -> Result<AnimSheet, String> {
    let dir = format!("{}/gifs", crate::data::cache_dir());
    let _ = std::fs::create_dir_all(&dir);
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let (inp, out) = (format!("{dir}/{stamp}.mp4"), format!("{dir}/{stamp}.png"));
    std::fs::write(&inp, raw).map_err(|e| format!("yozilmadi: {e}"))?;
    let r = uzcoders::android::activity_call_ret("gifSheet", &format!("{inp}|{out}|{max_frames}|{px}|{max_fps}"));
    let png = std::fs::read(&out).ok();
    let _ = std::fs::remove_file(&inp);
    let _ = std::fs::remove_file(&out);
    parse_gif_result(&r, png)
}

fn parse_gif_result(r: &str, png: Option<Vec<u8>>) -> Result<AnimSheet, String> {
    let p: Vec<&str> = r.split('|').collect();
    if p.len() != 5 {
        return Err(if r.is_empty() { "GIF dekodlanmadi (Java javobi bo'sh)".into() } else { format!("GIF dekodlanmadi: {r}") });
    }
    let (cols, count): (u32, u32) = (p[2].parse().map_err(|_| "ustunlar")?, p[3].parse().map_err(|_| "kadrlar")?);
    let fps: f32 = p[4].parse().map_err(|_| "fps")?;
    let img = uzcoders::prelude::decode_image(&png.ok_or("varaq o'qilmadi")?).ok_or("varaq dekodlanmadi")?;
    if cols == 0 || count == 0 || !(fps > 0.0) {
        return Err("varaq noto'g'ri".into());
    }
    Ok(AnimSheet { rgba: img.rgba, w: img.w, h: img.h, cols, count, fps })
}

/// Oldindan ko'paytirilgan (premultiplied) rangni oddiy alfaga aylantiradi.
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

#[cfg(test)]
mod tests {
    use super::*;

    const JSON: &str = r#"{"v":"5.7.6","fr":30,"ip":0,"op":30,"w":20,"h":20,"assets":[],"layers":[{"ddd":0,"ind":1,"ty":4,"nm":"r","sr":1,"ks":{"o":{"a":0,"k":100},"r":{"a":0,"k":0},"p":{"a":1,"k":[{"t":0,"s":[5,10,0],"i":{"x":0.5,"y":0.5},"o":{"x":0.5,"y":0.5}},{"t":30,"s":[15,10,0]}]},"a":{"a":0,"k":[0,0,0]},"s":{"a":0,"k":[100,100,100]}},"ao":0,"ip":0,"op":30,"st":0,"bm":0,"shapes":[{"ty":"gr","it":[{"ty":"rc","d":1,"s":{"a":0,"k":[6,6]},"p":{"a":0,"k":[0,0]},"r":{"a":0,"k":0}},{"ty":"fl","c":{"a":0,"k":[1,0,0,1]},"o":{"a":0,"k":50},"r":1},{"ty":"tr","p":{"a":0,"k":[0,0]},"a":{"a":0,"k":[0,0]},"s":{"a":0,"k":[100,100]},"r":{"a":0,"k":0},"o":{"a":0,"k":100}}]}]}]}"#;

    #[test]
    fn varaq_kadrlari_va_olchami() {
        let s = build(JSON.as_bytes(), 20, 10, 30.0).unwrap();
        assert_eq!(s.count, 10);
        assert_eq!(s.cols * (s.w / s.cols), s.w);
        assert_eq!(s.rgba.len() as u32, s.w * s.h * 4);
        assert!(s.fps > 5.0 && s.fps <= 31.0, "{}", s.fps);
        let (cw, ch) = (s.w / s.cols, s.h / s.count.div_ceil(s.cols));
        assert_eq!((cw, ch), (20, 20));
    }

    #[test]
    fn harakat_kadrlarda_korinadi() {
        let s = build(JSON.as_bytes(), 20, 10, 30.0).unwrap();
        let (cw, ch) = (s.w / s.cols, s.h / s.count.div_ceil(s.cols));
        // Birinchi kadrda kvadrat chapda (x~5), oxirgisida o'ngda (x~14).
        let alpha_x = |frame: u32, x: u32| s.rgba[((((frame / s.cols) * ch + 10) * s.w + (frame % s.cols) * cw + x) * 4 + 3) as usize];
        assert!(alpha_x(0, 5) > 100 && alpha_x(0, 16) == 0);
        assert!(alpha_x(s.count - 1, 5) == 0 && alpha_x(s.count - 1, 14) > 100);
    }

    #[test]
    fn straight_alfa_va_hoshiya_rangi() {
        let s = build(JSON.as_bytes(), 20, 4, 30.0).unwrap();
        // Yarim shaffof qizil (o'zgarmas 50%): oddiy alfada R=255, G=B=0.
        let i = (((10 * s.w) + 5) * 4) as usize;
        let p = &s.rgba[i..i + 4];
        assert!(p[3] > 100 && p[3] < 140 && p[0] >= 250 && p[1] == 0, "{p:?}");
        // To'liq shaffof qo'shni piksel qirrada qizil rangni oladi (qora emas).
        let edge = (((10 * s.w) + 1) * 4) as usize;
        assert_eq!(s.rgba[edge + 3], 0);
        assert!(s.rgba[edge] >= 250 && s.rgba[edge + 1] == 0, "{:?}", &s.rgba[edge..edge + 4]);
    }

    #[test]
    fn gif_javobi_tekshiriladi() {
        assert!(parse_gif_result("", None).is_err());
        assert!(parse_gif_result("10|10|2|4", None).is_err());
        assert!(parse_gif_result("10|10|2|4|12.5", None).is_err());
        assert!(parse_gif_result("10|10|2|4|12.5", Some(vec![1, 2, 3])).is_err());
    }

    #[test]
    fn yaroqsiz_kirish_xato() {
        assert!(build(b"not json", 20, 4, 30.0).is_err());
    }
}

