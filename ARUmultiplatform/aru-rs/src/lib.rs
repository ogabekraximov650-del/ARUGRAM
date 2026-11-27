//! ARU — media platforma (uzcoders ustida, noldan). Hozir: namuna kontent bilan to'liq interfeys.
//! Haqiqiy ma'lumot Telegram bot va kanal orqali keyin ulanadi (`data.rs` o'rniga).

mod art;
mod create;
mod data;
mod home;
mod icons;
mod kit;
mod me;
mod reels;
mod theme;
mod watch;

use icons::*;
use std::rc::Rc;
use theme::*;
use uzcoders::prelude::*;

static TEXT: &[u8] = include_bytes!("../assets/text.ttf");
static BOLD: &[u8] = include_bytes!("../assets/bold.ttf");
static DISPLAY: &[u8] = include_bytes!("../assets/display.ttf");

#[derive(Clone, Copy, PartialEq)]
pub enum Tab {
    Home,
    Reels,
    Subs,
    Me,
}

/// Ilova holati: hamma ekranlar shu yerdan o'qiydi.
pub struct App {
    pub(crate) tab: State<Tab>,
    pub(crate) watch: State<Option<usize>>,
    pub(crate) create: State<bool>,
    pub(crate) feed: State<usize>,
    pub(crate) liked: State<bool>,
    pub(crate) followed: State<bool>,
    pub(crate) playing: State<bool>,
    pub(crate) pos: State<f32>,
    pub(crate) pos_reel: State<f32>,
    pub(crate) reel: State<usize>,
    pub(crate) scroll_home: ScrollState,
    pub(crate) scroll_subs: ScrollState,
    pub(crate) scroll_watch: ScrollState,
    pub(crate) scroll_me: ScrollState,
}

impl App {
    pub fn new() -> Rc<App> {
        Rc::new(App {
            tab: State::new(Tab::Home),
            watch: State::new(None),
            create: State::new(false),
            feed: State::new(0),
            liked: State::new(false),
            followed: State::new(false),
            playing: State::new(false),
            pos: State::new(0.38),
            pos_reel: State::new(0.38),
            reel: State::new(0),
            scroll_home: ScrollState::new(),
            scroll_subs: ScrollState::new(),
            scroll_watch: ScrollState::new(),
            scroll_me: ScrollState::new(),
        })
    }

    /// Suzib turuvchi pastki menyu: faol bo'lim oq (teskari) plita bo'lib, nomi bilan ko'rinadi.
    fn nav(self: &Rc<Self>, bottom: f32) -> Element {
        let (w, _) = window_dp();
        let cur = self.tab.get();
        let item = |tab: Tab, path: &'static str, label: &'static str| -> Element {
            let a = self.clone();
            let e = if cur == tab {
                row(vec![icon(path, 24.0).color(BG), gap_box(7.0, 0.0), b(label, 13.0, BG)]).center().bg(WHITE).padding_xy(14.0, 0.0)
            } else {
                row(vec![icon(path, 24.0).color(DIM)]).center().width(52.0)
            };
            e.height(50.0).radius(15.0).on_click(move || a.tab.set(tab))
        };
        let c = self.clone();
        let plus = column(vec![icon(PLUS, 28.0).color(BG)]).center().width(54.0).height(54.0).radius(16.0).bg(SIG).on_click(move || c.create.set(true));
        let bar = row(vec![item(Tab::Home, HOME, "Bosh"), item(Tab::Reels, SHORTS, "Oqim"), plus, item(Tab::Subs, SUBS, "Obunalar"), item(Tab::Me, USER, "Profil")])
            .justify(Justify::SpaceBetween)
            .align(Align::Center)
            .width(w - 24.0)
            .height(70.0)
            .padding_xy(9.0, 0.0)
            .bg(S1)
            .border(1.0, LINE)
            .radius(26.0)
            .margin_each(12.0, 0.0, 12.0, 14.0 + bottom);
        column(vec![spacer(), bar])
    }

    pub fn view(self: &Rc<Self>) -> Element {
        let ins = safe_insets();
        let body = match self.tab.get() {
            Tab::Home => self.home_view(ins.top, ins.bottom),
            Tab::Reels => self.reels_view(ins.top, ins.bottom),
            Tab::Subs => self.subs_view(ins.top, ins.bottom),
            Tab::Me => self.me_view(ins.top, ins.bottom),
        };
        let mut layers = vec![column(vec![]).bg(BG), body, self.nav(ins.bottom)];
        if let Some(i) = self.watch.get() {
            layers.push(self.watch_view(i, ins.top, ins.bottom));
        }
        if self.create.get() {
            layers.push(self.create_view(ins.bottom));
        }
        stack(layers)
    }

    /// Orqaga tugmasi: avval oyna, keyin video, keyin bosh bo'lim; hech narsa qolmasa — tizimga qaytaradi.
    pub fn back(&self) -> bool {
        if self.create.get() {
            self.create.set(false);
        } else if self.watch.get().is_some() {
            self.watch.set(None);
        } else if self.tab.get() != Tab::Home {
            self.tab.set(Tab::Home);
        } else {
            return false;
        }
        true
    }
}

/// Interfeysni quradi; testlar uchun ilova holatini ham qaytaradi.
pub fn build(scale: f32) -> (Ui, Rc<App>) {
    let app = App::new();
    let a = app.clone();
    let mut ui = Ui::new(Rc::new(Font::parse(TEXT).expect("matn shrifti")), move || a.view());
    ui.add_font(700, Rc::new(Font::parse(BOLD).expect("qalin shrift")));
    ui.add_font(900, Rc::new(Font::parse(DISPLAY).expect("sarlavha shrifti")));
    ui.set_scale(scale);
    let a = app.clone();
    ui.set_back_handler(move || a.back());
    (ui, app)
}

/// Kirish nuqtasi (`scale` — ekran zichligi).
pub fn make_ui(scale: f32) -> Ui {
    build(scale).0
}

#[cfg(target_os = "android")]
uzcoders::android::android_app!(make_ui);

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(cmds: &[DrawCmd]) -> Vec<String> {
        cmds.iter()
            .filter_map(|c| match c {
                DrawCmd::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    /// `ARU_SNAPSHOT_DIR` berilsa kadrni PNG qilib saqlaydi (ko'z bilan tekshirish uchun).
    fn snap(ui: &mut Ui, name: &str) {
        let cmds = ui.frame(Size::new(780.0, 1688.0));
        if let Ok(dir) = std::env::var("ARU_SNAPSHOT_DIR") {
            let mut c = ui.canvas(780, 1688);
            c.clear(Color::BLACK);
            c.execute(&cmds);
            std::fs::write(format!("{dir}/{name}.png"), c.to_png()).unwrap();
        }
    }

    fn has(ui: &mut Ui, what: &str) -> bool {
        texts(&ui.frame(Size::new(780.0, 1688.0))).iter().any(|t| t.contains(what))
    }

    #[test]
    fn hamma_ekranlar_chiziladi() {
        let (mut ui, app) = build(2.0);
        assert!(has(&mut ui, "SIZ UCHUN"));
        snap(&mut ui, "1_bosh");
        app.tab.set(Tab::Reels);
        assert!(has(&mut ui, "TONG"));
        snap(&mut ui, "2_oqim");
        app.tab.set(Tab::Subs);
        assert!(has(&mut ui, "OBUNALAR"));
        snap(&mut ui, "3_obunalar");
        app.tab.set(Tab::Me);
        assert!(has(&mut ui, "KUNDALIGI"));
        snap(&mut ui, "4_profil");
        app.tab.set(Tab::Home);
        app.watch.set(Some(0));
        assert!(has(&mut ui, "Vaqt bo'yicha") || has(&mut ui, "VAQT"));
        snap(&mut ui, "5_korish");
        app.watch.set(None);
        app.create.set(true);
        assert!(has(&mut ui, "YARATISH"));
        snap(&mut ui, "6_yaratish");
    }

    #[test]
    fn orqaga_tugmasi_tartibi() {
        let (_ui, app) = build(1.0);
        app.tab.set(Tab::Me);
        app.watch.set(Some(1));
        app.create.set(true);
        assert!(app.back());
        assert!(!app.create.get() && app.watch.get().is_some());
        assert!(app.back());
        assert!(app.watch.get().is_none() && app.tab.get() == Tab::Me);
        assert!(app.back());
        assert!(app.tab.get() == Tab::Home);
        assert!(!app.back());
    }

    #[test]
    fn video_bosilsa_ochiladi() {
        let (mut ui, app) = build(2.0);
        let cmds = ui.frame(Size::new(780.0, 1688.0));
        let (x, y) = cmds
            .iter()
            .find_map(|c| match c {
                DrawCmd::Text { x, baseline, text, size, .. } if text.starts_with("Qirq kun") => Some((*x + 4.0, *baseline - *size * 0.3)),
                _ => None,
            })
            .expect("sarlavha topilmadi");
        ui.pointer(PointerEvent::new(PointerKind::Down, x, y));
        ui.pointer(PointerEvent::new(PointerKind::Up, x, y));
        assert_eq!(app.watch.get(), Some(0));
    }
}
