//! ARUMEDIATV — Rust + uzcoders versiyasi. Hozir: Telegram orqali kirish oynasi.

mod api;
mod backend;
mod data;
mod downloads;
mod home;
mod sync;
mod countries;
mod flags;
mod login;
mod player;
mod gate;
mod theme;
mod webp;
mod packs_data;

use backend::Backend;
#[cfg(not(target_os = "android"))]
use backend::StubBackend;
use home::Home;
use login::Login;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use uzcoders::prelude::*;

static REGULAR: &[u8] = include_bytes!("../assets/fonts/Roboto-Regular.ttf");
static BOLD: &[u8] = include_bytes!("../assets/fonts/Roboto-Bold.ttf");
static BLACK: &[u8] = include_bytes!("../assets/fonts/Roboto-Black.ttf");

#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Login,
    Home,
}

/// Ilovaning kirish nuqtasi: `scale` — ekran zichligi.
pub fn make_ui(scale: f32) -> Ui {
    let font = Rc::new(Font::parse(REGULAR).expect("Roboto Regular"));
    // Android'da haqiqiy Telegram (grammers), kompyuter testlarida tarmoqsiz sinov.
    #[cfg(target_os = "android")]
    let backend: Arc<dyn Backend> = backend::shared();
    #[cfg(not(target_os = "android"))]
    let backend: Arc<dyn Backend> = Arc::new(StubBackend);

    let screen = State::new(Screen::Login);
    let home: Rc<RefCell<Option<Rc<Home>>>> = Rc::new(RefCell::new(None));
    // Kirish tugagach: sessiya saqlanadi va bosh sahifa ochiladi.
    let login_slot: Rc<RefCell<Option<Rc<Login>>>> = Rc::new(RefCell::new(None));
    // Chiqish / hisobni o'chirish: sessiya va Telegram hisobi tozalanadi, kirish ekrani qaytadi.
    let (s3, h3, l3, b3) = (screen.clone(), home.clone(), login_slot.clone(), backend.clone());
    let on_logout: Rc<dyn Fn()> = Rc::new(move || {
        api::clear_session();
        let b = b3.clone();
        std::thread::spawn(move || b.logout());
        *h3.borrow_mut() = None;
        if let Some(l) = l3.borrow().as_ref() {
            l.restart();
        }
        s3.set(Screen::Login);
    });
    let (s2, h2) = (screen.clone(), home.clone());
    let on_done: Rc<dyn Fn(String)> = Rc::new(move |session: String| {
        *h2.borrow_mut() = Some(Home::new(session, on_logout.clone()));
        s2.set(Screen::Home);
    });
    let login = Login::new(backend, on_done.clone());
    *login_slot.borrow_mut() = Some(login.clone());
    // Saqlangan sessiya bo'lsa to'g'ridan-to'g'ri bosh sahifa.
    if let Some(session) = api::load_session() {
        on_done(session);
    }

    let (l, h, s) = (login.clone(), home.clone(), screen.clone());
    let mut ui = Ui::new(font, move || match s.get() {
        Screen::Login => l.view(),
        Screen::Home => match h.borrow().as_ref() {
            Some(home) => home.view(),
            None => l.view(),
        },
    });
    ui.add_font(700, Rc::new(Font::parse(BOLD).expect("Roboto Bold")));
    ui.add_font(900, Rc::new(Font::parse(BLACK).expect("Roboto Black")));
    let (l, h, s) = (login.clone(), home.clone(), screen.clone());
    ui.set_back_handler(move || match s.get() {
        Screen::Login => l.back(),
        Screen::Home => h.borrow().as_ref().is_some_and(|home| home.back()),
    });
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

    /// Fon vazifalari tugaguncha natijalarni UI oqimida qabul qiladi.
    fn pump() {
        for _ in 0..400 {
            uzcoders::core::drain_tasks();
            if uzcoders::core::pending_tasks() == 0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
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
            let mut c = ui.canvas(size.w as usize, size.h as usize);
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
        pump();
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
        pump();
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
        let (x, y) = pos(&cmds, "O'zbekiston");
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
