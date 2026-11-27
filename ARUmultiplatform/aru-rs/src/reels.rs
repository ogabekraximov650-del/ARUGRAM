//! Oqim: to'liq ekranli tik video; amallar pastda bir qatorda, fikrlar rasm ustida suzib yuradi.

use crate::art::{art, Art};
use crate::data::*;
use crate::icons::*;
use crate::kit::*;
use crate::theme::*;
use crate::App;
use std::rc::Rc;
use uzcoders::prelude::*;

fn bubble(letter: &str, who: &str, text: &str, tone: Tone) -> Element {
    row(vec![avatar(letter, 30.0, tone), gap_box(8.0, 0.0), column(vec![t(who, 11.0, DIM), b(text, 13.0, WHITE)])])
        .align(Align::Center)
        .bg(Color::rgba(0, 0, 0, 150))
        .radius(16.0)
        .padding_each(6.0, 6.0, 14.0, 6.0)
}

impl App {
    pub fn reels_view(self: &Rc<Self>, top: f32, bottom: f32) -> Element {
        let (w, _) = window_dp();
        let r = REELS[self.reel.get() % REELS.len()];
        let liked = self.liked.get();
        let (a, l) = (self.clone(), self.clone());
        let tabs = row(vec![disp("Kuzatilgan", 24.0, Color::rgba(255, 255, 255, 140)), disp("Oqim", 24.0, WHITE)]).gap(18.0);
        let bar = row(vec![tabs, spacer(), icon_btn(SEARCH)]).align(Align::Center).padding_each(16.0, top + 8.0, 8.0, 0.0);
        let follow = column(vec![b("Kuzatish", 13.0, BG)]).center().height(30.0).padding_xy(13.0, 0.0).radius(10.0).bg(WHITE);
        let who = row(vec![avatar("S", 38.0, Tone::Light), gap_box(10.0, 0.0), b("@sayohat_kundaligi", 15.5, WHITE), gap_box(10.0, 0.0), follow]).align(Align::Center);
        let iw = w - 32.0;
        let secs = 45.0 * self.pos_reel.get();
        let info = column(vec![
            who,
            disp(r.title, 36.0, WHITE).margin_each(0.0, 12.0, 0.0, 0.0),
            t(format!("{} · {}", r.place, r.tags), 13.5, DIM).margin_each(0.0, 8.0, 0.0, 14.0),
            ruler(&self.pos_reel, 48, 14.0, iw),
            row(vec![b(clock(secs), 11.5, WHITE), spacer(), t("0:45", 11.5, MUTE)]).width(iw).margin_each(0.0, 4.0, 0.0, 12.0),
            row(vec![
                wide_key(HEART, "248K", liked).on_click(move || l.liked.set(!l.liked.get())),
                wide_key(COMMENT, "3 412", false),
                wide_key(SHARE, "Ulashish", false).on_click(|| uzcoders::android::toast("Ulashish tez orada")),
                wide_key(SAVE, "Saqlash", false).on_click(|| uzcoders::android::toast("Saqlandi")),
            ])
            .gap(8.0)
            .width(iw),
        ])
        .padding_each(16.0, 80.0, 16.0, 98.0 + bottom)
        .gradient(Color::rgba(0, 0, 0, 0), Color::rgba(0, 0, 0, 215));
        let bubbles = column(vec![row(vec![bubble("M", "@malika.travel", "Bu joy qayerda?", Tone::Dark), spacer()]), row(vec![bubble("J", "@jasur_1995", "Manzara a'lo!", Tone::Signal), spacer()]).margin_each(48.0, 10.0, 0.0, 0.0)]).padding_each(16.0, 0.0, 16.0, 16.0);
        stack(vec![
            image_fill(&art(Art::Dawn)).on_click(move || a.reel.set(a.reel.get() + 1)),
            column(vec![bar, spacer(), bubbles, info]),
        ])
    }
}
