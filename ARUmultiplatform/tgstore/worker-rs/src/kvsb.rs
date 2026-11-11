//! UZ: tgstore RO'YXATI Supabase (Postgres) da — FAQAT BITTA JADVAL `DB_CHUNK`: har qator = bitta bo'lak fayli (`db_1_v1`, `db_2_v1`, ...) yoki yordamchi qator
//!     (eski fayl navbati, kesh belgisi). Bo'lak 1 MB ga to'lsa keyingi `db_<n+1>_v1` yaratiladi. Atomik almashtirish `kv_commit` funksiyasida (`tgstore/supabase/schema.sql`).
//! EN: the tgstore REGISTRY lives in Supabase (Postgres) as ONE table `DB_CHUNK`: one row per chunk file (`db_1_v1`, `db_2_v1`, ...) or helper row (old-file queue, cache mark).
//!     When a chunk fills up (1 MB) the next `db_<n+1>_v1` is created. The atomic swap is in the `kv_commit` function.
//! RU: REESTR tgstore v Supabase (Postgres) — TOLKO ODNA TABLITSA `DB_CHUNK`: odna stroka = odin fayl chanka (`db_1_v1`, `db_2_v1`, ...) ili sluzhebnaya stroka.

use crate::kvreg::{ChunkMeta, TableMeta};
use serde_json::{json, Value};
use worker::*;

/// UZ: almashtirilgan eski fayl shuncha vaqtdan keyin o'chiriladi. EN/RU: delay before deleting a replaced file.
pub const GC_DELAY_MS: i64 = 60_000;

pub enum CommitResult {
    /// UZ: muvaffaqiyat — yangi ro'yxat. EN/RU: success with the new list.
    Ok(TableMeta),
    /// UZ: to'qnashuv — o'qilgan/o'zgartirilgan bo'laklardan biri boshqa yozuvchi tomonidan almashtirilgan. EN/RU: a chunk we read/changed was replaced by another writer.
    Conflict,
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
    // UZ: har chaqiruvga maxfiy kalit (`STORE_KEY`) qo'shiladi; bazada faqat uning xeshi. EN/RU: every call carries the secret STORE_KEY.
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

fn parse_list(v: &Value) -> TableMeta {
    serde_json::from_value(v.clone()).unwrap_or_default()
}

/// UZ: hamma tirik bo'laklar ro'yxati (kalit oralig'i bo'yicha tartiblangan). EN/RU: all live chunks, sorted by key range.
pub async fn list(env: &Env) -> Result<TableMeta> {
    Ok(parse_list(&rpc(env, "kv_list", json!({})).await?))
}

/// UZ: bo'laklarni ATOMIK almashtiradi. `read` — o'qilgan bo'laklar (hali tirik bo'lishi shart), `remove` — olib tashlanadiganlar, `add` — yangilari.
///     Biri allaqachon boshqa yozuvchi tomonidan almashtirilgan bo'lsa HECH NARSA o'zgarmaydi (`Conflict`). EN/RU: atomic chunk swap with a read-set check.
pub async fn commit(env: &Env, now: i64, read: &[String], remove: &[String], add: &[ChunkMeta]) -> Result<CommitResult> {
    let r = rpc(env, "kv_commit", json!({"p": {"now": now, "gc_delay": GC_DELAY_MS, "read": read, "remove": remove, "add": add}})).await?;
    if r["ok"] == json!(true) { Ok(CommitResult::Ok(parse_list(&r["list"]))) } else { Ok(CommitResult::Conflict) }
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

/// UZ: kunlik "uyg'otish" so'rovi (Toshkent 05:05). EN/RU: daily keep-alive ping.
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
