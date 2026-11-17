//! UZ: tgstore-jr (Rust): Turso `pipeline` formatidagi so'rovlarni libSQL (SQLite) da bajaradi; yozuvlar `db_N_vV` jurnal papkalariga tushadi va Telegram'ga nusxalanadi.
//! EN: tgstore-jr (Rust): runs Turso-pipeline requests on libSQL (SQLite); writes land in `db_N_vV` journal folders and replicate to Telegram.
//! RU: tgstore-jr (Rust): запросы формата Turso pipeline на libSQL, журнал в папках db_N_vV и копия в Telegram.
mod archive;
mod crypto;
mod fetch;
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

/// UZ: javob siqiladi (trafikni tejash): mijoz `br` ko'rsatsa Brotli 6, aks holda `gzip` 6; 512 baytdan kichik javob siqilmaydi.
/// Cloudflare worker `fetch` ikkalasini ham o'zi ochadi. Brotli 6 tanlandi (o'lchov: gzip 6 dan ~20% kichik, vaqti deyarli teng; 7+ ko'p vaqt oladi, foydasi <1%).
/// EN: responses are compressed to save traffic: Brotli 6 if the client accepts `br`, else gzip 6; bodies under 512 bytes are left alone.
/// RU: otvety szhimayutsya: Brotli 6 pri `br`, inache gzip 6; otvety menshe 512 bajt ne szhimayutsya.
fn accepts(accept: &str, name: &str) -> bool {
    accept.split(',').any(|t| {
        let mut p = t.trim().split(';');
        let n = p.next().unwrap_or("").trim();
        let zero = p.any(|x| { let x = x.trim().replace(' ', ""); x == "q=0" || x == "q=0.0" || x == "q=0.00" || x == "q=0.000" });
        n.eq_ignore_ascii_case(name) && !zero
    })
}

pub(crate) fn compress_body(accept: &str, body: &[u8]) -> Option<(&'static str, Vec<u8>)> {
    use std::io::Write;
    if body.len() < 512 { return None; }
    if accepts(accept, "br") {
        let mut out = Vec::with_capacity(body.len() / 4);
        {
            let mut w = brotli::CompressorWriter::new(&mut out, 4096, 6, 22);
            w.write_all(body).ok()?;
            w.flush().ok()?;
        }
        return Some(("br", out));
    }
    if accepts(accept, "gzip") {
        let mut e = flate2::write::GzEncoder::new(Vec::with_capacity(body.len() / 4), flate2::Compression::new(6));
        e.write_all(body).ok()?;
        return Some(("gzip", e.finish().ok()?));
    }
    None
}

/// UZ: gzip bilan kelgan so'rov tanasini ochadi (worker Ample'ga so'rovlarni gzip 9 bilan siqib yuboradi). Hajm chegarasi 64 MiB (zip-bomba himoyasi). Noma'lum kodlash yoki buzuq gzip — `Err`.
/// EN: decodes a gzip request body (worker sends gzip-9 requests); capped at 64 MiB (zip-bomb guard); unknown encodings or corrupt gzip are errors.
/// RU: raspakovyvaet gzip-telo zaprosa; limit 64 MiB; neizvestnoe kodirovanie ili povrezhdenie — oshibka.
pub(crate) fn decode_request(enc: &str, body: &[u8]) -> Result<Vec<u8>, &'static str> {
    use std::io::Read;
    const CAP: u64 = 64 * 1024 * 1024;
    match enc.trim().to_ascii_lowercase().as_str() {
        "" | "identity" => Ok(body.to_vec()),
        "gzip" | "x-gzip" => {
            let mut out = Vec::new();
            let n = flate2::read::GzDecoder::new(body).take(CAP + 1).read_to_end(&mut out).map_err(|_| "bad gzip")?;
            if n as u64 > CAP { return Err("too large"); }
            Ok(out)
        }
        _ => Err("unsupported content-encoding"),
    }
}

/// UZ: tashqi kirish: so'rov tanasi gzip bo'lsa ochiladi; HAR javobga `x-req-gzip: 1` qo'shiladi (worker shu belgi bo'yicha gzip so'rov yuborishni boshlaydi — eski server bu belgini bermaydi).
/// EN: entry point: decodes gzip bodies; every response carries `x-req-gzip: 1` (the worker starts gzip requests only after seeing it).
/// RU: vhod: raspakovka gzip; kazhdyj otvet imeet `x-req-gzip: 1`.
async fn handle(State(app): State<Arc<App>>, method: Method, uri: Uri, headers: HeaderMap, body: Bytes) -> Response {
    let enc = headers.get("content-encoding").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let mut resp = match decode_request(&enc, &body) {
        Ok(b) => handle_resp(State(app), method, uri, headers, Bytes::from(b)).await,
        Err(why) => json_resp(415, json!({"message": why})),
    };
    resp.headers_mut().insert("x-req-gzip", axum::http::HeaderValue::from_static("1"));
    resp
}

async fn handle_resp(State(app): State<Arc<App>>, method: Method, uri: Uri, headers: HeaderMap, body: Bytes) -> Response {
    let accept = headers.get("accept-encoding").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let resp = handle_inner(State(app), method, uri, headers, body).await;
    if accept.is_empty() { return resp; }
    let json = resp.headers().get("content-type").and_then(|v| v.to_str().ok()).is_some_and(|v| v.starts_with("application/json"));
    if !json || resp.status() != StatusCode::OK { return resp; }
    let (mut parts, b) = resp.into_parts();
    let Ok(bytes) = axum::body::to_bytes(b, usize::MAX).await else { return Response::from_parts(parts, axum::body::Body::empty()) };
    match compress_body(&accept, &bytes) {
        Some((enc, out)) => {
            parts.headers.insert("content-encoding", axum::http::HeaderValue::from_static(enc));
            parts.headers.insert("vary", axum::http::HeaderValue::from_static("accept-encoding"));
            parts.headers.remove("content-length");
            Response::from_parts(parts, axum::body::Body::from(out))
        }
        None => { parts.headers.remove("content-length"); Response::from_parts(parts, axum::body::Body::from(bytes)) }
    }
}

async fn handle_inner(State(app): State<Arc<App>>, method: Method, uri: Uri, headers: HeaderMap, body: Bytes) -> Response {
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
        "usage" => { let mut u = app.st.lock().await.usage(); u["telegram_stats"] = tg::stats(); json_resp(200, u) }
        "marks" => {
            let list = |k: &str| b[k].as_array().cloned().unwrap_or_default().iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect::<Vec<_>>();
            match app.st.lock().await.marks(&list("bump"), &list("get"), b["at"].as_i64().unwrap_or(0)).await { Ok(v) => json_resp(200, v), Err(e) => json_resp(500, json!({"message": e.to_string()})) }
        }
        "status" => {
            let s = app.st.lock().await;
            let mut u = s.usage();
            u["snapshot"] = s.snap.as_ref().map(|x| json!({"seq": x.seq, "at": x.at, "parts": x.parts.len()})).unwrap_or(J::Null);
            u["dirty"] = json!(s.dirty);
            u["pending_important_since"] = json!(s.crit_since);
            u["last_flush"] = json!(s.last_flush);
            u["telegram_stats"] = tg::stats();
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
        "ahttp" => match fetch::run(&b).await { Ok(v) => json_resp(200, v), Err(e) => json_resp(502, json!({"message": e.to_string()})) },
        "replicate" | "snapshot" => {
            let (Some(tg), Some(key)) = (app.tg.as_ref(), app.key.as_ref()) else { return json_resp(200, json!({"skipped": true})) };
            let r = if path == "replicate" { store::replicate(&app.st, tg, key, true).await } else { store::snapshot(&app.st, tg, key).await };
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
        // Muhim yozuvdan keyin 2 daqiqa (yig'ish), faqat arzon yozuvlar bo'lsa 3 soat. EN: debounce / idle flush.
        crit_ms: env("JR_CRIT_SECS").and_then(|v| v.parse::<u64>().ok()).unwrap_or(120) * 1000,
        idle_ms: env("JR_IDLE_SECS").and_then(|v| v.parse::<u64>().ok()).unwrap_or(3 * 3600) * 1000,
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
        tokio::spawn(async move { loop { tokio::time::sleep(std::time::Duration::from_secs(15)).await; if let Err(e) = store::replicate(&st2, &tg2, &key, false).await { log(&format!("replicate xato: {}", e)); } } });
        tokio::spawn(async move { loop { tokio::time::sleep(std::time::Duration::from_secs(300)).await; let need = st.lock().await.need_snapshot(); if need { if let Err(e) = store::snapshot(&st, &tg, &key).await { log(&format!("snapshot xato: {}", e)); } } } });
    }

    let port: u16 = env("PORT").and_then(|p| p.parse().ok()).unwrap_or(8090);
    let router = Router::new().fallback(handle).with_state(app);
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await.expect("port band");
    log(&format!("tgstore-jr (rust) listening {}", port));
    axum::serve(listener, router).await.unwrap();
}

#[cfg(test)]
mod request_gzip_tests {
    use super::*;
    use std::io::Write;

    fn gz(data: &[u8]) -> Vec<u8> {
        let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
        e.write_all(data).unwrap();
        e.finish().unwrap()
    }

    #[test]
    fn plain_and_identity_pass_through() {
        assert_eq!(decode_request("", b"abc").unwrap(), b"abc");
        assert_eq!(decode_request("identity", b"abc").unwrap(), b"abc");
    }

    #[test]
    fn gzip_body_is_decoded() {
        let data = br#"{"requests":[{"type":"execute"}]}"#.repeat(100);
        assert_eq!(decode_request("gzip", &gz(&data)).unwrap(), data);
        assert_eq!(decode_request(" GZIP ", &gz(&data)).unwrap(), data);
    }

    #[test]
    fn broken_or_unknown_encoding_is_rejected() {
        assert!(decode_request("gzip", b"not gzip at all").is_err());
        assert!(decode_request("br", b"abc").is_err());
        assert!(decode_request("deflate", b"abc").is_err());
    }
}

#[cfg(test)]
mod compress_tests {
    use super::*;
    use std::io::Read;

    fn sample() -> Vec<u8> {
        let row = r#"{"type":"text","value":"INSERT INTO vd_queue (name,size,chunks) VALUES (?,?,?)"},"#;
        row.repeat(40).into_bytes()
    }

    #[test]
    fn small_body_is_left_alone() {
        assert!(compress_body("br, gzip", b"{\"ok\":true}").is_none());
    }

    #[test]
    fn brotli_round_trip_and_smaller() {
        let b = sample();
        let (enc, out) = compress_body("gzip, deflate, br", &b).unwrap();
        assert_eq!(enc, "br");
        assert!(out.len() < b.len() / 3);
        let mut back = Vec::new();
        brotli::Decompressor::new(&out[..], 4096).read_to_end(&mut back).unwrap();
        assert_eq!(back, b);
    }

    #[test]
    fn gzip_fallback_round_trip() {
        let b = sample();
        let (enc, out) = compress_body("gzip", &b).unwrap();
        assert_eq!(enc, "gzip");
        let mut back = Vec::new();
        flate2::read::GzDecoder::new(&out[..]).read_to_end(&mut back).unwrap();
        assert_eq!(back, b);
    }

    #[test]
    fn no_supported_encoding_or_q0() {
        let b = sample();
        assert!(compress_body("identity", &b).is_none());
        assert!(compress_body("br;q=0, gzip;q=0", &b).is_none());
        assert_eq!(compress_body("br;q=0, gzip", &b).unwrap().0, "gzip");
    }
}
