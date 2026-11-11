//! UZ: tgstore RO'YXATI (Durable Object). Unda FAQAT: bo'lak fayl nomi, Telegram `file_id`, ochish kaliti va kalit oralig'i. Ma'lumotning o'zi YO'Q.
//! EN: the tgstore REGISTRY (Durable Object). It holds ONLY: chunk file name, Telegram `file_id`, decryption key and key range. NO data.
//! RU: REESTR tgstore (Durable Object). Khranit TOLKO: imya fayla chanka, `file_id`, klyuch i diapazon klyuchey. Dannykh NET.
//!
//! UZ: Yangilanish "taqqoslab almashtirish" (CAS): `rev` mos kelmasa rad etiladi — ikki worker bir vaqtda yozsa biri qayta urinadi.
//! EN: Updates are compare-and-swap on `rev`: a stale writer is rejected and retries. RU: obnovlenie cherez CAS po `rev`.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use worker::*;

/// UZ: eski fayl Telegram'dan shuncha vaqtdan keyin o'chiriladi (o'qiyotganlar uzilmasin). EN: delay before deleting a replaced file. RU: zaderzhka udaleniya.
pub const GC_DELAY_MS: i64 = 120_000;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct ChunkMeta {
    pub no: u64,
    pub ver: u64,
    /// `DBP_<jadval>_<no>_v<ver>`
    pub name: String,
    pub file_id: String,
    pub msg_id: i64,
    /// 32 bayt AES-256-GCM kaliti (hex, 64 belgi) — har bo'lak versiyasiga alohida / per chunk version / na versiyu chanka
    pub key: String,
    pub min: String,
    pub max: String,
    pub rows: u32,
    pub size: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct TableMeta {
    pub rev: u64,
    pub next_no: u64,
    /// `min` bo'yicha tartiblangan, oraliqlar kesishmaydi / sorted by `min`, ranges do not overlap / otsortirovano po `min`
    pub chunks: Vec<ChunkMeta>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GcItem {
    pub msg_id: i64,
    pub name: String,
    pub due: i64,
}

pub enum Commit {
    Ok,
    Conflict,
}

/// UZ: sof mantiq (testlanadi): `rev` mos bo'lsa `remove` dagilarni olib tashlab, `add` ni qo'shadi, olib tashlanganlarni GC ga yozadi.
/// EN: pure logic (tested). RU: chistaya logika (testiruetsya).
pub fn apply_commit(meta: &mut TableMeta, rev: u64, remove: &[u64], add: Vec<ChunkMeta>, now: i64, gc: &mut Vec<GcItem>) -> Commit {
    if meta.rev != rev {
        return Commit::Conflict;
    }
    let mut keep = Vec::with_capacity(meta.chunks.len() + add.len());
    for c in meta.chunks.drain(..) {
        if remove.contains(&c.no) {
            if c.msg_id != 0 {
                gc.push(GcItem { msg_id: c.msg_id, name: c.name.clone(), due: now + GC_DELAY_MS });
            }
        } else {
            keep.push(c);
        }
    }
    for c in add {
        meta.next_no = meta.next_no.max(c.no + 1);
        keep.push(c);
    }
    keep.sort_by(|a, b| a.min.cmp(&b.min));
    meta.chunks = keep;
    meta.rev += 1;
    Commit::Ok
}

/// UZ: kalit qaysi bo'lakka tegishli: `min <= kalit` bo'lgan oxirgi bo'lak, bo'lmasa birinchisi. EN: routes a key to a chunk. RU: marshrutizatsiya klyucha.
pub fn route(chunks: &[ChunkMeta], key: &str) -> Option<usize> {
    if chunks.is_empty() {
        return None;
    }
    let p = chunks.partition_point(|c| c.min.as_str() <= key);
    Some(if p == 0 { 0 } else { p - 1 })
}

#[durable_object(fetch)]
pub struct KvRegistry {
    state: State,
}

impl DurableObject for KvRegistry {
    fn new(state: State, _env: Env) -> Self {
        Self { state }
    }

    async fn fetch(&self, mut req: Request) -> Result<Response> {
        let body: Value = req.json().await?;
        let storage = self.state.storage();
        let table = body["table"].as_str().unwrap_or("").to_string();
        let tkey = format!("t:{table}");
        match body["op"].as_str().unwrap_or("") {
            "get" => {
                let meta: TableMeta = storage.get(&tkey).await?.unwrap_or_default();
                Response::from_json(&meta)
            }
            "commit" => {
                let mut meta: TableMeta = storage.get(&tkey).await?.unwrap_or_default();
                let rev = body["rev"].as_u64().unwrap_or(u64::MAX);
                let remove: Vec<u64> = serde_json::from_value(body["remove"].clone()).unwrap_or_default();
                let add: Vec<ChunkMeta> = serde_json::from_value(body["add"].clone()).unwrap_or_default();
                let now = Date::now().as_millis() as i64;
                let mut gc: Vec<GcItem> = storage.get("gc").await?.unwrap_or_default();
                match apply_commit(&mut meta, rev, &remove, add, now, &mut gc) {
                    Commit::Conflict => {
                        let cur: TableMeta = storage.get(&tkey).await?.unwrap_or_default();
                        Response::from_json(&json!({"ok": false, "conflict": true, "meta": cur}))
                    }
                    Commit::Ok => {
                        storage.put(&tkey, &meta).await?;
                        storage.put("gc", &gc).await?;
                        Response::from_json(&json!({"ok": true, "meta": meta}))
                    }
                }
            }
            "gc_take" => {
                let now = Date::now().as_millis() as i64;
                let gc: Vec<GcItem> = storage.get("gc").await?.unwrap_or_default();
                let due: Vec<&GcItem> = gc.iter().filter(|g| g.due <= now).take(50).collect();
                Response::from_json(&json!({"due": due, "pending": gc.len()}))
            }
            "gc_done" => {
                let ids: Vec<i64> = serde_json::from_value(body["ids"].clone()).unwrap_or_default();
                let mut gc: Vec<GcItem> = storage.get("gc").await?.unwrap_or_default();
                gc.retain(|g| !ids.contains(&g.msg_id));
                storage.put("gc", &gc).await?;
                Response::from_json(&json!({"ok": true, "pending": gc.len()}))
            }
            _ => Response::error("noma'lum amal", 400),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(no: u64, min: &str, max: &str) -> ChunkMeta {
        ChunkMeta { no, ver: 1, name: format!("DBP_t_{no}_v1"), file_id: format!("F{no}"), msg_id: 100 + no as i64, key: "k".repeat(64), min: min.into(), max: max.into(), rows: 1, size: 10 }
    }

    #[test]
    fn route_picks_containing_or_nearest_chunk() {
        let v = vec![c(1, "a", "f"), c(2, "g", "m"), c(3, "n", "z")];
        assert_eq!(route(&v, "a"), Some(0));
        assert_eq!(route(&v, "c"), Some(0));
        assert_eq!(route(&v, "g"), Some(1));
        assert_eq!(route(&v, "mm"), Some(1));
        assert_eq!(route(&v, "zz"), Some(2));
        assert_eq!(route(&v, "0"), Some(0), "below the first range goes to the first chunk");
        assert_eq!(route(&[], "x"), None);
    }

    #[test]
    fn commit_cas_replaces_and_queues_gc() {
        let mut m = TableMeta::default();
        let mut gc = vec![];
        assert!(matches!(apply_commit(&mut m, 0, &[], vec![c(1, "a", "f")], 1000, &mut gc), Commit::Ok));
        assert_eq!((m.rev, m.next_no, m.chunks.len()), (1, 2, 1));
        // eskirgan rev rad etiladi / stale rev is rejected / ustarevshiy rev otklonyaetsya
        assert!(matches!(apply_commit(&mut m, 0, &[1], vec![], 1000, &mut gc), Commit::Conflict));
        assert_eq!(m.chunks.len(), 1);
        // chunk 1 yangi versiya bilan almashtiriladi / chunk 1 replaced by a new version / chank 1 zamenyaetsya
        let mut nv = c(1, "a", "f");
        nv.ver = 2;
        nv.msg_id = 555;
        assert!(matches!(apply_commit(&mut m, 1, &[1], vec![nv, c(2, "g", "z")], 2000, &mut gc), Commit::Ok));
        assert_eq!((m.rev, m.next_no, m.chunks.len()), (2, 3, 2));
        assert_eq!(gc.len(), 1);
        assert_eq!((gc[0].msg_id, gc[0].due), (101, 2000 + GC_DELAY_MS));
        assert_eq!(m.chunks[0].ver, 2);
    }

    #[test]
    fn commit_keeps_chunks_sorted_and_drops_emptied() {
        let mut m = TableMeta::default();
        let mut gc = vec![];
        apply_commit(&mut m, 0, &[], vec![c(2, "m", "z"), c(1, "a", "f")], 0, &mut gc);
        assert_eq!(m.chunks.iter().map(|x| x.no).collect::<Vec<_>>(), vec![1, 2]);
        apply_commit(&mut m, 1, &[1], vec![], 0, &mut gc);
        assert_eq!(m.chunks.len(), 1);
        assert_eq!(gc.len(), 1);
    }
}
