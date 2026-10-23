//! Rust shriftida (Roboto) bo'lmagan belgilar — emoji, koreys, xitoy, arab va h.k. — uchun matnni Android tizim shriftlari
//! bilan rasmga chizish (`AruActivity.textPng`). Kotlin ilovada buni tizimning o'zi qiladi (`RichPackText`); bu yerda shrift
//! faqat Roboto, shuning uchun bunday belgilar "▯" bo'lib chiqardi.
//!
//! Rasm fonda yasaladi va eslab qolinadi (`Home::rich_imgs`); tayyor bo'lguncha joy egallovchi, Androidsiz (testlarda)
//! yoki xatoda — oddiy matn.

use super::*;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// `rich_imgs` yozuvi holati.
#[derive(Clone)]
pub(super) enum RichState {
    Loading,
    Ready(Rc<Image>, u32, u32),
    Failed,
}

/// Matnda Roboto qamrab olmaydigan belgi (emoji, CJK, arab...) bormi.
pub(super) fn needs_rich(s: &str) -> bool {
    s.chars().any(|c| {
        let u = c as u32;
        !(u < 0x0530                       // lotin, kirill, yunon
            || (0x1E00..=0x1FFF).contains(&u) // kengaytirilgan lotin / yunon
            || (0x2000..=0x206F).contains(&u) // umumiy tinish belgilari
            || (0x20A0..=0x20BF).contains(&u) // valyuta
            || u == 0x2116)                   // №
    })
}

fn hash_of(s: &str) -> u64 {
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

/// Fon oqimida: matn -> PNG (Java) -> rasm. `None` — Android yo'q yoki chizilmadi.
fn render(text: &str, size_px: f32, max_w_px: i32, argb: u32, bold: bool) -> Option<(Image, u32, u32)> {
    // Bir vaqtda bitta chizish: emoji paneli o'nlab rasm so'raydi, Java chaqiruvlari ketma-ket bo'lsin.
    static RENDER: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _g = RENDER.lock().unwrap_or_else(|e| e.into_inner());
    let dir = format!("{}/rich", uzcoders::android::files_dir());
    let _ = std::fs::create_dir_all(&dir);
    let font = crate::emojifont::path();
    let path = format!("{dir}/{:016x}.png", hash_of(&format!("{size_px}|{max_w_px}|{argb}|{bold}|{font}|{text}")));
    let r = uzcoders::android::activity_call_ret("textPng", &format!("{size_px}|{max_w_px}|{argb:08X}|{}|{path}|{font}|{text}", bold as u8));
    let (w, h) = r.split_once('|')?;
    let (w, h): (u32, u32) = (w.parse().ok()?, h.parse().ok()?);
    let bytes = std::fs::read(&path).ok()?;
    let _ = std::fs::remove_file(&path);
    let img = uzcoders::prelude::decode_image(&bytes)?;
    Some((img, w, h))
}

impl Home {
    /// Matn rasmi (emoji/CJK bilan to'g'ri). `max_w_dp` — qatorga o'raladigan kenglik. Tayyor bo'lguncha bo'sh joy,
    /// chizib bo'lmasa — oddiy matn.
    pub(super) fn rich_el(self: &Rc<Self>, text: &str, size_dp: f32, max_w_dp: f32, argb: u32, bold: bool) -> Element {
        let key = format!("{size_dp}|{max_w_dp}|{argb:08X}|{}|{text}", bold as u8);
        let _ = self.gate_rev.get();
        let dens = density().max(0.5);
        let state = self.rich_imgs.borrow().get(&key).cloned();
        match state {
            Some(RichState::Ready(img, w, h)) => return image(&img, w as f32 / dens, h as f32 / dens),
            Some(RichState::Failed) => {
                let c = Color::rgba((argb >> 16) as u8, (argb >> 8) as u8, argb as u8, (argb >> 24) as u8);
                let e = tx(text.to_string(), size_dp, c);
                return if bold { e.bold() } else { e };
            }
            Some(RichState::Loading) => {}
            None => {
                self.rich_imgs.borrow_mut().insert(key.clone(), RichState::Loading);
                let (me, t) = (self.clone(), text.to_string());
                let (px, mw) = (size_dp * dens, (max_w_dp * dens) as i32);
                spawn_bg(move || render(&t, px, mw, argb, bold), move |r| {
                    let st = match r {
                        Some((img, w, h)) => RichState::Ready(Rc::new(img), w, h),
                        None => RichState::Failed,
                    };
                    me.rich_imgs.borrow_mut().insert(key, st);
                    me.gate_rev.set(me.gate_rev.get() + 1);
                });
            }
        }
        // Rasm tayyor bo'lguncha taxminiy joy (qatorlar sakramasin).
        let w = (text.chars().count() as f32 * size_dp * 0.55).min(max_w_dp).max(size_dp);
        column(vec![]).width(w).height(size_dp * 1.3)
    }
}

#[cfg(test)]
mod tests {
    use super::needs_rich;

    #[test]
    fn oddiy_matn_boy_emas() {
        assert!(!needs_rich("Salom, qalaysiz? o‘zbek tili — яхши №1 €5"));
        assert!(!needs_rich("O'zbekiston g'alaba"));
    }

    #[test]
    fn emoji_va_cjk_boy() {
        assert!(needs_rich("Salom 😀"));
        assert!(needs_rich("안녕하세요"));
        assert!(needs_rich("你好"));
        assert!(needs_rich("مرحبا"));
        assert!(needs_rich("❤️"));
    }
}
