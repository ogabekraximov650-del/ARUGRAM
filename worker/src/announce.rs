// ══════════════════════════════════════════════════════════════
//  YANGI QISM E'LONI — kanalga post
// ══════════════════════════════════════════════════════════════
//
// TALAB (foydalanuvchi): kanalga bazaga qo'shilgan HAR BIR yangi qism, faqat BIRINCHI sifati
// joylangach, post bo'lib tushsin: bo'lim rasmi, bo'lim nomi, "N-bo'lim M-qism" (film bo'lsa
// "N ta Film"), tagida "Tomosha qilish" tugmasi — u asosiy botga /start yuboradigan havola.
//
// Kanal ID: `tool/announce/channel.enc` (ENCODE_TOKEN bilan shifrlangan JSON) -> deployda secret
// `ANNOUNCE_CHANNEL`. Post asosiy bot nomidan (u kanalda ADMIN bo'lishi shart); bo'lmasa kodlash boti.
//
// Qism faqat BIR MARTA e'lon qilinadi (`announced` jadvali). Chaqiriladigan joylar:
//   * `/api/encode/quality` — qismning BIRINCHI sifati yozilganda (avto-kodlash);
//   * `POST /api/epizods` — qism darhol fayl bilan qo'shilganda (admin).
//
// MUQOVA: bo'lim rasmi Telegram'da SHIFRLANGAN (AES-128-CTR), shu sabab uni ochish kerak. Bu
// "worker orqali fayl baytlari o'tmaydi" qoidasidan YAGONA ISTISNO: faqat kichik muqova rasmi, har bir
// bo'lim uchun BIR MARTA (keyin Telegram `file_id` si `announce_cover` da eslab qolinadi).

use super::*;
use aes::Aes128;
use ctr::cipher::{KeyIvInit, StreamCipher};

pub(crate) const DDL: [&str; 2] = [
    "CREATE TABLE IF NOT EXISTS announced (
        anime_id INTEGER, season_id INTEGER, epizod_id INTEGER, at INTEGER,
        PRIMARY KEY (anime_id, season_id, epizod_id)
    )",
    // photo_name — bo'lim rasmining fayl nomi (rasm almashsa yangidan tayyorlanadi).
    "CREATE TABLE IF NOT EXISTS announce_cover (
        photo_name TEXT PRIMARY KEY, file_id TEXT NOT NULL
    )",
];

/// Cho'ntakdagi muqova hajmi chegarasi (Bot API `getFile` — 20 MB, muqova esa kichik bo'ladi).
const COVER_MAX: usize = 8 * 1024 * 1024;

fn channel(env: &Env) -> i64 {
    serde_json::from_str::<Value>(&tg_secret(env, "ANNOUNCE_CHANNEL")).ok()
        .and_then(|v| v["channel_id"].as_i64().or_else(|| v["channel_id"].as_str().and_then(|s| s.trim().parse().ok())))
        .unwrap_or(0)
}

/// Qism endi ko'rinadi (birinchi sifat joylandi) — kanalga e'lon. Xato bo'lsa admin'ga xabar.
pub(crate) async fn episode_ready(env: &Env, a: i64, s: i64, e: i64) {
    let ch = channel(env);
    if ch == 0 {
        return;
    }
    // Bir marta: atomik belgi.
    let claim = turso_exec(env,
        "INSERT INTO announced (anime_id, season_id, epizod_id, at) VALUES (?,?,?,?)
         ON CONFLICT DO NOTHING RETURNING at",
        vec![TursoArg::int(a), TursoArg::int(s), TursoArg::int(e), TursoArg::int(now_ms())]).await;
    if !claim.ok().and_then(|r| first_row(&r)).is_some() {
        return;
    }
    if let Err(why) = post(env, ch, a, s, e).await {
        // Keyingi urinish uchun belgi olib tashlanadi; admin ogohlantiriladi.
        let _ = turso_exec(env, "DELETE FROM announced WHERE anime_id=? AND season_id=? AND epizod_id=?",
            vec![TursoArg::int(a), TursoArg::int(s), TursoArg::int(e)]).await;
        tg_send(env, ADMIN_TELEGRAM_ID, &format!(
            "\u{26A0}\u{FE0F} Yangi qism e'lon qilinmadi ({a}/{s}/{e}): <code>{}</code>\n\
             Asosiy bot e'lon kanalida (<code>{ch}</code>) admin ekanini tekshiring.", html_escape(&why))).await;
    }
}

/// Admin tugmasi: qismni kanalga QO'LDA qayta joylash (avval e'lon qilingan bo'lsa ham).
pub(crate) async fn repost(env: &Env, a: i64, s: i64, e: i64) -> std::result::Result<(), String> {
    let ch = channel(env);
    if ch == 0 {
        return Err("e'lon kanali sozlanmagan".into());
    }
    post(env, ch, a, s, e).await?;
    let _ = turso_exec(env,
        "INSERT INTO announced (anime_id, season_id, epizod_id, at) VALUES (?,?,?,?)
         ON CONFLICT DO NOTHING",
        vec![TursoArg::int(a), TursoArg::int(s), TursoArg::int(e), TursoArg::int(now_ms())]).await;
    Ok(())
}

/// Qismi: "N-bo'lim M-qism" yoki film bo'lsa "Film" ("N ta Film" — bo'limda bir nechta bo'lsa).
fn episode_text(season: &Value, ep_number: i64) -> String {
    if season["turi"].as_str().unwrap_or("").trim().eq_ignore_ascii_case("film") {
        let n = jint(season, "epizod_count");
        return if n > 1 { format!("{n} ta Film") } else { "Film".to_string() };
    }
    format!("{}-bo'lim {ep_number}-qism", jint(season, "bolim_id").max(1))
}

/// Kanal posti matni (foydalanuvchi namunasi):
///   • <bo'lim nomi>
///   • <N-qism>  (film bo'lsa: Film)
///   • Ovoz berdi: <bo'limdagi tarjimon>
fn caption(season: &Value, ep_number: i64) -> String {
    let film = season["turi"].as_str().unwrap_or("").trim().eq_ignore_ascii_case("film");
    let ep = if film { "Film".to_string() } else { format!("{ep_number}-qism") };
    let mut out = format!("\u{2022} {}\n\u{2022} {}",
        html_escape(season["nomi"].as_str().unwrap_or("").trim()), html_escape(&ep));
    let tarjimon = season["tarjimon"].as_str().unwrap_or("").trim();
    if !tarjimon.is_empty() {
        out.push_str(&format!("\n\u{2022} Ovoz berdi: {}", html_escape(tarjimon)));
    }
    out
}

async fn post(env: &Env, ch: i64, a: i64, s: i64, e: i64) -> std::result::Result<(), String> {
    let season = turso_exec(env,
        "SELECT nomi, photo_url, bolim_id, turi, epizod_count, tarjimon FROM season_db WHERE anime_id=? AND season_id=?",
        vec![TursoArg::int(a), TursoArg::int(s)]).await.map_err(|e| e.to_string())?;
    let season = first_row(&season).ok_or("bo'lim topilmadi")?;
    let num = turso_exec(env, "SELECT epizod_number FROM epizod_db WHERE anime_id=? AND season_id=? AND epizod_id=?",
        vec![TursoArg::int(a), TursoArg::int(s), TursoArg::int(e)]).await.ok()
        .and_then(|r| first_row(&r)).map(|r| jint(&r, "epizod_number")).unwrap_or(0);
    let text = caption(&season, num);
    let bot = bot_username(env).await;
    if bot.is_empty() {
        return Err("asosiy bot nomi olinmadi".into());
    }
    // "Tomosha qilish" — asosiy botga /start (Mini App tugmasi bilan javob beradi).
    let markup = json!({"inline_keyboard": [[{
        "text": "Tomosha qilish",
        "url": format!("https://t.me/{bot}?start=w{a}_{s}_{e}"),
    }]]});

    let photo = bare_name(season["photo_url"].as_str().unwrap_or(""));
    let file_id = if photo.is_empty() { None } else { cover_file_id(env, &photo, ch).await.ok() };
    // Muqova bor — rasm bilan; yo'q yoki olinmasa — faqat matn (e'lon baribir chiqadi).
    let sent = match &file_id {
        Some(fid) => send(env, "sendPhoto", json!({"chat_id": ch, "photo": fid, "caption": text,
            "parse_mode": "HTML", "reply_markup": markup})).await,
        None => send(env, "sendMessage", json!({"chat_id": ch, "text": text, "parse_mode": "HTML",
            "disable_web_page_preview": true, "reply_markup": markup})).await,
    };
    sent.map(|_| ())
}

/// Asosiy bot, bo'lmasa kodlash boti.
async fn send(env: &Env, method: &str, body: Value) -> std::result::Result<Value, String> {
    match tg_api(env, method, body.clone()).await {
        Ok(v) => Ok(v),
        Err(first) => match encbot_api(env, method, body).await {
            Ok(v) => Ok(v),
            Err(_) => Err(first.to_string()),
        },
    }
}

fn hex_key(h: &str) -> Option<[u8; 16]> {
    if h.len() != 32 { return None; }
    let mut out = [0u8; 16];
    for i in 0..16 {
        out[i] = u8::from_str_radix(&h[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// Bo'lim rasmi uchun Telegram `file_id` (ochilgan nusxa): keshdan, yo'q bo'lsa — saqlash kanalidan
/// olinadi, ochiladi va e'lon kanaliga yuklanib `file_id` eslab qolinadi (rasm shu bilan chiqadi).
async fn cover_file_id(env: &Env, photo: &str, ch: i64) -> std::result::Result<String, String> {
    if let Some(r) = turso_exec(env, "SELECT file_id FROM announce_cover WHERE photo_name=?",
        vec![TursoArg::text(photo)]).await.ok().and_then(|r| first_row(&r)) {
        if let Some(f) = r["file_id"].as_str().filter(|f| !f.is_empty()) {
            return Ok(f.to_string());
        }
    }
    let row = turso_exec(env, "SELECT msg_id, file_key FROM tg_files WHERE file_name=?",
        vec![TursoArg::text(photo)]).await.map_err(|e| e.to_string())?;
    let row = first_row(&row).ok_or("rasm saqlash kanalida topilmadi")?;
    let msg_id = jint(&row, "msg_id");
    let key = hex_key(row["file_key"].as_str().unwrap_or("").trim()).ok_or("rasm kaliti yo'q")?;
    let store = tg_channel_id(env);
    if store == 0 || msg_id <= 0 {
        return Err("saqlash kanali sozlanmagan".into());
    }
    // Hujjatning `file_id` si: xabarni adminga uzatib (forward) olinadi va darhol o'chiriladi.
    let fwd = tg_api(env, "forwardMessage", json!({
        "chat_id": ADMIN_TELEGRAM_ID, "from_chat_id": store, "message_id": msg_id, "disable_notification": true,
    })).await.map_err(|e| e.to_string())?;
    if let Some(m) = fwd["message_id"].as_i64() {
        let _ = tg_api(env, "deleteMessage", json!({"chat_id": ADMIN_TELEGRAM_ID, "message_id": m})).await;
    }
    let fid = fwd["document"]["file_id"].as_str().ok_or("hujjat file_id si yo'q")?.to_string();
    let info = tg_api(env, "getFile", json!({"file_id": fid})).await.map_err(|e| e.to_string())?;
    let path = info["file_path"].as_str().ok_or("file_path yo'q")?.to_string();
    let token = tg_secret(env, "TELEGRAM_BOT_TOKEN");
    let mut resp = Fetch::Url(format!("https://api.telegram.org/file/bot{token}/{path}").parse()
        .map_err(|_| "manzil xato".to_string())?).send().await.map_err(|e| e.to_string())?;
    let mut bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    if bytes.is_empty() || bytes.len() > COVER_MAX {
        return Err("rasm hajmi noto'g'ri".into());
    }
    // AES-128-CTR (boshlang'ich hisoblagich 0) — yuklashdagi (`telegram.rs`) bilan bir xil.
    let mut c = ctr::Ctr128BE::<Aes128>::new(&key.into(), &[0u8; 16].into());
    c.apply_keystream(&mut bytes);

    // E'lon kanaliga rasm sifatida yuklaymiz, `file_id` ni olamiz, xabarni o'chiramiz.
    let sent = upload_photo(env, ch, &bytes).await?;
    let new_id = sent["result"]["photo"].as_array()
        .and_then(|a| a.last()).and_then(|p| p["file_id"].as_str())
        .ok_or("yuklangan rasm file_id si yo'q")?.to_string();
    if let Some(m) = sent["result"]["message_id"].as_i64() {
        let _ = tg_api(env, "deleteMessage", json!({"chat_id": ch, "message_id": m})).await;
    }
    let _ = turso_exec(env,
        "INSERT INTO announce_cover (photo_name, file_id) VALUES (?,?)
         ON CONFLICT(photo_name) DO UPDATE SET file_id=excluded.file_id",
        vec![TursoArg::text(photo), TursoArg::text(&new_id)]).await;
    Ok(new_id)
}

/// `sendPhoto` (multipart) — asosiy bot bilan.
async fn upload_photo(env: &Env, ch: i64, bytes: &[u8]) -> std::result::Result<Value, String> {
    let token = tg_secret(env, "TELEGRAM_BOT_TOKEN");
    let fd = web_sys::FormData::new().map_err(|_| "FormData".to_string())?;
    fd.append_with_str("chat_id", &ch.to_string()).map_err(|_| "FormData".to_string())?;
    fd.append_with_str("disable_notification", "true").map_err(|_| "FormData".to_string())?;
    let arr = js_sys::Uint8Array::from(bytes);
    let parts = js_sys::Array::new();
    parts.push(&arr);
    let blob = web_sys::Blob::new_with_u8_array_sequence(&parts).map_err(|_| "Blob".to_string())?;
    fd.append_with_blob_and_filename("photo", &blob, "cover.jpg").map_err(|_| "FormData".to_string())?;
    let req = Request::new_with_init(
        &format!("https://api.telegram.org/bot{token}/sendPhoto"),
        RequestInit::new().with_method(Method::Post).with_body(Some(fd.into())),
    ).map_err(|e| e.to_string())?;
    let mut r = Fetch::Request(req).send().await.map_err(|e| e.to_string())?;
    let v: Value = r.json().await.map_err(|e| e.to_string())?;
    if v["ok"] != json!(true) {
        return Err(format!("sendPhoto: {}", v["description"].as_str().unwrap_or("noma'lum")));
    }
    Ok(v)
}

/// `/start w<anime>_<bo'lim>[_<qism>]` (e'londagi "Tomosha qilish") -> (anime, bo'lim, qism).
pub(crate) fn parse_watch(arg: &str) -> Option<(i64, i64, Option<i64>)> {
    let rest = arg.trim().strip_prefix('w')?;
    let parts: Vec<&str> = rest.split('_').collect();
    if parts.len() < 2 || parts.len() > 3 || parts.iter().any(|p| p.is_empty() || !p.chars().all(|c| c.is_ascii_digit())) {
        return None;
    }
    let n: Vec<i64> = parts.iter().filter_map(|p| p.parse().ok()).collect();
    if n.len() != parts.len() { return None; }
    Some((n[0], n[1], n.get(2).copied()))
}

fn line(icon: &str, label: &str, value: &str) -> String {
    let v = value.trim();
    if v.is_empty() { String::new() } else { format!("{icon} <b>{label}:</b> {}\n", html_escape(v)) }
}

/// Botda ochiladigan karta: anime rasmi va TAVSIFDAN BOSHQA hamma ma'lumot (bo'lim nomi, nechta
/// bo'lim va qism, yili, janr, studiya, tarjimon, turi, holati, yosh, davlat), tagida Mini App
/// tugmasi — u aynan shu anime va qismni ochadi (`web/src/js/app.js`, `?w=`).
pub(crate) async fn watch_card(env: &Env, chat: i64, a: i64, s: i64, e: Option<i64>) {
    let Some(season) = turso_exec(env, "SELECT * FROM season_db WHERE anime_id=? AND season_id=?",
        vec![TursoArg::int(a), TursoArg::int(s)]).await.ok().and_then(|r| first_row(&r)) else {
        tg_send_help(env, chat).await;
        return;
    };
    let anime = turso_exec(env, "SELECT name, davlat FROM anime_db WHERE id=?", vec![TursoArg::int(a)]).await
        .ok().and_then(|r| first_row(&r)).unwrap_or(json!({}));
    let seasons = turso_exec(env, "SELECT COUNT(*) AS n FROM season_db WHERE anime_id=?", vec![TursoArg::int(a)]).await
        .ok().and_then(|r| first_row(&r)).map(|r| jint(&r, "n")).unwrap_or(0);
    let eps = turso_exec(env,
        "SELECT COUNT(*) AS n FROM epizod_db WHERE anime_id=? AND season_id=?
           AND (COALESCE(url_360p,'')<>'' OR COALESCE(url_480p,'')<>'' OR COALESCE(url_720p,'')<>'' OR COALESCE(url_1080p,'')<>'')",
        vec![TursoArg::int(a), TursoArg::int(s)]).await
        .ok().and_then(|r| first_row(&r)).map(|r| jint(&r, "n")).unwrap_or(0);
    let ep_number = match e {
        Some(e) => turso_exec(env, "SELECT epizod_number FROM epizod_db WHERE anime_id=? AND season_id=? AND epizod_id=?",
            vec![TursoArg::int(a), TursoArg::int(s), TursoArg::int(e)]).await.ok()
            .and_then(|r| first_row(&r)).map(|r| jint(&r, "epizod_number")),
        None => None,
    };
    let st = |k: &str| season[k].as_str().unwrap_or("").to_string();
    let is_film = st("turi").trim().eq_ignore_ascii_case("film");
    let mut t = format!("\u{1F3AC} <b>{}</b>\n", html_escape(st("nomi").trim()));
    let anime_name = anime["name"].as_str().unwrap_or("").trim().to_string();
    if !anime_name.is_empty() && anime_name != st("nomi").trim() {
        t.push_str(&format!("\u{1F4DA} {}\n", html_escape(&anime_name)));
    }
    t.push('\n');
    if let Some(n) = ep_number {
        t.push_str(&line("\u{25B6}\u{FE0F}", "Qism", &episode_text(&season, n)));
    }
    t.push_str(&line("\u{1F522}", "Bo'limlar", &if seasons > 0 { format!("{seasons} ta") } else { String::new() }));
    t.push_str(&line("\u{1F39E}", if is_film { "Filmlar" } else { "Qismlar" },
        &if eps > 0 { format!("{eps} ta") } else { String::new() }));
    t.push_str(&line("\u{1F4C5}", "Yili", &st("yili")));
    t.push_str(&line("\u{1F3AD}", "Janr", &st("janri")));
    t.push_str(&line("\u{1F3E2}", "Studiya", &st("studio")));
    t.push_str(&line("\u{1F399}", "Tarjimon", &st("tarjimon")));
    let tur = [st("turi"), st("holati")].iter().filter(|x| !x.trim().is_empty()).cloned().collect::<Vec<_>>().join(" \u{00B7} ");
    t.push_str(&line("\u{1F4FA}", "Turi", &tur));
    t.push_str(&line("\u{1F51E}", "Yosh chegarasi", &if jint(&season, "yosh") > 0 { format!("{}+", jint(&season, "yosh")) } else { String::new() }));
    let davlat = st("davlat");
    t.push_str(&line("\u{1F30D}", "Davlat", if davlat.trim().is_empty() { anime["davlat"].as_str().unwrap_or("") } else { &davlat }));
    t.push_str(&line("\u{1F3AC}", "Rejissyor", &st("rejissyor")));

    let url = match e {
        Some(e) => format!("{}/?w={a}_{s}_{e}", tma::WEB_URL),
        None => format!("{}/?w={a}_{s}", tma::WEB_URL),
    };
    let markup = json!({"inline_keyboard": [[{"text": "Tomosha qilish", "web_app": {"url": url}}]]});
    // Bot menyu tugmasi ham (eski odatdagidek).
    let _ = tg_api(env, "setChatMenuButton", json!({
        "chat_id": chat,
        "menu_button": {"type": "web_app", "text": "ARUmediaTV", "web_app": {"url": tma::WEB_URL}},
    })).await;

    let photo = bare_name(season["photo_url"].as_str().unwrap_or(""));
    let ch = channel(env);
    let file_id = if photo.is_empty() || ch == 0 { None } else { cover_file_id(env, &photo, ch).await.ok() };
    let r = match &file_id {
        Some(fid) => tg_api(env, "sendPhoto", json!({"chat_id": chat, "photo": fid, "caption": t,
            "parse_mode": "HTML", "reply_markup": markup})).await,
        None => tg_api(env, "sendMessage", json!({"chat_id": chat, "text": t, "parse_mode": "HTML",
            "disable_web_page_preview": true, "reply_markup": markup})).await,
    };
    if r.is_err() {
        // Rasm yoki matn rad etilsa — hech bo'lmasa tugmali oddiy xabar.
        let _ = tg_api(env, "sendMessage", json!({"chat_id": chat, "text": t, "parse_mode": "HTML",
            "reply_markup": markup})).await;
    }
}
