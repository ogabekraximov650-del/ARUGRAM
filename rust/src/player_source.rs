// rust/src/player_source.rs — pleyer uchun MAHALLIY SERVERSIZ manba.
//
// ── NEGA ──────────────────────────────────────────────────────────
//
// Ilgari pleyer videoni 127.0.0.1 dagi HTTP serverdan so'rardi. HTTP
// so'rovi "bytes=X-" (ya'ni faylning OXIRIGACHA) bo'lgani uchun server
// pleyer so'raganidan ko'proq yuklab qo'yardi. Telegram ilovasi esa
// ExoPlayer'ga o'z manbasini (DataSource) beradi va pleyer diskdan
// to'g'ridan-to'g'ri o'qiydi. Endi biz ham shunday qilamiz:
//
//   ExoPlayer ── AruDataSource (Java) ── JNI ── shu fayl
//                                                 ├─ diskdagi shifrlangan
//                                                 │  1 MiB bo'lak (ochiladi)
//                                                 └─ yo'q bo'lsa Telegram'dan
//                                                    olinadi, shifrlanib
//                                                    diskka yoziladi
//
// Pleyer to'xtasa (buferi to'lsa) o'qish ham to'xtaydi, ya'ni ortiqcha
// hech narsa yuklanmaydi — oldindan faqat `AHEAD` ta bo'lak olinadi.
//
// ── INTERNET UZILSA ───────────────────────────────────────────────
//
// TALAB (foydalanuvchi): "internet yonishini kutib, kelgan joydagi
// kadrni ko'rsatib tursin — hozir ekran qorayib boshiga qaytib
// qolyapti".
//
// Tarmoq xatosi (`telegram::is_net_err`) pleyerga XATO sifatida
// berilmaydi: JNI `RETRY` (-2) qaytaradi va Java tomoni
// (`AruDataSource`) biroz kutib o'sha joyni QAYTA so'raydi. Pleyer bu
// orada buferdagini ko'rsatib bo'ladi-da, "buferlanmoqda" holatida
// oxirgi kadrda to'xtab turadi. Internet qaytishi bilan o'qish o'zi
// davom etadi — pleyer qaytadan ochilmaydi, ekran qoraymaydi.
// Faqat haqiqiy xato (fayl yo'q va h.k.) pleyerga xato bo'lib boradi.

use crate::{telegram, video_cache};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};

/// Pleyer o'qiyotgan joydan oldinga shuncha bo'lak tayyorlab qo'yiladi.
///
/// Bir muddat 4 qilingan edi — foydalanuvchi: "onlayn ko'rishda pleyer
/// 1 MB bo'lak atrofida joy tashlab, keyingi joyni yuklab olyapti".
/// Bir vaqtda 4 ta bo'lak parallel olinganda kech kelgani orasida
/// teshik qolardi. Avvalgi holatiga (2) qaytarildi; tezlik oshirish
/// (`DL_CONNS`, `MAX_INFLIGHT`) yuklab olish va boshqa ishlar uchun qoladi.
const AHEAD: u64 = 2;

// ── ONLAYN KO'RISHDA KO'PI BILAN 1 DAQIQA OLDINGA ─────────────────
//
// YANGILANISH (foydalanuvchi: "tez-tez pauza bo'lib, o'rtasida
// aylanma chiqyapti"): pleyerning O'Z so'rovlarini to'xtatib turish
// ijroni buzdi — anime'da bitreyt sahnaga qarab bir necha barobar
// o'zgaradi va "o'rtacha 1 daqiqa" ba'zi joyda 15 soniyaga ham
// yetmaydi. Endi pleyer so'ragan bo'lak DOIM beriladi (pleyerning
// o'zi `AruLoadControl` bilan ko'pi bilan 30 s oldinga o'qiydi), bu
// chegara esa faqat BIZNING oldindan olishimizga (`prefetch`)
// qo'llanadi. Ya'ni oldinda turadigan hamma narsa: pleyer buferi
// (≤30 s) + ko'pi bilan `AHEAD` bo'lak.
//
// YANA YANGILANISH (foydalanuvchi: "pleyer chizig'ida 1 daqiqalik
// bo'sh joydan keyingi joy yuklanyapti"): ketma-ket o'qish (pleyer
// buferini to'ldirishi) cheklanmaydi, lekin oxirgi o'qilgan joydan
// SAKRAB chegaradan uzoqqa so'ralgan bo'lak (diskda yo'q bo'lsa)
// berilmaydi — `WAIT`, Java 1 s kutib qayta so'raydi. Sek oldidan
// ilova yangi joyni bildiradi, ya'ni sek bunga tushmaydi; pleyer
// buferlanayotgan (to'xtab qolgan) bo'lsa ham cheklov yo'q.
//
// TALAB (foydalanuvchi): "onlayn ko'rishda ko'rayotgan daqiqadan
// 1 daqiqagacha yuklab olishga ruxsat bo'lsin, undan ko'p emas".
//
// Ilova har soniyada ijro joyini beradi (`rust_player_position`).
// Diskda YO'Q bo'lak Telegram'dan faqat u ijro joyidan ko'pi bilan
// `AHEAD_MS` oldinda bo'lsa olinadi (bayt/vaqt — faylning o'rtacha
// bitreyti bo'yicha). Hozir bu faqat oldindan olishga (`prefetch`)
// qo'llanadi — quyidagi "YANGILANISH" ga qarang.
//
// TOPILGAN XATO (foydalanuvchi: "video birdan to'xtab qoldi, qayta
// kirsam ham ochilmayapti"): pleyer buferi TUGAGAN ("buferlanmoqda")
// paytda ijro joyi surilmaydi, ya'ni chegara ham surilmaydi — kerakli
// bo'lak chegaradan tashqarida bo'lsa pleyer abadiy kutib qolardi
// (masalan audio va video fayl ichida uzoq joylashgan yoki bitreyt
// o'rtachadan ancha baland joyda). Endi pleyer BUFERLANAYOTGANDA
// (ilova `buffering` deb bildiradi) cheklov yo'q — unga aynan hozir
// kerak bo'lgan narsa beriladi; chegara faqat bufer to'la, ijro
// ketayotgan yoki pauza paytida ishlaydi.
//
// Istisnolar (ular bo'lmasa video ochilmay qolishi mumkin): fayl boshi
// va oxiri (MP4 sarlavhasi `moov` ko'pincha oxirida), hamda ijro joyi
// noma'lum yoki eskirgan (2 daqiqadan ko'p xabar kelmagan) holat. Surish
// (barmoq ekranda) paytida xabar to'xtaydi — shu sabab muddat uzun.
const AHEAD_MS: u64 = 60_000;
/// Sakrab, chegaradan tashqaridan so'ralgan bo'lak — JNI `RETRY` qiladi.
const WAIT: &str = "oldinga chegara";

struct PlayPos {
    pos_ms: u64,
    dur_ms: u64,
    buffering: bool,
    at: std::time::Instant,
}

/// "Bayt → soniya" jadvali (pleyer chizig'i uchun): har bir 1 MiB
/// bo'lakda boshlanadigan birinchi kadrning vaqti.
pub(crate) struct TimeMap {
    pub chunk_ms: Vec<u32>,
    pub dur_ms: u64,
}

fn time_maps() -> &'static Mutex<HashMap<String, Arc<TimeMap>>> {
    static T: OnceLock<Mutex<HashMap<String, Arc<TimeMap>>>> = OnceLock::new();
    T.get_or_init(|| Mutex::new(HashMap::new()))
}

/// `key` — `video_cache::player_key(nom)`.
pub(crate) fn time_map(key: &str) -> Option<Arc<TimeMap>> {
    time_maps().lock().ok()?.get(key).cloned()
}

fn positions() -> &'static Mutex<HashMap<String, PlayPos>> {
    static P: OnceLock<Mutex<HashMap<String, PlayPos>>> = OnceLock::new();
    P.get_or_init(|| Mutex::new(HashMap::new()))
}

// ── SIRPANUVCHI OYNA: DISKDA DOIM `pos .. pos + 1 daqiqa` ──────────
//
// TALAB (foydalanuvchi): "pleyer 1:00 da bo'lsa 2:00 gacha, 1:01 da
// bo'lsa 2:01 gacha yuklab olinsin — har soniya surilganda keyingi
// soniya". Pleyerning o'z buferi (≤30 s) bunga yetmaydi, shu sabab
// ilova har joy xabarida (`rust_player_position`, ~0.8 s) fonda
// `pos .. pos + AHEAD_MS` oralig'idagi diskda YO'Q bo'laklar ketma-ket
// olinadi (bir fayl uchun bitta oqim). Chegaradan keyingisi olinmaydi —
// oyna ijro bilan birga suriladi. Bo'lak 1 MiB, ya'ni qadam bir necha
// soniya (bitreytga qarab).
fn filling() -> &'static Mutex<std::collections::HashSet<String>> {
    static F: OnceLock<Mutex<std::collections::HashSet<String>>> = OnceLock::new();
    F.get_or_init(|| Mutex::new(std::collections::HashSet::new()))
}

// ── ANIQ "SONIYA → BAYT" (VBR): 1 DAQIQA = 1 DAQIQA ────────────────
//
// TOPILGAN XATO (foydalanuvchi: "1 daqiqa deb edim, 5 daqiqagacha
// oldindan yuklanyapti"): oyna baytlari O'RTACHA bitreyt bilan
// hisoblangan edi (`bayt = vaqt / davomiylik × hajm`). Anime'da
// bitreyt sahnaga qarab keskin o'zgaradi: suhbat/tinch sahna bir
// necha barobar ARZON, ya'ni o'rtacha bo'yicha "1 daqiqalik" bayt
// tinch sahnada 4-5 daqiqani qamrab olardi.
//
// Endi faylning O'ZIDAGI namuna jadvallaridan (`moov`: stts/stsz/stsc/
// stco — `mp4::VideoTrack`) haqiqiy joy olinadi: `sample_at_ms(vaqt)` →
// `locate(namuna).offset`. `moov` fonda BIR marta o'qiladi (u odatda
// faylning boshida yoki oxirida — pleyer uni baribir o'qiydi). Jadval
// hali yo'q (yoki o'qilmadi) bo'lsa — o'rtacha bitreyt zaxira.

/// Fayldan `[off, off+len)` ni beradi (fayl boshi/oxirida `None`).
type SpanReader<'a> = &'a dyn Fn(u64, u64) -> Option<Vec<u8>>;

/// `moov` ni topib `VideoTrack` ga o'giradi. Yuqori darajadagi atomlar
/// kezib chiqiladi (`mdat` ni sakrab o'tadi), shu sabab `moov` oxirida
/// bo'lsa ham kam o'qiladi.
/// Video va (bo'lsa) ovoz yo'lakchalarining namuna jadvallari.
pub(crate) struct Tracks {
    pub video: crate::mp4::VideoTrack,
    pub audio: Option<crate::mp4::VideoTrack>,
}

fn track_from(total: u64, read: SpanReader) -> Option<Tracks> {
    /// Himoya: buzilgan faylda cheksiz aylanib qolmaslik uchun.
    const MAX_BOXES: usize = 64;
    /// `moov` odatda 1 MB atrofida; 32 MB dan kattasi shubhali.
    const MAX_MOOV: u64 = 32 * 1024 * 1024;

    let mut at: u64 = 0;
    for _ in 0..MAX_BOXES {
        if at + 8 > total {
            return None;
        }
        let head = read(at, 16.min(total - at))?;
        let (body_in_head, raw_len, kind) = crate::mp4::box_header(&head, 0)?;
        let body_start = at + body_in_head as u64;
        if body_start > total {
            return None;
        }
        let body_len = if raw_len == u64::MAX { total - body_start } else { raw_len };
        if &kind == b"moov" {
            if body_len == 0 || body_len > MAX_MOOV {
                return None;
            }
            let moov = read(body_start, body_len)?;
            let video = crate::mp4::parse_moov(&moov)?;
            let audio = crate::mp4::parse_moov_audio(&moov);
            return Some(Tracks { video, audio });
        }
        let next = body_start.checked_add(body_len)?;
        if next <= at {
            return None;
        }
        at = next;
    }
    None
}

/// Ijro joyi va oyna uzunligi bo'yicha `[boshlanish bo'lagi, chegara
/// bo'lagi]` — O'RTACHA bitreyt bo'yicha (zaxira).
fn avg_window(pos_ms: u64, dur_ms: u64, total: u64) -> (u64, u64) {
    let at = |ms: u64| (ms as u128 * total as u128 / dur_ms.max(1) as u128) as u64;
    let last = total.saturating_sub(1) / video_cache::PLAYER_CHUNK;
    let from = (at(pos_ms) / video_cache::PLAYER_CHUNK).min(last);
    let to = (at(pos_ms + AHEAD_MS) / video_cache::PLAYER_CHUNK).min(last);
    (from, to)
}

/// Fayldan bo'laklar orqali `[off, off+len)` ni yig'adi (diskdan yoki
/// Telegram'dan).
fn read_span(r: &Reader, off: u64, len: u64) -> Option<Vec<u8>> {
    let end = off.checked_add(len)?.min(r.total);
    let mut out = Vec::with_capacity(len.min(1 << 26) as usize);
    let mut pos = off;
    while pos < end {
        let idx = pos / video_cache::PLAYER_CHUNK;
        let c = load_chunk(&r.name, &r.dir, r.total, idx, "moov").ok()?;
        let o = (pos - idx * video_cache::PLAYER_CHUNK) as usize;
        let n = ((end - pos) as usize).min(c.len().checked_sub(o)?);
        if n == 0 {
            return None;
        }
        out.extend_from_slice(&c[o..o + n]);
        pos += n as u64;
    }
    Some(out)
}

/// Namuna jadvali: o'qilgan bo'lsa shu; `parse` bo'lsa va hali o'qilmagan
/// bo'lsa — o'qiydi (tarmoq bo'lishi mumkin: FAQAT fon oqimidan).
fn track_of(r: &Reader, parse: bool) -> Option<Arc<Tracks>> {
    {
        let g = r.track.lock().ok()?;
        if let Some(t) = &g.0 {
            return Some(Arc::clone(t));
        }
        if !parse || g.1 >= 3 {
            return None;
        }
    }
    let t = track_from(r.total, &|off, len| read_span(r, off, len)).map(Arc::new);
    let mut g = r.track.lock().ok()?;
    match t {
        Some(t) => {
            g.0 = Some(Arc::clone(&t));
            // Pleyer chizig'i uchun aniq "bayt → soniya" jadvali.
            let tm = TimeMap {
                chunk_ms: t.video.chunk_start_ms(video_cache::PLAYER_CHUNK, r.total),
                dur_ms: (t.video.duration_secs() * 1000.0) as u64,
            };
            if let Ok(mut m) = time_maps().lock() {
                m.insert(video_cache::player_key(&r.name), Arc::new(tm));
            }
            Some(t)
        }
        None => {
            g.1 += 1; // 3 marta o'qilmasa — o'rtacha bitreyt bilan davom
            None
        }
    }
}

/// Oyna: bo'laklar oraliqlari (video va ovoz alohida — ular faylda
/// uzoq-uzoqda turishi mumkin) — `[joy bo'lagi, chegara bo'lagi]`.
///
/// OVOZ HAM HISOBGA OLINADI (foydalanuvchi: "video qotib qoldi"):
/// ovoz va video kalta ketma-ketlikda joylashmagan (qo'pol interleave)
/// fayllarda ExoPlayer ovoz baytlari uchun faylning BOSHQA joyiga
/// sakraydi. Oyna faqat video bo'yicha bo'lsa, o'sha bo'lak "oynadan
/// tashqari" deb kutdirilib, pleyer qotib qolardi. Endi ikkala
/// yo'lakcha uchun `pos .. pos + 1 daqiqa` baytlari oynaga kiradi —
/// bu 1 daqiqalik ijro uchun eng kam kerakli baytlar.
fn windows_of(r: &Reader, parse: bool) -> Option<Vec<(u64, u64)>> {
    let (pos, dur) = {
        let m = positions().lock().ok()?;
        let p = m.get(&r.name)?;
        (p.pos_ms, p.dur_ms)
    };
    if dur == 0 || r.total == 0 {
        return None;
    }
    let last = r.total.saturating_sub(1) / video_cache::PLAYER_CHUNK;
    let mut out = Vec::new();
    if let Some(t) = track_of(r, parse) {
        for tr in std::iter::once(&t.video).chain(t.audio.iter()) {
            let off = |ms: u64| tr.locate(tr.sample_at_ms(ms)).map(|x| x.offset);
            if let (Some(a), Some(b)) = (off(pos), off(pos + AHEAD_MS)) {
                out.push((
                    (a / video_cache::PLAYER_CHUNK).min(last),
                    (b / video_cache::PLAYER_CHUNK).min(last),
                ));
            }
        }
    }
    if out.is_empty() {
        out.push(avg_window(pos, dur, r.total));
    }
    Some(out)
}

fn fill_window(name: &str) {
    let Some(r) = readers().lock().ok().and_then(|m| {
        m.values().find(|r| r.name == name && !r.closed.load(Ordering::SeqCst)).cloned()
    }) else {
        return;
    };
    match filling().lock() {
        Ok(mut f) => {
            if !f.insert(name.to_string()) {
                return; // allaqachon to'ldirilyapti
            }
        }
        Err(_) => return,
    }
    std::thread::spawn(move || {
        loop {
            if r.closed.load(Ordering::SeqCst) {
                break;
            }
            let Some(wins) = windows_of(&r, true) else { break };
            let next = wins.iter().flat_map(|&(a, b)| a..=b).find(|&i| {
                !video_cache::player_has_chunk(&r.dir, i, r.total)
                    && !flights().lock().map(|m| m.contains_key(&(r.name.clone(), i))).unwrap_or(true)
            });
            let Some(i) = next else { break }; // oyna to'la
            if load_chunk(&r.name, &r.dir, r.total, i, "oyna").is_err() {
                break; // tarmoq xatosi — keyingi joy xabarida qayta
            }
        }
        if let Ok(mut f) = filling().lock() {
            f.remove(&r.name);
        }
    });
}

/// Shu bo'lakni hozir tarmoqdan olsa bo'ladimi.
fn net_allowed(r: &Reader, index: u64) -> bool {
    let start = index * video_cache::PLAYER_CHUNK;
    let edge = 4 * video_cache::PLAYER_CHUNK;
    if start < edge || start + 2 * edge >= r.total {
        return true;
    }
    {
        let Ok(m) = positions().lock() else { return true };
        let Some(p) = m.get(&r.name) else { return true };
        if p.buffering || p.dur_ms == 0 || p.at.elapsed() > std::time::Duration::from_secs(120) {
            return true;
        }
    }
    match windows_of(r, false) {
        Some(wins) => wins.iter().any(|&(a, b)| index + 1 >= a && index <= b + 1),
        None => true,
    }
}

type ChunkResult = Result<Arc<Vec<u8>>, String>;

struct Reader {
    name: String,
    dir: PathBuf,
    total: u64,
    closed: Arc<AtomicBool>,
    /// Oxirgi o'qilgan bo'lak (pleyer uni mayda bo'laklab o'qiydi —
    /// har safar diskdan ochib o'tirmaslik uchun).
    last: Mutex<Option<(u64, Arc<Vec<u8>>)>>,
    /// Namuna jadvali (`moov`) va o'qib bo'lmagan urinishlar soni.
    track: Mutex<(Option<Arc<Tracks>>, u8)>,
    /// Pleyer so'ragan bo'lak oynadan tashqarida bo'lib, kutilayotgan
    /// bo'lsa: (bo'lak, qachondan). Uzoq kutilsa — majburan beriladi.
    waiting: Mutex<Option<(u64, std::time::Instant)>>,
}

/// Bir bo'lakni ikki marta yuklamaslik uchun: (fayl, indeks) -> kutish.
struct Flight {
    done: Mutex<Option<ChunkResult>>,
    cv: Condvar,
}

/// Shu kalitdagi fayl pleyerda ochiqmi (`video_cache::ThumbDirGuard`).
pub(crate) fn is_open(key: &str) -> bool {
    readers()
        .lock()
        .map(|m| {
            m.values().any(|r| {
                !r.closed.load(Ordering::SeqCst) && video_cache::player_key(&r.name) == key
            })
        })
        .unwrap_or(true)
}

fn readers() -> &'static Mutex<HashMap<i64, Arc<Reader>>> {
    static R: OnceLock<Mutex<HashMap<i64, Arc<Reader>>>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(HashMap::new()))
}

fn flights() -> &'static Mutex<HashMap<(String, u64), Arc<Flight>>> {
    static F: OnceLock<Mutex<HashMap<(String, u64), Arc<Flight>>>> = OnceLock::new();
    F.get_or_init(|| Mutex::new(HashMap::new()))
}

static NEXT: AtomicI64 = AtomicI64::new(1);

fn chunk_len(index: u64, total: u64) -> u64 {
    let start = index * video_cache::PLAYER_CHUNK;
    if start >= total {
        return 0;
    }
    (total - start).min(video_cache::PLAYER_CHUNK)
}

/// Faylni ochadi: hajm avval meta.json dan, keyin ilova bergan
/// hajmdan (`epizod_db.size_*`), bo'lmasa Telegram'dan.
fn open(name: &str, size_hint: u64) -> Result<i64, String> {
    let dir = video_cache::player_dir(name).ok_or("kesh tayyor emas")?;
    let mut total = video_cache::player_total(&dir);
    // HAQIQIY hajm ustun: bazadagi `size_*` fayl qayta yuklanganda
    // eskirib qolishi mumkin. Noto'g'ri hajm bilan oxirgi bo'lak
    // "qisqa javob" bo'lib yiqilardi (MP4 sarlavhasi ko'pincha oxirida
    // — ya'ni video umuman ochilmasdi).
    if let Some((size, mime)) = telegram::cached_doc_size(name) {
        if size != total {
            // Fayl almashgan (qayta yuklangan) — eski bo'laklar boshqa
            // faylniki, ular o'qilmasin.
            if total != 0 {
                let _ = std::fs::remove_dir_all(&dir);
                let _ = std::fs::create_dir_all(&dir);
            }
            video_cache::player_set_total(&dir, size, &mime);
            total = size;
        }
    }
    if total == 0 && size_hint > 0 {
        video_cache::player_set_total(&dir, size_hint, telegram::mime_of_name(name));
        total = size_hint;
    }
    if total == 0 {
        let (size, mime) = telegram::doc_size(name)?;
        if size == 0 {
            return Err("bo'sh fayl".to_string());
        }
        video_cache::player_set_total(&dir, size, &mime);
        total = size;
    }
    let h = NEXT.fetch_add(1, Ordering::SeqCst);
    let r = Arc::new(Reader {
        name: name.to_string(),
        dir,
        total,
        closed: Arc::new(AtomicBool::new(false)),
        last: Mutex::new(None),
        track: Mutex::new((None, 0)),
        waiting: Mutex::new(None),
    });
    readers().lock().map_err(|e| e.to_string())?.insert(h, r);
    Ok(h)
}

fn reader(h: i64) -> Option<Arc<Reader>> {
    readers().lock().ok()?.get(&h).cloned()
}

fn close(h: i64) {
    if let Some(r) = readers().lock().ok().and_then(|mut m| m.remove(&h)) {
        r.closed.store(true, Ordering::SeqCst);
    }
}

/// Bo'lakni diskdan yoki Telegram'dan oladi. Bir xil bo'lakni boshqa
/// oqim olayotgan bo'lsa — o'shani kutadi.
fn load_chunk(name: &str, dir: &PathBuf, total: u64, index: u64, why: &str) -> ChunkResult {
    if let Some(b) = video_cache::player_read_chunk(dir, name, index, total) {
        return Ok(Arc::new(b));
    }
    let key = (name.to_string(), index);
    let (flight, owner) = {
        let mut m = flights().lock().map_err(|e| e.to_string())?;
        match m.get(&key) {
            Some(f) => (Arc::clone(f), false),
            None => {
                let f = Arc::new(Flight { done: Mutex::new(None), cv: Condvar::new() });
                m.insert(key.clone(), Arc::clone(&f));
                (f, true)
            }
        }
    };
    if !owner {
        let mut g = flight.done.lock().map_err(|e| e.to_string())?;
        while g.is_none() {
            g = flight.cv.wait(g).map_err(|e| e.to_string())?;
        }
        return g.clone().unwrap();
    }
    // Kutish orasida boshqa oqim yozib ulgurgan bo'lishi mumkin.
    let res: ChunkResult = match video_cache::player_read_chunk(dir, name, index, total) {
        Some(b) => Ok(Arc::new(b)),
        None => {
            let len = chunk_len(index, total);
            // Diagnostika: tarmoqdan olingan har bir bo'lak — sababi bilan
            // (`pleyer` / `oyna` / `oldindan` / `moov`) va ijro joyi bilan.
            video_cache::tg_log(format!(
                "Pleyer: {name} #{index} tarmoqdan ({why}), joy {}",
                positions()
                    .lock()
                    .ok()
                    .and_then(|m| m.get(name).map(|p| format!("{} s", p.pos_ms / 1000)))
                    .unwrap_or_else(|| "?".into())
            ));
            telegram::fetch_range(name, index * video_cache::PLAYER_CHUNK, len).and_then(|b| {
                if b.len() as u64 != len {
                    return Err(format!("qisqa javob: {} / {len}", b.len()));
                }
                video_cache::player_write_chunk(dir, name, index, total, &b);
                Ok(Arc::new(b))
            })
        }
    };
    if let Ok(mut g) = flight.done.lock() {
        *g = Some(res.clone());
    }
    flight.cv.notify_all();
    if let Ok(mut m) = flights().lock() {
        m.remove(&key);
    }
    res
}

/// Keyingi bo'laklarni fonda tayyorlaydi (diskda yo'q va hech kim
/// olmayotganlarini).
fn prefetch(r: &Arc<Reader>, index: u64) {
    let last = (r.total.saturating_sub(1)) / video_cache::PLAYER_CHUNK;
    for i in index + 1..=(index + AHEAD).min(last) {
        if r.closed.load(Ordering::SeqCst) {
            return;
        }
        if !net_allowed(r, i) {
            return;
        }
        if flights().lock().map(|m| m.contains_key(&(r.name.clone(), i))).unwrap_or(true) {
            continue;
        }
        let r = Arc::clone(r);
        std::thread::spawn(move || {
            if !r.closed.load(Ordering::SeqCst) {
                let _ = load_chunk(&r.name, &r.dir, r.total, i, "oldindan");
            }
        });
    }
}

/// `pos` dan `out` ga o'qiydi (bo'lak chegarasidan o'tmaydi).
/// Qaytadi: o'qilgan bayt, fayl oxirida 0.
fn read(h: i64, pos: u64, out: &mut [u8]) -> Result<usize, String> {
    let r = reader(h).ok_or("manba yopilgan")?;
    if pos >= r.total || out.is_empty() {
        return Ok(0);
    }
    let index = pos / video_cache::PLAYER_CHUNK;
    let cached = r
        .last
        .lock()
        .ok()
        .and_then(|l| l.as_ref().filter(|(i, _)| *i == index).map(|(_, b)| Arc::clone(b)));
    let chunk = match cached {
        Some(b) => b,
        None => {
            // Oyna (`pos .. pos + 1 daqiqa`, `moov` bo'yicha aniq) ichida,
            // yoki pleyer ma'lumot kutayotgan bo'lsa — olinadi. Aks holda
            // diskda bo'lmasa kutadi (`WAIT`). Ketma-ket o'qish uchun
            // alohida ruxsat YO'Q: pleyerning o'z zaxirasi (≤30 s) oynaga
            // sig'adi, ruxsat esa oynani sekin "sudrab" ketardi.
            let allowed = net_allowed(&r, index) || {
                // Pleyer bir bo'lakni 3 soniyadan ortiq kutsa — majburan
                // beriladi: qotib qolgandan ko'ra bitta ortiqcha bo'lak.
                let mut w = r.waiting.lock().map_err(|e| e.to_string())?;
                match *w {
                    Some((i, since)) if i == index => since.elapsed() >= std::time::Duration::from_secs(3),
                    _ => {
                        *w = Some((index, std::time::Instant::now()));
                        false
                    }
                }
            };
            let b = if allowed {
                if let Ok(mut w) = r.waiting.lock() {
                    *w = None;
                }
                load_chunk(&r.name, &r.dir, r.total, index, "pleyer")?
            } else {
                match video_cache::player_read_chunk(&r.dir, &r.name, index, r.total) {
                    Some(b) => Arc::new(b),
                    None => return Err(WAIT.to_string()),
                }
            };
            if let Ok(mut l) = r.last.lock() {
                *l = Some((index, Arc::clone(&b)));
            }
            prefetch(&r, index);
            b
        }
    };
    let off = (pos - index * video_cache::PLAYER_CHUNK) as usize;
    let n = out.len().min(chunk.len().saturating_sub(off));
    out[..n].copy_from_slice(&chunk[off..off + n]);
    Ok(n)
}

fn size(h: i64) -> i64 {
    reader(h).map(|r| r.total as i64).unwrap_or(-1)
}

/// Ilova ijro joyini beradi (har soniyada va sek qilinganda).
/// `name` — fayl nomi (`aru://file/<nom>` dagi), `dur_ms` 0 bo'lsa
/// yozuv o'chiriladi (pleyer yopildi).
#[no_mangle]
pub extern "C" fn rust_player_position(
    name: *const std::os::raw::c_char,
    pos_ms: i64,
    dur_ms: i64,
    buffering: i32,
) {
    if name.is_null() {
        return;
    }
    let Ok(name) = unsafe { std::ffi::CStr::from_ptr(name) }.to_str() else { return };
    let Ok(mut m) = positions().lock() else { return };
    if dur_ms <= 0 {
        m.remove(name);
        return;
    }
    m.insert(name.to_string(), PlayPos {
        pos_ms: pos_ms.max(0) as u64,
        dur_ms: dur_ms as u64,
        buffering: buffering != 0,
        at: std::time::Instant::now(),
    });
    drop(m);
    fill_window(name);
}

// ═══════════════════════════════════════════════════════════════
//  JNI — io.flutter.plugins.videoplayer.AruDataSource
// ═══════════════════════════════════════════════════════════════

use jni::objects::{JByteArray, JClass, JString};
use jni::sys::{jint, jlong};
use jni::JNIEnv;

/// Tarmoq xatosi: Java biroz kutib qayta so'raydi.
const RETRY: i64 = -2;

/// Ochadi. Qaytadi: dastak (> 0); 0 — xato (Java IOException
/// tashlaydi); `RETRY` — internet yo'q, keyinroq qayta urinish.
#[no_mangle]
pub extern "system" fn Java_io_flutter_plugins_videoplayer_AruDataSource_nativeOpen<'l>(
    mut env: JNIEnv<'l>,
    _c: JClass<'l>,
    name: JString<'l>,
    size_hint: jlong,
) -> jlong {
    let Ok(name) = env.get_string(&name).map(String::from) else { return 0 };
    match open(&name, size_hint.max(0) as u64) {
        Ok(h) => h,
        Err(e) if telegram::is_net_err(&e) => RETRY,
        Err(e) => {
            video_cache::tg_log(format!("Pleyer: {name} ochilmadi: {e}"));
            0
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_io_flutter_plugins_videoplayer_AruDataSource_nativeSize<'l>(
    _env: JNIEnv<'l>,
    _c: JClass<'l>,
    h: jlong,
) -> jlong {
    size(h)
}

/// Qaytadi: o'qilgan bayt; 0 — fayl oxiri; -1 — xato; -2 (`RETRY`) —
/// internet yo'q, Java kutib qayta so'raydi.
#[no_mangle]
pub extern "system" fn Java_io_flutter_plugins_videoplayer_AruDataSource_nativeRead<'l>(
    env: JNIEnv<'l>,
    _c: JClass<'l>,
    h: jlong,
    pos: jlong,
    buf: JByteArray<'l>,
    off: jint,
    len: jint,
) -> jint {
    if pos < 0 || off < 0 || len <= 0 {
        return 0;
    }
    let mut tmp = vec![0u8; len as usize];
    match read(h, pos as u64, &mut tmp) {
        Ok(0) => 0,
        Ok(n) => {
            let signed: &[i8] =
                unsafe { std::slice::from_raw_parts(tmp.as_ptr() as *const i8, n) };
            if env.set_byte_array_region(&buf, off, signed).is_err() {
                return -1;
            }
            n as jint
        }
        Err(e) if e == WAIT || telegram::is_net_err(&e) => RETRY as jint,
        Err(e) => {
            video_cache::tg_log(format!("Pleyer: o'qib bo'lmadi: {e}"));
            -1
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_io_flutter_plugins_videoplayer_AruDataSource_nativeClose<'l>(
    _env: JNIEnv<'l>,
    _c: JClass<'l>,
    h: jlong,
) {
    close(h);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_reader(name: &str, total: u64) -> Reader {
        Reader {
            name: name.to_string(),
            dir: PathBuf::from("/nonexistent"),
            total,
            closed: Arc::new(AtomicBool::new(false)),
            last: Mutex::new(None),
            track: Mutex::new((None, 0)),
            waiting: Mutex::new(None),
        }
    }

    #[test]
    fn oldinga_bir_daqiqadan_ortiq_olinmaydi() {
        let c = video_cache::PLAYER_CHUNK;
        let total = 600 * c; // 600 MiB, 600 s — 1 MiB/s
        let name = "test_oldinga.mp4";
        let r = test_reader(name, total);
        // Joy noma'lum — cheklov yo'q.
        assert!(net_allowed(&r, 300));
        let cname = std::ffi::CString::new(name).unwrap();
        rust_player_position(cname.as_ptr(), 100_000, 600_000, 0);
        assert!(net_allowed(&r, 150)); // 150 s — 50 s oldinda
        assert!(!net_allowed(&r, 170)); // 170 s — 70 s oldinda
        assert!(net_allowed(&r, 598)); // fayl oxiri (moov)
        assert!(net_allowed(&r, 1)); // fayl boshi
        // Pleyer buferlanmoqda (to'xtab qolgan) — kerakli bo'lak beriladi.
        rust_player_position(cname.as_ptr(), 100_000, 600_000, 1);
        assert!(net_allowed(&r, 300));
        rust_player_position(cname.as_ptr(), 0, 0, 0);
    }

    #[test]
    fn oyna_joy_bilan_birga_suriladi() {
        let c = video_cache::PLAYER_CHUNK;
        let total = 600 * c; // 600 s — 1 MiB/s
        let name = "test_oyna.mp4";
        let r = test_reader(name, total);
        let cname = std::ffi::CString::new(name).unwrap();
        rust_player_position(cname.as_ptr(), 60_000, 600_000, 0);
        assert_eq!(windows_of(&r, false), Some(vec![(60, 120)]));
        rust_player_position(cname.as_ptr(), 61_000, 600_000, 0);
        assert_eq!(windows_of(&r, false), Some(vec![(61, 121)]));
        rust_player_position(cname.as_ptr(), 590_000, 600_000, 0);
        assert_eq!(windows_of(&r, false), Some(vec![(590, 599)]));
        rust_player_position(cname.as_ptr(), 0, 0, 0);
        assert_eq!(windows_of(&r, false), None);
    }

    /// Haqiqiy MP4 (`gif_h264.mp4`): namuna jadvali `moov` dan o'qiladi va
    /// vaqt → bayt monoton o'sadi (VBR uchun aniq oyna asosi).
    #[test]
    fn moov_dan_soniya_bayt_jadvali() {
        let file = include_bytes!("testdata/gif_h264.mp4");
        let total = file.len() as u64;
        let read = |off: u64, len: u64| -> Option<Vec<u8>> {
            let end = off.checked_add(len)?.min(total);
            file.get(off as usize..end as usize).map(|s| s.to_vec())
        };
        let tracks = track_from(total, &read).expect("moov o'qilishi kerak");
        let t = &tracks.video;
        let mut prev = 0u64;
        for ms in (0..2000u64).step_by(100) {
            let s = t.sample_at_ms(ms);
            let off = t.locate(s).expect("namuna joyi").offset;
            assert!(off >= prev, "vaqt o'sdi, bayt kamaydi: {ms} ms");
            assert!(off < total);
            prev = off;
        }
        // "Bayt → soniya" jadvali (pleyer chizig'i): monoton va davomiylikdan oshmaydi.
        let table = t.chunk_start_ms(video_cache::PLAYER_CHUNK, total);
        assert_eq!(table.len() as u64, total.div_ceil(video_cache::PLAYER_CHUNK));
        let dur_ms = (t.duration_secs() * 1000.0) as u32;
        assert!(table.windows(2).all(|w| w[0] <= w[1]));
        assert!(table.iter().all(|&v| v <= dur_ms));
    }

    #[test]
    fn chunk_len_edges() {
        let c = video_cache::PLAYER_CHUNK;
        assert_eq!(chunk_len(0, 10), 10);
        assert_eq!(chunk_len(0, c + 5), c);
        assert_eq!(chunk_len(1, c + 5), 5);
        assert_eq!(chunk_len(2, c + 5), 0);
    }
}
