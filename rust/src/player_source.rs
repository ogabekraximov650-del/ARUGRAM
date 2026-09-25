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

use crate::{telegram, video_cache};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};

/// Pleyer o'qiyotgan joydan oldinga shuncha bo'lak tayyorlab qo'yiladi.
const AHEAD: u64 = 2;

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

/// Faylni ochadi: hajm avval meta.json dan, bo'lmasa Telegram'dan.
fn open(name: &str) -> Result<i64, String> {
    let dir = video_cache::player_dir(name).ok_or("kesh tayyor emas")?;
    let mut total = video_cache::player_total(&dir);
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
            let b = load_chunk(&r.name, &r.dir, r.total, index)?;
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

// ═══════════════════════════════════════════════════════════════
//  JNI — io.flutter.plugins.videoplayer.AruDataSource
// ═══════════════════════════════════════════════════════════════

use jni::objects::{JByteArray, JClass, JString};
use jni::sys::{jint, jlong};
use jni::JNIEnv;

/// Ochadi. Xato bo'lsa 0 (Java IOException tashlaydi).
#[no_mangle]
pub extern "system" fn Java_io_flutter_plugins_videoplayer_AruDataSource_nativeOpen<'l>(
    mut env: JNIEnv<'l>,
    _c: JClass<'l>,
    name: JString<'l>,
) -> jlong {
    let Ok(name) = env.get_string(&name).map(String::from) else { return 0 };
    match open(&name) {
        Ok(h) => h,
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

/// Qaytadi: o'qilgan bayt; 0 — fayl oxiri; -1 — xato.
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
    fn chunk_len_edges() {
        let c = video_cache::PLAYER_CHUNK;
        assert_eq!(chunk_len(0, 10), 10);
        assert_eq!(chunk_len(0, c + 5), c);
        assert_eq!(chunk_len(1, c + 5), 5);
        assert_eq!(chunk_len(2, c + 5), 0);
    }
}
