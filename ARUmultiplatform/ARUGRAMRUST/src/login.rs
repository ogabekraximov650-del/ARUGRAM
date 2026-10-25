//! UZ: Telegram (DrKLO, GPL) uslubidagi kirish oqimi: telefon → mamlakat ro'yxati → kod → parol → tayyor. O'lchamlar va vaqtlar `LoginActivity` dan.
//! EN: The Telegram-style (DrKLO, GPL) sign-in flow: phone → country list → code → password → done. Sizes and timings come from `LoginActivity`.
//! RU: Поток входа в стиле Telegram (DrKLO, GPL): телефон → список стран → код → пароль → готово. Размеры и тайминги из `LoginActivity`.

use crate::countries::{Country, COUNTRIES};
use crate::flags;
use crate::lottie_sheet::{self, Sheet};
use crate::tg::*;
use std::cell::{Cell, RefCell};
use uzcoders::prelude::*;

static LAPTOP: &str = include_str!("../assets/lottie/code_laptop.json");
static LOCK: &str = include_str!("../assets/lottie/tsv_setup_intro.json");

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Page {
    Phone,
    Code,
    Pass,
    Done,
}

#[derive(Clone, Copy, PartialEq)]
enum Pending {
    ToCode,
    ToPass,
    ToDone,
}

struct Icon {
    img: Rc<Image>,
    cols: u32,
    count: u32,
    fps: f32,
}

const CODE_LEN: usize = 5;
const SLIDE_MS: f32 = 300.0;
const SHOW_DELAY: f32 = 100.0;
const ROW_H: f32 = 51.5;
const KB_H: f32 = 225.0;

pub struct Login {
    rev: State<u64>,
    page: Cell<Page>,
    prev: Cell<Page>,
    t_page: Cell<f32>,
    t_show: Cell<f32>,
    country: Cell<Option<usize>>,
    number: State<String>,
    code: RefCell<String>,
    pass: State<String>,
    show_pass: Cell<bool>,
    picker: RefCell<Tw>,
    kb_custom: RefCell<Tw>,
    fab: RefCell<Tw>,
    prog: RefCell<Tw>,
    pending: Cell<Option<(f32, Pending)>>,
    ver_t: Cell<Option<f32>>,
    icons: RefCell<[Option<Rc<Icon>>; 2]>,
    loading: Cell<[bool; 2]>,
    press: Cell<(i32, f32)>,
    ticking: Cell<bool>,
    scroll: ScrollState,
    focus_asked: Cell<bool>,
    started: Cell<bool>,
}

fn pattern_len(c: &Country) -> usize {
    let n = c.pattern.chars().filter(|x| *x == 'X').count();
    if n == 0 { 12 } else { n }
}

fn format_with(pattern: &str, digits: &str) -> String {
    if pattern.is_empty() {
        return digits.to_string();
    }
    let mut out = String::new();
    let mut it = digits.chars();
    for p in pattern.chars() {
        if p == 'X' {
            match it.next() {
                Some(c) => out.push(c),
                None => return out.trim_end().to_string(),
            }
        } else if it.clone().next().is_some() {
            out.push(p);
        }
    }
    out.extend(it);
    out
}

/// Kichik bayroq (16 dp balandlikda) / A small flag (16 dp high) / Небольшой флаг (высота 16 dp).
fn flag_small(iso: &str) -> Element {
    match flags::flag(iso) {
        Some(img) => {
            let a = img.w as f32 / img.h as f32;
            image(&img, (16.0 * a).min(26.0), 16.0)
        }
        None => t(iso, 11.0, HINT),
    }
}

impl Login {
    pub fn new() -> Rc<Login> {
        Rc::new(Login {
            rev: State::new(0),
            page: Cell::new(Page::Phone),
            prev: Cell::new(Page::Phone),
            t_page: Cell::new(-1.0e6),
            t_show: Cell::new(0.0),
            country: Cell::new(None),
            number: State::new(String::new()),
            code: RefCell::new(String::new()),
            pass: State::new(String::new()),
            show_pass: Cell::new(false),
            picker: RefCell::new(Tw::new(0.0, acc_dec, 200.0)),
            kb_custom: RefCell::new(Tw::new(0.0, default_ease, 300.0)),
            fab: RefCell::new(Tw::new(0.0, quint, 380.0)),
            prog: RefCell::new(Tw::new(0.0, quint, 380.0)),
            pending: Cell::new(None),
            ver_t: Cell::new(None),
            icons: RefCell::new([None, None]),
            loading: Cell::new([false; 2]),
            press: Cell::new((-1, 0.0)),
            ticking: Cell::new(false),
            scroll: ScrollState::new(),
            focus_asked: Cell::new(false),
            started: Cell::new(false),
        })
    }

    fn bump(&self) {
        self.rev.set(self.rev.get() + 1);
    }

    /// UZ: Kirish oqimi ko'rsatilganda chaqiriladi (fragment ochilishi). EN: Called when the flow is presented. RU: Вызывается при показе потока.
    pub fn show(self: &Rc<Self>) {
        let now = now_ms();
        self.started.set(true);
        self.t_show.set(now);
        self.page.set(Page::Phone);
        self.prev.set(Page::Phone);
        self.t_page.set(-1.0e6);
        self.kb_custom.borrow_mut().go(1.0, now + SHOW_DELAY);
        self.fab.borrow_mut().go(1.0, now + 200.0);
        self.bump();
    }

    #[doc(hidden)]
    pub fn debug_goto(self: &Rc<Self>, p: Page) {
        self.goto(p);
    }

    #[doc(hidden)]
    pub fn debug_icons_sync(self: &Rc<Self>) {
        let scale = density();
        for (i, (raw, dp)) in [(LAPTOP, 128.0f32), (LOCK, 120.0f32)].into_iter().enumerate() {
            let layers: &[(&str, [f32; 3])] = if i == 0 { &[("Bubble", [0.369, 0.627, 0.859]), ("Phone", [1.0, 1.0, 1.0]), ("Note", [1.0, 1.0, 1.0])] } else { &[] };
            if let Ok(sh) = lottie_sheet::build(raw, (dp * scale) as u32, 30.0, layers) {
                let img = Rc::new(Image { w: sh.w, h: sh.h, rgba: sh.rgba });
                self.icons.borrow_mut()[i] = Some(Rc::new(Icon { img, cols: sh.cols, count: sh.count, fps: sh.fps }));
            }
        }
    }

    #[doc(hidden)]
    pub fn debug_picker(self: &Rc<Self>, open: bool) {
        if open { self.open_picker() } else { self.close_picker() }
    }

    #[doc(hidden)]
    pub fn debug_digits(self: &Rc<Self>, country: usize, n: &str) {
        self.country.set(Some(country));
        self.number.set(n.to_string());
    }

    pub fn started(&self) -> bool {
        self.started.get()
    }

    pub fn page(&self) -> Page {
        self.page.get()
    }

    fn goto(self: &Rc<Self>, p: Page) {
        let now = now_ms();
        self.prev.set(self.page.get());
        self.page.set(p);
        self.t_page.set(now);
        self.focus_asked.set(false);
        self.pending.set(None);
        self.prog.borrow_mut().go(0.0, now);
        match p {
            Page::Phone => {
                self.kb_custom.borrow_mut().go(1.0, now);
                self.fab.borrow_mut().go(1.0, now);
            }
            Page::Code => {
                self.kb_custom.borrow_mut().go(1.0, now);
                self.fab.borrow_mut().go(0.0, now);
                self.t_show.set(now);
                self.code.borrow_mut().clear();
                self.ver_t.set(None);
            }
            Page::Pass => {
                self.kb_custom.borrow_mut().go(0.0, now);
                self.fab.borrow_mut().go(0.0, now);
                self.t_show.set(now);
                self.pass.set(String::new());
            }
            Page::Done => {
                self.kb_custom.borrow_mut().go(0.0, now);
                self.fab.borrow_mut().go(0.0, now);
            }
        }
        self.bump();
    }

    pub fn back(self: &Rc<Self>) -> bool {
        if self.picker.borrow().val(now_ms()) > 0.5 || self.picker.borrow().active(now_ms()) {
            self.close_picker();
            return true;
        }
        match self.page.get() {
            Page::Code | Page::Pass => {
                self.goto(Page::Phone);
                true
            }
            _ => false,
        }
    }

    fn open_picker(self: &Rc<Self>) {
        let now = now_ms();
        self.picker.borrow_mut().go(1.0, now);
        self.kb_custom.borrow_mut().go(0.0, now);
        self.bump();
    }

    fn close_picker(self: &Rc<Self>) {
        let now = now_ms();
        self.picker.borrow_mut().go(0.0, now);
        self.kb_custom.borrow_mut().go(1.0, now + 100.0);
        self.bump();
    }

    fn pick(self: &Rc<Self>, i: usize) {
        self.country.set(Some(i));
        let max = pattern_len(&COUNTRIES[i]);
        let n: String = self.number.get().chars().take(max).collect();
        self.number.set(n);
        self.close_picker();
    }

    fn press_key(self: &Rc<Self>, k: i32) {
        let now = now_ms();
        self.press.set((k, now));
        let page = self.page.get();
        if page == Page::Phone {
            let mut n = self.number.get();
            if k == -2 {
                n.pop();
            } else if k >= 0 {
                let max = self.country.get().map_or(12, |c| pattern_len(&COUNTRIES[c]));
                if n.chars().count() < max && self.country.get().is_some() {
                    n.push(char::from_digit(k as u32, 10).unwrap_or('0'));
                }
            }
            self.number.set(n);
        } else if page == Page::Code {
            let mut c = self.code.borrow_mut();
            if k == -2 {
                c.pop();
            } else if k >= 0 && c.chars().count() < CODE_LEN {
                c.push(char::from_digit(k as u32, 10).unwrap_or('0'));
            }
            let full = c.chars().count() == CODE_LEN;
            drop(c);
            if full && self.ver_t.get().is_none() {
                self.ver_t.set(Some(now + 450.0));
                self.pending.set(Some((now + 450.0 + 1100.0, Pending::ToPass)));
            }
        }
        self.bump();
    }

    fn fab_tap(self: &Rc<Self>) {
        let now = now_ms();
        if self.pending.get().is_some() {
            return;
        }
        match self.page.get() {
            Page::Phone => {
                let ok = self.country.get().is_some_and(|c| self.number.get().chars().count() >= pattern_len(&COUNTRIES[c]).min(7));
                if ok {
                    self.prog.borrow_mut().go(1.0, now);
                    self.pending.set(Some((now + 1000.0, Pending::ToCode)));
                }
            }
            Page::Pass => {
                if !self.pass.get().is_empty() {
                    self.prog.borrow_mut().go(1.0, now);
                    self.pending.set(Some((now + 1000.0, Pending::ToDone)));
                }
            }
            _ => {}
        }
        self.bump();
    }

    fn animating(&self, now: f32) -> bool {
        now < self.t_page.get() + SLIDE_MS + 50.0
            || self.picker.borrow().active(now)
            || self.kb_custom.borrow().active(now)
            || self.fab.borrow().active(now)
            || self.prog.borrow().active(now)
            || self.pending.get().is_some()
            || now < self.t_show.get() + 2400.0
            || now - self.press.get().1 < 200.0
            || self.prog.borrow().val(now) > 0.01
    }

    fn drive(self: &Rc<Self>, now: f32) {
        if let Some((due, p)) = self.pending.get() {
            if now >= due {
                self.pending.set(None);
                self.goto(match p {
                    Pending::ToCode => Page::Code,
                    Pending::ToPass => Page::Pass,
                    Pending::ToDone => Page::Done,
                });
                return;
            }
        }
        if self.ticking.get() {
            return;
        }
        let ms = if self.animating(now) { 16 } else { 250 };
        self.ticking.set(true);
        let me = self.clone();
        spawn_bg(
            move || std::thread::sleep(std::time::Duration::from_millis(ms)),
            move |_| {
                me.ticking.set(false);
                me.bump();
            },
        );
    }

    fn ensure_icons(self: &Rc<Self>) {
        let (w, _) = window_dp();
        let _ = w;
        let scale = density();
        for (i, (raw, dp)) in [(LAPTOP, 128.0f32), (LOCK, 120.0f32)].into_iter().enumerate() {
            if self.icons.borrow()[i].is_some() || self.loading.get()[i] {
                continue;
            }
            let mut l = self.loading.get();
            l[i] = true;
            self.loading.set(l);
            let px = ((dp * scale).round() as u32).clamp(96, 420);
            let me = self.clone();
            spawn_bg(
                move || {
                    let layers: &[(&str, [f32; 3])] = if i == 0 {
                        &[("Bubble", [0.369, 0.627, 0.859]), ("Phone", [1.0, 1.0, 1.0]), ("Note", [1.0, 1.0, 1.0])]
                    } else {
                        &[]
                    };
                    lottie_sheet::build(raw, px, 30.0, layers)
                },
                move |r: Result<Sheet, String>| {
                    let mut l = me.loading.get();
                    l[i] = false;
                    me.loading.set(l);
                    if let Ok(s) = r {
                        let img = Rc::new(Image { w: s.w, h: s.h, rgba: s.rgba });
                        me.icons.borrow_mut()[i] = Some(Rc::new(Icon { img, cols: s.cols, count: s.count, fps: s.fps }));
                    }
                    me.bump();
                },
            );
        }
    }

    fn icon_view(&self, i: usize, dp: f32, t_start: f32, now: f32) -> Element {
        match &self.icons.borrow()[i] {
            Some(ic) => {
                let f = (((now - t_start).max(0.0) / 1000.0) * ic.fps) as u32;
                sprite_frame(&ic.img, ic.cols, ic.count, f.min(ic.count - 1), dp, dp)
            }
            None => gap_box(dp, dp),
        }
    }

    // ---------------------------------------------------------------- ko'rinish / view

    pub fn view(self: &Rc<Self>) -> Element {
        let _ = self.rev.get();
        let now = now_ms();
        self.ensure_icons();
        self.drive(now);
        let (w, h) = window_dp();
        let ins = safe_insets();
        let hu = h - ins.bottom;

        let page = self.page.get();
        let p = acc_dec(seg(now, self.t_page.get(), self.t_page.get() + SLIDE_MS));
        let mut layers: Vec<Element> = Vec::new();
        if p < 1.0 {
            layers.push(self.page_view(self.prev.get(), w, hu, now).margin_each(-p * w, 0.0, p * w, 0.0));
            layers.push(self.page_view(page, w, hu, now).margin_each((1.0 - p) * w, 0.0, -(1.0 - p) * w, 0.0));
        } else {
            layers.push(self.page_view(page, w, hu, now));
        }
        // klaviatura va tugma / keyboard and button
        let kb = self.kb_custom.borrow().val(now);
        if kb > 0.001 && matches!(page, Page::Phone | Page::Code) {
            layers.push(self.numpad(w, hu, kb));
        }
        let fabv = self.fab.borrow().val(now);
        if fabv > 0.01 {
            let sys_kb = if page == Page::Pass { 260.0 * seg(now, self.t_page.get() + 150.0, self.t_page.get() + 450.0) } else { 0.0 };
            let bottom = 21.5 + (KB_H * kb).max(sys_kb);
            layers.push(self.fab_view(w, hu, bottom, fabv, now));
        }
        // mamlakat ro'yxati / country list
        let pk = self.picker.borrow().val(now);
        if pk > 0.001 {
            layers.push(self.picker_view(w, hu, pk, now));
        }
        // fragment ochilishi (alpha) / fragment presentation (alpha)
        let appear = seg(now, self.t_show.get() - 0.0, self.t_show.get() + 200.0);
        let root = stack(layers).bg(BG).width(w).height(h);
        let _ = appear;
        root
    }

    fn page_view(self: &Rc<Self>, p: Page, w: f32, hu: f32, now: f32) -> Element {
        match p {
            Page::Phone => self.phone_view(w, now),
            Page::Code => self.code_view(w, now),
            Page::Pass => self.pass_view(w, hu, now),
            Page::Done => self.done_view(w, now),
        }
        .width(w)
        .height(hu)
    }

    /// Konturli maydon (OutlineTextContainerView): chiziq 1,5 → 2 dp, yorliq konturda / outlined field with a label on the border.
    fn outline(&self, x: f32, y: f32, w: f32, h: f32, focus: f32, label: &str, raise: f32, inner: Vec<Element>) -> Element {
        let line = LINE.lerp(ACTIVE, focus);
        let lc = HINT.lerp(ACTIVE, focus);
        let bw = lerp(1.5, 2.0, focus);
        let boxed = column(vec![]).width(w).height(h).radius(7.0).border(bw, line);
        let ly = lerp(15.0, -11.0, raise);
        let lsize = lerp(17.0, 13.0, raise);
        let mut kids = vec![boxed];
        kids.extend(inner);
        let lab = if raise > 0.01 {
            at(12.0, ly, column(vec![t(label, lsize, lc)]).bg(BG).padding_xy(4.0, 0.0))
        } else {
            at(14.0, ly, t(label, lsize, lc))
        };
        kids.push(lab);
        at(x, y, stack(kids).width(w).height(h))
    }

    fn phone_view(self: &Rc<Self>, w: f32, now: f32) -> Element {
        let me = self.clone();
        let fw = w - 73.0;
        let c = self.country.get().map(|i| &COUNTRIES[i]);
        let digits = self.number.get();
        let pat = c.map_or("", |c| c.pattern);
        let shown = format_with(pat, &digits);
        let caret_on = (now / 500.0) as i64 % 2 == 0;
        let t_in = seg(now, self.t_show.get(), self.t_show.get() + 200.0);

        // mamlakat maydoni
        let mut cin: Vec<Element> = Vec::new();
        let raise_c = if c.is_some() { 1.0 } else { 0.0 };
        if let Some(c) = c {
            cin.push(at(14.0, 0.0, column(vec![flag_small(c.iso)]).width(30.0).height(51.5).justify(Justify::Center)));
            cin.push(at(52.0, 11.0, t(c.name, 18.0, TEXT).line_height(26.0)));
        }
        cin.push(at(fw - 36.0, 13.0, icon(CHEVRON_RIGHT, 24.0).color(HINT)));
        let country_field = self.outline(36.5, 224.0, fw, 51.5, 0.0, "Mamlakat", raise_c, cin);

        // telefon maydoni
        let mut pin: Vec<Element> = Vec::new();
        pin.push(at(14.0, 0.0, t(format!("+{}", c.map_or("", |c| c.code)), 20.0, TEXT).line_height(46.0)));
        pin.push(at(76.0, 9.0, column(vec![]).width(1.0).height(30.0).bg(LINE)));
        let mut row_kids = vec![t(shown.clone(), 20.0, TEXT).line_height(46.0)];
        if c.is_some() && caret_on {
            row_kids.push(gap_box(1.0, 0.0));
            row_kids.push(column(vec![]).width(2.0).height(24.0).bg(ACTIVE));
        } else {
            row_kids.push(gap_box(3.0, 0.0));
        }
        if c.is_some() && digits.is_empty() {
            pin.push(at(90.0, 0.0, t(c.map_or("", |c| c.pattern).replace('X', "0"), 20.0, Color::hex(0x6e6e6e)).line_height(46.0)));
        }
        pin.push(at(90.0, 0.0, row(row_kids).align(Align::Center).height(46.0)));
        let phone_field = self.outline(36.5, 307.5, fw, 50.5, 1.0, "Telefon raqami", 1.0, pin);

        let me2 = me.clone();
        let country_tap = at(36.5, 224.0, column(vec![]).width(fw).height(51.5).on_click(move || me2.open_picker()));

        stack(vec![
            at(0.0, 121.0, centered(w, t("Telefon raqamingiz", 18.0, TEXT).bold().line_height(22.0))),
            at(0.0, 153.0, centered(w, t("Telefon raqamingizni kiriting", 15.0, HINT).line_height(19.5))),
            at(0.0, 172.5, centered(w, row(vec![t("yoki ", 15.0, HINT).line_height(19.5), gap_box(3.0, 0.0), t("kirish kaliti yordamida kiring ›", 15.0, BLUE).line_height(19.5)]))),
            country_field,
            phone_field,
            country_tap,
        ])
        .bg(BG.with_opacity(t_in))
    }

    fn code_view(self: &Rc<Self>, w: f32, now: f32) -> Element {
        let me = self.clone();
        let code = self.code.borrow().clone();
        let n = code.chars().count();
        let t0 = self.t_show.get();
        let ver = self.ver_t.get();
        let cell_w = 32.0;
        let gap = 10.0;
        let total = CODE_LEN as f32 * cell_w + (CODE_LEN as f32 - 1.0) * gap;
        let mut cells = Vec::new();
        for i in 0..CODE_LEN {
            let ch = code.chars().nth(i).map(|c| c.to_string()).unwrap_or_default();
            let active = i == n.min(CODE_LEN - 1) && n < CODE_LEN;
            let line = if active { ACTIVE } else { LINE };
            let cell = column(vec![t(ch, 20.0, TEXT).bold()]).width(cell_w).height(39.0).center().radius(5.0).border(1.5, line);
            cells.push(cell);
        }
        let radial = match ver {
            Some(v) if now >= v => {
                let a = quint(seg(now, v, v + 150.0));
                at(w - 48.0, 40.0, column(vec![spinner(18.0 * a.max(0.2), 2.0, BLUE)]).width(32.0).height(32.0).center())
            }
            _ => gap_box(0.0, 0.0),
        };
        stack(vec![
            at(16.0, 40.0, column(vec![icon(ARROW_BACK, 24.0).color(TEXT)]).width(32.0).height(32.0).center().on_click(move || {
                me.goto(Page::Phone);
            })),
            at((w - 128.0) / 2.0, 24.0, self.icon_view(0, 128.0, t0 + SHOW_DELAY + SLIDE_MS, now)),
            at(0.0, 167.0, centered(w, t("ARUGRAM xabarlaringizni tekshiring", 18.0, TEXT).bold().line_height(22.0))),
            at(0.0, 208.0, centered(w, t("Boshqa qurilmangizdagi +998 90 123 45 67", 14.0, SUB).line_height(20.0))),
            at(0.0, 228.0, centered(w, row(vec![t("uchun", 14.0, SUB).line_height(20.0), gap_box(4.0, 0.0), t("ARUGRAM ilovasiga", 14.0, SUB_BOLD).bold().line_height(20.0), gap_box(4.0, 0.0), t("kod yuborildi.", 14.0, SUB).line_height(20.0)]))),
            at((w - total) / 2.0, 282.0, row(cells).gap(gap)),
            at(0.0, 460.0, centered(w, t("Kodni olmadingizmi?", 15.0, BLUE))),
            radial,
        ])
    }

    fn pass_view(self: &Rc<Self>, w: f32, hu: f32, now: f32) -> Element {
        let me = self.clone();
        let t0 = self.t_show.get();
        if !self.focus_asked.get() && now >= t0 + SLIDE_MS + SHOW_DELAY {
            self.focus_asked.set(true);
            request_focus(self.pass.id());
        }
        let fw = w - 32.0;
        let mut f = field(&self.pass)
            .size(18.0)
            .color(TEXT)
            .accent(ACTIVE)
            .hint_color(HINT)
            .label("Parolni kiriting")
            .label_bg(BG)
            .border(1.5, LINE)
            .radius(7.0)
            .padding_xy(16.0, 15.0)
            .width(fw);
        if !self.show_pass.get() {
            f = f.password();
        }
        let me2 = me.clone();
        let me3 = me.clone();
        let eye = column(vec![icon(if self.show_pass.get() { EYE } else { EYE_OFF }, 24.0).color(HINT)]).width(40.0).height(40.0).center().on_click(move || {
            me3.show_pass.set(!me3.show_pass.get());
            me3.bump();
        });
        let sys_kb = 260.0 * seg(now, self.t_page.get() + 150.0, self.t_page.get() + 450.0);
        stack(vec![
            at(16.0, 40.0, column(vec![icon(ARROW_BACK, 24.0).color(TEXT)]).width(32.0).height(32.0).center().on_click(move || {
                me.goto(Page::Phone);
            })),
            at((w - 120.0) / 2.0, 96.0, self.icon_view(1, 120.0, t0 + SHOW_DELAY + SLIDE_MS, now)),
            at(0.0, 232.0, centered(w, t("Parolingiz", 18.0, TEXT).bold().line_height(22.0))),
            at(0.0, 262.0, centered(w, t("Ikki bosqichli tasdiqlash yoqilgan.", 14.0, SUB).line_height(20.0))),
            at(0.0, 282.0, centered(w, t("Hisobingiz qo'shimcha parol bilan himoyalangan.", 14.0, SUB).line_height(20.0))),
            at(16.0, 334.0, f),
            at(w - 16.0 - 44.0, 334.0 + 9.0, eye),
            at(16.0, hu - sys_kb - 52.0 - 20.0, t("Parolni unutdingizmi?", 15.0, BLUE).on_click(move || me2.bump())),
        ])
    }

    fn done_view(self: &Rc<Self>, w: f32, now: f32) -> Element {
        let a = decelerate(seg(now, self.t_page.get() + SLIDE_MS + 350.0, self.t_page.get() + SLIDE_MS + 800.0));
        stack(vec![
            at(0.0, 0.0, column(vec![at(16.0, 44.0, t("ARUGRAM", 20.0, TEXT).bold())]).width(w).height(92.0).bg(Color::hex(0x212123))),
            at(0.0, 170.0, centered(w, t("Chatlar tez orada qo'shiladi", 20.0, TEXT.with_opacity(a)).bold())),
            at(0.0, 202.0, centered(w, t("Hisobingiz tayyor.", 14.0, HINT.with_opacity(a)))),
        ])
    }

    // ---------------------------------------------------------------- klaviatura / keyboard

    fn numpad(self: &Rc<Self>, w: f32, hu: f32, vis: f32) -> Element {
        let now = now_ms();
        let kw = (w - 22.0 - 14.0) / 3.0;
        let (pk, pt) = self.press.get();
        const LET: [&str; 10] = ["+", "", "ABC", "DEF", "GHI", "JKL", "MNO", "PQRS", "TUV", "WXYZ"];
        let mut rows = Vec::new();
        for r in 0..4 {
            let mut keys = Vec::new();
            for c in 0..3 {
                let n = r * 3 + c; // 0..11
                let k: i32 = match n {
                    9 => -1,
                    10 => 0,
                    11 => -2,
                    _ => n as i32 + 1,
                };
                let me = self.clone();
                let pressed = pk == k && now - pt < 130.0;
                let bg = if k == -1 {
                    Color::TRANSPARENT
                } else if pressed {
                    KEY_PRESSED
                } else {
                    KEY
                };
                let mut kids: Vec<Element> = Vec::new();
                if k >= 0 {
                    kids.push(at(13.0, 0.0, column(vec![t(k.to_string(), 24.0, KEY_TEXT)]).width(28.0).height(48.0).center()));
                    let l = if k == 0 { "+" } else { LET[k as usize] };
                    if !l.is_empty() {
                        kids.push(at(46.0, 0.0, column(vec![t(l, if k == 0 { 15.0 } else { 15.0 }, KEY_LETTERS)]).width(60.0).height(48.0).center()));
                    }
                } else if k == -2 {
                    kids.push(at((kw - 28.0) / 2.0, 10.0, icon(BACKSPACE, 28.0).color(TEXT)));
                }
                let key = stack(kids).width(kw).height(48.0).radius(14.0).bg(bg);
                keys.push(if k == -1 { key } else { key.on_click(move || me.press_key(k)) });
            }
            rows.push(row(keys).gap(7.0));
        }
        let pad = column(rows).gap(6.5).padding_each(11.0, 0.0, 11.0, 0.0);
        at(0.0, hu - KB_H + (1.0 - vis) * KB_H, column(vec![pad]).width(w).height(KB_H).bg(BG))
    }

    fn fab_view(self: &Rc<Self>, w: f32, hu: f32, bottom: f32, vis: f32, now: f32) -> Element {
        let me = self.clone();
        let prog = self.prog.borrow().val(now);
        let size = 56.0 * lerp(0.4, 1.0, vis);
        let inner = if prog > 0.5 {
            spinner(18.0, 2.0, TEXT)
        } else {
            icon(ARROW_FWD, 26.0 * lerp(0.4, 1.0, vis)).color(TEXT)
        };
        let btn = column(vec![inner]).width(size).height(size).radius(size / 2.0).bg(BTN.with_opacity(vis)).center().on_click(move || me.fab_tap());
        let y = hu - bottom - 56.0 + (1.0 - vis) * 40.0 + (56.0 - size) / 2.0;
        at(w - 20.0 - 56.0 + (56.0 - size) / 2.0, y, btn)
    }

    // ---------------------------------------------------------------- mamlakat ro'yxati / country list

    fn picker_view(self: &Rc<Self>, w: f32, hu: f32, v: f32, _now: f32) -> Element {
        let me = self.clone();
        let me2 = self.clone();
        let list = lazy_column(&self.scroll, COUNTRIES.len(), ROW_H, move |i| {
            let c = &COUNTRIES[i];
            let me = me2.clone();
            stack(vec![
                at(35.0, 0.0, column(vec![flag_small(c.iso)]).width(30.0).height(ROW_H).justify(Justify::Center)),
                at(71.0, 0.0, column(vec![t(c.name, 18.0, TEXT)]).height(ROW_H).justify(Justify::Center)),
                at(w - 34.0 - 70.0, 0.0, column(vec![t(format!("+{}", c.code), 18.0, BLUE)]).width(70.0).height(ROW_H).justify(Justify::Center).align(Align::End)),
            ])
            .width(w)
            .height(ROW_H)
            .on_click(move || me.pick(i))
        });
        let bar = stack(vec![
            at(16.0, 18.0, icon(ARROW_BACK, 26.0).color(TEXT)),
            at(75.0, 0.0, column(vec![t("Mamlakatni tanlang", 21.0, TEXT).bold()]).height(62.5).justify(Justify::Center)),
            at(w - 44.0, 18.0, icon(SEARCH, 26.0).color(TEXT)),
        ])
        .width(w)
        .height(62.5)
        .bg(BG);
        let back = at(0.0, 0.0, column(vec![]).width(70.0).height(86.5).on_click(move || me.close_picker()));
        let body = stack(vec![at(0.0, 86.5, column(vec![list]).width(w).height(hu - 86.5)), at(0.0, 24.0, bar), back]).width(w).height(hu).bg(BG);
        // fragment: alpha 0→1, 48dp → 0
        at((1.0 - v) * 48.0, 0.0, body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lottie_varaqlari_yasaladi() {
        let layers = [("Bubble", [0.369, 0.627, 0.859]), ("Phone", [1.0, 1.0, 1.0]), ("Note", [1.0, 1.0, 1.0])];
        let a = lottie_sheet::build(LAPTOP, 128, 30.0, &layers).expect("laptop");
        assert!(a.count > 10 && a.w > 0);
        let b = lottie_sheet::build(LOCK, 120, 30.0, &[]).expect("lock");
        assert!(b.count > 10);
    }

    #[test]
    fn raqam_formati() {
        assert_eq!(format_with("XX XXX XX XX", "901234567"), "90 123 45 67");
        assert_eq!(format_with("XX XXX XX XX", "90"), "90");
        assert_eq!(format_with("", "123"), "123");
    }
}
