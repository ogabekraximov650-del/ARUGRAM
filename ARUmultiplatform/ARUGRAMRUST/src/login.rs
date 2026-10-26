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
const KB_H: f32 = 230.0;

pub struct Login {
    rev: State<u64>,
    page: Cell<Page>,
    prev: Cell<Page>,
    t_page: Cell<f32>,
    t_show: Cell<f32>,
    ctitle: Cell<(f32, f32, f32)>,
    code_t: Cell<[f32; CODE_LEN]>,
    code_exit: Cell<Option<(usize, char, f32)>>,
    cfocus: Cell<(usize, usize, f32)>,
    cell_clip: Vec<ScrollState>,
    country: Cell<Option<usize>>,
    code_in: RefCell<String>,
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
    /// UZ: Telegram'ga so'rov ketgan (javob kutilmoqda). EN: A request to Telegram is in flight. RU: Запрос к Telegram отправлен.
    busy: Cell<bool>,
    /// UZ: Saqlangan sessiya Telegram bilan tekshirilganmi. EN: Whether the saved session was verified with Telegram. RU: Проверена ли сохранённая сессия у Telegram.
    checked: Cell<bool>,
}

fn pattern_len(c: &Country) -> usize {
    let n = c.pattern.chars().filter(|x| *x == 'X').count();
    if n == 0 { 12 } else { n }
}

/// UZ: Telegram `afterTextChanged`: probel hint (shablon) bo'yicha qo'yiladi, hint tugagach bitta probel qo'shilib to'xtaydi. EN: Telegram's `afterTextChanged`: spaces follow the hint (pattern); past its end one space is added and it stops. RU: `afterTextChanged` Telegram: пробелы по подсказке (шаблону); после её конца добавляется один пробел и всё.
fn format_with(pattern: &str, digits: &str) -> String {
    if pattern.is_empty() {
        return digits.to_string();
    }
    let hint: Vec<char> = pattern.chars().map(|c| if c == 'X' { '0' } else { c }).collect();
    let mut b: Vec<char> = digits.chars().collect();
    let mut a = 0;
    while a < b.len() {
        if a < hint.len() {
            if hint[a] == ' ' {
                b.insert(a, ' ');
                a += 1;
            }
        } else {
            b.insert(a, ' ');
            break;
        }
        a += 1;
    }
    b.into_iter().collect()
}

/// Kichik bayroq (16 dp balandlikda) / A small flag (16 dp high) / Небольшой флаг (высота 16 dp).
/// UZ: Telegram emoji bayrog'i: 20dp kvadrat (`Emoji.drawImgSize`). EN: Telegram's emoji flag: a 20dp square (`Emoji.drawImgSize`). RU: Эмодзи-флаг Telegram: квадрат 20dp (`Emoji.drawImgSize`).
fn flag_small(iso: &str) -> Element {
    match flags::flag(iso) {
        Some(img) => {
            let aspect = img.w as f32 / img.h as f32;
            let w = (18.0 * aspect).min(28.0);
            image(&img, w, w / aspect)
        }
        None => t(iso, 11.0, hint()),
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
            ctitle: Cell::new((0.0, 0.0, -1.0e6)),
            code_t: Cell::new([-1.0e6; CODE_LEN]),
            code_exit: Cell::new(None),
            cfocus: Cell::new((0, 0, -1.0e6)),
            cell_clip: (0..CODE_LEN).map(|_| ScrollState::new()).collect(),
            country: Cell::new(None),
            code_in: RefCell::new(String::new()),
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
            busy: Cell::new(false),
            checked: Cell::new(false),
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
        // UZ: Saqlangan sessiya bo'lsa, qayta kirish talab qilinmaydi. EN: With a saved session there is no need to sign in again. RU: Если есть сохранённая сессия, повторный вход не нужен.
        if crate::account::api_ready() {
            // UZ: Kod/parol kutilayotgan paytda ilova yopilgan bo'lsa, o'sha sahifadan davom etadi. EN: If the app was closed while a code/password was awaited, resume on that page. RU: Если приложение закрыли в ожидании кода/пароля, продолжается с той же страницы.
            match crate::account::stage().map(|s| s.stage) {
                Some(s) if s == "code" => self.goto(Page::Code),
                Some(s) if s == "password" => self.goto(Page::Pass),
                _ => {}
            }
        }
    }

    /// UZ: Saqlangan hisob bor: kirish oqimi va tanishtiruv ko'rsatilmaydi, darhol "tayyor" sahifasi (Kotlin ilovasidagidek). EN: A saved account exists: no intro or sign-in, straight to the "done" page (like the Kotlin app). RU: Сохранённый аккаунт есть: без интро и входа, сразу страница «готово» (как в Kotlin-приложении).
    pub fn resume_done(self: &Rc<Self>) {
        self.started.set(true);
        self.page.set(Page::Done);
        self.prev.set(Page::Done);
        self.t_page.set(-1.0e6);
        self.t_show.set(now_ms() - 5000.0);
        self.bump();
    }

    /// UZ: Telegram xatosini ko'rsatadi va yuklanishni to'xtatadi. EN: Shows a Telegram error and stops the spinner. RU: Показывает ошибку Telegram и останавливает индикатор.
    fn fail(self: &Rc<Self>, msg: &str) {
        self.busy.set(false);
        self.pending.set(None);
        self.prog.borrow_mut().go(0.0, now_ms());
        uzcoders::android::toast(msg);
        self.bump();
    }

    #[doc(hidden)]
    pub fn debug_code(&self, digits: &str, at_ms: f32) {
        *self.code.borrow_mut() = digits.to_string();
        let mut t = self.code_t.get();
        for i in 0..digits.len().min(CODE_LEN) {
            t[i] = at_ms;
        }
        self.code_t.set(t);
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
                if crate::account::api_ready() {
                    crate::account::reset();
                }
            }
            Page::Code => {
                self.kb_custom.borrow_mut().go(1.0, now);
                self.fab.borrow_mut().go(0.0, now);
                self.t_show.set(now);
                self.code.borrow_mut().clear();
                self.code_t.set([-1.0e6; CODE_LEN]);
                self.code_exit.set(None);
                self.cfocus.set((0, 0, -1.0e6));
                self.ver_t.set(None);
            }
            Page::Pass => {
                self.kb_custom.borrow_mut().go(0.0, now);
                self.fab.borrow_mut().go(1.0, now + SLIDE_MS * 0.5);
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
        self.code_in.borrow_mut().clear();
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
            let digit = if k >= 0 { char::from_digit(k as u32, 10) } else { None };
            match self.country.get() {
                Some(ci) => {
                    let mut n = self.number.get();
                    if k == -2 {
                        if n.pop().is_none() {
                            // raqam bo'sh: davlat kodiga qaytamiz / number empty: back to editing the country code
                            let code = COUNTRIES[ci].code;
                            *self.code_in.borrow_mut() = code[..code.len() - 1].to_string();
                            self.country.set(None);
                        }
                    } else if let Some(d) = digit {
                        if n.chars().count() < pattern_len(&COUNTRIES[ci]) {
                            n.push(d);
                        }
                    }
                    self.number.set(n);
                }
                None => {
                    let mut c = self.code_in.borrow_mut();
                    if k == -2 {
                        c.pop();
                    } else if let Some(d) = digit {
                        if c.chars().count() < 4 {
                            c.push(d);
                        }
                    }
                    if let Some(i) = crate::countries::country_by_code(&c) {
                        self.country.set(Some(i));
                        c.clear();
                    }
                }
            }
        } else if page == Page::Code {
            let mut c = self.code.borrow_mut();
            if k == -2 {
                if let Some(ch) = c.pop() {
                    self.code_exit.set(Some((c.chars().count(), ch, now)));
                }
            } else if k >= 0 && c.chars().count() < CODE_LEN {
                let i = c.chars().count();
                c.push(char::from_digit(k as u32, 10).unwrap_or('0'));
                let mut t = self.code_t.get();
                t[i] = now;
                self.code_t.set(t);
            }
            let full = c.chars().count() == CODE_LEN;
            drop(c);
            if full && self.ver_t.get().is_none() {
                self.ver_t.set(Some(now + 450.0));
                if crate::account::api_ready() {
                    self.sign_in_now();
                } else {
                    self.pending.set(Some((now + 450.0 + 1100.0, Pending::ToPass)));
                }
            }
        }
        self.bump();
    }

    fn fab_tap(self: &Rc<Self>) {
        let now = now_ms();
        if self.pending.get().is_some() || self.busy.get() {
            return;
        }
        let live = crate::account::api_ready();
        match self.page.get() {
            Page::Phone => {
                let ok = self.country.get().is_some_and(|c| self.number.get().chars().count() >= pattern_len(&COUNTRIES[c]).min(7));
                if ok {
                    self.prog.borrow_mut().go(1.0, now);
                    if live {
                        self.request_code_now();
                    } else {
                        self.pending.set(Some((now + 1000.0, Pending::ToCode)));
                    }
                }
            }
            Page::Pass => {
                if !self.pass.get().is_empty() {
                    self.prog.borrow_mut().go(1.0, now);
                    if live {
                        self.check_password_now();
                    } else {
                        self.pending.set(Some((now + 1000.0, Pending::ToDone)));
                    }
                }
            }
            _ => {}
        }
        self.bump();
    }

    /// UZ: Telegram'dan kirish kodini so'raydi (fon oqimida). EN: Asks Telegram for the sign-in code (on a background thread). RU: Запрашивает код входа у Telegram (в фоновом потоке).
    fn request_code_now(self: &Rc<Self>) {
        use crate::account::{self, CodeSent};
        let Some(ci) = self.country.get() else { return };
        let phone = format!("+{}{}", COUNTRIES[ci].code, self.number.get());
        self.busy.set(true);
        let me = self.clone();
        spawn_bg(move || account::request_code(&phone), move |r| {
            me.busy.set(false);
            match r {
                Ok(CodeSent::Code { .. }) => me.goto(Page::Code),
                Ok(CodeSent::LoggedIn) => me.goto(Page::Done),
                Ok(CodeSent::NeedPassword { .. }) => me.goto(Page::Pass),
                Err(e) => me.fail(&e),
            }
        });
    }

    /// UZ: Terilgan kod bilan kiradi. EN: Signs in with the typed code. RU: Входит по набранному коду.
    fn sign_in_now(self: &Rc<Self>) {
        use crate::account::{self, SignIn};
        let code = self.code.borrow().clone();
        self.busy.set(true);
        let me = self.clone();
        spawn_bg(move || account::sign_in(&code), move |r| {
            me.busy.set(false);
            match r {
                Ok(SignIn::Done) => me.pending.set(Some((now_ms() + 300.0, Pending::ToDone))),
                Ok(SignIn::NeedPassword { .. }) => me.pending.set(Some((now_ms() + 300.0, Pending::ToPass))),
                Err(e) => {
                    // UZ: Kod noto'g'ri: kataklar tozalanadi, qayta terish mumkin. EN: Wrong code: the cells are cleared, so it can be retyped. RU: Код неверный: ячейки очищаются, можно набрать снова.
                    me.code.borrow_mut().clear();
                    me.code_t.set([-1.0e6; CODE_LEN]);
                    me.ver_t.set(None);
                    me.fail(&e);
                }
            }
        });
    }

    /// UZ: 2 bosqichli parolni tekshiradi. EN: Checks the two-step password. RU: Проверяет пароль двух шагов.
    fn check_password_now(self: &Rc<Self>) {
        let pw = self.pass.get();
        self.busy.set(true);
        let me = self.clone();
        spawn_bg(move || crate::account::check_password(&pw), move |r| {
            me.busy.set(false);
            match r {
                Ok(true) => me.pending.set(Some((now_ms() + 300.0, Pending::ToDone))),
                Ok(false) => me.fail("Parol noto'g'ri"),
                Err(e) => me.fail(&e),
            }
        });
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
        if !self.checked.get() && self.page.get() == Page::Done && crate::account::is_authorized() {
            self.checked.set(true);
            let me = self.clone();
            spawn_bg(crate::account::check_session, move |ok| {
                if !ok {
                    me.show();
                }
            });
        }
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
            // UZ: Parol sahifasida tizim klaviaturasi oynani o'zi qisqartiradi (adjustResize): `hu` allaqachon klaviatura tepasi. EN: On the password page the system keyboard already shrinks the window (adjustResize): `hu` is the keyboard top. RU: На странице пароля системная клавиатура сама уменьшает окно (adjustResize): `hu` уже верх клавиатуры.
            let bottom = 10.0 + KB_H * kb;
            layers.push(self.fab_view(w, hu, bottom, fabv, now));
        }
        // mamlakat ro'yxati / country list
        let pk = self.picker.borrow().val(now);
        if pk > 0.001 {
            layers.push(self.picker_view(w, hu, pk, now));
        }
        // fragment ochilishi (alpha) / fragment presentation (alpha)
        let appear = seg(now, self.t_show.get() - 0.0, self.t_show.get() + 200.0);
        let root = stack(layers).bg(bg()).width(w).height(h);
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
    /// UZ: `OutlineTextContainerView` (Telegram): 58dp balandlik, tepada 6dp, radius 8, chiziq qalinligi 2px..1.6667dp, sarlavha 16sp -> 12sp. EN: Telegram's `OutlineTextContainerView`: 58dp tall, 6dp top padding, radius 8, stroke 2px..1.6667dp, label 16sp -> 12sp. RU: `OutlineTextContainerView` Telegram: высота 58dp, отступ сверху 6dp, радиус 8, обводка 2px..1.6667dp, подпись 16sp -> 12sp.
    fn outline(&self, x: f32, y: f32, w: f32, label: &str, title: f32, sel: f32, force_center: bool, inner: Vec<Element>) -> Element {
        const H: f32 = 58.0;
        let reg = 2.0 / density();
        let sw = if force_center { reg } else { lerp(reg, 1.6667, sel) };
        let sel_c = if force_center { 0.0 } else { sel };
        let ttl_c = if force_center { 0.0 } else { title };
        let line_c = line().lerp(active(), sel_c);
        let label_c = hint().lerp(active(), ttl_c);
        let boxed = at(sw / 2.0, 6.0 + sw / 2.0, column(vec![]).width(w - sw).height(H - 6.0 - sw).radius(8.0 + sw / 2.0).border(sw, line_c));
        let mut kids = vec![boxed];
        kids.extend(inner);
        // sarlavha: markaz (16sp) -> tepa (12sp) / label: centre (16sp) -> top (12sp)
        let size = lerp(16.0, 12.0, title);
        let cy = lerp(31.4, 8.3, title);
        let lx = lerp(14.0, 10.0, title);
        let pad = lerp(0.0, 5.0, title);
        let lab = column(vec![t(label, size, label_c).line_height(14.0)]).bg(bg().with_opacity(title)).padding_xy(pad, 0.0);
        kids.push(at(lx + (1.0 - title) * 0.0, cy - 7.0, lab));
        at(x, y, stack(kids).width(w).height(H))
    }

    fn phone_view(self: &Rc<Self>, w: f32, now: f32) -> Element {
        let me = self.clone();
        let ins = safe_insets();
        let (_, h) = window_dp();
        let hu = h - ins.bottom;
        let c = self.country.get().map(|i| &COUNTRIES[i]);
        let digits = self.number.get();
        let pat = c.map_or("", |c| c.pattern);
        let shown = format_with(pat, &digits);
        let caret_on = (now / 500.0) as i64 % 2 == 0;
        let t_in = seg(now, self.t_show.get(), self.t_show.get() + 200.0);

        // LoginActivity: slayd 30dp tepada, pastki chegarasi klaviatura tepasi + 16; mazmun markazlanadi
        // slide view: 30dp top margin, bottom = keyboard top + 16; content is centered
        let (title_h, sub_h) = (23.1, 36.8);
        let content = title_h + 8.0 + sub_h + 25.5 + 58.0 + 14.0 + 8.0 + 58.0 + 8.0;
        let center = (ins.top + 30.0 + hu - KB_H + 16.0) / 2.0;
        let top0 = center - content / 2.0;
        let sub_top = top0 + title_h + 8.0;
        let country_y = sub_top + sub_h + 25.5;
        let phone_y = country_y + 58.0 + 14.0 + 8.0;
        let (fx, fw) = (34.0, w - 68.0);

        // mamlakat sarlavhasi: pружина (500, to'siqsiz) / country label spring
        let want = if c.is_some() { 1.0 } else { 0.0 };
        let (from, to, t0) = self.ctitle.get();
        let (from, t0) = if (to - want).abs() > 0.5 { (spring_val(from, to, now - t0), now) } else { (from, t0) };
        self.ctitle.set((from, want, t0));
        let title_c = spring_val(from, want, now - t0);

        // mamlakat maydoni: [bayroq 18.75 + 16 bo'shliq + nom] / country field: flag + gap + name
        let mut cin: Vec<Element> = Vec::new();
        if let Some(c) = c {
            cin.push(at(16.0, 32.0 - 10.0, flag_small(c.iso)));
            cin.push(at(16.0 + 18.75 + 16.0, 32.0 - 13.0, t(c.name, 16.0, txt()).line_height(26.0)));
        }
        cin.push(at(fw - 14.0 - 24.0, 32.0 - 12.0, icon(CHEVRON_RIGHT, 24.0).color(hint())));
        let country_field = self.outline(fx, country_y, fw, "Mamlakat", title_c, 0.0, true, cin);

        // telefon maydoni: "+", kod (55dp, -9 chap, 10 ichki), ajratgich, raqam / phone field
        let mut pin: Vec<Element> = Vec::new();
        let code_txt = c.map_or_else(|| self.code_in.borrow().clone(), |c| c.code.to_string());
        let ty = 32.0 - 13.0;
        pin.push(at(16.0, ty, t(format!("+{code_txt}"), 16.0, txt()).line_height(26.0)));
        pin.push(at(16.0 + 9.2 - 9.0 + 55.0 + 4.0, 32.0 - 10.0, column(vec![]).width(2.0 / density()).height(20.0).bg(line())));
        let num_x = 16.0 + 9.2 - 9.0 + 55.0 + 4.0 + 2.0 / density() + 12.0;
        let hint_full: String = pat.chars().map(|c| if c == 'X' { '0' } else { c }).collect();
        let rest: String = hint_full.chars().skip(shown.chars().count()).collect();
        let mut row_kids = vec![t(shown.clone(), 16.0, txt()).line_height(26.0)];
        if c.is_some() && caret_on {
            row_kids.push(column(vec![]).width(1.5).height(20.0).bg(active()));
        } else {
            row_kids.push(gap_box(1.5, 0.0));
        }
        if c.is_some() && !rest.is_empty() {
            row_kids.push(t(rest, 16.0, hint()).line_height(26.0));
        }
        pin.push(at(num_x, ty, row(row_kids).align(Align::Center).height(26.0)));
        let phone_field = self.outline(fx, phone_y, fw, "Telefon raqami", 1.0, 1.0, false, pin);

        let me2 = me.clone();
        let country_tap = at(fx, country_y + 6.0, column(vec![]).width(fw).height(52.0).on_click(move || me2.open_picker()));

        stack(vec![
            at(0.0, top0, centered(w, t("Telefon raqamingiz", 18.0, txt()).font_weight(500).line_height(21.1))),
            at(0.0, sub_top, centered(w, t("Telefon raqamingizni kiriting", 14.0, hint()).line_height(16.4))),
            at(0.0, sub_top + 18.4, centered(w, row(vec![t("yoki ", 14.0, hint()).line_height(16.4), gap_box(3.0, 0.0), t("kirish kaliti yordamida kiring ›", 14.0, blue()).line_height(16.4)]))),
            country_field,
            phone_field,
            country_tap,
        ])
        .bg(bg().with_opacity(t_in))
    }

    /// UZ: `LoginActivitySmsView` (ilova kodi): ikonka 128dp (-24 siljish), sarlavha 18sp, matn 14sp, 34x42 katak, oraliq 7. EN: `LoginActivitySmsView` (app code): 128dp icon (-24 shift), 18sp title, 14sp text, 34x42 cells, 7 gap. RU: `LoginActivitySmsView` (код в приложении): иконка 128dp (сдвиг -24), заголовок 18sp, текст 14sp, ячейки 34x42, зазор 7.
    fn code_view(self: &Rc<Self>, w: f32, now: f32) -> Element {
        let me = self.clone();
        let ins = safe_insets();
        let (_, h) = window_dp();
        let hu = h - ins.bottom;
        let code = self.code.borrow().clone();
        let n = code.chars().count();
        let t0 = self.t_show.get();
        let ver = self.ver_t.get();
        let y0 = ins.top + 30.0;
        let (cw, ch, gap) = (34.0f32, 42.0f32, 7.0f32);
        let total = CODE_LEN as f32 * cw + (CODE_LEN as f32 - 1.0) * gap;

        // fokus: joriy katak (SpringForce 400, to'siqsiz) / focus: current cell (SpringForce 400, no bounce)
        let cur = n.min(CODE_LEN - 1);
        let (pf, cf, tf) = self.cfocus.get();
        let (pf, tf) = if cf != cur { (cf, now) } else { (pf, tf) };
        self.cfocus.set((pf, cur, tf));
        let spr = |ms: f32| {
            let wn = 400.0f32.sqrt();
            let t = (ms / 1000.0).max(0.0);
            1.0 - (1.0 + wn * t) * (-wn * t).exp()
        };
        let focus_of = |i: usize| -> f32 {
            let k = spr(now - tf);
            if i == cur && n < CODE_LEN {
                if pf == cur { 1.0 } else { k }
            } else if i == pf && pf != cur {
                1.0 - k
            } else {
                0.0
            }
        };

        let sw = 1.5f32;
        let mut cells: Vec<Element> = Vec::new();
        let times = self.code_t.get();
        for i in 0..CODE_LEN {
            let fcs = focus_of(i);
            let lc = line().lerp(active(), fcs);
            let boxed = at(sw / 2.0, sw / 2.0, column(vec![]).width(cw - sw).height(ch - sw).radius(4.0 + sw / 2.0).border(sw, lc));
            let mut kids = vec![boxed];
            // raqam: pastdan ko'tariladi (Overshoot 1.5, 350 ms) / digit rises from below (Overshoot 1.5, 350 ms)
            if let Some(c) = code.chars().nth(i) {
                let k = seg(now, times[i], times[i] + 350.0);
                let x = k - 1.0;
                let p = x * x * (2.5 * x + 1.5) + 1.0;
                let (iw, ih) = (cw - 2.0 * sw, ch - 2.0 * sw);
                let digit = column(vec![t(c.to_string(), 20.0, txt()).font_weight(500).line_height(24.0)]).width(iw).height(ih).center();
                let clip = column(vec![stack(vec![at(0.0, ih * (1.0 - p), digit)]).width(iw).height(ih)]).scroll(&self.cell_clip[i]).width(iw).height(ih);
                kids.push(at(sw, sw, clip));
            } else if let Some((ei, ec, et)) = self.code_exit.get().filter(|(ei, _, et)| *ei == i && now < et + 220.0) {
                let _ = ei;
                let k = seg(now, et, et + 220.0);
                let sc = (1.0 - k) * 0.5 + 0.5;
                kids.push(at(0.0, 0.0, column(vec![t(ec.to_string(), 20.0 * sc, txt().with_opacity(1.0 - k)).font_weight(500)]).width(cw).height(ch).center()));
            }
            cells.push(stack(kids).width(cw).height(ch));
        }

        // ikonka 128dp: y0 + 16 - 24 / icon 128dp: y0 + 16 - 24
        let title_top = y0 + 16.0 + 112.0 + 18.0;
        let conf_top = title_top + 23.1 + 17.0;
        let code_top = conf_top + 36.8 + 32.0;
        let radial = match ver {
            Some(v) if now >= v => {
                let a = quint(seg(now, v, v + 150.0));
                at(w - 48.0, code_top + 5.0, column(vec![spinner(18.0 * a.max(0.2), 2.0, blue())]).width(32.0).height(32.0).center())
            }
            _ => gap_box(0.0, 0.0),
        };
        let bottom_y = hu - KB_H - 51.6;
        stack(vec![
            at(15.0, ins.top + 16.0, column(vec![icon(ARROW_BACK, 24.0).color(txt())]).width(24.0).height(24.0).center()),
            at(0.0, ins.top, column(vec![]).width(54.0).height(56.0).on_click(move || me.goto(Page::Phone))),
            at((w - 128.0) / 2.0, y0 + 16.0 - 24.0, self.icon_view(0, 128.0, t0 + SHOW_DELAY + SLIDE_MS, now)),
            at(0.0, title_top, centered(w, t("ARUGRAM xabarlaringizni tekshiring", 18.0, txt()).font_weight(500).line_height(21.1))),
            at(0.0, conf_top, centered(w, row(vec![t("Kodni boshqa qurilmangizdagi", 14.0, sub()).line_height(16.4)]))),
            at(0.0, conf_top + 18.4, centered(w, row(vec![t("ARUGRAM", 14.0, sub_bold()).font_weight(500).line_height(16.4), gap_box(3.5, 0.0), t("ilovasiga yubordik.", 14.0, sub()).line_height(16.4)]))),
            at((w - total) / 2.0, code_top, row(cells).gap(gap)),
            at(0.0, bottom_y, centered(w, t("Kodni olmadingizmi?", 15.0, blue()).line_height(17.6))),
            radial,
        ])
    }

    fn pass_view(self: &Rc<Self>, w: f32, _hu: f32, now: f32) -> Element {
        let me = self.clone();
        let t0 = self.t_show.get();
        if !self.focus_asked.get() && now >= t0 + SLIDE_MS + SHOW_DELAY {
            self.focus_asked.set(true);
            request_focus(self.pass.id());
        }
        let fw = w - 32.0;
        let mut f = field(&self.pass)
            .size(18.0)
            .color(txt())
            .accent(active())
            .hint_color(hint())
            .label("Parolni kiriting")
            .label_bg(bg())
            .border(1.5, line())
            .radius(7.0)
            .padding_xy(16.0, 15.0)
            .on_submit({
                let me4 = me.clone();
                move || me4.fab_tap()
            })
            .width(fw);
        if !self.show_pass.get() {
            f = f.password();
        }
        let me2 = me.clone();
        let me3 = me.clone();
        let eye = column(vec![icon(if self.show_pass.get() { EYE } else { EYE_OFF }, 24.0).color(hint())]).width(40.0).height(40.0).center().on_click(move || {
            me3.show_pass.set(!me3.show_pass.get());
            me3.bump();
        });
        stack(vec![
            at(16.0, 40.0, column(vec![icon(ARROW_BACK, 24.0).color(txt())]).width(32.0).height(32.0).center().on_click(move || {
                me.goto(Page::Phone);
            })),
            at((w - 120.0) / 2.0, 96.0, self.icon_view(1, 120.0, t0 + SHOW_DELAY + SLIDE_MS, now)),
            at(0.0, 232.0, centered(w, t("Parolingiz", 18.0, txt()).font_weight(500).line_height(22.0))),
            at(0.0, 262.0, centered(w, t("Ikki bosqichli tasdiqlash yoqilgan.", 14.0, sub()).line_height(20.0))),
            at(0.0, 282.0, centered(w, t("Hisobingiz qo'shimcha parol bilan himoyalangan.", 14.0, sub()).line_height(20.0))),
            at(16.0, 334.0, f),
            at(w - 16.0 - 44.0, 334.0 + 9.0, eye),
            at(16.0, 334.0 + 58.0 + 22.0, t("Parolni unutdingizmi?", 15.0, blue()).on_click(move || me2.bump())),
        ])
    }

    fn done_view(self: &Rc<Self>, w: f32, now: f32) -> Element {
        let a = decelerate(seg(now, self.t_page.get() + SLIDE_MS + 350.0, self.t_page.get() + SLIDE_MS + 800.0));
        stack(vec![
            at(0.0, 0.0, column(vec![at(16.0, 44.0, t("ARUGRAM", 20.0, txt()).font_weight(500))]).width(w).height(92.0).bg(bar())),
            at(0.0, 170.0, centered(w, t("Chatlar tez orada qo'shiladi", 20.0, txt().with_opacity(a)).font_weight(500))),
            at(0.0, 202.0, centered(w, t("Hisobingiz tayyor.", 14.0, hint().with_opacity(a)))),
        ])
    }

    // ---------------------------------------------------------------- klaviatura / keyboard

    /// UZ: Telegram `CustomPhoneKeyboardView` geometriyasi: 230dp, chetlar 10, tugma oralig'i 6, burchaklar 24/12. EN: Geometry of Telegram's `CustomPhoneKeyboardView`: 230dp, 10 padding, 6 gap, corner radii 24/12. RU: Геометрия `CustomPhoneKeyboardView` Telegram: 230dp, отступ 10, зазор 6, радиусы углов 24/12.
    fn numpad(self: &Rc<Self>, w: f32, hu: f32, vis: f32) -> Element {
        let now = now_ms();
        let bw = (w - 32.0) / 3.0;
        let bh = (KB_H - 42.0) / 4.0;
        let (pk, pt) = self.press.get();
        const LET: [&str; 10] = ["+", "", "ABC", "DEF", "GHI", "JKL", "MNO", "PQRS", "TUV", "WXYZ"];
        let mut kids: Vec<Element> = Vec::new();
        for n in 0..12usize {
            let (r, c) = ((n / 3) as f32, (n % 3) as f32);
            let (x, y) = (c * (bw + 6.0) + 10.0, r * (bh + 6.0) + 10.0);
            let k: i32 = match n {
                9 => -1,
                10 => 0,
                11 => -2,
                _ => n as i32 + 1,
            };
            if k == -1 {
                continue;
            }
            let col = if pk == k && now - pt < 130.0 { key_pressed() } else { key() };
            // burchak: 0 = chap-yuqori, 1 = o'ng-yuqori, 3 = o'ng-past / corner: 0 = TL, 1 = TR, 3 = BR
            let corner = match n {
                0 => Some(0),
                2 => Some(1),
                11 => Some(3),
                _ => None,
            };
            let me = self.clone();
            let mut parts: Vec<Element> = Vec::new();
            match corner {
                None => parts.push(at(0.0, 0.0, column(vec![]).width(bw).height(bh).radius(12.0).bg(col))),
                Some(cn) => {
                    parts.push(at(0.0, 0.0, column(vec![]).width(bw).height(bh).radius(24.0).bg(col)));
                    let bx = if cn == 0 { bw / 2.0 - 12.0 } else { 0.0 };
                    let cy = if cn == 3 { 0.0 } else { bh / 2.0 - 12.0 };
                    parts.push(at(bx, 0.0, column(vec![]).width(bw / 2.0 + 12.0).height(bh).radius(12.0).bg(col)));
                    parts.push(at(0.0, cy, column(vec![]).width(bw).height(bh / 2.0 + 12.0).radius(12.0).bg(col)));
                }
            }
            if k >= 0 {
                parts.push(at(0.25 * bw - 20.0, 1.0, column(vec![t(k.to_string(), 24.0, key_text())]).width(40.0).height(bh).center()));
                if !LET[k as usize].is_empty() {
                    parts.push(at(0.7 * bw - 30.0, if k == 0 { -0.5 } else { 1.0 }, column(vec![t(LET[k as usize], 14.0, key_letters())]).width(60.0).height(bh).center()));
                }
            } else {
                parts.push(at((bw - 25.0) / 2.0, (bh - 25.0) / 2.0, icon(BACKSPACE, 25.0).color(txt())));
            }
            kids.push(at(x, y, stack(parts).width(bw).height(bh).on_click(move || me.press_key(k))));
        }
        at(0.0, hu - KB_H + (1.0 - vis) * KB_H, stack(kids).width(w).height(KB_H).bg(bg()))
    }

    fn fab_view(self: &Rc<Self>, w: f32, hu: f32, bottom: f32, vis: f32, now: f32) -> Element {
        let me = self.clone();
        let prog = self.prog.borrow().val(now);
        let size = 56.0 * lerp(0.4, 1.0, vis);
        let inner = if prog > 0.5 {
            spinner(18.0, 2.0, txt())
        } else {
            icon(ARROW_FWD, 26.0 * lerp(0.4, 1.0, vis)).color(txt())
        };
        let btn = column(vec![inner]).width(size).height(size).radius(size / 2.0).bg(btn().with_opacity(vis)).center().on_click(move || me.fab_tap());
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
                at(71.0, 0.0, column(vec![t(c.name, 18.0, txt())]).height(ROW_H).justify(Justify::Center)),
                at(w - 34.0 - 70.0, 0.0, column(vec![t(format!("+{}", c.code), 18.0, blue())]).width(70.0).height(ROW_H).justify(Justify::Center).align(Align::End)),
            ])
            .width(w)
            .height(ROW_H)
            .on_click(move || me.pick(i))
        });
        let bar = stack(vec![
            at(16.0, 18.0, icon(ARROW_BACK, 26.0).color(txt())),
            at(75.0, 0.0, column(vec![t("Mamlakatni tanlang", 21.0, txt()).font_weight(500)]).height(62.5).justify(Justify::Center)),
            at(w - 44.0, 18.0, icon(SEARCH, 26.0).color(txt())),
        ])
        .width(w)
        .height(62.5)
        .bg(bg());
        let back = at(0.0, 0.0, column(vec![]).width(70.0).height(86.5).on_click(move || me.close_picker()));
        let body = stack(vec![at(0.0, 86.5, column(vec![list]).width(w).height(hu - 86.5)), at(0.0, 24.0, bar), back]).width(w).height(hu).bg(bg());
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
