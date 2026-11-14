// UZ: GitHub Actions orqali og'ir ishlar: Ample faqat navbat, log va chat xabarini boshqaradi; yuklash/bo'lish/yuborishni Actions bajaradi.
//     Token SHIFRLANGAN fayldan (`gh.enc`, kalit — `encode_token`) o'qiladi, xotirada saqlanadi, diskka ochiq yozilmaydi.
// EN: Heavy jobs through GitHub Actions: Ample only manages the queue, logs and the chat message; Actions downloads/splits/uploads.
//     The token is read from an ENCRYPTED file (`gh.enc`, key = `encode_token`), kept in memory, never written to disk in clear.
use super::*;
use axum::extract::{Path as AxPath, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use std::collections::HashMap;

/// UZ: Chat xabarini yangilash oralig'i (Telegram cheklovi uchun). EN: min interval between chat edits (Telegram flood limits).
pub const EDIT_EVERY: Duration = Duration::from_secs(3);

static GH_TOKEN: tokio::sync::OnceCell<String> = tokio::sync::OnceCell::const_new();
static LAST_EDIT: std::sync::Mutex<Option<HashMap<i64, std::time::Instant>>> = std::sync::Mutex::new(None);

pub fn configured(cfg: &Cfg) -> bool {
    cfg.gh_repo.is_some() && Path::new(&format!("{}/gh.enc", cfg.data_dir)).is_file() && Path::new(&format!("{}/encode_token", cfg.data_dir)).is_file()
}

/// UZ: `gh.enc` ni `encode_token` kaliti bilan ochadi (openssl, AES-256-CBC, PBKDF2 200000) — natija faqat xotirada. EN: decrypts `gh.enc` with the `encode_token` key; result lives in memory only.
async fn token(cfg: &Cfg) -> Result<&'static str> {
    GH_TOKEN
        .get_or_try_init(|| async {
            let key = std::fs::read_to_string(format!("{}/encode_token", cfg.data_dir))?.trim().to_string();
            let o = tokio::process::Command::new("openssl")
                .args(["enc", "-d", "-aes-256-cbc", "-pbkdf2", "-iter", "200000", "-pass", "env:K", "-in"])
                .arg(format!("{}/gh.enc", cfg.data_dir))
                .env("K", key)
                .output()
                .await?;
            let t = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if !o.status.success() || t.is_empty() {
                return Err(anyhow!("gh.enc ochilmadi (kalit yoki fayl noto'g'ri)"));
            }
            Ok(t)
        })
        .await
        .map(|s| s.as_str())
}

/// UZ: 32 belgili tasodifiy kalit (/dev/urandom). EN: random 32-hex token.
pub fn new_tok() -> String {
    let mut b = [0u8; 16];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        use std::io::Read;
        let _ = f.read_exact(&mut b);
    }
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && !a.is_empty() && a.bytes().zip(b.bytes()).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}

/// UZ: `workflow_dispatch` so'rovining tanasi. `etok` — ishga xos kalitning SHIFRLANGAN ko'rinishi (repo ochiq bo'lsa ham kirishlar sirni oshkor qilmasin). EN: body of the `workflow_dispatch` request; `etok` is the per-job key ENCRYPTED so a public repo's inputs reveal nothing.
pub fn dispatch_body(job: i64, cb: &str, etok: &str) -> Value {
    json!({"ref": "main", "inputs": {"job": job.to_string(), "cb": cb, "etok": etok}})
}

/// UZ: kalitni `encode_token` bilan shifrlaydi (openssl AES-256-CBC, PBKDF2 200000, base64 bir qator). Workflow shuni `secrets.ENCODE_TOKEN` bilan ochadi.
/// EN: encrypts the key with `encode_token` (openssl AES-256-CBC, PBKDF2 200000, single-line base64); the workflow decrypts it with `secrets.ENCODE_TOKEN`.
async fn encrypt_tok(cfg: &Cfg, tok: &str) -> Result<String> {
    use tokio::io::AsyncWriteExt;
    let key = std::fs::read_to_string(format!("{}/encode_token", cfg.data_dir))?.trim().to_string();
    let mut c = tokio::process::Command::new("openssl")
        .args(["enc", "-aes-256-cbc", "-pbkdf2", "-iter", "200000", "-salt", "-a", "-A", "-pass", "env:K"])
        .env("K", key)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()?;
    c.stdin.take().ok_or_else(|| anyhow!("openssl stdin yo'q"))?.write_all(tok.as_bytes()).await?;
    let o = c.wait_with_output().await?;
    let t = String::from_utf8_lossy(&o.stdout).trim().to_string();
    if !o.status.success() || t.is_empty() {
        return Err(anyhow!("kalitni shifrlab bo'lmadi"));
    }
    Ok(t)
}

async fn dispatch(cfg: &Cfg, job: i64, tok: &str) -> Result<()> {
    let repo = cfg.gh_repo.as_deref().ok_or_else(|| anyhow!("GH_REPO yo'q"))?;
    let cb = cfg.public_url.as_deref().ok_or_else(|| anyhow!("PUBLIC_URL yo'q"))?;
    let t = token(cfg).await?;
    let r = reqwest::Client::new()
        .post(format!("https://api.github.com/repos/{repo}/actions/workflows/ytjob.yml/dispatches"))
        .header("Authorization", format!("Bearer {t}"))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", "ytbot")
        .json(&dispatch_body(job, cb, &encrypt_tok(cfg, tok).await?))
        .send()
        .await?;
    if r.status().as_u16() != 204 {
        let code = r.status();
        let body: String = r.text().await.unwrap_or_default().chars().take(200).collect();
        return Err(anyhow!("GitHub {code}: {body}"));
    }
    Ok(())
}

/// UZ: Ishni Actions'ga topshiradi va tugaguncha kutadi (navbat baribir birin-ketin). EN: hands the job to Actions and waits (the queue stays one-by-one).
pub async fn run_actions(app: &Arc<App>, job: &db::Job) -> Result<()> {
    let tok = new_tok();
    db::kv_set(&app.db, &format!("ajtok:{}", job.id), &tok).await?;
    db::kv_set(&app.db, &format!("ajstate:{}", job.id), "queued").await?;
    db::kv_set(&app.db, &format!("ajerr:{}", job.id), "").await?;
    app.bot.edit(job.chat_id, job.status_msg, &format!("☁️ #{} GitHub Actions'ga topshirilmoqda…\n{}", job.id, job.title.chars().take(100).collect::<String>()), None).await;
    let res = async {
        dispatch(&app.cfg, job.id, &tok).await?;
        let t0 = std::time::Instant::now();
        loop {
            tokio::time::sleep(Duration::from_secs(3)).await;
            if db::job_state(&app.db, job.id).await == "cancelled" {
                return Err(anyhow!("bekor qilindi"));
            }
            match db::kv_get(&app.db, &format!("ajstate:{}", job.id)).await.as_deref() {
                Some("done") => return Ok(()),
                Some("fail") => return Err(anyhow!("{}", db::kv_get(&app.db, &format!("ajerr:{}", job.id)).await.unwrap_or_else(|| "Actions xatosi".into()))),
                Some("queued") if t0.elapsed() > Duration::from_secs(600) => return Err(anyhow!("Actions 10 daqiqada ishga tushmadi (workflow yoki token tekshirilsin)")),
                _ if t0.elapsed() > Duration::from_secs(6 * 3600 + 1200) => return Err(anyhow!("Actions vaqti tugadi")),
                _ => {}
            }
        }
    }
    .await;
    // Ishga xos kalit o'chiriladi: endi hech kim u bilan kira olmaydi.
    let _ = db::kv_set(&app.db, &format!("ajtok:{}", job.id), "").await;
    res
}

async fn auth(app: &App, job: i64, tok: &str) -> bool {
    let want = db::kv_get(&app.db, &format!("ajtok:{job}")).await.unwrap_or_default();
    same(&want, tok)
}

/// `?tok=...&job=...` so'rov qatorlari. EN: `?tok=...&job=...` query string.
pub struct TokQ {
    tok: String,
    job: i64,
}

impl<S: Send + Sync> axum::extract::FromRequestParts<S> for TokQ {
    type Rejection = StatusCode;
    async fn from_request_parts(parts: &mut axum::http::request::Parts, _: &S) -> Result<Self, Self::Rejection> {
        let q = parts.uri.query().unwrap_or("");
        let get = |k: &str| q.split('&').find_map(|kv| kv.strip_prefix(&format!("{k}="))).unwrap_or("").to_string();
        Ok(TokQ { tok: get("tok"), job: get("job").parse().unwrap_or(0) })
    }
}

/// GET /ajob/{job}?tok= — ishchi (Actions) uchun ish ma'lumoti. Bot tokeni faqat shu ishga xos kalit bilan beriladi.
pub async fn ajob_info(State(app): State<Arc<App>>, AxPath(job): AxPath<i64>, q: TokQ) -> Response {
    if !auth(&app, job, &q.tok).await {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(j) = db::job_get(&app.db, job).await else { return StatusCode::NOT_FOUND.into_response() };
    let meta: Value = serde_json::from_str(&j.meta).unwrap_or(json!({}));
    let has_cookies = dl::cookie_file(&app.cfg).is_some();
    axum::Json(json!({"chat_id": j.chat_id, "title": j.title, "url": j.url, "fmt": j.fmt, "meta": meta, "bot_token": app.cfg.bot_token, "bot_username": app.bot_username, "cookies": has_cookies})).into_response()
}

/// GET /ack/{job}?tok= — cookies.txt (GitHub'ga saqlanmaydi, faqat ish davomida olinadi).
pub async fn ajob_cookies(State(app): State<Arc<App>>, AxPath(job): AxPath<i64>, q: TokQ) -> Response {
    if !auth(&app, job, &q.tok).await {
        return StatusCode::FORBIDDEN.into_response();
    }
    match dl::cookie_file(&app.cfg).and_then(|p| std::fs::read_to_string(p).ok()) {
        Some(t) => t.into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// GET /bin?job=&tok= — ishchi uchun shu dasturning o'zi (`ytbot ajob`).
pub async fn ajob_bin(State(app): State<Arc<App>>, q: TokQ, headers: HeaderMap) -> Response {
    let tok = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer ")).unwrap_or(&q.tok).to_string();
    if !auth(&app, q.job, &tok).await {
        return StatusCode::FORBIDDEN.into_response();
    }
    match std::env::current_exe().and_then(std::fs::read) {
        Ok(b) => ([("content-type", "application/octet-stream")], b).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}


/// POST /prog/{job} — Actions har soniyada log yuboradi; chat xabari kamida 3 soniyada bir yangilanadi. Javobda `cancel` bo'lsa ishchi to'xtaydi.
pub async fn ajob_prog(State(app): State<Arc<App>>, AxPath(job): AxPath<i64>, axum::Json(p): axum::Json<Value>) -> Response {
    let sget = |k: &str| p[k].as_str().unwrap_or("").to_string();
    let (ptok, ptext, pstate, perr) = (sget("tok"), sget("text"), sget("state"), sget("err"));
    if !auth(&app, job, &ptok).await {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(j) = db::job_get(&app.db, job).await else { return StatusCode::NOT_FOUND.into_response() };
    if db::job_state(&app.db, job).await == "cancelled" {
        return axum::Json(json!({"cancel": true})).into_response();
    }
    match pstate.as_str() {
        "done" | "fail" => {
            let _ = db::kv_set(&app.db, &format!("ajerr:{job}"), &perr).await;
            let _ = db::kv_set(&app.db, &format!("ajstate:{job}"), &pstate).await;
            if pstate == "done" && !ptext.is_empty() {
                app.bot.edit(j.chat_id, j.status_msg, &ptext, None).await;
            }
        }
        _ => {
            let _ = db::kv_set(&app.db, &format!("ajstate:{job}"), "run").await;
            if !ptext.is_empty() && may_edit(job) {
                app.bot.edit(j.chat_id, j.status_msg, &ptext.chars().take(3500).collect::<String>(), None).await;
            }
        }
    }
    axum::Json(json!({"cancel": false})).into_response()
}

/// UZ: bu ish uchun oxirgi yangilanishdan 3 soniya o'tdimi (o'tgan bo'lsa belgilab `true`). EN: true if 3 s passed since the last edit for this job (and records it).
fn may_edit(job: i64) -> bool {
    let mut g = LAST_EDIT.lock().unwrap_or_else(|e| e.into_inner());
    let m = g.get_or_insert_with(HashMap::new);
    let now = std::time::Instant::now();
    match m.get(&job) {
        Some(t) if now.duration_since(*t) < EDIT_EVERY => false,
        _ => {
            m.insert(job, now);
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_and_token_helpers() {
        let b = dispatch_body(7, "https://x.ample.run", "abc");
        assert_eq!(b["ref"], "main");
        assert_eq!(b["inputs"]["job"], "7");
        assert_eq!(b["inputs"]["etok"], "abc");
        assert!(same("abc", "abc") && !same("abc", "abd") && !same("", "") && !same("ab", "abc"));
        let t = new_tok();
        assert_eq!(t.len(), 32);
        assert_ne!(t, new_tok());
    }

    #[test]
    fn edits_are_throttled_to_3_seconds() {
        assert!(may_edit(990_001));
        assert!(!may_edit(990_001));
        assert!(may_edit(990_002)); // boshqa ish — alohida hisob
    }

    #[tokio::test]
    async fn encrypted_token_roundtrip() {
        if std::process::Command::new("openssl").arg("version").output().is_err() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("ytbot-gh-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("encode_token"), "kalit-123\n").unwrap();
        std::fs::write(dir.join("plain"), "ghp_TEST_TOKEN\n").unwrap();
        let ok = std::process::Command::new("openssl")
            .args(["enc", "-aes-256-cbc", "-pbkdf2", "-iter", "200000", "-salt", "-pass", "pass:kalit-123", "-in"])
            .arg(dir.join("plain"))
            .arg("-out")
            .arg(dir.join("gh.enc"))
            .status()
            .unwrap()
            .success();
        assert!(ok);
        let cfg = Cfg { bot_token: "x".into(), data_dir: dir.to_string_lossy().to_string(), ytdlp: "yt-dlp".into(), cookies: None, owner_env: None, api_id: None, public_url: Some("https://x".into()), webhook_secret: String::new(), port: 0, gh_repo: Some("o/r".into()) };
        assert!(configured(&cfg));
        assert_eq!(token(&cfg).await.unwrap(), "ghp_TEST_TOKEN");
        // ishga xos kalit: shifrlanadi va workflow'dagi kabi ochiladi
        let e = encrypt_tok(&cfg, "job-kalit-xyz").await.unwrap();
        assert!(!e.contains("job-kalit") && !e.contains('\n'));
        let d = std::process::Command::new("openssl").args(["enc", "-d", "-aes-256-cbc", "-pbkdf2", "-iter", "200000", "-a", "-A", "-pass", "pass:kalit-123"]).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn().unwrap();
        use std::io::Write;
        d.stdin.as_ref().unwrap();
        let mut d = d;
        d.stdin.take().unwrap().write_all(e.as_bytes()).unwrap();
        assert_eq!(String::from_utf8_lossy(&d.wait_with_output().unwrap().stdout).trim(), "job-kalit-xyz");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn http_flow_auth_progress_cancel() {
        let dir = std::env::temp_dir().join(format!("ytbot-http-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let cfg = Arc::new(Cfg { bot_token: "SIR-BOT-TOKEN".into(), data_dir: dir.to_string_lossy().to_string(), ytdlp: "yt-dlp".into(), cookies: None, owner_env: None, api_id: None, public_url: None, webhook_secret: String::new(), port: 0, gh_repo: None });
        let conn = db::open(&format!("{}/t.db", cfg.data_dir)).await.unwrap();
        let app = Arc::new(App { cfg, db: conn, bot: Bot::new("x"), acc: None, bot_username: String::new(), peer: Mutex::new(None) });
        let id = db::job_new(&app.db, 5, "https://x/v", "Sarlavha", &json!({"w": 640, "h": 360}).to_string()).await.unwrap();
        db::kv_set(&app.db, &format!("ajtok:{id}"), "tok123456").await.unwrap();
        let router = axum::Router::new()
            .route("/prog/{job}", axum::routing::post(ajob_prog))
            .route("/ajob/{job}", axum::routing::get(ajob_info))
            .route("/ack/{job}", axum::routing::get(ajob_cookies))
            .with_state(Arc::clone(&app));
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        tokio::spawn(async move { let _ = axum::serve(l, router).await; });
        let c = reqwest::Client::new();
        let base = format!("http://{addr}");
        // noto'g'ri kalit — 403
        assert_eq!(c.get(format!("{base}/ajob/{id}?tok=boshqa")).send().await.unwrap().status().as_u16(), 403);
        // to'g'ri kalit — ma'lumot; bot tokeni faqat shu yerda
        let info: Value = c.get(format!("{base}/ajob/{id}?tok=tok123456")).send().await.unwrap().json().await.unwrap();
        assert_eq!(info["chat_id"], 5);
        assert_eq!(info["bot_token"], "SIR-BOT-TOKEN");
        assert_eq!(info["cookies"], false);
        assert_eq!(c.get(format!("{base}/ack/{id}?tok=tok123456")).send().await.unwrap().status().as_u16(), 404);
        // log yuborish
        let r: Value = c.post(format!("{base}/prog/{id}")).json(&json!({"tok": "tok123456", "state": "run", "text": "10%"})).send().await.unwrap().json().await.unwrap();
        assert_eq!(r["cancel"], false);
        assert_eq!(db::kv_get(&app.db, &format!("ajstate:{id}")).await.as_deref(), Some("run"));
        assert_eq!(c.post(format!("{base}/prog/{id}")).json(&json!({"tok": "x", "state": "run"})).send().await.unwrap().status().as_u16(), 403);
        // xato bilan tugash
        c.post(format!("{base}/prog/{id}")).json(&json!({"tok": "tok123456", "state": "fail", "err": "yuklab bo'lmadi"})).send().await.unwrap();
        assert_eq!(db::kv_get(&app.db, &format!("ajstate:{id}")).await.as_deref(), Some("fail"));
        assert_eq!(db::kv_get(&app.db, &format!("ajerr:{id}")).await.as_deref(), Some("yuklab bo'lmadi"));
        // bekor qilingan ish — cancel:true
        db::job_cancel(&app.db, id).await;
        let r: Value = c.post(format!("{base}/prog/{id}")).json(&json!({"tok": "tok123456", "state": "run", "text": "x"})).send().await.unwrap().json().await.unwrap();
        assert_eq!(r["cancel"], true);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
