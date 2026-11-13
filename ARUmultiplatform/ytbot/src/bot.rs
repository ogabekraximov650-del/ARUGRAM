// UZ: bot mantig'i: Telegram Bot API (long polling), akkauntni kod bilan ulash, navbat ishchisi. EN: bot logic: Bot API long polling, code-based account login, queue worker.
use crate::{db, dl, Cfg};
use anyhow::{anyhow, Result};
use grammers_client::client::PasswordToken;
use grammers_client::tl;
use grammers_client::message::InputMessage;
use grammers_client::{media::Attribute, Client};
use grammers_session::storages::SqliteSession;
use grammers_session::types::PeerRef;
use libsql::Connection;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

pub struct Bot {
    http: reqwest::Client,
    base: String,
}

impl Bot {
    fn new(token: &str) -> Self {
        Self { http: reqwest::Client::builder().timeout(Duration::from_secs(70)).build().unwrap(), base: format!("https://api.telegram.org/bot{token}") }
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

    async fn delete(&self, chat: i64, msg: i64) {
        let _ = self.call("deleteMessage", json!({"chat_id": chat, "message_id": msg})).await;
    }
}

/// Yuborilgan kod: telefon, Telegram bergan `phone_code_hash`, qaysi usulda yuborilgani va keyingi usul.
struct Sent {
    phone: String,
    hash: String,
    kind: String,
    next: Option<String>,
}

enum Pending {
    None,
    Code(Sent),
    Pass(PasswordToken),
}

fn describe_type(t: &tl::enums::auth::SentCodeType) -> String {
    use tl::enums::auth::SentCodeType as T;
    match t {
        T::App(_) => "Telegram ILOVASIGA (\"Telegram\" xizmat chati, boshqa qurilmangizdagi Telegram)".into(),
        T::Sms(_) => "SMS orqali".into(),
        T::Call(_) => "telefon qo'ng'irog'i orqali (kodni robot ovozi aytadi)".into(),
        T::FlashCall(_) | T::MissedCall(_) => "o'tkazib yuborilgan qo'ng'iroq orqali (raqamning oxirgi raqamlari)".into(),
        T::FirebaseSms(_) => "SMS orqali (Google/Firebase)".into(),
        T::EmailCode(_) => "elektron pochtaga".into(),
        T::SetUpEmailRequired(_) => "pochta ulanishi talab qilinadi (bu akkaunt uchun kod pochta orqali)".into(),
        T::FragmentSms(_) => "Fragment orqali SMS".into(),
        _ => "boshqa usulda".into(),
    }
}

fn describe_next(t: &tl::enums::auth::CodeType) -> String {
    use tl::enums::auth::CodeType as C;
    match t {
        C::Sms => "SMS".into(),
        C::Call => "telefon qo'ng'irog'i".into(),
        C::FlashCall => "flash-qo'ng'iroq".into(),
        C::MissedCall => "o'tkazib yuborilgan qo'ng'iroq".into(),
        C::FragmentSms => "Fragment SMS".into(),
    }
}

fn sent_from(phone: &str, r: tl::enums::auth::SentCode) -> Result<Sent> {
    match r {
        tl::enums::auth::SentCode::Code(c) => Ok(Sent { phone: phone.to_string(), hash: c.phone_code_hash, kind: describe_type(&c.r#type), next: c.next_type.as_ref().map(describe_next) }),
        _ => Err(anyhow!("kutilmagan javob (kod yuborilmadi)")),
    }
}

/// Kodni so'raydi (usul ma'lum bo'ladi). Raqam boshqa data markazda bo'lsa kutubxona ko'chishni bajaradi, so'ng qayta so'raladi.
async fn send_code(app: &App, phone: &str) -> Result<Sent> {
    let req = tl::functions::auth::SendCode {
        phone_number: phone.to_string(),
        api_id: app.cfg.api_id,
        api_hash: app.cfg.api_hash.clone(),
        settings: tl::types::CodeSettings {
            allow_flashcall: false,
            current_number: false,
            allow_app_hash: false,
            allow_missed_call: false,
            allow_firebase: false,
            logout_tokens: None,
            token: None,
            app_sandbox: None,
            unknown_number: false,
        }
        .into(),
    };
    match app.tg.invoke(&req).await {
        Ok(r) => sent_from(phone, r),
        Err(e) if e.is("PHONE_MIGRATE_*") || e.is("NETWORK_MIGRATE_*") => {
            // Kutubxonaning o'z usuli ko'chishni bajaradi (u kod ham yuboradi), keyin usulni bilish uchun qayta so'raymiz.
            let _ = app.tg.request_login_code(phone, &app.cfg.api_hash).await;
            sent_from(phone, app.tg.invoke(&req).await?)
        }
        Err(e) => Err(anyhow!("{e}")),
    }
}

/// Keyingi usul bilan qayta yuboradi (SMS / qo'ng'iroq).
async fn resend_code(app: &App, s: &Sent) -> Result<Sent> {
    let r = app.tg.invoke(&tl::functions::auth::ResendCode { phone_number: s.phone.clone(), phone_code_hash: s.hash.clone(), reason: None }).await.map_err(|e| anyhow!("{e}"))?;
    sent_from(&s.phone, r)
}

enum SignIn {
    Ok,
    Password(PasswordToken),
    BadCode,
    Other(String),
}

async fn sign_in(app: &App, s: &Sent, code: &str) -> SignIn {
    match app.tg.invoke(&tl::functions::auth::SignIn { phone_number: s.phone.clone(), phone_code_hash: s.hash.clone(), phone_code: Some(code.to_string()), email_verification: None }).await {
        Ok(tl::enums::auth::Authorization::Authorization(_)) => SignIn::Ok,
        Ok(_) => SignIn::Other("bu raqamda Telegram akkaunt yo'q (avval ro'yxatdan o'ting)".into()),
        Err(e) if e.is("SESSION_PASSWORD_NEEDED") => match app.tg.invoke(&tl::functions::account::GetPassword {}).await {
            Ok(p) => SignIn::Password(PasswordToken::new(p.into())),
            Err(e) => SignIn::Other(e.to_string()),
        },
        Err(e) if e.is("PHONE_CODE_*") => SignIn::BadCode,
        Err(e) => SignIn::Other(e.to_string()),
    }
}

pub struct App {
    cfg: Arc<Cfg>,
    db: Connection,
    bot: Bot,
    username: String,
    tg: Client,
    pending: Mutex<Pending>,
    peer: Mutex<Option<PeerRef>>,
}

pub async fn run(cfg: Arc<Cfg>) -> Result<()> {
    let conn = db::open(&format!("{}/ytbot.db", cfg.data_dir)).await?;
    let bot = Bot::new(&cfg.bot_token);
    let me = bot.call("getMe", json!({})).await?;
    let username = me["username"].as_str().unwrap_or("").to_string();
    eprintln!("bot: @{username}");

    let session = Arc::new(SqliteSession::open(format!("{}/session.sqlite", cfg.data_dir)).await?);
    let pool = grammers_client::SenderPool::new(Arc::clone(&session), cfg.api_id);
    let tg = Client::new(pool.handle);
    tokio::spawn(pool.runner.run());
    let mut updates = pool.updates;
    tokio::spawn(async move { while updates.recv().await.is_some() {} }); // MTProto yangilanishlari kerak emas — navbat to'lib qolmasin.

    let app = Arc::new(App { cfg, db: conn, bot, username, tg, pending: Mutex::new(Pending::None), peer: Mutex::new(None) });
    // Tashxis: MTProto ulanishi va API kalitlari ishlayaptimi (yaroqsiz raqam so'raladi — hech kimga kod yuborilmaydi).
    let a0 = Arc::clone(&app);
    tokio::spawn(async move {
        let authorized = a0.tg.is_authorized().await;
        eprintln!("mtproto: is_authorized = {authorized:?}");
        match tokio::time::timeout(Duration::from_secs(30), a0.tg.request_login_code("+10000000000", &a0.cfg.api_hash)).await {
            Ok(Ok(_)) => eprintln!("mtproto: kod so'rovi ishladi (kutilmagan)"),
            Ok(Err(e)) => eprintln!("mtproto: sinov javobi (xato kutilgan edi): {e}"),
            Err(_) => eprintln!("mtproto: sinov 30 s ichida javob bermadi — ulanish muammosi"),
        }
    });
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
        let head = if head.chars().all(|c| c.is_ascii_digit() || c == ' ' || c == '+') && head.len() > 3 { "<raqam/kod>".to_string() } else { head };
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

const HELP: &str = "Havolani yuboring (YouTube, Instagram, TikTok, X va boshqalar) — sifatni tanlaysiz, video navbat bilan yuklanadi.\n\n/login — Telegram akkauntni ulash (katta fayllar uchun)\n/resend — kod kelmasa, SMS/qo'ng'iroq bilan qayta yuborish\n/queue — navbat\n/cancel <raqam> — bekor qilish\n/logout — akkauntni uzish\n\nBelgi: ⚡ — oqim bilan (diskka yozilmasdan) yuboriladi.";

async fn authorized(app: &App) -> bool {
    app.tg.is_authorized().await.unwrap_or(false)
}

async fn on_message(app: &Arc<App>, msg: &Value) -> Result<()> {
    let chat = msg["chat"]["id"].as_i64().unwrap_or(0);
    let from = msg["from"]["id"].as_i64().unwrap_or(0);
    let mid = msg["message_id"].as_i64().unwrap_or(0);
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
            let st = if authorized(app).await { "✅ Telegram akkaunt ulangan." } else { "⚠️ Akkaunt ulanmagan — /login" };
            app.bot.send(chat, &format!("{HELP}\n\n{st}"), None).await?;
            return Ok(());
        }
        "/login" => {
            db::kv_set(&app.db, "login_state", "phone").await?;
            app.bot.send(chat, "📱 Telegram akkaunt telefon raqamini yuboring (masalan +998901234567). Kod Telegram ilovasiga keladi.", None).await?;
            return Ok(());
        }
        "/resend" => {
            let prev = std::mem::replace(&mut *app.pending.lock().await, Pending::None);
            match prev {
                Pending::Code(sent) => match resend_code(app, &sent).await {
                    Ok(n) => {
                        let msg = format!("📨 Kod qayta yuborildi: {}.{}", n.kind, n.next.as_ref().map(|x| format!("\nKeyingi usul (/resend): {x}")).unwrap_or_default());
                        *app.pending.lock().await = Pending::Code(n);
                        app.bot.send(chat, &msg, None).await?;
                    }
                    Err(e) => {
                        eprintln!("resend xato: {e}");
                        *app.pending.lock().await = Pending::Code(sent);
                        app.bot.send(chat, &format!("❌ Qayta yuborib bo'lmadi: {e}\n(Telegram qayta yuborishni biroz kutishni talab qilishi mumkin.)"), None).await?;
                    }
                },
                other => {
                    *app.pending.lock().await = other;
                    app.bot.send(chat, "Avval /login va telefon raqamni yuboring.", None).await?;
                }
            }
            return Ok(());
        }
        "/logout" => {
            let _ = app.tg.sign_out().await;
            db::kv_set(&app.db, "login_state", "").await?;
            app.bot.send(chat, "Akkaunt uzildi.", None).await?;
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

    // Akkauntni ulash jarayoni: telefon -> kod -> (parol).
    let state = db::kv_get(&app.db, "login_state").await.unwrap_or_default();
    if !state.is_empty() && !text.starts_with("http") {
        return login_step(app, chat, mid, &state, &text).await;
    }

    if let Some(url) = text.split_whitespace().find(|w| w.starts_with("http://") || w.starts_with("https://")) {
        return handle_url(app, chat, url).await;
    }
    app.bot.send(chat, HELP, None).await?;
    Ok(())
}

async fn login_step(app: &Arc<App>, chat: i64, mid: i64, state: &str, text: &str) -> Result<()> {
    match state {
        "phone" => {
            let digits: String = text.chars().filter(|c| c.is_ascii_digit()).collect();
            if digits.len() < 9 {
                app.bot.send(chat, "Raqam noto'g'ri. Masalan: +998901234567", None).await?;
                return Ok(());
            }
            match send_code(app, &format!("+{digits}")).await {
                Ok(t) => {
                    eprintln!("kod yuborildi: usul = {}", t.kind);
                    let msg = format!("🔐 Kod yuborildi: {}.\nKodni raqamlar orasiga probel qo'yib yuboring: 1 2 3 4 5 (probelsiz yuborsangiz Telegram kodni bekor qilishi mumkin).{}", t.kind, t.next.as_ref().map(|x| format!("\n\nKod kelmasa /resend — keyingi usul: {x}.")).unwrap_or_else(|| "\n\nKod kelmasa /resend.".into()));
                    *app.pending.lock().await = Pending::Code(t);
                    db::kv_set(&app.db, "login_state", "code").await?;
                    app.bot.send(chat, &msg, None).await?;
                }
                Err(e) => {
                    eprintln!("kod so'rash xato: {e}");
                    db::kv_set(&app.db, "login_state", "").await?;
                    app.bot.send(chat, &format!("❌ Kod so'rab bo'lmadi: {e}"), None).await?;
                }
            }
        }
        "code" => {
            app.bot.delete(chat, mid).await;
            let code: String = text.chars().filter(|c| c.is_ascii_digit()).collect();
            let token = match std::mem::replace(&mut *app.pending.lock().await, Pending::None) {
                Pending::Code(t) => t,
                _ => {
                    db::kv_set(&app.db, "login_state", "").await?;
                    app.bot.send(chat, "Jarayon uzilgan. /login", None).await?;
                    return Ok(());
                }
            };
            match sign_in(app, &token, &code).await {
                SignIn::Ok => {
                    db::kv_set(&app.db, "login_state", "").await?;
                    app.bot.send(chat, "✅ Akkaunt ulandi. Endi havola yuborishingiz mumkin.", None).await?;
                }
                SignIn::Password(p) => {
                    *app.pending.lock().await = Pending::Pass(p);
                    db::kv_set(&app.db, "login_state", "pass").await?;
                    app.bot.send(chat, "🔑 2 bosqichli parolni yuboring (xabar o'chiriladi).", None).await?;
                }
                SignIn::BadCode => {
                    *app.pending.lock().await = Pending::Code(token);
                    app.bot.send(chat, "❌ Kod noto'g'ri yoki eskirgan. Qayta yuboring (1 2 3 4 5), kod kelmasa /resend.", None).await?;
                }
                SignIn::Other(e) => {
                    eprintln!("sign_in xato: {e}");
                    db::kv_set(&app.db, "login_state", "").await?;
                    app.bot.send(chat, &format!("❌ Ulanmadi: {e}"), None).await?;
                }
            }
        }
        "pass" => {
            app.bot.delete(chat, mid).await;
            let p = match std::mem::replace(&mut *app.pending.lock().await, Pending::None) {
                Pending::Pass(p) => p,
                _ => {
                    db::kv_set(&app.db, "login_state", "").await?;
                    app.bot.send(chat, "Jarayon uzilgan. /login", None).await?;
                    return Ok(());
                }
            };
            match app.tg.check_password(p, text.as_bytes()).await {
                Ok(_) => {
                    db::kv_set(&app.db, "login_state", "").await?;
                    app.bot.send(chat, "✅ Akkaunt ulandi.", None).await?;
                }
                Err(grammers_client::client::SignInError::InvalidPassword(p2)) => {
                    *app.pending.lock().await = Pending::Pass(p2);
                    app.bot.send(chat, "❌ Parol noto'g'ri. Qayta yuboring yoki /login.", None).await?;
                }
                Err(e) => {
                    db::kv_set(&app.db, "login_state", "").await?;
                    app.bot.send(chat, &format!("❌ Ulanmadi: {e}"), None).await?;
                }
            }
        }
        _ => {
            db::kv_set(&app.db, "login_state", "").await?;
        }
    }
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
    let meta = json!({
        "w": info.w, "h": info.h, "dur": info.duration,
        "opts": info.opts.iter().map(|o| json!({"h": o.h, "size": o.size, "stream": o.stream.as_ref().map(|(id, sz, ext)| json!([id, sz, ext]))})).collect::<Vec<_>>(),
    });
    let id = db::job_new(&app.db, chat, url, &info.title, &meta.to_string()).await?;
    let mut rows: Vec<Value> = Vec::new();
    let mut row: Vec<Value> = Vec::new();
    for o in &info.opts {
        let label = format!("{}p{}{}", o.h, o.size.map(|s| format!(" ~{}", dl::human(s))).unwrap_or_default(), if o.stream.is_some() { " ⚡" } else { "" });
        row.push(json!({"text": label, "callback_data": format!("q:{id}:{}", o.h)}));
        if row.len() == 2 {
            rows.push(Value::Array(std::mem::take(&mut row)));
        }
    }
    if !row.is_empty() {
        rows.push(Value::Array(row));
    }
    rows.push(json!([{"text": format!("🎵 Audio{}", info.audio_size.map(|s| format!(" ~{}", dl::human(s))).unwrap_or_default()), "callback_data": format!("q:{id}:a")}, {"text": "✖️ Bekor", "callback_data": format!("x:{id}")}]));
    let dur = if info.duration > 0 { format!("\n⏱ {}:{:02}", info.duration / 60, info.duration % 60) } else { String::new() };
    app.bot.edit(chat, st, &format!("🎬 {}{dur}\n\nSifatni tanlang:", info.title.chars().take(200).collect::<String>()), Some(json!({"inline_keyboard": rows}))).await;
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
                let note = if authorized(app).await { "" } else { "\n⚠️ Akkaunt ulanmagan: avval /login" };
                app.bot.edit(chat, msg, &format!("✅ Navbatga qo'yildi (#{id}). Oldinda: {ahead} ta.{note}"), None).await;
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

async fn peer(app: &App) -> Result<PeerRef> {
    let mut g = app.peer.lock().await;
    if let Some(p) = g.clone() {
        return Ok(p);
    }
    let p = app.tg.resolve_username(&app.username).await?.ok_or_else(|| anyhow!("bot topilmadi: @{}", app.username))?;
    let r = p.to_ref().await.map_err(|e| anyhow!("{e}"))?.ok_or_else(|| anyhow!("bot manzili aniqlanmadi"))?;
    *g = Some(r.clone());
    Ok(r)
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
    if !authorized(app).await {
        return Err(anyhow!("Telegram akkaunt ulanmagan — /login, keyin havolani qayta yuboring"));
    }
    let meta: Value = serde_json::from_str(&job.meta).unwrap_or(json!({}));
    let (w, h, dur) = (meta["w"].as_i64().unwrap_or(0) as i32, meta["h"].as_i64().unwrap_or(0) as i32, meta["dur"].as_i64().unwrap_or(0) as u64);
    let is_audio = job.fmt == "a";
    app.bot.edit(job.chat_id, job.status_msg, &format!("⏬ #{} yuklanmoqda…\n{}", job.id, job.title.chars().take(100).collect::<String>()), None).await;

    // Oqim: faqat bitta tayyor fayl va hajmi ANIQ bo'lsa; muvaffaqiyatsiz bo'lsa diskka yuklashga o'tiladi.
    let want: i64 = job.fmt.parse().unwrap_or(0);
    let stream = meta["opts"].as_array().and_then(|a| a.iter().find(|o| o["h"].as_i64() == Some(want))).and_then(|o| o["stream"].as_array().cloned());
    let mut uploaded = None;
    let mut name = dl::safe_name(&job.title, if is_audio { "mp3" } else { "mp4" });
    if let (Some(s), false) = (stream, is_audio) {
        let (fid, size, ext) = (s[0].as_str().unwrap_or("").to_string(), s[1].as_u64().unwrap_or(0), s[2].as_str().unwrap_or("mp4").to_string());
        if !fid.is_empty() && size > 0 {
            let n = dl::safe_name(&job.title, &ext);
            app.bot.edit(job.chat_id, job.status_msg, &format!("⚡ #{} oqim bilan yuborilmoqda (diskka yozilmaydi)…\n{}", job.id, job.title.chars().take(100).collect::<String>()), None).await;
            let r = tokio::select! {
                r = stream_upload(app, &job.url, &fid, size, &n) => r,
                _ = cancelled(app, job.id) => Err(anyhow!("bekor qilindi")),
            };
            match r {
                Ok(up) => {
                    uploaded = Some(up);
                    name = n;
                }
                Err(e) => {
                    if db::job_state(&app.db, job.id).await == "cancelled" {
                        return Err(e);
                    }
                    eprintln!("#{} oqim bo'lmadi ({e}) — diskka yuklanadi", job.id);
                }
            }
        }
    }

    let up = match uploaded {
        Some(u) => u,
        None => {
            let dir = std::path::PathBuf::from(format!("{}/tmp/{}", app.cfg.data_dir, job.id));
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
            let a2 = Arc::clone(app);
            let (chat, mid, jid, title) = (job.chat_id, job.status_msg, job.id, job.title.chars().take(80).collect::<String>());
            let edit_task = tokio::spawn(async move {
                let mut last = std::time::Instant::now() - Duration::from_secs(10);
                while let Some(p) = rx.recv().await {
                    if last.elapsed() >= Duration::from_secs(5) {
                        last = std::time::Instant::now();
                        let mut it = p.split('|');
                        let (pc, sp, eta) = (it.next().unwrap_or("").trim().to_string(), it.next().unwrap_or("").trim().to_string(), it.next().unwrap_or("").trim().to_string());
                        a2.bot.edit(chat, mid, &format!("⏬ #{jid} {title}\n{pc}  {sp}  qoldi {eta}"), None).await;
                    }
                }
            });
            let r = tokio::select! {
                r = dl::download_file(&app.cfg, &job.url, &job.fmt, &dir, |p| { let _ = tx.send(p); }) => r,
                _ = cancelled(app, job.id) => Err(anyhow!("bekor qilindi")),
            };
            drop(tx);
            let _ = edit_task.await;
            let path = r?;
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("mp4").to_string();
            name = dl::safe_name(&job.title, &ext);
            let size = std::fs::metadata(&path)?.len();
            app.bot.edit(job.chat_id, job.status_msg, &format!("📤 #{} Telegram'ga yuklanmoqda ({})…", job.id, dl::human(size)), None).await;
            let mut f = tokio::fs::File::open(&path).await?;
            let r = tokio::select! {
                r = app.tg.upload_stream(&mut f, size as usize, name.clone()) => r.map_err(|e| anyhow!("yuklash: {e}")),
                _ = cancelled(app, job.id) => Err(anyhow!("bekor qilindi")),
            };
            r?
        }
    };

    let mime = if is_audio { "audio/mpeg" } else if name.ends_with(".webm") { "video/webm" } else { "video/mp4" };
    let mut m = InputMessage::new().text(job.title.chars().take(900).collect::<String>()).document(up).mime_type(mime);
    if !is_audio {
        m = m.attribute(Attribute::Video { round_message: false, supports_streaming: true, duration: Duration::from_secs(dur), w, h });
    }
    let to = peer(app).await?;
    app.tg.send_message(to, m).await.map_err(|e| anyhow!("yuborish: {e}"))?;
    app.bot.edit(job.chat_id, job.status_msg, &format!("✅ #{} tayyor: {}", job.id, job.title.chars().take(100).collect::<String>()), None).await;
    Ok(())
}

/// yt-dlp stdout'idan to'g'ridan-to'g'ri Telegram'ga (diskka yozilmaydi). Hajm aniq bo'lishi shart.
async fn stream_upload(app: &Arc<App>, url: &str, fid: &str, size: u64, name: &str) -> Result<grammers_client::media::Uploaded> {
    let mut child = dl::spawn_stream(&app.cfg, url, fid)?;
    let mut out = child.stdout.take().ok_or_else(|| anyhow!("stdout yo'q"))?;
    let r = tokio::time::timeout(Duration::from_secs(4 * 3600), app.tg.upload_stream(&mut out, size as usize, name.to_string())).await;
    let _ = child.kill().await;
    match r {
        Ok(Ok(u)) => Ok(u),
        Ok(Err(e)) => Err(anyhow!("oqim yuklash: {e}")),
        Err(_) => Err(anyhow!("oqim vaqti tugadi")),
    }
}
