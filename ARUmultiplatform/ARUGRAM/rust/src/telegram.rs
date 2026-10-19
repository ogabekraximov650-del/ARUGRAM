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

use grammers_client::session::types::{DcOption, PeerAuth, PeerId, PeerInfo, PeerRef, UpdateState, UpdatesState};
use grammers_client::session::{BoxFuture, Session, SessionData};
use grammers_client::{tl, Client, InvocationError, SenderPool};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::runtime::Runtime;
use tokio::sync::Semaphore;

use crate::crypto;
use crate::ffi_utils::{cstr_to_str, string_to_cptr};

mod secret;

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

/// Kelgan yangilanishlar soni (`rust_tg_update_seq`): o'zgarsa — Kotlin ro'yxatni yangilaydi.
static UPDATE_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[no_mangle]
pub extern "C" fn rust_tg_update_seq() -> i64 {
    UPDATE_SEQ.load(Ordering::SeqCst) as i64
}

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
    let secret_client = client.clone();
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
            // Har qanday haqiqiy yangilanish (yangi xabar, o'qildi va h.k.) hisoblagichni oshiradi — ilova shuni tekshirib,
            // ro'yxat/suhbatni darhol yangilaydi (tarmoqqa so'rov yubormasdan, arzon).
            if let UpdatesLike::Updates(up) = &u {
                match up {
                    tl::enums::Updates::UpdateShort(s) => note_update(&s.update),
                    tl::enums::Updates::Updates(s) => s.updates.iter().for_each(note_update),
                    tl::enums::Updates::Combined(s) => s.updates.iter().for_each(note_update),
                    _ => {}
                }
                // Maxfiy chatlar: kalit almashinuvi va shifrlangan xabarlar (`secret.rs`).
                secret::on_updates(&secret_client, up).await;
                if !accepted {
                    UPDATE_SEQ.fetch_add(1, Ordering::SeqCst);
                }
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
    // ASL videolar (`orig_`) — faqat POST IZOHI bo'yicha: kodlash boti
    // orqali kelganlarida fayl nomi Telegram'niki (`video.mp4`), worker
    // ularni izohi bilan yuboradi. Qolganlari (kodlangan sifatlar va
    // h.k.) — faqat FAYL NOMI bo'yicha (foydalanuvchi talabi).
    let found = if name.starts_with("orig_") {
        m.message.lines().next().unwrap_or("").trim() == name
    } else {
        d.attributes.iter().any(|a| {
            matches!(a, tl::enums::DocumentAttribute::Filename(f) if f.file_name == name)
        })
    };
    if !found {
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
    let (Some("tg"), Some(mark), Some(key)) = (seg.next(), seg.next(), seg.next()) else {
        respond(&mut stream, "404 Not Found", "").await;
        return;
    };
    // `t` — tomosha tarixi KADRI uchun o'qish. Uning xatosi faylni
    // "Telegram'da ishlamaydi" deb belgilamasligi kerak.
    //
    // TOPILGAN XATO (foydalanuvchi: "yuklab olish ishlamaydi"): kadr
    // navbati fayl hali bot chatida ko'rinmay turib o'qishga urinsa,
    // `note_failure` faylni 2 daqiqaga Telegram'dan chetlatardi —
    // shu payt boshlangan YUKLAB OLISH worker yo'liga o'tib (u video
    // bermaydi) qotib qolardi. Worker hisobotida aynan shu fayllarga
    // `GET/HEAD /api/image/ep_..._1080p...mp4` so'rovlari ko'rindi.
    let soft = mark == "t";
    let key = key.to_string();
    let Some(client) = connect(t) else {
        respond(&mut stream, "503 Service Unavailable", "").await;
        return;
    };
    let doc = match doc_for(t, &client, &key, false).await {
        Ok(d) => d,
        Err(e) => {
            crate::video_cache::tg_log(format!("Telegram: {key} ochilmadi: {e}"));
            if !soft {
                note_failure(&key);
            }
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
            if !soft {
                note_failure(&key);
            }
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
    // Ichkaridagi kutilmagan `panic` ilovani yopmasin — xato bo'lib
    // qaytadi (sababi `last_crash.txt` ga ham yoziladi).
    let r = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(t, client))) {
        Ok(r) => r,
        Err(p) => {
            let why = p
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| p.downcast_ref::<String>().cloned())
                .unwrap_or_default();
            Err(format!("ichki xato: {why}"))
        }
    };
    match r {
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
                // Yangi hisob: ilova ism so'raydi, keyin `rust_tg_sign_up`.
                Ok(json!({"signup": true}).to_string())
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
        // Ulanish osilib qolsa tozalash (va uni kutayotgan fayl
        // so'rovlari) abadiy turib qolmasin — ko'pi 40 soniya.
        run_tmo(t, 40, async {
            let (id, hash) = bot_peer(t, &client).await?;
            // OXIRGI XABAR QOLADI (foydalanuvchi: "chat to'liq tozalanganda bot chatlar
            // ro'yxatidan yo'qolyapti"). Web ilovadagidek: `GetHistory` eng yangisidan
            // boshlaydi — birinchisini (eng yangi xabar) tashlab, qolgani aniq raqamlar
            // bilan o'chiriladi (`DeleteMessages`). Avval `DeleteHistory max_id` ishlatilardi,
            // u ba'zan oxirgi xabarni ham o'chirib yuborardi.
            //
            // `revoke` (ikkala tomondan) bot bilan suhbatda ba'zan rad etiladi — shunda
            // faqat o'z tomonidan o'chiriladi. Uzilgan so'rov bir necha marta qayta yuboriladi.
            let peer = tl::enums::InputPeer::User(tl::types::InputPeerUser { user_id: id, access_hash: hash });
            let mut revoke = true;
            let mut tries = 0;
            let mut pages = 0;
            while pages < 30 {
                let r = client
                    .invoke(&tl::functions::messages::GetHistory {
                        peer: peer.clone(),
                        offset_id: 0,
                        offset_date: 0,
                        add_offset: 0,
                        limit: 100,
                        max_id: 0,
                        min_id: 0,
                        hash: 0,
                    })
                    .await;
                let msgs = match r {
                    Ok(r) => messages_of(r),
                    Err(e) if is_transient(&e) && tries < 3 => {
                        tries += 1;
                        tokio::time::sleep(Duration::from_millis(500 * tries as u64)).await;
                        continue;
                    }
                    Err(e) => return Err(inv_err(&e)),
                };
                let total = msgs.len();
                // Eng yangisi — birinchi; u qoladi.
                let mut ids: Vec<i32> = msgs
                    .iter()
                    .map(|m| match m {
                        tl::enums::Message::Message(m) => m.id,
                        tl::enums::Message::Service(m) => m.id,
                        tl::enums::Message::Empty(m) => m.id,
                    })
                    .collect();
                ids.sort_unstable_by(|a, b| b.cmp(a));
                if !ids.is_empty() {
                    ids.remove(0);
                }
                if ids.is_empty() {
                    break;
                }
                let d = client
                    .invoke(&tl::functions::messages::DeleteMessages { revoke, id: ids })
                    .await;
                match d {
                    Ok(_) => {
                        pages += 1;
                        if total < 100 {
                            break;
                        }
                    }
                    Err(e) if is_transient(&e) && tries < 3 => {
                        tries += 1;
                        tokio::time::sleep(Duration::from_millis(500 * tries as u64)).await;
                    }
                    Err(e) if revoke && matches!(&e, InvocationError::Rpc(_)) => {
                        revoke = false;
                    }
                    Err(e) => return Err(inv_err(&e)),
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

// ═══════════════════════════════════════════════════════════════
//  MAJBURIY KANALLARGA QO'SHILISH (foydalanuvchi ruxsati bilan)
// ═══════════════════════════════════════════════════════════════
//
// TALAB (foydalanuvchi): bepul bo'limni ko'rish uchun ilova
// foydalanuvchidan RUXSAT so'raydi va ruxsat berilsa, uning
// Telegram hisobi bilan admin belgilagan kanallarga o'zi qo'shiladi
// (ochiq kanal) yoki qo'shilish so'rovini yuboradi (yopiq kanal).
//
// Bu funksiya FAQAT Dart tomoni foydalanuvchi "Ruxsat berish"ni
// bosgandan keyin chaqiriladi (`channel_gate.dart`) — hech qachon
// o'zi, jimgina emas. Kanaldan CHIQISH yo'q: foydalanuvchi o'zi
// istasa chiqadi.
//
// Natija (`state`): `joined` — a'zo bo'ldi (yoki allaqachon a'zo),
// `requested` — yopiq kanalga qo'shilish so'rovi yuborildi (admin
// tasdiqlaguncha a'zo emas, lekin server buni "bajarildi" deb
// hisoblaydi — majburiy obuna shunday ishlaydi).

/// Ochiq kanal nomi: `@nom`, `nom` yoki `https://t.me/nom` -> `nom`.
/// Telegram qoidasi: 5-32 belgi, lotin harf, raqam va `_`.
fn public_name(raw: &str) -> Option<String> {
    let t = raw.trim();
    let t = t
        .strip_prefix("https://t.me/")
        .or_else(|| t.strip_prefix("http://t.me/"))
        .or_else(|| t.strip_prefix("t.me/"))
        .unwrap_or(t);
    let t = t.trim_start_matches('@').trim_end_matches('/');
    let ok = (5..=32).contains(&t.len())
        && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    ok.then(|| t.to_string())
}

/// Yopiq kanal havolasidan xesh: `https://t.me/+AbC`, `t.me/joinchat/AbC`.
fn invite_hash(raw: &str) -> Option<String> {
    let t = raw.trim().trim_end_matches('/');
    let last = t.rsplit('/').next()?;
    let h = last.trim_start_matches('+');
    let ok = h.len() >= 8
        && h.len() <= 64
        && h.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    ok.then(|| h.to_string())
}

async fn join_one(client: &Client, kind: &str, target: &str) -> Result<&'static str, String> {
    match kind {
        "public" => {
            let name = public_name(target).ok_or("kanal nomi noto'g'ri")?;
            let tl::enums::contacts::ResolvedPeer::Peer(rp) = client
                .invoke(&tl::functions::contacts::ResolveUsername { username: name, referer: None })
                .await
                .map_err(|e| inv_err(&e))?;
            let (id, hash) = rp
                .chats
                .iter()
                .find_map(|c| match c {
                    tl::enums::Chat::Channel(c) => c.access_hash.map(|h| (c.id, h)),
                    _ => None,
                })
                .ok_or("kanal topilmadi")?;
            let r = client
                .invoke(&tl::functions::channels::JoinChannel {
                    channel: tl::enums::InputChannel::Channel(tl::types::InputChannel {
                        channel_id: id,
                        access_hash: hash,
                    }),
                })
                .await;
            match r {
                Ok(_) => Ok("joined"),
                Err(InvocationError::Rpc(r)) if r.name == "USER_ALREADY_PARTICIPANT" => Ok("joined"),
                Err(InvocationError::Rpc(r)) if r.name == "INVITE_REQUEST_SENT" => Ok("requested"),
                Err(e) => Err(inv_err(&e)),
            }
        }
        "private" => {
            let hash = invite_hash(target).ok_or("havola noto'g'ri")?;
            // Allaqachon a'zo bo'lsa qayta so'rov yuborilmaydi.
            match client
                .invoke(&tl::functions::messages::CheckChatInvite { hash: hash.clone() })
                .await
            {
                Ok(tl::enums::ChatInvite::Already(_)) => return Ok("joined"),
                Ok(_) => {}
                Err(e) => return Err(inv_err(&e)),
            }
            match client
                .invoke(&tl::functions::messages::ImportChatInvite { hash })
                .await
            {
                Ok(_) => Ok("joined"),
                Err(InvocationError::Rpc(r)) if r.name == "USER_ALREADY_PARTICIPANT" => Ok("joined"),
                // Qo'shilish so'rovi yuborildi — majburiy obuna uchun yetarli.
                Err(InvocationError::Rpc(r)) if r.name == "INVITE_REQUEST_SENT" => Ok("requested"),
                Err(e) => Err(inv_err(&e)),
            }
        }
        _ => Err("kanal turi noto'g'ri".to_string()),
    }
}

/// Foydalanuvchining Telegram hisobi bilan BITTA kanalga qo'shiladi.
///
/// Argument: `<tur>:<manzil>` — `public:https://t.me/nom` yoki
/// `private:https://t.me/+xesh`. Javob: `{"ok":true,"state":"joined|requested"}`
/// yoki `{"error":...}`. Tarmoqqa chiqadi (BLOKLAYDI) — Dart uni
/// fon isolate'ida chaqiradi (`tgCall`).
#[no_mangle]
pub extern "C" fn rust_tg_join_channel(arg_ptr: *const c_char) -> *mut c_char {
    let arg = unsafe { cstr_to_str(arg_ptr) }.unwrap_or("");
    let (kind, target) = arg.split_once(':').unwrap_or(("", ""));
    let (kind, target) = (kind.to_string(), target.to_string());
    string_to_cptr(with_client(|t, client| {
        if !t.authorized.load(Ordering::SeqCst) {
            return Err("Telegram ulanmagan".to_string());
        }
        let state = run_tmo(t, 30, join_one(&client, &kind, &target))?;
        Ok(json!({"ok": true, "state": state}).to_string())
    }))
}

/// Admin paneli — MAJBURIY KANAL QO'SHISHDAN OLDIN TEKSHIRISH.
///
/// TALAB (foydalanuvchi): "kanal ID yoki useri yozilsa avval ilova kanal
/// bor-yo'qligini tekshirsin; mavjud bo'lsa surati, nomi, username,
/// obunachilar va hokazo chiqsin". Adminning O'Z Telegram hisobi bilan:
///   * `@nom` / `t.me/nom` — `contacts.resolveUsername`;
///   * `-100...` — adminning suhbatlari orasidan (`find_channel`);
///   * `t.me/+xesh` (yopiq havola) — `messages.checkChatInvite` (a'zo
///     bo'lmasa ham nomi, surati, obunachilar soni keladi).
/// Ochiq/yopiq kanal uchun obunachilar va tavsif `channels.getFullChannel`
/// dan. Surat — kichik (160 px) JPEG, base64 bilan (diskka YOZILMAYDI).
///
/// Javob: `{"ok":true,"id":-100..,"title","username","about","members",
/// "verified","broadcast","public","member","photo":"<base64>"}` yoki
/// `{"error":...}`.
#[no_mangle]
pub extern "C" fn rust_tg_channel_info(arg_ptr: *const c_char) -> *mut c_char {
    let input = unsafe { cstr_to_str(arg_ptr) }.unwrap_or("").trim().to_string();
    string_to_cptr(with_client(|t, client| {
        if !t.authorized.load(Ordering::SeqCst) {
            return Err("Telegram ulanmagan".to_string());
        }
        let v = run_tmo(t, 30, channel_info(&client, &input))?;
        Ok(v.to_string())
    }))
}

/// Xabardagi ilova haqida ma'lumot (suhbat oynasi uchun).
#[derive(Default)]
struct MediaInfo {
    kind: &'static str,
    label: String,
    w: i32,
    h: i32,
    name: String,
    size: i64,
    dur: f64,
    mime: String,
    alt: String,
    poll: Option<Value>,
    /// Ovozli xabar to'lqin shakli (0..31, odatda 100 ta).
    wave: Vec<u8>,
}

/// Ovozli xabar to'lqin shakli: 5 bitli qiymatlar (little-endian) -> 0..31.
fn doc_waveform(d: &grammers_client::media::Document) -> Vec<u8> {
    let Some(tl::enums::Document::Document(doc)) = &d.raw.document else { return Vec::new() };
    for a in &doc.attributes {
        if let tl::enums::DocumentAttribute::Audio(a) = a {
            if let Some(b) = &a.waveform {
                let n = b.len() * 8 / 5;
                return (0..n)
                    .map(|i| {
                        let bit = i * 5;
                        let byte = bit / 8;
                        let shift = bit % 8;
                        let mut v = (b[byte] as u32) >> shift;
                        if shift > 3 && byte + 1 < b.len() {
                            v |= (b[byte + 1] as u32) << (8 - shift);
                        }
                        (v & 31) as u8
                    })
                    .collect();
            }
        }
    }
    Vec::new()
}

fn media_info(m: &grammers_client::media::Media) -> MediaInfo {
    use grammers_client::media::{Media, PhotoSize};
    match m {
        Media::Photo(p) => {
            let (w, h) = p
                .thumbs()
                .iter()
                .filter_map(|t| match t {
                    PhotoSize::Size(sz) => Some((sz.width, sz.height)),
                    _ => None,
                })
                .max_by_key(|(w, h)| w * h)
                .unwrap_or((0, 0));
            MediaInfo { kind: "photo", label: "Rasm".into(), w, h, ..Default::default() }
        }
        Media::Document(d) => {
            let mime = d.mime_type().unwrap_or("").to_string();
            let round = match &d.raw.document {
                Some(tl::enums::Document::Document(doc)) => doc.attributes.iter().any(|a| matches!(a, tl::enums::DocumentAttribute::Video(v) if v.round_message)),
                _ => false,
            };
            let animated = match &d.raw.document {
                Some(tl::enums::Document::Document(doc)) => doc.attributes.iter().any(|a| matches!(a, tl::enums::DocumentAttribute::Animated)),
                _ => false,
            };
            let (kind, label) = if mime.starts_with("video/") && animated {
                ("gif", "GIF")
            } else if mime.starts_with("video/") && round {
                ("round", "Video xabar")
            } else if mime.starts_with("video/") {
                ("video", "Video")
            } else if mime == "audio/ogg" {
                ("voice", "Ovozli xabar")
            } else if mime.starts_with("audio/") {
                ("audio", "Audio")
            } else {
                ("file", "Fayl")
            };
            let (w, h) = d.resolution().unwrap_or((0, 0));
            MediaInfo {
                kind,
                label: label.into(),
                w,
                h,
                name: d.name().unwrap_or("").to_string(),
                size: d.size().unwrap_or(0) as i64,
                dur: d.duration().unwrap_or(0.0),
                mime,
                alt: String::new(),
                poll: None,
                wave: doc_waveform(d),
            }
        }
        Media::Sticker(s) => {
            let (w, h) = s.document.resolution().unwrap_or((512, 512));
            let alt = s.raw_attrs.alt.clone();
            MediaInfo {
                kind: "sticker",
                label: format!("{alt} Stiker").trim().to_string(),
                w,
                h,
                mime: s.document.mime_type().unwrap_or("").to_string(),
                alt,
                ..Default::default()
            }
        }
        Media::Contact(_) => MediaInfo { kind: "other", label: "Kontakt".into(), ..Default::default() },
        Media::Poll(p) => {
            let tl::enums::TextWithEntities::Entities(q) = &p.raw.question;
            let mut votes: HashMap<Vec<u8>, (i32, bool)> = HashMap::new();
            if let Some(rs) = &p.raw_results.results {
                for v in rs {
                    let tl::enums::PollAnswerVoters::Voters(v) = v;
                    votes.insert(v.option.clone(), (v.voters.unwrap_or(0), v.chosen));
                }
            }
            let answers: Vec<Value> = p
                .raw
                .answers
                .iter()
                .map(|a| {
                    let tl::enums::PollAnswer::Answer(a) = a else { return Value::Null };
                    let tl::enums::TextWithEntities::Entities(t) = &a.text;
                    let (n, chosen) = votes.get(&a.option).copied().unwrap_or((0, false));
                    json!({"text": t.text, "option": hex::encode(&a.option), "votes": n, "chosen": chosen})
                })
                .collect();
            let poll = json!({
                "question": q.text,
                "closed": p.raw.closed,
                "quiz": p.raw.quiz,
                "multi": p.raw.multiple_choice,
                "total": p.raw_results.total_voters.unwrap_or(0),
                "answers": answers,
            });
            MediaInfo { kind: "poll", label: "So'rovnoma".into(), poll: Some(poll), ..Default::default() }
        }
        Media::Geo(_) | Media::Venue(_) | Media::GeoLive(_) => MediaInfo { kind: "other", label: "Joylashuv".into(), ..Default::default() },
        Media::Dice(_) => MediaInfo { kind: "other", label: "Zar".into(), ..Default::default() },
        Media::WebPage(_) => MediaInfo::default(),
        #[allow(unreachable_patterns)]
        _ => MediaInfo { kind: "other", label: "Ilova".into(), ..Default::default() },
    }
}

/// Suhbatlar ro'yxati (Telegram "Chatlar" oynasi uchun): `{"dialogs":[{"id","kind":"user|group|channel","name","pinned",
/// "unread","muted","out","date","text","bot","self","verified","username"}]}`. `arg` — nechta suhbat kerakligi (standart 40).
async fn dialogs_json(client: &Client, limit: usize) -> Result<Value, String> {
    use grammers_client::peer::Peer;
    let now_secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let mut out = Vec::new();
    let mut it = client.iter_dialogs();
    while out.len() < limit {
        let Some(d) = it.next().await.map_err(|e| e.to_string())? else { break };
        let (kind, name, bot, is_self, verified) = match d.peer() {
            Peer::User(u) => ("user", u.full_name(), u.is_bot(), u.is_self(), u.verified()),
            Peer::Group(g) => ("group", g.title().unwrap_or_default().to_string(), false, false, false),
            Peer::Channel(c) => ("channel", c.title().to_string(), false, false, false),
        };
        let (pinned, unread, muted, read_out, mentions, top) = match &d.raw {
            tl::enums::Dialog::Dialog(x) => {
                let muted = matches!(&x.notify_settings, tl::enums::PeerNotifySettings::Settings(n) if n.mute_until.unwrap_or(0) as i64 > now_secs);
                (x.pinned, x.unread_count, muted, x.read_outbox_max_id, x.unread_mentions_count, x.top_message)
            }
            tl::enums::Dialog::Folder(_) => continue,
        };
        let (text, date, outgoing) = match &d.last_message {
            Some(m) => {
                let mut t = m.text().replace('\n', " ");
                if t.is_empty() {
                    if let Some(md) = m.media() {
                        t = media_info(&md).label;
                    }
                }
                if t.is_empty() {
                    if let tl::enums::Message::Service(x) = &m.raw {
                        let who = m
                            .sender()
                            .map(|p| match p {
                                Peer::User(u) => u.full_name(),
                                Peer::Group(g) => g.title().unwrap_or_default().to_string(),
                                Peer::Channel(c) => c.title().to_string(),
                            })
                            .unwrap_or_default();
                        t = service_text(&who, &x.action, x.from_id.as_ref());
                    }
                }
                (t, m.date().timestamp(), m.outgoing())
            }
            None => (String::new(), 0, false),
        };
        out.push(json!({
            "id": d.peer_id().bot_api_dialog_id(),
            "kind": kind,
            "name": if is_self { "Saqlangan xabarlar".to_string() } else { name },
            "pinned": pinned,
            "unread": unread,
            "muted": muted,
            "read_out": read_out,
            "top": top,
            "mentions": mentions,
            "access": d.peer_ref().auth.hash().to_string(),
            "out": outgoing,
            "date": date,
            "text": text,
            "bot": bot,
            "self": is_self,
            "verified": verified,
            "username": d.peer().username().unwrap_or_default(),
        }));
    }
    Ok(json!({ "dialogs": out }))
}

#[no_mangle]
pub extern "C" fn rust_tg_dialogs(arg_ptr: *const c_char) -> *mut c_char {
    let limit = unsafe { cstr_to_str(arg_ptr) }.unwrap_or("").trim().parse::<usize>().unwrap_or(40).clamp(1, 200);
    string_to_cptr(with_client(|t, client| {
        if !t.authorized.load(Ordering::SeqCst) {
            return Err("Telegram ulanmagan".to_string());
        }
        let v = run_tmo(t, 30, dialogs_json(&client, limit))?;
        Ok(v.to_string())
    }))
}

/// Bot tugmalari: `{"inline":bool,"rows":[[{"text","kind":"url|cb|text","data"}]]}` yoki null.
fn markup_json(m: &grammers_client::message::Message) -> Value {
    let Some(rm) = m.reply_markup() else { return Value::Null };
    let (inline, rows) = match rm {
        tl::enums::ReplyMarkup::ReplyInlineMarkup(k) => (true, k.rows),
        tl::enums::ReplyMarkup::ReplyKeyboardMarkup(k) => (false, k.rows),
        _ => return Value::Null,
    };
    let mut out = Vec::new();
    for row in rows {
        let tl::enums::KeyboardButtonRow::Row(r) = row;
        let mut bs = Vec::new();
        for b in r.buttons {
            match b {
                tl::enums::KeyboardButton::Url(u) => bs.push(json!({"text": u.text, "kind": "url", "data": u.url})),
                tl::enums::KeyboardButton::Callback(c) => bs.push(json!({"text": c.text, "kind": "cb", "data": hex::encode(&c.data)})),
                tl::enums::KeyboardButton::Button(b) => bs.push(json!({"text": b.text, "kind": "text", "data": b.text})),
                tl::enums::KeyboardButton::WebView(w) => bs.push(json!({"text": w.text, "kind": "webapp", "data": w.url})),
                tl::enums::KeyboardButton::SimpleWebView(w) => bs.push(json!({"text": w.text, "kind": "simple", "data": w.url})),
                _ => {}
            }
        }
        if !bs.is_empty() {
            out.push(Value::Array(bs));
        }
    }
    if out.is_empty() {
        return Value::Null;
    }
    json!({"inline": inline, "rows": out})
}

/// Xizmat xabari matni (o'zbekcha).
fn service_text(who: &str, a: &tl::enums::MessageAction, from: Option<&tl::enums::Peer>) -> String {
    use tl::enums::MessageAction as A;
    let who = if who.is_empty() { "Foydalanuvchi".to_string() } else { who.to_string() };
    let from_id = from.map(peer_bot_id).unwrap_or(0);
    match a {
        A::ChatCreate(x) => format!("{who} “{}” guruhini yaratdi", x.title),
        A::ChannelCreate(x) => format!("“{}” kanali yaratildi", x.title),
        A::ChatEditTitle(x) => format!("{who} nomni “{}” ga o‘zgartirdi", x.title),
        A::ChatEditPhoto(_) => format!("{who} rasmni o‘zgartirdi"),
        A::ChatDeletePhoto => format!("{who} rasmni o‘chirdi"),
        A::ChatAddUser(x) => {
            if x.users.len() == 1 && x.users[0] == from_id { format!("{who} guruhga qo‘shildi") } else { format!("{who} a’zo qo‘shdi") }
        }
        A::ChatDeleteUser(x) => {
            if x.user_id == from_id { format!("{who} guruhni tark etdi") } else { format!("{who} a’zoni chiqardi") }
        }
        A::ChatJoinedByLink(_) => format!("{who} havola orqali qo‘shildi"),
        A::ChatJoinedByRequest => format!("{who} qo‘shilish so‘rovi bilan qo‘shildi"),
        A::PinMessage => format!("{who} xabarni mahkamladi"),
        A::HistoryClear => "Tarix tozalandi".to_string(),
        A::ContactSignUp => format!("{who} Telegramga qo‘shildi"),
        A::ScreenshotTaken => format!("{who} ekran suratini oldi"),
        A::PhoneCall(_) => "Qo‘ng‘iroq".to_string(),
        A::ChatMigrateTo(_) | A::ChannelMigrateFrom(_) => "Guruh superguruhga aylantirildi".to_string(),
        _ => "Xizmat xabari".to_string(),
    }
}

/// Xabar bezaklari: `[tur, offset, uzunlik, qo'shimcha]`.
fn ents_json(es: &[tl::enums::MessageEntity]) -> Vec<Value> {
    use tl::enums::MessageEntity as E;
    es.iter()
        .filter_map(|e| {
            Some(match e {
                E::Bold(x) => json!(["b", x.offset, x.length, ""]),
                E::Italic(x) => json!(["i", x.offset, x.length, ""]),
                E::Underline(x) => json!(["u", x.offset, x.length, ""]),
                E::Strike(x) => json!(["s", x.offset, x.length, ""]),
                E::Code(x) => json!(["c", x.offset, x.length, ""]),
                E::Pre(x) => json!(["p", x.offset, x.length, x.language]),
                E::TextUrl(x) => json!(["l", x.offset, x.length, x.url]),
                E::Url(x) => json!(["url", x.offset, x.length, ""]),
                E::Mention(x) => json!(["m", x.offset, x.length, ""]),
                E::MentionName(x) => json!(["n", x.offset, x.length, x.user_id.to_string()]),
                E::Hashtag(x) => json!(["h", x.offset, x.length, ""]),
                E::Cashtag(x) => json!(["h", x.offset, x.length, ""]),
                E::BotCommand(x) => json!(["bc", x.offset, x.length, ""]),
                E::Email(x) => json!(["e", x.offset, x.length, ""]),
                E::Phone(x) => json!(["ph", x.offset, x.length, ""]),
                E::Spoiler(x) => json!(["sp", x.offset, x.length, ""]),
                E::Blockquote(x) => json!(["q", x.offset, x.length, ""]),
                _ => return None,
            })
        })
        .collect()
}

/// Bitta xabarning JSON ko'rinishi (suhbat oynasi, qidiruv, mahkamlangan xabar uchun umumiy).
fn msg_json(m: &grammers_client::message::Message) -> Value {
    let sender = m.sender().map(|p| match p {
        grammers_client::peer::Peer::User(u) => u.full_name(),
        grammers_client::peer::Peer::Group(g) => g.title().unwrap_or_default().to_string(),
        grammers_client::peer::Peer::Channel(c) => c.title().to_string(),
    });
    let mi = m.media().map(|md| media_info(&md)).unwrap_or_default();
    let fwd = m.forward_header().and_then(|f| match f {
        tl::enums::MessageFwdHeader::Header(h) => h.from_name.clone(),
    });
    // Reaksiyalar: [["👍", soni, o'zimniki], ...]
    let mut reactions = Vec::new();
    if let tl::enums::Message::Message(x) = &m.raw {
        if let Some(tl::enums::MessageReactions::Reactions(r)) = &x.reactions {
            for rc in &r.results {
                let tl::enums::ReactionCount::Count(rc) = rc;
                if let tl::enums::Reaction::Emoji(e) = &rc.reaction {
                    reactions.push(json!([e.emoticon, rc.count, rc.chosen_order.is_some()]));
                }
            }
        }
    }
    // Matn bezaklari (qalin, kursiv, havola, kod, spoyler...): `[[tur, offset, uzunlik, qo'shimcha]]` (offset UTF-16 birligida — Kotlin bilan bir xil).
    let ents: Vec<Value> = match &m.raw {
        tl::enums::Message::Message(x) => x.entities.as_deref().map(ents_json).unwrap_or_default(),
        _ => Vec::new(),
    };
    // Havola ko'rinishi (sarlavha, tavsif).
    let web = match m.media() {
        Some(grammers_client::media::Media::WebPage(w)) => match &w.raw.webpage {
            tl::enums::WebPage::Page(p) => json!({
                "url": p.url,
                "site": p.site_name.clone().unwrap_or_default(),
                "title": p.title.clone().unwrap_or_default(),
                "desc": p.description.clone().unwrap_or_default(),
            }),
            _ => Value::Null,
        },
        _ => Value::Null,
    };
    // Xizmat xabarlari (guruhga qo'shildi, mahkamlandi...): markazda chiqadigan matn.
    let service = match &m.raw {
        tl::enums::Message::Service(x) => service_text(&sender.clone().unwrap_or_default(), &x.action, x.from_id.as_ref()),
        _ => String::new(),
    };
    // Yuboruvchi (guruhda avatar uchun): id, access_hash, profil rasmi id.
    let (sid, shash, sphoto) = match m.sender() {
        Some(grammers_client::peer::Peer::User(u)) => {
            let hash = match &u.raw {
                tl::enums::User::User(x) => x.access_hash.unwrap_or(0),
                _ => 0,
            };
            (u.id().bot_api_dialog_id().unwrap_or(0), hash, u.photo().map(|p| p.photo_id).unwrap_or(0))
        }
        _ => (0, 0, 0),
    };
    json!({
        "sid": sid,
        "shash": shash.to_string(),
        "sphoto": sphoto.to_string(),
        "service": service,
        "ents": ents,
        "web": web,
        "id": m.id(),
        "out": m.outgoing(),
        "date": m.date().timestamp(),
        "edited": m.edit_date().is_some() && !matches!(&m.raw, tl::enums::Message::Message(x) if x.edit_hide),
        "text": m.text(),
        "sender": sender.unwrap_or_default(),
        "reply": m.reply_to_message_id().unwrap_or(0),
        "fwd": fwd.unwrap_or_default(),
        "media": mi.label,
        "kind": mi.kind,
        "pw": mi.w,
        "ph": mi.h,
        "name": mi.name,
        "size": mi.size,
        "dur": mi.dur,
        "mime": mi.mime,
        "alt": mi.alt,
        "wave": mi.wave,
        "views": m.view_count().unwrap_or(0),
        "poll": mi.poll,
        "markup": markup_json(m),
        "pinned": m.pinned(),
        "reactions": reactions,
    })
}

/// Suhbat xabarlari (eng yangisi birinchi): `{"messages":[...]}`; `offset_id` > 0 bo'lsa undan ESKI xabarlar.
async fn messages_json(client: &Client, spec: &str, limit: usize, offset_id: i32) -> Result<Value, String> {
    let peer = resolve_peer(client, spec).await?;
    let mut out = Vec::new();
    let mut it = client.iter_messages(peer).limit(limit);
    if offset_id > 0 {
        it = it.offset_id(offset_id);
    }
    while let Some(m) = it.next().await.map_err(|e| e.to_string())? {
        out.push(msg_json(&m));
    }
    Ok(json!({ "messages": out }))
}

/// `arg` = `"<suhbat>:<soni>:<offset_id>"` (`<suhbat>` — `id` yoki `id@access_hash`).
#[no_mangle]
pub extern "C" fn rust_tg_messages(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let mut p = arg.trim().split(':');
        let spec = p.next().unwrap_or("").to_string();
        let limit = p.next().and_then(|x| x.parse::<usize>().ok()).unwrap_or(40).clamp(1, 200);
        let offset = p.next().and_then(|x| x.parse::<i32>().ok()).unwrap_or(0);
        messages_json(&client, &spec, limit, offset).await
    })
}

/// Matnli xabar yuboradi. `arg` = `"<suhbat_id>\n<matn>"`. Javob: `{"ok":true,"id":<xabar_id>}`.
#[no_mangle]
pub extern "C" fn rust_tg_send_text(arg_ptr: *const c_char) -> *mut c_char {
    let arg = unsafe { cstr_to_str(arg_ptr) }.unwrap_or("").to_string();
    let (id, text) = match arg.split_once('\n') {
        Some((a, b)) => (a.trim().parse::<i64>().unwrap_or(0), b.to_string()),
        None => (0, String::new()),
    };
    string_to_cptr(with_client(|t, client| {
        if !t.authorized.load(Ordering::SeqCst) {
            return Err("Telegram ulanmagan".to_string());
        }
        if text.trim().is_empty() {
            return Err("Xabar bo'sh".to_string());
        }
        let v = run_tmo(t, 30, async {
            let peer = find_channel(&client, id).await.map_err(|_| "Suhbat topilmadi".to_string())?;
            let m = client.send_message(peer, text.clone()).await.map_err(|e| e.to_string())?;
            Ok(json!({"ok": true, "id": m.id()}))
        })?;
        Ok(v.to_string())
    }))
}

/// Suhbat avatarlari: `arg` = vergul bilan ajratilgan suhbat ID'lari. Javob: `{"photos":{"<id>":"<base64 jpeg>"}}`
/// (rasmi yo'qlari kirmaydi). Bitta dialog aylanishida hammasi olinadi.
async fn avatars_json(client: &Client, ids: &[i64]) -> Result<Value, String> {
    use grammers_client::peer::Peer;
    let mut want: std::collections::HashSet<i64> = ids.iter().copied().collect();
    let mut photos = serde_json::Map::new();
    let mut it = client.iter_dialogs();
    while !want.is_empty() {
        let Some(d) = it.next().await.map_err(|e| e.to_string())? else { break };
        let Some(id) = d.peer_id().bot_api_dialog_id() else { continue };
        if !want.remove(&id) {
            continue;
        }
        let photo_id = match d.peer() {
            Peer::User(u) => u.photo().map(|p| p.photo_id),
            Peer::Group(g) => g.photo().map(|p| p.photo_id),
            Peer::Channel(c) => c.photo().map(|p| p.photo_id),
        };
        let Some(photo_id) = photo_id else { continue };
        let peer: tl::enums::InputPeer = d.peer_ref().into();
        let loc = tl::enums::InputFileLocation::InputPeerPhotoFileLocation(tl::types::InputPeerPhotoFileLocation { big: false, peer, photo_id });
        if let Some(b) = download_small(client, loc).await {
            photos.insert(id.to_string(), json!(b64(&b)));
        }
    }
    Ok(json!({ "photos": photos }))
}

#[no_mangle]
pub extern "C" fn rust_tg_avatars(arg_ptr: *const c_char) -> *mut c_char {
    let ids: Vec<i64> = unsafe { cstr_to_str(arg_ptr) }
        .unwrap_or("")
        .split(',')
        .filter_map(|x| x.trim().parse::<i64>().ok())
        .take(100)
        .collect();
    string_to_cptr(with_client(|t, client| {
        if !t.authorized.load(Ordering::SeqCst) {
            return Err("Telegram ulanmagan".to_string());
        }
        let v = run_tmo(t, 60, avatars_json(&client, &ids))?;
        Ok(v.to_string())
    }))
}

/// Suhbatni o'qilgan deb belgilaydi (Telegram suhbat ochilganda shunday qiladi). `arg` — suhbat (`id` yoki `id@hash`).
#[no_mangle]
pub extern "C" fn rust_tg_mark_read(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let peer = resolve_peer(&client, arg.trim()).await?;
        client.mark_as_read(peer).await.map_err(|e| e.to_string())?;
        Ok(json!({"ok": true}))
    })
}

/// Joriy hisob: `{"id","name","phone","username"}` (yon panel sarlavhasi uchun).
#[no_mangle]
pub extern "C" fn rust_tg_me() -> *mut c_char {
    string_to_cptr(with_client(|t, client| {
        if !t.authorized.load(Ordering::SeqCst) {
            return Err("Telegram ulanmagan".to_string());
        }
        let me = run_tmo(t, 30, async { client.get_me().await.map_err(|e| e.to_string()) })?;
        Ok(json!({
            "id": me.id().bot_api_dialog_id(),
            "name": me.full_name(),
            "phone": me.phone().unwrap_or_default(),
            "username": me.username().unwrap_or_default(),
        })
        .to_string())
    }))
}

/// Kontaktlar: `{"contacts":[{"id","name","status","online","seen","username"}]}` (`status` — matn, `seen` — oxirgi marta unix).
#[no_mangle]
pub extern "C" fn rust_tg_contacts() -> *mut c_char {
    string_to_cptr(with_client(|t, client| {
        if !t.authorized.load(Ordering::SeqCst) {
            return Err("Telegram ulanmagan".to_string());
        }
        let v = run_tmo(t, 30, async {
            let res = client.invoke(&tl::functions::contacts::GetContacts { hash: 0 }).await.map_err(|e| e.to_string())?;
            let users = match res {
                tl::enums::contacts::Contacts::Contacts(c) => c.users,
                _ => Vec::new(),
            };
            let mut out = Vec::new();
            for u in users {
                let tl::enums::User::User(u) = u else { continue };
                let name = format!("{} {}", u.first_name.clone().unwrap_or_default(), u.last_name.clone().unwrap_or_default()).trim().to_string();
                let (status, online, seen) = match &u.status {
                    Some(tl::enums::UserStatus::Online(_)) => ("onlayn", true, 0i64),
                    Some(tl::enums::UserStatus::Offline(o)) => ("", false, o.was_online as i64),
                    Some(tl::enums::UserStatus::Recently(_)) => ("yaqinda onlayn edi", false, 0),
                    Some(tl::enums::UserStatus::LastWeek(_)) => ("o'tgan hafta onlayn edi", false, 0),
                    Some(tl::enums::UserStatus::LastMonth(_)) => ("o'tgan oy onlayn edi", false, 0),
                    _ => ("uzoq vaqt oldin onlayn edi", false, 0),
                };
                out.push(json!({
                    "id": u.id,
                    "name": name,
                    "status": status,
                    "online": online,
                    "seen": seen,
                    "username": u.username.clone().unwrap_or_default(),
                    "hash": u.access_hash.unwrap_or(0).to_string(),
                    "photo": match &u.photo {
                        Some(tl::enums::UserProfilePhoto::Photo(p)) => p.photo_id.to_string(),
                        _ => String::new(),
                    },
                }));
            }
            Ok(json!({ "contacts": out }))
        })?;
        Ok(v.to_string())
    }))
}

/// Kontakt avatarlari: `arg` = `"<user_id>:<access_hash>:<photo_id>,..."`. Javob: `{"photos":{"<user_id>":"<base64>"}}`.
#[no_mangle]
pub extern "C" fn rust_tg_user_avatars(arg_ptr: *const c_char) -> *mut c_char {
    let items: Vec<(i64, i64, i64)> = unsafe { cstr_to_str(arg_ptr) }
        .unwrap_or("")
        .split(',')
        .filter_map(|x| {
            let mut p = x.trim().split(':');
            Some((p.next()?.parse().ok()?, p.next()?.parse().ok()?, p.next()?.parse().ok()?))
        })
        .take(60)
        .collect();
    string_to_cptr(with_client(|t, client| {
        if !t.authorized.load(Ordering::SeqCst) {
            return Err("Telegram ulanmagan".to_string());
        }
        let v = run_tmo(t, 60, async {
            let mut photos = serde_json::Map::new();
            for (user_id, access_hash, photo_id) in items.iter().copied() {
                let peer = tl::enums::InputPeer::User(tl::types::InputPeerUser { user_id, access_hash });
                let loc = tl::enums::InputFileLocation::InputPeerPhotoFileLocation(tl::types::InputPeerPhotoFileLocation { big: false, peer, photo_id });
                if let Some(b) = download_small(&client, loc).await {
                    photos.insert(user_id.to_string(), json!(b64(&b)));
                }
            }
            Ok(json!({ "photos": photos }))
        })?;
        Ok(v.to_string())
    }))
}

/// Xabardagi rasm (o'rtacha o'lcham, eng ko'pi 800 px): `arg` = `"<suhbat>:<xabar_id>"`. Javob: `{"photo":"<base64>"}`.
#[no_mangle]
pub extern "C" fn rust_tg_photo(arg_ptr: *const c_char) -> *mut c_char {
    let arg = unsafe { cstr_to_str(arg_ptr) }.unwrap_or("").trim().to_string();
    let (chat, msg) = match arg.split_once(':') {
        Some((a, b)) => (a.to_string(), b.parse::<i32>().unwrap_or(0)),
        None => (String::new(), 0),
    };
    string_to_cptr(with_client(|t, client| {
        if !t.authorized.load(Ordering::SeqCst) {
            return Err("Telegram ulanmagan".to_string());
        }
        let v = run_tmo(t, 60, async {
            use grammers_client::media::{Media, PhotoSize};
            let peer = resolve_peer(&client, &chat).await?;
            let msgs = client.get_messages_by_id(peer, &[msg]).await.map_err(|e| e.to_string())?;
            let Some(Some(m)) = msgs.into_iter().next() else { return Err("Xabar topilmadi".to_string()) };
            let Some(Media::Photo(p)) = m.media() else { return Err("Rasm emas".to_string()) };
            // 800 px dan oshmaydigan eng kattasi; bo'lmasa eng kichigi.
            let mut sizes: Vec<_> = p
                .thumbs()
                .into_iter()
                .filter_map(|t| match t {
                    PhotoSize::Size(sz) => Some(sz),
                    _ => None,
                })
                .collect();
            sizes.sort_by_key(|sz| sz.width.max(sz.height));
            let pick = sizes.iter().rev().find(|sz| sz.width.max(sz.height) <= 800).or_else(|| sizes.first());
            let Some(loc) = pick.and_then(|sz| sz.to_raw_input_location()) else { return Err("Rasm o'lchami yo'q".to_string()) };
            let Some(bytes) = download_small(&client, loc).await else { return Err("Rasm yuklanmadi".to_string()) };
            Ok(json!({ "photo": b64(&bytes) }))
        })?;
        Ok(v.to_string())
    }))
}

// ═══════════════════════════════════════════════════════════════
//  SUHBATLAR (Telegram mijozi): umumiy yordamchilar
// ═══════════════════════════════════════════════════════════════

/// Autorizatsiyani tekshirib, `f(client, arg)` ni ishga tushiradi va natijani JSON matn qilib qaytaradi.
fn tg_call<F, Fut>(arg_ptr: *const c_char, secs: u64, f: F) -> *mut c_char
where
    F: FnOnce(Client, String) -> Fut,
    Fut: std::future::Future<Output = Result<Value, String>>,
{
    let arg = unsafe { cstr_to_str(arg_ptr) }.unwrap_or("").to_string();
    string_to_cptr(with_client(|t, client| {
        if !t.authorized.load(Ordering::SeqCst) {
            return Err("Telegram ulanmagan".to_string());
        }
        let v = run_tmo(t, secs, f(client, arg))?;
        Ok(v.to_string())
    }))
}

/// Suhbatni topadi. `spec` — `id` yoki `id@access_hash`. Hash berilsa tarmoqqa chiqmasdan yasaladi; bo'lmasa dialoglar,
/// keyin kontaktlar ichidan qidiriladi.
async fn resolve_peer(client: &Client, spec: &str) -> Result<PeerRef, String> {
    let (id, hash) = match spec.split_once('@') {
        Some((a, b)) => (a.trim().parse::<i64>().unwrap_or(0), b.trim().parse::<i64>().ok()),
        None => (spec.trim().parse::<i64>().unwrap_or(0), None),
    };
    if id == 0 {
        return Err("Suhbat ko'rsatilmagan".to_string());
    }
    if let (Some(h), Some(pid)) = (hash, PeerId::from_bot_api_dialog_id(id)) {
        // Oddiy guruhda hash yo'q; foydalanuvchi va kanal uchun yetarli.
        if h != 0 || id < 0 && id > -1_000_000_000_000 {
            return Ok(PeerRef { id: pid, auth: PeerAuth::from_hash(h) });
        }
    }
    if let Ok(p) = find_channel(client, id).await {
        return Ok(p);
    }
    if id > 0 {
        if let Ok(tl::enums::contacts::Contacts::Contacts(c)) = client.invoke(&tl::functions::contacts::GetContacts { hash: 0 }).await {
            for u in c.users {
                if let tl::enums::User::User(u) = u {
                    if u.id == id {
                        if let (Some(h), Some(pid)) = (u.access_hash, PeerId::user(id)) {
                            return Ok(PeerRef { id: pid, auth: PeerAuth::from_hash(h) });
                        }
                    }
                }
            }
        }
    }
    Err("Suhbat topilmadi".to_string())
}

/// Argument qatorlarini `\n` bo'yicha bo'ladi.
fn lines(arg: &str, n: usize) -> Vec<String> {
    let mut v: Vec<String> = arg.splitn(n, '\n').map(|x| x.to_string()).collect();
    while v.len() < n {
        v.push(String::new());
    }
    v
}

fn ids_of(s: &str) -> Vec<i32> {
    s.split(',').filter_map(|x| x.trim().parse::<i32>().ok()).collect()
}

/// Matnli xabar (javob bilan): `arg` = `"<suhbat>\n<javob_id|0>\n<matn>"`. Javob: `{"ok":true,"id":..}`.
#[no_mangle]
pub extern "C" fn rust_tg_send(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let l = lines(&arg, 3);
        if l[2].trim().is_empty() {
            return Err("Xabar bo'sh".to_string());
        }
        let peer = resolve_peer(&client, &l[0]).await?;
        let reply = l[1].trim().parse::<i32>().ok().filter(|x| *x > 0);
        let msg = grammers_client::message::InputMessage::new().text(l[2].clone()).reply_to(reply);
        let m = client.send_message(peer, msg).await.map_err(|e| e.to_string())?;
        Ok(json!({"ok": true, "id": m.id()}))
    })
}

/// Xabarni tahrirlash: `arg` = `"<suhbat>\n<xabar_id>\n<yangi matn>"`.
#[no_mangle]
pub extern "C" fn rust_tg_edit(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let l = lines(&arg, 3);
        let id = l[1].trim().parse::<i32>().map_err(|_| "Xabar ko'rsatilmagan".to_string())?;
        if l[2].trim().is_empty() {
            return Err("Xabar bo'sh".to_string());
        }
        let peer = resolve_peer(&client, &l[0]).await?;
        client.edit_message(peer, id, l[2].clone()).await.map_err(|e| e.to_string())?;
        Ok(json!({"ok": true}))
    })
}

/// Xabarlarni o'chirish: `arg` = `"<suhbat>\n<id1,id2,..>"`.
#[no_mangle]
pub extern "C" fn rust_tg_delete(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let l = lines(&arg, 2);
        let ids = ids_of(&l[1]);
        if ids.is_empty() {
            return Err("Xabar tanlanmagan".to_string());
        }
        let peer = resolve_peer(&client, &l[0]).await?;
        let n = client.delete_messages(peer, &ids).await.map_err(|e| e.to_string())?;
        Ok(json!({"ok": true, "deleted": n}))
    })
}

/// Xabarlarni uzatish: `arg` = `"<qayerdan>\n<qayerga>\n<id1,id2,..>"`.
#[no_mangle]
pub extern "C" fn rust_tg_forward(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let l = lines(&arg, 3);
        let ids = ids_of(&l[2]);
        if ids.is_empty() {
            return Err("Xabar tanlanmagan".to_string());
        }
        let from = resolve_peer(&client, &l[0]).await?;
        let to = resolve_peer(&client, &l[1]).await?;
        client.forward_messages(to, &ids, from).await.map_err(|e| e.to_string())?;
        Ok(json!({"ok": true}))
    })
}

/// Xabarni mahkamlash/yechish: `arg` = `"<suhbat>\n<xabar_id>\n<1|0>"`.
#[no_mangle]
pub extern "C" fn rust_tg_pin_message(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let l = lines(&arg, 3);
        let id = l[1].trim().parse::<i32>().map_err(|_| "Xabar ko'rsatilmagan".to_string())?;
        let peer = resolve_peer(&client, &l[0]).await?;
        if l[2].trim() == "1" {
            client.pin_message(peer, id).await.map_err(|e| e.to_string())?;
        } else {
            client.unpin_message(peer, id).await.map_err(|e| e.to_string())?;
        }
        Ok(json!({"ok": true}))
    })
}

/// Suhbatni ro'yxatda mahkamlash/yechish: `arg` = `"<suhbat>\n<1|0>"`.
#[no_mangle]
pub extern "C" fn rust_tg_pin_dialog(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let l = lines(&arg, 2);
        let peer = resolve_peer(&client, &l[0]).await?;
        let ip: tl::enums::InputPeer = peer.into();
        client
            .invoke(&tl::functions::messages::ToggleDialogPin {
                pinned: l[1].trim() == "1",
                peer: tl::enums::InputDialogPeer::Peer(tl::types::InputDialogPeer { peer: ip }),
            })
            .await
            .map_err(|e| e.to_string())?;
        Ok(json!({"ok": true}))
    })
}

/// Suhbat bildirishnomalarini o'chirish/yoqish: `arg` = `"<suhbat>\n<1|0>"` (1 — sokin).
#[no_mangle]
pub extern "C" fn rust_tg_mute(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let l = lines(&arg, 2);
        let peer = resolve_peer(&client, &l[0]).await?;
        let ip: tl::enums::InputPeer = peer.into();
        // "0" — yoqish, "1" — doimiy, undan katta son — shuncha soniyaga.
        let secs = l[1].trim().parse::<i64>().unwrap_or(0);
        let now_s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
        let mute_until = if secs <= 0 { 0 } else if secs == 1 { i32::MAX } else { (now_s + secs).min(i32::MAX as i64) as i32 };
        client
            .invoke(&tl::functions::account::UpdateNotifySettings {
                peer: tl::enums::InputNotifyPeer::Peer(tl::types::InputNotifyPeer { peer: ip }),
                settings: tl::enums::InputPeerNotifySettings::Settings(tl::types::InputPeerNotifySettings {
                    show_previews: None,
                    silent: None,
                    mute_until: Some(mute_until),
                    sound: None,
                    stories_muted: None,
                    stories_hide_sender: None,
                    stories_sound: None,
                }),
            })
            .await
            .map_err(|e| e.to_string())?;
        Ok(json!({"ok": true}))
    })
}

/// Suhbat tarixini o'chirish/chiqish: `arg` = `"<suhbat>"` (guruh/kanaldan chiqadi, shaxsiy chatda tarix o'chadi).
#[no_mangle]
pub extern "C" fn rust_tg_delete_dialog(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 40, |client, arg| async move {
        let peer = resolve_peer(&client, arg.trim()).await?;
        client.delete_dialog(peer).await.map_err(|e| e.to_string())?;
        Ok(json!({"ok": true}))
    })
}

/// Rasm/video/ovoz/fayl yuborish: `arg` = `"<suhbat>\n<photo|video|voice|file>\n<yo'l>\n<javob_id|0>\n<soniya:eni:bo'yi>\n<izoh>"`.
#[no_mangle]
pub extern "C" fn rust_tg_send_media(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 600, |client, arg| async move {
        let l = lines(&arg, 6);
        let peer = resolve_peer(&client, &l[0]).await?;
        let mut f = tokio::fs::File::open(l[2].trim()).await.map_err(|e| format!("Fayl ochilmadi: {e}"))?;
        let size = f.metadata().await.map_err(|e| e.to_string())?.len() as usize;
        if size == 0 {
            return Err("Fayl bo'sh".to_string());
        }
        let name = std::path::Path::new(l[2].trim()).file_name().and_then(|x| x.to_str()).unwrap_or("fayl").to_string();
        let up = client.upload_stream(&mut f, size, name).await.map_err(|e| e.to_string())?;
        let reply = l[3].trim().parse::<i32>().ok().filter(|x| *x > 0);
        let msg = grammers_client::message::InputMessage::new().text(l[5].clone()).reply_to(reply);
        // Qo'shimcha: `<soniya>:<eni>:<bo'yi>` (ovoz va video uchun).
        let mut ex = l[4].trim().split(':');
        let dur = ex.next().and_then(|x| x.parse::<f64>().ok()).unwrap_or(0.0).max(0.0);
        let w = ex.next().and_then(|x| x.parse::<i32>().ok()).unwrap_or(0);
        let h = ex.next().and_then(|x| x.parse::<i32>().ok()).unwrap_or(0);
        // To'rtinchi qism (ovoz uchun): 5 bitli to'lqin shakli, hex.
        let waveform = ex.next().and_then(|x| hex::decode(x.trim()).ok()).filter(|w| !w.is_empty());
        let duration = Duration::from_secs(dur.round() as u64);
        let msg = match l[1].trim() {
            "photo" => msg.photo(up),
            "voice" => msg.document(up).mime_type("audio/ogg").attribute(grammers_client::media::Attribute::Voice { duration, waveform }),
            "video" => msg.document(up).mime_type("video/mp4").attribute(grammers_client::media::Attribute::Video {
                round_message: false,
                supports_streaming: true,
                duration,
                w,
                h,
            }),
            _ => msg.document(up),
        };
        let m = client.send_message(peer, msg).await.map_err(|e| e.to_string())?;
        Ok(json!({"ok": true, "id": m.id()}))
    })
}

/// Xabardagi faylni (rasm, video, ovoz, stiker, hujjat) diskka yuklaydi: `arg` = `"<suhbat>\n<xabar_id>\n<papka>"`.
/// Javob: `{"path","kind","name","mime","size"}`. Fayl avval yuklangan bo'lsa qayta yuklanmaydi.
#[no_mangle]
pub extern "C" fn rust_tg_media_file(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 900, |client, arg| async move {
        let l = lines(&arg, 3);
        let id = l[1].trim().parse::<i32>().map_err(|_| "Xabar ko'rsatilmagan".to_string())?;
        let peer = resolve_peer(&client, &l[0]).await?;
        let msgs = client.get_messages_by_id(peer, &[id]).await.map_err(|e| e.to_string())?;
        let Some(Some(m)) = msgs.into_iter().next() else { return Err("Xabar topilmadi".to_string()) };
        let Some(md) = m.media() else { return Err("Xabarda fayl yo'q".to_string()) };
        let mi = media_info(&md);
        let ext = match mi.kind {
            "photo" => "jpg".to_string(),
            "voice" => "ogg".to_string(),
            "video" | "gif" | "round" => "mp4".to_string(),
            "sticker" => {
                if mi.mime == "image/webp" {
                    "webp".to_string()
                } else if mi.mime.contains("tgsticker") {
                    "tgs".to_string()
                } else {
                    "webm".to_string()
                }
            }
            _ => std::path::Path::new(&mi.name).extension().and_then(|x| x.to_str()).unwrap_or("bin").to_string(),
        };
        let dir = std::path::PathBuf::from(l[2].trim());
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let key: String = l[0].chars().filter(|c| c.is_ascii_digit() || *c == '-').collect();
        let path = dir.join(format!("{key}_{id}.{ext}"));
        if !path.exists() || fs::metadata(&path).map(|x| x.len()).unwrap_or(0) == 0 {
            let tmp = dir.join(format!("{key}_{id}.part"));
            let mut out = fs::File::create(&tmp).map_err(|e| e.to_string())?;
            let mut it = client.iter_download(&md);
            let mut total: usize = 0;
            while let Some(chunk) = it.next().await.map_err(|e| e.to_string())? {
                total += chunk.len();
                if total > 200 * 1024 * 1024 {
                    let _ = fs::remove_file(&tmp);
                    return Err("Fayl juda katta (200 MB dan oshadi)".to_string());
                }
                std::io::Write::write_all(&mut out, &chunk).map_err(|e| e.to_string())?;
            }
            drop(out);
            fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
        }
        Ok(json!({
            "path": path.to_string_lossy(),
            "kind": mi.kind,
            "name": mi.name,
            "mime": mi.mime,
            "size": mi.size,
        }))
    })
}

/// Qidiruv (server): `arg` = so'rov. Javob: `{"results":[{"id","kind","name","username","hash"}]}`.
#[no_mangle]
pub extern "C" fn rust_tg_search(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let q = arg.trim().to_string();
        if q.is_empty() {
            return Ok(json!({"results": []}));
        }
        let res = client.invoke(&tl::functions::contacts::Search { broadcasts: false, bots: false, q, limit: 30 }).await.map_err(|e| e.to_string())?;
        let tl::enums::contacts::Found::Found(f) = res;
        let mut out = Vec::new();
        for u in f.users {
            if let tl::enums::User::User(u) = u {
                let name = format!("{} {}", u.first_name.clone().unwrap_or_default(), u.last_name.clone().unwrap_or_default()).trim().to_string();
                out.push(json!({
                    "id": u.id, "kind": "user", "name": name,
                    "username": u.username.clone().unwrap_or_default(),
                    "hash": u.access_hash.unwrap_or(0).to_string(), "bot": u.bot,
                }));
            }
        }
        for c in f.chats {
            match c {
                tl::enums::Chat::Chat(c) => out.push(json!({"id": -c.id, "kind": "group", "name": c.title, "username": "", "hash": "0", "bot": false})),
                tl::enums::Chat::Channel(c) => out.push(json!({
                    "id": -1_000_000_000_000i64 - c.id,
                    "kind": if c.broadcast { "channel" } else { "group" },
                    "name": c.title,
                    "username": c.username.clone().unwrap_or_default(),
                    "hash": c.access_hash.unwrap_or(0).to_string(),
                    "bot": false,
                })),
                _ => {}
            }
        }
        Ok(json!({ "results": out }))
    })
}

// ── "yozmoqda..." ko'rsatkichi: updates oqimidan to'planadi ──

fn typing_map() -> &'static Mutex<HashMap<i64, i64>> {
    static M: OnceLock<Mutex<HashMap<i64, i64>>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(HashMap::new()))
}

fn note_update(u: &tl::enums::Update) {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let (chat, cancel) = match u {
        tl::enums::Update::UserTyping(x) => (x.user_id, matches!(x.action, tl::enums::SendMessageAction::SendMessageCancelAction)),
        tl::enums::Update::ChatUserTyping(x) => (-x.chat_id, matches!(x.action, tl::enums::SendMessageAction::SendMessageCancelAction)),
        tl::enums::Update::ChannelUserTyping(x) => (-1_000_000_000_000i64 - x.channel_id, matches!(x.action, tl::enums::SendMessageAction::SendMessageCancelAction)),
        _ => return,
    };
    if let Ok(mut m) = typing_map().lock() {
        if cancel {
            m.remove(&chat);
        } else {
            m.insert(chat, now);
        }
    }
}

/// Hozir yozayotganlar: `{"<suhbat_id>": <oxirgi_unix>}` (so'nggi 8 soniya ichida).
#[no_mangle]
pub extern "C" fn rust_tg_typing() -> *mut c_char {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let mut obj = serde_json::Map::new();
    if let Ok(mut m) = typing_map().lock() {
        m.retain(|_, t| now - *t <= 8);
        for (k, v) in m.iter() {
            obj.insert(k.to_string(), json!(*v));
        }
    }
    string_to_cptr(Value::Object(obj).to_string())
}

/// Faol seanslar (qurilmalar): `{"sessions":[{"hash","device","platform","app","date","ip","country","current"}]}`.
#[no_mangle]
pub extern "C" fn rust_tg_sessions() -> *mut c_char {
    tg_call(std::ptr::null(), 30, |client, _| async move {
        let res = client.invoke(&tl::functions::account::GetAuthorizations {}).await.map_err(|e| e.to_string())?;
        let tl::enums::account::Authorizations::Authorizations(a) = res;
        let mut out = Vec::new();
        for x in a.authorizations {
            let tl::enums::Authorization::Authorization(x) = x;
            out.push(json!({
                "hash": x.hash.to_string(),
                "device": x.device_model,
                "platform": format!("{} {}", x.platform, x.system_version).trim().to_string(),
                "app": format!("{} {}", x.app_name, x.app_version).trim().to_string(),
                "date": x.date_active,
                "ip": x.ip,
                "country": x.country,
                "current": x.current,
            }));
        }
        Ok(json!({ "sessions": out }))
    })
}

/// Seansni tugatish: `arg` = hash.
#[no_mangle]
pub extern "C" fn rust_tg_terminate(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let hash = arg.trim().parse::<i64>().map_err(|_| "Seans noto'g'ri".to_string())?;
        client.invoke(&tl::functions::account::ResetAuthorization { hash }).await.map_err(|e| e.to_string())?;
        Ok(json!({"ok": true}))
    })
}

/// Profil: `{"first","last","about","username","phone","id"}`.
#[no_mangle]
pub extern "C" fn rust_tg_profile() -> *mut c_char {
    tg_call(std::ptr::null(), 30, |client, _| async move {
        let me = client.get_me().await.map_err(|e| e.to_string())?;
        let mut about = String::new();
        if let Ok(tl::enums::users::UserFull::Full(f)) = client.invoke(&tl::functions::users::GetFullUser { id: tl::enums::InputUser::UserSelf }).await {
            let tl::enums::UserFull::Full(u) = f.full_user;
            about = u.about.unwrap_or_default();
        }
        Ok(json!({
            "id": me.id().bot_api_dialog_id(),
            "first": me.first_name().unwrap_or_default(),
            "last": me.last_name().unwrap_or_default(),
            "about": about,
            "username": me.username().unwrap_or_default(),
            "phone": me.phone().unwrap_or_default(),
        }))
    })
}

/// Profilni yangilash: `arg` = `"<ism>\n<familiya>\n<bio>"`.
#[no_mangle]
pub extern "C" fn rust_tg_update_profile(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let l = lines(&arg, 3);
        if l[0].trim().is_empty() {
            return Err("Ism bo'sh bo'lmasligi kerak".to_string());
        }
        client
            .invoke(&tl::functions::account::UpdateProfile {
                first_name: Some(l[0].trim().to_string()),
                last_name: Some(l[1].trim().to_string()),
                about: Some(l[2].clone()),
            })
            .await
            .map_err(|e| e.to_string())?;
        Ok(json!({"ok": true}))
    })
}

/// Foydalanuvchi nomini o'zgartirish: `arg` = yangi nom (bo'sh — olib tashlash).
#[no_mangle]
pub extern "C" fn rust_tg_update_username(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        client
            .invoke(&tl::functions::account::UpdateUsername { username: arg.trim().trim_start_matches('@').to_string() })
            .await
            .map_err(|e| match e {
                e if e.is("USERNAME_OCCUPIED") => "Bu nom band".to_string(),
                e if e.is("USERNAME_INVALID") => "Nom noto'g'ri (5–32 belgi: lotin harflari, raqam, _)".to_string(),
                e if e.is("USERNAME_NOT_MODIFIED") => "Nom o'zgarmadi".to_string(),
                e => e.to_string(),
            })?;
        Ok(json!({"ok": true}))
    })
}

/// Yangi hisob ochish (ro'yxatdan o'tish): `arg` = `"<ism>\n<familiya>"`. Kod kiritilgach `signup:true` kelgan holatda chaqiriladi.
#[no_mangle]
pub extern "C" fn rust_tg_sign_up(arg_ptr: *const c_char) -> *mut c_char {
    let arg = unsafe { cstr_to_str(arg_ptr) }.unwrap_or("").to_string();
    string_to_cptr(with_client(|t, client| {
        let l = lines(&arg, 2);
        if l[0].trim().is_empty() {
            return Err("Ismingizni kiriting".to_string());
        }
        let st = load_login(t).ok_or("Kod muddati tugadi — raqamni qayta kiriting")?;
        let phone = st["phone"].as_str().unwrap_or("").to_string();
        let hash = st["hash"].as_str().unwrap_or("").to_string();
        let r = t.rt.block_on(client.invoke(&tl::functions::auth::SignUp {
            no_joined_notifications: false,
            phone_number: phone,
            phone_code_hash: hash,
            first_name: l[0].trim().to_string(),
            last_name: l[1].trim().to_string(),
        }));
        match r {
            Ok(tl::enums::auth::Authorization::Authorization(a)) => {
                save_token(t, a.future_auth_token.as_ref());
                clear_login(t);
                Ok(after_login(t))
            }
            Ok(tl::enums::auth::Authorization::SignUpRequired(_)) => Err("Ro'yxatdan o'tib bo'lmadi — qayta urinib ko'ring".to_string()),
            Err(e) => Err(friendly(&e)),
        }
    }))
}

/// Suhbat/foydalanuvchi haqida: `arg` = suhbat. Javob: `{"kind","name","about","username","phone","members","status"}`.
#[no_mangle]
pub extern "C" fn rust_tg_peer_info(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let peer = resolve_peer(&client, arg.trim()).await?;
        let ip: tl::enums::InputPeer = peer.into();
        match ip {
            tl::enums::InputPeer::User(u) => {
                let res = client
                    .invoke(&tl::functions::users::GetFullUser {
                        id: tl::enums::InputUser::User(tl::types::InputUser { user_id: u.user_id, access_hash: u.access_hash }),
                    })
                    .await
                    .map_err(|e| e.to_string())?;
                let tl::enums::users::UserFull::Full(f) = res;
                let tl::enums::UserFull::Full(full) = f.full_user;
                let mut name = String::new();
                let mut username = String::new();
                let mut phone = String::new();
                let mut status = String::new();
                let mut is_contact = false;
                for x in f.users {
                    if let tl::enums::User::User(x) = x {
                        if x.id == u.user_id {
                            is_contact = x.contact;
                            name = format!("{} {}", x.first_name.clone().unwrap_or_default(), x.last_name.clone().unwrap_or_default()).trim().to_string();
                            username = x.username.clone().unwrap_or_default();
                            phone = x.phone.clone().unwrap_or_default();
                            status = match &x.status {
                                Some(tl::enums::UserStatus::Online(_)) => "onlayn".to_string(),
                                Some(tl::enums::UserStatus::Offline(o)) => format!("@{}", o.was_online),
                                Some(tl::enums::UserStatus::Recently(_)) => "yaqinda onlayn edi".to_string(),
                                Some(tl::enums::UserStatus::LastWeek(_)) => "o'tgan hafta onlayn edi".to_string(),
                                Some(tl::enums::UserStatus::LastMonth(_)) => "o'tgan oy onlayn edi".to_string(),
                                _ => "uzoq vaqt oldin onlayn edi".to_string(),
                            };
                        }
                    }
                }
                // Bot buyruqlari (`/` tugmasi): [[buyruq, tavsif]].
                let commands: Vec<Value> = match &full.bot_info {
                    Some(tl::enums::BotInfo::Info(b)) => b
                        .commands
                        .iter()
                        .flatten()
                        .map(|c| {
                            let tl::enums::BotCommand::Command(c) = c;
                            json!([c.command, c.description])
                        })
                        .collect(),
                    _ => Vec::new(),
                };
                Ok(json!({"kind": "user", "name": name, "about": full.about.unwrap_or_default(), "username": username, "phone": phone, "members": 0, "status": status, "commands": commands, "contact": is_contact}))
            }
            tl::enums::InputPeer::Chat(c) => {
                let res = client.invoke(&tl::functions::messages::GetFullChat { chat_id: c.chat_id }).await.map_err(|e| e.to_string())?;
                let tl::enums::messages::ChatFull::Full(f) = res;
                let (about, members) = match f.full_chat {
                    tl::enums::ChatFull::Full(cf) => {
                        let n = match cf.participants {
                            tl::enums::ChatParticipants::Participants(p) => p.participants.len() as i64,
                            _ => 0,
                        };
                        (cf.about, n)
                    }
                    _ => (String::new(), 0),
                };
                Ok(json!({"kind": "group", "name": "", "about": about, "username": "", "phone": "", "members": members, "status": ""}))
            }
            tl::enums::InputPeer::Channel(c) => {
                let res = client
                    .invoke(&tl::functions::channels::GetFullChannel {
                        channel: tl::enums::InputChannel::Channel(tl::types::InputChannel { channel_id: c.channel_id, access_hash: c.access_hash }),
                    })
                    .await
                    .map_err(|e| e.to_string())?;
                let tl::enums::messages::ChatFull::Full(f) = res;
                let mut username = String::new();
                for ch in &f.chats {
                    if let tl::enums::Chat::Channel(ch) = ch {
                        if ch.id == c.channel_id {
                            username = ch.username.clone().unwrap_or_default();
                        }
                    }
                }
                let (about, members) = match f.full_chat {
                    tl::enums::ChatFull::ChannelFull(cf) => (cf.about, cf.participants_count.unwrap_or(0) as i64),
                    _ => (String::new(), 0),
                };
                Ok(json!({"kind": "channel", "name": "", "about": about, "username": username, "phone": "", "members": members, "status": ""}))
            }
            _ => Err("Bu suhbat turi qo'llab-quvvatlanmaydi".to_string()),
        }
    })
}

/// Suhbat ichida qidirish: `arg` = `"<suhbat>\n<so'rov>"`. Javob: `{"messages":[..]}`.
#[no_mangle]
pub extern "C" fn rust_tg_search_in_chat(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let l = lines(&arg, 2);
        let peer = resolve_peer(&client, &l[0]).await?;
        let mut it = client.search_messages(peer).query(l[1].trim()).limit(50);
        let mut out = Vec::new();
        while let Some(m) = it.next().await.map_err(|e| e.to_string())? {
            out.push(msg_json(&m));
        }
        Ok(json!({ "messages": out }))
    })
}

/// Mahkamlangan xabar(lar): `arg` = suhbat. Javob: `{"messages":[..]}` (eng yangisi birinchi).
#[no_mangle]
pub extern "C" fn rust_tg_pinned(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let peer = resolve_peer(&client, arg.trim()).await?;
        let mut it = client.search_messages(peer).filter(tl::enums::MessagesFilter::InputMessagesFilterPinned).limit(20);
        let mut out = Vec::new();
        while let Some(m) = it.next().await.map_err(|e| e.to_string())? {
            out.push(msg_json(&m));
        }
        Ok(json!({ "messages": out }))
    })
}

/// Reaksiya qo'yish/olish: `arg` = `"<suhbat>\n<xabar_id>\n<emoji|bo'sh>"` (bo'sh — reaksiyani olib tashlash).
#[no_mangle]
pub extern "C" fn rust_tg_react(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let l = lines(&arg, 3);
        let id = l[1].trim().parse::<i32>().map_err(|_| "Xabar ko'rsatilmagan".to_string())?;
        let peer = resolve_peer(&client, &l[0]).await?;
        let r = if l[2].trim().is_empty() {
            grammers_client::message::InputReactions::remove()
        } else {
            grammers_client::message::InputReactions::emoticon(l[2].trim().to_string())
        };
        client.send_reactions(peer, id, r).await.map_err(|e| e.to_string())?;
        Ok(json!({"ok": true}))
    })
}

fn input_peer_bot_id(p: &tl::enums::InputPeer) -> Option<i64> {
    match p {
        tl::enums::InputPeer::User(u) => Some(u.user_id),
        tl::enums::InputPeer::Chat(c) => Some(-c.chat_id),
        tl::enums::InputPeer::Channel(c) => Some(-1_000_000_000_000i64 - c.channel_id),
        _ => None,
    }
}

/// Suhbat jildlari: `{"folders":[{"id","title","contacts","non_contacts","groups","broadcasts","bots","exclude_muted","exclude_read","include":[id..],"exclude":[id..]}]}`.
#[no_mangle]
pub extern "C" fn rust_tg_folders() -> *mut c_char {
    tg_call(std::ptr::null(), 30, |client, _| async move {
        let res = client.invoke(&tl::functions::messages::GetDialogFilters {}).await.map_err(|e| e.to_string())?;
        let tl::enums::messages::DialogFilters::Filters(f) = res;
        let mut out = Vec::new();
        for x in f.filters {
            let tl::enums::DialogFilter::Filter(x) = x else { continue };
            let tl::enums::TextWithEntities::Entities(title) = &x.title;
            let include: Vec<i64> = x.include_peers.iter().chain(x.pinned_peers.iter()).filter_map(input_peer_bot_id).collect();
            let exclude: Vec<i64> = x.exclude_peers.iter().filter_map(input_peer_bot_id).collect();
            out.push(json!({
                "id": x.id,
                "title": title.text,
                "contacts": x.contacts,
                "non_contacts": x.non_contacts,
                "groups": x.groups,
                "broadcasts": x.broadcasts,
                "bots": x.bots,
                "exclude_muted": x.exclude_muted,
                "exclude_read": x.exclude_read,
                "include": include,
                "exclude": exclude,
            }));
        }
        Ok(json!({ "folders": out }))
    })
}

/// Yangi guruh: `arg` = `"<nom>\n<id@hash,id@hash,..>"` (a'zolar — kontaktlar).
#[no_mangle]
pub extern "C" fn rust_tg_create_group(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 40, |client, arg| async move {
        let l = lines(&arg, 2);
        if l[0].trim().is_empty() {
            return Err("Guruh nomini kiriting".to_string());
        }
        let mut users = Vec::new();
        for part in l[1].split(',') {
            if let Some((id, hash)) = part.trim().split_once('@') {
                if let (Ok(user_id), Ok(access_hash)) = (id.parse::<i64>(), hash.parse::<i64>()) {
                    users.push(tl::enums::InputUser::User(tl::types::InputUser { user_id, access_hash }));
                }
            }
        }
        if users.is_empty() {
            return Err("Kamida bitta a'zo tanlang".to_string());
        }
        client
            .invoke(&tl::functions::messages::CreateChat { users, title: l[0].trim().to_string(), ttl_period: None })
            .await
            .map_err(|e| e.to_string())?;
        Ok(json!({"ok": true}))
    })
}

/// Yangi kanal: `arg` = `"<nom>\n<tavsif>"`.
#[no_mangle]
pub extern "C" fn rust_tg_create_channel(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 40, |client, arg| async move {
        let l = lines(&arg, 2);
        if l[0].trim().is_empty() {
            return Err("Kanal nomini kiriting".to_string());
        }
        client
            .invoke(&tl::functions::channels::CreateChannel {
                broadcast: true,
                megagroup: false,
                for_import: false,
                forum: false,
                title: l[0].trim().to_string(),
                about: l[1].clone(),
                geo_point: None,
                address: None,
                ttl_period: None,
            })
            .await
            .map_err(|e| e.to_string())?;
        Ok(json!({"ok": true}))
    })
}

/// Mening stiker to'plamlarim: `{"sets":[{"id","hash","title","count"}]}`.
#[no_mangle]
pub extern "C" fn rust_tg_sticker_sets() -> *mut c_char {
    tg_call(std::ptr::null(), 30, |client, _| async move {
        let res = client.invoke(&tl::functions::messages::GetAllStickers { hash: 0 }).await.map_err(|e| e.to_string())?;
        let tl::enums::messages::AllStickers::Stickers(a) = res else { return Ok(json!({"sets": []})) };
        let mut out = Vec::new();
        for st in a.sets {
            let tl::enums::StickerSet::Set(s) = st;
            if s.masks || s.emojis {
                continue;
            }
            out.push(json!({"id": s.id.to_string(), "hash": s.access_hash.to_string(), "title": s.title, "count": s.count}));
        }
        Ok(json!({ "sets": out }))
    })
}

/// To'plamdagi stikerlar: `arg` = `"<id>:<hash>"`. Javob: `{"stickers":[{"ref":"<id>:<hash>:<fayl_havolasi_hex>","mime","alt"}]}`.
#[no_mangle]
pub extern "C" fn rust_tg_sticker_set(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let mut p = arg.trim().split(':');
        let id = p.next().and_then(|x| x.parse::<i64>().ok()).ok_or("To'plam noto'g'ri")?;
        let hash = p.next().and_then(|x| x.parse::<i64>().ok()).ok_or("To'plam noto'g'ri")?;
        let res = client
            .invoke(&tl::functions::messages::GetStickerSet {
                stickerset: tl::enums::InputStickerSet::Id(tl::types::InputStickerSetId { id, access_hash: hash }),
                hash: 0,
            })
            .await
            .map_err(|e| e.to_string())?;
        let tl::enums::messages::StickerSet::Set(set) = res else { return Ok(json!({"stickers": []})) };
        let mut out = Vec::new();
        for d in set.documents {
            let tl::enums::Document::Document(d) = d else { continue };
            let mut alt = String::new();
            for a in &d.attributes {
                if let tl::enums::DocumentAttribute::Sticker(st) = a {
                    alt = st.alt.clone();
                }
            }
            out.push(json!({
                "ref": format!("{}:{}:{}", d.id, d.access_hash, hex::encode(&d.file_reference)),
                "mime": d.mime_type,
                "alt": alt,
            }));
        }
        Ok(json!({ "stickers": out }))
    })
}

fn parse_doc_ref(s: &str) -> Option<(i64, i64, Vec<u8>)> {
    let mut p = s.trim().split(':');
    let id = p.next()?.parse::<i64>().ok()?;
    let hash = p.next()?.parse::<i64>().ok()?;
    let r = hex::decode(p.next()?).ok()?;
    Some((id, hash, r))
}

/// Stiker rasmi (webp): `arg` = `ref`. Javob: `{"data":"<base64>"}`.
#[no_mangle]
pub extern "C" fn rust_tg_sticker_file(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 40, |client, arg| async move {
        let (id, access_hash, file_reference) = parse_doc_ref(&arg).ok_or("Stiker noto'g'ri")?;
        let loc = tl::enums::InputFileLocation::InputDocumentFileLocation(tl::types::InputDocumentFileLocation {
            id,
            access_hash,
            file_reference,
            thumb_size: String::new(),
        });
        let Some(b) = download_small(&client, loc).await else { return Err("Stiker yuklanmadi".to_string()) };
        Ok(json!({ "data": b64(&b) }))
    })
}

/// Stiker yuborish: `arg` = `"<suhbat>\n<javob_id|0>\n<ref>"`.
#[no_mangle]
pub extern "C" fn rust_tg_send_sticker(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let l = lines(&arg, 3);
        let (id, access_hash, file_reference) = parse_doc_ref(&l[2]).ok_or("Stiker noto'g'ri")?;
        let peer = resolve_peer(&client, &l[0]).await?;
        let reply = l[1].trim().parse::<i32>().ok().filter(|x| *x > 0);
        let media = tl::types::InputMediaDocument {
            spoiler: false,
            id: tl::enums::InputDocument::Document(tl::types::InputDocument { id, access_hash, file_reference }),
            video_cover: None,
            video_timestamp: None,
            ttl_seconds: None,
            query: None,
        };
        let msg = grammers_client::message::InputMessage::new().reply_to(reply).media(tl::enums::InputMedia::Document(media));
        client.send_message(peer, msg).await.map_err(|e| e.to_string())?;
        Ok(json!({"ok": true}))
    })
}

/// @nom yoki t.me havolasi bo'yicha suhbatni topish: `arg` = nom. Javob: `{"id","kind","name","hash","username"}`.
#[no_mangle]
pub extern "C" fn rust_tg_resolve(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let name = arg.trim().trim_start_matches('@').to_string();
        let peer = client.resolve_username(&name).await.map_err(|e| e.to_string())?.ok_or("Bunday foydalanuvchi yoki kanal topilmadi")?;
        let (kind, title) = match &peer {
            grammers_client::peer::Peer::User(u) => ("user", u.full_name()),
            grammers_client::peer::Peer::Group(g) => ("group", g.title().unwrap_or_default().to_string()),
            grammers_client::peer::Peer::Channel(c) => ("channel", c.title().to_string()),
        };
        let r = peer.to_ref().await.map_err(|e| e.to_string())?.ok_or("Suhbatga murojaat qilib bo'lmadi")?;
        Ok(json!({
            "id": peer.id().bot_api_dialog_id(),
            "kind": kind,
            "name": title,
            "hash": r.auth.hash().to_string(),
            "username": peer.username().unwrap_or_default(),
        }))
    })
}

/// Kanal/guruhga qo'shilish: `arg` = suhbat.
#[no_mangle]
pub extern "C" fn rust_tg_join(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let peer = resolve_peer(&client, arg.trim()).await?;
        let ip: tl::enums::InputPeer = peer.into();
        match ip {
            tl::enums::InputPeer::Channel(c) => {
                client
                    .invoke(&tl::functions::channels::JoinChannel {
                        channel: tl::enums::InputChannel::Channel(tl::types::InputChannel { channel_id: c.channel_id, access_hash: c.access_hash }),
                    })
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(json!({"ok": true}))
            }
            _ => Err("Bu suhbatga qo'shilib bo'lmaydi".to_string()),
        }
    })
}

/// "Yozmoqda..." belgisini yuborish: `arg` = suhbat.
#[no_mangle]
pub extern "C" fn rust_tg_typing_send(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 15, |client, arg| async move {
        let peer = resolve_peer(&client, arg.trim()).await?;
        let ip: tl::enums::InputPeer = peer.into();
        client
            .invoke(&tl::functions::messages::SetTyping { peer: ip, top_msg_id: None, action: tl::enums::SendMessageAction::SendMessageTypingAction })
            .await
            .map_err(|e| e.to_string())?;
        Ok(json!({"ok": true}))
    })
}

/// Onlayn/oflayn holati: `arg` = `"1"` (onlayn) yoki `"0"` (oflayn).
#[no_mangle]
pub extern "C" fn rust_tg_set_online(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 15, |client, arg| async move {
        client.invoke(&tl::functions::account::UpdateStatus { offline: arg.trim() != "1" }).await.map_err(|e| e.to_string())?;
        Ok(json!({"ok": true}))
    })
}

/// Foydalanuvchini bloklash/ochish: `arg` = `"<suhbat>\n<1|0>"`.
#[no_mangle]
pub extern "C" fn rust_tg_block(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let l = lines(&arg, 2);
        let peer = resolve_peer(&client, &l[0]).await?;
        let ip: tl::enums::InputPeer = peer.into();
        if l[1].trim() == "1" {
            client.invoke(&tl::functions::contacts::Block { my_stories_from: false, id: ip }).await.map_err(|e| e.to_string())?;
        } else {
            client.invoke(&tl::functions::contacts::Unblock { my_stories_from: false, id: ip }).await.map_err(|e| e.to_string())?;
        }
        Ok(json!({"ok": true}))
    })
}

/// Kontakt qo'shish: `arg` = `"<telefon>\n<ism>\n<familiya>"`.
#[no_mangle]
pub extern "C" fn rust_tg_add_contact(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let l = lines(&arg, 3);
        let phone: String = l[0].chars().filter(|c| c.is_ascii_digit() || *c == '+').collect();
        if phone.len() < 5 || l[1].trim().is_empty() {
            return Err("Telefon raqami va ismni kiriting".to_string());
        }
        let res = client
            .invoke(&tl::functions::contacts::ImportContacts {
                contacts: vec![tl::enums::InputContact::InputPhoneContact(tl::types::InputPhoneContact {
                    client_id: 0,
                    phone,
                    first_name: l[1].trim().to_string(),
                    last_name: l[2].trim().to_string(),
                    note: None,
                })],
            })
            .await
            .map_err(|e| e.to_string())?;
        let tl::enums::contacts::ImportedContacts::Contacts(c) = res;
        if c.imported.is_empty() {
            return Err("Bu raqamda Telegram hisobi topilmadi".to_string());
        }
        Ok(json!({"ok": true}))
    })
}

/// Taklif havolasi (t.me/+hash) bo'yicha qo'shilish: `arg` = hash.
#[no_mangle]
pub extern "C" fn rust_tg_import_invite(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        client
            .invoke(&tl::functions::messages::ImportChatInvite { hash: arg.trim().trim_start_matches('+').to_string() })
            .await
            .map_err(|e| match e {
                e if e.is("INVITE_HASH_EXPIRED") || e.is("INVITE_HASH_INVALID") => "Taklif havolasi eskirgan yoki noto'g'ri".to_string(),
                e if e.is("USER_ALREADY_PARTICIPANT") => "Siz allaqachon a'zosiz".to_string(),
                e => e.to_string(),
            })?;
        Ok(json!({"ok": true}))
    })
}

/// Guruh/kanal a'zolari: `arg` = suhbat. Javob: `{"members":[{"id","name","hash","role"}]}` (200 tagacha).
#[no_mangle]
pub extern "C" fn rust_tg_members(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 40, |client, arg| async move {
        let peer = resolve_peer(&client, arg.trim()).await?;
        let mut it = client.iter_participants(peer);
        let mut out = Vec::new();
        while out.len() < 200 {
            let Some(p) = it.next().await.map_err(|e| e.to_string())? else { break };
            let role = match p.role {
                grammers_client::peer::Role::Creator(_) => "egasi",
                grammers_client::peer::Role::Admin(_) => "admin",
                _ => "",
            };
            let hash = match p.user.to_ref().await {
                Ok(Some(r)) => r.auth.hash().to_string(),
                _ => "0".to_string(),
            };
            out.push(json!({"id": p.user.id().bot_api_dialog_id(), "name": p.user.full_name(), "hash": hash, "role": role, "bot": p.user.is_bot()}));
        }
        Ok(json!({ "members": out }))
    })
}

/// Ulashilgan media: `arg` = `"<suhbat>\n<photos|video|docs|links|voice|music>"`. Javob: `{"messages":[..]}`.
#[no_mangle]
pub extern "C" fn rust_tg_shared(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 40, |client, arg| async move {
        let l = lines(&arg, 2);
        let peer = resolve_peer(&client, &l[0]).await?;
        let filter = match l[1].trim() {
            "photos" => tl::enums::MessagesFilter::InputMessagesFilterPhotos,
            "video" => tl::enums::MessagesFilter::InputMessagesFilterVideo,
            "docs" => tl::enums::MessagesFilter::InputMessagesFilterDocument,
            "links" => tl::enums::MessagesFilter::InputMessagesFilterUrl,
            "voice" => tl::enums::MessagesFilter::InputMessagesFilterVoice,
            "music" => tl::enums::MessagesFilter::InputMessagesFilterMusic,
            _ => tl::enums::MessagesFilter::InputMessagesFilterPhotoVideo,
        };
        let mut it = client.search_messages(peer).filter(filter).limit(60);
        let mut out = Vec::new();
        while let Some(m) = it.next().await.map_err(|e| e.to_string())? {
            out.push(msg_json(&m));
        }
        Ok(json!({ "messages": out }))
    })
}

/// So'rovnomada ovoz berish: `arg` = `"<suhbat>\n<xabar_id>\n<variant_hex,variant_hex>"` (bo'sh — ovozni qaytarib olish).
#[no_mangle]
pub extern "C" fn rust_tg_vote(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let l = lines(&arg, 3);
        let id = l[1].trim().parse::<i32>().map_err(|_| "Xabar ko'rsatilmagan".to_string())?;
        let peer = resolve_peer(&client, &l[0]).await?;
        let ip: tl::enums::InputPeer = peer.into();
        let options: Vec<Vec<u8>> = l[2].split(',').filter(|x| !x.trim().is_empty()).filter_map(|x| hex::decode(x.trim()).ok()).collect();
        client.invoke(&tl::functions::messages::SendVote { peer: ip, msg_id: id, options }).await.map_err(|e| e.to_string())?;
        Ok(json!({"ok": true}))
    })
}

/// Inline tugma bosish: `arg` = `"<suhbat>\n<xabar_id>\n<data_hex>"`. Javob: `{"message","alert","url"}`.
#[no_mangle]
pub extern "C" fn rust_tg_callback(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let l = lines(&arg, 3);
        let id = l[1].trim().parse::<i32>().map_err(|_| "Xabar ko'rsatilmagan".to_string())?;
        let peer = resolve_peer(&client, &l[0]).await?;
        let ip: tl::enums::InputPeer = peer.into();
        let data = hex::decode(l[2].trim()).map_err(|_| "Tugma noto'g'ri".to_string())?;
        let res = client
            .invoke(&tl::functions::messages::GetBotCallbackAnswer { game: false, peer: ip, msg_id: id, data: Some(data), password: None })
            .await
            .map_err(|e| e.to_string())?;
        let tl::enums::messages::BotCallbackAnswer::Answer(a) = res;
        Ok(json!({"message": a.message.unwrap_or_default(), "alert": a.alert, "url": a.url.unwrap_or_default()}))
    })
}

/// Guruh/kanal nomi va tavsifini o'zgartirish: `arg` = `"<suhbat>\n<nom>\n<tavsif>"`.
#[no_mangle]
pub extern "C" fn rust_tg_edit_chat(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let l = lines(&arg, 3);
        let peer = resolve_peer(&client, &l[0]).await?;
        let ip: tl::enums::InputPeer = peer.into();
        let title = l[1].trim().to_string();
        if !title.is_empty() {
            match &ip {
                tl::enums::InputPeer::Chat(c) => {
                    client.invoke(&tl::functions::messages::EditChatTitle { chat_id: c.chat_id, title: title.clone() }).await.map_err(|e| e.to_string())?;
                }
                tl::enums::InputPeer::Channel(c) => {
                    let r = client
                        .invoke(&tl::functions::channels::EditTitle {
                            channel: tl::enums::InputChannel::Channel(tl::types::InputChannel { channel_id: c.channel_id, access_hash: c.access_hash }),
                            title: title.clone(),
                        })
                        .await;
                    if let Err(e) = r {
                        if !e.is("CHAT_NOT_MODIFIED") {
                            return Err(e.to_string());
                        }
                    }
                }
                _ => return Err("Bu suhbat nomini o'zgartirib bo'lmaydi".to_string()),
            }
        }
        client.invoke(&tl::functions::messages::EditChatAbout { peer: ip, about: l[2].clone() }).await.ok();
        Ok(json!({"ok": true}))
    })
}

/// Guruh/kanalga a'zo qo'shish: `arg` = `"<suhbat>\n<id@hash,id@hash,..>"`.
#[no_mangle]
pub extern "C" fn rust_tg_add_members(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 40, |client, arg| async move {
        let l = lines(&arg, 2);
        let peer = resolve_peer(&client, &l[0]).await?;
        let ip: tl::enums::InputPeer = peer.into();
        let mut users = Vec::new();
        for part in l[1].split(',') {
            if let Some((id, hash)) = part.trim().split_once('@') {
                if let (Ok(user_id), Ok(access_hash)) = (id.parse::<i64>(), hash.parse::<i64>()) {
                    users.push(tl::enums::InputUser::User(tl::types::InputUser { user_id, access_hash }));
                }
            }
        }
        if users.is_empty() {
            return Err("A'zo tanlanmagan".to_string());
        }
        match ip {
            tl::enums::InputPeer::Chat(c) => {
                for u in users {
                    client
                        .invoke(&tl::functions::messages::AddChatUser { chat_id: c.chat_id, user_id: u, fwd_limit: 50 })
                        .await
                        .map_err(|e| e.to_string())?;
                }
            }
            tl::enums::InputPeer::Channel(c) => {
                client
                    .invoke(&tl::functions::channels::InviteToChannel {
                        channel: tl::enums::InputChannel::Channel(tl::types::InputChannel { channel_id: c.channel_id, access_hash: c.access_hash }),
                        users,
                    })
                    .await
                    .map_err(|e| e.to_string())?;
            }
            _ => return Err("Bu suhbatga a'zo qo'shib bo'lmaydi".to_string()),
        }
        Ok(json!({"ok": true}))
    })
}

/// Profil rasmini o'rnatish: `arg` = rasm fayli yo'li.
#[no_mangle]
pub extern "C" fn rust_tg_set_photo(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 120, |client, arg| async move {
        let path = arg.trim().to_string();
        let mut f = tokio::fs::File::open(&path).await.map_err(|e| format!("Fayl ochilmadi: {e}"))?;
        let size = f.metadata().await.map_err(|e| e.to_string())?.len() as usize;
        let up = client.upload_stream(&mut f, size, "profil.jpg".to_string()).await.map_err(|e| e.to_string())?;
        client
            .invoke(&tl::functions::photos::UploadProfilePhoto { fallback: false, bot: None, file: Some(up.raw), video: None, video_start_ts: None, video_emoji_markup: None })
            .await
            .map_err(|e| e.to_string())?;
        Ok(json!({"ok": true}))
    })
}

/// Suhbat tarixini tozalash (suhbat qoladi): `arg` = suhbat.
#[no_mangle]
pub extern "C" fn rust_tg_clear_history(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 60, |client, arg| async move {
        let peer = resolve_peer(&client, arg.trim()).await?;
        let ip: tl::enums::InputPeer = peer.into();
        match ip {
            tl::enums::InputPeer::Channel(c) => {
                client
                    .invoke(&tl::functions::channels::DeleteHistory {
                        for_everyone: false,
                        channel: tl::enums::InputChannel::Channel(tl::types::InputChannel { channel_id: c.channel_id, access_hash: c.access_hash }),
                        max_id: 0,
                    })
                    .await
                    .map_err(|e| e.to_string())?;
            }
            other => {
                client
                    .invoke(&tl::functions::messages::DeleteHistory { just_clear: true, revoke: false, peer: other, max_id: 0, min_date: None, max_date: None })
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(json!({"ok": true}))
    })
}

/// Taklif havolasi: `arg` = suhbat. Javob: `{"link"}`.
#[no_mangle]
pub extern "C" fn rust_tg_invite_link(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let peer = resolve_peer(&client, arg.trim()).await?;
        let ip: tl::enums::InputPeer = peer.into();
        let res = client
            .invoke(&tl::functions::messages::ExportChatInvite {
                legacy_revoke_permanent: false,
                request_needed: false,
                peer: ip,
                expire_date: None,
                usage_limit: None,
                title: None,
                subscription_pricing: None,
            })
            .await
            .map_err(|e| e.to_string())?;
        match res {
            tl::enums::ExportedChatInvite::ChatInviteExported(x) => Ok(json!({"link": x.link})),
            _ => Err("Havola olinmadi".to_string()),
        }
    })
}

fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

/// Faylni xotiraga yuklab oladi (kichik rasm uchun; 1 MB dan oshsa to'xtaydi).
async fn download_small(client: &Client, loc: tl::enums::InputFileLocation) -> Option<Vec<u8>> {
    let photo = grammers_client::media::ChatPhoto { raw: loc };
    let mut it = client.iter_download(&photo);
    let mut buf = Vec::new();
    while let Some(chunk) = it.next().await.ok()? {
        buf.extend(chunk);
        if buf.len() > 1024 * 1024 {
            return None;
        }
    }
    (!buf.is_empty()).then_some(buf)
}

fn channel_json(c: &tl::types::Channel) -> Value {
    json!({
        "id": -1_000_000_000_000 - c.id,
        "title": c.title,
        "username": c.username.clone().or_else(|| c.usernames.as_ref().and_then(|u| u.iter().find_map(|x| match x {
            tl::enums::Username::Username(x) if x.active => Some(x.username.clone()),
            _ => None,
        }))).unwrap_or_default(),
        "verified": c.verified,
        "broadcast": c.broadcast,
        "megagroup": c.megagroup,
        "members": c.participants_count.unwrap_or(0),
        "member": !c.left,
        "join_request": c.join_request,
    })
}

async fn channel_info(client: &Client, input: &str) -> Result<Value, String> {
    // ── YOPIQ HAVOLA ──
    if let Some(hash) = invite_hash(input) {
        let inv = client
            .invoke(&tl::functions::messages::CheckChatInvite { hash })
            .await
            .map_err(|e| match rpc_name(&e) {
                Some("INVITE_HASH_EXPIRED") | Some("INVITE_HASH_INVALID") => "Havola yaroqsiz yoki muddati o'tgan".to_string(),
                _ => inv_err(&e),
            })?;
        let chat = match inv {
            tl::enums::ChatInvite::Already(a) => a.chat,
            tl::enums::ChatInvite::Peek(p) => p.chat,
            tl::enums::ChatInvite::Invite(i) => {
                // A'zo emas — kanal IDsi noma'lum, ma'lumot havoladan.
                let mut v = json!({
                    "ok": true, "id": 0, "title": i.title, "username": "",
                    "about": i.about.unwrap_or_default(), "members": i.participants_count,
                    "verified": i.verified, "broadcast": i.broadcast, "megagroup": i.megagroup,
                    "public": i.public, "member": false, "join_request": i.request_needed,
                });
                if let tl::enums::Photo::Photo(p) = i.photo {
                    let thumb = p.sizes.iter().find_map(|s| match s {
                        tl::enums::PhotoSize::Size(x) if x.r#type == "a" => Some(x.r#type.clone()),
                        _ => None,
                    }).or_else(|| p.sizes.iter().find_map(|s| match s {
                        tl::enums::PhotoSize::Size(x) => Some(x.r#type.clone()),
                        tl::enums::PhotoSize::Progressive(x) => Some(x.r#type.clone()),
                        _ => None,
                    }));
                    if let Some(thumb_size) = thumb {
                        let loc = tl::enums::InputFileLocation::InputPhotoFileLocation(tl::types::InputPhotoFileLocation {
                            id: p.id, access_hash: p.access_hash, file_reference: p.file_reference, thumb_size,
                        });
                        if let Some(b) = download_small(client, loc).await {
                            v["photo"] = json!(b64(&b));
                        }
                    }
                }
                return Ok(v);
            }
        };
        return full_info(client, chat).await;
    }
    // ── OCHIQ NOM ──
    if let Some(name) = public_name(input) {
        let tl::enums::contacts::ResolvedPeer::Peer(rp) = client
            .invoke(&tl::functions::contacts::ResolveUsername { username: name, referer: None })
            .await
            .map_err(|e| match rpc_name(&e) {
                Some("USERNAME_NOT_OCCUPIED") | Some("USERNAME_INVALID") => "Bunday kanal topilmadi".to_string(),
                _ => inv_err(&e),
            })?;
        let chat = rp.chats.into_iter()
            .find(|c| matches!(c, tl::enums::Chat::Channel(_)))
            .ok_or("Bu nom kanalga tegishli emas (foydalanuvchi yoki bot)")?;
        return full_info(client, chat).await;
    }
    // ── ID ──
    let raw: i64 = input.parse().map_err(|_| "Kanalni @username, -100... ID yoki havola bilan yozing".to_string())?;
    let id = if raw < 0 { raw } else { -1_000_000_000_000 - raw };
    let (cid, hash) = match find_channel(client, id).await?.into() {
        tl::enums::InputPeer::Channel(p) => (p.channel_id, p.access_hash),
        _ => return Err("bu kanal emas".to_string()),
    };
    let res = client
        .invoke(&tl::functions::channels::GetChannels {
            id: vec![tl::enums::InputChannel::Channel(tl::types::InputChannel { channel_id: cid, access_hash: hash })],
        })
        .await
        .map_err(|e| inv_err(&e))?;
    let chats = match res {
        tl::enums::messages::Chats::Chats(c) => c.chats,
        tl::enums::messages::Chats::Slice(c) => c.chats,
    };
    let chat = chats.into_iter().next().ok_or("Kanal topilmadi")?;
    full_info(client, chat).await
}

/// Kanal obyektidan to'liq ma'lumot (obunachilar, tavsif, surat).
async fn full_info(client: &Client, chat: tl::enums::Chat) -> Result<Value, String> {
    let tl::enums::Chat::Channel(c) = chat else {
        return Err("Bu kanal emas (oddiy guruh)".to_string());
    };
    let mut v = channel_json(&c);
    v["ok"] = json!(true);
    v["public"] = json!(!v["username"].as_str().unwrap_or("").is_empty());
    let Some(hash) = c.access_hash else {
        return Ok(v);
    };
    let input = tl::enums::InputChannel::Channel(tl::types::InputChannel { channel_id: c.id, access_hash: hash });
    if let Ok(tl::enums::messages::ChatFull::Full(f)) = client
        .invoke(&tl::functions::channels::GetFullChannel { channel: input })
        .await
    {
        if let tl::enums::ChatFull::ChannelFull(cf) = f.full_chat {
            v["about"] = json!(cf.about);
            if let Some(n) = cf.participants_count {
                v["members"] = json!(n);
            }
        }
    }
    if let tl::enums::ChatPhoto::Photo(p) = &c.photo {
        let loc = tl::enums::InputFileLocation::InputPeerPhotoFileLocation(tl::types::InputPeerPhotoFileLocation {
            big: false,
            peer: tl::enums::InputPeer::Channel(tl::types::InputPeerChannel { channel_id: c.id, access_hash: hash }),
            photo_id: p.photo_id,
        });
        if let Some(b) = download_small(client, loc).await {
            v["photo"] = json!(b64(&b));
        }
    }
    Ok(v)
}

/// Admin paneli — kodlash holati JONLI: log kanalidagi qadalgan
/// `#arustatus` xabarini adminning O'Z Telegram hisobi bilan o'qiydi
/// (`channels.getMessages`). Worker ham, baza ham ishtirok etmaydi —
/// panel ochiq turganda ~2 soniyada bir chaqiriladi.
///
/// Kanal (`channel_id`, `access_hash`) va qadalgan xabar raqami eslab
/// qolinadi: har chaqiruv odatda BITTA so'rov. Xabar raqami 30 soniyada
/// bir marta (yoki xabar topilmasa) `channels.getFullChannel` bilan
/// yangilanadi — runner yangi xabar qadagan bo'lishi mumkin.
///
/// Argument: Bot API ko'rinishidagi kanal ID (`-100...`).
/// Javob: `{"ok":true,"text":"...","edit_date":unix}` yoki `{"error":...}`.
#[no_mangle]
pub extern "C" fn rust_tg_read_pinned(arg_ptr: *const c_char) -> *mut c_char {
    let chat: i64 = unsafe { cstr_to_str(arg_ptr) }.unwrap_or("").trim().parse().unwrap_or(0);
    string_to_cptr(with_client(|t, client| {
        if !t.authorized.load(Ordering::SeqCst) {
            return Err("Telegram ulanmagan".to_string());
        }
        if chat >= 0 {
            return Err("kanal ID noto'g'ri".to_string());
        }
        let (text, edit) = run_tmo(t, 20, read_pinned(&client, chat))?;
        Ok(json!({"ok": true, "text": text, "edit_date": edit}).to_string())
    }))
}

/// (kanal ID, channel_id, access_hash, qadalgan xabar, raqam olingan payt).
static PINNED: Mutex<Option<(i64, i64, i64, i32, Instant)>> = Mutex::new(None);

async fn read_pinned(client: &Client, chat: i64) -> Result<(String, i64), String> {
    let cached = PINNED.lock().ok().and_then(|g| *g).filter(|c| c.0 == chat);
    let (cid, hash) = match cached {
        Some(c) => (c.1, c.2),
        None => match find_channel(client, chat).await?.into() {
            tl::enums::InputPeer::Channel(p) => (p.channel_id, p.access_hash),
            _ => return Err("bu kanal emas".to_string()),
        },
    };
    let channel = || tl::enums::InputChannel::Channel(tl::types::InputChannel { channel_id: cid, access_hash: hash });
    let mut msg_id = cached
        .filter(|c| c.3 > 0 && c.4.elapsed() < Duration::from_secs(30))
        .map(|c| c.3)
        .unwrap_or(0);
    if msg_id == 0 {
        let full = client
            .invoke(&tl::functions::channels::GetFullChannel { channel: channel() })
            .await
            .map_err(|e| inv_err(&e))?;
        let tl::enums::messages::ChatFull::Full(f) = full;
        msg_id = match f.full_chat {
            tl::enums::ChatFull::ChannelFull(cf) => cf.pinned_msg_id.unwrap_or(0),
            _ => 0,
        };
        if let Ok(mut g) = PINNED.lock() {
            *g = Some((chat, cid, hash, msg_id, Instant::now()));
        }
    }
    if msg_id <= 0 {
        return Err("kanalda qadalgan holat xabari yo'q".to_string());
    }
    let res = client
        .invoke(&tl::functions::channels::GetMessages {
            channel: channel(),
            id: vec![tl::enums::InputMessage::Id(tl::types::InputMessageId { id: msg_id })],
        })
        .await
        .map_err(|e| inv_err(&e))?;
    let msgs = match res {
        tl::enums::messages::Messages::Messages(m) => m.messages,
        tl::enums::messages::Messages::Slice(m) => m.messages,
        tl::enums::messages::Messages::ChannelMessages(m) => m.messages,
        tl::enums::messages::Messages::NotModified(_) => Vec::new(),
    };
    for m in msgs {
        if let tl::enums::Message::Message(m) = m {
            return Ok((m.message, m.edit_date.unwrap_or(m.date) as i64));
        }
    }
    // Xabar o'chirilgan — keyingi safar raqam qaytadan olinadi.
    if let Ok(mut g) = PINNED.lock() {
        *g = Some((chat, cid, hash, 0, Instant::now()));
    }
    Err("qadalgan xabar topilmadi".to_string())
}

/// Admin paneli: ASOSIY botni kanalga ADMIN qiladi — adminning O'Z
/// Telegram hisobi bilan (`channels.editAdmin`), Telegram botiga
/// kirmasdan (foydalanuvchi talabi: "kanal IDsi yoki useri berilganda
/// kanalni o'zi topib admin qilsin, Telegram botsiz ham ishlasin").
///
/// Argument: `@nom`, `https://t.me/nom` yoki `-100...`. Ochiq kanal nomi
/// bo'yicha topiladi, ID bo'yicha esa adminning suhbatlari orasidan
/// (kanal egasi/admini bo'lgani uchun u yerda bor). Huquqlar —
/// aniraxuzbot15 dagidek: post yozish/tahrirlash/o'chirish, havola
/// orqali taklif qilish (yopiq kanal so'rovlari uchun shart), boshqarish.
///
/// Javob: `{"ok":true,"chat_id":-100...}` (Bot API ko'rinishidagi ID).
#[no_mangle]
pub extern "C" fn rust_tg_make_bot_admin(arg_ptr: *const c_char) -> *mut c_char {
    let target = unsafe { cstr_to_str(arg_ptr) }.unwrap_or("").trim().to_string();
    string_to_cptr(with_client(|t, client| {
        if !t.authorized.load(Ordering::SeqCst) {
            return Err("Telegram ulanmagan".to_string());
        }
        let chat_id = run_tmo(t, 60, async {
            let (channel_id, access_hash) = if let Some(name) = public_name(&target) {
                let tl::enums::contacts::ResolvedPeer::Peer(rp) = client
                    .invoke(&tl::functions::contacts::ResolveUsername { username: name, referer: None })
                    .await
                    .map_err(|e| inv_err(&e))?;
                rp.chats
                    .iter()
                    .find_map(|c| match c {
                        tl::enums::Chat::Channel(c) => c.access_hash.map(|h| (c.id, h)),
                        _ => None,
                    })
                    .ok_or("kanal topilmadi")?
            } else {
                let raw: i64 = target.parse().map_err(|_| "kanal @username yoki -100... ID bo'lsin".to_string())?;
                let id = if raw < 0 { raw } else { -1_000_000_000_000 - raw };
                match find_channel(&client, id).await?.into() {
                    tl::enums::InputPeer::Channel(p) => (p.channel_id, p.access_hash),
                    _ => return Err("bu kanal emas".to_string()),
                }
            };
            let (bot_id, bot_hash) = bot_peer(t, &client).await?;
            client
                .invoke(&tl::functions::channels::EditAdmin {
                    channel: tl::enums::InputChannel::Channel(tl::types::InputChannel { channel_id, access_hash }),
                    user_id: tl::enums::InputUser::User(tl::types::InputUser { user_id: bot_id, access_hash: bot_hash }),
                    admin_rights: tl::enums::ChatAdminRights::Rights(tl::types::ChatAdminRights {
                        change_info: false,
                        post_messages: true,
                        edit_messages: true,
                        delete_messages: true,
                        ban_users: false,
                        invite_users: true,
                        pin_messages: false,
                        add_admins: false,
                        anonymous: false,
                        manage_call: false,
                        other: true,
                        manage_topics: false,
                        post_stories: false,
                        edit_stories: false,
                        delete_stories: false,
                        manage_direct_messages: false,
                        manage_ranks: false,
                    }),
                    rank: None,
                })
                .await
                .map_err(|e| inv_err(&e))?;
            Ok::<i64, String>(-1_000_000_000_000 - channel_id)
        })?;
        Ok(json!({"ok": true, "chat_id": chat_id}).to_string())
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
    // Izoh — faqat fayl nomi (bot postni shu bo'yicha taniydi).
    // Foydalanuvchilarga izohsiz nusxa boradi (`remove_caption`).
    // XAVFSIZLIK (foydalanuvchi talabi): ochish kaliti kanal postiga
    // YOZILMAYDI — faqat fayl nomi. Kalit bazaga ilova orqali
    // (`/api/tg/claim`) yoziladi.
    let _ = &key;
    let caption = name.clone();
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
    secret::wipe();
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

/// Telegram so'rovi vaqt chegarasi bilan: javob kelmasa cheksiz
/// "aylanib" qolmasin, xato matni ko'rinsin.
fn run_tmo<T>(t: &Tg, secs: u64, f: impl std::future::Future<Output = Result<T, String>>) -> Result<T, String> {
    t.rt.block_on(async {
        tokio::time::timeout(Duration::from_secs(secs), f)
            .await
            .map_err(|_| format!("{NET_ERR}Telegram {secs} soniyada javob bermadi"))?
    })
}

/// Pleyer uchun manzil (Telegram ishlatib bo'lmasa bo'sh satr).
#[no_mangle]
pub extern "C" fn rust_tg_play_url(key_ptr: *const c_char) -> *mut c_char {
    let key = unsafe { cstr_to_str(key_ptr) }.unwrap_or("");
    string_to_cptr(route_url(key).unwrap_or_default())
}

#[cfg(test)]
mod tests {

    #[test]
    fn kanal_nomi_va_havola_xeshi() {
        assert_eq!(public_name("@arumediatv_news").as_deref(), Some("arumediatv_news"));
        assert_eq!(public_name("https://t.me/arumediatv_news/").as_deref(), Some("arumediatv_news"));
        assert_eq!(public_name("t.me/arumediatv_news").as_deref(), Some("arumediatv_news"));
        assert!(public_name("abc").is_none());
        assert!(public_name("a b c d e f").is_none());
        assert_eq!(invite_hash("https://t.me/+AbCdEf123456").as_deref(), Some("AbCdEf123456"));
        assert_eq!(invite_hash("t.me/joinchat/AbCdEf123456").as_deref(), Some("AbCdEf123456"));
        assert!(invite_hash("https://t.me/+ab").is_none());
        assert!(invite_hash("https://t.me/+ab cd!ef12345").is_none());
    }
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

include!("tg_media.rs");

// ═══════════════════════════════════════════════════════════════
//  HIKOYALAR (stories)
// ═══════════════════════════════════════════════════════════════

fn peer_bot_id(p: &tl::enums::Peer) -> i64 {
    match p {
        tl::enums::Peer::User(u) => u.user_id,
        tl::enums::Peer::Chat(c) => -c.chat_id,
        tl::enums::Peer::Channel(c) => -1_000_000_000_000i64 - c.channel_id,
    }
}

/// Hikoya elementi JSON'i: `{"id","date","expire","kind":"photo|video","caption","out"}` (o'chirilgan/o'tkazib yuborilgan — `None`).
fn story_json(s: &tl::enums::StoryItem) -> Option<Value> {
    let tl::enums::StoryItem::Item(s) = s else { return None };
    let kind = match &s.media {
        tl::enums::MessageMedia::Photo(_) => "photo",
        tl::enums::MessageMedia::Document(_) => "video",
        _ => return None,
    };
    Some(json!({
        "id": s.id,
        "date": s.date,
        "expire": s.expire_date,
        "kind": kind,
        "caption": s.caption.clone().unwrap_or_default(),
        "out": s.out,
    }))
}

/// Faol hikoyalar: javob `{"peers":[{"id","kind","name","hash","photo_id","max_read","stories":[..]}]}`.
#[no_mangle]
pub extern "C" fn rust_tg_stories() -> *mut c_char {
    tg_call(std::ptr::null(), 40, |client, _| async move {
        let res = client
            .invoke(&tl::functions::stories::GetAllStories { next: false, hidden: false, state: None })
            .await
            .map_err(|e| e.to_string())?;
        let tl::enums::stories::AllStories::Stories(all) = res else { return Ok(json!({"peers": []})) };
        let mut info: HashMap<i64, Value> = HashMap::new();
        for u in &all.users {
            if let tl::enums::User::User(u) = u {
                let name = format!("{} {}", u.first_name.clone().unwrap_or_default(), u.last_name.clone().unwrap_or_default()).trim().to_string();
                let photo_id = match &u.photo {
                    Some(tl::enums::UserProfilePhoto::Photo(p)) => p.photo_id,
                    _ => 0,
                };
                info.insert(u.id, json!({"kind": "user", "name": name, "hash": u.access_hash.unwrap_or(0).to_string(), "photo_id": photo_id.to_string()}));
            }
        }
        for c in &all.chats {
            match c {
                tl::enums::Chat::Chat(c) => {
                    info.insert(-c.id, json!({"kind": "group", "name": c.title, "hash": "0", "photo_id": "0"}));
                }
                tl::enums::Chat::Channel(c) => {
                    info.insert(-1_000_000_000_000i64 - c.id, json!({"kind": "channel", "name": c.title, "hash": c.access_hash.unwrap_or(0).to_string(), "photo_id": "0"}));
                }
                _ => {}
            }
        }
        let mut peers = Vec::new();
        for ps in &all.peer_stories {
            let tl::enums::PeerStories::Stories(ps) = ps;
            let id = peer_bot_id(&ps.peer);
            let list: Vec<Value> = ps.stories.iter().filter_map(story_json).collect();
            if list.is_empty() {
                continue;
            }
            let mut o = info.get(&id).cloned().unwrap_or_else(|| json!({"kind": "user", "name": "", "hash": "0", "photo_id": "0"}));
            o["id"] = json!(id);
            o["max_read"] = json!(ps.max_read_id.unwrap_or(0));
            o["stories"] = json!(list);
            peers.push(o);
        }
        Ok(json!({"peers": peers}))
    })
}

/// Hikoya faylini diskka yuklaydi: `arg` = `"<suhbat>\n<hikoya_id>\n<papka>"`. Javob: `{"path","kind"}`.
#[no_mangle]
pub extern "C" fn rust_tg_story_file(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 300, |client, arg| async move {
        use grammers_client::media::{Document, Media, Photo};
        let l = lines(&arg, 3);
        let id = l[1].trim().parse::<i32>().map_err(|_| "Hikoya ko'rsatilmagan".to_string())?;
        let peer = resolve_peer(&client, &l[0]).await?;
        let ip: tl::enums::InputPeer = peer.into();
        let res = client
            .invoke(&tl::functions::stories::GetStoriesById { peer: ip, id: vec![id] })
            .await
            .map_err(|e| e.to_string())?;
        let tl::enums::stories::Stories::Stories(s) = res;
        let Some(tl::enums::StoryItem::Item(item)) = s.stories.into_iter().next() else { return Err("Hikoya topilmadi".to_string()) };
        let (md, kind, ext) = match item.media {
            tl::enums::MessageMedia::Photo(p) => (Media::Photo(Photo::from_raw_media(p)), "photo", "jpg"),
            tl::enums::MessageMedia::Document(d) => (Media::Document(Document::from_raw_media(d)), "video", "mp4"),
            _ => return Err("Hikoya turi qo'llab-quvvatlanmaydi".to_string()),
        };
        let dir = std::path::PathBuf::from(l[2].trim());
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let key: String = l[0].chars().filter(|c| c.is_ascii_digit() || *c == '-').collect();
        let path = dir.join(format!("s{key}_{id}.{ext}"));
        if !path.exists() || fs::metadata(&path).map(|x| x.len()).unwrap_or(0) == 0 {
            let tmp = dir.join(format!("s{key}_{id}.part"));
            let mut out = fs::File::create(&tmp).map_err(|e| e.to_string())?;
            let mut it = client.iter_download(&md);
            let mut total: usize = 0;
            while let Some(chunk) = it.next().await.map_err(|e| e.to_string())? {
                total += chunk.len();
                if total > 100 * 1024 * 1024 {
                    let _ = fs::remove_file(&tmp);
                    return Err("Hikoya juda katta".to_string());
                }
                std::io::Write::write_all(&mut out, &chunk).map_err(|e| e.to_string())?;
            }
            drop(out);
            fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
        }
        Ok(json!({"path": path.to_string_lossy(), "kind": kind}))
    })
}

/// Hikoyalar ko'rilgan deb belgilanadi: `arg` = `"<suhbat>\n<oxirgi_hikoya_id>"`.
#[no_mangle]
pub extern "C" fn rust_tg_story_read(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 20, |client, arg| async move {
        let l = lines(&arg, 2);
        let max_id = l[1].trim().parse::<i32>().map_err(|_| "Hikoya ko'rsatilmagan".to_string())?;
        let peer = resolve_peer(&client, &l[0]).await?;
        let ip: tl::enums::InputPeer = peer.into();
        client.invoke(&tl::functions::stories::ReadStories { peer: ip, max_id }).await.map_err(|e| e.to_string())?;
        Ok(json!({"ok": true}))
    })
}

/// Hikoya joylash: `arg` = `"<photo|video>\n<yo'l>\n<izoh>\n<kimlarga: all|contacts>\n<soniya:eni:bo'yi>"`.
#[no_mangle]
pub extern "C" fn rust_tg_story_send(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 600, |client, arg| async move {
        let l = lines(&arg, 5);
        let mut f = tokio::fs::File::open(l[1].trim()).await.map_err(|e| format!("Fayl ochilmadi: {e}"))?;
        let size = f.metadata().await.map_err(|e| e.to_string())?.len() as usize;
        if size == 0 {
            return Err("Fayl bo'sh".to_string());
        }
        let name = std::path::Path::new(l[1].trim()).file_name().and_then(|x| x.to_str()).unwrap_or("story").to_string();
        let up = client.upload_stream(&mut f, size, name).await.map_err(|e| e.to_string())?;
        let media = if l[0].trim() == "video" {
            let mut ex = l[4].trim().split(':');
            let dur = ex.next().and_then(|x| x.parse::<f64>().ok()).unwrap_or(0.0).max(0.0);
            let w = ex.next().and_then(|x| x.parse::<i32>().ok()).unwrap_or(720);
            let h = ex.next().and_then(|x| x.parse::<i32>().ok()).unwrap_or(1280);
            tl::enums::InputMedia::UploadedDocument(tl::types::InputMediaUploadedDocument {
                nosound_video: false,
                force_file: false,
                spoiler: false,
                file: up.raw,
                thumb: None,
                mime_type: "video/mp4".to_string(),
                attributes: vec![tl::enums::DocumentAttribute::Video(tl::types::DocumentAttributeVideo {
                    round_message: false,
                    supports_streaming: true,
                    nosound: false,
                    duration: dur,
                    w,
                    h,
                    preload_prefix_size: None,
                    video_start_ts: None,
                    video_codec: None,
                })],
                stickers: None,
                video_cover: None,
                video_timestamp: None,
                ttl_seconds: None,
            })
        } else {
            tl::enums::InputMedia::UploadedPhoto(tl::types::InputMediaUploadedPhoto {
                spoiler: false,
                live_photo: false,
                file: up.raw,
                stickers: None,
                ttl_seconds: None,
                video: None,
            })
        };
        let rule = if l[3].trim() == "contacts" {
            tl::enums::InputPrivacyRule::InputPrivacyValueAllowContacts
        } else {
            tl::enums::InputPrivacyRule::InputPrivacyValueAllowAll
        };
        let caption = l[2].trim().to_string();
        let random_id: i64 = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as i64).unwrap_or(1)) ^ 0x5DEECE66D;
        client
            .invoke(&tl::functions::stories::SendStory {
                pinned: false,
                noforwards: false,
                fwd_modified: false,
                peer: tl::enums::InputPeer::PeerSelf,
                media,
                media_areas: None,
                caption: if caption.is_empty() { None } else { Some(caption) },
                entities: None,
                privacy_rules: vec![rule],
                random_id,
                period: Some(86400),
                fwd_from_id: None,
                fwd_from_story: None,
                albums: None,
                music: None,
            })
            .await
            .map_err(|e| e.to_string())?;
        Ok(json!({"ok": true}))
    })
}

// ═══════════════════════════════════════════════════════════════
//  QO'NG'IROQLAR TARIXI, PREMIUM, STARS
// ═══════════════════════════════════════════════════════════════

fn stars_amount(a: &tl::enums::StarsAmount) -> i64 {
    match a {
        tl::enums::StarsAmount::Amount(a) => a.amount,
        _ => 0,
    }
}

/// Qo'ng'iroqlar tarixi (`messages.search` + `InputMessagesFilterPhoneCalls`):
/// `{"calls":[{"id","user":<id>,"name","hash","out","video","missed","duration","date"}]}`.
#[no_mangle]
pub extern "C" fn rust_tg_calls() -> *mut c_char {
    tg_call(std::ptr::null(), 40, |client, _| async move {
        let res = client
            .invoke(&tl::functions::messages::Search {
                peer: tl::enums::InputPeer::Empty,
                q: String::new(),
                from_id: None,
                saved_peer_id: None,
                saved_reaction: None,
                top_msg_id: None,
                filter: tl::enums::MessagesFilter::InputMessagesFilterPhoneCalls(tl::types::InputMessagesFilterPhoneCalls { missed: false }),
                min_date: 0,
                max_date: 0,
                offset_id: 0,
                add_offset: 0,
                limit: 60,
                max_id: 0,
                min_id: 0,
                hash: 0,
            })
            .await
            .map_err(|e| e.to_string())?;
        let (msgs, users) = match res {
            tl::enums::messages::Messages::Messages(m) => (m.messages, m.users),
            tl::enums::messages::Messages::Slice(m) => (m.messages, m.users),
            tl::enums::messages::Messages::ChannelMessages(m) => (m.messages, m.users),
            _ => (Vec::new(), Vec::new()),
        };
        let mut names: HashMap<i64, (String, i64)> = HashMap::new();
        for u in users {
            if let tl::enums::User::User(u) = u {
                let name = format!("{} {}", u.first_name.clone().unwrap_or_default(), u.last_name.clone().unwrap_or_default()).trim().to_string();
                names.insert(u.id, (name, u.access_hash.unwrap_or(0)));
            }
        }
        let mut out = Vec::new();
        for m in msgs {
            let tl::enums::Message::Service(s) = m else { continue };
            let tl::enums::MessageAction::PhoneCall(pc) = &s.action else { continue };
            let tl::enums::Peer::User(pu) = &s.peer_id else { continue };
            let (name, hash) = names.get(&pu.user_id).cloned().unwrap_or_default();
            let missed = matches!(
                pc.reason,
                Some(tl::enums::PhoneCallDiscardReason::Missed) | Some(tl::enums::PhoneCallDiscardReason::Busy)
            );
            out.push(json!({
                "id": s.id,
                "user": pu.user_id,
                "name": name,
                "hash": hash.to_string(),
                "out": s.out,
                "video": pc.video,
                "missed": missed,
                "duration": pc.duration.unwrap_or(0),
                "date": s.date,
            }));
        }
        Ok(json!({"calls": out}))
    })
}

/// Telegram Premium haqida: `{"text","options":[{"months","currency","amount","url","current"}]}`.
#[no_mangle]
pub extern "C" fn rust_tg_premium() -> *mut c_char {
    tg_call(std::ptr::null(), 30, |client, _| async move {
        let res = client.invoke(&tl::functions::help::GetPremiumPromo {}).await.map_err(|e| e.to_string())?;
        let tl::enums::help::PremiumPromo::Promo(p) = res;
        let options: Vec<Value> = p
            .period_options
            .iter()
            .map(|o| {
                let tl::enums::PremiumSubscriptionOption::Option(o) = o;
                json!({"months": o.months, "currency": o.currency, "amount": o.amount, "url": o.bot_url, "current": o.current})
            })
            .collect();
        Ok(json!({"text": p.status_text, "options": options}))
    })
}

/// Stars balansi va tarixi: `{"balance","history":[{"title","amount","date","refund"}]}`.
#[no_mangle]
pub extern "C" fn rust_tg_stars() -> *mut c_char {
    tg_call(std::ptr::null(), 30, |client, _| async move {
        let res = client
            .invoke(&tl::functions::payments::GetStarsStatus { ton: false, peer: tl::enums::InputPeer::PeerSelf })
            .await
            .map_err(|e| e.to_string())?;
        let tl::enums::payments::StarsStatus::Status(s) = res;
        let history: Vec<Value> = s
            .history
            .unwrap_or_default()
            .iter()
            .map(|t| {
                let tl::enums::StarsTransaction::Transaction(t) = t;
                json!({
                    "title": t.title.clone().unwrap_or_default(),
                    "amount": stars_amount(&t.amount),
                    "date": t.date,
                    "refund": t.refund,
                })
            })
            .collect();
        Ok(json!({"balance": stars_amount(&s.balance), "history": history}))
    })
}

// ═══════════════════════════════════════════════════════════════
//  ARXIV (folder_id = 1)
// ═══════════════════════════════════════════════════════════════

/// Arxivlangan suhbatlar (`messages.getDialogs` + `folder_id = 1`). Javob `rust_tg_dialogs` bilan bir xil shaklda: `{"dialogs":[..]}`.
#[no_mangle]
pub extern "C" fn rust_tg_archived() -> *mut c_char {
    tg_call(std::ptr::null(), 40, |client, _| async move {
        let now_secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
        let res = client
            .invoke(&tl::functions::messages::GetDialogs {
                exclude_pinned: false,
                folder_id: Some(1),
                offset_date: 0,
                offset_id: 0,
                offset_peer: tl::enums::InputPeer::Empty,
                limit: 100,
                hash: 0,
            })
            .await
            .map_err(|e| e.to_string())?;
        let (dialogs, messages, chats, users) = match res {
            tl::enums::messages::Dialogs::Dialogs(d) => (d.dialogs, d.messages, d.chats, d.users),
            tl::enums::messages::Dialogs::Slice(d) => (d.dialogs, d.messages, d.chats, d.users),
            _ => (Vec::new(), Vec::new(), Vec::new(), Vec::new()),
        };
        // (id, nom, tur, hash, bot, verified)
        let mut names: HashMap<i64, (String, &'static str, i64, bool, bool, String)> = HashMap::new();
        for u in &users {
            if let tl::enums::User::User(u) = u {
                let name = format!("{} {}", u.first_name.clone().unwrap_or_default(), u.last_name.clone().unwrap_or_default()).trim().to_string();
                names.insert(u.id, (name, "user", u.access_hash.unwrap_or(0), u.bot, u.verified, u.username.clone().unwrap_or_default()));
            }
        }
        for c in &chats {
            match c {
                tl::enums::Chat::Chat(c) => {
                    names.insert(-c.id, (c.title.clone(), "group", 0, false, false, String::new()));
                }
                tl::enums::Chat::Channel(c) => {
                    let kind = if c.broadcast { "channel" } else { "group" };
                    names.insert(-1_000_000_000_000i64 - c.id, (c.title.clone(), kind, c.access_hash.unwrap_or(0), false, c.verified, c.username.clone().unwrap_or_default()));
                }
                _ => {}
            }
        }
        let mut last: HashMap<(i64, i32), (String, i64, bool)> = HashMap::new();
        for m in &messages {
            match m {
                tl::enums::Message::Message(m) => {
                    let t = if m.message.is_empty() { "Media".to_string() } else { m.message.replace('\n', " ") };
                    last.insert((peer_bot_id(&m.peer_id), m.id), (t, m.date as i64, m.out));
                }
                tl::enums::Message::Service(m) => {
                    last.insert((peer_bot_id(&m.peer_id), m.id), ("Xizmat xabari".to_string(), m.date as i64, m.out));
                }
                _ => {}
            }
        }
        let mut out = Vec::new();
        for d in &dialogs {
            let tl::enums::Dialog::Dialog(x) = d else { continue };
            let id = peer_bot_id(&x.peer);
            let Some((name, kind, hash, bot, verified, username)) = names.get(&id).cloned() else { continue };
            let muted = matches!(&x.notify_settings, tl::enums::PeerNotifySettings::Settings(n) if n.mute_until.unwrap_or(0) as i64 > now_secs);
            let (text, date, outgoing) = last.get(&(id, x.top_message)).cloned().unwrap_or((String::new(), 0, false));
            out.push(json!({
                "id": id, "kind": kind, "name": name, "pinned": false, "unread": x.unread_count, "muted": muted,
                "read_out": x.read_outbox_max_id, "top": x.top_message, "mentions": x.unread_mentions_count,
                "access": hash.to_string(), "out": outgoing, "date": date, "text": text, "bot": bot, "self": false,
                "verified": verified, "username": username,
            }));
        }
        Ok(json!({"dialogs": out}))
    })
}

/// Arxivga olish / chiqarish: `arg` = `"<suhbat>\n<1|0>"` (1 — arxivga).
#[no_mangle]
pub extern "C" fn rust_tg_archive(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let l = lines(&arg, 2);
        let peer = resolve_peer(&client, &l[0]).await?;
        let ip: tl::enums::InputPeer = peer.into();
        let folder_id = if l[1].trim() == "1" { 1 } else { 0 };
        client
            .invoke(&tl::functions::folders::EditPeerFolders {
                folder_peers: vec![tl::enums::InputFolderPeer::Peer(tl::types::InputFolderPeer { peer: ip, folder_id })],
            })
            .await
            .map_err(|e| e.to_string())?;
        Ok(json!({"ok": true}))
    })
}

// ═══════════════════════════════════════════════════════════════
//  OVOZSIZ / REJALASHTIRILGAN XABARLAR, JOYLASHUV, KONTAKT, SO'ROVNOMA
// ═══════════════════════════════════════════════════════════════

fn schedule_of(s: &str) -> Option<std::time::SystemTime> {
    let secs = s.trim().parse::<u64>().ok().filter(|x| *x > 0)?;
    Some(std::time::UNIX_EPOCH + Duration::from_secs(secs))
}

/// Matn (ovozsiz / rejalashtirilgan / bezakli): `arg` = `"<suhbat>\n<javob_id|0>\n<1|0 ovozsiz>\n<unix|0 rejalashtirish>\n<1|0 bezak belgilari>\n<matn>"`.
#[no_mangle]
pub extern "C" fn rust_tg_send_ex(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 40, |client, arg| async move {
        let l = lines(&arg, 6);
        let peer = resolve_peer(&client, &l[0]).await?;
        let reply = l[1].trim().parse::<i32>().ok().filter(|x| *x > 0);
        let mut msg = if l[4].trim() == "1" {
            let (plain, ents) = parse_fmt(&l[5]);
            grammers_client::message::InputMessage::new().text(plain).fmt_entities(ents)
        } else {
            grammers_client::message::InputMessage::new().text(l[5].clone())
        }
        .reply_to(reply)
        .silent(l[2].trim() == "1");
        if let Some(t) = schedule_of(&l[3]) {
            msg = msg.schedule_date(Some(t));
        }
        let m = client.send_message(peer, msg).await.map_err(|e| e.to_string())?;
        Ok(json!({"ok": true, "id": m.id()}))
    })
}

/// Joylashuv / kontakt / so'rovnoma: `arg` = `"<suhbat>\n<javob_id|0>\n<1|0 ovozsiz>\n<unix|0>\n<geo|contact|poll>\n<ma'lumot>"`.
/// Ma'lumot: geo — `lat,long`; contact — `telefon|ism|familiya`; poll — `savol|ko'p(0/1)|viktorina(0/1)|to'g'ri_indeks|variant1|variant2|...`.
#[no_mangle]
pub extern "C" fn rust_tg_send_special(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 60, |client, arg| async move {
        let l = lines(&arg, 6);
        let peer = resolve_peer(&client, &l[0]).await?;
        let reply = l[1].trim().parse::<i32>().ok().filter(|x| *x > 0);
        let media: tl::enums::InputMedia = match l[4].trim() {
            "geo" => {
                let (a, b) = l[5].trim().split_once(',').ok_or("Joylashuv noto'g'ri")?;
                let lat = a.trim().parse::<f64>().map_err(|_| "Joylashuv noto'g'ri".to_string())?;
                let long = b.trim().parse::<f64>().map_err(|_| "Joylashuv noto'g'ri".to_string())?;
                tl::enums::InputMedia::GeoPoint(tl::types::InputMediaGeoPoint {
                    geo_point: tl::enums::InputGeoPoint::Point(tl::types::InputGeoPoint { lat, long, accuracy_radius: None }),
                })
            }
            "contact" => {
                let p: Vec<&str> = l[5].split('|').collect();
                if p.first().map(|x| x.trim().is_empty()).unwrap_or(true) {
                    return Err("Telefon raqami yo'q".to_string());
                }
                tl::enums::InputMedia::Contact(tl::types::InputMediaContact {
                    phone_number: p[0].trim().to_string(),
                    first_name: p.get(1).map(|x| x.trim().to_string()).unwrap_or_default(),
                    last_name: p.get(2).map(|x| x.trim().to_string()).unwrap_or_default(),
                    vcard: String::new(),
                })
            }
            "poll" => {
                let p: Vec<&str> = l[5].split('|').collect();
                if p.len() < 6 || p[0].trim().is_empty() {
                    return Err("So'rovnoma uchun savol va kamida 2 ta variant kerak".to_string());
                }
                let multi = p[1].trim() == "1";
                let quiz = p[2].trim() == "1";
                let correct = p[3].trim().parse::<u8>().ok();
                let answers: Vec<tl::enums::PollAnswer> = p[4..]
                    .iter()
                    .enumerate()
                    .map(|(i, t)| {
                        tl::enums::PollAnswer::Answer(tl::types::PollAnswer {
                            text: tl::enums::TextWithEntities::Entities(tl::types::TextWithEntities { text: t.trim().to_string(), entities: Vec::new() }),
                            option: vec![i as u8],
                            media: None,
                            added_by: None,
                            date: None,
                        })
                    })
                    .collect();
                let poll = tl::types::Poll {
                    id: 0,
                    closed: false,
                    public_voters: false,
                    multiple_choice: multi && !quiz,
                    quiz,
                    open_answers: false,
                    revoting_disabled: false,
                    shuffle_answers: false,
                    hide_results_until_close: false,
                    creator: false,
                    subscribers_only: false,
                    question: tl::enums::TextWithEntities::Entities(tl::types::TextWithEntities { text: p[0].trim().to_string(), entities: Vec::new() }),
                    answers,
                    close_period: None,
                    close_date: None,
                    countries_iso2: None,
                    hash: 0,
                };
                tl::enums::InputMedia::Poll(Box::new(tl::types::InputMediaPoll {
                    poll: tl::enums::Poll::Poll(poll),
                    correct_answers: if quiz { correct.map(|c| vec![c as i32]) } else { None },
                    attached_media: None,
                    solution: None,
                    solution_entities: None,
                    solution_media: None,
                }))
            }
            _ => return Err("Noma'lum tur".to_string()),
        };
        let mut msg = grammers_client::message::InputMessage::new().reply_to(reply).silent(l[2].trim() == "1").media(media);
        if let Some(t) = schedule_of(&l[3]) {
            msg = msg.schedule_date(Some(t));
        }
        let m = client.send_message(peer, msg).await.map_err(|e| e.to_string())?;
        Ok(json!({"ok": true, "id": m.id()}))
    })
}

/// Rejalashtirilgan xabarlar: `arg` = `"<suhbat>"`. Javob `{"messages":[{"id","date","text"}]}`.
#[no_mangle]
pub extern "C" fn rust_tg_scheduled(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let peer = resolve_peer(&client, arg.trim()).await?;
        let ip: tl::enums::InputPeer = peer.into();
        let res = client
            .invoke(&tl::functions::messages::GetScheduledHistory { peer: ip, hash: 0 })
            .await
            .map_err(|e| e.to_string())?;
        let msgs = match res {
            tl::enums::messages::Messages::Messages(m) => m.messages,
            tl::enums::messages::Messages::Slice(m) => m.messages,
            tl::enums::messages::Messages::ChannelMessages(m) => m.messages,
            _ => Vec::new(),
        };
        let mut out = Vec::new();
        for m in msgs {
            if let tl::enums::Message::Message(m) = m {
                let text = if m.message.is_empty() { "Media".to_string() } else { m.message };
                out.push(json!({"id": m.id, "date": m.date, "text": text}));
            }
        }
        out.sort_by_key(|v| v["date"].as_i64().unwrap_or(0));
        Ok(json!({"messages": out}))
    })
}

/// Rejalashtirilganlar ustida amal: `arg` = `"<suhbat>\n<send|delete>\n<id,id,..>"`.
#[no_mangle]
pub extern "C" fn rust_tg_scheduled_act(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let l = lines(&arg, 3);
        let peer = resolve_peer(&client, &l[0]).await?;
        let ip: tl::enums::InputPeer = peer.into();
        let id = ids_of(&l[2]);
        if id.is_empty() {
            return Err("Xabar ko'rsatilmagan".to_string());
        }
        if l[1].trim() == "send" {
            client.invoke(&tl::functions::messages::SendScheduledMessages { peer: ip, id }).await.map_err(|e| e.to_string())?;
        } else {
            client.invoke(&tl::functions::messages::DeleteScheduledMessages { peer: ip, id }).await.map_err(|e| e.to_string())?;
        }
        Ok(json!({"ok": true}))
    })
}

// ═══════════════════════════════════════════════════════════════
//  BOT MINI-ILOVALARI (Web App), TARJIMA
// ═══════════════════════════════════════════════════════════════

fn input_user_of(spec: &str) -> Result<tl::enums::InputUser, String> {
    let (id, hash) = spec.trim().split_once('@').ok_or("Bot ko'rsatilmagan")?;
    Ok(tl::enums::InputUser::User(tl::types::InputUser {
        user_id: id.parse::<i64>().map_err(|_| "Bot ko'rsatilmagan".to_string())?,
        access_hash: hash.parse::<i64>().map_err(|_| "Bot ko'rsatilmagan".to_string())?,
    }))
}

/// Bot menyu tugmasi: `arg` = `"<id>@<hash>"`. Javob `{"text","url"}` yoki `{}`.
#[no_mangle]
pub extern "C" fn rust_tg_bot_menu(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 20, |client, arg| async move {
        let user_id = input_user_of(&arg)?;
        let res = client.invoke(&tl::functions::bots::GetBotMenuButton { user_id }).await.map_err(|e| e.to_string())?;
        Ok(match res {
            tl::enums::BotMenuButton::Button(b) => json!({"text": b.text, "url": b.url}),
            _ => json!({}),
        })
    })
}

/// Mini-ilova manzilini oladi: `arg` = `"<suhbat>\n<bot id@hash>\n<url>\n<menu|button|simple|main>\n<start_param>\n<theme json>"`.
/// Javob `{"url","query_id"}`.
#[no_mangle]
pub extern "C" fn rust_tg_webview(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 40, |client, arg| async move {
        let l = lines(&arg, 6);
        let peer = resolve_peer(&client, &l[0]).await?;
        let ip: tl::enums::InputPeer = peer.into();
        let bot = input_user_of(&l[1])?;
        let url = if l[2].trim().is_empty() { None } else { Some(l[2].trim().to_string()) };
        let start = if l[4].trim().is_empty() { None } else { Some(l[4].trim().to_string()) };
        let theme = if l[5].trim().is_empty() {
            None
        } else {
            Some(tl::enums::DataJson::Json(tl::types::DataJson { data: l[5].trim().to_string() }))
        };
        let platform = "android".to_string();
        let res = match l[3].trim() {
            "main" => client
                .invoke(&tl::functions::messages::RequestMainWebView { compact: false, fullscreen: false, peer: ip, bot, start_param: start, theme_params: theme, platform })
                .await,
            "simple" => client
                .invoke(&tl::functions::messages::RequestSimpleWebView {
                    from_switch_webview: false,
                    from_side_menu: false,
                    compact: false,
                    fullscreen: false,
                    bot,
                    url,
                    start_param: start,
                    theme_params: theme,
                    platform,
                })
                .await,
            mode => client
                .invoke(&tl::functions::messages::RequestWebView {
                    from_bot_menu: mode == "menu",
                    silent: false,
                    compact: false,
                    fullscreen: false,
                    peer: ip,
                    bot,
                    url,
                    start_param: start,
                    theme_params: theme,
                    platform,
                    reply_to: None,
                    send_as: None,
                })
                .await,
        }
        .map_err(|e| e.to_string())?;
        let tl::enums::WebViewResult::Url(r) = res;
        Ok(json!({"url": r.url, "query_id": r.query_id.unwrap_or(0).to_string()}))
    })
}

/// Xabarni tarjima qilish: `arg` = `"<suhbat>\n<xabar_id>\n<til kodi>"`. Javob `{"text"}`.
#[no_mangle]
pub extern "C" fn rust_tg_translate(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 40, |client, arg| async move {
        let l = lines(&arg, 3);
        let id = l[1].trim().parse::<i32>().map_err(|_| "Xabar ko'rsatilmagan".to_string())?;
        let peer = resolve_peer(&client, &l[0]).await?;
        let ip: tl::enums::InputPeer = peer.into();
        let res = client
            .invoke(&tl::functions::messages::TranslateText { peer: Some(ip), id: Some(vec![id]), text: None, to_lang: l[2].trim().to_string(), tone: None })
            .await
            .map_err(|e| e.to_string())?;
        let tl::enums::messages::TranslatedText::TranslateResult(r) = res;
        let text = r
            .result
            .iter()
            .map(|t| {
                let tl::enums::TextWithEntities::Entities(t) = t;
                t.text.clone()
            })
            .collect::<Vec<_>>()
            .join("\n");
        Ok(json!({"text": text}))
    })
}

/// Bezak belgilari: `**qalin**`, `__kursiv__`, `==tagiga chizilgan==`, `~~o'chirilgan~~`, `||spoyler||`, `` `kod` ``, `[matn](havola)`.
/// Natija: oddiy matn va entity'lar (offset/uzunlik UTF-16 birligida — Telegram talabi).
fn parse_fmt(src: &str) -> (String, Vec<tl::enums::MessageEntity>) {
    use tl::enums::MessageEntity as E;
    use tl::types as T;
    let chars: Vec<char> = src.chars().collect();
    let mut out = String::new();
    let mut u16len: i32 = 0;
    let mut ents: Vec<E> = Vec::new();
    // ochiq belgi: (belgi, chiqishdagi boshlanish)
    let mut open: Vec<(&'static str, i32)> = Vec::new();
    let marks: [(&'static str, &'static str); 6] = [("**", "b"), ("__", "i"), ("==", "u"), ("~~", "s"), ("||", "sp"), ("`", "c")];
    let mut i = 0;
    let push_char = |c: char, out: &mut String, u16len: &mut i32| {
        out.push(c);
        *u16len += c.len_utf16() as i32;
    };
    'outer: while i < chars.len() {
        // [matn](havola)
        if chars[i] == '[' {
            if let Some(close) = chars[i + 1..].iter().position(|c| *c == ']').map(|p| p + i + 1) {
                if chars.get(close + 1) == Some(&'(') {
                    if let Some(end) = chars[close + 2..].iter().position(|c| *c == ')').map(|p| p + close + 2) {
                        let text: String = chars[i + 1..close].iter().collect();
                        let url: String = chars[close + 2..end].iter().collect();
                        if !text.is_empty() && !url.trim().is_empty() {
                            let start = u16len;
                            for c in text.chars() {
                                push_char(c, &mut out, &mut u16len);
                            }
                            ents.push(E::TextUrl(T::MessageEntityTextUrl { offset: start, length: u16len - start, url: url.trim().to_string() }));
                            i = end + 1;
                            continue 'outer;
                        }
                    }
                }
            }
        }
        for (m, kind) in marks.iter() {
            let ml = m.chars().count();
            let matches_here = chars.len() >= i + ml && chars[i..i + ml].iter().copied().eq(m.chars());
            if !matches_here {
                continue;
            }
            if let Some(pos) = open.iter().rposition(|(k, _)| k == kind) {
                // yopuvchi belgi
                let (_, start) = open.remove(pos);
                let length = u16len - start;
                if length > 0 {
                    let e = match *kind {
                        "b" => E::Bold(T::MessageEntityBold { offset: start, length }),
                        "i" => E::Italic(T::MessageEntityItalic { offset: start, length }),
                        "u" => E::Underline(T::MessageEntityUnderline { offset: start, length }),
                        "s" => E::Strike(T::MessageEntityStrike { offset: start, length }),
                        "sp" => E::Spoiler(T::MessageEntitySpoiler { offset: start, length }),
                        _ => E::Code(T::MessageEntityCode { offset: start, length }),
                    };
                    ents.push(e);
                }
                i += ml;
                continue 'outer;
            }
            // ochuvchi belgi: keyin yopuvchisi bo'lsagina
            let rest: String = chars[i + ml..].iter().collect();
            if rest.contains(m) {
                open.push((*kind, u16len));
                i += ml;
                continue 'outer;
            }
        }
        push_char(chars[i], &mut out, &mut u16len);
        i += 1;
    }
    ents.sort_by_key(|e| match e {
        E::Bold(x) => x.offset,
        E::Italic(x) => x.offset,
        E::Underline(x) => x.offset,
        E::Strike(x) => x.offset,
        E::Spoiler(x) => x.offset,
        E::Code(x) => x.offset,
        E::TextUrl(x) => x.offset,
        _ => 0,
    });
    (out, ents)
}

// ═══════════════════════════════════════════════════════════════
//  A'ZOLAR USTIDA AMALLAR (admin)
// ═══════════════════════════════════════════════════════════════

/// A'zo ustida amal: `arg` = `"<suhbat>\n<user_id>@<hash>\n<kick|ban|unban|admin|demote>"`.
#[no_mangle]
pub extern "C" fn rust_tg_member_act(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 40, |client, arg| async move {
        let l = lines(&arg, 3);
        let chat = resolve_peer(&client, &l[0]).await?;
        let (uid, uhash) = l[1].trim().split_once('@').ok_or("Foydalanuvchi ko'rsatilmagan")?;
        let uid = uid.parse::<i64>().map_err(|_| "Foydalanuvchi ko'rsatilmagan".to_string())?;
        let uhash = uhash.parse::<i64>().map_err(|_| "Foydalanuvchi ko'rsatilmagan".to_string())?;
        let user = PeerRef { id: PeerId::user(uid).ok_or("Foydalanuvchi ko'rsatilmagan")?, auth: PeerAuth::from_hash(uhash) };
        match l[2].trim() {
            "kick" => client.kick_participant(chat, user).await.map_err(|e| e.to_string())?,
            "ban" => client.set_banned_rights(chat, user).view_messages(false).await.map_err(|e| e.to_string())?,
            "unban" => client.set_banned_rights(chat, user).await.map_err(|e| e.to_string())?,
            "admin" => client
                .set_admin_rights(chat, user)
                .change_info(true)
                .delete_messages(true)
                .ban_users(true)
                .invite_users(true)
                .pin_messages(true)
                .await
                .map_err(|e| e.to_string())?,
            "demote" => client.set_admin_rights(chat, user).await.map_err(|e| e.to_string())?,
            _ => return Err("Noma'lum amal".to_string()),
        }
        Ok(json!({"ok": true}))
    })
}

// ═══════════════════════════════════════════════════════════════
//  SUHBAT JILDLARINI YARATISH / O'CHIRISH
// ═══════════════════════════════════════════════════════════════

/// Jildni saqlash: `arg` = `"<id|0 yangi>\n<nom>\n<bayroqlar vergul bilan>\n<qo'shiladigan suhbatlar vergul bilan>"`.
/// Bayroqlar: contacts, non_contacts, groups, broadcasts, bots, exclude_muted, exclude_read.
#[no_mangle]
pub extern "C" fn rust_tg_folder_save(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 60, |client, arg| async move {
        let l = lines(&arg, 4);
        let title = l[1].trim().to_string();
        if title.is_empty() {
            return Err("Jild nomini kiriting".to_string());
        }
        let mut id = l[0].trim().parse::<i32>().unwrap_or(0);
        if id <= 1 {
            // Yangi jild: mavjud id'lardan keyingisi (kamida 2).
            let res = client.invoke(&tl::functions::messages::GetDialogFilters {}).await.map_err(|e| e.to_string())?;
            let tl::enums::messages::DialogFilters::Filters(f) = res;
            let max = f
                .filters
                .iter()
                .filter_map(|x| match x {
                    tl::enums::DialogFilter::Filter(x) => Some(x.id),
                    _ => None,
                })
                .max()
                .unwrap_or(1);
            id = max.max(1) + 1;
        }
        let flags: HashSet<String> = l[2].split(',').map(|x| x.trim().to_string()).collect();
        let mut include: Vec<tl::enums::InputPeer> = Vec::new();
        for spec in l[3].split(',').map(|x| x.trim()).filter(|x| !x.is_empty()).take(100) {
            if let Ok(p) = resolve_peer(&client, spec).await {
                include.push(p.into());
            }
        }
        let filter = tl::types::DialogFilter {
            contacts: flags.contains("contacts"),
            non_contacts: flags.contains("non_contacts"),
            groups: flags.contains("groups"),
            broadcasts: flags.contains("broadcasts"),
            bots: flags.contains("bots"),
            exclude_muted: flags.contains("exclude_muted"),
            exclude_read: flags.contains("exclude_read"),
            exclude_archived: false,
            title_noanimate: false,
            id,
            title: tl::enums::TextWithEntities::Entities(tl::types::TextWithEntities { text: title, entities: Vec::new() }),
            emoticon: None,
            color: None,
            pinned_peers: Vec::new(),
            include_peers: include,
            exclude_peers: Vec::new(),
        };
        client
            .invoke(&tl::functions::messages::UpdateDialogFilter { id, filter: Some(tl::enums::DialogFilter::Filter(filter.into())) })
            .await
            .map_err(|e| e.to_string())?;
        Ok(json!({"ok": true, "id": id}))
    })
}

/// Jildni o'chirish: `arg` = `"<id>"`.
#[no_mangle]
pub extern "C" fn rust_tg_folder_delete(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let id = arg.trim().parse::<i32>().map_err(|_| "Jild ko'rsatilmagan".to_string())?;
        client.invoke(&tl::functions::messages::UpdateDialogFilter { id, filter: None }).await.map_err(|e| e.to_string())?;
        Ok(json!({"ok": true}))
    })
}

// ═══════════════════════════════════════════════════════════════
//  MAXFIYLIK SOZLAMALARI
// ═══════════════════════════════════════════════════════════════

fn privacy_key(name: &str) -> Result<tl::enums::InputPrivacyKey, String> {
    use tl::enums::InputPrivacyKey as K;
    Ok(match name.trim() {
        "last_seen" => K::StatusTimestamp,
        "phone" => K::PhoneNumber,
        "photo" => K::ProfilePhoto,
        "invite" => K::ChatInvite,
        "call" => K::PhoneCall,
        "forwards" => K::Forwards,
        "about" => K::About,
        "voice" => K::VoiceMessages,
        _ => return Err("Noma'lum sozlama".to_string()),
    })
}

/// Maxfiylik qiymatini olish: `arg` = kalit (`last_seen`, `phone`, `photo`, `invite`, `call`, `forwards`, `about`, `voice`).
/// Javob `{"value":"all|contacts|nobody"}`.
#[no_mangle]
pub extern "C" fn rust_tg_privacy_get(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 20, |client, arg| async move {
        let key = privacy_key(&arg)?;
        let res = client.invoke(&tl::functions::account::GetPrivacy { key }).await.map_err(|e| e.to_string())?;
        let tl::enums::account::PrivacyRules::Rules(r) = res;
        let mut value = "nobody";
        for rule in &r.rules {
            match rule {
                tl::enums::PrivacyRule::PrivacyValueAllowAll => {
                    value = "all";
                    break;
                }
                tl::enums::PrivacyRule::PrivacyValueAllowContacts => value = "contacts",
                _ => {}
            }
        }
        Ok(json!({"value": value}))
    })
}

/// Maxfiylik qiymatini o'rnatish: `arg` = `"<kalit>\n<all|contacts|nobody>"`.
#[no_mangle]
pub extern "C" fn rust_tg_privacy_set(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 20, |client, arg| async move {
        let l = lines(&arg, 2);
        let key = privacy_key(&l[0])?;
        let rule = match l[1].trim() {
            "all" => tl::enums::InputPrivacyRule::InputPrivacyValueAllowAll,
            "contacts" => tl::enums::InputPrivacyRule::InputPrivacyValueAllowContacts,
            _ => tl::enums::InputPrivacyRule::InputPrivacyValueDisallowAll,
        };
        client.invoke(&tl::functions::account::SetPrivacy { key, rules: vec![rule] }).await.map_err(|e| e.to_string())?;
        Ok(json!({"ok": true}))
    })
}

// ═══════════════════════════════════════════════════════════════
//  STIKER TO'PLAMLARINI BOSHQARISH
// ═══════════════════════════════════════════════════════════════

/// To'plamni qo'shish: `arg` = qisqa nom yoki `t.me/addstickers/<nom>` havolasi.
#[no_mangle]
pub extern "C" fn rust_tg_sticker_install(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let name = arg.trim().trim_end_matches('/').rsplit('/').next().unwrap_or("").trim().to_string();
        if name.is_empty() {
            return Err("To'plam nomini kiriting".to_string());
        }
        client
            .invoke(&tl::functions::messages::InstallStickerSet {
                stickerset: tl::enums::InputStickerSet::ShortName(tl::types::InputStickerSetShortName { short_name: name }),
                archived: false,
            })
            .await
            .map_err(|e| match rpc_name(&e) {
                Some("STICKERSET_INVALID") => "Bunday stiker to'plami topilmadi".to_string(),
                _ => e.to_string(),
            })?;
        Ok(json!({"ok": true}))
    })
}

/// To'plamni olib tashlash: `arg` = `"<id>:<hash>"`.
#[no_mangle]
pub extern "C" fn rust_tg_sticker_uninstall(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let mut p = arg.trim().split(':');
        let id = p.next().and_then(|x| x.parse::<i64>().ok()).ok_or("To'plam noto'g'ri")?;
        let hash = p.next().and_then(|x| x.parse::<i64>().ok()).ok_or("To'plam noto'g'ri")?;
        client
            .invoke(&tl::functions::messages::UninstallStickerSet {
                stickerset: tl::enums::InputStickerSet::Id(tl::types::InputStickerSetId { id, access_hash: hash }),
            })
            .await
            .map_err(|e| e.to_string())?;
        Ok(json!({"ok": true}))
    })
}

// ═══════════════════════════════════════════════════════════════
//  SAQLANGAN GIFLAR
// ═══════════════════════════════════════════════════════════════

/// Saqlangan GIFlar: `{"gifs":[{"ref","w","h"}]}` (`ref` — stiker kabi `<id>:<hash>:<fayl_havolasi>`).
#[no_mangle]
pub extern "C" fn rust_tg_saved_gifs() -> *mut c_char {
    tg_call(std::ptr::null(), 30, |client, _| async move {
        let res = client.invoke(&tl::functions::messages::GetSavedGifs { hash: 0 }).await.map_err(|e| e.to_string())?;
        let tl::enums::messages::SavedGifs::Gifs(g) = res else { return Ok(json!({"gifs": []})) };
        let mut out = Vec::new();
        for d in g.gifs.iter().take(60) {
            let tl::enums::Document::Document(d) = d else { continue };
            let (mut w, mut h) = (0, 0);
            for a in &d.attributes {
                if let tl::enums::DocumentAttribute::Video(v) = a {
                    w = v.w;
                    h = v.h;
                }
            }
            out.push(json!({"ref": format!("{}:{}:{}", d.id, d.access_hash, hex::encode(&d.file_reference)), "w": w, "h": h}));
        }
        Ok(json!({"gifs": out}))
    })
}

/// Hujjat faylini diskka yuklaydi: `arg` = `"<ref>\n<papka>"`. Javob `{"path"}` (30 MB gacha).
#[no_mangle]
pub extern "C" fn rust_tg_doc_file(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 120, |client, arg| async move {
        let l = lines(&arg, 2);
        let (id, access_hash, file_reference) = parse_doc_ref(&l[0]).ok_or("Hujjat noto'g'ri")?;
        let dir = std::path::PathBuf::from(l[1].trim());
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join(format!("d{id}.mp4"));
        if !path.exists() || fs::metadata(&path).map(|x| x.len()).unwrap_or(0) == 0 {
            let loc = tl::enums::InputFileLocation::InputDocumentFileLocation(tl::types::InputDocumentFileLocation {
                id,
                access_hash,
                file_reference,
                thumb_size: String::new(),
            });
            let photo = grammers_client::media::ChatPhoto { raw: loc };
            let mut it = client.iter_download(&photo);
            let tmp = dir.join(format!("d{id}.part"));
            let mut out = fs::File::create(&tmp).map_err(|e| e.to_string())?;
            let mut total = 0usize;
            while let Some(chunk) = it.next().await.map_err(|e| e.to_string())? {
                total += chunk.len();
                if total > 30 * 1024 * 1024 {
                    let _ = fs::remove_file(&tmp);
                    return Err("Fayl juda katta".to_string());
                }
                std::io::Write::write_all(&mut out, &chunk).map_err(|e| e.to_string())?;
            }
            drop(out);
            fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
        }
        Ok(json!({"path": path.to_string_lossy()}))
    })
}

/// Xabardagi GIFni saqlash: `arg` = `"<suhbat>\n<xabar_id>"`.
#[no_mangle]
pub extern "C" fn rust_tg_save_gif(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let l = lines(&arg, 2);
        let id = l[1].trim().parse::<i32>().map_err(|_| "Xabar ko'rsatilmagan".to_string())?;
        let peer = resolve_peer(&client, &l[0]).await?;
        let msgs = client.get_messages_by_id(peer, &[id]).await.map_err(|e| e.to_string())?;
        let Some(Some(m)) = msgs.into_iter().next() else { return Err("Xabar topilmadi".to_string()) };
        let Some(grammers_client::media::Media::Document(d)) = m.media() else { return Err("Bu GIF emas".to_string()) };
        let Some(tl::enums::Document::Document(doc)) = &d.raw.document else { return Err("Bu GIF emas".to_string()) };
        client
            .invoke(&tl::functions::messages::SaveGif {
                id: tl::enums::InputDocument::Document(tl::types::InputDocument { id: doc.id, access_hash: doc.access_hash, file_reference: doc.file_reference.clone() }),
                unsave: false,
            })
            .await
            .map_err(|e| e.to_string())?;
        Ok(json!({"ok": true}))
    })
}

/// Foydalanuvchini kontaktga qo'shish / o'chirish: `arg` = `"<id>@<hash>\n<ism>\n<familiya>\n<add|remove>"`.
#[no_mangle]
pub extern "C" fn rust_tg_contact_edit(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let l = lines(&arg, 4);
        let id = input_user_of(&l[0])?;
        if l[3].trim() == "remove" {
            client.invoke(&tl::functions::contacts::DeleteContacts { id: vec![id] }).await.map_err(|e| e.to_string())?;
        } else {
            if l[1].trim().is_empty() {
                return Err("Ismni kiriting".to_string());
            }
            client
                .invoke(&tl::functions::contacts::AddContact {
                    add_phone_privacy_exception: false,
                    id,
                    first_name: l[1].trim().to_string(),
                    last_name: l[2].trim().to_string(),
                    phone: String::new(),
                    note: None,
                })
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(json!({"ok": true}))
    })
}

/// Profil rasmlari (eng ko'pi 10 ta, to'liq o'lchamda): `arg` = `"<suhbat>\n<papka>"`. Javob `{"paths":[".."]}`.
#[no_mangle]
pub extern "C" fn rust_tg_profile_photos(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 120, |client, arg| async move {
        let l = lines(&arg, 2);
        let peer = resolve_peer(&client, &l[0]).await?;
        let dir = std::path::PathBuf::from(l[1].trim());
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let mut it = client.iter_profile_photos(peer);
        let mut paths = Vec::new();
        while paths.len() < 10 {
            let Some(photo) = it.next().await.map_err(|e| e.to_string())? else { break };
            let path = dir.join(format!("p{}.jpg", photo.id()));
            if !path.exists() || fs::metadata(&path).map(|x| x.len()).unwrap_or(0) == 0 {
                let tmp = dir.join(format!("p{}.part", photo.id()));
                let mut out = fs::File::create(&tmp).map_err(|e| e.to_string())?;
                let mut dl = client.iter_download(&photo);
                while let Some(chunk) = dl.next().await.map_err(|e| e.to_string())? {
                    std::io::Write::write_all(&mut out, &chunk).map_err(|e| e.to_string())?;
                }
                drop(out);
                fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
            }
            paths.push(path.to_string_lossy().to_string());
        }
        Ok(json!({"paths": paths}))
    })
}

// ── Video oqimi (yuklab bo'lmasdan ijro) ──
// Pleyer faylni bo'laklab (512 KB) so'raydi: `arg` = "spec\nxabar_id\nbo'lak_raqami". Media tavsifi xotirada saqlanadi
// (har bo'lak uchun xabarni qayta so'ramaslik uchun). Javob: `{"data":"<base64>","size":<jami bayt>}`.
static STREAM_MEDIA: OnceLock<Mutex<HashMap<String, grammers_client::media::Media>>> = OnceLock::new();

const STREAM_CHUNK: i32 = 512 * 1024;

#[no_mangle]
pub extern "C" fn rust_tg_media_read(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 60, |client, arg| async move {
        let l = lines(&arg, 3);
        let id = l[1].trim().parse::<i32>().map_err(|_| "Xabar ko'rsatilmagan".to_string())?;
        let idx = l[2].trim().parse::<i32>().map_err(|_| "Bo'lak raqami noto'g'ri".to_string())?;
        let key = format!("{}_{}", l[0].trim(), id);
        let map = STREAM_MEDIA.get_or_init(|| Mutex::new(HashMap::new()));
        let cached = map.lock().ok().and_then(|g| g.get(&key).cloned());
        let md = match cached {
            Some(m) => m,
            None => {
                let peer = resolve_peer(&client, &l[0]).await?;
                let msgs = client.get_messages_by_id(peer, &[id]).await.map_err(|e| e.to_string())?;
                let Some(Some(m)) = msgs.into_iter().next() else { return Err("Xabar topilmadi".to_string()) };
                let Some(md) = m.media() else { return Err("Xabarda fayl yo'q".to_string()) };
                if let Ok(mut g) = map.lock() {
                    if g.len() > 8 {
                        g.clear();
                    }
                    g.insert(key, md.clone());
                }
                md
            }
        };
        let size = media_info(&md).size;
        let mut it = client.iter_download(&md).chunk_size(STREAM_CHUNK).skip_chunks(idx);
        let data = it.next().await.map_err(|e| e.to_string())?.unwrap_or_default();
        Ok(json!({"data": b64(&data), "size": size}))
    })
}
