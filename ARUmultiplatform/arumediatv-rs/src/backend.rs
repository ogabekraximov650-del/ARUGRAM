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

/// Saqlangan kirish bosqichi (ilova yopilib qayta ochilganda tiklash uchun).
#[derive(Debug, Clone, Default)]
pub struct LoginInfo {
    /// `phone`, `code`, `password` yoki `done` (Telegram allaqachon ulangan).
    pub stage: String,
    pub phone: String,
    pub hint: String,
    pub sent: Option<String>,
}

/// Bloklovchi chaqiruvlar: fon oqimida ishlatiladi (`uzcoders::spawn_bg`).
pub trait Backend: Send + Sync {
    fn request_code(&self, phone: &str) -> Step;
    fn sign_in(&self, code: &str) -> Step;
    fn check_password(&self, password: &str) -> Step;
    fn reset(&self);
    /// IP manzil bo'yicha davlat ISO kodi (`UZ`).
    fn nearest_country(&self) -> Option<String>;
    /// Saqlangan kirish bosqichi (tarmoqsiz).
    fn login_state(&self) -> Option<LoginInfo>;
    /// Telegram ulangach ilova hisobiga kirish: botga `/start <token>` yuboriladi, server sessiya beradi.
    fn finish_login(&self) -> Result<String, String>;
    /// Telegram hisobidan chiqish (boshqa raqam bilan kirish uchun).
    fn logout(&self);
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
    fn login_state(&self) -> Option<LoginInfo> {
        None
    }
    fn finish_login(&self) -> Result<String, String> {
        Err("Sinov rejimi: server ulanmagan".into())
    }
    fn logout(&self) {}
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

/// Haqiqiy Telegram: ARUMEDIATV'ning Rust yadrosi (`rust_core`, grammers/MTProto).
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub struct TgBackend {
    dir: String,
    ready: Mutex<bool>,
    bot: Mutex<String>,
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
/// Butun ilova uchun bitta Telegram yadro holati (kirish va posterlar bir xil yadrodan foydalanadi).
#[cfg(target_os = "android")]
pub fn shared() -> std::sync::Arc<TgBackend> {
    static S: std::sync::OnceLock<std::sync::Arc<TgBackend>> = std::sync::OnceLock::new();
    S.get_or_init(|| std::sync::Arc::new(TgBackend::new(&uzcoders::android::files_dir()))).clone()
}

/// Yadroni to'liq ishga tushiradi (api_id/hash, bot nomi, mahalliy server porti). Android bo'lmasa — hech narsa.
pub fn ensure_tg_ready() -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        shared().ensure()
    }
    #[cfg(not(target_os = "android"))]
    {
        Ok(())
    }
}

impl TgBackend {
    pub fn new(files_dir: &str) -> TgBackend {
        TgBackend { dir: format!("{files_dir}/tg"), ready: Mutex::new(false), bot: Mutex::new(String::new()) }
    }

    /// Tarmoqsiz ishga tushirish: kalit va saqlangan sessiya/kirish holati uchun (api_id keyinroq).
    fn ensure_offline(&self) -> Result<(), String> {
        let _ = std::fs::create_dir_all(&self.dir);
        let root = self.dir.trim_end_matches("/tg");
        ensure_master_key(root)?;
        let dir = CString::new(self.dir.as_str()).unwrap_or_default();
        let empty = CString::new("").unwrap_or_default();
        call(rust_core::telegram::rust_tg_init(dir.as_ptr(), 0, empty.as_ptr()));
        Ok(())
    }

    /// Yadroni ishga tushiradi va serverdan `api_id/api_hash` ni oladi (bir marta).
    fn ensure(&self) -> Result<(), String> {
        let mut ready = self.ready.lock().unwrap();
        if *ready {
            return Ok(());
        }
        let _ = std::fs::create_dir_all(&self.dir);
        let root = self.dir.trim_end_matches("/tg");
        ensure_master_key(root)?;
        let c = |s: &str| CString::new(s).unwrap_or_default();
        let (model, sys, app) = (c("Android"), c("Android"), c("ARUMEDIATV Rust 0.1"));
        rust_core::telegram::rust_tg_set_device(model.as_ptr(), sys.as_ptr(), app.as_ptr());
        let dir = c(&self.dir);
        let empty = c("");
        call(rust_core::telegram::rust_tg_init(dir.as_ptr(), 0, empty.as_ptr()));
        // Sozlama diskdan darhol olinadi (tarmoqni kutmaymiz), fonda yangilanadi.
        let cfg_path = format!("{}/tg_config.json", self.dir.trim_end_matches("/tg"));
        let cached: Option<Value> = std::fs::read_to_string(&cfg_path).ok().and_then(|s| serde_json::from_str(&s).ok()).filter(|c: &Value| c["enabled"].as_bool().unwrap_or(false));
        let cfg = match cached {
            Some(c) => {
                let p = cfg_path.clone();
                std::thread::spawn(move || {
                    if let Ok(c) = crate::api::get_json("/api/tg/config", "", None) {
                        let _ = std::fs::write(p, c.to_string());
                    }
                });
                c
            }
            None => {
                let c = crate::api::get_json("/api/tg/config", "", None)?;
                let _ = std::fs::write(&cfg_path, c.to_string());
                c
            }
        };
        if !cfg["enabled"].as_bool().unwrap_or(false) {
            return Err("Telegram orqali kirish hozircha yoqilmagan".into());
        }
        *self.bot.lock().unwrap() = cfg["bot"].as_str().unwrap_or("").to_string();
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
    fn finish_login(&self) -> Result<String, String> {
        self.ensure()?;
        let dev = serde_json::json!({"device": "Android", "platform": "Android", "app_version": crate::api::APP_VERSION});
        let (code, body) = crate::api::request("POST", "/api/auth/telegram/start", "", Some(&dev), None)?;
        let j: Value = serde_json::from_str(&body).map_err(|_| "Server javobi noto'g'ri".to_string())?;
        let token = j["token"].as_str().unwrap_or("").to_string();
        if code >= 400 || token.is_empty() {
            return Err("Server bilan bog'lanib bo'lmadi".into());
        }
        let bot = CString::new(self.bot.lock().unwrap().as_str()).unwrap_or_default();
        let tok = CString::new(token.as_str()).unwrap_or_default();
        let r = call(rust_core::telegram::rust_tg_start_bot(bot.as_ptr(), tok.as_ptr()));
        if !r["ok"].as_bool().unwrap_or(false) {
            return Err(r["error"].as_str().unwrap_or("Botga yuborilmadi").to_string());
        }
        for _ in 0..40 {
            std::thread::sleep(std::time::Duration::from_millis(750));
            let Ok(j) = crate::api::get_json("/api/auth/telegram/status", &format!("?token={token}"), None) else { continue };
            match j["status"].as_str().unwrap_or("") {
                "ok" => {
                    let s = j["session"].as_str().unwrap_or("");
                    return if s.is_empty() { Err("Sessiya olinmadi".into()) } else { Ok(s.to_string()) };
                }
                "expired" => return Err("Kirish muddati tugadi — qayta urinib ko'ring".into()),
                "banned" => return Err(j["message"].as_str().filter(|m| !m.is_empty()).unwrap_or("Hisobingiz bloklangan.").to_string()),
                _ => {}
            }
        }
        Err("Kirish tasdiqlanmadi — qayta urinib ko'ring".into())
    }

    fn login_state(&self) -> Option<LoginInfo> {
        self.ensure_offline().ok()?;
        let j = call(rust_core::telegram::rust_tg_login_state());
        let st = Step::from_json(&serde_json::json!({"sent": j["sent"]}));
        Some(LoginInfo {
            stage: j["stage"].as_str()?.to_string(),
            phone: j["phone"].as_str().unwrap_or("").to_string(),
            hint: j["hint"].as_str().unwrap_or("").to_string(),
            sent: st.sent,
        })
    }
    fn logout(&self) {
        if self.ensure_offline().is_ok() {
            call(rust_core::telegram::rust_tg_logout());
        }
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


/// Faylni Telegram'ga (bot chatiga) yuboradi; bot kanalga ko'chirgach (`/api/tg/claim`) tayyor bo'ladi. Baytlar worker'dan O'TMAYDI.
/// Bloklaydi — fon oqimida chaqiring. `progress(sent, total)` ixtiyoriy.
#[cfg(target_os = "android")]
pub fn upload_file(path: &str, name: &str, mime: &str, session: &str, progress: &dyn Fn(u64, u64)) -> Result<(), String> {
    shared().ensure()?;
    let (p, n, m) = (CString::new(path).unwrap_or_default(), CString::new(name).unwrap_or_default(), CString::new(mime).unwrap_or_default());
    let started = call(rust_core::telegram::rust_tg_upload_start(p.as_ptr(), n.as_ptr(), m.as_ptr(), 0));
    let job = started["job"].as_u64().unwrap_or(0);
    if job == 0 {
        return Err(started["error"].as_str().unwrap_or("Yuklash boshlanmadi").to_string());
    }
    let mut key = String::new();
    let mut all_sent_at: Option<std::time::Instant> = None;
    loop {
        std::thread::sleep(std::time::Duration::from_millis(400));
        let st = call(rust_core::telegram::rust_tg_upload_status(job));
        let (sent, total) = (st["sent"].as_u64().unwrap_or(0), st["total"].as_u64().unwrap_or(0));
        if total > 0 {
            progress(sent, total);
        }
        if total > 0 && sent >= total && !st["done"].as_bool().unwrap_or(false) {
            let t0 = *all_sent_at.get_or_insert_with(std::time::Instant::now);
            if t0.elapsed().as_secs() > 120 {
                rust_core::telegram::rust_tg_upload_cancel(job);
                return Err("Telegram javob bermadi — internetni tekshirib, qayta urinib ko'ring".into());
            }
        }
        if st["done"].as_bool().unwrap_or(false) {
            if let Some(e) = st["error"].as_str().filter(|e| !e.is_empty()) {
                return Err(e.to_string());
            }
            key = st["key"].as_str().unwrap_or("").to_string();
            break;
        }
    }
    let q = format!("?file={}&key={}", urlenc(name), urlenc(&key));
    for wait in [1u64, 1, 2, 2, 3, 4, 5, 8] {
        std::thread::sleep(std::time::Duration::from_secs(wait));
        if let Ok(j) = crate::api::get_json("/api/tg/claim", &q, Some(session)) {
            if j["ready"].as_bool().unwrap_or(false) {
                break;
            }
        }
    }
    Ok(())
}

#[cfg(not(target_os = "android"))]
pub fn upload_file(_path: &str, _name: &str, _mime: &str, _session: &str, _progress: &dyn Fn(u64, u64)) -> Result<(), String> {
    Err("Yuklash faqat telefonda ishlaydi".into())
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
