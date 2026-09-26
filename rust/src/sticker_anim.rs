//! Telegram animatsiyali stikerlari va maxsus emoji kadrlarini chizish.
//!
//! TALAB (foydalanuvchi): "stikerlar Telegram'dagidek harakatlansin,
//! to'plamlarni ochganda ilova qotmasin — Cherrygram qanday qilsa
//! shunday qil".
//!
//! Telegram/Cherrygram (`RLottieDrawable`, `AnimatedFileDrawable`)
//! stikerlarni tizim pleyeri bilan EMAS, ilova ichidagi kutubxonalar
//! bilan, FON OQIMIDA chizadi:
//!   * `.tgs` — tlottie (Telegram Android / Cherrygram'ning o'z Lottie
//!     chizgichi, sof Rust; gzip'langan Lottie JSON);
//!   * `.webm` — libvpx: rang va shaffoflik ikki alohida VP9 oqimi
//!     (`native/vp9_shim.c`).
//!
//! Bu yerda ham xuddi shunday: Dart kadrni fon isolate'idan so'raydi
//! (`rust_anim_render`), UI oqimi faqat tayyor rasmni chizadi.

use std::collections::HashMap;
use std::ffi::{c_char, c_void};
use std::io::Read;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use crate::ffi_utils::cstr_to_str;

extern "C" {
    fn aru_vp9_open() -> *mut c_void;
    fn aru_vp9_close(p: *mut c_void);
    fn aru_vp9_decode(
        p: *mut c_void,
        color: *const u8,
        clen: usize,
        alpha: *const u8,
        alen: usize,
        out: *mut u8,
        w: i32,
        h: i32,
    ) -> i32;
}

/// WebM'dagi bitta kadr: rang va (bo'lsa) shaffoflik bloki.
struct WebmFrame {
    color: Vec<u8>,
    alpha: Option<Vec<u8>>,
}

enum Kind {
    /// Telegram'ning o'z Lottie chizgichi (tlottie).
    Lottie { r: Box<tlottie::CPURenderer>, frames: usize, fps: f64 },
    Webm { dec: *mut c_void, frames: Vec<WebmFrame>, fps: f64, next: usize },
}

// Ko'rsatkichlar faqat `Mutex` ichida, bitta oqimda ishlatiladi.
unsafe impl Send for Kind {}

struct Anim {
    kind: Kind,
    w: usize,
    h: usize,
}

impl Drop for Anim {
    fn drop(&mut self) {
        unsafe {
            match &self.kind {
                Kind::Lottie { .. } => {}
                Kind::Webm { dec, .. } => {
                    if !dec.is_null() {
                        aru_vp9_close(*dec)
                    }
                }
            }
        }
    }
}

fn anims() -> &'static Mutex<HashMap<i64, Arc<Mutex<Anim>>>> {
    static M: OnceLock<Mutex<HashMap<i64, Arc<Mutex<Anim>>>>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(HashMap::new()))
}

static NEXT: AtomicI64 = AtomicI64::new(1);

/// Eng katta chizish o'lchami (xotira va vaqt uchun).
const MAX_SIDE: usize = 512;

fn open_lottie(bytes: &[u8]) -> Option<Kind> {
    // `.tgs` — gzip; oddiy JSON ham qabul qilinadi.
    let json = if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut out = Vec::new();
        flate2::read::GzDecoder::new(bytes).take(16 * 1024 * 1024).read_to_end(&mut out).ok()?;
        out
    } else {
        bytes.to_vec()
    };
    // Telegram Android (`jni/lottie.cpp`) kabi: tlottie, standart
    // cheklovlar, RGBA tartibi (Flutter premultiplied RGBA kutadi).
    let comp = tlottie::Composition::parse(&json, &tlottie::Limits::default()).ok()?;
    let frames = comp.frame_count().max(1) as usize;
    let fps = comp.frame_rate as f64;
    let r = Box::new(tlottie::CPURenderer::new(comp));
    Some(Kind::Lottie { r, frames, fps: if fps > 0.0 { fps } else { 30.0 } })
}

// ── WEBM (EBML) ─────────────────────────────────────────────────
//
// To'liq demuxer kerak emas: stiker — bitta VP9 yo'lak, 3 soniyagacha.
// Elementlar ketma-ket o'qiladi; "ichiga kiriladigan" (master)
// elementlarning o'lchami e'tiborsiz — ularning bolalari shu oqimda
// davom etadi. Shu tufayli o'lchami noma'lum klasterlar ham o'qiladi.

const ID_SEGMENT: u32 = 0x1853_8067;
const ID_CLUSTER: u32 = 0x1F43_B675;
const ID_TRACKS: u32 = 0x1654_AE6B;
const ID_TRACK_ENTRY: u32 = 0xAE;
const ID_BLOCK_GROUP: u32 = 0xA0;
const ID_BLOCK: u32 = 0xA1;
const ID_SIMPLE_BLOCK: u32 = 0xA3;
const ID_BLOCK_ADDITIONS: u32 = 0x75A1;
const ID_BLOCK_MORE: u32 = 0xA6;
const ID_BLOCK_ADDITIONAL: u32 = 0xA5;
const ID_DEFAULT_DURATION: u32 = 0x23_E383;
const ID_INFO: u32 = 0x1549_A966;
const ID_VIDEO: u32 = 0xE0;

fn read_id(b: &[u8], p: &mut usize) -> Option<u32> {
    let first = *b.get(*p)?;
    let len = first.leading_zeros() as usize + 1;
    if len > 4 || *p + len > b.len() {
        return None;
    }
    let mut v: u32 = 0;
    for i in 0..len {
        v = (v << 8) | b[*p + i] as u32;
    }
    *p += len;
    Some(v)
}

/// Uzunlik (vint). `None` ichida `u64::MAX` — "noma'lum".
fn read_size(b: &[u8], p: &mut usize) -> Option<u64> {
    let first = *b.get(*p)?;
    let len = first.leading_zeros() as usize + 1;
    if len > 8 || *p + len > b.len() {
        return None;
    }
    let mask = if len == 8 { 0 } else { 0xFFu8 >> len };
    let mut v: u64 = (first & mask) as u64;
    let mut all_ones = (first & mask) == mask;
    for i in 1..len {
        let x = b[*p + i];
        all_ones &= x == 0xFF;
        v = (v << 8) | x as u64;
    }
    *p += len;
    Some(if all_ones { u64::MAX } else { v })
}

fn uint(b: &[u8]) -> u64 {
    b.iter().fold(0u64, |a, x| (a << 8) | *x as u64)
}

/// Blok ichidan kadr baytlari (yo'lak raqami, vaqt, bayroqlar tashlanadi).
fn block_payload(b: &[u8]) -> Option<Vec<u8>> {
    let mut p = 0;
    read_size(b, &mut p)?; // yo'lak raqami (vint)
    p += 3; // vaqt (2) + bayroqlar (1); stikerlarda "lacing" yo'q
    b.get(p..).map(|s| s.to_vec())
}

fn parse_webm(b: &[u8]) -> Option<(Vec<WebmFrame>, f64)> {
    let mut p = 0;
    let mut frames: Vec<WebmFrame> = Vec::new();
    let mut default_ns: u64 = 0;
    while p < b.len() {
        let id = read_id(b, &mut p)?;
        let size = read_size(b, &mut p)?;
        match id {
            // Ichiga kiriladi.
            ID_SEGMENT | ID_CLUSTER | ID_TRACKS | ID_TRACK_ENTRY | ID_BLOCK_GROUP | ID_BLOCK_ADDITIONS
            | ID_BLOCK_MORE | ID_INFO | ID_VIDEO => continue,
            _ => {}
        }
        if size == u64::MAX {
            return None;
        }
        let end = p.checked_add(size as usize)?;
        if end > b.len() {
            break;
        }
        let data = &b[p..end];
        match id {
            ID_SIMPLE_BLOCK | ID_BLOCK => {
                if let Some(color) = block_payload(data) {
                    frames.push(WebmFrame { color, alpha: None });
                }
            }
            ID_BLOCK_ADDITIONAL => {
                if let Some(last) = frames.last_mut() {
                    last.alpha = Some(data.to_vec());
                }
            }
            ID_DEFAULT_DURATION => default_ns = uint(data),
            _ => {}
        }
        p = end;
    }
    if frames.is_empty() {
        return None;
    }
    let fps = if default_ns > 0 { 1e9 / default_ns as f64 } else { 30.0 };
    Some((frames, fps.clamp(1.0, 60.0)))
}

fn open_webm(bytes: &[u8]) -> Option<Kind> {
    let (frames, fps) = parse_webm(bytes)?;
    let dec = unsafe { aru_vp9_open() };
    if dec.is_null() {
        return None;
    }
    Some(Kind::Webm { dec, frames, fps, next: 0 })
}

/// Faylni ochadi: `.tgs`/Lottie yoki `.webm`. Qaytadi: tutqich (0 — xato).
#[no_mangle]
pub extern "C" fn rust_anim_open(path_ptr: *const c_char, w: i32, h: i32) -> i64 {
    std::panic::catch_unwind(|| open_anim(path_ptr, w, h)).unwrap_or(0)
}

fn open_anim(path_ptr: *const c_char, w: i32, h: i32) -> i64 {
    let Some(path) = (unsafe { cstr_to_str(path_ptr) }) else { return 0 };
    let Ok(bytes) = std::fs::read(path) else { return 0 };
    let kind = if bytes.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        open_webm(&bytes)
    } else {
        open_lottie(&bytes)
    };
    let Some(kind) = kind else { return 0 };
    let w = (w.max(1) as usize).min(MAX_SIDE);
    let h = (h.max(1) as usize).min(MAX_SIDE);
    let id = NEXT.fetch_add(1, Ordering::SeqCst);
    if let Ok(mut m) = anims().lock() {
        m.insert(id, Arc::new(Mutex::new(Anim { kind, w, h })));
    }
    id
}

fn get(id: i64) -> Option<Arc<Mutex<Anim>>> {
    anims().lock().ok()?.get(&id).cloned()
}

/// Kadrlar soni.
#[no_mangle]
pub extern "C" fn rust_anim_frames(id: i64) -> i32 {
    let Some(a) = get(id) else { return 0 };
    let Ok(a) = a.lock() else { return 0 };
    match &a.kind {
        Kind::Lottie { frames, .. } => *frames as i32,
        Kind::Webm { frames, .. } => frames.len() as i32,
    }
}

/// Soniyasiga kadr.
#[no_mangle]
pub extern "C" fn rust_anim_fps(id: i64) -> f64 {
    let Some(a) = get(id) else { return 0.0 };
    let Ok(a) = a.lock() else { return 0.0 };
    match &a.kind {
        Kind::Lottie { fps, .. } | Kind::Webm { fps, .. } => *fps,
    }
}

/// [frame] ni [out] ga (w*h*4, premultiplied RGBA) chizadi.
/// Qaytadi: 1 — tayyor, 0 — bu kadr ko'rinmaydi (oldingisi qoladi),
/// -1 — xato.
#[no_mangle]
pub extern "C" fn rust_anim_render(id: i64, frame: i32, out: *mut u8) -> i32 {
    // Buzuq stiker faylida chizgich `panic` qilsa — ilova yopilmaydi,
    // shu stiker birinchi kadrida qoladi.
    std::panic::catch_unwind(|| render_frame(id, frame, out)).unwrap_or(-1)
}

fn render_frame(id: i64, frame: i32, out: *mut u8) -> i32 {
    let Some(a) = get(id) else { return -1 };
    let Ok(mut a) = a.lock() else { return -1 };
    let (w, h) = (a.w, a.h);
    if out.is_null() {
        return -1;
    }
    match &mut a.kind {
        Kind::Lottie { r, frames, .. } => {
            let f = (frame.max(0) as usize).min(*frames - 1);
            let px = unsafe { std::slice::from_raw_parts_mut(out as *mut u32, w * h) };
            // Premultiplied RGBA ([R,G,B,A] baytlar), avval tozalanadi.
            match r.render(f as f32, px, w as u32, h as u32, tlottie::RenderOptions::default()) {
                Ok(()) => 1,
                Err(_) => -1,
            }
        }
        Kind::Webm { dec, frames, next, .. } => {
            let want = (frame.max(0) as usize).min(frames.len() - 1);
            // VP9 — ketma-ket: orqaga qaytilsa dekoder boshidan.
            if want < *next {
                unsafe {
                    aru_vp9_close(*dec);
                    *dec = aru_vp9_open();
                }
                if dec.is_null() {
                    return -1;
                }
                *next = 0;
            }
            let mut r = 0;
            while *next <= want {
                let f = &frames[*next];
                let (ap, al) = match &f.alpha {
                    Some(v) => (v.as_ptr(), v.len()),
                    None => (std::ptr::null(), 0),
                };
                r = unsafe {
                    aru_vp9_decode(*dec, f.color.as_ptr(), f.color.len(), ap, al, out, w as i32, h as i32)
                };
                *next += 1;
                if r < 0 {
                    return -1;
                }
            }
            r
        }
    }
}

/// Yopadi (xotira bo'shaydi).
#[no_mangle]
pub extern "C" fn rust_anim_close(id: i64) {
    if let Ok(mut m) = anims().lock() {
        m.remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ebml_vint() {
        let b = [0x81u8];
        let mut p = 0;
        assert_eq!(read_size(&b, &mut p), Some(1));
        let b = [0x01u8, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        let mut p = 0;
        assert_eq!(read_size(&b, &mut p), Some(u64::MAX));
        let b = [0x1A, 0x45, 0xDF, 0xA3];
        let mut p = 0;
        assert_eq!(read_id(&b, &mut p), Some(0x1A45_DFA3));
    }

    #[test]
    fn lottie_renders() {
        // Eng oddiy Lottie: 10 kadr, 20x20, bitta qizil to'rtburchak.
        let json = r##"{"v":"5.5.2","fr":30,"ip":0,"op":10,"w":20,"h":20,"layers":[
          {"ty":1,"sc":"#ff0000","sw":20,"sh":20,"ip":0,"op":10,"st":0,
           "ks":{"o":{"a":0,"k":100},"r":{"a":0,"k":0},"p":{"a":0,"k":[10,10,0]},
                 "a":{"a":0,"k":[10,10,0]},"s":{"a":0,"k":[100,100,100]}}}]}"##;
        let kind = open_lottie(json.as_bytes()).expect("ochilmadi");
        let Kind::Lottie { frames, fps, .. } = &kind else { panic!("lottie emas") };
        // Harakatsiz kompozitsiyani tlottie bitta kadr deb sanaydi
        // (animatsiyalida — op - ip).
        assert_eq!(*frames, 1);
        assert!((fps - 30.0).abs() < 0.01);
        let id = NEXT.fetch_add(1, Ordering::SeqCst);
        anims().lock().unwrap().insert(id, Arc::new(Mutex::new(Anim { kind, w: 8, h: 8 })));
        let mut buf = vec![0u8; 8 * 8 * 4];
        assert_eq!(rust_anim_render(id, 3, buf.as_mut_ptr()), 1);
        // Markazdagi nuqta — to'liq qizil (RGBA).
        let c = &buf[(4 * 8 + 4) * 4..(4 * 8 + 4) * 4 + 4];
        assert_eq!(c, &[255, 0, 0, 255]);
        rust_anim_close(id);
    }

    /// FFmpeg Telegram video stikerini qanday joylasa, xuddi shunday
    /// fayl: 64x64, 2 kadr, chap yarmi qizil va ko'rinadigan, o'ng
    /// yarmi to'liq shaffof (rang `Block` da, alpha `BlockAdditional`
    /// da). libvpx'ning o'z `vpxenc` i bilan yasalgan.
    #[test]
    fn webm_alpha_sticker() {
        let bytes = include_bytes!("testdata/alpha_sticker.webm");
        let (frames, fps) = parse_webm(bytes).expect("webm o'qilmadi");
        assert_eq!(frames.len(), 2);
        assert!(frames.iter().all(|f| f.alpha.is_some()));
        assert!((fps - 30.0).abs() < 0.1);
        let kind = open_webm(bytes).expect("dekoder");
        let id = NEXT.fetch_add(1, Ordering::SeqCst);
        anims().lock().unwrap().insert(id, Arc::new(Mutex::new(Anim { kind, w: 32, h: 32 })));
        let mut buf = vec![0u8; 32 * 32 * 4];
        assert_eq!(rust_anim_render(id, 1, buf.as_mut_ptr()), 1);
        let px = |x: usize, y: usize| &buf[(y * 32 + x) * 4..(y * 32 + x) * 4 + 4];
        // Chapda — qizil, ko'rinadi.
        let l = px(4, 16);
        assert!(l[0] > 200 && l[1] < 60 && l[2] < 60 && l[3] == 255, "chap: {l:?}");
        // O'ngda — to'liq shaffof (premultiplied: rang ham 0).
        assert_eq!(px(28, 16), &[0, 0, 0, 0]);
        // Orqaga qaytish (qayta boshlash) ham ishlaydi.
        assert_eq!(rust_anim_render(id, 0, buf.as_mut_ptr()), 1);
        rust_anim_close(id);
    }

    #[test]
    fn vp9_decoder_opens() {
        let d = unsafe { aru_vp9_open() };
        assert!(!d.is_null());
        unsafe { aru_vp9_close(d) };
    }
}
