//! Muqova rasmlari: serverdan bir marta olinadi (fon oqimida), qirqiladi va xotirada saqlanadi.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use uzcoders::prelude::*;

thread_local! {
    static RAW: RefCell<HashMap<String, Option<Rc<Image>>>> = RefCell::new(HashMap::new());
    static CROP: RefCell<HashMap<String, Rc<Image>>> = RefCell::new(HashMap::new());
    static BUSY: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

/// Rasmni `rw:rh` nisbatiga markazdan qirqadi.
fn crop(img: &Image, rw: u32, rh: u32) -> Image {
    let (w, h) = (img.w, img.h);
    if w == 0 || h == 0 || rw == 0 || rh == 0 {
        return Image { w, h, rgba: img.rgba.clone() };
    }
    let (cw, ch) = if w * rh > h * rw { ((h * rw / rh).max(1), h) } else { (w, (w * rh / rw).max(1)) };
    let (x0, y0) = ((w - cw) / 2, (h - ch) / 2);
    let mut out = Vec::with_capacity((cw * ch * 4) as usize);
    for y in 0..ch {
        let s = (((y0 + y) * w + x0) * 4) as usize;
        out.extend_from_slice(&img.rgba[s..s + (cw * 4) as usize]);
    }
    Image { w: cw, h: ch, rgba: out }
}

/// Video muqovasi (`rw:rh` nisbatda). Hali olinmagan bo'lsa `None` (olish boshlanadi, tayyor bo'lgach kadr qayta chiziladi).
pub fn thumb(id: &str, rw: u32, rh: u32) -> Option<Rc<Image>> {
    let key = format!("{id}@{rw}x{rh}");
    if let Some(c) = CROP.with(|m| m.borrow().get(&key).cloned()) {
        return Some(c);
    }
    match RAW.with(|m| m.borrow().get(id).cloned()) {
        Some(Some(raw)) => {
            let c = Rc::new(crop(&raw, rw, rh));
            CROP.with(|m| {
                let mut m = m.borrow_mut();
                if m.len() > 80 {
                    m.clear();
                }
                m.insert(key, c.clone());
            });
            Some(c)
        }
        Some(None) => None,
        None => {
            if BUSY.with(|b| b.borrow_mut().insert(id.to_string())) {
                let (vid, vid2) = (id.to_string(), id.to_string());
                let file = format!("{}/th_{}.jpg", uzcoders::android::files_dir(), id);
                spawn_bg_fast(
                    move || {
                        let spec = format!("{}\n{}", crate::net::url(&format!("/api/video/{vid}/thumb")), file);
                        let code = uzcoders::android::activity_call_ret("download", &spec);
                        let img = if code.trim() == "200" { std::fs::read(&file).ok().and_then(|b| decode_image(&b)).map(|i| i.resized(640, 640)) } else { None };
                        let _ = std::fs::remove_file(&file);
                        img
                    },
                    move |img| {
                        BUSY.with(|b| b.borrow_mut().remove(&vid2));
                        RAW.with(|m| {
                            let mut m = m.borrow_mut();
                            if m.len() > 80 {
                                m.clear();
                            }
                            m.insert(vid2.clone(), img.map(Rc::new));
                        });
                        request_frame();
                    },
                );
            }
            None
        }
    }
}
