// ══════════════════════════════════════════════════════════════
//  MANGA TARJIMA — kodlash botining "🌐 Manga tarjima" bo'limi
// ══════════════════════════════════════════════════════════════
//
// TALAB (foydalanuvchi): botga rasmlarni ketma-ket (yoki ZIP/CBZ/PDF fayl) yuborsa, ular
// eslab qolinsin; oxirida bitta fayl (PDF/CBZ/ZIP) bo'lib tarjima qilingan holda qaytsin.
// Tarjima ikkinchi GitHub akkauntdagi (`GH_REPO`) ALOHIDA workflow'da (`translate.yml`,
// shablon `tool/translate/translate.workflow.yml`) bajariladi — post/anibla bilan bir xil usul.
//
// OQIM:
//   * har yuborilgan rasm/fayl `tr_pages` ga YOZILADI (Turso) — worker qayta ishga tushsa ham,
//     bot soatlab jim tursa ham hech narsa yo'qolmaydi. Takror webhook (chat, msg_id) bo'yicha e'tiborsiz;
//   * "✅ Tarjima qilish" bosilganda hamma fayl `copyMessages` bilan yopiq kanalga ko'chadi
//     (fayl worker'dan o'tmaydi), `tr_jobs` ga bitta ish yoziladi, `kick` workflow'ni ishga tushiradi;
//   * `tool/translate/translate.py` ishni oladi (`/api/tr/claim`), rasmlarni kanaldan yuklaydi,
//     OCR qiladi, matnlarni worker orqali tarjima qiladi (`/api/encode/translate`), rasmga yozadi,
//     bitta fayl qilib suhbatga yuboradi va `/api/tr/finish` ni chaqiradi.
//
// Suhbat holati `app_config` da: `encbot_mode=tr`, `tr_src` (manba tili), `tr_fmt` (pdf|cbz|zip),
// `tr_title` (fayl nomi), `tr_cnt` (hisoblagich xabari).

use super::*;

pub(crate) const DDL: [&str; 4] = [
    // kind: img | zip | pdf. job_id=0 — hali ishga qo'yilmagan (yig'ilmoqda).
    "CREATE TABLE IF NOT EXISTS tr_pages (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        chat INTEGER NOT NULL,
        msg_id INTEGER NOT NULL,
        name TEXT DEFAULT '',
        kind TEXT DEFAULT 'img',
        size INTEGER DEFAULT 0,
        job_id INTEGER DEFAULT 0,
        created_at INTEGER NOT NULL
    )",
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_tr_pages_msg ON tr_pages(chat, msg_id)",
    // pages — kanal xabarlari (JSON: [{"m": kanal_xabari, "n": nom, "k": tur}]); state: queued | running | error.
    "CREATE TABLE IF NOT EXISTS tr_jobs (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        chat INTEGER NOT NULL,
        title TEXT DEFAULT '',
        src TEXT DEFAULT 'korean',
        fmt TEXT DEFAULT 'pdf',
        pages TEXT DEFAULT '[]',
        queued_at INTEGER NOT NULL,
        state TEXT DEFAULT 'queued',
        runner TEXT DEFAULT '',
        lease_until INTEGER DEFAULT 0,
        attempts INTEGER DEFAULT 0,
        error TEXT DEFAULT ''
    )",
    "CREATE INDEX IF NOT EXISTS idx_tr_q ON tr_jobs(state, queued_at)",
];

const TR_WORKFLOW: &str = "translate.yml";
/// Bitta ish uchun ijara (run o'lsa shundan keyin qayta olinadi).
const TR_LEASE_MS: i64 = 300 * 60 * 1000;
const TR_MAX_ATTEMPTS: i64 = 3;
/// Bir ishdagi fayllar soni chegarasi.
const TR_MAX_FILES: i64 = 400;

pub(crate) const BTN: &str = "\u{1F310} Manga tarjima";
const BTN_GO: &str = "\u{2705} Tarjima qilish";
const BTN_CLEAR: &str = "\u{1F5D1} Tozalash";
const BTN_QUEUE: &str = "\u{1F4CB} Tarjima navbati";

/// (tugma, kod) — manba tili. Kod `tool/translate/translate.py` va tarjima so'rovida ishlatiladi.
const LANGS: [(&str, &str); 5] = [
    ("\u{1F1F0}\u{1F1F7} Koreyscha", "korean"),
    ("\u{1F1EF}\u{1F1F5} Yaponcha", "japanese"),
    ("\u{1F1E8}\u{1F1F3} Xitoycha", "chinese"),
    ("\u{1F1EC}\u{1F1E7} Inglizcha", "english"),
    ("\u{1F1F7}\u{1F1FA} Ruscha", "russian"),
];
/// (tugma, kod) — natija fayli turi.
const FORMATS: [(&str, &str); 3] = [
    ("\u{1F4C4} PDF", "pdf"),
    ("\u{1F4DA} CBZ", "cbz"),
    ("\u{1F5DC} ZIP", "zip"),
];

fn keyboard() -> Value {
    encbot_keyboard(vec![
        vec![BTN_GO.to_string()],
        LANGS[..3].iter().map(|l| l.0.to_string()).collect(),
        LANGS[3..].iter().map(|l| l.0.to_string()).collect(),
        FORMATS.iter().map(|f| f.0.to_string()).collect(),
        vec![BTN_CLEAR.to_string(), BTN_QUEUE.to_string()],
        vec![postbot::BTN_HOME.to_string()],
    ])
}

fn lang_name(code: &str) -> &'static str {
    LANGS.iter().find(|l| l.1 == code).map(|l| l.0).unwrap_or(LANGS[0].0)
}

async fn settings(env: &Env) -> (String, String, String) {
    let src = config_get(env, "tr_src").await.filter(|v| LANGS.iter().any(|l| l.1 == v)).unwrap_or_else(|| "korean".into());
    let fmt = config_get(env, "tr_fmt").await.filter(|v| FORMATS.iter().any(|f| f.1 == v)).unwrap_or_else(|| "pdf".into());
    let title = config_get(env, "tr_title").await.unwrap_or_default();
    (src, fmt, title)
}

async fn pending_count(env: &Env, chat: i64) -> i64 {
    turso_exec(env, "SELECT COUNT(*) AS n FROM tr_pages WHERE chat=? AND job_id=0", vec![TursoArg::int(chat)]).await
        .ok().and_then(|r| first_row(&r)).map(|r| jint(&r, "n")).unwrap_or(0)
}

pub(crate) async fn menu(env: &Env, chat: i64) {
    config_put(env, "encbot_mode", "tr").await;
    config_put(env, "post_edit", "").await;
    config_put(env, "tr_cnt", "").await;
    let (src, fmt, title) = settings(env).await;
    let n = pending_count(env, chat).await;
    encbot_send(env, chat, &format!(
        "\u{1F310} <b>Manga tarjima</b>\n\n\
         Rasmlarni ketma-ket yuboring (albom qilib ham bo'ladi), yoki ZIP / CBZ / PDF fayl yuboring. \
         Yuborganlaringiz <b>eslab qolinadi</b>: bot qayta ishga tushsa ham yo'qolmaydi.\n\
         Hammasi yuborilgach \u{00AB}{BTN_GO}\u{00BB} ni bosing \u{2014} tarjima qilingan bitta fayl qaytadi (o'zbekcha).\n\n\
         Fayl nomi uchun oddiy matn yozing.\n\n\
         \u{1F310} Manba tili: <b>{}</b>\n\u{1F4E6} Natija: <b>{}</b>\n\u{270F}\u{FE0F} Nom: <b>{}</b>\n\u{1F4E5} Yig'ilgan: <b>{n}</b> ta fayl",
        lang_name(&src), fmt.to_uppercase(),
        if title.is_empty() { "(nomsiz)".to_string() } else { html_escape(&title) }),
        Some(keyboard())).await;
}

/// Qabul qilingan faylning turi: (tur, nom) yoki None.
fn classify(msg: &Value) -> Option<(&'static str, String, i64)> {
    let id = msg["message_id"].as_i64().unwrap_or(0);
    if let Some(p) = msg["photo"].as_array() {
        let size = p.last().and_then(|x| x["file_size"].as_i64()).unwrap_or(0);
        return Some(("img", format!("photo_{id}.jpg"), size));
    }
    let doc = &msg["document"];
    if !doc.is_object() {
        return None;
    }
    let mime = doc["mime_type"].as_str().unwrap_or("").to_ascii_lowercase();
    let name = doc["file_name"].as_str().unwrap_or("").to_string();
    let ext = name.to_ascii_lowercase().rsplit_once('.').map(|(_, e)| e.to_string()).unwrap_or_default();
    let size = doc["file_size"].as_i64().unwrap_or(0);
    let name = if name.is_empty() { format!("file_{id}") } else { name };
    if mime.starts_with("image/") || ["jpg", "jpeg", "png", "webp", "bmp"].contains(&ext.as_str()) {
        return Some(("img", name, size));
    }
    if ["zip", "cbz"].contains(&ext.as_str()) || mime == "application/zip" || mime == "application/x-cbz" {
        return Some(("zip", name, size));
    }
    if ext == "pdf" || mime == "application/pdf" {
        return Some(("pdf", name, size));
    }
    None
}

/// Tarjima rejimidagi xabar. `true` — ishlandi.
pub(crate) async fn on_message(env: &Env, msg: &Value, chat: i64) -> bool {
    let text = msg["text"].as_str().unwrap_or("").trim().to_string();
    if text == BTN_GO {
        go(env, chat).await;
        return true;
    }
    if text == BTN_QUEUE {
        queue_list(env, chat).await;
        return true;
    }
    if text == BTN_CLEAR {
        let n = turso_exec(env, "DELETE FROM tr_pages WHERE chat=? AND job_id=0 RETURNING id", vec![TursoArg::int(chat)]).await
            .map(|r| rows_of(&r).len()).unwrap_or(0);
        config_put(env, "tr_cnt", "").await;
        config_put(env, "tr_title", "").await;
        encbot_send(env, chat, &format!("\u{1F5D1} Tozalandi ({n} ta fayl o'chirildi). Yangidan boshlashingiz mumkin."), Some(keyboard())).await;
        return true;
    }
    if let Some(l) = LANGS.iter().find(|l| l.0 == text) {
        config_put(env, "tr_src", l.1).await;
        encbot_send(env, chat, &format!("\u{1F310} Manba tili: <b>{}</b>", l.0), Some(keyboard())).await;
        return true;
    }
    if let Some(f) = FORMATS.iter().find(|f| f.0 == text) {
        config_put(env, "tr_fmt", f.1).await;
        encbot_send(env, chat, &format!("\u{1F4E6} Natija fayli: <b>{}</b>", f.0), Some(keyboard())).await;
        return true;
    }

    let caption = msg["caption"].as_str().unwrap_or("").trim().to_string();
    let Some((kind, name, size)) = classify(msg) else {
        if !text.is_empty() {
            // Oddiy matn — fayl nomi.
            let title: String = text.chars().take(80).collect();
            config_put(env, "tr_title", &title).await;
            encbot_send(env, chat, &format!("\u{270F}\u{FE0F} Nom: <b>{}</b>", html_escape(&title)), Some(keyboard())).await;
        } else {
            encbot_send(env, chat, "Rasm, ZIP/CBZ yoki PDF fayl yuboring (rasmni \u{00AB}fayl\u{00BB} qilib yuborsangiz sifati saqlanadi).", Some(keyboard())).await;
        }
        return true;
    };
    if pending_count(env, chat).await >= TR_MAX_FILES {
        encbot_send(env, chat, &format!("\u{26A0}\u{FE0F} Bir ishga {TR_MAX_FILES} tagacha fayl. Avval \u{00AB}{BTN_GO}\u{00BB} ni bosing."), Some(keyboard())).await;
        return true;
    }
    if !caption.is_empty() && config_get(env, "tr_title").await.is_none() {
        config_put(env, "tr_title", &caption.chars().take(80).collect::<String>()).await;
    }
    let msg_id = msg["message_id"].as_i64().unwrap_or(0);
    let _ = turso_exec(env,
        "INSERT OR IGNORE INTO tr_pages (chat, msg_id, name, kind, size, created_at) VALUES (?,?,?,?,?,?)",
        vec![TursoArg::int(chat), TursoArg::int(msg_id), TursoArg::text(&name.chars().take(120).collect::<String>()),
             TursoArg::text(kind), TursoArg::int(size), TursoArg::int(now_ms())]).await;
    counter(env, chat).await;
    true
}

/// Bitta "qabul qilindi: N ta" xabari — har yangi fayl uni tahrirlaydi (40 ta rasmga 40 ta javob yo'q).
async fn counter(env: &Env, chat: i64) {
    let n = pending_count(env, chat).await;
    let text = format!("\u{1F4E5} Qabul qilindi: <b>{n}</b> ta fayl.\nHammasi yuborilgach \u{00AB}{BTN_GO}\u{00BB} ni bosing.");
    let cur: i64 = config_get(env, "tr_cnt").await.and_then(|v| v.parse().ok()).unwrap_or(0);
    if cur > 0 {
        let r = encbot_api(env, "editMessageText", json!({
            "chat_id": chat, "message_id": cur, "text": text, "parse_mode": "HTML",
        })).await;
        if r.is_ok() {
            return;
        }
        // Xabar o'chirilgan yoki matn bir xil — yangisi yuboriladi (bir xil matn xatosi ham shu yerga tushadi, zarari yo'q).
    }
    let sent = encbot_api(env, "sendMessage", json!({
        "chat_id": chat, "text": text, "parse_mode": "HTML", "reply_markup": keyboard(),
    })).await.ok().and_then(|v| v["message_id"].as_i64()).unwrap_or(0);
    if sent > 0 {
        config_put(env, "tr_cnt", &sent.to_string()).await;
    }
}

/// Yig'ilgan fayllarni kanalga ko'chirib, ishga qo'yadi.
async fn go(env: &Env, chat: i64) {
    let rows = turso_exec(env,
        "SELECT id, msg_id, name, kind FROM tr_pages WHERE chat=? AND job_id=0 ORDER BY msg_id ASC LIMIT ?",
        vec![TursoArg::int(chat), TursoArg::int(TR_MAX_FILES)]).await.map(|r| rows_of(&r)).unwrap_or_default();
    if rows.is_empty() {
        encbot_send(env, chat, "\u{1F4ED} Hali fayl yuborilmagan. Rasmlarni yoki ZIP/CBZ/PDF yuboring.", Some(keyboard())).await;
        return;
    }
    let channel = tg_channel_id(env);
    if channel == 0 {
        encbot_send(env, chat, "\u{274C} TG_CHANNEL_ID sozlanmagan.", Some(keyboard())).await;
        return;
    }
    // Kanalga: 100 tadan `copyMessages` (bitta so'rovda; albom tartibi saqlanadi).
    let mut chan: Vec<i64> = Vec::new();
    for chunk in rows.chunks(100) {
        let ids: Vec<i64> = chunk.iter().map(|r| jint(r, "msg_id")).collect();
        let got: Vec<i64> = encbot_api(env, "copyMessages", json!({
            "chat_id": channel, "from_chat_id": chat, "message_ids": ids, "disable_notification": true,
        })).await.ok().and_then(|v| v.as_array().map(|a| a.iter().map(|x| jint(x, "message_id")).collect())).unwrap_or_default();
        if got.len() == ids.len() && got.iter().all(|m| *m > 0) {
            chan.extend(got);
            continue;
        }
        // Ba'zilari ko'chmadi — tartibni saqlab birma-bir.
        let mut fail = false;
        let mut done: Vec<i64> = Vec::new();
        for id in &ids {
            match encbot_api(env, "copyMessage", json!({
                "chat_id": channel, "from_chat_id": chat, "message_id": id, "disable_notification": true,
            })).await.ok().and_then(|v| v["message_id"].as_i64()).filter(|m| *m > 0) {
                Some(m) => done.push(m),
                None => { fail = true; break; }
            }
        }
        if fail {
            let _ = encbot_api(env, "deleteMessages", json!({"chat_id": channel, "message_ids": got})).await;
            for m in &done {
                let _ = encbot_api(env, "deleteMessage", json!({"chat_id": channel, "message_id": m})).await;
            }
            if !chan.is_empty() {
                let _ = encbot_api(env, "deleteMessages", json!({"chat_id": channel, "message_ids": chan})).await;
            }
            encbot_send(env, chat, "\u{274C} Fayllarni kanalga ko'chirib bo'lmadi (bot kanalda admin ekanini tekshiring). \
                Fayllaringiz saqlanib turibdi \u{2014} \u{00AB}Tarjima qilish\u{00BB} ni qayta bosing.", Some(keyboard())).await;
            return;
        }
        chan.extend(done);
    }
    let pages: Vec<Value> = rows.iter().zip(chan.iter())
        .map(|(r, m)| json!({"m": m, "n": r["name"].as_str().unwrap_or(""), "k": r["kind"].as_str().unwrap_or("img")}))
        .collect();
    let (src, fmt, title) = settings(env).await;
    let title = if title.is_empty() { "Tarjima".to_string() } else { title };
    let ins = turso_exec(env,
        "INSERT INTO tr_jobs (chat, title, src, fmt, pages, queued_at) VALUES (?,?,?,?,?,?) RETURNING id",
        vec![TursoArg::int(chat), TursoArg::text(&title), TursoArg::text(&src), TursoArg::text(&fmt),
             TursoArg::text(&Value::Array(pages).to_string()), TursoArg::int(now_ms())]).await;
    let Some(id) = ins.ok().and_then(|r| first_row(&r)).map(|r| jint(&r, "id")) else {
        let _ = encbot_api(env, "deleteMessages", json!({"chat_id": channel, "message_ids": chan})).await;
        encbot_send(env, chat, "\u{274C} Navbatga qo'yib bo'lmadi (baza xatosi). Qayta bosing \u{2014} fayllar saqlanib turibdi.", Some(keyboard())).await;
        return;
    };
    // Faqat shu ishga olinganlar belgilanadi (shu orada yangi kelganlari keyingi ishga qoladi).
    let ids: Vec<String> = rows.iter().map(|r| jint(r, "id").to_string()).collect();
    let _ = turso_exec(env, &format!("UPDATE tr_pages SET job_id=? WHERE id IN ({})", ids.join(",")), vec![TursoArg::int(id)]).await;
    config_put(env, "tr_cnt", "").await;
    config_put(env, "tr_title", "").await;
    let n = turso_exec(env, "SELECT COUNT(*) AS n FROM tr_jobs WHERE state IN ('queued','running')", vec![]).await
        .ok().and_then(|r| first_row(&r)).map(|r| jint(&r, "n")).unwrap_or(1);
    let kick = kick(env).await;
    encbot_send(env, chat, &format!(
        "\u{2705} Navbatga qo'yildi: <b>#{id} {}</b>\n\u{1F4C4} Fayllar: {}\n\u{1F310} {} \u{2192} o'zbekcha \u{00B7} {}\n\
         \u{1F4CB} Navbatdagi tarjimalar: {n} ta\n\n{kick}",
        html_escape(&title), rows.len(), lang_name(&src), fmt.to_uppercase()), Some(keyboard())).await;
}

/// Navbatda ish bo'lsa-yu, hech kim ishlamayotgan bo'lsa — `translate.yml` ni ishga tushiradi.
pub(crate) async fn kick(env: &Env) -> String {
    let now = now_ms();
    let res = turso_exec(env,
        "SELECT
           (SELECT COUNT(*) FROM tr_jobs WHERE state='queued' OR (state='running' AND lease_until<=?)) AS pending,
           (SELECT COUNT(*) FROM tr_jobs WHERE state='running' AND lease_until>?) AS active",
        vec![TursoArg::int(now), TursoArg::int(now)]).await;
    let Some(r) = res.ok().and_then(|r| first_row(&r)) else {
        return "\u{26A0}\u{FE0F} Tarjima navbatini o'qib bo'lmadi.".into();
    };
    if jint(&r, "active") > 0 {
        return "\u{2699}\u{FE0F} Tarjima ishlayapti \u{2014} navbatdagilar ketma-ket bajariladi.".into();
    }
    if jint(&r, "pending") == 0 {
        return String::new();
    }
    let repo = tg_secret(env, "GH_REPO");
    if !repo.contains('/') {
        return "\u{26A0}\u{FE0F} GH_REPO o'rnatilmagan \u{2014} Actions ishga tushirilmadi.".into();
    }
    match gh_workflow_busy(env, &repo, TR_WORKFLOW).await {
        Ok(true) => return "\u{23F3} Tarjima (GitHub Actions) ishga tushmoqda \u{2014} biroz kuting.".into(),
        Ok(false) => {}
        Err(e) => return e,
    }
    // Ikki joydan bir vaqtda (cron + bot) ikkita run ochilmasin — atomik belgi.
    let claim = turso_exec(env,
        "INSERT INTO app_config (cfg_key,cfg_value) VALUES ('tr_kicked_at', ?)
         ON CONFLICT(cfg_key) DO UPDATE SET cfg_value=excluded.cfg_value
           WHERE CAST(app_config.cfg_value AS INTEGER) < ?
         RETURNING cfg_key",
        vec![TursoArg::text(&now.to_string()), TursoArg::int(now - 3 * 60 * 1000)]).await;
    if !claim.ok().and_then(|r| first_row(&r)).is_some() {
        return "\u{23F3} Tarjima hozirgina ishga tushirilgan \u{2014} biroz kuting.".into();
    }
    match gh_api(env, Method::Post,
        &format!("/repos/{repo}/actions/workflows/{TR_WORKFLOW}/dispatches"),
        Some(json!({"ref": "main"}))).await {
        Ok((204, _)) => "\u{25B6}\u{FE0F} Tarjima boshlandi (GitHub Actions ishga tushirildi).".into(),
        Ok((code, v)) => format!("\u{26A0}\u{FE0F} Tarjima ishga tushmadi (GitHub {code}): {}",
            html_escape(v["message"].as_str().unwrap_or(""))),
        Err(e) => format!("\u{26A0}\u{FE0F} Tarjima ishga tushmadi: {}", html_escape(&e.to_string())),
    }
}

fn short(s: &str) -> String {
    let line = s.lines().next().unwrap_or("").trim();
    let mut o: String = line.chars().take(40).collect();
    if line.chars().count() > 40 {
        o.push('\u{2026}');
    }
    if o.is_empty() { "(nomsiz)".into() } else { o }
}

fn state_icon(job: &Value) -> &'static str {
    match job["state"].as_str().unwrap_or("") {
        "running" if jint(job, "lease_until") > now_ms() => "\u{2699}\u{FE0F}",
        "error" => "\u{274C}",
        _ => "\u{23F3}",
    }
}

/// "📋 Tarjima navbati" va "📋 Holat": ishlar ro'yxati, har biriga inline tugmalar.
pub(crate) async fn queue_list(env: &Env, chat: i64) {
    let rows = turso_exec(env,
        "SELECT id, title, state, lease_until, error, pages FROM tr_jobs ORDER BY queued_at ASC LIMIT 20", vec![]).await
        .map(|r| rows_of(&r)).unwrap_or_default();
    let n = pending_count(env, chat).await;
    if rows.is_empty() {
        encbot_send(env, chat, &format!("\u{1F4CB} Tarjima navbati bo'sh.\n\u{1F4E5} Yig'ilayotgan: {n} ta fayl."), Some(keyboard())).await;
        return;
    }
    let mut kb: Vec<Vec<Value>> = Vec::new();
    let mut text = format!("\u{1F4CB} <b>Tarjima navbati</b> ({} ta)\n\u{1F4E5} Yig'ilayotgan: {n} ta fayl\n\n", rows.len());
    for r in &rows {
        let id = jint(r, "id");
        let pages = serde_json::from_str::<Vec<Value>>(r["pages"].as_str().unwrap_or("[]")).map(|v| v.len()).unwrap_or(0);
        text.push_str(&format!("{} <b>#{id} {}</b> \u{2014} {pages} fayl{}\n", state_icon(r), html_escape(&short(r["title"].as_str().unwrap_or(""))),
            if r["state"].as_str() == Some("error") { format!(" \u{2014} {}", html_escape(r["error"].as_str().unwrap_or(""))) } else { String::new() }));
        let mut row = vec![json!({"text": format!("\u{1F5D1} #{id}"), "callback_data": format!("td:{id}")})];
        if r["state"].as_str() == Some("error") {
            row.push(json!({"text": format!("\u{1F501} #{id}"), "callback_data": format!("ty:{id}")}));
        }
        kb.push(row);
    }
    encbot_send(env, chat, &text, Some(json!({"inline_keyboard": kb}))).await;
}

/// Inline tugmalar: td (o'chirishni so'rash), tx (o'chirish), tn (bekor), ty (qayta urinish).
pub(crate) async fn on_callback(env: &Env, chat: i64, data: &str) -> bool {
    let mut it = data.split(':');
    let kind = it.next().unwrap_or("");
    if !matches!(kind, "td" | "tx" | "tn" | "ty") {
        return false;
    }
    let id: i64 = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    match kind {
        "td" => encbot_send(env, chat, &format!("\u{1F5D1} #{id} tarjima ishi o'chirilsinmi?"), Some(json!({"inline_keyboard": [[
            {"text": "\u{2705} Ha, o'chirish", "callback_data": format!("tx:{id}")},
            {"text": "\u{274C} Yo'q", "callback_data": format!("tn:{id}")},
        ]]}))).await,
        "tx" => {
            let job = turso_exec(env, "DELETE FROM tr_jobs WHERE id=? RETURNING pages", vec![TursoArg::int(id)]).await
                .ok().and_then(|r| first_row(&r));
            match job {
                Some(j) => {
                    channel_cleanup(env, j["pages"].as_str().unwrap_or("[]")).await;
                    let _ = turso_exec(env, "DELETE FROM tr_pages WHERE job_id=?", vec![TursoArg::int(id)]).await;
                    encbot_send(env, chat, &format!("\u{1F5D1} #{id} o'chirildi."), Some(keyboard())).await;
                }
                None => encbot_send(env, chat, &format!("#{id} allaqachon yo'q."), Some(keyboard())).await,
            }
        }
        "tn" => encbot_send(env, chat, "O'chirilmadi.", Some(keyboard())).await,
        _ => {
            let _ = turso_exec(env,
                "UPDATE tr_jobs SET state='queued', attempts=0, error='', runner='', lease_until=0 WHERE id=? AND state='error'",
                vec![TursoArg::int(id)]).await;
            let kick = kick(env).await;
            encbot_send(env, chat, &format!("\u{1F501} #{id} yana navbatga qo'yildi.\n\n{kick}"), Some(keyboard())).await;
        }
    }
    true
}

/// Ish tugagach kanaldagi nusxalar o'chadi (100 tadan `deleteMessages`).
async fn channel_cleanup(env: &Env, pages_json: &str) {
    let channel = tg_channel_id(env);
    if channel == 0 {
        return;
    }
    let ids: Vec<i64> = serde_json::from_str::<Vec<Value>>(pages_json).unwrap_or_default().iter()
        .map(|p| jint(p, "m")).filter(|m| *m > 0).collect();
    for chunk in ids.chunks(100) {
        let _ = encbot_api(env, "deleteMessages", json!({"chat_id": channel, "message_ids": chunk})).await;
    }
}

// ── Actions (`translate.yml` -> `tool/translate/translate.py`) ──────────

fn status_head(id: i64, title: &str, attempt: i64) -> String {
    format!("\u{1F310} <b>Tarjima #{id}: {}</b>{}", html_escape(&short(title)),
        if attempt > 1 { format!(" (urinish {attempt})") } else { String::new() })
}

async fn own(env: &Env, b: &Value) -> Option<Value> {
    let runner = b["runner"].as_str().unwrap_or("");
    if runner.is_empty() {
        return None;
    }
    turso_exec(env, "SELECT * FROM tr_jobs WHERE id=? AND runner=? AND state='running'",
        vec![TursoArg::int(jint(b, "id")), TursoArg::text(runner)]).await
        .ok().and_then(|r| first_row(&r))
}

pub(crate) async fn route(mut req: Request, env: &Env, path: &str, method: Method) -> Result<Response> {
    if !encode_token_ok(&req, env) {
        return json_resp(&json!({"error": "unauthorized"}), 401);
    }
    if method != Method::Post {
        return err404("topilmadi");
    }
    let b: Value = req.json().await.unwrap_or(json!({}));
    let now = now_ms();

    if path == "/api/tr/claim" {
        let runner = b["runner"].as_str().unwrap_or("").trim().to_string();
        if runner.is_empty() || runner.len() > 64 {
            return json_resp(&json!({"error": "runner"}), 400);
        }
        let channel = tg_channel_id(env);
        for _ in 0..5 {
            let res = turso_exec(env,
                "SELECT * FROM tr_jobs WHERE state='queued' OR (state='running' AND lease_until<=?)
                  ORDER BY queued_at ASC LIMIT 1", vec![TursoArg::int(now)]).await?;
            let Some(job) = first_row(&res) else {
                return ok(json!({"none": true}));
            };
            let id = jint(&job, "id");
            let title = job["title"].as_str().unwrap_or("").to_string();
            let chat = jint(&job, "chat");
            if jint(&job, "attempts") >= TR_MAX_ATTEMPTS {
                let _ = turso_exec(env,
                    "UPDATE tr_jobs SET state='error', runner='', lease_until=0, error=? WHERE id=?",
                    vec![TursoArg::text(&format!("{TR_MAX_ATTEMPTS} marta urinildi")), TursoArg::int(id)]).await;
                encbot_send(env, chat, &format!(
                    "\u{274C} Tarjima bajarilmadi: <b>#{id} {}</b> \u{2014} {TR_MAX_ATTEMPTS} marta urinildi.\n\
                     \u{1F4CB} Tarjima navbatidan qayta urinish yoki o'chirish mumkin.", html_escape(&short(&title))), None).await;
                continue;
            }
            let got = turso_exec(env,
                "UPDATE tr_jobs SET state='running', runner=?, lease_until=?, attempts=attempts+1
                  WHERE id=? AND (state='queued' OR (state='running' AND lease_until<=?))
                  RETURNING attempts",
                vec![TursoArg::text(&runner), TursoArg::int(now + TR_LEASE_MS), TursoArg::int(id), TursoArg::int(now)]).await?;
            let Some(got) = first_row(&got) else {
                continue;
            };
            let head = status_head(id, &title, jint(&got, "attempts"));
            let status_msg = encbot_api(env, "sendMessage", json!({
                "chat_id": chat, "parse_mode": "HTML", "text": format!("{head}\n\n\u{23F3} boshlanmoqda..."),
            })).await.ok().and_then(|v| v["message_id"].as_i64()).unwrap_or(0);
            let pages: Value = serde_json::from_str(job["pages"].as_str().unwrap_or("[]")).unwrap_or(json!([]));
            return ok(json!({"channel": channel, "status_msg": status_msg, "job": {
                "id": id, "chat": chat, "title": title, "src": job["src"].as_str().unwrap_or("korean"),
                "fmt": job["fmt"].as_str().unwrap_or("pdf"), "pages": pages, "attempt": jint(&got, "attempts"),
            }}));
        }
        return ok(json!({"none": true}));
    }

    // Jonli holat: holat xabarini tahrirlaydi. Bazaga tegmaydi.
    if path == "/api/tr/progress" {
        let text: String = b["text"].as_str().unwrap_or("").chars().take(3000).collect();
        let (chat, msg) = (jint(&b, "chat"), jint(&b, "status_msg"));
        if !text.is_empty() && chat != 0 && msg > 0 {
            let head = status_head(jint(&b, "id"), b["title"].as_str().unwrap_or(""), jint(&b, "attempt"));
            let _ = encbot_api(env, "editMessageText", json!({
                "chat_id": chat, "message_id": msg, "parse_mode": "HTML",
                "text": format!("{head}\n\n{}", html_escape(&text)),
            })).await;
        }
        return ok_nostore(json!({"ok": true}));
    }

    // Yuborishdan oldin: ish hali shu run'nikimi (admin o'chirmaganmi).
    if path == "/api/tr/check" {
        return match own(env, &b).await {
            Some(_) => ok(json!({"ok": true})),
            None => json_resp(&json!({"error": "lost"}), 409),
        };
    }

    if path == "/api/tr/finish" {
        let Some(job) = own(env, &b).await else {
            return json_resp(&json!({"error": "lost"}), 409);
        };
        let id = jint(&job, "id");
        let chat = jint(&job, "chat");
        let title = job["title"].as_str().unwrap_or("").to_string();
        let status_msg = jint(&b, "status_msg");
        let say = |line: String| {
            let head = status_head(id, &title, jint(&job, "attempts"));
            json!({"chat_id": chat, "message_id": status_msg, "parse_mode": "HTML", "text": format!("{head}\n\n{line}")})
        };
        if b["ok"].as_bool() == Some(true) {
            if status_msg > 0 {
                let _ = encbot_api(env, "editMessageText", say(format!(
                    "\u{2705} Tayyor va yuborildi ({:.1} MB, {} sahifa)", jint(&b, "size") as f64 / 1048576.0, jint(&b, "pages")))).await;
            }
            channel_cleanup(env, job["pages"].as_str().unwrap_or("[]")).await;
            turso_exec(env, "DELETE FROM tr_jobs WHERE id=?", vec![TursoArg::int(id)]).await?;
            let _ = turso_exec(env, "DELETE FROM tr_pages WHERE job_id=?", vec![TursoArg::int(id)]).await;
            return ok(json!({"ok": true}));
        }
        let err = b["error"].as_str().unwrap_or("").chars().take(300).collect::<String>();
        if b["cancelled"].as_bool() == Some(true) {
            if status_msg > 0 {
                let _ = encbot_api(env, "editMessageText", say("\u{23F8} Run to'xtatildi \u{2014} ish navbatga qaytdi".to_string())).await;
            }
            turso_exec(env,
                "UPDATE tr_jobs SET state='queued', runner='', lease_until=0, attempts=MAX(attempts-1,0) WHERE id=?",
                vec![TursoArg::int(id)]).await?;
            return ok(json!({"ok": true}));
        }
        let fatal = b["fatal"].as_bool() == Some(true) || jint(&job, "attempts") >= TR_MAX_ATTEMPTS;
        if status_msg > 0 {
            let _ = encbot_api(env, "editMessageText", say(format!(
                "{} xato: {}", if fatal { "\u{274C}" } else { "\u{26A0}\u{FE0F}" }, html_escape(&err)))).await;
        }
        turso_exec(env, "UPDATE tr_jobs SET state=?, runner='', lease_until=0, error=? WHERE id=?",
            vec![TursoArg::text(if fatal { "error" } else { "queued" }), TursoArg::text(&err), TursoArg::int(id)]).await?;
        encbot_send(env, chat, &format!(
            "{} Tarjima: <b>#{id} {}</b> \u{2014} <code>{}</code>\n{}",
            if fatal { "\u{274C}" } else { "\u{26A0}\u{FE0F}" }, html_escape(&short(&title)), html_escape(&err),
            if fatal { "To'xtadi. \u{1F4CB} Tarjima navbatidan qayta urinish yoki o'chirish mumkin." }
            else { "Keyinroq yana urinib ko'riladi." }), None).await;
        return ok(json!({"ok": true}));
    }

    err404("topilmadi")
}
