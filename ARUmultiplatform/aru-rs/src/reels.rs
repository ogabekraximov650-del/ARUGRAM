//! Oqim: tik qisqa videolar; to'liq ekranda o'ynaydi, o'ngga/chapga surilsa keyingisi/oldingisi.

use crate::app::App;
use crate::icons::*;
use crate::kit::*;
use crate::model::*;
use crate::theme::*;
use uzcoders::prelude::*;

impl App {
    pub fn reels_view(self: &Rc<Self>, top: f32, bottom: f32) -> Element {
        let (w, h) = window_dp();
        let f = &self.f_reels;
        f.ensure("reels", self.q_reels());
        let tabs = row(vec![disp("Oqim", 28.0, WHITE)]).padding_each(16.0, top + 10.0, 16.0, 0.0);
        let items = f.items.get();
        if items.is_empty() {
            let (a, g) = (self.clone(), self.clone());
            let body = if f.loading.get() {
                column(vec![spinner(34.0, 3.0, SIG)]).center().grow(1.0)
            } else if !f.err.get().is_empty() {
                let (r, q) = (f.clone(), self.q_reels());
                column(vec![empty(CLOSE, "Yuklanmadi", &f.err.get(), Some(("Qayta urinish", Rc::new(move || r.fetch(q.clone(), false, true)))))]).center().grow(1.0)
            } else {
                column(vec![empty(SHORTS, "Hozircha oqim yo'q", "Tik qisqa video yuklab, birinchi bo'ling", Some(("Oqim yuklash", Rc::new(move || a.pick("reel")))))]).center().grow(1.0)
            };
            let _ = g;
            return column(vec![tabs, body]);
        }
        let i = self.reel_i.get().min(items.len() - 1);
        let v = items[i].clone();
        // Keyingi sahifa: oxirgi 2 ta qolganda yuklanadi.
        if f.more.get() && i + 2 >= items.len() && !f.loading.get() {
            f.fetch(self.q_reels(), true, false);
        }
        let st = self.st.borrow().clone();
        let (vw, vh) = if st.w > 0 && st.h > 0 {
            let r = (st.w as f32 / st.h as f32).max(0.2);
            if w / h > r { (h * r, h) } else { (w, w / r) }
        } else {
            (w, h)
        };
        let (n, p, t1) = (self.clone(), self.clone(), self.clone());
        let video = stack(vec![
            cover(&v.id, v.has_thumb, 9, 16, 0.0, vec![]).width(w).height(h),
            column(vec![column(vec![]).video().width(vw).height(vh)]).center(),
        ])
        .on_swipe(move |d| {
            if d > 0 {
                n.reel_go(1)
            } else {
                p.reel_go(-1)
            }
        })
        .on_click(move || t1.play_pause());
        let (l, o) = (self.clone(), self.clone());
        let vv = v.clone();
        let iw = w - 32.0;
        let info = column(vec![
            row(vec![avatar(&initial(&v.channel), 38.0, Tone::Light), gap_box(10.0, 0.0), b(short(&v.channel, 22), 15.5, WHITE)]).align(Align::Center),
            disp(&v.title, 34.0, WHITE).margin_each(0.0, 12.0, 0.0, 0.0),
            t(format!("{} ko'rish · {}", compact(v.views), ago(v.created, now_ms())), 13.5, DIM).margin_each(0.0, 8.0, 0.0, 14.0),
            ruler(&self.pos, 48, 14.0, iw, {
                let s = self.clone();
                Rc::new(move |x, up| s.scrub(x, up))
            }),
            row(vec![b(clock(st.pos_ms as f32 / 1000.0), 11.5, WHITE), spacer(), t(clock(st.dur_ms as f32 / 1000.0), 11.5, MUTE)]).width(iw).margin_each(0.0, 4.0, 0.0, 12.0),
            row(vec![
                wide_key(HEART, &compact(v.likes), v.liked).on_click(move || l.toggle_like(&vv)),
                wide_key(COMMENT, &compact(v.notes), false).on_click({
                    let v2 = v.clone();
                    move || o.open_video(v2.clone())
                }),
            ])
            .gap(8.0)
            .width(iw),
        ])
        .padding_each(16.0, 80.0, 16.0, 98.0 + bottom)
        .gradient(Color::rgba(0, 0, 0, 0), Color::rgba(0, 0, 0, 215));
        let status_line = if !st.err.is_empty() {
            column(vec![t(st.err.clone(), 12.0, Color::hex(0xF87171))]).padding_each(16.0, 8.0, 16.0, 0.0)
        } else if st.buffering || !st.ready {
            column(vec![row(vec![spinner(22.0, 2.5, WHITE), gap_box(8.0, 0.0), t("Yuklanmoqda…", 12.5, DIM)]).align(Align::Center)]).padding_each(16.0, 8.0, 16.0, 0.0)
        } else {
            column(vec![])
        };
        stack(vec![video, column(vec![tabs, status_line, spacer(), info]).bg(Color::rgba(0, 0, 0, 0))])
    }
}
