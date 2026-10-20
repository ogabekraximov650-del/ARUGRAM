//! Telegram orqali kirish backend'i: haqiqiy (grammers yadrosi) va sinov (tarmoqsiz).

use serde_json::Value;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::Mutex;

/// Kirish bosqichi natijasi (Rust yadrosi `ok/password/hint/error/sent/wait`).
#[derive(Default, Debug, Clone)]
pub struct Step {
    pub done: bool,
    pub need_password: bool,
    pub hint: String,
    pub error: Option<String>,
    /// Kod qayerga yuborilgani (odam tushunadigan matn).
    pub sent: Option<String>,
    pub logged_in: bool,
    pub wait: u32,
}

impl Step {
    pub fn err(e: impl Into<String>) -> Step {
        Step { error: Some(e.into()), ..Step::default() }
    }

    fn from_json(j: &Value) -> Step {
        let sent = j.get("sent").filter(|s| !s.is_null());
        Step {
            done: j["ok"].as_bool().unwrap_or(false),
            need_password: j["password"].as_bool().unwrap_or(false),
            hint: j["hint"].as_str().unwrap_or("").to_string(),
            error: j["error"].as_str().filter(|e| !e.is_empty()).map(String::from),
            sent: sent.map(|s| {
                match s["type"].as_str().unwrap_or("") {
                    "app" => "Kod Telegram ilovasiga yuborildi",
                    "sms" | "sms_word" | "sms_phrase" => "Kod SMS orqali yuborildi",
                    "call" | "flash_call" | "missed_call" => "Kod qo'ng'iroq orqali beriladi",
                    "email" => "Kod elektron pochtaga yuborildi",
                    _ => "Kod yuborildi",
                }
                .to_string()
            }),
            logged_in: j["ok"].as_bool().unwrap_or(false) && sent.is_none(),
            wait: j["wait"].as_u64().unwrap_or(0) as u32,
        }
    }
}

/// Bloklovchi chaqiruvlar: fon oqimida ishlatiladi (`uzcoders::spawn_bg`).
pub trait Backend: Send + Sync {
    fn request_code(&self, phone: &str) -> Step;
    fn sign_in(&self, code: &str) -> Step;
    fn check_password(&self, password: &str) -> Step;
    fn reset(&self);
    /// IP manzil bo'yicha davlat ISO kodi (`UZ`).
    fn nearest_country(&self) -> Option<String>;
}

/// Sinov rejimi: tarmoqsiz. Kod `12345` bo'lsa ikki bosqichli parol so'raladi.
pub struct StubBackend;

impl Backend for StubBackend {
    fn request_code(&self, _phone: &str) -> Step {
        Step { done: true, sent: Some("Kod yuborildi (sinov rejimi: 12345)".into()), ..Step::default() }
    }
    fn sign_in(&self, code: &str) -> Step {
        if code == "12345" {
            Step { need_password: true, hint: "sinov".into(), ..Step::default() }
        } else {
            Step::err("Kod noto'g'ri")
        }
    }
    fn check_password(&self, _password: &str) -> Step {
        Step::err("Sinov rejimi: Telegram yadrosi ulanmagan")
    }
    fn reset(&self) {}
    fn nearest_country(&self) -> Option<String> {
        None
    }
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
const API_BASE: &str = "https://arumediatv.uzcom.workers.dev";

/// Haqiqiy Telegram: ARUMEDIATV'ning Rust yadrosi (`rust_core`, grammers/MTProto).
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub struct TgBackend {
    dir: String,
    ready: Mutex<bool>,
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
impl TgBackend {
    pub fn new(files_dir: &str) -> TgBackend {
        TgBackend { dir: format!("{files_dir}/tg"), ready: Mutex::new(false) }
    }

    /// Yadroni ishga tushiradi va serverdan `api_id/api_hash` ni oladi (bir marta).
    fn ensure(&self) -> Result<(), String> {
        let mut ready = self.ready.lock().unwrap();
        if *ready {
            return Ok(());
        }
        let _ = std::fs::create_dir_all(&self.dir);
        let c = |s: &str| CString::new(s).unwrap_or_default();
        let (model, sys, app) = (c("Android"), c("Android"), c("ARUMEDIATV Rust 0.1"));
        rust_core::telegram::rust_tg_set_device(model.as_ptr(), sys.as_ptr(), app.as_ptr());
        let dir = c(&self.dir);
        let empty = c("");
        call(rust_core::telegram::rust_tg_init(dir.as_ptr(), 0, empty.as_ptr()));
        let cfg = http_get("/api/tg/config")?;
        if !cfg["enabled"].as_bool().unwrap_or(false) {
            return Err("Telegram orqali kirish hozircha yoqilmagan".into());
        }
        let bot = c(cfg["bot"].as_str().unwrap_or(""));
        rust_core::telegram::rust_tg_set_bot(bot.as_ptr());
        let id = cfg["api_id"].as_i64().unwrap_or(0) as i32;
        let hash = c(cfg["api_hash"].as_str().unwrap_or(""));
        let r = call(rust_core::telegram::rust_tg_init(dir.as_ptr(), id, hash.as_ptr()));
        if let Some(e) = r["error"].as_str() {
            return Err(e.to_string());
        }
        *ready = true;
        Ok(())
    }

    fn run(&self, f: impl FnOnce() -> *mut c_char) -> Step {
        match self.ensure() {
            Ok(()) => Step::from_json(&call(f())),
            Err(e) => Step::err(e),
        }
    }
}

/// Yadro qaytargan C satrini JSON ga o'giradi va xotirani bo'shatadi.
fn call(p: *mut c_char) -> Value {
    if p.is_null() {
        return Value::Null;
    }
    let s = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
    rust_core::ffi_utils::rust_free_string(p);
    serde_json::from_str(&s).unwrap_or(Value::Null)
}

/// Worker'ga imzolangan (`X-App-Sig`) GET so'rovi.
fn http_get(path: &str) -> Result<Value, String> {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        .to_string();
    let (t, m, p) = (CString::new(ts).unwrap_or_default(), CString::new("GET").unwrap_or_default(), CString::new(path).unwrap_or_default());
    let sig_ptr = unsafe { rust_core::app_sign(t.as_ptr(), m.as_ptr(), p.as_ptr()) };
    let sig = if sig_ptr.is_null() {
        String::new()
    } else {
        let s = unsafe { CStr::from_ptr(sig_ptr) }.to_string_lossy().into_owned();
        rust_core::ffi_utils::rust_free_string(sig_ptr);
        s
    };
    let mut req = ureq::get(&format!("{API_BASE}{path}"))
        .timeout(std::time::Duration::from_secs(10))
        .set("X-App-Version", "0.1.0");
    if !sig.is_empty() {
        req = req.set("X-App-Sig", &sig);
    }
    let resp = req.call().map_err(|e| format!("Serverga ulanib bo'lmadi: {e}"))?;
    let body = resp.into_string().map_err(|e| format!("Server javobi o'qilmadi: {e}"))?;
    serde_json::from_str::<Value>(&body).map_err(|e| format!("Server javobi noto'g'ri: {e}"))
}

impl Backend for TgBackend {
    fn request_code(&self, phone: &str) -> Step {
        let p = CString::new(phone).unwrap_or_default();
        self.run(|| rust_core::telegram::rust_tg_request_code(p.as_ptr()))
    }
    fn sign_in(&self, code: &str) -> Step {
        let c = CString::new(code).unwrap_or_default();
        self.run(|| rust_core::telegram::rust_tg_sign_in(c.as_ptr()))
    }
    fn check_password(&self, password: &str) -> Step {
        let c = CString::new(password).unwrap_or_default();
        self.run(|| rust_core::telegram::rust_tg_check_password(c.as_ptr()))
    }
    fn reset(&self) {
        if self.ensure().is_ok() {
            rust_core::telegram::rust_tg_login_reset();
        }
    }
    fn nearest_country(&self) -> Option<String> {
        self.ensure().ok()?;
        let j = call(rust_core::telegram::rust_tg_nearest_country());
        j["country"].as_str().filter(|c| !c.is_empty()).map(|c| c.to_uppercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn javob_tahlili() {
        let s = Step::from_json(&serde_json::json!({"ok": true, "sent": {"type": "app", "length": 5}}));
        assert!(s.done && !s.logged_in);
        assert_eq!(s.sent.as_deref(), Some("Kod Telegram ilovasiga yuborildi"));
        let s = Step::from_json(&serde_json::json!({"ok": true}));
        assert!(s.logged_in);
        let s = Step::from_json(&serde_json::json!({"password": true, "hint": "h"}));
        assert!(s.need_password && s.hint == "h");
        let s = Step::from_json(&serde_json::json!({"error": "xato", "wait": 30}));
        assert_eq!((s.error.as_deref(), s.wait), (Some("xato"), 30));
    }
}
