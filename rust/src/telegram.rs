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

use grammers_client::client::PasswordToken;
use grammers_client::session::types::{DcOption, PeerId, PeerInfo, UpdateState, UpdatesState};
use grammers_client::session::{BoxFuture, Session, SessionData};
use grammers_client::{tl, Client, InvocationError, SenderPool, SignInError};
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
const MAX_INFLIGHT: usize = 12;

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
    password_token: Mutex<Option<PasswordToken>>,
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
}

static TG: OnceLock<Tg> = OnceLock::new();

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
        SenderPool::new(Arc::clone(&session), api_id)
    };
    let SenderPool { runner, handle, mut updates } = pool;
    let client = Client::new(handle);
    t.rt.spawn(runner.run());
    // Yangilanishlar kanali o'qilmasa xotirada cheksiz o'sadi.
    t.rt.spawn(async move { while updates.recv().await.is_some() {} });
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
}

fn disconnect(t: &Tg) {
    if let Ok(mut c) = t.client.lock() {
        if let Some(c) = c.take() {
            c.disconnect();
        }
    }
    if let Ok(mut s) = t.session.lock() {
        *s = None;
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

fn status_json(t: &Tg) -> String {
    json!({
        "configured": t.api_id.lock().map(|v| *v > 0).unwrap_or(false),
        "authorized": t.authorized.load(Ordering::SeqCst),
        "port": t.port,
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

fn note_failure(key: &str) {
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
        .map_err(|e| e.to_string())?;
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

/// Xabardagi fayl shu nomdagi faylmi (fayl nomi yoki izoh bo'yicha).
fn doc_matches(m: &tl::types::Message, name: &str) -> Option<DocInfo> {
    let Some(tl::enums::MessageMedia::Document(md)) = &m.media else { return None };
    let Some(tl::enums::Document::Document(d)) = &md.document else { return None };
    let by_attr = d.attributes.iter().any(|a| {
        matches!(a, tl::enums::DocumentAttribute::Filename(f) if f.file_name == name)
    });
    if !by_attr && m.message.trim() != name {
        return None;
    }
    Some(DocInfo {
        id: d.id,
        access_hash: d.access_hash,
        file_reference: d.file_reference.clone(),
        dc_id: d.dc_id,
        size: d.size.max(0) as u64,
        mime: if d.mime_type.is_empty() { "video/mp4".to_string() } else { d.mime_type.clone() },
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
    let (id, hash) = bot_peer(t, client).await?;
    let res = client
        .invoke(&tl::functions::messages::GetHistory {
            peer: tl::enums::InputPeer::User(tl::types::InputPeerUser { user_id: id, access_hash: hash }),
            offset_id: 0,
            offset_date: 0,
            add_offset: 0,
            limit: 100,
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
    // Javob eng yangisidan boshlanadi — qayta yuborilgan bo'lsa ham
    // eng oxirgi nusxa olinadi.
    for m in &messages {
        if let tl::enums::Message::Message(m) = m {
            if let Some(d) = doc_matches(m, name) {
                return Ok(d);
            }
        }
    }
    Err("bot chatida fayl topilmadi".to_string())
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
            if SESSION_DEAD.iter().any(|k| e.contains(k)) {
                t.authorized.store(false, Ordering::SeqCst);
                save_config(t);
                // O'lik kalit bilan qayta kirib bo'lmaydi (masalan
                // AUTH_KEY_DUPLICATED) — sessiya butunlay tashlanadi,
                // keyingi kirish toza boshlanadi.
                if let Ok(s) = t.session.lock() {
                    if let Some(s) = s.as_ref() {
                        s.wipe();
                    }
                }
                disconnect(t);
                if let Ok(mut d) = t.auth_dcs.try_lock() {
                    d.clear();
                }
                crate::video_cache::tg_log(format!("Telegram sessiyasi bekor qilingan: {e}"));
            }
            return Err(e);
        }
    };
    if let Ok(mut m) = t.docs.lock() {
        m.insert(name.to_string(), d.clone());
    }
    Ok(d)
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
            location: tl::enums::InputFileLocation::InputDocumentFileLocation(
                tl::types::InputDocumentFileLocation {
                    id: doc.id,
                    access_hash: doc.access_hash,
                    file_reference: doc.file_reference.clone(),
                    thumb_size: String::new(),
                },
            ),
            offset: offset as i64,
            limit: PART as i32,
        };
        match client.invoke_in_dc(dc, &req).await {
            Ok(tl::enums::upload::File::File(f)) => return Ok(f.bytes),
            Ok(tl::enums::upload::File::CdnRedirect(_)) => {
                return Err("CDN yo'naltirishi kutilmagan".to_string());
            }
            Err(e) => {
                last_err = e.to_string();
                match rpc_name(&e) {
                    // Fayl havolasi eskirgan — xabarni qayta olamiz.
                    Some(n) if n.starts_with("FILE_REFERENCE_") => {
                        doc = doc_for(t, &client, &name, true).await?;
                    }
                    // Boshqa DC: hisobni o'sha yerga ko'chiramiz.
                    Some("AUTH_KEY_UNREGISTERED") => {
                        copy_auth(t, &client, dc).await?;
                    }
                    Some("FILE_MIGRATE") => {
                        if let InvocationError::Rpc(r) = &e {
                            if let Some(v) = r.value {
                                dc = v as i32;
                            }
                        }
                    }
                    _ => return Err(last_err),
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

fn init(dir: &str, api_id: i32, api_hash: &str) -> Result<&'static Tg, String> {
    if TG.get().is_none() {
        let dir = PathBuf::from(dir);
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
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
        };
        if TG.set(t).is_ok() {
            let t = TG.get().unwrap();
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
        Err(e) => err_json(e),
    }
}

fn after_login(t: &Tg) -> String {
    t.authorized.store(true, Ordering::SeqCst);
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
    let body = json!({"stage": stage, "phone": phone, "hash": hash, "hint": hint, "at": now_ms()});
    let _ = write_sealed(&login_path(t), LABEL_LOGIN, body.to_string().as_bytes());
}

fn clear_login(t: &Tg) {
    let _ = fs::remove_file(login_path(t));
    if let Ok(mut p) = t.password_token.lock() {
        *p = None;
    }
}

fn code_settings() -> tl::enums::CodeSettings {
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

fn friendly(e: &InvocationError) -> String {
    match rpc_name(e) {
        Some("PHONE_NUMBER_INVALID") => "Telefon raqami noto'g'ri".to_string(),
        Some("PHONE_NUMBER_BANNED") => "Bu raqam Telegram'da bloklangan".to_string(),
        Some("PHONE_NUMBER_FLOOD") | Some("FLOOD_WAIT") => {
            "Juda ko'p urinish — birozdan keyin qayta urining".to_string()
        }
        _ => e.to_string(),
    }
}

async fn send_code(t: &Tg, client: &Client, phone: &str) -> Result<String, String> {
    let api_id = t.api_id.lock().map(|v| *v).unwrap_or(0);
    let api_hash = t.api_hash.lock().map(|v| v.clone()).unwrap_or_default();
    let req = tl::functions::auth::SendCode {
        phone_number: phone.to_string(),
        api_id,
        api_hash,
        settings: code_settings(),
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
    }
    .map_err(|e| friendly(&e))?;
    match res {
        tl::enums::auth::SentCode::Code(c) => Ok(c.phone_code_hash),
        _ => Err("Telegram kutilmagan javob berdi — qayta urining".to_string()),
    }
}

async fn password_token(client: &Client) -> Result<PasswordToken, String> {
    let pw: tl::types::account::Password = client
        .invoke(&tl::functions::account::GetPassword {})
        .await
        .map_err(|e| e.to_string())?
        .into();
    Ok(PasswordToken::new(pw))
}

/// Telefon raqamiga kirish kodini yuboradi.
#[no_mangle]
pub extern "C" fn rust_tg_request_code(phone_ptr: *const c_char) -> *mut c_char {
    let phone = unsafe { cstr_to_str(phone_ptr) }.unwrap_or("").trim().to_string();
    string_to_cptr(with_client(|t, client| {
        if phone.len() < 6 {
            return Err("Telefon raqamini kiriting".to_string());
        }
        let hash = t.rt.block_on(send_code(t, &client, &phone))?;
        save_login(t, "code", &phone, &hash, "");
        Ok(json!({"ok": true}).to_string())
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
            Ok(tl::enums::auth::Authorization::Authorization(_)) => {
                clear_login(t);
                Ok(after_login(t))
            }
            Ok(tl::enums::auth::Authorization::SignUpRequired(_)) => {
                clear_login(t);
                Err("Bu raqamda Telegram hisobi yo'q — avval Telegram ilovasida ro'yxatdan o'ting".to_string())
            }
            Err(e) if e.is("SESSION_PASSWORD_NEEDED") => {
                let pt = t.rt.block_on(password_token(&client))?;
                let hint = pt.hint().unwrap_or("").to_string();
                if let Ok(mut p) = t.password_token.lock() {
                    *p = Some(pt);
                }
                save_login(t, "password", &phone, &hash, &hint);
                Ok(json!({"password": true, "hint": hint}).to_string())
            }
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
        let pt = match t.password_token.lock().ok().and_then(|mut p| p.take()) {
            Some(pt) => pt,
            None => t.rt.block_on(password_token(&client))?,
        };
        match t.rt.block_on(client.check_password(pt, pw.as_bytes())) {
            Ok(_) => {
                clear_login(t);
                Ok(after_login(t))
            }
            Err(SignInError::InvalidPassword(pt)) => {
                let hint = pt.hint().unwrap_or("").to_string();
                if let Ok(mut p) = t.password_token.lock() {
                    *p = Some(pt);
                }
                Ok(json!({"password": true, "hint": hint, "error": "Parol noto'g'ri"}).to_string())
            }
            Err(e) => Err(e.to_string()),
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
        })
        .to_string(),
        None => json!({"stage": "phone"}).to_string(),
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
}

static UPLOADS: OnceLock<Mutex<HashMap<u64, UploadJob>>> = OnceLock::new();
static UPLOAD_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn uploads() -> &'static Mutex<HashMap<u64, UploadJob>> {
    UPLOADS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// O'qilgan baytlarni sanaydigan o'quvchi (yuklash foizi uchun).
struct Counting<R> {
    inner: R,
    n: Arc<std::sync::atomic::AtomicU64>,
}

impl<R: tokio::io::AsyncRead + Unpin> tokio::io::AsyncRead for Counting<R> {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let before = buf.filled().len();
        let r = std::pin::Pin::new(&mut self.inner).poll_read(cx, buf);
        let got = buf.filled().len() - before;
        if got > 0 {
            self.n.fetch_add(got as u64, Ordering::Relaxed);
        }
        r
    }
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

async fn upload_to_channel(
    client: Client,
    path: String,
    name: String,
    mime: String,
    channel_id: i64,
    sent: Arc<std::sync::atomic::AtomicU64>,
    total: u64,
) -> Result<i32, String> {
    let peer = find_channel(&client, channel_id).await?;
    let file = tokio::fs::File::open(&path).await.map_err(|e| format!("fayl ochilmadi: {e}"))?;
    let mut reader = Counting { inner: file, n: Arc::clone(&sent) };
    let uploaded = client
        .upload_stream(&mut reader, total as usize, name.clone())
        .await
        .map_err(|e| format!("yuklashda xato: {e}"))?;
    let mut rnd = [0u8; 8];
    getrandom::getrandom(&mut rnd).map_err(|e| e.to_string())?;
    let random_id = i64::from_le_bytes(rnd);
    let res = client
        .invoke(&tl::functions::messages::SendMedia {
            silent: true,
            background: false,
            clear_draft: false,
            noforwards: false,
            update_stickersets_order: false,
            invert_media: false,
            allow_paid_floodskip: false,
            peer: peer.into(),
            reply_to: None,
            // FAYL sifatida (qayta kodlanmaydi, sifat buzilmaydi).
            media: tl::enums::InputMedia::UploadedDocument(tl::types::InputMediaUploadedDocument {
                nosound_video: false,
                force_file: true,
                spoiler: false,
                file: uploaded.raw,
                thumb: None,
                mime_type: mime,
                attributes: vec![tl::enums::DocumentAttribute::Filename(
                    tl::types::DocumentAttributeFilename { file_name: name.clone() },
                )],
                stickers: None,
                video_cover: None,
                video_timestamp: None,
                ttl_seconds: None,
            }),
            // Izoh = fayl nomi: bot shu bo'yicha postni taniydi.
            message: name,
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
        })
        .await
        .map_err(|e| format!("kanalga yuborilmadi: {e}"))?;
    msg_id_of(&res, random_id).ok_or_else(|| "xabar raqami olinmadi".to_string())
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
        if channel_id == 0 {
            return Err("Kanal sozlanmagan (TG_CHANNEL_ID)".to_string());
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
        let task = t.rt.spawn(async move {
            let r = upload_to_channel(client, path, name, mime, channel_id, s2, total).await;
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
            let _ = t.rt.block_on(async {
                tokio::time::timeout(Duration::from_secs(10), client.sign_out()).await
            });
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

/// Pleyer uchun manzil (Telegram ishlatib bo'lmasa bo'sh satr).
#[no_mangle]
pub extern "C" fn rust_tg_play_url(key_ptr: *const c_char) -> *mut c_char {
    let key = unsafe { cstr_to_str(key_ptr) }.unwrap_or("");
    string_to_cptr(route_url(key).unwrap_or_default())
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
    fn yuklash_foizi_sanaladi() {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let data = vec![7u8; 300_000];
            let n = Arc::new(std::sync::atomic::AtomicU64::new(0));
            let mut r = Counting { inner: &data[..], n: Arc::clone(&n) };
            let mut out = Vec::new();
            r.read_to_end(&mut out).await.unwrap();
            assert_eq!(out.len(), 300_000);
            assert_eq!(n.load(Ordering::Relaxed), 300_000);
        });
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
