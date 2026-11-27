//! "Yaratish" oynasi, sarlavha so'rovi, yuklash holati va birinchi kirish (kanal nomi).

use crate::app::App;
use crate::icons::*;
use crate::kit::*;
use crate::theme::*;
use uzcoders::prelude::*;

fn tile(path: &'static str, title: &str, sub: &str, white: bool) -> Element {
    let (bg, fg, sc, ib, ifg) = if white { (WHITE, BG, Color::hex(0x444444), BG, WHITE) } else { (S2, WHITE, MUTE, WHITE, BG) };
    column(vec![
        column(vec![icon(path, 24.0).color(ifg)]).center().width(46.0).height(46.0).radius(14.0).bg(ib),
        spacer(),
        disp(title, 25.0, fg),
        t(sub, 12.0, sc).margin_each(0.0, 5.0, 0.0, 0.0),
    ])
    .padding(14.0)
    .height(150.0)
    .radius(24.0)
    .bg(bg)
}

fn sheet(kids: Vec<Element>, bottom: f32) -> Element {
    column(kids).padding_each(14.0, 10.0, 14.0, 14.0).bg(Color::hex(0x0B0B0B)).border(1.0, LINE).radius(28.0).margin_each(8.0, 0.0, 8.0, 8.0 + bottom)
}

fn handle() -> Element {
    row(vec![spacer(), column(vec![]).width(40.0).height(4.0).radius(2.0).bg(Color::hex(0x444444)), spacer()])
}

impl App {
    /// "Yaratish": video yoki oqim (tik qisqa video) yuklash.
    pub fn create_view(self: &Rc<Self>, bottom: f32) -> Element {
        let (a, c, d, e) = (self.clone(), self.clone(), self.clone(), self.clone());
        let head = row(vec![disp("Yaratish", 40.0, WHITE), spacer(), column(vec![icon(CLOSE, 22.0).color(WHITE)]).center().width(44.0).height(44.0).radius(14.0).bg(S2).on_click(move || c.create.set(false))])
            .align(Align::Center)
            .margin_each(0.0, 14.0, 0.0, 14.0);
        let tiles = grid(2, 8.0, vec![tile(VIDEO, "Video", "Telefondan video tanlang", true).on_click(move || d.pick("video")), tile(SHORTS, "Oqim", "Tik qisqa video", false).on_click(move || e.pick("reel"))]);
        stack(vec![column(vec![]).bg(Color::rgba(0, 0, 0, 170)).on_click(move || a.create.set(false)), column(vec![spacer(), sheet(vec![handle(), head, tiles], bottom)])])
    }

    /// Tanlangan video uchun sarlavha so'raydi.
    pub fn title_view(self: &Rc<Self>, bottom: f32) -> Element {
        let (a, b2, c) = (self.clone(), self.clone(), self.clone());
        let reel = self.up_kind.get() == "reel";
        let err = self.msg.get();
        let mut kids = vec![
            handle(),
            disp(if reel { "Oqim sarlavhasi" } else { "Video sarlavhasi" }, 30.0, WHITE).margin_each(0.0, 14.0, 0.0, 12.0),
            input(&self.title_text, "Sarlavha yozing").on_submit(move || c.start_upload()),
        ];
        if !err.is_empty() {
            kids.push(t(err, 12.5, Color::hex(0xF87171)).margin_each(0.0, 8.0, 0.0, 0.0));
        }
        kids.push(
            row(vec![
                column(vec![b("Bekor", 15.0, WHITE)]).center().height(48.0).radius(15.0).bg(S2).weight(1.0).on_click(move || {
                    a.pending.set(None);
                    a.msg.set(String::new());
                }),
                column(vec![b("Yuklash", 15.0, BG)]).center().height(48.0).radius(15.0).bg(SIG).weight(1.0).on_click(move || b2.start_upload()),
            ])
            .gap(8.0)
            .margin_each(0.0, 14.0, 0.0, 0.0),
        );
        let close = self.clone();
        stack(vec![column(vec![]).bg(Color::rgba(0, 0, 0, 170)).on_click(move || close.pending.set(None)), column(vec![spacer(), sheet(kids, bottom)])])
    }

    /// Yuklash holati (pastki menyu ustida).
    pub fn upload_banner(self: &Rc<Self>, bottom: f32) -> Option<Element> {
        let u = self.up.get();
        let (w, _) = window_dp();
        let (label, color, bar) = match u.state.as_str() {
            "run" => {
                let p = if u.total > 0 { (u.sent as f32 / u.total as f32).clamp(0.0, 1.0) } else { 0.0 };
                (format!("Yuklanmoqda · {}%", (p * 100.0) as i32), WHITE, Some(p))
            }
            "done" => ("Yuklandi".to_string(), WHITE, Some(1.0)),
            "error" => (format!("Yuklanmadi: {}", short(&u.err, 60)), Color::hex(0xF87171), None),
            _ => return None,
        };
        let a = self.clone();
        let running = u.state == "run";
        let mut kids = vec![row(vec![
            column(vec![b(label, 13.5, color)]).weight(1.0),
            column(vec![icon(CLOSE, 18.0).color(DIM)]).center().width(32.0).height(32.0).on_click(move || {
                if a.up.get().state == "run" {
                    a.cancel_upload();
                } else {
                    a.up.set(Default::default());
                }
            }),
        ])
        .align(Align::Center)];
        if let Some(p) = bar {
            let bw = w - 24.0 - 28.0;
            kids.push(column(vec![column(vec![]).width((bw * p).max(2.0)).height(4.0).radius(2.0).bg(SIG)]).width(bw).height(4.0).radius(2.0).bg(S3).margin_each(0.0, 6.0, 0.0, 0.0));
        }
        let _ = running;
        let card = column(kids).padding_each(14.0, 10.0, 6.0, 12.0).bg(S1).border(1.0, LINE).radius(20.0).margin_each(12.0, 0.0, 12.0, 14.0 + 70.0 + 10.0 + bottom);
        Some(column(vec![spacer(), card]))
    }

    /// Birinchi kirish: kanal nomi.
    pub fn onboard_view(self: &Rc<Self>, top: f32) -> Element {
        let a = self.clone();
        let b2 = self.clone();
        let err = self.msg.get();
        let mut kids = vec![
            logo(72.0),
            disp("Xush kelibsiz", 46.0, WHITE).margin_each(0.0, 22.0, 0.0, 0.0),
            t("Kanalingiz nomini yozing. Videolar, fikrlar va obunalar shu nom bilan bo'ladi.", 14.0, MUTE).margin_each(0.0, 10.0, 0.0, 22.0),
            input(&self.name_text, "Kanal nomi").on_submit(move || b2.register()),
        ];
        if !err.is_empty() {
            kids.push(t(err, 12.5, Color::hex(0xF87171)).margin_each(0.0, 8.0, 0.0, 0.0));
        }
        let busy = self.busy.get();
        kids.push(
            column(vec![if busy { spinner(24.0, 2.5, BG) } else { b("Boshlash", 16.0, BG) }])
                .center()
                .height(52.0)
                .radius(16.0)
                .bg(WHITE)
                .margin_each(0.0, 16.0, 0.0, 0.0)
                .on_click(move || a.register()),
        );
        column(vec![column(kids).padding_each(24.0, top + 80.0, 24.0, 0.0)]).bg(BG)
    }
}
