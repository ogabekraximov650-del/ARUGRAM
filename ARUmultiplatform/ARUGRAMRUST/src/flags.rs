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
                    .map(|i| Rc::new(upscale_if_small(i).round_corners(6.0)))
            })
            .clone()
    })
}

/// UZ: GPU'da kichik rasmlar umumiy atlasga tushadi va ba'zi qurilmalarda ko'rinmay qoladi; shuning uchun kichik bayroqlar 2 marta kattalashtiriladi (katta rasm yo'li).
/// EN: Small images go to the shared GPU atlas and may not show on some devices, so small flags are doubled (the big-image path).
/// RU: Маленькие изображения на GPU попадают в общий атлас и на некоторых устройствах не видны, поэтому маленькие флаги увеличиваются вдвое (путь больших изображений).
fn upscale_if_small(i: Image) -> Image {
    if i.w * i.h > 4096 {
        return i;
    }
    let (w, h) = (i.w * 2, i.h * 2);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let s = (((y / 2) * i.w + x / 2) * 4) as usize;
            let d = ((y * w + x) * 4) as usize;
            rgba[d..d + 4].copy_from_slice(&i.rgba[s..s + 4]);
        }
    }
    Image { w, h, rgba }
}

/// Bayroq (22 dp balandlikda) yoki, topilmasa, ISO kodi; 40 dp kenglikdagi katakda.
pub fn flag_view(iso: &str) -> Element {
    match flag(iso) {
        Some(img) => {
            let aspect = img.w as f32 / img.h as f32;
            let w = (22.0 * aspect).min(36.0);
            column(vec![image(&img, w, w / aspect)])
                .width(40.0)
                .align(Align::Start)
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
        // Burchak shaffof (yumaloqlangan).
        assert!(uz.rgba[3] < 128);
    }
}
