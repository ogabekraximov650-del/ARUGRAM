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
/// Video kesh serveri porti (`/thumb`, `/v` yo'llari shu yerda; Telegram serveri porti BOSHQA). `<= 0` — ishga tushmadi.
pub fn cache_port() -> i32 {
    static PORT: std::sync::OnceLock<i32> = std::sync::OnceLock::new();
    *PORT.get_or_init(|| {
        let c = CString::new(uzcoders::android::files_dir()).unwrap_or_default();
        unsafe { rust_video_cache_start(c.as_ptr()) }
    })
}

pub fn ensure_cache() -> bool {
    cache_port() > 0
}

pub fn stats(urls: &[String]) -> HashMap<String, VStat> {
    let mut out = HashMap::new();
    for u in urls {
        let name = file_name_of(u);
        if name.is_empty() {
            continue;
        }
        let (total, have, downloading, retrying, speed) = crate::vdsrv::dl_stats(&name);
        out.insert(u.clone(), VStat { total: total as i64, downloaded: have as i64, downloading, queued: false, retrying, speed });
    }
    out
}

/// Yuklashni boshlaydi (worker `vd` tizimi: 18 MiB oynalar, bo'laklar telefonga shifrlab yoziladi). Fonda ishlaydi, darhol qaytadi.
pub fn start(session: &str, url: &str) -> Result<(), String> {
    let name = file_name_of(url);
    if name.is_empty() {
        return Err("Video manzili noto'g'ri".into());
    }
    crate::vdsrv::start(session);
    crate::vdsrv::dl_start(session, &name);
    Ok(())
}

pub fn pause(url: &str) {
    crate::vdsrv::dl_pause(&file_name_of(url));
}

pub fn delete(url: &str) {
    crate::vdsrv::dl_delete(&file_name_of(url));
}

pub fn wipe_all() {
    crate::vdsrv::dl_wipe();
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
        let Some(v) = crate::secure::read_text(&e.path().to_string_lossy(), "eps").and_then(|t| serde_json::from_str::<Value>(&t).ok()) else { continue };
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
    if drafts.is_empty() {
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

/// Ijro manbasi: fayl telefonda to'liq bo'lsa — kesh (internet kerak emas); aks holda Telegram (fayl bot chatiga nusxalanadi).
/// Qaytadi: (manba, bot chatini ushlab turish kerakmi).
pub fn resolve(session: &str, url: &str) -> Result<(crate::player::Source, bool), String> {
    use crate::player::Source;
    let name = file_name_of(url);
    if name.is_empty() {
        return if url.starts_with("http") { Ok((Source::Http(url.to_string()), false)) } else { Err("Video manzili noto'g'ri".into()) };
    }
    // Internetdan: worker `vd` videosi mahalliy ochuvchi server (`vdsrv`) orqali shifrdan ochilib HTTP manba sifatida beriladi.
    // Pleyer mahalliy serversiz (Kotlin kabi): `VdDataSource` baytlarni JNI orqali oladi. Sessiya `vdsrv` ga beriladi (hajm/kalit so'rovi uchun).
    crate::vdsrv::start(session);
    Ok((Source::Vd(name), false))
}

/// Ushlab turish bo'shatiladi (pleyer yopildi).
pub fn release_hold() {
    let _ = HOLDS.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |v| Some(v.saturating_sub(1)));
}

/// Qayerda to'xtagani (fayl nomi bo'yicha): `files/progress.json`.
pub fn progress_get(url: &str) -> Option<(i64, i64)> {
    let name = file_name_of(url);
    let s = crate::secure::read_text(&format!("{}/progress.json", uzcoders::android::files_dir()), "progress")?;
    let v: Value = serde_json::from_str(&s).ok()?;
    let e = v.get(&name)?.as_str()?;
    let (a, b) = e.split_once('/')?;
    Some((a.parse().ok()?, b.parse().ok()?))
}

pub fn progress_set(url: &str, pos_ms: i64, dur_ms: i64) {
    let name = file_name_of(url);
    if name.is_empty() || dur_ms <= 0 {
        return;
    }
    let path = format!("{}/progress.json", uzcoders::android::files_dir());
    let mut v: Value = crate::secure::read_text(&path, "progress").and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Object(Default::default()));
    if let Value::Object(m) = &mut v {
        m.insert(name, Value::String(format!("{pos_ms}/{dur_ms}")));
    }
    let _ = crate::secure::write_text(&path, "progress", &v.to_string());
}

/// Tomosha tarixi uchun to'xtagan joydagi kadr (Kotlin `ThumbStore`): yadroning `/thumb` yo'li bitta kadrlik MP4 beradi,
/// Java `MediaMetadataRetriever` undan JPEG chiqaradi. Natija diskda shifrlangan; bir vaqtda bitta kadr, pleyer ochiq paytda kutadi.
pub fn history_frame(session: &str, video_url: &str, thumb_url: &str, pos_ms: i64, key: &str) -> Option<uzcoders::prelude::Image> {
    history_frame_r(session, video_url, thumb_url, pos_ms, key).ok()
}

/// `history_frame` — xato sababi bilan (ekranda ko'rsatiladi va qayta urinish shunga qarab).
pub fn history_frame_r(session: &str, video_url: &str, thumb_url: &str, pos_ms: i64, key: &str) -> Result<uzcoders::prelude::Image, String> {
    static QUEUE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let dir = format!("{}/cache/hframes", uzcoders::android::files_dir());
    let _ = std::fs::create_dir_all(&dir);
    let path = format!("{dir}/{key}.jpg");
    if let Some(img) = crate::secure::read(&path, "hframe").and_then(|b| crate::webp::decode_any(&b)) {
        return Ok(img.resized(960, 540));
    }
    let _g = QUEUE.lock().unwrap_or_else(|e| e.into_inner());
    // Pleyer ochiq — tarmoq faqat videoniki.
    for _ in 0..60 {
        if crate::gate::BUSY.load(Ordering::SeqCst) == 0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(400));
    }
    let src = if thumb_url.is_empty() { video_url } else { thumb_url };
    let name = file_name_of(src);
    if name.is_empty() {
        return Err("fayl nomi yo'q".into());
    }
    if !ensure_cache() {
        return Err("video kesh ishga tushmadi".into());
    }
    // Manba: worker `vd` videosi, mahalliy ochuvchi server (`vdsrv`) orqali shifrdan ochilib HTTP Range bilan beriladi.
    let _ = session;
    let u = crate::vdsrv::url(&name).ok_or_else(|| "video ochuvchi server ishga tushmagan".to_string())?;
    // `/thumb` — video KESH serverida (Kotlin `cache.ensureStarted()`), manba (`u`) esa Telegram serveri manzili.
    let port = cache_port();
    if port <= 0 {
        return Err("video kesh porti yo'q".into());
    }
    let url = format!("http://127.0.0.1:{port}/thumb?u={}&ms={}", crate::backend::urlenc(&u), pos_ms.max(0));
    let r = ureq::get(&url).timeout(std::time::Duration::from_secs(60)).call().map_err(|e| format!("kadr so'rovi: {e}"))?;
    let mut mp4 = Vec::new();
    std::io::Read::read_to_end(&mut std::io::Read::take(r.into_reader(), 16 << 20), &mut mp4).map_err(|e| format!("kadr o'qilmadi: {e}"))?;
    if mp4.len() < 64 {
        return Err("kadr bo'sh keldi".into());
    }
    let tmp_in = format!("{dir}/{key}.mp4");
    let tmp_out = format!("{dir}/{key}.tmp.jpg");
    std::fs::write(&tmp_in, &mp4).map_err(|e| format!("yozilmadi: {e}"))?;
    uzcoders::android::frame_to_file(&format!("{tmp_in}|{tmp_out}"));
    let _ = std::fs::remove_file(&tmp_in);
    let jpg = std::fs::read(&tmp_out).ok();
    let _ = std::fs::remove_file(&tmp_out);
    let jpg = jpg.ok_or_else(|| "Java kadr chiqarmadi".to_string())?;
    // Shu qismning eski kadrlari kerak emas.
    let prefix = key.rsplit_once('_').map(|(a, _)| format!("{a}_")).unwrap_or_default();
    if !prefix.is_empty() {
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().into_owned();
                if n.starts_with(&prefix) && n.ends_with(".jpg") && n != format!("{key}.jpg") {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
    }
    let img = crate::webp::decode_any(&jpg).ok_or_else(|| "kadr dekodlanmadi".to_string())?;
    let _ = crate::secure::write(&path, "hframe", &jpg);
    Ok(img.resized(960, 540))
}
