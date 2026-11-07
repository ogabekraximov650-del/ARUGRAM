//! ARUDRIVE worker (Rust → WASM): Bot API orqali fayl/video ombori. ARUMEDIATV workeridan ALOHIDA.
//! Oqim: ilova -> R2 -> (ikkinchi akkauntdagi Actions) 18 MB bo'laklar -> Telegram kanali (Bot API);
//! Ko'rish: Telegram -> Cloudflare keshi -> ilova (R2 orqali O'TMAYDI; keshga tushmaguncha video ochilmaydi). Tafsilot: `bdrive.rs`, `cache.rs`.
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::Sha256;
use worker::*;

mod bdrive;
mod cache;

const APP_SIG_SKEW_SECS: i64 = 120;

// ── Yordamchilar ─────────────────────────────────────────────

pub(crate) fn set_cors(resp: &mut Response) {
    let h = resp.headers_mut();
    let _ = h.set("Access-Control-Allow-Origin", "*");
    let _ = h.set("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS");
    let _ = h.set("Access-Control-Allow-Headers", "Content-Type, Range, X-App-Sig, X-Encode-Token");
    let _ = h.set("Access-Control-Expose-Headers", "Content-Range, Content-Length, Accept-Ranges");
}

pub(crate) fn json_resp(data: &Value, status: u16) -> Result<Response> {
    let mut resp = Response::from_json(data)?.with_status(status);
    set_cors(&mut resp);
    Ok(resp)
}

pub(crate) fn ok(v: Value) -> Result<Response> {
    json_resp(&v, 200)
}

pub(crate) fn err404(msg: &str) -> Result<Response> {
    json_resp(&json!({"error": msg}), 404)
}

pub(crate) fn now_ms() -> i64 {
    Date::now().as_millis() as i64
}

pub(crate) fn hex_of(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn ct_eq(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

pub(crate) fn tg_secret(env: &Env, name: &str) -> String {
    env.secret(name)
        .map(|s| s.to_string().trim().to_string())
        .or_else(|_| env.var(name).map(|s| s.to_string().trim().to_string()))
        .unwrap_or_default()
}

pub(crate) fn jint(b: &Value, k: &str) -> i64 {
    b[k].as_i64().or_else(|| b[k].as_str().and_then(|s| s.trim().parse().ok())).unwrap_or(0)
}

/// Kanal ID'si Bot API ko'rinishida (`-100...`): `-1001234567890`, `1001234567890` yoki yalang `1234567890` tushuniladi.
pub(crate) fn tg_channel_id(env: &Env) -> i64 {
    let t = tg_secret(env, "TG_CHANNEL_ID");
    let Ok(n) = t.parse::<i64>() else { return 0 };
    if n <= 0 {
        return n;
    }
    if n > 1_000_000_000_000 {
        -n
    } else {
        -(1_000_000_000_000 + n)
    }
}

// ── Turso (HTTP) natijalarini o'qish ─────────────────────────

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct TursoArg {
    #[serde(rename = "type")]
    type_: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<String>,
}

impl TursoArg {
    pub(crate) fn text(v: &str) -> Self {
        Self { type_: "text".into(), value: Some(v.into()) }
    }
    pub(crate) fn int(v: i64) -> Self {
        Self { type_: "integer".into(), value: Some(v.to_string()) }
    }
}

fn row_to_obj(cols: &[Value], row: &[Value]) -> Value {
    let mut map = serde_json::Map::new();
    for (i, col) in cols.iter().enumerate() {
        let name = col["name"].as_str().unwrap_or("").to_string();
        let cell = &row[i];
        let t = cell["type"].as_str().unwrap_or("null");
        let val = if t == "null" {
            Value::Null
        } else if t == "integer" || t == "float" {
            let s = cell["value"].as_str().unwrap_or("0");
            s.parse::<i64>().map(|n| json!(n)).or_else(|_| s.parse::<f64>().map(|f| json!(f))).unwrap_or(Value::Null)
        } else {
            json!(cell["value"].as_str().unwrap_or(""))
        };
        map.insert(name, val);
    }
    Value::Object(map)
}

pub(crate) fn rows_of(res: &Value) -> Vec<Value> {
    let cols = res["cols"].as_array().cloned().unwrap_or_default();
    res["rows"].as_array().cloned().unwrap_or_default().iter().map(|r| row_to_obj(&cols, r.as_array().unwrap_or(&vec![]))).collect()
}

pub(crate) fn first_row(res: &Value) -> Option<Value> {
    rows_of(res).into_iter().next()
}

// ── Telegram va GitHub ───────────────────────────────────────

pub(crate) async fn tg_api_tok(token: &str, method: &str, body: Value) -> Result<Value> {
    let h = Headers::new();
    h.set("Content-Type", "application/json")?;
    let req = Request::new_with_init(
        &format!("https://api.telegram.org/bot{token}/{method}"),
        RequestInit::new().with_method(Method::Post).with_headers(h).with_body(Some(body.to_string().into())),
    )?;
    let mut r = Fetch::Request(req).send().await?;
    let d: Value = r.json().await?;
    if d["ok"] != json!(true) {
        return Err(Error::RustError(format!("Telegram xatosi ({method}): {}", d["description"].as_str().unwrap_or("noma'lum"))));
    }
    Ok(d["result"].clone())
}

pub(crate) async fn gh_api(env: &Env, method: Method, path: &str, body: Option<Value>) -> Result<(u16, Value)> {
    let token = tg_secret(env, "GH_ACTIONS_TOKEN");
    if token.is_empty() {
        return Err(Error::RustError("GH_ACTIONS_TOKEN yo'q".into()));
    }
    let h = Headers::new();
    h.set("Authorization", &format!("Bearer {token}"))?;
    h.set("Accept", "application/vnd.github+json")?;
    h.set("X-GitHub-Api-Version", "2022-11-28")?;
    h.set("User-Agent", "arudrive-worker")?;
    let mut init = RequestInit::new();
    init.with_method(method).with_headers(h);
    if let Some(b) = body {
        init.with_body(Some(b.to_string().into()));
    }
    let req = Request::new_with_init(&format!("https://api.github.com{path}"), &init)?;
    let mut r = Fetch::Request(req).send().await?;
    let status = r.status_code();
    let v: Value = r.json().await.unwrap_or(json!({}));
    Ok((status, v))
}

/// Workflow'ning biror run'i hali tugamagan bo'lsa `Ok(true)`.
pub(crate) async fn gh_workflow_busy(env: &Env, repo: &str, workflow: &str) -> std::result::Result<bool, String> {
    match gh_api(env, Method::Get, &format!("/repos/{repo}/actions/workflows/{workflow}/runs?per_page=10"), None).await {
        Ok((200, v)) => Ok(v["workflow_runs"].as_array().map(|a| a.iter().any(|r| r["status"].as_str().map(|s| s != "completed").unwrap_or(false))).unwrap_or(false)),
        Ok((code, v)) => Err(format!("GitHub javobi {code}: {}", v["message"].as_str().unwrap_or(""))),
        Err(e) => Err(format!("Actions ishga tushmadi: {e}")),
    }
}

// ── Himoya ───────────────────────────────────────────────────

/// Actions (`X-Encode-Token`) — ARUMEDIATV'dagi bilan bir xil sir.
pub(crate) fn encode_token_ok(req: &Request, env: &Env) -> bool {
    let want = tg_secret(env, "ENCODE_TOKEN");
    if want.len() < 16 {
        return false;
    }
    let got = req.headers().get("X-Encode-Token").ok().flatten().unwrap_or_default();
    ct_eq(got.trim(), &want)
}

fn mac_hex(secret: &str, data: &str) -> Option<String> {
    let mut h = <Hmac<Sha256> as Mac>::new_from_slice(secret.as_bytes()).ok()?;
    h.update(data.as_bytes());
    Some(hex_of(&h.finalize().into_bytes()))
}

/// `X-App-Sig: v2.<vaqt>.<hex>`; imzolanadigan matn `"<vaqt>.<METOD>.<yo'l>"` (ARUMEDIATV bilan bir xil).
fn verify_app_sig(secret: &str, got: &str, method: &str, path: &str) -> bool {
    let mut parts = got.split('.');
    if parts.next() != Some("v2") {
        return false;
    }
    let (Some(ts), Some(mac)) = (parts.next(), parts.next()) else { return false };
    if parts.next().is_some() {
        return false;
    }
    let Ok(ts_num) = ts.parse::<i64>() else { return false };
    if (now_ms() / 1000 - ts_num).abs() > APP_SIG_SKEW_SECS {
        return false;
    }
    match mac_hex(secret, &format!("{ts}.{}.{path}", method.to_uppercase())) {
        Some(want) => want.len() == mac.len() && want.eq_ignore_ascii_case(mac),
        None => false,
    }
}

/// Pleyer manzili (`?t=<muddat>.<hex>`), matn `"play.<yo'l>.<muddat>"` — ExoPlayer sarlavha qo'sha olmaydi.
fn verify_play_token(secret: &str, token: &str, path: &str) -> bool {
    let Some((exp, mac)) = token.split_once('.') else { return false };
    let Ok(exp_num) = exp.parse::<i64>() else { return false };
    if now_ms() / 1000 > exp_num {
        return false;
    }
    match mac_hex(secret, &format!("play.{path}.{exp}")) {
        Some(want) => want.len() == mac.len() && want.eq_ignore_ascii_case(mac),
        None => false,
    }
}

const ACTION_PATHS: &[&str] = &["/api/bd/claim", "/api/bd/read", "/api/bd/chunkdone", "/api/bd/done", "/api/bd/fail"];

/// So'rovni o'tkazamizmi. Actions yo'llari o'z kaliti (`ENCODE_TOKEN`) bilan; qolgani — ilova imzosi (yoki pleyer tokeni).
fn gate(req: &Request, env: &Env, path: &str) -> bool {
    if ACTION_PATHS.contains(&path) {
        return true; // `bdrive::route` ichida `encode_token_ok` tekshiriladi
    }
    // Admin/diagnostika: `X-Encode-Token` (ENCODE_TOKEN) bilan hamma yo'l ochiq (ilova imzosisiz sinash uchun).
    if encode_token_ok(req, env) {
        return true;
    }
    let secret = tg_secret(env, "APP_SIGN_SECRET");
    if secret.is_empty() {
        return false; // sir qo'yilmagan bo'lsa HAMMA narsa yopiq
    }
    if path.starts_with("/api/play/") {
        let token = req.url().ok().and_then(|u| u.query_pairs().find(|(k, _)| k == "t").map(|(_, v)| v.to_string())).unwrap_or_default();
        if !token.is_empty() && verify_play_token(&secret, &token, path) {
            return true;
        }
    }
    let got = req.headers().get("X-App-Sig").ok().flatten().unwrap_or_default();
    verify_app_sig(&secret, &got, req.method().to_string().as_str(), path)
}

/// Har 10 daqiqada (cron): R2 dagi yuklangan fayllarni Telegram'ga bo'laklab saqlaydi (Actions kerak emas), so'ng tashlab ketilgan yuklashlarni tozalaydi.
#[event(scheduled)]
async fn scheduled(_e: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    bdrive::process_and_chain(&env).await;
    let _ = bdrive::cleanup(&env).await;
}

#[event(fetch)]
async fn main(req: Request, env: Env, ctx: Context) -> Result<Response> {
    let _ = &ctx;
    let url = req.url()?;
    let path = url.path().to_string();
    let method = req.method();
    if method == Method::Options {
        let mut r = Response::empty()?.with_status(204);
        set_cors(&mut r);
        return Ok(r);
    }
    if path == "/" || path == "/health" {
        return ok(json!({"ok": true, "name": "arudrive"}));
    }
    if !gate(&req, &env, &path) {
        return json_resp(&json!({"error": "forbidden"}), 403);
    }
    let range = req.headers().get("Range").ok().flatten();
    let res = if let Some(fname) = path.strip_prefix("/api/play/") {
        if !fname.starts_with("bd~") {
            err404("topilmadi")
        } else {
            cache::b2_play(&env, &ctx, fname, range).await
        }
    } else if let Some(fname) = path.strip_prefix("/api/warm/") {
        let widx = url.query_pairs().find(|(k, _)| k == "w").and_then(|(_, v)| v.parse::<u64>().ok()).unwrap_or(0);
        let force = url.query_pairs().any(|(k, v)| k == "force" && (v == "1" || v == "true"));
        match fname.strip_prefix("bd~") {
            Some(id) => bdrive::warm(&env, id, widx, force).await,
            None => err404("topilmadi"),
        }
    } else if path.starts_with("/api/bd/") {
        bdrive::route(req, &env, &ctx, &path).await
    } else {
        err404("topilmadi")
    };
    match res {
        Ok(r) => Ok(r),
        Err(e) => json_resp(&json!({"error": e.to_string()}), 502),
    }
}
