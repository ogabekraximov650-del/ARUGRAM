//! Bosh sahifa: aralash mozaika (video, oqim, rasm, post) va qolgan videolar ro'yxati; "Obunalar" bo'limi.

use crate::art::Art;
use crate::data::*;
use crate::icons::*;
use crate::kit::*;
use crate::theme::*;
use crate::{App, Tab};
use std::rc::Rc;
use uzcoders::prelude::*;

impl App {
    /// Tanlangan yorliq bo'yicha videolar tartibi.
    fn order(&self) -> Vec<usize> {
        match self.feed.get() {
            1 => (0..VIDEOS.len()).filter(|&i| VIDEOS[i].ch != CHANNELS[2]).collect(),
            2 => (0..VIDEOS.len()).rev().collect(),
            _ => (0..VIDEOS.len()).collect(),
        }
    }

    fn open(self: &Rc<Self>, i: usize) -> impl Fn() + 'static {
        let a = self.clone();
        move || {
            a.playing.set(false);
            a.watch.set(Some(i));
        }
    }

    /// Katta karta: rasm, o'yilgan burchakda sarlavha va kanal.
    fn feature(self: &Rc<Self>, i: usize, cw: f32) -> Element {
        let v = VIDEOS[i];
        let nw = (cw - 80.0).min(272.0);
        let info = row(vec![
            avatar(&initial(v.ch), 36.0, Tone::Light),
            gap_box(10.0, 0.0),
            column(vec![b(short(v.title, 46), 15.5, WHITE), t(format!("{} · {}", v.ch, v.views), 12.0, MUTE).margin_each(0.0, 3.0, 0.0, 0.0)]).weight(1.0),
        ])
        .padding_each(0.0, 13.0, 14.0, 0.0);
        let tag = column(vec![row(vec![spacer(), column(vec![b(v.dur, 12.5, BG)]).center().height(30.0).padding_xy(10.0, 0.0).radius(9.0).bg(WHITE)])]).padding(12.0);
        let play = column(vec![spacer(), row(vec![spacer(), column(vec![icon(PLAY, 26.0).color(BG)]).center().width(54.0).height(54.0).radius(16.0).bg(WHITE)])]).padding(12.0);
        pic(v.art, 26.0, vec![tag, play, at_bottom_left(notch(nw, 104.0, 24.0, info))])
            .width(cw)
            .height(288.0)
            .margin_each(12.0, 0.0, 12.0, 0.0)
            .on_click(self.open(i))
    }

    /// Oddiy karta: katta muqova, ostida kanal belgisi, sarlavha va ma'lumot.
    fn card(self: &Rc<Self>, i: usize, cw: f32) -> Element {
        let v = VIDEOS[i];
        let cover = pic(v.art, 22.0, vec![dur_tab(v.dur)]).width(cw).height(cw * 0.5625);
        let info = row(vec![
            avatar(&initial(v.ch), 38.0, Tone::Light),
            gap_box(12.0, 0.0),
            column(vec![b(short(v.title, 60), 15.5, WHITE), t(format!("{} · {} · {}", v.ch, v.views, v.ago), 12.5, MUTE).margin_each(0.0, 3.0, 0.0, 0.0)]).weight(1.0),
        ])
        .padding_each(2.0, 12.0, 2.0, 0.0);
        column(vec![cover, info]).width(cw).margin_each(12.0, 0.0, 12.0, 18.0).on_click(self.open(i))
    }

    fn chip_on_pic(icon_path: &'static str, label: &str) -> Element {
        let chip = row(vec![icon(icon_path, 13.0).color(BG), gap_box(4.0, 0.0), b(label, 12.0, BG)]).align(Align::Center).bg(WHITE).radius(9.0).padding_each(7.0, 6.0, 9.0, 6.0);
        column(vec![row(vec![chip, spacer()])]).padding(10.0)
    }

    fn caption(title: &str, sub: &str) -> Element {
        column(vec![spacer(), column(vec![b(title, 13.5, WHITE), t(sub, 11.5, DIM)]).padding_each(12.0, 36.0, 12.0, 10.0).gradient(Color::rgba(0, 0, 0, 0), Color::rgba(0, 0, 0, 200))])
    }

    /// Mozaika: tik oqim kartasi, rasm va matnli post.
    fn mosaic(self: &Rc<Self>, cw: f32) -> Element {
        let hw = (cw - 8.0) / 2.0;
        let a = self.clone();
        let reel = pic(Art::Dawn, 22.0, vec![Self::chip_on_pic(SHORTS, "1,8 mln"), Self::caption("Tong qarshilash", "@sayohat_kundaligi")])
            .width(hw)
            .height(282.0)
            .on_click(move || a.tab.set(Tab::Reels));
        let photo = pic(Art::Dunes, 22.0, vec![Self::chip_on_pic(PHOTO, "2 458"), Self::caption("Qizilqum quyoshi", "@oshxona_sirlari")]).width(hw).height(137.0);
        let post = column(vec![
            row(vec![icon(POST, 15.0).color(BG), gap_box(5.0, 0.0), b("Post", 12.0, BG)]).align(Align::Center),
            disp("Rejasiz sayohat — eng yaxshisi", 22.0, BG).margin_each(0.0, 8.0, 0.0, 0.0),
            spacer(),
            b("@sardor_k", 11.5, BG),
        ])
        .padding(12.0)
        .width(hw)
        .height(137.0)
        .radius(22.0)
        .bg(WHITE);
        row(vec![reel, column(vec![photo, post]).gap(8.0)]).gap(8.0).margin_each(12.0, 0.0, 12.0, 18.0)
    }

    pub fn home_view(self: &Rc<Self>, top: f32, bottom: f32) -> Element {
        let (w, _) = window_dp();
        let cw = w - 24.0;
        let head = row(vec![
            logo(38.0),
            spacer(),
            icon_btn(SEARCH).on_click(|| uzcoders::android::toast("Qidiruv tez orada")),
            stack(vec![icon_btn(BELL), column(vec![]).width(9.0).height(9.0).radius(5.0).bg(SIG).border(2.0, BG).abs().margin_each(25.0, 6.0, 0.0, 0.0)]).width(42.0).height(42.0),
            gap_box(4.0, 0.0),
            avatar("A", 34.0, Tone::Light),
        ])
        .align(Align::Center)
        .padding_each(16.0, top + 6.0, 14.0, 0.0);
        let cur = self.feed.get();
        let tabs = row(["Siz uchun", "Obunalar", "Yangi"]
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let a = self.clone();
                disp(s, 26.0, if i == cur { WHITE } else { Color::hex(0x555555) }).on_click(move || a.feed.set(i))
            })
            .collect())
        .gap(20.0)
        .padding_each(16.0, 12.0, 16.0, 12.0);
        let order = self.order();
        let mut kids: Vec<Element> = Vec::new();
        if let Some(&first) = order.first() {
            kids.push(self.feature(first, cw).margin_each(12.0, 0.0, 12.0, 14.0));
        }
        if cur == 0 {
            kids.push(self.mosaic(cw));
        }
        for &i in order.iter().skip(1) {
            kids.push(self.card(i, cw));
        }
        kids.push(gap_box(0.0, 110.0 + bottom));
        column(vec![head, tabs, scroll_column(&self.scroll_home, kids).grow(1.0)])
    }

    /// Obunalar: kanallar qatori va ularning yangi videolari.
    pub fn subs_view(self: &Rc<Self>, top: f32, bottom: f32) -> Element {
        let (w, _) = window_dp();
        let cw = w - 24.0;
        let chans: Vec<Element> = CHANNELS
            .iter()
            .map(|c| column(vec![avatar(&initial(c), 56.0, Tone::Light), t(short(c, 12), 11.5, DIM).margin_each(0.0, 6.0, 0.0, 0.0)]).align(Align::Center))
            .collect();
        let mut kids = vec![row(chans).gap(18.0).padding_each(16.0, 4.0, 16.0, 16.0)];
        for i in (0..VIDEOS.len()).filter(|&i| VIDEOS[i].ch != CHANNELS[2]) {
            kids.push(self.card(i, cw));
        }
        kids.push(gap_box(0.0, 110.0 + bottom));
        column(vec![disp("Obunalar", 40.0, WHITE).padding_each(16.0, top + 14.0, 16.0, 10.0), scroll_column(&self.scroll_subs, kids).grow(1.0)])
    }
}
