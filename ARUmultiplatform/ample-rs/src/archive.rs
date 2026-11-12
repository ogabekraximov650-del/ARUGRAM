//! UZ: DOIRA TIZIMI (Rust): baza 70% to'lganda sovuq qatorlar `db_N_vV/` papkalaridagi jurnallarga (PostgreSQL `COPY ... CSV HEADER` formatida) qo'shilib, shifrlanib
//!     Telegram'ga yuklanadi va bazadan o'chiriladi; arxivdan o'qish (`lookup`), bazaga qaytarish (`restore`/`warm`) va tozalash (`purge`) bor. Jurnal papkalari
//!     `JR_ARCH_DIR` (bazaning o'z jurnallaridan ALOHIDA) da turadi. 1 MiB ga yetgach bo'lak yopiladi, keyingi raqamli papka ochiladi.
//! EN: CIRCLE SYSTEM (Rust): past 70% fill, cold rows are appended as CSV journals into `db_N_vV/` folders, sealed, uploaded to Telegram and deleted from the DB;
//!     rows can be read back (`lookup`), restored/kept warm, or purged. Folder cache lives in `JR_ARCH_DIR`, separate from the DB's own journals.
//! RU: СИСТЕМА КРУГА (Rust): при 70% холодные строки уходят в CSV-журналы папок db_N_vV, шифруются и загружаются в Telegram.
use crate::crypto::{self, key32};
use crate::store::{now_ms, Res, Shared, Stmt};
use crate::tg::Tg;
use anyhow::{anyhow, Result};
use libsql::Value;
use serde_json::{json, Value as J};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub const THRESHOLD: f64 = 70.0;
pub const STOP_AT: f64 = 65.0;
const BATCH: i64 = 500;
const DAY: i64 = 86_400_000;

pub struct Spec { pub table: &'static str, pub keys: &'static [&'static str], pub cold: &'static str, pub days: i64 }

/// Sovuq jadvallar (ilovada qayta ko'rsatilmaydi): kalit ustunlari, sovuqlik sharti (`?1` = kesim vaqti ms), necha kundan keyin. EN/RU: cold tables.
pub const SPECS: &[Spec] = &[
    Spec { table: "billing_log", keys: &["id"], cold: "created_at < ?1", days: 180 },
    Spec { table: "reports_db", keys: &["id"], cold: "created_at < ?1", days: 180 },
    Spec { table: "comments_db", keys: &["id"], cold: "deleted = 1 AND created_at < ?1", days: 90 },
    Spec { table: "sessions_db", keys: &["id"], cold: "last_seen_at < ?1", days: 90 },
    Spec { table: "watch_history_db", keys: &["user_id", "anime_id", "season_id", "epizod_id"], cold: "deleted_at > 0 AND deleted_at < ?1", days: 90 },
];

fn spec_of(t: &str) -> Option<&'static Spec> { SPECS.iter().find(|s| s.table == t) }
fn key_sql(s: &Spec, alias: &str) -> String { s.keys.iter().map(|k| format!("CAST({}{} AS TEXT)", if alias.is_empty() { String::new() } else { format!("{alias}.") }, k)).collect::<Vec<_>>().join(" || ':' || ") }
fn ti(n: i64) -> J { json!({"type": "integer", "value": n.to_string()}) }
fn tt(s: &str) -> J { json!({"type": "text", "value": s}) }
fn st(sql: &str, args: Vec<J>) -> Stmt { Stmt { sql: sql.to_string(), args } }

// ── CSV (PostgreSQL COPY ... CSV HEADER): NULL = bo'sh, bo'sh satr = "" ─────────────────────────────────────────────────────────────────────
fn csv_cell(v: &Option<String>) -> String { match v { None => String::new(), Some(s) => format!("\"{}\"", s.replace('"', "\"\"")) } }
pub fn to_csv(cols: &[String], rows: &[Vec<Option<String>>]) -> String {
    let mut o = cols.join(",");
    o.push('\n');
    for r in rows { o.push_str(&r.iter().map(csv_cell).collect::<Vec<_>>().join(",")); o.push('\n'); }
    o
}
pub fn parse_csv(text: &str) -> Vec<Vec<Option<String>>> {
    let mut rows = vec![]; let mut row: Vec<Option<String>> = vec![]; let mut cur = String::new(); let (mut quoted, mut in_q) = (false, false);
    let mut it = text.chars().peekable();
    while let Some(c) = it.next() {
        if in_q { if c == '"' { if it.peek() == Some(&'"') { cur.push('"'); it.next(); } else { in_q = false; } } else { cur.push(c); } }
        else { match c {
            '"' => { in_q = true; quoted = true; }
            ',' => { row.push(if cur.is_empty() && !quoted { None } else { Some(std::mem::take(&mut cur)) }); quoted = false; }
            '\n' => { row.push(if cur.is_empty() && !quoted { None } else { Some(std::mem::take(&mut cur)) }); quoted = false; rows.push(std::mem::take(&mut row)); }
            '\r' => {}
            _ => cur.push(c),
        } }
    }
    if !cur.is_empty() || quoted || !row.is_empty() { row.push(if cur.is_empty() && !quoted { None } else { Some(cur) }); rows.push(row); }
    rows
}
fn objects(text: &str) -> (Vec<String>, Vec<BTreeMap<String, Option<String>>>) {
    let mut p = parse_csv(text);
    if p.is_empty() { return (vec![], vec![]); }
    let head: Vec<String> = p.remove(0).into_iter().map(|c| c.unwrap_or_default()).collect();
    let rows = p.into_iter().map(|r| head.iter().enumerate().map(|(i, h)| (h.clone(), r.get(i).cloned().unwrap_or(None))).collect()).collect();
    (head, rows)
}
fn key_of(s: &Spec, o: &BTreeMap<String, Option<String>>) -> String { s.keys.iter().map(|k| o.get(*k).cloned().flatten().unwrap_or_default()).collect::<Vec<_>>().join(":") }
fn val_str(v: &Value) -> Option<String> {
    match v { Value::Null => None, Value::Integer(i) => Some(i.to_string()), Value::Real(f) => Some(f.to_string()), Value::Text(s) => Some(s.clone()),
              Value::Blob(b) => Some(format!("x'{}'", hex::encode(b))) }
}

pub struct Arch { pub st: Shared, pub tg: Tg, pub dir: PathBuf, pub chunk_bytes: u64, pub busy: tokio::sync::Mutex<()> }

impl Arch {
    pub fn new(st: Shared, tg: Tg, dir: PathBuf, chunk_bytes: u64) -> Self { let _ = std::fs::create_dir_all(&dir); Self { st, tg, dir, chunk_bytes, busy: tokio::sync::Mutex::new(()) } }

    pub async fn init(&self) -> Result<()> {
        let mut s = self.st.lock().await;
        for q in ["create table if not exists arch_files(id integer primary key autoincrement, no integer, ver integer default 1, sealed integer default 0, name text, file_id text, msg_id integer, key text, rows integer, size integer, created_at integer, restored_at integer default 0)",
                  "create table if not exists arch_keys(tbl text not null, k text not null, fid integer not null, primary key(tbl, k))",
                  "create table if not exists arch_warm(tbl text not null, k text not null, until integer not null, primary key(tbl, k))"] {
            if let Res::Err(e) = s.exec(&[st(q, vec![])], true).await.remove(0) { return Err(anyhow!(e)); }
        }
        // UZ: eski (Worker yaratgan) `arch_files` da bu ustunlar yo'q bo'lishi mumkin: qo'shiladi, takror xatosi e'tiborsiz. EN/RU: add columns missing from an older table.
        for q in ["alter table arch_files add column no integer", "alter table arch_files add column ver integer default 1", "alter table arch_files add column sealed integer default 0"] {
            if let Res::Err(e) = s.exec(&[st(q, vec![])], true).await.remove(0) { if !e.contains("duplicate column") { return Err(anyhow!(e)); } }
        }
        Ok(())
    }

    fn folder(&self, no: i64, ver: i64) -> PathBuf { self.dir.join(format!("db_{no}_v{ver}")) }

    /// Bo'lak papkasini ochadi: papka yangi bo'lsa shu yerdan, aks holda Telegram'dan olib o'z nomli papkaga joylaydi. EN/RU: open a chunk folder (download+unpack if stale).
    async fn load(&self, row: &BTreeMap<String, Value>) -> Result<BTreeMap<String, Vec<u8>>> {
        let get_i = |k: &str| match row.get(k) { Some(Value::Integer(i)) => *i, _ => 0 };
        let get_s = |k: &str| match row.get(k) { Some(Value::Text(s)) => s.clone(), _ => String::new() };
        let dir = self.folder(get_i("no"), get_i("ver").max(1));
        if let Ok(m) = std::fs::read(dir.join("meta.json")) {
            if serde_json::from_slice::<J>(&m).ok().and_then(|j| j["msg_id"].as_i64()) == Some(get_i("msg_id")) {
                let mut files = BTreeMap::new();
                for e in std::fs::read_dir(&dir)?.flatten() { let n = e.file_name().to_string_lossy().to_string(); if n.ends_with(".csv") { files.insert(n, std::fs::read(e.path())?); } }
                return Ok(files);
            }
        }
        let files = crypto::unpack(&crypto::open(&key32(&get_s("key"))?, &self.tg.get(&get_s("file_id")).await?)?)?;
        self.write_folder(get_i("no"), get_i("ver").max(1), get_i("msg_id"), get_i("sealed") != 0, &files);
        Ok(files)
    }
    fn write_folder(&self, no: i64, ver: i64, msg_id: i64, sealed: bool, files: &BTreeMap<String, Vec<u8>>) {
        let d = self.folder(no, ver);
        let _ = std::fs::remove_dir_all(&d); let _ = std::fs::create_dir_all(&d);
        for (n, b) in files { let _ = std::fs::write(d.join(n), b); }
        let _ = std::fs::write(d.join("meta.json"), json!({"no": no, "ver": ver, "msg_id": msg_id, "sealed": sealed}).to_string());
        self.trim();
    }
    fn trim(&self) { // yopilgan papkalar keshi 200 MB dan oshmasin. EN/RU: bounded cache of sealed folders
        let mut list: Vec<(std::time::SystemTime, PathBuf, u64, bool)> = vec![];
        if let Ok(rd) = std::fs::read_dir(&self.dir) { for e in rd.flatten() {
            let p = e.path(); if !p.is_dir() { continue; }
            let size: u64 = std::fs::read_dir(&p).map(|d| d.flatten().filter_map(|x| x.metadata().ok()).map(|m| m.len()).sum()).unwrap_or(0);
            let sealed = std::fs::read(p.join("meta.json")).ok().and_then(|m| serde_json::from_slice::<J>(&m).ok()).map(|j| j["sealed"] == json!(true)).unwrap_or(true);
            list.push((e.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH), p, size, sealed));
        } }
        list.sort_by_key(|x| x.0);
        let mut total: u64 = list.iter().map(|x| x.2).sum();
        for (_, p, sz, sealed) in list { if total <= 200 * 1024 * 1024 { break; } if sealed { let _ = std::fs::remove_dir_all(p); total -= sz; } }
    }

    async fn row_map(&self, sql: &str, args: Vec<Value>) -> Result<Vec<BTreeMap<String, Value>>> {
        let (cols, rows) = self.st.lock().await.query_raw(sql, args).await?;
        Ok(rows.into_iter().map(|r| cols.iter().cloned().zip(r).collect()).collect())
    }

    /// Bitta jadvaldan BIR bo'lak qatorni ochiq bo'lak jurnaliga qo'shadi. EN/RU: appends one batch of one table to the open chunk.
    async fn archive_one(&self, spec: &Spec, dry: bool) -> Result<J> {
        let now = now_ms() as i64;
        let cut = now - spec.days * DAY;
        let q = format!("select * from {t} t where {cold} and not exists (select 1 from arch_warm w where w.tbl = ?2 and w.k = {k} and w.until > ?3) order by {ord} limit {lim}",
            t = spec.table, cold = spec.cold, k = key_sql(spec, "t"), ord = spec.keys.iter().map(|k| format!("t.{k}")).collect::<Vec<_>>().join(", "), lim = BATCH);
        let (cols, rows) = self.st.lock().await.query_raw(&q, vec![Value::Integer(cut), Value::Text(spec.table.into()), Value::Integer(now)]).await?;
        if rows.is_empty() || dry { return Ok(json!({"table": spec.table, "found": rows.len(), "archived": 0, "dry": dry})); }
        let cells: Vec<Vec<Option<String>>> = rows.iter().map(|r| r.iter().map(val_str).collect()).collect();
        let keys: Vec<String> = cells.iter().map(|r| spec.keys.iter().map(|k| { let i = cols.iter().position(|c| c == k).unwrap(); r[i].clone().unwrap_or_default() }).collect::<Vec<_>>().join(":")).collect();
        let body = to_csv(&cols, &cells);
        // Ochiq bo'lak yoki yangisi.
        let open = self.row_map("select * from arch_files where sealed = 0 order by id desc limit 1", vec![]).await?.into_iter().next();
        let (no, ver, mut files) = match &open {
            Some(r) => { let no = match r.get("no") { Some(Value::Integer(i)) => *i, _ => 1 }; (no, match r.get("ver") { Some(Value::Integer(i)) => (*i).max(1), _ => 1 }, self.load(r).await?) }
            None => { let m = self.row_map("select coalesce(max(no),0)+1 as n from arch_files", vec![]).await?; (match m[0].get("n") { Some(Value::Integer(i)) => *i, _ => 1 }, 1, BTreeMap::new()) }
        };
        let fname = format!("{}.csv", spec.table);
        let add = match files.get(&fname) { Some(prev) => { let mut p = prev.clone(); p.extend_from_slice(body.split_once('\n').map(|x| x.1).unwrap_or("").as_bytes()); p } None => body.into_bytes() };
        files.insert(fname, add);
        let total: usize = files.values().map(|b| b.len()).sum();
        let sealed = total as u64 >= self.chunk_bytes;
        let key_hex = crypto::random_key_hex();
        let packed = crypto::seal(&key32(&key_hex)?, &crypto::pack(&files));
        let name = format!("db_{no}_v{ver}.bin");
        let (file_id, msg_id) = self.tg.send(&name, packed.clone()).await?;
        // Bazada: ro'yxat + qatorlarni o'chirish (sovuqlik qayta tekshiriladi) + kalit indeksi — BITTA tranzaksiya.
        let old_msg = open.as_ref().and_then(|r| match r.get("msg_id") { Some(Value::Integer(i)) => Some(*i), _ => None });
        let list_stmt = match &open {
            Some(r) => { let id = match r.get("id") { Some(Value::Integer(i)) => *i, _ => 0 };
                st("update arch_files set file_id=?1, msg_id=?2, key=?3, rows=coalesce(rows,0)+?4, size=?5, sealed=?6, name=?7 where id=?8 returning id", vec![tt(&file_id), ti(msg_id), tt(&key_hex), ti(rows.len() as i64), ti(packed.len() as i64), ti(sealed as i64), tt(&name), ti(id)]) }
            None => st("insert into arch_files(no,ver,sealed,name,file_id,msg_id,key,rows,size,created_at) values(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10) returning id", vec![ti(no), ti(ver), ti(sealed as i64), tt(&name), tt(&file_id), ti(msg_id), tt(&key_hex), ti(rows.len() as i64), ti(packed.len() as i64), ti(now)]),
        };
        let keys_json = serde_json::to_string(&keys)?;
        let del = format!("delete from {t} where {k} in (select value from json_each(?1)) and ({cold}) returning {k} as k", t = spec.table, k = key_sql(spec, ""), cold = spec.cold.replace("?1", "?2"));
        let res = { let mut s = self.st.lock().await; s.exec(&[list_stmt, st(&del, vec![tt(&keys_json), ti(cut)])], true).await };
        let (fid, gone): (i64, Vec<String>) = match (&res[0], res.get(1)) {
            (Res::Ok(a), Some(Res::Ok(b))) => (a["rows"][0][0]["value"].as_str().and_then(|x| x.parse().ok()).unwrap_or(0), b["rows"].as_array().cloned().unwrap_or_default().iter().filter_map(|r| r[0]["value"].as_str().map(|x| x.to_string())).collect()),
            (Res::Err(e), _) | (_, Some(Res::Err(e))) => { self.tg.del(msg_id).await; return Err(anyhow!(e.clone())); }
            _ => { self.tg.del(msg_id).await; return Err(anyhow!("noma'lum natija")); }
        };
        if let Res::Err(e) = self.st.lock().await.exec(&[st("insert or replace into arch_keys(tbl,k,fid) select ?1, value, ?2 from json_each(?3)", vec![tt(spec.table), ti(fid), tt(&serde_json::to_string(&gone)?)])], true).await.remove(0) { return Err(anyhow!(e)); }
        self.write_folder(no, ver, msg_id, sealed, &files);
        if let Some(o) = old_msg { self.tg.del(o).await; }
        Ok(json!({"table": spec.table, "found": rows.len(), "archived": gone.len(), "chunk": format!("db_{no}_v{ver}"), "sealed": sealed, "bytes": packed.len(), "dry": false}))
    }

    /// Baza `THRESHOLD` dan oshsa ishga tushadi; `STOP_AT` ga tushsa to'xtaydi. EN/RU: one scheduler tick.
    pub async fn run(&self, force: bool, dry: bool, max: usize) -> Result<J> {
        let Ok(_g) = self.busy.try_lock() else { return Ok(json!({"skipped": "band"})) };
        self.init().await?;
        let pct = |s: &crate::store::Store| s.cfg.limit_mb as f64;
        let _ = pct;
        let percent = async { let s = self.st.lock().await; s.live_bytes().await as f64 / (s.cfg.limit_mb as f64 * 1048576.0) * 100.0 };
        let p0 = percent.await;
        if p0 < THRESHOLD && !force { return Ok(json!({"active": false, "percent": p0, "threshold": THRESHOLD})); }
        if !dry {
            let _ = self.st.lock().await.exec(&[st("delete from login_tokens where expires_at < ?1", vec![ti(now_ms() as i64 - DAY)]), st("delete from arch_warm where until < ?1", vec![ti(now_ms() as i64)])], true).await;
        }
        let mut steps = vec![];
        for _ in 0..max {
            let mut did = false;
            for spec in SPECS {
                let r = self.archive_one(spec, dry).await?;
                let moved = r["archived"].as_i64().unwrap_or(0) > 0 || (dry && r["found"].as_i64().unwrap_or(0) > 0);
                steps.push(r);
                if moved { did = true; break; }
            }
            if !did || dry { break; }
            if !force { let p = { let s = self.st.lock().await; s.live_bytes().await as f64 / (s.cfg.limit_mb as f64 * 1048576.0) * 100.0 }; if p < STOP_AT { break; } }
        }
        Ok(json!({"active": true, "percent": p0, "threshold": THRESHOLD, "steps": steps}))
    }

    pub async fn files(&self) -> Result<J> {
        self.init().await?;
        let (cols, rows) = self.st.lock().await.query_raw("select id, no, ver, sealed, name, rows, size, created_at, restored_at from arch_files order by id desc limit 100", vec![]).await?;
        let list: Vec<J> = rows.iter().map(|r| J::Object(cols.iter().zip(r).map(|(c, v)| (c.clone(), match v { Value::Integer(i) => json!(i), Value::Text(s) => json!(s), _ => J::Null })).collect())).collect();
        Ok(json!({"files": list, "threshold": THRESHOLD}))
    }

    async fn file_row(&self, id: i64) -> Result<Option<BTreeMap<String, Value>>> { Ok(self.row_map("select * from arch_files where id=?1", vec![Value::Integer(id)]).await?.into_iter().next()) }

    /// Arxivdagi bitta qator (yoki `null`). EN/RU: one archived row.
    pub async fn lookup(&self, table: &str, key: &str) -> Result<J> {
        let spec = spec_of(table).ok_or_else(|| anyhow!("noma'lum jadval"))?;
        let k = self.row_map("select fid from arch_keys where tbl=?1 and k=?2", vec![Value::Text(table.into()), Value::Text(key.into())]).await?;
        let Some(fid) = k.first().and_then(|r| match r.get("fid") { Some(Value::Integer(i)) => Some(*i), _ => None }) else { return Ok(J::Null) };
        let Some(row) = self.file_row(fid).await? else { return Ok(J::Null) };
        let files = self.load(&row).await?;
        let Some(csv) = files.get(&format!("{table}.csv")) else { return Ok(J::Null) };
        let (_, objs) = objects(&String::from_utf8_lossy(csv));
        Ok(objs.into_iter().find(|o| key_of(spec, o) == key).map(|o| J::Object(o.into_iter().map(|(k, v)| (k, v.map(J::String).unwrap_or(J::Null))).collect())).unwrap_or(J::Null))
    }

    /// Bo'lakdagi hamma qatorni bazaga qaytaradi (mavjudlariga tegmaydi) va `warm_days` kunga qayta arxivlanishdan himoya qiladi. EN/RU: restore + keep warm.
    pub async fn restore(&self, id: i64, warm_days: i64) -> Result<J> {
        let row = self.file_row(id).await?.ok_or_else(|| anyhow!("fayl topilmadi"))?;
        let files = self.load(&row).await?;
        let until = now_ms() as i64 + warm_days * DAY;
        let mut stmts = vec![]; let mut tables = vec![];
        for (n, buf) in &files {
            let Some(spec) = spec_of(n.trim_end_matches(".csv")) else { continue };
            let (head, objs) = objects(&String::from_utf8_lossy(buf));
            if objs.is_empty() { continue; }
            tables.push(spec.table);
            let ins = format!("insert or ignore into {} ({}) values ({})", spec.table, head.iter().map(|h| format!("\"{h}\"")).collect::<Vec<_>>().join(","), (1..=head.len()).map(|i| format!("?{i}")).collect::<Vec<_>>().join(","));
            let keys: Vec<String> = objs.iter().map(|o| key_of(spec, o)).collect();
            for o in &objs { stmts.push(st(&ins, head.iter().map(|h| match o.get(h).cloned().flatten() { Some(v) => tt(&v), None => json!({"type": "null"}) }).collect())); }
            stmts.push(st("insert or replace into arch_warm(tbl,k,until) select ?1, value, ?2 from json_each(?3)", vec![tt(spec.table), ti(until), tt(&serde_json::to_string(&keys)?)]));
        }
        stmts.push(st("update arch_files set restored_at=?1 where id=?2", vec![ti(now_ms() as i64), ti(id)]));
        let res = self.st.lock().await.exec(&stmts, true).await;
        if let Some(Res::Err(e)) = res.iter().find(|r| matches!(r, Res::Err(_))) { return Err(anyhow!(e.clone())); }
        let restored: i64 = res.iter().zip(&stmts).filter(|(_, s)| s.sql.starts_with("insert or ignore")).map(|(r, _)| match r { Res::Ok(v) => v["affected_row_count"].as_i64().unwrap_or(0), _ => 0 }).sum();
        Ok(json!({"restored": restored, "tables": tables, "warm_until": until}))
    }

    pub async fn warm(&self, table: &str, key: &str, days: i64) -> Result<J> {
        let k = self.row_map("select fid from arch_keys where tbl=?1 and k=?2", vec![Value::Text(table.into()), Value::Text(key.into())]).await?;
        match k.first().and_then(|r| match r.get("fid") { Some(Value::Integer(i)) => Some(*i), _ => None }) {
            None => Ok(json!({"restored": 0, "found": false})),
            Some(fid) => { let mut r = self.restore(fid, days).await?; r["found"] = json!(true); Ok(r) }
        }
    }

    /// Bo'lakni butunlay o'chiradi (ma'lumot arxivda QOLMAYDI). Faqat sinov/tozalash. EN/RU: removes a chunk entirely.
    pub async fn purge(&self, id: i64) -> Result<J> {
        let Some(row) = self.file_row(id).await? else { return Ok(json!({"purged": false})) };
        let get_i = |k: &str| match row.get(k) { Some(Value::Integer(i)) => *i, _ => 0 };
        let ok = self.tg.del(get_i("msg_id")).await;
        let _ = self.st.lock().await.exec(&[st("delete from arch_keys where fid=?1", vec![ti(id)]), st("delete from arch_files where id=?1", vec![ti(id)])], true).await;
        let _ = std::fs::remove_dir_all(self.folder(get_i("no"), get_i("ver").max(1)));
        Ok(json!({"purged": true, "telegram_deleted": ok}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_keeps_null_empty_quotes_and_newlines() {
        let cols = vec!["id".to_string(), "body".to_string(), "note".to_string()];
        let rows = vec![vec![Some("1".into()), Some("salom, \"dunyo\"".into()), None], vec![Some("2".into()), Some("".into()), Some("a\nb".into())]];
        let p = parse_csv(&to_csv(&cols, &rows));
        assert_eq!(p[0], vec![Some("id".into()), Some("body".into()), Some("note".into())]);
        assert_eq!(p[1], rows[0]);
        assert_eq!(p[2], rows[1]);
        let (head, objs) = objects(&to_csv(&cols, &rows));
        assert_eq!(head, cols);
        assert_eq!(objs[1].get("note").cloned().flatten(), Some("a\nb".to_string()));
    }

    #[test]
    fn composite_keys_join_with_colon() {
        let w = spec_of("watch_history_db").unwrap();
        let o: BTreeMap<String, Option<String>> = [("user_id", "1"), ("anime_id", "2"), ("season_id", "3"), ("epizod_id", "4")].iter().map(|(k, v)| (k.to_string(), Some(v.to_string()))).collect();
        assert_eq!(key_of(w, &o), "1:2:3:4");
        assert_eq!(key_sql(w, "t"), "CAST(t.user_id AS TEXT) || ':' || CAST(t.anime_id AS TEXT) || ':' || CAST(t.season_id AS TEXT) || ':' || CAST(t.epizod_id AS TEXT)");
    }
}
