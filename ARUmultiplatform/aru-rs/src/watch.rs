//! Video ko'rish: pleyer o'rni, vaqt chizg'ichi (fikrlar shu joyga qadalgan), amallar, vaqtga bog'langan fikrlar.

use crate::data::*;
use crate::icons::*;
use crate::kit::*;
use crate::theme::*;
use crate::App;
use std::rc::Rc;
use uzcoders::prelude::*;

impl App {
    /// Chizg'ich: fikr egalarining belgilari o'z soniyasida turadi, bosilsa video shu joyga o'tadi.
    fn timeline(self: &Rc<Self>, w: f32) -> Element {
        let p = self.pos.get();
        let mut kids: Vec<Element> = vec![column(vec![spacer(), ruler(&self.pos, 49, 14.0, w)])];
        for n in NOTES.iter() {
            let (a, f) = (self.clone(), n.frac);
            let near = (p - n.frac).abs() < 0.07;
            kids.push(
                column(vec![avatar(n.letter, 22.0, if near { Tone::Signal } else { Tone::Dark })])
                    .abs()
                    .margin_each((n.frac * w - 11.0).max(0.0), 0.0, 0.0, 0.0)
                    .on_click(move || a.pos.set(f)),
            );
        }
        kids.push(column(vec![]).width(3.0).height(26.0).radius(2.0).bg(SIG).abs().margin_each((p * w - 1.0).max(0.0), 50.0, 0.0, 0.0));
        stack(kids).width(w).height(76.0)
    }

    fn note_row(self: &Rc<Self>, n: &Note, w: f32) -> Element {
        let (a, f) = (self.clone(), n.frac);
        row(vec![
            column(vec![b(n.at, 11.5, BG)]).center().height(26.0).padding_xy(8.0, 0.0).radius(8.0).bg(SIG),
            gap_box(10.0, 0.0),
            column(vec![t(n.who, 12.0, MUTE), t(n.text, 13.5, WHITE).margin_each(0.0, 1.0, 0.0, 0.0)]).weight(1.0),
            row(vec![icon(LIKE, 15.0).color(MUTE), gap_box(4.0, 0.0), t(n.likes, 12.0, MUTE)]).align(Align::Center),
        ])
        .width(w)
        .padding_each(0.0, 0.0, 0.0, 12.0)
        .on_click(move || a.pos.set(f))
    }

    fn mini(self: &Rc<Self>, i: usize, w: f32) -> Element {
        let v = VIDEOS[i];
        let a = self.clone();
        column(vec![
            crate::kit::pic(v.art, 16.0, vec![dur_tab(v.dur)]).width(w).height(w * 0.5625),
            b(short(v.title, 38), 13.0, WHITE).margin_each(0.0, 7.0, 0.0, 0.0),
            t(format!("{} · {}", v.ch, v.views), 11.5, MUTE),
        ])
        .width(w)
        .on_click(move || {
            a.pos.set(0.0);
            a.watch.set(Some(i));
        })
    }

    pub fn watch_view(self: &Rc<Self>, i: usize, top: f32, bottom: f32) -> Element {
        let v = VIDEOS[i];
        let (w, _) = window_dp();
        let (a, p, f, l) = (self.clone(), self.clone(), self.clone(), self.clone());
        let playing = self.playing.get();
        let total = dur_secs(v.dur);
        let back = column(vec![icon(CHEVRON_DOWN, 26.0).color(WHITE)]).center().width(44.0).height(44.0).on_click(move || a.watch.set(None));
        let toggle = column(vec![icon(if playing { PAUSE } else { PLAY }, 30.0).color(BG)])
            .center()
            .width(60.0)
            .height(60.0)
            .radius(18.0)
            .bg(WHITE)
            .on_click(move || p.playing.set(!p.playing.get()));
        let time = at_bottom_left(notch(110.0, 32.0, 16.0, column(vec![b(format!("{} / {}", clock(total * self.pos.get()), v.dur), 12.5, WHITE)]).center()));
        let player = crate::kit::pic(v.art, 0.0, vec![column(vec![row(vec![back, spacer()]).padding_each(6.0, 4.0, 0.0, 0.0)]), column(vec![toggle]).center(), time]).width(w).height(w * 0.5625);
        let cw = w - 28.0;
        let followed = self.followed.get();
        let follow = column(vec![b(if followed { "Kuzatilmoqda" } else { "Kuzatish" }, 14.0, if followed { WHITE } else { BG })])
            .center()
            .height(38.0)
            .padding_xy(18.0, 0.0)
            .radius(12.0)
            .bg(if followed { S3 } else { WHITE })
            .on_click(move || f.followed.set(!f.followed.get()));
        let channel = row(vec![
            avatar(&initial(v.ch), 40.0, Tone::Light),
            gap_box(10.0, 0.0),
            column(vec![b(v.ch, 14.0, WHITE), t("1,24 mln kuzatuvchi", 12.0, MUTE)]).weight(1.0),
            follow,
        ])
        .align(Align::Center)
        .width(cw)
        .margin_each(0.0, 12.0, 0.0, 0.0);
        let liked = self.liked.get();
        let keys = row(vec![
            key(LIKE, v.likes, liked).on_click(move || l.liked.set(!l.liked.get())),
            key(DISLIKE, "Yoqmadi", false),
            key(SHARE, "Ulashish", false).on_click(|| uzcoders::android::toast("Ulashish tez orada")),
            key(DOWNLOAD, "Yuklash", false).on_click(|| uzcoders::android::toast("Yuklash tez orada")),
            key(SAVE, "Saqlash", false),
        ])
        .gap(7.0)
        .width(cw)
        .margin_each(0.0, 12.0, 0.0, 0.0);
        let mut body: Vec<Element> = vec![
            self.timeline(cw).margin_each(14.0, 8.0, 14.0, 0.0),
            column(vec![
                b(v.title, 17.0, WHITE),
                t(format!("{} ko'rish · {}", v.views, v.ago), 12.5, MUTE).margin_each(0.0, 3.0, 0.0, 0.0),
                channel,
                keys,
                row(vec![disp("Vaqt bo'yicha fikrlar", 21.0, WHITE), spacer(), t(format!("{} ta", NOTES.len()), 12.5, MUTE)]).align(Align::Center).width(cw).margin_each(0.0, 20.0, 0.0, 10.0),
            ])
            .padding_each(14.0, 10.0, 14.0, 0.0),
        ];
        for n in NOTES.iter().take(3) {
            body.push(self.note_row(n, cw).margin_each(14.0, 0.0, 14.0, 0.0));
        }
        let mw = (cw - 10.0) / 2.0;
        let next: Vec<usize> = (0..VIDEOS.len()).filter(|&k| k != i).take(2).collect();
        body.push(row(next.iter().map(|&k| self.mini(k, mw)).collect()).gap(10.0).margin_each(14.0, 10.0, 14.0, 0.0));
        body.push(gap_box(0.0, 24.0 + bottom));
        column(vec![gap_box(0.0, top), player, scroll_column(&self.scroll_watch, body).grow(1.0)]).bg(BG)
    }
}
