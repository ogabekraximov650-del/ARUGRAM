//! Yuklab olish boshqaruvi: holat so'rovi, sifat qatori, Kutubxona → Yuklanmalar.

use super::*;
use crate::data::format_bytes;
use crate::downloads::{self, DlItem, VStat};
use std::collections::{HashMap, HashSet};

const D_DOWN: &str = "M19,9h-4V3H9v6H5l7,7 7,-7zM5,18v2h14v-2H5z";
const D_PAUSE: &str = "M6,19h4V5H6v14zM14,5v14h4V5h-4z";
const D_TRASH: &str = "M6,19c0,1.1 0.9,2 2,2h8c1.1,0 2,-0.9 2,-2V7H6v12zM19,4h-3.5l-1,-1h-5l-1,1H5v2h14V4z";
const D_CHECK: &str = "M9,16.17L4.83,12l-1.42,1.41L9,19 21,7l-1.41,-1.41z";
const D_PLAY: &str = "M8,5v14l11,-7z";
const GREEN: Color = Color::rgb(0x4A, 0xDE, 0x80);

pub struct DlAnimePage {
    anime_id: i64,
    season_id: i64,
    title: String,
    scroll: ScrollState,
}

impl Home {
    pub(super) fn dl_watch(self: &Rc<Self>, urls: &[String]) {
        {
            let mut w = self.dl_watch.borrow_mut();
            for u in urls {
                w.insert(u.clone());
            }
        }
        self.dl_poll();
    }

    pub(super) fn dl_unwatch_all(&self) {
        self.dl_watch.borrow_mut().clear();
    }

    pub(super) fn dl_poll(self: &Rc<Self>) {
        if self.dl_running.replace(true) {
            return;
        }
        self.dl_tick();
    }

    fn dl_tick(self: &Rc<Self>) {
        let urls: Vec<String> = self.dl_watch.borrow().iter().chain(self.dl_active.borrow().iter()).cloned().collect::<HashSet<_>>().into_iter().collect();
        if urls.is_empty() {
            self.dl_running.set(false);
            return;
        }
        let me = self.clone();
        spawn_bg(
            move || {
                let s = downloads::stats(&urls);
                let busy = s.values().any(|v| v.downloading);
                std::thread::sleep(std::time::Duration::from_millis(if busy { 500 } else { 2000 }));
                s
            },
            move |s| {
                let mut cur: HashMap<String, VStat> = (*me.dl.get()).clone();
                for (u, st) in s {
                    if !st.downloading {
                        me.dl_active.borrow_mut().remove(&u);
                    }
                    cur.insert(u, st);
                }
                if *me.dl.get() != cur {
                    me.dl.set(Rc::new(cur));
                }
                me.dl_tick();
            },
        );
    }

    pub(super) fn dl_start(self: &Rc<Self>, url: String) {
        self.dl_active.borrow_mut().insert(url.clone());
        let mut cur: HashMap<String, VStat> = (*self.dl.get()).clone();
        cur.entry(url.clone()).or_default().downloading = true;
        self.dl.set(Rc::new(cur));
        self.dl_poll();
        let (me, session) = (self.clone(), self.session.clone());
        spawn_bg(move || downloads::start(&session, &url).map_err(|e| (url, e)), move |r| {
            if let Err((u, e)) = r {
                me.dl_active.borrow_mut().remove(&u);
                let mut cur: HashMap<String, VStat> = (*me.dl.get()).clone();
                cur.entry(u).or_default().downloading = false;
                me.dl.set(Rc::new(cur));
                uzcoders::android::toast(&e);
            }
        });
    }

    pub(super) fn dl_pause(self: &Rc<Self>, url: String) {
        downloads::pause(&url);
        self.dl_active.borrow_mut().remove(&url);
        let mut cur: HashMap<String, VStat> = (*self.dl.get()).clone();
        let e = cur.entry(url).or_default();
        e.downloading = false;
        e.queued = false;
        self.dl.set(Rc::new(cur));
    }

    pub(super) fn dl_delete(self: &Rc<Self>, url: String) {
        downloads::delete(&url);
        self.dl_active.borrow_mut().remove(&url);
        let mut cur: HashMap<String, VStat> = (*self.dl.get()).clone();
        let e = cur.entry(url.clone()).or_default();
        e.downloaded = 0;
        e.downloading = false;
        e.queued = false;
        self.dl.set(Rc::new(cur));
        if let Some(idx) = self.dl_index.get() {
            let v: Vec<DlItem> = idx.iter().filter(|d| d.url != url).cloned().collect();
            self.dl_index.set(Some(Rc::new(v)));
        }
    }

    pub(super) fn dl_reload_index(self: &Rc<Self>) {
        if self.dl_loading.replace(true) {
            return;
        }
        let (me, seasons) = (self.clone(), (*self.seasons.get()).clone());
        spawn_bg(move || downloads::collect(&seasons), move |list| {
            me.dl_loading.set(false);
            let urls: Vec<String> = list.iter().map(|d| d.url.clone()).collect();
            let mut cur: HashMap<String, VStat> = (*me.dl.get()).clone();
            for d in &list {
                cur.insert(d.url.clone(), d.stat.clone());
            }
            me.dl.set(Rc::new(cur));
            me.dl_index.set(Some(Rc::new(list)));
            me.dl_watch(&urls);
        });
    }

    /// Sifat qatori: sifat, foiz, hajm, tezlik, chiziq va yuklash/pauza + o'chirish tugmalari.
    pub(super) fn quality_row(self: &Rc<Self>, url: String, label: String, size: i64) -> Element {
        let st = self.dl.get().get(&url).cloned().unwrap_or_default();
        let total = if st.total > 0 { st.total } else { size };
        let mut meta = vec![
            tx(label.clone(), 13.0, Color::WHITE).black(),
            tx(format!("{}%", st.percent()), 13.0, if st.complete() { ACCENT2 } else { Color::WHITE }).black(),
            tx(format!("{} / {}", format_bytes(st.downloaded), if total > 0 { format_bytes(total) } else { "—".to_string() }), 11.5, Color::WHITE).bold(),
        ];
        let speed = if st.downloading && !st.queued { st.speed_label() } else { String::new() };
        let mut chips: Vec<Element> = Vec::new();
        let chip = |t: &str, c: Color| tx(t, 11.0, c).bold().bg(c.with_opacity(0.18)).radius(6.0).padding_xy(6.0, 2.0);
        if st.queued {
            chips.push(chip("navbatda", Color::rgba(255, 255, 255, 190)));
        }
        if !speed.is_empty() {
            chips.push(chip(&speed, ACCENT2));
        }
        if st.retrying {
            chips.push(chip("qayta urinilmoqda", Color::rgb(0xFF, 0x98, 0x00)));
        }
        let mut left = vec![row(std::mem::take(&mut meta)).gap(8.0).align(Align::Center)];
        if !chips.is_empty() {
            left.push(row(chips).gap(6.0).padding_each(0.0, 3.0, 0.0, 0.0));
        }
        left.push(gap_box(0.0, 5.0));
        let fill = (st.ratio() * 1000.0).max(0.0);
        left.push(row(vec![column(vec![]).weight(fill.max(0.001)).height(4.0).bg(if st.complete() { Color::WHITE } else { ACCENT }), column(vec![]).weight((1000.0 - fill).max(0.001)).height(4.0)]).height(4.0).radius(2.0).bg(Color::rgba(255, 255, 255, 26)));
        let btn = |ic: &'static str, tint: Color, bg: Color, f: Box<dyn Fn()>| column(vec![icon(ic, 21.0).color(tint)]).center().width(42.0).height(42.0).radius(13.0).bg(bg).on_click(f);
        let mut kids = vec![column(left).weight(1.0)];
        if !st.complete() {
            let (me, u) = (self.clone(), url.clone());
            if st.downloading {
                kids.push(btn(D_PAUSE, ACCENT2, ACCENT.with_opacity(0.25), Box::new(move || me.dl_pause(u.clone()))));
            } else {
                kids.push(btn(D_DOWN, Color::WHITE, Color::rgba(255, 255, 255, 22), Box::new(move || me.dl_start(u.clone()))));
            }
        }
        let (me, u, l) = (self.clone(), url, label);
        kids.push(btn(D_TRASH, Color::rgba(255, 255, 255, 190), Color::rgba(255, 255, 255, 22), Box::new(move || me.del_confirm.set(Some((u.clone(), l.clone()))))));
        row(kids).gap(8.0).align(Align::Center).padding_each(0.0, 8.0, 0.0, 0.0)
    }

    /// Tozalash tasdiqi (sifat yoki hammasi) — umumiy shisha oyna.
    pub(super) fn dl_dialogs(self: &Rc<Self>) -> Option<Element> {
        let (title, text, action): (String, String, Box<dyn Fn()>) = if let Some((u, l)) = self.del_confirm.get() {
            let me = self.clone();
            ("Tozalash".into(), format!("Rostanham {l} sifatidagi videoni tozalab tashlaysizmi?"), Box::new(move || me.dl_delete(u.clone())))
        } else if self.dl_wipe_confirm.get() {
            let me = self.clone();
            ("Hammasini o'chirish".into(), "Yuklab olingan barcha videolar telefondan o'chiriladi.".into(), Box::new(move || {
                downloads::wipe_all();
                me.dl.set(Rc::new(HashMap::new()));
                me.dl_active.borrow_mut().clear();
                me.dl_index.set(Some(Rc::new(Vec::new())));
            }))
        } else {
            return None;
        };
        let (m1, m2) = (self.clone(), self.clone());
        let panel = column(vec![
            tx(title, 18.0, Color::WHITE).bold(),
            gap_box(0.0, 10.0),
            tx(text, 14.0, Color::WHITE),
            gap_box(0.0, 18.0),
            row(vec![
                spacer(),
                self.chip("Bekor qilish", false, move || {
                    m1.del_confirm.set(None);
                    m1.dl_wipe_confirm.set(false);
                }),
                self.chip("Ha", true, move || {
                    action();
                    m2.del_confirm.set(None);
                    m2.dl_wipe_confirm.set(false);
                }),
            ])
            .gap(8.0),
        ])
        .lens(Color::rgba(255, 255, 255, 14), GLASS_ZOOM)
        .radius(28.0)
        .padding(22.0)
        .margin_each(24.0, 0.0, 24.0, 0.0);
        let me = self.clone();
        Some(column(vec![panel]).center().bg(Color::rgba(0, 0, 0, 45)).on_click(move || {
            me.del_confirm.set(None);
            me.dl_wipe_confirm.set(false);
        }))
    }

    /// Kutubxona → Yuklanmalar: har bir bo'lim bitta karta.
    pub(super) fn downloads_view(self: &Rc<Self>, w: f32, top: f32, bottom: f32) -> Element {
        let Some(idx) = self.dl_index.get() else {
            self.dl_reload_index();
            return column(vec![spinner(32.0, 3.0, ACCENT2)]).center();
        };
        if idx.is_empty() {
            return Self::empty(D_PLAY, "Yuklab olingan qism yo'q\nQismlar ro'yxatida yuklash belgisini bosib yuklab oling");
        }
        let live = self.dl.get();
        // Bo'lim bo'yicha guruhlash.
        let mut groups: Vec<((i64, i64), Vec<DlItem>)> = Vec::new();
        for d in idx.iter() {
            match groups.iter_mut().find(|(k, _)| *k == (d.anime_id, d.season_id)) {
                Some((_, v)) => v.push(d.clone()),
                None => groups.push(((d.anime_id, d.season_id), vec![d.clone()])),
            }
        }
        let total: i64 = idx.iter().map(|d| live.get(&d.url).map(|s| s.downloaded).unwrap_or(d.stat.downloaded)).sum();
        let me = self.clone();
        let mut kids: Vec<Element> = vec![row(vec![
            tx(format!("{} ta fayl · {}", idx.len(), format_bytes(total)), 12.0, Color::WHITE).weight(1.0),
            tx("Hammasini o'chirish", 12.0, ERROR).bold().on_click(move || me.dl_wipe_confirm.set(true)),
        ])
        .padding_each(4.0, 0.0, 4.0, 10.0)];
        let img_w = w - 32.0 - 20.0;
        for ((a, s), items) in groups {
            let first = items[0].clone();
            let full = {
                let mut eps: HashSet<i64> = HashSet::new();
                for d in &items {
                    if live.get(&d.url).map(|st| st.complete()).unwrap_or(d.stat.complete()) {
                        eps.insert(d.episode_id);
                    }
                }
                eps.len()
            };
            let on_disk: i64 = items.iter().map(|d| live.get(&d.url).map(|st| st.downloaded).unwrap_or(d.stat.downloaded)).sum();
            let pic = match self.frame(&first.poster) {
                Some(f) => stack(vec![image_fill(&f)]).height(img_w * 9.0 / 16.0),
                None => {
                    let (c1, c2) = name_colors(&first.title);
                    column(vec![]).gradient(c1, c2).radius(14.0).height(img_w * 9.0 / 16.0)
                }
            };
            let (me, title) = (self.clone(), first.title.clone());
            kids.push(
                Self::pglass(
                    column(vec![
                        pic,
                        gap_box(0.0, 10.0),
                        tx(if first.title.is_empty() { "Anime".to_string() } else { first.title.clone() }, 15.5, Color::WHITE).bold(),
                        tx(format!("{}-bo'lim", first.section_no.max(1)), 13.0, Color::WHITE),
                        tx(format!("{full} ta qism to'liq yuklangan"), 12.5, Color::WHITE),
                        tx(format!("Hajmi: {}", format_bytes(on_disk)), 12.0, Color::WHITE),
                    ])
                    .padding(10.0),
                )
                .radius(22.0)
                .margin_each(0.0, 0.0, 0.0, 12.0)
                .on_click(move || me.open_dl_anime(a, s, title.clone())),
            );
        }
        kids.push(gap_box(0.0, 8.0));
        scroll_column(&self.dl_scroll, kids).padding_each(16.0, top, 16.0, bottom)
    }

    pub(super) fn open_dl_anime(self: &Rc<Self>, anime_id: i64, season_id: i64, title: String) {
        self.push(pages::Page::DlAnime(Rc::new(DlAnimePage { anime_id, season_id, title, scroll: ScrollState::new() })));
    }

    pub(super) fn dl_anime_page(self: &Rc<Self>, p: &Rc<DlAnimePage>) -> Element {
        let idx = self.dl_index.get().unwrap_or_default();
        let mut eps: Vec<(i64, i64, Vec<DlItem>)> = Vec::new();
        for d in idx.iter().filter(|d| d.anime_id == p.anime_id && d.season_id == p.season_id) {
            match eps.iter_mut().find(|(id, _, _)| *id == d.episode_id) {
                Some((_, _, v)) => v.push(d.clone()),
                None => eps.push((d.episode_id, d.episode_no, vec![d.clone()])),
            }
        }
        eps.sort_by_key(|(_, n, _)| *n);
        if eps.is_empty() {
            return self.scaffold(&p.title, Self::empty(D_PLAY, "Yuklab olingan qism yo'q"));
        }
        let mut kids: Vec<Element> = Vec::new();
        for (_, no, items) in eps {
            let mut col: Vec<Element> = vec![tx(format!("{no}-qism"), 15.5, Color::WHITE).bold()];
            for d in items {
                col.push(self.quality_row(d.url.clone(), d.quality_title.clone(), 0));
            }
            kids.push(Self::pglass(column(col).padding(14.0)).radius(22.0).margin_each(0.0, 0.0, 0.0, 12.0));
        }
        kids.push(gap_box(0.0, 40.0));
        self.scaffold(&p.title, scroll_column(&p.scroll, kids).padding_each(16.0, 8.0, 16.0, 0.0))
    }
}
