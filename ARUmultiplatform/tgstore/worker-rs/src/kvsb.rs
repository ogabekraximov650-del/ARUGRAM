//! UZ: tgstore RO'YXATI Supabase (Postgres) da — DO o'rniga. Faqat bo'lak nomi/file_id/kalit/oraliq. PostgREST `rpc/<funksiya>` orqali.
//!     Atomik commit (bir nechta jadval bitta tranzaksiyada) `kv_commit` funksiyasida (`tgstore/supabase/schema.sql`).
//! EN: the tgstore REGISTRY lives in Supabase (Postgres) instead of a Durable Object. Names/file_id/keys/ranges only, via PostgREST RPC.
//! RU: REESTR tgstore v Supabase (Postgres) vmesto DO. Tolko imena/file_id/klyuchi/diapazony.

use crate::kvreg::{ChunkMeta, TableMeta};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use worker::*;

/// UZ: almashtirilgan eski fayl shuncha vaqtdan keyin o'chiriladi. EN/RU: delay before deleting a replaced file.
pub const GC_DELAY_MS: i64 = 60_000;

pub struct TableCommit {
    pub tbl: String,
    pub rev: u64,
    pub remove: Vec<u64>,
    pub add: Vec<ChunkMeta>,
}

pub enum CommitResult {
    Ok(BTreeMap<String, TableMeta>),
    Conflict(String),
}

fn cfg(env: &Env) -> Result<(String, String)> {
    let url = crate::tg_secret(env, "SUPABASE_URL");
    // UZ: ochiq (publishable/anon) kalit yetarli — service_role KERAK EMAS (xavfsizlik `p_tok` da). EN/RU: the public key is enough.
    let key = crate::tg_secret(env, "SUPABASE_KEY");
    if url.is_empty() || key.is_empty() { return Err(Error::RustError("SUPABASE_URL/SUPABASE_KEY yo'q".into())); }
    Ok((url.trim_end_matches('/').to_string(), key))
}

pub async fn rpc(env: &Env, func: &str, mut body: Value) -> Result<Value> {
    let (url, key) = cfg(env)?;
    // UZ: har chaqiruvga maxfiy kalit (`STORE_KEY`) qo'shiladi; bazada faqat uning xeshi (`kv_auth`). EN/RU: every call carries the secret STORE_KEY.
    let tok = crate::tg_secret(env, "STORE_KEY");
    if let Some(o) = body.as_object_mut() { o.insert("p_tok".into(), json!(tok)); }
    let h = Headers::new();
    h.set("apikey", &key)?;
    h.set("Authorization", &format!("Bearer {key}"))?;
    h.set("Content-Type", "application/json")?;
    let mut init = RequestInit::new();
    init.with_method(Method::Post).with_headers(h).with_body(Some(body.to_string().into()));
    let mut last = String::new();
    for attempt in 0..3 {
        let mut r = Fetch::Request(Request::new_with_init(&format!("{url}/rest/v1/rpc/{func}"), &init)?).send().await?;
        let code = r.status_code();
        if (200..300).contains(&code) { return r.json().await; }
        last = format!("{code}: {}", r.text().await.unwrap_or_default().chars().take(300).collect::<String>());
        // UZ: vaqtincha xato (5xx/429) — qayta uriniladi. EN/RU: transient errors retry.
        if !(code == 429 || code >= 500) || attempt == 2 { break; }
    }
    Err(Error::RustError(format!("supabase {func} {last}")))
}

pub async fn get_many(env: &Env, tables: &[String]) -> Result<BTreeMap<String, TableMeta>> {
    let v = rpc(env, "kv_get_many", json!({"p_tbls": tables})).await?;
    serde_json::from_value(v).map_err(|e| Error::RustError(format!("meta: {e}")))
}

/// UZ: bir nechta jadval ro'yxatini ATOMIK almashtiradi (hammasi yoki hech narsa). EN/RU: atomic multi-table registry swap.
pub async fn commit(env: &Env, now: i64, tables: Vec<TableCommit>) -> Result<CommitResult> {
    let t: Vec<Value> = tables.iter().map(|c| json!({"tbl": c.tbl, "rev": c.rev, "remove": c.remove, "add": c.add})).collect();
    let r = rpc(env, "kv_commit", json!({"p": {"now": now, "gc_delay": GC_DELAY_MS, "tables": t}})).await?;
    if r["ok"] == json!(true) {
        let metas: BTreeMap<String, TableMeta> = serde_json::from_value(r["metas"].clone()).unwrap_or_default();
        Ok(CommitResult::Ok(metas))
    } else {
        Ok(CommitResult::Conflict(r["conflict"].as_str().unwrap_or("").to_string()))
    }
}

pub async fn gc_take(env: &Env, now: i64) -> Result<Vec<i64>> {
    let r = rpc(env, "kv_gc_take", json!({"p_now": now})).await?;
    Ok(r["due"].as_array().map(|a| a.iter().filter_map(|g| g["msg_id"].as_i64()).collect()).unwrap_or_default())
}

pub async fn gc_done(env: &Env, ids: &[i64]) -> Result<i64> {
    let r = rpc(env, "kv_gc_done", json!({"p_ids": ids})).await?;
    Ok(r["pending"].as_i64().unwrap_or(0))
}

/// UZ: keshni bekor qilish belgilari (`MarkHub` o'rniga). EN/RU: cache-invalidation marks (instead of MarkHub).
pub async fn marks(env: &Env, bump: &[String], at: i64, get: &[String]) -> Result<std::collections::HashMap<String, i64>> {
    let r = rpc(env, "kv_marks", json!({"p": {"bump": bump, "at": at, "get": get}})).await?;
    Ok(r["v"].as_object().map(|m| m.iter().filter_map(|(k, x)| Some((k.clone(), x.as_i64()?))).collect()).unwrap_or_default())
}

/// UZ: kunlik "uyg'otish" so'rovi (Toshkent 05:00). EN/RU: daily keep-alive ping.
pub async fn ping(env: &Env) -> Result<Value> {
    rpc(env, "kv_ping", json!({})).await
}

/// UZ: tozalash uchun xabar raqamlari (navbatdagi `limit` ta). EN/RU: next batch of message ids to wipe.
pub async fn wipe_take(env: &Env, limit: i64) -> Result<Vec<i64>> {
    let r = rpc(env, "kv_wipe_take", json!({"p_limit": limit})).await?;
    Ok(r["ids"].as_array().map(|a| a.iter().filter_map(|x| x.as_i64()).collect()).unwrap_or_default())
}

pub async fn wipe_done(env: &Env, ids: &[i64], fin: bool) -> Result<()> {
    rpc(env, "kv_wipe_done", json!({"p_ids": ids, "p_final": fin})).await?;
    Ok(())
}

/// UZ: jadvalni ro'yxatdan olib tashlaydi, Telegram'dan o'chiriladigan xabar raqamlarini qaytaradi. EN/RU: drop a table, return its message ids.
pub async fn drop_table(env: &Env, table: &str) -> Result<Vec<i64>> {
    let r = rpc(env, "kv_drop_table", json!({"p_tbl": table})).await?;
    Ok(r["ids"].as_array().map(|a| a.iter().filter_map(|x| x.as_i64()).collect()).unwrap_or_default())
}
