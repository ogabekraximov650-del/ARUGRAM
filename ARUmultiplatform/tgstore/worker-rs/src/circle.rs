//! UZ: "DOIRA TIZIMI". Ample PostgreSQL hajmi 70% ga yetganda sovuq (kam o'qiladigan) qatorlar fayllarga yig'ilib Telegram'ga yuklanadi
//!     va PostgreSQL'dan o'chiriladi: ma'lumot doira bo'ylab aylanadi, Ample'dagi baza o'smaydi.
//!     FAYL FORMATI = PostgreSQL `COPY ... CSV HEADER` (NULL — bo'sh, bo'sh satr — ""), AES-256-GCM bilan shifrlangan. Kalitni ochgach fayl
//!     to'g'ridan-to'g'ri `\copy jadval FROM fayl CSV HEADER` bilan Postgres'ga yuklanadi; ilovada esa `restore` buyrug'i bor.
//!     Ro'yxat (fayl nomi, file_id, kalit, qatorlar soni) Ample'dagi kichik `arch_files` jadvalida. Bosqich: faqat qayta ko'rsatilmaydigan
//!     sovuq jadvallar (eski to'lovlar jurnali, shikoyatlar, o'chirilgan izohlar, eski sessiyalar, o'chirilgan tarix).
//! EN: "CIRCLE SYSTEM". When the Ample PostgreSQL size reaches 70%, cold rows are packed into files, uploaded to Telegram and deleted from
//!     PostgreSQL. FILE FORMAT = PostgreSQL `COPY ... CSV HEADER` (NULL = empty, empty string = ""), sealed with AES-256-GCM; once opened it loads
//!     straight into Postgres with `\copy table FROM file CSV HEADER`; the app has a `restore` command. A small `arch_files` table in Ample is the registry.
//! RU: «СИСТЕМА КРУГА». При 70% заполнения Ample PostgreSQL холодные строки пакуются в файлы (формат PostgreSQL `COPY ... CSV HEADER`,
//!     шифрование AES-256-GCM), уходят в Telegram и удаляются из PostgreSQL; реестр — малая таблица `arch_files`; есть команда `restore`.

use crate::{kvdb, kvfmt, now_ms, random_hex, row_to_obj, turso_batch, turso_exec, TursoArg};
use serde_json::{json, Value};
use worker::*;

/// UZ: doira yoqiladigan to'lish foizi. EN/RU: fill percent at which the circle starts.
pub const THRESHOLD: f64 = 70.0;
const BATCH: i64 = 500;
const DAY: i64 = 86_400_000;

struct Spec {
    table: &'static str,
    keys: &'static [&'static str],
    /// WHERE sharti; `?` — kesim vaqti (ms). EN/RU: cold predicate; `?` is the cutoff (ms).
    cold: &'static str,
    days: i64,
}

const SPECS: &[Spec] = &[
    Spec { table: "billing_log", keys: &["id"], cold: "created_at < ?", days: 180 },
    Spec { table: "reports_db", keys: &["id"], cold: "created_at < ?", days: 180 },
    Spec { table: "comments_db", keys: &["id"], cold: "deleted = 1 AND created_at < ?", days: 90 },
    Spec { table: "sessions_db", keys: &["id"], cold: "last_seen_at < ?", days: 90 },
    Spec { table: "watch_history_db", keys: &["user_id", "anime_id", "season_id", "epizod_id"], cold: "deleted_at > 0 AND deleted_at < ?", days: 90 },
];

/// UZ: Ample darvozasidan hajm (`/usage`). EN/RU: size from the Ample gateway.
pub async fn usage(env: &Env) -> Result<Value> {
    let (url, key) = (env.secret("TURSO_URL")?.to_string(), env.secret("TURSO_TOKEN")?.to_string());
    let mut h = Headers::new();
    h.set("Authorization", &format!("Bearer {key}"))?;
    h.set("Content-Type", "application/json")?;
    let r = Request::new_with_init(&format!("{url}/usage"),
        RequestInit::new().with_method(Method::Post).with_headers(h).with_body(Some("{}".into())))?;
    let mut resp = Fetch::Request(r).send().await?;
    resp.json().await
}

// ── CSV (PostgreSQL COPY ... CSV HEADER) ─────────────────────────────────────

fn csv_cell(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => format!("\"{}\"", s.replace('"', "\"\"")),
        other => other.to_string(),
    }
}

/// UZ: qatorlar -> CSV matn (birinchi satr — ustun nomlari). EN/RU: rows -> CSV text with header.
pub fn to_csv(cols: &[String], objs: &[Value]) -> String {
    let mut out = cols.join(",");
    out.push('\n');
    for o in objs {
        out.push_str(&cols.iter().map(|c| csv_cell(&o[c.as_str()])).collect::<Vec<_>>().join(","));
        out.push('\n');
    }
    out
}

/// UZ: CSV -> qatorlar; `None` = NULL (qo'shtirnoqsiz bo'sh maydon). EN/RU: CSV -> rows; `None` is NULL.
pub fn parse_csv(text: &str) -> Vec<Vec<Option<String>>> {
    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    let mut row: Vec<Option<String>> = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut in_q = false;
    let mut it = text.chars().peekable();
    let push = |row: &mut Vec<Option<String>>, cur: &mut String, quoted: &mut bool| {
        row.push(if cur.is_empty() && !*quoted { None } else { Some(std::mem::take(cur)) });
        *quoted = false;
    };
    while let Some(c) = it.next() {
        if in_q {
            if c == '"' { if it.peek() == Some(&'"') { cur.push('"'); it.next(); } else { in_q = false; } } else { cur.push(c); }
        } else {
            match c {
                '"' => { in_q = true; quoted = true; }
                ',' => push(&mut row, &mut cur, &mut quoted),
                '\n' => { push(&mut row, &mut cur, &mut quoted); rows.push(std::mem::take(&mut row)); }
                '\r' => {}
                _ => cur.push(c),
            }
        }
    }
    if !cur.is_empty() || quoted || !row.is_empty() { push(&mut row, &mut cur, &mut quoted); rows.push(row); }
    rows
}

fn arg_of(v: &Value) -> TursoArg {
    match v.as_i64() { Some(n) => TursoArg::int(n), None => TursoArg::text(v.as_str().unwrap_or("")) }
}

async fn ensure_registry(env: &Env) -> Result<()> {
    turso_exec(env, "CREATE TABLE IF NOT EXISTS arch_files (
        id INTEGER PRIMARY KEY AUTOINCREMENT, tbl TEXT, name TEXT, file_id TEXT, msg_id INTEGER, key TEXT,
        rows INTEGER, size INTEGER, created_at INTEGER, restored_at INTEGER DEFAULT 0)", vec![]).await?;
    Ok(())
}

/// UZ: bitta jadvaldan BIR bo'lak (<= 500 qator) arxivlaydi. EN/RU: archives one batch from one table.
async fn archive_one(env: &Env, spec: &Spec, dry: bool) -> Result<Value> {
    let cut = now_ms() - spec.days * DAY;
    let sql = format!("SELECT * FROM {} WHERE {} ORDER BY {} LIMIT {BATCH}", spec.table, spec.cold, spec.keys[0]);
    let res = turso_exec(env, &sql, vec![TursoArg::int(cut)]).await?;
    let cols = res["cols"].as_array().cloned().unwrap_or_default();
    let rows = res["rows"].as_array().cloned().unwrap_or_default();
    if rows.is_empty() || dry {
        return Ok(json!({"table": spec.table, "found": rows.len(), "archived": 0, "dry": dry}));
    }
    let names: Vec<String> = cols.iter().map(|c| c["name"].as_str().unwrap_or("").to_string()).collect();
    let objs: Vec<Value> = rows.iter().map(|r| row_to_obj(&cols, r.as_array().map(|a| a.as_slice()).unwrap_or(&[]))).collect();
    let csv = to_csv(&names, &objs);
    let key_hex = random_hex(64);
    let key = kvdb::hex_key(&key_hex)?;
    let sealed = kvfmt::seal(&key, csv.as_bytes()).map_err(Error::RustError)?;
    let fname = format!("arch_{}_{}.csv.enc", spec.table, now_ms());
    // UZ: tartib: Telegram -> ro'yxat (Ample) -> PostgreSQL'dan o'chirish. Har bosqich yiqilsa oldingisi qaytariladi. EN/RU: upload -> register -> delete; roll back on failure.
    let (file_id, msg_id) = crate::vd::tg_send_document(&kvdb::token(env), kvdb::channel(env), &fname, "", &sealed).await?;
    let reg = turso_exec(env, "INSERT INTO arch_files (tbl, name, file_id, msg_id, key, rows, size, created_at) VALUES (?,?,?,?,?,?,?,?)",
        vec![TursoArg::text(spec.table), TursoArg::text(&fname), TursoArg::text(&file_id), TursoArg::int(msg_id), TursoArg::text(&key_hex),
             TursoArg::int(objs.len() as i64), TursoArg::int(sealed.len() as i64), TursoArg::int(now_ms())]).await;
    if let Err(e) = reg { kvdb::tg_delete_many(env, &[msg_id]).await; return Err(e); }
    let del = format!("DELETE FROM {} WHERE {}", spec.table, spec.keys.iter().map(|k| format!("{k} = ?")).collect::<Vec<_>>().join(" AND "));
    let stmts: Vec<(&str, Vec<TursoArg>)> = objs.iter().map(|o| (del.as_str(), spec.keys.iter().map(|k| arg_of(&o[*k])).collect())).collect();
    turso_batch(env, &stmts).await?;
    Ok(json!({"table": spec.table, "found": objs.len(), "archived": objs.len(), "file": fname, "bytes": sealed.len(), "dry": false}))
}

/// UZ: cron/qo'lda ishga tushirish. `force` — foizdan qat'i nazar; `dry` — faqat sanash. EN/RU: cron / manual run.
pub async fn run(env: &Env, force: bool, dry: bool) -> Result<Value> {
    let u = usage(env).await?;
    let pct = u["percent"].as_f64().unwrap_or(0.0);
    if pct < THRESHOLD && !force {
        return Ok(json!({"active": false, "percent": pct, "threshold": THRESHOLD}));
    }
    ensure_registry(env).await?;
    // UZ: muddati o'tgan kirish tokenlari arxivsiz tozalanadi. EN/RU: expired login tokens are simply purged.
    if !dry {
        let _ = turso_exec(env, "DELETE FROM login_tokens WHERE expires_at < ?", vec![TursoArg::int(now_ms() - DAY)]).await;
    }
    let start = ((now_ms() / 600_000) as usize) % SPECS.len();
    let mut out = Vec::new();
    for i in 0..SPECS.len() {
        let spec = &SPECS[(start + i) % SPECS.len()];
        match archive_one(env, spec, dry).await {
            Ok(v) => { let n = v["archived"].as_i64().unwrap_or(0) + v["found"].as_i64().unwrap_or(0) * dry as i64; out.push(v); if n > 0 { break; } }
            Err(e) => { out.push(json!({"table": spec.table, "error": e.to_string()})); break; }
        }
    }
    Ok(json!({"active": true, "percent": pct, "threshold": THRESHOLD, "steps": out}))
}

/// UZ: arxiv fayllari ro'yxati. EN/RU: list of archive files.
pub async fn files(env: &Env) -> Result<Value> {
    ensure_registry(env).await?;
    let r = turso_exec(env, "SELECT id, tbl, name, rows, size, created_at, restored_at FROM arch_files ORDER BY id DESC LIMIT 100", vec![]).await?;
    let cols = r["cols"].as_array().cloned().unwrap_or_default();
    let list: Vec<Value> = r["rows"].as_array().cloned().unwrap_or_default().iter()
        .map(|row| row_to_obj(&cols, row.as_array().map(|a| a.as_slice()).unwrap_or(&[]))).collect();
    Ok(json!({"files": list}))
}

/// UZ: arxiv faylini PostgreSQL'ga qaytaradi (mavjud qatorlarga tegmaydi). EN/RU: loads an archive file back into PostgreSQL (existing rows untouched).
pub async fn restore(env: &Env, id: i64) -> Result<Value> {
    let r = turso_exec(env, "SELECT tbl, file_id, key FROM arch_files WHERE id = ?", vec![TursoArg::int(id)]).await?;
    let row = r["rows"].get(0).cloned().unwrap_or(Value::Null);
    if row.is_null() { return Err(Error::RustError("fayl topilmadi".into())); }
    let cell = |i: usize| row[i]["value"].as_str().unwrap_or("").to_string();
    let (tbl, file_id, key_hex) = (cell(0), cell(1), cell(2));
    let spec = SPECS.iter().find(|s| s.table == tbl).ok_or_else(|| Error::RustError("noma'lum jadval".into()))?;
    let sealed = kvdb::download(env, &file_id).await?;
    let plain = kvfmt::open(&kvdb::hex_key(&key_hex)?, &sealed).map_err(Error::RustError)?;
    let text = String::from_utf8(plain).map_err(|_| Error::RustError("fayl matn emas".into()))?;
    let mut rows = parse_csv(&text);
    if rows.is_empty() { return Ok(json!({"restored": 0})); }
    let header: Vec<String> = rows.remove(0).into_iter().map(|c| c.unwrap_or_default()).collect();
    // UZ: nom faqat arxivlangan jadval ustunlaridan iborat bo'lishi shart (SQL kiritmasligi uchun). EN/RU: header must be plain identifiers.
    if header.iter().any(|h| h.is_empty() || !h.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')) { return Err(Error::RustError("noto'g'ri sarlavha".into())); }
    let sql = format!("INSERT OR IGNORE INTO {} ({}) VALUES ({})", spec.table, header.join(","), header.iter().map(|_| "?").collect::<Vec<_>>().join(","));
    let stmts: Vec<(&str, Vec<TursoArg>)> = rows.iter().map(|r| (sql.as_str(), r.iter().map(|c| match c {
        Some(v) => TursoArg::text(v),
        None => TursoArg { type_: "null".into(), value: None },
    }).collect())).collect();
    turso_batch(env, &stmts).await?;
    let _ = turso_exec(env, "UPDATE arch_files SET restored_at = ? WHERE id = ?", vec![TursoArg::int(now_ms()), TursoArg::int(id)]).await;
    Ok(json!({"restored": rows.len(), "table": tbl}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_roundtrip_keeps_null_empty_quotes_and_newlines() {
        let cols = vec!["id".to_string(), "body".to_string(), "note".to_string()];
        let rows = vec![
            json!({"id": 1, "body": "salom, \"dunyo\"", "note": null}),
            json!({"id": 2, "body": "", "note": "a\nb"}),
        ];
        let text = to_csv(&cols, &rows);
        let p = parse_csv(&text);
        assert_eq!(p[0], vec![Some("id".into()), Some("body".into()), Some("note".into())]);
        assert_eq!(p[1], vec![Some("1".into()), Some("salom, \"dunyo\"".into()), None]);
        assert_eq!(p[2], vec![Some("2".into()), Some("".into()), Some("a\nb".into())]);
        assert_eq!(p.len(), 3);
    }
}
