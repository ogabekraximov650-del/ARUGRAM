//! Worker (server) bilan imzolangan HTTP so'rovlari va ilova sessiyasini saqlash.

use serde_json::Value;
use std::ffi::{CStr, CString};

pub const BASE: &str = "https://arumediatv.uzcom.workers.dev";
pub const APP_VERSION: &str = "0.1.0";

/// Ulanish 8 soniyada, butun so'rov `secs` soniyada tugashi kerak (IPv6 yoki sekin tarmoqda uzoq osilib qolmaslik uchun).
fn agent(secs: u64) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(8))
        .timeout(std::time::Duration::from_secs(secs))
        .build()
}

/// Yadro (`rust_core`) imzosi: `X-App-Sig`.
fn sign(method: &str, path: &str) -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        .to_string();
    let (t, m, p) = (
        CString::new(ts).unwrap_or_default(),
        CString::new(method).unwrap_or_default(),
        CString::new(path).unwrap_or_default(),
    );
    let ptr = unsafe { rust_core::app_sign(t.as_ptr(), m.as_ptr(), p.as_ptr()) };
    if ptr.is_null() {
        return String::new();
    }
    let s = unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned();
    rust_core::ffi_utils::rust_free_string(ptr);
    s
}

/// Imzolangan so'rov. `path` — so'rovsiz yo'l, `query` — `?a=b` (imzolanmaydi). Qaytadi: (HTTP kodi, javob matni).
pub fn request(method: &str, path: &str, query: &str, body: Option<&Value>, bearer: Option<&str>) -> Result<(u16, String), String> {
    // Tarmoq xatosida (masalan server sekin uyg'onayotganda) bir marta qayta uriniladi.
    let mut last = Err(String::new());
    for _ in 0..2 {
        last = request_once(method, path, query, body, bearer);
        if last.is_ok() {
            break;
        }
    }
    last
}

fn request_once(method: &str, path: &str, query: &str, body: Option<&Value>, bearer: Option<&str>) -> Result<(u16, String), String> {
    let url = format!("{BASE}{path}{query}");
    let mut req = agent(40).request(method, &url)
        .set("X-App-Version", APP_VERSION);
    let sig = sign(method, path);
    if !sig.is_empty() {
        req = req.set("X-App-Sig", &sig);
    }
    if let Some(b) = bearer {
        req = req.set("Authorization", &format!("Bearer {b}"));
    }
    let res = match body {
        Some(b) => req.set("Content-Type", "application/json").send_string(&b.to_string()),
        None => req.call(),
    };
    match res {
        Ok(r) => {
            let code = r.status();
            Ok((code, r.into_string().map_err(|e| e.to_string())?))
        }
        Err(ureq::Error::Status(code, r)) => Ok((code, r.into_string().unwrap_or_default())),
        Err(e) => Err(format!("Serverga ulanib bo'lmadi: {e}")),
    }
}

pub fn get_json(path: &str, query: &str, bearer: Option<&str>) -> Result<Value, String> {
    let (code, body) = request("GET", path, query, None, bearer)?;
    if code >= 400 {
        return Err(format!("Server xatosi ({code})"));
    }
    serde_json::from_str(&body).map_err(|e| format!("Server javobi noto'g'ri: {e}"))
}

// ── Ilova sessiyasi (token) ──

fn session_file() -> String {
    format!("{}/session.txt", uzcoders::android::files_dir())
}

pub fn load_session() -> Option<String> {
    std::fs::read_to_string(session_file()).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

pub fn save_session(token: &str) {
    let _ = std::fs::write(session_file(), token);
}

pub fn clear_session() {
    let _ = std::fs::remove_file(session_file());
}

/// ETag bilan GET: `Ok(None)` — o'zgarmagan (304). Qaytadi: (matn, yangi ETag).
pub fn get_etag(path: &str, etag: Option<&str>) -> Result<Option<(String, Option<String>)>, String> {
    let url = format!("{BASE}{path}");
    let mut req = agent(30).get(&url).set("X-App-Version", APP_VERSION);
    let sig = sign("GET", path);
    if !sig.is_empty() {
        req = req.set("X-App-Sig", &sig);
    }
    if let Some(e) = etag {
        req = req.set("If-None-Match", e);
    }
    match req.call() {
        Ok(r) if r.status() == 304 => Ok(None),
        Ok(r) => {
            let tag = r.header("ETag").map(String::from);
            Ok(Some((r.into_string().map_err(|e| e.to_string())?, tag)))
        }
        Err(ureq::Error::Status(304, _)) => Ok(None),
        Err(ureq::Error::Status(c, _)) => Err(format!("Server xatosi ({c})")),
        Err(e) => Err(format!("Serverga ulanib bo'lmadi: {e}")),
    }
}
