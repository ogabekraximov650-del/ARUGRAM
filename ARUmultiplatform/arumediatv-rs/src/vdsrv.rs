//! Mahalliy "ochuvchi" server: worker `vd` videolarini shifrdan ochib `http://127.0.0.1:<port>/vd/<nom>` ko'rinishida beradi (HTTP Range bilan).
//! UZ: video baytlari worker'dan shifrlangan keladi (AES-128-CTR, kalit `/api/vd/info` da); bu server ularni ochib beradi. Shu manzilni video kesh
//! serveri (`/thumb`, kadr yasash) va pleyer oddiy HTTP manba sifatida o'qiydi — Telegram'ga to'g'ridan-to'g'ri ulanish kerak emas.
//! EN: a loopback HTTP server that decrypts worker `vd` videos on the fly (Range supported).

use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::ffi::{CStr, CString};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

const CHUNK: u64 = 18 * 1024 * 1024;
/// Bitta javobda eng ko'pi (Range oxiri ko'rsatilmasa): mijoz keyingi qismni o'zi so'raydi.
const OPEN_ENDED_MAX: u64 = 32 * 1024 * 1024;

static PORT: OnceLock<u16> = OnceLock::new();
static SESSION: Mutex<String> = Mutex::new(String::new());
static INFO: Mutex<Option<HashMap<String, (u64, String)>>> = Mutex::new(None);
static WARM: Mutex<Option<HashSet<(String, u64)>>> = Mutex::new(None);

/// Serverni ishga tushiradi (bir marta) va sessiyani yangilaydi. Qaytadi: port (0 — bo'lmadi).
pub fn start(session: &str) -> u16 {
    if let Ok(mut s) = SESSION.lock() {
        *s = session.to_string();
    }
    *PORT.get_or_init(|| {
        let Ok(l) = TcpListener::bind("127.0.0.1:0") else { return 0 };
        let port = l.local_addr().map(|a| a.port()).unwrap_or(0);
        std::thread::spawn(move || {
            for c in l.incoming().flatten() {
                std::thread::spawn(move || {
                    let _ = handle(c);
                });
            }
        });
        port
    })
}

/// Shu video uchun mahalliy manzil (server ishga tushgan bo'lsa).
pub fn url(name: &str) -> Option<String> {
    let p = *PORT.get()?;
    if p == 0 {
        return None;
    }
    Some(format!("http://127.0.0.1:{p}/vd/{name}"))
}

fn session() -> String {
    SESSION.lock().map(|s| s.clone()).unwrap_or_default()
}

fn valid_name(n: &str) -> bool {
    !n.is_empty() && n.len() <= 200 && n != "." && n != ".." && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
}

/// Fayl hajmi va kaliti (`POST /api/vd/info`); tayyor (`ready`) bo'lsa xotirada saqlanadi.
fn info(name: &str) -> Option<(u64, String)> {
    if let Some(v) = INFO.lock().ok()?.as_ref().and_then(|m| m.get(name).cloned()) {
        return Some(v);
    }
    let body = serde_json::json!({ "names": [name] });
    let (code, text) = crate::api::request("POST", "/api/vd/info", "", Some(&body), Some(&session())).ok()?;
    if code != 200 {
        return None;
    }
    let j: Value = serde_json::from_str(&text).ok()?;
    let f = j["files"].get(name)?;
    let (size, key) = (f["size"].as_u64()?, f["key"].as_str()?.to_string());
    if f["state"].as_str() != Some("ready") || key.len() != 32 || size == 0 {
        return None;
    }
    INFO.lock().ok()?.get_or_insert_with(HashMap::new).insert(name.to_string(), (size, key.clone()));
    Some((size, key))
}

/// Oynani Telegram'dan Cloudflare keshiga isitadi (kerak bo'lsa kutadi). `true` — tayyor.
fn warm(name: &str, w: u64) -> bool {
    if WARM.lock().ok().and_then(|s| s.as_ref().map(|s| s.contains(&(name.to_string(), w)))).unwrap_or(false) {
        return true;
    }
    for _ in 0..40 {
        let Ok(text) = crate::api::request_long("GET", &format!("/api/warm/vd~{name}"), &format!("?w={w}"), 120) else { return false };
        let st = serde_json::from_str::<Value>(&text).ok().and_then(|j| j["status"].as_str().map(String::from)).unwrap_or_default();
        if st == "cached" || st == "warmed" {
            if let Ok(mut s) = WARM.lock() {
                s.get_or_insert_with(HashSet::new).insert((name.to_string(), w));
            }
            return true;
        }
        if !st.starts_with("pending") {
            return false;
        }
        std::thread::sleep(Duration::from_secs(3));
    }
    false
}

fn play_url(name: &str) -> String {
    let path = format!("/api/play/vd~{name}");
    let c = CString::new(path.as_str()).unwrap_or_default();
    let p = unsafe { rust_core::app_play_token(c.as_ptr(), 6 * 3600) };
    let t = if p.is_null() {
        String::new()
    } else {
        let s = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
        rust_core::ffi_utils::rust_free_string(p);
        s
    };
    format!("{}{}{}", crate::api::BASE, path, if t.is_empty() { String::new() } else { format!("?t={t}") })
}

fn reply(s: &mut TcpStream, code: u16, text: &str) -> std::io::Result<()> {
    write!(s, "HTTP/1.1 {code} {text}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
}

fn handle(mut s: TcpStream) -> std::io::Result<()> {
    s.set_read_timeout(Some(Duration::from_secs(20)))?;
    let mut rd = BufReader::new(s.try_clone()?);
    let mut line = String::new();
    rd.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let (method, target) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    let mut range: Option<(u64, Option<u64>)> = None;
    loop {
        let mut h = String::new();
        if rd.read_line(&mut h)? == 0 || h == "\r\n" || h == "\n" {
            break;
        }
        if let Some(v) = h.to_ascii_lowercase().strip_prefix("range:") {
            if let Some(r) = v.trim().strip_prefix("bytes=") {
                let mut it = r.split('-');
                let a = it.next().and_then(|x| x.trim().parse::<u64>().ok());
                let b = it.next().and_then(|x| x.trim().parse::<u64>().ok());
                if let Some(a) = a {
                    range = Some((a, b));
                }
            }
        }
    }
    if method != "GET" && method != "HEAD" {
        return reply(&mut s, 405, "Method Not Allowed");
    }
    let name = target.strip_prefix("/vd/").unwrap_or("").split('?').next().unwrap_or("");
    if !valid_name(name) {
        return reply(&mut s, 404, "Not Found");
    }
    let Some((total, key)) = info(name) else { return reply(&mut s, 404, "Not Found") };
    let (start, end, partial) = match range {
        Some((a, b)) if a < total => (a, b.map(|b| b.min(total - 1)).unwrap_or((a + OPEN_ENDED_MAX - 1).min(total - 1)), true),
        Some(_) => return reply(&mut s, 416, "Range Not Satisfiable"),
        None => (0, (OPEN_ENDED_MAX - 1).min(total - 1), false),
    };
    let (start, end) = (start, end.max(start));
    let ctype = if name.ends_with(".mp4") { "video/mp4" } else { "application/octet-stream" };
    let len = end - start + 1;
    // Faqat 206/200 sarlavhasi (HEAD uchun tana yo'q).
    let head = if partial || end + 1 < total {
        format!("HTTP/1.1 206 Partial Content\r\nContent-Type: {ctype}\r\nAccept-Ranges: bytes\r\nContent-Range: bytes {start}-{end}/{total}\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n")
    } else {
        format!("HTTP/1.1 200 OK\r\nContent-Type: {ctype}\r\nAccept-Ranges: bytes\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n")
    };
    if method == "HEAD" {
        return s.write_all(head.as_bytes());
    }
    // Sarlavha DARHOL yuboriladi (oyna isitilishini kutmasdan): pleyer ulanishni ochiq deb biladi va o'qish muddati bilan kutadi.
    // Oynalar ketma-ket isitiladi va uzatiladi: birinchi oyna tayyor bo'lishi bilan bayt ketadi (avval hamma oyna isitilguncha kutilardi),
    // keyingi oyna esa fonda oldindan isitiladi (Kotlin `VdAhead`).
    s.write_all(head.as_bytes())?;
    let mut buf = vec![0u8; 64 * 1024];
    let mut off = start;
    while off <= end {
        let w = off / CHUNK;
        let seg_end = end.min((w + 1) * CHUNK - 1);
        if !warm(name, w) {
            return Ok(());
        }
        if (w + 1) * CHUNK < total {
            prefetch(name, w + 1);
        }
        let resp = match agent().get(&play_url(name)).set("Range", &format!("bytes={off}-{seg_end}")).timeout(Duration::from_secs(120)).call() {
            Ok(r) if r.status() == 206 || (r.status() == 200 && off == 0 && seg_end + 1 == total) => r,
            _ => return Ok(()),
        };
        let mut r = resp.into_reader();
        while off <= seg_end {
            let want = ((seg_end - off + 1) as usize).min(buf.len());
            let n = r.read(&mut buf[..want])?;
            if n == 0 {
                return Ok(());
            }
            crate::vd::apply(&key, off, &mut buf[..n]);
            s.write_all(&buf[..n])?;
            off += n as u64;
        }
    }
    Ok(())
}

/// Umumiy HTTP mijoz (ulanishlar qayta ishlatiladi: har bo'lak uchun yangi TLS qo'l siqish kerak emas).
fn agent() -> &'static ureq::Agent {
    static A: OnceLock<ureq::Agent> = OnceLock::new();
    A.get_or_init(|| ureq::AgentBuilder::new().max_idle_connections_per_host(8).build())
}

/// Keyingi oynani fonda isitadi (bir oyna uchun bitta oqim).
fn prefetch(name: &str, w: u64) {
    static BUSY: Mutex<Option<HashSet<(String, u64)>>> = Mutex::new(None);
    let key = (name.to_string(), w);
    {
        let Ok(mut g) = BUSY.lock() else { return };
        if !g.get_or_insert_with(HashSet::new).insert(key.clone()) {
            return;
        }
    }
    let n = name.to_string();
    std::thread::spawn(move || {
        let _ = warm(&n, w);
        if let Ok(mut g) = BUSY.lock() {
            if let Some(set) = g.as_mut() {
                set.remove(&key);
            }
        }
    });
}
