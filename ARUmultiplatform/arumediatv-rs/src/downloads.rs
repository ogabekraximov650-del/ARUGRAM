//! Qism yuklab olish: Rust yadrosining video-keshi (shifrlangan bo'laklar diskda). Holat yadrodan so'raladi.

use crate::data::{cache_dir, file_name_of, parse_episodes, tg_deliver, Season, HOLDS};
use serde_json::Value;
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::atomic::Ordering;

extern "C" {
    fn rust_video_cache_start(dir: *const c_char) -> i32;
    fn rust_video_cache_stats(urls_json: *const c_char) -> *mut c_char;
    fn rust_video_cache_download(url: *const c_char) -> i32;
    fn rust_video_cache_pause(url: *const c_char) -> i32;
    fn rust_video_cache_delete(url: *const c_char) -> i32;
    fn rust_video_cache_wipe() -> i32;
}

#[derive(Clone, Default, PartialEq)]
pub struct VStat {
    pub total: i64,
    pub downloaded: i64,
    pub downloading: bool,
    pub queued: bool,
    pub retrying: bool,
    pub speed: i64,
}

impl VStat {
    pub fn ratio(&self) -> f32 {
        if self.total > 0 { (self.downloaded as f32 / self.total as f32).clamp(0.0, 1.0) } else { 0.0 }
    }
    pub fn percent(&self) -> i64 {
        (self.ratio() * 100.0) as i64
    }
    pub fn complete(&self) -> bool {
        self.total > 0 && self.downloaded >= self.total
    }
    pub fn speed_label(&self) -> String {
        if self.speed <= 0 {
            String::new()
        } else if self.speed >= 1 << 20 {
            format!("{:.1} MB/s", self.speed as f64 / 1048576.0)
        } else {
            format!("{} KB/s", self.speed / 1024)
        }
    }
}

/// Yadro video-kesh serverini ishga tushiradi (bir marta). `true` — tayyor.
pub fn ensure_cache() -> bool {
    static PORT: std::sync::OnceLock<i32> = std::sync::OnceLock::new();
    *PORT.get_or_init(|| {
        let c = CString::new(uzcoders::android::files_dir()).unwrap_or_default();
        unsafe { rust_video_cache_start(c.as_ptr()) }
    }) > 0
}

pub fn stats(urls: &[String]) -> HashMap<String, VStat> {
    let mut out = HashMap::new();
    if urls.is_empty() || !ensure_cache() {
        return out;
    }
    let j = CString::new(serde_json::to_string(urls).unwrap_or_default()).unwrap_or_default();
    let p = unsafe { rust_video_cache_stats(j.as_ptr()) };
    if p.is_null() {
        return out;
    }
    let s = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
    rust_core::ffi_utils::rust_free_string(p);
    if let Ok(Value::Object(m)) = serde_json::from_str::<Value>(&s) {
        for (u, o) in m {
            out.insert(
                u,
                VStat {
                    total: o["total"].as_i64().unwrap_or(0),
                    downloaded: o["downloaded"].as_i64().unwrap_or(0),
                    downloading: o["downloading"].as_bool().unwrap_or(false),
                    queued: o["queued"].as_bool().unwrap_or(false),
                    retrying: o["retrying"].as_bool().unwrap_or(false),
                    speed: o["speed"].as_i64().unwrap_or(0),
                },
            );
        }
    }
    out
}

/// Yuklashni boshlaydi: fayl avval Telegram bot chatiga nusxalanadi, so'ng yadro bo'laklarni tortadi. Bloklaydi — fon oqimida.
pub fn start(session: &str, url: &str) -> Result<(), String> {
    if !ensure_cache() {
        return Err("Video kesh ishga tushmadi".into());
    }
    let name = file_name_of(url);
    if !name.is_empty() {
        HOLDS.fetch_add(1, Ordering::SeqCst);
        let r = tg_deliver(session, &[name.clone()]);
        // Nusxa bot chatida kamida 10 soniya turadi (Kotlin'dagi `delivering` kabi), keyin ushlab turish bo'shatiladi.
        std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_secs(10));
            HOLDS.fetch_sub(1, Ordering::SeqCst);
        });
        let ok = r?;
        if !ok.contains(&name) {
            return Err("Fayl Telegram'da topilmadi".into());
        }
    }
    let c = CString::new(url).unwrap_or_default();
    if unsafe { rust_video_cache_download(c.as_ptr()) } == 1 { Ok(()) } else { Err("Yuklash boshlanmadi".into()) }
}

pub fn pause(url: &str) {
    if let Ok(c) = CString::new(url) {
        unsafe { rust_video_cache_pause(c.as_ptr()) };
    }
}

pub fn delete(url: &str) {
    if let Ok(c) = CString::new(url) {
        unsafe { rust_video_cache_delete(c.as_ptr()) };
    }
}

pub fn wipe_all() {
    if ensure_cache() {
        unsafe { rust_video_cache_wipe() };
    }
}

/// Yuklanmalar ro'yxatidagi bitta (qism + sifat) qatori.
#[derive(Clone)]
pub struct DlItem {
    pub anime_id: i64,
    pub season_id: i64,
    pub section_no: i64,
    pub episode_id: i64,
    pub episode_no: i64,
    pub title: String,
    pub poster: String,
    pub quality: String,
    pub quality_title: String,
    pub url: String,
    pub stat: VStat,
}

/// Diskdagi qismlar ro'yxatlaridan (`eps_*.json` — ochib ko'rilgan bo'limlar) qaysilari yuklangani yoki yuklanayotganini topadi.
pub fn collect(seasons: &[Season]) -> Vec<DlItem> {
    let by: HashMap<(i64, i64), &Season> = seasons.iter().map(|s| ((s.anime_id, s.season_id), s)).collect();
    let mut drafts: Vec<DlItem> = Vec::new();
    let mut urls: Vec<String> = Vec::new();
    let Ok(rd) = std::fs::read_dir(cache_dir()) else { return Vec::new() };
    for e in rd.flatten() {
        let fname = e.file_name().to_string_lossy().into_owned();
        let Some(rest) = fname.strip_prefix("eps_").and_then(|r| r.strip_suffix(".json")) else { continue };
        let mut it = rest.split('_');
        let (Some(a), Some(s)) = (it.next().and_then(|v| v.parse::<i64>().ok()), it.next().and_then(|v| v.parse::<i64>().ok())) else { continue };
        let Some(v) = std::fs::read_to_string(e.path()).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) else { continue };
        let season = by.get(&(a, s));
        for ep in parse_episodes(&v) {
            for q in &ep.qualities {
                let u = ep.url(q);
                if u.is_empty() {
                    continue;
                }
                urls.push(u.clone());
                drafts.push(DlItem {
                    anime_id: a,
                    season_id: s,
                    section_no: season.map(|x| x.section_no).unwrap_or(0),
                    episode_id: ep.id,
                    episode_no: ep.number,
                    title: season.map(|x| x.name.clone()).unwrap_or_default(),
                    poster: season.map(|x| x.photo_url.clone()).unwrap_or_default(),
                    quality: q.clone(),
                    quality_title: ep.quality_title(q),
                    url: u,
                    stat: VStat::default(),
                });
            }
        }
    }
    if drafts.is_empty() || !ensure_cache() {
        return Vec::new();
    }
    let mut raw = stats(&urls);
    // Ilova endi ochilgan bo'lsa birinchi javob bo'sh bo'lishi mumkin (disk fon oqimida skanerlanadi) — bir marta kutamiz.
    if !raw.values().any(|s| s.downloaded > 0) {
        std::thread::sleep(std::time::Duration::from_millis(1200));
        raw = stats(&urls);
    }
    let mut out: Vec<DlItem> = drafts
        .into_iter()
        .filter_map(|mut d| {
            let st = raw.get(&d.url)?.clone();
            if st.downloaded > 0 || st.downloading {
                d.stat = st;
                Some(d)
            } else {
                None
            }
        })
        .collect();
    out.sort_by(|a, b| b.stat.downloading.cmp(&a.stat.downloading).then(a.title.cmp(&b.title)).then(a.episode_no.cmp(&b.episode_no)));
    out
}
