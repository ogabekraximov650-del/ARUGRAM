// ═══════════════════════════════════════════════════════════
//  DUBLYAJ — kodlash botining "🗣 Dublyaj" bo'limi
// ═══════════════════════════════════════════════════════════
//
// TALAB (foydalanuvchi): tugma bosilganda bot videoni so'raydi; video yuborilsa videodagi ovozlar KLONLANIB, gaplar tanlangan
// tilga (standart — o'zbekcha) tarjima qilinadi va tarjima asl spikerning ovozi bilan ovozlanib videoga qo'yiladi (dublyaj).
//
// OQIM: video botga yuboriladi -> `copyMessage` bilan yopiq kanalga ko'chadi (fayl worker'dan o'tmaydi) -> `dub_jobs` ga ish
// yoziladi -> `kick` ikkinchi GitHub akkauntdagi (`GH_REPO`) ALOHIDA workflow'ni (`dub.yml`, shablon `tool/dub/dub.workflow.yml`)
// ishga tushiradi -> `tool/dub/dub.py` ishni oladi (`/api/dub/claim`), videoni kanaldan yuklaydi, VoiceStudio (CPU) bilan dublyaj
// qiladi (`tool/voicedub/run.py`: Whisper -> NLLB tarjima -> OmniVoice klon ovozi -> ffmpeg), tayyor videoni kanalga yuklaydi va
// `/api/dub/finish` — worker tayyor videoni `copyMessage` bilan admin chatiga ko'chiradi, kanal nusxalarini o'chiradi.
// Bitta run — bitta video (CPU'da sekin: ~9 daqiqa / 30 soniya video), shu sabab video uzunligi `MAX_DUR_S` bilan cheklangan.
//
// Suhbat holati: `app_config.encbot_mode = dub`. Izohga (caption) maqsad til kodi yoziladi (uz, en, ru, tr...); bo'sh bo'lsa — uz.

use super::*;

pub(crate) const DDL: [&str; 2] = [
    // ch_msg — kanaldagi nusxa xabari; state: queued | running | error.
    "CREATE TABLE IF NOT EXISTS dub_jobs (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        chat INTEGER NOT NULL,
        name TEXT DEFAULT '',
        lang TEXT DEFAULT 'uz',
        dur INTEGER DEFAULT 0,
        ch_msg INTEGER NOT NULL,
        size INTEGER DEFAULT 0,
        queued_at INTEGER NOT NULL,
        state TEXT DEFAULT 'queued',
        runner TEXT DEFAULT '',
        lease_until INTEGER DEFAULT 0,
        attempts INTEGER DEFAULT 0,
        error TEXT DEFAULT ''
    )",
    "CREATE INDEX IF NOT EXISTS idx_dub_q ON dub_jobs(state, queued_at)",
];

const DUB_WORKFLOW: &str = "dub.yml";
/// Bitta ish uchun ijara (run o'lsa shundan keyin qayta olinadi).
const DUB_LEASE_MS: i64 = 330 * 60 * 1000;
/// Video uzunligi chegarasi (soniya): CPU'da dublyaj sekin.
const MAX_DUR_S: i64 = 180;
const DUB_MAX_ATTEMPTS: i64 = 2;

pub(crate) const BTN: &str = "\u{1F5E3} Dublyaj";
const BTN_QUEUE: &str = "\u{1F4CB} Dublyaj navbati";

fn keyboard() -> Value {
    encbot_keyboard(vec![
        vec![ENCBOT_BTN_STATUS.to_string(), BTN_QUEUE.to_string()],
        vec![postbot::BTN_HOME.to_string()],
    ])
}

pub(crate) async fn menu(env: &Env, chat: i64) {
    config_put(env, "encbot_mode", "dub").await;
    config_put(env, "post_edit", "").await;
    encbot_send(env, chat,
        "\u{1F5E3} <b>Dublyaj</b>\n\n\
         Videoni yuboring \u{2014} videodagi ovoz <b>klonlanadi</b>, gaplar <b>o'zbekchaga</b> tarjima qilinadi va tarjima asl \
         spikerning ovozi bilan aytilib, videoga qo'yiladi.\n\n\
         \u{1F310} Boshqa tilga: video izohiga (caption) til kodini yozing (<code>uz</code>, <code>en</code>, <code>ru</code>, <code>tr</code>, ...). Bo'sh bo'lsa \u{2014} <code>uz</code>.\n\
         \u{23F1} Video uzunligi <b>3 daqiqagacha</b> (jarayon GPU'siz, 30 soniya video ~10 daqiqa oladi).\n\
         Bir nechta video ketma-ket yuborsangiz, navbat bilan ishlanadi. Tayyor video shu chatga keladi.",
        Some(keyboard())).await;
}

/// Qabul qilingan video: (nom, hajm, davomiylik soniyada — fayl sifatida yuborilsa 0) yoki None.
fn classify(msg: &Value) -> Option<(String, i64, i64)> {
    let id = msg["message_id"].as_i64().unwrap_or(0);
    let v = &msg["video"];
    if v.is_object() {
        let name = v["file_name"].as_str().filter(|n| !n.is_empty()).map(|n| n.to_string()).unwrap_or_else(|| format!("video_{id}"));
        return Some((name, v["file_size"].as_i64().unwrap_or(0), v["duration"].as_i64().unwrap_or(0)));
    }
    let doc = &msg["document"];
    if !doc.is_object() {
        return None;
    }
    let mime = doc["mime_type"].as_str().unwrap_or("").to_ascii_lowercase();
    let name = doc["file_name"].as_str().unwrap_or("").to_string();
    let ext = name.to_ascii_lowercase().rsplit_once('.').map(|(_, e)| e.to_string()).unwrap_or_default();
    if mime.starts_with("video/") || ["mp4", "mkv", "mov", "avi", "webm", "m4v", "ts", "flv", "wmv", "3gp"].contains(&ext.as_str()) {
        let name = if name.is_empty() { format!("video_{id}") } else { name };
        return Some((name, doc["file_size"].as_i64().unwrap_or(0), 0));
    }
    None
}

/// Izohdan maqsad til kodi: "uz", "en", "ru", "tr", ... (2–3 harf) yoki til nomi; topilmasa — "uz".
fn parse_lang(caption: &str) -> String {
    let first = caption.split_whitespace().next().unwrap_or("").trim_matches(|c: char| !c.is_alphanumeric()).to_ascii_lowercase();
    let by_name = match first.as_str() {
        "uzbek" | "ozbek" | "o'zbek" | "o\u{2019}zbek" | "uzbekcha" | "ozbekcha" => "uz",
        "english" | "ingliz" | "inglizcha" => "en",
        "russian" | "rus" | "ruscha" => "ru",
        "turkish" | "turk" | "turkcha" => "tr",
        "arabic" | "arab" | "arabcha" => "ar",
        _ => "",
    };
    if !by_name.is_empty() {
        return by_name.to_string();
    }
    if (2..=3).contains(&first.len()) && first.chars().all(|c| c.is_ascii_alphabetic()) {
        return first;
    }
    "uz".to_string()
}

/// Dublyaj rejimidagi xabar. `true` — ishlandi.
pub(crate) async fn on_message(env: &Env, msg: &Value, chat: i64) -> bool {
    let text = msg["text"].as_str().unwrap_or("").trim().to_string();
    if text == BTN_QUEUE {
        queue_list(env, chat).await;
        return true;
    }
    let Some((fname, size, dur)) = classify(msg) else {
        encbot_send(env, chat, "Dublyaj uchun video yuboring (oddiy video yoki video fayl).", Some(keyboard())).await;
        return true;
    };
    if dur > MAX_DUR_S {
        encbot_send(env, chat, &format!("\u{274C} Video juda uzun ({}:{:02}). Dublyaj GPU'siz ishlagani uchun {} daqiqagacha video qabul qilinadi.", dur / 60, dur % 60, MAX_DUR_S / 60), Some(keyboard())).await;
        return true;
    }
    let channel = tg_channel_id(env);
    if channel == 0 {
        encbot_send(env, chat, "\u{274C} TG_CHANNEL_ID sozlanmagan.", Some(keyboard())).await;
        return true;
    }
    let msg_id = msg["message_id"].as_i64().unwrap_or(0);
    // Takror webhook: shu xabar uchun ish allaqachon bormi (kanalga ikki marta ko'chirmaslik).
    let dup = turso_exec(env, "SELECT id FROM dub_jobs WHERE chat=? AND name=? AND size=? AND queued_at>?",
        vec![TursoArg::int(chat), TursoArg::text(&fname.chars().take(120).collect::<String>()), TursoArg::int(size),
             TursoArg::int(now_ms() - 60_000)]).await.ok().and_then(|r| first_row(&r)).is_some();
    if dup && size > 0 {
        return true;
    }
    let ch_msg = encbot_api(env, "copyMessage", json!({
        "chat_id": channel, "from_chat_id": chat, "message_id": msg_id, "disable_notification": true,
    })).await.ok().and_then(|v| v["message_id"].as_i64()).filter(|m| *m > 0);
    let Some(ch_msg) = ch_msg else {
        encbot_send(env, chat, "\u{274C} Videoni kanalga ko'chirib bo'lmadi (bot kanalda admin ekanini tekshiring). Qayta yuboring.", Some(keyboard())).await;
        return true;
    };
    let caption = msg["caption"].as_str().unwrap_or("").trim().to_string();
    let lang = parse_lang(&caption);
    let name: String = if caption.is_empty() { fname } else { caption }.chars().take(120).collect();
    let ins = turso_exec(env,
        "INSERT INTO dub_jobs (chat, name, lang, dur, ch_msg, size, queued_at) VALUES (?,?,?,?,?,?,?) RETURNING id",
        vec![TursoArg::int(chat), TursoArg::text(&name), TursoArg::text(&lang), TursoArg::int(dur), TursoArg::int(ch_msg),
             TursoArg::int(size), TursoArg::int(now_ms())]).await;
    let Some(id) = ins.ok().and_then(|r| first_row(&r)).map(|r| jint(&r, "id")) else {
        let _ = encbot_api(env, "deleteMessage", json!({"chat_id": channel, "message_id": ch_msg})).await;
        encbot_send(env, chat, "\u{274C} Navbatga qo'yib bo'lmadi (baza xatosi). Qayta yuboring.", Some(keyboard())).await;
        return true;
    };
    let n = turso_exec(env, "SELECT COUNT(*) AS n FROM dub_jobs WHERE state IN ('queued','running')", vec![]).await
        .ok().and_then(|r| first_row(&r)).map(|r| jint(&r, "n")).unwrap_or(1);
    let kick = kick(env).await;
    encbot_send(env, chat, &format!(
        "\u{2705} Navbatga qo'yildi: <b>#{id} {}</b>\n\u{1F310} Til: <code>{lang}</code>\n\u{1F4CB} Navbatda: {n} ta\n\n{kick}", html_escape(&short(&name))), Some(keyboard())).await;
    true
}

/// Navbatda ish bo'lsa-yu, hech kim ishlamayotgan bo'lsa — `dub.yml` ni ishga tushiradi.
pub(crate) async fn kick(env: &Env) -> String {
    let now = now_ms();
    let res = turso_exec(env,
        "SELECT
           (SELECT COUNT(*) FROM dub_jobs WHERE state='queued' OR (state='running' AND lease_until<=?)) AS pending,
           (SELECT COUNT(*) FROM dub_jobs WHERE state='running' AND lease_until>?) AS active",
        vec![TursoArg::int(now), TursoArg::int(now)]).await;
    let Some(r) = res.ok().and_then(|r| first_row(&r)) else {
        return "\u{26A0}\u{FE0F} Dublyaj navbatini o'qib bo'lmadi.".into();
    };
    if jint(&r, "active") > 0 {
        return "\u{2699}\u{FE0F} Dublyaj ishlayapti \u{2014} navbatdagilar ketma-ket bajariladi.".into();
    }
    if jint(&r, "pending") == 0 {
        return String::new();
    }
    let repo = tg_secret(env, "GH_REPO");
    if !repo.contains('/') {
        return "\u{26A0}\u{FE0F} GH_REPO o'rnatilmagan \u{2014} Actions ishga tushirilmadi.".into();
    }
    match gh_workflow_busy(env, &repo, DUB_WORKFLOW).await {
        Ok(true) => return "\u{23F3} Dublyaj (GitHub Actions) ishga tushmoqda \u{2014} biroz kuting.".into(),
        Ok(false) => {}
        Err(e) => return e,
    }
    // Ikki joydan bir vaqtda (cron + bot) ikkita run ochilmasin — atomik belgi.
    let claim = turso_exec(env,
        "INSERT INTO app_config (cfg_key,cfg_value) VALUES ('dub_kicked_at', ?)
         ON CONFLICT(cfg_key) DO UPDATE SET cfg_value=excluded.cfg_value
           WHERE CAST(app_config.cfg_value AS INTEGER) < ?
         RETURNING cfg_key",
        vec![TursoArg::text(&now.to_string()), TursoArg::int(now - 3 * 60 * 1000)]).await;
    if !claim.ok().and_then(|r| first_row(&r)).is_some() {
        return "\u{23F3} Dublyaj hozirgina ishga tushirilgan \u{2014} biroz kuting.".into();
    }
    match gh_api(env, Method::Post,
        &format!("/repos/{repo}/actions/workflows/{DUB_WORKFLOW}/dispatches"),
        Some(json!({"ref": "main"}))).await {
        Ok((204, _)) => "\u{25B6}\u{FE0F} Dublyaj boshlandi (GitHub Actions ishga tushirildi).".into(),
        Ok((code, v)) => format!("\u{26A0}\u{FE0F} Dublyaj ishga tushmadi (GitHub {code}): {}",
            html_escape(v["message"].as_str().unwrap_or(""))),
        Err(e) => format!("\u{26A0}\u{FE0F} Dublyaj ishga tushmadi: {}", html_escape(&e.to_string())),
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

/// "📋 Holat" — "Dublyaj" bo'limida: hozirgi ish va navbat (jonli holat dublyaj ayotgan videoning o'z xabarida yangilanadi).
pub(crate) async fn status(env: &Env, chat: i64) {
    let rows = turso_exec(env,
        "SELECT id, name, state, lease_until FROM dub_jobs ORDER BY queued_at ASC LIMIT 10", vec![]).await
        .map(|r| rows_of(&r)).unwrap_or_default();
    let last = config_get(env, "dub_last").await.unwrap_or_default();
    let last_txt = if last.is_empty() { String::new() } else { format!("\n\n\u{1F550} <b>Oxirgi kodlash:</b>\n{}", html_escape(&last)) };
    if rows.is_empty() {
        encbot_send(env, chat, &format!("\u{1F5E3} <b>Dublyaj</b>\n\n\u{1F4A4} Hozir dublyaj ayotgan yoki navbatdagi video yo'q.{last_txt}"), Some(keyboard())).await;
        return;
    }
    let mut text = String::from("\u{1F5E3} <b>Dublyaj</b>\n\n");
    for r in &rows {
        text.push_str(&format!("{} <b>#{} {}</b>\n", state_icon(r), jint(r, "id"), html_escape(&short(r["name"].as_str().unwrap_or("")))));
    }
    text.push_str("\n\u{2699}\u{FE0F} \u{2014} dublyaj moqda; jarayon shu video uchun yuborilgan holat xabarida ko'rinadi.");
    text.push_str(&last_txt);
    encbot_send(env, chat, &text, Some(keyboard())).await;
}

/// "📋 Dublyaj navbati": ishlar ro'yxati, har biriga inline tugmalar.
pub(crate) async fn queue_list(env: &Env, chat: i64) {
    let rows = turso_exec(env,
        "SELECT id, name, state, lease_until, error FROM dub_jobs ORDER BY queued_at ASC LIMIT 20", vec![]).await
        .map(|r| rows_of(&r)).unwrap_or_default();
    if rows.is_empty() {
        encbot_send(env, chat, "\u{1F4CB} Dublyaj navbati bo'sh.", Some(keyboard())).await;
        return;
    }
    let mut kb: Vec<Vec<Value>> = Vec::new();
    let mut text = format!("\u{1F4CB} <b>Dublyaj navbati</b> ({} ta)\n\n", rows.len());
    for r in &rows {
        let id = jint(r, "id");
        text.push_str(&format!("{} <b>#{id} {}</b>{}\n", state_icon(r), html_escape(&short(r["name"].as_str().unwrap_or(""))),
            if r["state"].as_str() == Some("error") { format!(" \u{2014} {}", html_escape(r["error"].as_str().unwrap_or(""))) } else { String::new() }));
        let mut row = vec![json!({"text": format!("\u{1F5D1} #{id}"), "callback_data": format!("dd:{id}")})];
        if r["state"].as_str() == Some("error") {
            row.push(json!({"text": format!("\u{1F501} #{id}"), "callback_data": format!("dy:{id}")}));
        }
        kb.push(row);
    }
    encbot_send(env, chat, &text, Some(json!({"inline_keyboard": kb}))).await;
}

/// Inline tugmalar: cd (o'chirishni so'rash), cx (o'chirish), cn (bekor), cy (qayta urinish).
pub(crate) async fn on_callback(env: &Env, chat: i64, data: &str) -> bool {
    let mut it = data.split(':');
    let kind = it.next().unwrap_or("");
    if !matches!(kind, "dd" | "dx" | "dn" | "dy") {
        return false;
    }
    let id: i64 = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    match kind {
        "dd" => encbot_send(env, chat, &format!("\u{1F5D1} #{id} dublyaj ishi o'chirilsinmi?"), Some(json!({"inline_keyboard": [[
            {"text": "\u{2705} Ha, o'chirish", "callback_data": format!("dx:{id}")},
            {"text": "\u{274C} Yo'q", "callback_data": format!("dn:{id}")},
        ]]}))).await,
        "dx" => {
            let job = turso_exec(env, "DELETE FROM dub_jobs WHERE id=? RETURNING ch_msg", vec![TursoArg::int(id)]).await
                .ok().and_then(|r| first_row(&r));
            match job {
                Some(j) => {
                    channel_cleanup(env, jint(&j, "ch_msg")).await;
                    encbot_send(env, chat, &format!("\u{1F5D1} #{id} o'chirildi."), Some(keyboard())).await;
                }
                None => encbot_send(env, chat, &format!("#{id} allaqachon yo'q."), Some(keyboard())).await,
            }
        }
        "dn" => encbot_send(env, chat, "O'chirilmadi.", Some(keyboard())).await,
        _ => {
            let _ = turso_exec(env,
                "UPDATE dub_jobs SET state='queued', attempts=0, error='', runner='', lease_until=0 WHERE id=? AND state='error'",
                vec![TursoArg::int(id)]).await;
            let kick = kick(env).await;
            encbot_send(env, chat, &format!("\u{1F501} #{id} yana navbatga qo'yildi.\n\n{kick}"), Some(keyboard())).await;
        }
    }
    true
}

/// Ish tugagach kanaldagi nusxa o'chadi.
async fn channel_cleanup(env: &Env, ch_msg: i64) {
    let channel = tg_channel_id(env);
    if channel == 0 || ch_msg <= 0 {
        return;
    }
    let _ = encbot_api(env, "deleteMessage", json!({"chat_id": channel, "message_id": ch_msg})).await;
}

// ── Actions (`dub.yml` -> `tool/dub/dub.py`) ─────────────

fn status_head(id: i64, name: &str, attempt: i64) -> String {
    format!("\u{1F5E3} <b>Dublyaj #{id}: {}</b>{}", html_escape(&short(name)),
        if attempt > 1 { format!(" (urinish {attempt})") } else { String::new() })
}

async fn own(env: &Env, b: &Value) -> Option<Value> {
    let runner = b["runner"].as_str().unwrap_or("");
    if runner.is_empty() {
        return None;
    }
    turso_exec(env, "SELECT * FROM dub_jobs WHERE id=? AND runner=? AND state='running'",
        vec![TursoArg::int(jint(b, "id")), TursoArg::text(runner)]).await
        .ok().and_then(|r| first_row(&r))
}

async fn edit_status(env: &Env, chat: i64, msg: i64, html: &str) {
    if msg > 0 {
        let _ = encbot_api(env, "editMessageText", json!({
            "chat_id": chat, "message_id": msg, "text": html, "parse_mode": "HTML",
        })).await;
    }
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

    if path == "/api/dub/claim" {
        let runner = b["runner"].as_str().unwrap_or("").trim().to_string();
        if runner.is_empty() || runner.len() > 64 {
            return json_resp(&json!({"error": "runner"}), 400);
        }
        let channel = tg_channel_id(env);
        for _ in 0..5 {
            let res = turso_exec(env,
                "SELECT * FROM dub_jobs WHERE state='queued' OR (state='running' AND lease_until<=?)
                  ORDER BY queued_at ASC LIMIT 1", vec![TursoArg::int(now)]).await?;
            let Some(job) = first_row(&res) else {
                return ok(json!({"none": true}));
            };
            let id = jint(&job, "id");
            let name = job["name"].as_str().unwrap_or("").to_string();
            let chat = jint(&job, "chat");
            if jint(&job, "attempts") >= DUB_MAX_ATTEMPTS {
                let _ = turso_exec(env,
                    "UPDATE dub_jobs SET state='error', runner='', lease_until=0, error=? WHERE id=?",
                    vec![TursoArg::text(&format!("{DUB_MAX_ATTEMPTS} marta urinildi")), TursoArg::int(id)]).await;
                encbot_send(env, chat, &format!(
                    "\u{274C} Dublyaj bajarilmadi: <b>#{id} {}</b> \u{2014} {DUB_MAX_ATTEMPTS} marta urinildi.\n\
                     \u{1F4CB} Dublyaj navbatidan qayta urinish yoki o'chirish mumkin.", html_escape(&short(&name))), None).await;
                continue;
            }
            let got = turso_exec(env,
                "UPDATE dub_jobs SET state='running', runner=?, lease_until=?, attempts=attempts+1
                  WHERE id=? AND (state='queued' OR (state='running' AND lease_until<=?))
                  RETURNING attempts",
                vec![TursoArg::text(&runner), TursoArg::int(now + DUB_LEASE_MS), TursoArg::int(id), TursoArg::int(now)]).await?;
            let Some(got) = first_row(&got) else {
                continue;
            };
            let head = status_head(id, &name, jint(&got, "attempts"));
            let status_msg = encbot_api(env, "sendMessage", json!({
                "chat_id": chat, "parse_mode": "HTML", "text": format!("{head}\n\n\u{23F3} boshlanmoqda..."),
            })).await.ok().and_then(|v| v["message_id"].as_i64()).unwrap_or(0);
            return ok(json!({"channel": channel, "status_msg": status_msg, "job": {
                "id": id, "chat": chat, "name": name, "lang": job["lang"].as_str().unwrap_or("uz"), "ch_msg": jint(&job, "ch_msg"),
                "attempt": jint(&got, "attempts"),
            }}));
        }
        return ok(json!({"none": true}));
    }

    // Jonli holat: holat xabarini tahrirlaydi. Bazaga tegmaydi.
    if path == "/api/dub/progress" {
        let text: String = b["text"].as_str().unwrap_or("").chars().take(3000).collect();
        let (chat, msg) = (jint(&b, "chat"), jint(&b, "status_msg"));
        if !text.is_empty() && chat != 0 && msg > 0 {
            let head = status_head(jint(&b, "id"), b["name"].as_str().unwrap_or(""), jint(&b, "attempt"));
            edit_status(env, chat, msg, &format!("{head}\n\n{}", html_escape(&text))).await;
        }
        return ok_nostore(json!({"ok": true}));
    }

    // Workflow oxirida: navbatda yana ish bormi (bo'lsa yangi run ochiladi).
    if path == "/api/dub/peek" {
        let n = turso_exec(env,
            "SELECT COUNT(*) AS n FROM dub_jobs WHERE state='queued' OR (state='running' AND lease_until<=?)",
            vec![TursoArg::int(now)]).await.ok().and_then(|r| first_row(&r)).map(|r| jint(&r, "n")).unwrap_or(0);
        return ok_nostore(json!({"pending": n}));
    }

    // Yuborishdan oldin: ish hali shu run'nikimi (admin o'chirmaganmi).
    if path == "/api/dub/check" {
        return match own(env, &b).await {
            Some(_) => ok(json!({"ok": true})),
            None => json_resp(&json!({"error": "lost"}), 409),
        };
    }

    if path == "/api/dub/finish" {
        let Some(job) = own(env, &b).await else {
            return json_resp(&json!({"error": "lost"}), 409);
        };
        let id = jint(&job, "id");
        let chat = jint(&job, "chat");
        let name = job["name"].as_str().unwrap_or("").to_string();
        let status_msg = jint(&b, "status_msg");
        let say = |line: String| format!("{}\n\n{line}", status_head(id, &name, jint(&job, "attempts")));
        if b["ok"].as_bool() == Some(true) {
            // Tayyor video Actions tomonidan kanalga yuklangan (`res_msg`): bot uni admin chatiga ko'chiradi.
            let res_msg = jint(&b, "res_msg");
            let lang = job["lang"].as_str().unwrap_or("uz").to_string();
            let cap = format!("\u{1F5E3} {} \u{2014} dublyaj ({lang})", short(&name));
            let delivered = if res_msg > 0 {
                encbot_api(env, "copyMessage", json!({
                    "chat_id": chat, "from_chat_id": tg_channel_id(env), "message_id": res_msg, "caption": cap,
                })).await.is_ok()
            } else { false };
            let summary: String = b["summary"].as_str().unwrap_or("").chars().take(400).collect();
            if !summary.is_empty() {
                config_put(env, "dub_last", &format!("#{id} {} \u{2014} {summary}", short(&name))).await;
            }
            if status_msg > 0 {
                edit_status(env, chat, status_msg, &say(format!(
                    "{} Tayyor \u{2014} dublyaj qilingan video {}\n{}",
                    if delivered { "\u{2705}" } else { "\u{26A0}\u{FE0F}" },
                    if delivered { "shu chatga yuborildi" } else { "kanalga yuklandi, lekin chatga ko'chirib bo'lmadi" },
                    html_escape(&summary)))).await;
            }
            channel_cleanup(env, jint(&job, "ch_msg")).await;
            if delivered {
                channel_cleanup(env, res_msg).await;
            }
            turso_exec(env, "DELETE FROM dub_jobs WHERE id=?", vec![TursoArg::int(id)]).await?;
            return ok(json!({"ok": true}));
        }
        let err = b["error"].as_str().unwrap_or("").chars().take(300).collect::<String>();
        if b["cancelled"].as_bool() == Some(true) {
            if status_msg > 0 {
                edit_status(env, chat, status_msg, &say("\u{23F8} Run to'xtatildi \u{2014} ish navbatga qaytdi".to_string())).await;
            }
            turso_exec(env,
                "UPDATE dub_jobs SET state='queued', runner='', lease_until=0, attempts=MAX(attempts-1,0) WHERE id=?",
                vec![TursoArg::int(id)]).await?;
            return ok(json!({"ok": true}));
        }
        let fatal = b["fatal"].as_bool() == Some(true) || jint(&job, "attempts") >= DUB_MAX_ATTEMPTS;
        if status_msg > 0 {
            edit_status(env, chat, status_msg, &say(format!(
                "{} xato: {}", if fatal { "\u{274C}" } else { "\u{26A0}\u{FE0F}" }, html_escape(&err)))).await;
        }
        turso_exec(env, "UPDATE dub_jobs SET state=?, runner='', lease_until=0, error=? WHERE id=?",
            vec![TursoArg::text(if fatal { "error" } else { "queued" }), TursoArg::text(&err), TursoArg::int(id)]).await?;
        encbot_send(env, chat, &format!(
            "{} Dublyaj: <b>#{id} {}</b> \u{2014} <code>{}</code>\n{}",
            if fatal { "\u{274C}" } else { "\u{26A0}\u{FE0F}" }, html_escape(&short(&name)), html_escape(&err),
            if fatal { "To'xtadi. \u{1F4CB} Dublyaj navbatidan qayta urinish yoki o'chirish mumkin." }
            else { "Keyinroq yana urinib ko'riladi." }), None).await;
        return ok(json!({"ok": true}));
    }

    err404("topilmadi")
}
