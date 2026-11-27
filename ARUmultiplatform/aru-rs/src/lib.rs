//! ARU — media platforma (uzcoders ustida, noldan): videolar Telegram kanalida, ro'yxat Supabase'da, worker `aru-api` orqali.

mod app;
mod create;
mod feed;
mod home;
mod icons;
mod ident;
mod json;
mod kit;
mod media;
mod model;
mod net;
mod profile;
mod reels;
mod theme;
mod thumbs;
mod watch;

pub use app::{App, Tab};
use icons::*;
use std::rc::Rc;
use theme::*;
use uzcoders::prelude::*;

static TEXT: &[u8] = include_bytes!("../assets/text.ttf");
static BOLD: &[u8] = include_bytes!("../assets/bold.ttf");
static DISPLAY: &[u8] = include_bytes!("../assets/display.ttf");

impl App {
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
        if self.me.with(|m| m.is_none()) {
            return self.onboard_view(ins.top);
        }
        self.take_picked();
        self.poll_upload();
        self.sync_player();
        // Video ochiq bo'lsa orqadagi sahifalar qurilmaydi (kadr tez bo'lsin).
        if let Some(v) = self.watch.get() {
            return self.watch_view(&v, ins.top, ins.bottom);
        }
        let body = match self.tab.get() {
            Tab::Home => self.home_view(ins.top, ins.bottom),
            Tab::Reels => self.reels_view(ins.top, ins.bottom),
            Tab::Subs => self.subs_view(ins.top, ins.bottom),
            Tab::Me => self.me_view(ins.top, ins.bottom),
        };
        let mut layers = vec![column(vec![]).bg(BG), body];
        if let Some(b) = self.upload_banner(ins.bottom) {
            layers.push(b);
        }
        layers.push(self.nav(ins.bottom));
        if self.search_on.get() {
            layers.push(self.search_view(ins.top, ins.bottom));
        }
        if self.create.get() {
            layers.push(self.create_view(ins.bottom));
        }
        if self.pending.get().is_some() {
            layers.push(self.title_view(ins.bottom));
        }
        stack(layers)
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
mod tests;
