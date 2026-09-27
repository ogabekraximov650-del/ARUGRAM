//! Telegram animatsiyali stikerlari va maxsus emoji kadrlarini chizish.
//!
//! TALAB (foydalanuvchi): "stikerlar Telegram'dagidek harakatlansin,
//! to'plamlarni ochganda ilova qotmasin — Cherrygram qanday qilsa
//! shunday qil".
//!
//! Telegram/Cherrygram (`RLottieDrawable`, `AnimatedFileDrawable`)
//! stikerlarni tizim pleyeri bilan EMAS, ilova ichidagi kutubxonalar
//! bilan, FON OQIMIDA chizadi:
//!   * `.tgs` — tlottie (Telegram Android / Cherrygram'ning o'z Lottie
//!     chizgichi, sof Rust; gzip'langan Lottie JSON);
//!   * `.webm` — libvpx: rang va shaffoflik ikki alohida VP9 oqimi
//!     (`native/vp9_shim.c`).
//!
//! Bu yerda ham xuddi shunday: Dart kadrni fon isolate'idan so'raydi
//! (`rust_anim_render`), UI oqimi faqat tayyor rasmni chizadi.

use std::collections::HashMap;
use std::ffi::{c_char, c_void};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use crate::ffi_utils::cstr_to_str;

extern "C" {
    fn aru_h264_open(extra: *const u8, len: i32) -> *mut c_void;
    fn aru_h264_close(p: *mut c_void);
    fn aru_h264_send(p: *mut c_void, data: *const u8, len: i32) -> i32;
    fn aru_h264_recv(p: *mut c_void, out: *mut u8, w: i32, h: i32) -> i32;
    fn aru_vp9_open() -> *mut c_void;
    fn aru_vp9_close(p: *mut c_void);
    fn aru_vp9_decode(
        p: *mut c_void,
        color: *const u8,
        clen: usize,
        alpha: *const u8,
        alen: usize,
        out: *mut u8,
        w: i32,
        h: i32,
    ) -> i32;
}

/// WebM'dagi bitta kadr: rang va (bo'lsa) shaffoflik bloki.
struct WebmFrame {
    color: Vec<u8>,
    alpha: Option<Vec<u8>>,
}

enum Kind {
    /// Telegram'ning o'z Lottie chizgichi (tlottie).
    Lottie { r: Box<tlottie::CPURenderer>, frames: usize, fps: f64 },
    Webm { dec: *mut c_void, frames: Vec<WebmFrame>, fps: f64, next: usize },
    /// GIF (H.264 MP4) — ffmpeg dekoderi (`native/h264_shim.c`).
    Mp4 {
        dec: *mut c_void,
        data: Vec<u8>,
        mp4: Mp4Info,
        fps: f64,
        /// Dekoderga berilgan namunalar soni.
        sent: usize,
        /// Chiqqan (ko'rsatish tartibidagi) kadrlar soni.
        out: usize,
        eof: bool,
    },
}

// Ko'rsatkichlar faqat `Mutex` ichida, bitta oqimda ishlatiladi.
unsafe impl Send for Kind {}

struct Anim {
    kind: Kind,
    w: usize,
    h: usize,
    disk: Option<DiskFrames>,
}

// ── KADRLAR DISKDA (Telegram `BitmapsCache` kabi) ──────────────────
//
// TALAB (foydalanuvchi): "Telegram qanday xatosiz ishlasa shunday —
// stikerlar qotmasin, telefon qizimasin".
//
// Telegram stiker kadrini bir marta chizadi va siqilgan holda diskda
// saqlaydi; keyingi aylanishlarda (va stiker qayta ochilganda) kadr
// chizilmaydi — diskdan o'qib ochiladi. Bu yerda ham shunday:
// chizilgan kadr `deflate` (tez daraja) bilan siqiladi va tutqich
// yopilganda stiker yonidagi `.afc` faylga yoziladi. Keyingi safar
// kadr diskdan o'qiladi (VP9 dekoder va Lottie chizgich ishlamaydi).
// Papkadagi `.afc` fayllari jami [DISK_BUDGET] dan oshsa eng eskilari
// o'chadi; ular stiker fayllari bilan birga "Keshni tozalash"da ham
// o'chadi.

const DISK_BUDGET: u64 = 200 * 1024 * 1024;
const MAGIC: &[u8; 4] = b"AFC1";

// XOTIRA (foydalanuvchi: "2 GB telefonda ham silliq ishlasin"):
// ilgari shu safar chizilgan kadrlar tutqich yopilguncha XOTIRADA
// turardi — uzun GIF (300 kadr, 512 px) o'nlab MB egallardi, yopishda
// esa butun fayl yana ikki marta xotirada yig'ilardi. Endi yangi kadr
// darhol vaqtinchalik `.spill` faylga yoziladi (xotirada faqat joyi),
// yopishda esa `.afc` bo'lakma-bo'lak, oqim bilan yoziladi.

struct DiskFrames {
    path: PathBuf,
    /// Diskdagi kadrlar: (joyi, uzunligi); uzunlik 0 — yo'q.
    table: Vec<(u64, u32)>,
    file: Option<std::fs::File>,
    /// Shu safar chizilgan (siqilgan) yangi kadrlar: `.spill` dagi joyi.
    fresh: HashMap<usize, (u64, u32)>,
    spill: Option<(PathBuf, std::fs::File, u64)>,
}

fn read_at(file: &mut std::fs::File, o: u64, l: u32) -> Option<Vec<u8>> {
    if l == 0 || file.seek(SeekFrom::Start(o)).is_err() {
        return None;
    }
    let mut v = vec![0u8; l as usize];
    file.read_exact(&mut v).ok()?;
    Some(v)
}

impl DiskFrames {
    fn open(path: PathBuf, frames: usize, w: usize, h: usize) -> DiskFrames {
        let mut d =
            DiskFrames { path, table: vec![(0, 0); frames], file: None, fresh: HashMap::new(), spill: None };
        if let Ok(mut f) = std::fs::File::open(&d.path) {
            let mut head = [0u8; 16];
            if f.read_exact(&mut head).is_ok()
                && &head[0..4] == MAGIC
                && u32::from_le_bytes(head[4..8].try_into().unwrap()) as usize == w
                && u32::from_le_bytes(head[8..12].try_into().unwrap()) as usize == h
                && u32::from_le_bytes(head[12..16].try_into().unwrap()) as usize == frames
            {
                let mut tab = vec![0u8; frames * 12];
                if f.read_exact(&mut tab).is_ok() {
                    for i in 0..frames {
                        let o = u64::from_le_bytes(tab[i * 12..i * 12 + 8].try_into().unwrap());
                        let l = u32::from_le_bytes(tab[i * 12 + 8..i * 12 + 12].try_into().unwrap());
                        d.table[i] = (o, l);
                    }
                    d.file = Some(f);
                }
            }
        }
        d
    }

    /// Siqilgan kadr (yangi yoki diskdagi).
    fn raw(&mut self, f: usize) -> Option<Vec<u8>> {
        if let Some(&(o, l)) = self.fresh.get(&f) {
            let (_, file, _) = self.spill.as_mut()?;
            return read_at(file, o, l);
        }
        let (o, l) = self.table.get(f).copied().unwrap_or((0, 0));
        read_at(self.file.as_mut()?, o, l)
    }

    /// Kadr diskda (yoki shu safar chizilgan) bo'lsa — [out] ga ochadi.
    fn read(&mut self, f: usize, out: &mut [u8]) -> bool {
        let Some(data) = self.raw(f) else { return false };
        let mut z = flate2::read::DeflateDecoder::new(&data[..]);
        z.read_exact(out).is_ok()
    }

    fn has(&self, f: usize) -> bool {
        self.fresh.contains_key(&f) || self.table.get(f).map(|t| t.1 > 0).unwrap_or(false)
    }

    fn put(&mut self, f: usize, px: &[u8]) {
        if self.has(f) {
            return;
        }
        let mut z = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::fast());
        if z.write_all(px).is_err() {
            return;
        }
        let Ok(v) = z.finish() else { return };
        if self.spill.is_none() {
            // Bir stiker bir necha joyda ochiq bo'lishi mumkin — har
            // tutqich o'z vaqtinchalik faylida.
            let sp = self.path.with_extension(format!("afc.{}.spill", SEQ.fetch_add(1, Ordering::SeqCst)));
            let Ok(file) =
                std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(true).open(&sp)
            else {
                return;
            };
            self.spill = Some((sp, file, 0));
        }
        let Some((_, file, end)) = self.spill.as_mut() else { return };
        if file.seek(SeekFrom::Start(*end)).is_ok() && file.write_all(&v).is_ok() {
            self.fresh.insert(f, (*end, v.len() as u32));
            *end += v.len() as u64;
        }
    }

    /// Yangi kadrlar bo'lsa — eski va yangilarini bitta faylga yozadi
    /// (oqim bilan: xotirada bir vaqtda bitta kadr).
    fn save(&mut self, w: usize, h: usize) {
        if !self.fresh.is_empty() {
            self.write_all(w, h);
        }
        self.fresh.clear();
        if let Some((sp, _, _)) = self.spill.take() {
            let _ = std::fs::remove_file(sp);
        }
    }

    fn write_all(&mut self, w: usize, h: usize) {
        let n = self.table.len();
        let head_len = (16 + n * 12) as u64;
        let mut tab = vec![(0u64, 0u32); n];
        let mut off = head_len;
        for (i, t) in tab.iter_mut().enumerate() {
            let l = match self.fresh.get(&i) {
                Some(&(_, l)) => l,
                None if self.file.is_some() => self.table[i].1,
                None => 0,
            };
            if l > 0 {
                *t = (off, l);
                off += l as u64;
            }
        }
        let tmp = self.path.with_extension(format!("afc.{}.part", SEQ.fetch_add(1, Ordering::SeqCst)));
        let ok = (|| -> Option<()> {
            let mut out = std::io::BufWriter::new(std::fs::File::create(&tmp).ok()?);
            let mut head = Vec::with_capacity(head_len as usize);
            head.extend_from_slice(MAGIC);
            head.extend_from_slice(&(w as u32).to_le_bytes());
            head.extend_from_slice(&(h as u32).to_le_bytes());
            head.extend_from_slice(&(n as u32).to_le_bytes());
            for (o, l) in &tab {
                head.extend_from_slice(&o.to_le_bytes());
                head.extend_from_slice(&l.to_le_bytes());
            }
            out.write_all(&head).ok()?;
            for (i, t) in tab.iter().enumerate() {
                if t.1 == 0 {
                    continue;
                }
                let v = self.raw(i)?;
                if v.len() != t.1 as usize {
                    return None;
                }
                out.write_all(&v).ok()?;
            }
            out.flush().ok()
        })()
        .is_some();
        self.file = None;
        if !ok || std::fs::rename(&tmp, &self.path).is_err() {
            let _ = std::fs::remove_file(&tmp);
            // Eski fayl (bo'lsa) buzilmagan — qayta ochiladi.
            self.file = std::fs::File::open(&self.path).ok();
        } else {
            self.table = tab;
            self.file = std::fs::File::open(&self.path).ok();
            if let Some(dir) = self.path.parent() {
                trim_dir(dir);
            }
        }
    }
}

impl Drop for DiskFrames {
    fn drop(&mut self) {
        if let Some((sp, _, _)) = self.spill.take() {
            let _ = std::fs::remove_file(sp);
        }
    }
}

static SEQ: AtomicI64 = AtomicI64::new(0);

/// `.afc` fayllari jami [DISK_BUDGET] dan oshsa — eng eskilari o'chadi.
fn trim_dir(dir: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let entries: Vec<std::fs::DirEntry> = rd.filter_map(|e| e.ok()).collect();
    // Ilova yopilib qolganda qolgan vaqtinchalik fayllar (1 soatdan eski).
    let hour = std::time::Duration::from_secs(3600);
    for e in &entries {
        let p = e.path();
        let tmp = p.extension().map(|x| x == "spill" || x == "part").unwrap_or(false);
        let old = e
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .map(|d| d > hour)
            .unwrap_or(false);
        if tmp && old {
            let _ = std::fs::remove_file(p);
        }
    }
    let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = entries
        .into_iter()
        .filter(|e| e.path().extension().map(|x| x == "afc").unwrap_or(false))
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            Some((m.modified().ok()?, m.len(), e.path()))
        })
        .collect();
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    if total <= DISK_BUDGET {
        return;
    }
    files.sort_by_key(|f| f.0);
    for (_, len, p) in files {
        if total <= DISK_BUDGET * 3 / 4 {
            break;
        }
        if std::fs::remove_file(&p).is_ok() {
            total = total.saturating_sub(len);
        }
    }
}

impl Drop for Anim {
    fn drop(&mut self) {
        let (w, h) = (self.w, self.h);
        if let Some(d) = self.disk.as_mut() {
            d.save(w, h);
        }
        unsafe {
            match &self.kind {
                Kind::Lottie { .. } => {}
                Kind::Webm { dec, .. } => {
                    if !dec.is_null() {
                        aru_vp9_close(*dec)
                    }
                }
                Kind::Mp4 { dec, .. } => {
                    if !dec.is_null() {
                        aru_h264_close(*dec)
                    }
                }
            }
        }
    }
}

fn anims() -> &'static Mutex<HashMap<i64, Arc<Mutex<Anim>>>> {
    static M: OnceLock<Mutex<HashMap<i64, Arc<Mutex<Anim>>>>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(HashMap::new()))
}

static NEXT: AtomicI64 = AtomicI64::new(1);

/// Eng katta chizish o'lchami (xotira va vaqt uchun).
const MAX_SIDE: usize = 512;

fn open_lottie(bytes: &[u8]) -> Option<Kind> {
    // `.tgs` — gzip; oddiy JSON ham qabul qilinadi.
    let json = if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut out = Vec::new();
        flate2::read::GzDecoder::new(bytes).take(16 * 1024 * 1024).read_to_end(&mut out).ok()?;
        out
    } else {
        bytes.to_vec()
    };
    // Telegram Android (`jni/lottie.cpp`) kabi: tlottie, standart
    // cheklovlar, RGBA tartibi (Flutter premultiplied RGBA kutadi).
    let comp = tlottie::Composition::parse(&json, &tlottie::Limits::default()).ok()?;
    let frames = comp.frame_count().max(1) as usize;
    let fps = comp.frame_rate as f64;
    let r = Box::new(tlottie::CPURenderer::new(comp));
    Some(Kind::Lottie { r, frames, fps: if fps > 0.0 { fps } else { 30.0 } })
}

// ── WEBM (EBML) ─────────────────────────────────────────────────
//
// To'liq demuxer kerak emas: stiker — bitta VP9 yo'lak, 3 soniyagacha.
// Elementlar ketma-ket o'qiladi; "ichiga kiriladigan" (master)
// elementlarning o'lchami e'tiborsiz — ularning bolalari shu oqimda
// davom etadi. Shu tufayli o'lchami noma'lum klasterlar ham o'qiladi.

const ID_SEGMENT: u32 = 0x1853_8067;
const ID_CLUSTER: u32 = 0x1F43_B675;
const ID_TRACKS: u32 = 0x1654_AE6B;
const ID_TRACK_ENTRY: u32 = 0xAE;
const ID_BLOCK_GROUP: u32 = 0xA0;
const ID_BLOCK: u32 = 0xA1;
const ID_SIMPLE_BLOCK: u32 = 0xA3;
const ID_BLOCK_ADDITIONS: u32 = 0x75A1;
const ID_BLOCK_MORE: u32 = 0xA6;
const ID_BLOCK_ADDITIONAL: u32 = 0xA5;
const ID_DEFAULT_DURATION: u32 = 0x23_E383;
const ID_INFO: u32 = 0x1549_A966;
const ID_VIDEO: u32 = 0xE0;

fn read_id(b: &[u8], p: &mut usize) -> Option<u32> {
    let first = *b.get(*p)?;
    let len = first.leading_zeros() as usize + 1;
    if len > 4 || *p + len > b.len() {
        return None;
    }
    let mut v: u32 = 0;
    for i in 0..len {
        v = (v << 8) | b[*p + i] as u32;
    }
    *p += len;
    Some(v)
}

/// Uzunlik (vint). `None` ichida `u64::MAX` — "noma'lum".
fn read_size(b: &[u8], p: &mut usize) -> Option<u64> {
    let first = *b.get(*p)?;
    let len = first.leading_zeros() as usize + 1;
    if len > 8 || *p + len > b.len() {
        return None;
    }
    let mask = if len == 8 { 0 } else { 0xFFu8 >> len };
    let mut v: u64 = (first & mask) as u64;
    let mut all_ones = (first & mask) == mask;
    for i in 1..len {
        let x = b[*p + i];
        all_ones &= x == 0xFF;
        v = (v << 8) | x as u64;
    }
    *p += len;
    Some(if all_ones { u64::MAX } else { v })
}

fn uint(b: &[u8]) -> u64 {
    b.iter().fold(0u64, |a, x| (a << 8) | *x as u64)
}

/// Blok ichidan kadr baytlari (yo'lak raqami, vaqt, bayroqlar tashlanadi).
fn block_payload(b: &[u8]) -> Option<Vec<u8>> {
    let mut p = 0;
    read_size(b, &mut p)?; // yo'lak raqami (vint)
    p += 3; // vaqt (2) + bayroqlar (1); stikerlarda "lacing" yo'q
    b.get(p..).map(|s| s.to_vec())
}

fn parse_webm(b: &[u8]) -> Option<(Vec<WebmFrame>, f64)> {
    let mut p = 0;
    let mut frames: Vec<WebmFrame> = Vec::new();
    let mut default_ns: u64 = 0;
    while p < b.len() {
        let id = read_id(b, &mut p)?;
        let size = read_size(b, &mut p)?;
        match id {
            // Ichiga kiriladi.
            ID_SEGMENT | ID_CLUSTER | ID_TRACKS | ID_TRACK_ENTRY | ID_BLOCK_GROUP | ID_BLOCK_ADDITIONS
            | ID_BLOCK_MORE | ID_INFO | ID_VIDEO => continue,
            _ => {}
        }
        if size == u64::MAX {
            return None;
        }
        let end = p.checked_add(size as usize)?;
        if end > b.len() {
            break;
        }
        let data = &b[p..end];
        match id {
            ID_SIMPLE_BLOCK | ID_BLOCK => {
                if let Some(color) = block_payload(data) {
                    frames.push(WebmFrame { color, alpha: None });
                }
            }
            ID_BLOCK_ADDITIONAL => {
                if let Some(last) = frames.last_mut() {
                    last.alpha = Some(data.to_vec());
                }
            }
            ID_DEFAULT_DURATION => default_ns = uint(data),
            _ => {}
        }
        p = end;
    }
    if frames.is_empty() {
        return None;
    }
    let fps = if default_ns > 0 { 1e9 / default_ns as f64 } else { 30.0 };
    Some((frames, fps.clamp(1.0, 60.0)))
}

fn open_webm(bytes: &[u8]) -> Option<Kind> {
    let (frames, fps) = parse_webm(bytes)?;
    let dec = unsafe { aru_vp9_open() };
    if dec.is_null() {
        return None;
    }
    Some(Kind::Webm { dec, frames, fps, next: 0 })
}

// ── MP4 (GIF) ───────────────────────────────────────────────────
//
// To'liq demuxer kerak emas: Telegram GIF — bitta H.264 video yo'lak.
// `moov` ichidan video yo'lakning namunalar jadvali (`stsz`, `stco`/
// `co64`, `stsc`), vaqt (`mdhd`, `stts`) va dekoder sozlamasi (`avcC`)
// o'qiladi; namunalar fayldan to'g'ridan-to'g'ri olinadi.

#[derive(Default)]
struct Mp4Info {
    w: u32,
    h: u32,
    extra: Vec<u8>,
    /// Har namuna: (fayldagi joyi, uzunligi) — dekodlash tartibida.
    samples: Vec<(usize, usize)>,
    timescale: u32,
    duration: u64,
}

fn be32(b: &[u8], p: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(p..p + 4)?.try_into().ok()?))
}
fn be64(b: &[u8], p: usize) -> Option<u64> {
    Some(u64::from_be_bytes(b.get(p..p + 8)?.try_into().ok()?))
}

/// [b] ichidagi qutilar: (turi, mazmuni).
fn boxes(b: &[u8]) -> Vec<([u8; 4], &[u8])> {
    let mut out = Vec::new();
    let mut p = 0usize;
    while p + 8 <= b.len() {
        let Some(sz) = be32(b, p) else { break };
        let typ: [u8; 4] = b[p + 4..p + 8].try_into().unwrap_or([0; 4]);
        let (head, size) = match sz {
            1 => match be64(b, p + 8) {
                Some(v) => (16usize, v as usize),
                None => break,
            },
            0 => (8, b.len() - p),
            v => (8, v as usize),
        };
        if size < head || p + size > b.len() {
            break;
        }
        out.push((typ, &b[p + head..p + size]));
        p += size;
    }
    out
}

fn child<'a>(b: &'a [u8], t: &[u8; 4]) -> Option<&'a [u8]> {
    boxes(b).into_iter().find(|(k, _)| k == t).map(|(_, v)| v)
}

fn parse_mp4(b: &[u8]) -> Option<Mp4Info> {
    let moov = child(b, b"moov")?;
    for (k, trak) in boxes(moov) {
        if &k != b"trak" {
            continue;
        }
        let mdia = child(trak, b"mdia")?;
        let hdlr = child(mdia, b"hdlr")?;
        if hdlr.get(8..12)? != b"vide" {
            continue;
        }
        let mut info = Mp4Info::default();
        let mdhd = child(mdia, b"mdhd")?;
        if mdhd.first() == Some(&1) {
            info.timescale = be32(mdhd, 20)?;
            info.duration = be64(mdhd, 24)?;
        } else {
            info.timescale = be32(mdhd, 12)?;
            info.duration = be32(mdhd, 16)? as u64;
        }
        let stbl = child(child(mdia, b"minf")?, b"stbl")?;
        // Dekoder sozlamasi va o'lcham (`avc1` -> `avcC`).
        let stsd = child(stbl, b"stsd")?;
        let entry = boxes(stsd.get(8..)?).into_iter().next()?;
        if &entry.0 != b"avc1" && &entry.0 != b"avc3" {
            return None;
        }
        let e = entry.1;
        info.w = u16::from_be_bytes(e.get(24..26)?.try_into().ok()?) as u32;
        info.h = u16::from_be_bytes(e.get(26..28)?.try_into().ok()?) as u32;
        info.extra = child(e.get(78..)?, b"avcC")?.to_vec();
        // Namuna o'lchamlari.
        let stsz = child(stbl, b"stsz")?;
        let fixed = be32(stsz, 4)? as usize;
        let count = be32(stsz, 8)? as usize;
        if count == 0 || count > 20_000 {
            return None;
        }
        let sizes: Vec<usize> = if fixed != 0 {
            vec![fixed; count]
        } else {
            (0..count).map(|i| be32(stsz, 12 + i * 4).map(|v| v as usize)).collect::<Option<_>>()?
        };
        // Bo'laklar joyi.
        let chunks: Vec<usize> = if let Some(stco) = child(stbl, b"stco") {
            let n = be32(stco, 4)? as usize;
            (0..n).map(|i| be32(stco, 8 + i * 4).map(|v| v as usize)).collect::<Option<_>>()?
        } else {
            let co = child(stbl, b"co64")?;
            let n = be32(co, 4)? as usize;
            (0..n).map(|i| be64(co, 8 + i * 8).map(|v| v as usize)).collect::<Option<_>>()?
        };
        // Bo'lakdagi namunalar soni (`stsc`: birinchi bo'lak, soni).
        let stsc = child(stbl, b"stsc")?;
        let n = be32(stsc, 4)? as usize;
        let runs: Vec<(usize, usize)> = (0..n)
            .map(|i| Some((be32(stsc, 8 + i * 12)? as usize, be32(stsc, 12 + i * 12)? as usize)))
            .collect::<Option<_>>()?;
        let mut si = 0usize;
        for (ci, &off) in chunks.iter().enumerate() {
            let c = ci + 1;
            let per = runs.iter().rev().find(|(first, _)| *first <= c).map(|r| r.1).unwrap_or(1);
            let mut o = off;
            for _ in 0..per {
                if si >= count {
                    break;
                }
                info.samples.push((o, sizes[si]));
                o += sizes[si];
                si += 1;
            }
        }
        if info.samples.len() != count {
            return None;
        }
        // `mdhd` davomiyligi 0 bo'lsa — `stts` dan.
        if info.duration == 0 {
            if let Some(stts) = child(stbl, b"stts") {
                let n = be32(stts, 4)? as usize;
                info.duration = (0..n)
                    .filter_map(|i| Some(be32(stts, 8 + i * 8)? as u64 * be32(stts, 12 + i * 8)? as u64))
                    .sum();
            }
        }
        return Some(info);
    }
    None
}

fn open_mp4(bytes: Vec<u8>) -> Option<Kind> {
    let mp4 = parse_mp4(&bytes)?;
    let secs = if mp4.timescale > 0 { mp4.duration as f64 / mp4.timescale as f64 } else { 0.0 };
    let fps = if secs > 0.0 { (mp4.samples.len() as f64 / secs).clamp(1.0, 60.0) } else { 25.0 };
    let dec = unsafe { aru_h264_open(mp4.extra.as_ptr(), mp4.extra.len() as i32) };
    if dec.is_null() {
        return None;
    }
    Some(Kind::Mp4 { dec, data: bytes, mp4, fps, sent: 0, out: 0, eof: false })
}

/// Video (GIF) o'lchami — `{"w":..,"h":..}` (yoki xato). Dart GIF
/// katagining nisbatini ochishdan oldin shundan biladi.
#[no_mangle]
pub extern "C" fn rust_anim_probe(path_ptr: *const c_char) -> *mut c_char {
    let path = unsafe { cstr_to_str(path_ptr) }.unwrap_or("");
    let r = std::panic::catch_unwind(|| {
        let bytes = std::fs::read(path).ok()?;
        let m = parse_mp4(&bytes)?;
        Some((m.w, m.h))
    })
    .ok()
    .flatten();
    let s = match r {
        Some((w, h)) => format!("{{\"w\":{w},\"h\":{h}}}"),
        None => "{\"error\":\"mp4 emas\"}".to_string(),
    };
    crate::ffi_utils::string_to_cptr(s)
}

/// Faylni ochadi: `.tgs`/Lottie yoki `.webm`. Qaytadi: tutqich (0 — xato).
#[no_mangle]
pub extern "C" fn rust_anim_open(path_ptr: *const c_char, w: i32, h: i32) -> i64 {
    std::panic::catch_unwind(|| open_anim(path_ptr, w, h)).unwrap_or(0)
}

fn open_anim(path_ptr: *const c_char, w: i32, h: i32) -> i64 {
    let Some(path) = (unsafe { cstr_to_str(path_ptr) }) else { return 0 };
    open_path(path, w, h)
}

// ── Ichki API (`anim_player.rs` — kadrlar to'g'ridan-to'g'ri ekranga) ──

/// Faylni ochadi (tutqich, 0 — xato).
pub(crate) fn open_path(path: &str, w: i32, h: i32) -> i64 {
    let Ok(bytes) = std::fs::read(path) else { return 0 };
    let kind = if bytes.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        open_webm(&bytes)
    } else if bytes.len() > 8 && &bytes[4..8] == b"ftyp" {
        open_mp4(bytes)
    } else {
        open_lottie(&bytes)
    };
    let Some(kind) = kind else { return 0 };
    let w = (w.max(1) as usize).min(MAX_SIDE);
    let h = (h.max(1) as usize).min(MAX_SIDE);
    let frames = match &kind {
        Kind::Lottie { frames, .. } => *frames,
        Kind::Webm { frames, .. } => frames.len(),
        Kind::Mp4 { mp4, .. } => mp4.samples.len(),
    };
    let disk = if frames > 1 {
        Some(DiskFrames::open(PathBuf::from(format!("{path}.{w}x{h}.afc")), frames, w, h))
    } else {
        None
    };
    let id = NEXT.fetch_add(1, Ordering::SeqCst);
    if let Ok(mut m) = anims().lock() {
        m.insert(id, Arc::new(Mutex::new(Anim { kind, w, h, disk })));
    }
    id
}

fn get(id: i64) -> Option<Arc<Mutex<Anim>>> {
    anims().lock().ok()?.get(&id).cloned()
}

/// Kadrlar soni.
#[no_mangle]
pub extern "C" fn rust_anim_frames(id: i64) -> i32 {
    let Some(a) = get(id) else { return 0 };
    let Ok(a) = a.lock() else { return 0 };
    match &a.kind {
        Kind::Lottie { frames, .. } => *frames as i32,
        Kind::Webm { frames, .. } => frames.len() as i32,
        Kind::Mp4 { mp4, .. } => mp4.samples.len() as i32,
    }
}

/// Soniyasiga kadr.
#[no_mangle]
pub extern "C" fn rust_anim_fps(id: i64) -> f64 {
    let Some(a) = get(id) else { return 0.0 };
    let Ok(a) = a.lock() else { return 0.0 };
    match &a.kind {
        Kind::Lottie { fps, .. } | Kind::Webm { fps, .. } | Kind::Mp4 { fps, .. } => *fps,
    }
}

/// [frame] ni [out] ga (w*h*4, premultiplied RGBA) chizadi.
/// Qaytadi: 1 — tayyor, 0 — bu kadr ko'rinmaydi (oldingisi qoladi),
/// -1 — xato.
#[no_mangle]
pub extern "C" fn rust_anim_render(id: i64, frame: i32, out: *mut u8) -> i32 {
    // Buzuq stiker faylida chizgich `panic` qilsa — ilova yopilmaydi,
    // shu stiker birinchi kadrida qoladi.
    std::panic::catch_unwind(|| render_frame(id, frame, out)).unwrap_or(-1)
}

fn render_frame(id: i64, frame: i32, out: *mut u8) -> i32 {
    let Some(a) = get(id) else { return -1 };
    let Ok(mut a) = a.lock() else { return -1 };
    let (w, h) = (a.w, a.h);
    if out.is_null() {
        return -1;
    }
    let a = &mut *a;
    let buf = unsafe { std::slice::from_raw_parts_mut(out, w * h * 4) };
    // Kadr diskda bo'lsa — chizilmaydi, ochiladi.
    let fi = frame.max(0) as usize;
    if let Some(d) = a.disk.as_mut() {
        if d.read(fi, buf) {
            return 1;
        }
    }
    let r = draw(&mut a.kind, frame, out, w, h);
    if r == 1 {
        if let Some(d) = a.disk.as_mut() {
            d.put(fi, buf);
        }
    }
    r
}

fn draw(kind: &mut Kind, frame: i32, out: *mut u8, w: usize, h: usize) -> i32 {
    match kind {
        Kind::Mp4 { dec, data, mp4, sent, out: done, eof, .. } => {
            let n = mp4.samples.len();
            if n == 0 {
                return -1;
            }
            let want = (frame.max(0) as usize).min(n - 1);
            // Orqaga qaytilsa — dekoder boshidan (H.264 ketma-ket).
            if want < *done || dec.is_null() {
                unsafe {
                    if !dec.is_null() {
                        aru_h264_close(*dec);
                    }
                    *dec = aru_h264_open(mp4.extra.as_ptr(), mp4.extra.len() as i32);
                }
                if dec.is_null() {
                    return -1;
                }
                *sent = 0;
                *done = 0;
                *eof = false;
            }
            // Cheksiz aylanib qolmasin.
            for _ in 0..(n * 4 + 16) {
                let target = if *done == want { out } else { std::ptr::null_mut() };
                let r = unsafe { aru_h264_recv(*dec, target, w as i32, h as i32) };
                match r {
                    1 => {
                        *done += 1;
                        if *done == want + 1 {
                            return 1;
                        }
                    }
                    0 => {
                        if *sent < n {
                            let (o, l) = mp4.samples[*sent];
                            let Some(slice) = data.get(o..o + l) else { return -1 };
                            let s = unsafe { aru_h264_send(*dec, slice.as_ptr(), l as i32) };
                            if s < 0 {
                                return -1;
                            }
                            if s == 0 {
                                *sent += 1;
                            }
                        } else if !*eof {
                            unsafe { aru_h264_send(*dec, std::ptr::null(), 0) };
                            *eof = true;
                        } else {
                            return -1;
                        }
                    }
                    _ => return -1,
                }
            }
            -1
        }
        Kind::Lottie { r, frames, .. } => {
            let f = (frame.max(0) as usize).min(*frames - 1);
            let px = unsafe { std::slice::from_raw_parts_mut(out as *mut u32, w * h) };
            // Premultiplied RGBA ([R,G,B,A] baytlar), avval tozalanadi.
            match r.render(f as f32, px, w as u32, h as u32, tlottie::RenderOptions::default()) {
                Ok(()) => 1,
                Err(_) => -1,
            }
        }
        Kind::Webm { dec, frames, next, .. } => {
            let want = (frame.max(0) as usize).min(frames.len() - 1);
            // VP9 — ketma-ket: orqaga qaytilsa dekoder boshidan.
            if want < *next {
                unsafe {
                    aru_vp9_close(*dec);
                    *dec = aru_vp9_open();
                }
                if dec.is_null() {
                    return -1;
                }
                *next = 0;
            }
            let mut r = 0;
            while *next <= want {
                let f = &frames[*next];
                let (ap, al) = match &f.alpha {
                    Some(v) => (v.as_ptr(), v.len()),
                    None => (std::ptr::null(), 0),
                };
                r = unsafe {
                    aru_vp9_decode(*dec, f.color.as_ptr(), f.color.len(), ap, al, out, w as i32, h as i32)
                };
                *next += 1;
                if r < 0 {
                    return -1;
                }
            }
            r
        }
    }
}

/// Yopadi (xotira bo'shaydi).
#[no_mangle]
pub extern "C" fn rust_anim_close(id: i64) {
    // Tutqich umumiy qulfdan TASHQARIDA yopiladi: kadrlarni diskka
    // yozish boshqa stikerlarning chizilishini to'xtatib turmasin.
    let a = anims().lock().ok().and_then(|mut m| m.remove(&id));
    drop(a);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ebml_vint() {
        let b = [0x81u8];
        let mut p = 0;
        assert_eq!(read_size(&b, &mut p), Some(1));
        let b = [0x01u8, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        let mut p = 0;
        assert_eq!(read_size(&b, &mut p), Some(u64::MAX));
        let b = [0x1A, 0x45, 0xDF, 0xA3];
        let mut p = 0;
        assert_eq!(read_id(&b, &mut p), Some(0x1A45_DFA3));
    }

    #[test]
    fn lottie_renders() {
        // Eng oddiy Lottie: 10 kadr, 20x20, bitta qizil to'rtburchak.
        let json = r##"{"v":"5.5.2","fr":30,"ip":0,"op":10,"w":20,"h":20,"layers":[
          {"ty":1,"sc":"#ff0000","sw":20,"sh":20,"ip":0,"op":10,"st":0,
           "ks":{"o":{"a":0,"k":100},"r":{"a":0,"k":0},"p":{"a":0,"k":[10,10,0]},
                 "a":{"a":0,"k":[10,10,0]},"s":{"a":0,"k":[100,100,100]}}}]}"##;
        let kind = open_lottie(json.as_bytes()).expect("ochilmadi");
        let Kind::Lottie { frames, fps, .. } = &kind else { panic!("lottie emas") };
        // Harakatsiz kompozitsiyani tlottie bitta kadr deb sanaydi
        // (animatsiyalida — op - ip).
        assert_eq!(*frames, 1);
        assert!((fps - 30.0).abs() < 0.01);
        let id = NEXT.fetch_add(1, Ordering::SeqCst);
        anims().lock().unwrap().insert(id, Arc::new(Mutex::new(Anim { kind, w: 8, h: 8, disk: None })));
        let mut buf = vec![0u8; 8 * 8 * 4];
        assert_eq!(rust_anim_render(id, 3, buf.as_mut_ptr()), 1);
        // Markazdagi nuqta — to'liq qizil (RGBA).
        let c = &buf[(4 * 8 + 4) * 4..(4 * 8 + 4) * 4 + 4];
        assert_eq!(c, &[255, 0, 0, 255]);
        rust_anim_close(id);
    }

    /// FFmpeg Telegram video stikerini qanday joylasa, xuddi shunday
    /// fayl: 64x64, 2 kadr, chap yarmi qizil va ko'rinadigan, o'ng
    /// yarmi to'liq shaffof (rang `Block` da, alpha `BlockAdditional`
    /// da). libvpx'ning o'z `vpxenc` i bilan yasalgan.
    #[test]
    fn webm_alpha_sticker() {
        let bytes = include_bytes!("testdata/alpha_sticker.webm");
        let (frames, fps) = parse_webm(bytes).expect("webm o'qilmadi");
        assert_eq!(frames.len(), 2);
        assert!(frames.iter().all(|f| f.alpha.is_some()));
        assert!((fps - 30.0).abs() < 0.1);
        let kind = open_webm(bytes).expect("dekoder");
        let id = NEXT.fetch_add(1, Ordering::SeqCst);
        anims().lock().unwrap().insert(id, Arc::new(Mutex::new(Anim { kind, w: 32, h: 32, disk: None })));
        let mut buf = vec![0u8; 32 * 32 * 4];
        assert_eq!(rust_anim_render(id, 1, buf.as_mut_ptr()), 1);
        let px = |x: usize, y: usize| &buf[(y * 32 + x) * 4..(y * 32 + x) * 4 + 4];
        // Chapda — qizil, ko'rinadi.
        let l = px(4, 16);
        assert!(l[0] > 200 && l[1] < 60 && l[2] < 60 && l[3] == 255, "chap: {l:?}");
        // O'ngda — to'liq shaffof (premultiplied: rang ham 0).
        assert_eq!(px(28, 16), &[0, 0, 0, 0]);
        // Orqaga qaytish (qayta boshlash) ham ishlaydi.
        assert_eq!(rust_anim_render(id, 0, buf.as_mut_ptr()), 1);
        rust_anim_close(id);
    }

    /// Telegram GIF kabi: H.264 High profil (B-kadrlar bilan) MP4,
    /// 96x64, 10 kadr — ffmpeg dekoderi bilan ochiladi.
    #[test]
    fn mp4_gif_decodes() {
        let bytes = include_bytes!("testdata/gif_h264.mp4").to_vec();
        let m = parse_mp4(&bytes).expect("mp4 o'qilmadi");
        assert_eq!((m.w, m.h), (96, 64));
        assert_eq!(m.samples.len(), 10);
        let kind = open_mp4(bytes).expect("dekoder ochilmadi");
        let Kind::Mp4 { fps, .. } = &kind else { panic!() };
        assert!((fps - 25.0).abs() < 0.5, "fps {fps}");
        let id = NEXT.fetch_add(1, Ordering::SeqCst);
        anims().lock().unwrap().insert(id, Arc::new(Mutex::new(Anim { kind, w: 48, h: 48, disk: None })));
        let mut buf = vec![0u8; 48 * 48 * 4];
        for f in [0, 3, 9, 2] {
            assert_eq!(rust_anim_render(id, f, buf.as_mut_ptr()), 1, "kadr {f}");
            // To'liq shaffof emas va qora emas (testsrc — rangli).
            assert!(buf.chunks(4).all(|p| p[3] == 255));
            assert!(buf.chunks(4).any(|p| p[0] > 40 || p[1] > 40 || p[2] > 40));
        }
        rust_anim_close(id);
    }

    #[test]
    fn disk_frames_roundtrip() {
        let dir = std::env::temp_dir().join(format!("afc_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("s.4x4.afc");
        let _ = std::fs::remove_file(&path);
        let a: Vec<u8> = (0..64).map(|i| i as u8).collect();
        let b: Vec<u8> = (0..64).map(|i| 255 - i as u8).collect();
        let mut d = DiskFrames::open(path.clone(), 3, 4, 4);
        let mut out = vec![0u8; 64];
        assert!(!d.read(0, &mut out));
        d.put(0, &a);
        d.put(2, &b);
        assert!(d.read(2, &mut out) && out == b);
        d.save(4, 4);
        // Qayta ochilganda diskdan o'qiladi.
        let mut d = DiskFrames::open(path.clone(), 3, 4, 4);
        assert!(d.read(0, &mut out) && out == a);
        assert!(d.read(2, &mut out) && out == b);
        assert!(!d.read(1, &mut out));
        // Yangi kadr qo'shilsa eskilari saqlanib qoladi.
        d.put(1, &b);
        d.save(4, 4);
        let mut d = DiskFrames::open(path.clone(), 3, 4, 4);
        assert!(d.read(0, &mut out) && out == a);
        assert!(d.read(1, &mut out) && out == b);
        // O'lchami boshqa — eski fayl ishlatilmaydi.
        let mut d2 = DiskFrames::open(path, 3, 8, 8);
        assert!(!d2.read(0, &mut vec![0u8; 256]));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vp9_decoder_opens() {
        let d = unsafe { aru_vp9_open() };
        assert!(!d.is_null());
        unsafe { aru_vp9_close(d) };
    }
}

/// (kadrlar soni, soniyasiga kadr).
pub(crate) fn info(id: i64) -> (usize, f64) {
    (rust_anim_frames(id).max(0) as usize, rust_anim_fps(id))
}

/// Kadrni [out] ga (w*h*4) chizadi: 1 — tayyor.
pub(crate) fn render_into(id: i64, frame: usize, out: &mut [u8]) -> i32 {
    rust_anim_render(id, frame as i32, out.as_mut_ptr())
}

pub(crate) fn close(id: i64) {
    rust_anim_close(id)
}
