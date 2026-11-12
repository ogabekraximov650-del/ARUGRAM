//! UZ: tgstore-jr (Rust): Turso `pipeline` formatidagi so'rovlarni libSQL (SQLite) da bajaradi; yozuvlar `db_N_vV` jurnal papkalariga tushadi va Telegram'ga nusxalanadi.
//! EN: tgstore-jr (Rust): runs Turso-pipeline requests on libSQL (SQLite); writes land in `db_N_vV` journal folders and replicate to Telegram.
//! RU: tgstore-jr (Rust): запросы формата Turso pipeline на libSQL, журнал в папках db_N_vV и копия в Telegram.
mod archive;
mod crypto;
mod store;
mod tg;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::Router;
use serde_json::{json, Value as J};
use std::collections::HashSet;
use std::sync::Arc;
use store::{Cfg, Res, Shared, Stmt};
use tokio::sync::Mutex;

struct App {
    st: Shared,
    tg: Option<tg::Tg>,
    key: Option<[u8; 32]>,
    auths: Mutex<HashSet<String>>,
    arch: Option<Arc<archive::Arch>>,
}

fn log(m: &str) { eprintln!("{} {}", store::now_ms(), m); }

fn json_resp(code: u16, v: J) -> Response { (StatusCode::from_u16(code).unwrap(), [("content-type", "application/json")], v.to_string()).into_response() }

fn res_json(r: Vec<Res>) -> Vec<J> { r.into_iter().map(|x| match x { Res::Ok(v) => json!({"ok": true, "result": v}), Res::Err(e) => json!({"ok": false, "error": e}) }).collect() }

async fn handle(State(app): State<Arc<App>>, method: Method, uri: Uri, headers: HeaderMap, body: Bytes) -> Response {
    if method == Method::GET { return json_resp(200, json!({"ok": true, "service": "tgstore-jr", "engine": "libsql-rs"})); }
    let path = uri.path().trim_start_matches('/').to_string();
    let tok = headers.get("authorization").and_then(|v| v.to_str().ok()).unwrap_or("").trim_start_matches("Bearer ").to_string();
    let b: J = serde_json::from_slice(&body).unwrap_or(json!({}));

    if path == "setkey" { // UZ: kalit hali o'rnatilmagan bo'lsagina birinchi chaqiruv o'rnatadi. EN/RU: first call sets the key while none is set.
        let k = b["key"].as_str().unwrap_or("").to_string();
        if k.len() < 16 { return json_resp(400, json!({"message": "bad key"})); }
        let mut st = app.st.lock().await;
        let have = matches!(st.exec(&[Stmt { sql: "select 1 from kv_conf where k='key_hash'".into(), args: vec![] }], false).await.remove(0), Res::Ok(ref v) if v["rows"].as_array().map(|r| !r.is_empty()).unwrap_or(false));
        if have { return json_resp(403, json!({"message": "already set"})); }
        let h = crypto::sha256_hex(&k);
        st.exec(&[Stmt { sql: "insert into kv_conf(k,v) values('key_hash',?1)".into(), args: vec![json!({"type": "text", "value": h})] }], true).await;
        app.auths.lock().await.insert(h);
        return json_resp(200, json!({"ok": true}));
    }
    if !app.auths.lock().await.contains(&crypto::sha256_hex(&tok)) { return json_resp(401, json!({"message": "unauthorized"})); }

    match path.as_str() {
        "v2/pipeline" => {
            let reqs = b["requests"].as_array().cloned().unwrap_or_default();
            let stmts: Vec<Stmt> = reqs.iter().filter(|q| q["type"] == "execute").filter_map(|q| serde_json::from_value(q["stmt"].clone()).ok()).collect();
            let r = res_json(app.st.lock().await.exec(&stmts, true).await);
            let mut i = 0;
            let results: Vec<J> = reqs.iter().map(|q| {
                if q["type"] != "execute" { return json!({"type": "ok", "response": {"type": q["type"]}}); }
                let x = r.get(i); i += 1;
                match x {
                    None => json!({"type": "error", "error": {"message": "skipped after error"}}),
                    Some(x) if x["ok"] == json!(true) => json!({"type": "ok", "response": {"type": "execute", "result": x["result"]}}),
                    Some(x) => json!({"type": "error", "error": {"message": x["error"]}}),
                }
            }).collect();
            json_resp(200, json!({"baton": null, "base_url": null, "results": results}))
        }
        "sql" => {
            let stmts: Vec<Stmt> = serde_json::from_value(b["stmts"].clone()).unwrap_or_default();
            let journal = b["journal"] != json!(false);
            json_resp(200, json!({"results": res_json(app.st.lock().await.exec(&stmts, journal).await)}))
        }
        "usage" => json_resp(200, app.st.lock().await.usage()),
        "marks" => {
            let list = |k: &str| b[k].as_array().cloned().unwrap_or_default().iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect::<Vec<_>>();
            match app.st.lock().await.marks(&list("bump"), &list("get"), b["at"].as_i64().unwrap_or(0)).await { Ok(v) => json_resp(200, v), Err(e) => json_resp(500, json!({"message": e.to_string()})) }
        }
        "status" => {
            let s = app.st.lock().await;
            let mut u = s.usage();
            u["snapshot"] = s.snap.as_ref().map(|x| json!({"seq": x.seq, "at": x.at, "parts": x.parts.len()})).unwrap_or(J::Null);
            u["dirty"] = json!(s.dirty);
            u["telegram"] = json!(app.tg.is_some() && app.key.is_some());
            json_resp(200, u)
        }
        "schema" => {
            let n = app.st.lock().await.apply_schema(include_str!("../schema.sql")).await;
            match n { Ok(n) => json_resp(200, json!({"statements": n})), Err(e) => json_resp(500, json!({"message": e.to_string()})) }
        }
        p if p.starts_with("archive/") => {
            let Some(a) = app.arch.as_ref() else { return json_resp(503, json!({"message": "doira yoqilmagan (TG_BOT_TOKEN/TG_CHANNEL_ID yo'q)"})) };
            let r = match &p[8..] {
                "run" => a.run(b["force"] == json!(true), b["dry"] == json!(true), b["max"].as_u64().unwrap_or(5) as usize).await,
                "files" => a.files().await,
                "get" => a.lookup(b["table"].as_str().unwrap_or(""), b["key"].as_str().unwrap_or("")).await.map(|row| json!({"row": row})),
                "warm" => a.warm(b["table"].as_str().unwrap_or(""), b["key"].as_str().unwrap_or(""), b["days"].as_i64().unwrap_or(7)).await,
                "restore" => a.restore(b["id"].as_i64().unwrap_or(0), b["days"].as_i64().unwrap_or(7)).await,
                "purge" => a.purge(b["id"].as_i64().unwrap_or(0)).await,
                _ => return json_resp(404, json!({"message": "no such endpoint"})),
            };
            match r { Ok(v) => json_resp(200, v), Err(e) => json_resp(500, json!({"message": e.to_string()})) }
        }
        "replicate" | "snapshot" => {
            let (Some(tg), Some(key)) = (app.tg.as_ref(), app.key.as_ref()) else { return json_resp(200, json!({"skipped": true})) };
            let r = if path == "replicate" { store::replicate(&app.st, tg, key).await } else { store::snapshot(&app.st, tg, key).await };
            match r { Ok(v) => json_resp(200, v), Err(e) => json_resp(500, json!({"message": e.to_string()})) }
        }
        _ => json_resp(404, json!({"message": "no such endpoint"})),
    }
}

#[tokio::main]
async fn main() {
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let cfg = Cfg {
        dir: env("JR_DIR").unwrap_or_else(|| "/var/lib/tgstore-data".into()).into(),
        chunk_bytes: env("JR_CHUNK_BYTES").and_then(|v| v.parse().ok()).unwrap_or(1024 * 1024),
        limit_mb: env("DB_LIMIT_MB").and_then(|v| v.parse().ok()).unwrap_or(1024),
    };
    let tg = match (env("TG_BOT_TOKEN"), env("TG_CHANNEL_ID")) { (Some(t), Some(c)) => Some(tg::Tg::new(t, c)), _ => None };
    let key = env("JR_KEY").and_then(|k| crypto::key32(&k).ok());

    let store = if !cfg.dir.join("main.db").exists() && tg.is_some() && key.is_some() {
        match store::recover(cfg.clone(), tg.as_ref().unwrap(), key.as_ref().unwrap()).await {
            Ok((s, n)) => { log(&format!("Telegram'dan tiklandi, seq {} qayta o'ynaldi {}", s.seq, n)); s }
            Err(e) => { log(&format!("tiklab bo'lmadi (yangi bo'sh baza ochiladi): {}", e)); let _ = std::fs::remove_dir_all(&cfg.dir); store::Store::open(cfg.clone()).await.expect("baza ochilmadi") }
        }
    } else { store::Store::open(cfg.clone()).await.expect("baza ochilmadi") };
    let st: Shared = Arc::new(Mutex::new(store));
    {
        let mut s = st.lock().await;
        s.exec(&[Stmt { sql: "create table if not exists kv_conf(k text primary key, v text not null)".into(), args: vec![] }], false).await;
    }
    let mut auths = HashSet::new();
    for k in ["STORE_KEY", "ADMIN_KEY"] { if let Some(v) = env(k) { auths.insert(crypto::sha256_hex(&v)); } }
    if let Res::Ok(v) = st.lock().await.exec(&[Stmt { sql: "select v from kv_conf where k='key_hash'".into(), args: vec![] }], false).await.remove(0) {
        if let Some(h) = v["rows"][0][0]["value"].as_str() { auths.insert(h.to_string()); }
    }
    let arch = tg.clone().map(|t| Arc::new(archive::Arch::new(st.clone(), t, env("JR_ARCH_DIR").unwrap_or_else(|| "/var/lib/tgstore-arch".into()).into(), cfg.chunk_bytes)));
    let app = Arc::new(App { st: st.clone(), tg: tg.clone(), key, auths: Mutex::new(auths), arch: arch.clone() });
    if let Some(a) = arch {
        // UZ: har 5 daqiqada baza foizi tekshiriladi; >= 70% bo'lsa sovuq qatorlar Telegram'ga chiqariladi. EN/RU: circle tick every 5 minutes.
        tokio::spawn(async move { loop { tokio::time::sleep(std::time::Duration::from_secs(300)).await; match a.run(false, false, 5).await { Ok(r) if r["active"] == json!(true) => log(&format!("doira: {}", r.to_string().chars().take(300).collect::<String>())), Err(e) => log(&format!("doira xato: {}", e)), _ => {} } } });
    }

    if let (Some(tg), Some(key)) = (tg, key) {
        let (st2, tg2) = (st.clone(), tg.clone());
        tokio::spawn(async move { loop { tokio::time::sleep(std::time::Duration::from_secs(20)).await; if let Err(e) = store::replicate(&st2, &tg2, &key).await { log(&format!("replicate xato: {}", e)); } } });
        tokio::spawn(async move { loop { tokio::time::sleep(std::time::Duration::from_secs(300)).await; let need = st.lock().await.need_snapshot(); if need { if let Err(e) = store::snapshot(&st, &tg, &key).await { log(&format!("snapshot xato: {}", e)); } } } });
    }

    let port: u16 = env("PORT").and_then(|p| p.parse().ok()).unwrap_or(8090);
    let router = Router::new().fallback(handle).with_state(app);
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await.expect("port band");
    log(&format!("tgstore-jr (rust) listening {}", port));
    axum::serve(listener, router).await.unwrap();
}
