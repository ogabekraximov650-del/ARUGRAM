//! UZ: Telegram hisobiga kirish (MTProto, `grammers`): kod so'rash, kod bilan kirish, 2 bosqichli parol. `api_id` / `api_hash` qurish vaqtida Secrets'dan beriladi. Barcha chaqiruvlar BLOKLAYDI — UI'dan `spawn_bg` bilan chaqiring.
//! EN: Telegram account sign-in (MTProto, `grammers`): request code, sign in with the code, two-step password. `api_id` / `api_hash` come from Secrets at build time. All calls BLOCK — call them from the UI through `spawn_bg`.
//! RU: Вход в аккаунт Telegram (MTProto, `grammers`): запрос кода, вход по коду, пароль двух шагов. `api_id` / `api_hash` берутся из Secrets при сборке. Все вызовы БЛОКИРУЮТ — вызывайте из UI через `spawn_bg`.

use grammers_client::session::types::{DcOption, PeerId, PeerInfo, UpdateState, UpdatesState};
use grammers_client::session::{BoxFuture, Session, SessionData};
use grammers_client::{tl, Client, InvocationError, SenderPool};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use tokio::runtime::Runtime;

/// UZ: Qurish vaqtida beriladi (`ARUGRAM_API_ID`, `ARUGRAM_API_HASH`). EN: Provided at build time. RU: Задаётся при сборке.
const API_ID: &str = match option_env!("ARUGRAM_API_ID") {
    Some(v) => v,
    None => "",
};
const API_HASH: &str = match option_env!("ARUGRAM_API_HASH") {
    Some(v) => v,
    None => "",
};

/// UZ: `api_id` raqam sifatida (yo'q bo'lsa 0). EN: `api_id` as a number (0 if absent). RU: `api_id` числом (0, если нет).
pub fn api_id() -> i32 {
    API_ID.trim().parse().unwrap_or(0)
}

/// UZ: API ma'lumotlari qurish vaqtida berilganmi. EN: Whether the API credentials were provided at build time. RU: Заданы ли данные API при сборке.
pub fn api_ready() -> bool {
    api_id() > 0 && !API_HASH.trim().is_empty()
}

/// UZ: Kod qayerga yuborildi. EN: Where the code was sent. RU: Куда отправлен код.
#[derive(Clone, Debug, PartialEq)]
pub enum Via {
    App,
    Sms,
    Call,
    Email,
    Other,
}

/// UZ: Kod so'rovi natijasi. EN: Result of the code request. RU: Результат запроса кода.
#[derive(Clone, Debug, PartialEq)]
pub enum CodeSent {
    /// UZ: Kod yuborildi: qayerga va uzunligi. EN: Code sent: where to and its length. RU: Код отправлен: куда и длина.
    Code { via: Via, length: usize },
    /// UZ: Telegram kodsiz kiritdi. EN: Telegram let us in without a code. RU: Telegram впустил без кода.
    LoggedIn,
    /// UZ: Kodsiz, lekin parol kerak. EN: No code, but a password is needed. RU: Без кода, но нужен пароль.
    NeedPassword { hint: String },
}

/// UZ: Kod bilan kirish natijasi. EN: Result of signing in with the code. RU: Результат входа по коду.
#[derive(Clone, Debug, PartialEq)]
pub enum SignIn {
    Done,
    NeedPassword { hint: String },
}

// ── Sessiya (faylda) ─────────────────────────────────────────

#[derive(Debug)]
struct SessionError;

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "sessiya qulfi buzilgan")
    }
}

impl std::error::Error for SessionError {}

/// UZ: Faqat asosiy DC va avtorizatsiya kalitlari saqlanadi (`session.json`, ilovaning shaxsiy papkasida). EN: Only the home DC and auth keys are stored (`session.json`, in the app's private directory). RU: Хранятся только основной DC и ключи авторизации (`session.json` в личной папке приложения).
struct FileSession {
    path: PathBuf,
    data: Mutex<SessionData>,
}

impl FileSession {
    fn load(path: PathBuf) -> Self {
        let mut data = SessionData::default();
        if let Ok(raw) = std::fs::read(&path) {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&raw) {
                if let Some(dc) = v["home_dc"].as_i64() {
                    data.home_dc = dc as i32;
                }
                if let Ok(dcs) = serde_json::from_value::<Vec<DcOption>>(v["dcs"].clone()) {
                    for d in dcs {
                        data.dc_options.insert(d.id, d);
                    }
                }
            }
        }
        Self { path, data: Mutex::new(data) }
    }

    fn has_keys(&self) -> bool {
        self.data.lock().map(|d| !d.dc_options.is_empty()).unwrap_or(false)
    }

    fn save(&self) {
        let Ok(d) = self.data.lock() else { return };
        let dcs: Vec<&DcOption> = d.dc_options.values().collect();
        let body = serde_json::json!({"home_dc": d.home_dc, "dcs": dcs});
        drop(d);
        let tmp = self.path.with_extension("tmp");
        if std::fs::write(&tmp, body.to_string()).is_ok() {
            let _ = std::fs::rename(&tmp, &self.path);
        }
    }

    fn wipe(&self) {
        if let Ok(mut d) = self.data.lock() {
            *d = SessionData::default();
        }
        let _ = std::fs::remove_file(&self.path);
    }
}

impl Session for FileSession {
    type Error = SessionError;

    fn home_dc_id(&self) -> Result<i32, SessionError> {
        Ok(self.data.lock().map_err(|_| SessionError)?.home_dc)
    }

    fn set_home_dc_id(&self, dc_id: i32) -> BoxFuture<'_, Result<(), SessionError>> {
        Box::pin(async move {
            self.data.lock().map_err(|_| SessionError)?.home_dc = dc_id;
            self.save();
            Ok(())
        })
    }

    fn dc_option(&self, dc_id: i32) -> Result<Option<DcOption>, SessionError> {
        Ok(self.data.lock().map_err(|_| SessionError)?.dc_options.get(&dc_id).cloned())
    }

    fn set_dc_option(&self, dc_option: &DcOption) -> BoxFuture<'_, Result<(), SessionError>> {
        let dc_option = dc_option.clone();
        Box::pin(async move {
            self.data.lock().map_err(|_| SessionError)?.dc_options.insert(dc_option.id, dc_option);
            self.save();
            Ok(())
        })
    }

    fn peer(&self, peer: PeerId) -> BoxFuture<'_, Result<Option<PeerInfo>, SessionError>> {
        Box::pin(async move { Ok(self.data.lock().map_err(|_| SessionError)?.peer_infos.get(&peer).cloned()) })
    }

    fn cache_peer(&self, peer: &PeerInfo) -> BoxFuture<'_, Result<(), SessionError>> {
        let peer = peer.clone();
        Box::pin(async move {
            self.data.lock().map_err(|_| SessionError)?.peer_infos.insert(peer.id(), peer);
            Ok(())
        })
    }

    fn updates_state(&self) -> BoxFuture<'_, Result<UpdatesState, SessionError>> {
        Box::pin(async move { Ok(self.data.lock().map_err(|_| SessionError)?.updates_state.clone()) })
    }

    fn set_update_state(&self, _update: UpdateState) -> BoxFuture<'_, Result<(), SessionError>> {
        Box::pin(async move { Ok(()) })
    }
}

// ── Kirish bosqichi (diskda) ─────────────────────────────────

/// UZ: Kod kutilayotgan paytda ilova yopilsa ham (kodni olish uchun boshqa ilovaga o'tilganda) kirish davom etadi: bosqich, raqam va kod belgisi diskda saqlanadi.
/// EN: The sign-in survives the app being closed while waiting for the code (when switching to another app to read it): the stage, number and code hash are kept on disk.
/// RU: Вход продолжается, даже если приложение закрыли в ожидании кода (при переходе в другое приложение за кодом): этап, номер и хеш кода хранятся на диске.
const STAGE_TTL_MS: i64 = 25 * 60 * 1000;

#[derive(Clone, Debug, PartialEq)]
pub struct Stage {
    /// `code` yoki `password` / `code` or `password` / `code` или `password`.
    pub stage: String,
    pub phone: String,
    pub hash: String,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn stage_path() -> PathBuf {
    dir().join("login.json")
}

fn save_stage(stage: &str, phone: &str, hash: &str) {
    let body = serde_json::json!({"stage": stage, "phone": phone, "hash": hash, "at": now_ms()});
    let _ = std::fs::write(stage_path(), body.to_string());
}

fn clear_stage() {
    let _ = std::fs::remove_file(stage_path());
}

/// UZ: "Hisobga kirilgan" belgisi diskda (ARUMEDIATV Kotlin ilovasidagidek `authorized`): ilova ochilishi bilan tarmoqsiz o'qiladi, so'ng Telegram bilan tekshiriladi. Tarmoq xatosi kirishni bekor QILMAYDI.
/// EN: The "signed in" flag on disk (like `authorized` in the ARUMEDIATV Kotlin app): read at launch without the network, then verified with Telegram. A network error does NOT sign the user out.
/// RU: Отметка «выполнен вход» на диске (как `authorized` в Kotlin-приложении ARUMEDIATV): читается при запуске без сети, затем проверяется у Telegram. Ошибка сети НЕ разлогинивает.
fn auth_path() -> PathBuf {
    dir().join("authorized")
}

fn mark_authorized() {
    let _ = std::fs::write(auth_path(), b"1");
}

fn clear_authorized() {
    let _ = std::fs::remove_file(auth_path());
}

/// UZ: Oldin kirilganmi (tarmoqsiz, darhol). EN: Whether signed in earlier (no network, instant). RU: Выполнен ли вход ранее (без сети, мгновенно).
pub fn is_authorized() -> bool {
    api_ready() && auth_path().exists() && FileSession::load(dir().join("session.json")).has_keys()
}

/// UZ: Saqlangan bosqich (tarmoqsiz). Muddati o'tgan bo'lsa `None`. EN: The saved stage (no network); `None` once expired. RU: Сохранённый этап (без сети); `None`, если срок истёк.
pub fn stage() -> Option<Stage> {
    let raw = std::fs::read(stage_path()).ok()?;
    let v: serde_json::Value = serde_json::from_slice(&raw).ok()?;
    if now_ms() - v["at"].as_i64().unwrap_or(0) > STAGE_TTL_MS {
        clear_stage();
        return None;
    }
    Some(Stage {
        stage: v["stage"].as_str()?.to_string(),
        phone: v["phone"].as_str().unwrap_or("").to_string(),
        hash: v["hash"].as_str().unwrap_or("").to_string(),
    })
}

/// UZ: Boshidan boshlash (raqam o'zgartirilganda). EN: Start over (when the number is changed). RU: Начать заново (при смене номера).
pub fn reset() {
    clear_stage();
    if let Ok(mut g) = slot().lock() {
        if let Some(t) = g.as_mut() {
            t.hash.clear();
            t.password = None;
        }
    }
}

// ── Holat ────────────────────────────────────────────────────

struct Inner {
    rt: Runtime,
    client: Client,
    session: Arc<FileSession>,
    phone: String,
    hash: String,
    password: Option<tl::types::account::Password>,
}

static INNER: OnceLock<Mutex<Option<Inner>>> = OnceLock::new();

fn slot() -> &'static Mutex<Option<Inner>> {
    INNER.get_or_init(|| Mutex::new(None))
}

fn dir() -> PathBuf {
    PathBuf::from(uzcoders::android::files_dir())
}

fn params() -> grammers_mtsender::ConnectionParams {
    let mut p = grammers_mtsender::ConnectionParams::default();
    p.system_lang_code = "uz".to_string();
    p.lang_code = "uz".to_string();
    p
}

/// UZ: Ulanishni (bir marta) yaratadi va `f` ni bajaradi. EN: Creates the connection (once) and runs `f`. RU: Создаёт соединение (один раз) и выполняет `f`.
fn with<R>(f: impl FnOnce(&mut Inner) -> Result<R, String>) -> Result<R, String> {
    if !api_ready() {
        return Err("API ma'lumotlari ilovaga ulanmagan".to_string());
    }
    let mut g = slot().lock().map_err(|_| "ichki xato".to_string())?;
    if g.is_none() {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|e| format!("ishga tushmadi: {e}"))?;
        let session = Arc::new(FileSession::load(dir().join("session.json")));
        let pool = {
            let _g = rt.enter();
            SenderPool::with_configuration(Arc::clone(&session), api_id(), params())
        };
        let SenderPool { runner, handle, mut updates } = pool;
        let client = Client::new(handle);
        rt.spawn(runner.run());
        // UZ: Yangilanishlar kanali o'qilmasa xotirada o'sadi. EN: An unread updates channel grows in memory. RU: Непрочитанный канал обновлений растёт в памяти.
        rt.spawn(async move { while updates.recv().await.is_some() {} });
        let (phone, hash) = stage().map(|s| (s.phone, s.hash)).unwrap_or_default();
        *g = Some(Inner { rt, client, session, phone, hash, password: None });
    }
    let inner = g.as_mut().ok_or("ichki xato")?;
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(inner))) {
        Ok(r) => r,
        Err(_) => Err("ichki xato".to_string()),
    }
}

fn friendly(e: &InvocationError) -> String {
    if let InvocationError::Rpc(r) = e {
        if r.name.starts_with("FLOOD_WAIT") || r.name.starts_with("FLOOD_PREMIUM_WAIT") {
            return match r.value {
                Some(s) => format!("Juda ko'p urinish. {} kutib qayta urining", wait_text(s)),
                None => "Juda ko'p urinish — birozdan keyin qayta urining".to_string(),
            };
        }
        match r.name.as_str() {
            "PHONE_NUMBER_INVALID" => return "Telefon raqami noto'g'ri".to_string(),
            "PHONE_NUMBER_BANNED" => return "Bu raqam Telegram'da bloklangan".to_string(),
            "PHONE_NUMBER_FLOOD" => return "Bu raqamga juda ko'p kod so'raldi. Keyinroq urining".to_string(),
            "API_ID_INVALID" => return "API ID yoki API HASH noto'g'ri".to_string(),
            _ => {}
        }
    }
    match e {
        InvocationError::Io(_) | InvocationError::Dropped | InvocationError::Transport(_) => {
            "Internet bilan aloqa yo'q — qayta urining".to_string()
        }
        _ => e.to_string(),
    }
}

fn wait_text(s: u32) -> String {
    if s >= 3600 {
        format!("{} soat", s / 3600)
    } else if s >= 60 {
        format!("{} daqiqa", s / 60)
    } else {
        format!("{s} soniya")
    }
}

fn via_of(t: &tl::enums::auth::SentCodeType) -> (Via, usize) {
    use tl::enums::auth::SentCodeType as T;
    match t {
        T::App(x) => (Via::App, x.length as usize),
        T::Sms(x) => (Via::Sms, x.length as usize),
        T::FirebaseSms(x) => (Via::Sms, x.length as usize),
        T::Call(x) => (Via::Call, x.length as usize),
        T::EmailCode(x) => (Via::Email, x.length as usize),
        _ => (Via::Other, 5),
    }
}

fn settings() -> tl::enums::CodeSettings {
    tl::types::CodeSettings {
        allow_flashcall: false,
        current_number: false,
        allow_app_hash: false,
        allow_missed_call: false,
        allow_firebase: false,
        logout_tokens: None,
        token: None,
        app_sandbox: None,
        unknown_number: false,
    }
    .into()
}

async fn password_info(client: &Client) -> Result<tl::types::account::Password, String> {
    let tl::enums::account::Password::Password(pw) =
        client.invoke(&tl::functions::account::GetPassword {}).await.map_err(|e| friendly(&e))?;
    Ok(pw)
}

/// UZ: Telegram raqamga kirish kodini yuboradi. EN: Asks Telegram to send a sign-in code to the number. RU: Просит Telegram отправить код входа на номер.
pub fn request_code(phone: &str) -> Result<CodeSent, String> {
    let phone: String = phone.chars().filter(|c| c.is_ascii_digit() || *c == '+').collect();
    if phone.len() < 6 {
        return Err("Telefon raqamini kiriting".to_string());
    }
    with(|t| {
        let req = tl::functions::auth::SendCode {
            phone_number: phone.clone(),
            api_id: api_id(),
            api_hash: API_HASH.trim().to_string(),
            settings: settings(),
        };
        let res = t.rt.block_on(async {
            match t.client.invoke(&req).await {
                // UZ: Raqam boshqa DC'ga tegishli: o'sha DC asosiy bo'ladi. EN: The number belongs to another DC: it becomes the home DC. RU: Номер относится к другому DC: он становится основным.
                Err(InvocationError::Rpc(e)) if e.code == 303 => {
                    let dc = e.value.unwrap_or(2) as i32;
                    t.session.set_home_dc_id(dc).await.map_err(|e| e.to_string())?;
                    t.client.invoke(&req).await.map_err(|e| friendly(&e))
                }
                Ok(r) => Ok(r),
                Err(e) if e.is("SESSION_PASSWORD_NEEDED") => Err("__pw".to_string()),
                Err(e) => Err(friendly(&e)),
            }
        });
        let res = match res {
            Ok(r) => r,
            Err(e) if e == "__pw" => {
                let pw = t.rt.block_on(password_info(&t.client))?;
                let hint = pw.hint.clone().unwrap_or_default();
                save_stage("password", &phone, "");
                t.phone = phone;
                t.password = Some(pw);
                return Ok(CodeSent::NeedPassword { hint });
            }
            Err(e) => return Err(e),
        };
        match res {
            tl::enums::auth::SentCode::Code(c) => {
                if matches!(c.r#type, tl::enums::auth::SentCodeType::SetUpEmailRequired(_)) {
                    return Err("Telegram bu raqam uchun avval kirish emailini o'rnatishni so'rayapti".to_string());
                }
                let (via, length) = via_of(&c.r#type);
                save_stage("code", &phone, &c.phone_code_hash);
                t.phone = phone;
                t.hash = c.phone_code_hash;
                Ok(CodeSent::Code { via, length })
            }
            tl::enums::auth::SentCode::Success(x) => match x.authorization {
                tl::enums::auth::Authorization::Authorization(_) => {
                    t.session.save();
                    clear_stage();
                    mark_authorized();
                    Ok(CodeSent::LoggedIn)
                }
                tl::enums::auth::Authorization::SignUpRequired(_) => {
                    Err("Bu raqamda Telegram hisobi yo'q — avval Telegram ilovasida ro'yxatdan o'ting".to_string())
                }
            },
            tl::enums::auth::SentCode::PaymentRequired(_) => {
                Err("Telegram bu raqamga kodni faqat to'lov evaziga yuboradi".to_string())
            }
        }
    })
}

/// UZ: Kelgan kod bilan kiradi. EN: Signs in with the received code. RU: Входит по полученному коду.
pub fn sign_in(code: &str) -> Result<SignIn, String> {
    with(|t| {
        if t.hash.is_empty() {
            return Err("Kod muddati tugadi — raqamni qayta kiriting".to_string());
        }
        let r = t.rt.block_on(t.client.invoke(&tl::functions::auth::SignIn {
            phone_number: t.phone.clone(),
            phone_code_hash: t.hash.clone(),
            phone_code: Some(code.trim().to_string()),
            email_verification: None,
        }));
        match r {
            Ok(tl::enums::auth::Authorization::Authorization(_)) => {
                t.session.save();
                t.hash.clear();
                clear_stage();
                mark_authorized();
                Ok(SignIn::Done)
            }
            Ok(tl::enums::auth::Authorization::SignUpRequired(_)) => {
                Err("Bu raqamda Telegram hisobi yo'q — avval Telegram ilovasida ro'yxatdan o'ting".to_string())
            }
            Err(e) if e.is("SESSION_PASSWORD_NEEDED") => {
                let pw = t.rt.block_on(password_info(&t.client))?;
                let hint = pw.hint.clone().unwrap_or_default();
                save_stage("password", &t.phone, &t.hash);
                t.password = Some(pw);
                Ok(SignIn::NeedPassword { hint })
            }
            Err(e) if e.is("PHONE_CODE_EXPIRED") => {
                t.hash.clear();
                clear_stage();
                Err("Kod muddati tugadi — raqamni qayta kiriting".to_string())
            }
            Err(e) if e.is("PHONE_CODE_*") => Err("Kod noto'g'ri".to_string()),
            Err(e) => Err(friendly(&e)),
        }
    })
}

/// UZ: 2 bosqichli parolni (SRP) tekshiradi. `Ok(false)` — parol noto'g'ri. EN: Checks the two-step password (SRP). `Ok(false)` means a wrong password. RU: Проверяет пароль двух шагов (SRP). `Ok(false)` — пароль неверный.
pub fn check_password(password: &str) -> Result<bool, String> {
    with(|t| {
        use grammers_crypto::two_factor_auth::{calculate_2fa, check_p_and_g};
        use tl::enums::PasswordKdfAlgo;
        // UZ: SRP qiymatlari bir martalik: har urinishga yangi ma'lumot olinadi. EN: SRP values are single-use: fresh data per attempt. RU: Значения SRP одноразовые: на каждую попытку берутся новые данные.
        let pw = t.rt.block_on(password_info(&t.client))?;
        t.password = None;
        let alg = match &pw.current_algo {
            Some(PasswordKdfAlgo::Sha256Sha256Pbkdf2Hmacsha512iter100000Sha256ModPow(a)) => a.clone(),
            _ => return Err("Bu parol turini ilova qo'llamaydi".to_string()),
        };
        if !check_p_and_g(&alg.p, &alg.g) {
            return Err("Telegram noto'g'ri parol ma'lumoti berdi — qayta urining".to_string());
        }
        let srp_b = pw.srp_b.clone().ok_or("Parol ma'lumoti to'liq emas")?;
        let srp_id = pw.srp_id.ok_or("Parol ma'lumoti to'liq emas")?;
        let (m1, g_a) = calculate_2fa(&alg.salt1, &alg.salt2, &alg.p, &alg.g, srp_b, pw.secure_random.clone(), password);
        let req = tl::functions::auth::CheckPassword {
            password: tl::enums::InputCheckPasswordSrp::Srp(tl::types::InputCheckPasswordSrp {
                srp_id,
                a: g_a.to_vec(),
                m1: m1.to_vec(),
            }),
        };
        match t.rt.block_on(t.client.invoke(&req)) {
            Ok(tl::enums::auth::Authorization::Authorization(_)) => {
                t.session.save();
                clear_stage();
                mark_authorized();
                Ok(true)
            }
            Ok(tl::enums::auth::Authorization::SignUpRequired(_)) => Err("Bu raqamda Telegram hisobi yo'q".to_string()),
            Err(e) if e.is("PASSWORD_HASH_INVALID") => Ok(false),
            Err(e) => Err(friendly(&e)),
        }
    })
}

/// UZ: Sessiyani Telegram bilan tekshiradi (fonda, ilova ochilganda). Faqat Telegram sessiyani HAQIQATAN bekor qilgan bo'lsa `false`; tarmoq xatosi yoki kutish tugashi — `true` (kirish saqlanadi). EN: Verifies the session with Telegram (in the background at launch). `false` only if Telegram truly revoked it; a network error or timeout gives `true` (the sign-in is kept). RU: Проверяет сессию у Telegram (в фоне при запуске). `false` только если Telegram действительно отозвал её; ошибка сети или таймаут — `true` (вход сохраняется).
pub fn check_session() -> bool {
    if !is_authorized() {
        return false;
    }
    let r = with(|t| {
        let res = t.rt.block_on(async {
            tokio::time::timeout(
                std::time::Duration::from_secs(12),
                t.client.invoke(&tl::functions::updates::GetState {}),
            )
            .await
        });
        match res {
            Ok(Err(e)) if e.is("AUTH_KEY_UNREGISTERED") || e.is("SESSION_REVOKED") || e.is("USER_DEACTIVATED") || e.is("AUTH_KEY_INVALID") => {
                t.session.wipe();
                clear_authorized();
                Ok(false)
            }
            _ => Ok(true),
        }
    });
    r.unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_yoq_bolsa_xato() {
        if !api_ready() {
            assert!(request_code("+998901234567").is_err());
        }
    }

    #[test]
    fn kutish_matni() {
        assert_eq!(wait_text(30), "30 soniya");
        assert_eq!(wait_text(120), "2 daqiqa");
        assert_eq!(wait_text(7200), "2 soat");
    }
}
