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
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const MB: u64 = 1024 * 1024;
/// UZ: snapshot bo'lagi 18 MiB (Telegram: yuklash 50 MB gacha, Bot API orqali yuklab olish 20 MB gacha). EN: snapshot part size; fits the Bot API 20 MB download limit.
const SNAP_PART: u64 = 18 * MB;

/// UZ: jurnal bo'lagining Telegram'ga yuklangan QISMI: faylning `from..from+len` baytlari (siqilgan + shifrlangan). EN: an uploaded slice of the journal file.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Seg { pub file_id: String, pub msg_id: i64, pub key: String, pub from: u64, pub len: u64, #[serde(default)] pub z: bool }

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct ChunkMeta {
    pub no: u64, pub ver: u64, pub sealed: bool,
    #[serde(default)] pub first_seq: u64, #[serde(default)] pub last_seq: u64,
    /// Eski format (butun fayl har safar qayta yuklanardi). Yangi tizimda `segs` ishlatiladi; eski yozuv birinchi yuklashda almashtiriladi. EN: legacy whole-file upload.
    #[serde(default)] pub file_id: Option<String>, #[serde(default)] pub msg_id: Option<i64>, #[serde(default)] pub key: Option<String>,
    /// Yangi format: yuklangan bo'laklar ketma-ketligi va ularning jami baytlari (`up_len` — fayl shu joygacha zaxirada).
    #[serde(default)] pub segs: Vec<Seg>, #[serde(default)] pub up_len: u64,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SnapPart { pub file_id: String, pub msg_id: i64 }
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Snap { pub seq: u64, pub at: u64, pub key: String, pub parts: Vec<SnapPart>, #[serde(default)] pub bytes: u64, #[serde(default)] pub z: bool }

#[derive(Clone)]
pub struct Cfg {
    pub dir: PathBuf, pub chunk_bytes: u64, pub limit_mb: u64,
    /// Muhim yozuvdan keyin shuncha kutib yuklanadi (yig'ish uchun). Faqat "arzon" yozuvlar bo'lsa — `idle_ms` da. EN: debounce for important writes / max wait for cheap ones.
    pub crit_ms: u64, pub idle_ms: u64,
}

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
    /// Yuklanmagan MUHIM yozuv paydo bo'lgan vaqt (ms) va oxirgi muvaffaqiyatli yuklash vaqti. EN: first unflushed important write; last flush.
    pub crit_since: Option<u64>,
    pub last_flush: u64,
}

pub fn now_ms() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64 }

fn is_write(sql: &str) -> bool {
    let l = sql.trim_start().to_lowercase();
    ["insert", "update", "delete", "replace", "create", "drop", "alter", "vacuum", "reindex"].iter().any(|k| l.starts_with(k) && !l[k.len()..].starts_with(|c: char| c.is_alphanumeric() || c == '_'))
}

fn is_ddl(sql: &str) -> bool {
    let l = sql.trim_start().to_lowercase();
    ["create", "drop", "alter", "reindex"].iter().any(|k| l.starts_with(k) && !l[k.len()..].starts_with(|c: char| c.is_alphanumeric() || c == '_'))
}

/// Buyruq nishonidagi jadval: `(amal, jadval)` (insert/update/delete). EN: statement kind and target table.
fn target_table(sql: &str) -> Option<(&'static str, String)> {
    let l = sql.trim_start().to_lowercase();
    let w: Vec<&str> = l.split(|c: char| c.is_whitespace() || c == '(' || c == '"' || c == '`' || c == ';').filter(|x| !x.is_empty()).collect();
    let tbl = |i: usize| w.get(i).map(|x| x.to_string());
    match *w.first()? {
        "update" => { let i = if w.get(1) == Some(&"or") { 3 } else { 1 }; Some(("update", tbl(i)?)) }
        "delete" => { let i = w.iter().position(|x| *x == "from")? + 1; Some(("delete", tbl(i)?)) }
        "insert" | "replace" => { let i = w.iter().position(|x| *x == "into")? + 1; Some(("insert", tbl(i)?)) }
        _ => None,
    }
}

/// UZ: "ARZON" yozuv — yo'qolsa zarar kichik (statistika, tomosha pozitsiyasi, sessiyaning oxirgi ko'rilgan vaqti). Faqat arzon yozuvlar bo'lsa Telegram'ga kam yuklanadi.
/// EN: a cheap write (stats, watch position, session last-seen): losing it hurts little, so it is backed up less often. `JR_CHEAP` overrides the table list.
pub fn is_cheap(sql: &str) -> bool {
    let Some((kind, t)) = target_table(sql) else { return false };
    let cheap = std::env::var("JR_CHEAP").unwrap_or_else(|_| "stats_hourly,stats_daily,watch_history_db".to_string());
    cheap.split(',').any(|c| c.trim() == t) || (t == "sessions_db" && kind == "update")
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
        let mut st = Store { cfg, db, conn, seq: 0, chunks: vec![], cur: 0, dirty: BTreeSet::new(), snap: None, manifest_msg: None, gap: false, crit_since: None, last_flush: now_ms() };
        st.seq = st.scalar_i64("select v from jr_state where k='seq'").await?.max(0) as u64;
        if let Ok(b) = std::fs::read(st.cfg.dir.join("jr_snap.json")) {
            if let Ok(j) = serde_json::from_slice::<J>(&b) {
                st.snap = serde_json::from_value(j["snap"].clone()).ok();
                st.manifest_msg = j["manifest_msg"].as_i64();
            }
        }
        st.scan()?;
        // Qayta ishga tushganda yuklanmagan yozuv bo'lsa — tez orada yuklanadi.
        st.refresh_dirty();
        if !st.dirty.is_empty() { st.crit_since = Some(now_ms()); }
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
        // UZ: snapshotdan keyin jurnal bo'sh bo'ladi — bo'shliq faqat snapshotdan ham, jurnaldan ham ilgarigi yozuv bo'lsa. Aks holda har qayta ishga tushganda keraksiz snapshot yuklanardi.
        // EN: after a snapshot the journal is empty; a gap exists only if seq is ahead of both the journal and the snapshot.
        let base = top.max(self.snap.as_ref().map(|s| s.seq).unwrap_or(0));
        self.gap = self.seq > base;
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
    /// SQLite sxema versiyasi (DDL o'zgarganda oshadi).
    async fn schema_version(&self) -> i64 {
        match self.conn.query("pragma schema_version", ()).await {
            Ok(mut rows) => match rows.next().await { Ok(Some(r)) => r.get::<i64>(0).unwrap_or(0), _ => 0 },
            Err(_) => 0,
        }
    }

    /// Buyruq natijasida ma'lumot yoki sxema haqiqatan o'zgardimi.
    async fn changed(&self, tc0: u64, sv0: i64, ddl: bool) -> bool {
        self.conn.total_changes() != tc0 || (ddl && self.schema_version().await != sv0)
    }

    async fn one(&self, st: &Stmt) -> Result<(bool, J)> {
        let params: Vec<Value> = st.args.iter().map(to_value).collect::<Result<_>>()?;
        let write = is_write(&st.sql);
        // UZ: jurnalga FAQAT haqiqatan o'zgargan buyruq yoziladi (qator o'zgargan yoki sxema o'zgargan): bo'sh UPDATE va allaqachon bor `CREATE ... IF NOT EXISTS`
        // jurnal papkasini "o'zgargan" qilib, Telegram'ga qayta yuklanishiga sabab bo'lmasin. EN: journal only statements that really changed data or schema.
        let ddl = write && is_ddl(&st.sql);
        let (tc0, sv0) = if write { (self.conn.total_changes(), if ddl { self.schema_version().await } else { 0 }) } else { (0, 0) };
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
            let effective = write && self.changed(tc0, sv0, ddl).await;
            Ok((effective, json!({"cols": cols, "rows": out, "affected_row_count": cnt, "rows_read": cnt, "rows_written": if write { cnt } else { 0 }})))
        } else {
            let changes = stmt.execute(params).await?;
            let effective = write && self.changed(tc0, sv0, ddl).await;
            Ok((effective, json!({"cols": [], "rows": [], "affected_row_count": changes, "rows_read": 0, "rows_written": if write { changes } else { 0 }, "last_insert_rowid": self.conn.last_insert_rowid().to_string()})))
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
        } else { match self.conn.execute("commit", ()).await { Err(e) => { return vec![Res::Err(e.to_string())]; } _ => {}}}
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
        let critical = writes.iter().any(|w| !is_cheap(w[0].as_str().unwrap_or("")));
        if critical && self.crit_since.is_none() { self.crit_since = Some(now_ms()); }
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

    /// UZ: yangi snapshot: yo'q bo'lsa yoki zaxirada bo'shliq bo'lsa; aks holda FAQAT o'zgarish bo'lgan bo'lsa va (6 soatdan keyin jurnal hajmi snapshot hajmiga yetgan bo'lsa, yoki 7 kun o'tgan bo'lsa).
    /// Shunda umumiy trafik haqiqiy o'zgarish hajmidan ~2 baravardan oshmaydi. EN: snapshot only when changed and the journal has grown to the snapshot's size (min 6 h) or 7 days passed.
    pub fn need_snapshot(&self) -> bool {
        let Some(s) = &self.snap else { return true };
        if self.gap { return true; }
        if self.seq <= s.seq { return false; }
        let age = now_ms().saturating_sub(s.at);
        let journal: u64 = self.chunks.iter().map(|c| std::fs::metadata(c.dir.join("journal.jsonl")).map(|m| m.len()).unwrap_or(0)).sum();
        (age >= 6 * 3600_000 && journal >= s.bytes.max(MB)) || age >= 7 * 24 * 3600_000
    }

    /// Fayli `up_len` dan uzun bo'lgan (yuklanmagan baytlari bor) jurnal bo'laklari. EN: chunks with bytes not yet backed up.
    pub fn refresh_dirty(&mut self) {
        self.dirty = self.chunks.iter().filter(|c| std::fs::metadata(c.dir.join("journal.jsonl")).map(|m| m.len()).unwrap_or(0) > c.meta.up_len || (c.meta.up_len == 0 && c.meta.file_id.is_none() && c.meta.last_seq > 0)).map(|c| c.meta.no).collect();
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

/// UZ: jurnalning YANGI qismini yuklaydi (butun fayl emas), siqilgan va shifrlangan. Muhim yozuv bo'lsa `crit_ms` dan keyin, faqat arzon yozuv bo'lsa `idle_ms` dan keyin; `force` — darhol.
/// Telegram'ga: bo'lak uchun 1 so'rov + ro'yxatni yangilash uchun 1 so'rov. 24 bo'lakdan oshsa bo'lak butun fayl sifatida bitta bo'lakka birlashtiriladi.
/// EN: uploads only the NEW journal bytes (compressed + sealed); debounced by importance; compacts after 24 segments.
pub async fn replicate(st: &Shared, tg: &Tg, key: &[u8; 32], force: bool) -> Result<J> {
    struct Job { no: u64, ver: u64, from: u64, total: u64, bytes: Vec<u8>, replace: bool }
    let now = now_ms();
    let (jobs, prev_crit, need_manifest) = {
        let mut s = st.lock().await;
        s.refresh_dirty();
        let sealed_pending = s.chunks.iter().any(|c| c.meta.sealed && s.dirty.contains(&c.meta.no));
        let due = force || sealed_pending
            || s.crit_since.is_some_and(|t| now.saturating_sub(t) >= s.cfg.crit_ms)
            || (!s.dirty.is_empty() && now.saturating_sub(s.last_flush) >= s.cfg.idle_ms);
        let need_manifest = s.manifest_msg.is_none();
        if s.dirty.is_empty() && !need_manifest { return Ok(json!({"uploaded": 0, "pending": false})); }
        if !due && !need_manifest { return Ok(json!({"uploaded": 0, "pending": true})); }
        let prev = s.crit_since;
        s.crit_since = None;
        let jobs: Vec<Job> = s.dirty.iter().filter_map(|no| s.chunks.iter().find(|c| c.meta.no == *no)).filter_map(|c| {
            let all = std::fs::read(c.dir.join("journal.jsonl")).unwrap_or_default();
            let replace = c.meta.file_id.is_some() || c.meta.segs.len() >= 24;
            let from = if replace { 0 } else { c.meta.up_len } as usize;
            if all.len() <= from && !replace { return None; }
            Some(Job { no: c.meta.no, ver: c.meta.ver, from: from as u64, total: all.len() as u64, bytes: all[from.min(all.len())..].to_vec(), replace })
        }).collect();
        (jobs, prev, need_manifest)
    };
    let res: Result<(usize, Vec<i64>)> = async {
        let mut n = 0usize;
        let mut old_msgs: Vec<i64> = vec![];
        for j in jobs {
            let kh = crypto::random_key_hex();
            let sealed = crypto::seal(&key32(&kh)?, &crypto::deflate(&j.bytes));
            let (file_id, msg_id) = tg.send(&format!("db_{}_v{}_{}.bin", j.no, j.ver, j.from), sealed).await?;
            let mut s = st.lock().await;
            if let Some(c) = s.chunks.iter_mut().find(|c| c.meta.no == j.no) {
                if j.replace {
                    old_msgs.extend(c.meta.segs.iter().map(|g| g.msg_id));
                    if let Some(m) = c.meta.msg_id.take() { old_msgs.push(m); }
                    c.meta.file_id = None; c.meta.key = None; c.meta.segs.clear();
                }
                c.meta.segs.push(Seg { file_id, msg_id, key: kh, from: j.from, len: j.total - j.from, z: true });
                c.meta.up_len = j.total;
                write_meta(c);
            }
            s.refresh_dirty();
            n += 1;
        }
        Ok((n, old_msgs))
    }.await;
    match res {
        Err(e) => { let mut s = st.lock().await; if s.crit_since.is_none() { s.crit_since = Some(prev_crit.unwrap_or(now)); } Err(e) }
        Ok((n, old_msgs)) => {
            if n > 0 || need_manifest { write_manifest(st, tg, key).await?; }
            if !old_msgs.is_empty() { tg.del_many(&old_msgs).await; }
            if n > 0 { st.lock().await.last_flush = now_ms(); }
            Ok(json!({"uploaded": n, "pending": false}))
        }
    }
}

async fn write_manifest(st: &Shared, tg: &Tg, key: &[u8; 32]) -> Result<()> {
    let (m, old) = {
        let s = st.lock().await;
        let chunks: Vec<J> = s.chunks.iter().filter(|c| c.meta.file_id.is_some() || !c.meta.segs.is_empty()).map(|c| serde_json::to_value(&c.meta).unwrap()).collect();
        (json!({"v": 2, "at": now_ms(), "seq": s.seq, "snapshot": s.snap, "chunks": chunks}), s.manifest_msg)
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

/// To'liq surat: baza fayli 18 MiB bo'laklarda (har biri siqiladi va shifrlanadi) yuklanadi; suratga to'liq kirgan jurnal bo'laklari (ochiq bo'lak ham) o'chiriladi va raqamlash qaytadan `db_1_v1` dan boshlanadi.
/// EN/RU: full snapshot in 18 MiB parts, each compressed and sealed; the superseded journal chunks and old parts are deleted in batches.
pub async fn snapshot(st: &Shared, tg: &Tg, key: &[u8; 32]) -> Result<J> {
    use std::io::Read;
    let (seq, tmp) = {
        let s = st.lock().await;
        let tmp = s.cfg.dir.join("snap.tmp");
        let _ = std::fs::remove_file(&tmp);
        s.conn.execute("vacuum into ?1", vec![Value::Text(tmp.to_string_lossy().to_string())]).await?;
        (s.seq, tmp)
    };
    let kh = crypto::random_key_hex();
    let sk = key32(&kh)?;
    let mut parts = vec![];
    let mut raw_total = 0u64;
    {
        let mut f = std::fs::File::open(&tmp)?;
        let mut p = 0;
        loop {
            let mut buf = vec![0u8; SNAP_PART as usize];
            let mut got = 0usize;
            while got < buf.len() { let n = f.read(&mut buf[got..])?; if n == 0 { break; } got += n; }
            if got == 0 && p > 0 { break; }
            buf.truncate(got);
            raw_total += got as u64;
            let (file_id, msg_id) = tg.send(&format!("snap_{}_{}.bin", seq, p), crypto::seal(&sk, &crypto::deflate(&buf))).await?;
            parts.push(SnapPart { file_id, msg_id });
            p += 1;
            if got < SNAP_PART as usize { break; }
        }
    }
    let _ = std::fs::remove_file(&tmp);
    let (old_parts, del_msgs) = {
        let mut s = st.lock().await;
        let old = s.snap.take().map(|o| o.parts);
        s.snap = Some(Snap { seq, at: now_ms(), key: kh, parts: parts.clone(), bytes: raw_total, z: true });
        let mut del: Vec<i64> = vec![];
        let keep: Vec<Chunk> = std::mem::take(&mut s.chunks).into_iter().filter_map(|c| {
            if c.meta.last_seq <= seq { del.extend(c.meta.segs.iter().map(|g| g.msg_id)); if let Some(m) = c.meta.msg_id { del.push(m); } let _ = std::fs::remove_dir_all(&c.dir); None } else { Some(c) }
        }).collect();
        s.chunks = keep;
        match s.chunks.iter().rposition(|c| !c.meta.sealed) { Some(i) => s.cur = i, None => s.open_next() }
        s.gap = false; s.refresh_dirty();
        (old, del)
    };
    write_manifest(st, tg, key).await?;
    if !del_msgs.is_empty() { tg.del_many(&del_msgs).await; }
    if let Some(o) = old_parts { let ids: Vec<i64> = o.iter().map(|p| p.msg_id).collect(); tg.del_many(&ids).await; }
    Ok(json!({"seq": seq, "parts": parts.len(), "bytes": raw_total}))
}

/// Disk yo'qolsa: Telegram'dagi pin qilingan ro'yxat -> surat -> jurnallarni qayta o'ynash (yangi bo'laklar va eski butun-fayl formati ikkalasi ham o'qiladi). EN/RU: disaster recovery.
pub async fn recover(cfg: Cfg, tg: &Tg, key: &[u8; 32]) -> Result<(Store, u64)> {
    let (pmsg, pfile) = tg.get_pinned().await?.ok_or_else(|| anyhow!("Telegram'da ro'yxat (pin) topilmadi"))?;
    let manifest: J = serde_json::from_slice(&crypto::open(key, &tg.get(&pfile).await?)?)?;
    std::fs::create_dir_all(&cfg.dir)?;
    let snap: Snap = serde_json::from_value(manifest["snapshot"].clone())?;
    let sk = key32(&snap.key)?;
    let mut db = std::fs::File::create(cfg.dir.join("main.db"))?;
    for p in &snap.parts {
        let part = crypto::open(&sk, &tg.get(&p.file_id).await?)?;
        let part = if snap.z { crypto::inflate(&part)? } else { part };
        std::io::Write::write_all(&mut db, &part)?;
    }
    drop(db);
    let mut metas: Vec<ChunkMeta> = serde_json::from_value(manifest["chunks"].clone()).unwrap_or_default();
    metas.sort_by_key(|m| m.no);
    for m in &metas {
        let d = cfg.dir.join(format!("db_{}_v{}", m.no, m.ver));
        std::fs::create_dir_all(&d)?;
        let mut journal: Vec<u8> = vec![];
        if !m.segs.is_empty() {
            let mut segs = m.segs.clone();
            segs.sort_by_key(|g| g.from);
            for g in &segs {
                let part = crypto::open(&key32(&g.key)?, &tg.get(&g.file_id).await?)?;
                let part = if g.z { crypto::inflate(&part)? } else { part };
                if g.from as usize != journal.len() { return Err(anyhow!("jurnal bo'lagi tartibi buzilgan ({} != {})", g.from, journal.len())); }
                journal.extend(part);
            }
        } else if let Some(fid) = &m.file_id {
            let files = crypto::unpack(&crypto::open(&key32(m.key.as_deref().unwrap_or(""))?, &tg.get(fid).await?)?)?;
            journal = files.get("journal.jsonl").cloned().unwrap_or_default();
        }
        std::fs::write(d.join("journal.jsonl"), &journal)?;
        let mut meta = m.clone();
        meta.up_len = journal.len() as u64;
        std::fs::write(d.join("meta.json"), serde_json::to_vec(&meta)?)?;
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
