//! Ma'lumotlar: bo'limlar (animelar) ro'yxati va Telegram orqali keladigan posterlar.

use serde_json::Value;
use std::ffi::{CStr, CString};
use uzcoders::prelude::{decode_image, Image};

#[derive(Clone, Debug, Default)]
pub struct Season {
    pub anime_id: i64,
    pub season_id: i64,
    pub name: String,
    pub section_no: i64,
    pub episodes: i64,
    pub photo_url: String,
    pub rating_avg: f64,
    pub rating_count: i64,
    pub views: i64,
    pub age: i64,
    pub genres: Vec<String>,
    pub year: String,
    pub status: String,
    pub kind: String,
}

fn num(v: &Value) -> i64 {
    v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)).or_else(|| v.as_str().and_then(|s| s.parse().ok())).unwrap_or(0)
}

pub fn parse_seasons(body: &str) -> Vec<Season> {
    let Ok(Value::Array(arr)) = serde_json::from_str::<Value>(body) else { return Vec::new() };
    arr.iter()
        .filter_map(|o| {
            let (anime_id, season_id) = (num(&o["anime_id"]), num(&o["season_id"]));
            if anime_id == 0 && season_id == 0 {
                return None;
            }
            let (rc, rs) = (num(&o["rating_count"]), num(&o["rating_sum"]));
            Some(Season {
                anime_id,
                season_id,
                name: o["nomi"].as_str().unwrap_or("").to_string(),
                section_no: num(&o["bolim_id"]),
                episodes: num(&o["epizod_count"]),
                photo_url: o["photo_url"].as_str().unwrap_or("").to_string(),
                rating_avg: if rc > 0 { rs as f64 / rc as f64 } else { 0.0 },
                rating_count: rc,
                views: num(&o["views_total"]),
                age: num(&o["yosh"]),
                genres: o["janri"].as_str().unwrap_or("").split(',').map(|g| g.trim().to_string()).filter(|g| !g.is_empty()).collect(),
                year: match &o["yili"] {
                    Value::String(y) => y.trim().to_string(),
                    Value::Number(n) => n.to_string(),
                    _ => String::new(),
                },
                status: o["holati"].as_str().unwrap_or("").trim().to_string(),
                kind: o["turi"].as_str().unwrap_or("").trim().to_uppercase(),
            })
        })
        .collect()
}

fn cache_file() -> String {
    format!("{}/seasons.json", uzcoders::android::files_dir())
}

/// Diskdagi oxirgi ro'yxat (tarmoqsiz, darhol).
pub fn cached_seasons() -> Vec<Season> {
    std::fs::read_to_string(cache_file()).map(|b| parse_seasons(&b)).unwrap_or_default()
}

/// Serverdan (ETag bilan): `Ok(None)` — o'zgarmagan, `Ok(Some)` — yangi ro'yxat (diskka yoziladi).
pub fn fetch_seasons() -> Result<Option<Vec<Season>>, String> {
    let etag_file = format!("{}.etag", cache_file());
    let etag = std::fs::read_to_string(&etag_file).ok().filter(|_| std::path::Path::new(&cache_file()).exists());
    match crate::api::get_etag("/api/seasons", etag.as_deref().map(str::trim))? {
        None => Ok(None),
        Some((body, tag)) => {
            let items = parse_seasons(&body);
            if !items.is_empty() {
                let _ = std::fs::write(cache_file(), &body);
                match tag {
                    Some(t) => {
                        let _ = std::fs::write(&etag_file, t);
                    }
                    None => {
                        let _ = std::fs::remove_file(&etag_file);
                    }
                }
            }
            Ok(Some(items))
        }
    }
}

/// `https://x/api/image/ep_1.jpg?t=1` -> `ep_1.jpg`; xavfli nom -> bo'sh.
pub fn file_name_of(url: &str) -> String {
    let last = url.split('?').next().unwrap_or("").rsplit('/').next().unwrap_or("");
    let ok = !last.is_empty() && last != "." && last != ".." && last.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c));
    if ok {
        last.to_string()
    } else {
        String::new()
    }
}

extern "C" {
    fn rust_tg_set_keys(json: *const std::os::raw::c_char);
}

fn call_json(p: *mut std::os::raw::c_char) -> Value {
    if p.is_null() {
        return Value::Null;
    }
    let s = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
    rust_core::ffi_utils::rust_free_string(p);
    serde_json::from_str(&s).unwrap_or(Value::Null)
}

fn poster_dir() -> String {
    let d = format!("{}/posters", uzcoders::android::files_dir());
    let _ = std::fs::create_dir_all(&d);
    d
}

fn finish_image(bytes: &[u8]) -> Option<Image> {
    // Karta uchun yetarli o'lcham; burchaklar yumaloq (karta radiusiga mos).
    decode_image(bytes).map(|i| i.resized(480, 720).round_corners(48.0))
}

/// Posterlarni oladi: avval diskdagi nusxa (tarmoqsiz), yo'qlari Telegram orqali — worker `deliver` bot chatiga nusxalaydi,
/// yadro mahalliy serverda beradi. Natija: (url, tayyor rasm); muvaffaqiyatsiz bo'lsa `None`.
pub fn load_posters(session: &str, urls: &[String]) -> (Vec<(String, Option<Image>)>, Option<String>) {
    let mut diag: Option<String> = None;
    let dir = poster_dir();
    let names: Vec<String> = urls.iter().map(|u| file_name_of(u)).collect();
    // Diskdagi nusxalar parallel dekodlanadi (har biri alohida oqimda).
    let decoded: Vec<Option<Image>> = std::thread::scope(|sc| {
        let hs: Vec<_> = names
            .iter()
            .map(|n| {
                let dir = &dir;
                sc.spawn(move || if n.is_empty() { None } else { std::fs::read(format!("{dir}/{n}")).ok().and_then(|b| finish_image(&b)) })
            })
            .collect();
        hs.into_iter().map(|h| h.join().ok().flatten()).collect()
    });
    let mut out: Vec<(String, Option<Image>)> = Vec::new();
    let mut missing: Vec<usize> = Vec::new();
    for (i, (u, img)) in urls.iter().zip(decoded).enumerate() {
        if img.is_none() && !names[i].is_empty() {
            missing.push(i);
        }
        out.push((u.clone(), img));
    }
    if missing.is_empty() {
        return (out, None);
    }
    // Telegram yadrosi tayyor bo'lishi kerak (api_id, mahalliy server porti).
    if let Err(e) = crate::backend::ensure_tg_ready() {
        return (out, Some(format!("yadro: {e}")));
    }
    let port = call_json(rust_core::telegram::rust_tg_status())["port"].as_i64().unwrap_or(0);
    if port <= 0 {
        return (out, Some("Telegram ulanmagan (port 0)".into()));
    }
    let files: Vec<&String> = missing.iter().map(|&i| &names[i]).collect();
    let mut delivered: Vec<String> = Vec::new();
    let dres = crate::api::request("POST", "/api/tg/deliver", "", Some(&serde_json::json!({ "files": files })), Some(session));
    match &dres {
        Ok((200, _)) => {}
        Ok((c, b)) => diag = Some(format!("deliver HTTP {c}: {}", b.chars().take(70).collect::<String>())),
        Err(e) => diag = Some(format!("deliver: {e}")),
    }
    if let Ok((200, body)) = dres {
        if let Ok(j) = serde_json::from_str::<Value>(&body) {
            if let Some(keys) = j["keys"].as_object().filter(|k| !k.is_empty()) {
                let c = CString::new(Value::Object(keys.clone()).to_string()).unwrap_or_default();
                unsafe { rust_tg_set_keys(c.as_ptr()) };
            }
            delivered = j["files"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
            if delivered.is_empty() {
                diag = Some(format!("deliver: fayl topilmadi (0/{})", missing.len()));
            }
        }
    }
    for &i in &missing {
        let n = &names[i];
        if !delivered.contains(n) {
            continue;
        }
        let got = ureq::get(&format!("http://127.0.0.1:{port}/tg/0/{n}")).timeout(std::time::Duration::from_secs(25)).call();
        match got {
            Ok(r) => {
                let mut buf = Vec::new();
                let _ = std::io::Read::read_to_end(&mut std::io::Read::take(r.into_reader(), 12 << 20), &mut buf);
                let _ = std::fs::write(format!("{dir}/{n}"), &buf);
                out[i].1 = finish_image(&buf);
                if out[i].1.is_none() {
                    diag = Some(format!("dekod xato: {} bayt, boshi {:02x?}", buf.len(), &buf[..buf.len().min(4)]));
                }
            }
            Err(e) => diag = Some(format!("mahalliy server: {e}")),
        }
    }
    // Yuborilgan nusxalar bot chatida qolib ketmasin.
    let _ = call_json(rust_core::telegram::rust_tg_clear_bot_chat());
    (out, diag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn royxat_tahlili() {
        let b = r#"[{"anime_id":"5","season_id":2,"nomi":"Bleach","bolim_id":1,"epizod_count":366,"photo_url":"https://x/api/image/p_1.jpg?t=3","rating_count":2,"rating_sum":20,"views_total":55,"yosh":15},{"foo":1}]"#;
        let s = parse_seasons(b);
        assert_eq!(s.len(), 1);
        assert_eq!((s[0].anime_id, s[0].episodes, s[0].age), (5, 366, 15));
        assert_eq!(s[0].rating_avg, 10.0);
        assert_eq!(file_name_of(&s[0].photo_url), "p_1.jpg");
        assert_eq!(file_name_of("https://x/a/../"), "");
    }
}

/// Tomosha tarixi qatori (`GET /api/history`).
#[derive(Clone)]
pub struct HistoryItem {
    pub anime_id: i64,
    pub season_id: i64,
    pub episode_no: i64,
    pub section_no: i64,
    pub title: String,
    pub photo_url: String,
    pub updated_at: i64,
}

/// Kutubxona: (sevimlilar, tarix). Tarix bo'lim bo'yicha guruhlanadi (har bir bo'lim — eng oxirgi ko'rilgani).
pub fn fetch_library(session: &str) -> Result<(Vec<Season>, Vec<HistoryItem>), String> {
    let fav = crate::api::get_json("/api/favorites", "", Some(session))?;
    let hist = crate::api::get_json("/api/history", "", Some(session))?;
    let favs = parse_seasons(&fav["items"].to_string());
    let mut items: Vec<HistoryItem> = hist["items"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|o| {
                    let pick = |a: &str, b: &str| {
                        let v = o[a].as_str().unwrap_or("");
                        if v.is_empty() { o[b].as_str().unwrap_or("") } else { v }.to_string()
                    };
                    HistoryItem {
                        anime_id: num(&o["anime_id"]),
                        season_id: num(&o["season_id"]),
                        episode_no: num(&o["epizod_number"]),
                        section_no: num(&o["bolim_id"]),
                        title: pick("season_name", "anime_name"),
                        photo_url: pick("season_photo", "anime_photo"),
                        updated_at: num(&o["updated_at"]),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    items.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    let mut seen = std::collections::HashSet::new();
    items.retain(|h| seen.insert((h.anime_id, h.season_id)));
    Ok((favs, items))
}

/// Millisekund (epoch) -> "HH:mm/dd/MM/yyyy" (Toshkent, UTC+5).
pub fn fmt_date(ms: i64) -> String {
    let t = ms / 1000 + 5 * 3600;
    let (days, sec) = (t.div_euclid(86400), t.rem_euclid(86400));
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{:02}:{:02}/{:02}/{:02}/{}", sec / 3600, sec % 3600 / 60, d, m, y)
}

#[cfg(test)]
mod date_tests {
    #[test]
    fn sana() {
        // 2026-10-20 12:00 UTC = 17:00 Toshkent
        assert_eq!(super::fmt_date(1_792_497_600_000), "17:00/20/10/2026");
    }
}

/// Hisob va shaxsiy statistika (`/api/auth/me` + `/api/me/stats`).
#[derive(Clone, Default)]
pub struct Profile {
    pub id: i64,
    pub name: String,
    pub username: String,
    pub photo_url: String,
    pub balance: i64,
    pub animes: i64,
    pub episodes: i64,
    pub watch_ms: i64,
    pub traffic: i64,
    pub seasons: i64,
    pub favorites: i64,
    pub rated: i64,
    pub comments: i64,
}

pub fn fetch_profile(session: &str) -> Result<Profile, String> {
    let me = crate::api::get_json("/api/auth/me", "", Some(session))?;
    let u = &me["user"];
    let st = crate::api::get_json("/api/me/stats", "", Some(session)).unwrap_or(Value::Null);
    let first = u["first_name"].as_str().unwrap_or("").trim();
    let last = u["last_name"].as_str().unwrap_or("").trim();
    let username = u["username"].as_str().unwrap_or("").to_string();
    let name = match format!("{first} {last}").trim() {
        n if !n.is_empty() => n.to_string(),
        _ if !username.is_empty() => username.clone(),
        _ => "Foydalanuvchi".to_string(),
    };
    Ok(Profile {
        id: num(&u["id"]),
        name,
        username,
        photo_url: u["photo_url"].as_str().unwrap_or("").to_string(),
        balance: num(&u["balance"]),
        animes: num(&st["animes"]),
        episodes: num(&st["episodes"]),
        watch_ms: num(&st["watch_ms"]),
        traffic: num(&st["traffic"]),
        seasons: num(&st["seasons"]),
        favorites: num(&st["favorites"]),
        rated: num(&st["rated"]),
        comments: num(&st["comments"]),
    })
}

/// Hisobni serverdan o'chiradi (`POST /api/auth/delete-account`).
pub fn delete_account(session: &str) -> Result<(), String> {
    let (code, _) = crate::api::request("POST", "/api/auth/delete-account", "", None, Some(session))?;
    if code >= 400 { Err(format!("O'chirib bo'lmadi ({code})")) } else { Ok(()) }
}

/// `1234567` -> `1.234.567`
pub fn format_count(n: i64) -> String {
    let d = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in d.chars().enumerate() {
        if i > 0 && (d.len() - i) % 3 == 0 {
            out.push('.');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// Millisekund -> `soat:daqiqa`.
pub fn format_hours(ms: i64) -> String {
    let t = ms / 1000;
    format!("{}:{:02}", format_count(t / 3600), t % 3600 / 60)
}

pub fn format_bytes(b: i64) -> String {
    let s = if b < 1024 {
        format!("{b} B")
    } else if b < 1024 * 1024 {
        format!("{} KB", b / 1024)
    } else if b < 1024 * 1024 * 1024 {
        format!("{:.1} MB", b as f64 / 1048576.0)
    } else {
        format!("{:.2} GB", b as f64 / 1073741824.0)
    };
    s.replace('.', ",")
}
