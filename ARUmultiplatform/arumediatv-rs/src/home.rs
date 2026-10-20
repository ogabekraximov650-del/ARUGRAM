//! Bosh sahifa: sarlavha, "Ommabop anime" va poster kartalari to'ri, pastda haqiqiy shisha (xiralashtiruvchi) navigatsiya paneli.

use crate::data::{fetch_seasons, load_posters, Season};
use crate::theme::*;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use uzcoders::prelude::*;

const BELL: &str = "M12,22c1.1,0 2,-0.9 2,-2h-4c0,1.1 0.89,2 2,2zM18,16v-5c0,-3.07 -1.64,-5.64 -4.5,-6.32V4c0,-0.83 -0.67,-1.5 -1.5,-1.5s-1.5,0.67 -1.5,1.5v0.68C7.63,5.36 6,7.92 6,11v5l-2,2v1h16v-1l-2,-2z";
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
    seasons: State<Vec<Season>>,
    status: State<Status>,
    posters: RefCell<HashMap<String, Rc<Image>>>,
    queue: RefCell<Vec<String>>,
    loading_posters: Cell<bool>,
    scroll: ScrollState,
    tab: State<usize>,
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
    pub fn new(session: String) -> Rc<Home> {
        let h = Rc::new(Home {
            session,
            seasons: State::new(Vec::new()),
            status: State::new(Status::Loading),
            posters: RefCell::new(HashMap::new()),
            queue: RefCell::new(Vec::new()),
            loading_posters: Cell::new(false),
            scroll: ScrollState::new(),
            tab: State::new(0),
            poster_diag: State::new(String::new()),
        });
        h.reload();
        h
    }

    pub fn back(&self) -> bool {
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
        self.seasons.set(items);
        self.status.set(Status::Ready);
        self.next_posters();
    }

    /// Ekranda ko'rinadigan kartalar posterlari navbatning boshiga ko'tariladi.
    fn prioritize(&self, first_row: usize, last_row: usize) {
        let seasons = self.seasons.get();
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
            let n = q.len().min(8);
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

    fn card(&self, s: &Season, h: f32) -> Element {
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
        stack(vec![base, column(top).padding(8.0), bottom]).weight(1.0).height(h).on_click(|| {})
    }

    fn header(&self, top_inset: f32) -> Element {
        let bell = column(vec![icon(BELL, 22.0).color(Color::rgba(255, 255, 255, 180))])
            .width(42.0)
            .height(42.0)
            .center()
            .bg(CARD)
            .radius(14.0)
            .border(1.0, Color::argb(0x0FFFFFFF));
        column(vec![
            row(vec![
                column(vec![
                    row(vec![tx("ARU", 30.0, Color::WHITE).black(), tx("MEDIATV", 30.0, ACCENT2).black()]),
                    tx("Anime dunyosi", 14.0, TEXT_DIM),
                ])
                .weight(1.0),
                bell,
            ])
            .align(Align::Center),
            gap_box(0.0, 14.0),
            tx("Ommabop anime", 18.0, Color::WHITE).bold().margin_each(4.0, 0.0, 0.0, 0.0),
            if self.poster_diag.with(|d| d.is_empty()) || !self.posters.borrow().is_empty() {
                gap_box(0.0, 10.0)
            } else {
                tx(self.poster_diag.get(), 11.0, ERROR).margin_each(4.0, 4.0, 0.0, 6.0)
            },
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
                let mut e = column(kids).center().weight(1.0).height(52.0).radius(20.0).on_click(move || me.tab.set(i));
                if on {
                    e = e.gradient(ACCENT, ACCENT2);
                }
                e
            })
            .collect();
        row(items)
            .gap(4.0)
            .lens(Color::rgba(255, 255, 255, 10), 1.18)
            .radius(28.0)
            .padding_xy(6.0, 10.0)
            .margin_each(16.0, -(72.0 + 12.0 + bottom_inset), 16.0, 12.0 + bottom_inset)
    }

    pub fn view(self: &Rc<Self>) -> Element {
        let ins = safe_insets();
        let (w, _) = window_dp();
        let card_w = ((w - 32.0 - 12.0) / 2.0).max(80.0);
        let card_h = card_w * 1.5;
        let tab = self.tab.get();
        let content = if tab != 0 {
            column(vec![tx("Tez orada", 16.0, TEXT_DIM)]).center().grow(1.0)
        } else {
            match self.status.get() {
                Status::Loading if self.seasons.with(|s| s.is_empty()) => column(vec![spinner(32.0, 3.0, ACCENT2)]).center().grow(1.0),
                Status::Error(e) if self.seasons.with(|s| s.is_empty()) => {
                    let me = self.clone();
                    column(vec![
                        tx(e, 14.0, ERROR),
                        gap_box(0.0, 14.0),
                        column(vec![tx("Qayta urinish", 15.0, Color::WHITE).bold()]).bg(ACCENT).radius(14.0).padding_xy(22.0, 12.0).on_click(move || me.reload()),
                    ])
                    .center()
                    .padding(24.0)
                    .grow(1.0)
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
                    .padding_each(16.0, 0.0, 16.0, 90.0 + ins.bottom)
                    .grow(1.0)
                }
            }
        };
        column(vec![if tab == 0 { self.header(ins.top) } else { gap_box(0.0, ins.top) }, content, self.nav(ins.bottom)]).bg(BG)
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
            })
            .collect();
        let home = Rc::new(Home {
            session: String::new(),
            seasons: State::new(seasons),
            status: State::new(Status::Ready),
            posters: RefCell::new((0..4).map(|i| (format!("p{i}.jpg"), Rc::new(poster(i)))).collect()),
            queue: RefCell::new(Vec::new()),
            loading_posters: Cell::new(false),
            scroll: ScrollState::new(),
            tab: State::new(0),
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
        }
    }
}
