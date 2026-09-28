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
const AHEAD: u64 = 2;

// ── ONLAYN KO'RISHDA KO'PI BILAN 1 DAQIQA OLDINGA ─────────────────
//
// TALAB (foydalanuvchi): "onlayn ko'rishda ko'rayotgan daqiqadan
// 1 daqiqagacha yuklab olishga ruxsat bo'lsin, undan ko'p emas".
//
// Ilova har soniyada ijro joyini beradi (`rust_player_position`).
// Diskda YO'Q bo'lak Telegram'dan faqat u ijro joyidan ko'pi bilan
// `AHEAD_MS` oldinda bo'lsa olinadi (bayt/vaqt — faylning o'rtacha
// bitreyti bo'yicha). Undan uzoqdagisi so'ralsa `WAIT` qaytadi — Java
// (`AruDataSource`) biroz kutib qayta so'raydi, ijro esa buferdan davom
// etadi va joy surilgan sari chegara ham suriladi.
//
// Istisnolar (ular bo'lmasa video ochilmay qolishi mumkin): fayl boshi
// va oxiri (MP4 sarlavhasi `moov` ko'pincha oxirida), hamda ijro joyi
// noma'lum yoki eskirgan (2 daqiqadan ko'p xabar kelmagan) holat. Surish
// (barmoq ekranda) paytida xabar to'xtaydi — shu sabab muddat uzun.
const AHEAD_MS: u64 = 60_000;
/// Chegaradan tashqari so'rov — JNI buni `RETRY` qiladi.
const WAIT: &str = "oldinga chegara";

struct PlayPos {
    pos_ms: u64,
    dur_ms: u64,
    at: std::time::Instant,
}

fn positions() -> &'static Mutex<HashMap<String, PlayPos>> {
    static P: OnceLock<Mutex<HashMap<String, PlayPos>>> = OnceLock::new();
    P.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Shu bo'lakni hozir tarmoqdan olsa bo'ladimi.
fn net_allowed(name: &str, total: u64, index: u64) -> bool {
    let start = index * video_cache::PLAYER_CHUNK;
    let edge = 4 * video_cache::PLAYER_CHUNK;
    if start < edge || start + 2 * edge >= total {
        return true;
    }
    let Ok(m) = positions().lock() else { return true };
    let Some(p) = m.get(name) else { return true };
    if p.dur_ms == 0 || p.at.elapsed() > std::time::Duration::from_secs(120) {
        return true;
    }
    let limit = ((p.pos_ms + AHEAD_MS) as u128 * total as u128 / p.dur_ms as u128) as u64
        + video_cache::PLAYER_CHUNK;
    start <= limit
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
}

/// Bir bo'lakni ikki marta yuklamaslik uchun: (fayl, indeks) -> kutish.
struct Flight {
    done: Mutex<Option<ChunkResult>>,
    cv: Condvar,
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
fn load_chunk(name: &str, dir: &PathBuf, total: u64, index: u64) -> ChunkResult {
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
        if !net_allowed(&r.name, r.total, i) {
            return;
        }
        if flights().lock().map(|m| m.contains_key(&(r.name.clone(), i))).unwrap_or(true) {
            continue;
        }
        let r = Arc::clone(r);
        std::thread::spawn(move || {
            if !r.closed.load(Ordering::SeqCst) {
                let _ = load_chunk(&r.name, &r.dir, r.total, i);
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
            let b = if net_allowed(&r.name, r.total, index) {
                load_chunk(&r.name, &r.dir, r.total, index)?
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
pub extern "C" fn rust_player_position(name: *const std::os::raw::c_char, pos_ms: i64, dur_ms: i64) {
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
        at: std::time::Instant::now(),
    });
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

    #[test]
    fn oldinga_bir_daqiqadan_ortiq_olinmaydi() {
        let c = video_cache::PLAYER_CHUNK;
        let total = 600 * c; // 600 MiB, 600 s — 1 MiB/s
        let name = "test_oldinga.mp4";
        // Joy noma'lum — cheklov yo'q.
        assert!(net_allowed(name, total, 300));
        let cname = std::ffi::CString::new(name).unwrap();
        rust_player_position(cname.as_ptr(), 100_000, 600_000);
        assert!(net_allowed(name, total, 150)); // 150 s — 50 s oldinda
        assert!(!net_allowed(name, total, 170)); // 170 s — 70 s oldinda
        assert!(net_allowed(name, total, 598)); // fayl oxiri (moov)
        assert!(net_allowed(name, total, 1)); // fayl boshi
        rust_player_position(cname.as_ptr(), 0, 0);
        assert!(net_allowed(name, total, 300));
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
