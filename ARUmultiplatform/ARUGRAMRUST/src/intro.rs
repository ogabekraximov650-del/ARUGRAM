//! UZ: Telegram uslubidagi tanishtiruv: 6 sahifa, har birida Telegram'ning haqiqiy chizuvchisi (`intro_scene`, GPL) bilan chizilgan animatsiya.
//! EN: The Telegram-style intro: 6 pages, each animated by the real Telegram renderer (`intro_scene`, GPL).
//! RU: Интро в стиле Telegram: 6 страниц, каждая анимирована настоящим отрисовщиком Telegram (`intro_scene`, GPL).

use crate::intro_assets;
use crate::intro_scene::Scene;
use crate::lottie_sheet;
use crate::tg::*;
use std::cell::{Cell, RefCell};
use uzcoders::prelude::*;

static SUN: &str = include_str!("../assets/lottie/sun.json");

const TITLES: [&str; 6] = ["ARUGRAM", "Tez", "Bepul", "Kuchli", "Xavfsiz", "Bulutli"];
const MESSAGES: [&str; 6] = [
    "Dunyoning **eng tezkor** muloqot ilovasi.\nBu **bepul** va **xavfsiz**.",
    "**ARUGRAM** xabarlarni boshqa har qanday\nilovadan tezroq yetkazadi.",
    "**ARUGRAM** suhbatlar va media uchun bepul\ncheksiz bulutli xotira taqdim etadi.",
    "**ARUGRAM**da media va suhbatlar\nhajmiga cheklov yo'q.",
    "**ARUGRAM** xabarlaringizni xakerlik\nhujumlaridan himoya qiladi.",
    "**ARUGRAM** xabarlaringizga bir nechta\nqurilmadan kirish imkonini beradi.",
];
const SETTLE_MS: f32 = 450.0;

struct Frame {
    img: Rc<Image>,
}

pub struct Intro {
    rev: State<u64>,
    page: Cell<usize>,
    pos: Cell<f32>,
    drag: Cell<Option<(f32, f32, usize)>>, // (frac0, pos0, page0)
    last_drag: Cell<(f32, f32)>,           // (t, frac)
    vel: Cell<f32>,
    settle: Cell<Option<(f32, f32, f32)>>, // (from, to, t0)
    night: Cell<bool>,
    scene: RefCell<Option<Scene>>,
    frame: RefCell<Option<Frame>>,
    working: Cell<bool>,
    scene_night: Cell<bool>,
    last_render: Cell<f32>,
    on_start: RefCell<Option<Rc<dyn Fn()>>>,
    sun: RefCell<Option<(Rc<Image>, u32, u32, f32)>>,
    sun_loading: Cell<bool>,
    sun_t: Cell<(f32, bool)>, // (qachon bosildi, kechaga) / (when tapped, to night)
    press: Cell<f32>,
    theme_t: Cell<f32>,
    theme_from: Cell<f32>,
}


impl Intro {
    pub fn new() -> Rc<Intro> {
        Rc::new(Intro {
            rev: State::new(0),
            page: Cell::new(0),
            pos: Cell::new(0.0),
            drag: Cell::new(None),
            last_drag: Cell::new((0.0, 0.0)),
            vel: Cell::new(0.0),
            settle: Cell::new(None),
            night: Cell::new(true),
            scene: RefCell::new(None),
            frame: RefCell::new(None),
            working: Cell::new(false),
            scene_night: Cell::new(true),
            last_render: Cell::new(0.0),
            on_start: RefCell::new(None),
            sun: RefCell::new(None),
            sun_loading: Cell::new(false),
            sun_t: Cell::new((-1.0e6, true)),
            press: Cell::new(-1.0e6),
            theme_t: Cell::new(-1.0e6),
            theme_from: Cell::new(1.0),
        })
    }

    #[doc(hidden)]
    pub fn is_night(&self) -> bool {
        self.night.get()
    }

    #[doc(hidden)]
    pub fn debug_set(&self, page: usize) {
        self.page.set(page);
        self.pos.set(page as f32);
        self.settle.set(None);
    }

    fn bump(&self) {
        self.rev.set(self.rev.get() + 1);
    }

    /// UZ: "Suhbatni boshlash" bosilganda chaqiriladi. EN: Called when "Start" is tapped. RU: Вызывается при нажатии «Начать».
    pub fn set_on_start(&self, f: impl Fn() + 'static) {
        *self.on_start.borrow_mut() = Some(Rc::new(f));
    }

    pub fn back(self: &Rc<Self>) -> bool {
        let p = self.page.get();
        if p > 0 {
            self.settle_to(p - 1);
            return true;
        }
        false
    }

    /// Joriy sahifa o'zgaradi va qatlam tezda siljiydi (ViewPager `setCurrentItem`) / The page changes and the pager glides (ViewPager `setCurrentItem`).
    fn settle_to(self: &Rc<Self>, target: usize) {
        let now = now_ms();
        self.page.set(target);
        self.settle.set(Some((self.pos.get(), target as f32, now)));
        self.drag.set(None);
        self.bump();
    }

    fn on_drag(self: &Rc<Self>, frac: f32, released: bool) {
        let now = now_ms();
        let d = self.drag.get();
        match (d, released) {
            (None, false) => {
                self.settle.set(None);
                self.pos.set(self.pos.get().clamp(0.0, 5.0));
                self.drag.set(Some((frac, self.pos.get(), self.page.get())));
                self.last_drag.set((now, frac));
                self.vel.set(0.0);
            }
            (Some((f0, p0, _)), false) => {
                let (lt, lf) = self.last_drag.get();
                if now > lt {
                    self.vel.set(0.8 * self.vel.get() + 0.2 * ((frac - lf) / (now - lt) * 1000.0));
                }
                self.last_drag.set((now, frac));
                self.pos.set((p0 - (frac - f0)).clamp(0.0, 5.0));
            }
            (Some((_, p0, page0)), true) => {
                let pos = self.pos.get();
                let moved = pos - p0;
                let v = -self.vel.get(); // sahifalar/s
                let mut target = page0;
                if moved > 0.3 || (moved > 0.05 && v > 1.2) {
                    target = page0 + 1;
                } else if moved < -0.3 || (moved < -0.05 && v < -1.2) {
                    target = page0.saturating_sub(1);
                }
                self.settle_to(target.min(5));
            }
            _ => {}
        }
        self.bump();
    }

    fn toggle_theme(self: &Rc<Self>) {
        let now = now_ms();
        self.theme_from.set(self.night_level(now));
        self.night.set(!self.night.get());
        self.sun_t.set((now, self.night.get()));
        self.theme_t.set(now);
        self.bump();
    }

    /// UZ: Tun darajasi (0..1), 300 ms da silliq o'tadi. EN: Night level (0..1), eased over 300 ms. RU: Уровень ночи (0..1), плавно за 300 мс.
    fn night_level(&self, now: f32) -> f32 {
        let to = if self.night.get() { 1.0 } else { 0.0 };
        let k = seg(now, self.theme_t.get(), self.theme_t.get() + 300.0);
        lerp(self.theme_from.get(), to, ease_in_out_quad(k))
    }

    fn current_pos(&self, now: f32) -> f32 {
        if let Some((from, to, t0)) = self.settle.get() {
            let k = seg(now, t0, t0 + SETTLE_MS);
            let q = (k - 1.0).powi(5) + 1.0; // ViewPager interpolator
            if k >= 1.0 {
                self.settle.set(None);
                self.pos.set(to);
                return to;
            }
            let p = from + (to - from) * q;
            self.pos.set(p);
            return p;
        }
        self.pos.get()
    }

    fn ensure_sun(self: &Rc<Self>) {
        if self.sun.borrow().is_some() || self.sun_loading.get() {
            return;
        }
        self.sun_loading.set(true);
        let px = ((28.0 * density()).round() as u32).clamp(28, 112);
        let me = self.clone();
        spawn_bg(
            move || lottie_sheet::build(SUN, px, 60.0, &[("*", [0.141, 0.604, 0.945])]),
            move |r| {
                me.sun_loading.set(false);
                if let Ok(s) = r {
                    me.sun.borrow_mut().replace((Rc::new(Image { w: s.w, h: s.h, rgba: s.rgba }), s.cols, s.count, s.fps));
                }
                me.bump();
            },
        );
    }

    /// GL kadrini fon oqimida chizadi (CPU) va tayyor bo'lgach yangilaydi / Renders the GL frame on a worker thread and refreshes when ready.
    fn pump_scene(self: &Rc<Self>, now: f32, pos: f32) {
        if self.working.get() {
            return;
        }
        if now - self.last_render.get() < 14.0 && self.frame.borrow().is_some() {
            // keyingi kadr uchun uyg'onish / wake up for the next frame
            self.wake_later(16);
            return;
        }
        let night = self.night.get();
        let theming = now < self.theme_t.get() + 320.0;
        if self.scene_night.get() != night && !theming {
            self.scene.borrow_mut().take();
            self.scene_night.set(night);
        }
        self.working.set(true);
        let mut scene = self.scene.borrow_mut().take();
        let page = self.page.get() as i32;
        let scroll = pos - self.page.get() as f32;
        let dt = (now - self.last_render.get()).clamp(1.0, 16.0) as i32;
        self.last_render.set(now);
        let sf = density().min(2.0).max(1.0);
        let bgc = bg();
        let bg = [bgc.r as f32 / 255.0, bgc.g as f32 / 255.0, bgc.b as f32 / 255.0];
        let date = 1.0 + now / 1000.0;
        let me = self.clone();
        spawn_bg(
            move || {
                let mut sc = scene.take().unwrap_or_else(|| Scene::new(intro_assets::load(bg, sf), bg, (200.0 * sf).round() as usize, (150.0 * sf).round() as usize, sf));
                sc.set_bg(bg);
                sc.set_page(page);
                sc.set_scroll_offset(scroll);
                sc.set_date(date);
                sc.draw_frame(dt);
                let (w, h) = ((200.0 * sf).round() as u32, (150.0 * sf).round() as u32);
                let img = Image { w, h, rgba: sc.to_rgba8() };
                (sc, img)
            },
            move |(sc, img)| {
                *me.scene.borrow_mut() = Some(sc);
                *me.frame.borrow_mut() = Some(Frame { img: Rc::new(img) });
                me.working.set(false);
                me.bump();
            },
        );
    }

    fn wake_later(self: &Rc<Self>, ms: u64) {
        let me = self.clone();
        spawn_bg(move || std::thread::sleep(std::time::Duration::from_millis(ms)), move |_| me.bump());
    }

    fn text_color(&self) -> Color {
        fn mix_text() -> Color {
            Color::hex(0x000000).lerp(Color::WHITE, night_t())
        }
        mix_text()
    }

    fn message(&self, p: usize, col: Color) -> Element {
        let lines: Vec<Element> = MESSAGES[p]
            .split('\n')
            .map(|line| {
                let spans: Vec<Element> = line
                    .split("**")
                    .enumerate()
                    .filter(|(_, s)| !s.is_empty())
                    .flat_map(|(i, s)| {
                        let tx = t(s.trim(), 15.0, col).line_height(20.0);
                        let tx = if i % 2 == 1 { tx.bold() } else { tx };
                        let sp = |on: bool| gap_box(if on { 4.0 } else { 0.0 }, 0.0);
                        [sp(s.starts_with(' ')), tx, sp(s.ends_with(' '))]
                    })
                    .collect();
                row(spans).justify(Justify::Center)
            })
            .collect();
        column(lines).align(Align::Center)
    }

    pub fn view(self: &Rc<Self>) -> Element {
        let _ = self.rev.get();
        let now = now_ms();
        let (w, h) = window_dp();
        let hu = h - safe_insets().bottom;
        let pos = self.current_pos(now);
        self.pump_scene(now, pos);
        self.ensure_sun();
        let animating = self.settle.get().is_some() || self.drag.get().is_some();
        if animating {
            self.wake_later(8);
        }
        set_night_t(self.night_level(now));
        let night = night_t() > 0.5;
        let col = self.text_color();
        let q = hu / 4.0;
        let y0 = (3.0 * q - 275.0) / 2.0;

        let mut layers: Vec<Element> = Vec::new();
        // GL kadri / GL frame
        if let Some(f) = self.frame.borrow().as_ref() {
            layers.push(at((w - 200.0) / 2.0, y0, image(&f.img, 200.0, 150.0)));
        }
        // matnlar (ViewPager sahifalari) / texts (ViewPager pages)
        for i in 0..6usize {
            let x = (i as f32 - pos) * w;
            if x.abs() >= w {
                continue;
            }
            let title = if i == 0 { t(TITLES[0], 26.0, col).bold().letter_spacing(1.0) } else { t(TITLES[i], 26.0, col).bold() };
            layers.push(at(x, y0 + 179.5, centered(w, title.line_height(30.0))));
            layers.push(at(x, y0 + 219.0, column(vec![self.message(i, col)]).width(w).align(Align::Center)));
        }
        // sahifa surish maydoni (ViewPager) / the page-swipe area
        let me = self.clone();
        layers.push(at(0.0, 100.0, column(vec![]).width(w).height(y0 + 300.0).on_drag(move |f, rel| me.on_drag(f, rel))));
        // nuqtalar / dots
        let dx = (w - 66.0) / 2.0;
        let base = pos.floor().clamp(0.0, 5.0);
        let pr = (pos - base).clamp(0.0, 1.0);
        let inactive = Color::hex(0xbbbbbb).lerp(Color::hex(0x555555), night_t());
        let _ = night;
        for i in 0..6 {
            layers.push(at(dx + i as f32 * 11.0, y0 + 289.0, column(vec![]).width(5.0).height(5.0).radius(2.5).bg(inactive)));
        }
        layers.push(at(dx + base * 11.0, y0 + 289.0, column(vec![]).width(5.0 + 11.0 * pr).height(5.0).radius(2.5).bg(Color::hex(0x229af0))));
        // til havolasi va tugma / language link and the button
        let bty = 3.0 * q + (q - 48.0) / 2.0;
        layers.push(at(0.0, bty - 63.4, column(vec![t("Continue in English", 16.0, blue()).line_height(30.0)]).width(w).align(Align::Center)));
        layers.push(self.start_button(w, bty, now));
        // kun/tun tugmasi / theme button
        layers.push(self.theme_button(w, now));
        stack(layers).bg(bg()).width(w).height(h)
    }

    fn start_button(self: &Rc<Self>, w: f32, y: f32, now: f32) -> Element {
        let me = self.clone();
        let bw = w - 32.0;
        let press = {
            let t0 = self.press.get();
            let k = seg(now, t0, t0 + 100.0) * (1.0 - seg(now, t0 + 100.0, t0 + 250.0));
            1.0 - 0.02 * k
        };
        // gorizontal gradient: chap/o'ng yarim doira + oraliq bo'laklar / horizontal gradient: left/right half-circles plus slices in between
        let (c0, c1) = (btn(), btn2());
        let n = 28usize;
        let mid = bw - 48.0;
        let mut kids: Vec<Element> = Vec::new();
        kids.push(at(0.0, 0.0, column(vec![]).width(48.0).height(48.0).radius(24.0).bg(c0.lerp(c1, 24.0 / bw))));
        kids.push(at(bw - 48.0, 0.0, column(vec![]).width(48.0).height(48.0).radius(24.0).bg(c0.lerp(c1, (bw - 24.0) / bw))));
        for i in 0..n {
            let x = 24.0 + mid * i as f32 / n as f32;
            let cc = c0.lerp(c1, (x + mid / n as f32 / 2.0) / bw);
            kids.push(at(x, 0.0, column(vec![]).width(mid / n as f32 + 0.6).height(48.0).bg(cc)));
        }
        kids.push(at(0.0, 0.0, column(vec![t("Suhbatni boshlash", 15.0, Color::WHITE).bold()]).width(bw).height(48.0).center()));
        let btn = stack(kids).width(bw).height(48.0);
        let tap = column(vec![]).width(bw).height(48.0).on_click(move || {
            me.press.set(now_ms());
            if let Some(f) = me.on_start.borrow().clone() {
                f();
            }
        });
        let _ = press;
        at(16.0, y, stack(vec![btn, tap]).width(bw).height(48.0))
    }

    fn theme_button(self: &Rc<Self>, w: f32, now: f32) -> Element {
        let me = self.clone();
        let mut kids: Vec<Element> = Vec::new();
        if let Some((img, cols, count, fps)) = self.sun.borrow().as_ref() {
            let (t0, to_night) = self.sun_t.get();
            let k = ((now - t0).max(0.0) / 1000.0 * fps) as u32;
            // kunduzga: oxirgi kadrdan 0 gacha; tunga: 0 dan oxirigacha (Telegram: dark = oxirgi kadr) / dark = last frame
            let last = count - 1;
            let f = if self.sun_t.get().0 < -1.0e5 {
                if to_night { last } else { 0 }
            } else if to_night {
                k.min(last)
            } else {
                last.saturating_sub(k.min(last))
            };
            kids.push(sprite_frame(img, *cols, *count, f, 28.0, 28.0));
        }
        let ic = column(kids).width(48.0).height(48.0).center().on_click(move || me.toggle_theme());
        at(w - 4.0 - 48.0 - 10.0, 28.0 + 14.0, ic)
    }
}
