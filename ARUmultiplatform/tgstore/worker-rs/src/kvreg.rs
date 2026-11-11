//! UZ: tgstore RO'YXATI turlari. Ro'yxat Supabase (Postgres) da (`kvsb.rs`, `tgstore/supabase/schema.sql`): FAQAT bo'lak nomi, Telegram `file_id`,
//!     ochish kaliti va kalit oralig'i. Ma'lumotning o'zi YO'Q. Yangilanish — `rev` bo'yicha taqqoslab almashtirish (CAS), jadvallararo ATOMIK.
//! EN: registry types. The registry lives in Supabase (Postgres): ONLY chunk name, file_id, key and range; no data. Updates are CAS on `rev`, atomic across tables.
//! RU: tipy reestra. Reestr v Supabase (Postgres): tolko imya, file_id, klyuch i diapazon. Obnovlenie — CAS po `rev`, atomarno mezhdu tablitsami.

use serde::{Deserialize, Serialize};

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

/// UZ: kalit qaysi bo'lakka tegishli: `min <= kalit` bo'lgan oxirgi bo'lak, bo'lmasa birinchisi. EN: routes a key to a chunk. RU: marshrutizatsiya klyucha.
pub fn route(chunks: &[ChunkMeta], key: &str) -> Option<usize> {
    if chunks.is_empty() {
        return None;
    }
    let p = chunks.partition_point(|c| c.min.as_str() <= key);
    Some(if p == 0 { 0 } else { p - 1 })
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
    fn json_shape_matches_the_sql_functions() {
        // `kv_get_many` shu kalitlar bilan qaytaradi / `kv_get_many` returns these keys / vozvrashchaet eti klyuchi
        let j = serde_json::json!({"rev": 3, "next_no": 5, "chunks": [{"no": 1, "ver": 2, "name": "DBP_t_1_v2", "file_id": "F", "msg_id": 9, "key": "ab", "min": "a", "max": "z", "rows": 4, "size": 100}]});
        let m: TableMeta = serde_json::from_value(j).unwrap();
        assert_eq!((m.rev, m.next_no, m.chunks[0].ver, m.chunks[0].min.as_str()), (3, 5, 2, "a"));
        let back = serde_json::to_value(&m.chunks[0]).unwrap();
        for k in ["no", "ver", "name", "file_id", "msg_id", "key", "min", "max", "rows", "size"] { assert!(back.get(k).is_some(), "{k}"); }
    }
}
