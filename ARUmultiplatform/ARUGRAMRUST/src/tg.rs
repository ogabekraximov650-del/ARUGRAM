//! UZ: Telegram (DrKLO, GPL) kirish oqimi uchun umumiy qiymatlar: ranglar (`night.attheme`), vaqt va silliqlash funksiyalari.
//! EN: Shared values for the Telegram (DrKLO, GPL) sign-in flow: colors (`night.attheme`), time and easing functions.
//! RU: Общие значения для потока входа Telegram (DrKLO, GPL): цвета (`night.attheme`), время и функции сглаживания.

use std::cell::Cell;
use uzcoders::prelude::*;

thread_local! {
    static NIGHT_T: Cell<f32> = const { Cell::new(1.0) };
}

/// UZ: Tun darajasi 0 (kunduz) .. 1 (tun); ranglar shu qiymat bo'yicha aralashadi. EN: Night level 0 (day) .. 1 (night); colors blend by it. RU: Уровень ночи 0 (день) .. 1 (ночь); цвета смешиваются по нему.
pub fn set_night_t(v: f32) {
    NIGHT_T.with(|c| c.set(v.clamp(0.0, 1.0)));
}
pub fn night_t() -> f32 {
    NIGHT_T.with(|c| c.get())
}
fn mix(day: u32, night: u32) -> Color {
    Color::hex(day).lerp(Color::hex(night), night_t())
}

// Telegram `day.attheme` / `night.attheme` qiymatlari / values from Telegram's day/night themes.
pub fn bg() -> Color {
    mix(0xffffff, 0x181819)
}
pub fn txt() -> Color {
    mix(0x333333, 0xffffff)
}
pub fn hint() -> Color {
    mix(0x979797, 0x7d7d7d)
}
pub fn line() -> Color {
    mix(0xdbdbdb, 0x505050)
}
pub fn active() -> Color {
    mix(0x3e96e8, 0x6eb2ef)
}
pub fn blue() -> Color {
    mix(0x3e96e8, 0x64b5ef)
}
pub fn btn() -> Color {
    mix(0x4da0eb, 0x249af1)
}
pub fn btn2() -> Color {
    mix(0x4da0eb, 0x54b9f1)
}
pub fn key() -> Color {
    mix(0xf0f0f0, 0x242424)
}
pub fn key_text() -> Color {
    txt()
}
pub fn key_letters() -> Color {
    hint()
}
pub fn key_pressed() -> Color {
    mix(0xe0e0e0, 0x3a3a3a)
}
pub fn sub() -> Color {
    mix(0x808384, 0x7e7e7e)
}
pub fn sub_bold() -> Color {
    mix(0x333333, 0xb0b0b0)
}
pub fn bar() -> Color {
    mix(0xf4f4f4, 0x212123)
}

/// UZ: `t` ni [a, b] oralig'ida 0..1 ga keltiradi. EN: Maps `t` in [a, b] to 0..1. RU: Приводит `t` из [a, b] к 0..1.
pub fn seg(t: f32, a: f32, b: f32) -> f32 {
    ((t - a) / (b - a)).clamp(0.0, 1.0)
}

pub fn lerp(a: f32, b: f32, x: f32) -> f32 {
    a + (b - a) * x
}

/// UZ: CSS `cubic-bezier` kabi (Android `CubicBezierInterpolator`). EN: Like CSS `cubic-bezier` (Android `CubicBezierInterpolator`). RU: Как CSS `cubic-bezier` (Android `CubicBezierInterpolator`).
pub fn bezier(x1: f32, y1: f32, x2: f32, y2: f32, x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    let (mut lo, mut hi, mut u) = (0.0f32, 1.0f32, x);
    for _ in 0..24 {
        u = (lo + hi) * 0.5;
        let cx = 3.0 * (1.0 - u) * (1.0 - u) * u * x1 + 3.0 * (1.0 - u) * u * u * x2 + u * u * u;
        if cx < x {
            lo = u;
        } else {
            hi = u;
        }
    }
    3.0 * (1.0 - u) * (1.0 - u) * u * y1 + 3.0 * (1.0 - u) * u * u * y2 + u * u * u
}

/// `CubicBezierInterpolator.DEFAULT` (klaviatura / keyboard).
pub fn default_ease(x: f32) -> f32 {
    bezier(0.25, 0.1, 0.25, 1.0, x)
}
/// `EASE_OUT_QUINT` (suzuvchi tugma / floating button).
pub fn quint(x: f32) -> f32 {
    bezier(0.22, 1.0, 0.36, 1.0, x)
}
/// `AccelerateDecelerateInterpolator` (sahifa almashishi / page switch).
pub fn acc_dec(x: f32) -> f32 {
    (((x.clamp(0.0, 1.0) + 1.0) * std::f32::consts::PI).cos() / 2.0) + 0.5
}
pub fn ease_in_out_quad(x: f32) -> f32 {
    if x < 0.5 { 2.0 * x * x } else { 1.0 - (-2.0 * x + 2.0).powi(2) / 2.0 }
}
pub fn decelerate(x: f32) -> f32 {
    1.0 - (1.0 - x) * (1.0 - x)
}

thread_local! {
    static TEST_NOW: Cell<Option<f32>> = const { Cell::new(None) };
    static START: std::time::Instant = std::time::Instant::now();
}

/// UZ: Animatsiya soati (ms). Testlarda `set_test_time` bilan qo'lda beriladi. EN: Animation clock (ms); tests set it with `set_test_time`. RU: Часы анимации (мс); в тестах задаются через `set_test_time`.
pub fn now_ms() -> f32 {
    TEST_NOW.with(|t| t.get()).unwrap_or_else(|| START.with(|s| s.elapsed().as_secs_f32() * 1000.0))
}

pub fn set_test_time(ms: Option<f32>) {
    TEST_NOW.with(|t| t.set(ms));
}

/// UZ: Qiymatni belgilangan vaqt ichida o'zgartiruvchi oddiy tween. EN: A simple value tween over a fixed time. RU: Простой твин значения за фиксированное время.
#[derive(Clone, Copy)]
pub struct Tw {
    from: f32,
    to: f32,
    t0: f32,
    dur: f32,
    ease: fn(f32) -> f32,
}

impl Tw {
    pub fn new(v: f32, ease: fn(f32) -> f32, dur: f32) -> Tw {
        Tw { from: v, to: v, t0: 0.0, dur, ease }
    }
    pub fn val(&self, now: f32) -> f32 {
        lerp(self.from, self.to, (self.ease)(seg(now, self.t0, self.t0 + self.dur)))
    }
    /// Yangi maqsad; boshlanish — hozirgi qiymat / New target; starts from the current value.
    pub fn go(&mut self, to: f32, now: f32) {
        if (self.to - to).abs() > f32::EPSILON {
            self.from = self.val(now);
            self.to = to;
            self.t0 = now;
        }
    }
    pub fn go_after(&mut self, to: f32, now: f32, delay: f32) {
        self.go(to, now + delay);
    }
    pub fn active(&self, now: f32) -> bool {
        now < self.t0 + self.dur
    }
}

/// Matn / Text / Текст.
pub fn t(s: impl Into<String>, size: f32, color: Color) -> Element {
    text(s).size(size).color(color)
}

/// Mutlaq joylashtirish (stack ichida): chap va tepa dp / Absolute placement inside a stack: left and top in dp.
pub fn at(x: f32, y: f32, e: Element) -> Element {
    e.margin_each(x, y, 0.0, 0.0).abs()
}

/// Gorizontal markazlangan matn qatori (kengligi `w`) / A horizontally centered text line of width `w`.
pub fn centered(w: f32, e: Element) -> Element {
    column(vec![e]).width(w).align(Align::Center)
}

/// Material ikonka yo'llari (24x24) / Material icon paths (24x24).
pub const ARROW_FWD: &str = "M3.50 11.00 L20.50 11.00 L20.50 13.00 L3.50 13.00 Z M13.20 4.29 L20.91 12.00 L19.50 13.41 L11.79 5.70 Z M11.79 18.30 L19.50 10.59 L20.91 12.00 L13.20 19.71 Z";
pub const ARROW_BACK: &str = "M20 11H7.83l5.59-5.59L12 4l-8 8 8 8 1.41-1.41L7.83 13H20v-2z";
pub const CHEVRON_RIGHT: &str = "M10 6L8.59 7.41 13.17 12l-4.58 4.59L10 18l6-6z";
pub const SEARCH: &str = "M15.5 14h-.79l-.28-.27A6.471 6.471 0 0 0 16 9.5 6.5 6.5 0 1 0 9.5 16c1.61 0 3.09-.59 4.23-1.57l.27.28v.79l5 4.99L20.49 19l-4.99-5zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 11.99 14 9.5 14z";
pub const BACKSPACE: &str = "M1.47 12.09 L8.11 4.27 L9.33 5.31 L2.69 13.13 Z M7.40 4.60 L22.20 4.60 L22.20 6.20 L7.40 6.20 Z M22.20 4.60 L22.20 19.40 L20.60 19.40 L20.60 4.60 Z M22.20 19.40 L7.40 19.40 L7.40 17.80 L22.20 17.80 Z M8.11 19.73 L1.47 11.91 L2.69 10.87 L9.33 18.69 Z M10.90 8.17 L17.43 14.70 L16.30 15.83 L9.77 9.30 Z M17.43 9.30 L10.90 15.83 L9.77 14.70 L16.30 8.17 Z";
pub const EYE: &str = "M12 4.5C7 4.5 2.73 7.61 1 12c1.73 4.39 6 7.5 11 7.5s9.27-3.11 11-7.5c-1.73-4.39-6-7.5-11-7.5zM12 17c-2.76 0-5-2.24-5-5s2.24-5 5-5 5 2.24 5 5-2.24 5-5 5zm0-8c-1.66 0-3 1.34-3 3s1.34 3 3 3 3-1.34 3-3-1.34-3-3-3z";
pub const EYE_OFF: &str = "M12 7c2.76 0 5 2.24 5 5 0 .65-.13 1.26-.36 1.83l2.92 2.92c1.51-1.26 2.7-2.89 3.43-4.75-1.73-4.39-6-7.5-11-7.5-1.4 0-2.74.25-3.98.7l2.16 2.16C10.74 7.13 11.35 7 12 7zM2 4.27l2.28 2.28.46.46C3.08 8.3 1.78 10.02 1 12c1.73 4.39 6 7.5 11 7.5 1.55 0 3.03-.3 4.38-.84l.42.42L19.73 22 21 20.73 3.27 3 2 4.27zM7.53 9.8l1.55 1.55c-.05.21-.08.43-.08.65 0 1.66 1.34 3 3 3 .22 0 .44-.03.65-.08l1.55 1.55c-.67.33-1.41.53-2.2.53-2.76 0-5-2.24-5-5 0-.79.2-1.53.53-2.2zm4.31-.78l3.15 3.15.02-.16c0-1.66-1.34-3-3-3l-.17.01z";

/// UZ: `SpringForce` (rigidlik 500, to'siqsiz): `from` dan `to` ga `ms` da. EN: `SpringForce` (stiffness 500, no bounce): from `from` to `to` after `ms`. RU: `SpringForce` (жёсткость 500, без отскока): от `from` к `to` за `ms`.
pub fn spring_val(from: f32, to: f32, ms: f32) -> f32 {
    let w = 500.0f32.sqrt();
    let t = (ms / 1000.0).max(0.0);
    let k = 1.0 - (1.0 + w * t) * (-w * t).exp();
    from + (to - from) * k
}
