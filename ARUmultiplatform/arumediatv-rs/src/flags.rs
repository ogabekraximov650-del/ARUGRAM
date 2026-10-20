//! Davlat bayroqlari: `assets/flags.bin` (flagcdn.com PNG'lari, 80 px kenglikda) dan, kerak bo'lganda ochiladi va keshlanadi.

use crate::theme::TEXT_DIM;
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
                    .map(|i| Rc::new(i.round_corners(6.0)))
            })
            .clone()
    })
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
            .color(TEXT_DIM)
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
