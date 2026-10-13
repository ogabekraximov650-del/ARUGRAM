// ══════════════════════════════════════════════════════════════
//  JONLI HOLAT XABARI — kodlash botining uchala bo'limi uchun bitta
// ══════════════════════════════════════════════════════════════
//
// TALAB (foydalanuvchi): botdagi uchala bo'limda ("📱 Ilova uchun",
// "🎬 Post kodlash", "🎞 Anibla yuklash") "📋 Holat" tugmasi; qaysi bo'limda
// bosilsa, bot AYNAN o'sha bo'limning jonli holati va log'ini yangi xabarda
// ko'rsatadi va FAQAT o'sha xabarni yangilab turadi — oldingi holat
// xabarlari yangilanishdan to'xtaydi.
//
// Shu sabab "kuzatilayotgan xabar" bitta: `app_config.live_watch` =
// `bo'lim|chat|xabar|vaqt`. Runner'lar holatni avvalgidek yuboradi
// (`/api/encode/push`, `/api/post/progress`, `/api/anibla/progress`), worker
// esa faqat kuzatilayotgan bo'lim kelganda shu bitta xabarni tahrirlaydi.
//
// Avtomatik o'tish: post yoki anibla ishi BOSHLANGANDA (claim) — agar hech
// narsa kuzatilmayotgan bo'lsa, shu bo'lim kuzatilayotgan bo'lsa yoki
// oxirgi tanlov 30 daqiqadan eski bo'lsa — yangi ishning holat xabari
// kuzatiladi (avvalgidek o'zi yangilanib turadi). Admin "Holat" bilan
// boshqa bo'limni tanlagan bo'lsa, unga tegilmaydi.
//
// Bazaga faqat O'QISH (har yangilanishda bitta, izolyatda 4 soniya eslab
// qolinadi); YOZUV — faqat "Holat" bosilganda yoki ish boshlanganda.

use super::*;

pub(crate) const APP: &str = "app";
pub(crate) const POST: &str = "post";
pub(crate) const ANIBLA: &str = "anibla";

const KEY: &str = "live_watch";
const MEMO_MS: i64 = 4_000;
/// Shundan eski tanlovni yangi boshlangan ish egallashi mumkin.
const TAKEOVER_MS: i64 = 30 * 60 * 1000;

#[derive(Clone)]
pub(crate) struct Watch {
    pub sec: String,
    pub chat: i64,
    pub msg: i64,
    pub at: i64,
}

thread_local! {
    static MEMO: std::cell::RefCell<Option<(Option<Watch>, i64)>> = const { std::cell::RefCell::new(None) };
}

fn parse(v: &str) -> Option<Watch> {
    let p: Vec<&str> = v.split('|').collect();
    if p.len() < 3 {
        return None;
    }
    Some(Watch {
        sec: p[0].to_string(),
        chat: p[1].parse().ok()?,
        msg: p[2].parse().ok().filter(|m: &i64| *m > 0)?,
        at: p.get(3).and_then(|x| x.parse().ok()).unwrap_or(0),
    })
}

pub(crate) async fn get(env: &Env) -> Option<Watch> {
    let now = now_ms();
    if let Some(w) = MEMO.with(|m| m.borrow().clone().filter(|(_, at)| now - *at < MEMO_MS).map(|(w, _)| w)) {
        return w;
    }
    let w = config_get_db(env, KEY).await.and_then(|v| parse(&v));
    MEMO.with(|m| *m.borrow_mut() = Some((w.clone(), now)));
    w
}

pub(crate) async fn set(env: &Env, sec: &str, chat: i64, msg: i64) {
    let now = now_ms();
    config_put(env, KEY, &format!("{sec}|{chat}|{msg}|{now}")).await;
    let w = Watch { sec: sec.to_string(), chat, msg, at: now };
    MEMO.with(|m| *m.borrow_mut() = Some((Some(w), now)));
}

/// Yangi ish boshlandi: uning holat xabari kuzatiladimi (izohga qarang).
pub(crate) async fn claim(env: &Env, sec: &str, chat: i64, msg: i64) {
    if msg <= 0 {
        return;
    }
    let take = match get(env).await {
        None => true,
        Some(w) => w.sec == sec || now_ms() - w.at > TAKEOVER_MS,
    };
    if take {
        set(env, sec, chat, msg).await;
    }
}

/// Bo'lim kuzatilayotgan bo'lsa — uning xabari (`(chat, msg)`).
pub(crate) async fn target(env: &Env, sec: &str) -> Option<(i64, i64)> {
    get(env).await.filter(|w| w.sec == sec).map(|w| (w.chat, w.msg))
}

/// Kuzatilayotgan bo'lim shu bo'lsa — xabarni tahrirlaydi.
/// `Ok(true)` — tahrirlandi, `Ok(false)` — boshqa bo'lim kuzatilmoqda,
/// `Err(soniya)` — Telegram "sekinroq" dedi (shuncha kutish kerak).
pub(crate) async fn edit(env: &Env, sec: &str, html: &str) -> std::result::Result<bool, i64> {
    let Some((chat, msg)) = target(env, sec).await else { return Ok(false) };
    edit_msg(env, chat, msg, html).await.map(|_| true)
}

/// Bitta xabarni HTML bilan tahrirlaydi ("o'zgarmadi" xatosi — muvaffaqiyat).
pub(crate) async fn edit_msg(env: &Env, chat: i64, msg: i64, html: &str) -> std::result::Result<(), i64> {
    let text: String = html.chars().take(4000).collect();
    match encbot_api(env, "editMessageText", json!({
        "chat_id": chat, "message_id": msg, "parse_mode": "HTML",
        "text": text, "disable_web_page_preview": true,
    })).await {
        Ok(_) => Ok(()),
        Err(e) => {
            let e = e.to_string();
            match e.split("retry after").nth(1) {
                Some(rest) => Err(rest.trim().chars().take_while(|c| c.is_ascii_digit())
                    .collect::<String>().parse::<i64>().unwrap_or(5).max(1)),
                None => Ok(()),
            }
        }
    }
}

/// "Holat" bosildi: yangi xabar yuboradi va uni shu bo'lim uchun kuzatadi.
pub(crate) async fn start(env: &Env, sec: &str, chat: i64, html: &str) -> i64 {
    let msg = encbot_api(env, "sendMessage", json!({
        "chat_id": chat, "parse_mode": "HTML", "text": html, "disable_web_page_preview": true,
    })).await.ok().and_then(|v| v["message_id"].as_i64()).unwrap_or(0);
    if msg > 0 {
        set(env, sec, chat, msg).await;
    }
    msg
}

// ── Umumiy ko'rinish yordamchilari ───────────────────────────

pub(crate) fn bar(pct: f64) -> String {
    let full = ((pct.clamp(0.0, 100.0) / 100.0) * 12.0).round() as usize;
    format!("{}{}", "\u{2593}".repeat(full), "\u{2591}".repeat(12 - full))
}

pub(crate) fn hms(s: i64) -> String {
    if s < 0 {
        return "--:--".into();
    }
    let (h, m, sec) = (s / 3600, s % 3600 / 60, s % 60);
    if h > 0 { format!("{h}:{m:02}:{sec:02}") } else { format!("{m:02}:{sec:02}") }
}

// ── "📱 Ilova uchun" (avto-kodlash, `encode.yml`) ────────────

thread_local! {
    /// `a/s` -> (anime nomi, bo'lim nomi): har yangilanishda bazaga bormasin.
    static TITLES: std::cell::RefCell<std::collections::HashMap<String, (String, String)>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

async fn titles(env: &Env, a: i64, s: i64) -> (String, String) {
    let key = format!("{a}/{s}");
    if let Some(t) = TITLES.with(|m| m.borrow().get(&key).cloned()) {
        return t;
    }
    let t = encbot_titles(env, a, s).await
        .unwrap_or_else(|| (format!("anime #{a}"), format!("bo'lim #{s}")));
    TITLES.with(|m| m.borrow_mut().insert(key, t.clone()));
    t
}

/// Ilova uchun kodlash holati (runner `#arustatus` matnidan) — "Post kodlash"
/// holati bilan bir xil ko'rinish: sarlavha, bosqichlar, foiz-chiziq,
/// tezlik, hajm, o'tdi/qoldi, oxirgi log qatorlari, jami vaqt.
pub(crate) async fn app_html(env: &Env, st: Option<&Value>) -> String {
    app_html_n(env, st, 4).await
}

/// `n` — oxirgi log qatorlari soni.
pub(crate) async fn app_html_n(env: &Env, st: Option<&Value>, n: usize) -> String {
    let fresh = st.filter(|s| now_ms() - jint(s, "updated_at") < 3 * 60 * 1000);
    let Some(st) = fresh.filter(|s| jint(s, "anime_id") > 0) else {
        let mut t = "\u{1F4F1} <b>Ilova uchun kodlash</b>\n\n\u{1F4A4} Hozir kodlanayotgan qism yo'q.".to_string();
        if let Some(s) = st {
            let upd = jint(s, "updated_at");
            if upd > 0 {
                t.push_str(&format!("\n\u{1F552} Runner oxirgi marta {} oldin yozgan.", hms((now_ms() - upd) / 1000)));
            }
        }
        t.push_str("\n\nKodlash boshlansa, shu xabar o'zi yangilanadi.");
        return t;
    };
    let (a, s) = (jint(st, "anime_id"), jint(st, "season_id"));
    let (an, sn) = titles(env, a, s).await;
    let d = &st["data"];
    let mut t = format!("\u{1F4F1} <b>Ilova uchun: {}</b>\n{}, {}-qism", html_escape(&an), html_escape(&sn),
        jint(st, "epizod_number"));
    if jint(d, "attempt") > 1 {
        t.push_str(&format!(" (urinish {})", jint(d, "attempt")));
    }
    t.push('\n');
    // Manba.
    if d["src"].is_object() {
        let src = &d["src"];
        t.push_str(&format!("\n\u{2705} Manba: {}p \u{00B7} {} \u{00B7} {:.1} MB",
            jint(src, "h"), hms(src["dur_s"].as_f64().unwrap_or(0.0) as i64), src["mb"].as_f64().unwrap_or(0.0)));
    }
    // Sifatlar zinasi.
    if let Some(ladder) = d["ladder"].as_array().filter(|l| !l.is_empty()) {
        let row: Vec<String> = ladder.iter().map(|l| {
            let icon = match l["state"].as_str().unwrap_or("") {
                "done" => "\u{2705}",
                "enc" => "\u{2699}\u{FE0F}",
                "upload" => "\u{2B06}\u{FE0F}",
                "error" => "\u{274C}",
                _ => "\u{23F3}",
            };
            format!("{icon}{}", l["q"].as_str().unwrap_or("?"))
        }).collect();
        t.push_str(&format!("\n\u{1F39E} Sifatlar: {}", row.join(" ")));
    }
    // Joriy bosqich.
    let p: Vec<&str> = st["progress"].as_str().unwrap_or("").split('|').collect();
    let x = &d["xfer"];
    let cur = match p.as_slice() {
        ["enc", q, pct, i, n, speed, fps, br, mb, est, el, eta] => {
            let pf = pct.parse::<f64>().unwrap_or(0.0);
            let est = if *est == "-" { String::new() } else { format!(" (~{est} MB bo'ladi)") };
            format!("\u{2699}\u{FE0F} Kodlanmoqda {q} ({i}/{n})\n{} {pct}%\n\
                     tezlik {speed} \u{00B7} {fps} kadr/s \u{00B7} bitreyt {br} kb/s\n\
                     hajm {mb} MB{est}\no'tdi {} \u{00B7} qoldi ~{}",
                bar(pf), hms(el.parse().unwrap_or(-1)), hms(eta.parse().unwrap_or(-1)))
        }
        ["enc", q, pct, i, n] => {
            let pf = pct.parse::<f64>().unwrap_or(0.0);
            format!("\u{2699}\u{FE0F} Kodlanmoqda {q} ({i}/{n})\n{} {pct}%", bar(pf))
        }
        _ if x.is_object() => {
            // `kind`: "yuklab olinmoqda" (asl video) yoki "720p Telegram'ga yuklanmoqda".
            let kind = x["kind"].as_str().unwrap_or("");
            let head = if kind.contains("Telegram") {
                format!("\u{2B06}\u{FE0F} {kind}")
            } else {
                "\u{2B07}\u{FE0F} Asl video yuklab olinmoqda".to_string()
            };
            let pct = x["pct"].as_f64().unwrap_or(0.0);
            format!("{head}\n{} {pct:.1}%\ntezlik {:.2} MB/s\nhajm {:.1} / {:.1} MB\no'tdi {} \u{00B7} qoldi ~{}",
                bar(pct), x["mbps"].as_f64().unwrap_or(0.0), x["cur_mb"].as_f64().unwrap_or(0.0),
                x["total_mb"].as_f64().unwrap_or(0.0), hms(jint(x, "elapsed")), hms(jint(x, "eta")))
        }
        ["upload", q, i, n] => format!("\u{2B06}\u{FE0F} {q} ({i}/{n}) Telegram'ga yuklanmoqda"),
        ["download"] => "\u{2B07}\u{FE0F} Asl video yuklab olinmoqda".to_string(),
        _ => "\u{23F3} boshlanmoqda...".to_string(),
    };
    t.push_str(&format!("\n{}", html_escape(&cur)));
    // Oxirgi log qatorlari.
    let lines: Vec<String> = st["lines"].as_array().cloned().unwrap_or_default().iter()
        .rev().take(n).rev().filter_map(|l| l.as_str().map(|s| html_escape(s.trim()))).filter(|s| !s.is_empty()).collect();
    if !lines.is_empty() {
        t.push_str(&format!("\n\n\u{1F4DC} Log:\n{}", lines.join("\n")));
    }
    let started = jint(d, "job_started");
    if started > 0 {
        t.push_str(&format!("\n\n\u{23F1} jami: {}", hms(now_ms() / 1000 - started)));
    }
    t
}

/// Runner yangi holat yubordi (`/api/encode/push`) — "Ilova uchun"
/// kuzatilayotgan bo'lsa, xabar yangilanadi.
pub(crate) async fn app_push(env: &Env, text: &str) {
    if target(env, APP).await.is_none() {
        return;
    }
    let st = parse_status_pin(text);
    let html = app_html(env, st.as_ref()).await;
    let _ = edit(env, APP, &html).await;
}

/// "📋 Holat" — "Ilova uchun" bo'limida.
pub(crate) async fn app_status(env: &Env, chat: i64) {
    // Sifat-workflow'lar (360p/480p/720p/1080p) ishlatilyapti — hamma sifat BITTA xabarda, o'zi yangilanadi.
    if !encode_live_all(env).await.is_empty() {
        let html = app_all_html(env).await;
        start(env, APP, chat, &html).await;
        return;
    }
    let st = encode_live_current(env).await.and_then(|t| parse_status_pin(&t));
    let html = app_html(env, st.as_ref()).await;
    start(env, APP, chat, &html).await;
}

// ── SIFAT-WORKFLOW'LAR (360p/480p/720p/1080p): BITTA UMUMIY XABAR ─────────
//
// "Ilova uchun" holati — BITTA xabarda hamma sifat: har biri uchun qaysi qism, bosqich (yuklab olish / kodlash /
// Telegram'ga yuklash) foiz-chiziq bilan, tezlik, hajm, o'tdi/qoldi va oxirgi log qatorlari. Xabar "📋 Holat" bosilganda
// (yoki ish boshlanganda, hech narsa kuzatilmayotgan bo'lsa) ochiladi va OXIRGI xabar bo'lib o'zi yangilanib turadi
// (boshqa bo'limlardagidek: kuzatuv `APP`).

thread_local! {
    /// Kutayotgan qismlar soni (sifat bo'yicha) va jami navbat — har yangilanishda bazaga bormasin.
    static PEND: std::cell::RefCell<(i64, [i64; 4], i64)> = const { std::cell::RefCell::new((0, [0; 4], 0)) };
}
const PEND_MS: i64 = 30_000;

async fn pending_all(env: &Env) -> ([i64; 4], i64) {
    let now = now_ms();
    let (at, c, q) = PEND.with(|m| *m.borrow());
    if now - at < PEND_MS {
        return (c, q);
    }
    let mut counts = [0i64; 4];
    for (i, qq) in QUALITIES.iter().enumerate() {
        counts[i] = encode_pending_q(env, qq).await;
    }
    let queued = turso_exec(env, "SELECT COUNT(*) AS n FROM encode_jobs WHERE state='queued'", vec![]).await.ok()
        .and_then(|r| first_row(&r)).map(|r| jint(&r, "n")).unwrap_or(0);
    PEND.with(|m| *m.borrow_mut() = (now, counts, queued));
    (counts, queued)
}

fn short(s: &str, n: usize) -> String {
    let t: String = s.chars().take(n).collect();
    if s.chars().count() > n { format!("{t}\u{2026}") } else { t }
}

/// Bitta sifat bloki: qism, bosqich (foiz-chiziq bilan), manba va oxirgi `nlog` ta log qatori.
async fn q_block(env: &Env, q: &str, st: &Value, pending: i64, nlog: usize) -> String {
    let d = &st["data"];
    let (a, s) = (jint(st, "anime_id"), jint(st, "season_id"));
    let (an, sn) = titles(env, a, s).await;
    let mut t = format!("\u{1F539} <b>{q}</b> \u{00B7} {}, {}, {}-qism", html_escape(&short(&an, 28)),
        html_escape(&short(&sn, 20)), jint(st, "epizod_number"));
    if jint(d, "attempt") > 1 {
        t.push_str(&format!(" (urinish {})", jint(d, "attempt")));
    }
    t.push_str(&format!(" \u{00B7} kutayotgan: {pending}"));
    if d["src"].is_object() {
        let src = &d["src"];
        t.push_str(&format!("\n\u{1F39E} Manba: {}p \u{00B7} {} \u{00B7} {:.1} MB",
            jint(src, "h"), hms(src["dur_s"].as_f64().unwrap_or(0.0) as i64), src["mb"].as_f64().unwrap_or(0.0)));
    }
    let p: Vec<&str> = st["progress"].as_str().unwrap_or("").split('|').collect();
    let x = &d["xfer"];
    let stage = match p.as_slice() {
        ["enc", _, pct, _, _, speed, fps, br, mb, est, el, eta] => {
            let pf = pct.parse::<f64>().unwrap_or(0.0);
            let est = if *est == "-" { String::new() } else { format!(" (~{est} MB)") };
            format!("\u{2699}\u{FE0F} Kodlanmoqda {} {pct}%\n    tezlik {speed} \u{00B7} {fps} kadr/s \u{00B7} {br} kb/s \u{00B7} hajm {mb} MB{est}\n    o'tdi {} \u{00B7} qoldi ~{}",
                bar(pf), hms(el.parse().unwrap_or(-1)), hms(eta.parse().unwrap_or(-1)))
        }
        ["enc", _, pct, _, _] => format!("\u{2699}\u{FE0F} Kodlanmoqda {} {pct}%", bar(pct.parse::<f64>().unwrap_or(0.0))),
        _ if x.is_object() => {
            let kind = x["kind"].as_str().unwrap_or("");
            let head = if kind.contains("Telegram") { "\u{2B06}\u{FE0F} Telegram'ga yuklanmoqda" } else { "\u{2B07}\u{FE0F} Asl video yuklab olinmoqda" };
            let pct = x["pct"].as_f64().unwrap_or(0.0);
            format!("{head} {} {pct:.1}%\n    tezlik {:.2} MB/s \u{00B7} {:.1} / {:.1} MB\n    o'tdi {} \u{00B7} qoldi ~{}",
                bar(pct), x["mbps"].as_f64().unwrap_or(0.0), x["cur_mb"].as_f64().unwrap_or(0.0),
                x["total_mb"].as_f64().unwrap_or(0.0), hms(jint(x, "elapsed")), hms(jint(x, "eta")))
        }
        ["upload", ..] => "\u{2B06}\u{FE0F} Telegram'ga yuklanmoqda".to_string(),
        ["download"] => "\u{2B07}\u{FE0F} Asl video yuklab olinmoqda".to_string(),
        _ => "\u{23F3} boshlanmoqda...".to_string(),
    };
    t.push_str(&format!("\n{stage}"));
    let lines: Vec<String> = st["lines"].as_array().cloned().unwrap_or_default().iter()
        .rev().take(nlog).rev()
        .filter_map(|l| l.as_str().map(|s| html_escape(&short(s.trim(), 90)))).filter(|s| !s.is_empty()).collect();
    if !lines.is_empty() {
        t.push_str(&format!("\n\u{1F4DC} {}", lines.join("\n\u{1F4DC} ")));
    }
    let started = jint(d, "job_started");
    if started > 0 {
        t.push_str(&format!("\n\u{23F1} jami: {}", hms(now_ms() / 1000 - started)));
    }
    t
}

/// Hamma sifat-workflow'lar BITTA xabarda (4096 belgidan oshmasligi uchun log qatorlari kamaytiriladi).
pub(crate) async fn app_all_html(env: &Env) -> String {
    let all = encode_live_all(env).await;
    let (pend, queued) = pending_all(env).await;
    let now = now_ms();
    let mut nlog = 3usize;
    loop {
        let mut blocks = Vec::new();
        for (i, q) in QUALITIES.iter().enumerate() {
            let st = all.get(*q).and_then(|e| parse_status_pin(e["t"].as_str().unwrap_or("")));
            let live = st.as_ref().filter(|s| now - jint(s, "updated_at") < 3 * 60 * 1000
                && s["progress"].as_str().unwrap_or("idle") != "idle" && jint(s, "anime_id") > 0);
            match live {
                Some(s) => blocks.push(q_block(env, q, s, pend[i], nlog).await),
                None => blocks.push(format!("\u{1F4A4} <b>{q}</b>: hozir ishlamayapti \u{00B7} kutayotgan: {}", pend[i])),
            }
        }
        let t = format!("\u{1F4F1} <b>Ilova uchun kodlash</b> \u{00B7} navbatda: {queued} ta qism\n\n{}", blocks.join("\n\n"));
        if t.chars().count() <= 3900 || nlog == 0 {
            return t;
        }
        nlog -= 1;
    }
}

/// Sifat-workflow holat yubordi: umumiy xabar yangilanadi — butun dunyo bo'yicha ~6 soniyada BIR marta
/// (Durable Object darvozasi: 4 ta run bir vaqtda yuborsa ham Telegram'ni ko'p tahrirlamaymiz).
pub(crate) async fn app_push_all(env: &Env) {
    if target(env, APP).await.is_none() {
        return;
    }
    if !encode_live_gate(env, 6_000).await {
        return;
    }
    let html = app_all_html(env).await;
    let _ = edit(env, APP, &html).await;
}

/// Ish boshlandi: hech narsa kuzatilmayotgan (yoki kuzatuv eski, boshqa bo'limniki) bo'lsa — umumiy holat xabari
/// ochiladi va kuzatiladi. Allaqachon "Ilova uchun" kuzatilayotgan bo'lsa — o'sha xabar yangilanaveradi.
pub(crate) async fn app_autostart(env: &Env) {
    let take = match get(env).await {
        None => true,
        // "Ilova uchun" xabari 1 soatdan eski bo'lsa — yangisi ochiladi (OXIRGI xabar yangilanib tursin).
        Some(w) if w.sec == APP => now_ms() - w.at > 60 * 60 * 1000,
        Some(w) => now_ms() - w.at > TAKEOVER_MS,
    };
    if take {
        let html = app_all_html(env).await;
        start(env, APP, ADMIN_TELEGRAM_ID, &html).await;
    }
}
