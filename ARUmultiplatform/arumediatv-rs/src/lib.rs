//! ARUMEDIATV — Rust + uzcoders versiyasi. Hozir: Telegram orqali kirish oynasi.

mod backend;
mod countries;
mod login;
mod theme;

use backend::StubBackend;
use login::Login;
use std::rc::Rc;
use uzcoders::prelude::*;

static FONT: &[u8] = include_bytes!("../../uzcoders/assets/fonts/DejaVuSans.ttf");

/// Ilovaning kirish nuqtasi: `scale` — ekran zichligi.
pub fn make_ui(scale: f32) -> Ui {
    let font = Rc::new(Font::parse(FONT).expect("shrift"));
    let login = Login::new(Rc::new(StubBackend));
    let l = login.clone();
    let mut ui = Ui::new(font, move || l.view());
    let l = login.clone();
    ui.set_back_handler(move || l.back());
    ui.set_scale(scale);
    ui
}

#[cfg(target_os = "android")]
uzcoders::android::android_app!(make_ui);

#[cfg(test)]
mod tests {
    use super::*;

    fn tap(ui: &mut Ui, x: f32, y: f32) {
        ui.pointer(PointerEvent::new(PointerKind::Down, x, y));
        ui.pointer(PointerEvent::new(PointerKind::Up, x, y));
    }

    fn texts(cmds: &[DrawCmd]) -> Vec<String> {
        cmds.iter()
            .filter_map(|c| match c {
                DrawCmd::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn kirish_oynasi_chiziladi() {
        let mut ui = make_ui(1.0);
        let t = texts(&ui.frame(Size::new(360.0, 640.0)));
        assert!(t.iter().any(|x| x == "ARU"));
        assert!(t.iter().any(|x| x == "MEDIATV"));
        assert!(t.iter().any(|x| x == "O'zbekiston" || x.contains("zbekiston")));
        assert!(t.iter().any(|x| x == "Kod yuborish"));
    }

    fn pos(cmds: &[DrawCmd], what: &str) -> (f32, f32) {
        cmds.iter()
            .find_map(|c| match c {
                DrawCmd::Text {
                    x,
                    baseline,
                    text,
                    size,
                    ..
                } if text == what => Some((*x + 4.0, *baseline - *size * 0.3)),
                _ => None,
            })
            .unwrap_or_else(|| panic!("matn topilmadi: {what}"))
    }

    /// `ARU_SNAPSHOT_DIR` berilsa, kadrni PNG qilib saqlaydi (ko'z bilan tekshirish uchun).
    fn snap(ui: &mut Ui, name: &str, size: Size) {
        if let Ok(dir) = std::env::var("ARU_SNAPSHOT_DIR") {
            let cmds = ui.frame(size);
            let mut c = Canvas::new(size.w as usize, size.h as usize, ui.font());
            c.clear(Color::BLACK);
            c.execute(&cmds);
            std::fs::write(format!("{dir}/{name}.png"), c.to_png()).unwrap();
        }
    }

    #[test]
    fn telefon_kod_parol_bosqichlari() {
        let size = Size::new(360.0, 640.0);
        let mut ui = make_ui(1.0);
        snap(&mut ui, "1_telefon", size);
        let cmds = ui.frame(size);
        // Raqam maydoniga bosib, 9 ta raqam yozamiz.
        let (x, y) = pos(&cmds, "00 000 00 00");
        tap(&mut ui, x, y);
        assert!(ui.wants_keyboard());
        ui.frame(size);
        for c in "901234567".chars() {
            assert!(ui.key_char(c));
            ui.frame(size);
        }
        snap(&mut ui, "2_raqam", size);
        let t = texts(&ui.frame(size));
        assert!(t.iter().any(|x| x == "90 123 45 67"), "{t:?}");
        // "Kod yuborish" -> kod bosqichi.
        let cmds = ui.frame(size);
        let (x, y) = pos(&cmds, "Kod yuborish");
        tap(&mut ui, x, y);
        let cmds = ui.frame(size);
        assert!(texts(&cmds).iter().any(|x| x == "Tasdiqlash"));
        snap(&mut ui, "3_kod", size);
        // Kodni yozamiz -> parol bosqichi.
        let (x, y) = pos(&cmds, "Kod");
        tap(&mut ui, x, y);
        ui.frame(size);
        for c in "12345".chars() {
            ui.key_char(c);
        }
        let cmds = ui.frame(size);
        let (x, y) = pos(&cmds, "Tasdiqlash");
        tap(&mut ui, x, y);
        let cmds = ui.frame(size);
        assert!(texts(&cmds).iter().any(|x| x == "Kirish"));
        snap(&mut ui, "4_parol", size);
        // Orqaga: parol -> telefon.
        assert!(ui.key_back());
        let t = texts(&ui.frame(size));
        assert!(t.iter().any(|x| x == "Kod yuborish"));
    }

    #[test]
    fn davlat_tanlash() {
        let size = Size::new(360.0, 640.0);
        let mut ui = make_ui(1.0);
        let cmds = ui.frame(size);
        let (x, y) = pos(&cmds, "UZ");
        tap(&mut ui, x, y);
        let cmds = ui.frame(size);
        assert!(texts(&cmds).iter().any(|x| x == "Qidirish" || x.contains("Qidirish")));
        snap(&mut ui, "5_davlat", size);
        // Qidiruvga "rus" yozamiz: ro'yxat qisqaradi.
        let (x, y) = pos(&cmds, "Qidirish");
        tap(&mut ui, x, y);
        ui.frame(size);
        for c in "rus".chars() {
            ui.key_char(c);
        }
        let n = texts(&ui.frame(size)).len();
        assert!(n < 15, "{n}");
        // Orqaga -> kirish oynasi.
        assert!(ui.key_back()); // fokusni oladi
        assert!(ui.key_back()); // ekranni yopadi
        assert!(texts(&ui.frame(size)).iter().any(|x| x == "Kod yuborish"));
    }

    #[test]
    fn countries_royxati() {
        assert!(crate::countries::COUNTRIES.len() > 200);
        let uz = crate::login::country_by_code("998", None).unwrap();
        assert_eq!(crate::countries::COUNTRIES[uz].iso, "UZ");
    }
}
