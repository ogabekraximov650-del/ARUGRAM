//! To'plam fayllari (Kotlin `Packs.kt`): Telegram'dagi `.arup` faylidan sarlavha va elementlarni bo'lak-bo'lak o'qish.
//! Fayl: `ARUP` + 16 bayt sarlavha, JSON sarlavha, so'ng kichik rasmlar (WebP), so'ng elementlar. O'qish yadroning shifrlangan keshi orqali.

use serde_json::Value;
use std::collections::HashMap;
use std::ffi::CString;
use std::os::raw::c_char;
use std::sync::{Arc, Mutex, OnceLock};
use uzcoders::prelude::Image;

extern "C" {
    fn rust_player_open(name: *const c_char, size_hint: i64) -> i64;
    fn rust_player_read(h: i64, pos: i64, buf: *mut u8, len: i32) -> i32;
}

#[derive(Clone, Debug)]
pub struct Item {
    pub id: i64,
    pub off: i64,
    pub len: i64,
    pub toff: i64,
    pub tlen: i64,
    /// 0 — statik, 1 — animatsiyali (WebP), 2 — video.
    pub anim: u8,
    pub w: i32,
    pub h: i32,
    pub emoji: String,
}

#[derive(Clone, Debug)]
pub struct Header {
    pub id: i64,
    pub ver: i64,
    pub base: i64,
    pub items: Vec<Item>,
}

fn handles() -> &'static Mutex<HashMap<String, i64>> {
    static H: OnceLock<Mutex<HashMap<String, i64>>> = OnceLock::new();
    H.get_or_init(|| Mutex::new(HashMap::new()))
}

fn headers() -> &'static Mutex<HashMap<i64, Arc<Header>>> {
    static H: OnceLock<Mutex<HashMap<i64, Arc<Header>>>> = OnceLock::new();
    H.get_or_init(|| Mutex::new(HashMap::new()))
}

fn dir() -> String {
    let d = format!("{}/cache/packs", uzcoders::android::files_dir());
    let _ = std::fs::create_dir_all(&d);
    d
}

/// Faylni Telegram'dan yetkazib, o'qish dastagini beradi (har fayl uchun bir marta).
fn handle(session: &str, file: &str) -> Result<i64, String> {
    if let Some(h) = handles().lock().ok().and_then(|m| m.get(file).copied()) {
        return Ok(h);
    }
    if !crate::downloads::ensure_cache() {
        return Err("Kesh ishga tushmadi".into());
    }
    let ok = crate::data::tg_deliver(session, &[file.to_string()])?;
    if !ok.iter().any(|f| f == file) {
        return Err("To'plam fayli Telegram'da topilmadi".into());
    }
    let c = CString::new(file).map_err(|_| "Fayl nomi noto'g'ri".to_string())?;
    let mut h = 0i64;
    for _ in 0..20 {
        h = unsafe { rust_player_open(c.as_ptr(), 0) };
        if h != -2 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(400));
    }
    if h <= 0 {
        return Err("To'plam fayli ochilmadi".into());
    }
    if let Ok(mut m) = handles().lock() {
        m.insert(file.to_string(), h);
    }
    Ok(h)
}

/// `[off, off+len)` ni o'qiydi (fayl oxirida qisqa bo'lishi mumkin).
fn read_range(session: &str, file: &str, off: i64, len: i64) -> Result<Vec<u8>, String> {
    if len <= 0 {
        return Ok(Vec::new());
    }
    let h = handle(session, file)?;
    let mut out = vec![0u8; len as usize];
    let mut got = 0usize;
    let mut waits = 0;
    while got < out.len() {
        let want = (out.len() - got).min(1 << 20) as i32;
        let n = unsafe { rust_player_read(h, off + got as i64, out[got..].as_mut_ptr(), want) };
        match n {
            n if n > 0 => {
                got += n as usize;
                waits = 0;
            }
            0 => break,
            -2 => {
                waits += 1;
                if waits > 80 {
                    return Err("Telegram javob bermadi".into());
                }
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
            _ => return Err("O'qib bo'lmadi".into()),
        }
    }
    out.truncate(got);
    Ok(out)
}

fn parse_header(j: &Value, base: i64) -> Header {
    let items = j["items"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|o| {
                    let n = |k: &str| o[k].as_i64().unwrap_or(0);
                    Item { id: n("i"), off: n("o"), len: n("l"), toff: n("to"), tlen: n("tl"), anim: n("a") as u8, w: n("w") as i32, h: n("h") as i32, emoji: o["e"].as_str().unwrap_or("").into() }
                })
                .collect()
        })
        .unwrap_or_default();
    Header { id: j["id"].as_i64().unwrap_or(0), ver: j["ver"].as_i64().unwrap_or(0), base, items }
}

/// Sarlavha: xotira -> disk -> Telegram (birinchi 256 KB).
pub fn header(session: &str, file: &str, pack: i64, ver: i64) -> Result<Arc<Header>, String> {
    if let Some(h) = headers().lock().ok().and_then(|m| m.get(&pack).cloned()).filter(|h| h.ver >= ver) {
        return Ok(h);
    }
    let disk = format!("{}/hdr_{pack}.json", dir());
    if let Some(j) = std::fs::read_to_string(&disk).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
        let base = j["base"].as_i64().unwrap_or(0);
        if base >= 16 {
            let h = parse_header(&j, base);
            if h.ver >= ver {
                let h = Arc::new(h);
                if let Ok(mut m) = headers().lock() {
                    m.insert(pack, h.clone());
                }
                return Ok(h);
            }
        }
    }
    let first = read_range(session, file, 0, 262_144)?;
    if first.len() < 16 || &first[0..4] != b"ARUP" {
        return Err("To'plam fayli noto'g'ri".into());
    }
    let hlen = u32::from_be_bytes([first[8], first[9], first[10], first[11]]) as i64;
    if hlen <= 0 || hlen > 16 << 20 {
        return Err("To'plam sarlavhasi noto'g'ri".into());
    }
    let mut bytes = first;
    if 16 + hlen > bytes.len() as i64 {
        let rest = read_range(session, file, bytes.len() as i64, 16 + hlen - bytes.len() as i64)?;
        bytes.extend(rest);
    }
    let end = (16 + hlen) as usize;
    if bytes.len() < end {
        return Err("To'plam sarlavhasi to'liq emas".into());
    }
    let mut j: Value = serde_json::from_slice(&bytes[16..end]).map_err(|_| "Sarlavha o'qilmadi".to_string())?;
    let h = parse_header(&j, 16 + hlen);
    j["base"] = Value::from(16 + hlen);
    let _ = std::fs::write(&disk, j.to_string());
    let h = Arc::new(h);
    if let Ok(mut m) = headers().lock() {
        m.insert(pack, h.clone());
    }
    Ok(h)
}

/// Barcha kichik rasmlarni bir so'rovda o'qib dekodlaydi (diskdagi nusxa bo'lsa — tarmoqsiz). Qaytadi: (element, rasm).
pub fn thumbs(session: &str, file: &str, h: &Header) -> Vec<(i64, Image)> {
    let d = dir();
    let mut out: Vec<(i64, Image)> = Vec::new();
    let mut missing: Vec<&Item> = Vec::new();
    for it in &h.items {
        let p = format!("{d}/t_{}_{}", h.id, it.id);
        match std::fs::read(&p).ok().and_then(|b| crate::webp::decode_any(&b)) {
            Some(img) => out.push((it.id, img.resized(192, 192))),
            None => {
                if it.tlen > 0 {
                    missing.push(it);
                }
            }
        }
    }
    if missing.is_empty() {
        return out;
    }
    let lo = missing.iter().map(|i| i.toff).min().unwrap_or(0);
    let hi = missing.iter().map(|i| i.toff + i.tlen).max().unwrap_or(0);
    if hi - lo > 24 << 20 {
        return out;
    }
    let Ok(blob) = read_range(session, file, h.base + lo, hi - lo) else { return out };
    for it in missing {
        let (a, b) = ((it.toff - lo) as usize, (it.toff - lo + it.tlen) as usize);
        if b > blob.len() {
            continue;
        }
        let bytes = &blob[a..b];
        let _ = std::fs::write(format!("{d}/t_{}_{}", h.id, it.id), bytes);
        if let Some(img) = crate::webp::decode_any(bytes) {
            out.push((it.id, img.resized(192, 192)));
        }
    }
    out
}

/// Elementning o'zi (katta ko'rinish): birinchi kadr.
pub fn data_image(session: &str, file: &str, h: &Header, item: &Item) -> Option<Image> {
    if item.anim == 2 || item.len <= 0 || item.len > 16 << 20 {
        return None;
    }
    let p = format!("{}/d_{}_{}", dir(), h.id, item.id);
    let bytes = match std::fs::read(&p) {
        Ok(b) if b.len() as i64 == item.len => b,
        _ => {
            let b = read_range(session, file, h.base + item.off, item.len).ok()?;
            if b.len() as i64 == item.len {
                let _ = std::fs::write(&p, &b);
            }
            b
        }
    };
    crate::webp::decode_any(&bytes).map(|i| i.resized(720, 720))
}

/// Bitta elementning rasmi (izohdagi stiker/GIF): to'plam ma'lumoti -> sarlavha -> statik bo'lsa o'zi, aks holda kichik rasm.
pub fn item_image(session: &str, pack: i64, item: i64) -> Option<Image> {
    let d = dir();
    let info_path = format!("{d}/info_{pack}.json");
    let info: Value = match std::fs::read_to_string(&info_path).ok().and_then(|t| serde_json::from_str(&t).ok()) {
        Some(v) => v,
        None => {
            let j = crate::api::get_json("/api/packs/info", &format!("?ids={pack}"), Some(session)).ok()?;
            let p = j["packs"].as_array()?.iter().find(|p| p["id"].as_i64() == Some(pack))?.clone();
            let _ = std::fs::write(&info_path, p.to_string());
            p
        }
    };
    let file = info["file"].as_str().filter(|f| !f.is_empty() && *f != "null")?.to_string();
    let ver = info["version"].as_i64().unwrap_or(0);
    let h = header(session, &file, pack, ver).ok()?;
    let it = h.items.iter().find(|i| i.id == item)?.clone();
    if it.anim == 0 {
        if let Some(img) = data_image(session, &file, &h, &it) {
            return Some(img);
        }
    }
    thumbs_one(session, &file, &h, &it)
}

fn thumbs_one(session: &str, file: &str, h: &Header, it: &Item) -> Option<Image> {
    let p = format!("{}/t_{}_{}", dir(), h.id, it.id);
    let bytes = match std::fs::read(&p) {
        Ok(b) if !b.is_empty() => b,
        _ => {
            let b = read_range(session, file, h.base + it.toff, it.tlen).ok()?;
            let _ = std::fs::write(&p, &b);
            b
        }
    };
    crate::webp::decode_any(&bytes).map(|i| i.resized(256, 256))
}
