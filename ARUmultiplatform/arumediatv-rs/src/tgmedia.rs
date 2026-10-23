//! Telegram hisobidagi stiker, maxsus (premium) emoji va saqlangan GIFlar (Kotlin `TgStickers.kt`). Hamma narsa yadro
//! (`rust_core::telegram::rust_tg_media_*`, MTProto) orqali olinadi; ro'yxatlar va rasmlar shifrlab diskda saqlanadi.
//! Animatsiyasiz (statik) ko'rinish: stiker — o'zi (WebP) yoki kichik rasmi, GIF/animatsion emoji — kichik rasmi.
//! Chatga faqat havola yuboriladi: `tgs.<to'plam_nomi>.<hujjat_id>` yoki `tgg.<id>.<kalit>.<dc>.<havola>` — fayl yuklanmaydi.

use serde_json::{json, Value};
use std::ffi::CString;
use uzcoders::prelude::Image;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TgDoc {
    pub id: i64,
    pub hash: i64,
    pub fref: String,
    pub dc: i32,
    pub kind: String,
    pub thumb: String,
    pub emoji: String,
    pub custom: bool,
    pub w: i32,
    pub h: i32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TgSet {
    pub id: i64,
    pub hash: i64,
    pub title: String,
    pub short: String,
    pub count: i64,
    pub thumb_ver: i64,
}

fn doc_of(v: &Value) -> Option<TgDoc> {
    Some(TgDoc {
        id: v["id"].as_i64()?,
        hash: v["hash"].as_i64()?,
        fref: v["ref"].as_str().unwrap_or("").to_string(),
        dc: v["dc"].as_i64().unwrap_or(2) as i32,
        kind: v["kind"].as_str().unwrap_or("").to_string(),
        thumb: v["thumb"].as_str().unwrap_or("").to_string(),
        emoji: v["emoji"].as_str().unwrap_or("").to_string(),
        custom: v["custom"].as_bool().unwrap_or(false),
        w: v["w"].as_i64().unwrap_or(0) as i32,
        h: v["h"].as_i64().unwrap_or(0) as i32,
    })
}

fn set_of(v: &Value) -> Option<TgSet> {
    Some(TgSet {
        id: v["id"].as_i64()?,
        hash: v["hash"].as_i64()?,
        title: v["title"].as_str().unwrap_or("").to_string(),
        short: v["short"].as_str().unwrap_or("").to_string(),
        count: v["count"].as_i64().unwrap_or(0),
        thumb_ver: v["thumb_ver"].as_i64().unwrap_or(0),
    })
}

fn dir() -> String {
    let d = format!("{}/tgm", crate::data::cache_dir());
    let _ = std::fs::create_dir_all(&d);
    d
}

fn age_secs(path: &str) -> u64 {
    std::fs::metadata(path).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).map(|d| d.as_secs()).unwrap_or(u64::MAX)
}

/// Yadroga ro'yxat so'rovi (BLOKLAYDI). Javob JSON'i yoki xato matni.
fn core_list(arg: &str) -> Result<Value, String> {
    crate::backend::ensure_tg_ready()?;
    let a = CString::new(arg).map_err(|_| "noto'g'ri so'rov".to_string())?;
    let j = crate::data::call_json(rust_core::telegram::rust_tg_media_list(a.as_ptr()));
    if let Some(e) = j["error"].as_str() {
        return Err(e.to_string());
    }
    if j["ok"].as_bool() != Some(true) {
        return Err("Telegram javob bermadi".into());
    }
    Ok(j)
}

/// Shifrlangan keshli ro'yxat: `ttl` soniyadan yangi bo'lsa — tarmoqsiz; tarmoq xato bersa — eskisi.
fn cached(key: &str, arg: &str, ttl: u64) -> Option<Value> {
    let path = format!("{}/{key}.json", dir());
    let old = crate::secure::read_text(&path, "tgm").and_then(|s| serde_json::from_str::<Value>(&s).ok());
    if old.is_some() && age_secs(&path) < ttl {
        return old;
    }
    match core_list(arg) {
        Ok(j) => {
            let _ = crate::secure::write_text(&path, "tgm", &j.to_string());
            Some(j)
        }
        Err(_) => old,
    }
}

/// Keshni o'chiradi (havolalar eskirganda), lekin so'nggi daqiqada yangilangan bo'lsa — tegmaydi (parallel so'rovlar Telegram'ni bosmasin).
fn invalidate(key: &str) {
    let path = format!("{}/{key}.json", dir());
    if age_secs(&path) >= 60 {
        let _ = std::fs::remove_file(path);
    }
}

/// Foydalanuvchining stiker (`emoji=false`) yoki maxsus emoji to'plamlari.
pub fn sets(emoji: bool) -> Vec<TgSet> {
    let (key, arg) = if emoji { ("sets_emoji", "sets:emoji") } else { ("sets_stickers", "sets:stickers") };
    cached(key, arg, 6 * 3600).and_then(|j| j["sets"].as_array().map(|a| a.iter().filter_map(set_of).collect())).unwrap_or_default()
}

/// To'plam hujjatlari. Havola (`file_reference`) eskirsa keshni `fresh=true` bilan yangilang.
pub fn set_docs(s: &TgSet, fresh: bool) -> Vec<TgDoc> {
    let key = format!("set_{}", s.id);
    if fresh {
        invalidate(&key);
    }
    cached(&key, &format!("set:{}:{}", s.id, s.hash), 12 * 3600)
        .and_then(|j| j["docs"].as_array().map(|a| a.iter().filter_map(doc_of).collect()))
        .unwrap_or_default()
}

/// Saqlangan GIFlar.
pub fn gifs(fresh: bool) -> Vec<TgDoc> {
    if fresh {
        invalidate("gifs");
    }
    cached("gifs", "gifs", 3600).and_then(|j| j["docs"].as_array().map(|a| a.iter().filter_map(doc_of).collect())).unwrap_or_default()
}

/// Ochiq nom bo'yicha to'plam hujjatlari (boshqa foydalanuvchi yuborgan stiker/emoji uchun).
pub fn named_docs(short: &str) -> Vec<TgDoc> {
    if short.is_empty() || !short.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Vec::new();
    }
    cached(&format!("name_{short}"), &format!("setname:{short}"), 24 * 3600)
        .and_then(|j| j["docs"].as_array().map(|a| a.iter().filter_map(doc_of).collect()))
        .unwrap_or_default()
}

/// Maxsus emoji hujjati raqami bo'yicha.
#[allow(dead_code)]
pub fn emoji_doc(id: i64) -> Option<TgDoc> {
    let path = format!("{}/ce_{id}.json", dir());
    if let Some(d) = crate::secure::read_text(&path, "tgm").and_then(|s| serde_json::from_str::<Value>(&s).ok()).and_then(|v| doc_of(&v)) {
        return Some(d);
    }
    let j = core_list(&format!("emoji:{id}")).ok()?;
    let v = j["docs"].as_array()?.first()?.clone();
    let _ = crate::secure::write_text(&path, "tgm", &v.to_string());
    doc_of(&v)
}

/// `tgs.<nom>.<id>` -> hujjat.
pub fn resolve_sticker(r: &str) -> Option<TgDoc> {
    let (short, id) = r.strip_prefix("tgs.")?.rsplit_once('.')?;
    let id: i64 = id.parse().ok()?;
    named_docs(short).into_iter().find(|d| d.id == id)
}

/// `tgs.<nom>.<id>` -> hujjat; to'plam keshi eski bo'lsa (havola eskirgan) yangilab qayta hal qiladi.
pub fn resolve_sticker_fresh(r: &str) -> Option<TgDoc> {
    let (short, _) = r.strip_prefix("tgs.")?.rsplit_once('.')?;
    invalidate(&format!("name_{short}"));
    resolve_sticker(r)
}

pub fn sticker_ref(short: &str, d: &TgDoc) -> String {
    format!("tgs.{short}.{}", d.id)
}

pub fn gif_ref(d: &TgDoc) -> String {
    format!("tgg.{:x}.{:x}.{}.{}", d.id, d.hash, d.dc, if d.fref.is_empty() { "x" } else { &d.fref })
}

pub fn resolve_gif(r: &str) -> Option<TgDoc> {
    let p: Vec<&str> = r.strip_prefix("tgg.")?.split('.').collect();
    if p.len() != 4 {
        return None;
    }
    Some(TgDoc {
        id: i64::from_str_radix(p[0], 16).ok().or_else(|| u64::from_str_radix(p[0], 16).ok().map(|v| v as i64))?,
        hash: i64::from_str_radix(p[1], 16).ok().or_else(|| u64::from_str_radix(p[1], 16).ok().map(|v| v as i64))?,
        dc: p[2].parse().ok()?,
        fref: if p[3] == "x" { String::new() } else { p[3].to_string() },
        kind: "mp4".into(),
        thumb: "m".into(),
        ..Default::default()
    })
}

/// Bir vaqtda ko'pi bilan 4 ta yuklash (panel o'nlab rasm so'raydi).
fn slot<T>(f: impl FnOnce() -> T) -> T {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static BUSY: AtomicUsize = AtomicUsize::new(0);
    loop {
        let n = BUSY.load(Ordering::SeqCst);
        if n < 4 && BUSY.compare_exchange(n, n + 1, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(60));
    }
    let r = f();
    BUSY.fetch_sub(1, Ordering::SeqCst);
    r
}

fn fetch(arg: Value, path: &str) -> Result<(), String> {
    crate::backend::ensure_tg_ready()?;
    let mut v = arg;
    v["out"] = json!(path);
    let a = CString::new(v.to_string()).map_err(|_| "noto'g'ri so'rov".to_string())?;
    let j = crate::data::call_json(rust_core::telegram::rust_tg_media_get(a.as_ptr()));
    match j["error"].as_str() {
        Some(e) => Err(e.to_string()),
        None if j["ok"].as_bool() == Some(true) => Ok(()),
        None => Err("Telegram javob bermadi".into()),
    }
}

/// Hujjat rasmi (BLOKLAYDI): diskdagi shifrlangan nusxa yoki Telegram'dan. Statik WebP stiker — to'liq fayl,
/// qolganlari (tgs/webm/mp4 va h.k.) — kichik rasm. `Err("FILE_REFERENCE_EXPIRED")` — ro'yxatni yangilash kerak.
pub fn image(d: &TgDoc, px: u32) -> Result<Image, String> {
    let full = d.kind == "webp";
    let thumb = if full { "" } else if d.thumb.is_empty() { "m" } else { d.thumb.as_str() };
    let path = format!("{}/img_{:x}_{}.bin", dir(), d.id, if full { "f" } else { thumb });
    if let Some(img) = crate::secure::read(&path, "tgm").and_then(|b| crate::webp::decode_any(&b)) {
        return Ok(img.resized(px, px));
    }
    slot(|| {
        let tmp = format!("{path}.dl");
        let r = fetch(json!({"id": d.id, "hash": d.hash, "ref": d.fref, "dc": d.dc, "thumb": thumb, "max": 6 * 1024 * 1024}), &tmp);
        let bytes = std::fs::read(&tmp).ok();
        let _ = std::fs::remove_file(&tmp);
        r?;
        let bytes = bytes.ok_or_else(|| "fayl o'qilmadi".to_string())?;
        let img = crate::webp::decode_any(&bytes).ok_or_else(|| "rasm dekodlanmadi".to_string())?;
        let _ = crate::secure::write(&path, "tgm", &bytes);
        Ok(img.resized(px, px))
    })
}

/// Hujjatning to'liq baytlari (kichik fayllar: `.tgs` stikerlar). Diskda shifrlab saqlanadi. `Err("FILE_REFERENCE_EXPIRED")` — ro'yxatni yangilash kerak.
pub fn file_bytes(d: &TgDoc, max: usize) -> Result<Vec<u8>, String> {
    let path = format!("{}/file_{:x}.bin", dir(), d.id);
    if let Some(b) = crate::secure::read(&path, "tgm") {
        return Ok(b);
    }
    slot(|| {
        let tmp = format!("{path}.dl");
        let r = fetch(json!({"id": d.id, "hash": d.hash, "ref": d.fref, "dc": d.dc, "thumb": "", "max": max}), &tmp);
        let bytes = std::fs::read(&tmp).ok();
        let _ = std::fs::remove_file(&tmp);
        r?;
        let bytes = bytes.ok_or_else(|| "fayl o'qilmadi".to_string())?;
        let _ = crate::secure::write(&path, "tgm", &bytes);
        Ok(bytes)
    })
}

/// To'plam muqovasi (kichik rasm); yo'q bo'lsa — birinchi hujjatning rasmi.
pub fn set_cover(s: &TgSet, px: u32) -> Result<Image, String> {
    let path = format!("{}/cover_{:x}_{}.bin", dir(), s.id, s.thumb_ver);
    if let Some(img) = crate::secure::read(&path, "tgm").and_then(|b| crate::webp::decode_any(&b)) {
        return Ok(img.resized(px, px));
    }
    let got = slot(|| {
        let tmp = format!("{path}.dl");
        let r = fetch(json!({"set_id": s.id, "set_hash": s.hash, "set_ver": s.thumb_ver, "dc": 2, "max": 2 * 1024 * 1024}), &tmp);
        let b = std::fs::read(&tmp).ok();
        let _ = std::fs::remove_file(&tmp);
        r.ok().and(b)
    });
    if let Some(img) = got.as_ref().and_then(|b| crate::webp::decode_any(b)) {
        let _ = crate::secure::write(&path, "tgm", got.as_ref().map(|b| b.as_slice()).unwrap_or(&[]));
        return Ok(img.resized(px, px));
    }
    let first = set_docs(s, false).into_iter().next().ok_or_else(|| "to'plam bo'sh".to_string())?;
    image(&first, px)
}

/// Matndagi `[ce:<nom>:<id>:<emoji>]` belgilarini oddiy emojiga almashtiradi (maxsus emoji rasmi chizilmaguncha ham o'qiladi).
pub fn plain_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find("[ce:") {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        match tail.find(']') {
            Some(j) => {
                let parts: Vec<&str> = tail[1..j].splitn(4, ':').collect();
                if parts.len() == 4 {
                    out.push_str(parts[3]);
                }
                rest = &tail[j + 1..];
            }
            None => {
                out.push_str(tail);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// Maxsus emoji belgisi (matnga qo'shiladi).
pub fn emoji_token(short: &str, d: &TgDoc) -> String {
    format!("[ce:{short}:{}:{}]", d.id, if d.emoji.is_empty() { "🙂" } else { &d.emoji })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gif_havolasi_aylanadi() {
        let d = TgDoc { id: 0x1234, hash: -5, dc: 4, fref: "abcd".into(), ..Default::default() };
        let r = gif_ref(&d);
        let b = resolve_gif(&r).unwrap();
        assert_eq!((b.id, b.hash, b.dc, b.fref.as_str()), (0x1234, -5, 4, "abcd"));
    }

    #[test]
    fn maxsus_emoji_matnga_aylanadi() {
        assert_eq!(plain_text("a [ce:pack_1:123:😀] b"), "a 😀 b");
        assert_eq!(plain_text("[ce:oddiy"), "[ce:oddiy");
        assert_eq!(plain_text("salom"), "salom");
    }
}
