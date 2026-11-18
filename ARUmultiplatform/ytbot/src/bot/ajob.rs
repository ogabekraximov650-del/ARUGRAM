// UZ: GitHub Actions ishchisi (`ytbot ajob`): ish ma'lumotini Ample'dan oladi, yuklaydi, vaqt bo'yicha bo'ladi, Bot API orqali yuboradi va har soniyada logni Ample'ga uzatadi.
// EN: GitHub Actions worker (`ytbot ajob`): fetches the job from Ample, downloads, splits by time, sends through the Bot API and pushes its log to Ample every second.
use super::*;

/// UZ: Ample'ga log/holat yuboradi; `true` qaytsa ish bekor qilingan. EN: pushes a log line/state to Ample; `true` means the job was cancelled.
async fn post(http: &reqwest::Client, cb: &str, job: &str, tok: &str, state: &str, text: &str, err: &str) -> bool {
    let r = http.post(format!("{cb}/prog/{job}")).json(&json!({"tok": tok, "state": state, "text": text, "err": err})).send().await;
    match r {
        Ok(resp) => resp.json::<Value>().await.map(|v| v["cancel"].as_bool() == Some(true)).unwrap_or(false),
        Err(_) => false,
    }
}

/// UZ: chat xabarini Actions o'zi Telegram'da yangilaydi (Ample orqali EMAS): (bot, chat, xabar). EN: the worker edits the chat message in Telegram directly (NOT through Ample).
static CHAT: std::sync::OnceLock<(Bot, i64, i64)> = std::sync::OnceLock::new();

/// UZ: har 3 soniyada `line()` natijasini Telegram xabariga yozadigan fon vazifasi. Ample'ga hech narsa yuborilmaydi (bekor qilishni `heartbeat` tekshiradi).
/// EN: background task writing `line()` into the Telegram message every 3 s; nothing goes through Ample (cancel is polled by `heartbeat`).
fn poster(_http: reqwest::Client, _cb: String, _job: String, _tok: String, line: impl Fn() -> String + Send + 'static) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut last = String::new();
        loop {
            tokio::time::sleep(Duration::from_secs(3)).await;
            let t: String = line().chars().take(3500).collect();
            if t.is_empty() || t == last {
                continue;
            }
            if let Some((bot, chat, msg)) = CHAT.get() {
                bot.edit(*chat, *msg, &t, None).await;
            }
            last = t;
        }
    })
}

/// UZ: Ample'ga har 15 soniyada bitta kichik "tirikman" so'rovi (matnsiz): ish holati "run" bo'ladi va bekor qilinganini bilamiz. Trafik deyarli yo'q.
/// EN: one tiny textless heartbeat to Ample every 15 s: marks the job as running and learns about cancellation. Negligible traffic.
fn heartbeat(http: reqwest::Client, cb: String, job: String, tok: String) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            if post(&http, &cb, &job, &tok, "run", "", "").await {
                eprintln!("ish bekor qilindi");
                std::process::exit(0);
            }
            tokio::time::sleep(Duration::from_secs(15)).await;
        }
    })
}

pub async fn ajob_main() -> Result<()> {
    let env = |k: &str| std::env::var(k).map_err(|_| anyhow!("{k} muhit o'zgaruvchisi yo'q"));
    let cb = env("CB")?.trim_end_matches('/').to_string();
    let (job, tok) = (env("JOB")?, env("TOK")?);
    let http = reqwest::Client::builder().timeout(Duration::from_secs(60)).build()?;
    let info: Value = http.get(format!("{cb}/ajob/{job}?tok={tok}")).send().await?.error_for_status().map_err(|e| anyhow!("Ample ish ma'lumotini bermadi: {e}"))?.json().await?;
    if std::env::var("MODE").map(|m| m == "probe").unwrap_or(false) {
        return probe_main(&http, &cb, &job, &tok, &info).await;
    }
    let hb = heartbeat(http.clone(), cb.clone(), job.clone(), tok.clone());
    let r = run(&http, &cb, &job, &tok, &info).await;
    hb.abort();
    match r {
        Ok(text) => {
            post(&http, &cb, &job, &tok, "done", &text, "").await;
            Ok(())
        }
        Err(e) => {
            eprintln!("xato: {e}");
            post(&http, &cb, &job, &tok, "fail", "", &e.to_string()).await;
            Err(e)
        }
    }
}

/// UZ: sifatlar ro'yxatini yt-dlp bilan oladi va Ample'ga (`/pres`) beradi; Actions'da ishlaydi. EN: fetches the quality list with yt-dlp and posts it to Ample (`/pres`); runs inside Actions.
async fn probe_main(http: &reqwest::Client, cb: &str, job: &str, tok: &str, info: &Value) -> Result<()> {
    let data_dir = "./adata".to_string();
    std::fs::create_dir_all(&data_dir)?;
    let mut cookies = None;
    if info["cookies"].as_bool() == Some(true) {
        if let Ok(r) = http.get(format!("{cb}/ack/{job}?tok={tok}")).send().await {
            if let Ok(t) = r.error_for_status()?.text().await {
                let p = format!("{data_dir}/cookies.txt");
                std::fs::write(&p, t)?;
                cookies = Some(p);
            }
        }
    }
    let cfg = Cfg { bot_token: String::new(), data_dir, ytdlp: "yt-dlp".into(), cookies, owner_env: None, api_id: None, public_url: None, webhook_secret: String::new(), port: 0, gh_repo: None };
    let url = info["url"].as_str().unwrap_or("").to_string();
    let body = match dl::probe(&cfg, &url).await {
        Ok(i) => json!({"tok": tok, "ok": true, "info": dl::info_to_json(&i)}),
        Err(e) => json!({"tok": tok, "ok": false, "err": e.to_string().chars().take(600).collect::<String>()}),
    };
    http.post(format!("{cb}/pres/{job}")).json(&body).send().await?.error_for_status()?;
    Ok(())
}

async fn run(http: &reqwest::Client, cb: &str, job: &str, tok: &str, info: &Value) -> Result<String> {
    let chat = info["chat_id"].as_i64().ok_or_else(|| anyhow!("chat_id yo'q"))?;
    let title = info["title"].as_str().unwrap_or("video").to_string();
    let url = info["url"].as_str().unwrap_or("").to_string();
    let fmt = info["fmt"].as_str().unwrap_or("b").to_string();
    let meta = &info["meta"];
    let (w, h) = (meta["w"].as_i64().unwrap_or(0), meta["h"].as_i64().unwrap_or(0));
    let is_audio = fmt == "a";
    let data_dir = "./adata".to_string();
    let dir = std::path::PathBuf::from(format!("{data_dir}/job"));
    std::fs::create_dir_all(&dir)?;
    let mut cookies = None;
    if info["cookies"].as_bool() == Some(true) {
        let t = http.get(format!("{cb}/ack/{job}?tok={tok}")).send().await?.error_for_status()?.text().await?;
        let p = format!("{data_dir}/cookies.txt");
        std::fs::write(&p, t)?;
        cookies = Some(p);
    }
    let cfg = Cfg { bot_token: info["bot_token"].as_str().unwrap_or("").to_string(), data_dir: data_dir.clone(), ytdlp: "yt-dlp".into(), cookies, owner_env: None, api_id: std::env::var("API_ID").ok().and_then(|v| v.trim().parse().ok()), public_url: None, webhook_secret: String::new(), port: 0, gh_repo: None };
    let t80: String = title.chars().take(80).collect();
    // Chat xabarini keyingi yangilashlar to'g'ridan-to'g'ri Telegram'ga ketadi (status_msg Ample'dan bir marta olingan).
    if let Some(m) = info["status_msg"].as_i64() {
        let _ = CHAT.set((Bot::new(&cfg.bot_token), chat, m));
    }

    // 1) Yuklash. To'g'ridan-to'g'ri havolalar bo'lsa (masalan Rumble) — shu fayl havolasi.
    let (src_url, src_fmt) = match meta["direct"].as_object().filter(|m| !m.is_empty()) {
        Some(m) if is_audio => (m.iter().filter_map(|(k, u)| Some((k.parse::<i64>().ok()?, u.as_str()?))).min_by_key(|(h, _)| *h).map(|(_, u)| u).unwrap_or(&url).to_string(), "a".to_string()),
        Some(m) => (m.get(&fmt).and_then(|u| u.as_str()).unwrap_or(&url).to_string(), "d".to_string()),
        None => (url.clone(), fmt.clone()),
    };
    let last = Arc::new(std::sync::Mutex::new(String::new()));
    let (l2, t2) = (Arc::clone(&last), t80.clone());
    let p1 = poster(http.clone(), cb.to_string(), job.to_string(), tok.to_string(), move || format!("⏬ Actions: yuklab olinmoqda\n{}\n{t2}", l2.lock().map(|g| g.clone()).unwrap_or_default()));
    let l3 = Arc::clone(&last);
    let path = dl::download_file(&cfg, &src_url, &src_fmt, &dir, move |p| {
        let mut it = p.split('|');
        let (pc, sp, eta) = (it.next().unwrap_or("").trim().to_string(), it.next().unwrap_or("").trim().to_string(), it.next().unwrap_or("").trim().to_string());
        let pct: f64 = pc.trim_end_matches('%').trim().parse().unwrap_or(0.0);
        if let Ok(mut g) = l3.lock() {
            *g = format!("{}\n{sp} · qoldi {eta}", bar(pct));
        }
    })
    .await;
    p1.abort();
    let path = path?;
    let size = std::fs::metadata(&path)?.len();

    // 2*) Akkaunt (MTProto) bor bo'lsa: butun fayl (<= ~2 GB) yoki ~2 GB li eng katta qismlar (5 GB -> 2 + 2 + 1) akkaunt orqali bot chatiga yuboriladi; Ample ularni egasiga nusxalaydi.
    if size > dl::PART_LIMIT && std::path::Path::new(&format!("{data_dir}/pyro.session")).is_file() {
        if let Some(acc) = init_account(&cfg).await {
            let uname = info["bot_username"].as_str().unwrap_or("").to_string();
            let peer = acc.client.resolve_username(&uname).await.ok().flatten().ok_or_else(|| anyhow!("bot topilmadi: @{uname}"))?;
            let peer = peer.to_ref().await.map_err(|e| anyhow!("{e}"))?.ok_or_else(|| anyhow!("bot manzili aniqlanmadi"))?;
            let pct0 = Arc::new(AtomicU32::new(0));
            let big = if size <= dl::ACC_LIMIT {
                vec![path.clone()]
            } else {
                let (pc, t, sz) = (Arc::clone(&pct0), t80.clone(), dl::human(size));
                let pp = poster(http.clone(), cb.to_string(), job.to_string(), tok.to_string(), move || format!("✂️ Actions: moov bo'yicha ~2 GB li qismlarga bo'linmoqda ({sz})\n{}\n{t}", bar(pc.load(Ordering::Relaxed) as f64)));
                let r = dl::split_with(&path, &dir, Arc::clone(&pct0), dl::ACC_LIMIT, dl::ACC_TARGET).await;
                pp.abort();
                r?
            };
            let n = big.len();
            for (i, part) in big.iter().enumerate() {
                let psize = std::fs::metadata(part)?.len();
                let sent = Arc::new(AtomicU64::new(0));
                let (s2, t4) = (Arc::clone(&sent), t80.clone());
                let pp = poster(http.clone(), cb.to_string(), job.to_string(), tok.to_string(), move || {
                    let s = s2.load(Ordering::Relaxed);
                    let pc = (s as f64 / psize.max(1) as f64 * 100.0).min(99.0);
                    let overall = ((i as f64 + pc / 100.0) / n as f64) * 100.0;
                    format!("📤 Actions: qism {}/{} akkaunt orqali yuborilmoqda\n{}  ({} / {})\nUmumiy: {}\n{t4}", i + 1, n, bar(pc), dl::human(s), dl::human(psize), bar(overall))
                });
                let r = acc_send(&acc, peer.clone(), &title, part, is_audio, w as i32, h as i32, &dir, Arc::clone(&sent)).await;
                pp.abort();
                r.map_err(|e| anyhow!("{}/{} qism yuborilmadi: {e}", i + 1, n))?;
                let _ = std::fs::remove_file(part);
            }
            return Ok(format!("✅ #{job} tayyor: {t80}\n{}", if n > 1 { format!("{n} qismda (har biri ~2 GB gacha), Actions + akkaunt orqali yuborildi") } else { format!("Bitta fayl ({}), Actions + akkaunt orqali yuborildi", dl::human(size)) }));
        }
    }

    // 2) Vaqt bo'yicha ~45 MB li qismlar (kalit kadrlarda, qayta kodlamasdan).
    let pct = Arc::new(AtomicU32::new(0));
    let (pc2, t3, sz) = (Arc::clone(&pct), t80.clone(), dl::human(size));
    let p2 = poster(http.clone(), cb.to_string(), job.to_string(), tok.to_string(), move || format!("✂️ Actions: qismlarga bo'linmoqda ({sz})\n{}\n{t3}", bar(pc2.load(Ordering::Relaxed) as f64)));
    let parts = dl::split(&path, &dir, Arc::clone(&pct)).await;
    p2.abort();
    let parts = parts?;
    let n = parts.len();

    // 3) Bot API orqali birin-ketin yuborish (yuborilgan qism o'chiriladi).
    let bot = Bot::new(&cfg.bot_token);
    for (i, part) in parts.iter().enumerate() {
        let psize = std::fs::metadata(part)?.len();
        let dur = dl::probe_duration(part).await.round() as u64;
        let thumb = if is_audio { None } else { dl::make_thumb(part, &dir.join(format!("thumb{i}.jpg"))).await };
        let caption = if n > 1 { format!("{} ({}/{})", title.chars().take(900).collect::<String>(), i + 1, n) } else { title.chars().take(900).collect::<String>() };
        let sent = Arc::new(AtomicU64::new(0));
        let (s2, t4) = (Arc::clone(&sent), t80.clone());
        let p3 = poster(http.clone(), cb.to_string(), job.to_string(), tok.to_string(), move || {
            let s = s2.load(Ordering::Relaxed);
            let pc = (s as f64 / psize.max(1) as f64 * 100.0).min(99.0);
            let overall = ((i as f64 + pc / 100.0) / n as f64) * 100.0;
            format!("📤 Actions: qism {}/{} yuborilmoqda\n{}  ({} / {})\nUmumiy: {}\n{t4}", i + 1, n, bar(pc), dl::human(s), dl::human(psize), bar(overall))
        });
        let r = bot.send_file(chat, is_audio, part, &caption, dur, w, h, thumb.as_deref(), Arc::clone(&sent)).await;
        p3.abort();
        r.map_err(|e| anyhow!("{}/{} qism yuborilmadi: {e}", i + 1, n))?;
        let _ = std::fs::remove_file(part);
    }
    Ok(format!("✅ #{job} tayyor: {}\n{}", t80, if n > 1 { format!("{n} qismda yuborildi (Actions)") } else { "Actions orqali yuborildi".to_string() }))
}

/// UZ: faylni akkaunt orqali bot chatiga yuklaydi (thumbnail va foiz bilan). EN: uploads the file to the bot chat through the account (with thumbnail and progress).
async fn acc_send(acc: &Account, to: PeerRef, title: &str, path: &Path, audio: bool, w: i32, h: i32, dir: &Path, sent: Arc<AtomicU64>) -> Result<()> {
    let size = std::fs::metadata(path)?.len();
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or(if audio { "mp3" } else { "mp4" }).to_string();
    let name = dl::safe_name(title, &ext);
    let dur = dl::probe_duration(path).await.round() as u64;
    let thumb = if audio { None } else { dl::make_thumb(path, &dir.join("acc_thumb.jpg")).await };
    let file = tokio::fs::File::open(path).await?;
    let mut rd = CountRead { inner: file, n: sent };
    let up = acc.client.upload_stream(&mut rd, size as usize, name).await.map_err(|e| anyhow!("yuklash: {e}"))?;
    let mime = if audio { "audio/mpeg" } else if ext == "webm" { "video/webm" } else { "video/mp4" };
    let mut m = InputMessage::new().text(title.chars().take(900).collect::<String>()).document(up).mime_type(mime);
    if let Some(t) = &thumb {
        if let Ok(tu) = acc.client.upload_file(t).await {
            m = m.thumbnail(tu);
        }
    }
    m = if audio { m.attribute(Attribute::Audio { duration: Duration::from_secs(dur), title: None, performer: None }) } else { m.attribute(Attribute::Video { round_message: false, supports_streaming: true, duration: Duration::from_secs(dur), w, h }) };
    acc.client.send_message(to, m).await.map_err(|e| anyhow!("yuborish: {e}"))?;
    Ok(())
}
