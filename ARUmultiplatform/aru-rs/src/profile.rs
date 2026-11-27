//! Profil (o'z kanalim): oq plita (nom va raqamlar), yuklagan videolarim.

use crate::app::{App, Tab};
use crate::icons::*;
use crate::kit::*;
use crate::model::*;
use crate::theme::*;
use uzcoders::prelude::*;

fn stat(n: &str, label: &str) -> Element {
    column(vec![disp(n, 26.0, BG), b(label, 10.0, BG).margin_each(0.0, 4.0, 0.0, 0.0)]).weight(1.0)
}

impl App {
    pub fn me_view(self: &Rc<Self>, top: f32, bottom: f32) -> Element {
        let (w, _) = window_dp();
        let cw = w - 24.0;
        self.load_mine();
        let name = self.me.with(|m| m.as_ref().map(|m| m.name.clone()).unwrap_or_default());
        let c = self.mine.get().unwrap_or_default();
        let slab = column(vec![
            row(vec![avatar(&initial(&name), 80.0, Tone::Dark), gap_box(14.0, 0.0), column(vec![disp(&short(&name, 14), 36.0, BG)]).weight(1.0)]).align(Align::Center),
            row(vec![
                stat(&compact(c.followers), "kuzatuvchi"),
                column(vec![]).width(2.0).bg(BG),
                stat(&c.videos.to_string(), "video").padding_each(12.0, 0.0, 0.0, 0.0),
                column(vec![]).width(2.0).bg(BG),
                stat(&compact(c.views), "ko'rish").padding_each(12.0, 0.0, 0.0, 0.0),
            ])
            .align(Align::Stretch)
            .width(cw - 32.0)
            .margin_each(0.0, 18.0, 0.0, 0.0),
        ])
        .padding(16.0)
        .width(cw)
        .radius(30.0)
        .bg(WHITE)
        .margin_each(12.0, top + 12.0, 12.0, 12.0);
        let (a, s) = (self.clone(), self.clone());
        let upload = row(vec![
            column(vec![icon(VIDEO, 22.0).color(BG), b("Video", 14.0, BG)]).center().height(52.0).radius(16.0).bg(WHITE).weight(1.0).on_click(move || a.pick("video")),
            column(vec![icon(SHORTS, 22.0).color(WHITE), b("Oqim", 14.0, WHITE)]).center().height(52.0).radius(16.0).bg(S2).weight(1.0).on_click(move || s.pick("reel")),
        ])
        .gap(8.0)
        .width(cw)
        .margin_each(12.0, 0.0, 12.0, 14.0);
        let g = self.clone();
        let empty_el = empty(VIDEO, "Hali video yo'q", "Yuqoridagi tugma bilan birinchi videoni yuklang", Some(("Yuklash", Rc::new(move || g.create.set(true)))));
        let _ = Tab::Me;
        self.feed_view(&self.f_mine, "mine", self.q_mine(), &self.sc_me, vec![slab, upload, disp("Videolarim", 22.0, WHITE).margin_each(16.0, 4.0, 16.0, 12.0)], empty_el, false, bottom)
    }
}
