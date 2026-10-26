//! UZ: ARUGRAM (Rust + uzcoders): Telegram uslubidagi tanishtiruv va kirish oqimi. Litsenziya: GPL-2.0-or-later (`LICENSE`, `NOTICE.md`).
//! EN: ARUGRAM (Rust + uzcoders): the Telegram-style intro and sign-in flow. License: GPL-2.0-or-later (`LICENSE`, `NOTICE.md`).
//! RU: ARUGRAM (Rust + uzcoders): интро и вход в стиле Telegram. Лицензия: GPL-2.0-or-later (`LICENSE`, `NOTICE.md`).

pub mod account;
pub mod countries;
pub mod flags;
pub mod intro_assets;
pub mod intro_scene;
pub mod intro_shapes;
pub mod softgl;
pub mod intro;
pub mod login;
pub mod lottie_sheet;
pub mod tg;

use intro::Intro;
use login::Login;
use std::rc::Rc;
use uzcoders::prelude::*;

static REGULAR: &[u8] = include_bytes!("../assets/fonts/Roboto-Regular.ttf");
static BOLD: &[u8] = include_bytes!("../assets/fonts/Roboto-Bold.ttf");
/// UZ: Telegram `AndroidUtilities.bold()` = `rmedium.ttf`. EN: Telegram's `AndroidUtilities.bold()` = `rmedium.ttf`. RU: `AndroidUtilities.bold()` Telegram = `rmedium.ttf`.
static MEDIUM: &[u8] = include_bytes!("../assets/fonts/Roboto-Medium.ttf");

/// UZ: Ilovaning kirish nuqtasi: `scale` — ekran zichligi. EN: App entry point: `scale` is the screen density. RU: Точка входа: `scale` — плотность экрана.
pub fn make_ui(scale: f32) -> Ui {
    make_ui_parts(scale).0
}

#[doc(hidden)]
pub fn make_ui_parts(scale: f32) -> (Ui, Rc<Intro>, Rc<Login>) {
    let font = Rc::new(Font::parse(REGULAR).expect("Roboto Regular"));
    let intro = Intro::new();
    let login = Login::new();
    let l = login.clone();
    intro.set_on_start(move || l.show());
    // UZ: Saqlangan hisob bor: tanishtiruv va kirish o'tkazib yuboriladi. EN: A saved account exists: the intro and sign-in are skipped. RU: Сохранённый аккаунт есть: интро и вход пропускаются.
    if account::is_authorized() {
        login.resume_done();
    }
    let (i, l) = (intro.clone(), login.clone());
    let mut ui = Ui::new(font, move || if l.started() { l.view() } else { i.view() });
    ui.add_font(700, Rc::new(Font::parse(BOLD).expect("Roboto Bold")));
    ui.add_font(500, Rc::new(Font::parse(MEDIUM).expect("Roboto Medium")));
    let (i, l) = (intro.clone(), login.clone());
    ui.set_back_handler(move || if l.started() { l.back() } else { i.back() });
    ui.set_scale(scale);
    (ui, intro, login)
}

#[cfg(target_os = "android")]
uzcoders::android::android_app!(make_ui);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_quriladi() {
        let mut ui = make_ui(2.0);
        let _ = ui.frame(Size::new(720.0, 1500.0));
    }
}
