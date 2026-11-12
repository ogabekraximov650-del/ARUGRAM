//! UZ: JURNAL-BAZA: libSQL (SQLite) + `db_N_vV/` jurnal papkalari (1 MiB da aylanadi) + Telegram nusxa (jurnal bo'laklari, to'liq surat, pin qilingan ro'yxat) va tiklash.
//! EN: JOURNAL STORE: libSQL (SQLite) + rotating `db_N_vV/` journal folders + Telegram copy (chunks, snapshot, pinned manifest) and recovery.
//! RU: ЖУРНАЛЬНАЯ БАЗА: libSQL + папки-журналы db_N_vV + копия в Telegram и восстановление.
use crate::crypto::{self, key32};
use crate::tg::Tg;
use anyhow::{anyhow, Result};
use base64::Engine;
use libsql::Value;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as J};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const MB: u64 = 1024 * 1024;

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct ChunkMeta {
    pub no: u64, pub ver: u64, pub sealed: bool,
    #[serde(default)] pub first_seq: u64, #[serde(default)] pub last_seq: u64,
    #[serde(default)] pub file_id: Option<String>, #[serde(default)] pub msg_id: Option<i64>, #[serde(default)] pub key: Option<String>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SnapPart { pub file_id: String, pub msg_id: i64 }
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Snap { pub seq: u64, pub at: u64, pub key: String, pub parts: Vec<SnapPart>, #[serde(default)] pub bytes: u64 }

#[derive(Clone)]
pub struct Cfg { pub dir: PathBuf, pub chunk_bytes: u64, pub limit_mb: u64 }

pub struct Chunk { pub meta: ChunkMeta, pub dir: PathBuf }

#[derive(Deserialize, Clone)]
pub struct Stmt { pub sql: String, #[serde(default)] pub args: Vec<J> }

pub enum Res { Ok(J), Err(String) }

pub struct Store {
    pub cfg: Cfg,
    #[allow(dead_code)] db: libsql::Database,
    conn: libsql::Connection,
    pub seq: u64,
    pub chunks: Vec<Chunk>,
    cur: usize,
    pub dirty: BTreeSet<u64>,
    pub snap: Option<Snap>,
    pub manifest_msg: Option<i64>,
    pub gap: bool,
}

pub fn now_ms() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64 }

fn is_write(sql: &str) -> bool {
    let l = sql.trim_start().to_lowercase();
    ["insert", "update", "delete", "replace", "create", "drop", "alter", "vacuum", "reindex"].iter().any(|k| l.starts_with(k) && !l[k.len()..].starts_with(|c: char| c.is_alphanumeric() || c == '_'))
}

pub fn to_value(a: &J) -> Result<Value> {
    let t = a["type"].as_str().unwrap_or("null");
    let v = &a["value"];
    let s = || v.as_str().map(|x| x.to_string()).unwrap_or_else(|| v.to_string());
    Ok(match t {
        "null" => Value::Null,
        "integer" => Value::Integer(s().parse::<i64>().map_err(|_| anyhow!("integer emas: {}", s()))?),
        "float" => Value::Real(v.as_f64().or_else(|| s().parse().ok()).ok_or_else(|| anyhow!("float emas"))?),
        "blob" => Value::Blob(base64::engine::general_purpose::STANDARD.decode(a["base64"].as_str().or(v.as_str()).unwrap_or(""))?),
        _ => Value::Text(s()),
    })
}

pub fn cell(v: &Value) -> J {
    match v {
        Value::Null => json!({"type": "null"}),
        Value::Integer(i) => json!({"type": "integer", "value": i.to_string()}),
        Value::Real(f) => json!({"type": "float", "value": f}),
        Value::Text(s) => json!({"type": "text", "value": s}),
        Value::Blob(b) => json!({"type": "blob", "base64": base64::engine::general_purpose::STANDARD.encode(b)}),
    }
}

fn write_meta(c: &Chunk) { let _ = std::fs::write(c.dir.join("meta.json"), serde_json::to_vec(&c.meta).unwrap()); }

impl Store {
    pub async fn open(cfg: Cfg) -> Result<Store> {
        std::fs::create_dir_all(&cfg.dir)?;
        let db = libsql::Builder::new_local(cfg.dir.join("main.db")).build().await?;
        let conn = db.connect()?;
        let _ = conn.query("pragma journal_mode=wal", ()).await?; // qator qaytaradi. EN/RU: returns a row
        conn.execute_batch("pragma synchronous=full; pragma foreign_keys=off; create table if not exists jr_state(k text primary key, v integer); insert or ignore into jr_state values('seq',0); create table if not exists kv_marks(k text primary key, v integer not null default 0);").await?;
        let mut st = Store { cfg, db, conn, seq: 0, chunks: vec![], cur: 0, dirty: BTreeSet::new(), snap: None, manifest_msg: None, gap: false };
        st.seq = st.scalar_i64("select v from jr_state where k='seq'").await?.max(0) as u64;
        if let Ok(b) = std::fs::read(st.cfg.dir.join("jr_snap.json")) {
            if let Ok(j) = serde_json::from_slice::<J>(&b) {
                st.snap = serde_json::from_value(j["snap"].clone()).ok();
                st.manifest_msg = j["manifest_msg"].as_i64();
            }
        }
        st.scan()?;
        Ok(st)
    }

    async fn scalar_i64(&self, sql: &str) -> Result<i64> {
        let mut r = self.conn.query(sql, ()).await?;
        match r.next().await? { Some(row) => Ok(match row.get_value(0)? { Value::Integer(i) => i, _ => 0 }), None => Ok(0) }
    }

    fn scan(&mut self) -> Result<()> {
        let mut found: Vec<(u64, u64, PathBuf)> = vec![];
        for e in std::fs::read_dir(&self.cfg.dir)? {
            let e = e?;
            let n = e.file_name().to_string_lossy().to_string();
            if let Some(rest) = n.strip_prefix("db_") {
                if let Some((no, ver)) = rest.split_once("_v") { if let (Ok(no), Ok(ver)) = (no.parse(), ver.parse()) { found.push((no, ver, e.path())); } }
            }
        }
        found.sort();
        self.chunks = found.into_iter().map(|(no, ver, dir)| {
            let mut meta: ChunkMeta = std::fs::read(dir.join("meta.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(ChunkMeta { no, ver, ..Default::default() });
            if let Ok(t) = std::fs::read_to_string(dir.join("journal.jsonl")) {
                let lines: Vec<&str> = t.lines().filter(|l| !l.trim().is_empty()).collect();
                if let (Some(f), Some(l)) = (lines.first(), lines.last()) {
                    meta.first_seq = serde_json::from_str::<J>(f).ok().and_then(|j| j["q"].as_u64()).unwrap_or(0);
                    meta.last_seq = serde_json::from_str::<J>(l).ok().and_then(|j| j["q"].as_u64()).unwrap_or(0);
                }
            }
            Chunk { meta, dir }
        }).collect();
        match self.chunks.iter().rposition(|c| !c.meta.sealed) { Some(i) => self.cur = i, None => self.open_next() }
        let top = self.chunks.iter().map(|c| c.meta.last_seq).max().unwrap_or(0);
        self.gap = self.seq > top;
        Ok(())
    }

    fn open_next(&mut self) {
        let no = self.chunks.iter().map(|c| c.meta.no).max().unwrap_or(0) + 1;
        let dir = self.cfg.dir.join(format!("db_{}_v1", no));
        let _ = std::fs::create_dir_all(&dir);
        let c = Chunk { meta: ChunkMeta { no, ver: 1, ..Default::default() }, dir };
        write_meta(&c);
        self.chunks.push(c);
        self.cur = self.chunks.len() - 1;
    }

    // ── so'rovlar ──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
    async fn one(&self, st: &Stmt) -> Result<(bool, J)> {
        let params: Vec<Value> = st.args.iter().map(to_value).collect::<Result<_>>()?;
        let write = is_write(&st.sql);
        let stmt = self.conn.prepare(&st.sql).await?;
        if stmt.column_count() > 0 {
            let cols: Vec<J> = stmt.columns().iter().map(|c| json!({"name": c.name()})).collect();
            let n = cols.len();
            let mut rows = stmt.query(params).await?;
            let mut out: Vec<J> = vec![];
            while let Some(r) = rows.next().await? {
                out.push(J::Array((0..n).map(|i| r.get_value(i as i32).map(|v| cell(&v)).unwrap_or(json!({"type": "null"}))).collect()));
            }
            let cnt = out.len();
            Ok((write, json!({"cols": cols, "rows": out, "affected_row_count": cnt, "rows_read": cnt, "rows_written": if write { cnt } else { 0 }})))
        } else {
            let changes = stmt.execute(params).await?;
            Ok((write, json!({"cols": [], "rows": [], "affected_row_count": changes, "rows_read": 0, "rows_written": if write { changes } else { 0 }, "last_insert_rowid": self.conn.last_insert_rowid().to_string()})))
        }
    }

    /// Bir paket = BITTA tranzaksiya (xato bo'lsa hammasi qaytariladi). EN/RU: one batch = one transaction.
    pub async fn exec(&mut self, stmts: &[Stmt], journal: bool) -> Vec<Res> {
        let mut results = vec![];
        let mut writes: Vec<J> = vec![];
        if let Err(e) = self.conn.execute("begin immediate", ()).await { return vec![Res::Err(e.to_string())]; }
        for st in stmts {
            match self.one(st).await {
                Ok((w, r)) => { results.push(Res::Ok(r)); if w { writes.push(json!([st.sql, st.args])); } }
                Err(e) => { let _ = self.conn.execute("rollback", ()).await; return vec![Res::Err(e.to_string())]; }
            }
        }
        let wrote = !writes.is_empty();
        if wrote {
            let ns = self.seq + 1;
            if let Err(e) = self.conn.execute("update jr_state set v=?1 where k='seq'", vec![Value::Integer(ns as i64)]).await { let _ = self.conn.execute("rollback", ()).await; return vec![Res::Err(e.to_string())]; }
            if let Err(e) = self.conn.execute("commit", ()).await { let _ = self.conn.execute("rollback", ()).await; return vec![Res::Err(e.to_string())]; }
            self.seq = ns;
            if journal { self.append(ns, writes); }
        } else if let Err(e) = self.conn.execute("commit", ()).await { return vec![Res::Err(e.to_string())]; }
        results
    }

    fn append(&mut self, q: u64, writes: Vec<J>) {
        use std::io::Write;
        let chunk_bytes = self.cfg.chunk_bytes;
        let c = &mut self.chunks[self.cur];
        let line = json!({"q": q, "t": now_ms(), "s": writes}).to_string();
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(c.dir.join("journal.jsonl")) { let _ = writeln!(f, "{}", line); }
        if c.meta.first_seq == 0 { c.meta.first_seq = q; }
        c.meta.last_seq = q;
        self.dirty.insert(c.meta.no);
        let size = std::fs::metadata(c.dir.join("journal.jsonl")).map(|m| m.len()).unwrap_or(0);
        if size >= chunk_bytes { c.meta.sealed = true; write_meta(c); self.open_next(); }
    }

    /// Faqat o'qish: ustun nomlari va xom qiymatlar (jurnalga yozilmaydi). EN/RU: read-only query returning raw values.
    pub async fn query_raw(&self, sql: &str, args: Vec<Value>) -> Result<(Vec<String>, Vec<Vec<Value>>)> {
        let stmt = self.conn.prepare(sql).await?;
        let cols: Vec<String> = stmt.columns().iter().map(|c| c.name().to_string()).collect();
        let n = cols.len();
        let mut rows = stmt.query(args).await?;
        let mut out = vec![];
        while let Some(r) = rows.next().await? { out.push((0..n).map(|i| r.get_value(i as i32).unwrap_or(Value::Null)).collect()); }
        Ok((cols, out))
    }

    pub async fn marks(&self, bump: &[String], get: &[String], at: i64) -> Result<J> {
        for k in bump {
            self.conn.execute("insert into kv_marks(k,v) values(?1,?2) on conflict(k) do update set v=max(excluded.v, kv_marks.v+1)", vec![Value::Text(k.clone()), Value::Integer(at.max(1))]).await?;
        }
        let mut v = serde_json::Map::new();
        for k in get {
            let mut r = self.conn.query("select v from kv_marks where k=?1", vec![Value::Text(k.clone())]).await?;
            if let Some(row) = r.next().await? { if let Value::Integer(i) = row.get_value(0)? { v.insert(k.clone(), json!(i)); } }
        }
        Ok(json!({"v": v}))
    }

    fn dir_bytes(p: &Path) -> u64 {
        std::fs::read_dir(p).map(|d| d.filter_map(|e| e.ok()).map(|e| { let m = e.metadata(); match m { Ok(m) if m.is_dir() => Self::dir_bytes(&e.path()), Ok(m) => m.len(), _ => 0 } }).sum()).unwrap_or(0)
    }

    /// UZ: bazaning JONLI hajmi: ishlatilayotgan sahifalar + jurnal bo'laklari (bo'sh sahifalar va WAL hisobga olinmaydi). EN/RU: live data size (free pages and WAL excluded).
    pub async fn live_bytes(&self) -> u64 {
        let pc = self.scalar_i64("pragma page_count").await.unwrap_or(0).max(0) as u64;
        let fl = self.scalar_i64("pragma freelist_count").await.unwrap_or(0).max(0) as u64;
        let ps = self.scalar_i64("pragma page_size").await.unwrap_or(4096).max(0) as u64;
        let journals: u64 = self.chunks.iter().map(|c| Self::dir_bytes(&c.dir)).sum();
        pc.saturating_sub(fl) * ps + journals
    }

    pub fn usage(&self) -> J {
        let used = Self::dir_bytes(&self.cfg.dir);
        let limit = self.cfg.limit_mb * MB;
        json!({"used_bytes": used, "limit_bytes": limit, "percent": if limit > 0 { used as f64 / limit as f64 * 100.0 } else { 0.0 }, "top": [], "seq": self.seq, "chunks": self.chunks.len()})
    }

    pub fn save_snap_meta(&self) {
        let _ = std::fs::write(self.cfg.dir.join("jr_snap.json"), json!({"snap": self.snap, "manifest_msg": self.manifest_msg}).to_string());
    }

    pub fn need_snapshot(&self) -> bool {
        match &self.snap { None => true, Some(s) => self.gap || self.seq.saturating_sub(s.seq) > 5000 || now_ms().saturating_sub(s.at) > 24 * 3600_000 }
    }

    /// Sxemani qo'llaydi (mavjud jadval/ustun xatolari e'tiborsiz). EN/RU: apply the bootstrap schema.
    pub async fn apply_schema(&mut self, schema: &str) -> Result<usize> {
        let mut n = 0;
        for q in schema.split(";\n").map(|s| s.trim()).filter(|s| !s.is_empty()) {
            match self.exec(&[Stmt { sql: q.to_string(), args: vec![] }], true).await.remove(0) {
                Res::Err(e) if !(e.contains("already exists") || e.contains("duplicate column")) => return Err(anyhow!("sxema: {}", e)),
                _ => n += 1,
            }
        }
        Ok(n)
    }
}

// ── Telegram nusxasi (qulf tashqarisida: Arc<Mutex<Store>> bilan chaqiriladi) ──────────────────────────────────────────────────────────────

pub type Shared = std::sync::Arc<tokio::sync::Mutex<Store>>;

/// O'zgargan jurnal bo'laklarini yuklaydi (eski xabar o'chadi) va pin qilingan ro'yxatni yangilaydi. EN/RU: replicate dirty chunks + manifest.
pub async fn replicate(st: &Shared, tg: &Tg, key: &[u8; 32]) -> Result<J> {
    let jobs: Vec<(u64, Vec<u8>, Option<i64>, u64)> = {
        let s = st.lock().await;
        s.dirty.iter().filter_map(|no| s.chunks.iter().find(|c| c.meta.no == *no).map(|c| {
            let mut f = BTreeMap::new();
            f.insert("journal.jsonl".to_string(), std::fs::read(c.dir.join("journal.jsonl")).unwrap_or_default());
            (*no, crypto::pack(&f), c.meta.msg_id, c.meta.ver)
        })).collect()
    };
    let mut n = 0;
    for (no, plain, old, ver) in jobs {
        let kh = crypto::random_key_hex();
        let sealed = crypto::seal(&key32(&kh)?, &plain);
        let (file_id, msg_id) = tg.send(&format!("db_{}_v{}.bin", no, ver), sealed).await?;
        {
            let mut s = st.lock().await;
            if let Some(c) = s.chunks.iter_mut().find(|c| c.meta.no == no) { c.meta.file_id = Some(file_id); c.meta.msg_id = Some(msg_id); c.meta.key = Some(kh); write_meta(c); }
            s.dirty.remove(&no);
        }
        n += 1;
        if let Some(o) = old { tg.del(o).await; }
    }
    let need = { st.lock().await.manifest_msg.is_none() };
    if n > 0 || need { write_manifest(st, tg, key).await?; }
    Ok(json!({"uploaded": n}))
}

async fn write_manifest(st: &Shared, tg: &Tg, key: &[u8; 32]) -> Result<()> {
    let (m, old) = {
        let s = st.lock().await;
        let chunks: Vec<J> = s.chunks.iter().filter(|c| c.meta.file_id.is_some()).map(|c| serde_json::to_value(&c.meta).unwrap()).collect();
        (json!({"v": 1, "at": now_ms(), "seq": s.seq, "snapshot": s.snap, "chunks": chunks}), s.manifest_msg)
    };
    let data = crypto::seal(key, m.to_string().as_bytes());
    // UZ: ro'yxat bir marta pin qilinadi, keyin joyida almashtiriladi (kanalda "pin qildi" xabarlari to'planmaydi). EN/RU: pin once, then edit in place.
    if let Some(o) = old {
        if tg.edit(o, "manifest.bin", data.clone()).await.is_ok() { st.lock().await.save_snap_meta(); return Ok(()); }
    }
    let (_, msg) = tg.send("manifest.bin", data).await?;
    tg.pin(msg).await;
    if let Some(o) = old { tg.del(o).await; }
    let mut s = st.lock().await;
    s.manifest_msg = Some(msg);
    s.save_snap_meta();
    Ok(())
}

/// To'liq surat: baza fayli 1 MB bo'laklarda yuklanadi; suratga to'liq kirgan jurnal bo'laklari (ochiq bo'lak ham) o'chiriladi va raqamlash qaytadan `db_1_v1` dan boshlanadi. EN/RU: full snapshot in 1 MB parts.
pub async fn snapshot(st: &Shared, tg: &Tg, key: &[u8; 32]) -> Result<J> {
    let (seq, buf) = {
        let s = st.lock().await;
        let tmp = s.cfg.dir.join("snap.tmp");
        let _ = std::fs::remove_file(&tmp);
        s.conn.execute("vacuum into ?1", vec![Value::Text(tmp.to_string_lossy().to_string())]).await?;
        let b = std::fs::read(&tmp)?; let _ = std::fs::remove_file(&tmp);
        (s.seq, b)
    };
    let kh = crypto::random_key_hex();
    let sk = key32(&kh)?;
    let mut parts = vec![];
    let mut i = 0usize; let mut p = 0;
    loop {
        let end = (i + MB as usize).min(buf.len());
        let (file_id, msg_id) = tg.send(&format!("snap_{}_{}.bin", seq, p), crypto::seal(&sk, &buf[i..end])).await?;
        parts.push(SnapPart { file_id, msg_id });
        i = end; p += 1;
        if i >= buf.len() { break; }
    }
    let (old_parts, del_chunks) = {
        let mut s = st.lock().await;
        let old = s.snap.take().map(|o| o.parts);
        s.snap = Some(Snap { seq, at: now_ms(), key: kh, parts: parts.clone(), bytes: buf.len() as u64 });
        let mut del = vec![];
        let keep: Vec<Chunk> = std::mem::take(&mut s.chunks).into_iter().filter_map(|c| {
            if c.meta.last_seq <= seq { del.push((c.meta.msg_id, c.dir.clone())); None } else { Some(c) }
        }).collect();
        s.chunks = keep;
        for (_, d) in &del { let _ = std::fs::remove_dir_all(d); }
        match s.chunks.iter().rposition(|c| !c.meta.sealed) { Some(i) => s.cur = i, None => s.open_next() }
        let live: BTreeSet<u64> = s.chunks.iter().map(|c| c.meta.no).collect();
        s.gap = false; s.dirty.retain(|no| live.contains(no));
        (old, del)
    };
    for (m, _) in del_chunks { if let Some(m) = m { tg.del(m).await; } }
    write_manifest(st, tg, key).await?;
    if let Some(o) = old_parts { for p in o { tg.del(p.msg_id).await; } }
    Ok(json!({"seq": seq, "parts": parts.len(), "bytes": buf.len()}))
}

/// Disk yo'qolsa: Telegram'dagi pin qilingan ro'yxat -> surat -> jurnallarni qayta o'ynash. EN/RU: disaster recovery.
pub async fn recover(cfg: Cfg, tg: &Tg, key: &[u8; 32]) -> Result<(Store, u64)> {
    let (pmsg, pfile) = tg.get_pinned().await?.ok_or_else(|| anyhow!("Telegram'da ro'yxat (pin) topilmadi"))?;
    let manifest: J = serde_json::from_slice(&crypto::open(key, &tg.get(&pfile).await?)?)?;
    std::fs::create_dir_all(&cfg.dir)?;
    let snap: Snap = serde_json::from_value(manifest["snapshot"].clone())?;
    let sk = key32(&snap.key)?;
    let mut db = vec![];
    for p in &snap.parts { db.extend(crypto::open(&sk, &tg.get(&p.file_id).await?)?); }
    std::fs::write(cfg.dir.join("main.db"), db)?;
    let mut metas: Vec<ChunkMeta> = serde_json::from_value(manifest["chunks"].clone()).unwrap_or_default();
    metas.sort_by_key(|m| m.no);
    for m in &metas {
        let d = cfg.dir.join(format!("db_{}_v{}", m.no, m.ver));
        std::fs::create_dir_all(&d)?;
        let files = crypto::unpack(&crypto::open(&key32(m.key.as_deref().unwrap_or(""))?, &tg.get(m.file_id.as_deref().unwrap_or("")).await?)?)?;
        std::fs::write(d.join("journal.jsonl"), files.get("journal.jsonl").cloned().unwrap_or_default())?;
        std::fs::write(d.join("meta.json"), serde_json::to_vec(m)?)?;
    }
    let mut st = Store::open(cfg).await?;
    let mut lines: Vec<J> = vec![];
    for c in &st.chunks {
        if let Ok(t) = std::fs::read_to_string(c.dir.join("journal.jsonl")) { for l in t.lines().filter(|l| !l.trim().is_empty()) { if let Ok(j) = serde_json::from_str::<J>(l) { lines.push(j); } } }
    }
    lines.sort_by_key(|j| j["q"].as_u64().unwrap_or(0));
    let mut replayed = 0u64;
    for e in lines {
        let q = e["q"].as_u64().unwrap_or(0);
        if q <= st.seq { continue; }
        let stmts: Vec<Stmt> = e["s"].as_array().cloned().unwrap_or_default().iter().map(|p| Stmt { sql: p[0].as_str().unwrap_or("").to_string(), args: p[1].as_array().cloned().unwrap_or_default() }).collect();
        if let Some(Res::Err(er)) = st.exec(&stmts, false).await.into_iter().find(|r| matches!(r, Res::Err(_))) { return Err(anyhow!("jurnalni qayta o'ynash {}: {}", q, er)); }
        st.conn.execute("update jr_state set v=?1 where k='seq'", vec![Value::Integer(q as i64)]).await?;
        st.seq = q; replayed += 1;
    }
    st.snap = Some(snap); st.manifest_msg = Some(pmsg); st.save_snap_meta();
    Ok((st, replayed))
}
