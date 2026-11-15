// UZ: Ample ish yurituvchisi. Worker `POST /kick {"token": ENCODE_TOKEN}` yuboradi; yurituvchi worker'ning `/api/vd/jobstep` ini ish tugaguncha (idle) takror chaqiradi.
//     Har qism Cloudflare 50 so'rov chegarasiga sig'adi; xatoda kutib qayta uriniladi. Token faqat xotirada. Bir vaqtda BITTA aylanish ishlaydi.
// EN: Ample job runner. The worker sends `POST /kick {"token": ENCODE_TOKEN}`; the runner calls the worker's `/api/vd/jobstep` repeatedly until idle.
//     Each step fits Cloudflare's 50-subrequest limit; on errors it waits and retries. The token lives in memory only. Only ONE loop runs at a time.
// RU: Исполнитель задач Ample. Воркер шлёт `POST /kick {"token": ENCODE_TOKEN}`; исполнитель повторно вызывает `/api/vd/jobstep` воркера до простоя.
//     Каждый шаг укладывается в лимит 50 подзапросов Cloudflare; при ошибках ждёт и повторяет. Токен только в памяти. Одновременно работает ОДИН цикл.
mod hub;
use axum::{extract::State, http::{HeaderMap, StatusCode}, routing::{get, post}, Json, Router};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::time::Duration;

#[derive(Clone)]
struct App {
    hub: Arc<hub::Hub>,
    worker: String,
    running: Arc<AtomicBool>,
    token: Arc<Mutex<String>>,
}

/// UZ: bir aylanish: `idle` bo'lguncha qadamlar. Natija — bajarilgan qadamlar soni yoki xato matni. EN: one loop; returns steps done or an error.
pub async fn run_loop(client: &reqwest::Client, worker: &str, token: &(dyn Fn() -> String + Send + Sync), pause: Duration) -> Result<u32, String> {
    let mut steps = 0u32;
    let mut fails = 0u32;
    loop {
        let r = client
            .post(format!("{worker}/api/vd/jobstep"))
            .header("X-Encode-Token", token())
            .header("User-Agent", "arumediatv-jobrunner")
            .json(&json!({}))
            .send()
            .await;
        match r {
            Ok(resp) if resp.status().as_u16() == 403 => return Err("worker tokenni rad etdi (403)".into()),
            Ok(resp) if resp.status().is_success() => {
                fails = 0;
                let v: Value = resp.json().await.unwrap_or(json!({}));
                if v["idle"] == json!(true) {
                    return Ok(steps);
                }
                steps += 1;
            }
            other => {
                fails += 1;
                let why = match other { Ok(r) => format!("HTTP {}", r.status()), Err(e) => e.to_string() };
                if fails >= 6 {
                    return Err(format!("6 marta ketma-ket xato: {why}"));
                }
                tokio::time::sleep(pause.saturating_mul(1 << fails.min(6))).await;
                continue;
            }
        }
        tokio::time::sleep(pause).await;
    }
}

async fn kick(State(app): State<App>, Json(b): Json<Value>) -> (StatusCode, Json<Value>) {
    let tok = b["token"].as_str().unwrap_or("").to_string();
    if tok.len() < 16 {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "token"})));
    }
    *app.token.lock().unwrap_or_else(|e| e.into_inner()) = tok;
    // Aylanish allaqachon ishlayotgan bo'lsa — yangisi ochilmaydi (u oxirgi tokenni ishlatadi, yangi ish keyingi qadamda ko'rinadi).
    if app.running.swap(true, Ordering::SeqCst) {
        return (StatusCode::ACCEPTED, Json(json!({"ok": true, "started": false})));
    }
    tokio::spawn(async move {
        let client = reqwest::Client::builder().timeout(Duration::from_secs(100)).build().unwrap_or_default();
        let tok = Arc::clone(&app.token);
        let get_tok = move || tok.lock().unwrap_or_else(|e| e.into_inner()).clone();
        match run_loop(&client, &app.worker, &get_tok, Duration::from_millis(300)).await {
            Ok(n) => eprintln!("ish yurituvchi: tugadi ({n} qadam)"),
            Err(e) => eprintln!("ish yurituvchi: to'xtadi: {e}"),
        }
        app.running.store(false, Ordering::SeqCst);
    });
    (StatusCode::OK, Json(json!({"ok": true, "started": true})))
}

fn strs(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default()
}

fn hub_key(h: &HeaderMap) -> String {
    h.get("x-hub-key").and_then(|v| v.to_str().ok()).unwrap_or("").to_string()
}

/// POST /setkey {key} — kalit hali yo'q bo'lsagina o'rnatadi (birinchi chaqiruv).
async fn setkey(State(app): State<App>, Json(b): Json<Value>) -> (StatusCode, Json<Value>) {
    if app.hub.set_key(b["key"].as_str().unwrap_or("")) {
        (StatusCode::OK, Json(json!({"ok": true})))
    } else {
        (StatusCode::FORBIDDEN, Json(json!({"error": "already set or invalid"})))
    }
}

/// POST /marks {bump:[], at, get:[]} -> {v:{kalit: vaqt}} (`MarkHub` bilan bir xil shakl).
async fn marks(State(app): State<App>, headers: HeaderMap, Json(b): Json<Value>) -> (StatusCode, Json<Value>) {
    if !app.hub.authorized(&hub_key(&headers)) {
        return (StatusCode::UNAUTHORIZED, Json(json!({"error": "unauthorized"})));
    }
    (StatusCode::OK, Json(app.hub.marks(&strs(&b["bump"]), b["at"].as_i64().unwrap_or(0), &strs(&b["get"]))))
}

/// POST /wait {keys:[], since, timeout_ms} -> {max, changed}: o'zgarguncha (<= 25 s) kutadi.
async fn wait(State(app): State<App>, headers: HeaderMap, Json(b): Json<Value>) -> (StatusCode, Json<Value>) {
    if !app.hub.authorized(&hub_key(&headers)) {
        return (StatusCode::UNAUTHORIZED, Json(json!({"error": "unauthorized"})));
    }
    let t = b["timeout_ms"].as_u64().unwrap_or(20_000).clamp(1_000, 25_000);
    (StatusCode::OK, Json(app.hub.wait(&strs(&b["keys"]), b["since"].as_i64().unwrap_or(-1), Duration::from_millis(t)).await))
}

#[tokio::main]
async fn main() {
    let worker = std::env::var("WORKER_URL").unwrap_or_else(|_| "https://arumediatv.uzcom.workers.dev".into()).trim_end_matches('/').to_string();
    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(3000);
    let dir = std::env::var("HUB_DIR").unwrap_or_else(|_| "/var/lib/jobrunner".into());
    let app = App { hub: Arc::new(hub::Hub::new(std::path::PathBuf::from(dir).join("hubkey"))), worker, running: Arc::new(AtomicBool::new(false)), token: Arc::new(Mutex::new(String::new())) };
    let router = Router::new().route("/", get(|| async { "ok" })).route("/kick", post(kick)).route("/setkey", post(setkey)).route("/marks", post(marks)).route("/wait", post(wait)).with_state(app);
    let l = tokio::net::TcpListener::bind(("0.0.0.0", port)).await.expect("port band");
    eprintln!("jobrunner :{port}");
    axum::serve(l, router).await.expect("server");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;

    async fn mock(script: Vec<(u16, Value)>) -> (String, Arc<AtomicU32>) {
        let hits = Arc::new(AtomicU32::new(0));
        let (h2, script) = (Arc::clone(&hits), Arc::new(script));
        let r = Router::new().route("/api/vd/jobstep", post(move |headers: axum::http::HeaderMap| {
            let (h, sc) = (Arc::clone(&h2), Arc::clone(&script));
            async move {
                assert_eq!(headers.get("x-encode-token").and_then(|v| v.to_str().ok()), Some("tok-1234567890abcdef"));
                let i = h.fetch_add(1, Ordering::SeqCst) as usize;
                let (code, v) = sc.get(i).cloned().unwrap_or((200, json!({"idle": true})));
                (StatusCode::from_u16(code).unwrap(), Json(v))
            }
        }));
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        tokio::spawn(async move { let _ = axum::serve(l, r).await; });
        (format!("http://{addr}"), hits)
    }

    fn tok() -> String { "tok-1234567890abcdef".to_string() }

    #[tokio::test]
    async fn runs_until_idle() {
        let (url, hits) = mock(vec![(200, json!({"idle": false, "done": false})), (200, json!({"idle": false, "done": true})), (200, json!({"idle": true}))]).await;
        let n = run_loop(&reqwest::Client::new(), &url, &tok, Duration::from_millis(1)).await.unwrap();
        assert_eq!(n, 2);
        assert_eq!(hits.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn retries_on_server_error_then_continues() {
        let (url, _) = mock(vec![(500, json!({})), (200, json!({"idle": false})), (200, json!({"idle": true}))]).await;
        assert_eq!(run_loop(&reqwest::Client::new(), &url, &tok, Duration::from_millis(1)).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn stops_on_forbidden_and_after_repeated_failures() {
        let (url, hits) = mock(vec![(403, json!({}))]).await;
        assert!(run_loop(&reqwest::Client::new(), &url, &tok, Duration::from_millis(1)).await.is_err());
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        let (url2, _) = mock((0..10).map(|_| (500, json!({}))).collect()).await;
        assert!(run_loop(&reqwest::Client::new(), &url2, &tok, Duration::from_millis(1)).await.unwrap_err().contains("6 marta"));
    }
}
