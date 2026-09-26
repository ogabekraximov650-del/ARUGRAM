// rust/src/telegram.rs — VIDEONI TELEGRAM SERVERIDAN OLISH
//
// ═══════════════════════════════════════════════════════════════
//  NEGA BU FAYL BOR
// ═══════════════════════════════════════════════════════════════
//
// TALAB (foydalanuvchi): xarajatni kamaytirish uchun videolar
// Telegram serveri orqali uzatilsin. Videolar yopiq kanalda
// (shifrlanmagan holda) turadi, bot ularni obunasi bor
// foydalanuvchining bot bilan shaxsiy chatiga `copyMessage` bilan
// yuboradi, ilova esa foydalanuvchining O'Z Telegram hisobi bilan
// faylni o'sha chatdan oladi. Foydalanuvchi kanalga a'zo EMAS.
//
// ── NEGA TDLib EMAS, `grammers` ─────────────────────────────────
//
// TDLib yuklangan faylni diskka OCHIQ holda yozadi. Bizning qoida
// esa qat'iy: diskda ochiq video bir soniya ham turmaydi.
// `grammers` baytlarni faqat XOTIRAGA qaytaradi, diskka o'zi hech
// narsa yozmaydi.
//
// ── QANDAY ULANGAN ─────────────────────────────────────────────
//
// Shu fayl telefonda KICHIK MAHALLIY HTTP MANBA ochadi:
//
//     http://127.0.0.1:<port>/tg/<xabar_id>/<fayl_nomi>
//
// U worker'ning `/api/play/` va `/api/image/` yo'llari kabi Range
// so'rovlarini qabul qiladi, lekin baytlarni Telegram'dan oladi.
// Shu sabab qolgan tizim o'zgarmaydi:
//
//   * ONLAYN KO'RISH — pleyer to'g'ridan-to'g'ri shu manzildan
//     o'qiydi. Baytlar faqat XOTIRADA o'tadi, diskka yozilmaydi.
//   * YUKLAB OLISH — `video_cache.rs` shu manzildan hozirgidek 1 MB
//     bo'laklab oladi va har bo'lakni AES-128-GCM bilan shifrlab
//     yozadi (`run_download` -> `route_url`).
//
// Fayl nomi (`<fayl_nomi>`) B2'dagi nom bilan BIR XIL, ya'ni kesh
// kaliti ham bir xil: Telegram'dan yuklangan qism B2'dan yuklangan
// qism o'rnini bosadi va aksincha.
//
// ── TELEGRAM ISHLAMASA ─────────────────────────────────────────
//
// Mahalliy manba xato qaytarsa, shu fayl uchun Telegram
// `FAIL_COOLDOWN` davomida chetga suriladi va `route_url` hech
// narsa qaytarmaydi — yuklash avtomatik B2 (worker) yo'liga qaytadi.
//
// ── SHIFRLAB YUKLASH (AES-128-CTR) ─────────────────────────────
//
// TALAB (foydalanuvchi): "fayllarni shifrlab yuklasin va shifrlangan
// fayldan faqat kerakli baytlarni olsin".
//
// Ilova Telegram'ga yuklaydigan HAR BIR fayl yuklanish paytida
// AES-128-CTR bilan shifrlanadi (`CtrReader`). Har faylga ALOHIDA
// tasodifiy 16 baytlik kalit; IV nol — kalit takrorlanmagani uchun
// bu xavfsiz. Kalit serverga yoziladi (`tg_files.file_key`, qismlar
// uchun `epizod_db.key_*`) va faylni ko'rishga RUXSATI bor odamga
// `/api/tg/deliver` javobida beriladi.
//
// CTR'da har 16 bayt o'z tartib raqami bilan shifrlanadi, ya'ni
// faylning istalgan joyini alohida ochish mumkin: `fetch_part`
// Telegram'dan kelgan qismni `offset` dan boshlab ochadi
// (`ctr_apply`). Shifrlangan fayl hajmi aslidan bir bayt ham farq
// qilmaydi.
//
// Kalit yo'q fayl (masalan admin Telegram ilovasidan o'zi qo'ygan
// post) — ochiq deb hisoblanadi va avvalgidek o'qiladi.
//
// Kalitlar telefonda ham shifrlangan faylda saqlanadi (`keys.bin`):
// ilova qayta ochilganda bot chatidagi nusxani qayta so'ramasdan
// o'qiy oladi.
//
// ── SIRLAR ─────────────────────────────────────────────────────
//
// `api_id`/`api_hash` worker'dan keladi va sessiya bilan birga
// shifrlangan faylda saqlanadi (`crypto::seal_blob`, AES-256-GCM).
// Shifrlash kaliti hali o'rnatilmagan bo'lsa (ilova ishga tushishi
// tugamagan) hech narsa diskka yozilmaydi — ochiq holda HECH QACHON.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::os::raw::c_char;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use grammers_client::session::types::{DcOption, PeerId, PeerInfo, UpdateState, UpdatesState};
use grammers_client::session::{BoxFuture, Session, SessionData};
use grammers_client::{tl, Client, InvocationError, SenderPool};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::runtime::Runtime;
use tokio::sync::Semaphore;

use crate::crypto;
use crate::ffi_utils::{cstr_to_str, string_to_cptr};

/// Bitta `upload.getFile` so'rovi hajmi.
///
/// Telegram qoidasi: `limit` 4 KiB ga karrali, 1 MiB ni qoldiqsiz
/// bo'lsin va so'rov 1 MiB chegarasidan o'tmasin. 512 KiB va unga
/// karrali `offset` bu shartlarning hammasini bajaradi.
const PART: u64 = 512 * 1024;

/// Bitta HTTP javobi uchun havoda turgan so'rovlar soni. Biri kelishi
/// bilan keyingisi yuboriladi, ya'ni tezlik bitta so'rovning kechikishiga
/// (RTT) bog'lanib qolmaydi.
const PIPELINE: usize = 4;

/// Butun ilova bo'yicha havodagi `getFile` so'rovlari chegarasi.
/// Yuklab olish 16 ta parallel HTTP so'rov yuboradi; chegara
/// bo'lmasa bu 64 ta so'rov bo'lardi va Telegram FLOOD_WAIT berardi.
const MAX_INFLIGHT: usize = 24;

/// Fayl qismlari uchun QO'SHIMCHA ulanishlar soni (asosiysidan tashqari).
///
/// TOPILGAN SABAB (foydalanuvchi: "videolar juda sekin yuklanyapti"):
/// `grammers` har bir DC ga BITTA TCP ulanish ochadi — 16 ta oqim
/// so'ragan hamma qismlar bitta ulanishdan navbat bilan o'tardi.
/// Bitta TCP ulanishning tezligi yo'l kechikishi bilan cheklanadi,
/// Telegram ham bitta ulanishni cheklaydi. Rasmiy ilovalar fayllar
/// uchun alohida bir nechta ulanish ochadi — endi biz ham: qismlar
/// asosiy + 4 ta qo'shimcha ulanishga navbat bilan taqsimlanadi.
/// Hammasi BITTA sessiya (bitta auth kalit) bilan — Telegram buni
/// ruxsat etadi, har bir ulanish o'z `session_id` siga ega.
const DL_CONNS: usize = 4;

/// Xatodan keyin shu fayl uchun Telegram qancha vaqt chetga suriladi.
const FAIL_COOLDOWN: Duration = Duration::from_secs(120);

/// Bitta qismni olishga urinishlar soni.
const PART_ATTEMPTS: usize = 3;

/// Shu xatolar sessiya butunlay o'lganini bildiradi.
const SESSION_DEAD: [&str; 4] = [
    "AUTH_KEY_UNREGISTERED",
    "SESSION_REVOKED",
    "USER_DEACTIVATED",
    "AUTH_KEY_DUPLICATED",
];

const LABEL_CONFIG: &str = "tg-config-v1";
const LABEL_SESSION: &str = "tg-session-v1";
const LABEL_ROUTES: &str = "tg-routes-v1";
const LABEL_LOGIN: &str = "tg-login-v1";
const LABEL_KEYS: &str = "tg-keys-v1";
const LABEL_TOKENS: &str = "tg-auth-tokens-v1";

/// TARMOQ xatosi belgisi: shunday xato bilan tugagan o'qishni
/// internet qaytgach QAYTA urinish mumkin (pleyer uni "kutish" deb
/// biladi, xato deb emas — `player_source.rs`).
pub(crate) const NET_ERR: &str = "tarmoq: ";

/// Xato tarmoq sababli (qayta urinsa bo'ladi)mi.
pub(crate) fn is_net_err(e: &str) -> bool {
    e.starts_with(NET_ERR)
}

/// Telegram xatosi vaqtinchalikmi: ulanish uzildi, server band yoki
/// "biroz kuting" (FLOOD_WAIT). Bunday xatodan keyin qayta urinish
/// to'g'ri; boshqalari (fayl yo'q va h.k.) — haqiqiy xato.
fn is_transient(e: &InvocationError) -> bool {
    match e {
        InvocationError::Rpc(r) => r.code >= 500 || r.name.starts_with("FLOOD_WAIT"),
        InvocationError::Io(_) | InvocationError::Dropped | InvocationError::Transport(_) => true,
        _ => false,
    }
}

fn inv_err(e: &InvocationError) -> String {
    if is_transient(e) {
        format!("{NET_ERR}{e}")
    } else {
        e.to_string()
    }
}

/// Bitta raqamga shundan tez-tez yangi kod so'ralmaydi.
const CODE_REQUEST_GAP_MS: i64 = 2 * 60 * 1000;

/// QR orqali kirish tasdiqlandi (`updateLoginToken` keldi).
static QR_ACCEPTED: AtomicBool = AtomicBool::new(false);

/// Kod bosqichi shuncha vaqt saqlanadi (Telegram kodi ham shuncha
/// yashaydi) — undan keyin ilova yana raqam so'raydi.
const LOGIN_STATE_TTL_MS: i64 = 30 * 60 * 1000;

// ═══════════════════════════════════════════════════════════════
//  HOLAT
// ═══════════════════════════════════════════════════════════════

/// Telegram'dagi faylning yuklash uchun kerakli ma'lumoti.
#[derive(Clone)]
struct DocInfo {
    id: i64,
    access_hash: i64,
    file_reference: Vec<u8>,
    dc_id: i32,
    size: u64,
    mime: String,
    /// `Some` — bu SURAT (oddiy ko'rinishda yuborilgan), qiymati eng
    /// katta o'lcham turi; `None` — video/hujjat.
    photo_size: Option<String>,
}

struct Tg {
    rt: Runtime,
    dir: PathBuf,
    port: u16,
    client: Mutex<Option<Client>>,
    session: Mutex<Option<Arc<SealedSession>>>,
    api_id: Mutex<i32>,
    api_hash: Mutex<String>,
    authorized: AtomicBool,
    /// 2 bosqichli parol ma'lumoti (`account.getPassword`).
    password_token: Mutex<Option<tl::types::account::Password>>,
    /// fayl nomi -> fayl ma'lumoti (xotirada; `file_reference` eskirsa
    /// qayta olinadi).
    docs: Mutex<HashMap<String, DocInfo>>,
    /// Kirish boti username'i (videolar shu bot chatidan olinadi).
    bot: Mutex<String>,
    /// Bot foydalanuvchisi: (id, access_hash) — `ResolveUsername` bir marta.
    bot_peer: Mutex<Option<(i64, i64)>>,
    /// kesh kaliti (fayl nomi) -> bot chatidagi xabar id.
    routes: Mutex<HashMap<String, i32>>,
    failed: Mutex<HashMap<String, Instant>>,
    /// Qaysi DC larga hisob (auth) ko'chirilgan.
    auth_dcs: tokio::sync::Mutex<HashSet<i32>>,
    inflight: Arc<Semaphore>,
    /// Fayl qismlari uchun qo'shimcha ulanishlar (`DL_CONNS`).
    dl: Mutex<Vec<Client>>,
    dl_next: std::sync::atomic::AtomicUsize,
    /// Asosiy ulanish orqali muvaffaqiyatli ishlagan DC lar — bu DC ning
    /// kaliti sessiyada tayyor, qo'shimcha ulanishlar shu kalit bilan
    /// ulanadi (aks holda har biri o'z kalitini yasab, ruxsatsiz qolardi).
    dl_dcs: Mutex<HashSet<i32>>,
    /// fayl nomi -> AES-128-CTR kaliti (`keys.bin` da shifrlangan).
    keys: Mutex<HashMap<String, [u8; 16]>>,
    /// Sessiya Telegram tomonidan bekor qilindi (ilova bot chatini
    /// qayta kirgach tozalashi kerak). `rust_tg_status` da `lost`.
    lost: AtomicBool,
}

static TG: OnceLock<Tg> = OnceLock::new();

/// Telegram'ga o'zini qanday tanitadi (`initConnection`).
///
/// TOPILGAN FARQ (Cherrygram bilan solishtirganda): `grammers`
/// standarti — "Android 32-bit", ilova versiyasi "0.10.0"
/// (kutubxonaning o'z versiyasi), til "en". Telegram'ning
/// firibgarlikka qarshi tizimi bunday "noma'lum" mijozga kirish
/// kodini ehtiyotkorroq yuboradi. Endi haqiqiy telefon modeli,
/// Android va ilova versiyasi hamda o'zbek tili beriladi
/// (`rust_tg_set_device`, Dart ishga tushishda chaqiradi).
static DEVICE: Mutex<Option<(String, String, String)>> = Mutex::new(None);

#[no_mangle]
pub extern "C" fn rust_tg_set_device(model_ptr: *const c_char, system_ptr: *const c_char, app_ptr: *const c_char) {
    let s = |p| unsafe { cstr_to_str(p) }.unwrap_or("").trim().to_string();
    let (model, system, app) = (s(model_ptr), s(system_ptr), s(app_ptr));
    if let Ok(mut d) = DEVICE.lock() {
        *d = Some((model, system, app));
    }
}

fn connection_params() -> grammers_mtsender::ConnectionParams {
    let mut p = grammers_mtsender::ConnectionParams::default();
    if let Some((model, system, app)) = DEVICE.lock().ok().and_then(|d| d.clone()) {
        if !model.is_empty() {
            p.device_model = model;
        }
        if !system.is_empty() {
            p.system_version = format!("Android {system}");
        }
        if !app.is_empty() {
            p.app_version = app;
        }
    }
    p.system_lang_code = "uz".to_string();
    p.lang_code = "uz".to_string();
    p
}

fn tg() -> Option<&'static Tg> {
    TG.get()
}

fn read_sealed(path: &PathBuf, label: &str) -> Option<Vec<u8>> {
    if !crypto::is_enabled() {
        return None;
    }
    let raw = fs::read(path).ok()?;
    crypto::open_blob(label, &raw)
}

/// Kichik faylni FAQAT shifrlangan holda yozadi. Kalit yo'q bo'lsa
/// hech narsa yozilmaydi (ochiq holda hech qachon).
fn write_sealed(path: &PathBuf, label: &str, plain: &[u8]) -> bool {
    if !crypto::is_enabled() {
        return false;
    }
    let Some(bytes) = crypto::seal_blob(label, plain) else {
        return false;
    };
    let tmp = path.with_extension("writing");
    if fs::write(&tmp, &bytes).is_err() {
        return false;
    }
    fs::rename(&tmp, path).is_ok()
}

// ═══════════════════════════════════════════════════════════════
//  SHIFRLANGAN SESSIYA
// ═══════════════════════════════════════════════════════════════
//
// `grammers` sessiyasi: qaysi DC asosiy va har bir DC uchun
// avtorizatsiya kaliti. Bu kalit HISOBNING O'ZI — kim uni olsa,
// foydalanuvchi nomidan Telegram'ga kira oladi. Shu sabab u faqat
// shifrlangan faylda turadi.
//
// Peer keshi (kontaktlar va h.k.) diskka yozilmaydi: bizga kerak
// emas (bot chatidagi xabarlar peer'siz olinadi) va har bir yangi
// peer diskka yozish degani bo'lardi.

#[derive(Debug)]
struct SessionError;

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "sessiya qulfi buzilgan")
    }
}

impl std::error::Error for SessionError {}

struct SealedSession {
    path: PathBuf,
    data: Mutex<SessionData>,
}

impl SealedSession {
    fn load(path: PathBuf) -> Self {
        let mut data = SessionData::default();
        if let Some(plain) = read_sealed(&path, LABEL_SESSION) {
            if let Ok(v) = serde_json::from_slice::<Value>(&plain) {
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

    fn save(&self) {
        let Ok(d) = self.data.lock() else { return };
        let dcs: Vec<&DcOption> = d.dc_options.values().collect();
        let body = json!({"home_dc": d.home_dc, "dcs": dcs});
        drop(d);
        let _ = write_sealed(&self.path, LABEL_SESSION, body.to_string().as_bytes());
    }

    fn wipe(&self) {
        if let Ok(mut d) = self.data.lock() {
            *d = SessionData::default();
        }
        let _ = fs::remove_file(&self.path);
    }
}

impl Session for SealedSession {
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
        Ok(self
            .data
            .lock()
            .map_err(|_| SessionError)?
            .dc_options
            .get(&dc_id)
            .cloned())
    }

    fn set_dc_option(&self, dc_option: &DcOption) -> BoxFuture<'_, Result<(), SessionError>> {
        let dc_option = dc_option.clone();
        Box::pin(async move {
            self.data
                .lock()
                .map_err(|_| SessionError)?
                .dc_options
                .insert(dc_option.id, dc_option);
            self.save();
            Ok(())
        })
    }

    fn peer(&self, peer: PeerId) -> BoxFuture<'_, Result<Option<PeerInfo>, SessionError>> {
        Box::pin(async move {
            Ok(self
                .data
                .lock()
                .map_err(|_| SessionError)?
                .peer_infos
                .get(&peer)
                .cloned())
        })
    }

    fn cache_peer(&self, peer: &PeerInfo) -> BoxFuture<'_, Result<(), SessionError>> {
        let peer = peer.clone();
        Box::pin(async move {
            self.data
                .lock()
                .map_err(|_| SessionError)?
                .peer_infos
                .insert(peer.id(), peer);
            Ok(())
        })
    }

    fn updates_state(&self) -> BoxFuture<'_, Result<UpdatesState, SessionError>> {
        Box::pin(async move { Ok(self.data.lock().map_err(|_| SessionError)?.updates_state.clone()) })
    }

    fn set_update_state(&self, _update: UpdateState) -> BoxFuture<'_, Result<(), SessionError>> {
        // Yangilanishlar (updates) bizga kerak emas — hech narsa
        // saqlanmaydi.
        Box::pin(async move { Ok(()) })
    }
}

// ═══════════════════════════════════════════════════════════════
//  ULANISH
// ═══════════════════════════════════════════════════════════════

fn connect(t: &Tg) -> Option<Client> {
    // Qulf butun yaratish davomida ushlanadi: ikki oqim bir vaqtda
    // kelsa ikkita ulanish (va ikkita sessiya yozuvchisi) ochilmasin.
    let mut slot = t.client.lock().ok()?;
    if let Some(c) = slot.as_ref() {
        return Some(c.clone());
    }
    let api_id = *t.api_id.lock().ok()?;
    if api_id <= 0 {
        return None;
    }
    let session = Arc::new(SealedSession::load(t.dir.join("session.bin")));
    let pool = {
        // `SenderPool::new` ichida tokio kontekstida ishlaydigan
        // narsalar yaratiladi.
        let _g = t.rt.enter();
        SenderPool::with_configuration(Arc::clone(&session), api_id, connection_params())
    };
    let SenderPool { runner, handle, mut updates } = pool;
    let client = Client::new(handle);
    t.rt.spawn(runner.run());
    // Yangilanishlar kanali o'qilmasa xotirada cheksiz o'sadi.
    // Faqat bittasi kerak: QR orqali kirish tasdiqlandi
    // (`updateLoginToken`).
    t.rt.spawn(async move {
        use grammers_session::updates::UpdatesLike;
        while let Some(u) = updates.recv().await {
            let accepted = match &u {
                UpdatesLike::Updates(tl::enums::Updates::UpdateShort(s)) => {
                    matches!(s.update, tl::enums::Update::LoginToken)
                }
                UpdatesLike::Updates(tl::enums::Updates::Updates(s)) => {
                    s.updates.iter().any(|x| matches!(x, tl::enums::Update::LoginToken))
                }
                UpdatesLike::Updates(tl::enums::Updates::Combined(s)) => {
                    s.updates.iter().any(|x| matches!(x, tl::enums::Update::LoginToken))
                }
                _ => false,
            };
            if accepted {
                QR_ACCEPTED.store(true, Ordering::SeqCst);
            }
        }
    });
    *slot = Some(client.clone());
    drop(slot);
    if let Ok(mut s) = t.session.lock() {
        *s = Some(session);
    }
    Some(client)
}

/// Hisobga bog'liq xotiradagi narsalarni tozalaydi (boshqa hisob
/// bilan kirilganda eskisi ishlatilib qolmasin).
fn forget_account_state(t: &Tg) {
    if let Ok(mut d) = t.docs.lock() {
        d.clear();
    }
    if let Ok(mut f) = t.failed.lock() {
        f.clear();
    }
    // Boshqa DC larga ko'chirilgan avtorizatsiya ESKI hisobniki.
    t.rt.block_on(async { t.auth_dcs.lock().await.clear() });
    forget_peers(t);
}

/// Hisobga bog'liq "manzillar" (tarmoqsiz, istalgan joydan chaqirsa
/// bo'ladi).
///
/// TOPILGAN XATO (foydalanuvchi: "boshqa account orqali kirib video
/// yukladim — PEER_ID_INVALID caused by messages.sendMedia"): botning
/// `access_hash` i HAR BIR Telegram hisobi uchun boshqacha, ilova esa
/// uni xotirada saqlab qolardi — yangi hisob eski hisobning qiymati
/// bilan botga yozardi. Qo'shimcha ulanishlar uchun "tayyor DC"
/// belgilari ham eski hisobniki (yangi hisobning kaliti boshqa).
fn forget_peers(t: &Tg) {
    if let Ok(mut p) = t.bot_peer.lock() {
        *p = None;
    }
    if let Ok(mut d) = t.dl_dcs.lock() {
        d.clear();
    }
}

fn disconnect(t: &Tg) {
    if let Ok(mut c) = t.client.lock() {
        if let Some(c) = c.take() {
            c.disconnect();
        }
    }
    if let Ok(mut v) = t.dl.lock() {
        for c in v.drain(..) {
            c.disconnect();
        }
    }
    if let Ok(mut s) = t.session.lock() {
        *s = None;
    }
}

/// Qism uchun ulanish: asosiysi yoki qo'shimchalardan biri (navbat
/// bilan). [dc] asosiy ulanish orqali hali ishlamagan bo'lsa — faqat
/// asosiysi (kalit shu yerda yasaladi va ruxsat ko'chiriladi).
fn part_client(t: &Tg, main: &Client, dc: i32) -> Client {
    if !t.dl_dcs.lock().map(|s| s.contains(&dc)).unwrap_or(false) {
        return main.clone();
    }
    let n = t.dl_next.fetch_add(1, Ordering::Relaxed) % (DL_CONNS + 1);
    if n == 0 {
        return main.clone();
    }
    let Some(session) = t.session.lock().ok().and_then(|s| s.clone()) else {
        return main.clone();
    };
    let Ok(mut v) = t.dl.lock() else { return main.clone() };
    while v.len() < n {
        let api_id = t.api_id.lock().map(|v| *v).unwrap_or(0);
        let pool = {
            let _g = t.rt.enter();
            SenderPool::with_configuration(Arc::clone(&session) as Arc<_>, api_id, connection_params())
        };
        // Yangilanishlar kerak emas — qabul qiluvchi tashlanadi
        // (`run_sender` yuborish xatosini e'tiborsiz qoldiradi).
        let SenderPool { runner, handle, updates } = pool;
        drop(updates);
        t.rt.spawn(runner.run());
        v.push(Client::new(handle));
    }
    v[n - 1].clone()
}

/// [dc] asosiy ulanish orqali ishladi — qo'shimchalar ham ishlatsa bo'ladi.
fn mark_dc_ready(t: &Tg, dc: i32) {
    if let Ok(mut s) = t.dl_dcs.lock() {
        s.insert(dc);
    }
}

fn save_config(t: &Tg) {
    let api_id = t.api_id.lock().map(|v| *v).unwrap_or(0);
    let api_hash = t.api_hash.lock().map(|v| v.clone()).unwrap_or_default();
    let body = json!({
        "api_id": api_id,
        "api_hash": api_hash,
        "authorized": t.authorized.load(Ordering::SeqCst),
    });
    let _ = write_sealed(&t.dir.join("config.bin"), LABEL_CONFIG, body.to_string().as_bytes());
}

fn save_routes(t: &Tg) {
    let Ok(r) = t.routes.lock() else { return };
    let body = serde_json::to_vec(&*r).unwrap_or_default();
    drop(r);
    let _ = write_sealed(&t.dir.join("routes.bin"), LABEL_ROUTES, &body);
}

// ═══════════════════════════════════════════════════════════════
//  AES-128-CTR (fayl boshidagi "SHIFRLAB YUKLASH" izohiga qarang)
// ═══════════════════════════════════════════════════════════════

type Aes128Ctr = ctr::Ctr128BE<aes::Aes128>;

/// 32 ta hex belgi -> 16 bayt.
fn parse_key(hex_key: &str) -> Option<[u8; 16]> {
    let v = hex::decode(hex_key.trim()).ok()?;
    v.try_into().ok()
}

/// `buf` — faylning `offset` dan boshlanadigan baytlari. CTR'da
/// shifrlash va ochish bir xil amal.
fn ctr_apply(key: &[u8; 16], offset: u64, buf: &mut [u8]) {
    use ctr::cipher::{KeyIvInit, StreamCipher, StreamCipherSeek};
    let mut c = Aes128Ctr::new(key.into(), &[0u8; 16].into());
    c.seek(offset);
    c.apply_keystream(buf);
}

fn key_of(t: &Tg, name: &str) -> Option<[u8; 16]> {
    t.keys.lock().ok()?.get(name).copied()
}

fn save_keys(t: &Tg) {
    let Ok(k) = t.keys.lock() else { return };
    let m: HashMap<&String, String> = k.iter().map(|(n, v)| (n, hex::encode(v))).collect();
    let body = serde_json::to_vec(&m).unwrap_or_default();
    drop(k);
    let _ = write_sealed(&t.dir.join("keys.bin"), LABEL_KEYS, &body);
}

/// Kalitlarni qo'shadi (`{"nom": "hex", ...}`); o'zgargan bo'lsa diskka.
fn add_keys(t: &Tg, m: &serde_json::Map<String, Value>) {
    let mut changed = false;
    if let Ok(mut k) = t.keys.lock() {
        for (name, v) in m {
            let Some(key) = v.as_str().and_then(parse_key) else { continue };
            if k.get(name) != Some(&key) {
                k.insert(name.clone(), key);
                changed = true;
            }
        }
    }
    if changed {
        // Eski (kalitsiz) ma'lumot xotirada qolmasin.
        if let Ok(mut d) = t.docs.lock() {
            for name in m.keys() {
                d.remove(name);
            }
        }
        save_keys(t);
    }
}

/// Shifrlangan fayl Telegram'da `application/octet-stream` bo'lib
/// turadi — pleyer va rasm ochuvchiga asl turi nomidan beriladi.
pub(crate) fn mime_of_name(name: &str) -> &'static str {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "mp4" => "video/mp4",
        "mkv" => "video/x-matroska",
        "m4a" => "audio/mp4",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    }
}

fn status_json(t: &Tg) -> String {
    json!({
        "configured": t.api_id.lock().map(|v| *v > 0).unwrap_or(false),
        "authorized": t.authorized.load(Ordering::SeqCst),
        "port": t.port,
        "lost": t.lost.load(Ordering::SeqCst),
    })
    .to_string()
}

fn err_json(msg: impl std::fmt::Display) -> String {
    json!({"error": msg.to_string()}).to_string()
}

fn rpc_name(e: &InvocationError) -> Option<&str> {
    match e {
        InvocationError::Rpc(r) => Some(r.name.as_str()),
        _ => None,
    }
}

// ═══════════════════════════════════════════════════════════════
//  VIDEO_CACHE UCHUN YO'NALTIRISH
// ═══════════════════════════════════════════════════════════════

/// Shu fayl Telegram'dan olinishi kerak bo'lsa — mahalliy manba
/// manzilini qaytaradi. Aks holda `None` (odatdagi worker yo'li).
pub fn route_url(key: &str) -> Option<String> {
    let t = tg()?;
    if !t.authorized.load(Ordering::SeqCst) {
        return None;
    }
    let msg_id = *t.routes.lock().ok()?.get(key)?;
    if let Ok(f) = t.failed.lock() {
        if let Some(at) = f.get(key) {
            if at.elapsed() < FAIL_COOLDOWN {
                return None;
            }
        }
    }
    Some(format!("http://127.0.0.1:{}/tg/{msg_id}/{key}", t.port))
}

/// Manzil shu mahalliy Telegram manbasigami. Bunday manzil uchun
/// worker keshini "isitish" umuman kerak emas (B2'ga so'rov ketmasin).
pub fn is_origin_url(url: &str) -> bool {
    match tg() {
        Some(t) => url.starts_with(&format!("http://127.0.0.1:{}/tg/", t.port)),
        None => false,
    }
}

/// Mahalliy Telegram manzilidan fayl nomi (`.../tg/<belgi>/<nom>`).
/// Yuklab olish shu nom bilan baytlarni TO'G'RIDAN-TO'G'RI oladi
/// (`fetch_range`) — mahalliy HTTP server orqali emas.
pub(crate) fn origin_name(url: &str) -> Option<String> {
    let t = tg()?;
    let rest = url.strip_prefix(&format!("http://127.0.0.1:{}/tg/", t.port))?;
    let name = rest.split('?').next()?.rsplit('/').next()?;
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

pub(crate) fn note_failure(key: &str) {
    if let Some(t) = tg() {
        if let Ok(mut f) = t.failed.lock() {
            f.insert(key.to_string(), Instant::now());
        }
    }
}

// ═══════════════════════════════════════════════════════════════
//  FAYLNI OLISH
// ═══════════════════════════════════════════════════════════════

/// Kirish botini (foydalanuvchi sifatida) topadi.
async fn bot_peer(t: &Tg, client: &Client) -> Result<(i64, i64), String> {
    if let Some(p) = t.bot_peer.lock().ok().and_then(|p| *p) {
        return Ok(p);
    }
    let bot = t.bot.lock().map(|b| b.clone()).unwrap_or_default();
    if bot.is_empty() {
        return Err("bot nomi noma'lum".to_string());
    }
    let tl::enums::contacts::ResolvedPeer::Peer(rp) = client
        .invoke(&tl::functions::contacts::ResolveUsername { username: bot, referer: None })
        .await
        .map_err(|e| inv_err(&e))?;
    let p = rp
        .users
        .iter()
        .find_map(|u| match u {
            tl::enums::User::User(u) if u.bot => u.access_hash.map(|h| (u.id, h)),
            _ => None,
        })
        .ok_or("bot topilmadi")?;
    if let Ok(mut c) = t.bot_peer.lock() {
        *c = Some(p);
    }
    Ok(p)
}

/// Xabardagi faylning nomi: izoh (birinchi qator) yoki fayl nomi.
fn message_name(m: &tl::types::Message) -> Option<String> {
    let cap = m.message.lines().next().unwrap_or("").trim();
    if !cap.is_empty() {
        return Some(cap.to_string());
    }
    let Some(tl::enums::MessageMedia::Document(md)) = &m.media else { return None };
    let Some(tl::enums::Document::Document(d)) = &md.document else { return None };
    d.attributes.iter().find_map(|a| match a {
        tl::enums::DocumentAttribute::Filename(f) => Some(f.file_name.clone()),
        _ => None,
    })
}

/// Xabardagi fayl shu nomdagi faylmi (fayl nomi yoki izoh bo'yicha).
fn doc_matches(m: &tl::types::Message, name: &str) -> Option<DocInfo> {
    // Surat: nomi faqat izohda (suratda fayl nomi bo'lmaydi).
    if let Some(tl::enums::MessageMedia::Photo(mp)) = &m.media {
        if m.message.lines().next().unwrap_or("").trim() != name {
            return None;
        }
        let Some(tl::enums::Photo::Photo(p)) = &mp.photo else { return None };
        // Eng katta o'lcham.
        let (kind, size) = p
            .sizes
            .iter()
            .filter_map(|s| match s {
                tl::enums::PhotoSize::Size(s) => Some((s.r#type.clone(), s.size.max(0) as u64, s.w * s.h)),
                tl::enums::PhotoSize::Progressive(s) => {
                    Some((s.r#type.clone(), s.sizes.last().copied().unwrap_or(0).max(0) as u64, s.w * s.h))
                }
                _ => None,
            })
            .max_by_key(|(_, _, area)| *area)
            .map(|(k, s, _)| (k, s))?;
        return Some(DocInfo {
            id: p.id,
            access_hash: p.access_hash,
            file_reference: p.file_reference.clone(),
            dc_id: p.dc_id,
            size,
            mime: "image/jpeg".to_string(),
            photo_size: Some(kind),
        });
    }
    let Some(tl::enums::MessageMedia::Document(md)) = &m.media else { return None };
    let Some(tl::enums::Document::Document(d)) = &md.document else { return None };
    let by_attr = d.attributes.iter().any(|a| {
        matches!(a, tl::enums::DocumentAttribute::Filename(f) if f.file_name == name)
    });
    if !by_attr && m.message.lines().next().unwrap_or("").trim() != name {
        return None;
    }
    Some(DocInfo {
        id: d.id,
        access_hash: d.access_hash,
        file_reference: d.file_reference.clone(),
        dc_id: d.dc_id,
        size: d.size.max(0) as u64,
        mime: if d.mime_type.is_empty() || d.mime_type == "application/octet-stream" {
            mime_of_name(name).to_string()
        } else {
            d.mime_type.clone()
        },
        photo_size: None,
    })
}

/// Bot chatidan shu nomdagi faylni topadi.
///
/// TOPILGAN XATO (foydalanuvchi: "video kanalga yuklanyapti, lekin
/// pleyerda ochilmayapti"): ilgari fayl worker qaytargan XABAR
/// RAQAMI bilan olinardi. Lekin shaxsiy chatlarda raqamlar HAR BIR
/// HISOB uchun alohida: botning `copyMessage` bergan raqami
/// foydalanuvchi hisobida BOSHQA xabarni ko'rsatadi. Endi fayl bot
/// chatining oxirgi xabarlari orasidan NOMI bo'yicha topiladi
/// (bot izohga ham, fayl nomiga ham shu nomni qo'yadi).
async fn fetch_doc(t: &Tg, client: &Client, name: &str) -> Result<DocInfo, String> {
    let found = find_in_chat(t, client, &[name.to_string()]).await?;
    if found.contains(name) {
        if let Some(d) = t.docs.lock().ok().and_then(|m| m.get(name).cloned()) {
            return Ok(d);
        }
    }
    Err("bot chatida fayl topilmadi".to_string())
}

fn messages_of(res: tl::enums::messages::Messages) -> Vec<tl::enums::Message> {
    match res {
        tl::enums::messages::Messages::Messages(m) => m.messages,
        tl::enums::messages::Messages::Slice(m) => m.messages,
        tl::enums::messages::Messages::ChannelMessages(m) => m.messages,
        tl::enums::messages::Messages::NotModified(_) => Vec::new(),
    }
}

/// Xabarlardagi HAMMA faylni keshga yozadi (keyingi so'rovlar
/// Telegram'ga bormasin); topilgan nomlar [found] ga, ularning xabar
/// raqami `routes` ga qo'shiladi. Eng yangisi ustun turadi.
fn remember(t: &Tg, messages: &[tl::enums::Message], found: &mut HashSet<String>) -> i32 {
    let mut last_id = 0;
    let mut routes_changed = false;
    for m in messages.iter().rev() {
        let tl::enums::Message::Message(m) = m else { continue };
        last_id = if last_id == 0 { m.id } else { last_id.min(m.id) };
        let Some(key) = message_name(m) else { continue };
        if let Some(d) = doc_matches(m, &key) {
            if let Ok(mut c) = t.docs.lock() {
                c.insert(key.clone(), d);
            }
            if let Ok(mut r) = t.routes.lock() {
                if r.get(&key) != Some(&m.id) {
                    r.insert(key.clone(), m.id);
                    routes_changed = true;
                }
            }
            // Fayl chatda bor — oldingi o'qish xatosi sababli qo'yilgan
            // chetlatish olib tashlanadi (aks holda ilova uni "yo'q"
            // deb bilib, botdan YANA nusxa so'rardi).
            if let Ok(mut f) = t.failed.lock() {
                f.remove(&key);
            }
            found.insert(key);
        }
    }
    if routes_changed {
        save_routes(t);
    }
    last_id
}

/// Bu nomlar bot chatida qaysi xabarda — chatning OXIRGI 100 ta
/// xabaridan (bot nusxani so'ralishi bilan yuboradi va chat
/// ishlatilgach tozalanadi, ya'ni nusxa doim shu yerda).
///
/// Telegram qidiruvi (`messages.search`) ATAYLAB yo'q — foydalanuvchi:
/// "bot chatidan izlash juda sekin". U tez-tez cheklanardi ham.
async fn find_in_chat(t: &Tg, client: &Client, names: &[String]) -> Result<HashSet<String>, String> {
    let (id, hash) = bot_peer(t, client).await?;
    let peer = tl::enums::InputPeer::User(tl::types::InputPeerUser { user_id: id, access_hash: hash });
    let res = client
        .invoke(&tl::functions::messages::GetHistory {
            peer,
            offset_id: 0,
            offset_date: 0,
            add_offset: 0,
            limit: 100,
            max_id: 0,
            min_id: 0,
            hash: 0,
        })
        .await
        .map_err(|e| inv_err(&e))?;
    let mut found: HashSet<String> = HashSet::new();
    remember(t, &messages_of(res), &mut found);
    found.retain(|n| names.contains(n));
    Ok(found)
}

async fn doc_for(t: &'static Tg, client: &Client, name: &str, refresh: bool) -> Result<DocInfo, String> {
    if !refresh {
        if let Some(d) = t.docs.lock().ok().and_then(|m| m.get(name).cloned()) {
            return Ok(d);
        }
    }
    let d = match fetch_doc(t, client, name).await {
        Ok(d) => d,
        Err(e) => {
            // Sessiya Telegram tomonidan bekor qilingan (masalan
            // foydalanuvchi "Qurilmalar"dan chiqarib yuborgan). Endi
            // har bir video avval Telegram'ni sinab vaqt yo'qotmasin —
            // qayta ulanmaguncha hammasi worker yo'lidan ketadi.
            check_dead(t, &e);
            return Err(e);
        }
    };
    if let Ok(mut m) = t.docs.lock() {
        m.insert(name.to_string(), d.clone());
    }
    Ok(d)
}

// ═══════════════════════════════════════════════════════════════
//  SESSIYA UZILDI (foydalanuvchi Telegram'da "Qurilmalar"dan chiqardi)
// ═══════════════════════════════════════════════════════════════
//
// TOPILGAN XATO (foydalanuvchi): "Telegram orqali sessiyani uzib,
// ilovada video bosdim — pleyer ochildi, lekin video ishlamadi; bot
// videoni chatga yuborgan, lekin o'chirilmagan".
//
// Ilgari sessiya o'lganini faqat bitta joy (bot chatidan fayl
// qidirish) sezardi. Endi Telegram "sessiya yo'q" degan HAR BIR
// joyda (`check_dead`) va ilova ochilganda/qaytganda/video oldidan
// (`rust_tg_check_session`) aniqlanadi. Aniqlangach sessiya
// tashlanadi, ilova esa darhol raqam oynasini ko'rsatadi
// (`AuthGate`). Qayta kirilgach bot chatida qolgan nusxalar
// tozalanadi (Dart: `_afterLogin`).

/// Sessiya o'lganini bildiradigan xatomi — bo'lsa sessiyani tashlaydi.
/// Faqat kirilgan holatda (kirish jarayonidagi AUTH_KEY_UNREGISTERED
/// normal holat).
fn check_dead(t: &Tg, err: &str) -> bool {
    if !t.authorized.load(Ordering::SeqCst) || !SESSION_DEAD.iter().any(|k| err.contains(k)) {
        return false;
    }
    t.authorized.store(false, Ordering::SeqCst);
    t.lost.store(true, Ordering::SeqCst);
    save_config(t);
    // O'lik kalit bilan qayta kirib bo'lmaydi — sessiya butunlay
    // tashlanadi, keyingi kirish toza boshlanadi.
    if let Ok(s) = t.session.lock() {
        if let Some(s) = s.as_ref() {
            s.wipe();
        }
    }
    let _ = fs::remove_file(t.dir.join("session.bin"));
    disconnect(t);
    if let Ok(mut d) = t.auth_dcs.try_lock() {
        d.clear();
    }
    forget_peers(t);
    if let Ok(mut d) = t.docs.lock() {
        d.clear();
    }
    if let Ok(mut r) = t.routes.lock() {
        r.clear();
    }
    save_routes(t);
    crate::video_cache::tg_log(format!("Telegram sessiyasi bekor qilingan: {err}"));
    true
}

/// Asosiy DC dan boshqa DC ga hisobni ko'chiradi (fayl boshqa DC da
/// turganda kerak). Har bir DC uchun bir marta.
async fn copy_auth(t: &Tg, client: &Client, dc_id: i32) -> Result<(), String> {
    let mut done = t.auth_dcs.lock().await;
    if done.contains(&dc_id) {
        return Ok(());
    }
    let tl::enums::auth::ExportedAuthorization::Authorization(exp) = client
        .invoke(&tl::functions::auth::ExportAuthorization { dc_id })
        .await
        .map_err(|e| e.to_string())?;
    client
        .invoke_in_dc(
            dc_id,
            &tl::functions::auth::ImportAuthorization { id: exp.id, bytes: exp.bytes },
        )
        .await
        .map_err(|e| e.to_string())?;
    done.insert(dc_id);
    Ok(())
}

/// Faylning `offset` dan boshlanadigan bitta qismini (eng ko'pi
/// `PART` bayt) XOTIRAGA oladi.
async fn fetch_part(t: &'static Tg, client: Client, name: String, offset: u64) -> Result<Vec<u8>, String> {
    let _permit = t.inflight.acquire().await.map_err(|e| e.to_string())?;
    let mut doc = doc_for(t, &client, &name, false).await?;
    let mut dc = doc.dc_id;
    let mut last_err = String::new();
    for _ in 0..PART_ATTEMPTS {
        let req = tl::functions::upload::GetFile {
            precise: false,
            cdn_supported: false,
            location: match &doc.photo_size {
                Some(kind) => tl::enums::InputFileLocation::InputPhotoFileLocation(
                    tl::types::InputPhotoFileLocation {
                        id: doc.id,
                        access_hash: doc.access_hash,
                        file_reference: doc.file_reference.clone(),
                        thumb_size: kind.clone(),
                    },
                ),
                None => tl::enums::InputFileLocation::InputDocumentFileLocation(
                    tl::types::InputDocumentFileLocation {
                        id: doc.id,
                        access_hash: doc.access_hash,
                        file_reference: doc.file_reference.clone(),
                        thumb_size: String::new(),
                    },
                ),
            },
            offset: offset as i64,
            limit: PART as i32,
        };
        let c = part_client(t, &client, dc);
        match c.invoke_in_dc(dc, &req).await {
            Ok(tl::enums::upload::File::File(f)) => {
                mark_dc_ready(t, dc);
                let mut bytes = f.bytes;
                // Shifrlangan fayl — shu qismning o'zi ochiladi.
                if let Some(key) = key_of(t, &name) {
                    ctr_apply(&key, offset, &mut bytes);
                }
                return Ok(bytes);
            }
            Ok(tl::enums::upload::File::CdnRedirect(_)) => {
                return Err("CDN yo'naltirishi kutilmagan".to_string());
            }
            Err(e) => {
                last_err = inv_err(&e);
                match rpc_name(&e) {
                    // Fayl havolasi eskirgan — xabarni qayta olamiz.
                    Some(n) if n.starts_with("FILE_REFERENCE_") => {
                        doc = doc_for(t, &client, &name, true).await?;
                    }
                    // Boshqa DC: hisobni o'sha yerga ko'chiramiz.
                    Some("AUTH_KEY_UNREGISTERED") => {
                        // Asosiy DC ham "kalit yo'q" desa — sessiya o'lgan.
                        if let Err(e) = copy_auth(t, &client, dc).await {
                            check_dead(t, &e);
                            return Err(e);
                        }
                    }
                    Some("FILE_MIGRATE") => {
                        if let InvocationError::Rpc(r) = &e {
                            if let Some(v) = r.value {
                                dc = v as i32;
                            }
                        }
                    }
                    _ => {
                        check_dead(t, &last_err);
                        return Err(last_err);
                    }
                }
            }
        }
    }
    Err(last_err)
}

// ═══════════════════════════════════════════════════════════════
//  MAHALLIY HTTP MANBA
// ═══════════════════════════════════════════════════════════════

async fn read_head(stream: &mut TcpStream) -> Option<String> {
    let mut buf = Vec::with_capacity(1024);
    let mut tmp = [0u8; 1024];
    loop {
        let n = stream.read(&mut tmp).await.ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&tmp[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
            return String::from_utf8(buf).ok();
        }
        if buf.len() > 16 * 1024 {
            return None;
        }
    }
}

/// "bytes=a-b" / "bytes=a-" / "bytes=-n" -> (boshi, oxiri) qo'shib.
fn parse_range(h: &str, size: u64) -> Option<(u64, u64)> {
    let spec = h.trim().strip_prefix("bytes=")?.split(',').next()?.trim();
    let (a, b) = spec.split_once('-')?;
    if size == 0 {
        return None;
    }
    if a.is_empty() {
        let n: u64 = b.parse().ok()?;
        if n == 0 {
            return None;
        }
        return Some((size.saturating_sub(n), size - 1));
    }
    let start: u64 = a.parse().ok()?;
    if start >= size {
        return None;
    }
    let end = if b.is_empty() { size - 1 } else { b.parse::<u64>().ok()?.min(size - 1) };
    if end < start {
        return None;
    }
    Some((start, end))
}

async fn respond(stream: &mut TcpStream, status: &str, extra: &str) {
    let _ = stream
        .write_all(
            format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n{extra}\r\n")
                .as_bytes(),
        )
        .await;
}

async fn handle(t: &'static Tg, mut stream: TcpStream) {
    let Some(head) = read_head(&mut stream).await else { return };
    let mut lines = head.split("\r\n");
    let first = lines.next().unwrap_or("");
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("").to_string();
    let mut range: Option<String> = None;
    for l in lines {
        if let Some((k, v)) = l.split_once(':') {
            if k.trim().eq_ignore_ascii_case("range") {
                range = Some(v.trim().to_string());
            }
        }
    }
    if method != "GET" && method != "HEAD" {
        respond(&mut stream, "405 Method Not Allowed", "").await;
        return;
    }
    // /tg/<belgi>/<fayl_nomi> — fayl NOMI bo'yicha topiladi
    // (`fetch_doc` izohiga qarang); o'rtadagi belgi faqat manzilni
    // yangilash uchun.
    let path = path.split('?').next().unwrap_or("");
    let mut seg = path.trim_start_matches('/').split('/');
    let (Some("tg"), Some(_), Some(key)) = (seg.next(), seg.next(), seg.next()) else {
        respond(&mut stream, "404 Not Found", "").await;
        return;
    };
    let key = key.to_string();
    let Some(client) = connect(t) else {
        respond(&mut stream, "503 Service Unavailable", "").await;
        return;
    };
    let doc = match doc_for(t, &client, &key, false).await {
        Ok(d) => d,
        Err(e) => {
            crate::video_cache::tg_log(format!("Telegram: {key} ochilmadi: {e}"));
            note_failure(&key);
            respond(&mut stream, "502 Bad Gateway", "").await;
            return;
        }
    };
    let size = doc.size;
    let (start, end, partial) = match range.as_deref() {
        Some(r) => match parse_range(r, size) {
            Some((s, e)) => (s, e, true),
            None => {
                respond(&mut stream, "416 Range Not Satisfiable", &format!("Content-Range: bytes */{size}\r\n"))
                    .await;
                return;
            }
        },
        None => (0, size.saturating_sub(1), false),
    };
    let len = if size == 0 { 0 } else { end - start + 1 };
    let mut h = if partial {
        format!("HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {start}-{end}/{size}\r\n")
    } else {
        "HTTP/1.1 200 OK\r\n".to_string()
    };
    h.push_str(&format!(
        "Content-Type: {}\r\nContent-Length: {len}\r\nAccept-Ranges: bytes\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        doc.mime
    ));
    if stream.write_all(h.as_bytes()).await.is_err() || method == "HEAD" || len == 0 {
        return;
    }

    let spawn = |off: u64| t.rt.spawn(fetch_part(t, client.clone(), key.clone(), off));
    if let Err(e) = pump(&mut stream, start, end, spawn).await {
        if let PumpError::Source(e) = e {
            crate::video_cache::tg_log(format!("Telegram: {key} olinmadi: {e}"));
            note_failure(&key);
        }
    }
}

enum PumpError {
    /// Telegram'dan olib bo'lmadi.
    Source(String),
    /// Pleyer ulanishni uzdi (masalan, oldinga surdi) — xato emas.
    Closed,
}

/// `start..=end` baytlarini `out` ga yozadi. Qismlar (`PART`)
/// tartib bilan yoziladi, lekin `PIPELINE` tasi oldindan so'rab
/// qo'yiladi. To'xtaganda havodagi so'rovlar bekor qilinadi —
/// ortiqcha trafik sarflanmaydi.
async fn pump<W, F>(out: &mut W, start: u64, end: u64, spawn: F) -> Result<(), PumpError>
where
    W: tokio::io::AsyncWrite + Unpin,
    F: Fn(u64) -> tokio::task::JoinHandle<Result<Vec<u8>, String>>,
{
    let mut next_off = start / PART * PART;
    let mut queue: VecDeque<(u64, tokio::task::JoinHandle<Result<Vec<u8>, String>>)> = VecDeque::new();
    let mut pos = start;
    let result = loop {
        if pos > end {
            break Ok(());
        }
        while queue.len() < PIPELINE && next_off <= end {
            queue.push_back((next_off, spawn(next_off)));
            next_off += PART;
        }
        let Some((off, job)) = queue.pop_front() else { break Ok(()) };
        let bytes = match job.await {
            Ok(Ok(b)) => b,
            Ok(Err(e)) => break Err(PumpError::Source(e)),
            Err(e) => break Err(PumpError::Source(e.to_string())),
        };
        let from = (pos - off) as usize;
        if from >= bytes.len() {
            break Err(PumpError::Source(format!("fayl kutilganidan qisqa ({off})")));
        }
        let to = ((end + 1 - off) as usize).min(bytes.len());
        if out.write_all(&bytes[from..to]).await.is_err() {
            break Err(PumpError::Closed);
        }
        pos = off + to as u64;
    };
    for (_, j) in queue {
        j.abort();
    }
    result
}

async fn serve(t: &'static Tg, listener: TcpListener) {
    loop {
        let Ok((stream, _)) = listener.accept().await else { continue };
        let _ = stream.set_nodelay(true);
        t.rt.spawn(handle(t, stream));
    }
}

/// Rust ichida `panic` bo'lsa (release'da ilova darhol yopiladi)
/// sababi `last_crash.txt` ga yoziladi — keyingi ochilishda ilova uni
/// ekranda ko'rsatadi (`crash_log.dart`).
fn install_crash_log(dir: &std::path::Path) {
    static ONCE: std::sync::Once = std::sync::Once::new();
    let path = dir.parent().unwrap_or(dir).join("last_crash.txt");
    ONCE.call_once(move || {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let thread = std::thread::current().name().unwrap_or("?").to_string();
            let _ = fs::write(&path, format!("Rust panic [{thread}]: {info}"));
            prev(info);
        }));
    });
}

fn init(dir: &str, api_id: i32, api_hash: &str) -> Result<&'static Tg, String> {
    if TG.get().is_none() {
        let dir = PathBuf::from(dir);
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        install_crash_log(&dir);
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("tg")
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        let listener = rt
            .block_on(TcpListener::bind("127.0.0.1:0"))
            .map_err(|e| e.to_string())?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();

        let mut cfg_id = 0;
        let mut cfg_hash = String::new();
        let mut authorized = false;
        if let Some(plain) = read_sealed(&dir.join("config.bin"), LABEL_CONFIG) {
            if let Ok(v) = serde_json::from_slice::<Value>(&plain) {
                cfg_id = v["api_id"].as_i64().unwrap_or(0) as i32;
                cfg_hash = v["api_hash"].as_str().unwrap_or("").to_string();
                authorized = v["authorized"].as_bool().unwrap_or(false);
            }
        }
        let routes: HashMap<String, i32> = read_sealed(&dir.join("routes.bin"), LABEL_ROUTES)
            .and_then(|p| serde_json::from_slice(&p).ok())
            .unwrap_or_default();

        let keys: HashMap<String, [u8; 16]> = read_sealed(&dir.join("keys.bin"), LABEL_KEYS)
            .and_then(|p| serde_json::from_slice::<HashMap<String, String>>(&p).ok())
            .map(|m| m.into_iter().filter_map(|(k, v)| Some((k, parse_key(&v)?))).collect())
            .unwrap_or_default();

        let t = Tg {
            rt,
            dir,
            port,
            client: Mutex::new(None),
            session: Mutex::new(None),
            api_id: Mutex::new(cfg_id),
            api_hash: Mutex::new(cfg_hash),
            authorized: AtomicBool::new(authorized),
            password_token: Mutex::new(None),
            docs: Mutex::new(HashMap::new()),
            bot: Mutex::new(String::new()),
            bot_peer: Mutex::new(None),
            routes: Mutex::new(routes),
            failed: Mutex::new(HashMap::new()),
            auth_dcs: tokio::sync::Mutex::new(HashSet::new()),
            inflight: Arc::new(Semaphore::new(MAX_INFLIGHT)),
            dl: Mutex::new(Vec::new()),
            dl_next: std::sync::atomic::AtomicUsize::new(0),
            dl_dcs: Mutex::new(HashSet::new()),
            keys: Mutex::new(keys),
            lost: AtomicBool::new(false),
        };
        if TG.set(t).is_ok() {
            let t = TG.get().unwrap();
            load_wait(t);
            t.rt.spawn(serve(t, listener));
        }
    }
    let t = TG.get().ok_or("init")?;

    // Worker'dan yangi qiymatlar keldi — saqlaymiz. `api_id`
    // o'zgarsa eski sessiya endi yaroqsiz.
    if api_id > 0 && !api_hash.is_empty() {
        let old = t.api_id.lock().map(|v| *v).unwrap_or(0);
        if old != api_id {
            disconnect(t);
            if old > 0 {
                let _ = fs::remove_file(t.dir.join("session.bin"));
                t.authorized.store(false, Ordering::SeqCst);
            }
        }
        if let Ok(mut v) = t.api_id.lock() {
            *v = api_id;
        }
        if let Ok(mut v) = t.api_hash.lock() {
            *v = api_hash.to_string();
        }
        save_config(t);
    }
    Ok(t)
}

fn with_client<F, R>(f: F) -> String
where
    F: FnOnce(&'static Tg, Client) -> Result<R, String>,
    R: Into<String>,
{
    let Some(t) = tg() else { return err_json("Telegram ishga tushmagan") };
    let Some(client) = connect(t) else { return err_json("api_id berilmagan") };
    match f(t, client) {
        Ok(s) => s.into(),
        Err(e) => {
            check_dead(t, &e);
            let left = wait_left();
            if left > 0 {
                json!({"error": e, "wait": left}).to_string()
            } else {
                err_json(e)
            }
        }
    }
}

fn after_login(t: &Tg) -> String {
    forget_peers(t);
    if let Ok(mut d) = t.auth_dcs.try_lock() {
        d.clear();
    }
    t.authorized.store(true, Ordering::SeqCst);
    WAIT_UNTIL.store(0, Ordering::SeqCst);
    let _ = fs::remove_file(wait_path(t));
    t.lost.store(false, Ordering::SeqCst);
    if let Ok(mut d) = t.docs.lock() {
        d.clear();
    }
    if let Ok(mut f) = t.failed.lock() {
        f.clear();
    }
    if let Ok(s) = t.session.lock() {
        if let Some(s) = s.as_ref() {
            s.save();
        }
    }
    save_config(t);
    json!({"ok": true}).to_string()
}

// ═══════════════════════════════════════════════════════════════
//  FFI (Dart tomoni)
// ═══════════════════════════════════════════════════════════════
//
// Hamma funksiyalar JSON satr qaytaradi (`rust_free_string` bilan
// bo'shatiladi). Tarmoqqa chiqadiganlari (kod so'rash, kirish)
// BLOKLAYDI — Dart ularni alohida isolate'da chaqiradi.

/// Ishga tushiradi. `api_id <= 0` bo'lsa avval saqlangan qiymat
/// ishlatiladi (oflayn ishga tushish).
#[no_mangle]
pub extern "C" fn rust_tg_init(dir_ptr: *const c_char, api_id: i32, api_hash_ptr: *const c_char) -> *mut c_char {
    let dir = unsafe { cstr_to_str(dir_ptr) }.unwrap_or("");
    let hash = unsafe { cstr_to_str(api_hash_ptr) }.unwrap_or("");
    if dir.is_empty() {
        return string_to_cptr(err_json("papka berilmagan"));
    }
    string_to_cptr(match init(dir, api_id, hash) {
        Ok(t) => status_json(t),
        Err(e) => err_json(e),
    })
}

/// Sessiya hali tirikmi — Telegram'ga bitta arzon so'rov
/// (`updates.getState`). O'lgan bo'lsa sessiya tashlanadi. Javob —
/// `rust_tg_status` bilan bir xil. Tarmoq yo'q bo'lsa holat
/// o'zgarmaydi. BLOKLAYDI (Dart uni alohida isolate'da chaqiradi).
#[no_mangle]
pub extern "C" fn rust_tg_check_session() -> *mut c_char {
    let Some(t) = tg() else {
        return string_to_cptr(json!({"configured": false, "authorized": false, "port": 0}).to_string());
    };
    if t.authorized.load(Ordering::SeqCst) {
        if let Some(client) = connect(t) {
            let r = t.rt.block_on(async {
                tokio::time::timeout(Duration::from_secs(10), client.invoke(&tl::functions::updates::GetState {})).await
            });
            if let Ok(Err(e)) = r {
                check_dead(t, &e.to_string());
            }
        }
    }
    string_to_cptr(status_json(t))
}

#[no_mangle]
pub extern "C" fn rust_tg_status() -> *mut c_char {
    string_to_cptr(match tg() {
        Some(t) => status_json(t),
        None => json!({"configured": false, "authorized": false, "port": 0}).to_string(),
    })
}

// ── KIRISH BOSQICHI DISKDA SAQLANADI ───────────────────────────
//
// TALAB (foydalanuvchi): "kod yozish oynasi sessiya qilib saqlab
// qolinsin — ilova yopilgan bo'lsa ham qaytib kirganda kod yozadigan
// joy avtomatik ochilsin".
//
// Kod so'ralgach `{bosqich, raqam, phone_code_hash}` shifrlangan
// faylga (`login.bin`) yoziladi. Ilova qayta ochilganda
// `rust_tg_login_state` shu bosqichni qaytaradi va ekran to'g'ridan-
// to'g'ri kod (yoki parol) oynasidan boshlanadi.
//
// `grammers` ning `LoginToken`i diskka saqlanmaydi (maydonlari
// yopiq), shu sabab kod yuborish va kirish Telegram API'ning o'zi
// bilan (`auth.sendCode` / `auth.signIn`) bajariladi.

fn login_path(t: &Tg) -> PathBuf {
    t.dir.join("login.bin")
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn load_login(t: &Tg) -> Option<Value> {
    let v: Value = serde_json::from_slice(&read_sealed(&login_path(t), LABEL_LOGIN)?).ok()?;
    if now_ms() - v["at"].as_i64().unwrap_or(0) > LOGIN_STATE_TTL_MS {
        let _ = fs::remove_file(login_path(t));
        return None;
    }
    Some(v)
}

fn save_login(t: &Tg, stage: &str, phone: &str, hash: &str, hint: &str) {
    save_login_info(t, stage, phone, hash, hint, &load_login(t).map(|v| v["sent"].clone()).unwrap_or(Value::Null));
}

/// `sent` — kod QAYERGA yuborilgani (`sent_info`), ekranda aytiladi.
fn save_login_info(t: &Tg, stage: &str, phone: &str, hash: &str, hint: &str, sent: &Value) {
    let body = json!({"stage": stage, "phone": phone, "hash": hash, "hint": hint, "sent": sent, "at": now_ms()});
    let _ = write_sealed(&login_path(t), LABEL_LOGIN, body.to_string().as_bytes());
}

fn clear_login(t: &Tg) {
    let _ = fs::remove_file(login_path(t));
    if let Ok(mut p) = t.password_token.lock() {
        *p = None;
    }
}

// ═══════════════════════════════════════════════════════════════
//  KIRISH TOKENLARI (Telegram / Cherrygram ilovasidagi kabi)
// ═══════════════════════════════════════════════════════════════
//
// TALAB (foydalanuvchi): "sessiya uzilgach qayta kirishda kod
// kelmayapti — Cherrygram'da kod yuborish qanday ishlashini ko'r".
//
// Cherrygram (`LoginActivity.java`) har muvaffaqiyatli kirishdan
// keyin Telegram bergan `future_auth_token` ni saqlaydi (chiqishda
// ham — `auth.loggedOut`), keyingi `auth.sendCode` da esa ularni
// `CodeSettings.logout_tokens` ga qo'yadi. Tokeni tanilgan qurilmani
// Telegram ko'pincha KODSIZ kiritadi (`auth.sentCodeSuccess`) yoki
// darhol parolni so'raydi (SESSION_PASSWORD_NEEDED).
//
// Tokenlar sessiyadan ALOHIDA faylda (`tokens.bin`, shifrlangan)
// turadi — sessiya uzilganda ham, chiqishda ham o'chmaydi. Eng
// ko'pi 20 ta (Telegram chegarasi).

fn load_tokens(t: &Tg) -> Vec<Vec<u8>> {
    read_sealed(&t.dir.join("tokens.bin"), LABEL_TOKENS)
        .and_then(|p| serde_json::from_slice::<Vec<String>>(&p).ok())
        .map(|v| v.iter().filter_map(|h| hex::decode(h).ok()).collect())
        .unwrap_or_default()
}

fn save_token(t: &Tg, token: Option<&Vec<u8>>) {
    let Some(token) = token.filter(|t| !t.is_empty()) else { return };
    let mut all = load_tokens(t);
    all.retain(|x| x != token);
    all.insert(0, token.clone());
    all.truncate(20);
    let hexes: Vec<String> = all.iter().map(hex::encode).collect();
    let _ = write_sealed(
        &t.dir.join("tokens.bin"),
        LABEL_TOKENS,
        &serde_json::to_vec(&hexes).unwrap_or_default(),
    );
}

fn code_settings(tokens: Vec<Vec<u8>>) -> tl::enums::CodeSettings {
    tl::types::CodeSettings {
        allow_flashcall: false,
        current_number: false,
        allow_app_hash: false,
        allow_missed_call: false,
        allow_firebase: false,
        logout_tokens: if tokens.is_empty() { None } else { Some(tokens) },
        token: None,
        app_sandbox: None,
        unknown_number: false,
    }
    .into()
}

// ── QANCHA KUTISH KERAK ──────────────────────────────────────
//
// TALAB (foydalanuvchi): "hisobga kirish uchun qancha kutish
// kerakligini aniq ko'rsatsin".
//
// Telegram vaqtni faqat `FLOOD_WAIT_<soniya>` (va
// `FLOOD_PREMIUM_WAIT_<soniya>`) da aniq aytadi — o'sha son
// saqlanadi (`wait.bin`, ilova yopilsa ham) va ekranda teskari
// sanoq bo'lib turadi; tugaguncha Telegram'ga so'rov ketmaydi.
// `PHONE_NUMBER_FLOOD` va `SEND_CODE_UNAVAILABLE` da Telegram vaqt
// BERMAYDI — ekranda shunday deb aytiladi.

static WAIT_UNTIL: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

fn wait_path(t: &Tg) -> PathBuf {
    t.dir.join("wait.bin")
}

/// Telegram aytgan kutishning qolgan qismi (soniya).
fn wait_left() -> i64 {
    let until = WAIT_UNTIL.load(Ordering::SeqCst);
    ((until - now_ms()) / 1000).max(0)
}

fn set_wait(secs: u32) {
    let until = now_ms() + secs as i64 * 1000;
    WAIT_UNTIL.store(until, Ordering::SeqCst);
    if let Some(t) = tg() {
        let _ = write_sealed(&wait_path(t), LABEL_LOGIN, json!({"until": until}).to_string().as_bytes());
    }
}

fn load_wait(t: &Tg) {
    if let Some(v) = read_sealed(&wait_path(t), LABEL_LOGIN).and_then(|p| serde_json::from_slice::<Value>(&p).ok()) {
        WAIT_UNTIL.store(v["until"].as_i64().unwrap_or(0), Ordering::SeqCst);
    }
}

fn friendly(e: &InvocationError) -> String {
    if let InvocationError::Rpc(r) = e {
        if r.name.starts_with("FLOOD_WAIT") || r.name.starts_with("FLOOD_PREMIUM_WAIT") {
            if let Some(v) = r.value {
                set_wait(v);
                return "Telegram juda ko'p urinish sababli kirishni vaqtincha to'xtatdi".to_string();
            }
        }
    }
    match rpc_name(e) {
        Some("PHONE_NUMBER_INVALID") => "Telefon raqami noto'g'ri".to_string(),
        Some("PHONE_NUMBER_BANNED") => "Bu raqam Telegram'da bloklangan".to_string(),
        Some("PHONE_NUMBER_FLOOD") => "Bu raqamga juda ko'p kod so'raldi. Telegram kutish vaqtini \
            aytmadi — odatda bir necha soatdan bir kungacha"
            .to_string(),
        Some("FLOOD_WAIT") => "Juda ko'p urinish — birozdan keyin qayta urining".to_string(),
        _ => e.to_string(),
    }
}

/// Kod qayerga yuborildi — ekranda AYNAN shu aytiladi.
///
/// TOPILGAN XATO (foydalanuvchi: "kod yuborildi deyapti, lekin kod
/// umuman kelmayapti"): ilova Telegram javobidagi "kod QAYERGA
/// ketdi" ma'lumotini o'qimasdi va doim "Telegram chatida" deb
/// yozardi. Telegram esa kodni SMS, qo'ng'iroq yoki emailga ham
/// yuboradi, ba'zan esa avval kirish emailini o'rnatishni talab
/// qiladi. Qayta yuborish (`auth.resendCode`) ham yo'q edi.
fn sent_info(sc: &tl::types::auth::SentCode) -> Value {
    use tl::enums::auth::{CodeType, SentCodeType};
    let (via, length, pattern) = match &sc.r#type {
        SentCodeType::App(x) => ("app", x.length, String::new()),
        SentCodeType::Sms(x) => ("sms", x.length, String::new()),
        SentCodeType::SmsWord(_) => ("sms_word", 0, String::new()),
        SentCodeType::SmsPhrase(_) => ("sms_phrase", 0, String::new()),
        SentCodeType::Call(x) => ("call", x.length, String::new()),
        SentCodeType::FlashCall(x) => ("flash_call", 0, x.pattern.clone()),
        SentCodeType::MissedCall(x) => ("missed_call", x.length, x.prefix.clone()),
        SentCodeType::EmailCode(x) => ("email", x.length, x.email_pattern.clone()),
        SentCodeType::SetUpEmailRequired(_) => ("email_setup", 0, String::new()),
        SentCodeType::FragmentSms(x) => ("fragment", x.length, x.url.clone()),
        SentCodeType::FirebaseSms(x) => ("sms", x.length, String::new()),
    };
    let next = match &sc.next_type {
        Some(CodeType::Sms) => "sms",
        Some(CodeType::Call) => "call",
        Some(CodeType::FlashCall) => "flash_call",
        Some(CodeType::MissedCall) => "missed_call",
        Some(CodeType::FragmentSms) => "fragment",
        None => "",
    };
    json!({
        "via": via,
        "length": length,
        "pattern": pattern,
        "next": next,
        "timeout": sc.timeout.unwrap_or(0),
        "at": now_ms(),
    })
}

/// Kod so'rovi natijasi.
enum CodeSent {
    /// Kod yuborildi: (phone_code_hash, qayerga).
    Code(String, Value),
    /// Telegram kodsiz kiritdi (kirish tokeni tanildi).
    LoggedIn,
    /// Kodsiz, lekin 2 bosqichli parol kerak.
    NeedPassword,
}

fn code_result(t: &Tg, res: tl::enums::auth::SentCode) -> Result<CodeSent, String> {
    match res {
        tl::enums::auth::SentCode::Code(c) => {
            if matches!(c.r#type, tl::enums::auth::SentCodeType::SetUpEmailRequired(_)) {
                return Err("Telegram bu raqam uchun avval KIRISH EMAILini o'rnatishni so'rayapti: \
                            Telegram ilovasida Sozlamalar → Maxfiylik va xavfsizlik → \
                            Kirish emaili ni o'rnating, so'ng qayta urining"
                    .to_string());
            }
            let info = sent_info(&c);
            Ok(CodeSent::Code(c.phone_code_hash, info))
        }
        tl::enums::auth::SentCode::Success(x) => match x.authorization {
            tl::enums::auth::Authorization::Authorization(a) => {
                save_token(t, a.future_auth_token.as_ref());
                Ok(CodeSent::LoggedIn)
            }
            tl::enums::auth::Authorization::SignUpRequired(_) => Err(
                "Bu raqamda Telegram hisobi yo'q — avval Telegram ilovasida ro'yxatdan o'ting".to_string(),
            ),
        },
        tl::enums::auth::SentCode::PaymentRequired(_) => Err(
            "Telegram bu raqamga kodni faqat to'lov (Premium) evaziga yuboradi — \
             Telegram ilovasi ochiq bo'lgan boshqa qurilma orqali kiring yoki keyinroq urining"
                .to_string(),
        ),
    }
}

async fn send_code(t: &Tg, client: &Client, phone: &str) -> Result<CodeSent, String> {
    let api_id = t.api_id.lock().map(|v| *v).unwrap_or(0);
    let api_hash = t.api_hash.lock().map(|v| v.clone()).unwrap_or_default();
    let req = tl::functions::auth::SendCode {
        phone_number: phone.to_string(),
        api_id,
        api_hash,
        settings: code_settings(load_tokens(t)),
    };
    let res = match client.invoke(&req).await {
        Err(InvocationError::Rpc(e)) if e.code == 303 => {
            // Raqam boshqa DC ga tegishli — o'sha DC asosiy bo'ladi.
            let dc = e.value.unwrap_or(2) as i32;
            let session = t.session.lock().ok().and_then(|s| s.clone()).ok_or("sessiya yo'q")?;
            session.set_home_dc_id(dc).await.map_err(|e| e.to_string())?;
            client.invoke(&req).await
        }
        other => other,
    };
    match res {
        Ok(r) => code_result(t, r),
        // Kirish tokeni tanildi, lekin hisobda qo'shimcha parol bor.
        Err(e) if e.is("SESSION_PASSWORD_NEEDED") => Ok(CodeSent::NeedPassword),
        Err(e) => Err(friendly(&e)),
    }
}

async fn password_token(client: &Client) -> Result<tl::types::account::Password, String> {
    let tl::enums::account::Password::Password(pw) = client
        .invoke(&tl::functions::account::GetPassword {})
        .await
        .map_err(|e| e.to_string())?;
    Ok(pw)
}

/// Parol bosqichiga o'tadi (ma'lumot olinadi va saqlanadi).
fn enter_password_stage(t: &Tg, client: &Client, phone: &str, hash: &str) -> Result<String, String> {
    let pw = t.rt.block_on(password_token(client))?;
    let hint = pw.hint.clone().unwrap_or_default();
    if let Ok(mut p) = t.password_token.lock() {
        *p = Some(pw);
    }
    save_login(t, "password", phone, hash, &hint);
    Ok(json!({"password": true, "hint": hint}).to_string())
}

/// Parolni SRP bilan tekshiradi (`grammers` ning `check_password`
/// o'rniga: u kirish tokenini tashlab yuboradi va kutilmagan javobda
/// ilovani yiqitardi — `panic`).
enum PwResult {
    Ok,
    Invalid,
}

async fn check_password_srp(
    t: &Tg,
    client: &Client,
    mut pw: tl::types::account::Password,
    password: &str,
) -> Result<PwResult, String> {
    use grammers_crypto::two_factor_auth::{calculate_2fa, check_p_and_g};
    use tl::enums::PasswordKdfAlgo;
    let algo = |pw: &tl::types::account::Password| match &pw.current_algo {
        Some(PasswordKdfAlgo::Sha256Sha256Pbkdf2Hmacsha512iter100000Sha256ModPow(a)) => Some(a.clone()),
        _ => None,
    };
    let mut alg = algo(&pw).ok_or("Bu parol turini ilova qo'llamaydi — Telegram ilovasini yangilang")?;
    if !check_p_and_g(&alg.p, &alg.g) {
        pw = password_token(client).await?;
        alg = algo(&pw).ok_or("Parol ma'lumoti noto'g'ri")?;
        if !check_p_and_g(&alg.p, &alg.g) {
            return Err("Telegram noto'g'ri parol ma'lumoti berdi — qayta urining".to_string());
        }
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
    match client.invoke(&req).await {
        Ok(tl::enums::auth::Authorization::Authorization(a)) => {
            save_token(t, a.future_auth_token.as_ref());
            Ok(PwResult::Ok)
        }
        Ok(tl::enums::auth::Authorization::SignUpRequired(_)) => {
            Err("Bu raqamda Telegram hisobi yo'q".to_string())
        }
        Err(e) if e.is("PASSWORD_HASH_INVALID") => Ok(PwResult::Invalid),
        Err(e) => Err(friendly(&e)),
    }
}

/// Telefon raqamiga kirish kodini yuboradi.
#[no_mangle]
pub extern "C" fn rust_tg_request_code(phone_ptr: *const c_char) -> *mut c_char {
    let phone = unsafe { cstr_to_str(phone_ptr) }.unwrap_or("").trim().to_string();
    string_to_cptr(with_client(|t, client| {
        if phone.len() < 6 {
            return Err("Telefon raqamini kiriting".to_string());
        }
        // Telegram aytgan kutish tugamagan — so'rov umuman ketmaydi
        // (aks holda kutish yana uzayishi mumkin).
        if wait_left() > 0 {
            return Err("Telegram kirishni vaqtincha to'xtatgan — kuting".to_string());
        }
        // ── BIR RAQAMGA TEZ-TEZ KOD SO'RALMASIN ───────────────
        //
        // Har yangi `auth.sendCode` oldingi kodni bekor qiladi, ko'p
        // so'rov esa Telegram'ning cheklovini yoqadi — u "yubordim"
        // deydi-yu, kodni yubormaydi (foydalanuvchi ko'rgan holat,
        // `auth.resendCode` -> SEND_CODE_UNAVAILABLE). Shu raqamga
        // 2 daqiqa ichida kod so'ralgan bo'lsa — yangi so'rov
        // yuborilmaydi, avvalgi kod kutiladi.
        if let Some(st) = load_login(t) {
            let at = st["sent"]["at"].as_i64().unwrap_or(0);
            if st["stage"] == "code"
                && st["phone"].as_str() == Some(phone.as_str())
                && now_ms() - at < CODE_REQUEST_GAP_MS
            {
                return Ok(json!({"ok": true, "sent": st["sent"]}).to_string());
            }
        }
        match t.rt.block_on(send_code(t, &client, &phone))? {
            CodeSent::Code(hash, sent) => {
                save_login_info(t, "code", &phone, &hash, "", &sent);
                Ok(json!({"ok": true, "sent": sent}).to_string())
            }
            CodeSent::LoggedIn => {
                clear_login(t);
                Ok(after_login(t))
            }
            CodeSent::NeedPassword => enter_password_stage(t, &client, &phone, ""),
        }
    }))
}

// ═══════════════════════════════════════════════════════════════
//  QR ORQALI KIRISH (kod kerak emas)
// ═══════════════════════════════════════════════════════════════
//
// TALAB (foydalanuvchi): kirish kodi kelmasa ham kirish imkoni
// bo'lsin. Cherrygram (`LoginActivity.java`) kabi: ilova
// `auth.exportLoginToken` dan token oladi va QR ko'rsatadi
// (`tg://login?token=...`). Boshqa qurilmadagi Telegram'da
// Sozlamalar → Qurilmalar → "Qurilmani ulash" bilan skanerlanadi.
// Telegram tasdiqlagach `updateLoginToken` keladi va token qayta
// so'raladi — bu safar `loginTokenSuccess` (yoki boshqa DC ga
// `loginTokenMigrateTo` -> `auth.importLoginToken`).

fn b64url(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len() * 4 / 3 + 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(A[(n >> 18) as usize & 63] as char);
        out.push(A[(n >> 12) as usize & 63] as char);
        if c.len() > 1 {
            out.push(A[(n >> 6) as usize & 63] as char);
        }
        if c.len() > 2 {
            out.push(A[n as usize & 63] as char);
        }
    }
    out
}

fn login_token_result(t: &Tg, client: &Client, r: tl::enums::auth::LoginToken) -> Result<String, String> {
    match r {
        tl::enums::auth::LoginToken::Token(tok) => {
            let left = (tok.expires as i64 - now_ms() / 1000).clamp(5, 600);
            Ok(json!({"url": format!("tg://login?token={}", b64url(&tok.token)), "expires": left}).to_string())
        }
        tl::enums::auth::LoginToken::Success(x) => match x.authorization {
            tl::enums::auth::Authorization::Authorization(a) => {
                save_token(t, a.future_auth_token.as_ref());
                clear_login(t);
                Ok(after_login(t))
            }
            tl::enums::auth::Authorization::SignUpRequired(_) => {
                Err("Bu hisob Telegram'da ro'yxatdan o'tmagan".to_string())
            }
        },
        tl::enums::auth::LoginToken::MigrateTo(m) => {
            let session = t.session.lock().ok().and_then(|s| s.clone()).ok_or("sessiya yo'q")?;
            t.rt.block_on(session.set_home_dc_id(m.dc_id)).map_err(|e| e.to_string())?;
            match t.rt.block_on(client.invoke(&tl::functions::auth::ImportLoginToken { token: m.token })) {
                Ok(r) => login_token_result(t, client, r),
                Err(e) if e.is("SESSION_PASSWORD_NEEDED") => enter_password_stage(t, client, "", ""),
                Err(e) => Err(friendly(&e)),
            }
        }
    }
}

/// QR uchun yangi token (yoki tasdiqlangan bo'lsa — kirish).
/// Javob: `{"url","expires"}` | `{"ok":true}` | `{"password":true,"hint"}` | `{"error"}`.
#[no_mangle]
pub extern "C" fn rust_tg_qr_token() -> *mut c_char {
    string_to_cptr(with_client(|t, client| {
        QR_ACCEPTED.store(false, Ordering::SeqCst);
        let api_id = t.api_id.lock().map(|v| *v).unwrap_or(0);
        let api_hash = t.api_hash.lock().map(|v| v.clone()).unwrap_or_default();
        let r = t.rt.block_on(client.invoke(&tl::functions::auth::ExportLoginToken {
            api_id,
            api_hash,
            except_ids: Vec::new(),
        }));
        match r {
            Ok(r) => login_token_result(t, &client, r),
            Err(e) if e.is("SESSION_PASSWORD_NEEDED") => enter_password_stage(t, &client, "", ""),
            Err(e) => Err(friendly(&e)),
        }
    }))
}

/// QR tasdiqlandimi (tarmoqsiz). 1 — ha: `rust_tg_qr_token` ni
/// qayta chaqirish kerak.
#[no_mangle]
pub extern "C" fn rust_tg_qr_accepted() -> i32 {
    QR_ACCEPTED.load(Ordering::SeqCst) as i32
}

/// Kodni QAYTA yuboradi (`auth.resendCode`) — odatda keyingi usulda
/// (masalan SMS yoki qo'ng'iroq). Javob: `{"ok":true,"sent":{..}}`.
#[no_mangle]
pub extern "C" fn rust_tg_resend_code() -> *mut c_char {
    string_to_cptr(with_client(|t, client| {
        let st = load_login(t).ok_or("Kod muddati tugadi — raqamni qayta kiriting")?;
        let phone = st["phone"].as_str().unwrap_or("").to_string();
        let hash = st["hash"].as_str().unwrap_or("").to_string();
        let res = t
            .rt
            .block_on(client.invoke(&tl::functions::auth::ResendCode {
                phone_number: phone.clone(),
                phone_code_hash: hash,
                reason: None,
            }))
            .map_err(|e| match rpc_name(&e) {
                Some("SEND_CODE_UNAVAILABLE") => "Telegram bu raqamga kod yuborishni vaqtincha \
                    to'xtatdi. Kutish vaqtini Telegram aytmadi — odatda bir necha soatdan bir \
                    kungacha. Bu orada qayta so'ramang"
                    .to_string(),
                Some("PHONE_CODE_EXPIRED") => "Kod muddati tugadi — raqamni qayta kiriting".to_string(),
                _ => friendly(&e),
            })?;
        match code_result(t, res)? {
            CodeSent::Code(hash, sent) => {
                save_login_info(t, "code", &phone, &hash, "", &sent);
                Ok(json!({"ok": true, "sent": sent}).to_string())
            }
            CodeSent::LoggedIn => {
                clear_login(t);
                Ok(after_login(t))
            }
            CodeSent::NeedPassword => enter_password_stage(t, &client, &phone, ""),
        }
    }))
}

/// Kelgan kod bilan kiradi. Javob: `{"ok":true}`,
/// `{"password":true,"hint":".."}` (2 bosqichli parol kerak) yoki
/// `{"error":".."}`.
#[no_mangle]
pub extern "C" fn rust_tg_sign_in(code_ptr: *const c_char) -> *mut c_char {
    let code = unsafe { cstr_to_str(code_ptr) }.unwrap_or("").trim().to_string();
    string_to_cptr(with_client(|t, client| {
        let st = load_login(t).ok_or("Kod muddati tugadi — raqamni qayta kiriting")?;
        let phone = st["phone"].as_str().unwrap_or("").to_string();
        let hash = st["hash"].as_str().unwrap_or("").to_string();
        let r = t.rt.block_on(client.invoke(&tl::functions::auth::SignIn {
            phone_number: phone.clone(),
            phone_code_hash: hash.clone(),
            phone_code: Some(code),
            email_verification: None,
        }));
        match r {
            Ok(tl::enums::auth::Authorization::Authorization(a)) => {
                save_token(t, a.future_auth_token.as_ref());
                clear_login(t);
                Ok(after_login(t))
            }
            Ok(tl::enums::auth::Authorization::SignUpRequired(_)) => {
                clear_login(t);
                Err("Bu raqamda Telegram hisobi yo'q — avval Telegram ilovasida ro'yxatdan o'ting".to_string())
            }
            Err(e) if e.is("SESSION_PASSWORD_NEEDED") => enter_password_stage(t, &client, &phone, &hash),
            Err(e) if e.is("PHONE_CODE_EXPIRED") => {
                clear_login(t);
                Err("Kod muddati tugadi — raqamni qayta kiriting".to_string())
            }
            Err(e) if e.is("PHONE_CODE_*") => Err("Kod noto'g'ri".to_string()),
            Err(e) => Err(friendly(&e)),
        }
    }))
}

/// 2 bosqichli parolni tekshiradi.
#[no_mangle]
pub extern "C" fn rust_tg_check_password(pw_ptr: *const c_char) -> *mut c_char {
    let pw = unsafe { cstr_to_str(pw_ptr) }.unwrap_or("").to_string();
    string_to_cptr(with_client(|t, client| {
        // Ilova qayta ochilgan bo'lsa parol ma'lumoti xotirada yo'q —
        // Telegram'dan qayta olinadi (kod allaqachon qabul qilingan).
        // Har urinishga YANGI ma'lumot: SRP qiymatlari bir martalik.
        let info = match t.password_token.lock().ok().and_then(|mut p| p.take()) {
            Some(pw) => pw,
            None => t.rt.block_on(password_token(&client))?,
        };
        let hint = info.hint.clone().unwrap_or_default();
        match t.rt.block_on(check_password_srp(t, &client, info, &pw))? {
            PwResult::Ok => {
                clear_login(t);
                Ok(after_login(t))
            }
            PwResult::Invalid => {
                Ok(json!({"password": true, "hint": hint, "error": "Parol noto'g'ri"}).to_string())
            }
        }
    }))
}

/// Saqlangan kirish bosqichi: `{"stage":"phone"|"code"|"password",
/// "phone":"..","hint":".."}` (tarmoqqa chiqmaydi).
#[no_mangle]
pub extern "C" fn rust_tg_login_state() -> *mut c_char {
    let Some(t) = tg() else {
        return string_to_cptr(json!({"stage": "phone"}).to_string());
    };
    if t.authorized.load(Ordering::SeqCst) {
        return string_to_cptr(json!({"stage": "done"}).to_string());
    }
    string_to_cptr(match load_login(t) {
        Some(v) => json!({
            "stage": v["stage"].as_str().unwrap_or("phone"),
            "phone": v["phone"].as_str().unwrap_or(""),
            "hint": v["hint"].as_str().unwrap_or(""),
            "sent": v["sent"].clone(),
            "wait": wait_left(),
        })
        .to_string(),
        None => json!({"stage": "phone", "wait": wait_left()}).to_string(),
    })
}

/// "Raqamni o'zgartirish" — saqlangan bosqich o'chadi.
#[no_mangle]
pub extern "C" fn rust_tg_login_reset() {
    if let Some(t) = tg() {
        clear_login(t);
    }
}

/// BOT CHATINI TOZALAYDI (foydalanuvchi talabi: "pleyerni tark
/// etganda bot tarixni o'chirsin").
///
/// Foydalanuvchining O'Z hisobi bilan (`messages.deleteHistory`,
/// `revoke` — ikkala tomondan) — worker'ga ham, bazaga ham bitta
/// so'rov ketmaydi. Keyingi ko'rishda bot videoni qayta yuboradi.
/// Tarmoq bo'lmasa xato qaytadi va Dart tomoni internet qaytgach
/// qayta uradi.
#[no_mangle]
pub extern "C" fn rust_tg_clear_bot_chat() -> *mut c_char {
    string_to_cptr(with_client(|t, client| {
        if !t.authorized.load(Ordering::SeqCst) {
            return Ok(json!({"ok": true}).to_string());
        }
        t.rt.block_on(async {
            let (id, hash) = bot_peer(t, &client).await?;
            // Katta tarix bir necha qadamda o'chadi (`offset > 0`).
            for _ in 0..20 {
                let tl::enums::messages::AffectedHistory::History(r) = client
                    .invoke(&tl::functions::messages::DeleteHistory {
                        just_clear: false,
                        revoke: true,
                        peer: tl::enums::InputPeer::User(tl::types::InputPeerUser {
                            user_id: id,
                            access_hash: hash,
                        }),
                        max_id: 0,
                        min_date: None,
                        max_date: None,
                    })
                    .await
                    .map_err(|e| e.to_string())?;
                if r.offset <= 0 {
                    break;
                }
            }
            Ok::<(), String>(())
        })?;
        // Eski havolalar endi yaroqsiz.
        if let Ok(mut d) = t.docs.lock() {
            d.clear();
        }
        if let Ok(mut r) = t.routes.lock() {
            r.clear();
        }
        save_routes(t);
        Ok(json!({"ok": true}).to_string())
    }))
}

/// Kirish boti username'i (videolar shu bot chatidan olinadi).
#[no_mangle]
pub extern "C" fn rust_tg_set_bot(bot_ptr: *const c_char) {
    let Some(t) = tg() else { return };
    let bot = unsafe { cstr_to_str(bot_ptr) }.unwrap_or("").trim_start_matches('@').to_string();
    if bot.is_empty() {
        return;
    }
    if let Ok(mut b) = t.bot.lock() {
        if *b != bot {
            *b = bot;
            if let Ok(mut p) = t.bot_peer.lock() {
                *p = None;
            }
        }
    }
}

/// Ilovaga KIRISH: foydalanuvchi nomidan botga `/start <token>`
/// yuboradi (xuddi odam botda START bosgandek).
///
/// Worker'dagi bot orqali kirish tizimi o'zgarmaydi: webhook
/// `/start <token>` ni ko'radi, xabarni yuborgan HAQIQIY Telegram
/// hisobini (`from.id`) oladi va sessiya ochadi. Ya'ni ilova
/// hech narsani "da'vo qilmaydi" — shaxsni Telegram'ning o'zi
/// tasdiqlaydi.
#[no_mangle]
pub extern "C" fn rust_tg_start_bot(bot_ptr: *const c_char, param_ptr: *const c_char) -> *mut c_char {
    let bot = unsafe { cstr_to_str(bot_ptr) }.unwrap_or("").trim_start_matches('@').to_string();
    let param = unsafe { cstr_to_str(param_ptr) }.unwrap_or("").to_string();
    string_to_cptr(with_client(|t, client| {
        if bot.is_empty() || param.is_empty() {
            return Err("bot yoki token berilmagan".to_string());
        }
        t.rt.block_on(async {
            let tl::enums::contacts::ResolvedPeer::Peer(rp) = client
                .invoke(&tl::functions::contacts::ResolveUsername { username: bot.clone(), referer: None })
                .await
                .map_err(|e| e.to_string())?;
            let (id, hash) = rp
                .users
                .iter()
                .find_map(|u| match u {
                    tl::enums::User::User(u) if u.bot => u.access_hash.map(|h| (u.id, h)),
                    _ => None,
                })
                .ok_or("bot topilmadi")?;
            let mut rnd = [0u8; 8];
            getrandom::getrandom(&mut rnd).map_err(|e| e.to_string())?;
            client
                .invoke(&tl::functions::messages::StartBot {
                    bot: tl::enums::InputUser::User(tl::types::InputUser { user_id: id, access_hash: hash }),
                    peer: tl::enums::InputPeer::User(tl::types::InputPeerUser { user_id: id, access_hash: hash }),
                    random_id: i64::from_le_bytes(rnd),
                    start_param: param.clone(),
                })
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"ok": true}).to_string())
        })
    }))
}

/// Foydalanuvchi IP manzili bo'yicha davlat (ISO kodi, `UZ`) —
/// Telegram ilovasi raqam oynasida davlatni aynan shunday tanlaydi
/// (`help.getNearestDc`). Javob: `{"ok":true,"country":"UZ"}`.
#[no_mangle]
pub extern "C" fn rust_tg_nearest_country() -> *mut c_char {
    string_to_cptr(with_client(|t, client| {
        let tl::enums::NearestDc::Dc(dc) = t
            .rt
            .block_on(client.invoke(&tl::functions::help::GetNearestDc {}))
            .map_err(|e| friendly(&e))?;
        Ok(json!({"ok": true, "country": dc.country}).to_string())
    }))
}

// ═══════════════════════════════════════════════════════════════
//  ADMIN: VIDEONI KANALGA YUKLASH
// ═══════════════════════════════════════════════════════════════
//
// TALAB (foydalanuvchi): "telegramga kanalga fayl yuklash tizimi".
//
// Admin qism qo'shish ekranida video tanlaydi va ilova uni ADMINNING
// O'Z Telegram hisobi bilan yopiq kanalga yuklaydi (MTProto, 2 GB
// gacha — Bot API ning 50 MB chegarasi yo'q). Izoh (caption) —
// fayl nomi, ya'ni bot kanal postini ko'rib uni o'zi ham
// ro'yxatga oladi (`tg_channel_post`). Ilova esa kafolat uchun
// natijani worker'ga to'g'ridan-to'g'ri yozadi.
//
// Yuklash fon'da ketadi; Dart holatni `rust_tg_upload_status` bilan
// so'rab turadi (foiz chizig'i).

struct UploadJob {
    sent: Arc<std::sync::atomic::AtomicU64>,
    total: u64,
    state: Arc<Mutex<UploadState>>,
    task: Option<tokio::task::JoinHandle<()>>,
}

#[derive(Default, Clone)]
struct UploadState {
    done: bool,
    msg_id: i32,
    error: Option<String>,
    /// Faylning AES-128-CTR kaliti (hex) — serverga yoziladi.
    key: String,
}

static UPLOADS: OnceLock<Mutex<HashMap<u64, UploadJob>>> = OnceLock::new();
static UPLOAD_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn uploads() -> &'static Mutex<HashMap<u64, UploadJob>> {
    UPLOADS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Kanalni adminning suhbatlari orasidan topadi (yopiq kanalga
/// murojaat uchun `access_hash` kerak, u faqat shu yo'l bilan
/// olinadi).
async fn find_channel(client: &Client, channel_id: i64) -> Result<grammers_client::session::types::PeerRef, String> {
    let mut dialogs = client.iter_dialogs();
    while let Some(d) = dialogs.next().await.map_err(|e| e.to_string())? {
        if d.peer_id().bot_api_dialog_id() == Some(channel_id) {
            return Ok(d.peer_ref());
        }
    }
    Err("Kanal topilmadi — shu Telegram hisobi kanalda admin ekanini tekshiring".to_string())
}

/// MP4 videoning (davomiyligi soniyada, eni, bo'yi) — `moov` dan.
/// Telegram'ga ODDIY VIDEO sifatida yuborish uchun kerak.
fn video_meta(path: &str) -> Option<(f64, i32, i32)> {
    use std::io::{Read, Seek, SeekFrom};
    const MAX_BOXES: usize = 64;
    const MAX_MOOV: u64 = 32 * 1024 * 1024;
    let mut f = fs::File::open(path).ok()?;
    let total = f.metadata().ok()?.len();
    let mut at: u64 = 0;
    for _ in 0..MAX_BOXES {
        if at + 8 > total {
            return None;
        }
        let mut head = vec![0u8; 16.min((total - at) as usize)];
        f.seek(SeekFrom::Start(at)).ok()?;
        f.read_exact(&mut head).ok()?;
        let (body_in_head, raw_len, kind) = crate::mp4::box_header(&head, 0)?;
        let body_start = at + body_in_head as u64;
        let body_len = if raw_len == u64::MAX { total.saturating_sub(body_start) } else { raw_len };
        if &kind == b"moov" {
            if body_len == 0 || body_len > MAX_MOOV {
                return None;
            }
            let mut moov = vec![0u8; body_len as usize];
            f.seek(SeekFrom::Start(body_start)).ok()?;
            f.read_exact(&mut moov).ok()?;
            let t = crate::mp4::parse_moov(&moov)?;
            return Some((t.duration_secs(), t.width as i32, t.height as i32));
        }
        let next = body_start.checked_add(body_len)?;
        if next <= at {
            return None;
        }
        at = next;
    }
    None
}

/// Yuklanadigan faylning Telegram'dagi ko'rinishi.
///
/// SHIFRLANGAN fayl (hozir ilova yuklaydigan HAMMA fayl) — HUJJAT:
/// shifrlangan baytlar surat ham, video ham emas, Telegram ularni
/// ochib ko'rsata olmaydi. Pleyer va rasm keshi ularni o'zi ochadi.
///
/// Quyidagi "oddiy ko'rinish" qoidasi faqat kalitsiz yuklash uchun
/// qoldi.
///
/// TALAB (foydalanuvchi): fayllar HUJJAT sifatida emas, ODDIY
/// ko'rinishda yuborilsin — hujjatni Telegram'da yuklab olib boshqa
/// dastur bilan ochish oson. Video — oddiy (oqimli) video. Fayl nomi
/// (`Filename`) baribir qo'shiladi: ilova nusxani bot chatidan shu nom
/// bo'yicha topadi (`doc_matches`).
fn media_for(uploaded: tl::enums::InputFile, path: &str, name: &str, mime: &str, encrypted: bool) -> tl::enums::InputMedia {
    if encrypted {
        return tl::enums::InputMedia::UploadedDocument(tl::types::InputMediaUploadedDocument {
            nosound_video: false,
            force_file: true,
            spoiler: false,
            file: uploaded,
            thumb: None,
            mime_type: "application/octet-stream".to_string(),
            attributes: vec![tl::enums::DocumentAttribute::Filename(
                tl::types::DocumentAttributeFilename { file_name: name.to_string() },
            )],
            stickers: None,
            video_cover: None,
            video_timestamp: None,
            ttl_seconds: None,
        });
    }
    // Rasm — oddiy SURAT (izoh = nom, `doc_matches` shu bo'yicha topadi).
    if mime.starts_with("image/") {
        return tl::enums::InputMedia::UploadedPhoto(tl::types::InputMediaUploadedPhoto {
            spoiler: false,
            live_photo: false,
            file: uploaded,
            stickers: None,
            ttl_seconds: None,
            video: None,
        });
    }
    let mut attributes = vec![tl::enums::DocumentAttribute::Filename(
        tl::types::DocumentAttributeFilename { file_name: name.to_string() },
    )];
    if mime.starts_with("video/") {
        let (duration, w, h) = video_meta(path).unwrap_or((0.0, 0, 0));
        attributes.push(tl::enums::DocumentAttribute::Video(tl::types::DocumentAttributeVideo {
            round_message: false,
            supports_streaming: true,
            nosound: false,
            duration,
            w,
            h,
            preload_prefix_size: None,
            video_start_ts: None,
            video_codec: None,
        }));
    }
    tl::enums::InputMedia::UploadedDocument(tl::types::InputMediaUploadedDocument {
        nosound_video: false,
        force_file: false,
        spoiler: false,
        file: uploaded,
        thumb: None,
        mime_type: mime.to_string(),
        attributes,
        stickers: None,
        video_cover: None,
        video_timestamp: None,
        ttl_seconds: None,
    })
}

async fn upload_to_channel(
    t: &'static Tg,
    client: Client,
    path: String,
    name: String,
    mime: String,
    channel_id: i64,
    sent: Arc<std::sync::atomic::AtomicU64>,
    total: u64,
    key: [u8; 16],
) -> Result<i32, String> {
    // `channel_id == 0` — BOT CHATIGA (kanalga admin bo'lmagan
    // foydalanuvchi: yozishmadagi katta video). Bot uni o'zi kanalga
    // ko'chiradi (worker'dagi `tg_user_media`).
    let mut peer: tl::enums::InputPeer = if channel_id != 0 {
        find_channel(&client, channel_id).await?.into()
    } else {
        let (id, hash) = bot_peer(t, &client).await?;
        tl::enums::InputPeer::User(tl::types::InputPeerUser { user_id: id, access_hash: hash })
    };
    let uploaded = upload_parts(t, &path, &name, total, key, &sent).await?;
    // Izoh: 1-qator — fayl nomi (bot postni shu bo'yicha taniydi),
    // 2-qator — ochish kaliti. Bot kanal postini ko'rib kalitni
    // O'ZI yozadi (`tg_channel_post` / `tg_user_media`), ya'ni
    // ilova keyingi so'rovni yubora olmasa ham fayl ishlaydi.
    // Foydalanuvchilarga izohsiz nusxa boradi (`remove_caption`).
    let caption = format!("{name}\nkey:{}", hex::encode(key));
    let mut rnd = [0u8; 8];
    getrandom::getrandom(&mut rnd).map_err(|e| e.to_string())?;
    let random_id = i64::from_le_bytes(rnd);
    let mut req = tl::functions::messages::SendMedia {
        silent: true,
        background: false,
        clear_draft: false,
        noforwards: false,
        update_stickersets_order: false,
        invert_media: false,
        allow_paid_floodskip: false,
        peer: peer.clone(),
        reply_to: None,
        media: media_for(uploaded, &path, &name, &mime, true),
        message: caption,
        random_id,
        reply_markup: None,
        entities: None,
        schedule_date: None,
        schedule_repeat_period: None,
        send_as: None,
        quick_reply_shortcut: None,
        effect: None,
        allow_paid_stars: None,
        suggested_post: None,
    };
    // ── POST YUBORISH: QAYTA URINISH BILAN ─────────────────────
    //
    // Fayl allaqachon Telegram serverida — bu bosqich yiqilsa
    // butun faylni qaytadan yuklash kerak EMAS. Xuddi o'sha
    // `random_id` bilan qayta yuboriladi: birinchi urinish aslida
    // o'tib, faqat javobi yo'qolgan bo'lsa Telegram
    // RANDOM_ID_DUPLICATE deydi va post chatdan nomi bo'yicha
    // topiladi — ikki marta joylanmaydi.
    let mut wait = Duration::from_secs(2);
    let mut last_err = String::new();
    let mut retried_peer = false;
    for _ in 0..UP_ATTEMPTS {
        let Some(client) = connect(t) else { return Err("Telegram'ga ulanib bo'lmadi".to_string()) };
        match tokio::time::timeout(UP_TIMEOUT, client.invoke(&req)).await {
            Ok(Ok(res)) => {
                return msg_id_of(&res, random_id).ok_or_else(|| "xabar raqami olinmadi".to_string());
            }
            Ok(Err(e)) if e.is("RANDOM_ID_DUPLICATE") => {
                return find_posted(&client, &peer, &name).await;
            }
            // Bot manzili eskirgan (masalan boshqa hisobniki) — bot
            // qaytadan topiladi va AYNAN o'sha fayl qayta yuboriladi
            // (fayl allaqachon Telegram serverida, qayta yuklanmaydi).
            Ok(Err(e)) if channel_id == 0 && e.is("PEER_ID_INVALID") && !retried_peer => {
                retried_peer = true;
                if let Ok(mut p) = t.bot_peer.lock() {
                    *p = None;
                }
                let (id, hash) = bot_peer(t, &client).await?;
                peer = tl::enums::InputPeer::User(tl::types::InputPeerUser { user_id: id, access_hash: hash });
                req.peer = peer.clone();
                continue;
            }
            Ok(Err(e)) if is_transient(&e) => last_err = e.to_string(),
            Ok(Err(e)) => return Err(format!("yuborilmadi: {e}")),
            Err(_) => {
                last_err = "vaqt tugadi".to_string();
                drop_stale_connection(t);
            }
        }
        tokio::time::sleep(wait).await;
        wait = (wait * 2).min(Duration::from_secs(30));
    }
    Err(format!("yuborilmadi: {last_err}"))
}

/// Bitta qism hajmi (Telegram: 1 MiB ni qoldiqsiz bo'ladi).
const UP_PART: u64 = 512 * 1024;
/// Parallel yuboriladigan qismlar.
const UP_WORKERS: usize = 8;
/// Bitta qism (yoki post) uchun urinishlar: ~2 daqiqa kutish.
const UP_ATTEMPTS: u32 = 8;
/// Bitta so'rov javobini kutish chegarasi.
const UP_TIMEOUT: Duration = Duration::from_secs(60);
/// Telegram qoidasi: shundan katta fayl "katta fayl" usulida.
const UP_BIG: u64 = 10 * 1024 * 1024;

/// ── FAYLNI QISMLAB YUKLASH (har qism — qayta urinish bilan) ───
///
/// TOPILGAN MUAMMO (foydalanuvchi: "video to'liq yuklab bo'lingach
/// internet sekinlashsa, yuklanmadi deb qayta yuklatyapti"):
///   * foiz fayldan O'QILGAN baytlar bo'yicha sanalardi — ekranda
///     100% turganda oxirgi qismlar hali havoda bo'lardi;
///   * `upload_stream` bitta qism xatosida BUTUN yuklashni bekor
///     qilardi va hammasi boshidan ketardi.
///
/// Endi har bir qism o'zi alohida qayta uriniladi (tarmoq xatosi,
/// FLOOD_WAIT, javob kelmasa), foiz esa Telegram QABUL QILGAN
/// baytlar bo'yicha sanaladi. Baytlar yo'lda AES-128-CTR bilan
/// shifrlanadi.
async fn upload_parts(
    t: &'static Tg,
    path: &str,
    name: &str,
    total: u64,
    key: [u8; 16],
    sent: &Arc<std::sync::atomic::AtomicU64>,
) -> Result<tl::enums::InputFile, String> {
    use std::sync::atomic::AtomicU32;
    let parts = total.div_ceil(UP_PART) as i32;
    let big = total > UP_BIG;
    let mut rnd = [0u8; 8];
    getrandom::getrandom(&mut rnd).map_err(|e| e.to_string())?;
    let file_id = i64::from_le_bytes(rnd);
    let next = Arc::new(AtomicU32::new(0));
    let failed = Arc::new(AtomicBool::new(false));
    let mut jobs = Vec::new();
    for _ in 0..UP_WORKERS.min(parts.max(1) as usize) {
        let (next, failed, sent) = (Arc::clone(&next), Arc::clone(&failed), Arc::clone(sent));
        let path = path.to_string();
        jobs.push(t.rt.spawn(async move {
            let mut f = tokio::fs::File::open(&path).await.map_err(|e| format!("fayl ochilmadi: {e}"))?;
            loop {
                if failed.load(Ordering::SeqCst) {
                    return Ok(());
                }
                let part = next.fetch_add(1, Ordering::SeqCst) as i32;
                if part >= parts {
                    return Ok(());
                }
                let off = part as u64 * UP_PART;
                let len = (total - off).min(UP_PART) as usize;
                let mut buf = vec![0u8; len];
                use tokio::io::AsyncSeekExt;
                f.seek(std::io::SeekFrom::Start(off)).await.map_err(|e| e.to_string())?;
                f.read_exact(&mut buf).await.map_err(|e| format!("fayl o'qilmadi: {e}"))?;
                ctr_apply(&key, off, &mut buf);
                if let Err(e) = save_part(t, file_id, part, parts, big, buf).await {
                    failed.store(true, Ordering::SeqCst);
                    return Err(e);
                }
                sent.fetch_add(len as u64, Ordering::Relaxed);
            }
        }));
    }
    for j in jobs {
        match j.await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(format!("yuklashda xato: {e}")),
            Err(e) => return Err(format!("yuklashda xato: {e}")),
        }
    }
    Ok(if big {
        tl::types::InputFileBig { id: file_id, parts, name: name.to_string() }.into()
    } else {
        tl::types::InputFile { id: file_id, parts, name: name.to_string(), md5_checksum: String::new() }.into()
    })
}

/// Bitta qismni yuboradi; vaqtinchalik xatoda kutib qayta uradi.
async fn save_part(t: &'static Tg, file_id: i64, part: i32, parts: i32, big: bool, bytes: Vec<u8>) -> Result<(), String> {
    let mut wait = Duration::from_secs(1);
    let mut last_err = String::new();
    for _ in 0..UP_ATTEMPTS {
        let Some(main) = connect(t) else { return Err("Telegram'ga ulanib bo'lmadi".to_string()) };
        // Qismlar ham bir nechta ulanishga taqsimlanadi (`DL_CONNS`).
        let home = t.session.lock().ok().and_then(|s| s.as_ref().and_then(|s| s.home_dc_id().ok()));
        let client = match home {
            Some(dc) => {
                mark_dc_ready(t, dc);
                part_client(t, &main, dc)
            }
            None => main,
        };
        let r = if big {
            let req = tl::functions::upload::SaveBigFilePart {
                file_id,
                file_part: part,
                file_total_parts: parts,
                bytes: bytes.clone(),
            };
            tokio::time::timeout(UP_TIMEOUT, client.invoke(&req)).await
        } else {
            let req = tl::functions::upload::SaveFilePart { file_id, file_part: part, bytes: bytes.clone() };
            tokio::time::timeout(UP_TIMEOUT, client.invoke(&req)).await
        };
        match r {
            Ok(Ok(true)) => return Ok(()),
            Ok(Ok(false)) => last_err = "server qismni saqlamadi".to_string(),
            Ok(Err(e)) if is_transient(&e) => {
                // FLOOD_WAIT_N — aynan N soniya kutiladi.
                if let InvocationError::Rpc(r) = &e {
                    if r.name.starts_with("FLOOD_WAIT") {
                        if let Some(v) = r.value {
                            wait = Duration::from_secs(v as u64 + 1);
                        }
                    }
                }
                last_err = e.to_string();
            }
            Ok(Err(e)) => return Err(e.to_string()),
            Err(_) => {
                last_err = "vaqt tugadi".to_string();
                drop_stale_connection(t);
            }
        }
        tokio::time::sleep(wait).await;
        wait = (wait * 2).min(Duration::from_secs(30));
    }
    Err(format!("qism #{part}: {last_err}"))
}

/// Post allaqachon yuborilgan (RANDOM_ID_DUPLICATE) — raqamini chat
/// tarixidan fayl nomi bo'yicha topadi.
async fn find_posted(client: &Client, peer: &tl::enums::InputPeer, name: &str) -> Result<i32, String> {
    let res = client
        .invoke(&tl::functions::messages::GetHistory {
            peer: peer.clone(),
            offset_id: 0,
            offset_date: 0,
            add_offset: 0,
            limit: 30,
            max_id: 0,
            min_id: 0,
            hash: 0,
        })
        .await
        .map_err(|e| e.to_string())?;
    let messages = match res {
        tl::enums::messages::Messages::Messages(m) => m.messages,
        tl::enums::messages::Messages::Slice(m) => m.messages,
        tl::enums::messages::Messages::ChannelMessages(m) => m.messages,
        tl::enums::messages::Messages::NotModified(_) => Vec::new(),
    };
    messages
        .iter()
        .find_map(|m| match m {
            tl::enums::Message::Message(m) if doc_matches(m, name).is_some() => Some(m.id),
            _ => None,
        })
        .ok_or_else(|| "post topilmadi".to_string())
}

/// `SendMedia` javobidan yangi xabarning raqamini oladi.
fn msg_id_of(res: &tl::enums::Updates, random_id: i64) -> Option<i32> {
    let updates: &[tl::enums::Update] = match res {
        tl::enums::Updates::Updates(u) => &u.updates,
        tl::enums::Updates::Combined(u) => &u.updates,
        tl::enums::Updates::UpdateShort(u) => std::slice::from_ref(&u.update),
        _ => &[],
    };
    let mut fallback = None;
    for u in updates {
        match u {
            tl::enums::Update::MessageId(m) if m.random_id == random_id => return Some(m.id),
            tl::enums::Update::NewChannelMessage(m) => {
                if let tl::enums::Message::Message(m) = &m.message {
                    fallback = Some(m.id);
                }
            }
            _ => {}
        }
    }
    fallback
}

/// Yuklashni boshlaydi. Javob: `{"job": N}` yoki `{"error": ".."}`.
#[no_mangle]
pub extern "C" fn rust_tg_upload_start(
    path_ptr: *const c_char,
    name_ptr: *const c_char,
    mime_ptr: *const c_char,
    channel_id: i64,
) -> *mut c_char {
    let path = unsafe { cstr_to_str(path_ptr) }.unwrap_or("").to_string();
    let name = unsafe { cstr_to_str(name_ptr) }.unwrap_or("").to_string();
    let mime = unsafe { cstr_to_str(mime_ptr) }.unwrap_or("").to_string();
    string_to_cptr(with_client(|t, client| {
        if !t.authorized.load(Ordering::SeqCst) {
            return Err("Avval Telegram hisobini ulang".to_string());
        }
        if name.is_empty() || name.contains('/') {
            return Err("Fayl nomi noto'g'ri".to_string());
        }
        let total = fs::metadata(&path).map_err(|e| format!("fayl topilmadi: {e}"))?.len();
        if total == 0 {
            return Err("Fayl bo'sh".to_string());
        }
        // Telegram chegarasi: 2 GB (Premium hisobda 4 GB).
        if total > 4000 * 1024 * 1024 {
            return Err("Fayl juda katta (4 GB dan oshmasin)".to_string());
        }
        let id = UPLOAD_SEQ.fetch_add(1, Ordering::SeqCst);
        let sent = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let state = Arc::new(Mutex::new(UploadState::default()));
        let st = Arc::clone(&state);
        let s2 = Arc::clone(&sent);
        let mime = if mime.is_empty() { "video/mp4".to_string() } else { mime };
        // Har faylga yangi tasodifiy kalit.
        let mut key = [0u8; 16];
        getrandom::getrandom(&mut key).map_err(|e| e.to_string())?;
        if let Ok(mut s) = state.lock() {
            s.key = hex::encode(key);
        }
        // Yuklovchining o'zi ham faylni darhol ocha olsin.
        if let Ok(mut k) = t.keys.lock() {
            k.insert(name.clone(), key);
        }
        save_keys(t);
        let task = t.rt.spawn(async move {
            let r = upload_to_channel(t, client, path, name, mime, channel_id, s2, total, key).await;
            if let Err(e) = &r {
                check_dead(t, e);
            }
            if let Ok(mut s) = st.lock() {
                s.done = true;
                match r {
                    Ok(m) => s.msg_id = m,
                    Err(e) => s.error = Some(e),
                }
            }
        });
        if let Ok(mut m) = uploads().lock() {
            m.insert(id, UploadJob { sent, total, state, task: Some(task) });
        }
        Ok(json!({"job": id}).to_string())
    }))
}

/// Yuklash holati: `{"sent","total","done","msg_id","error"}`.
/// Tugagan ish shu chaqiruvda ro'yxatdan o'chiriladi.
#[no_mangle]
pub extern "C" fn rust_tg_upload_status(job: u64) -> *mut c_char {
    let Ok(mut m) = uploads().lock() else {
        return string_to_cptr(err_json("holat qulfi"));
    };
    let Some(j) = m.get(&job) else {
        return string_to_cptr(json!({"done": true, "error": "yuklash topilmadi"}).to_string());
    };
    let st = j.state.lock().map(|s| s.clone()).unwrap_or_default();
    let body = json!({
        "sent": j.sent.load(Ordering::Relaxed).min(j.total),
        "total": j.total,
        "done": st.done,
        "msg_id": st.msg_id,
        "error": st.error,
        "key": st.key,
    });
    if st.done {
        m.remove(&job);
    }
    string_to_cptr(body.to_string())
}

/// Yuklashni bekor qiladi.
#[no_mangle]
pub extern "C" fn rust_tg_upload_cancel(job: u64) {
    if let Ok(mut m) = uploads().lock() {
        if let Some(mut j) = m.remove(&job) {
            if let Some(t) = j.task.take() {
                t.abort();
            }
        }
    }
}

/// Telegram hisobidan chiqadi va sessiyani o'chiradi.
#[no_mangle]
pub extern "C" fn rust_tg_logout() -> *mut c_char {
    let Some(t) = tg() else { return string_to_cptr(json!({"ok": true}).to_string()) };
    if t.authorized.load(Ordering::SeqCst) {
        if let Some(client) = connect(t) {
            // Chiqishda ham Telegram kirish tokenini beradi — keyingi
            // kirish kodsiz bo'lishi mumkin (Cherrygram kabi).
            let r = t.rt.block_on(async {
                tokio::time::timeout(Duration::from_secs(10), client.invoke(&tl::functions::auth::LogOut {})).await
            });
            if let Ok(Ok(tl::enums::auth::LoggedOut::Out(o))) = r {
                save_token(t, o.future_auth_token.as_ref());
            }
        }
    }
    if let Ok(s) = t.session.lock() {
        if let Some(s) = s.as_ref() {
            s.wipe();
        }
    }
    let _ = fs::remove_file(t.dir.join("session.bin"));
    disconnect(t);
    t.authorized.store(false, Ordering::SeqCst);
    forget_account_state(t);
    if let Ok(mut r) = t.routes.lock() {
        r.clear();
    }
    let _ = fs::remove_file(t.dir.join("routes.bin"));
    save_config(t);
    string_to_cptr(json!({"ok": true}).to_string())
}

/// Fayl nomini bot chatidagi xabarga bog'laydi (`msg_id <= 0` —
/// bog'lanishni olib tashlaydi). Yuklab olish shundan keyin
/// Telegram'dan ketadi.
#[no_mangle]
pub extern "C" fn rust_tg_route(key_ptr: *const c_char, msg_id: i32) -> i32 {
    let Some(t) = tg() else { return 0 };
    let Some(key) = (unsafe { cstr_to_str(key_ptr) }) else { return 0 };
    if key.is_empty() || key.contains('/') {
        return 0;
    }
    // Xabar almashdi yoki olib tashlandi — eski fayl havolasi endi
    // yaroqsiz (bot xabarni o'chirgan bo'lishi mumkin).
    if let Ok(mut d) = t.docs.lock() {
        d.remove(key);
    }
    if let Ok(mut r) = t.routes.lock() {
        if msg_id > 0 {
            if r.get(key) == Some(&msg_id) {
                return 1;
            }
            r.insert(key.to_string(), msg_id);
        } else {
            r.remove(key);
        }
    }
    if let Ok(mut f) = t.failed.lock() {
        f.remove(key);
    }
    save_routes(t);
    1
}

/// Fayllarning ochish kalitlari (`/api/tg/deliver` javobidagi
/// `keys`: `{"nom": "hex", ...}`).
#[no_mangle]
pub extern "C" fn rust_tg_set_keys(json_ptr: *const c_char) {
    let Some(t) = tg() else { return };
    let Some(json) = (unsafe { cstr_to_str(json_ptr) }) else { return };
    if let Ok(Value::Object(m)) = serde_json::from_str::<Value>(json) {
        add_keys(t, &m);
    }
}

/// Pleyer uchun manzil (Telegram ishlatib bo'lmasa bo'sh satr).
#[no_mangle]
pub extern "C" fn rust_tg_play_url(key_ptr: *const c_char) -> *mut c_char {
    let key = unsafe { cstr_to_str(key_ptr) }.unwrap_or("");
    string_to_cptr(route_url(key).unwrap_or_default())
}

// ═══════════════════════════════════════════════════════════════
//  STIKERLAR, MAXSUS EMOJI, GIF (Telegram'dagidek yozish paneli)
// ═══════════════════════════════════════════════════════════════
//
// TALAB (foydalanuvchi): "izoh va support chatga Telegram emoji, GIF
// va stikerlarni ulab ber — Telegram'dagi pastki panel qanday
// ishlasa xuddi shunday; premium emoji'ni faqat premium'i bor odam
// yubora olsin".
//
// Hammasi foydalanuvchining O'Z Telegram hisobi bilan olinadi —
// worker ham, baza ham fayl ko'rmaydi:
//   * stiker xabarda `stk_<to'plam>_<hash>_<hujjat>` havolasi bo'lib
//     turadi, ko'ruvchi uni `messages.getStickerSet` bilan oladi;
//   * maxsus emoji matnda hujjat ID si bilan, ko'ruvchi
//     `messages.getCustomEmojiDocuments` bilan oladi;
//   * GIF'ni boshqalar ID bo'yicha ololmaydi — u bot chatiga TAYYOR
//     hujjat sifatida yuboriladi (qayta yuklanmaydi) va bot uni
//     kanalga ko'chiradi (`rust_tg_send_gif`), keyin xuddi video kabi.

#[derive(Clone)]
struct MediaDoc {
    id: i64,
    access_hash: i64,
    file_reference: Vec<u8>,
    dc_id: i32,
    /// Eng mos kichik rasm turi (`m`) — GIF va video stikerlar uchun.
    thumb: Option<String>,
    /// Qayerdan olingan: fayl havolasi eskirsa qaytadan shu yerdan.
    set: Option<(i64, i64)>,
    emoji: bool,
}

fn media_docs() -> &'static Mutex<HashMap<i64, MediaDoc>> {
    static M: OnceLock<Mutex<HashMap<i64, MediaDoc>>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(HashMap::new()))
}

/// To'plam ichidagi hujjatlar (xotirada — panel har ochilganda
/// qaytadan so'ralmasin).
fn set_cache() -> &'static Mutex<HashMap<i64, Vec<Value>>> {
    static M: OnceLock<Mutex<HashMap<i64, Vec<Value>>>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(HashMap::new()))
}

fn media_dir(t: &Tg) -> PathBuf {
    let d = t.dir.join("media");
    let _ = fs::create_dir_all(&d);
    d
}

/// Hujjatni eslab qoladi va ilovaga kerakli qisqa ko'rinishini qaytaradi.
fn doc_value(d: &tl::enums::Document, set: Option<(i64, i64)>) -> Option<Value> {
    let tl::enums::Document::Document(d) = d else { return None };
    let mut alt = String::new();
    let mut set = set;
    let mut emoji = false;
    let mut free = true;
    let (mut w, mut h) = (0, 0);
    for a in &d.attributes {
        match a {
            tl::enums::DocumentAttribute::Sticker(s) => {
                alt = s.alt.clone();
                if let tl::enums::InputStickerSet::Id(i) = &s.stickerset {
                    set = Some((i.id, i.access_hash));
                }
            }
            tl::enums::DocumentAttribute::CustomEmoji(c) => {
                alt = c.alt.clone();
                emoji = true;
                free = c.free;
                if let tl::enums::InputStickerSet::Id(i) = &c.stickerset {
                    set = Some((i.id, i.access_hash));
                }
            }
            tl::enums::DocumentAttribute::Video(v) => {
                w = v.w;
                h = v.h;
            }
            tl::enums::DocumentAttribute::ImageSize(s) => {
                w = s.w;
                h = s.h;
            }
            _ => {}
        }
    }
    let kind = match d.mime_type.as_str() {
        "application/x-tgsticker" => "tgs",
        "image/webp" => "webp",
        "video/webm" => "webm",
        "video/mp4" => "mp4",
        m if m.starts_with("image/") => "image",
        _ => return None,
    };
    // Kichik rasm: `m` (320px) bo'lsa o'sha, bo'lmasa eng kattasi.
    let thumb = d.thumbs.as_ref().and_then(|v| {
        let sizes: Vec<(String, i32)> = v
            .iter()
            .filter_map(|p| match p {
                tl::enums::PhotoSize::Size(s) => Some((s.r#type.clone(), s.w)),
                tl::enums::PhotoSize::Progressive(s) => Some((s.r#type.clone(), s.w)),
                _ => None,
            })
            .collect();
        sizes
            .iter()
            .find(|(k, _)| k == "m")
            .or_else(|| sizes.iter().max_by_key(|(_, w)| *w))
            .map(|(k, _)| k.clone())
    });
    let md = MediaDoc {
        id: d.id,
        access_hash: d.access_hash,
        file_reference: d.file_reference.clone(),
        dc_id: d.dc_id,
        thumb: thumb.clone(),
        set,
        emoji,
    };
    if let Ok(mut m) = media_docs().lock() {
        m.insert(d.id, md);
    }
    let (sid, shash) = set.unwrap_or((0, 0));
    Some(json!({
        // 64 bitli sonlar matn ko'rinishida (Dart'da ham aniq qolsin).
        "id": d.id.to_string(),
        "kind": kind,
        "emoji": alt,
        "set_id": sid.to_string(),
        "set_hash": shash.to_string(),
        "w": w,
        "h": h,
        "thumb": thumb.is_some(),
        "custom": emoji,
        "free": free,
    }))
}

fn docs_value(docs: &[tl::enums::Document], set: Option<(i64, i64)>) -> Vec<Value> {
    docs.iter().filter_map(|d| doc_value(d, set)).collect()
}

fn sets_value(sets: &[tl::enums::StickerSet]) -> Vec<Value> {
    sets.iter()
        .map(|s| {
            let tl::enums::StickerSet::Set(s) = s;
            json!({
                "id": s.id.to_string(),
                "hash": s.access_hash.to_string(),
                "title": s.title,
                "count": s.count,
                "thumb_doc": s.thumb_document_id.map(|v| v.to_string()),
            })
        })
        .collect()
}

fn parse_i64(v: &Value) -> i64 {
    match v {
        Value::String(s) => s.parse().unwrap_or(0),
        v => v.as_i64().unwrap_or(0),
    }
}

// ── DISK KESHI (Telegram'dagidek) ────────────────────────────
//
// TALAB (foydalanuvchi): "emoji, stiker va giflar xotiraga yuklansin
// va keyingi safar qayta yuklanmasin — huddi Telegram'nikidek".
//
// Telegram Android (`MediaDataController`) to'plamlar ro'yxatini va
// har to'plamning hujjatlarini bazada saqlaydi, keyingi so'rovda esa
// oxirgi `hash` ni yuboradi: o'zgarmagan bo'lsa server `NotModified`
// qaytaradi va hech narsa qayta yuklanmaydi. Bu yerda ham xuddi shu:
//   * javob + hash + hujjat havolalari (`MediaDoc`) —
//     `tg/media/meta/<nom>.json`;
//   * yangi yozuv [fresh] muddatda umuman tarmoqqa chiqmaydi;
//   * eskirgani `hash` bilan tekshiriladi, tarmoq xato bersa —
//     keshdagisi qaytadi (internetsiz ham panel ochiladi);
//   * fayllarning o'zi `tg/media/<id>` da (bir marta yuklanadi).

fn meta_path(t: &Tg, name: &str) -> PathBuf {
    let d = media_dir(t).join("meta");
    let _ = fs::create_dir_all(&d);
    d.join(format!("{name}.json"))
}

fn md_json(m: &MediaDoc) -> Value {
    json!({
        "id": m.id.to_string(),
        "ah": m.access_hash.to_string(),
        "fr": hex::encode(&m.file_reference),
        "dc": m.dc_id,
        "th": m.thumb,
        "s": m.set.map(|(a, b)| [a.to_string(), b.to_string()]),
        "e": m.emoji,
    })
}

fn md_parse(v: &Value) -> Option<MediaDoc> {
    Some(MediaDoc {
        id: parse_i64(&v["id"]),
        access_hash: parse_i64(&v["ah"]),
        file_reference: hex::decode(v["fr"].as_str()?).ok()?,
        dc_id: v["dc"].as_i64()? as i32,
        thumb: v["th"].as_str().map(str::to_string),
        set: v["s"].as_array().map(|a| (parse_i64(&a[0]), parse_i64(&a[1]))),
        emoji: v["e"].as_bool().unwrap_or(false),
    })
}

/// Javob ichidagi hamma hujjat ID'lari (`{"id":..,"kind":..}`).
fn doc_ids(v: &Value, out: &mut Vec<i64>) {
    match v {
        Value::Array(a) => a.iter().for_each(|x| doc_ids(x, out)),
        Value::Object(o) => {
            if o.contains_key("kind") {
                out.push(parse_i64(&v["id"]));
            } else {
                o.values().for_each(|x| doc_ids(x, out));
            }
        }
        _ => {}
    }
}

/// Keshdan o'qiydi va hujjat havolalarini xotiraga qaytaradi.
/// `(yozuv, yoshi)`.
fn meta_load(t: &Tg, name: &str) -> Option<(Value, Duration)> {
    let p = meta_path(t, name);
    let age = fs::metadata(&p).ok()?.modified().ok()?.elapsed().unwrap_or_default();
    let v: Value = serde_json::from_slice(&fs::read(&p).ok()?).ok()?;
    if let (Some(mds), Ok(mut m)) = (v["mds"].as_array(), media_docs().lock()) {
        for md in mds.iter().filter_map(md_parse) {
            m.entry(md.id).or_insert(md);
        }
    }
    Some((v, age))
}

fn meta_save(t: &Tg, name: &str, resp: &Value, hash: &Value) {
    let mut ids = Vec::new();
    doc_ids(resp, &mut ids);
    let mds: Vec<Value> = match media_docs().lock() {
        Ok(m) => ids.iter().filter_map(|id| m.get(id)).map(md_json).collect(),
        Err(_) => Vec::new(),
    };
    let p = meta_path(t, name);
    let tmp = p.with_extension("part");
    let body = json!({"hash": hash, "resp": resp, "mds": mds}).to_string();
    if fs::write(&tmp, body).is_ok() {
        let _ = fs::rename(&tmp, &p);
    }
}

/// Keshdagi javob yangi bo'lsa — o'sha; aks holda [fetch] (u eski
/// yozuvni oladi: `hash` va `NotModified` bo'lsa eski javob uchun).
/// Tarmoq xato bersa — eski javob.
fn cached<F>(t: &Tg, name: &str, fresh: Duration, fetch: F) -> Result<String, String>
where
    F: FnOnce(Option<&Value>) -> Result<(Value, Value), String>,
{
    let old = meta_load(t, name);
    if let Some((v, age)) = &old {
        if *age < fresh {
            return Ok(v["resp"].to_string());
        }
    }
    match fetch(old.as_ref().map(|(v, _)| v)) {
        Ok((resp, hash)) => {
            meta_save(t, name, &resp, &hash);
            Ok(resp.to_string())
        }
        Err(e) => old.map(|(v, _)| v["resp"].to_string()).ok_or(e),
    }
}

/// Faqat keshdan (ulanishsiz) — Telegram ishga tushmagan yoki
/// ulanib bo'lmaganda ham panel ochilsin.
fn cached_only(name: &str) -> Option<String> {
    let t = tg()?;
    meta_load(t, name).map(|(v, _)| v["resp"].to_string())
}

fn hash_of(old: Option<&Value>, key: &str) -> i64 {
    old.map(|v| parse_i64(&v["hash"][key])).unwrap_or(0)
}

fn old_resp(old: Option<&Value>, key: &str) -> Value {
    old.map(|v| v["resp"][key].clone()).unwrap_or(json!([]))
}

/// Keshdan yoki tarmoqdan: ulanish bo'lmasa ham kesh qaytadi.
fn with_cache<F>(name: &str, f: F) -> String
where
    F: FnOnce(&'static Tg, Client) -> Result<String, String>,
{
    let r = with_client(f);
    if r.contains("\"error\"") && serde_json::from_str::<Value>(&r).map(|v| v["error"].is_string()).unwrap_or(false) {
        if let Some(c) = cached_only(name) {
            return c;
        }
    }
    r
}

/// Telegram so'rovi vaqt chegarasi bilan: javob kelmasa panel
/// cheksiz "aylanib" qolmasin, xato matni ko'rinsin.
fn run_tmo<T>(t: &Tg, secs: u64, f: impl std::future::Future<Output = Result<T, String>>) -> Result<T, String> {
    t.rt.block_on(async {
        tokio::time::timeout(Duration::from_secs(secs), f)
            .await
            .map_err(|_| format!("{NET_ERR}Telegram {secs} soniyada javob bermadi"))?
    })
}

/// Foydalanuvchida Telegram Premium bormi (`{"premium": bool}`).
#[no_mangle]
pub extern "C" fn rust_tg_premium() -> *mut c_char {
    string_to_cptr(with_cache("premium", |t, client| {
        cached(t, "premium", Duration::from_secs(3600), |_| {
            let users = run_tmo(t, 20, async { client.invoke(&tl::functions::users::GetUsers {
                    id: vec![tl::enums::InputUser::UserSelf],
                }).await.map_err(|e| inv_err(&e)) })?;
            let premium = users.iter().any(|u| matches!(u, tl::enums::User::User(u) if u.premium));
            Ok((json!({"premium": premium}), json!({})))
        })
    }))
}

/// O'rnatilgan stiker to'plamlari + yaqinda ishlatilgan va sevimli
/// stikerlar. [emoji] = 1 — maxsus emoji to'plamlari.
#[no_mangle]
pub extern "C" fn rust_tg_sticker_sets(emoji: i32) -> *mut c_char {
    let name = if emoji != 0 { "sets_emoji" } else { "sets_stickers" };
    string_to_cptr(with_cache(name, |t, client| {
        cached(t, name, Duration::from_secs(600), |old| {
            run_tmo(t, 20, async {
                let h = hash_of(old, "all");
                let all = if emoji != 0 {
                    client.invoke(&tl::functions::messages::GetEmojiStickers { hash: h }).await
                } else {
                    client.invoke(&tl::functions::messages::GetAllStickers { hash: h }).await
                }
                .map_err(|e| inv_err(&e))?;
                let (sets, all_hash) = match all {
                    tl::enums::messages::AllStickers::Stickers(a) => (json!(sets_value(&a.sets)), a.hash),
                    tl::enums::messages::AllStickers::NotModified => (old_resp(old, "sets"), h),
                };
                if emoji != 0 {
                    return Ok((json!({"sets": sets}), json!({"all": all_hash.to_string()})));
                }
                let rh = hash_of(old, "recent");
                let (recent, rh) = match client
                    .invoke(&tl::functions::messages::GetRecentStickers { attached: false, hash: rh })
                    .await
                {
                    Ok(tl::enums::messages::RecentStickers::Stickers(r)) => (json!(docs_value(&r.stickers, None)), r.hash),
                    _ => (old_resp(old, "recent"), rh),
                };
                let fh = hash_of(old, "faved");
                let (faved, fh) = match client.invoke(&tl::functions::messages::GetFavedStickers { hash: fh }).await {
                    Ok(tl::enums::messages::FavedStickers::Stickers(r)) => (json!(docs_value(&r.stickers, None)), r.hash),
                    _ => (old_resp(old, "faved"), fh),
                };
                Ok((
                    json!({"sets": sets, "recent": recent, "faved": faved}),
                    json!({"all": all_hash.to_string(), "recent": rh.to_string(), "faved": fh.to_string()}),
                ))
            })
        })
    }))
}

async fn load_set_hashed(client: &Client, id: i64, hash: i64, known: i32) -> Result<Option<(Vec<Value>, i32)>, String> {
    let r = client
        .invoke(&tl::functions::messages::GetStickerSet {
            stickerset: tl::enums::InputStickerSet::Id(tl::types::InputStickerSetId { id, access_hash: hash }),
            hash: known,
        })
        .await
        .map_err(|e| inv_err(&e))?;
    match r {
        tl::enums::messages::StickerSet::Set(s) => {
            let tl::enums::StickerSet::Set(info) = &s.set;
            let docs = docs_value(&s.documents, Some((id, hash)));
            if let Ok(mut c) = set_cache().lock() {
                c.insert(id, docs.clone());
            }
            Ok(Some((docs, info.hash)))
        }
        tl::enums::messages::StickerSet::NotModified => Ok(None),
    }
}

async fn load_set(client: &Client, id: i64, hash: i64) -> Result<Vec<Value>, String> {
    Ok(load_set_hashed(client, id, hash, 0).await?.map(|(d, _)| d).unwrap_or_default())
}

/// Bitta to'plamning stikerlari: `{"id":"..","hash":".."}` -> `{"docs":[..]}`.
#[no_mangle]
pub extern "C" fn rust_tg_sticker_set(json_ptr: *const c_char) -> *mut c_char {
    let arg: Value = serde_json::from_str(unsafe { cstr_to_str(json_ptr) }.unwrap_or("{}")).unwrap_or(json!({}));
    let (id, hash) = (parse_i64(&arg["id"]), parse_i64(&arg["hash"]));
    let name = format!("set_{id}");
    string_to_cptr(with_cache(&name, |t, client| {
        if let Some(d) = set_cache().lock().ok().and_then(|c| c.get(&id).cloned()) {
            return Ok(json!({"docs": d}).to_string());
        }
        // To'plam ichi deyarli o'zgarmaydi — bir kun tekshirilmaydi.
        cached(t, &name, Duration::from_secs(24 * 3600), |old| {
            let known = old.map(|v| parse_i64(&v["hash"]["h"]) as i32).unwrap_or(0);
            match run_tmo(t, 20, load_set_hashed(&client, id, hash, known))? {
                Some((docs, h)) => Ok((json!({"docs": docs}), json!({"h": h.to_string()}))),
                None => Ok((json!({"docs": old_resp(old, "docs")}), json!({"h": known.to_string()}))),
            }
        })
    }))
}

/// Maxsus emoji hujjatlari ID bo'yicha: `["id", ..]` -> `{"docs":[..]}`.
///
/// Har bir emoji bir marta so'raladi va `custom_emoji` keshida qoladi.
#[no_mangle]
pub extern "C" fn rust_tg_custom_emoji(json_ptr: *const c_char) -> *mut c_char {
    let arg: Value = serde_json::from_str(unsafe { cstr_to_str(json_ptr) }.unwrap_or("[]")).unwrap_or(json!([]));
    let ids: Vec<i64> = arg.as_array().map(|a| a.iter().map(parse_i64).filter(|v| *v != 0).collect()).unwrap_or_default();
    if ids.is_empty() {
        return string_to_cptr(json!({"docs": []}).to_string());
    }
    let Some(t) = tg() else { return string_to_cptr(err_json("Telegram ishga tushmagan")) };
    let mut known: serde_json::Map<String, Value> = meta_load(t, "custom_emoji")
        .and_then(|(v, _)| v["resp"].as_object().cloned())
        .unwrap_or_default();
    let missing: Vec<i64> = ids.iter().copied().filter(|id| !known.contains_key(&id.to_string())).collect();
    if !missing.is_empty() {
        let r = with_client(|t, client| {
            let docs = run_tmo(t, 20, async { client.invoke(&tl::functions::messages::GetCustomEmojiDocuments { document_id: missing.clone() }).await.map_err(|e| inv_err(&e)) })?;
            Ok::<_, String>(json!(docs_value(&docs, None)).to_string())
        });
        match serde_json::from_str::<Value>(&r) {
            Ok(Value::Array(docs)) => {
                for d in docs {
                    if let Some(id) = d["id"].as_str() {
                        known.insert(id.to_string(), d);
                    }
                }
                let resp = Value::Object(known.clone());
                meta_save(t, "custom_emoji", &resp, &json!({}));
            }
            _ if ids.iter().all(|id| !known.contains_key(&id.to_string())) => return string_to_cptr(r),
            _ => {}
        }
    }
    let docs: Vec<Value> = ids.iter().filter_map(|id| known.get(&id.to_string()).cloned()).collect();
    string_to_cptr(json!({"docs": docs}).to_string())
}

/// Saqlangan GIF'lar.
#[no_mangle]
pub extern "C" fn rust_tg_saved_gifs() -> *mut c_char {
    string_to_cptr(with_cache("saved_gifs", |t, client| {
        cached(t, "saved_gifs", Duration::from_secs(600), |old| {
            let h = hash_of(old, "h");
            let r = run_tmo(t, 20, async { client.invoke(&tl::functions::messages::GetSavedGifs { hash: h }).await.map_err(|e| inv_err(&e)) })?;
            Ok(match r {
                tl::enums::messages::SavedGifs::Gifs(g) => {
                    (json!({"docs": docs_value(&g.gifs, None)}), json!({"h": g.hash.to_string()}))
                }
                tl::enums::messages::SavedGifs::NotModified => (json!({"docs": old_resp(old, "docs")}), json!({"h": h.to_string()})),
            })
        })
    }))
}

/// GIF qidiruvi — Telegram'dagi kabi `@gif` bot orqali.
/// `{"q":"..","offset":".."}` -> `{"docs":[..],"next":".."}`.
#[no_mangle]
pub extern "C" fn rust_tg_gif_search(json_ptr: *const c_char) -> *mut c_char {
    let arg: Value = serde_json::from_str(unsafe { cstr_to_str(json_ptr) }.unwrap_or("{}")).unwrap_or(json!({}));
    let q = arg["q"].as_str().unwrap_or("").to_string();
    let offset = arg["offset"].as_str().unwrap_or("").to_string();
    string_to_cptr(with_client(|t, client| {
        run_tmo(t, 20, async {
            static GIF_BOT: OnceLock<Mutex<Option<(i64, i64)>>> = OnceLock::new();
            let cell = GIF_BOT.get_or_init(|| Mutex::new(None));
            let cached = cell.lock().ok().and_then(|c| *c);
            let (id, hash) = match cached {
                Some(p) => p,
                None => {
                    let tl::enums::contacts::ResolvedPeer::Peer(rp) = client
                        .invoke(&tl::functions::contacts::ResolveUsername { username: "gif".into(), referer: None })
                        .await
                        .map_err(|e| inv_err(&e))?;
                    let p = rp
                        .users
                        .iter()
                        .find_map(|u| match u {
                            tl::enums::User::User(u) if u.bot => u.access_hash.map(|h| (u.id, h)),
                            _ => None,
                        })
                        .ok_or("@gif topilmadi")?;
                    if let Ok(mut c) = cell.lock() {
                        *c = Some(p);
                    }
                    p
                }
            };
            let tl::enums::messages::BotResults::Results(r) = client
                .invoke(&tl::functions::messages::GetInlineBotResults {
                    bot: tl::enums::InputUser::User(tl::types::InputUser { user_id: id, access_hash: hash }),
                    peer: tl::enums::InputPeer::PeerSelf,
                    geo_point: None,
                    query: q.clone(),
                    offset: offset.clone(),
                })
                .await
                .map_err(|e| inv_err(&e))?;
            let docs: Vec<Value> = r
                .results
                .iter()
                .filter_map(|x| match x {
                    tl::enums::BotInlineResult::BotInlineMediaResult(m) => m.document.as_ref(),
                    _ => None,
                })
                .filter_map(|d| doc_value(d, None))
                .collect();
            Ok(json!({"docs": docs, "next": r.next_offset.unwrap_or_default()}).to_string())
        })
    }))
}

/// Emoji bo'yicha stikerlar (Telegram'dagi qidiruv qatoridagi ❤️ 👍 …
/// tugmalari): `{"q":"❤"}` -> `{"docs":[..]}`.
#[no_mangle]
pub extern "C" fn rust_tg_stickers_by_emoji(json_ptr: *const c_char) -> *mut c_char {
    let arg: Value = serde_json::from_str(unsafe { cstr_to_str(json_ptr) }.unwrap_or("{}")).unwrap_or(json!({}));
    let q = arg["q"].as_str().unwrap_or("").to_string();
    let name = format!("by_emoji_{}", hex::encode(q.as_bytes()));
    string_to_cptr(with_cache(&name, |t, client| {
        cached(t, &name, Duration::from_secs(3600), |old| {
            let h = hash_of(old, "h");
            let r = run_tmo(t, 20, async {
                client
                    .invoke(&tl::functions::messages::GetStickers { emoticon: q.clone(), hash: h })
                    .await
                    .map_err(|e| inv_err(&e))
            })?;
            Ok(match r {
                tl::enums::messages::Stickers::Stickers(s) => {
                    (json!({"docs": docs_value(&s.stickers, None)}), json!({"h": s.hash.to_string()}))
                }
                tl::enums::messages::Stickers::NotModified => (json!({"docs": old_resp(old, "docs")}), json!({"h": h.to_string()})),
            })
        })
    }))
}

/// Telegram'ning emoji kalit so'zlari (`messages.getEmojiKeywords`) —
/// emoji qidiruvi uchun. Bir kun keshlanadi.
/// `{"lang":"ru"}` -> `{"k":{"so'z":["😀",..],..}}`.
#[no_mangle]
pub extern "C" fn rust_tg_emoji_keywords(json_ptr: *const c_char) -> *mut c_char {
    let arg: Value = serde_json::from_str(unsafe { cstr_to_str(json_ptr) }.unwrap_or("{}")).unwrap_or(json!({}));
    let lang = arg["lang"].as_str().unwrap_or("en").to_string();
    let name = format!("emoji_kw_{lang}");
    string_to_cptr(with_cache(&name, |t, client| {
        cached(t, &name, Duration::from_secs(24 * 3600), |_| {
            let r = run_tmo(t, 25, async {
                client
                    .invoke(&tl::functions::messages::GetEmojiKeywords { lang_code: lang.clone() })
                    .await
                    .map_err(|e| inv_err(&e))
            })?;
            let tl::enums::EmojiKeywordsDifference::Difference(d) = r;
            let mut k = serde_json::Map::new();
            for w in d.keywords {
                if let tl::enums::EmojiKeyword::Keyword(w) = w {
                    k.insert(w.keyword, json!(w.emoticons));
                }
            }
            Ok((json!({"k": k}), json!({})))
        })
    }))
}

/// Fayl havolasi eskirgan — hujjat qaytadan olinadi.
async fn refresh_media(client: &Client, md: &MediaDoc) -> Result<MediaDoc, String> {
    if md.emoji {
        let docs = client
            .invoke(&tl::functions::messages::GetCustomEmojiDocuments { document_id: vec![md.id] })
            .await
            .map_err(|e| inv_err(&e))?;
        docs_value(&docs, None);
    } else if let Some((id, hash)) = md.set {
        load_set(client, id, hash).await?;
    } else {
        return Err("fayl havolasi eskirgan".to_string());
    }
    media_docs()
        .lock()
        .ok()
        .and_then(|m| m.get(&md.id).cloned())
        .ok_or_else(|| "hujjat topilmadi".to_string())
}

/// Hujjatni (yoki uning kichik rasmini) to'liq yuklab oladi.
async fn download_media(t: &Tg, client: &Client, mut md: MediaDoc, thumb: bool) -> Result<Vec<u8>, String> {
    const MAX: u64 = 20 * 1024 * 1024;
    let mut out: Vec<u8> = Vec::new();
    let mut dc = md.dc_id;
    let mut refreshed = false;
    let mut offset: u64 = 0;
    let mut tries = 0;
    loop {
        let req = tl::functions::upload::GetFile {
            precise: false,
            cdn_supported: false,
            location: tl::enums::InputFileLocation::InputDocumentFileLocation(tl::types::InputDocumentFileLocation {
                id: md.id,
                access_hash: md.access_hash,
                file_reference: md.file_reference.clone(),
                thumb_size: if thumb { md.thumb.clone().unwrap_or_default() } else { String::new() },
            }),
            offset: offset as i64,
            limit: PART as i32,
        };
        match client.invoke_in_dc(dc, &req).await {
            Ok(tl::enums::upload::File::File(f)) => {
                let n = f.bytes.len() as u64;
                out.extend_from_slice(&f.bytes);
                offset += n;
                if n < PART || offset >= MAX {
                    return Ok(out);
                }
            }
            Ok(tl::enums::upload::File::CdnRedirect(_)) => return Err("CDN yo'naltirishi kutilmagan".to_string()),
            Err(e) => {
                tries += 1;
                if tries > 4 {
                    return Err(inv_err(&e));
                }
                match rpc_name(&e) {
                    Some(n) if n.starts_with("FILE_REFERENCE_") && !refreshed => {
                        refreshed = true;
                        md = refresh_media(client, &md).await?;
                    }
                    Some("AUTH_KEY_UNREGISTERED") => copy_auth(t, client, dc).await?,
                    Some("FILE_MIGRATE") => {
                        if let InvocationError::Rpc(r) = &e {
                            if let Some(v) = r.value {
                                dc = v as i32;
                            }
                        }
                    }
                    _ => return Err(inv_err(&e)),
                }
            }
        }
    }
}

/// Hujjat faylining diskdagi yo'li (bir marta yuklanadi, keyin
/// keshdan). `{"id":"..","thumb":true}` -> `{"path":".."}`.
#[no_mangle]
pub extern "C" fn rust_tg_media_file(json_ptr: *const c_char) -> *mut c_char {
    let arg: Value = serde_json::from_str(unsafe { cstr_to_str(json_ptr) }.unwrap_or("{}")).unwrap_or(json!({}));
    let id = parse_i64(&arg["id"]);
    let thumb = arg["thumb"].as_bool().unwrap_or(false);
    // Diskda bor bo'lsa — ulanishsiz, darhol (qayta yuklanmaydi).
    if let Some(t) = tg() {
        let path = media_dir(t).join(format!("{id}{}", if thumb { ".t" } else { "" }));
        if path.exists() {
            return string_to_cptr(json!({"path": path.to_string_lossy()}).to_string());
        }
    }
    string_to_cptr(with_client(|t, client| {
        let path = media_dir(t).join(format!("{id}{}", if thumb { ".t" } else { "" }));
        if path.exists() {
            return Ok(json!({"path": path.to_string_lossy()}).to_string());
        }
        let known = media_docs().lock().ok().and_then(|m| m.get(&id).cloned());
        let md = match known {
            Some(md) => md,
            // Xotira oynasida kesh (`tg/media/meta` bilan birga)
            // tozalangach ilova qayta ochilsa, hujjat havolasi
            // xotirada bo'lmaydi. "Yaqinda ishlatilgan" maxsus
            // emojilar to'plam ochilmasdan so'raladi — ular ID
            // bo'yicha qaytadan olinadi (stiker bo'lsa ro'yxat bo'sh
            // qaytadi va xato avvalgidek).
            None => {
                let docs = run_tmo(t, 20, async {
                    client
                        .invoke(&tl::functions::messages::GetCustomEmojiDocuments { document_id: vec![id] })
                        .await
                        .map_err(|e| inv_err(&e))
                })?;
                docs_value(&docs, None);
                media_docs()
                    .lock()
                    .ok()
                    .and_then(|m| m.get(&id).cloned())
                    .ok_or("hujjat noma'lum")?
            }
        };
        if thumb && md.thumb.is_none() {
            return Err("kichik rasm yo'q".to_string());
        }
        let bytes = t.rt.block_on(async {
            tokio::time::timeout(Duration::from_secs(30), download_media(t, &client, md, thumb))
                .await
                .map_err(|_| format!("{NET_ERR}vaqt tugadi"))?
        })?;
        let tmp = path.with_extension("part");
        fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
        fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
        Ok(json!({"path": path.to_string_lossy()}).to_string())
    }))
}

/// GIF'ni bot chatiga TAYYOR hujjat sifatida yuboradi (qayta
/// yuklanmaydi). Izoh — fayl nomi: bot uni kanalga ko'chiradi
/// (`tg_user_media`). `{"id":"..","name":".."}` -> `{"ok":true}`.
#[no_mangle]
pub extern "C" fn rust_tg_send_gif(json_ptr: *const c_char) -> *mut c_char {
    let arg: Value = serde_json::from_str(unsafe { cstr_to_str(json_ptr) }.unwrap_or("{}")).unwrap_or(json!({}));
    let id = parse_i64(&arg["id"]);
    let name = arg["name"].as_str().unwrap_or("").to_string();
    string_to_cptr(with_client(|t, client| {
        let md = media_docs()
            .lock()
            .ok()
            .and_then(|m| m.get(&id).cloned())
            .ok_or("GIF noma'lum")?;
        t.rt.block_on(async {
            let mut rnd = [0u8; 8];
            getrandom::getrandom(&mut rnd).map_err(|e| e.to_string())?;
            let mut retried = false;
            loop {
                let (uid, hash) = bot_peer(t, &client).await?;
                let r = client
                    .invoke(&tl::functions::messages::SendMedia {
                        silent: true,
                        background: false,
                        clear_draft: false,
                        noforwards: false,
                        update_stickersets_order: false,
                        invert_media: false,
                        allow_paid_floodskip: false,
                        peer: tl::enums::InputPeer::User(tl::types::InputPeerUser { user_id: uid, access_hash: hash }),
                        reply_to: None,
                        media: tl::enums::InputMedia::Document(tl::types::InputMediaDocument {
                            spoiler: false,
                            id: tl::enums::InputDocument::Document(tl::types::InputDocument {
                                id: md.id,
                                access_hash: md.access_hash,
                                file_reference: md.file_reference.clone(),
                            }),
                            video_cover: None,
                            video_timestamp: None,
                            ttl_seconds: None,
                            query: None,
                        }),
                        message: name.clone(),
                        random_id: i64::from_le_bytes(rnd),
                        reply_markup: None,
                        entities: None,
                        schedule_date: None,
                        schedule_repeat_period: None,
                        send_as: None,
                        quick_reply_shortcut: None,
                        effect: None,
                        allow_paid_stars: None,
                        suggested_post: None,
                    })
                    .await;
                match r {
                    Ok(_) => return Ok(json!({"ok": true}).to_string()),
                    Err(e) if e.is("PEER_ID_INVALID") && !retried => {
                        retried = true;
                        if let Ok(mut p) = t.bot_peer.lock() {
                            *p = None;
                        }
                    }
                    Err(e) => return Err(inv_err(&e)),
                }
            }
        })
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oraliq_tahlili() {
        assert_eq!(parse_range("bytes=0-99", 1000), Some((0, 99)));
        assert_eq!(parse_range("bytes=900-", 1000), Some((900, 999)));
        assert_eq!(parse_range("bytes=900-5000", 1000), Some((900, 999)));
        assert_eq!(parse_range("bytes=-100", 1000), Some((900, 999)));
        assert_eq!(parse_range("bytes=1000-", 1000), None);
        assert_eq!(parse_range("bytes=5-2", 1000), None);
        assert_eq!(parse_range("items=0-1", 1000), None);
    }

    /// Soxta "Telegram": `offset` dan eng ko'pi `PART` bayt beradi.
    fn fake_file(len: usize) -> Arc<Vec<u8>> {
        Arc::new((0..len).map(|i| (i * 31 % 251) as u8).collect())
    }

    fn run_pump(file: Arc<Vec<u8>>, start: u64, end: u64) -> Vec<u8> {
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
        rt.block_on(async move {
            let mut out: Vec<u8> = Vec::new();
            let f = Arc::clone(&file);
            let spawn = move |off: u64| {
                let f = Arc::clone(&f);
                tokio::spawn(async move {
                    // Qismlar TARTIBSIZ tugasin — tartib pump'da saqlanishini tekshiramiz.
                    tokio::time::sleep(Duration::from_millis((off / PART % 3) * 5)).await;
                    let a = off as usize;
                    let b = (a + PART as usize).min(f.len());
                    Ok(f[a.min(b)..b].to_vec())
                })
            };
            let ok = pump(&mut out, start, end, spawn).await;
            assert!(ok.is_ok());
            out
        })
    }

    #[test]
    fn oqim_sorangan_baytlarni_aniq_beradi() {
        let len = 5 * PART as usize + 12_345;
        let file = fake_file(len);
        for (s, e) in [
            (0u64, len as u64 - 1),
            (1, 2),
            (PART - 1, PART),
            (PART, 3 * PART - 1),
            (777_777, len as u64 - 1),
            (len as u64 - 1, len as u64 - 1),
        ] {
            let got = run_pump(Arc::clone(&file), s, e);
            assert_eq!(got, file[s as usize..=e as usize], "oraliq {s}-{e}");
        }
    }

    #[test]
    fn manba_xatosi_oqimni_toxtatadi() {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let mut out: Vec<u8> = Vec::new();
            let spawn = |off: u64| {
                tokio::spawn(async move {
                    if off >= PART { Err("tarmoq".to_string()) } else { Ok(vec![1u8; PART as usize]) }
                })
            };
            let r = pump(&mut out, 0, 3 * PART - 1, spawn).await;
            assert!(matches!(r, Err(PumpError::Source(_))));
            assert_eq!(out.len(), PART as usize);
        });
    }

    #[test]
    fn yuborilgan_xabar_raqami_topiladi() {
        let res = tl::enums::Updates::Updates(tl::types::Updates {
            updates: vec![tl::enums::Update::MessageId(tl::types::UpdateMessageId { id: 77, random_id: 5 })],
            users: vec![],
            chats: vec![],
            date: 0,
            seq: 0,
        });
        assert_eq!(msg_id_of(&res, 5), Some(77));
        // Boshqa random_id — bu bizning xabar emas.
        assert_eq!(msg_id_of(&res, 6), None);
    }

    #[test]
    fn ctr_istalgan_joydan_ochiladi() {
        let key = [7u8; 16];
        let plain: Vec<u8> = (0..300_000u32).map(|i| (i * 13 % 251) as u8).collect();
        // Yuklashdagidek: butun fayl bir oqimda, mayda bo'laklab.
        let mut enc = plain.clone();
        let mut pos = 0usize;
        for step in [1usize, 15, 16, 17, 4096, 100_000] {
            let end = (pos + step).min(enc.len());
            ctr_apply(&key, pos as u64, &mut enc[pos..end]);
            pos = end;
        }
        ctr_apply(&key, pos as u64, &mut enc[pos..]);
        assert_ne!(enc, plain);
        // O'qishdagidek: faqat kerakli oraliq, istalgan joydan.
        for (a, b) in [(0usize, 10usize), (5, 37), (131_071, 131_200), (299_990, 300_000)] {
            let mut part = enc[a..b].to_vec();
            ctr_apply(&key, a as u64, &mut part);
            assert_eq!(part, plain[a..b]);
        }
    }

    #[test]
    fn qr_tokeni_base64url() {
        assert_eq!(b64url(b""), "");
        assert_eq!(b64url(b"f"), "Zg");
        assert_eq!(b64url(b"fo"), "Zm8");
        assert_eq!(b64url(b"foo"), "Zm9v");
        assert_eq!(b64url(&[0xfb, 0xff, 0xfe]), "-__-");
    }

    #[test]
    fn kalit_hexdan_oqiladi() {
        assert_eq!(parse_key("00112233445566778899aabbccddeeff").unwrap()[15], 0xff);
        assert!(parse_key("0011").is_none());
        assert!(parse_key("zz112233445566778899aabbccddeeff").is_none());
    }

    #[test]
    fn qism_chegarasi_telegram_qoidasiga_mos() {
        // limit 4 KiB ga karrali va 1 MiB ni qoldiqsiz bo'ladi.
        assert_eq!(PART % 4096, 0);
        assert_eq!((1024 * 1024) % PART, 0);
        // Har qanday qism boshi PART ga karrali — so'rov 1 MiB
        // chegarasidan o'tmaydi.
        for start in [0u64, 1, 524_287, 524_288, 1_048_575, 7_340_033] {
            let off = start / PART * PART;
            assert_eq!(off / (1024 * 1024), (off + PART - 1) / (1024 * 1024));
        }
    }
}

// ═══════════════════════════════════════════════════════════════
//  PLEYER UCHUN TO'G'RIDAN-TO'G'RI O'QISH (`player_source.rs`)
// ═══════════════════════════════════════════════════════════════
//
// Bular ExoPlayer'ning yuklash oqimidan (tokio'dan tashqarida)
// chaqiriladi va natija kelguncha kutadi.

/// Bitta o'qish uchun kutish chegarasi. Internet uzilsa pleyer
/// abadiy kutib qolmasin: tarmoq xatosi qaytadi va pleyer (Java
/// tomoni) internet qaytishini kutib QAYTA so'raydi.
const READ_TIMEOUT: Duration = Duration::from_secs(20);

/// Kutish chegarasi tugadi — ulanish "yarim o'lik" bo'lishi mumkin
/// (javob ham, xato ham kelmaydi). Keyingi so'rov YANGI ulanish
/// ochsin.
fn drop_stale_connection(t: &Tg) {
    disconnect(t);
}

/// Fayl hajmi va turi (bot chatidan topiladi).
pub(crate) fn doc_size(name: &str) -> Result<(u64, String), String> {
    let t = tg().ok_or("Telegram ishga tushmagan")?;
    if !t.authorized.load(Ordering::SeqCst) {
        return Err("Telegram hisobiga kirilmagan".to_string());
    }
    let client = connect(t).ok_or("Telegram'ga ulanib bo'lmadi")?;
    let name = name.to_string();
    let r = t.rt.block_on(async move {
        tokio::time::timeout(READ_TIMEOUT, doc_for(t, &client, &name, false)).await
    });
    match r {
        Ok(d) => d.map(|d| (d.size, d.mime)),
        Err(_) => {
            drop_stale_connection(t);
            Err(format!("{NET_ERR}vaqt tugadi"))
        }
    }
}

/// Fayl hajmi — faqat XOTIRADAGI ma'lumotdan (tarmoqsiz). Bot
/// chatidagi nusxa bir marta o'qilgach (`fetch_doc`) shu yerda bo'ladi va
/// bu — Telegram'dagi HAQIQIY hajm.
pub(crate) fn cached_doc_size(name: &str) -> Option<(u64, String)> {
    let t = tg()?;
    let d = t.docs.lock().ok()?.get(name).cloned()?;
    (d.size > 0).then(|| (d.size, d.mime))
}

/// `offset` dan `len` bayt (offset `PART` ga karrali). Qismlar
/// parallel so'raladi. Xato tarmoq sababli bo'lsa `NET_ERR` bilan
/// boshlanadi (`is_net_err`).
pub(crate) fn fetch_range(name: &str, offset: u64, len: u64) -> Result<Vec<u8>, String> {
    let t = tg().ok_or("Telegram ishga tushmagan")?;
    let client = connect(t).ok_or("Telegram'ga ulanib bo'lmadi")?;
    let mut jobs = Vec::new();
    let mut off = offset;
    while off < offset + len {
        jobs.push(t.rt.spawn(fetch_part(t, client.clone(), name.to_string(), off)));
        off += PART;
    }
    // Kutish tugasa ham havodagi so'rovlar to'xtatilsin (aks holda
    // ular fon'da ishlab, trafik sarflardi).
    let aborts: Vec<_> = jobs.iter().map(|j| j.abort_handle()).collect();
    let r = t.rt.block_on(async move {
        tokio::time::timeout(READ_TIMEOUT, async move {
            let mut out = Vec::with_capacity(len as usize);
            for j in jobs {
                match j.await {
                    Ok(Ok(b)) => out.extend_from_slice(&b),
                    Ok(Err(e)) => return Err(e),
                    Err(e) => return Err(format!("{NET_ERR}{e}")),
                }
            }
            out.truncate(len as usize);
            Ok(out)
        })
        .await
    });
    let r = match r {
        Ok(r) => r,
        Err(_) => {
            drop_stale_connection(t);
            Err(format!("{NET_ERR}vaqt tugadi"))
        }
    };
    if r.is_err() {
        for a in aborts {
            a.abort();
        }
    }
    if let Err(e) = &r {
        crate::video_cache::tg_log(format!("Telegram: {name} olinmadi: {e}"));
    }
    r
}

#[cfg(test)]
mod media_cache_tests {
    use super::*;

    #[test]
    fn media_doc_json_roundtrip() {
        let m = MediaDoc {
            id: -5_000_000_000_123,
            access_hash: 77,
            file_reference: vec![1, 2, 255],
            dc_id: 4,
            thumb: Some("m".into()),
            set: Some((9, -9)),
            emoji: true,
        };
        let b = md_parse(&md_json(&m)).unwrap();
        assert_eq!((b.id, b.access_hash, b.file_reference, b.dc_id), (m.id, 77, vec![1, 2, 255], 4));
        assert_eq!((b.thumb.as_deref(), b.set, b.emoji), (Some("m"), Some((9, -9)), true));
    }

    #[test]
    fn doc_ids_found_in_nested_response() {
        let v = json!({"sets": [{"id": "1", "title": "x"}],
                       "recent": [{"id": "5", "kind": "tgs"}],
                       "faved": [{"id": "-7", "kind": "webp"}]});
        let mut ids = Vec::new();
        doc_ids(&v, &mut ids);
        ids.sort();
        assert_eq!(ids, vec![-7, 5]);
    }
}
