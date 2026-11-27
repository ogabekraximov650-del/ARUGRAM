//! Bosh, Obunalar va ro'yxatlar: haqiqiy videolar serverdan.

use crate::app::{App, Tab};
use crate::feed::Feed;
use crate::icons::*;
use crate::kit::*;
use crate::model::*;
use crate::theme::*;
use uzcoders::prelude::*;

impl App {
    /// Katta karta (ro'yxatdagi birinchi video): rasm, o'yilgan burchakda sarlavha va kanal.
    fn feature(self: &Rc<Self>, v: &Video, cw: f32) -> Element {
        let nw = (cw - 80.0).min(272.0);
        let info = row(vec![
            avatar(&initial(&v.channel), 36.0, Tone::Light),
            gap_box(10.0, 0.0),
            column(vec![b(short(&v.title, 46), 15.5, WHITE), t(format!("{} · {}", short(&v.channel, 18), compact(v.views)), 12.0, MUTE).margin_each(0.0, 3.0, 0.0, 0.0)]).weight(1.0),
        ])
        .padding_each(0.0, 13.0, 14.0, 0.0);
        let tag = column(vec![row(vec![spacer(), column(vec![b(dur(v.duration), 12.5, BG)]).center().height(30.0).padding_xy(10.0, 0.0).radius(9.0).bg(WHITE)])]).padding(12.0);
        let play = column(vec![spacer(), row(vec![spacer(), column(vec![icon(PLAY, 26.0).color(BG)]).center().width(54.0).height(54.0).radius(16.0).bg(WHITE)])]).padding(12.0);
        let (a, vv) = (self.clone(), v.clone());
        cover(&v.id, v.has_thumb, 16, 9, 26.0, vec![tag, play, at_bottom_left(notch(nw, 104.0, 24.0, info))])
            .width(cw)
            .height(288.0)
            .margin_each(12.0, 0.0, 12.0, 14.0)
            .on_click(move || a.open_video(vv.clone()))
    }

    /// Oddiy karta: katta muqova, ostida kanal belgisi, sarlavha va ma'lumot.
    pub fn card(self: &Rc<Self>, v: &Video, cw: f32) -> Element {
        let cv = cover(&v.id, v.has_thumb, 16, 9, 22.0, vec![dur_tab(&dur(v.duration))]).width(cw).height(cw * 0.5625);
        let info = row(vec![
            avatar(&initial(&v.channel), 38.0, Tone::Light),
            gap_box(12.0, 0.0),
            column(vec![
                b(short(&v.title, 60), 15.5, WHITE),
                t(format!("{} · {} ko'rish · {}", short(&v.channel, 18), compact(v.views), ago(v.created, now_ms())), 12.5, MUTE).margin_each(0.0, 3.0, 0.0, 0.0),
            ])
            .weight(1.0),
        ])
        .padding_each(2.0, 12.0, 2.0, 0.0);
        let (a, vv) = (self.clone(), v.clone());
        column(vec![cv, info]).width(cw).margin_each(12.0, 0.0, 12.0, 18.0).on_click(move || a.open_video(vv.clone()))
    }

    /// Ro'yxat: yuklanish, xato, bo'sh holat, sahifalash va tortib yangilash bilan.
    pub fn feed_view(self: &Rc<Self>, feed: &Rc<Feed>, q: String, sc: &ScrollState, head: Vec<Element>, empty_el: Element, big_first: bool, bottom: f32) -> Element {
        let (w, _) = window_dp();
        let cw = w - 24.0;
        feed.ensure(q.clone());
        let (f, q2) = (feed.clone(), q.clone());
        sc.on_refresh(move || f.fetch(q2.clone(), false));
        let items = feed.items.get();
        let mut kids = head;
        if items.is_empty() {
            if feed.loading.get() {
                kids.push(column(vec![spinner(34.0, 3.0, SIG)]).center().height(240.0));
            } else if !feed.err.get().is_empty() {
                let (f, q3) = (feed.clone(), q);
                kids.push(empty(CLOSE, "Yuklanmadi", &feed.err.get(), Some(("Qayta urinish", Rc::new(move || f.fetch(q3.clone(), false))))));
            } else {
                kids.push(empty_el);
            }
        } else {
            for (i, v) in items.iter().enumerate() {
                kids.push(if big_first && i == 0 { self.feature(v, cw) } else { self.card(v, cw) });
            }
            if feed.more.get() {
                if sc.max_offset() > 0.0 && sc.offset() > sc.max_offset() - 900.0 && !feed.loading.get() {
                    feed.fetch(q, true);
                }
                kids.push(column(vec![spinner(26.0, 2.5, SIG)]).center().height(60.0));
            }
        }
        kids.push(gap_box(0.0, 110.0 + bottom));
        scroll_column(sc, kids).grow(1.0)
    }

    fn avatar_btn(self: &Rc<Self>) -> Element {
        let a = self.clone();
        let name = self.me.with(|m| m.as_ref().map(|m| m.name.clone()).unwrap_or_default());
        avatar(&initial(&name), 34.0, Tone::Light).on_click(move || a.tab.set(Tab::Me))
    }

    pub fn home_view(self: &Rc<Self>, top: f32, bottom: f32) -> Element {
        let hot = self.hot.get();
        let a = self.clone();
        let head = row(vec![
            logo(38.0),
            spacer(),
            icon_btn(SEARCH).on_click(move || a.search_on.set(true)),
            gap_box(4.0, 0.0),
            self.avatar_btn(),
        ])
        .align(Align::Center)
        .padding_each(16.0, top + 6.0, 14.0, 0.0);
        let tab = |label: &str, on: bool, h: bool| {
            let a = self.clone();
            disp(label, 26.0, if on { WHITE } else { Color::hex(0x555555) }).on_click(move || a.hot.set(h))
        };
        let tabs = row(vec![tab("Yangi", !hot, false), tab("Ommabop", hot, true)]).gap(20.0).padding_each(16.0, 12.0, 16.0, 12.0);
        let (feed, q) = if hot { (&self.f_hot, self.q_hot()) } else { (&self.f_new, self.q_new()) };
        let a = self.clone();
        let empty_el = empty(VIDEO, "Hozircha video yo'q", "Birinchi bo'lib video yuklang", Some(("Video yuklash", Rc::new(move || a.create.set(true)))));
        column(vec![head, tabs, self.feed_view(feed, q, &self.sc_home, Vec::new(), empty_el, !hot, bottom)])
    }

    /// Obunalar: obuna bo'lingan kanallarning videolari.
    pub fn subs_view(self: &Rc<Self>, top: f32, bottom: f32) -> Element {
        let a = self.clone();
        let empty_el = empty(SUBS, "Obunalar yo'q", "Videolarni ko'rib, yoqqan kanalga obuna bo'ling", Some(("Videolarga o'tish", Rc::new(move || a.tab.set(Tab::Home)))));
        column(vec![
            disp("Obunalar", 40.0, WHITE).padding_each(16.0, top + 14.0, 16.0, 10.0),
            self.feed_view(&self.f_subs, self.q_subs(), &self.sc_subs, Vec::new(), empty_el, false, bottom),
        ])
    }

    /// Qidiruv: sarlavha bo'yicha.
    pub fn search_view(self: &Rc<Self>, top: f32, bottom: f32) -> Element {
        let (a, s, g) = (self.clone(), self.clone(), self.clone());
        let back = column(vec![icon(BACK, 22.0).color(WHITE)]).center().width(44.0).height(44.0).on_click(move || a.search_on.set(false));
        let run = move || {
            let q = g.search_text.get().trim().to_string();
            g.f_search.items.set(Vec::new());
            if q.is_empty() {
                g.f_search.reset();
                return;
            }
            let enc: String = q.bytes().map(|c| if c.is_ascii_alphanumeric() { (c as char).to_string() } else { format!("%{c:02X}") }).collect();
            g.f_search.fetch(g.q(&format!("sort=new&q={enc}")), false);
        };
        let go = run.clone();
        let bar = row(vec![back, input(&self.search_text, "Video qidirish").on_submit(run).weight(1.0), gap_box(8.0, 0.0), column(vec![icon(SEARCH, 22.0).color(BG)]).center().width(46.0).height(46.0).radius(14.0).bg(WHITE).on_click(go)])
            .align(Align::Center)
            .padding_each(8.0, top + 8.0, 14.0, 8.0);
        let q = self.search_text.get().trim().to_string();
        let body = if q.is_empty() && self.f_search.items.with(|l| l.is_empty()) {
            column(vec![empty(SEARCH, "Qidiruv", "Sarlavhadan so'z yozing", None)]).grow(1.0)
        } else {
            let enc: String = q.bytes().map(|c| if c.is_ascii_alphanumeric() { (c as char).to_string() } else { format!("%{c:02X}") }).collect();
            let nothing = empty(SEARCH, "Hech narsa topilmadi", "Boshqa so'z bilan urinib ko'ring", None);
            if self.f_search.asked.get() {
                self.feed_view(&self.f_search, s.q(&format!("sort=new&q={enc}")), &self.sc_search, Vec::new(), nothing, false, bottom)
            } else {
                nothing
            }
        };
        column(vec![bar, body]).bg(BG)
    }
}
