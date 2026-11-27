//! Video ko'rish: haqiqiy pleyer, vaqt chizg'ichi (fikrlar shu soniyaga qadalgan), like, obuna, fikrlar.

use crate::app::App;
use crate::icons::*;
use crate::kit::*;
use crate::model::*;
use crate::theme::*;
use uzcoders::prelude::*;

impl App {
    /// Chizg'ich: fikr yozilgan soniyalarda nuqta turadi, bosilsa video shu joyga o'tadi.
    fn timeline(self: &Rc<Self>, w: f32, total_s: i64) -> Element {
        let s = self.clone();
        let mut kids: Vec<Element> = vec![column(vec![spacer(), ruler(&self.pos, 49, 14.0, w, Rc::new(move |x, up| s.scrub(x, up)))])];
        if total_s > 0 {
            for n in self.notes.get().iter().take(60) {
                let (a, at) = (self.clone(), n.at);
                let x = (n.at as f32 / total_s as f32).clamp(0.0, 1.0) * w - 5.0;
                kids.push(column(vec![]).width(10.0).height(10.0).radius(5.0).bg(SIG).abs().margin_each(x.max(0.0), 0.0, 0.0, 0.0).on_click(move || a.seek_secs(at)));
            }
        }
        stack(kids).width(w).height(40.0)
    }

    fn note_row(self: &Rc<Self>, n: &Note, w: f32) -> Element {
        let (a, at) = (self.clone(), n.at);
        row(vec![
            column(vec![b(clock(n.at as f32), 11.5, BG)]).center().height(26.0).padding_xy(8.0, 0.0).radius(8.0).bg(SIG),
            gap_box(10.0, 0.0),
            column(vec![t(short(&n.name, 24), 12.0, MUTE), t(&n.body, 13.5, WHITE).margin_each(0.0, 1.0, 0.0, 0.0)]).weight(1.0),
        ])
        .width(w)
        .padding_each(0.0, 0.0, 0.0, 12.0)
        .on_click(move || a.seek_secs(at))
    }

    fn player_box(self: &Rc<Self>, v: &Video, w: f32, h: f32, full: bool, top: f32) -> Element {
        let st = self.st.borrow().clone();
        let (vw, vh) = if st.w > 0 && st.h > 0 {
            let r = (st.w as f32 / st.h as f32).max(0.2);
            if w / h > r { (h * r, h) } else { (w, w / r) }
        } else {
            (w, h)
        };
        let (a, p, f, x) = (self.clone(), self.clone(), self.clone(), self.clone());
        let close = column(vec![icon(if full { FULL } else { CHEVRON_DOWN }, 26.0).color(WHITE)])
            .center()
            .width(44.0)
            .height(44.0)
            .on_click(move || if a.full.get() { a.set_full(false) } else { a.close_video() });
        let center = if !st.playing && st.ready {
            column(vec![column(vec![icon(if st.ended { PLAY } else { PLAY }, 30.0).color(BG)]).center().width(60.0).height(60.0).radius(18.0).bg(WHITE).on_click(move || p.play_pause())]).center()
        } else if st.buffering || !st.ready {
            column(vec![spinner(38.0, 3.0, WHITE)]).center()
        } else {
            column(vec![])
        };
        let rw = w - 32.0 - 78.0 - 44.0;
        let bar = column(vec![
            spacer(),
            row(vec![
                t(format!("{} / {}", clock(st.pos_ms as f32 / 1000.0), clock(st.dur_ms.max(v.duration * 1000) as f32 / 1000.0)), 12.0, WHITE).width(78.0),
                ruler(&self.pos, 36, 12.0, rw, Rc::new(move |xx, up| x.scrub(xx, up))),
                column(vec![icon(FULL, 22.0).color(WHITE)]).center().width(44.0).height(44.0).on_click(move || f.set_full(!f.full.get())),
            ])
            .align(Align::Center)
            .padding_each(16.0, 0.0, 0.0, 6.0),
        ]);
        let tap = self.clone();
        stack(vec![
            column(vec![]).bg(Color::BLACK),
            cover(&v.id, v.has_thumb, 16, 9, 0.0, vec![]).width(w).height(h),
            column(vec![column(vec![]).video().width(vw).height(vh)]).center().on_click(move || tap.play_pause()),
            column(vec![row(vec![close, spacer()]).padding_each(6.0, top + 4.0, 0.0, 0.0)]),
            center,
            bar,
        ])
        .width(w)
        .height(h)
    }

    pub fn watch_view(self: &Rc<Self>, v: &Video, top: f32, bottom: f32) -> Element {
        let (w, h) = window_dp();
        let full = self.full.get();
        if full {
            return self.player_box(v, w, h, true, 0.0).bg(Color::BLACK);
        }
        let ph = w * 0.5625;
        let player = self.player_box(v, w, ph, false, 0.0);
        let cw = w - 28.0;
        let mine = v.uid == self.uid();
        let chan = self.chan.get();
        let (l, fo, del, add, ask) = (self.clone(), self.clone(), self.clone(), self.clone(), self.clone());
        let follow = match &chan {
            Some(c) if !mine => {
                let on = c.following;
                let target = v.uid.clone();
                column(vec![b(if on { "Kuzatilmoqda" } else { "Kuzatish" }, 14.0, if on { WHITE } else { BG })])
                    .center()
                    .height(38.0)
                    .padding_xy(18.0, 0.0)
                    .radius(12.0)
                    .bg(if on { S3 } else { WHITE })
                    .on_click(move || fo.toggle_follow(&target))
            }
            _ => column(vec![]),
        };
        let followers = chan.as_ref().map(|c| format!("{} kuzatuvchi", compact(c.followers))).unwrap_or_default();
        let channel = row(vec![
            avatar(&initial(&v.channel), 40.0, Tone::Light),
            gap_box(10.0, 0.0),
            column(vec![b(short(&v.channel, 22), 14.0, WHITE), t(followers, 12.0, MUTE)]).weight(1.0),
            follow,
        ])
        .align(Align::Center)
        .width(cw)
        .margin_each(0.0, 12.0, 0.0, 0.0);
        let vv = v.clone();
        let confirm = self.confirm_del.get();
        let mut actions = vec![key(LIKE, &compact(v.likes), v.liked).on_click(move || l.toggle_like(&vv)), key(COMMENT, &compact(v.notes), false)];
        if mine {
            let st = self.confirm_del.clone();
            actions.push(if confirm {
                key(TRASH, "Aniqmi?", true).on_click(move || del.delete_video())
            } else {
                key(TRASH, "O'chirish", false).on_click(move || st.set(true))
            });
        }
        let actions = row(actions).gap(8.0).width(cw).margin_each(0.0, 12.0, 0.0, 0.0);
        let send = column(vec![icon(SEND, 22.0).color(BG)]).center().width(46.0).height(46.0).radius(14.0).bg(WHITE).on_click(move || add.add_note());
        let write = row(vec![input(&self.note_text, "Shu soniyaga fikr yozing").on_submit(move || ask.add_note()).weight(1.0), gap_box(8.0, 0.0), send]).align(Align::Center).width(cw).margin_each(0.0, 0.0, 0.0, 14.0);
        let notes = self.notes.get();
        let mut body: Vec<Element> = vec![
            self.timeline(cw, (self.st.borrow().dur_ms / 1000).max(v.duration)).margin_each(14.0, 8.0, 14.0, 0.0),
            column(vec![
                b(&v.title, 17.0, WHITE),
                t(format!("{} ko'rish · {}", compact(v.views), ago(v.created, now_ms())), 12.5, MUTE).margin_each(0.0, 3.0, 0.0, 0.0),
                channel,
                actions,
                disp("Vaqt bo'yicha fikrlar", 21.0, WHITE).margin_each(0.0, 20.0, 0.0, 10.0),
                write,
            ])
            .padding_each(14.0, 10.0, 14.0, 0.0),
        ];
        if notes.is_empty() {
            body.push(t("Hali fikr yo'q. Birinchi bo'lib yozing.", 13.0, MUTE).margin_each(14.0, 0.0, 14.0, 16.0));
        }
        for n in notes.iter() {
            body.push(self.note_row(n, cw).margin_each(14.0, 0.0, 14.0, 0.0));
        }
        self.f_new.ensure(self.q_new());
        let next: Vec<Video> = self.f_new.items.with(|l| l.iter().filter(|x| x.id != v.id).take(4).cloned().collect());
        if !next.is_empty() {
            body.push(disp("Keyingi", 21.0, WHITE).margin_each(14.0, 14.0, 14.0, 10.0));
            for x in next.iter() {
                body.push(self.card(x, w - 24.0));
            }
        }
        body.push(gap_box(0.0, 24.0 + bottom));
        column(vec![gap_box(0.0, top), player, scroll_column(&self.sc_watch, body).grow(1.0)]).bg(BG)
    }
}
