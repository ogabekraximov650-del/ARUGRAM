//! UZ: tgstore bo'lak fayli formati (ikkilik, mundarijali, MessagePack) + AES-256-GCM. Sof mantiq: tarmoq ham, DO ham yo'q.
//! EN: tgstore chunk file format (binary, indexed, MessagePack) + AES-256-GCM. Pure logic: no network, no DO.
//! RU: format fayla chanka tgstore (binarnyy, s oglavleniem, MessagePack) + AES-256-GCM. Chistaya logika.
//!
//! UZ: Fayl tuzilishi / EN: layout / RU: struktura:
//! ```text
//! "TGK1" | count:u32 | count × { klen:u16 | key | off:u32 | len:u32 } | data...
//! ```
//! UZ: Mundarija kalit bo'yicha TARTIBLANGAN (baytlar bo'yicha). Bitta yozuvni topish = mundarijada binar qidiruv + shu yozuvning
//!     baytlarini ochish; qolgan yozuvlar ochilmaydi. Tahrirda o'zgarmagan yozuvlar xom baytlari bilan ko'chiriladi (qayta kodlanmaydi).
//! EN: The index is SORTED by key (bytewise). Finding one record = binary search in the index + decoding only that record's bytes.
//!     On edit, unchanged records are copied as raw bytes (not re-encoded).
//! RU: Oglavlenie otsortirovano po klyuchu. Poisk odnoy zapisi = dvoichnyy poisk + dekodirovanie tolko ee bayt. Pri pravke
//!     neizmenennye zapisi kopiruyutsya syrymi baytami.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use serde_json::Value;
use std::collections::BTreeMap;

const MAGIC: &[u8; 4] = b"TGK1";
/// UZ: bo'lak chegarasi (Bot API `getFile` 20 MB gacha beradi; ~1 MB tez ochiladi). EN: chunk size target. RU: tselevoy razmer chanka.
pub const TARGET_CHUNK: usize = 1000 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    Put(String, Value),
    Del(String),
}

#[derive(Debug, Clone)]
struct Ent {
    key: (usize, usize), // buf ichidagi kalit oralig'i / key range in buf / diapazon klyucha
    val: (usize, usize), // buf ichidagi qiymat oralig'i / value range / diapazon znacheniya
}

/// UZ: o'qish uchun ko'rinish: faqat mundarija tahlil qilinadi, qiymatlar so'ralganda ochiladi.
/// EN: read view: only the index is parsed, values are decoded on demand. RU: tolko oglavlenie razbiraetsya, znacheniya po trebovaniyu.
pub struct View {
    buf: Vec<u8>,
    idx: Vec<Ent>,
}

fn rd_u16(b: &[u8], p: usize) -> Option<u16> { b.get(p..p + 2).map(|x| u16::from_le_bytes([x[0], x[1]])) }
fn rd_u32(b: &[u8], p: usize) -> Option<u32> { b.get(p..p + 4).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]])) }

fn enc_val(v: &Value) -> Result<Vec<u8>, String> {
    rmp_serde::to_vec(v).map_err(|e| format!("msgpack: {e}"))
}

/// UZ: xom (allaqachon kodlangan) yozuvlardan fayl yig'adi; kalitlar tartiblangan bo'lishi kerak (BTreeMap kafolatlaydi).
/// EN: builds a file from raw (already encoded) records; keys are sorted by BTreeMap. RU: sobiraet fayl iz syrykh zapisey.
fn build(rows: &BTreeMap<Vec<u8>, Vec<u8>>) -> Result<Vec<u8>, String> {
    let mut head = Vec::with_capacity(8 + rows.len() * 20);
    head.extend_from_slice(MAGIC);
    head.extend_from_slice(&(rows.len() as u32).to_le_bytes());
    let mut data: Vec<u8> = Vec::new();
    for (k, v) in rows {
        if k.is_empty() || k.len() > u16::MAX as usize { return Err("bad key length".into()); }
        head.extend_from_slice(&(k.len() as u16).to_le_bytes());
        head.extend_from_slice(k);
        head.extend_from_slice(&(data.len() as u32).to_le_bytes());
        head.extend_from_slice(&(v.len() as u32).to_le_bytes());
        data.extend_from_slice(v);
    }
    head.extend_from_slice(&data);
    Ok(head)
}

/// UZ: qatorlardan fayl (kalitlar takrorlanmasin, tartib avtomatik). EN: file from rows. RU: fayl iz strok.
pub fn encode(rows: &[(String, Value)]) -> Result<Vec<u8>, String> {
    let mut m = BTreeMap::new();
    for (k, v) in rows { m.insert(k.as_bytes().to_vec(), enc_val(v)?); }
    build(&m)
}

impl View {
    pub fn parse(buf: Vec<u8>) -> Result<View, String> {
        if buf.get(0..4) != Some(MAGIC) { return Err("bad magic".into()); }
        let n = rd_u32(&buf, 4).ok_or("short")? as usize;
        let mut p = 8usize;
        let mut raw: Vec<((usize, usize), u32, u32)> = Vec::with_capacity(n);
        for _ in 0..n {
            let kl = rd_u16(&buf, p).ok_or("short index")? as usize;
            let ks = p + 2;
            let off = rd_u32(&buf, ks + kl).ok_or("short index")?;
            let len = rd_u32(&buf, ks + kl + 4).ok_or("short index")?;
            raw.push(((ks, ks + kl), off, len));
            p = ks + kl + 8;
        }
        let data0 = p;
        let mut idx = Vec::with_capacity(n);
        for (key, off, len) in raw {
            let s = data0 + off as usize;
            let e = s + len as usize;
            if e > buf.len() { return Err("record out of range".into()); }
            idx.push(Ent { key, val: (s, e) });
        }
        for w in idx.windows(2) {
            if buf[w[0].key.0..w[0].key.1] >= buf[w[1].key.0..w[1].key.1] { return Err("index not sorted".into()); }
        }
        Ok(View { buf, idx })
    }

    pub fn len(&self) -> usize { self.idx.len() }
    pub fn is_empty(&self) -> bool { self.idx.is_empty() }
    pub fn size(&self) -> usize { self.buf.len() }
    pub fn into_bytes(self) -> Vec<u8> { self.buf }

    fn key_of(&self, i: usize) -> &[u8] { &self.buf[self.idx[i].key.0..self.idx[i].key.1] }
    fn find(&self, key: &[u8]) -> Result<usize, usize> {
        self.idx.binary_search_by(|e| self.buf[e.key.0..e.key.1].cmp(key))
    }
    pub fn first_key(&self) -> Option<String> { (!self.idx.is_empty()).then(|| String::from_utf8_lossy(self.key_of(0)).into_owned()) }
    pub fn last_key(&self) -> Option<String> { (!self.idx.is_empty()).then(|| String::from_utf8_lossy(self.key_of(self.idx.len() - 1)).into_owned()) }
    pub fn contains(&self, key: &str) -> bool { self.find(key.as_bytes()).is_ok() }

    fn val_at(&self, i: usize) -> Result<Value, String> {
        let (s, e) = self.idx[i].val;
        rmp_serde::from_slice(&self.buf[s..e]).map_err(|e| format!("msgpack: {e}"))
    }

    /// UZ: bitta yozuv — faqat shu yozuv ochiladi. EN: one record, only it is decoded. RU: odna zapis.
    pub fn get(&self, key: &str) -> Result<Option<Value>, String> {
        match self.find(key.as_bytes()) { Ok(i) => self.val_at(i).map(Some), Err(_) => Ok(None) }
    }

    pub fn keys(&self) -> Vec<String> { (0..self.idx.len()).map(|i| String::from_utf8_lossy(self.key_of(i)).into_owned()).collect() }

    /// UZ: [lo, hi) oralig'idagi yozuvlar (tartib bo'yicha; `hi=None` — oxirigacha), `limit` bilan; `rev` — teskari tartib.
    /// EN: records in [lo, hi) in key order (`rev` = descending). RU: zapisi v diapazone [lo, hi).
    pub fn range(&self, lo: &str, hi: Option<&str>, limit: usize, rev: bool) -> Result<Vec<(String, Value)>, String> {
        let a = self.find(lo.as_bytes()).unwrap_or_else(|i| i);
        let b = match hi { Some(h) => self.find(h.as_bytes()).unwrap_or_else(|i| i), None => self.idx.len() };
        let mut out = Vec::new();
        let ids: Vec<usize> = if rev { (a..b).rev().collect() } else { (a..b).collect() };
        for i in ids.into_iter().take(limit) {
            out.push((String::from_utf8_lossy(self.key_of(i)).into_owned(), self.val_at(i)?));
        }
        Ok(out)
    }

    /// UZ: amallarni qo'llab YANGI bo'laklar qaytaradi. Chegaradan oshsa teng ikkiga bo'linadi (har biri `(fayl baytlari, min_kalit, max_kalit)`).
    ///     Bo'sh qolsa — bo'sh vektor (bo'lak ro'yxatdan olib tashlanadi). O'zgarmagan yozuvlar xom ko'chiriladi.
    /// EN: applies ops and returns NEW chunks; splits into two halves above the limit; empty result = drop the chunk.
    /// RU: primenyaet operatsii i vozvrashchaet NOVYE chanki; delit popolam pri prevyshenii; pustoy — chank udalyaetsya.
    pub fn apply(&self, ops: &[Op], max: usize) -> Result<Vec<(Vec<u8>, String, String)>, String> {
        let mut m: BTreeMap<Vec<u8>, Vec<u8>> = BTreeMap::new();
        for (i, e) in self.idx.iter().enumerate() {
            m.insert(self.key_of(i).to_vec(), self.buf[e.val.0..e.val.1].to_vec());
        }
        for op in ops {
            match op {
                Op::Put(k, v) => { if k.is_empty() { return Err("bad key".into()); } m.insert(k.as_bytes().to_vec(), enc_val(v)?); }
                Op::Del(k) => { m.remove(k.as_bytes()); }
            }
        }
        split_build(m, max)
    }
}

fn est(m: &BTreeMap<Vec<u8>, Vec<u8>>) -> usize { 8 + m.iter().map(|(k, v)| 10 + k.len() + v.len()).sum::<usize>() }

fn split_build(m: BTreeMap<Vec<u8>, Vec<u8>>, max: usize) -> Result<Vec<(Vec<u8>, String, String)>, String> {
    if m.is_empty() { return Ok(vec![]); }
    if est(&m) <= max || m.len() == 1 {
        let (lo, hi) = (String::from_utf8_lossy(m.keys().next().unwrap()).into_owned(), String::from_utf8_lossy(m.keys().next_back().unwrap()).into_owned());
        return Ok(vec![(build(&m)?, lo, hi)]);
    }
    // UZ: baytlar bo'yicha o'rtadan bo'lamiz / EN: split at the byte midpoint / RU: delim po seredine bayt
    let total = est(&m);
    let mut acc = 0usize;
    let mut cut: Option<Vec<u8>> = None;
    for (k, v) in &m {
        acc += 10 + k.len() + v.len();
        if acc >= total / 2 { cut = Some(k.clone()); break; }
    }
    let cut = cut.unwrap();
    let mut left = m;
    let mut right = left.split_off(&cut);
    // cut kalitining o'zi chapga kiradi / the cut key itself goes left / sam klyuch idet vlevo
    if let Some((k, v)) = right.pop_first() { left.insert(k, v); }
    if right.is_empty() { return split_build(left, usize::MAX); }
    let mut out = split_build(left, max)?;
    out.extend(split_build(right, max)?);
    Ok(out)
}

// ── Shifrlash: har BO'LAK VERSIYASIGA yangi tasodifiy kalit (registrida saqlanadi) => nonce qat'iy 0 xavfsiz.
// ── Encryption: a fresh random key per chunk VERSION (kept in the registry) => a fixed zero nonce is safe.
// ── Shifrovanie: novyy sluchaynyy klyuch na kazhduyu VERSIYU chanka => nulevoy nonce bezopasen.
const NONCE: [u8; 12] = [0u8; 12];

pub fn seal(key: &[u8; 32], plain: &[u8]) -> Result<Vec<u8>, String> {
    Aes256Gcm::new(key.into()).encrypt(Nonce::from_slice(&NONCE), plain).map_err(|_| "seal failed".to_string())
}

pub fn open(key: &[u8; 32], sealed: &[u8]) -> Result<Vec<u8>, String> {
    Aes256Gcm::new(key.into()).decrypt(Nonce::from_slice(&NONCE), sealed).map_err(|_| "open failed (wrong key or corrupt file)".to_string())
}

/// UZ: bo'lak fayl nomi / EN: chunk file name / RU: imya fayla chanka — `DBP_<jadval>_<no>_v<ver>`
pub fn chunk_name(table: &str, no: u64, ver: u64) -> String { format!("DBP_{table}_{no}_v{ver}") }

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rows(n: usize, pad: usize) -> Vec<(String, Value)> {
        (0..n).map(|i| (format!("{i:08}"), json!({"i": i, "name": format!("Anime {i} ўзбек"), "pad": "x".repeat(pad)}))).collect()
    }

    #[test]
    fn roundtrip_get_and_unicode() {
        let v = View::parse(encode(&rows(100, 5)).unwrap()).unwrap();
        assert_eq!(v.len(), 100);
        assert_eq!(v.get("00000042").unwrap().unwrap()["name"], "Anime 42 ўзбек");
        assert!(v.get("99999999").unwrap().is_none());
        assert!(v.contains("00000000") && !v.contains("nope"));
        assert_eq!(v.first_key().unwrap(), "00000000");
        assert_eq!(v.last_key().unwrap(), "00000099");
    }

    #[test]
    fn unsorted_input_is_sorted_and_dedup() {
        let b = encode(&[("b".into(), json!(2)), ("a".into(), json!(1)), ("b".into(), json!(3))]).unwrap();
        let v = View::parse(b).unwrap();
        assert_eq!(v.keys(), vec!["a", "b"]);
        assert_eq!(v.get("b").unwrap().unwrap(), json!(3));
    }

    #[test]
    fn range_and_reverse_and_limit() {
        let v = View::parse(encode(&rows(50, 0)).unwrap()).unwrap();
        let r = v.range("00000010", Some("00000015"), 100, false).unwrap();
        assert_eq!(r.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), ["00000010", "00000011", "00000012", "00000013", "00000014"]);
        let r = v.range("", None, 3, true).unwrap();
        assert_eq!(r.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), ["00000049", "00000048", "00000047"]);
    }

    #[test]
    fn apply_put_edit_delete_keeps_untouched_raw() {
        let v = View::parse(encode(&rows(10, 0)).unwrap()).unwrap();
        let out = v.apply(&[Op::Put("00000003".into(), json!({"edited": true})), Op::Del("00000005".into()), Op::Put("zzz".into(), json!(1))], TARGET_CHUNK).unwrap();
        assert_eq!(out.len(), 1);
        let n = View::parse(out[0].0.clone()).unwrap();
        assert_eq!(n.len(), 10);
        assert_eq!(n.get("00000003").unwrap().unwrap(), json!({"edited": true}));
        assert!(n.get("00000005").unwrap().is_none());
        assert_eq!(n.get("00000007").unwrap(), v.get("00000007").unwrap());
        assert_eq!((out[0].1.as_str(), out[0].2.as_str()), ("00000000", "zzz"));
    }

    #[test]
    fn big_chunk_splits_into_ordered_ranges_within_limit() {
        let v = View::parse(encode(&rows(10, 0)).unwrap()).unwrap();
        let ops: Vec<Op> = rows(2000, 400).into_iter().map(|(k, x)| Op::Put(k, x)).collect();
        let out = v.apply(&ops, 100_000).unwrap();
        assert!(out.len() >= 8, "{}", out.len());
        let mut total = 0;
        let mut prev_hi: Option<String> = None;
        for (bytes, lo, hi) in &out {
            assert!(bytes.len() <= 100_000 + 2_000, "chunk {} bytes", bytes.len());
            let c = View::parse(bytes.clone()).unwrap();
            assert_eq!((c.first_key().unwrap(), c.last_key().unwrap()), (lo.clone(), hi.clone()));
            if let Some(p) = &prev_hi { assert!(p < lo, "ranges overlap"); }
            prev_hi = Some(hi.clone());
            total += c.len();
        }
        assert_eq!(total, 2000);
    }

    #[test]
    fn deleting_everything_drops_the_chunk() {
        let v = View::parse(encode(&rows(3, 0)).unwrap()).unwrap();
        let ops: Vec<Op> = v.keys().into_iter().map(Op::Del).collect();
        assert!(v.apply(&ops, TARGET_CHUNK).unwrap().is_empty());
    }

    #[test]
    fn corrupt_files_are_rejected_not_panicked() {
        assert!(View::parse(b"nope".to_vec()).is_err());
        let mut b = encode(&rows(5, 0)).unwrap();
        b.truncate(b.len() - 3);
        assert!(View::parse(b).is_err());
        assert!(View::parse(vec![]).is_err());
    }

    #[test]
    fn seal_open_roundtrip_and_tamper() {
        let key = [7u8; 32];
        let plain = encode(&rows(20, 3)).unwrap();
        let s = seal(&key, &plain).unwrap();
        assert_ne!(&s[..plain.len().min(20)], &plain[..plain.len().min(20)]);
        assert_eq!(open(&key, &s).unwrap(), plain);
        assert!(open(&[8u8; 32], &s).is_err(), "wrong key");
        let mut t = s.clone();
        t[5] ^= 1;
        assert!(open(&key, &t).is_err(), "tampered");
    }

    #[test]
    fn names() { assert_eq!(chunk_name("epizod_db", 3, 12), "DBP_epizod_db_3_v12"); }
}
