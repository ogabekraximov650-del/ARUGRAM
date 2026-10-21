//! Bosh sahifa: sarlavha, "Ommabop anime" va poster kartalari to'ri, pastda haqiqiy shisha (xiralashtiruvchi) navigatsiya paneli.

mod admin;
mod admin2;
mod billing;
mod comments;
mod dl;
mod edit;
mod packs;
mod pages;
mod player_ui;
mod storage;
mod support;
mod profile;
use crate::data::{Device, Profile, fetch_library, fetch_seasons, fmt_date, load_posters, HistoryItem, Season};
use crate::theme::*;
use std::cell::{Cell, RefCell};
use crate::downloads::{DlItem, VStat};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use uzcoders::prelude::*;

const BELL: &str = "M12,22c1.1,0 2,-0.9 2,-2h-4c0,1.1 0.89,2 2,2zM18,16v-5c0,-3.07 -1.64,-5.64 -4.5,-6.32V4c0,-0.83 -0.67,-1.5 -1.5,-1.5s-1.5,0.67 -1.5,1.5v0.68C7.63,5.36 6,7.92 6,11v5l-2,2v1h16v-1l-2,-2z";
const CLOSE: &str = "M19,6.41L17.59,5 12,10.59 6.41,5 5,6.41 10.59,12 5,17.59 6.41,19 12,13.41 17.59,19 19,17.59 13.41,12z";
const MENU: &str = "M3,18h18v-2L3,16v2zM3,13h18v-2L3,11v2zM3,6v2h18L21,6L3,6z";
const CAT_TABS: [&str; 7] = ["Barchasi", "Reytingi baland", "Eng ko'p ko'rilgan", "Ongoing", "Tugallangan", "Filmlar", "OVA"];
const LIB_TABS: [&str; 3] = ["Tarix", "Yuklanmalar", "Sevimlilar"];
const PLAY: &str = "M8,5v14l11,-7z";
const HEART: &str = "M12,21.35l-1.45,-1.32C5.4,15.36 2,12.28 2,8.5 2,5.42 4.42,3 7.5,3c1.74,0 3.41,0.81 4.5,2.09C13.09,3.81 14.76,3 16.5,3 19.58,3 22,5.42 22,8.5c0,3.78 -3.4,6.86 -8.55,11.54L12,21.35z";
const STAR: &str = "M12,17.27L18.18,21l-1.64,-7.03L22,9.24l-7.19,-0.61L12,2 9.19,8.63 2,9.24l5.46,4.73L5.82,21z";
const EYE: &str = "M12,4.5C7,4.5 2.73,7.61 1,12c1.73,4.39 6,7.5 11,7.5s9.27,-3.11 11,-7.5c-1.73,-4.39 -6,-7.5 -11,-7.5zM12,17c-2.76,0 -5,-2.24 -5,-5s2.24,-5 5,-5 5,2.24 5,5 -2.24,5 -5,5zM12,9c-1.66,0 -3,1.34 -3,3s1.34,3 3,3 3,-1.34 3,-3 -1.34,-3 -3,-3z";
const NAV: [(&str, &str); 5] = [
    ("Bosh sahifa", "M10,20v-6h4v6h5v-8h3L12,3 2,12h3v8z"),
    ("Qidiruv", "M15.5,14h-0.79l-0.28,-0.27C15.41,12.59 16,11.11 16,9.5 16,5.91 13.09,3 9.5,3S3,5.91 3,9.5 5.91,16 9.5,16c1.61,0 3.09,-0.59 4.23,-1.57l0.27,0.28v0.79l5,4.99L20.49,19l-4.99,-5zM9.5,14C7.01,14 5,11.99 5,9.5S7.01,5 9.5,5 14,7.01 14,9.5 11.99,14 9.5,14z"),
    ("Katalog", "M3,13h8L11,3L3,3v10zM3,21h8v-6L3,15v6zM13,21h8L21,11h-8v10zM13,3v6h8L21,3h-8z"),
    ("Kutubxona", "M10,4H4c-1.1,0 -1.99,0.9 -1.99,2L2,18c0,1.1 0.9,2 2,2h16c1.1,0 2,-0.9 2,-2V8c0,-1.1 -0.9,-2 -2,-2h-8l-2,-2z"),
    ("Profil", "M12,12c2.21,0 4,-1.79 4,-4s-1.79,-4 -4,-4 -4,1.79 -4,4 1.79,4 4,4zM12,14c-2.67,0 -8,1.34 -8,4v2h16v-2c0,-2.66 -5.33,-4 -8,-4z"),
];

#[derive(Clone, PartialEq)]
enum Status {
    Loading,
    Ready,
    Error(String),
}

pub struct Home {
    session: String,
    profile: State<Rc<Profile>>,
    chips_scroll2: ScrollState,
    dl: State<Rc<HashMap<String, VStat>>>,
    dl_watch: RefCell<HashSet<String>>,
    dl_active: RefCell<HashSet<String>>,
    dl_running: Cell<bool>,
    dl_index: State<Option<Rc<Vec<DlItem>>>>,
    dl_loading: Cell<bool>,
    dl_scroll: ScrollState,
    del_confirm: State<Option<(String, String)>>,
    dl_wipe_confirm: State<bool>,
    unread: State<u32>,
    picked_cb: RefCell<Option<Box<dyn Fn(String)>>>,
    pages: State<Vec<pages::Page>>,
    hidden: State<Vec<String>>,
    consent: State<bool>,
    devices: State<Option<Result<Rc<Vec<Device>>, ()>>>,
    settings_scroll: ScrollState,
    sessions_scroll: ScrollState,
    dlg: State<u8>,
    dlg_msg: State<String>,
    on_logout: Rc<dyn Fn()>,
    prof_scroll: ScrollState,
    avatar: RefCell<Option<Rc<Image>>>,
    cache_bytes: Cell<i64>,
    seasons: State<Rc<Vec<Season>>>,
    status: State<Status>,
    posters: RefCell<HashMap<String, Rc<Image>>>,
    queue: RefCell<Vec<String>>,
    loading_posters: Cell<bool>,
    scroll: ScrollState,
    tab: State<usize>,
    query: State<String>,
    search_scroll: ScrollState,
    cat_tab: State<usize>,
    cat_scroll: ScrollState,
    cat_tabs: ScrollState,
    lib_tab: State<usize>,
    hist: State<Vec<HistoryItem>>,
    favs: State<Rc<Vec<Season>>>,
    /// Filtrlangan/saralangan ro'yxat keshi (kalit -> natija): har kadrda qayta hisoblanmasin.
    memo: RefCell<(String, Rc<Vec<Season>>)>,
    lib_state: State<u8>,
    hist_scroll: ScrollState,
    fav_scroll: ScrollState,
    frames: RefCell<HashMap<String, Rc<Image>>>,
    genres: State<Vec<String>>,
    years: State<Vec<String>>,
    filter_open: State<bool>,
    filter_scroll: ScrollState,
    /// Posterlar yuklanmasa sababi (kichik qizil matn).
    poster_diag: State<String>,
}

fn tx(s: impl Into<String>, size: f32, color: Color) -> Element {
    text(s).size(size).color(color).line_height(size * 1.3)
}

/// Nomdan barqaror rang (poster yo'q paytdagi fon).
fn name_colors(name: &str) -> (Color, Color) {
    let h = name.bytes().fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
    let hue = (h % 360) as f32;
    let mk = |hh: f32, l: f32| {
        // Oddiy HSL -> RGB (to'yinganlik 0.45).
        let (s, c) = (0.45f32, (1.0 - (2.0 * l - 1.0).abs()) * 0.45);
        let _ = s;
        let x = c * (1.0 - ((hh / 60.0) % 2.0 - 1.0).abs());
        let m = l - c / 2.0;
        let (r, g, b) = match (hh / 60.0) as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        Color::rgb(((r + m) * 255.0) as u8, ((g + m) * 255.0) as u8, ((b + m) * 255.0) as u8)
    };
    (mk(hue, 0.30), mk((hue + 40.0) % 360.0, 0.14))
}

fn compact(n: i64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1e6)
    } else if n >= 1000 {
        format!("{:.1}K", n as f64 / 1e3)
    } else {
        n.to_string()
    }
}

fn badge(children: Vec<Element>) -> Element {
    row(children)
        .gap(3.0)
        .align(Align::Center)
        .bg(Color::rgba(0, 0, 0, 140))
        .radius(9.0)
        .padding_xy(7.0, 3.0)
}

impl Home {
    pub fn new(session: String, on_logout: Rc<dyn Fn()>) -> Rc<Home> {
        let h = Rc::new(Home {
            session,
            profile: State::new(Rc::new(Profile::default())),
            chips_scroll2: ScrollState::new(),
            dl: State::new(Rc::new(HashMap::new())),
            dl_watch: RefCell::new(HashSet::new()),
            dl_active: RefCell::new(HashSet::new()),
            dl_running: Cell::new(false),
            dl_index: State::new(None),
            dl_loading: Cell::new(false),
            dl_scroll: ScrollState::new(),
            del_confirm: State::new(None),
            dl_wipe_confirm: State::new(false),
            unread: State::new(0),
            picked_cb: RefCell::new(None),
            pages: State::new(Vec::new()),
            hidden: State::new(Vec::new()),
            consent: State::new(false),
            devices: State::new(None),
            settings_scroll: ScrollState::new(),
            sessions_scroll: ScrollState::new(),
            dlg: State::new(0),
            dlg_msg: State::new(String::new()),
            on_logout,
            prof_scroll: ScrollState::new(),
            avatar: RefCell::new(None),
            cache_bytes: Cell::new(0),
            seasons: State::new(Rc::new(Vec::new())),
            status: State::new(Status::Loading),
            posters: RefCell::new(HashMap::new()),
            queue: RefCell::new(Vec::new()),
            loading_posters: Cell::new(false),
            scroll: ScrollState::new(),
            tab: State::new(0),
            query: State::new(String::new()),
            search_scroll: ScrollState::new(),
            cat_tab: State::new(0),
            cat_scroll: ScrollState::new(),
            cat_tabs: ScrollState::new(),
            lib_tab: State::new(0),
            hist: State::new(Vec::new()),
            favs: State::new(Rc::new(Vec::new())),
            memo: RefCell::new((String::new(), Rc::new(Vec::new()))),
            lib_state: State::new(0),
            hist_scroll: ScrollState::new(),
            fav_scroll: ScrollState::new(),
            frames: RefCell::new(HashMap::new()),
            genres: State::new(Vec::new()),
            years: State::new(Vec::new()),
            filter_open: State::new(false),
            filter_scroll: ScrollState::new(),
            poster_diag: State::new(String::new()),
        });
        h.reload();
        h.load_library();
        h.load_profile();
        h
    }

    pub fn back(&self) -> bool {
        if let Some(pages::Page::Packs(p)) = self.pages.get().last() {
            if p.has_detail() {
                p.close_detail();
                return true;
            }
        }
        if let Some(pages::Page::Season(p)) = self.pages.get().last() {
            if p.pl.fullscreen.get() {
                self.pl_set_fullscreen(p, false);
                return true;
            }
        }
        if self.pop() {
            return true;
        }
        if self.filter_open.get() {
            self.filter_open.set(false);
            return true;
        }
        if self.tab.get() != 0 {
            self.tab.set(0);
            true
        } else {
            false
        }
    }

    /// Diskdagi ro'yxat darhol ko'rsatiladi, server esa fonda yangilaydi (ETag: o'zgarmagan bo'lsa deyarli bepul).
    fn reload(self: &Rc<Self>) {
        let cached = crate::data::cached_seasons();
        if !cached.is_empty() {
            self.set_items(cached);
        } else {
            self.status.set(Status::Loading);
        }
        let me = self.clone();
        spawn_bg(fetch_seasons, move |r| match r {
            Ok(Some(items)) if !items.is_empty() => me.set_items(items),
            Ok(_) => {
                if me.seasons.with(|s| s.is_empty()) {
                    me.status.set(Status::Error("Ro'yxat bo'sh".into()));
                }
            }
            Err(e) => {
                if me.seasons.with(|s| s.is_empty()) {
                    me.status.set(Status::Error(e));
                }
            }
        });
    }

    fn set_items(self: &Rc<Self>, items: Vec<Season>) {
        *self.queue.borrow_mut() = items.iter().map(|s| s.photo_url.clone()).filter(|u| !u.is_empty()).collect();
        self.seasons.set(Rc::new(items));
        self.status.set(Status::Ready);
        self.next_posters();
    }

    /// Ekranda ko'rinadigan kartalar posterlari navbatning boshiga ko'tariladi.
    fn prioritize(&self, first_row: usize, last_row: usize) {
        let seasons = self.seasons.get();
        self.prioritize_in(&seasons, first_row, last_row);
    }

    fn prioritize_in(&self, seasons: &[Season], first_row: usize, last_row: usize) {
        let have = self.posters.borrow();
        let mut front: Vec<String> = Vec::new();
        for r in first_row..=last_row {
            for s in seasons.iter().skip(2 * r).take(2) {
                if !s.photo_url.is_empty() && !have.contains_key(&s.photo_url) {
                    front.push(s.photo_url.clone());
                }
            }
        }
        if front.is_empty() {
            return;
        }
        let mut q = self.queue.borrow_mut();
        q.retain(|u| !front.contains(u));
        front.extend(q.drain(..));
        *q = front;
    }

    /// Posterlar 8 tadan navbat bilan yuklanadi (bot chatiga ko'p fayl birdaniga tushmasligi uchun).
    fn next_posters(self: &Rc<Self>) {
        if self.loading_posters.get() {
            return;
        }
        let batch: Vec<String> = {
            let mut q = self.queue.borrow_mut();
            let n = q.len().min(6);
            q.drain(..n).collect()
        };
        if batch.is_empty() {
            return;
        }
        self.loading_posters.set(true);
        let (me, session) = (self.clone(), self.session.clone());
        spawn_bg(move || load_posters(&session, &batch), move |(res, diag)| {
            if let Some(d) = diag {
                me.poster_diag.set(d);
            }
            for (url, img) in res {
                if let Some(i) = img {
                    me.posters.borrow_mut().insert(url, Rc::new(i));
                }
            }
            me.loading_posters.set(false);
            uzcoders::core::invalidate();
            me.next_posters();
        });
    }

    fn card(self: &Rc<Self>, s: &Season, h: f32) -> Element {
        let base = match self.posters.borrow().get(&s.photo_url) {
            Some(img) => image_fill(img),
            None => {
                let (a, b) = name_colors(&s.name);
                let letter: String = s.name.trim().chars().take(1).flat_map(|c| c.to_uppercase()).collect();
                column(vec![tx(letter, 84.0, Color::rgba(255, 255, 255, 40)).black()]).gradient(a, b).radius(20.0).center()
            }
        };
        let mut r1 = vec![];
        if s.rating_count > 0 {
            r1.push(badge(vec![icon(STAR, 12.0).color(GOLD), tx(format!("{:.1}", s.rating_avg), 10.5, Color::WHITE).bold()]));
        } else {
            r1.push(gap_box(0.0, 0.0));
        }
        r1.push(spacer());
        if s.views > 0 {
            r1.push(badge(vec![icon(EYE, 12.0).color(Color::rgba(255, 255, 255, 180)), tx(compact(s.views), 10.5, Color::WHITE).bold()]));
        }
        let mut top = vec![row(r1)];
        if s.age > 0 {
            let c = if s.age >= 18 { Color::hex(0xEF4444) } else if s.age >= 16 { ACCENT2 } else { GOLD };
            top.push(gap_box(0.0, 6.0));
            top.push(row(vec![tx(format!("{}+", s.age), 10.5, Color::WHITE)
                .black()
                .bg(c.with_opacity(0.92))
                .radius(7.0)
                .border(1.0, Color::rgba(255, 255, 255, 70))
                .padding_xy(7.0, 2.5)]));
        }
        let name: String = if s.name.chars().count() > 34 {
            s.name.chars().take(33).collect::<String>() + "…"
        } else {
            s.name.clone()
        };
        let bottom = column(vec![
            spacer(),
            column(vec![tx(format!("{}-bo'lim · {} ta qism", s.section_no, s.episodes), 12.0, Color::rgba(255, 255, 255, 190)), tx(name, 16.0, Color::WHITE).bold()])
                .padding_each(10.0, 30.0, 10.0, 12.0)
                .gradient(Color::rgba(0, 0, 0, 0), Color::rgba(0, 0, 0, 215))
                .radius(20.0),
        ]);
        let (me, s2) = (self.clone(), s.clone());
        stack(vec![base, column(top).padding(8.0), bottom]).weight(1.0).height(h).on_click(move || me.open_season(s2.clone()))
    }

    /// Sarlavha paneli: nom, tavsif va bo'lim nomi — lupali shisha ichida; qo'ng'iroq tugmasi alohida shisha doira.
    fn header(&self, top_inset: f32) -> Element {
        let panel = column(vec![
            row(vec![tx("ARU", 30.0, Color::WHITE).black(), tx("MEDIATV", 30.0, ACCENT2).black()]),
            tx("Anime dunyosi", 14.0, Color::rgba(255, 255, 255, 190)),
            gap_box(0.0, 6.0),
            tx("Ommabop anime", 18.0, Color::WHITE).bold(),
            if self.poster_diag.with(|d| d.is_empty()) || !self.posters.borrow().is_empty() {
                gap_box(0.0, 0.0)
            } else {
                tx(self.poster_diag.get(), 11.0, ERROR)
            },
        ])
        .lens(Color::rgba(255, 255, 255, 4), GLASS_ZOOM)
        .radius(24.0)
        .padding_xy(18.0, 12.0);
        let bell = column(vec![icon(BELL, 22.0).color(Color::rgba(255, 255, 255, 220))])
            .width(48.0)
            .height(48.0)
            .center()
            .lens(Color::rgba(255, 255, 255, 5), GLASS_ZOOM)
            .radius(24.0);
        column(vec![
            row(vec![panel.weight(1.0), bell]).gap(10.0).align(Align::Start),
            spacer(),
        ])
        .padding_each(16.0, 8.0 + top_inset, 16.0, 0.0)
    }

    /// Poster kartalari to'ri (2 ustun), faqat ko'rinadigan qatorlar quriladi.
    fn poster_grid(self: &Rc<Self>, items: Rc<Vec<Season>>, scroll: &ScrollState, card_h: f32, top: f32, bottom: f32) -> Element {
        let rows = items.len().div_ceil(2);
        let ihp = (card_h + 12.0) * density();
        let first = (scroll.offset() / ihp) as usize;
        let last = (((scroll.offset() + scroll.viewport().max(1200.0)) / ihp).ceil() as usize + 1).min(rows.saturating_sub(1));
        self.prioritize_in(&items, first.min(last), last);
        self.next_posters();
        let me = self.clone();
        lazy_column(scroll, rows, card_h + 12.0, move |r| {
            let a = me.card(&items[2 * r], card_h);
            let b = match items.get(2 * r + 1) {
                Some(s) => me.card(s, card_h),
                None => spacer().weight(1.0),
            };
            row(vec![a, b]).gap(12.0).padding_each(0.0, 0.0, 0.0, 12.0)
        })
        .padding_each(16.0, top, 16.0, bottom)
    }

    /// Filtr tanlov tugmasi: matn kengligiga qarab cho'ziladigan yarim aylana (kapsula).
    fn chip(&self, label: &str, on: bool, f: impl Fn() + 'static) -> Element {
        let e = column(vec![tx(label, 13.0, if on { Color::WHITE } else { Color::rgba(255, 255, 255, 200) }).bold()])
            .center()
            .padding_xy(16.0, 9.0)
            .radius(19.0)
            .on_click(f);
        if on {
            e.gradient(ACCENT, ACCENT2)
        } else {
            e.bg(Color::rgba(255, 255, 255, 16)).border(1.0, Color::rgba(255, 255, 255, 40))
        }
    }

    /// Kapsulaning taxminiy kengligi (dp): qatorlarga joylash uchun.
    fn chip_w(label: &str) -> f32 {
        thread_local! {
            static BOLD: Font = Font::parse(include_bytes!("../assets/fonts/Roboto-Bold.ttf")).expect("Roboto Bold");
        }
        BOLD.with(|f| f.text_width(label, 13.0)).ceil() + 34.0
    }

    /// Kalit o'zgarmasa oldingi natija qaytadi (Rc nusxasi — arzon); o'zgarsa `build` qayta hisoblaydi.
    fn memoized(&self, key: String, build: impl FnOnce(&[Season]) -> Vec<Season>) -> Rc<Vec<Season>> {
        let all = self.seasons.get();
        let key = format!("{key}|{:p}|{}", Rc::as_ptr(&all), all.len());
        {
            let m = self.memo.borrow();
            if m.0 == key {
                return m.1.clone();
            }
        }
        let v = Rc::new(build(&all));
        *self.memo.borrow_mut() = (key, v.clone());
        v
    }

    fn catalog_items(&self) -> Rc<Vec<Season>> {
        let (gs, ys, tab) = (self.genres.get(), self.years.get(), self.cat_tab.get());
        self.memoized(format!("c{tab}|{gs:?}|{ys:?}"), |all| {
            let mut v: Vec<Season> = all
                .iter()
                .filter(|s| (gs.is_empty() || s.genres.iter().any(|g| gs.contains(g))) && (ys.is_empty() || ys.contains(&s.year)))
                .cloned()
                .collect();
            match tab {
                1 => {
                    v.retain(|s| s.rating_count > 0);
                    v.sort_by(|a, b| b.rating_avg.partial_cmp(&a.rating_avg).unwrap_or(std::cmp::Ordering::Equal));
                }
                2 => {
                    v.retain(|s| s.views > 0);
                    v.sort_by(|a, b| b.views.cmp(&a.views));
                }
                3 => v.retain(|s| s.status == "Davom etmoqda"),
                4 => v.retain(|s| s.status == "Tugallangan"),
                5 => v.retain(|s| s.kind == "FILM"),
                6 => v.retain(|s| s.kind == "OVA"),
                _ => {}
            }
            v
        })
    }

    /// Katalog sarlavhasi: nom, filtr tugmasi va bo'limlar — hammasi shaffof lupali shisha kapsulalar ichida.
    fn catalog_bar(self: &Rc<Self>, top_inset: f32) -> Element {
        let sel = self.cat_tab.get();
        let n = self.genres.with(|g| g.len()) + self.years.with(|y| y.len());
        let fo = self.filter_open.clone();
        let glass = |e: Element| e.lens(Color::rgba(255, 255, 255, 5), GLASS_ZOOM);
        let filter = glass(
            row(vec![icon(MENU, 18.0).color(if n > 0 { ACCENT2 } else { Color::WHITE }), tx(if n > 0 { format!("Filtr · {n}") } else { "Filtr".into() }, 14.0, Color::WHITE).bold()])
                .gap(6.0)
                .align(Align::Center)
                .padding_xy(18.0, 14.0),
        )
        .radius(26.0)
        .on_click(move || fo.set(true));
        let title = glass(column(vec![tx("Katalog", 24.0, Color::WHITE).black()]).padding_xy(22.0, 8.0)).radius(26.0);
        let tabs: Vec<Element> = CAT_TABS
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let (t, sc) = (self.cat_tab.clone(), self.cat_scroll.clone());
                let on = i == sel;
                let e = column(vec![tx(*name, 13.5, if on { Color::WHITE } else { Color::rgba(255, 255, 255, 190) }).bold()])
                    .center()
                    .padding_xy(16.0, 9.0)
                    .radius(20.0)
                    .on_click(move || {
                        t.set(i);
                        sc.scroll_to_top();
                    });
                if on {
                    e.gradient(ACCENT, ACCENT2)
                } else {
                    e
                }
            })
            .collect();
        let tab_bar = glass(column(vec![row(tabs).gap(4.0).scroll(&self.cat_tabs)]).padding_xy(6.0, 6.0)).radius(28.0);
        column(vec![
            row(vec![title, spacer(), filter]).align(Align::Center),
            gap_box(0.0, 8.0),
            tab_bar,
            spacer(),
        ])
        .padding_each(16.0, 8.0 + top_inset, 16.0, 0.0)
    }

    /// Filtr oynasi: janr va yil kapsulalari matn kengligiga qarab qatorlarga joylanadi (shisha panel).
    fn filter_panel(self: &Rc<Self>, top_inset: f32, bottom_inset: f32) -> Element {
        let seasons = self.seasons.get();
        let mut genres: Vec<String> = seasons.iter().flat_map(|s| s.genres.clone()).collect();
        genres.sort();
        genres.dedup();
        let mut years: Vec<String> = seasons.iter().map(|s| s.year.clone()).filter(|y| !y.is_empty()).collect();
        years.sort_by(|a, b| b.cmp(a));
        years.dedup();
        let toggle = |st: &State<Vec<String>>, name: &str| {
            let (st, name) = (st.clone(), name.to_string());
            move || {
                let mut v = st.get();
                if let Some(i) = v.iter().position(|x| *x == name) {
                    v.remove(i);
                } else {
                    v.push(name.clone());
                }
                st.set(v);
            }
        };
        let (gs, ys) = (self.genres.get(), self.years.get());
        let (w, h) = window_dp();
        let avail = w - 24.0 - 36.0;
        let gap = 8.0;
        let flow = |names: &[String], sel: &[String], st: &State<Vec<String>>| -> Vec<Element> {
            let mut rows: Vec<Vec<Element>> = vec![Vec::new()];
            let mut used = 0.0f32;
            for n in names {
                let cw = Self::chip_w(n);
                if used > 0.0 && used + gap + cw > avail {
                    rows.push(Vec::new());
                    used = 0.0;
                }
                used += if used > 0.0 { gap + cw } else { cw };
                rows.last_mut().unwrap().push(self.chip(n, sel.contains(n), toggle(st, n)));
            }
            rows.into_iter().filter(|r| !r.is_empty()).map(|r| row(r).gap(gap).padding_each(0.0, 0.0, 0.0, 8.0)).collect()
        };
        let mut kids = vec![tx("Janr", 17.0, Color::WHITE).bold(), gap_box(0.0, 10.0)];
        kids.extend(flow(&genres, &gs, &self.genres));
        kids.push(gap_box(0.0, 14.0));
        kids.push(tx("Yil", 17.0, Color::WHITE).bold());
        kids.push(gap_box(0.0, 10.0));
        kids.extend(flow(&years, &ys, &self.years));
        let (g2, y2, fo) = (self.genres.clone(), self.years.clone(), self.filter_open.clone());
        let sc = self.cat_scroll.clone();
        let buttons = row(vec![
            self.chip("Tozalash", false, move || {
                g2.set(Vec::new());
                y2.set(Vec::new());
            })
            .weight(1.0),
            self.chip("Qo'llash", true, move || {
                fo.set(false);
                sc.scroll_to_top();
            })
            .weight(1.0),
        ])
        .gap(10.0);
        column(vec![
            spacer(),
            column(vec![scroll_column(&self.filter_scroll, kids).grow(1.0), gap_box(0.0, 10.0), buttons])
                .lens(Color::rgba(255, 255, 255, 14), GLASS_ZOOM)
                .radius(32.0)
                .padding_xy(18.0, 18.0)
                .height(h * 0.62)
                .margin_each(12.0, 0.0, 12.0, 12.0 + bottom_inset),
        ])
        .padding_each(0.0, top_inset, 0.0, 0.0)
    }

    /// Sevimlilar va tarix (bir marta, fonda): lib_state 0 — yuklanmoqda, 1 — tayyor, 2 — xato.
    fn load_library(self: &Rc<Self>) {
        let (me, session) = (self.clone(), self.session.clone());
        spawn_bg(move || fetch_library(&session), move |r| match r {
            Ok((favs, hist)) => {
                {
                    let have = me.posters.borrow();
                    let mut q = me.queue.borrow_mut();
                    for u in favs.iter().map(|s| &s.photo_url).chain(hist.iter().map(|h| &h.photo_url)) {
                        if !u.is_empty() && !have.contains_key(u) && !q.contains(u) {
                            q.push(u.clone());
                        }
                    }
                }
                me.favs.set(Rc::new(favs));
                me.hist.set(hist);
                me.lib_state.set(1);
                me.next_posters();
            }
            Err(_) => me.lib_state.set(2),
        });
    }

    /// 16:9 kadr: poster o'rtasidan kesib olinadi (bir marta, eslab qolinadi).
    fn frame(&self, url: &str) -> Option<Rc<Image>> {
        if let Some(f) = self.frames.borrow().get(url) {
            return Some(f.clone());
        }
        let img = self.posters.borrow().get(url)?.clone();
        let (w, h) = (img.w as usize, img.h as usize);
        let ch = (w * 9 / 16).min(h);
        let y0 = ((h - ch) as f32 * 0.3) as usize;
        let rgba = img.rgba[y0 * w * 4..(y0 + ch) * w * 4].to_vec();
        let f = Rc::new(Image { w: w as u32, h: ch as u32, rgba }.round_corners(28.0));
        self.frames.borrow_mut().insert(url.to_string(), f.clone());
        Some(f)
    }

    fn empty(icon_path: &'static str, msg: &str) -> Element {
        column(vec![icon(icon_path, 48.0).color(Color::rgba(255, 255, 255, 90)), gap_box(0.0, 12.0), tx(msg, 14.0, TEXT_DIM)]).center().padding(32.0)
    }

    /// Kutubxona sarlavhasi: nom va 3 ta bo'lim — shaffof lupali shisha kapsulalarda.
    fn library_bar(self: &Rc<Self>, top_inset: f32) -> Element {
        let sel = self.lib_tab.get();
        let glass = |e: Element| e.lens(Color::rgba(255, 255, 255, 5), GLASS_ZOOM);
        let title = glass(column(vec![tx("Kutubxona", 24.0, Color::WHITE).black()]).padding_xy(22.0, 8.0)).radius(26.0);
        let tabs: Vec<Element> = LIB_TABS
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let t = self.lib_tab.clone();
                let on = i == sel;
                let e = column(vec![tx(*name, 13.0, if on { Color::WHITE } else { Color::rgba(255, 255, 255, 190) }).bold()])
                    .center()
                    .weight(1.0)
                    .padding_xy(8.0, 11.0)
                    .radius(20.0)
                    .on_click(move || t.set(i));
                if on {
                    e.gradient(ACCENT, ACCENT2)
                } else {
                    e
                }
            })
            .collect();
        let tab_bar = glass(row(tabs).gap(4.0).padding_xy(6.0, 6.0)).radius(28.0);
        column(vec![row(vec![title, spacer()]), gap_box(0.0, 8.0), tab_bar, spacer()]).padding_each(16.0, 8.0 + top_inset, 16.0, 0.0)
    }

    fn history_view(self: &Rc<Self>, w: f32, top: f32, bottom: f32) -> Element {
        let items = self.hist.get();
        if items.is_empty() {
            return match self.lib_state.get() {
                0 => column(vec![spinner(32.0, 3.0, ACCENT2)]).center(),
                2 => Self::empty(PLAY, "Yuklab bo'lmadi · qayta kiring"),
                _ => Self::empty(PLAY, "Tarix bo'sh · ko'rgan qismlaringiz shu yerda chiqadi"),
            };
        }
        let img_w = w - 32.0 - 20.0;
        let img_h = img_w * 9.0 / 16.0;
        let card_h = img_h + 96.0;
        let me = self.clone();
        lazy_column(&self.hist_scroll, items.len(), card_h + 12.0, move |i| {
            let h = &items[i];
            let pic = match me.frame(&h.photo_url) {
                Some(f) => image_fill(&f),
                None => {
                    let (a, b) = name_colors(&h.title);
                    column(vec![]).gradient(a, b).radius(14.0)
                }
            };
            let title: String = if h.title.chars().count() > 36 { h.title.chars().take(35).collect::<String>() + "…" } else { h.title.clone() };
            column(vec![
                stack(vec![pic]).height(img_h),
                gap_box(0.0, 10.0),
                tx(title, 15.5, Color::WHITE).bold(),
                gap_box(0.0, 6.0),
                tx(format!("Oxirgi marta {}-bo'lim {}-qismni ko'rdingiz", h.section_no, h.episode_no), 12.5, Color::rgba(255, 255, 255, 175)),
                gap_box(0.0, 2.0),
                tx(format!("Sana: {}", fmt_date(h.updated_at)), 12.0, Color::rgba(255, 255, 255, 130)),
            ])
            .glass(Color::rgba(255, 255, 255, 20))
            .border(1.0, Color::rgba(255, 255, 255, 36))
            .radius(22.0)
            .padding(10.0)
            .height(card_h)
            .margin_each(0.0, 0.0, 0.0, 12.0)
        })
        .padding_each(16.0, top, 16.0, bottom)
    }

    /// Qidiruv paneli: sarlavha va yozish joyi — butunlay shaffof lupali shisha (pastgi panel kabi).
    fn search_bar(&self, top_inset: f32) -> Element {
        let q = self.query.clone();
        let mut kids = vec![
            icon(NAV[1].1, 22.0).color(Color::rgba(255, 255, 255, 190)),
            field(&self.query)
                .color(Color::WHITE)
                .size(16.0)
                .hint("Anime qidirish...")
                .hint_color(Color::rgba(255, 255, 255, 140))
                .accent(ACCENT2)
                .padding_xy(0.0, 10.0)
                .no_border()
                .weight(1.0),
        ];
        if !self.query.with(|v| v.is_empty()) {
            kids.push(icon(CLOSE, 20.0).color(Color::rgba(255, 255, 255, 190)).on_click(move || q.set(String::new())));
        }
        column(vec![
            tx("Qidiruv", 26.0, Color::WHITE).black().padding_each(6.0, 0.0, 0.0, 10.0),
            row(kids)
                .gap(10.0)
                .align(Align::Center)
                .lens(Color::rgba(255, 255, 255, 10), GLASS_ZOOM)
                .radius(26.0)
                .padding_xy(18.0, 5.0),
            spacer(),
        ])
        .padding_each(16.0, 12.0 + top_inset, 16.0, 0.0)
    }

    fn nav(self: &Rc<Self>, bottom_inset: f32) -> Element {
        let sel = self.tab.get();
        let items: Vec<Element> = NAV
            .iter()
            .enumerate()
            .map(|(i, (label, path))| {
                let on = i == sel;
                let me = self.clone();
                let mut kids = vec![icon(path, 22.0).color(if on { Color::WHITE } else { Color::rgba(255, 255, 255, 115) })];
                if on {
                    kids.push(tx(*label, 10.0, Color::WHITE).bold());
                }
                let mut e = column(kids).center().weight(1.0).height(52.0).radius(20.0).on_click(move || {
                    if i == 3 && me.tab.get() != 3 {
                        me.dl_index.set(None);
                    }
                    me.tab.set(i)
                });
                if on {
                    e = e.gradient(ACCENT, ACCENT2);
                }
                e
            })
            .collect();
        column(vec![
            spacer(),
            row(items)
                .gap(4.0)
                .lens(Color::rgba(255, 255, 255, 10), GLASS_ZOOM)
                .radius(28.0)
                .padding_xy(6.0, 10.0)
                .margin_each(16.0, 0.0, 16.0, 12.0 + bottom_inset),
        ])
    }

    /// Fayl tanlagichni ochadi; tanlangach `cb(yo'l)` chaqiriladi.
    pub(super) fn pick(&self, mime: &str, cb: impl Fn(String) + 'static) {
        *self.picked_cb.borrow_mut() = Some(Box::new(cb));
        uzcoders::android::pick_file(mime);
    }

    pub fn view(self: &Rc<Self>) -> Element {
        for path in uzcoders::android::take_picked() {
            if let Some(cb) = self.picked_cb.borrow_mut().take() {
                cb(path);
            }
        }
        if self.pages.get().is_empty() && self.tab.get() != 3 {
            self.dl_unwatch_all();
        }
        if let Some(page) = self.pages.get().last().cloned() {
            let v = self.page_view(page);
            return match self.dl_dialogs() {
                Some(d) => stack(vec![v, d]),
                None => v,
            };
        }
        let ins = safe_insets();
        let (w, _) = window_dp();
        let card_w = ((w - 32.0 - 12.0) / 2.0).max(80.0);
        let card_h = card_w * 1.5;
        let tab = self.tab.get();
        // Ro'yxat BUTUN ekranni egallaydi; sarlavha va navigatsiya uning ustida (shisha) turadi, orqasidan posterlar ko'rinadi.
        let header_h = 8.0 + 124.0;
        let content = if tab == 1 {
            let qs = self.query.get().trim().to_lowercase();
            let items = self.memoized(format!("s{qs}"), |all| if qs.is_empty() { Vec::new() } else { all.iter().filter(|s| s.name.to_lowercase().contains(&qs)).cloned().collect() });
            if items.is_empty() {
                let msg = if qs.is_empty() { "Anime nomini yozib qidiruv qiling" } else { "Qidiruv natijalari topilmadi" };
                column(vec![tx(msg, 15.0, TEXT_DIM)]).center()
            } else {
                self.poster_grid(items, &self.search_scroll, card_h, 8.0 + 90.0 + ins.top, 90.0 + ins.bottom)
            }
        } else if tab == 2 {
            let items = self.catalog_items();
            let body = if items.is_empty() {
                let msg = if self.genres.with(|g| !g.is_empty()) || self.years.with(|y| !y.is_empty()) { "Filtr bo'yicha hech narsa topilmadi" } else { "Bu bo'limda hozircha anime yo'q" };
                column(vec![tx(msg, 15.0, TEXT_DIM)]).center()
            } else {
                self.poster_grid(items, &self.cat_scroll, card_h, 8.0 + 112.0 + ins.top, 90.0 + ins.bottom)
            };
            body
        } else if tab == 3 {
            let top = 8.0 + 112.0 + ins.top;
            let bottom = 90.0 + ins.bottom;
            match self.lib_tab.get() {
                0 => self.history_view(w, top, bottom),
                1 => self.downloads_view(w, top, bottom),
                _ => {
                    let favs = self.favs.get();
                    if favs.is_empty() {
                        match self.lib_state.get() {
                            0 => column(vec![spinner(32.0, 3.0, ACCENT2)]).center(),
                            2 => Self::empty(HEART, "Yuklab bo'lmadi · qayta kiring"),
                            _ => Self::empty(HEART, "Hali sevimli bo'lim qo'shilmagan"),
                        }
                    } else {
                        self.poster_grid(favs, &self.fav_scroll, card_h, top, bottom)
                    }
                }
            }
        } else if tab == 4 {
            self.profile_view()
        } else if tab != 0 {
            column(vec![tx("Tez orada", 16.0, TEXT_DIM)]).center()
        } else {
            match self.status.get() {
                Status::Loading if self.seasons.with(|s| s.is_empty()) => column(vec![spinner(32.0, 3.0, ACCENT2)]).center(),
                Status::Error(e) if self.seasons.with(|s| s.is_empty()) => {
                    let me = self.clone();
                    column(vec![
                        tx(e, 14.0, ERROR),
                        gap_box(0.0, 14.0),
                        column(vec![tx("Qayta urinish", 15.0, Color::WHITE).bold()]).bg(ACCENT).radius(14.0).padding_xy(22.0, 12.0).on_click(move || me.reload()),
                    ])
                    .center()
                    .padding(24.0)
                }
                _ => {
                    let items = self.seasons.get();
                    let rows = items.len().div_ceil(2);
                    {
                        let ihp = (card_h + 12.0) * density();
                        let first = (self.scroll.offset() / ihp) as usize;
                        let last = (((self.scroll.offset() + self.scroll.viewport().max(1200.0)) / ihp).ceil() as usize + 1).min(rows.saturating_sub(1));
                        self.prioritize(first.min(last), last);
                        self.next_posters();
                    }
                    let me = self.clone();
                    lazy_column(&self.scroll, rows, card_h + 12.0, move |r| {
                        let a = me.card(&items[2 * r], card_h);
                        let b = match items.get(2 * r + 1) {
                            Some(s) => me.card(s, card_h),
                            None => spacer().weight(1.0),
                        };
                        row(vec![a, b]).gap(12.0).padding_each(0.0, 0.0, 0.0, 12.0)
                    })
                    .padding_each(16.0, header_h + ins.top, 16.0, 90.0 + ins.bottom)
                }
            }
        };
        // Bo'limlar orasida gorizontal surib o'tish (Kotlin `HorizontalPager` kabi).
        let content = match tab {
            2 => {
                let t = self.cat_tab.clone();
                let sc = self.cat_scroll.clone();
                content.on_swipe(move |d| {
                    let n = (t.get() as i32 + d).clamp(0, CAT_TABS.len() as i32 - 1) as usize;
                    if n != t.get() {
                        t.set(n);
                        sc.scroll_to_top();
                    }
                })
            }
            3 => {
                let t = self.lib_tab.clone();
                content.on_swipe(move |d| t.set((t.get() as i32 + d).clamp(0, LIB_TABS.len() as i32 - 1) as usize))
            }
            _ => content,
        };
        let mut layers = vec![Self::glass_backdrop(), content];
        if tab == 0 {
            layers.push(self.header(ins.top));
        } else if tab == 1 {
            layers.push(self.search_bar(ins.top));
        } else if tab == 2 {
            layers.push(self.catalog_bar(ins.top));
        } else if tab == 3 {
            layers.push(self.library_bar(ins.top));
        }
        if tab == 2 && self.filter_open.get() {
            layers.push(self.filter_panel(ins.top, ins.bottom));
        }
        if !(tab == 2 && self.filter_open.get()) {
            layers.push(self.nav(ins.bottom));
        }
        if tab == 3 {
            if let Some(d) = self.dl_dialogs() {
                layers.push(d);
            }
        }
        stack(layers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn poster(seed: u32) -> Image {
        let (w, h) = (240u32, 360u32);
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let t = (x + y + seed * 40) as f32;
                rgba.extend_from_slice(&[(128.0 + 120.0 * (t / 50.0).sin()) as u8, (128.0 + 120.0 * (t / 70.0).cos()) as u8, (60 + seed * 30) as u8, 255]);
            }
        }
        Image { w, h, rgba }.round_corners(24.0)
    }

    #[test]
    fn bosh_sahifa_chiziladi() {
        let seasons: Vec<Season> = (0..6)
            .map(|i| Season {
                anime_id: i,
                season_id: 1,
                name: ["Avatar: Aang afsonasi", "Naruto 2: Buyuk to'qnashuv", "Bleach", "One piece", "Spirited Away", "Jujutsu Kaisen"][i as usize].into(),
                section_no: 1,
                episodes: 20 + i * 40,
                photo_url: format!("p{i}.jpg"),
                rating_avg: 10.0,
                rating_count: i % 2,
                views: i * 400,
                age: [6, 13, 15, 15, 0, 16][i as usize],
                genres: vec!["Action".into()],
                year: "2020".into(),
                status: if i % 2 == 0 { "Davom etmoqda".into() } else { "Tugallangan".into() },
                kind: "TV".into(),
                free: Some(true),
            })
            .collect();
        let home = Rc::new(Home {
            session: String::new(),
            profile: State::new(Rc::new(Profile::default())),
            chips_scroll2: ScrollState::new(),
            dl: State::new(Rc::new(HashMap::new())),
            dl_watch: RefCell::new(HashSet::new()),
            dl_active: RefCell::new(HashSet::new()),
            dl_running: Cell::new(false),
            dl_index: State::new(None),
            dl_loading: Cell::new(false),
            dl_scroll: ScrollState::new(),
            del_confirm: State::new(None),
            dl_wipe_confirm: State::new(false),
            unread: State::new(0),
            picked_cb: RefCell::new(None),
            pages: State::new(Vec::new()),
            hidden: State::new(Vec::new()),
            consent: State::new(false),
            devices: State::new(None),
            settings_scroll: ScrollState::new(),
            sessions_scroll: ScrollState::new(),
            dlg: State::new(0),
            dlg_msg: State::new(String::new()),
            on_logout: Rc::new(|| {}),
            prof_scroll: ScrollState::new(),
            avatar: RefCell::new(None),
            cache_bytes: Cell::new(0),
            seasons: State::new(Rc::new(seasons)),
            status: State::new(Status::Ready),
            posters: RefCell::new((0..4).map(|i| (format!("p{i}.jpg"), Rc::new(poster(i)))).collect()),
            queue: RefCell::new(Vec::new()),
            loading_posters: Cell::new(false),
            scroll: ScrollState::new(),
            tab: State::new(0),
            query: State::new(String::new()),
            search_scroll: ScrollState::new(),
            cat_tab: State::new(0),
            cat_scroll: ScrollState::new(),
            cat_tabs: ScrollState::new(),
            lib_tab: State::new(0),
            hist: State::new(Vec::new()),
            favs: State::new(Rc::new(Vec::new())),
            memo: RefCell::new((String::new(), Rc::new(Vec::new()))),
            lib_state: State::new(0),
            hist_scroll: ScrollState::new(),
            fav_scroll: ScrollState::new(),
            frames: RefCell::new(HashMap::new()),
            genres: State::new(Vec::new()),
            years: State::new(Vec::new()),
            filter_open: State::new(false),
            filter_scroll: ScrollState::new(),
            poster_diag: State::new(String::new()),
        });
        let font = Rc::new(Font::parse(include_bytes!("../assets/fonts/Roboto-Regular.ttf")).unwrap());
        let h = home.clone();
        let mut ui = Ui::new(font, move || h.view());
        ui.add_font(700, Rc::new(Font::parse(include_bytes!("../assets/fonts/Roboto-Bold.ttf")).unwrap()));
        ui.add_font(900, Rc::new(Font::parse(include_bytes!("../assets/fonts/Roboto-Black.ttf")).unwrap()));
        ui.set_scale(1.0);
        let size = Size::new(360.0, 640.0);
        let cmds = ui.frame(size);
        assert!(cmds.iter().any(|c| matches!(c, DrawCmd::Backdrop { .. })));
        assert!(cmds.iter().filter(|c| matches!(c, DrawCmd::Image { .. })).count() >= 4);
        if let Ok(dir) = std::env::var("ARU_SNAPSHOT_DIR") {
            let mut c = ui.canvas(360, 640);
            c.clear(Color::BLACK);
            c.execute(&cmds);
            std::fs::write(format!("{dir}/6_home.png"), c.to_png()).unwrap();
            home.tab.set(1);
            home.query.set("a".into());
            let cmds = ui.frame(size);
            let mut c = ui.canvas(360, 640);
            c.clear(Color::BLACK);
            c.execute(&cmds);
            std::fs::write(format!("{dir}/7_search.png"), c.to_png()).unwrap();
            home.tab.set(2);
            home.push(pages::Page::Sessions);
            home.open_support(None);
            if let Some(pages::Page::Support(p)) = home.pages.get().last().cloned() {
                let mk = |id: &str, adm: bool, b: &str, t: i64| support::ChatMsg::test(id, adm, b, t);
                p.items_set(vec![mk("1", false, "Salom, savolim bor edi", 1_792_497_600_000), mk("2", true, "Assalomu alaykum! Qanday yordam beray?", 1_792_497_700_000), mk("3", false, "Obuna to'lovi tushmadi", 1_792_584_000_000)]);
                let cmds = ui.frame(size);
                let mut c = ui.canvas(360, 640);
                c.clear(Color::BLACK);
                c.execute(&cmds);
                std::fs::write(format!("{dir}/16_support.png"), c.to_png()).unwrap();
            }
            home.pop();
            for k in 0..3 {
                match k {
                    0 => home.open_billing(),
                    1 => home.open_edit(),
                    _ => home.open_storage(),
                }
                let cmds = ui.frame(size);
                let mut c = ui.canvas(360, 640);
                c.clear(Color::BLACK);
                c.execute(&cmds);
                std::fs::write(format!("{dir}/15_page{k}.png"), c.to_png()).unwrap();
            }
            for _ in 0..4 {
                home.pop();
            }
            home.hist.set((0..3).map(|i| HistoryItem { anime_id: i, season_id: 1, episode_no: 3 + i, section_no: 1, title: format!("Naruto {i}"), photo_url: format!("p{i}.jpg"), updated_at: 1_792_497_600_000 }).collect());
            home.lib_state.set(1);
            home.tab.set(3);
            for (n, name) in [(0, "10_lib0"), (1, "10_lib1"), (2, "10_lib2")] {
                home.lib_tab.set(n);
                let cmds = ui.frame(size);
                let mut c = ui.canvas(360, 640);
                c.clear(Color::BLACK);
                c.execute(&cmds);
                std::fs::write(format!("{dir}/{name}.png"), c.to_png()).unwrap();
            }
            home.profile.set(Rc::new(Profile { id: 1234, name: "Ogabek Raximov".into(), username: "ogabek".into(), balance: 5000, animes: 12, episodes: 340, watch_ms: 90_000_000, seasons: 20, favorites: 4, rated: 3, comments: 1, traffic: 123_456_789, ..Profile::default() }));
            home.tab.set(4);
            for (n, name) in [(0, "11_profile"), (1, "12_dialog")] {
                home.dlg.set(n);
                let cmds = ui.frame(size);
                let mut c = ui.canvas(360, 640);
                c.clear(Color::BLACK);
                c.execute(&cmds);
                std::fs::write(format!("{dir}/{name}.png"), c.to_png()).unwrap();
            }
            home.dlg.set(0);
            home.open_season(home.seasons.get()[0].clone());
            if let Some(pages::Page::Season(p)) = home.pages.get().last().cloned() {
                p.info.set(Rc::new(crate::data::SeasonInfo { season: serde_json::json!({"views_total": 1234, "fav_count": 7, "rating_count": 3, "rating_sum": 27, "turi": "TV", "yili": "2005", "janri": "Action, Fantasy", "studio": "Pierrot", "tavsif": "Uzun tavsif matni shu yerda turadi."}), rating: 9.0, my_stars: 8, is_fav: true }));
                p.eps.set(Rc::new((1..=4).map(|i| crate::data::Episode { id: i, number: i, name: "Nom".into(), views: 100 * i, qualities: vec!["720p".into(), "480p".into()], urls: vec![("720p".into(), "https://x/api/image/a.mp4".into()), ("480p".into(), "https://x/api/image/b.mp4".into())], sizes: vec![("720p".into(), 100_000_000)], origin: String::new(), intro: vec![], added_at: 0 }).collect()));
                p.sections.set(home.seasons.get());
                p.cm.items.set(Rc::new(vec![crate::data::Comment { id: "c1".into(), name: "Ali".into(), body: "Zo'r anime!".into(), created_at: 1_792_497_600_000, likes: 3, replies: 1, ..Default::default() }]));
                p.cm.status.set(1);
                p.exp.set(2);
                home.dl.set(Rc::new([("https://x/api/image/a.mp4".to_string(), crate::downloads::VStat { total: 100, downloaded: 40, downloading: true, speed: 2_000_000, ..Default::default() })].into_iter().collect()));
                for (n, name) in [(0, "13_season_info"), (1, "13_season_eps"), (2, "13_season_sec"), (3, "13_season_com")] {
                    p.tab.set(n);
                    let cmds = ui.frame(size);
                    let mut c = ui.canvas(360, 640);
                    c.clear(Color::BLACK);
                    c.execute(&cmds);
                    std::fs::write(format!("{dir}/{name}.png"), c.to_png()).unwrap();
                }
            }
            home.pop();
            home.hidden.set(vec!["anime".into()]);
            home.push(pages::Page::Settings);
            let cmds = ui.frame(size);
            let mut c = ui.canvas(360, 640);
            c.clear(Color::BLACK);
            c.execute(&cmds);
            std::fs::write(format!("{dir}/14_settings.png"), c.to_png()).unwrap();
            home.pop();
            for (n, name) in [(0, "8_catalog"), (1, "9_filter")] {
                home.filter_open.set(n == 1);
                let cmds = ui.frame(size);
                let mut c = ui.canvas(360, 640);
                c.clear(Color::BLACK);
                c.execute(&cmds);
                std::fs::write(format!("{dir}/{name}.png"), c.to_png()).unwrap();
            }
        }
    }
}
