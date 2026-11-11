//! UZ: O'QISH = Telegram (bot API) + Cloudflare keshi, YOZISH/TAHRIRLASH/O'CHIRISH = Worker + Ample PostgreSQL.
//!     Hamma uchun bir xil "katalog" jadvallari (anime, bo'lim, qism, janr) Ample'dan har ~5 daqiqada (o'zgargan bo'lsagina) `kvdb` bo'laklariga
//!     (Telegram fayllari, ro'yxati Supabase `DB_CHUNK` da) nusxalanadi. Katalogni FAQAT o'qiydigan SELECT lar o'sha nusxadan, xotiradagi SQLite orqali
//!     (production SQL o'zgarishsiz) bajariladi: Ample'ga murojaat yo'q. Yozuvlar va shaxsiy ma'lumotlar (foydalanuvchi, tarix, balans...) har doim Ample'dan.
//!     Ishonchlilik: nusxa bo'lmasa yoki o'qish yiqilsa — Ample'dan; Ample yiqilsa — katalog nusxadan.
//! EN: READS = Telegram (bot API) + Cloudflare cache; WRITES/EDITS/DELETES = Worker + Ample PostgreSQL. The shared "catalog" tables (anime, season, episode,
//!     genre) are copied from Ample into `kvdb` chunks about every 5 minutes (only when changed). SELECTs that touch ONLY the catalog run against that copy via
//!     in-memory SQLite (production SQL unchanged), never touching Ample. Writes and personal data always use Ample. Falls back both ways.
//! RU: ЧТЕНИЕ = Telegram + кэш Cloudflare, ЗАПИСЬ = Worker + Ample PostgreSQL. Каталог копируется в чанки раз в ~5 минут; SELECT только по каталогу идёт из копии.

use crate::kvfmt::Op;
use crate::kvdb::{self, TableOps};
use crate::{kvsql, marks_get, now_ms, row_to_obj, turso_exec_pg};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use worker::*;

/// UZ: katalog jadvallari va ularning birlamchi kaliti. EN/RU: catalog tables and their primary keys.
pub const CATALOG: &[(&str, &[&str])] = &[
    ("anime_db", &["id"]),
    ("season_db", &["anime_id", "season_id"]),
    ("season_janr", &["anime_id", "season_id"]),
    ("epizod_db", &["anime_id", "season_id", "epizod_id"]),
];

/// UZ: production sxemasidagi HAMMA jadval nomlari (so'rov faqat katalogni o'qiydimi — shu bilan aniqlanadi). EN/RU: every production table name.
const ALL_TABLES: &[&str] = &[
    "anime_db", "app_config", "billing_log", "chat_messages", "chat_threads", "comment_likes", "comments_db", "encode_jobs", "encode_q", "epizod_db",
    "favorites_db", "login_tokens", "payments_db", "ratings_db", "reports_db", "season_db", "season_janr", "sessions_db", "stats_daily", "stats_hourly",
    "subs_db", "sync_batches", "tg_files", "users_db", "watch_history_db", "arch_files",
];

const PAGE: usize = 2000;
/// UZ: ikki nashr orasidagi eng kam vaqt. EN/RU: minimum gap between two publishes.
const MIN_GAP_MS: i64 = 120_000;

thread_local! {
    /// (nashr vaqti, tekshirilgan vaqt). EN/RU: (published_at, checked_at).
    static STATE: RefCell<(i64, i64)> = RefCell::new((0, 0));
}

fn is_pg(env: &Env) -> bool {
    env.var("DB_BACKEND").map(|v| v.to_string() == "pg").unwrap_or(false)
}

/// UZ: so'rov FAQAT katalog jadvallarini o'qiydimi (sof tekshiruv). `MAX(` (raqam ajratish) va `FOR UPDATE` yozish yo'lida ishlatiladi — ular har doim Ample'dan.
/// EN/RU: pure check: does the statement read ONLY catalog tables. `MAX(` (id allocation) always goes to Ample.
pub fn catalog_only(sql: &str) -> bool {
    let low = sql.trim_start().to_lowercase();
    if !low.starts_with("select") || low.contains("max(") || low.contains("for update") { return false; }
    let all: BTreeSet<String> = ALL_TABLES.iter().map(|t| t.to_string()).collect();
    let used = kvsql::tables_in(sql, &all);
    !used.is_empty() && used.iter().all(|t| CATALOG.iter().any(|(c, _)| c == t))
}

async fn published_at(env: &Env) -> i64 {
    let (at, chk) = STATE.with(|s| *s.borrow());
    let age = now_ms() - chk;
    if (at > 0 && age < 60_000) || (at == 0 && age < 15_000 && chk > 0) { return at; }
    let v = kvdb::get(env, "_snap", "state").await.ok().flatten();
    let at = v.and_then(|v| v["at"].as_i64()).unwrap_or(0);
    STATE.with(|s| *s.borrow_mut() = (at, now_ms()));
    at
}

/// UZ: bu so'rovlarni nusxadan o'qish mumkinmi (Postgres rejimi, katalog, nusxa bor, sxema tiklandi). EN/RU: can these statements be served from the copy.
pub async fn can_read(env: &Env, sqls: &[&str]) -> bool {
    if !is_pg(env) || sqls.is_empty() || !sqls.iter().all(|s| catalog_only(s)) { return false; }
    published_at(env).await > 0 && kvsql::load_schema(env).await
}

fn ddl_for(table: &str, cols: &[String], pk: &[&str]) -> String {
    let q = |c: &str| format!("\"{c}\"");
    format!("CREATE TABLE IF NOT EXISTS {table} ({}, PRIMARY KEY ({}))", cols.iter().map(|c| q(c)).collect::<Vec<_>>().join(", "), pk.iter().map(|c| q(c)).collect::<Vec<_>>().join(", "))
}

async fn read_table(env: &Env, table: &str, pk: &[&str]) -> Result<(Vec<String>, Vec<Value>)> {
    let mut cols: Vec<String> = Vec::new();
    let mut objs: Vec<Value> = Vec::new();
    let mut off = 0usize;
    loop {
        let sql = format!("SELECT * FROM {table} ORDER BY {} LIMIT {PAGE} OFFSET {off}", pk.join(", "));
        let r = turso_exec_pg(env, &sql, vec![]).await?;
        let c = r["cols"].as_array().cloned().unwrap_or_default();
        if cols.is_empty() { cols = c.iter().map(|x| x["name"].as_str().unwrap_or("").to_string()).collect(); }
        let rows = r["rows"].as_array().cloned().unwrap_or_default();
        let n = rows.len();
        for row in &rows { objs.push(row_to_obj(&c, row.as_array().map(|a| a.as_slice()).unwrap_or(&[]))); }
        if n < PAGE { break; }
        off += PAGE;
    }
    Ok((cols, objs))
}

/// UZ: katalogni Ample'dan Telegram bo'laklariga nusxalaydi (faqat o'zgargan qatorlar). `force` — o'zgarish belgisiz ham. EN/RU: publishes the catalog copy.
pub async fn publish(env: &Env, force: bool) -> Result<Value> {
    if !is_pg(env) { return Ok(json!({"skipped": "pg rejimi emas"})); }
    let at = { STATE.with(|s| s.borrow().0) };
    let at = if at > 0 { at } else { kvdb::get(env, "_snap", "state").await.ok().flatten().and_then(|v| v["at"].as_i64()).unwrap_or(0) };
    let names: Vec<&str> = CATALOG.iter().map(|(t, _)| *t).collect();
    if !force && at > 0 {
        if now_ms() - at < MIN_GAP_MS { return Ok(json!({"skipped": "tez-tez", "published_at": at})); }
        let (last, _) = marks_get(env, &names).await;
        if last <= at { return Ok(json!({"skipped": "o'zgarish yo'q", "published_at": at})); }
    }
    let started = now_ms();
    for attempt in 0..3 {
        let tables: Vec<String> = names.iter().map(|t| t.to_string()).collect();
        let (read, olds) = kvdb::snapshot_tables(env, &tables, true).await?;
        let mut items: Vec<TableOps> = Vec::new();
        let mut ddl: Vec<(String, String)> = Vec::new();
        let mut counts = serde_json::Map::new();
        for (i, (t, pk)) in CATALOG.iter().enumerate() {
            let (cols, objs) = read_table(env, t, pk).await?;
            let pkv: Vec<String> = pk.iter().map(|s| s.to_string()).collect();
            let mut new: BTreeMap<String, Value> = BTreeMap::new();
            for o in objs { if let Some(m) = o.as_object() { new.insert(kvsql::row_key(m, &pkv), o.clone()); } }
            let old: BTreeMap<String, Value> = olds[i].iter().cloned().collect();
            counts.insert(t.to_string(), json!(new.len()));
            ddl.push((t.to_string(), ddl_for(t, &cols, pk)));
            let ops = kvsql::diff(&old, &new);
            if !ops.is_empty() { items.push(TableOps { table: t.to_string(), ops, read: Some(read.clone()) }); }
        }
        let schema_now = kvdb::get(env, "_schema", "ddl").await.ok().flatten();
        let schema_new = json!({"hash": "snap", "ddl": ddl});
        if schema_now.as_ref() != Some(&schema_new) { items.push(TableOps { table: "_schema".into(), ops: vec![Op::Put("ddl".into(), schema_new)], read: None }); }
        items.push(TableOps { table: "_snap".into(), ops: vec![Op::Put("state".into(), json!({"at": started, "rows": Value::Object(counts.clone())}))], read: None });
        if kvdb::commit_multi(env, items).await? {
            STATE.with(|s| *s.borrow_mut() = (started, now_ms()));
            return Ok(json!({"published": true, "at": started, "rows": counts, "attempt": attempt + 1}));
        }
    }
    Err(Error::RustError("nashr: to'qnashuv (3 urinish)".into()))
}

/// UZ: holat (sinov/admin uchun). EN/RU: status for tests/admin.
pub async fn status(env: &Env) -> Value {
    let at = published_at(env).await;
    let v = kvdb::get(env, "_snap", "state").await.ok().flatten().unwrap_or(Value::Null);
    json!({"published_at": at, "age_s": if at > 0 { (now_ms() - at) / 1000 } else { -1 }, "state": v})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_only_detects_pure_catalog_reads() {
        assert!(catalog_only("SELECT * FROM anime_db ORDER BY id DESC LIMIT 100"));
        assert!(catalog_only("SELECT e.* FROM epizod_db e JOIN season_db s ON s.anime_id=e.anime_id WHERE e.anime_id = ?"));
        assert!(!catalog_only("SELECT * FROM users_db WHERE id = ?"));
        assert!(!catalog_only("SELECT a.id FROM anime_db a JOIN favorites_db f ON f.anime_id=a.id"));
        assert!(!catalog_only("SELECT COALESCE(MAX(epizod_id), 0) FROM epizod_db"));
        assert!(!catalog_only("UPDATE season_db SET views_total = views_total + 1"));
        assert!(!catalog_only("SELECT 1"));
    }

    #[test]
    fn ddl_is_untyped_with_primary_key() {
        let d = ddl_for("season_db", &["anime_id".into(), "season_id".into(), "nomi".into()], &["anime_id", "season_id"]);
        assert_eq!(d, "CREATE TABLE IF NOT EXISTS season_db (\"anime_id\", \"season_id\", \"nomi\", PRIMARY KEY (\"anime_id\", \"season_id\"))");
    }
}
