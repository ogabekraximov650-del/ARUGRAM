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
/// Diskdagi bo'lak o'lchami (Kotlin `BlockCache.BLOCK`).
const BLOCK: u64 = 1024 * 1024;
/// Bitta tarmoq so'rovida ko'pi bilan nechta bo'lak (ortiqcha bayt kelmasin).
const RUN_MAX: u64 = 8;

/// Telefondagi shifrlangan bo'lak keshi papkasi (UZ: ko'rilgan joy keyingi safar tarmoqsiz ochiladi — Kotlin `BlockCache` kabi).
pub fn cache_dir() -> String {
    let d = format!("{}/vdcache", uzcoders::android::files_dir());
    let _ = std::fs::create_dir_all(&d);
    d
}

fn block_len(total: u64, idx: u64) -> usize {
    BLOCK.min(total - idx * BLOCK) as usize
}

fn block_read(name: &str, idx: u64, total: u64) -> Option<Vec<u8>> {
    let p = format!("{}/{name}__{idx}", cache_dir());
    let v = crate::secure::read(&p, &format!("vd:{name}:{idx}"))?;
    if v.len() == block_len(total, idx) { Some(v) } else { None }
}

fn block_write(name: &str, idx: u64, plain: &[u8]) {
    let _ = crate::secure::write(&format!("{}/{name}__{idx}", cache_dir()), &format!("vd:{name}:{idx}"), plain);
}
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
    if let Some(v) = info_local(name) {
        return Some(v);
    }
    info_net(name)
}

/// Faqat xotira/disk: tarmoqsiz.
fn info_local(name: &str) -> Option<(u64, String)> {
    if let Some(v) = INFO.lock().ok()?.as_ref().and_then(|m| m.get(name).cloned()) {
        return Some(v);
    }
    if let Some(t) = crate::secure::read_text(&format!("{}/{name}__info", cache_dir()), "vdinfo") {
        if let Some((a, b)) = t.split_once('|') {
            if let (Ok(size), true) = (a.parse::<u64>(), b.len() == 32) {
                INFO.lock().ok()?.get_or_insert_with(HashMap::new).insert(name.to_string(), (size, b.to_string()));
                return Some((size, b.to_string()));
            }
        }
    }
    None
}

fn info_net(name: &str) -> Option<(u64, String)> {
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
    let _ = crate::secure::write_text(&format!("{}/{name}__info", cache_dir()), "vdinfo", &format!("{size}|{key}"));
    Some((size, key))
}

/// Oynani Telegram'dan Cloudflare keshiga isitadi (kerak bo'lsa kutadi). `true` — tayyor.
fn warm(name: &str, w: u64) -> bool {
    warm_opts(name, w, false)
}

/// `force` — Cloudflare kesh o'chib ketgan bo'lishi mumkin: eski belgi tashlanadi va oyna majburan qayta isitiladi.
fn warm_opts(name: &str, w: u64, force: bool) -> bool {
    if force {
        if let Ok(mut s) = WARM.lock() {
            if let Some(set) = s.as_mut() {
                set.remove(&(name.to_string(), w));
            }
        }
    }
    let mut first = force;
    if !force && WARM.lock().ok().and_then(|s| s.as_ref().map(|s| s.contains(&(name.to_string(), w)))).unwrap_or(false) {
        return true;
    }
    for _ in 0..40 {
        let q = if first { format!("?w={w}&force=1") } else { format!("?w={w}") };
        first = false;
        let Ok(text) = crate::api::request_long("GET", &format!("/api/warm/vd~{name}"), &q, 120) else { return false };
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
    // Bo'lak telefon keshida bo'lsa tarmoqsiz beriladi; bo'lmasa shu oynaning keyingi bo'laklari (ko'pi bilan RUN_MAX) bitta so'rovda olinadi,
    // shifri ochiladi, keshga yoziladi va pleyerga uzatiladi. Keyingi oyna fonda oldindan isitiladi (Kotlin `VdAhead`).
    s.write_all(head.as_bytes())?;
    let mut buf = vec![0u8; 64 * 1024];
    let mut off = start;
    while off <= end {
        let b = off / BLOCK;
        if let Some(plain) = block_read(name, b, total) {
            let bs = b * BLOCK;
            let from = (off - bs) as usize;
            let to = ((end - bs + 1) as usize).min(plain.len());
            s.write_all(&plain[from..to])?;
            off = bs + to as u64;
            continue;
        }
        let w = off / CHUNK;
        if !warm(name, w) {
            return Ok(());
        }
        prefetch_ahead(name, w, total);
        let win_last_block = ((w + 1) * CHUNK / BLOCK).saturating_sub(1);
        let mut last_b = (end / BLOCK).min(win_last_block).min(b + RUN_MAX - 1);
        // Keshda bor bo'laklarga yetganda to'xtaymiz (ular diskdan olinadi).
        for k in (b + 1)..=last_b {
            if std::path::Path::new(&format!("{}/{name}__{k}", cache_dir())).is_file() {
                last_b = k - 1;
                break;
            }
        }
        let (rs, re) = (b * BLOCK, ((last_b + 1) * BLOCK).min(total) - 1);
        let resp = match agent().get(&play_url(name)).set("Range", &format!("bytes={rs}-{re}")).call() {
            Ok(r) if r.status() == 206 || (r.status() == 200 && rs == 0 && re + 1 == total) => r,
            _ => return Ok(()),
        };
        let mut r = resp.into_reader();
        let mut pos = rs;
        let mut cur = b;
        let mut blk: Vec<u8> = Vec::with_capacity(BLOCK as usize);
        while pos <= re {
            let want = ((re - pos + 1) as usize).min(buf.len());
            let n = r.read(&mut buf[..want])?;
            if n == 0 {
                return Ok(());
            }
            crate::vd::apply(&key, pos, &mut buf[..n]);
            // Pleyerga: faqat so'ralgan joydan (`off`) boshlab.
            let upto = (pos + n as u64).min(end + 1);
            if upto > off {
                let skip = off.saturating_sub(pos) as usize;
                s.write_all(&buf[skip..(upto - pos) as usize])?;
                off = upto;
            }
            blk.extend_from_slice(&buf[..n]);
            pos += n as u64;
            while blk.len() >= block_len(total, cur) && !blk.is_empty() {
                let l = block_len(total, cur);
                block_write(name, cur, &blk[..l]);
                blk.drain(..l);
                cur += 1;
                if cur * BLOCK >= total {
                    break;
                }
            }
        }
    }
    Ok(())
}

/// Umumiy HTTP mijoz (ulanishlar qayta ishlatiladi: har bo'lak uchun yangi TLS qo'l siqish kerak emas).
fn agent() -> &'static ureq::Agent {
    static A: OnceLock<ureq::Agent> = OnceLock::new();
    A.get_or_init(|| ureq::AgentBuilder::new().max_idle_connections_per_host(8).timeout_connect(Duration::from_secs(20)).timeout_read(Duration::from_secs(60)).build())
}

/// Joriy oynadan keyingi `AHEAD` (1) oynani fonda isitadi. Bir fayl uchun bitta oqim.
const AHEAD: u64 = 1;

fn prefetch_ahead(name: &str, cur_w: u64, total: u64) {
    static BUSY: Mutex<Option<HashSet<String>>> = Mutex::new(None);
    {
        let Ok(mut g) = BUSY.lock() else { return };
        if !g.get_or_insert_with(HashSet::new).insert(name.to_string()) {
            return;
        }
    }
    let n = name.to_string();
    std::thread::spawn(move || {
        for k in 1..=AHEAD {
            let w = cur_w + k;
            if w * CHUNK >= total {
                break;
            }
            if !warm(&n, w) {
                break;
            }
        }
        if let Ok(mut g) = BUSY.lock() {
            if let Some(set) = g.as_mut() {
                set.remove(&n);
            }
        }
    });
}

// ── Yuklab olish (Kotlin `VdDownloads`): fayl 18 MiB oynalar bo'yicha olinadi, 1 MiB bo'laklar diskka shifrlab yoziladi ──

struct DlRt {
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    retrying: std::sync::atomic::AtomicBool,
    speed: std::sync::atomic::AtomicI64,
    /// Jami olingan bayt va tezlik o'lchash nuqtasi (Kotlin `Job.got/lastGot`).
    got: std::sync::atomic::AtomicU64,
    sample: Mutex<(std::time::Instant, u64)>,
}

static DLS: Mutex<Option<HashMap<String, std::sync::Arc<DlRt>>>> = Mutex::new(None);

fn dl_get(name: &str) -> Option<std::sync::Arc<DlRt>> {
    DLS.lock().ok()?.as_ref()?.get(name).cloned()
}

fn block_exists(name: &str, idx: u64) -> bool {
    std::path::Path::new(&format!("{}/{name}__{idx}", cache_dir())).is_file()
}

/// (hajm, telefonda bor bayt, yuklanmoqda, qayta urinilmoqda, tezlik B/s). Hajm noma'lum bo'lsa 0.
pub fn dl_stats(name: &str) -> (u64, u64, bool, bool, i64) {
    let Some((total, _)) = info_local(name) else {
        let rt = dl_get(name);
        return (0, 0, rt.is_some(), rt.map(|r| r.retrying.load(std::sync::atomic::Ordering::Relaxed)).unwrap_or(false), 0);
    };
    let nb = total.div_ceil(BLOCK);
    let have: u64 = (0..nb).filter(|&i| block_exists(name, i)).map(|i| block_len(total, i) as u64).sum();
    let rt = dl_get(name);
    let (downloading, retrying, speed) = match &rt {
        Some(r) => {
            if let Ok(mut sm) = r.sample.lock() {
                let el = sm.0.elapsed();
                if el.as_millis() >= 900 {
                    let g = r.got.load(std::sync::atomic::Ordering::Relaxed);
                    r.speed.store((g.saturating_sub(sm.1) as f64 / el.as_secs_f64()) as i64, std::sync::atomic::Ordering::Relaxed);
                    *sm = (std::time::Instant::now(), g);
                }
            }
            (true, r.retrying.load(std::sync::atomic::Ordering::Relaxed), r.speed.load(std::sync::atomic::Ordering::Relaxed))
        }
        None => (false, false, 0),
    };
    (total, have, downloading && have < total, retrying, speed)
}

/// Yuklanishi kerak bo'lgan fayllar ro'yxati (ilova qayta ochilganda davom ettiriladi).
fn wanted_path() -> String {
    format!("{}/dl_wanted", cache_dir())
}

fn wanted_get() -> Vec<String> {
    crate::secure::read_text(&wanted_path(), "vdwanted").map(|t| t.lines().filter(|l| !l.is_empty()).map(String::from).collect()).unwrap_or_default()
}

fn wanted_set(name: &str, on: bool) {
    static LOCK: Mutex<()> = Mutex::new(());
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut v = wanted_get();
    v.retain(|x| x != name);
    if on {
        v.push(name.to_string());
    }
    let _ = crate::secure::write_text(&wanted_path(), "vdwanted", &v.join("\n"));
}

/// Ilova ochilganda yarim qolgan yuklashlarni davom ettiradi.
pub fn dl_resume_all(session: &str) {
    for n in wanted_get() {
        dl_start(session, &n);
    }
}

pub fn dl_pause(name: &str) {
    wanted_set(name, false);
    if let Some(r) = dl_get(name) {
        r.cancel.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

pub fn dl_delete(name: &str) {
    dl_pause(name);
    if let Ok(rd) = std::fs::read_dir(cache_dir()) {
        for e in rd.flatten() {
            if e.file_name().to_string_lossy().starts_with(&format!("{name}__")) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

pub fn dl_wipe() {
    if let Ok(g) = DLS.lock() {
        for r in g.iter().flat_map(|m| m.values()) {
            r.cancel.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }
    if let Ok(rd) = std::fs::read_dir(cache_dir()) {
        for e in rd.flatten() {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// Yuklashni fonda boshlaydi (allaqachon ketayotgan bo'lsa — hech narsa qilmaydi). Pauza/o'chirishda `cancel` o'rnatiladi.
pub fn dl_start(session: &str, name: &str) {
    use std::sync::atomic::Ordering::SeqCst;
    if !valid_name(name) {
        return;
    }
    if let Ok(mut s) = SESSION.lock() {
        *s = session.to_string();
    }
    let rt = std::sync::Arc::new(DlRt { cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)), retrying: std::sync::atomic::AtomicBool::new(false), speed: std::sync::atomic::AtomicI64::new(0), got: std::sync::atomic::AtomicU64::new(0), sample: Mutex::new((std::time::Instant::now(), 0)) });
    {
        let Ok(mut g) = DLS.lock() else { return };
        let m = g.get_or_insert_with(HashMap::new);
        if let Some(old) = m.get(name) {
            if !old.cancel.load(SeqCst) {
                return;
            }
        }
        m.insert(name.to_string(), rt.clone());
    }
    wanted_set(name, true);
    let _ = std::fs::write(format!("{}/{name}__pin", cache_dir()), b"1");
    let n = name.to_string();
    std::thread::spawn(move || {
        let mut fails = 0;
        loop {
            match dl_run(&n, &rt) {
                Ok(()) => break,
                Err(_) if rt.cancel.load(SeqCst) => break,
                Err(_) => {
                    // Tarmoq uzilsa to'xtab qolmaydi: urinish oraliqi oshib boradi (3..30 s), foydalanuvchi pauza bosmaguncha davom etadi.
                    fails += 1;
                    rt.retrying.store(true, SeqCst);
                    for _ in 0..(3 * fails.min(10)) {
                        if rt.cancel.load(SeqCst) {
                            break;
                        }
                        std::thread::sleep(Duration::from_secs(1));
                    }
                }
            }
        }
        if !rt.cancel.load(SeqCst) {
            wanted_set(&n, false);
        }
        rt.speed.store(0, SeqCst);
        if let Ok(mut g) = DLS.lock() {
            if let Some(m) = g.as_mut() {
                if m.get(&n).map(|c| std::sync::Arc::ptr_eq(c, &rt)).unwrap_or(false) {
                    m.remove(&n);
                }
            }
        }
    });
}

/// Bir vaqtda nechta 18 MiB oyna parallel olinadi (Kotlin `WINDOWS_AT_ONCE`).
const WINDOWS_AT_ONCE: u64 = 3;
const MAX_TRIES: u32 = 40;

fn dl_run(name: &str, rt: &std::sync::Arc<DlRt>) -> Result<(), String> {
    use std::sync::atomic::{AtomicU64, Ordering::SeqCst};
    let (total, key) = info(name).ok_or("video ma'lumoti yo'q")?;
    let windows = total.div_ceil(CHUNK);
    let next = std::sync::Arc::new(AtomicU64::new(0));
    let mut hs = Vec::new();
    for _ in 0..WINDOWS_AT_ONCE.min(windows) {
        let (n, k, rt, next) = (name.to_string(), key.clone(), rt.clone(), next.clone());
        hs.push(std::thread::spawn(move || {
            while !rt.cancel.load(SeqCst) {
                let w = next.fetch_add(1, SeqCst);
                if w >= windows {
                    break;
                }
                let mut tries = 0u32;
                let mut carry: (u64, Vec<u8>) = (u64::MAX, Vec::new());
                while !rt.cancel.load(SeqCst) {
                    match dl_window(&n, &k, total, w, &rt, &mut carry, tries > 0) {
                        Ok(()) => {
                            rt.retrying.store(false, SeqCst);
                            break;
                        }
                        Err(_) if rt.cancel.load(SeqCst) => break,
                        Err(_) => {
                            tries += 1;
                            if tries >= MAX_TRIES {
                                break;
                            }
                            rt.retrying.store(true, SeqCst);
                            for _ in 0..(2 * tries).min(15) {
                                if rt.cancel.load(SeqCst) {
                                    break;
                                }
                                std::thread::sleep(Duration::from_secs(1));
                            }
                        }
                    }
                }
            }
        }));
    }
    for h in hs {
        let _ = h.join();
    }
    if rt.cancel.load(SeqCst) {
        return Ok(());
    }
    let nb = total.div_ceil(BLOCK);
    if (0..nb).all(|i| block_exists(name, i)) { Ok(()) } else { Err("to'liq emas".into()) }
}

/// Bitta 18 MiB oyna: yetishmayotgan birinchi bo'lakdan oyna oxirigacha BITTA so'rov bilan; uzilsa chala bo'lak saqlanadi va aynan shu baytdan davom etadi.
fn dl_window(name: &str, key: &str, total: u64, w: u64, rt: &DlRt, carry: &mut (u64, Vec<u8>), force: bool) -> Result<(), String> {
    use std::sync::atomic::Ordering::SeqCst;
    let nb = total.div_ceil(BLOCK);
    let first = w * CHUNK / BLOCK;
    let last = (first + CHUNK / BLOCK - 1).min(nb - 1);
    let resume = carry.0 != u64::MAX && !carry.1.is_empty();
    let Some(miss) = (first..=last).find(|&b| !block_exists(name, b)) else { return Ok(()) };
    if !warm_opts(name, w, force) {
        return Err("oyna isitilmadi".into());
    }
    let (mut cur, mut blk): (u64, Vec<u8>) = if resume && carry.0 >= miss && carry.0 <= last { (carry.0, std::mem::take(&mut carry.1)) } else { (miss, Vec::new()) };
    carry.0 = u64::MAX;
    let rs = cur * BLOCK + blk.len() as u64;
    let re = ((last + 1) * BLOCK).min(total) - 1;
    let resp = agent().get(&play_url(name)).set("Range", &format!("bytes={rs}-{re}")).call().map_err(|e| e.to_string())?;
    if !(resp.status() == 206 || (resp.status() == 200 && rs == 0 && re + 1 == total)) {
        return Err(format!("HTTP {}", resp.status()));
    }
    let mut r = resp.into_reader();
    let mut buf = vec![0u8; 64 * 1024];
    let mut pos = rs;
    let res: Result<(), String> = (|| {
        while pos <= re {
            if rt.cancel.load(SeqCst) {
                return Ok(());
            }
            let want = ((re - pos + 1) as usize).min(buf.len());
            let n = r.read(&mut buf[..want]).map_err(|e| e.to_string())?;
            if n == 0 {
                return Err("uzildi".into());
            }
            crate::vd::apply(key, pos, &mut buf[..n]);
            blk.extend_from_slice(&buf[..n]);
            pos += n as u64;
            rt.got.fetch_add(n as u64, SeqCst);
            while cur <= last && blk.len() >= block_len(total, cur) {
                let l = block_len(total, cur);
                if !block_exists(name, cur) {
                    block_write(name, cur, &blk[..l]);
                }
                blk.drain(..l);
                cur += 1;
            }
        }
        Ok(())
    })();
    if res.is_err() {
        *carry = (cur, blk);
    }
    res
}

/// Telefon keshi hajmini cheklaydi (Kotlin `BlockCache` 2 GB): eng eskilaridan boshlab o'chiriladi, lekin YUKLAB OLINGAN (qadalgan, `<nom>__pin`) videolar tegilmaydi.
pub fn gc(max_bytes: u64) {
    let Ok(rd) = std::fs::read_dir(cache_dir()) else { return };
    let mut pinned: HashSet<String> = HashSet::new();
    let mut files: Vec<(std::time::SystemTime, u64, std::path::PathBuf, String)> = Vec::new();
    for e in rd.flatten() {
        let fname = e.file_name().to_string_lossy().into_owned();
        let Some((n, _)) = fname.split_once("__") else { continue };
        if fname.ends_with("__pin") {
            pinned.insert(n.to_string());
            continue;
        }
        let Ok(m) = e.metadata() else { continue };
        files.push((m.modified().unwrap_or(std::time::UNIX_EPOCH), m.len(), e.path(), n.to_string()));
    }
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    if total <= max_bytes {
        return;
    }
    files.sort_by_key(|f| f.0);
    for (_, len, p, n) in files {
        if total <= max_bytes {
            break;
        }
        if pinned.contains(&n) {
            continue;
        }
        if std::fs::remove_file(&p).is_ok() {
            total -= len;
        }
    }
}

// ── Pleyer uchun serversiz o'qish (Kotlin `EncCacheDataSource` + `VdBlocks`): Java `VdDataSource` JNI orqali shu yerdan o'qiydi ──

static INFL: Mutex<Option<HashSet<(String, u64)>>> = Mutex::new(None);

/// Ochilgan (shifri yechilgan) so'nggi bo'laklar xotirada: pleyer har o'qishda 1 MiB ni diskdan qayta ochmasin (har 16-64 KB uchun shifr ochish qotishga sabab edi).
static MEM: Mutex<Vec<(String, u64, std::sync::Arc<Vec<u8>>)>> = Mutex::new(Vec::new());
const MEM_CAP: usize = 8;

fn mem_get(name: &str, idx: u64) -> Option<std::sync::Arc<Vec<u8>>> {
    MEM.lock().ok()?.iter().find(|(n, i, _)| n == name && *i == idx).map(|(_, _, b)| b.clone())
}

fn mem_put(name: &str, idx: u64, b: std::sync::Arc<Vec<u8>>) {
    if let Ok(mut g) = MEM.lock() {
        g.retain(|(n, i, _)| !(n == name && *i == idx));
        g.push((name.to_string(), idx, b));
        while g.len() > MEM_CAP {
            g.remove(0);
        }
    }
}

fn infl_claim(name: &str, idx: u64) -> bool {
    INFL.lock().ok().map(|mut g| g.get_or_insert_with(HashSet::new).insert((name.to_string(), idx))).unwrap_or(false)
}

fn infl_release(name: &str, idx: u64) {
    if let Ok(mut g) = INFL.lock() {
        if let Some(s) = g.as_mut() {
            s.remove(&(name.to_string(), idx));
        }
    }
}

/// Fayl hajmi (pleyer ochilganda); tayyor bo'lmasa `None`.
pub fn size_of(name: &str) -> Option<u64> {
    if !valid_name(name) {
        return None;
    }
    info(name).map(|(t, _)| t)
}

/// `pos` dan boshlab `out` ni to'ldiradi (ko'pi bilan bitta 1 MiB bo'lak ichidan). `Ok(0)` — fayl oxiri.
pub fn read_at(name: &str, pos: u64, out: &mut [u8]) -> Result<usize, String> {
    let (total, _) = info(name).ok_or("video ma'lumoti yo'q")?;
    if pos >= total {
        return Ok(0);
    }
    let idx = pos / BLOCK;
    let blk = get_block(name, idx, total)?;
    maybe_readahead(name, idx, total);
    let off = (pos - idx * BLOCK) as usize;
    if off >= blk.len() {
        return Ok(0);
    }
    let n = out.len().min(blk.len() - off);
    out[..n].copy_from_slice(&blk[off..off + n]);
    Ok(n)
}

fn infl_has(name: &str, idx: u64) -> bool {
    INFL.lock().ok().map(|g| g.as_ref().map(|s| s.contains(&(name.to_string(), idx))).unwrap_or(false)).unwrap_or(false)
}

/// Ko'rilayotgan joydan oldinda 10 MiB dan kam (telefonda yoki yo'lda) qolsa, KEYINGI 18 MiB oyna bitta so'rovda olinadi (Kotlin `AheadPlan`);
/// undan ko'p oldindan olinmaydi.
const AHEAD_MIN_BLOCKS: u64 = 10;

fn maybe_readahead(name: &str, idx: u64, total: u64) {
    static LAST: Mutex<Option<(String, u64)>> = Mutex::new(None);
    {
        let Ok(mut g) = LAST.lock() else { return };
        if g.as_ref().map(|(n, i)| n == name && *i == idx).unwrap_or(false) {
            return;
        }
        *g = Some((name.to_string(), idx));
    }
    let nb = total.div_ceil(BLOCK);
    let mut k = idx + 1;
    while k < nb && k - idx <= AHEAD_MIN_BLOCKS && (block_exists(name, k) || infl_has(name, k)) {
        k += 1;
    }
    if k >= nb || k - idx > AHEAD_MIN_BLOCKS {
        return;
    }
    if infl_claim(name, k) {
        let n = name.to_string();
        std::thread::spawn(move || {
            let _ = start_run(&n, k, total);
        });
    }
}

fn get_block(name: &str, idx: u64, total: u64) -> Result<std::sync::Arc<Vec<u8>>, String> {
    for _ in 0..100 {
        if let Some(b) = mem_get(name, idx) {
            return Ok(b);
        }
        if let Some(b) = block_read(name, idx, total) {
            let a = std::sync::Arc::new(b);
            mem_put(name, idx, a.clone());
            return Ok(a);
        }
        if infl_claim(name, idx) {
            let a = std::sync::Arc::new(start_run(name, idx, total)?);
            mem_put(name, idx, a.clone());
            return Ok(a);
        }
        // Boshqa so'rov shu bo'lakni olyapti — tugashini kutamiz.
        std::thread::sleep(Duration::from_millis(10));
    }
    Err("kutilmoqda".into())
}

/// `idx` dan boshlab shu oynaning oxirigacha (keshda yoki band bo'lakkacha) BITTA so'rov bilan oladi. Birinchi bo'lak tayyor bo'lishi bilan qaytadi,
/// qolganlari fonda davom etadi (pleyer buferi to'lguncha oldindan olinadi; Kotlin `VdAhead`).
fn start_run(name: &str, idx: u64, total: u64) -> Result<Vec<u8>, String> {
    let nb = total.div_ceil(BLOCK);
    let w = idx * BLOCK / CHUNK;
    let win_last = (((w + 1) * CHUNK / BLOCK).saturating_sub(1)).min(nb - 1);
    let mut last = idx;
    while last < win_last {
        let k = last + 1;
        if block_exists(name, k) || !infl_claim(name, k) {
            break;
        }
        last = k;
    }
    // Keyingi oyna Telegramdan keshga DARHOL isitila boshlaydi (u kelganda tayyor bo'lsin; Kotlin `VdWarm.onRead`).
    if (w + 1) * CHUNK < total {
        prefetch_ahead(name, w, total);
    }
    let (tx, rx) = std::sync::mpsc::channel::<Result<Vec<u8>, String>>();
    let n = name.to_string();
    std::thread::spawn(move || {
        let key = match info(&n) {
            Some((_, k)) => k,
            None => {
                for b in idx..=last {
                    infl_release(&n, b);
                }
                let _ = tx.send(Err("video ma'lumoti yo'q".into()));
                return;
            }
        };
        let mut sent = false;
        let mut done_to = idx; // [idx, done_to) yozilgan
        let mut force = false;
        let mut attempt = 0;
        let res: Result<(), String> = loop {
            attempt += 1;
            let r = run_fetch(&n, &key, total, w, done_to, last, force, &mut |b, data| {
                block_write(&n, b, &data);
                mem_put(&n, b, std::sync::Arc::new(data.clone()));
                infl_release(&n, b);
                done_to = b + 1;
                if b == idx && !sent {
                    sent = true;
                    let _ = tx.send(Ok(data));
                }
            });
            match r {
                Ok(()) => break Ok(()),
                Err(e) if attempt < 3 => {
                    // 503 — Cloudflare keshida oyna yo'q: majburan qayta isitiladi.
                    force = e.contains("503");
                    std::thread::sleep(Duration::from_millis(500 * attempt));
                }
                Err(e) => break Err(e),
            }
        };
        for b in done_to..=last {
            infl_release(&n, b);
        }
        if let (Err(e), false) = (res, sent) {
            let _ = tx.send(Err(e));
        }
    });
    // Uzoq kutilmaydi: Java qayta so'raydi (ExoPlayer yuklashni bekor qilganda native chaqiruv uzoq osilib qolmasin).
    match rx.recv_timeout(Duration::from_secs(2)) {
        Ok(r) => r,
        Err(_) => Err("kutilmoqda".into()),
    }
}

/// Oyna isitiladi va `[from_b, last]` bo'laklar bitta so'rov bilan olinadi; har tayyor bo'lak `on_block` ga beriladi.
fn run_fetch(name: &str, key: &str, total: u64, w: u64, from_b: u64, last: u64, force: bool, on_block: &mut dyn FnMut(u64, Vec<u8>)) -> Result<(), String> {
    if from_b > last {
        return Ok(());
    }
    if !warm_opts(name, w, force) {
        return Err("oyna isitilmadi".into());
    }
    let (rs, re) = (from_b * BLOCK, ((last + 1) * BLOCK).min(total) - 1);
    let resp = agent().get(&play_url(name)).set("Range", &format!("bytes={rs}-{re}")).call().map_err(|e| e.to_string())?;
    if !(resp.status() == 206 || (resp.status() == 200 && rs == 0 && re + 1 == total)) {
        return Err(format!("HTTP {}", resp.status()));
    }
    let mut r = resp.into_reader();
    let mut buf = vec![0u8; 64 * 1024];
    let mut blk: Vec<u8> = Vec::with_capacity(BLOCK as usize);
    let (mut pos, mut cur) = (rs, from_b);
    while pos <= re {
        let want = ((re - pos + 1) as usize).min(buf.len());
        let n = r.read(&mut buf[..want]).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("uzildi".into());
        }
        crate::vd::apply(key, pos, &mut buf[..n]);
        blk.extend_from_slice(&buf[..n]);
        pos += n as u64;
        while cur <= last && blk.len() >= block_len(total, cur) {
            let l = block_len(total, cur);
            let data: Vec<u8> = blk.drain(..l).collect();
            on_block(cur, data);
            cur += 1;
        }
    }
    Ok(())
}
