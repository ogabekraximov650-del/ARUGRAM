//! Telegram orqali kirish oynasi (Kotlin ilovadagi `LoginScreen` ga mos): telefon -> kod -> (2 bosqichli parol).

use crate::backend::{Backend, CodeResult, SignIn};
use crate::countries::{Country, COUNTRIES};
use crate::theme::*;
use std::cell::RefCell;
use std::rc::Rc;
use uzcoders::prelude::*;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Step {
    Phone,
    Code,
    Password,
    Qr,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Route {
    Login,
    CountryPicker,
}

thread_local! {
    /// Raqam maydoni formatlovchisi (`fn` ko'rsatkich bo'lgani uchun joriy shablon shu yerda).
    static PATTERN: RefCell<&'static str> = const { RefCell::new("") };
}

fn format_number(s: &str) -> String {
    let digits: Vec<char> = s.chars().filter(|c| c.is_ascii_digit()).collect();
    let pattern = PATTERN.with(|p| *p.borrow());
    if pattern.is_empty() {
        return digits.into_iter().collect();
    }
    let mut out = String::new();
    let mut i = 0;
    for ch in pattern.chars() {
        if i >= digits.len() {
            break;
        }
        if ch == ' ' {
            out.push(' ');
        } else {
            out.push(digits[i]);
            i += 1;
        }
    }
    out.extend(digits[i..].iter());
    out
}

fn format_prefix(s: &str) -> String {
    let d: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    format!("+{d}")
}

fn is_digit(c: char) -> bool {
    c.is_ascii_digit()
}

fn digits_of(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_digit()).collect()
}

pub fn country_by_code(code: &str, prefer: Option<usize>) -> Option<usize> {
    if code.is_empty() {
        return None;
    }
    if let Some(p) = prefer {
        if COUNTRIES[p].code == code {
            return Some(p);
        }
    }
    const MAIN: &[(&str, &str)] = &[
        ("1", "US"),
        ("7", "RU"),
        ("44", "GB"),
        ("39", "IT"),
        ("61", "AU"),
        ("262", "RE"),
        ("358", "FI"),
        ("590", "GP"),
        ("599", "CW"),
        ("47", "NO"),
        ("212", "MA"),
    ];
    if let Some((_, iso)) = MAIN.iter().find(|(c, _)| *c == code) {
        if let Some(i) = COUNTRIES.iter().position(|c| c.iso == *iso) {
            return Some(i);
        }
    }
    COUNTRIES.iter().position(|c| c.code == code)
}

/// Material 3 `bodyLarge` uslubidagi matn: qator qutisi 24dp, harflar orasi 0.5.
fn t(s: impl Into<String>, size: f32, color: Color) -> Element {
    text(s).size(size).color(color).line_height(24.0).letter_spacing(0.5)
}

const DROP_DOWN: &str = "M7,10l5,5 5,-5z";
const ARROW_BACK: &str = "M20,11H7.83l5.59,-5.59L12,4l-8,8 8,8 1.41,-1.41L7.83,13H20v-2z";

fn pattern_len(c: &Country) -> usize {
    c.pattern.chars().filter(|x| *x != ' ').count()
}

/// Kirish oynasi holati.
pub struct Login {
    pub nav: Navigator<Route>,
    step: State<Step>,
    busy: State<bool>,
    error: State<Option<String>>,
    prefix: State<String>,
    number: State<String>,
    code: State<String>,
    password: State<String>,
    sent_info: State<String>,
    country: State<Option<usize>>,
    query: State<String>,
    scroll: ScrollState,
    picker_scroll: ScrollState,
    backend: Rc<dyn Backend>,
}

impl Login {
    pub fn new(backend: Rc<dyn Backend>) -> Rc<Login> {
        let uz = COUNTRIES.iter().position(|c| c.iso == "UZ");
        Rc::new(Login {
            nav: Navigator::new(Route::Login),
            step: State::new(Step::Phone),
            busy: State::new(false),
            error: State::new(None),
            prefix: State::new(format!("+{}", uz.map_or("998", |i| COUNTRIES[i].code))),
            number: State::new(String::new()),
            code: State::new(String::new()),
            password: State::new(String::new()),
            sent_info: State::new(String::new()),
            country: State::new(uz),
            query: State::new(String::new()),
            scroll: ScrollState::new(),
            picker_scroll: ScrollState::new(),
            backend,
        })
    }

    /// "Orqaga": birinchi navbatda ekranlar steki, keyin kirish bosqichi.
    pub fn back(&self) -> bool {
        if self.nav.pop() {
            return true;
        }
        if self.step.get() != Step::Phone && !self.busy.get() {
            self.step.set(Step::Phone);
            self.error.set(None);
            return true;
        }
        false
    }

    fn phone_digits(&self) -> String {
        digits_of(&self.number.get())
    }

    fn submit(&self) {
        if self.busy.get() {
            return;
        }
        self.error.set(None);
        match self.step.get() {
            Step::Phone => {
                let nd = self.phone_digits();
                let need = self.country.get().map_or(0, |i| pattern_len(&COUNTRIES[i]));
                let full = digits_of(&self.prefix.get()) + &nd;
                if full.len() < 8 || (need > 0 && nd.len() < need) {
                    self.error.set(Some("Raqamni to'liq kiriting".into()));
                    return;
                }
                match self.backend.request_code(&format!("+{full}")) {
                    CodeResult::Sent(info) => {
                        self.sent_info.set(info.into());
                        self.code.set(String::new());
                        self.step.set(Step::Code);
                    }
                    CodeResult::Error(e) => self.error.set(Some(e)),
                }
            }
            Step::Code => {
                let code = self.code.get();
                if code.trim().is_empty() {
                    return;
                }
                match self.backend.sign_in(code.trim()) {
                    SignIn::NeedPassword(_) => {
                        self.password.set(String::new());
                        self.step.set(Step::Password);
                    }
                    SignIn::Done => {}
                    SignIn::Error(e) => self.error.set(Some(e)),
                }
            }
            Step::Password => {
                let p = self.password.get();
                if p.is_empty() {
                    return;
                }
                if let Err(e) = self.backend.check_password(&p) {
                    self.error.set(Some(e));
                }
            }
            Step::Qr => {}
        }
    }

    pub fn view(self: &Rc<Self>) -> Element {
        match self.nav.current() {
            Route::Login => self.login_view(),
            Route::CountryPicker => self.picker_view(),
        }
    }

    fn login_view(self: &Rc<Self>) -> Element {
        // Kod maydoni o'zgargan bo'lsa davlat yangilanadi.
        let digits = digits_of(&self.prefix.get());
        let cur = self.country.get();
        let wanted = country_by_code(&digits, cur);
        if wanted != cur {
            self.country.set(wanted);
        }
        let country = self.country.get().map(|i| &COUNTRIES[i]);
        PATTERN.with(|p| *p.borrow_mut() = country.map_or("", |c| c.pattern));

        let step = self.step.get();
        let subtitle = match step {
            Step::Phone => "Telegram raqamingizni kiriting".to_string(),
            Step::Code => format!(
                "{}\n{} {}",
                self.sent_info.get(),
                self.prefix.get(),
                self.number.get()
            ),
            Step::Password => "Ikki bosqichli parol".to_string(),
            Step::Qr => "QR kodni Telegram bilan skanerlang".to_string(),
        };

        let me = self.clone();
        let back = icon(ARROW_BACK, 28.0).color(Color::WHITE).on_click(move || {
            me.back();
        });

        let mut body: Vec<Element> = vec![
            row(vec![back]),
            gap_box(0.0, 24.0),
            row(vec![
                t("ARU", 30.0, Color::WHITE).black(),
                t("MEDIATV", 30.0, ACCENT2).black(),
            ]),
            gap_box(0.0, 6.0),
            t(subtitle, 15.0, TEXT_DIM),
            gap_box(0.0, 28.0),
        ];

        match step {
            Step::Phone => {
                body.push(
                    t(
                        "Davlatni tanlang va Telegram hisobingiz ulangan telefon raqamini kiriting.",
                        14.0,
                        TEXT_DIM,
                    ),
                );
                body.push(gap_box(0.0, 14.0));
                let me = self.clone();
                let (iso, name, name_color) = match country {
                    Some(c) => (c.iso, c.name.to_string(), Color::WHITE),
                    None if digits.is_empty() => ("", "Davlatni tanlang".into(), ERROR),
                    None => ("", "Noto'g'ri kod".into(), ERROR),
                };
                body.push(
                    row(vec![
                        t(iso, 14.0, TEXT_DIM).width(30.0),
                        t(name, 16.0, name_color).weight(1.0),
                        icon(DROP_DOWN, 20.0).color(TEXT_DIM),
                    ])
                    .gap(8.0)
                    .align(Align::Center)
                    .border(1.0, BORDER_BRIGHT)
                    .radius(14.0)
                    .padding_xy(14.0, 14.0)
                    .on_click(move || {
                        me.query.set(String::new());
                        me.picker_scroll.scroll_to_top();
                        me.nav.push(Route::CountryPicker);
                    }),
                );
                body.push(gap_box(0.0, 12.0));
                let hint = country
                    .map(|c| c.pattern.replace('X', "0"))
                    .filter(|h| !h.is_empty())
                    .unwrap_or_else(|| "Telefon raqam".into());
                let me = self.clone();
                body.push(
                    row(vec![
                        self.styled(field(&self.prefix))
                            .filter(|c| c.is_ascii_digit() || c == '+')
                            .max_len(5)
                            .format(format_prefix)
                            .label("Kod")
                            .width(96.0),
                        self.styled(field(&self.number))
                            .filter(is_digit)
                            .max_len(country.map_or(15, |c| pattern_len(c).max(1)))
                            .format(format_number)
                            .label(hint)
                            .weight(1.0)
                            .on_submit(move || me.submit()),
                    ])
                    .gap(10.0)
                    .align(Align::Center),
                );
            }
            Step::Code => {
                let me = self.clone();
                body.push(
                    self.styled(field(&self.code))
                        .filter(is_digit)
                        .max_len(8)
                        .label("Kod")
                        .on_submit(move || me.submit()),
                );
            }
            Step::Qr => {
                body.push(
                    column(vec![
                        t("QR kod Telegram yadrosi ulangach chiqadi", 13.0, TEXT_DIM),
                    ])
                    .center()
                    .border(1.0, BORDER_BRIGHT)
                    .radius(16.0)
                    .width(260.0)
                    .height(260.0),
                );
                body.push(gap_box(0.0, 12.0));
                body.push(t(
                    "Boshqa qurilmada Telegram -> Sozlamalar -> Qurilmalar -> Qurilmani ulash",
                    12.0,
                    TEXT_DIM,
                ));
            }
            Step::Password => {
                let me = self.clone();
                body.push(
                    self.styled(field(&self.password))
                        .password()
                        .label("Parol")
                        .on_submit(move || me.submit()),
                );
            }
        }

        if let Some(e) = self.error.get() {
            body.push(gap_box(0.0, 12.0));
            body.push(t(e, 13.0, ERROR));
        }

        body.push(gap_box(0.0, 24.0));
        let me = self.clone();
        let label = match step {
            Step::Phone => "Kod yuborish",
            Step::Code => "Tasdiqlash",
            Step::Password => "Kirish",
            Step::Qr => "",
        };
        if step != Step::Qr {
            body.push(
                column(vec![t(label, 16.0, Color::WHITE).bold()])
                    .bg(if self.busy.get() {
                        ACCENT.with_opacity(0.5)
                    } else {
                        ACCENT
                    })
                    .radius(16.0)
                    .padding_xy(0.0, 16.0)
                    .center()
                    .on_click(move || me.submit()),
            );
        }
        if step == Step::Phone {
            body.push(gap_box(0.0, 16.0));
            let me = self.clone();
            body.push(
                column(vec![t("QR orqali kirish", 14.0, ACCENT2).bold()])
                    .center()
                    .on_click(move || {
                        me.error.set(None);
                        me.step.set(Step::Qr);
                    }),
            );
        }
        if step == Step::Code || step == Step::Password {
            body.push(gap_box(0.0, 12.0));
            let me = self.clone();
            body.push(
                column(vec![t("Raqamni o'zgartirish", 14.0, ACCENT2)])
                    .center()
                    .on_click(move || {
                        me.code.set(String::new());
                        me.password.set(String::new());
                        me.error.set(None);
                        me.step.set(Step::Phone);
                    }),
            );
        }
        body.push(gap_box(0.0, 24.0));
        body.push(
            t(
                "Telegram hisobingiz ilova ichida xavfsiz saqlanadi: u rasm va videolarni yopiq kanaldan olish uchun kerak.",
                12.0,
                TEXT_FAINT,
            ),
        );
        body.push(gap_box(0.0, 24.0));

        // Status bar va navigatsiya paneli ostida qolmaslik (Kotlin: `statusBarsPadding`).
        let ins = safe_insets();
        column(vec![scroll_column(&self.scroll, body)
            .padding_each(24.0, 16.0 + ins.top, 24.0, 16.0 + ins.bottom)
            .grow(1.0)])
        .bg(BG)
    }

    /// Kirish maydonlarining umumiy ko'rinishi (Kotlin `OutlinedTextField` ga mos).
    fn styled(&self, f: Element) -> Element {
        f.color(Color::WHITE)
            .size(16.0)
            .line_height(24.0)
            .letter_spacing(0.5)
            .padding_xy(16.0, 16.0)
            .label_bg(BG)
            .hint_color(TEXT_DIM)
            .accent(ACCENT2)
            .border(1.0, BORDER_BRIGHT)
            .radius(14.0)
    }

    fn picker_view(self: &Rc<Self>) -> Element {
        let q = self.query.get().trim().to_lowercase();
        let qcode = q.trim_start_matches('+').to_string();
        let list: Vec<usize> = COUNTRIES
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                q.is_empty() || c.name.to_lowercase().contains(&q) || c.code.starts_with(&qcode)
            })
            .map(|(i, _)| i)
            .collect();
        let this = self.clone();
        let rows = lazy_column(&self.picker_scroll, list.len(), 48.0, move |k| {
            let i = list[k];
            let c = &COUNTRIES[i];
            let me = this.clone();
            row(vec![
                t(c.iso, 14.0, TEXT_DIM).width(30.0),
                t(c.name, 15.0, Color::WHITE).weight(1.0),
                t(format!("+{}", c.code), 14.0, TEXT_DIM),
            ])
            .gap(8.0)
            .align(Align::Center)
            .padding_xy(8.0, 0.0)
            .on_click(move || {
                me.country.set(Some(i));
                me.prefix.set(format!("+{}", COUNTRIES[i].code));
                me.number.set(String::new());
                me.nav.pop();
            })
        });
        let me = self.clone();
        let back = icon(ARROW_BACK, 28.0).color(Color::WHITE).on_click(move || {
            me.nav.pop();
        });
        column(vec![
            row(vec![
                back,
                self.styled(field(&self.query))
                    .label_bg(SURFACE)
                    .label("Qidirish")
                    .weight(1.0),
            ])
            .gap(12.0)
            .align(Align::Center)
            .padding_each(16.0, 12.0 + safe_insets().top, 16.0, 12.0),
            rows.padding_xy(16.0, 0.0).grow(1.0),
        ])
        .bg(SURFACE)
    }
}
