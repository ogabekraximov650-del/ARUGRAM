//! UZ: SQL ko'prigi. Production'dagi SQL o'zgarishsiz ishlaydi: kerakli jadvallar bo'laklardan xotiradagi SQLite'ga yuklanadi,
//!     buyruqlar bajariladi, o'zgargan qatorlar (farq) bo'laklarga qaytariladi. Jadval = alohida bo'laklar to'plami.
//! EN: SQL bridge. Production SQL runs unchanged: needed tables are loaded from chunks into in-memory SQLite, statements run, and the
//!     changed rows (a diff) are written back to chunks. One table = its own set of chunks.
//! RU: SQL-most. SQL produkshna rabotaet bez izmeneniy: tablitsy gruzyatsya iz chankov v SQLite v pamyati, komandy vypolnyayutsya,
//!     izmenennye stroki (diff) vozvrashchayutsya v chanki.
//!
//! UZ: Cheklov: bir paketdagi buyruqlar bir nechta jadvalga yozsa, jadvallar ketma-ket saqlanadi (jadvallararo tranzaksiya YO'Q).
//! EN: Limit: a batch writing several tables commits them one after another (no cross-table transaction). RU: net mezhtablichnoy tranzaktsii.

use crate::kvfmt::Op;
use crate::TursoArg;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use worker::*;

#[cfg(target_arch = "wasm32")]
use crate::sqlmem::{Mem, Param};

pub fn enabled(env: &Env) -> bool {
    env.var("DB_BACKEND").map(|v| v.to_string() == "kv").unwrap_or(false)
}

thread_local! {
    /// UZ: sxema: (jadval, DDL) ro'yxati, bajarilgan tartibda. EN: schema (table, DDL) in execution order. RU: skhema.
    static SCHEMA: RefCell<Vec<(String, String)>> = RefCell::new(Vec::new());
    static READY: RefCell<bool> = RefCell::new(false);
    /// UZ: sxema yaratilayotganda faqat DDL bajariladi, DML o'tkazib yuboriladi (bo'sh baza + ichki chaqiruvlar chegarasi). EN/RU: DDL-only while creating the schema.
    static DDL_ONLY: RefCell<bool> = RefCell::new(false);
}

// ── Sof yordamchilar (native testlar) / Pure helpers (native tests) / Chistye pomoshchniki ──────────────────────────────

fn tokens(sql: &str) -> Vec<String> {
    sql.to_lowercase()
        .replace(['(', ')', ',', ';'], " ")
        .split_whitespace()
        .map(|t| t.trim_matches(|c| c == '"' || c == '`' || c == '[' || c == ']').to_string())
        .collect()
}

pub fn is_ddl(sql: &str) -> bool {
    let t = sql.trim_start().to_lowercase();
    t.starts_with("create ") || t.starts_with("alter ") || t.starts_with("drop ")
}

/// UZ: DDL qaysi jadvalga tegishli (`CREATE TABLE`, `CREATE INDEX .. ON`, `ALTER TABLE`, `DROP TABLE`). EN/RU: table a DDL statement belongs to.
pub fn ddl_table(sql: &str) -> Option<String> {
    let t = tokens(sql);
    let at = |i: usize| t.get(i).map(|s| s.as_str());
    match (at(0), at(1)) {
        (Some("create"), Some("table")) | (Some("drop"), Some("table")) => {
            let mut i = 2;
            while matches!(at(i), Some("if") | Some("not") | Some("exists")) { i += 1; }
            at(i).map(|s| s.to_string())
        }
        (Some("create"), Some("index")) | (Some("create"), Some("unique")) => {
            let on = t.iter().position(|x| x == "on")?;
            at(on + 1).map(|s| s.to_string())
        }
        (Some("alter"), Some("table")) => at(2).map(|s| s.to_string()),
        _ => None,
    }
}

/// UZ: DML qaysi jadvalga YOZADI (`INSERT [OR x] INTO t`, `REPLACE INTO t`, `UPDATE [OR x] t`, `DELETE FROM t`). EN/RU: write target table.
pub fn write_target(sql: &str) -> Option<String> {
    let t = tokens(sql);
    let at = |i: usize| t.get(i).map(|s| s.as_str());
    match at(0)? {
        "insert" => {
            let mut i = 1;
            if at(i) == Some("or") { i += 2; }
            if at(i) == Some("into") { at(i + 1).map(|s| s.to_string()) } else { None }
        }
        "replace" => if at(1) == Some("into") { at(2).map(|s| s.to_string()) } else { None },
        "update" => {
            let mut i = 1;
            if at(i) == Some("or") { i += 2; }
            at(i).map(|s| s.to_string())
        }
        "delete" => if at(1) == Some("from") { at(2).map(|s| s.to_string()) } else { None },
        _ => None,
    }
}

fn is_word_char(c: char) -> bool { c.is_ascii_alphanumeric() || c == '_' }

/// UZ: SQL ichida qaysi ma'lum jadval nomlari so'z sifatida uchraydi. EN/RU: known table names appearing as whole words.
pub fn tables_in(sql: &str, known: &BTreeSet<String>) -> BTreeSet<String> {
    let low = sql.to_lowercase();
    let mut out = BTreeSet::new();
    for name in known {
        let mut from = 0;
        while let Some(pos) = low[from..].find(name.as_str()) {
            let a = from + pos;
            let b = a + name.len();
            let before = low[..a].chars().next_back().map_or(false, is_word_char);
            let after = low[b..].chars().next().map_or(false, is_word_char);
            if !before && !after { out.insert(name.clone()); break; }
            from = a + 1;
        }
    }
    out
}

/// UZ: tartibni saqlaydigan kalit: butun son -> 20 xonali, matn -> o'zi; murakkab kalit `\u{1f}` bilan. EN/RU: order-preserving row key.
pub fn enc_key_part(v: &Value) -> String {
    match v {
        Value::Number(n) => match n.as_i64() {
            Some(i) => format!("{:020}", (i as i128 + (1i128 << 63)) as u128),
            None => format!("{}", n),
        },
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

pub fn row_key(obj: &serde_json::Map<String, Value>, pk: &[String]) -> String {
    if pk.is_empty() {
        return obj.get("__rid").map(enc_key_part).unwrap_or_default();
    }
    pk.iter().map(|c| enc_key_part(obj.get(c).unwrap_or(&Value::Null))).collect::<Vec<_>>().join("\u{1f}")
}

/// UZ: eski va yangi suratlar farqidan amallar. EN/RU: ops from the difference of two snapshots.
pub fn diff(old: &BTreeMap<String, Value>, new: &BTreeMap<String, Value>) -> Vec<Op> {
    let mut ops = Vec::new();
    for (k, v) in new {
        if old.get(k) != Some(v) { ops.push(Op::Put(k.clone(), v.clone())); }
    }
    for k in old.keys() {
        if !new.contains_key(k) { ops.push(Op::Del(k.clone())); }
    }
    ops
}

/// UZ: xotira SQLite natijasi -> Turso javob shakli (`cols`, `rows` hujayralari `{type,value}`). EN/RU: result in Turso's JSON shape.
pub fn to_turso(cols: &[String], rows: &[Vec<Value>], changes: i64, is_write: bool) -> Value {
    let cells: Vec<Value> = rows.iter().map(|r| {
        Value::Array(r.iter().map(|v| match v {
            Value::Null => json!({"type": "null"}),
            Value::Number(n) if n.is_i64() => json!({"type": "integer", "value": n.to_string()}),
            Value::Number(n) => json!({"type": "float", "value": n.as_f64().unwrap_or(0.0)}),
            Value::String(s) => json!({"type": "text", "value": s}),
            other => json!({"type": "text", "value": other.to_string()}),
        }).collect())
    }).collect();
    json!({
        "cols": cols.iter().map(|c| json!({"name": c})).collect::<Vec<_>>(),
        "rows": cells,
        "affected_row_count": changes,
        "rows_read": rows.len(),
        "rows_written": if is_write { changes } else { 0 },
    })
}

// ── Ishlash (faqat wasm32) / Execution (wasm32 only) / Vypolnenie ────────────────────────────────────────────────────────

fn known_tables() -> BTreeSet<String> {
    SCHEMA.with(|s| s.borrow().iter().map(|(t, _)| t.clone()).collect())
}

#[cfg(target_arch = "wasm32")]
fn param(a: &TursoArg) -> Param {
    match (a.type_.as_str(), a.value.as_deref()) {
        ("null", _) | (_, None) => Param::Null,
        ("integer", Some(v)) => v.parse::<i64>().map(Param::Int).unwrap_or(Param::Text(v.to_string())),
        ("float", Some(v)) => v.parse::<f64>().map(Param::Real).unwrap_or(Param::Text(v.to_string())),
        (_, Some(v)) => Param::Text(v.to_string()),
    }
}

#[cfg(target_arch = "wasm32")]
fn vparam(v: &Value) -> Param {
    match v {
        Value::Null => Param::Null,
        Value::Bool(b) => Param::Int(*b as i64),
        Value::Number(n) => n.as_i64().map(Param::Int).unwrap_or_else(|| Param::Real(n.as_f64().unwrap_or(0.0))),
        Value::String(s) => Param::Text(s.clone()),
        other => Param::Text(other.to_string()),
    }
}

fn db_err(e: impl std::fmt::Display) -> Error { Error::RustError(format!("baza xatosi: {e}")) }

#[cfg(target_arch = "wasm32")]
fn replay_schema(m: &Mem, tables: &BTreeSet<String>) -> Result<()> {
    let ddl: Vec<String> = SCHEMA.with(|s| s.borrow().iter().filter(|(t, _)| tables.contains(t)).map(|(_, d)| d.clone()).collect());
    for d in ddl { m.exec(&d, &[]).map_err(db_err)?; }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn pk_cols(m: &Mem, table: &str) -> Result<Vec<String>> {
    let r = m.exec(&format!("SELECT name FROM pragma_table_info('{table}') WHERE pk>0 ORDER BY pk"), &[]).map_err(db_err)?;
    Ok(r.rows.iter().filter_map(|x| x[0].as_str().map(|s| s.to_string())).collect())
}

#[cfg(target_arch = "wasm32")]
fn load_rows(m: &Mem, table: &str, rows: &[(String, Value)]) -> Result<()> {
    for (_, v) in rows {
        let Some(o) = v.as_object() else { continue };
        let cols: Vec<String> = o.keys().map(|c| if c == "__rid" { "rowid".to_string() } else { format!("\"{c}\"") }).collect();
        let marks = vec!["?"; cols.len()].join(",");
        let ps: Vec<Param> = o.values().map(vparam).collect();
        m.exec(&format!("INSERT OR REPLACE INTO \"{table}\"({}) VALUES({marks})", cols.join(",")), &ps).map_err(db_err)?;
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn dump(m: &Mem, table: &str, has_pk: bool) -> Result<BTreeMap<String, Value>> {
    let sql = if has_pk { format!("SELECT * FROM \"{table}\"") } else { format!("SELECT rowid AS __rid, * FROM \"{table}\"") };
    let r = m.exec(&sql, &[]).map_err(db_err)?;
    let pk = if has_pk { pk_cols(m, table)? } else { vec![] };
    let mut out = BTreeMap::new();
    for row in &r.rows {
        let mut o = serde_json::Map::new();
        for (c, v) in r.cols.iter().zip(row.iter()) { o.insert(c.clone(), v.clone()); }
        out.insert(row_key(&o, &pk), Value::Object(o));
    }
    Ok(out)
}

/// UZ: buyruqlar paketini bajaradi (Turso `pipeline` o'rniga). Har buyruqqa bitta Turso-shakldagi natija. EN/RU: runs a batch of statements.
#[cfg(target_arch = "wasm32")]
pub async fn run(env: &Env, stmts: &[(&str, Vec<TursoArg>)]) -> Result<Vec<Value>> {
    let ddl_only = DDL_ONLY.with(|d| *d.borrow());
    if ddl_only {
        let mut out = Vec::new();
        for (sql, _) in stmts { out.push(if is_ddl(sql) { run_ddl(sql)? } else { to_turso(&[], &[], 0, false) }); }
        return Ok(out);
    }
    if !stmts.iter().any(|(s, _)| is_ddl(s)) {
        return run_dml(env, stmts).await;
    }
    // UZ: DDL — sxemaga (bo'laklarga tegmaydi); aralash paketda DML alohida bajariladi. EN/RU: DDL goes to the schema; DML in a mixed batch runs one by one.
    let mut out = Vec::new();
    for (sql, args) in stmts {
        if is_ddl(sql) { out.push(run_ddl(sql)?); }
        else { out.push(run_dml(env, &[(*sql, args.clone())]).await?.remove(0)); }
    }
    Ok(out)
}

#[cfg(target_arch = "wasm32")]
async fn run_dml(env: &Env, stmts: &[(&str, Vec<TursoArg>)]) -> Result<Vec<Value>> {
    let known = known_tables();
    let mut involved: BTreeSet<String> = BTreeSet::new();
    let mut writes: BTreeSet<String> = BTreeSet::new();
    for (sql, _) in stmts {
        involved.extend(tables_in(sql, &known));
        if let Some(t) = write_target(sql) {
            if known.contains(&t) { writes.insert(t); }
        }
    }
    for _attempt in 0..5 {
        let m = Mem::open().map_err(db_err)?;
        replay_schema(&m, &involved)?;
        let snaps = futures_util::future::join_all(involved.iter().map(|t| crate::kvdb::snapshot(env, t, !writes.is_empty()))).await;
        let mut revs: BTreeMap<String, u64> = BTreeMap::new();
        let mut olds: BTreeMap<String, BTreeMap<String, Value>> = BTreeMap::new();
        for (t, s) in involved.iter().zip(snaps) {
            let (rev, rows) = s?;
            load_rows(&m, t, &rows)?;
            revs.insert(t.clone(), rev);
            olds.insert(t.clone(), rows.into_iter().collect());
        }
        let mut results = Vec::with_capacity(stmts.len());
        for (sql, args) in stmts {
            let ps: Vec<Param> = args.iter().map(param).collect();
            let r = m.exec(sql, &ps).map_err(db_err)?;
            let is_write = write_target(sql).is_some();
            let mut v = to_turso(&r.cols, &r.rows, r.changes, is_write);
            if let Ok(id) = m.exec("SELECT last_insert_rowid()", &[]) {
                if let Some(x) = id.rows.first().and_then(|r| r.first()).and_then(|x| x.as_i64()) { v["last_insert_rowid"] = json!(x.to_string()); }
            }
            results.push(v);
        }
        // UZ: HAMMA yozilgan jadvallar bitta atomik tranzaksiyada (ro'yxat Supabase'da). EN/RU: all written tables in one atomic commit.
        let mut items: Vec<crate::kvdb::TableOps> = Vec::new();
        for t in &writes {
            let has_pk = !pk_cols(&m, t)?.is_empty();
            let new = dump(&m, t, has_pk)?;
            let ops = diff(&olds[t], &new);
            if !ops.is_empty() {
                let (p, d) = (ops.iter().filter(|o| matches!(o, Op::Put(..))).count(), ops.iter().filter(|o| matches!(o, Op::Del(..))).count());
                console_log!("tgs_w table={} rows_before={} put={} del={} rev={} attempt={}", t, olds[t].len(), p, d, revs[t], _attempt);
                items.push(crate::kvdb::TableOps { table: t.clone(), ops, expect: Some(revs[t]) });
            }
        }
        if !crate::kvdb::commit_multi(env, items).await? { console_log!("tgs_conflict attempt={}", _attempt); continue; }
        let sqls: Vec<&str> = stmts.iter().map(|(s, _)| *s).collect();
        crate::turso_after(env, &sqls, &results).await;
        return Ok(results);
    }
    Err(Error::RustError("baza xatosi: yozish to'qnashuvi (5 urinish)".into()))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn run(_env: &Env, _stmts: &[(&str, Vec<TursoArg>)]) -> Result<Vec<Value>> {
    Err(Error::RustError("kvsql faqat WASM da".into()))
}

#[cfg(target_arch = "wasm32")]
fn run_ddl(sql: &str) -> Result<Value> {
    let Some(table) = ddl_table(sql) else { return Ok(to_turso(&[], &[], 0, false)) };
    if SCHEMA.with(|s| s.borrow().iter().any(|(_, d)| d == sql)) { return Ok(to_turso(&[], &[], 0, false)); }
    let tset: BTreeSet<String> = [table.clone()].into_iter().collect();
    let m = Mem::open().map_err(db_err)?;
    replay_schema(&m, &tset)?;
    m.exec(sql, &[]).map_err(db_err)?;
    let low = sql.trim_start().to_lowercase();
    SCHEMA.with(|s| {
        let mut s = s.borrow_mut();
        if low.starts_with("drop table") { s.retain(|(t, _)| t != &table); } else { s.push((table, sql.to_string())); }
    });
    Ok(to_turso(&[], &[], 0, false))
}

/// UZ: izolyat boshida sxemani tiklaydi (`_schema` jadvalidan) yoki `init_db` ni ishga tushirib saqlaydi. EN/RU: restores/creates the schema.
pub async fn ensure_schema(env: &Env, mark: &str, init: impl std::future::Future<Output = bool>) -> bool {
    if READY.with(|r| *r.borrow()) { return true; }
    if let Ok(Some(v)) = crate::kvdb::get(env, "_schema", "ddl").await {
        if v["hash"].as_str() == Some(mark) {
            if let Ok(list) = serde_json::from_value::<Vec<(String, String)>>(v["ddl"].clone()) {
                SCHEMA.with(|s| *s.borrow_mut() = list);
                READY.with(|r| *r.borrow_mut() = true);
                return true;
            }
        }
    }
    // UZ: yangi kod/sxema — init_db DDL larni `run` orqali sxemaga yozadi, keyin saqlaymiz. EN/RU: first run for this code version.
    SCHEMA.with(|s| s.borrow_mut().clear());
    DDL_ONLY.with(|d| *d.borrow_mut() = true);
    let ok = init.await;
    DDL_ONLY.with(|d| *d.borrow_mut() = false);
    if ok {
        let ddl = SCHEMA.with(|s| s.borrow().clone());
        let row = json!({"hash": mark, "ddl": ddl});
        let _ = crate::kvdb::commit(env, "_schema", vec![Op::Put("ddl".into(), row)]).await;
        READY.with(|r| *r.borrow_mut() = true);
    }
    ok
}

/// UZ: `/__kv/sqltest?phase=...` — haqiqiy production sxemasi va `turso_*` chaqiruvlari bo'laklar ustida. Bosqichlarga bo'lingan: Cloudflare bitta
///     so'rovda cheklangan miqdorda ichki chaqiruv (subrequest) beradi. Bosqichlar: init, write, update, config, delete.
/// EN/RU: end-to-end test of the SQL bridge, split into phases to stay under the per-invocation subrequest limit.
pub async fn sqltest(env: &Env, phase: &str) -> Result<Value> {
    crate::ensure_db(env).await;
    let mut steps: Vec<Value> = Vec::new();
    let mut rec = |name: &str, ok: bool, note: String| steps.push(json!({"step": name, "ok": ok, "note": note}));
    let base = 900_000i64;
    let range = || vec![TursoArg::int(base), TursoArg::int(base + 1000)];
    match phase {
        "init" => {
            let m = crate::kvdb::meta(env, "_schema").await?;
            rec("sxema saqlangan (_schema)", !m.chunks.is_empty(), format!("{} bo'lak", m.chunks.len()));
        }
        "write" => {
            let pad = "x".repeat(6000);
            let mut ins: Vec<(String, Vec<TursoArg>)> = Vec::new();
            for i in 0..250i64 {
                ins.push(("INSERT INTO anime_db(id,name,davlat,tavsif) VALUES(?,?,?,?)".to_string(),
                    vec![TursoArg::int(base + i), TursoArg::text(&format!("Test anime {i} ўзбек")), TursoArg::text("Xitoy"), TursoArg::text(&pad)]));
            }
            let refs: Vec<(&str, Vec<TursoArg>)> = ins.iter().map(|(s, a)| (s.as_str(), a.clone())).collect();
            let w = crate::turso_batch(env, &refs).await;
            rec("250 qator INSERT (bitta paket)", w.is_ok(), format!("{:?}", w.as_ref().err().map(|e| e.to_string())));
            let c = crate::turso_exec(env, "SELECT COUNT(*) FROM anime_db WHERE id>=? AND id<?", range()).await;
            rec("COUNT = 250", matches!(&c, Ok(v) if crate::scalar(v) == 250), format!("{:?}", c.as_ref().map(|v| crate::scalar(v)).map_err(|e| e.to_string())));
            let m = crate::kvdb::meta(env, "anime_db").await?;
            rec("anime_db bo'laklarga bo'lingan (>=2)", m.chunks.len() >= 2, format!("{} bo'lak", m.chunks.len()));
            let l = crate::turso_exec(env, "SELECT id,name FROM anime_db WHERE name LIKE ? AND id>=? ORDER BY id DESC LIMIT 3", vec![TursoArg::text("Test anime%ўзбек"), TursoArg::int(base)]).await;
            let ok_l = matches!(&l, Ok(v) if v["rows"].as_array().map_or(false, |r| r.len() == 3) && v["rows"][0][0]["value"] == json!((base + 249).to_string()));
            rec("LIKE + ORDER BY DESC LIMIT", ok_l, String::new());
        }
        "update" => {
            let u = crate::turso_exec(env, "UPDATE anime_db SET davlat=? WHERE id>=? AND id<?", vec![TursoArg::text("Yaponiya"), TursoArg::int(base), TursoArg::int(base + 10)]).await;
            rec("UPDATE 10 qator", matches!(&u, Ok(v) if v["rows_written"] == json!(10)), format!("{:?}", u.as_ref().map(|v| v["rows_written"].clone()).map_err(|e| e.to_string())));
            let g = crate::turso_exec(env, "SELECT davlat, COUNT(*) c FROM anime_db WHERE id>=? AND id<? GROUP BY davlat ORDER BY c DESC", range()).await;
            let ok_g = matches!(&g, Ok(v) if v["rows"][0][0]["value"] == json!("Xitoy") && v["rows"][0][1]["value"] == json!("240") && v["rows"][1][1]["value"] == json!("10"));
            rec("GROUP BY natijasi 240/10", ok_g, format!("{:?}", g.as_ref().map(|v| v["rows"].to_string()).map_err(|e| e.to_string())));
            let j = crate::turso_exec(env, "SELECT COUNT(*) FROM anime_db a JOIN anime_db b ON a.id=b.id WHERE a.id>=? AND a.id<?", range()).await;
            rec("JOIN (o'zi bilan)", matches!(&j, Ok(v) if crate::scalar(v) == 250), String::new());
        }
        "config" => {
            crate::config_put(env, "kv_selftest", "salom").await;
            let cg = crate::config_get_db(env, "kv_selftest").await;
            rec("app_config yozish/o'qish", cg.as_deref() == Some("salom"), format!("{cg:?}"));
            let _ = crate::turso_exec(env, "DELETE FROM app_config WHERE cfg_key=?", vec![TursoArg::text("kv_selftest")]).await;
            let cg2 = crate::config_get_db(env, "kv_selftest").await;
            rec("app_config o'chirish", cg2.is_none(), format!("{cg2:?}"));
        }
        "delete" => {
            let d = crate::turso_exec(env, "DELETE FROM anime_db WHERE id>=? AND id<?", vec![TursoArg::int(base + 240), TursoArg::int(base + 1000)]).await;
            let c2 = crate::turso_exec(env, "SELECT COUNT(*) FROM anime_db WHERE id>=? AND id<?", range()).await;
            rec("DELETE 10 -> 240 qoladi", d.is_ok() && matches!(&c2, Ok(v) if crate::scalar(v) == 240), format!("{:?}", c2.as_ref().map(|v| crate::scalar(v)).map_err(|e| e.to_string())));
            let _ = crate::turso_exec(env, "DELETE FROM anime_db WHERE id>=? AND id<?", range()).await;
            let c3 = crate::turso_exec(env, "SELECT COUNT(*) FROM anime_db WHERE id>=? AND id<?", range()).await;
            rec("tozalash: 0 qator", matches!(&c3, Ok(v) if crate::scalar(v) == 0), String::new());
        }
        other => rec("noma'lum bosqich", false, other.to_string()),
    }
    let ok = steps.iter().all(|s| s["ok"] == json!(true));
    Ok(json!({"ok": ok, "phase": phase, "steps": steps}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known() -> BTreeSet<String> { ["anime_db", "season_db", "epizod_db", "users_db", "app_config"].iter().map(|s| s.to_string()).collect() }

    #[test]
    fn ddl_tables() {
        assert_eq!(ddl_table("CREATE TABLE IF NOT EXISTS anime_db (id INTEGER PRIMARY KEY)").as_deref(), Some("anime_db"));
        assert_eq!(ddl_table("create table users_db(\n id int)").as_deref(), Some("users_db"));
        assert_eq!(ddl_table("CREATE INDEX IF NOT EXISTS ix_ep ON epizod_db(anime_id, no)").as_deref(), Some("epizod_db"));
        assert_eq!(ddl_table("CREATE UNIQUE INDEX u ON users_db(email)").as_deref(), Some("users_db"));
        assert_eq!(ddl_table("ALTER TABLE epizod_db ADD COLUMN ready_at INTEGER DEFAULT 0").as_deref(), Some("epizod_db"));
        assert_eq!(ddl_table("DROP TABLE IF EXISTS old_db").as_deref(), Some("old_db"));
        assert!(is_ddl("  ALTER TABLE x ADD y") && !is_ddl("SELECT 1"));
    }

    #[test]
    fn write_targets() {
        assert_eq!(write_target("INSERT INTO anime_db(id) VALUES(?)").as_deref(), Some("anime_db"));
        assert_eq!(write_target("INSERT OR REPLACE INTO app_config(k,v) VALUES(?,?)").as_deref(), Some("app_config"));
        assert_eq!(write_target("insert or ignore into users_db(id) values(1)").as_deref(), Some("users_db"));
        assert_eq!(write_target("REPLACE INTO season_db(id) VALUES(1)").as_deref(), Some("season_db"));
        assert_eq!(write_target("UPDATE epizod_db SET no=1 WHERE id=2").as_deref(), Some("epizod_db"));
        assert_eq!(write_target("UPDATE OR IGNORE epizod_db SET no=1").as_deref(), Some("epizod_db"));
        assert_eq!(write_target("DELETE FROM users_db WHERE id=1").as_deref(), Some("users_db"));
        assert_eq!(write_target("SELECT * FROM users_db"), None);
    }

    #[test]
    fn tables_found_by_whole_word() {
        let t = tables_in("SELECT e.id FROM epizod_db e JOIN season_db s ON s.id=e.season_id WHERE e.x > 1", &known());
        assert_eq!(t.into_iter().collect::<Vec<_>>(), vec!["epizod_db", "season_db"]);
        let none = tables_in("SELECT my_anime_db_x FROM t", &known());
        assert!(none.is_empty(), "substring of another identifier must not match");
        assert_eq!(tables_in("INSERT INTO app_config(k) VALUES('anime_db')", &known()).len(), 2, "string literal mention is harmless (extra load only)");
    }

    #[test]
    fn keys_sort_like_numbers_and_support_composite() {
        let a = enc_key_part(&json!(-5));
        let b = enc_key_part(&json!(3));
        let c = enc_key_part(&json!(1000));
        assert!(a < b && b < c);
        let mut o = serde_json::Map::new();
        o.insert("anime_id".into(), json!(2));
        o.insert("no".into(), json!(10));
        let k = row_key(&o, &["anime_id".into(), "no".into()]);
        assert!(k.contains('\u{1f}'));
        o.insert("__rid".into(), json!(7));
        assert_eq!(row_key(&o, &[]), enc_key_part(&json!(7)));
    }

    #[test]
    fn diff_gives_minimal_ops() {
        let old: BTreeMap<String, Value> = [("a", json!({"v": 1})), ("b", json!({"v": 2})), ("c", json!({"v": 3}))].into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        let mut new = old.clone();
        new.insert("b".into(), json!({"v": 20}));
        new.remove("c");
        new.insert("d".into(), json!({"v": 4}));
        let ops = diff(&old, &new);
        assert_eq!(ops.len(), 3);
        assert!(ops.contains(&Op::Put("b".into(), json!({"v": 20}))));
        assert!(ops.contains(&Op::Put("d".into(), json!({"v": 4}))));
        assert!(ops.contains(&Op::Del("c".into())));
        assert!(diff(&old, &old).is_empty());
    }

    #[test]
    fn turso_shape() {
        let v = to_turso(&["id".into(), "name".into(), "x".into(), "r".into()], &[vec![json!(5), json!("Ali"), Value::Null, json!(1.5)]], 0, false);
        assert_eq!(v["cols"][1]["name"], "name");
        assert_eq!(v["rows"][0][0], json!({"type": "integer", "value": "5"}));
        assert_eq!(v["rows"][0][1], json!({"type": "text", "value": "Ali"}));
        assert_eq!(v["rows"][0][2], json!({"type": "null"}));
        assert_eq!(v["rows"][0][3]["type"], "float");
        assert_eq!(v["rows_written"], 0);
        assert_eq!(to_turso(&[], &[], 3, true)["rows_written"], 3);
    }
}
