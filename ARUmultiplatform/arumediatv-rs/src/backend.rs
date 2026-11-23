//! Kirish backend'i: Telegram bot orqali (worker bilan), grammers/MTProto YO'Q.
//! UZ: ilova Telegram'ga to'g'ridan-to'g'ri ulanmaydi. Kirish: worker bir martalik token beradi, foydalanuvchi Telegram'da botda «Start» bosadi,
//! ilova holatni so'rab turadi (Kotlin `BotLoginScreen` bilan bir xil oqim). EN: no MTProto; the Telegram bot flow runs through the worker.

use serde_json::Value;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

/// Worker bergan kirish so'rovi: token, Telegram havolasi (`deep_link`) va amal qilish muddati (soniya).
#[derive(Clone, Debug, Default)]
pub struct BotLink {
    pub token: String,
    pub link: String,
    pub expires: u64,
}

/// Kirish holati so'rovi natijasi.
#[derive(Clone, Debug, PartialEq)]
pub enum Poll {
    Pending,
    Ok(String),
    Expired,
    Banned(String),
    Error(String),
}

/// Bloklovchi chaqiruvlar: fon oqimida ishlatiladi (`uzcoders::spawn_bg`).
pub trait Backend: Send + Sync {
    /// Bir martalik kirish tokenini oladi (`POST /api/auth/telegram/start`).
    fn start(&self) -> Result<BotLink, String>;
    /// Google orqali kirish: token va brauzerda ochiladigan Google havolasi (`POST /api/auth/google/start`); holat shu `poll` bilan so'raladi.
    fn google_start(&self) -> Result<BotLink, String> {
        Err("Google orqali kirish bu yerda mavjud emas".into())
    }
    /// Token holatini so'raydi (`GET /api/auth/telegram/status`).
    fn poll(&self, token: &str) -> Poll;
    /// Chiqish: mahalliy sessiya tozalanadi (Telegram hisobi yo'q).
    fn logout(&self) {}
}

fn device_json() -> Value {
    serde_json::json!({"device": "Android", "platform": "Android", "app_version": crate::api::APP_VERSION})
}

/// Haqiqiy backend: faqat worker.
pub struct WorkerBackend;

impl Backend for WorkerBackend {
    fn start(&self) -> Result<BotLink, String> {
        let (code, body) = crate::api::request("POST", "/api/auth/telegram/start", "", Some(&device_json()), None)?;
        let j: Value = serde_json::from_str(&body).map_err(|_| "Server javobi noto'g'ri".to_string())?;
        let token = j["token"].as_str().unwrap_or("").to_string();
        let link = j["deep_link"].as_str().unwrap_or("").to_string();
        if code >= 400 || token.is_empty() || link.is_empty() {
            return Err("Serverga ulanib bo'lmadi. Internetni tekshirib, qaytadan urinib ko'ring.".into());
        }
        Ok(BotLink { token, link, expires: j["expires_in"].as_u64().unwrap_or(300) })
    }

    fn google_start(&self) -> Result<BotLink, String> {
        let (code, body) = crate::api::request("POST", "/api/auth/google/start", "", Some(&device_json()), None)?;
        let j: Value = serde_json::from_str(&body).map_err(|_| "Server javobi noto'g'ri".to_string())?;
        let token = j["token"].as_str().unwrap_or("").to_string();
        let url = j["url"].as_str().unwrap_or("").to_string();
        if code >= 400 || token.is_empty() || url.is_empty() {
            return Err("Serverga ulanib bo'lmadi. Internetni tekshirib, qaytadan urinib ko'ring.".into());
        }
        Ok(BotLink { token, link: url, expires: j["expires_in"].as_u64().unwrap_or(300) })
    }

    fn poll(&self, token: &str) -> Poll {
        let Ok(j) = crate::api::get_json("/api/auth/telegram/status", &format!("?token={token}"), None) else { return Poll::Pending };
        match j["status"].as_str().unwrap_or("") {
            "ok" => {
                let s = j["session"].as_str().unwrap_or("");
                if s.is_empty() { Poll::Error("Sessiya olinmadi".into()) } else { Poll::Ok(s.to_string()) }
            }
            "expired" => Poll::Expired,
            "banned" => Poll::Banned(j["message"].as_str().filter(|m| !m.is_empty()).unwrap_or("Hisobingiz bloklangan.").to_string()),
            _ => Poll::Pending,
        }
    }
}

/// Sinov rejimi (kompyuterda, tarmoqsiz).
pub struct StubBackend;

impl Backend for StubBackend {
    fn start(&self) -> Result<BotLink, String> {
        Ok(BotLink { token: "test".into(), link: "tg://resolve?domain=test".into(), expires: 300 })
    }
    fn poll(&self, _token: &str) -> Poll {
        Poll::Pending
    }
}

/// Butun ilova uchun backend.
pub fn shared() -> std::sync::Arc<WorkerBackend> {
    static S: std::sync::OnceLock<std::sync::Arc<WorkerBackend>> = std::sync::OnceLock::new();
    S.get_or_init(|| std::sync::Arc::new(WorkerBackend)).clone()
}

/// UZ: Telegram yadrosi (MTProto) olib tashlangan — bu chaqiruv har doim xato qaytaradi, Telegram'ga bog'liq yo'llar worker yo'liga o'tadi.
/// EN: the MTProto core is gone; callers fall back to the worker path.
pub fn ensure_tg_ready() -> Result<(), String> {
    Err("Telegram yadrosi o'chirilgan: hamma narsa worker orqali".into())
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
extern "C" {
    // Yadro (`rust_core::crypto`) ning C funksiyalari: sessiya va kirish holati shu kalit bilan shifrlanib saqlanadi.
    fn rust_crypto_generate_key() -> *mut c_char;
    fn rust_crypto_set_key(hex: *const c_char) -> i32;
}

/// Shifrlash kalitini tayyorlaydi: `files/master.key` da saqlanadi (ilovaning yopiq papkasi), yo'q bo'lsa yaratiladi.
/// Kalitsiz yadro sessiyani va kirish holatini umuman saqlamaydi.
fn ensure_master_key(dir: &str) -> Result<(), String> {
    let path = format!("{dir}/master.key");
    let hex = match std::fs::read_to_string(&path) {
        Ok(h) if h.trim().len() >= 32 => h.trim().to_string(),
        _ => {
            let p = unsafe { rust_crypto_generate_key() };
            if p.is_null() {
                return Err("Shifrlash kaliti yaratilmadi".into());
            }
            let h = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
            rust_core::ffi_utils::rust_free_string(p);
            std::fs::write(&path, &h).map_err(|e| format!("Kalit saqlanmadi: {e}"))?;
            h
        }
    };
    let c = CString::new(hex).map_err(|_| "Kalit noto'g'ri".to_string())?;
    if unsafe { rust_crypto_set_key(c.as_ptr()) } != 1 {
        return Err("Shifrlash kaliti qabul qilinmadi".into());
    }
    Ok(())
}

/// Shifrlash kalitini (bir marta) o'rnatadi — diskni shifrlash uchun tarmoqsiz ham kerak.
pub fn ensure_master() -> bool {
    static OK: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *OK.get_or_init(|| match ensure_master_key(&uzcoders::android::files_dir()) {
        Ok(()) => true,
        Err(e) => {
            uzcoders::prelude::report_error("shifrlash kaliti", &e);
            false
        }
    })
}

pub fn urlenc(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{b:02X}"));
        }
    }
    o
}


/// Fayl yuklash (worker orqali): hozircha faqat kichik fayllar (<= 18 MiB) — worker o'zi shifrlab Telegram'ga saqlaydi (`PUT /api/vd/small`).
/// Katta fayllar keyingi bosqichda (bo'laklab). Bloklaydi — fon oqimida chaqiring.
pub fn upload_file_key(path: &str, name: &str, _mime: &str, session: &str, progress: &dyn Fn(u64, u64)) -> Result<String, String> {
    let len = std::fs::metadata(path).map_err(|e| format!("Fayl o'qilmadi: {e}"))?.len();
    if len == 0 {
        return Err("Fayl bo'sh".into());
    }
    if len > 18 * 1024 * 1024 {
        return Err("Fayl 18 MB dan katta — bu versiyada hali qo'llab-quvvatlanmaydi".into());
    }
    let bytes = std::fs::read(path).map_err(|e| format!("Fayl o'qilmadi: {e}"))?;
    progress(0, len);
    let (code, body) = crate::api::request_bytes("PUT", "/api/vd/small", &format!("?name={}", urlenc(name)), &bytes, Some(session))?;
    let j: Value = serde_json::from_str(&body).map_err(|_| "Server javobi noto'g'ri".to_string())?;
    if code == 200 && j["ok"].as_bool() == Some(true) {
        progress(len, len);
        return Ok(j["key"].as_str().unwrap_or("").to_string());
    }
    Err(j["error"].as_str().unwrap_or("Yuklab bo'lmadi").to_string())
}

pub fn upload_file(path: &str, name: &str, mime: &str, session: &str, progress: &dyn Fn(u64, u64)) -> Result<(), String> {
    upload_file_key(path, name, mime, session, progress).map(|_| ())
}

/// Admin kanal amallari (MTProto kerak edi) — worker orqali hali yo'q.
pub fn channel_info(_arg: &str) -> Value {
    serde_json::json!({"error": "Bu amal hali worker orqali ishlamaydi"})
}

pub fn make_bot_admin(_arg: &str) -> Value {
    serde_json::json!({"error": "Bu amal hali worker orqali ishlamaydi"})
}

/// Majburiy kanallarga qo'shilish Telegram hisobini talab qilardi — endi ilova qo'shilmaydi.
pub fn tg_authorized() -> bool {
    false
}

pub fn join_channel(_arg: &str) -> Value {
    serde_json::json!({"error": "Kanalga qo'shilish bu versiyada yo'q"})
}
