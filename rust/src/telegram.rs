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

use grammers_client::client::{LoginToken, PasswordToken};
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
    login_token: Mutex<Option<LoginToken>>,
    password_token: Mutex<Option<PasswordToken>>,
    /// xabar id -> fayl ma'lumoti (xotirada; `file_reference` eskirsa
    /// qayta olinadi).
    docs: Mutex<HashMap<i32, DocInfo>>,
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
    if let Ok(c) = t.client.lock() {
        if let Some(c) = c.as_ref() {
            return Some(c.clone());
        }
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
    if let Ok(mut c) = t.client.lock() {
        *c = Some(client.clone());
    }
    if let Ok(mut s) = t.session.lock() {
        *s = Some(session);
    }
    Some(client)
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

/// Bot chatidagi xabardan faylni topadi. Shaxsiy chatlarda xabar id
/// hisob bo'yicha yagona, ya'ni peer kerak emas.
async fn fetch_doc(client: &Client, msg_id: i32) -> Result<DocInfo, String> {
    let res = client
        .invoke(&tl::functions::messages::GetMessages {
            id: vec![tl::enums::InputMessage::Id(tl::types::InputMessageId { id: msg_id })],
        })
        .await
        .map_err(|e| e.to_string())?;
    let messages = match res {
        tl::enums::messages::Messages::Messages(m) => m.messages,
        tl::enums::messages::Messages::Slice(m) => m.messages,
        tl::enums::messages::Messages::ChannelMessages(m) => m.messages,
        tl::enums::messages::Messages::NotModified(_) => Vec::new(),
    };
    for m in messages {
        let tl::enums::Message::Message(m) = m else { continue };
        if m.id != msg_id {
            continue;
        }
        let Some(tl::enums::MessageMedia::Document(md)) = m.media else {
            return Err("xabarda fayl yo'q".to_string());
        };
        let Some(tl::enums::Document::Document(d)) = md.document else {
            return Err("fayl o'chirilgan".to_string());
        };
        return Ok(DocInfo {
            id: d.id,
            access_hash: d.access_hash,
            file_reference: d.file_reference,
            dc_id: d.dc_id,
            size: d.size.max(0) as u64,
            mime: if d.mime_type.is_empty() { "video/mp4".to_string() } else { d.mime_type },
        });
    }
    Err("xabar topilmadi".to_string())
}

async fn doc_for(t: &'static Tg, client: &Client, msg_id: i32, refresh: bool) -> Result<DocInfo, String> {
    if !refresh {
        if let Some(d) = t.docs.lock().ok().and_then(|m| m.get(&msg_id).cloned()) {
            return Ok(d);
        }
    }
    let d = match fetch_doc(client, msg_id).await {
        Ok(d) => d,
        Err(e) => {
            // Sessiya Telegram tomonidan bekor qilingan (masalan
            // foydalanuvchi "Qurilmalar"dan chiqarib yuborgan). Endi
            // har bir video avval Telegram'ni sinab vaqt yo'qotmasin —
            // qayta ulanmaguncha hammasi worker yo'lidan ketadi.
            if SESSION_DEAD.iter().any(|k| e.contains(k)) {
                t.authorized.store(false, Ordering::SeqCst);
                save_config(t);
                crate::video_cache::tg_log(format!("Telegram sessiyasi bekor qilingan: {e}"));
            }
            return Err(e);
        }
    };
    if let Ok(mut m) = t.docs.lock() {
        m.insert(msg_id, d.clone());
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
async fn fetch_part(t: &'static Tg, client: Client, msg_id: i32, offset: u64) -> Result<Vec<u8>, String> {
    let _permit = t.inflight.acquire().await.map_err(|e| e.to_string())?;
    let mut doc = doc_for(t, &client, msg_id, false).await?;
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
                        doc = doc_for(t, &client, msg_id, true).await?;
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
    // /tg/<xabar_id>/<fayl_nomi>
    let path = path.split('?').next().unwrap_or("");
    let mut seg = path.trim_start_matches('/').split('/');
    let (Some("tg"), Some(id), Some(key)) = (seg.next(), seg.next(), seg.next()) else {
        respond(&mut stream, "404 Not Found", "").await;
        return;
    };
    let Ok(msg_id) = id.parse::<i32>() else {
        respond(&mut stream, "404 Not Found", "").await;
        return;
    };
    let Some(client) = connect(t) else {
        respond(&mut stream, "503 Service Unavailable", "").await;
        return;
    };
    let doc = match doc_for(t, &client, msg_id, false).await {
        Ok(d) => d,
        Err(e) => {
            crate::video_cache::tg_log(format!("Telegram: xabar #{msg_id} ochilmadi: {e}"));
            note_failure(key);
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

    let spawn = |off: u64| t.rt.spawn(fetch_part(t, client.clone(), msg_id, off));
    if let Err(e) = pump(&mut stream, start, end, spawn).await {
        if let PumpError::Source(e) = e {
            crate::video_cache::tg_log(format!("Telegram: #{msg_id} olinmadi: {e}"));
            note_failure(key);
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
            login_token: Mutex::new(None),
            password_token: Mutex::new(None),
            docs: Mutex::new(HashMap::new()),
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
    if let Ok(s) = t.session.lock() {
        if let Some(s) = s.as_ref() {
            s.save();
        }
    }
    save_config(t);
    json!({"ok": true}).to_string()
}

fn sign_in_result(t: &Tg, r: Result<grammers_client::peer::User, SignInError>) -> Result<String, String> {
    match r {
        Ok(_) => Ok(after_login(t)),
        Err(SignInError::PasswordRequired(pt)) => {
            let hint = pt.hint().unwrap_or("").to_string();
            if let Ok(mut p) = t.password_token.lock() {
                *p = Some(pt);
            }
            Ok(json!({"password": true, "hint": hint}).to_string())
        }
        Err(SignInError::InvalidPassword(pt)) => {
            let hint = pt.hint().unwrap_or("").to_string();
            if let Ok(mut p) = t.password_token.lock() {
                *p = Some(pt);
            }
            Ok(json!({"password": true, "hint": hint, "error": "Parol noto'g'ri"}).to_string())
        }
        Err(SignInError::InvalidCode) => Err("Kod noto'g'ri".to_string()),
        Err(SignInError::SignUpRequired) => {
            Err("Bu raqamda Telegram hisobi yo'q — avval Telegram ilovasida ro'yxatdan o'ting".to_string())
        }
        Err(e) => Err(e.to_string()),
    }
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

/// Telefon raqamiga kirish kodini yuboradi.
#[no_mangle]
pub extern "C" fn rust_tg_request_code(phone_ptr: *const c_char) -> *mut c_char {
    let phone = unsafe { cstr_to_str(phone_ptr) }.unwrap_or("").trim().to_string();
    string_to_cptr(with_client(|t, client| {
        if phone.is_empty() {
            return Err("Telefon raqamini kiriting".to_string());
        }
        let hash = t.api_hash.lock().map(|v| v.clone()).unwrap_or_default();
        let token = t
            .rt
            .block_on(client.request_login_code(&phone, &hash))
            .map_err(|e| match rpc_name(&e) {
                Some("PHONE_NUMBER_INVALID") => "Telefon raqami noto'g'ri".to_string(),
                Some("PHONE_NUMBER_BANNED") => "Bu raqam Telegram'da bloklangan".to_string(),
                Some("FLOOD_WAIT") => "Juda ko'p urinish — birozdan keyin qayta urining".to_string(),
                _ => e.to_string(),
            })?;
        if let Ok(mut l) = t.login_token.lock() {
            *l = Some(token);
        }
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
        let token = t
            .login_token
            .lock()
            .ok()
            .and_then(|mut l| l.take())
            .ok_or("Avval kod so'rang")?;
        let r = t.rt.block_on(client.sign_in(&token, &code));
        if matches!(r, Err(SignInError::InvalidCode)) {
            // Kod xato — xuddi shu token bilan qayta urinish mumkin.
            if let Ok(mut l) = t.login_token.lock() {
                *l = Some(token);
            }
        }
        sign_in_result(t, r)
    }))
}

/// 2 bosqichli parolni tekshiradi.
#[no_mangle]
pub extern "C" fn rust_tg_check_password(pw_ptr: *const c_char) -> *mut c_char {
    let pw = unsafe { cstr_to_str(pw_ptr) }.unwrap_or("").to_string();
    string_to_cptr(with_client(|t, client| {
        let pt = t
            .password_token
            .lock()
            .ok()
            .and_then(|mut p| p.take())
            .ok_or("Avval kod bilan kiring")?;
        let r = t.rt.block_on(client.check_password(pt, pw.as_bytes()));
        sign_in_result(t, r)
    }))
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
    if let Ok(mut d) = t.docs.lock() {
        d.clear();
    }
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
