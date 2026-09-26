//! Stiker, emoji va GIF kadrlarini TO'G'RIDAN-TO'G'RI ekranga chizish.
//!
//! TALAB (foydalanuvchi): "GIF, stiker va emoji animatsiyalari juda sekin
//! va qotib ishlayapti — ekranda nima ko'rinib turgan bo'lsa, hammasi
//! animatsiyalansin va ilova silliq ishlasin".
//!
//! SABAB: har kadr Rust -> Dart isolate -> UI oqimi -> `ui.Image` (GPU'ga
//! yuklash) yo'lidan o'tardi — ekrandagi 50 ta emoji soniyasiga yuzlab
//! ko'chirish va rasm yasash degani, UI oqimi band bo'lib qolardi.
//!
//! ENDI (Telegram `RLottieDrawable` kabi): har animatsiyaga Flutter
//! `Texture` (Android `SurfaceProducer`) beriladi va Rust o'z fon
//! oqimlarida kadrni to'g'ridan-to'g'ri o'sha yuzaga (`ANativeWindow`)
//! chizadi. Dart kadrlar bilan umuman shug'ullanmaydi — faqat
//! "o'ynasin / to'xtasin" deydi. Kadrlar diskda saqlanadi
//! (`sticker_anim.rs`, `.afc`) — ikkinchi aylanishdan protsessor deyarli
//! ishlamaydi.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::ffi_utils::{cstr_to_str, string_to_cptr};
use crate::sticker_anim;

// ── ANativeWindow (faqat Android) ──────────────────────────────────

#[repr(C)]
struct WinBuf {
    width: i32,
    height: i32,
    stride: i32,
    format: i32,
    bits: *mut c_void,
    reserved: [u32; 6],
}

#[cfg(target_os = "android")]
#[link(name = "android")]
extern "C" {
    fn ANativeWindow_fromSurface(env: *mut c_void, surface: *mut c_void) -> *mut c_void;
    fn ANativeWindow_release(w: *mut c_void);
    fn ANativeWindow_setBuffersGeometry(w: *mut c_void, width: i32, height: i32, format: i32) -> i32;
    fn ANativeWindow_lock(w: *mut c_void, out: *mut WinBuf, dirty: *mut c_void) -> i32;
    fn ANativeWindow_unlockAndPost(w: *mut c_void) -> i32;
}

/// `WINDOW_FORMAT_RGBA_8888`.
#[cfg(target_os = "android")]
const RGBA_8888: i32 = 1;

// ── O'YNATUVCHI ────────────────────────────────────────────────────

struct Player {
    anim: i64,
    win: *mut c_void,
    w: usize,
    h: usize,
    frames: usize,
    /// Manba kadrlaridan qanchasi o'tkazib yuboriladi (60 -> 30 kadr/s).
    step: f64,
    interval: Duration,
    /// Ko'rsatilgan kadrlar hisobi (vaqt bo'yicha emas — ketma-ket).
    n: u64,
    playing: bool,
    /// Yuza yangi ulandi / hali birorta kadr chizilmagan.
    need_draw: bool,
    next: Instant,
    buf: Vec<u8>,
}

// Ko'rsatkich faqat `Mutex` ichida, bir vaqtda bitta oqimda ishlatiladi.
unsafe impl Send for Player {}

struct Shared {
    p: Mutex<Player>,
    /// Hozir chizilyapti (navbatga ikki marta qo'yilmasin).
    busy: AtomicBool,
    /// Kamida bitta kadr yuzaga chizildi (Dart o'rinbosarni olib tashlaydi).
    drawn: AtomicBool,
}

fn players() -> &'static Mutex<HashMap<i64, Arc<Shared>>> {
    static M: OnceLock<Mutex<HashMap<i64, Arc<Shared>>>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(HashMap::new()))
}

static NEXT: AtomicI64 = AtomicI64::new(1);

fn get(id: i64) -> Option<Arc<Shared>> {
    players().lock().ok()?.get(&id).cloned()
}

// ── Oqimlar: bitta rejalashtiruvchi + bir nechta chizuvchi ─────────

fn queue() -> &'static mpsc::SyncSender<i64> {
    static Q: OnceLock<mpsc::SyncSender<i64>> = OnceLock::new();
    Q.get_or_init(|| {
        let (tx, rx) = mpsc::sync_channel::<i64>(1024);
        let rx = Arc::new(Mutex::new(rx));
        let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        let workers = (cores / 2).clamp(1, 4);
        for i in 0..workers {
            let rx = rx.clone();
            let _ = std::thread::Builder::new().name(format!("aru-anim-{i}")).spawn(move || loop {
                let id = match rx.lock() {
                    Ok(r) => match r.recv() {
                        Ok(v) => v,
                        Err(_) => return,
                    },
                    Err(_) => return,
                };
                if let Some(s) = get(id) {
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| draw(&s)));
                    s.busy.store(false, Ordering::SeqCst);
                }
            });
        }
        let tx2 = tx.clone();
        let _ = std::thread::Builder::new().name("aru-anim-clock".into()).spawn(move || scheduler(tx2));
        tx
    })
}

/// Vaqti kelgan o'yinchilarni navbatga qo'yadi.
fn scheduler(tx: mpsc::SyncSender<i64>) {
    loop {
        let now = Instant::now();
        let mut soonest = now + Duration::from_millis(50);
        let list: Vec<(i64, Arc<Shared>)> = match players().lock() {
            Ok(m) => m.iter().map(|(k, v)| (*k, v.clone())).collect(),
            Err(_) => Vec::new(),
        };
        for (id, s) in list {
            if s.busy.load(Ordering::SeqCst) {
                continue;
            }
            let Ok(p) = s.p.try_lock() else { continue };
            if p.win.is_null() || !(p.playing || p.need_draw) {
                continue;
            }
            if p.next <= now {
                drop(p);
                s.busy.store(true, Ordering::SeqCst);
                if tx.try_send(id).is_err() {
                    s.busy.store(false, Ordering::SeqCst);
                }
            } else if p.next < soonest {
                soonest = p.next;
            }
        }
        let wait = soonest.saturating_duration_since(Instant::now()).max(Duration::from_millis(2));
        std::thread::sleep(wait);
    }
}

/// Navbatdagi kadrni chizib, yuzaga qo'yadi.
fn draw(s: &Shared) {
    let Ok(mut p) = s.p.lock() else { return };
    if p.win.is_null() {
        return;
    }
    let frame = if p.frames == 0 { 0 } else { ((p.n as f64 * p.step) as usize) % p.frames };
    let anim = p.anim;
    let mut buf = std::mem::take(&mut p.buf);
    let ok = sticker_anim::render_into(anim, frame, &mut buf) == 1;
    p.buf = buf;
    if ok && blit(&p) {
        s.drawn.store(true, Ordering::SeqCst);
    }
    p.need_draw = false;
    if p.playing && p.frames > 1 {
        p.n = p.n.wrapping_add(1);
    }
    // Kechiksa — sakramaydi, keyingi kadr interval o'tib (silliq).
    let now = Instant::now();
    let due = p.next + p.interval;
    p.next = if due < now { now + p.interval / 2 } else { due };
}

#[cfg(target_os = "android")]
fn blit(p: &Player) -> bool {
    unsafe {
        let mut b = WinBuf { width: 0, height: 0, stride: 0, format: 0, bits: std::ptr::null_mut(), reserved: [0; 6] };
        if ANativeWindow_lock(p.win, &mut b, std::ptr::null_mut()) != 0 || b.bits.is_null() {
            return false;
        }
        let w = (b.width.max(0) as usize).min(p.w);
        let h = (b.height.max(0) as usize).min(p.h);
        let stride = b.stride.max(0) as usize * 4;
        let dst = b.bits as *mut u8;
        for y in 0..h {
            std::ptr::copy_nonoverlapping(p.buf.as_ptr().add(y * p.w * 4), dst.add(y * stride), w * 4);
        }
        ANativeWindow_unlockAndPost(p.win);
    }
    true
}

#[cfg(not(target_os = "android"))]
fn blit(_p: &Player) -> bool {
    false
}

// ── C API (Dart) ───────────────────────────────────────────────────

/// Ochadi: `{"path","w","h"}` -> `{"id","frames"}` (yoki `{"error"}`).
/// O'yinchi yuzasiz va to'xtagan holda yaratiladi.
#[no_mangle]
pub extern "C" fn rust_player_open(json_ptr: *const std::ffi::c_char) -> *mut std::ffi::c_char {
    let arg: Value = serde_json::from_str(unsafe { cstr_to_str(json_ptr) }.unwrap_or("{}")).unwrap_or(json!({}));
    let path = arg["path"].as_str().unwrap_or("");
    let w = arg["w"].as_i64().unwrap_or(0).clamp(1, 512) as usize;
    let h = arg["h"].as_i64().unwrap_or(0).clamp(1, 512) as usize;
    let anim = sticker_anim::open_path(path, w as i32, h as i32);
    if anim <= 0 {
        return string_to_cptr(json!({"error": "ochilmadi"}).to_string());
    }
    let (frames, fps) = sticker_anim::info(anim);
    // Telegram `limitFps`: 30 kadr/s dan oshmaydi.
    let show = if fps > 30.0 { 30.0 } else if fps > 0.0 { fps } else { 30.0 };
    let src = if fps > 0.0 { fps } else { show };
    let p = Player {
        anim,
        win: std::ptr::null_mut(),
        w,
        h,
        frames: frames.max(1),
        step: src / show,
        interval: Duration::from_secs_f64(1.0 / show),
        n: 0,
        playing: false,
        need_draw: true,
        next: Instant::now(),
        buf: vec![0u8; w * h * 4],
    };
    let id = NEXT.fetch_add(1, Ordering::SeqCst);
    if let Ok(mut m) = players().lock() {
        m.insert(id, Arc::new(Shared { p: Mutex::new(p), busy: AtomicBool::new(false), drawn: AtomicBool::new(false) }));
    }
    let _ = queue();
    string_to_cptr(json!({"id": id, "frames": frames}).to_string())
}

/// O'ynasin (1) yoki to'xtasin (0).
#[no_mangle]
pub extern "C" fn rust_player_set(id: i64, playing: i32) {
    if let Some(s) = get(id) {
        if let Ok(mut p) = s.p.lock() {
            let on = playing != 0 && p.frames > 1;
            if on && !p.playing {
                p.next = Instant::now();
            }
            p.playing = on;
        }
    }
}

/// Kamida bitta kadr ekranga chiqdimi (1/0).
#[no_mangle]
pub extern "C" fn rust_player_drawn(id: i64) -> i32 {
    get(id).map(|s| s.drawn.load(Ordering::SeqCst) as i32).unwrap_or(0)
}

/// Yopadi (yuza allaqachon ajratilgan bo'lishi kerak).
#[no_mangle]
pub extern "C" fn rust_player_free(id: i64) {
    let s = players().lock().ok().and_then(|mut m| m.remove(&id));
    if let Some(s) = s {
        if let Ok(mut p) = s.p.lock() {
            detach_locked(&mut p);
            sticker_anim::close(p.anim);
            p.anim = 0;
        }
    }
}

fn detach_locked(p: &mut Player) {
    if !p.win.is_null() {
        #[cfg(target_os = "android")]
        unsafe {
            ANativeWindow_release(p.win)
        };
        p.win = std::ptr::null_mut();
    }
}

// ── JNI — io.flutter.plugins.videoplayer.AruAnimTextures ─────────────

use jni::objects::{JClass, JObject};
use jni::sys::jlong;
use jni::JNIEnv;

/// Yuza tayyor (yoki qayta yaratildi) — Rust unga chiza boshlaydi.
#[no_mangle]
pub extern "system" fn Java_io_flutter_plugins_videoplayer_AruAnimTextures_nativeAttach<'l>(
    env: JNIEnv<'l>,
    _c: JClass<'l>,
    id: jlong,
    surface: JObject<'l>,
) {
    let Some(s) = get(id) else { return };
    let Ok(mut p) = s.p.lock() else { return };
    detach_locked(&mut p);
    #[cfg(target_os = "android")]
    unsafe {
        let w = ANativeWindow_fromSurface(env.get_raw() as *mut c_void, surface.as_raw() as *mut c_void);
        if !w.is_null() {
            ANativeWindow_setBuffersGeometry(w, p.w as i32, p.h as i32, RGBA_8888);
            p.win = w;
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (&env, &surface);
    }
    p.need_draw = true;
    p.next = Instant::now();
}

/// Yuza olib qo'yilmoqda — Rust unga boshqa chizmaydi.
#[no_mangle]
pub extern "system" fn Java_io_flutter_plugins_videoplayer_AruAnimTextures_nativeDetach<'l>(
    _env: JNIEnv<'l>,
    _c: JClass<'l>,
    id: jlong,
) {
    if let Some(s) = get(id) {
        if let Ok(mut p) = s.p.lock() {
            detach_locked(&mut p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn player_opens_and_frees() {
        let dir = std::env::temp_dir().join(format!("aru_player_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let f = dir.join("g.mp4");
        std::fs::write(&f, include_bytes!("testdata/gif_h264.mp4")).unwrap();
        let arg = CString::new(json!({"path": f.to_string_lossy(), "w": 32, "h": 24}).to_string()).unwrap();
        let r = unsafe { CString::from_raw(rust_player_open(arg.as_ptr())) };
        let v: Value = serde_json::from_str(r.to_str().unwrap()).unwrap();
        let id = v["id"].as_i64().expect("id");
        assert_eq!(v["frames"].as_i64(), Some(10));
        rust_player_set(id, 1);
        // Yuzasiz — chizilmaydi, lekin yiqilmaydi ham.
        std::thread::sleep(Duration::from_millis(60));
        assert_eq!(rust_player_drawn(id), 0);
        rust_player_free(id);
        assert!(get(id).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
