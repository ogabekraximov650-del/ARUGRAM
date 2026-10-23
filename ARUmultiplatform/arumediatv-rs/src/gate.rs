//! Bepul bo'limlar uchun majburiy kanallar (Kotlin `ChannelGate`): foydalanuvchi bir marta ruxsat bersa, ilova Telegram hisobi bilan
//! kanallarga BIR-BIRLAB (5-10 soniya oraliq) qo'shiladi. Pleyer ochiq paytda tarmoq faqat videoniki; Telegram limiti (FLOOD) bo'lsa
//! kamida 30 daqiqa kutiladi. Ruxsat hisobda (`chan_consent`) saqlanadi.

use serde_json::Value;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Debug)]
pub struct GateCh {
    pub kind: String,
    pub title: String,
    pub url: String,
}

impl GateCh {
    fn done_key(&self) -> String {
        format!("{}|{}", self.kind, self.url)
    }
}

/// Pleyer ochiq turgan ekranlar soni: shu paytda kanallarga qo'shilish to'xtaydi.
pub static BUSY: AtomicUsize = AtomicUsize::new(0);
static CONSENT: AtomicBool = AtomicBool::new(false);
static STARTED: AtomicBool = AtomicBool::new(false);

fn channels_cell() -> &'static Mutex<Option<Vec<GateCh>>> {
    static C: OnceLock<Mutex<Option<Vec<GateCh>>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(None))
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn dir() -> String {
    uzcoders::android::files_dir()
}

fn done_file(uid: i64) -> String {
    format!("{}/gate_done_{uid}.json", dir())
}

fn pause_file(uid: i64) -> String {
    format!("{}/gate_pause_{uid}.txt", dir())
}

fn read_done(uid: i64) -> HashSet<String> {
    crate::secure::read_text(&done_file(uid), "gate").and_then(|t| serde_json::from_str::<Vec<String>>(&t).ok()).unwrap_or_default().into_iter().collect()
}

fn write_done(uid: i64, s: &HashSet<String>) {
    let v: Vec<&String> = s.iter().collect();
    let _ = crate::secure::write_text(&done_file(uid), "gate", &serde_json::to_string(&v).unwrap_or_default());
}

fn pause_until(uid: i64) -> u64 {
    crate::secure::read_text(&pause_file(uid), "gate").and_then(|t| t.trim().parse().ok()).unwrap_or(0)
}

fn parse(j: &Value) -> Vec<GateCh> {
    j["items"]
        .as_array()
        .map(|a| a.iter().map(|o| GateCh { kind: o["kind"].as_str().unwrap_or("").into(), title: o["title"].as_str().unwrap_or("").into(), url: o["url"].as_str().unwrap_or("").into() }).collect())
        .unwrap_or_default()
}

/// Kanallar ro'yxati (`None` — hali noma'lum).
pub fn channels() -> Option<Vec<GateCh>> {
    channels_cell().lock().ok().and_then(|c| c.clone())
}

/// Ro'yxatni serverdan oladi (bloklaydi).
pub fn fetch(session: &str) {
    if let Ok(j) = crate::api::get_json("/api/channels", "", Some(session)) {
        if let Ok(mut c) = channels_cell().lock() {
            *c = Some(parse(&j));
        }
    }
}

/// Ruxsat holati o'zgardi (profil yuklanganda, sozlamada, "Ruxsat berish" bosilganda).
pub fn set_consent(on: bool) {
    CONSENT.store(on, Ordering::SeqCst);
}

/// Fon oqimini bir marta ishga tushiradi.
pub fn start(session: String, uid: i64) {
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        let mut skip: HashSet<String> = HashSet::new();
        let mut list_at = 0u64;
        std::thread::sleep(std::time::Duration::from_secs(5));
        loop {
            let next = tick(&session, uid, &mut skip, &mut list_at);
            std::thread::sleep(std::time::Duration::from_millis(next));
        }
    });
}

const EVERY: u64 = 5 * 60_000;

fn tick(session: &str, uid: i64, skip: &mut HashSet<String>, list_at: &mut u64) -> u64 {
    if !CONSENT.load(Ordering::SeqCst) {
        return 30_000;
    }
    if BUSY.load(Ordering::SeqCst) > 0 {
        return 10_000;
    }
    let now = now_ms();
    let pu = pause_until(uid);
    if now < pu {
        return pu - now;
    }
    if !crate::backend::tg_authorized() {
        return EVERY;
    }
    if channels().is_none() || now.saturating_sub(*list_at) > 15 * 60_000 {
        fetch(session);
        *list_at = now;
    }
    if BUSY.load(Ordering::SeqCst) > 0 {
        return 10_000;
    }
    let mut done = read_done(uid);
    let todo: Vec<GateCh> = channels().unwrap_or_default().into_iter().filter(|c| !c.url.is_empty() && !done.contains(&c.done_key()) && !skip.contains(&c.done_key())).collect();
    let Some(c) = todo.first() else {
        skip.clear();
        return EVERY;
    };
    let j = crate::backend::join_channel(&format!("{}:{}", c.kind, c.url));
    let err = j["error"].as_str().unwrap_or("").to_string();
    if j["ok"].as_bool().unwrap_or(false) {
        done.insert(c.done_key());
        write_done(uid, &done);
    } else if err.contains("FLOOD") || err.contains("CHANNELS_TOO_MUCH") || j.get("wait").is_some() {
        let mut wait = 30 * 60_000u64;
        let secs = err.split("value:").nth(1).and_then(|t| t.trim().split(|ch: char| !ch.is_ascii_digit()).next().and_then(|n| n.parse::<u64>().ok())).or_else(|| err.split("WAIT_").nth(1).and_then(|t| t.split(|ch: char| !ch.is_ascii_digit()).next().and_then(|n| n.parse::<u64>().ok()))).or_else(|| j["wait"].as_u64());
        if let Some(s) = secs {
            wait = wait.max(s * 1000);
        }
        let _ = crate::secure::write_text(&pause_file(uid), "gate", &(now_ms() + wait).to_string());
        return wait;
    } else if err.contains("INVITE_HASH") || err.contains("USERNAME_") || err.contains("noto'g'ri") || err.contains("topilmadi") {
        // Havola yaroqsiz / kanal yo'q — admin tuzatmaguncha urinilmaydi.
        done.insert(c.done_key());
        write_done(uid, &done);
    } else {
        skip.insert(c.done_key());
    }
    5_000 + now_ms() % 5_000
}
