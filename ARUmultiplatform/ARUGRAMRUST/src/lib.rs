//! UZ: ARUGRAM (Rust + uzcoders): 6 sahifali tanishtiruv va telefon raqami ekrani.
//! EN: ARUGRAM (Rust + uzcoders): a 6-page intro and the phone-number screen.
//! RU: ARUGRAM (Rust + uzcoders): знакомство из 6 страниц и экран ввода номера телефона.

pub mod art;

use std::cell::{Cell, RefCell};
use std::time::Instant;
use uzcoders::prelude::*;

static REGULAR: &[u8] = include_bytes!("../assets/fonts/Roboto-Regular.ttf");
static BOLD: &[u8] = include_bytes!("../assets/fonts/Roboto-Bold.ttf");

const SLIDE_MS: f32 = 380.0;
const THEME_MS: f32 = 520.0;
const ART_DP: f32 = 200.0;
const ART_PX: u32 = 256;

const TITLES: [&str; 6] = ["ARUGRAM", "Tez", "Bepul", "Kuchli", "Xavfsiz", "Bulutli"];
const MESSAGES: [&str; 6] = [
    "Dunyoning **eng tezkor** muloqot ilovasi.\nBu **bepul** va **xavfsiz**.",
    "**ARUGRAM** xabarlarni boshqa har qanday\nilovadan tezroq yetkazadi.",
    "**ARUGRAM** suhbatlar va media uchun bepul\ncheksiz bulutli xotira taqdim etadi.",
    "**ARUGRAM**da media va suhbatlar\nhajmiga cheklov yo'q.",
    "**ARUGRAM** xabarlaringizni xakerlik\nhujumlaridan himoya qiladi.",
    "**ARUGRAM** xabarlaringizga bir nechta\nqurilmadan kirish imkonini beradi.",
];

fn ease(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    1.0 - (1.0 - x).powi(3)
}

/// UZ: Raqamlarni `XX XXX XX XX` ko'rinishiga keltiradi. EN: Formats digits as `XX XXX XX XX`. RU: Форматирует цифры как `XX XXX XX XX`.
fn format_number(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().filter(|c| c.is_ascii_digit()).enumerate() {
        if matches!(i, 2 | 5 | 7) {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

fn digits(s: &str) -> usize {
    s.chars().filter(|c| c.is_ascii_digit()).count()
}

struct App {
    rev: State<u64>,
    page: Cell<usize>,
    from: Cell<usize>,
    slide_t0: Cell<Option<Instant>>,
    moon: Cell<bool>,
    theme_from: Cell<f32>,
    theme_t0: Cell<Option<Instant>>,
    phone_t0: Cell<Option<Instant>>,
    started: Cell<bool>,
    ticking: Cell<bool>,
    sheets: RefCell<[Option<(Rc<Image>, u32, u32)>; 6]>,
    loading: RefCell<[bool; 6]>,
    sun: Rc<Image>,
    moon_img: Rc<Image>,
    number: State<String>,
    notice: State<bool>,
}

impl App {
    fn new() -> Rc<App> {
        Rc::new(App {
            rev: State::new(0),
            page: Cell::new(0),
            from: Cell::new(0),
            slide_t0: Cell::new(None),
            moon: Cell::new(false),
            theme_from: Cell::new(0.0),
            theme_t0: Cell::new(None),
            phone_t0: Cell::new(None),
            started: Cell::new(false),
            ticking: Cell::new(false),
            sheets: RefCell::new(Default::default()),
            loading: RefCell::new([false; 6]),
            sun: Rc::new(art::theme_icon(false, 64)),
            moon_img: Rc::new(art::theme_icon(true, 64)),
            number: State::new(String::new()),
            notice: State::new(false),
        })
    }

    fn ms(t: Option<Instant>) -> Option<f32> {
        t.map(|t| t.elapsed().as_secs_f32() * 1000.0)
    }

    /// UZ: Kun/tun qiymati 0 (tun) .. 1 (kun). EN: Day/night value 0 (night) .. 1 (day). RU: Значение день/ночь 0 (ночь) .. 1 (день).
    fn day(&self) -> f32 {
        let target = if self.moon.get() { 0.0 } else { 1.0 };
        match Self::ms(self.theme_t0.get()) {
            Some(ms) if ms < THEME_MS => {
                let e = ease(ms / THEME_MS);
                self.theme_from.get() + (target - self.theme_from.get()) * e
            }
            _ => target,
        }
    }

    fn goto(self: &Rc<Self>, p: usize) {
        if p == self.page.get() || p >= 6 {
            return;
        }
        self.from.set(self.page.get());
        self.page.set(p);
        self.slide_t0.set(Some(Instant::now()));
        self.rev.set(self.rev.get() + 1);
    }

    fn toggle_theme(self: &Rc<Self>) {
        self.theme_from.set(self.day());
        self.moon.set(!self.moon.get());
        self.theme_t0.set(Some(Instant::now()));
        self.rev.set(self.rev.get() + 1);
    }

    /// UZ: Sahifa varag'ini fon oqimida tayyorlaydi (joriy ±1), uzoqdagilarini xotiradan chiqaradi.
    /// EN: Builds page sheets on a background thread (current ±1) and drops far ones from memory.
    /// RU: Готовит листы страниц в фоне (текущая ±1) и выгружает далёкие из памяти.
    fn ensure_sheets(self: &Rc<Self>) {
        let cur = self.page.get();
        for p in 0..6usize {
            let near = p.abs_diff(cur) <= 1;
            if !near {
                self.sheets.borrow_mut()[p] = None;
                continue;
            }
            if self.sheets.borrow()[p].is_some() || self.loading.borrow()[p] {
                continue;
            }
            self.loading.borrow_mut()[p] = true;
            let me = self.clone();
            spawn_bg(
                move || art::sheet(p, ART_PX),
                move |(img, cols, count)| {
                    me.loading.borrow_mut()[p] = false;
                    if me.page.get().abs_diff(p) <= 1 {
                        me.sheets.borrow_mut()[p] = Some((Rc::new(img), cols, count));
                    }
                    me.rev.set(me.rev.get() + 1);
                },
            );
        }
    }

    fn animating(&self) -> bool {
        Self::ms(self.slide_t0.get()).is_some_and(|m| m < SLIDE_MS)
            || Self::ms(self.theme_t0.get()).is_some_and(|m| m < THEME_MS)
            || Self::ms(self.phone_t0.get()).is_some_and(|m| m < SLIDE_MS)
    }

    fn drive(self: &Rc<Self>) {
        if self.animating() && !self.ticking.get() {
            self.ticking.set(true);
            let me = self.clone();
            spawn_bg(
                || std::thread::sleep(std::time::Duration::from_millis(16)),
                move |_| {
                    me.ticking.set(false);
                    me.rev.set(me.rev.get() + 1);
                },
            );
        }
    }

    fn palette(&self) -> (Color, Color, Color) {
        let d = self.day();
        let bg = Color::hex(0x111418).lerp(Color::hex(0xF4F7FB), d);
        let fg = Color::WHITE.lerp(Color::hex(0x101822), d);
        let dim = Color::hex(0xD0D6DE).lerp(Color::hex(0x4C5A6C), d);
        (bg, fg, dim)
    }

    fn message(&self, p: usize, fg: Color, dim: Color) -> Element {
        let lines: Vec<Element> = MESSAGES[p]
            .split('\n')
            .map(|line| {
                let spans: Vec<Element> = line
                    .split("**")
                    .enumerate()
                    .filter(|(_, s)| !s.is_empty())
                    .map(|(i, s)| {
                        let t = text(s).size(15.0);
                        if i % 2 == 1 { t.bold().color(fg) } else { t.color(dim) }
                    })
                    .collect();
                row(spans).justify(Justify::Center)
            })
            .collect();
        column(lines).gap(2.0).align(Align::Center)
    }

    fn intro_page(self: &Rc<Self>, p: usize, x: f32, w: f32, fg: Color, dim: Color) -> Element {
        let art = match &self.sheets.borrow()[p] {
            Some((img, cols, count)) => sprite(img, *cols, *count, art::FPS as f32, true, ART_DP, ART_DP),
            None => gap_box(ART_DP, ART_DP),
        };
        column(vec![
            art,
            gap_box(0.0, 26.0),
            text(TITLES[p]).size(if p == 0 { 30.0 } else { 26.0 }).bold().color(fg).letter_spacing(if p == 0 { 2.0 } else { 0.0 }),
            gap_box(0.0, 12.0),
            self.message(p, fg, dim),
        ])
        .align(Align::Center)
        .justify(Justify::Center)
        .width(w)
        .grow(1.0)
        .margin_each(x, 0.0, -x, 0.0)
    }

    fn dots(&self, dim: Color) -> Element {
        let cur = self.page.get();
        let blue = Color::hex(0x2A9DE8);
        let dots: Vec<Element> = (0..6)
            .map(|i| {
                let on = i == cur;
                gap_box(if on { 19.0 } else { 7.0 }, 7.0).bg(if on { blue } else { dim.with_opacity(0.4) }).radius(3.5)
            })
            .collect();
        row(dots).gap(8.0).justify(Justify::Center).align(Align::Center)
    }

    fn intro(self: &Rc<Self>, w: f32, fg: Color, dim: Color) -> Element {
        let slide = Self::ms(self.slide_t0.get()).map_or(1.0, |m| ease(m / SLIDE_MS));
        let (cur, from) = (self.page.get(), self.from.get());
        let dir = if cur > from { 1.0 } else { -1.0 };
        let mut pages = Vec::new();
        if slide < 1.0 {
            pages.push(self.intro_page(from, -dir * slide * w, w, fg, dim));
            pages.push(self.intro_page(cur, dir * (1.0 - slide) * w, w, fg, dim));
        } else {
            pages.push(self.intro_page(cur, 0.0, w, fg, dim));
        }
        let (me, me2) = (self.clone(), self.clone());
        let pager = stack(pages).grow(1.0).on_swipe(move |d| {
            let p = me.page.get() as i32 + d;
            if (0..6).contains(&p) {
                me.goto(p as usize);
            }
        });
        let icon = if self.moon.get() { &self.moon_img } else { &self.sun };
        let theme = image(icon, 28.0, 28.0);
        let top = row(vec![spacer(), column(vec![theme]).padding(10.0).on_click(move || me2.toggle_theme())]).padding_xy(10.0, 6.0);
        let me3 = self.clone();
        let cta = column(vec![text("Suhbatni boshlash").size(16.0).bold().color(Color::WHITE)])
            .center()
            .gradient(Color::hex(0x1F7FD1), Color::hex(0x4AA3D9))
            .radius(23.0)
            .height(46.0)
            .width((w - 32.0).min(330.0))
            .on_click(move || {
                me3.started.set(true);
                me3.phone_t0.set(Some(Instant::now()));
                me3.rev.set(me3.rev.get() + 1);
            });
        column(vec![
            top,
            pager,
            self.dots(dim),
            gap_box(0.0, 26.0),
            row(vec![cta]).justify(Justify::Center),
            gap_box(0.0, 18.0),
            row(vec![text("Continue in English").size(15.0).color(Color::hex(0x4AA3E0))]).justify(Justify::Center),
            gap_box(0.0, 26.0),
        ])
        .width(w)
    }

    fn phone(self: &Rc<Self>, w: f32, fg: Color, dim: Color) -> Element {
        let ready = digits(&self.number.get()) >= 9;
        let me = self.clone();
        let line = Color::hex(0x2A9DE8);
        let prefix = column(vec![text("+998").size(18.0).color(fg)]).padding_xy(14.0, 14.0).border(1.0, dim.with_opacity(0.5)).radius(12.0);
        let num = field(&self.number)
            .keyboard(KeyboardKind::Phone)
            .filter(|c| c.is_ascii_digit())
            .max_len(9)
            .format(format_number)
            .size(18.0)
            .color(fg)
            .accent(line)
            .hint("00 000 00 00")
            .hint_color(dim.with_opacity(0.6))
            .border(1.0, line)
            .weight(1.0);
        let next = column(vec![text("→").size(22.0).bold().color(Color::WHITE)])
            .center()
            .gradient(Color::hex(0x1F7FD1), Color::hex(0x4AA3D9))
            .radius(29.0)
            .width(58.0)
            .height(58.0)
            .on_click(move || {
                me.notice.set(true);
            });
        let mut kids = vec![
            gap_box(0.0, 70.0),
            text("Telefon raqamingiz").size(24.0).bold().color(fg),
            gap_box(0.0, 10.0),
            text("Davlat kodi va telefon raqamingizni kiriting.").size(15.0).color(dim),
            gap_box(0.0, 28.0),
            row(vec![prefix, num]).gap(10.0).align(Align::Center),
            gap_box(0.0, 22.0),
        ];
        if self.notice.get() && ready {
            kids.push(text("Kod yuborish va chatlar tez orada qo'shiladi.").size(15.0).color(line));
            kids.push(gap_box(0.0, 16.0));
        }
        if ready {
            kids.push(row(vec![spacer(), next]));
        }
        column(kids).padding_xy(24.0, 0.0).width(w)
    }

    fn view(self: &Rc<Self>) -> Element {
        let _ = self.rev.get();
        self.ensure_sheets();
        self.drive();
        let (w, _) = window_dp();
        let (bg, fg, dim) = self.palette();
        let body = if self.started.get() {
            let e = Self::ms(self.phone_t0.get()).map_or(1.0, |m| ease(m / SLIDE_MS));
            if e < 1.0 {
                let a = self.intro(w, fg, dim).margin_each(-e * w, 0.0, e * w, 0.0);
                let b = self.phone(w, fg, dim).margin_each((1.0 - e) * w, 0.0, -(1.0 - e) * w, 0.0);
                stack(vec![a, b])
            } else {
                self.phone(w, fg, dim)
            }
        } else {
            self.intro(w, fg, dim)
        };
        column(vec![body]).bg(bg).grow(1.0)
    }

    fn back(self: &Rc<Self>) -> bool {
        if self.started.get() {
            self.started.set(false);
            self.rev.set(self.rev.get() + 1);
            return true;
        }
        if self.page.get() > 0 {
            self.goto(self.page.get() - 1);
            return true;
        }
        false
    }
}

/// UZ: Ilovaning kirish nuqtasi: `scale` — ekran zichligi. EN: App entry point: `scale` is the screen density. RU: Точка входа: `scale` — плотность экрана.
pub fn make_ui(scale: f32) -> Ui {
    let font = Rc::new(Font::parse(REGULAR).expect("Roboto Regular"));
    let app = App::new();
    let a = app.clone();
    let mut ui = Ui::new(font, move || a.view());
    ui.add_font(700, Rc::new(Font::parse(BOLD).expect("Roboto Bold")));
    let a = app.clone();
    ui.set_back_handler(move || a.back());
    ui.set_scale(scale);
    ui
}

#[cfg(target_os = "android")]
uzcoders::android::android_app!(make_ui);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raqam_formati() {
        assert_eq!(format_number("901234567"), "90 123 45 67");
        assert_eq!(format_number("90"), "90");
    }

    #[test]
    fn ui_quriladi() {
        let mut ui = make_ui(2.0);
        let _ = &mut ui;
    }
}
