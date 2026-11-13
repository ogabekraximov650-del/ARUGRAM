// UZ: bot mantig'i: Bot API (webhook yoki long polling), navbat ishchisi, video ~45 MB li qismlarga bo'linib yuboriladi. EN: bot logic: Bot API, queue worker, video sent in ~45 MB parts.
use crate::{db, dl, Cfg};
use anyhow::{anyhow, Result};
use libsql::Connection;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Foiz chizig'i: ▓▓▓▓░░░░░░ 40%
fn bar(pct: f64) -> String {
    let p = pct.clamp(0.0, 100.0);
    let n = (p / 10.0).round() as usize;
    format!("{}{} {:.0}%", "▓".repeat(n), "░".repeat(10 - n), p)
}

pub struct Bot {
    http: reqwest::Client,
    /// Fayl yuborish uchun alohida mijoz: 50 MB lik qism sekin tarmoqda daqiqalab yuboriladi.
    up: reqwest::Client,
    base: String,
}

impl Bot {
    fn new(token: &str) -> Self {
        Self {
            http: reqwest::Client::builder().timeout(Duration::from_secs(70)).build().unwrap(),
            up: reqwest::Client::builder().timeout(Duration::from_secs(900)).build().unwrap(),
            base: format!("https://api.telegram.org/bot{token}"),
        }
    }

    async fn call(&self, method: &str, body: Value) -> Result<Value> {
        let r: Value = self.http.post(format!("{}/{method}", self.base)).json(&body).send().await?.json().await?;
        if r["ok"].as_bool() == Some(true) {
            Ok(r["result"].clone())
        } else {
            Err(anyhow!("{}", r["description"].as_str().unwrap_or("Bot API xatosi")))
        }
    }

    /// Xabar yuboradi, `message_id` qaytadi.
    async fn send(&self, chat: i64, text: &str, markup: Option<Value>) -> Result<i64> {
        let mut b = json!({"chat_id": chat, "text": text, "disable_web_page_preview": true});
        if let Some(m) = markup {
            b["reply_markup"] = m;
        }
        Ok(self.call("sendMessage", b).await?["message_id"].as_i64().unwrap_or(0))
    }

    async fn edit(&self, chat: i64, msg: i64, text: &str, markup: Option<Value>) {
        let mut b = json!({"chat_id": chat, "message_id": msg, "text": text, "disable_web_page_preview": true});
        if let Some(m) = markup {
            b["reply_markup"] = m;
        }
        let _ = self.call("editMessageText", b).await;
    }

    /// Faylni (video yoki audio) yuboradi; yuborilgan baytlar `sent` ga yoziladi (foiz uchun). 429 (`retry_after`) va tarmoq xatolarida 3 martagacha qayta uriniladi.
    async fn send_file(&self, chat: i64, audio: bool, path: &Path, caption: &str, dur: u64, w: i64, h: i64, thumb: Option<&Path>, sent: Arc<AtomicU64>) -> Result<()> {
        use futures_util::StreamExt;
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or(if audio { "audio.mp3" } else { "video.mp4" }).to_string();
        let len = tokio::fs::metadata(path).await?.len();
        let thumb_bytes = match thumb {
            Some(t) => tokio::fs::read(t).await.ok(),
            None => None,
        };
        let mut last = anyhow!("yuborilmadi");
        for attempt in 0..3 {
            sent.store(0, Ordering::Relaxed);
            let (method, field, mime) = if audio { ("sendAudio", "audio", "audio/mpeg") } else { ("sendVideo", "video", "video/mp4") };
            let mut form = reqwest::multipart::Form::new().text("chat_id", chat.to_string()).text("caption", caption.chars().take(1000).collect::<String>());
            if !audio {
                form = form.text("supports_streaming", "true").text("duration", dur.to_string());
                if w > 0 && h > 0 {
                    form = form.text("width", w.to_string()).text("height", h.to_string());
                }
            } else {
                form = form.text("duration", dur.to_string());
            }
            let file = tokio::fs::File::open(path).await?;
            let counter = Arc::clone(&sent);
            let stream = tokio_util::io::ReaderStream::new(file).map(move |chunk| {
                if let Ok(b) = &chunk {
                    counter.fetch_add(b.len() as u64, Ordering::Relaxed);
                }
                chunk
            });
            form = form.part(field, reqwest::multipart::Part::stream_with_length(reqwest::Body::wrap_stream(stream), len).file_name(name.clone()).mime_str(mime)?);
            if let Some(tb) = &thumb_bytes {
                form = form.part("thumbnail", reqwest::multipart::Part::bytes(tb.clone()).file_name("thumb.jpg").mime_str("image/jpeg")?);
            }
            match self.up.post(format!("{}/{method}", self.base)).multipart(form).send().await {
                Ok(resp) => {
                    let r: Value = resp.json().await.unwrap_or(json!({}));
                    if r["ok"].as_bool() == Some(true) {
                        sent.store(len, Ordering::Relaxed);
                        return Ok(());
                    }
                    let wait = r["parameters"]["retry_after"].as_u64();
                    last = anyhow!("{}", r["description"].as_str().unwrap_or("Bot API xatosi"));
                    match wait {
                        Some(s) if attempt < 2 => tokio::time::sleep(Duration::from_secs(s.min(120) + 1)).await,
                        _ if attempt < 2 && !last.to_string().contains("too big") => tokio::time::sleep(Duration::from_secs(3)).await,
                        _ => break,
                    }
                }
                Err(e) => {
                    last = anyhow!("tarmoq: {e}");
                    tokio::time::sleep(Duration::from_secs(3 * (attempt as u64 + 1))).await;
                }
            }
        }
        Err(last)
    }
}

pub struct App {
    cfg: Arc<Cfg>,
    db: Connection,
    bot: Bot,
}

pub async fn run(cfg: Arc<Cfg>) -> Result<()> {
    let conn = db::open(&format!("{}/ytbot.db", cfg.data_dir)).await?;
    let bot = Bot::new(&cfg.bot_token);
    let me = bot.call("getMe", json!({})).await?;
    eprintln!("bot: @{}", me["username"].as_str().unwrap_or(""));
    let app = Arc::new(App { cfg, db: conn, bot });
    let w = Arc::clone(&app);
    tokio::spawn(async move { worker(w).await });

    // HTTP: sog'liq tekshiruvi (`/`) va webhook yo'li. Ample xizmatni shu orqali "tirik" deb biladi va so'rov kelganda uyg'otadi.
    let router = axum::Router::new()
        .route("/", axum::routing::get(|| async { "ok" }))
        .route("/tg/{path_secret}", axum::routing::post(webhook))
        .with_state(Arc::clone(&app));
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", app.cfg.port)).await?;
    if let Some(public) = app.cfg.public_url.clone() {
        // ── WEBHOOK: Telegram yangilanishlarni Ample manziliga yuboradi ──
        let secret = app.cfg.webhook_secret.clone();
        let url = format!("{public}/tg/{secret}");
        app.bot.call("setWebhook", json!({"url": url, "secret_token": secret, "allowed_updates": ["message", "callback_query"], "max_connections": 4})).await?;
        eprintln!("webhook o'rnatildi: {public}/tg/***");
        axum::serve(listener, router).await?;
        return Ok(());
    }
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });

    // ── Zaxira: long polling (PUBLIC_URL berilmagan bo'lsa) ──
    let _ = app.bot.call("deleteWebhook", json!({})).await;
    let mut offset = 0i64;
    loop {
        let r = app.bot.call("getUpdates", json!({"offset": offset, "timeout": 25, "allowed_updates": ["message", "callback_query"]})).await;
        let list = match r {
            Ok(v) => v.as_array().cloned().unwrap_or_default(),
            Err(e) => {
                eprintln!("getUpdates xato: {e}");
                tokio::time::sleep(Duration::from_secs(3)).await;
                continue;
            }
        };
        for u in list {
            offset = offset.max(u["update_id"].as_i64().unwrap_or(0) + 1);
            handle_update(Arc::clone(&app), u);
        }
    }
}

fn handle_update(app: Arc<App>, u: Value) {
    tokio::spawn(async move {
        let kind = if u["message"].is_object() { "message" } else if u["callback_query"].is_object() { "callback" } else { "boshqa" };
        let head: String = u["message"]["text"].as_str().or_else(|| u["callback_query"]["data"].as_str()).unwrap_or("").chars().take(12).collect();
        eprintln!("yangilanish: {kind} {head}");
        let r = if u["message"].is_object() { on_message(&app, &u["message"]).await } else if u["callback_query"].is_object() { on_callback(&app, &u["callback_query"]).await } else { Ok(()) };
        if let Err(e) = r {
            eprintln!("yangilanish xatosi: {e}");
        }
    });
}

/// POST /tg/<sir>: manzildagi sir VA Telegram sarlavhasi mos bo'lsagina qabul qilinadi; darhol 200 qaytadi, ish fonda bajariladi.
async fn webhook(
    axum::extract::State(app): axum::extract::State<Arc<App>>,
    axum::extract::Path(path_secret): axum::extract::Path<String>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> axum::http::StatusCode {
    let hdr = headers.get("x-telegram-bot-api-secret-token").and_then(|v| v.to_str().ok()).unwrap_or("");
    if path_secret != app.cfg.webhook_secret || hdr != app.cfg.webhook_secret {
        return axum::http::StatusCode::FORBIDDEN;
    }
    match serde_json::from_slice::<Value>(&body) {
        Ok(u) => {
            handle_update(app, u);
            axum::http::StatusCode::OK
        }
        Err(_) => axum::http::StatusCode::BAD_REQUEST,
    }
}

async fn owner(app: &App) -> i64 {
    if let Some(o) = app.cfg.owner_env {
        return o;
    }
    db::kv_get(&app.db, "owner").await.and_then(|v| v.parse().ok()).unwrap_or(0)
}

const HELP: &str = "Havolani yuboring (YouTube, Instagram, TikTok, X va boshqalar) — sifatni tanlaysiz, video navbat bilan yuklanadi.\n\nKatta video ~45 MB li qismlarga bo'lib yuboriladi (bot limiti 50 MB).\n\n/queue — navbat\n/cancel <raqam> — bekor qilish";

async fn on_message(app: &Arc<App>, msg: &Value) -> Result<()> {
    let chat = msg["chat"]["id"].as_i64().unwrap_or(0);
    let from = msg["from"]["id"].as_i64().unwrap_or(0);
    let text = msg["text"].as_str().unwrap_or("").trim().to_string();
    if chat == 0 || msg["chat"]["type"].as_str() != Some("private") {
        return Ok(());
    }
    let mut own = owner(app).await;
    if own == 0 && text.starts_with("/start") {
        db::kv_set(&app.db, "owner", &from.to_string()).await?;
        own = from;
    }
    if own == 0 || from != own {
        let _ = app.bot.send(chat, "Bu bot shaxsiy.", None).await;
        return Ok(());
    }
    let cmd = text.split_whitespace().next().unwrap_or("");
    match cmd {
        "/start" | "/help" => {
            app.bot.send(chat, HELP, None).await?;
            return Ok(());
        }
        "/queue" => {
            let q = db::queue_list(&app.db).await;
            let t = if q.is_empty() { "Navbat bo'sh.".to_string() } else { q.iter().map(|(id, t, s)| format!("#{id} {} — {}", if s == "running" { "▶️" } else { "⏳" }, t.chars().take(60).collect::<String>())).collect::<Vec<_>>().join("\n") };
            app.bot.send(chat, &t, None).await?;
            return Ok(());
        }
        "/cancel" => {
            let id: i64 = text.split_whitespace().nth(1).and_then(|v| v.trim_start_matches('#').parse().ok()).unwrap_or(0);
            let ok = id > 0 && db::job_cancel(&app.db, id).await;
            app.bot.send(chat, if ok { "Bekor qilindi." } else { "Bunday ish topilmadi." }, None).await?;
            return Ok(());
        }
        _ => {}
    }
    if let Some(url) = text.split_whitespace().find(|w| w.starts_with("http://") || w.starts_with("https://")) {
        return handle_url(app, chat, url).await;
    }
    app.bot.send(chat, HELP, None).await?;
    Ok(())
}

async fn handle_url(app: &Arc<App>, chat: i64, url: &str) -> Result<()> {
    let st = app.bot.send(chat, "⏳ Ma'lumot olinmoqda…", None).await?;
    let info = match dl::probe(&app.cfg, url).await {
        Ok(i) => i,
        Err(e) => {
            app.bot.edit(chat, st, &format!("❌ {e}"), None).await;
            return Ok(());
        }
    };
    let direct: serde_json::Map<String, Value> = info.direct.iter().map(|(h, u)| (h.to_string(), json!(u))).collect();
    let meta = json!({"w": info.w, "h": info.h, "dur": info.duration, "direct": direct});
    let id = db::job_new(&app.db, chat, url, &info.title, &meta.to_string()).await?;
    let mut rows: Vec<Value> = Vec::new();
    let mut row: Vec<Value> = Vec::new();
    for o in &info.opts {
        let label = format!("{}p{}", o.h, o.size.map(|s| format!(" ~{}", dl::human(s))).unwrap_or_default());
        row.push(json!({"text": label, "callback_data": format!("q:{id}:{}", o.h)}));
        if row.len() == 2 {
            rows.push(Value::Array(std::mem::take(&mut row)));
        }
    }
    if !row.is_empty() {
        rows.push(Value::Array(row));
    }
    if info.opts.is_empty() {
        // Sifat ro'yxati yo'q: mavjud faylni yuklab olish tugmasi.
        rows.push(json!([{"text": "⬇️ Yuklab olish (mavjud fayl)", "callback_data": format!("q:{id}:b")}]));
    }
    rows.push(json!([{"text": format!("🎵 Audio{}", info.audio_size.map(|s| format!(" ~{}", dl::human(s))).unwrap_or_default()), "callback_data": format!("q:{id}:a")}, {"text": "✖️ Bekor", "callback_data": format!("x:{id}")}]));
    let dur = if info.duration > 0 { format!("\n⏱ {}:{:02}", info.duration / 60, info.duration % 60) } else { String::new() };
    app.bot.edit(chat, st, &format!("🎬 {}{dur}\n\n{}", info.title.chars().take(200).collect::<String>(), if info.opts.is_empty() { "Sifatlar ro'yxati yo'q — mavjud fayl yuklanadi:" } else { "Sifatni tanlang:" }), Some(json!({"inline_keyboard": rows}))).await;
    Ok(())
}

async fn on_callback(app: &Arc<App>, cb: &Value) -> Result<()> {
    let cid = cb["id"].as_str().unwrap_or("");
    let from = cb["from"]["id"].as_i64().unwrap_or(0);
    let data = cb["data"].as_str().unwrap_or("");
    let chat = cb["message"]["chat"]["id"].as_i64().unwrap_or(0);
    let msg = cb["message"]["message_id"].as_i64().unwrap_or(0);
    if from != owner(app).await {
        let _ = app.bot.call("answerCallbackQuery", json!({"callback_query_id": cid})).await;
        return Ok(());
    }
    let parts: Vec<&str> = data.split(':').collect();
    match parts.as_slice() {
        ["q", id, fmt] => {
            let id: i64 = id.parse().unwrap_or(0);
            if db::job_enqueue(&app.db, id, fmt, msg).await? {
                let ahead = db::queue_list(&app.db).await.iter().filter(|(i, _, _)| *i < id).count();
                app.bot.edit(chat, msg, &format!("✅ Navbatga qo'yildi (#{id}). Oldinda: {ahead} ta."), None).await;
            }
            let _ = app.bot.call("answerCallbackQuery", json!({"callback_query_id": cid})).await;
        }
        ["x", id] => {
            let id: i64 = id.parse().unwrap_or(0);
            db::job_cancel(&app.db, id).await;
            app.bot.edit(chat, msg, "Bekor qilindi.", None).await;
            let _ = app.bot.call("answerCallbackQuery", json!({"callback_query_id": cid})).await;
        }
        _ => {
            let _ = app.bot.call("answerCallbackQuery", json!({"callback_query_id": cid})).await;
        }
    }
    Ok(())
}

// ── Navbat ishchisi: birin-ketin ──────────────────────────────

async fn worker(app: Arc<App>) {
    loop {
        match db::job_take(&app.db).await {
            Some(job) => {
                let id = job.id;
                let res = run_job(&app, &job).await;
                let st = db::job_state(&app.db, id).await;
                match res {
                    Ok(()) => db::job_finish(&app.db, id, "done", "").await,
                    Err(e) if st == "cancelled" => {
                        eprintln!("#{id} bekor qilindi: {e}");
                        app.bot.edit(job.chat_id, job.status_msg, "Bekor qilindi.", None).await;
                    }
                    Err(e) => {
                        eprintln!("#{id} xato: {e}");
                        db::job_finish(&app.db, id, "failed", &e.to_string()).await;
                        app.bot.edit(job.chat_id, job.status_msg, &format!("❌ #{id}: {e}"), None).await;
                    }
                }
                let _ = std::fs::remove_dir_all(format!("{}/tmp/{id}", app.cfg.data_dir));
            }
            None => tokio::time::sleep(Duration::from_secs(2)).await,
        }
    }
}

/// Bekor qilingunga qadar kutadi (ish holatini har 3 soniyada tekshiradi).
async fn cancelled(app: &App, id: i64) {
    loop {
        tokio::time::sleep(Duration::from_secs(3)).await;
        if db::job_state(&app.db, id).await == "cancelled" {
            return;
        }
    }
}

async fn run_job(app: &Arc<App>, job: &db::Job) -> Result<()> {
    let meta: Value = serde_json::from_str(&job.meta).unwrap_or(json!({}));
    let (w, h) = (meta["w"].as_i64().unwrap_or(0), meta["h"].as_i64().unwrap_or(0));
    let is_audio = job.fmt == "a";
    let title: String = job.title.chars().take(100).collect();
    app.bot.edit(job.chat_id, job.status_msg, &format!("⏬ #{} yuklanmoqda…\n{title}", job.id), None).await;

    // 1) Yuklash (progress har 5 soniyada).
    let dir = std::path::PathBuf::from(format!("{}/tmp/{}", app.cfg.data_dir, job.id));
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let a2 = Arc::clone(app);
    let (chat, mid, jid, t80) = (job.chat_id, job.status_msg, job.id, job.title.chars().take(80).collect::<String>());
    let edit_task = tokio::spawn(async move {
        let mut last = std::time::Instant::now() - Duration::from_secs(10);
        while let Some(p) = rx.recv().await {
            if last.elapsed() >= Duration::from_secs(3) {
                last = std::time::Instant::now();
                let mut it = p.split('|');
                let (pc, sp, eta) = (it.next().unwrap_or("").trim().to_string(), it.next().unwrap_or("").trim().to_string(), it.next().unwrap_or("").trim().to_string());
                let pct: f64 = pc.trim_end_matches('%').trim().parse().unwrap_or(0.0);
                a2.bot.edit(chat, mid, &format!("⏬ #{jid} yuklab olinmoqda\n{}\n{sp} · qoldi {eta}\n{t80}", bar(pct)), None).await;
            }
        }
    });
    // To'g'ridan-to'g'ri havolalar bo'lsa (masalan Rumble) — yt-dlp shu fayl havolasini yuklaydi.
    let (src_url, src_fmt) = match meta["direct"].as_object().filter(|m| !m.is_empty()) {
        Some(m) if is_audio => (m.iter().filter_map(|(k, u)| Some((k.parse::<i64>().ok()?, u.as_str()?))).min_by_key(|(h, _)| *h).map(|(_, u)| u).unwrap_or(&job.url).to_string(), "a".to_string()), // audio uchun eng kichik fayl
        Some(m) => (m.get(&job.fmt).and_then(|u| u.as_str()).unwrap_or(&job.url).to_string(), "d".to_string()),
        None => (job.url.clone(), job.fmt.clone()),
    };
    let r = tokio::select! {
        r = dl::download_file(&app.cfg, &src_url, &src_fmt, &dir, |p| { let _ = tx.send(p); }) => r,
        _ = cancelled(app, job.id) => Err(anyhow!("bekor qilindi")),
    };
    drop(tx);
    let _ = edit_task.await;
    let path = r?;

    // 2) VAQT bo'yicha ~45 MB li qismlarga bo'lish (kerak bo'lsa).
    let size = std::fs::metadata(&path)?.len();
    app.bot.edit(job.chat_id, job.status_msg, &format!("✂️ #{} {} — qismlarga bo'linmoqda…", job.id, dl::human(size)), None).await;
    let split_pct = Arc::new(AtomicU32::new(0));
    let split_tick = {
        let (a2, sp, chat, mid, jid) = (Arc::clone(app), Arc::clone(&split_pct), job.chat_id, job.status_msg, job.id);
        let sz = dl::human(size);
        tokio::spawn(async move {
            let mut first = true;
            loop {
                tokio::time::sleep(Duration::from_millis(if first { 1200 } else { 3000 })).await;
                first = false;
                a2.bot.edit(chat, mid, &format!("✂️ #{jid} qismlarga bo'linmoqda ({sz})\n{}", bar(sp.load(Ordering::Relaxed) as f64)), None).await;
            }
        })
    };
    let parts = tokio::select! {
        r = dl::split(&path, &dir, Arc::clone(&split_pct)) => r,
        _ = cancelled(app, job.id) => Err(anyhow!("bekor qilindi")),
    };
    split_tick.abort();
    let parts = parts?;
    let n = parts.len();

    // 3) Qismlarni birin-ketin yuborish; yuborilgan qism darhol o'chiriladi (disk to'lmasin).
    for (i, part) in parts.iter().enumerate() {
        if db::job_state(&app.db, job.id).await == "cancelled" {
            return Err(anyhow!("bekor qilindi"));
        }
        let psize = std::fs::metadata(part)?.len();
        app.bot.edit(job.chat_id, job.status_msg, &format!("📤 #{} qism {}/{} yuborilmoqda ({})…\n{title}", job.id, i + 1, n, dl::human(psize)), None).await;
        let dur = dl::probe_duration(part).await.round() as u64;
        // Kichik rasm (thumbnail): faqat video uchun; yasalmasa Telegram o'zi tanlaydi.
        let thumb = if is_audio { None } else { dl::make_thumb(part, &dir.join(format!("thumb{i}.jpg"))).await };
        let caption = if n > 1 { format!("{} ({}/{})", job.title.chars().take(900).collect::<String>(), i + 1, n) } else { job.title.chars().take(900).collect::<String>() };
        let sent = Arc::new(AtomicU64::new(0));
        let up_tick = {
            let (a2, sn, chat, mid, jid, t100) = (Arc::clone(app), Arc::clone(&sent), job.chat_id, job.status_msg, job.id, title.clone());
            tokio::spawn(async move {
                let mut first = true;
                loop {
                    tokio::time::sleep(Duration::from_millis(if first { 1200 } else { 3000 })).await;
                    first = false;
                    let s = sn.load(Ordering::Relaxed);
                    let pct = (s as f64 / psize.max(1) as f64 * 100.0).min(99.0);
                    let overall = ((i as f64 + pct / 100.0) / n as f64) * 100.0;
                    a2.bot.edit(chat, mid, &format!("📤 #{jid} qism {}/{} yuborilmoqda\n{}  ({} / {})\nUmumiy: {}\n{t100}", i + 1, n, bar(pct), dl::human(s), dl::human(psize), bar(overall)), None).await;
                }
            })
        };
        let r = tokio::select! {
            r = app.bot.send_file(job.chat_id, is_audio, part, &caption, dur, w, h, thumb.as_deref(), Arc::clone(&sent)) => r,
            _ = cancelled(app, job.id) => Err(anyhow!("bekor qilindi")),
        };
        up_tick.abort();
        r.map_err(|e| anyhow!("{}/{} qism yuborilmadi: {e}", i + 1, n))?;
        let _ = std::fs::remove_file(part);
    }
    app.bot.edit(job.chat_id, job.status_msg, &format!("✅ #{} tayyor: {title}{}", job.id, if n > 1 { format!("\n{n} qismda yuborildi") } else { String::new() }), None).await;
    Ok(())
}

/// Tashxis: `ytbot sendtest <fayl>` — faylni egasiga yuboradi (thumbnail va foiz hisobi bilan) va natijani chiqaradi.
pub async fn cli_sendtest(cfg: Arc<Cfg>, file: &str) -> Result<()> {
    let conn = db::open(&format!("{}/ytbot.db", cfg.data_dir)).await?;
    let bot = Bot::new(&cfg.bot_token);
    let own: i64 = db::kv_get(&conn, "owner").await.and_then(|v| v.parse().ok()).unwrap_or(0);
    if own == 0 {
        return Err(anyhow!("egasi topilmadi (botga /start yozilmagan)"));
    }
    let path = Path::new(file);
    let thumb = dl::make_thumb(path, &std::env::temp_dir().join("ytbot-test-thumb.jpg")).await;
    println!("thumbnail: {:?}", thumb.as_ref().map(|t| std::fs::metadata(t).map(|m| m.len()).unwrap_or(0)));
    let dur = dl::probe_duration(path).await.round() as u64;
    let sent = Arc::new(AtomicU64::new(0));
    let total = std::fs::metadata(path)?.len();
    let s2 = Arc::clone(&sent);
    let ticker = tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(700)).await;
            println!("yuborilmoqda: {}", bar(s2.load(Ordering::Relaxed) as f64 / total.max(1) as f64 * 100.0));
        }
    });
    let r = bot.send_file(own, false, path, "🧪 Sinov: foiz va thumbnail (o'chirib tashlashingiz mumkin)", dur, 640, 360, thumb.as_deref(), sent).await;
    ticker.abort();
    println!("natija: {:?}", r.as_ref().map(|_| "ok").map_err(|e| e.to_string()));
    r
}
