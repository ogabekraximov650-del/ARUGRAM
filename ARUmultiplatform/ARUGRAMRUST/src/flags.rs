//! Davlat bayroqlari: `assets/flags.bin` (flagcdn.com PNG'lari, 80 px kenglikda) dan, kerak bo'lganda ochiladi va keshlanadi.

use crate::tg::hint;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use uzcoders::prelude::*;

static BLOB: &[u8] = include_bytes!("../assets/flags.bin");

thread_local! {
    static CACHE: RefCell<HashMap<String, Option<Rc<Image>>>> = RefCell::new(HashMap::new());
}

/// Blob formati: `u16` soni, keyin har biri: ISO (2 bayt) + `u32` uzunlik + PNG.
fn find(iso: &str) -> Option<&'static [u8]> {
    let n = u16::from_le_bytes([*BLOB.first()?, *BLOB.get(1)?]) as usize;
    let mut p = 2;
    for _ in 0..n {
        let code = BLOB.get(p..p + 2)?;
        let len = u32::from_le_bytes(BLOB.get(p + 2..p + 6)?.try_into().ok()?) as usize;
        let data = BLOB.get(p + 6..p + 6 + len)?;
        if code == iso.as_bytes() {
            return Some(data);
        }
        p += 6 + len;
    }
    None
}

pub fn flag(iso: &str) -> Option<Rc<Image>> {
    CACHE.with(|c| {
        c.borrow_mut()
            .entry(iso.to_string())
            .or_insert_with(|| {
                find(iso)
                    .and_then(decode_png)
                    .map(|i| Rc::new(waved(&i)))
            })
            .clone()
    })
}

/// UZ: Telegram emoji bayroqlari kabi "hilpiragan" bayroq: tekis bayroq 72x72 kvadratga to'lqin shaklida tushiriladi, to'lqin qiyaligiga qarab yorug'lik o'zgaradi. 72x72 > 64x64, shuning uchun GPU'da katta rasm yo'lidan (atlasdan emas) o'tadi.
/// EN: A "waving" flag like Telegram's emoji flags: the flat flag is mapped onto a 72x72 square along a wave, brightness follows the wave slope. 72x72 > 64x64, so it takes the GPU's big-image path (not the atlas).
/// RU: «Развевающийся» флаг, как эмодзи-флаги Telegram: плоский флаг ложится на квадрат 72x72 по волне, яркость зависит от наклона волны. 72x72 > 64x64, поэтому идёт путём больших изображений GPU (не атласом).
fn waved(src: &Image) -> Image {
    const S: u32 = 72;
    const FW: f32 = 62.0;
    const FH: f32 = 44.0;
    const SS: u32 = 4;
    let (x0, y0) = ((S as f32 - FW) / 2.0, (S as f32 - FH) / 2.0);
    let sample = |u: f32, v: f32| -> [f32; 4] {
        let fx = (u * src.w as f32 - 0.5).clamp(0.0, src.w as f32 - 1.0);
        let fy = (v * src.h as f32 - 0.5).clamp(0.0, src.h as f32 - 1.0);
        let (ix, iy) = (fx as u32, fy as u32);
        let (tx, ty) = (fx - ix as f32, fy - iy as f32);
        let at = |x: u32, y: u32| -> [f32; 4] {
            let k = (((y.min(src.h - 1)) * src.w + x.min(src.w - 1)) * 4) as usize;
            let a = src.rgba[k + 3] as f32 / 255.0;
            [src.rgba[k] as f32 * a, src.rgba[k + 1] as f32 * a, src.rgba[k + 2] as f32 * a, a]
        };
        let (a, b, c, d) = (at(ix, iy), at(ix + 1, iy), at(ix, iy + 1), at(ix + 1, iy + 1));
        let mut o = [0.0f32; 4];
        for n in 0..4 {
            o[n] = (a[n] * (1.0 - tx) + b[n] * tx) * (1.0 - ty) + (c[n] * (1.0 - tx) + d[n] * tx) * ty;
        }
        o
    };
    let mut rgba = vec![0u8; (S * S * 4) as usize];
    for py in 0..S {
        for px in 0..S {
            let mut acc = [0.0f32; 4];
            for sy in 0..SS {
                for sx in 0..SS {
                    let x = px as f32 + (sx as f32 + 0.5) / SS as f32;
                    let y = py as f32 + (sy as f32 + 0.5) / SS as f32;
                    let u = (x - x0) / FW;
                    if !(0.0..=1.0).contains(&u) {
                        continue;
                    }
                    let ph = u * 5.6 + 0.5;
                    let dy = 3.6 * ph.sin() + (u - 0.5) * 3.0;
                    let v = (y - y0 - dy) / FH;
                    if !(0.0..=1.0).contains(&v) {
                        continue;
                    }
                    // burchaklarni yumshatish / soften the corners
                    let (cx, cy) = ((u - 0.5).abs() * FW - (FW / 2.0 - 3.0), (v - 0.5).abs() * FH - (FH / 2.0 - 3.0));
                    if cx > 0.0 && cy > 0.0 && cx * cx + cy * cy > 9.0 {
                        continue;
                    }
                    let shade = 1.0 + 0.2 * ph.cos() * (1.0 - 0.3 * v);
                    let c = sample(u, v);
                    acc[0] += (c[0] * shade).min(255.0 * c[3]);
                    acc[1] += (c[1] * shade).min(255.0 * c[3]);
                    acc[2] += (c[2] * shade).min(255.0 * c[3]);
                    acc[3] += c[3];
                }
            }
            let n = (SS * SS) as f32;
            let a = acc[3] / n;
            let k = ((py * S + px) * 4) as usize;
            if a > 0.0 {
                for c in 0..3 {
                    rgba[k + c] = (acc[c] / n / a).clamp(0.0, 255.0) as u8;
                }
                rgba[k + 3] = (a * 255.0).round() as u8;
            }
        }
    }
    Image { w: S, h: S, rgba }
}

/// Bayroq (22 dp balandlikda) yoki, topilmasa, ISO kodi; 40 dp kenglikdagi katakda.
pub fn flag_view(iso: &str) -> Element {
    match flag(iso) {
        Some(img) => {
            column(vec![image(&img, 24.0, 24.0)]).width(40.0).align(Align::Start)
        }
        None => text(iso)
            .size(14.0)
            .color(hint())
            .line_height(24.0)
            .width(40.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bayroqlar_ochiladi() {
        let uz = flag("UZ").expect("UZ bayrog'i");
        assert!(uz.w > 0 && uz.h > 0);
        assert!(flag("US").is_some());
        assert!(flag("FT").is_none());
        // Kvadrat, burchak shaffof / square with a transparent corner.
        assert!(uz.w == 72 && uz.h == 72 && uz.rgba[3] < 128);
    }
}
