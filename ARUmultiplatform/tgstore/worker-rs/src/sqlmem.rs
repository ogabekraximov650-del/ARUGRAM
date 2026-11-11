//! UZ: xotiradagi SQLite (WASM) — production'dagi SQL buyruqlari o'zgarishsiz bajariladi. Bo'laklardan yuklangan qatorlar shu bazaga qo'yiladi.
//! EN: in-memory SQLite (WASM) — production SQL runs unchanged. Rows loaded from chunks are put into this DB.
//! RU: SQLite v pamyati (WASM) — SQL produkshna vypolnyaetsya bez izmeneniy.
//!
//! UZ: Bu faqat wasm32 da yig'iladi (`sqlite-wasm-rs`, xotira VFS'i). Xavfsiz o'ram: hamma `unsafe` shu faylda.
//! EN: wasm32 only. All `unsafe` is confined to this file. RU: tolko wasm32; ves `unsafe` zdes.

use serde_json::{json, Value};
use sqlite_wasm_rs as ffi;
use std::ffi::{c_char, c_int, CStr, CString};
use std::sync::atomic::{AtomicU32, Ordering};
use worker::{Env, Request, Response, Result};

static NEXT: AtomicU32 = AtomicU32::new(1);

pub enum Param {
    Null,
    Int(i64),
    Real(f64),
    Text(String),
}

pub struct Rows {
    pub cols: Vec<String>,
    pub rows: Vec<Vec<Value>>,
    pub changes: i64,
}

pub struct Mem {
    db: *mut ffi::sqlite3,
}

impl Mem {
    /// UZ: yangi bo'sh xotira bazasi / EN: new empty in-memory DB / RU: novaya pustaya baza v pamyati
    pub fn open() -> std::result::Result<Mem, String> {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let name = CString::new(format!("tgs{n}.db")).map_err(|e| e.to_string())?;
        let mut db: *mut ffi::sqlite3 = std::ptr::null_mut();
        let rc = unsafe {
            ffi::sqlite3_open_v2(name.as_ptr().cast(), &mut db, ffi::SQLITE_OPEN_READWRITE | ffi::SQLITE_OPEN_CREATE, std::ptr::null())
        };
        if rc != ffi::SQLITE_OK {
            return Err(format!("sqlite open rc={rc}"));
        }
        Ok(Mem { db })
    }

    fn errmsg(&self) -> String {
        unsafe { CStr::from_ptr(ffi::sqlite3_errmsg(self.db)).to_string_lossy().into_owned() }
    }

    /// UZ: bitta buyruq (parametrlar bilan). EN: one statement with params. RU: odna komanda s parametrami.
    pub fn exec(&self, sql: &str, params: &[Param]) -> std::result::Result<Rows, String> {
        let csql = CString::new(sql).map_err(|e| e.to_string())?;
        let mut st: *mut ffi::sqlite3_stmt = std::ptr::null_mut();
        let rc = unsafe { ffi::sqlite3_prepare_v2(self.db, csql.as_ptr().cast(), -1, &mut st, std::ptr::null_mut()) };
        if rc != ffi::SQLITE_OK {
            return Err(format!("prepare: {}", self.errmsg()));
        }
        let result = (|| {
            for (i, p) in params.iter().enumerate() {
                let idx = (i + 1) as c_int;
                let rc = unsafe {
                    match p {
                        Param::Null => ffi::sqlite3_bind_null(st, idx),
                        Param::Int(v) => ffi::sqlite3_bind_int64(st, idx, *v),
                        Param::Real(v) => ffi::sqlite3_bind_double(st, idx, *v),
                        Param::Text(s) => ffi::sqlite3_bind_text(st, idx, s.as_ptr().cast::<c_char>(), s.len() as c_int, ffi::SQLITE_TRANSIENT()),
                    }
                };
                if rc != ffi::SQLITE_OK {
                    return Err(format!("bind {i}: {}", self.errmsg()));
                }
            }
            let ncol = unsafe { ffi::sqlite3_column_count(st) } as usize;
            let cols: Vec<String> = (0..ncol)
                .map(|i| unsafe { CStr::from_ptr(ffi::sqlite3_column_name(st, i as c_int)).to_string_lossy().into_owned() })
                .collect();
            let mut rows = Vec::new();
            loop {
                let rc = unsafe { ffi::sqlite3_step(st) };
                if rc == ffi::SQLITE_ROW {
                    let mut r = Vec::with_capacity(ncol);
                    for i in 0..ncol {
                        let i = i as c_int;
                        let v = unsafe {
                            match ffi::sqlite3_column_type(st, i) {
                                ffi::SQLITE_INTEGER => json!(ffi::sqlite3_column_int64(st, i)),
                                ffi::SQLITE_FLOAT => json!(ffi::sqlite3_column_double(st, i)),
                                ffi::SQLITE_NULL => Value::Null,
                                _ => {
                                    let p = ffi::sqlite3_column_text(st, i);
                                    let n = ffi::sqlite3_column_bytes(st, i) as usize;
                                    if p.is_null() { Value::Null } else { Value::String(String::from_utf8_lossy(std::slice::from_raw_parts(p.cast::<u8>(), n)).into_owned()) }
                                }
                            }
                        };
                        r.push(v);
                    }
                    rows.push(r);
                } else if rc == ffi::SQLITE_DONE {
                    break;
                } else {
                    return Err(format!("step: {}", self.errmsg()));
                }
            }
            Ok(Rows { cols, rows, changes: unsafe { ffi::sqlite3_changes(self.db) } as i64 })
        })();
        unsafe { ffi::sqlite3_finalize(st) };
        result
    }
}

impl Drop for Mem {
    fn drop(&mut self) {
        unsafe { ffi::sqlite3_close(self.db) };
    }
}

/// UZ: `/__sqlcheck` — production'da ishlatiladigan SQL shakllari xotira SQLite'da ishlashini ko'rsatadi.
/// EN: shows that SQL shapes used in production work in the in-memory SQLite. RU: pokazyvaet, chto SQL produkshna rabotaet.
pub fn selfcheck(req: &Request, env: &Env) -> Result<Response> {
    let key = env.secret("STORE_KEY").map(|s| s.to_string()).unwrap_or_default();
    let got = req.headers().get("authorization")?.unwrap_or_default();
    if key.is_empty() || got != format!("Bearer {key}") {
        return Response::error("unauthorized", 401);
    }
    let t0 = js_sys::Date::now();
    let run = || -> std::result::Result<Value, String> {
        let m = Mem::open()?;
        m.exec("CREATE TABLE IF NOT EXISTS epizod_db (id INTEGER PRIMARY KEY, anime_id INTEGER, no INTEGER, title TEXT, views INTEGER DEFAULT 0, added TEXT)", &[])?;
        m.exec("CREATE INDEX IF NOT EXISTS ix_ep ON epizod_db(anime_id, no)", &[])?;
        for i in 1..=2000i64 {
            m.exec("INSERT OR REPLACE INTO epizod_db(id,anime_id,no,title,views,added) VALUES(?,?,?,?,?,datetime('now'))",
                &[Param::Int(i), Param::Int(i % 20), Param::Int(i), Param::Text(format!("Qism {i} ўзбек")), Param::Int(i * 3)])?;
        }
        m.exec("INSERT INTO epizod_db(id,anime_id,no,title) VALUES(1,1,1,'x') ON CONFLICT(id) DO UPDATE SET title=excluded.title, views=views+1", &[])?;
        let a = m.exec("SELECT anime_id, COUNT(*) c, SUM(views) s FROM epizod_db GROUP BY anime_id ORDER BY s DESC LIMIT 3", &[])?;
        let b = m.exec("SELECT id,title FROM epizod_db WHERE title LIKE ? ORDER BY id DESC LIMIT 2", &[Param::Text("%ўзбек".into())])?;
        let c = m.exec("UPDATE epizod_db SET views = views + 1 WHERE anime_id = ?", &[Param::Int(3)])?;
        let d = m.exec("DELETE FROM epizod_db WHERE id > ?", &[Param::Int(1990)])?;
        let e = m.exec("SELECT strftime('%Y', 'now') y, (SELECT COUNT(*) FROM epizod_db) n", &[])?;
        Ok(json!({"group": a.rows, "like": b.rows, "updated": c.changes, "deleted": d.changes, "misc": e.rows}))
    };
    match run() {
        Ok(v) => Response::from_json(&json!({"ok": true, "ms": js_sys::Date::now() - t0, "result": v})),
        Err(e) => Response::from_json(&json!({"ok": false, "error": e})),
    }
}
