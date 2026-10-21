//! Video pleyer: NDK `AMediaExtractor` + `AMediaCodec`. Video — GL tashqi teksturaga (uzcoders `.video()`), audio — AAudio.
//! Manba: Rust yadrosining shifrlangan keshi/Telegram (`rust_player_*`) yoki oddiy http manzil.
//! Audio — bosh soat: video kadrlari audio vaqtiga qarab chiqariladi.

#![allow(clippy::missing_safety_doc)]

use std::ffi::{c_void, CString};
use std::os::raw::c_char;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicI64, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct BufInfo {
    offset: i32,
    size: i32,
    pts_us: i64,
    flags: u32,
}

#[repr(C)]
struct ADataSource(c_void);

#[cfg_attr(target_os = "android", link(name = "mediandk"))]
extern "C" {
    fn AMediaExtractor_new() -> *mut c_void;
    fn AMediaExtractor_delete(ex: *mut c_void) -> i32;
    fn AMediaExtractor_setDataSourceCustom(ex: *mut c_void, ds: *mut ADataSource) -> i32;
    fn AMediaExtractor_setDataSource(ex: *mut c_void, location: *const c_char) -> i32;
    fn AMediaExtractor_getTrackCount(ex: *mut c_void) -> usize;
    fn AMediaExtractor_getTrackFormat(ex: *mut c_void, idx: usize) -> *mut c_void;
    fn AMediaExtractor_selectTrack(ex: *mut c_void, idx: usize) -> i32;
    fn AMediaExtractor_readSampleData(ex: *mut c_void, buf: *mut u8, cap: usize) -> isize;
    fn AMediaExtractor_getSampleTime(ex: *mut c_void) -> i64;
    fn AMediaExtractor_getSampleTrackIndex(ex: *mut c_void) -> i32;
    fn AMediaExtractor_advance(ex: *mut c_void) -> bool;
    fn AMediaExtractor_seekTo(ex: *mut c_void, us: i64, mode: i32) -> i32;
    fn AMediaFormat_getString(f: *mut c_void, name: *const c_char, out: *mut *const c_char) -> bool;
    fn AMediaFormat_getInt32(f: *mut c_void, name: *const c_char, out: *mut i32) -> bool;
    fn AMediaFormat_getInt64(f: *mut c_void, name: *const c_char, out: *mut i64) -> bool;
    fn AMediaFormat_delete(f: *mut c_void) -> i32;
    fn AMediaCodec_createDecoderByType(mime: *const c_char) -> *mut c_void;
    fn AMediaCodec_configure(c: *mut c_void, fmt: *mut c_void, window: *mut c_void, crypto: *mut c_void, flags: u32) -> i32;
    fn AMediaCodec_start(c: *mut c_void) -> i32;
    fn AMediaCodec_stop(c: *mut c_void) -> i32;
    fn AMediaCodec_flush(c: *mut c_void) -> i32;
    fn AMediaCodec_delete(c: *mut c_void) -> i32;
    fn AMediaCodec_dequeueInputBuffer(c: *mut c_void, timeout_us: i64) -> isize;
    fn AMediaCodec_getInputBuffer(c: *mut c_void, idx: usize, out_size: *mut usize) -> *mut u8;
    fn AMediaCodec_queueInputBuffer(c: *mut c_void, idx: usize, offset: i64, size: usize, time: u64, flags: u32) -> i32;
    fn AMediaCodec_dequeueOutputBuffer(c: *mut c_void, info: *mut BufInfo, timeout_us: i64) -> isize;
    fn AMediaCodec_getOutputBuffer(c: *mut c_void, idx: usize, out_size: *mut usize) -> *mut u8;
    fn AMediaCodec_releaseOutputBuffer(c: *mut c_void, idx: usize, render: bool) -> i32;
    fn AMediaCodec_getOutputFormat(c: *mut c_void) -> *mut c_void;
    fn AMediaDataSource_new() -> *mut ADataSource;
    fn AMediaDataSource_delete(ds: *mut ADataSource);
    fn AMediaDataSource_setUserdata(ds: *mut ADataSource, ud: *mut c_void);
    fn AMediaDataSource_setReadAt(ds: *mut ADataSource, f: extern "C" fn(*mut c_void, i64, *mut c_void, usize) -> isize);
    fn AMediaDataSource_setGetSize(ds: *mut ADataSource, f: extern "C" fn(*mut c_void) -> isize);
    fn AMediaDataSource_setClose(ds: *mut ADataSource, f: extern "C" fn(*mut c_void));
}

#[cfg_attr(target_os = "android", link(name = "aaudio"))]
extern "C" {
    fn AAudio_createStreamBuilder(b: *mut *mut c_void) -> i32;
    fn AAudioStreamBuilder_setDirection(b: *mut c_void, d: i32);
    fn AAudioStreamBuilder_setSampleRate(b: *mut c_void, r: i32);
    fn AAudioStreamBuilder_setChannelCount(b: *mut c_void, c: i32);
    fn AAudioStreamBuilder_setFormat(b: *mut c_void, f: i32);
    fn AAudioStreamBuilder_setPerformanceMode(b: *mut c_void, m: i32);
    fn AAudioStreamBuilder_openStream(b: *mut c_void, s: *mut *mut c_void) -> i32;
    fn AAudioStreamBuilder_delete(b: *mut c_void) -> i32;
    fn AAudioStream_requestStart(s: *mut c_void) -> i32;
    fn AAudioStream_requestPause(s: *mut c_void) -> i32;
    fn AAudioStream_requestFlush(s: *mut c_void) -> i32;
    fn AAudioStream_requestStop(s: *mut c_void) -> i32;
    fn AAudioStream_write(s: *mut c_void, buf: *const c_void, frames: i32, timeout_ns: i64) -> i32;
    fn AAudioStream_getFramesRead(s: *mut c_void) -> i64;
    fn AAudioStream_close(s: *mut c_void) -> i32;
}

extern "C" {
    fn rust_player_open(name: *const c_char, size_hint: i64) -> i64;
    fn rust_player_size(h: i64) -> i64;
    fn rust_player_read(h: i64, pos: i64, buf: *mut u8, len: i32) -> i32;
    fn rust_player_close(h: i64);
    fn rust_player_position(name: *const c_char, pos_ms: i64, dur_ms: i64, buffering: i32);
}

const INFO_TRY_AGAIN: isize = -1;
const INFO_FORMAT_CHANGED: isize = -2;
const FLAG_EOS: u32 = 4;
const SEEK_PREVIOUS_SYNC: i32 = 0;

#[derive(Clone, Copy)]
struct Ptr(*mut c_void);
unsafe impl Send for Ptr {}
unsafe impl Sync for Ptr {}

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

struct Clock {
    /// Audio bo'lmasa: tizim soati bo'yicha.
    base_pts: i64,
    base_at: Option<Instant>,
    /// Audio bo'lsa: shu `pts` va shu paytdagi o'qilgan kadrlar.
    audio_base_pts: i64,
    audio_base_frames: i64,
    audio_ready: bool,
    rate: i32,
}

struct Inner {
    state: AtomicU8,
    error: Mutex<String>,
    playing: AtomicBool,
    stop: AtomicBool,
    buffering: AtomicBool,
    seek_to: AtomicI64,
    /// Seek bo'lganda o'sadi: eski kadrlar tashlanadi.
    generation: AtomicU64,
    /// Seekdan keyingi birinchi kadr to'xtab turgan holatda ham ko'rsatiladi.
    preview: AtomicBool,
    /// To'xtab turganda audio paketlari tashlandi: ijro boshlanganda joriy joydan qayta sinxronlash kerak.
    dropped_audio: AtomicBool,
    pos_us: AtomicI64,
    dur_us: AtomicI64,
    /// Tezlik (foiz: 100 = 1x). Audio chastotasi shunga ko'paytiriladi.
    speed: AtomicI32,
    /// Diagnostika: ekranga chiqarilgan kadrlar, dekoderdan chiqqan kadrlar, oxirgi dekoder holati.
    rendered: AtomicU64,
    decoded: AtomicU64,
    stage: AtomicU8,
    audio_dead: AtomicBool,
    a_out: AtomicU64,
    a_state: AtomicU8,
    /// Diagnostika: manbadan o'qilgan baytlar, hozir o'qish ichidami, dekoderga berilgan video/audio paketlar.
    read_bytes: AtomicU64,
    in_read: AtomicBool,
    fed_v: AtomicU64,
    fed_a: AtomicU64,
    last_sz: AtomicI64,
    width: AtomicI32,
    height: AtomicI32,
    has_audio: AtomicBool,
    clock: Mutex<Clock>,
    /// Ovoz soati hali tayyor emas: kutish boshlangan payt (1.5 s dan keyin tizim soatiga o'tiladi).
    audio_wait: Mutex<Option<Instant>>,
    audio_stream: Mutex<Ptr>,
    vcodec: Mutex<Ptr>,
    acodec: Mutex<Ptr>,
    name: String,
}

pub struct Player {
    inner: Arc<Inner>,
}

struct DsCtx {
    handle: i64,
    stop: Arc<AtomicBool>,
    inner: Arc<Inner>,
}

extern "C" fn ds_read(ud: *mut c_void, offset: i64, buf: *mut c_void, size: usize) -> isize {
    let ctx = unsafe { &*(ud as *const DsCtx) };
    let want = size.min(1 << 20) as i32;
    loop {
        ctx.inner.in_read.store(true, Ordering::SeqCst);
        let r = unsafe { rust_player_read(ctx.handle, offset, buf as *mut u8, want) };
        ctx.inner.in_read.store(false, Ordering::SeqCst);
        match r {
            n if n > 0 => {
                ctx.inner.read_bytes.fetch_add(n as u64, Ordering::Relaxed);
                ctx.inner.buffering.store(false, Ordering::SeqCst);
                return n as isize;
            }
            0 => return -1,
            -2 => {
                ctx.inner.buffering.store(true, Ordering::SeqCst);
                if ctx.stop.load(Ordering::SeqCst) || ctx.inner.stop.load(Ordering::SeqCst) {
                    return -1;
                }
                std::thread::sleep(Duration::from_millis(150));
            }
            _ => return -1,
        }
    }
}

extern "C" fn ds_size(ud: *mut c_void) -> isize {
    let ctx = unsafe { &*(ud as *const DsCtx) };
    unsafe { rust_player_size(ctx.handle) as isize }
}

extern "C" fn ds_close(ud: *mut c_void) {
    // Kontekst `Box::into_raw` bilan berilgan: bu yerda qaytarib olinadi.
    let ctx = unsafe { Box::from_raw(ud as *mut DsCtx) };
    unsafe { rust_player_close(ctx.handle) };
}

fn cstr(s: &str) -> CString {
    CString::new(s).unwrap_or_default()
}

unsafe fn fmt_string(f: *mut c_void, key: &str) -> String {
    let mut p: *const c_char = std::ptr::null();
    if AMediaFormat_getString(f, cstr(key).as_ptr(), &mut p) && !p.is_null() {
        std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned()
    } else {
        String::new()
    }
}

unsafe fn fmt_i32(f: *mut c_void, key: &str) -> Option<i32> {
    let mut v = 0;
    if AMediaFormat_getInt32(f, cstr(key).as_ptr(), &mut v) { Some(v) } else { None }
}

impl Inner {
    fn fail(&self, msg: impl Into<String>) {
        let m = msg.into();
        eprintln!("pleyer xato: {m}");
        *self.error.lock().unwrap_or_else(|e| e.into_inner()) = m;
        self.state.store(3, Ordering::SeqCst);
    }

    /// Bosh soat (mikrosekund).
    fn clock_us(&self) -> i64 {
        let c = self.clock.lock().unwrap_or_else(|e| e.into_inner());
        if self.has_audio.load(Ordering::SeqCst) {
            if !c.audio_ready {
                // Ovoz boshlanmasa video qotib qolmasin: 1.5 s dan keyin tizim soati bo'yicha davom etadi.
                if !self.playing.load(Ordering::SeqCst) {
                    *self.audio_wait.lock().unwrap_or_else(|e| e.into_inner()) = None;
                    return c.audio_base_pts;
                }
                let mut w = self.audio_wait.lock().unwrap_or_else(|e| e.into_inner());
                let t = *w.get_or_insert_with(Instant::now);
                let el = t.elapsed().as_micros() as i64 - 1_500_000;
                return c.audio_base_pts + el.max(0) * self.speed.load(Ordering::SeqCst) as i64 / 100;
            }
            *self.audio_wait.lock().unwrap_or_else(|e| e.into_inner()) = None;
            let s = self.audio_stream.lock().unwrap_or_else(|e| e.into_inner()).0;
            if s.is_null() {
                return c.audio_base_pts;
            }
            let fr = unsafe { AAudioStream_getFramesRead(s) } - c.audio_base_frames;
            return c.audio_base_pts + fr.max(0) * 1_000_000 / c.rate.max(1) as i64;
        }
        match c.base_at {
            Some(t) if self.playing.load(Ordering::SeqCst) => c.base_pts + t.elapsed().as_micros() as i64 * self.speed.load(Ordering::SeqCst) as i64 / 100,
            _ => c.base_pts,
        }
    }

    /// Audio bo'lmagan videolar uchun tizim soatini shu vaqtdan boshlaydi/to'xtatadi.
    fn sys_clock_set(&self, pts: i64, running: bool) {
        let mut c = self.clock.lock().unwrap_or_else(|e| e.into_inner());
        c.base_pts = pts;
        c.base_at = if running { Some(Instant::now()) } else { None };
    }
}

impl Player {
    /// Ochadi (fon oqimida). Pleyer sahifasi `.video()` elementini ko'rsatib turishi kerak (Surface shunda yaratiladi).
    pub fn open(source: Source, start_ms: i64, auto_play: bool) -> Player {
        let name = match &source {
            Source::Aru(n) => n.clone(),
            Source::Http(_) => String::new(),
        };
        let inner = Arc::new(Inner {
            state: AtomicU8::new(0),
            error: Mutex::new(String::new()),
            playing: AtomicBool::new(auto_play),
            stop: AtomicBool::new(false),
            buffering: AtomicBool::new(true),
            seek_to: AtomicI64::new(if start_ms > 1000 { start_ms * 1000 } else { -1 }),
            generation: AtomicU64::new(0),
            preview: AtomicBool::new(true),
            dropped_audio: AtomicBool::new(false),
            pos_us: AtomicI64::new(start_ms.max(0) * 1000),
            dur_us: AtomicI64::new(0),
            speed: AtomicI32::new(100),
            rendered: AtomicU64::new(0),
            decoded: AtomicU64::new(0),
            stage: AtomicU8::new(0),
            audio_dead: AtomicBool::new(false),
            a_out: AtomicU64::new(0),
            a_state: AtomicU8::new(0),
            audio_wait: Mutex::new(None),
            read_bytes: AtomicU64::new(0),
            in_read: AtomicBool::new(false),
            fed_v: AtomicU64::new(0),
            fed_a: AtomicU64::new(0),
            last_sz: AtomicI64::new(0),
            width: AtomicI32::new(0),
            height: AtomicI32::new(0),
            has_audio: AtomicBool::new(false),
            clock: Mutex::new(Clock { base_pts: start_ms.max(0) * 1000, base_at: None, audio_base_pts: start_ms.max(0) * 1000, audio_base_frames: 0, audio_ready: false, rate: 44100 }),
            audio_stream: Mutex::new(Ptr(std::ptr::null_mut())),
            vcodec: Mutex::new(Ptr(std::ptr::null_mut())),
            acodec: Mutex::new(Ptr(std::ptr::null_mut())),
            name,
        });
        let i2 = inner.clone();
        std::thread::spawn(move || unsafe { run(i2, source) });
        Player { inner }
    }

    pub fn status(&self) -> Status {
        let i = &self.inner;
        let st = match i.state.load(Ordering::SeqCst) {
            0 => State::Opening,
            1 => State::Ready,
            2 => State::Ended,
            _ => State::Error,
        };
        let pos = if i.seek_to.load(Ordering::SeqCst) >= 0 { i.seek_to.load(Ordering::SeqCst) } else { i.pos_us.load(Ordering::SeqCst) };
        Status {
            state: st,
            pos_ms: pos / 1000,
            dur_ms: i.dur_us.load(Ordering::SeqCst) / 1000,
            playing: i.playing.load(Ordering::SeqCst),
            buffering: i.buffering.load(Ordering::SeqCst),
            error: i.error.lock().map(|e| e.clone()).unwrap_or_default(),
            width: i.width.load(Ordering::SeqCst),
            height: i.height.load(Ordering::SeqCst),
            speed: i.speed.load(Ordering::SeqCst) as f32 / 100.0,
            dbg: {
                let (tex, win, up) = uzcoders::android::video_stats();
                let stage = match i.stage.load(Ordering::SeqCst) {
                    0 => "yuza kutilmoqda",
                    1 => "manba ochilmoqda",
                    2 => "dekoder tayyor",
                    _ => "?",
                };
                format!("{stage} | yuza={} tex={tex} | dekod={} chiqdi={} tekstura={up} | {}x{} | o'qildi={}KB {} | berildi v={} a={} sz={} | ovoz chiq={} oqim={}", if win { "bor" } else { "yo'q" }, i.decoded.load(Ordering::SeqCst), i.rendered.load(Ordering::SeqCst), i.width.load(Ordering::SeqCst), i.height.load(Ordering::SeqCst), i.read_bytes.load(Ordering::Relaxed) / 1024, if i.in_read.load(Ordering::SeqCst) { "(o'qishda)" } else { "" }, i.fed_v.load(Ordering::Relaxed), i.fed_a.load(Ordering::Relaxed), i.last_sz.load(Ordering::Relaxed), i.a_out.load(Ordering::Relaxed), if i.audio_dead.load(Ordering::SeqCst) { "o'lik" } else { "ok" })
            },
        }
    }

    pub fn play(&self) {
        let i = &self.inner;
        if i.state.load(Ordering::SeqCst) == 2 {
            i.state.store(1, Ordering::SeqCst);
            i.seek_to.store(0, Ordering::SeqCst);
        }
        if !i.has_audio.load(Ordering::SeqCst) {
            i.sys_clock_set(i.pos_us.load(Ordering::SeqCst), true);
        }
        if i.dropped_audio.swap(false, Ordering::SeqCst) && i.has_audio.load(Ordering::SeqCst) {
            i.seek_to.store(i.pos_us.load(Ordering::SeqCst), Ordering::SeqCst);
        }
        i.playing.store(true, Ordering::SeqCst);
    }

    /// Ijro tezligi (0.5..2.0). Ovoz balandligi ham shunga qarab o'zgaradi (chastota ko'paytiriladi).
    pub fn set_speed(&self, x: f32) {
        let i = &self.inner;
        let v = ((x * 100.0).round() as i32).clamp(50, 200);
        if !i.has_audio.load(Ordering::SeqCst) {
            i.sys_clock_set(i.clock_us(), i.playing.load(Ordering::SeqCst));
        }
        i.speed.store(v, Ordering::SeqCst);
    }

    pub fn pause(&self) {
        let i = &self.inner;
        if !i.has_audio.load(Ordering::SeqCst) {
            i.sys_clock_set(i.clock_us(), false);
        }
        i.playing.store(false, Ordering::SeqCst);
    }

    pub fn seek_ms(&self, ms: i64) {
        let dur = self.inner.dur_us.load(Ordering::SeqCst);
        let mut us = ms.max(0) * 1000;
        if dur > 0 {
            us = us.min(dur);
        }
        self.inner.state.compare_exchange(2, 1, Ordering::SeqCst, Ordering::SeqCst).ok();
        self.inner.seek_to.store(us, Ordering::SeqCst);
    }

    pub fn close(&self) {
        self.inner.stop.store(true, Ordering::SeqCst);
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.close();
    }
}

/// Surface bitta; bir vaqtda faqat bitta pleyer ishlaydi (yangi qism eskisi to'liq yopilguncha kutadi).
static PLAYER_LOCK: Mutex<()> = Mutex::new(());

unsafe fn run(inner: Arc<Inner>, source: Source) {
    let _guard = loop {
        match PLAYER_LOCK.try_lock() {
            Ok(g) => break g,
            Err(std::sync::TryLockError::Poisoned(p)) => break p.into_inner(),
            Err(_) => {
                if inner.stop.load(Ordering::SeqCst) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(30));
            }
        }
    };
    run_locked(inner, source);
}

unsafe fn run_locked(inner: Arc<Inner>, source: Source) {
    // 1) Dekoderga yozadigan Surface (GL oqimi video elementini birinchi marta chizganda yaratadi).
    let t0 = Instant::now();
    let window = loop {
        let w = uzcoders::android::video_window();
        if !w.is_null() {
            break w;
        }
        if inner.stop.load(Ordering::SeqCst) {
            return;
        }
        if t0.elapsed() > Duration::from_secs(8) {
            inner.fail("Video yuzasi tayyor bo'lmadi");
            return;
        }
        std::thread::sleep(Duration::from_millis(30));
    };

    inner.stage.store(1, Ordering::SeqCst);
    // 2) Manba.
    let ex = AMediaExtractor_new();
    let stop_flag = Arc::new(AtomicBool::new(false));
    let mut ds: *mut ADataSource = std::ptr::null_mut();
    let rc = match &source {
        Source::Http(url) => AMediaExtractor_setDataSource(ex, cstr(url).as_ptr()),
        Source::Aru(name) => {
            // Kesh/Telegram ochilishi: tarmoq vaqtincha yo'q bo'lsa kutiladi.
            let mut h = 0i64;
            for _ in 0..40 {
                h = rust_player_open(cstr(name).as_ptr(), 0);
                if h != -2 || inner.stop.load(Ordering::SeqCst) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(500));
            }
            if h <= 0 {
                inner.fail("Videoni ochib bo'lmadi");
                AMediaExtractor_delete(ex);
                return;
            }
            let ctx = Box::into_raw(Box::new(DsCtx { handle: h, stop: stop_flag.clone(), inner: inner.clone() }));
            ds = AMediaDataSource_new();
            AMediaDataSource_setUserdata(ds, ctx as *mut c_void);
            AMediaDataSource_setReadAt(ds, ds_read);
            AMediaDataSource_setGetSize(ds, ds_size);
            AMediaDataSource_setClose(ds, ds_close);
            AMediaExtractor_setDataSourceCustom(ex, ds)
        }
    };
    if rc != 0 {
        inner.fail(format!("Manba ochilmadi (kod {rc})"));
        AMediaExtractor_delete(ex);
        return;
    }

    // 3) Treklar.
    let (mut vtrack, mut atrack) = (-1i32, -1i32);
    let (mut vcodec, mut acodec) = (std::ptr::null_mut::<c_void>(), std::ptr::null_mut::<c_void>());
    let mut dur_us = 0i64;
    let n = AMediaExtractor_getTrackCount(ex);
    for i in 0..n {
        let f = AMediaExtractor_getTrackFormat(ex, i);
        let mime = fmt_string(f, "mime");
        let mut d = 0i64;
        if AMediaFormat_getInt64(f, cstr("durationUs").as_ptr(), &mut d) {
            dur_us = dur_us.max(d);
        }
        if mime.starts_with("video/") && vtrack < 0 {
            let c = AMediaCodec_createDecoderByType(cstr(&mime).as_ptr());
            if !c.is_null() && AMediaCodec_configure(c, f, window, std::ptr::null_mut(), 0) == 0 && AMediaCodec_start(c) == 0 {
                vtrack = i as i32;
                vcodec = c;
                inner.width.store(fmt_i32(f, "width").unwrap_or(0), Ordering::SeqCst);
                inner.height.store(fmt_i32(f, "height").unwrap_or(0), Ordering::SeqCst);
                AMediaExtractor_selectTrack(ex, i);
            } else if !c.is_null() {
                AMediaCodec_delete(c);
            }
        } else if mime.starts_with("audio/") && atrack < 0 {
            let c = AMediaCodec_createDecoderByType(cstr(&mime).as_ptr());
            if !c.is_null() && AMediaCodec_configure(c, f, std::ptr::null_mut(), std::ptr::null_mut(), 0) == 0 && AMediaCodec_start(c) == 0 {
                atrack = i as i32;
                acodec = c;
                inner.clock.lock().map(|mut cl| cl.rate = fmt_i32(f, "sample-rate").unwrap_or(44100)).ok();
                AMediaExtractor_selectTrack(ex, i);
            } else if !c.is_null() {
                AMediaCodec_delete(c);
            }
        }
        AMediaFormat_delete(f);
    }
    if vtrack < 0 {
        inner.fail("Video trek ochilmadi (format qo'llab-quvvatlanmaydi)");
        AMediaExtractor_delete(ex);
        return;
    }
    inner.dur_us.store(dur_us, Ordering::SeqCst);
    inner.has_audio.store(atrack >= 0, Ordering::SeqCst);
    *inner.vcodec.lock().unwrap_or_else(|e| e.into_inner()) = Ptr(vcodec);
    *inner.acodec.lock().unwrap_or_else(|e| e.into_inner()) = Ptr(acodec);
    inner.state.store(1, Ordering::SeqCst);
    inner.stage.store(2, Ordering::SeqCst);

    // 4) Chiqish oqimlari.
    let iv = inner.clone();
    let vh = std::thread::spawn(move || video_out(iv));
    let ia = inner.clone();
    let ah = if atrack >= 0 { Some(std::thread::spawn(move || audio_out(ia))) } else { None };

    // 5) Tashuvchi (demux) halqasi shu oqimda.
    let mut buf = vec![0u8; 8 << 20];
    let mut eos = false;
    let mut last_report = Instant::now();
    loop {
        if inner.stop.load(Ordering::SeqCst) {
            break;
        }
        let seek = inner.seek_to.swap(-1, Ordering::SeqCst);
        if seek >= 0 {
            inner.generation.fetch_add(1, Ordering::SeqCst);
            AMediaExtractor_seekTo(ex, seek, SEEK_PREVIOUS_SYNC);
            {
                let g = inner.vcodec.lock().unwrap_or_else(|e| e.into_inner());
                AMediaCodec_flush(g.0);
            }
            if atrack >= 0 {
                let g = inner.acodec.lock().unwrap_or_else(|e| e.into_inner());
                AMediaCodec_flush(g.0);
                let s = inner.audio_stream.lock().unwrap_or_else(|e| e.into_inner()).0;
                if !s.is_null() {
                    AAudioStream_requestPause(s);
                    AAudioStream_requestFlush(s);
                }
                if let Ok(mut c) = inner.clock.lock() {
                    c.audio_ready = false;
                    c.audio_base_pts = seek;
                }
            } else {
                inner.sys_clock_set(seek, inner.playing.load(Ordering::SeqCst));
            }
            inner.pos_us.store(seek, Ordering::SeqCst);
            inner.preview.store(true, Ordering::SeqCst);
            eos = false;
            // Seek paytida ko'rsatilgan tarmoq holati qayta hisoblanadi.
        }
        if eos {
            std::thread::sleep(Duration::from_millis(20));
        } else {
            let sz = AMediaExtractor_readSampleData(ex, buf.as_mut_ptr(), buf.len());
            let tr = AMediaExtractor_getSampleTrackIndex(ex);
            inner.last_sz.store(sz as i64, Ordering::Relaxed);
            if sz < 0 || tr < 0 {
                // Fayl oxiri: ikkala dekoderga EOS.
                for (c, present) in [(&inner.vcodec, true), (&inner.acodec, atrack >= 0)] {
                    if !present {
                        continue;
                    }
                    loop {
                        if inner.stop.load(Ordering::SeqCst) || inner.seek_to.load(Ordering::SeqCst) >= 0 {
                            break;
                        }
                        let g = c.lock().unwrap_or_else(|e| e.into_inner());
                        let idx = AMediaCodec_dequeueInputBuffer(g.0, 0);
                        if idx >= 0 {
                            AMediaCodec_queueInputBuffer(g.0, idx as usize, 0, 0, 0, FLAG_EOS);
                            break;
                        }
                        drop(g);
                        std::thread::sleep(Duration::from_millis(3));
                    }
                }
                eos = true;
            } else {
                let pts = AMediaExtractor_getSampleTime(ex);
                if tr != vtrack && inner.audio_dead.load(Ordering::SeqCst) {
                    AMediaExtractor_advance(ex);
                    continue;
                }
                let codec_lock = if tr == vtrack { &inner.vcodec } else { &inner.acodec };
                loop {
                    if inner.stop.load(Ordering::SeqCst) || inner.seek_to.load(Ordering::SeqCst) >= 0 {
                        break;
                    }
                    let g = codec_lock.lock().unwrap_or_else(|e| e.into_inner());
                    // Kutish qulf USTIDA bo'lmasin: aks holda chiqish oqimlari qulfni ola olmay och qoladi (std Mutex adolatli emas).
                    let idx = AMediaCodec_dequeueInputBuffer(g.0, 0);
                    if idx < 0 && tr != vtrack && !inner.playing.load(Ordering::SeqCst) {
                        // To'xtab turganda audio chiqishi bo'shatilmaydi: to'lib qolsa video kadri kelmay qotib qolmasin.
                        inner.dropped_audio.store(true, Ordering::SeqCst);
                        AMediaExtractor_advance(ex);
                        break;
                    }
                    if idx < 0 {
                        drop(g);
                        std::thread::sleep(Duration::from_millis(3));
                        continue;
                    }
                    if idx >= 0 {
                        let mut cap = 0usize;
                        let p = AMediaCodec_getInputBuffer(g.0, idx as usize, &mut cap);
                        let n = (sz as usize).min(cap);
                        if !p.is_null() {
                            std::ptr::copy_nonoverlapping(buf.as_ptr(), p, n);
                        }
                        AMediaCodec_queueInputBuffer(g.0, idx as usize, 0, n, pts.max(0) as u64, 0);
                        if tr == vtrack { inner.fed_v.fetch_add(1, Ordering::Relaxed); } else { inner.fed_a.fetch_add(1, Ordering::Relaxed); }
                        AMediaExtractor_advance(ex);
                        break;
                    }
                }
            }
        }
        // Ijro joyini yadroga bildirish (oldindan olish oynasi uchun) — taxminan har soniyada.
        if last_report.elapsed() >= Duration::from_secs(1) && !inner.name.is_empty() {
            last_report = Instant::now();
            rust_player_position(cstr(&inner.name).as_ptr(), inner.pos_us.load(Ordering::SeqCst) / 1000, dur_us / 1000, inner.buffering.load(Ordering::SeqCst) as i32);
        }
    }

    // 6) To'xtatish.
    inner.stop.store(true, Ordering::SeqCst);
    stop_flag.store(true, Ordering::SeqCst);
    let _ = vh.join();
    if let Some(h) = ah {
        let _ = h.join();
    }
    if !inner.name.is_empty() {
        rust_player_position(cstr(&inner.name).as_ptr(), 0, 0, 0);
    }
    for c in [&inner.vcodec, &inner.acodec] {
        let g = c.lock().unwrap_or_else(|e| e.into_inner());
        if !g.0.is_null() {
            AMediaCodec_stop(g.0);
            AMediaCodec_delete(g.0);
        }
    }
    let s = inner.audio_stream.lock().unwrap_or_else(|e| e.into_inner()).0;
    if !s.is_null() {
        AAudioStream_requestStop(s);
        AAudioStream_close(s);
    }
    AMediaExtractor_delete(ex);
    if !ds.is_null() {
        AMediaDataSource_delete(ds);
    }
    uzcoders::android::video_release();
}

unsafe fn video_out(inner: Arc<Inner>) {
    let mut my_gen = inner.generation.load(Ordering::SeqCst);
    let mut info = BufInfo::default();
    // SurfaceTexture kadri chiqarilgandan biroz keyin tayyor bo'ladi: shunda UI'ni yana bir bor uyg'otamiz.
    let mut kick_at: Option<Instant> = None;
    loop {
        if inner.stop.load(Ordering::SeqCst) {
            return;
        }
        if kick_at.is_some_and(|t| Instant::now() >= t) {
            kick_at = None;
            uzcoders::core::request_frame();
        }
        // To'xtab turganda: faqat seekdan keyingi birinchi kadr ko'rsatiladi.
        let preview = inner.preview.load(Ordering::SeqCst);
        if !inner.playing.load(Ordering::SeqCst) && !preview {
            std::thread::sleep(Duration::from_millis(15));
            continue;
        }
        let idx = {
            let g = inner.vcodec.lock().unwrap_or_else(|e| e.into_inner());
            AMediaCodec_dequeueOutputBuffer(g.0, &mut info, 0)
        };
        if idx == INFO_FORMAT_CHANGED {
            let g = inner.vcodec.lock().unwrap_or_else(|e| e.into_inner());
            let f = AMediaCodec_getOutputFormat(g.0);
            if !f.is_null() {
                if let (Some(w), Some(h)) = (fmt_i32(f, "width"), fmt_i32(f, "height")) {
                    inner.width.store(w, Ordering::SeqCst);
                    inner.height.store(h, Ordering::SeqCst);
                }
                AMediaFormat_delete(f);
            }
            continue;
        }
        if idx < 0 {
            if idx == INFO_TRY_AGAIN {
                std::thread::sleep(Duration::from_millis(2));
            }
            continue;
        }
        inner.decoded.fetch_add(1, Ordering::Relaxed);
        let g_now = inner.generation.load(Ordering::SeqCst);
        let release = |render: bool| {
            let g = inner.vcodec.lock().unwrap_or_else(|e| e.into_inner());
            AMediaCodec_releaseOutputBuffer(g.0, idx as usize, render);
        };
        if g_now != my_gen {
            my_gen = g_now;
            release(false);
            continue;
        }
        let pts = info.pts_us;
        if info.flags & FLAG_EOS != 0 {
            release(false);
            inner.state.store(2, Ordering::SeqCst);
            inner.playing.store(false, Ordering::SeqCst);
            uzcoders::core::request_frame();
            continue;
        }
        // Seekdan keyingi: maqsad vaqtigacha bo'lgan kadrlar (oldingi kalit kadrdan) tashlanadi.
        let target = inner.clock.lock().map(|c| if c.audio_ready || !inner.has_audio.load(Ordering::SeqCst) { i64::MIN } else { c.audio_base_pts }).unwrap_or(i64::MIN);
        if preview && pts < target - 20_000 && target != i64::MIN {
            release(false);
            continue;
        }
        if preview {
            inner.preview.store(false, Ordering::SeqCst);
            release(true);
            inner.rendered.fetch_add(1, Ordering::Relaxed);
            inner.pos_us.store(pts, Ordering::SeqCst);
            uzcoders::core::request_frame();
            kick_at = Some(Instant::now() + Duration::from_millis(25));
            if !inner.has_audio.load(Ordering::SeqCst) {
                inner.sys_clock_set(pts, inner.playing.load(Ordering::SeqCst));
            }
            continue;
        }
        // Vaqti kelguncha kutamiz (audio soati bo'yicha).
        let mut render = true;
        loop {
            if inner.stop.load(Ordering::SeqCst) || inner.generation.load(Ordering::SeqCst) != my_gen {
                render = false;
                break;
            }
            if !inner.playing.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
            if kick_at.is_some_and(|t| Instant::now() >= t) {
                kick_at = None;
                uzcoders::core::request_frame();
            }
            let now = inner.clock_us();
            if pts <= now + 15_000 {
                render = pts >= now - 120_000;
                break;
            }
            std::thread::sleep(Duration::from_millis(((pts - now) / 2000).clamp(1, 20) as u64));
        }
        release(render);
        if render {
            inner.rendered.fetch_add(1, Ordering::Relaxed);
            inner.pos_us.store(pts, Ordering::SeqCst);
            uzcoders::core::request_frame();
            kick_at = Some(Instant::now() + Duration::from_millis(12));
        }
    }
}

unsafe fn audio_out(inner: Arc<Inner>) {
    let mut info = BufInfo::default();
    let mut my_gen = inner.generation.load(Ordering::SeqCst);
    let mut stream: *mut c_void = std::ptr::null_mut();
    let (mut rate, mut ch) = (44100i32, 2i32);
    let mut started = false;
    let mut first_after_flush = true;
    let mut stream_speed = inner.speed.load(Ordering::SeqCst);
    loop {
        if inner.stop.load(Ordering::SeqCst) {
            return;
        }
        // Tezlik o'zgardi: oqim yangi chastota bilan qayta ochiladi, soat joriy joydan davom etadi.
        let want_speed = inner.speed.load(Ordering::SeqCst);
        if want_speed != stream_speed && !stream.is_null() {
            let pos = inner.clock_us();
            AAudioStream_requestStop(stream);
            AAudioStream_close(stream);
            stream = std::ptr::null_mut();
            *inner.audio_stream.lock().unwrap_or_else(|e| e.into_inner()) = Ptr(std::ptr::null_mut());
            started = false;
            if let Ok(mut c) = inner.clock.lock() {
                c.audio_ready = false;
                c.audio_base_pts = pos;
            }
            first_after_flush = true;
        }
        stream_speed = want_speed;
        let playing = inner.playing.load(Ordering::SeqCst);
        if !stream.is_null() {
            if playing && !started {
                AAudioStream_requestStart(stream);
                started = true;
            } else if !playing && started {
                AAudioStream_requestPause(stream);
                started = false;
            }
        }
        if !playing {
            std::thread::sleep(Duration::from_millis(15));
            continue;
        }
        let idx = {
            let g = inner.acodec.lock().unwrap_or_else(|e| e.into_inner());
            AMediaCodec_dequeueOutputBuffer(g.0, &mut info, 0)
        };
        if idx == INFO_FORMAT_CHANGED {
            let f = {
                let g = inner.acodec.lock().unwrap_or_else(|e| e.into_inner());
                AMediaCodec_getOutputFormat(g.0)
            };
            if !f.is_null() {
                rate = fmt_i32(f, "sample-rate").unwrap_or(rate);
                ch = fmt_i32(f, "channel-count").unwrap_or(ch);
                AMediaFormat_delete(f);
            }
            if !stream.is_null() {
                AAudioStream_requestStop(stream);
                AAudioStream_close(stream);
                stream = std::ptr::null_mut();
                started = false;
            }
            continue;
        }
        if stream.is_null() {
            let mut b: *mut c_void = std::ptr::null_mut();
            if AAudio_createStreamBuilder(&mut b) == 0 {
                AAudioStreamBuilder_setDirection(b, 0);
                AAudioStreamBuilder_setSampleRate(b, (rate as i64 * stream_speed as i64 / 100) as i32);
                AAudioStreamBuilder_setChannelCount(b, ch);
                AAudioStreamBuilder_setFormat(b, 1);
                AAudioStreamBuilder_setPerformanceMode(b, 10); // NONE
                let rc = AAudioStreamBuilder_openStream(b, &mut stream);
                AAudioStreamBuilder_delete(b);
                if rc != 0 {
                    // Ovoz ochilmasa video baribir ijro etiladi (tizim soati bo'yicha), audio paketlar tashlanadi.
                    stream = std::ptr::null_mut();
                    inner.audio_dead.store(true, Ordering::SeqCst);
                    inner.has_audio.store(false, Ordering::SeqCst);
                    inner.sys_clock_set(inner.clock.lock().map(|c| c.audio_base_pts).unwrap_or(0), inner.playing.load(Ordering::SeqCst));
                    return;
                }
                *inner.audio_stream.lock().unwrap_or_else(|e| e.into_inner()) = Ptr(stream);
                inner.clock.lock().map(|mut c| c.rate = rate).ok();
                AAudioStream_requestStart(stream);
                started = true;
                first_after_flush = true;
            }
        }
        if idx < 0 {
            if idx == INFO_TRY_AGAIN {
                std::thread::sleep(Duration::from_millis(2));
            }
            continue;
        }
        inner.a_out.fetch_add(1, Ordering::Relaxed);
        let g_now = inner.generation.load(Ordering::SeqCst);
        if g_now != my_gen {
            my_gen = g_now;
            first_after_flush = true;
            let g = inner.acodec.lock().unwrap_or_else(|e| e.into_inner());
            AMediaCodec_releaseOutputBuffer(g.0, idx as usize, false);
            continue;
        }
        if info.flags & FLAG_EOS != 0 {
            let g = inner.acodec.lock().unwrap_or_else(|e| e.into_inner());
            AMediaCodec_releaseOutputBuffer(g.0, idx as usize, false);
            continue;
        }
        let mut cap = 0usize;
        let p = {
            let g = inner.acodec.lock().unwrap_or_else(|e| e.into_inner());
            AMediaCodec_getOutputBuffer(g.0, idx as usize, &mut cap)
        };
        if !p.is_null() && info.size > 0 && !stream.is_null() {
            if first_after_flush {
                // Seek/start: shu paketning vaqti — yangi audio asosi.
                if !started {
                    AAudioStream_requestStart(stream);
                    started = true;
                }
                if let Ok(mut c) = inner.clock.lock() {
                    c.audio_base_pts = info.pts_us;
                    c.audio_base_frames = AAudioStream_getFramesRead(stream);
                    c.audio_ready = true;
                }
                first_after_flush = false;
            }
            let frames = info.size / (2 * ch.max(1));
            let data = p.add(info.offset.max(0) as usize);
            let mut left = frames;
            let mut off = 0usize;
            while left > 0 && !inner.stop.load(Ordering::SeqCst) && inner.generation.load(Ordering::SeqCst) == my_gen {
                if !inner.playing.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(15));
                    continue;
                }
                let w = AAudioStream_write(stream, data.add(off) as *const c_void, left, 100_000_000);
                if w <= 0 {
                    break;
                }
                left -= w;
                off += w as usize * 2 * ch as usize;
            }
        }
        let g = inner.acodec.lock().unwrap_or_else(|e| e.into_inner());
        AMediaCodec_releaseOutputBuffer(g.0, idx as usize, false);
    }
}
