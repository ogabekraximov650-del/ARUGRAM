//! Video pleyer: Media3 ExoPlayer (Java: `AruActivity`), Kotlin ilovadagi bilan bir xil. Rust faqat buyruq beradi va holatni o'qiydi.
//! Video uzcoders'ning GL tashqi teksturasiga chiqadi (`.video()` elementi); manba — Rust yadrosining shifrlangan keshi/Telegram
//! (`aru://file/<nom>`, `AruDataSource`) yoki oddiy http manzil. Ijro joyi yadroga (oldindan olish oynasi uchun) shu yerdan bildiriladi.

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::ffi::CString;
use std::os::raw::c_char;
use std::time::Instant;

extern "C" {
    fn rust_player_position(name: *const c_char, pos_ms: i64, dur_ms: i64, buffering: i32);
}

/// Manba: Rust yadrosi fayl nomi (`aru://file/<nom>`) yoki oddiy http manzil.
#[derive(Clone)]
pub enum Source {
    Aru(String),
    Http(String),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum State {
    Opening,
    Ready,
    Ended,
    Error,
}

#[derive(Clone)]
pub struct Status {
    pub state: State,
    pub pos_ms: i64,
    pub dur_ms: i64,
    pub playing: bool,
    pub buffering: bool,
    pub error: String,
    pub width: i32,
    pub height: i32,
    pub speed: f32,
    /// Diagnostika matni (pleyer ostida ko'rsatiladi).
    pub dbg: String,
}

/// Kadr berib turuvchi oqim bilan umumiy holat: video kadrlari Java tomonda keladi, UI esa har kadrda qayta chizilishi kerak.
struct Ticker {
    closed: AtomicBool,
    playing: AtomicBool,
    /// Shu paytgacha (ms, `opened_at` dan) pauza bo'lsa ham kadr so'raladi (seek/ochilgandan keyingi birinchi kadr uchun).
    boost_until: AtomicU64,
}

pub struct Player {
    ticker: Arc<Ticker>,
    /// `aru://` manbada fayl nomi (yadroga ijro joyini bildirish uchun).
    name: String,
    closed: Cell<bool>,
    speed: Cell<f32>,
    last_report: Cell<Option<Instant>>,
    opened_at: Instant,
}

impl Player {
    /// Ochadi (Java tomonda, asosiy oqimda). Pleyer sahifasi `.video()` elementini ko'rsatib turishi kerak (Surface shunda yaratiladi).
    pub fn open(source: Source, start_ms: i64, auto_play: bool) -> Player {
        let (name, uri) = match &source {
            Source::Aru(n) => (n.clone(), format!("aru://file/{n}")),
            Source::Http(u) => (String::new(), u.clone()),
        };
        uzcoders::android::activity_call("exoOpen", &format!("{uri}|{}|{}|100", start_ms.max(0), if auto_play { 1 } else { 0 }));
        let ticker = Arc::new(Ticker { closed: AtomicBool::new(false), playing: AtomicBool::new(auto_play), boost_until: AtomicU64::new(10_000) });
        let (t2, t0) = (ticker.clone(), Instant::now());
        std::thread::spawn(move || {
            while !t2.closed.load(Ordering::SeqCst) {
                let now = t0.elapsed().as_millis() as u64;
                if t2.playing.load(Ordering::SeqCst) || now < t2.boost_until.load(Ordering::SeqCst) {
                    uzcoders::core::request_frame();
                }
                std::thread::sleep(std::time::Duration::from_millis(33));
            }
        });
        Player { ticker, name, closed: Cell::new(false), speed: Cell::new(1.0), last_report: Cell::new(None), opened_at: Instant::now() }
    }

    pub fn status(&self) -> Status {
        let raw = if self.closed.get() { String::new() } else { uzcoders::android::activity_call_ret("exoStatus", "") };
        let mut it = raw.splitn(8, '|');
        let mut num = || it.next().and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
        let (state, pos, dur, playing, buffering, w, h) = (num(), num(), num(), num(), num(), num(), num());
        let error = it.next().unwrap_or("").to_string();
        if !self.name.is_empty() && !self.closed.get() && dur > 0 && self.last_report.get().map_or(true, |t| t.elapsed().as_millis() >= 700) {
            self.last_report.set(Some(Instant::now()));
            if let Ok(c) = CString::new(self.name.as_str()) {
                unsafe { rust_player_position(c.as_ptr(), pos, dur, buffering as i32) };
            }
        }
        self.ticker.playing.store(playing == 1 && state != 2, Ordering::SeqCst);
        let (tex, win, up) = uzcoders::android::video_stats();
        Status {
            state: match state {
                1 => State::Ready,
                2 => State::Ended,
                3 => State::Error,
                _ => State::Opening,
            },
            pos_ms: pos,
            dur_ms: dur,
            playing: playing == 1,
            buffering: buffering == 1,
            error,
            width: w as i32,
            height: h as i32,
            speed: self.speed.get(),
            dbg: format!("ExoPlayer | yuza={} tex={tex} tekstura={up} | {}x{} | {:.1}s | {}", if win { "bor" } else { "yo'q" }, w, h, self.opened_at.elapsed().as_secs_f32(), raw.chars().take(40).collect::<String>()),
        }
    }

    pub fn play(&self) {
        self.ticker.playing.store(true, Ordering::SeqCst);
        uzcoders::android::activity_call("exoCmd", "play");
    }

    pub fn pause(&self) {
        uzcoders::android::activity_call("exoCmd", "pause");
    }

    fn boost(&self) {
        self.ticker.boost_until.store(self.opened_at.elapsed().as_millis() as u64 + 2_000, Ordering::SeqCst);
    }

    pub fn seek_ms(&self, ms: i64) {
        self.boost();
        uzcoders::android::activity_call("exoCmd", &format!("seek:{}", ms.max(0)));
    }

    /// Ijro tezligi (0.5..2.0); ExoPlayer ovoz balandligini saqlaydi.
    pub fn set_speed(&self, x: f32) {
        self.speed.set(x);
        uzcoders::android::activity_call("exoCmd", &format!("speed:{}", ((x * 100.0).round() as i32).clamp(50, 200)));
    }

    pub fn close(&self) {
        if self.closed.replace(true) {
            return;
        }
        self.ticker.closed.store(true, Ordering::SeqCst);
        uzcoders::android::activity_call("exoCmd", "close");
        if !self.name.is_empty() {
            if let Ok(c) = CString::new(self.name.as_str()) {
                unsafe { rust_player_position(c.as_ptr(), 0, 0, 0) };
            }
        }
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.close();
    }
}
