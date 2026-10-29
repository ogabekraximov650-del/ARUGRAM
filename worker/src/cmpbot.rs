// ══════════════════════════════════════════════════════════════
//  SIQISH — kodlash botining "🗜 Siqish" bo'limi
// ══════════════════════════════════════════════════════════════
//
// TALAB (foydalanuvchi): tugma bosilganda bot videoni so'raydi; video yuborilsa H.265, 720p, CRF 30 bilan
// siqilib, GIF (ovozsiz animatsiya) qilib ASOSIY akkauntning "Saqlangan xabarlar"iga (Saved Messages) yuboriladi.
//
// OQIM: video botga yuboriladi -> `copyMessage` bilan yopiq kanalga ko'chadi (fayl worker'dan o'tmaydi, hajm
// chegarasi yo'q) -> `cmp_jobs` ga ish yoziladi -> `kick` ikkinchi GitHub akkauntdagi (`GH_REPO`) ALOHIDA workflow'ni
// (`compress.yml`, shablon `tool/compress/compress.workflow.yml`) ishga tushiradi -> `tool/compress/compress.py`
// ishni oladi (`/api/cmp/claim`), kanaldan asosiy akkaunt bilan yuklaydi, ffmpeg (libx265, CRF 30, qisqa tomoni
// 720 dan katta bo'lsa 720 gacha) bilan siqadi, asosiy akkauntning "Saqlangan xabarlar"iga yuboradi, `/api/cmp/finish`.
// Bitta run — bitta video; navbatda yana ish bo'lsa workflow yangi run ochadi (`/api/cmp/peek`).
//
// Suhbat holati: `app_config.encbot_mode = cmp`.

use super::*;

pub(crate) const DDL: [&str; 2] = [
    // ch_msg — kanaldagi nusxa xabari; state: queued | running | error.
    "CREATE TABLE IF NOT EXISTS cmp_jobs (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        chat INTEGER NOT NULL,
        name TEXT DEFAULT '',
        ch_msg INTEGER NOT NULL,
        size INTEGER DEFAULT 0,
        queued_at INTEGER NOT NULL,
        state TEXT DEFAULT 'queued',
        runner TEXT DEFAULT '',
        lease_until INTEGER DEFAULT 0,
        attempts INTEGER DEFAULT 0,
        error TEXT DEFAULT ''
    )",
    "CREATE INDEX IF NOT EXISTS idx_cmp_q ON cmp_jobs(state, queued_at)",
];

const CMP_WORKFLOW: &str = "compress.yml";
/// Bitta ish uchun ijara (run o'lsa shundan keyin qayta olinadi).
const CMP_LEASE_MS: i64 = 340 * 60 * 1000;
const CMP_MAX_ATTEMPTS: i64 = 3;

pub(crate) const BTN: &str = "\u{1F5DC} Siqish";
const BTN_QUEUE: &str = "\u{1F4CB} Siqish navbati";

fn keyboard() -> Value {
    encbot_keyboard(vec![
        vec![ENCBOT_BTN_STATUS.to_string(), BTN_QUEUE.to_string()],
        vec![postbot::BTN_HOME.to_string()],
    ])
}

pub(crate) async fn menu(env: &Env, chat: i64) {
    config_put(env, "encbot_mode", "cmp").await;
    config_put(env, "post_edit", "").await;
    encbot_send(env, chat,
        "\u{1F5DC} <b>Siqish</b>\n\n\
         Videoni yuboring (oddiy video yoki fayl sifatida, hajmi cheklanmagan) \u{2014} men uni \
         <b>H.265, 720p, CRF 30</b> bilan siqib, <b>GIF</b> (ovozsiz) qilib asosiy akkauntingizning <b>Saqlangan xabarlar</b>iga yuboraman.\n\
         Bir nechta video ketma-ket yuborsangiz, navbat bilan siqiladi. Video izohiga (caption) yozsangiz, nom shu bo'ladi.",
        Some(keyboard())).await;
}

/// Qabul qilingan video: (nom, hajm) yoki None.
fn classify(msg: &Value) -> Option<(String, i64)> {
    let id = msg["message_id"].as_i64().unwrap_or(0);
    let v = &msg["video"];
    if v.is_object() {
        let name = v["file_name"].as_str().filter(|n| !n.is_empty()).map(|n| n.to_string()).unwrap_or_else(|| format!("video_{id}"));
        return Some((name, v["file_size"].as_i64().unwrap_or(0)));
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
        return Some((name, doc["file_size"].as_i64().unwrap_or(0)));
    }
    None
}

/// Siqish rejimidagi xabar. `true` — ishlandi.
pub(crate) async fn on_message(env: &Env, msg: &Value, chat: i64) -> bool {
    let text = msg["text"].as_str().unwrap_or("").trim().to_string();
    if text == BTN_QUEUE {
        queue_list(env, chat).await;
        return true;
    }
    let Some((fname, size)) = classify(msg) else {
        encbot_send(env, chat, "Siqish uchun video yuboring (oddiy video yoki video fayl).", Some(keyboard())).await;
        return true;
    };
    let channel = tg_channel_id(env);
    if channel == 0 {
        encbot_send(env, chat, "\u{274C} TG_CHANNEL_ID sozlanmagan.", Some(keyboard())).await;
        return true;
    }
    let msg_id = msg["message_id"].as_i64().unwrap_or(0);
    // Takror webhook: shu xabar uchun ish allaqachon bormi (kanalga ikki marta ko'chirmaslik).
    let dup = turso_exec(env, "SELECT id FROM cmp_jobs WHERE chat=? AND name=? AND size=? AND queued_at>?",
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
    let name: String = if caption.is_empty() { fname } else { caption }.chars().take(120).collect();
    let ins = turso_exec(env,
        "INSERT INTO cmp_jobs (chat, name, ch_msg, size, queued_at) VALUES (?,?,?,?,?) RETURNING id",
        vec![TursoArg::int(chat), TursoArg::text(&name), TursoArg::int(ch_msg), TursoArg::int(size), TursoArg::int(now_ms())]).await;
    let Some(id) = ins.ok().and_then(|r| first_row(&r)).map(|r| jint(&r, "id")) else {
        let _ = encbot_api(env, "deleteMessage", json!({"chat_id": channel, "message_id": ch_msg})).await;
        encbot_send(env, chat, "\u{274C} Navbatga qo'yib bo'lmadi (baza xatosi). Qayta yuboring.", Some(keyboard())).await;
        return true;
    };
    let n = turso_exec(env, "SELECT COUNT(*) AS n FROM cmp_jobs WHERE state IN ('queued','running')", vec![]).await
        .ok().and_then(|r| first_row(&r)).map(|r| jint(&r, "n")).unwrap_or(1);
    let kick = kick(env).await;
    encbot_send(env, chat, &format!(
        "\u{2705} Navbatga qo'yildi: <b>#{id} {}</b>\n\u{1F4CB} Navbatda: {n} ta\n\n{kick}", html_escape(&short(&name))), Some(keyboard())).await;
    true
}

/// Navbatda ish bo'lsa-yu, hech kim ishlamayotgan bo'lsa — `compress.yml` ni ishga tushiradi.
pub(crate) async fn kick(env: &Env) -> String {
    let now = now_ms();
    let res = turso_exec(env,
        "SELECT
           (SELECT COUNT(*) FROM cmp_jobs WHERE state='queued' OR (state='running' AND lease_until<=?)) AS pending,
           (SELECT COUNT(*) FROM cmp_jobs WHERE state='running' AND lease_until>?) AS active",
        vec![TursoArg::int(now), TursoArg::int(now)]).await;
    let Some(r) = res.ok().and_then(|r| first_row(&r)) else {
        return "\u{26A0}\u{FE0F} Siqish navbatini o'qib bo'lmadi.".into();
    };
    if jint(&r, "active") > 0 {
        return "\u{2699}\u{FE0F} Siqish ishlayapti \u{2014} navbatdagilar ketma-ket bajariladi.".into();
    }
    if jint(&r, "pending") == 0 {
        return String::new();
    }
    let repo = tg_secret(env, "GH_REPO");
    if !repo.contains('/') {
        return "\u{26A0}\u{FE0F} GH_REPO o'rnatilmagan \u{2014} Actions ishga tushirilmadi.".into();
    }
    match gh_workflow_busy(env, &repo, CMP_WORKFLOW).await {
        Ok(true) => return "\u{23F3} Siqish (GitHub Actions) ishga tushmoqda \u{2014} biroz kuting.".into(),
        Ok(false) => {}
        Err(e) => return e,
    }
    // Ikki joydan bir vaqtda (cron + bot) ikkita run ochilmasin — atomik belgi.
    let claim = turso_exec(env,
        "INSERT INTO app_config (cfg_key,cfg_value) VALUES ('cmp_kicked_at', ?)
         ON CONFLICT(cfg_key) DO UPDATE SET cfg_value=excluded.cfg_value
           WHERE CAST(app_config.cfg_value AS INTEGER) < ?
         RETURNING cfg_key",
        vec![TursoArg::text(&now.to_string()), TursoArg::int(now - 3 * 60 * 1000)]).await;
    if !claim.ok().and_then(|r| first_row(&r)).is_some() {
        return "\u{23F3} Siqish hozirgina ishga tushirilgan \u{2014} biroz kuting.".into();
    }
    match gh_api(env, Method::Post,
        &format!("/repos/{repo}/actions/workflows/{CMP_WORKFLOW}/dispatches"),
        Some(json!({"ref": "main"}))).await {
        Ok((204, _)) => "\u{25B6}\u{FE0F} Siqish boshlandi (GitHub Actions ishga tushirildi).".into(),
        Ok((code, v)) => format!("\u{26A0}\u{FE0F} Siqish ishga tushmadi (GitHub {code}): {}",
            html_escape(v["message"].as_str().unwrap_or(""))),
        Err(e) => format!("\u{26A0}\u{FE0F} Siqish ishga tushmadi: {}", html_escape(&e.to_string())),
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

/// "📋 Holat" — "Siqish" bo'limida: hozirgi ish va navbat (jonli holat siqilayotgan videoning o'z xabarida yangilanadi).
pub(crate) async fn status(env: &Env, chat: i64) {
    let rows = turso_exec(env,
        "SELECT id, name, state, lease_until FROM cmp_jobs ORDER BY queued_at ASC LIMIT 10", vec![]).await
        .map(|r| rows_of(&r)).unwrap_or_default();
    if rows.is_empty() {
        encbot_send(env, chat, "\u{1F5DC} <b>Siqish</b>\n\n\u{1F4A4} Hozir siqilayotgan yoki navbatdagi video yo'q.", Some(keyboard())).await;
        return;
    }
    let mut text = String::from("\u{1F5DC} <b>Siqish</b>\n\n");
    for r in &rows {
        text.push_str(&format!("{} <b>#{} {}</b>\n", state_icon(r), jint(r, "id"), html_escape(&short(r["name"].as_str().unwrap_or("")))));
    }
    text.push_str("\n\u{2699}\u{FE0F} \u{2014} siqilmoqda; jarayon shu video uchun yuborilgan holat xabarida ko'rinadi.");
    encbot_send(env, chat, &text, Some(keyboard())).await;
}

/// "📋 Siqish navbati": ishlar ro'yxati, har biriga inline tugmalar.
pub(crate) async fn queue_list(env: &Env, chat: i64) {
    let rows = turso_exec(env,
        "SELECT id, name, state, lease_until, error FROM cmp_jobs ORDER BY queued_at ASC LIMIT 20", vec![]).await
        .map(|r| rows_of(&r)).unwrap_or_default();
    if rows.is_empty() {
        encbot_send(env, chat, "\u{1F4CB} Siqish navbati bo'sh.", Some(keyboard())).await;
        return;
    }
    let mut kb: Vec<Vec<Value>> = Vec::new();
    let mut text = format!("\u{1F4CB} <b>Siqish navbati</b> ({} ta)\n\n", rows.len());
    for r in &rows {
        let id = jint(r, "id");
        text.push_str(&format!("{} <b>#{id} {}</b>{}\n", state_icon(r), html_escape(&short(r["name"].as_str().unwrap_or(""))),
            if r["state"].as_str() == Some("error") { format!(" \u{2014} {}", html_escape(r["error"].as_str().unwrap_or(""))) } else { String::new() }));
        let mut row = vec![json!({"text": format!("\u{1F5D1} #{id}"), "callback_data": format!("cd:{id}")})];
        if r["state"].as_str() == Some("error") {
            row.push(json!({"text": format!("\u{1F501} #{id}"), "callback_data": format!("cy:{id}")}));
        }
        kb.push(row);
    }
    encbot_send(env, chat, &text, Some(json!({"inline_keyboard": kb}))).await;
}

/// Inline tugmalar: cd (o'chirishni so'rash), cx (o'chirish), cn (bekor), cy (qayta urinish).
pub(crate) async fn on_callback(env: &Env, chat: i64, data: &str) -> bool {
    let mut it = data.split(':');
    let kind = it.next().unwrap_or("");
    if !matches!(kind, "cd" | "cx" | "cn" | "cy") {
        return false;
    }
    let id: i64 = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    match kind {
        "cd" => encbot_send(env, chat, &format!("\u{1F5D1} #{id} siqish ishi o'chirilsinmi?"), Some(json!({"inline_keyboard": [[
            {"text": "\u{2705} Ha, o'chirish", "callback_data": format!("cx:{id}")},
            {"text": "\u{274C} Yo'q", "callback_data": format!("cn:{id}")},
        ]]}))).await,
        "cx" => {
            let job = turso_exec(env, "DELETE FROM cmp_jobs WHERE id=? RETURNING ch_msg", vec![TursoArg::int(id)]).await
                .ok().and_then(|r| first_row(&r));
            match job {
                Some(j) => {
                    channel_cleanup(env, jint(&j, "ch_msg")).await;
                    encbot_send(env, chat, &format!("\u{1F5D1} #{id} o'chirildi."), Some(keyboard())).await;
                }
                None => encbot_send(env, chat, &format!("#{id} allaqachon yo'q."), Some(keyboard())).await,
            }
        }
        "cn" => encbot_send(env, chat, "O'chirilmadi.", Some(keyboard())).await,
        _ => {
            let _ = turso_exec(env,
                "UPDATE cmp_jobs SET state='queued', attempts=0, error='', runner='', lease_until=0 WHERE id=? AND state='error'",
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

// ── Actions (`compress.yml` -> `tool/compress/compress.py`) ─────────────

fn status_head(id: i64, name: &str, attempt: i64) -> String {
    format!("\u{1F5DC} <b>Siqish #{id}: {}</b>{}", html_escape(&short(name)),
        if attempt > 1 { format!(" (urinish {attempt})") } else { String::new() })
}

async fn own(env: &Env, b: &Value) -> Option<Value> {
    let runner = b["runner"].as_str().unwrap_or("");
    if runner.is_empty() {
        return None;
    }
    turso_exec(env, "SELECT * FROM cmp_jobs WHERE id=? AND runner=? AND state='running'",
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

    if path == "/api/cmp/claim" {
        let runner = b["runner"].as_str().unwrap_or("").trim().to_string();
        if runner.is_empty() || runner.len() > 64 {
            return json_resp(&json!({"error": "runner"}), 400);
        }
        let channel = tg_channel_id(env);
        for _ in 0..5 {
            let res = turso_exec(env,
                "SELECT * FROM cmp_jobs WHERE state='queued' OR (state='running' AND lease_until<=?)
                  ORDER BY queued_at ASC LIMIT 1", vec![TursoArg::int(now)]).await?;
            let Some(job) = first_row(&res) else {
                return ok(json!({"none": true}));
            };
            let id = jint(&job, "id");
            let name = job["name"].as_str().unwrap_or("").to_string();
            let chat = jint(&job, "chat");
            if jint(&job, "attempts") >= CMP_MAX_ATTEMPTS {
                let _ = turso_exec(env,
                    "UPDATE cmp_jobs SET state='error', runner='', lease_until=0, error=? WHERE id=?",
                    vec![TursoArg::text(&format!("{CMP_MAX_ATTEMPTS} marta urinildi")), TursoArg::int(id)]).await;
                encbot_send(env, chat, &format!(
                    "\u{274C} Siqish bajarilmadi: <b>#{id} {}</b> \u{2014} {CMP_MAX_ATTEMPTS} marta urinildi.\n\
                     \u{1F4CB} Siqish navbatidan qayta urinish yoki o'chirish mumkin.", html_escape(&short(&name))), None).await;
                continue;
            }
            let got = turso_exec(env,
                "UPDATE cmp_jobs SET state='running', runner=?, lease_until=?, attempts=attempts+1
                  WHERE id=? AND (state='queued' OR (state='running' AND lease_until<=?))
                  RETURNING attempts",
                vec![TursoArg::text(&runner), TursoArg::int(now + CMP_LEASE_MS), TursoArg::int(id), TursoArg::int(now)]).await?;
            let Some(got) = first_row(&got) else {
                continue;
            };
            let head = status_head(id, &name, jint(&got, "attempts"));
            let status_msg = encbot_api(env, "sendMessage", json!({
                "chat_id": chat, "parse_mode": "HTML", "text": format!("{head}\n\n\u{23F3} boshlanmoqda..."),
            })).await.ok().and_then(|v| v["message_id"].as_i64()).unwrap_or(0);
            return ok(json!({"channel": channel, "status_msg": status_msg, "job": {
                "id": id, "chat": chat, "name": name, "ch_msg": jint(&job, "ch_msg"),
                "attempt": jint(&got, "attempts"),
            }}));
        }
        return ok(json!({"none": true}));
    }

    // Jonli holat: holat xabarini tahrirlaydi. Bazaga tegmaydi.
    if path == "/api/cmp/progress" {
        let text: String = b["text"].as_str().unwrap_or("").chars().take(3000).collect();
        let (chat, msg) = (jint(&b, "chat"), jint(&b, "status_msg"));
        if !text.is_empty() && chat != 0 && msg > 0 {
            let head = status_head(jint(&b, "id"), b["name"].as_str().unwrap_or(""), jint(&b, "attempt"));
            edit_status(env, chat, msg, &format!("{head}\n\n{}", html_escape(&text))).await;
        }
        return ok_nostore(json!({"ok": true}));
    }

    // Workflow oxirida: navbatda yana ish bormi (bo'lsa yangi run ochiladi).
    if path == "/api/cmp/peek" {
        let n = turso_exec(env,
            "SELECT COUNT(*) AS n FROM cmp_jobs WHERE state='queued' OR (state='running' AND lease_until<=?)",
            vec![TursoArg::int(now)]).await.ok().and_then(|r| first_row(&r)).map(|r| jint(&r, "n")).unwrap_or(0);
        return ok_nostore(json!({"pending": n}));
    }

    // Yuborishdan oldin: ish hali shu run'nikimi (admin o'chirmaganmi).
    if path == "/api/cmp/check" {
        return match own(env, &b).await {
            Some(_) => ok(json!({"ok": true})),
            None => json_resp(&json!({"error": "lost"}), 409),
        };
    }

    if path == "/api/cmp/finish" {
        let Some(job) = own(env, &b).await else {
            return json_resp(&json!({"error": "lost"}), 409);
        };
        let id = jint(&job, "id");
        let chat = jint(&job, "chat");
        let name = job["name"].as_str().unwrap_or("").to_string();
        let status_msg = jint(&b, "status_msg");
        let say = |line: String| format!("{}\n\n{line}", status_head(id, &name, jint(&job, "attempts")));
        if b["ok"].as_bool() == Some(true) {
            if status_msg > 0 {
                edit_status(env, chat, status_msg, &say(format!(
                    "\u{2705} Tayyor, GIF qilib Saqlangan xabarlarga yuborildi\n\u{1F4E6} {:.1} MB \u{2192} {:.1} MB",
                    jint(&b, "src_size") as f64 / 1048576.0, jint(&b, "size") as f64 / 1048576.0))).await;
            }
            channel_cleanup(env, jint(&job, "ch_msg")).await;
            turso_exec(env, "DELETE FROM cmp_jobs WHERE id=?", vec![TursoArg::int(id)]).await?;
            return ok(json!({"ok": true}));
        }
        let err = b["error"].as_str().unwrap_or("").chars().take(300).collect::<String>();
        if b["cancelled"].as_bool() == Some(true) {
            if status_msg > 0 {
                edit_status(env, chat, status_msg, &say("\u{23F8} Run to'xtatildi \u{2014} ish navbatga qaytdi".to_string())).await;
            }
            turso_exec(env,
                "UPDATE cmp_jobs SET state='queued', runner='', lease_until=0, attempts=MAX(attempts-1,0) WHERE id=?",
                vec![TursoArg::int(id)]).await?;
            return ok(json!({"ok": true}));
        }
        let fatal = b["fatal"].as_bool() == Some(true) || jint(&job, "attempts") >= CMP_MAX_ATTEMPTS;
        if status_msg > 0 {
            edit_status(env, chat, status_msg, &say(format!(
                "{} xato: {}", if fatal { "\u{274C}" } else { "\u{26A0}\u{FE0F}" }, html_escape(&err)))).await;
        }
        turso_exec(env, "UPDATE cmp_jobs SET state=?, runner='', lease_until=0, error=? WHERE id=?",
            vec![TursoArg::text(if fatal { "error" } else { "queued" }), TursoArg::text(&err), TursoArg::int(id)]).await?;
        encbot_send(env, chat, &format!(
            "{} Siqish: <b>#{id} {}</b> \u{2014} <code>{}</code>\n{}",
            if fatal { "\u{274C}" } else { "\u{26A0}\u{FE0F}" }, html_escape(&short(&name)), html_escape(&err),
            if fatal { "To'xtadi. \u{1F4CB} Siqish navbatidan qayta urinish yoki o'chirish mumkin." }
            else { "Keyinroq yana urinib ko'riladi." }), None).await;
        return ok(json!({"ok": true}));
    }

    err404("topilmadi")
}
