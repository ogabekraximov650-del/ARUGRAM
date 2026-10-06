// ══════════════════════════════════════════════════════════════
//  POST KODLASH — kodlash botining ikkinchi bo'limi
// ══════════════════════════════════════════════════════════════
//
// TALAB (foydalanuvchi): kodlash botida ikki bo'lim — "Ilova uchun"
// (eski oqim, `encbot_*`) va "Post kodlash" (`anime` repodagi Encode
// tizimi). Botga rasm va video yuboriladi, izohida (caption) post nomi;
// Actions videoni H.264 bilan kodlaydi (boshida 3 soniya rasm, burchakda
// logotip) va logotip fayli nomidagi ID'ga (`tool/post/<ID>_logo.png`)
// Telegram'da ochiladigan VIDEO qilib, tagida aynan shu nom bilan yuboradi.
// Navbatdagi postlar tugma bo'lib ko'rinadi; bosilganda rasm va video,
// tagida tahrirlash/o'chirish (inline) tugmalari.
//
// OQIM:
//   * rasm va video admin chatidan yopiq kanalga `copyMessage` bilan
//     ko'chadi (fayl worker'dan o'tmaydi, hajm chegarasi yo'q);
//   * `post_jobs` ga bitta yozuv; `kick` ALOHIDA workflow'ni (`post.yml`,
//     `avtoencode` repoda, shablon `tool/post/post.workflow.yml`) ishga
//     tushiradi — avto-kodlash (H.265) bilan bog'liq emas (foydalanuvchi
//     talabi: "ikkalasi alohida narsalar"). Cron ham har 10 daqiqada;
//   * `tool/post/post.py` navbat bo'shaguncha `/api/post/claim` qiladi;
//     jarayonni botdagi bitta xabarda jonli ko'rsatadi (`/api/post/progress`
//     — bazaga yozmaydi, faqat xabarni tahrirlaydi);
//   * `/api/post/finish` — tayyor bo'lsa yozuv va kanal postlari o'chadi.
//
// Suhbat holati `app_config` da: `encbot_mode` (`post` yoki bo'sh),
// `post_draft` (yig'ilayotgan post), `post_edit` (`<id>:n|p|v`).

use super::*;

pub(crate) const DDL: [&str; 2] = [
    // photo_msg / video_msg — yopiq kanaldagi postlar (kodlash boti ko'chirgan).
    // state: queued | running | error.
    "CREATE TABLE IF NOT EXISTS post_jobs (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL,
        photo_msg INTEGER NOT NULL,
        video_msg INTEGER NOT NULL,
        video_size INTEGER DEFAULT 0,
        queued_at INTEGER NOT NULL,
        state TEXT DEFAULT 'queued',
        runner TEXT DEFAULT '',
        lease_until INTEGER DEFAULT 0,
        attempts INTEGER DEFAULT 0,
        error TEXT DEFAULT ''
    )",
    "CREATE INDEX IF NOT EXISTS idx_post_q ON post_jobs(state, queued_at)",
];

/// Bitta post kodlanishi uchun ijara (run o'lsa shundan keyin qayta olinadi).
const POST_LEASE_MS: i64 = 180 * 60 * 1000;
const POST_MAX_ATTEMPTS: i64 = 3;

pub(crate) const BTN_APP: &str = "\u{1F4F1} Ilova uchun";
pub(crate) const BTN_POST: &str = "\u{1F3AC} Post kodlash";
pub(crate) const BTN_HOME: &str = "\u{1F3E0} Bosh menyu";
const BTN_QUEUE: &str = "\u{1F4CB} Navbatdagi postlar";

const POST_WORKFLOW: &str = "post.yml";

/// Navbatda post bo'lsa-yu, uni hech kim ishlamayotgan bo'lsa — `post.yml`
/// ni ishga tushiradi. Natija matni (bot xabari uchun).
pub(crate) async fn kick(env: &Env) -> String {
    let now = now_ms();
    let res = turso_exec(env,
        "SELECT
           (SELECT COUNT(*) FROM post_jobs WHERE state='queued' OR (state='running' AND lease_until<=?)) AS pending,
           (SELECT COUNT(*) FROM post_jobs WHERE state='running' AND lease_until>?) AS active",
        vec![TursoArg::int(now), TursoArg::int(now)]).await;
    let Some(r) = res.ok().and_then(|r| first_row(&r)) else {
        return "\u{26A0}\u{FE0F} Post navbatini o'qib bo'lmadi.".into();
    };
    if jint(&r, "active") > 0 {
        return "\u{2699}\u{FE0F} Post kodlash ishlayapti \u{2014} navbatdagilar ketma-ket kodlanadi.".into();
    }
    if jint(&r, "pending") == 0 {
        return String::new();
    }
    let repo = tg_secret(env, "GH_REPO");
    if !repo.contains('/') {
        return "\u{26A0}\u{FE0F} GH_REPO o'rnatilmagan \u{2014} Actions ishga tushirilmadi.".into();
    }
    match gh_workflow_busy(env, &repo, POST_WORKFLOW).await {
        Ok(true) => return "\u{23F3} Post kodlash (GitHub Actions) ishga tushmoqda \u{2014} biroz kuting.".into(),
        Ok(false) => {}
        Err(e) => return e,
    }
    // Ikki joydan bir vaqtda (cron + bot) ikkita run ochilmasin — atomik belgi.
    let claim = turso_exec(env,
        "INSERT INTO app_config (cfg_key,cfg_value) VALUES ('post_kicked_at', ?)
         ON CONFLICT(cfg_key) DO UPDATE SET cfg_value=excluded.cfg_value
           WHERE CAST(app_config.cfg_value AS INTEGER) < ?
         RETURNING cfg_key",
        vec![TursoArg::text(&now.to_string()), TursoArg::int(now - 3 * 60 * 1000)]).await;
    if !claim.ok().and_then(|r| first_row(&r)).is_some() {
        return "\u{23F3} Post kodlash hozirgina ishga tushirilgan \u{2014} biroz kuting.".into();
    }
    match gh_api(env, Method::Post,
        &format!("/repos/{repo}/actions/workflows/{POST_WORKFLOW}/dispatches"),
        Some(json!({"ref": "main"}))).await {
        Ok((204, _)) => "\u{25B6}\u{FE0F} Post kodlash boshlandi (GitHub Actions ishga tushirildi).".into(),
        Ok((code, v)) => format!("\u{26A0}\u{FE0F} Post kodlash ishga tushmadi (GitHub {code}): {}",
            html_escape(v["message"].as_str().unwrap_or(""))),
        Err(e) => format!("\u{26A0}\u{FE0F} Post kodlash ishga tushmadi: {}", html_escape(&e.to_string())),
    }
}

// ── Bot: menyular ────────────────────────────────────────────

pub(crate) async fn main_menu(env: &Env, chat: i64) {
    config_put(env, "post_edit", "").await;
    encbot_send(env, chat,
        "\u{1F3E0} <b>Bosh menyu</b>\n\n\
         \u{1F4F1} <b>Ilova uchun</b> \u{2014} ilovadagi anime bo'limiga qism qo'shish.\n\
         \u{1F3AC} <b>Post kodlash</b> \u{2014} rasm + video + nom: kodlab, tayyor videoni yuboradi.",
        Some(encbot_keyboard(vec![
            vec![BTN_APP.to_string(), BTN_POST.to_string()],
            vec![ENCBOT_BTN_STATUS.to_string()],
        ]))).await;
}

fn post_keyboard() -> Value {
    encbot_keyboard(vec![vec![BTN_QUEUE.to_string()], vec![BTN_HOME.to_string()]])
}

pub(crate) async fn post_menu(env: &Env, chat: i64) {
    config_put(env, "encbot_mode", "post").await;
    config_put(env, "post_edit", "").await;
    let n = turso_exec(env, "SELECT COUNT(*) AS n FROM post_jobs", vec![]).await
        .ok().and_then(|r| first_row(&r)).map(|r| jint(&r, "n")).unwrap_or(0);
    encbot_send(env, chat, &format!(
        "\u{1F3AC} <b>Post kodlash</b>\n\n\
         Rasm va videoni yuboring, izohiga (caption) post nomini yozing.\n\
         Bittasiga yozsangiz ham bo'ladi, albom qilib yuborsangiz ham bo'ladi.\n\n\
         Video H.264 bilan kodlanadi (boshida rasm, burchakda logotip) va \
         tagida shu nom bilan yuboriladi.\n\n\u{1F4CB} Navbatda: {n} ta post."),
        Some(post_keyboard())).await;
}

// ── Bot: xabarlar ────────────────────────────────────────────

#[derive(Default, Serialize, Deserialize)]
struct Draft {
    #[serde(default)]
    name: String,
    /// Admin chatidagi xabar raqamlari (kanalga navbatga qo'yilganda ko'chadi).
    #[serde(default)]
    photo: i64,
    #[serde(default)]
    video: i64,
    #[serde(default)]
    size: i64,
}

async fn draft_get(env: &Env) -> Draft {
    config_get(env, "post_draft").await
        .and_then(|v| serde_json::from_str(&v).ok())
        .unwrap_or_default()
}

async fn draft_put(env: &Env, d: &Draft) {
    config_put(env, "post_draft", &serde_json::to_string(d).unwrap_or_default()).await;
}

/// Xabardagi rasm (bor bo'lsa) va video (bor bo'lsa, hajmi bilan).
fn media(msg: &Value) -> (bool, Option<i64>) {
    let mut photo = msg["photo"].is_array();
    let mut video = None;
    if msg["video"].is_object() {
        video = Some(msg["video"]["file_size"].as_i64().unwrap_or(0));
    }
    let doc = &msg["document"];
    if doc.is_object() {
        let mime = doc["mime_type"].as_str().unwrap_or("").to_ascii_lowercase();
        let fname = doc["file_name"].as_str().unwrap_or("").to_ascii_lowercase();
        let ext = fname.rsplit_once('.').map(|(_, e)| e.to_string()).unwrap_or_default();
        if mime.starts_with("video/")
            || ["mp4", "mkv", "mov", "avi", "webm", "m4v", "ts", "flv", "wmv"].contains(&ext.as_str()) {
            video = Some(doc["file_size"].as_i64().unwrap_or(0));
        } else if mime.starts_with("image/") || ["jpg", "jpeg", "png", "webp", "bmp"].contains(&ext.as_str()) {
            photo = true;
        }
    }
    (photo, video)
}

/// Telegram izohi 1024 belgidan oshmasin.
fn clip_name(s: &str) -> String {
    s.trim().chars().take(1000).collect()
}

/// Admin chatidagi xabarni yopiq kanalga ko'chiradi — kanal xabari raqami.
async fn to_channel(env: &Env, chat: i64, msg_id: i64, caption: &str) -> std::result::Result<i64, String> {
    let channel = tg_channel_id(env);
    if channel == 0 {
        return Err("TG_CHANNEL_ID sozlanmagan".into());
    }
    match encbot_api(env, "copyMessage", json!({
        "chat_id": channel, "from_chat_id": chat, "message_id": msg_id,
        "caption": caption, "disable_notification": true,
    })).await {
        Ok(v) => v["message_id"].as_i64().filter(|m| *m > 0).ok_or_else(|| "kanalga ko'chmadi".to_string()),
        Err(e) => Err(format!("{e} (bot kanalda admin ekanini va asl xabar o'chmaganini tekshiring)")),
    }
}

async fn channel_delete(env: &Env, msg_id: i64) {
    let channel = tg_channel_id(env);
    if channel != 0 && msg_id > 0 {
        let _ = encbot_api(env, "deleteMessage", json!({"chat_id": channel, "message_id": msg_id})).await;
    }
}

fn short(name: &str) -> String {
    let line = name.lines().next().unwrap_or("").trim();
    let mut s: String = line.chars().take(40).collect();
    if line.chars().count() > 40 {
        s.push('\u{2026}');
    }
    if s.is_empty() { "(nomsiz)".into() } else { s }
}

/// Post rejimidagi xabar. `true` — ishlandi.
pub(crate) async fn on_message(env: &Env, msg: &Value, chat: i64) -> bool {
    let text = msg["text"].as_str().unwrap_or("").trim().to_string();
    if text == BTN_QUEUE {
        queue_list(env, chat).await;
        return true;
    }
    let msg_id = msg["message_id"].as_i64().unwrap_or(0);
    let caption = msg["caption"].as_str().unwrap_or("").trim().to_string();
    let (photo, video) = media(msg);

    // Tahrirlash kutilmoqda.
    let edit = config_get(env, "post_edit").await.unwrap_or_default();
    if let Some((id, field)) = edit.split_once(':') {
        let id: i64 = id.parse().unwrap_or(0);
        let Some(job) = job_get(env, id).await else {
            config_put(env, "post_edit", "").await;
            encbot_send(env, chat, "\u{274C} Post topilmadi (o'chirilgan yoki tayyor bo'lgan).", Some(post_keyboard())).await;
            return true;
        };
        if job["state"].as_str() == Some("running") {
            config_put(env, "post_edit", "").await;
            encbot_send(env, chat, "\u{2699}\u{FE0F} Bu post hozir kodlanmoqda \u{2014} uni tahrirlab bo'lmaydi.", Some(post_keyboard())).await;
            return true;
        }
        let res = match field {
            "n" if !text.is_empty() => {
                turso_exec(env, "UPDATE post_jobs SET name=? WHERE id=?",
                    vec![TursoArg::text(&clip_name(&text)), TursoArg::int(id)]).await
                    .map(|_| ()).map_err(|e| e.to_string())
            }
            "p" if photo => match to_channel(env, chat, msg_id, &format!("post_photo_{id}")).await {
                Ok(m) => {
                    let r = turso_exec(env, "UPDATE post_jobs SET photo_msg=? WHERE id=?",
                        vec![TursoArg::int(m), TursoArg::int(id)]).await;
                    if r.is_ok() { channel_delete(env, jint(&job, "photo_msg")).await; }
                    r.map(|_| ()).map_err(|e| e.to_string())
                }
                Err(e) => Err(e),
            },
            "v" if video.is_some() => match to_channel(env, chat, msg_id, &format!("post_video_{id}")).await {
                Ok(m) => {
                    let r = turso_exec(env, "UPDATE post_jobs SET video_msg=?, video_size=? WHERE id=?",
                        vec![TursoArg::int(m), TursoArg::int(video.unwrap_or(0)), TursoArg::int(id)]).await;
                    if r.is_ok() { channel_delete(env, jint(&job, "video_msg")).await; }
                    r.map(|_| ()).map_err(|e| e.to_string())
                }
                Err(e) => Err(e),
            },
            _ => {
                let want = match field { "n" => "yangi nomni (matn)", "p" => "yangi rasmni", _ => "yangi videoni" };
                encbot_send(env, chat, &format!(
                    "Hozir #{id} post uchun {want} kutyapman.\nBekor qilish: {BTN_HOME}"), None).await;
                return true;
            }
        };
        config_put(env, "post_edit", "").await;
        match res {
            Ok(()) => {
                // Xato bilan to'xtagan post tahrirlansa — yana navbatga.
                let _ = turso_exec(env,
                    "UPDATE post_jobs SET state='queued', attempts=0, error='' WHERE id=? AND state='error'",
                    vec![TursoArg::int(id)]).await;
                encbot_send(env, chat, "\u{2705} O'zgartirildi.", Some(post_keyboard())).await;
                show(env, chat, id).await;
            }
            Err(e) => encbot_send(env, chat, &format!("\u{274C} O'zgartirib bo'lmadi: <code>{}</code>",
                html_escape(&e)), Some(post_keyboard())).await,
        }
        return true;
    }

    // Yangi post yig'ilmoqda.
    let mut d = draft_get(env).await;
    let label = if !caption.is_empty() { caption.clone() } else { text.clone() };
    if !label.is_empty() {
        d.name = clip_name(&label);
    }
    if photo {
        d.photo = msg_id;
    }
    if let Some(size) = video {
        d.video = msg_id;
        d.size = size;
    }
    if label.is_empty() && !photo && video.is_none() {
        encbot_send(env, chat, "Rasm, video yoki post nomini yuboring.", Some(post_keyboard())).await;
        return true;
    }
    if d.name.is_empty() || d.photo == 0 || d.video == 0 {
        draft_put(env, &d).await;
        // Albomning birinchi qismi — qolgani hozir keladi, ortiqcha xabar yo'q.
        if msg["media_group_id"].is_string() && !(d.photo == 0 && d.video == 0) {
            return true;
        }
        let mut need = Vec::new();
        if d.name.is_empty() { need.push("\u{270F}\u{FE0F} post nomi (matn yoki izoh)"); }
        if d.photo == 0 { need.push("\u{1F5BC} rasm"); }
        if d.video == 0 { need.push("\u{1F3AC} video"); }
        let have = if d.name.is_empty() { String::new() } else {
            format!("\u{270F}\u{FE0F} Nom: <b>{}</b>\n", html_escape(&short(&d.name)))
        };
        encbot_send(env, chat, &format!("{have}Yana kerak: {}", need.join(", ")), Some(post_keyboard())).await;
        return true;
    }

    // Hammasi bor — kanalga va navbatga.
    // TAKROR HIMOYASI: Telegram javobni kutib ulgurmasa xabarni QAYTA yuboradi
    // (navbatda bitta post bir necha marta paydo bo'lardi). Oxirgi navbatga
    // qo'yilgan video xabari eslab qolinadi — o'sha xabar qayta kelsa e'tiborsiz.
    let src_key = format!("{}:{}", d.photo, d.video);
    if config_get(env, "post_last_src").await.as_deref() == Some(src_key.as_str()) {
        draft_put(env, &Draft::default()).await;
        return true;
    }
    config_put(env, "post_last_src", &src_key).await;
    let now = now_ms();
    let photo_ch = match to_channel(env, chat, d.photo, &format!("post_photo_{now}")).await {
        Ok(m) => m,
        Err(e) => {
            d.photo = 0;
            draft_put(env, &d).await;
            encbot_send(env, chat, &format!("\u{274C} Rasmni kanalga ko'chirib bo'lmadi: <code>{}</code>\nRasmni qayta yuboring.",
                html_escape(&e)), Some(post_keyboard())).await;
            return true;
        }
    };
    let video_ch = match to_channel(env, chat, d.video, &format!("post_video_{now}")).await {
        Ok(m) => m,
        Err(e) => {
            channel_delete(env, photo_ch).await;
            d.video = 0;
            draft_put(env, &d).await;
            encbot_send(env, chat, &format!("\u{274C} Videoni kanalga ko'chirib bo'lmadi: <code>{}</code>\nVideoni qayta yuboring.",
                html_escape(&e)), Some(post_keyboard())).await;
            return true;
        }
    };
    let ins = turso_exec(env,
        "INSERT INTO post_jobs (name, photo_msg, video_msg, video_size, queued_at) VALUES (?,?,?,?,?) RETURNING id",
        vec![TursoArg::text(&d.name), TursoArg::int(photo_ch), TursoArg::int(video_ch),
             TursoArg::int(d.size.max(0)), TursoArg::int(now)]).await;
    let Some(id) = ins.ok().and_then(|r| first_row(&r)).map(|r| jint(&r, "id")) else {
        channel_delete(env, photo_ch).await;
        channel_delete(env, video_ch).await;
        encbot_send(env, chat, "\u{274C} Navbatga qo'yib bo'lmadi (baza xatosi). Qayta yuboring.", Some(post_keyboard())).await;
        return true;
    };
    draft_put(env, &Draft::default()).await;
    let n = turso_exec(env, "SELECT COUNT(*) AS n FROM post_jobs WHERE state IN ('queued','running')", vec![]).await
        .ok().and_then(|r| first_row(&r)).map(|r| jint(&r, "n")).unwrap_or(1);
    let kick = kick(env).await;
    encbot_send(env, chat, &format!(
        "\u{2705} Navbatga qo'yildi: <b>#{id} {}</b>\n\u{1F4E6} Video: {:.1} MB\n\u{1F4CB} Navbatdagi postlar: {n} ta\n\n{kick}",
        html_escape(&short(&d.name)), d.size as f64 / 1048576.0), Some(post_keyboard())).await;
    true
}

async fn job_get(env: &Env, id: i64) -> Option<Value> {
    turso_exec(env, "SELECT * FROM post_jobs WHERE id=?", vec![TursoArg::int(id)]).await
        .ok().and_then(|r| first_row(&r))
}

/// Jonli holat xabarining sarlavhasi.
fn status_head(id: i64, name: &str, attempt: i64) -> String {
    format!("\u{1F3AC} <b>Post #{id}: {}</b>{}", html_escape(&short(name)),
        if attempt > 1 { format!(" (urinish {attempt})") } else { String::new() })
}

fn state_icon(job: &Value) -> &'static str {
    match job["state"].as_str().unwrap_or("") {
        "running" if jint(job, "lease_until") > now_ms() => "\u{2699}\u{FE0F}",
        "error" => "\u{274C}",
        _ => "\u{23F3}",
    }
}

/// Navbat — har post alohida tugma ("⏳ #12 nomi").
async fn queue_list(env: &Env, chat: i64) {
    let rows = turso_exec(env,
        "SELECT id, name, state, lease_until FROM post_jobs ORDER BY queued_at ASC LIMIT 40", vec![]).await
        .map(|r| rows_of(&r)).unwrap_or_default();
    if rows.is_empty() {
        encbot_send(env, chat, "\u{1F4CB} Navbat bo'sh.", Some(post_keyboard())).await;
        return;
    }
    let mut kb: Vec<Vec<String>> = rows.iter()
        .map(|r| vec![format!("{} #{} {}", state_icon(r), jint(r, "id"), short(r["name"].as_str().unwrap_or("")))])
        .collect();
    kb.push(vec![BTN_HOME.to_string()]);
    encbot_send(env, chat, &format!(
        "\u{1F4CB} Navbatda {} ta post.\n\u{23F3} kutmoqda \u{00B7} \u{2699}\u{FE0F} kodlanmoqda \u{00B7} \u{274C} xato\n\n\
         Ko'rish, tahrirlash yoki o'chirish uchun postni bosing.", rows.len()),
        Some(encbot_keyboard(kb))).await;
}

/// Navbat tugmasi matnidan post raqami: "⏳ #12 nomi" -> 12.
pub(crate) fn button_id(text: &str) -> Option<i64> {
    let rest = text.strip_prefix("\u{23F3}")
        .or_else(|| text.strip_prefix("\u{2699}\u{FE0F}"))
        .or_else(|| text.strip_prefix("\u{274C}"))?;
    rest.trim().strip_prefix('#')?.split_whitespace().next()?.parse().ok()
}

/// Post: rasm, video va tagida inline tugmalar.
pub(crate) async fn show(env: &Env, chat: i64, id: i64) {
    let Some(job) = job_get(env, id).await else {
        encbot_send(env, chat, "\u{274C} Post topilmadi (o'chirilgan yoki tayyor bo'lgan).", Some(post_keyboard())).await;
        return;
    };
    let channel = tg_channel_id(env);
    let name = job["name"].as_str().unwrap_or("").to_string();
    let running = state_icon(&job) == "\u{2699}\u{FE0F}";
    let state = match state_icon(&job) {
        "\u{2699}\u{FE0F}" => "\u{2699}\u{FE0F} hozir kodlanmoqda".to_string(),
        "\u{274C}" => format!("\u{274C} xato: {}", job["error"].as_str().unwrap_or("")),
        _ => "\u{23F3} navbatda".to_string(),
    };
    let _ = encbot_api(env, "copyMessage", json!({
        "chat_id": chat, "from_chat_id": channel, "message_id": jint(&job, "photo_msg"),
        "caption": format!("#{id} \u{2014} rasm"),
    })).await;
    let mut kb = Vec::new();
    if running {
        kb.push(vec![json!({"text": "\u{1F5D1} O'chirish (to'xtatish)", "callback_data": format!("pd:{id}")})]);
    } else {
        kb.push(vec![json!({"text": "\u{270F}\u{FE0F} Nomni tahrirlash", "callback_data": format!("pe:{id}:n")})]);
        kb.push(vec![
            json!({"text": "\u{1F5BC} Rasmni almashtirish", "callback_data": format!("pe:{id}:p")}),
            json!({"text": "\u{1F3AC} Videoni almashtirish", "callback_data": format!("pe:{id}:v")}),
        ]);
        if job["state"].as_str() == Some("error") {
            kb.push(vec![json!({"text": "\u{1F501} Qayta urinish", "callback_data": format!("pr:{id}")})]);
        }
        kb.push(vec![json!({"text": "\u{1F5D1} O'chirish", "callback_data": format!("pd:{id}")})]);
    }
    let caption: String = format!("{name}\n\n#{id} \u{00B7} {state}").chars().take(1020).collect();
    let r = encbot_api(env, "copyMessage", json!({
        "chat_id": chat, "from_chat_id": channel, "message_id": jint(&job, "video_msg"),
        "caption": caption, "reply_markup": {"inline_keyboard": kb},
    })).await;
    if r.is_err() {
        encbot_send(env, chat, &format!("\u{26A0}\u{FE0F} Videoni ko'rsatib bo'lmadi (kanalda topilmadi).\n\n{}",
            html_escape(&caption)), Some(json!({"inline_keyboard": kb}))).await;
    }
}

/// Inline tugmalar: pe (tahrirlash), pd (o'chirishni so'rash),
/// px (o'chirish), pn (o'chirmaslik), pr (qayta urinish). `true` — ishlandi.
pub(crate) async fn on_callback(env: &Env, chat: i64, data: &str) -> bool {
    let mut it = data.split(':');
    let kind = it.next().unwrap_or("");
    let id: i64 = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    let field = it.next().unwrap_or("");
    match kind {
        "pe" => {
            let Some(job) = job_get(env, id).await else {
                encbot_send(env, chat, "\u{274C} Post topilmadi.", Some(post_keyboard())).await;
                return true;
            };
            if state_icon(&job) == "\u{2699}\u{FE0F}" {
                encbot_send(env, chat, "\u{2699}\u{FE0F} Bu post hozir kodlanmoqda \u{2014} uni tahrirlab bo'lmaydi.", None).await;
                return true;
            }
            let (f, ask) = match field {
                "n" => ("n", "\u{270F}\u{FE0F} Yangi post nomini yozing:"),
                "p" => ("p", "\u{1F5BC} Yangi rasmni yuboring:"),
                _ => ("v", "\u{1F3AC} Yangi videoni yuboring:"),
            };
            config_put(env, "encbot_mode", "post").await;
            config_put(env, "post_edit", &format!("{id}:{f}")).await;
            encbot_send(env, chat, &format!("#{id} \u{2014} {ask}\n\nBekor qilish: {BTN_HOME}"), None).await;
        }
        "pd" => {
            encbot_send(env, chat, &format!("\u{1F5D1} #{id} post o'chirilsinmi?"), Some(json!({"inline_keyboard": [[
                {"text": "\u{2705} Ha, o'chirish", "callback_data": format!("px:{id}")},
                {"text": "\u{274C} Yo'q", "callback_data": format!("pn:{id}")},
            ]]}))).await;
        }
        "px" => {
            let job = turso_exec(env, "DELETE FROM post_jobs WHERE id=? RETURNING photo_msg, video_msg",
                vec![TursoArg::int(id)]).await.ok().and_then(|r| first_row(&r));
            match job {
                Some(j) => {
                    channel_delete(env, jint(&j, "photo_msg")).await;
                    channel_delete(env, jint(&j, "video_msg")).await;
                    encbot_send(env, chat, &format!("\u{1F5D1} #{id} o'chirildi."), None).await;
                }
                None => encbot_send(env, chat, &format!("#{id} allaqachon yo'q."), None).await,
            }
            queue_list(env, chat).await;
        }
        "pn" => encbot_send(env, chat, "O'chirilmadi.", None).await,
        "pr" => {
            let _ = turso_exec(env,
                "UPDATE post_jobs SET state='queued', attempts=0, error='', runner='', lease_until=0 WHERE id=? AND state='error'",
                vec![TursoArg::int(id)]).await;
            let kick = kick(env).await;
            encbot_send(env, chat, &format!("\u{1F501} #{id} yana navbatga qo'yildi.\n\n{kick}"), None).await;
        }
        _ => return false,
    }
    true
}

// ── Actions (`post.yml` -> `tool/post/post.py`) ──────────────

async fn own(env: &Env, b: &Value) -> Option<Value> {
    let runner = b["runner"].as_str().unwrap_or("");
    if runner.is_empty() {
        return None;
    }
    turso_exec(env, "SELECT * FROM post_jobs WHERE id=? AND runner=? AND state='running'",
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

    if path == "/api/post/claim" {
        let runner = b["runner"].as_str().unwrap_or("").trim().to_string();
        if runner.is_empty() || runner.len() > 64 {
            return json_resp(&json!({"error": "runner"}), 400);
        }
        let channel = tg_channel_id(env);
        for _ in 0..5 {
            let res = turso_exec(env,
                "SELECT * FROM post_jobs WHERE state='queued' OR (state='running' AND lease_until<=?)
                  ORDER BY queued_at ASC LIMIT 1", vec![TursoArg::int(now)]).await?;
            let Some(job) = first_row(&res) else {
                return ok(json!({"none": true}));
            };
            let id = jint(&job, "id");
            let name = job["name"].as_str().unwrap_or("").to_string();
            if jint(&job, "attempts") >= POST_MAX_ATTEMPTS {
                let _ = turso_exec(env,
                    "UPDATE post_jobs SET state='error', runner='', lease_until=0, error=? WHERE id=?",
                    vec![TursoArg::text(&format!("{POST_MAX_ATTEMPTS} marta urinildi")), TursoArg::int(id)]).await;
                encbot_send(env, ADMIN_TELEGRAM_ID, &format!(
                    "\u{274C} Post kodlanmadi: <b>#{id} {}</b> \u{2014} {POST_MAX_ATTEMPTS} marta urinildi.\n\
                     \u{1F4CB} Navbatdagi postlar ichidan qayta urinish yoki o'chirish mumkin.",
                    html_escape(&short(&name))), None).await;
                continue;
            }
            let got = turso_exec(env,
                "UPDATE post_jobs SET state='running', runner=?, lease_until=?, attempts=attempts+1
                  WHERE id=? AND (state='queued' OR (state='running' AND lease_until<=?))
                  RETURNING attempts",
                vec![TursoArg::text(&runner), TursoArg::int(now + POST_LEASE_MS), TursoArg::int(id), TursoArg::int(now)]).await?;
            let Some(got) = first_row(&got) else {
                continue;
            };
            // JONLI HOLAT XABARI: Actions uni `/api/post/progress` bilan tahrirlab
            // turadi (ilovadagi kodlash holati kabi). Raqami faqat runner'da —
            // bazaga yozilmaydi.
            let head = status_head(id, &name, jint(&got, "attempts"));
            let status_msg = encbot_api(env, "sendMessage", json!({
                "chat_id": ADMIN_TELEGRAM_ID, "parse_mode": "HTML",
                "text": format!("{head}\n\n\u{23F3} boshlanmoqda..."),
            })).await.ok().and_then(|v| v["message_id"].as_i64()).unwrap_or(0);
            return ok(json!({"channel": channel, "status_msg": status_msg, "job": {
                "id": id, "name": name, "photo_msg": jint(&job, "photo_msg"),
                "video_msg": jint(&job, "video_msg"), "attempt": jint(&got, "attempts"),
            }}));
        }
        return ok(json!({"none": true}));
    }

    // Jonli holat: holat xabarini tahrirlaydi. Bazaga tegmaydi (har ~10 s keladi).
    if path == "/api/post/progress" {
        let msg = jint(&b, "status_msg");
        let text: String = b["text"].as_str().unwrap_or("").chars().take(3000).collect();
        if msg > 0 && !text.is_empty() {
            let head = status_head(jint(&b, "id"), b["name"].as_str().unwrap_or(""), jint(&b, "attempt"));
            let _ = encbot_api(env, "editMessageText", json!({
                "chat_id": ADMIN_TELEGRAM_ID, "message_id": msg, "parse_mode": "HTML",
                "text": format!("{head}\n\n{}", html_escape(&text)),
            })).await;
        }
        return ok_nostore(json!({"ok": true}));
    }

    // Yuborishdan oldin: post hali shu run'nikimi (admin o'chirmaganmi).
    if path == "/api/post/check" {
        return match own(env, &b).await {
            Some(_) => ok(json!({"ok": true})),
            None => json_resp(&json!({"error": "lost"}), 409),
        };
    }

    if path == "/api/post/finish" {
        let Some(job) = own(env, &b).await else {
            return json_resp(&json!({"error": "lost"}), 409);
        };
        let id = jint(&job, "id");
        let name = job["name"].as_str().unwrap_or("").to_string();
        let status_msg = jint(&b, "status_msg");
        let final_status = |line: String| {
            let head = status_head(id, &name, jint(&job, "attempts"));
            json!({"chat_id": ADMIN_TELEGRAM_ID, "message_id": status_msg, "parse_mode": "HTML",
                   "text": format!("{head}\n\n{line}")})
        };
        if b["ok"].as_bool() == Some(true) {
            if status_msg > 0 {
                let _ = encbot_api(env, "editMessageText", final_status(format!(
                    "\u{2705} Tayyor va {} ga yuborildi ({:.1} MB)",
                    html_escape(b["to"].as_str().unwrap_or("")), jint(&b, "size") as f64 / 1048576.0))).await;
            }
            turso_exec(env, "DELETE FROM post_jobs WHERE id=?", vec![TursoArg::int(id)]).await?;
            channel_delete(env, jint(&job, "photo_msg")).await;
            channel_delete(env, jint(&job, "video_msg")).await;
            let to = b["to"].as_str().unwrap_or("").to_string();
            encbot_send(env, ADMIN_TELEGRAM_ID, &format!(
                "\u{2705} Post tayyor: <b>#{id} {}</b>\n\u{1F4E6} {:.1} MB \u{2192} {:.1} MB\n\u{1F4E8} {} ga yuborildi.",
                html_escape(&short(&name)), jint(&job, "video_size") as f64 / 1048576.0,
                jint(&b, "size") as f64 / 1048576.0, html_escape(&to)), None).await;
            return ok(json!({"ok": true}));
        }
        let err = b["error"].as_str().unwrap_or("").chars().take(300).collect::<String>();
        if b["cancelled"].as_bool() == Some(true) {
            if status_msg > 0 {
                let _ = encbot_api(env, "editMessageText",
                    final_status("\u{23F8} Run to'xtatildi \u{2014} post navbatga qaytdi".to_string())).await;
            }
            turso_exec(env,
                "UPDATE post_jobs SET state='queued', runner='', lease_until=0, attempts=MAX(attempts-1,0) WHERE id=?",
                vec![TursoArg::int(id)]).await?;
            return ok(json!({"ok": true}));
        }
        let fatal = b["fatal"].as_bool() == Some(true) || jint(&job, "attempts") >= POST_MAX_ATTEMPTS;
        if status_msg > 0 {
            let _ = encbot_api(env, "editMessageText", final_status(format!(
                "{} xato: {}", if fatal { "\u{274C}" } else { "\u{26A0}\u{FE0F}" }, html_escape(&err)))).await;
        }
        turso_exec(env,
            "UPDATE post_jobs SET state=?, runner='', lease_until=0, error=? WHERE id=?",
            vec![TursoArg::text(if fatal { "error" } else { "queued" }), TursoArg::text(&err), TursoArg::int(id)]).await?;
        encbot_send(env, ADMIN_TELEGRAM_ID, &format!(
            "{} Post: <b>#{id} {}</b> \u{2014} <code>{}</code>\n{}",
            if fatal { "\u{274C}" } else { "\u{26A0}\u{FE0F}" }, html_escape(&short(&name)), html_escape(&err),
            if fatal { "To'xtadi. Navbatdagi postlar ichidan qayta urinish yoki o'chirish mumkin." }
            else { "Keyinroq yana urinib ko'riladi." }), None).await;
        return ok(json!({"ok": true}));
    }

    err404("topilmadi")
}
