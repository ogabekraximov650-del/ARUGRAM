// UZ: libsql (SQLite) baza: kv sozlamalari va yuklash navbati. EN: libsql (SQLite) database: settings and the download queue.
use anyhow::Result;
use libsql::{params, Connection};

#[derive(Clone, Debug)]
pub struct Job {
    pub id: i64,
    pub chat_id: i64,
    pub url: String,
    pub title: String,
    pub meta: String,
    pub fmt: String,
    pub status_msg: i64,
}

pub async fn open(path: &str) -> Result<Connection> {
    let db = libsql::Builder::new_local(path).build().await?;
    let c = db.connect()?;
    c.execute("CREATE TABLE IF NOT EXISTS kv (k TEXT PRIMARY KEY, v TEXT NOT NULL)", ()).await?;
    c.execute(
        "CREATE TABLE IF NOT EXISTS jobs (
            id INTEGER PRIMARY KEY AUTOINCREMENT, chat_id INTEGER NOT NULL, url TEXT NOT NULL, title TEXT NOT NULL DEFAULT '',
            meta TEXT NOT NULL DEFAULT '', fmt TEXT NOT NULL DEFAULT '', state TEXT NOT NULL DEFAULT 'choose', err TEXT NOT NULL DEFAULT '',
            status_msg INTEGER NOT NULL DEFAULT 0, created_at INTEGER NOT NULL, started_at INTEGER NOT NULL DEFAULT 0, done_at INTEGER NOT NULL DEFAULT 0)",
        (),
    )
    .await?;
    c.execute("CREATE INDEX IF NOT EXISTS jobs_state ON jobs(state, id)", ()).await?;
    // Qayta ishga tushganda yarim qolgan ish navbat boshiga qaytadi.
    c.execute("UPDATE jobs SET state='queued' WHERE state='running'", ()).await?;
    Ok(c)
}

pub fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

pub async fn kv_get(c: &Connection, k: &str) -> Option<String> {
    let mut r = c.query("SELECT v FROM kv WHERE k=?1", params![k]).await.ok()?;
    r.next().await.ok()??.get::<String>(0).ok()
}

pub async fn kv_set(c: &Connection, k: &str, v: &str) -> Result<()> {
    c.execute("INSERT INTO kv(k,v) VALUES(?1,?2) ON CONFLICT(k) DO UPDATE SET v=excluded.v", params![k, v]).await?;
    Ok(())
}

pub async fn job_new(c: &Connection, chat_id: i64, url: &str, title: &str, meta: &str) -> Result<i64> {
    c.execute("INSERT INTO jobs(chat_id,url,title,meta,created_at) VALUES(?1,?2,?3,?4,?5)", params![chat_id, url, title, meta, now()]).await?;
    Ok(c.last_insert_rowid())
}

pub async fn job_get(c: &Connection, id: i64) -> Option<Job> {
    let mut r = c.query("SELECT id,chat_id,url,title,meta,fmt,status_msg FROM jobs WHERE id=?1", params![id]).await.ok()?;
    let row = r.next().await.ok()??;
    Some(Job { id: row.get(0).ok()?, chat_id: row.get(1).ok()?, url: row.get(2).ok()?, title: row.get(3).ok()?, meta: row.get(4).ok()?, fmt: row.get(5).ok()?, status_msg: row.get(6).ok()? })
}

pub async fn job_enqueue(c: &Connection, id: i64, fmt: &str, status_msg: i64) -> Result<bool> {
    let n = c.execute("UPDATE jobs SET state='queued', fmt=?2, status_msg=?3 WHERE id=?1 AND state='choose'", params![id, fmt, status_msg]).await?;
    Ok(n > 0)
}

/// Navbatdagi eng eski ishni oladi va "running" qiladi.
pub async fn job_take(c: &Connection) -> Option<Job> {
    let mut r = c.query("SELECT id FROM jobs WHERE state='queued' ORDER BY id LIMIT 1", ()).await.ok()?;
    let id: i64 = r.next().await.ok()??.get(0).ok()?;
    c.execute("UPDATE jobs SET state='running', started_at=?2 WHERE id=?1 AND state='queued'", params![id, now()]).await.ok()?;
    job_get(c, id).await
}

pub async fn job_finish(c: &Connection, id: i64, state: &str, err: &str) {
    let _ = c.execute("UPDATE jobs SET state=?2, err=?3, done_at=?4 WHERE id=?1", params![id, state, err, now()]).await;
}

pub async fn job_state(c: &Connection, id: i64) -> String {
    match c.query("SELECT state FROM jobs WHERE id=?1", params![id]).await {
        Ok(mut r) => r.next().await.ok().flatten().and_then(|row| row.get::<String>(0).ok()).unwrap_or_default(),
        Err(_) => String::new(),
    }
}

/// (id, sarlavha, holat) — kutayotgan va ishlayotgan ishlar.
pub async fn queue_list(c: &Connection) -> Vec<(i64, String, String)> {
    let mut out = Vec::new();
    if let Ok(mut r) = c.query("SELECT id,title,state FROM jobs WHERE state IN ('queued','running') ORDER BY id", ()).await {
        while let Ok(Some(row)) = r.next().await {
            out.push((row.get(0).unwrap_or(0), row.get(1).unwrap_or_default(), row.get(2).unwrap_or_default()));
        }
    }
    out
}

pub async fn job_cancel(c: &Connection, id: i64) -> bool {
    c.execute("UPDATE jobs SET state='cancelled', done_at=?2 WHERE id=?1 AND state IN ('choose','queued','running')", params![id, now()]).await.map(|n| n > 0).unwrap_or(false)
}
