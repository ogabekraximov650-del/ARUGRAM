//! Yagona yozuv yo'li: baho, sevimli va sozlamalar avval diskdagi navbatga tushadi, serverga bitta paket bo'lib (`POST /api/sync`) ketadi.

use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// Navbat qatori: (kalit, tur, ma'lumot). Bir kalit bir yozuv — oxirgisi saqlanadi.
static ROWS: Mutex<Vec<(String, String, Value)>> = Mutex::new(Vec::new());
static LOADED: AtomicBool = AtomicBool::new(false);
static SCHEDULED: AtomicBool = AtomicBool::new(false);
const FLUSH_DELAY_SECS: u64 = 20;

fn file() -> String {
    format!("{}/sync_queue.json", uzcoders::android::files_dir())
}

fn load(rows: &mut Vec<(String, String, Value)>) {
    if LOADED.swap(true, Ordering::SeqCst) {
        return;
    }
    if let Ok(s) = std::fs::read_to_string(file()) {
        if let Ok(Value::Array(a)) = serde_json::from_str::<Value>(&s) {
            for r in a {
                if let (Some(k), Some(t)) = (r["key"].as_str(), r["kind"].as_str()) {
                    rows.push((k.to_string(), t.to_string(), r["data"].clone()));
                }
            }
        }
    }
}

fn save(rows: &[(String, String, Value)]) {
    let v: Vec<Value> = rows.iter().map(|(k, t, d)| json!({"key": k, "kind": t, "data": d})).collect();
    let _ = std::fs::write(file(), Value::Array(v).to_string());
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn put(kind: &str, key: String, data: Value) {
    {
        let mut rows = ROWS.lock().unwrap_or_else(|e| e.into_inner());
        load(&mut rows);
        rows.retain(|r| r.0 != key);
        rows.push((key, kind.to_string(), data));
        save(&rows);
    }
    schedule();
}

pub fn put_favorite(anime: i64, season: i64, on: bool) {
    if anime > 0 {
        put("favorite", format!("f:{anime}:{season}"), json!({"anime_id": anime, "season_id": season, "on": on, "updated_at": now()}));
    }
}

pub fn put_rating(anime: i64, season: i64, stars: i64) {
    if anime > 0 && (1..=10).contains(&stars) {
        put("rating", format!("r:{anime}:{season}"), json!({"anime_id": anime, "season_id": season, "stars": stars, "updated_at": now()}));
    }
}

pub fn put_settings(hidden: &[String], consent: bool) {
    put("settings", "settings".into(), json!({"hidden_stats": hidden, "chan_consent": consent}));
}

/// Bir nechta o'zgarish bitta paketga to'planishi uchun biroz kutiladi.
fn schedule() {
    if SCHEDULED.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(FLUSH_DELAY_SECS));
        SCHEDULED.store(false, Ordering::SeqCst);
        flush();
    });
}

/// Navbatni serverga yuboradi (fon oqimida chaqiring).
pub fn flush() {
    let Some(session) = crate::api::load_session() else { return };
    let batch: Vec<(String, String, Value)> = {
        let mut rows = ROWS.lock().unwrap_or_else(|e| e.into_inner());
        load(&mut rows);
        rows.clone()
    };
    if batch.is_empty() {
        return;
    }
    let pick = |kind: &str| Value::Array(batch.iter().filter(|r| r.1 == kind).map(|r| r.2.clone()).collect());
    let mut body = json!({
        "batch_id": format!("{}-{:x}", now(), now() & 0xffff_ffff),
        "history": [],
        "ratings": pick("rating"),
        "favorites": pick("favorite"),
        "packs": [],
        "traffic_bytes": 0,
    });
    if let Some(s) = batch.iter().find(|r| r.1 == "settings") {
        body["settings"] = s.2.clone();
    }
    match crate::api::request("POST", "/api/sync", "", Some(&body), Some(&session)) {
        Ok((c, _)) if c < 500 && c != 401 => {
            // 2xx — yuborildi; 4xx — server rad etdi (abadiy qayta yubormaymiz).
            let mut rows = ROWS.lock().unwrap_or_else(|e| e.into_inner());
            rows.retain(|r| !batch.iter().any(|b| b.0 == r.0 && b.2 == r.2));
            save(&rows);
        }
        _ => schedule(),
    }
}
