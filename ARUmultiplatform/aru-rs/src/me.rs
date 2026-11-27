//! Profil (kanal): tepada oq plita (qora yozuv), pastda aralash mozaika.

use crate::art::Art;
use crate::icons::*;
use crate::kit::*;
use crate::theme::*;
use crate::App;
use std::rc::Rc;
use uzcoders::prelude::*;

fn tile(a: Art, w: f32, h: f32, ic: &'static str, label: &str) -> Element {
    let chip = row(vec![icon(ic, 13.0).color(BG), gap_box(4.0, 0.0), b(label, 11.5, BG)]).align(Align::Center).bg(WHITE).radius(8.0).padding_each(6.0, 5.0, 8.0, 5.0);
    pic(a, 18.0, vec![column(vec![row(vec![chip, spacer()])]).padding(8.0)]).width(w).height(h)
}

fn stat(n: &str, label: &str) -> Element {
    column(vec![disp(n, 26.0, BG), b(label, 10.0, BG).margin_each(0.0, 4.0, 0.0, 0.0)]).weight(1.0)
}

impl App {
    pub fn me_view(self: &Rc<Self>, top: f32, bottom: f32) -> Element {
        let (w, _) = window_dp();
        let cw = w - 24.0;
        let plate = |p: &'static str| column(vec![icon(p, 22.0).color(WHITE)]).center().width(40.0).height(40.0).radius(13.0).bg(BG);
        let f = self.clone();
        let followed = self.followed.get();
        let slab = column(vec![
            row(vec![spacer(), plate(MORE)]),
            row(vec![avatar("S", 80.0, Tone::Dark), gap_box(14.0, 0.0), column(vec![disp("Sayohat", 44.0, BG), disp("Kundaligi", 44.0, BG)])]).align(Align::Center).margin_each(0.0, 6.0, 0.0, 14.0),
            row(vec![stat("1,24 mln", "kuzatuvchi"), column(vec![]).width(2.0).bg(BG), stat("342", "video").padding_each(12.0, 0.0, 0.0, 0.0), column(vec![]).width(2.0).bg(BG), stat("58 mln", "ko'rish").padding_each(12.0, 0.0, 0.0, 0.0)])
                .align(Align::Stretch)
                .width(cw - 32.0),
        ])
        .padding(16.0)
        .width(cw)
        .radius(30.0)
        .bg(WHITE);
        let act = row(vec![
            column(vec![b(if followed { "Kuzatilmoqda" } else { "Kuzatish" }, 15.0, BG)]).center().height(46.0).radius(15.0).bg(if followed { DIM } else { SIG }).weight(1.0).on_click(move || f.followed.set(!f.followed.get())),
            column(vec![icon(BELL, 22.0).color(WHITE)]).center().width(46.0).height(46.0).radius(15.0).bg(S2),
            column(vec![icon(SHARE, 21.0).color(WHITE)]).center().width(46.0).height(46.0).radius(15.0).bg(S2),
        ])
        .gap(8.0)
        .width(cw)
        .margin_each(0.0, 12.0, 0.0, 12.0);
        let pills = row(vec![pill("Hammasi", true), pill("Videolar", false), pill("Oqim", false), pill("Rasmlar", false)]).gap(6.0);
        let c = (cw - 8.0) / 3.0;
        let big = 2.0 * c + 4.0;
        let grid1 = row(vec![tile(Art::Mountain, big, big, PLAY, "24:01"), column(vec![tile(Art::Dawn, c, c, SHORTS, "1,8 mln"), tile(Art::Dunes, c, c, PHOTO, "2 458")]).gap(4.0)]).gap(4.0);
        let grid2 = row(vec![tile(Art::City, c, c, PLAY, "18:07"), tile(Art::Forest, c, c, PLAY, "09:18"), tile(Art::Food, c, c, PLAY, "15:42")]).gap(4.0).margin_each(0.0, 4.0, 0.0, 0.0);
        let kids = vec![column(vec![slab, act, pills, gap_box(0.0, 12.0), grid1, grid2, gap_box(0.0, 110.0 + bottom)]).padding_each(12.0, top + 8.0, 12.0, 0.0)];
        scroll_column(&self.scroll_me, kids).grow(1.0)
    }
}
