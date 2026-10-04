// worker/src/channels.rs — MAJBURIY OBUNA KANALLARI
//
// TALAB (foydalanuvchi): bepul bo'limni ko'rish uchun ilova
// foydalanuvchidan ruxsat so'raydi; ruxsat berilsa pleyer darhol
// ochiladi, ilova esa orqa fonda (har 5 daqiqada) foydalanuvchining
// O'Z Telegram hisobi bilan admin belgilagan kanallarga qo'shiladi
// (ochiq kanal) yoki qo'shilish so'rovini yuboradi (yopiq kanal).
//
// Tizim `aniraxuzbot15` (`bot/src/handlers/channels.ts`) dagi
// "Majburiy obunalar" asosida:
//
//   * kanal turlari: 📢 ochiq (public), 🔐 yopiq (private, qo'shilish
//     so'rovi bilan), 🔗 tashqi havola (Instagram, YouTube...);
//   * har kanalga LIMIT (`need`) — shuncha odam qo'shilgach kanal
//     endi talab qilinmaydi; `need = 0` — cheklovsiz;
//   * ko'pi bilan 8 ta kanal (tashqi havolalar bunga kirmaydi);
//   * HISOBLAGICH (`joined`) bot orqali, Telegram'ning o'z hodisalari
//     bilan oshadi (ilovaga ishonilmaydi):
//       - yopiq kanal: `chat_join_request` — so'rov yuborilgani.
//         `getChatMember` so'rovni KO'RMAYDI (odam tasdiqlanmaguncha
//         a'zo emas), shu sabab yagona ishonchli yo'l — shu hodisa;
//       - ochiq kanal: `chat_member` — odam a'zo bo'ldi.
//     Ikkalasi uchun ham asosiy bot kanalda ADMIN bo'lishi shart
//     ("Foydalanuvchi qo'shish" huquqi bilan).
//   * BAZADA FAQAT SO'ROV YUBORGANLAR saqlanadi (`chan_requests`,
//     aniraxuzbot15 dagi `request_chanel_db` kabi) — bir odamning so'rovi
//     bir marta sanaladi; so'rovdan keyin darhol a'zo bo'lishi shart emas.
//     Kanal o'chirilsa uning so'rovlari ham o'chiriladi. Ochiq kanalga
//     qo'shilganlar jurnalga YOZILMAYDI — faqat hisoblagich oshadi.
//
// ── YOZUVLAR ───────────────────────────────────────────────────
//
// Ilova bu yerga hech narsa YOZMAYDI (yagona yozuv yo'li qoidasi
// buzilmaydi): bazaga faqat admin amallari va Telegram hodisalari
// yozadi. Ilova faqat ro'yxatni o'qiydi (`GET /api/channels`).

use super::*;

pub(crate) const DDL: [&str; 2] = [
    // kind: public | private | external.
    //   public  — chat_id, username (@nom), title;
    //   private — chat_id, link (bot yaratgan, so'rov bilan), title;
    //   external — title (havola nomi), link (URL).
    "CREATE TABLE IF NOT EXISTS channels_db (
        id INTEGER PRIMARY KEY,
        kind TEXT NOT NULL,
        chat_id INTEGER DEFAULT 0,
        username TEXT DEFAULT '',
        link TEXT DEFAULT '',
        title TEXT DEFAULT '',
        need INTEGER DEFAULT 0,
        joined INTEGER DEFAULT 0,
        created_at INTEGER
    )",
    // Yopiq kanalga so'rov yuborganlar (takror sanalmasin). Telegram ID
    // bo'yicha (hodisada faqat shu bor — `users_db` dan qidirib
    // o'qish shart emas).
    "CREATE TABLE IF NOT EXISTS chan_requests (
        channel_id INTEGER NOT NULL,
        tg_id INTEGER NOT NULL,
        at INTEGER,
        PRIMARY KEY (channel_id, tg_id)
    ) WITHOUT ROWID",
];

/// Ko'pi bilan shuncha kanal (tashqi havolalar hisobga kirmaydi).
const MAX_CHANNELS: i64 = 8;

/// Ochiq kanal nomi: `@nom`, `nom`, `https://t.me/nom` -> `nom`.
fn public_name(raw: &str) -> Option<String> {
    let t = raw.trim();
    let t = t
        .strip_prefix("https://t.me/")
        .or_else(|| t.strip_prefix("http://t.me/"))
        .or_else(|| t.strip_prefix("t.me/"))
        .unwrap_or(t);
    let t = t.trim_start_matches('@').trim_end_matches('/');
    let ok = (5..=32).contains(&t.len()) && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    ok.then(|| t.to_string())
}

/// Admin yozgan kanal: `-100...` (ID) yoki `@nom`. Bot API `chat_id`
/// maydoniga to'g'ridan-to'g'ri qo'yiladigan qiymat.
fn chat_ref(raw: &str) -> Option<Value> {
    let t = raw.trim();
    if let Ok(n) = t.parse::<i64>() {
        let id = normalize_channel_id(t);
        return (n != 0 && id != 0).then(|| json!(id));
    }
    public_name(t).map(|n| json!(format!("@{n}")))
}

fn valid_url(u: &str) -> bool {
    let l = u.to_ascii_lowercase();
    (l.starts_with("https://") || l.starts_with("http://")) && u.len() > 10 && u.len() <= 300 && !u.contains(char::is_whitespace)
}

/// Kanal hali talab qilinadimi (limit to'lmagan).
fn active(c: &Value) -> bool {
    let need = jint(c, "need");
    need <= 0 || jint(c, "joined") < need
}

fn kind_label(kind: &str) -> &'static str {
    match kind {
        "public" => "\u{1F4E2} ochiq",
        "private" => "\u{1F510} yopiq",
        _ => "\u{1F517} tashqi havola",
    }
}

/// Foydalanuvchi qo'shiladigan manzil.
fn join_url(c: &Value) -> String {
    match c["kind"].as_str().unwrap_or("") {
        "public" => format!("https://t.me/{}", c["username"].as_str().unwrap_or("").trim_start_matches('@')),
        _ => c["link"].as_str().unwrap_or("").to_string(),
    }
}

async fn all_channels(env: &Env) -> Vec<Value> {
    turso_exec(env, "SELECT * FROM channels_db ORDER BY id ASC", vec![]).await
        .map(|r| rows_of(&r))
        .unwrap_or_default()
}

async fn channel_by_id(env: &Env, id: i64) -> Option<Value> {
    turso_exec(env, "SELECT * FROM channels_db WHERE id=?", vec![TursoArg::int(id)]).await
        .ok()
        .and_then(|r| first_row(&r))
}

// ═══════════════════════════════════════════════════════════════
//  TELEGRAM HODISALARI (asosiy botning webhook'i)
// ═══════════════════════════════════════════════════════════════

/// Yopiq kanalga so'rov: BIR MARTA yoziladi va sanaladi.
async fn record_request(env: &Env, chat_id: i64, tg_id: i64) {
    if chat_id == 0 || tg_id == 0 {
        return;
    }
    let Some(ch) = turso_exec(env,
        "SELECT id FROM channels_db WHERE chat_id=? AND kind='private' LIMIT 1",
        vec![TursoArg::int(chat_id)]).await.ok().and_then(|r| first_row(&r))
    else {
        return;
    };
    let id = jint(&ch, "id");
    // `changes()` — oldingi INSERT haqiqatda qator qo'shganmi (takror
    // bo'lsa 0): hisoblagich faqat yangi odamda oshadi.
    let _ = turso_batch(env, &[
        ("INSERT OR IGNORE INTO chan_requests (channel_id,tg_id,at) VALUES (?,?,?)",
         vec![TursoArg::int(id), TursoArg::int(tg_id), TursoArg::int(now_ms())]),
        ("UPDATE channels_db SET joined = joined + changes() WHERE id=?",
         vec![TursoArg::int(id)]),
    ]).await;
}

/// Ochiq kanalga qo'shildi: faqat hisoblagich (jurnal yozilmaydi).
async fn count_join(env: &Env, chat_id: i64) {
    if chat_id == 0 {
        return;
    }
    let _ = turso_exec(env,
        "UPDATE channels_db SET joined = joined + 1 WHERE chat_id=? AND kind='public'",
        vec![TursoArg::int(chat_id)]).await;
}

fn is_member_status(m: &Value) -> bool {
    match m["status"].as_str().unwrap_or("") {
        "member" | "administrator" | "creator" => true,
        "restricted" => m["is_member"].as_bool().unwrap_or(false),
        _ => false,
    }
}

/// Webhook'dagi kanal hodisasi. `true` — hodisa shu yerda qabul qilindi.
pub(crate) async fn on_update(env: &Env, update: &Value) -> bool {
    // Yopiq kanalga qo'shilish so'rovi.
    let jr = &update["chat_join_request"];
    if jr.is_object() {
        record_request(env, jr["chat"]["id"].as_i64().unwrap_or(0), jr["from"]["id"].as_i64().unwrap_or(0)).await;
        return true;
    }
    // A'zolik o'zgardi: faqat OCHIQ kanalda "a'zo emas -> a'zo" sanaladi
    // (yopiq kanal so'rov paytida sanalgan — tasdiqlanganda qayta emas).
    let cm = &update["chat_member"];
    if cm.is_object() {
        if !is_member_status(&cm["old_chat_member"]) && is_member_status(&cm["new_chat_member"]) {
            count_join(env, cm["chat"]["id"].as_i64().unwrap_or(0)).await;
        }
        return true;
    }
    // Botning o'z huquqi o'zgargani — e'tiborsiz.
    update["my_chat_member"].is_object()
}

// ═══════════════════════════════════════════════════════════════
//  ADMIN AMALLARI (ilovadagi admin paneli VA kodlash boti uchun bitta)
// ═══════════════════════════════════════════════════════════════

/// Asosiy botning IDsi (token boshi, sir emas).
fn bot_id(env: &Env) -> i64 {
    env.secret("TELEGRAM_BOT_TOKEN").ok()
        .map(|t| t.to_string())
        .and_then(|t| t.split(':').next().and_then(|s| s.parse().ok()))
        .unwrap_or(0)
}

/// Kanal qo'shadi. `input` — public/private uchun `@nom` yoki `-100...`,
/// external uchun URL; `name` — tashqi havola nomi. Javob — yangi qator.
pub(crate) async fn add(env: &Env, kind: &str, input: &str, name: &str, need: i64) -> std::result::Result<Value, String> {
    let now = now_ms();
    if kind == "external" {
        let name = name.trim();
        let url = input.trim();
        if name.is_empty() || name.chars().count() > 40 {
            return Err("Havola nomi 1-40 belgi bo'lsin".into());
        }
        if !valid_url(url) {
            return Err("http:// yoki https:// bilan boshlanadigan to'liq URL yuboring".into());
        }
        let res = turso_exec(env,
            "INSERT INTO channels_db (kind,title,link,created_at) VALUES ('external',?,?,?) RETURNING *",
            vec![TursoArg::text(name), TursoArg::text(url), TursoArg::int(now)]).await
            .map_err(|e| e.to_string())?;
        return first_row(&res).ok_or_else(|| "saqlanmadi".to_string());
    }
    if kind != "public" && kind != "private" {
        return Err("kanal turi noto'g'ri".into());
    }
    let cnt = turso_exec(env, "SELECT COUNT(*) FROM channels_db WHERE kind<>'external'", vec![]).await
        .map(|r| scalar(&r)).unwrap_or(0);
    if cnt >= MAX_CHANNELS {
        return Err(format!("Ko'pi bilan {MAX_CHANNELS} ta kanal. Hozir {cnt} ta qo'shilgan."));
    }
    let Some(chat) = chat_ref(input) else {
        return Err("Kanalni @username yoki -100... ID ko'rinishida yuboring (yoki kanaldan post forward qiling)".into());
    };
    let info = tg_api(env, "getChat", json!({"chat_id": chat})).await
        .map_err(|_| "Kanal topilmadi. Asosiy botni kanalga ADMIN qiling va qaytadan urining.".to_string())?;
    let chat_id = info["id"].as_i64().unwrap_or(0);
    if chat_id == 0 || !matches!(info["type"].as_str(), Some("channel") | Some("supergroup")) {
        return Err("Bu kanal yoki guruh emas".into());
    }
    let title = info["title"].as_str().unwrap_or("").to_string();
    let username = info["username"].as_str().map(|u| format!("@{u}")).unwrap_or_default();
    // Bot admin bo'lmasa hodisalar (`chat_join_request`, `chat_member`)
    // kelmaydi va hisoblagich hech qachon oshmaydi.
    let me = tg_api(env, "getChatMember", json!({"chat_id": chat_id, "user_id": bot_id(env)})).await
        .unwrap_or(json!({}));
    if me["status"].as_str() != Some("administrator") {
        return Err("Asosiy bot bu kanalda admin emas. Botni \"Foydalanuvchi qo'shish\" huquqi bilan admin qiling.".into());
    }
    let dup = turso_exec(env, "SELECT id FROM channels_db WHERE chat_id=?", vec![TursoArg::int(chat_id)]).await
        .ok().and_then(|r| first_row(&r));
    if dup.is_some() {
        return Err("Bu kanal allaqachon qo'shilgan".into());
    }
    let mut link = String::new();
    if kind == "public" {
        if username.is_empty() {
            return Err("Bu kanalning ochiq @username'i yo'q — uni \"yopiq kanal\" sifatida qo'shing".into());
        }
    } else {
        // Qo'shilish SO'ROVI bilan havola — so'rov yuborilgani shu bilan
        // botga `chat_join_request` bo'lib keladi.
        let l = tg_api(env, "createChatInviteLink", json!({
            "chat_id": chat_id,
            "creates_join_request": true,
            "name": "ARUGRAM majburiy obuna",
        })).await
            .map_err(|_| "Havola yaratib bo'lmadi. Botga \"Havola orqali taklif qilish\" huquqini bering.".to_string())?;
        link = l["invite_link"].as_str().unwrap_or("").to_string();
        if link.is_empty() {
            return Err("Havola yaratib bo'lmadi".into());
        }
    }
    let res = turso_exec(env,
        "INSERT INTO channels_db (kind,chat_id,username,link,title,need,created_at)
         VALUES (?,?,?,?,?,?,?) RETURNING *",
        vec![
            TursoArg::text(kind), TursoArg::int(chat_id), TursoArg::text(&username),
            TursoArg::text(&link), TursoArg::text(&title), TursoArg::int(need.max(0)),
            TursoArg::int(now),
        ]).await.map_err(|e| e.to_string())?;
    first_row(&res).ok_or_else(|| "saqlanmadi".to_string())
}

/// Limitni o'zgartiradi (`delta` musbat — oshirish, manfiy — kamaytirish,
/// 0 dan pastga tushmaydi).
pub(crate) async fn change_limit(env: &Env, id: i64, delta: i64) -> Option<Value> {
    let res = turso_exec(env,
        "UPDATE channels_db SET need = MAX(0, need + ?) WHERE id=? AND kind<>'external' RETURNING *",
        vec![TursoArg::int(delta), TursoArg::int(id)]).await.ok()?;
    first_row(&res)
}

/// O'chiradi — yopiq kanal bo'lsa unga so'rov yuborganlar ham o'chadi.
pub(crate) async fn delete(env: &Env, id: i64) -> bool {
    turso_batch(env, &[
        ("DELETE FROM chan_requests WHERE channel_id=?", vec![TursoArg::int(id)]),
        ("DELETE FROM channels_db WHERE id=?", vec![TursoArg::int(id)]),
    ]).await.is_ok()
}

/// Admin uchun ko'rinish (hisoblagichlar bilan).
fn admin_view(c: &Value) -> Value {
    json!({
        "id": jint(c, "id"),
        "kind": c["kind"],
        "chat_id": jint(c, "chat_id"),
        "username": c["username"],
        "link": c["link"],
        "url": join_url(c),
        "title": c["title"],
        "need": jint(c, "need"),
        "joined": jint(c, "joined"),
        "active": active(c),
    })
}

// ═══════════════════════════════════════════════════════════════
//  HTTP
// ═══════════════════════════════════════════════════════════════
//
//   GET  /api/channels        — ilova: hozir talab qilinadigan kanallar;
//   GET  /api/admin/channels  — admin: hammasi, hisoblagichlar bilan;
//   POST /api/admin/channels  — admin: {op: add|limit|del, ...}.

pub(crate) async fn route(req: Request, env: &Env, path: &str, method: Method) -> Result<Response> {
    let Some(u) = session_user(env, &bearer(&req)).await? else {
        return json_resp(&json!({"error": "unauthorized"}), 401);
    };

    if path == "/api/channels" && method == Method::Get {
        // Limiti to'lgan kanal endi talab qilinmaydi va ko'rsatilmaydi.
        // Hisoblagichlar va kanal IDsi ilovaga berilmaydi.
        let items: Vec<Value> = all_channels(env).await.iter()
            .filter(|c| c["kind"] == "external" || active(c))
            .map(|c| json!({
                "id": jint(c, "id"),
                "kind": c["kind"],
                "title": c["title"],
                "url": join_url(c),
            }))
            .collect();
        return ok_nostore(json!({"items": items}));
    }

    if path != "/api/admin/channels" {
        return err404("not_found");
    }
    if !is_admin(&u) {
        return json_resp(&json!({"error": "forbidden"}), 403);
    }
    if method == Method::Get {
        let items: Vec<Value> = all_channels(env).await.iter().map(admin_view).collect();
        return ok_nostore(json!({"items": items, "max": MAX_CHANNELS}));
    }
    if method != Method::Post {
        return err404("not_found");
    }
    let mut req = req;
    let b: Value = req.json().await.unwrap_or(json!({}));
    let id = jint(&b, "id");
    match b["op"].as_str().unwrap_or("") {
        "add" => match add(env,
            b["kind"].as_str().unwrap_or(""),
            b["input"].as_str().unwrap_or(""),
            b["name"].as_str().unwrap_or(""),
            jint(&b, "need")).await
        {
            Ok(c) => ok_nostore(json!({"ok": true, "item": admin_view(&c)})),
            Err(e) => json_resp(&json!({"error": e}), 400),
        },
        "limit" => match change_limit(env, id, jint(&b, "delta")).await {
            Some(c) => ok_nostore(json!({"ok": true, "item": admin_view(&c)})),
            None => err404("Topilmadi"),
        },
        "del" => ok_nostore(json!({"ok": delete(env, id).await})),
        _ => json_resp(&json!({"error": "op noto'g'ri"}), 400),
    }
}

// ═══════════════════════════════════════════════════════════════
//  KODLASH BOTI (admin paneli boti): "🔐 Majburiy obunalar"
// ═══════════════════════════════════════════════════════════════
//
// `aniraxuzbot15` dagi menyu: yangi kanal qo'shish (turini tanlash),
// ro'yxat, kanal ichida limitni oshirish/kamaytirish va o'chirish.
// Matn kutilayotgan holat `app_config.encbot_chwait` da turadi.

pub(crate) const BTN_CHANNELS: &str = "\u{1F510} Majburiy obunalar";
const WAIT_KEY: &str = "encbot_chwait";

fn ikb(rows: Vec<Vec<(String, String)>>) -> Value {
    json!({"inline_keyboard": rows.into_iter()
        .map(|r| r.into_iter().map(|(t, d)| json!({"text": t, "callback_data": d})).collect::<Vec<_>>())
        .collect::<Vec<_>>()})
}

fn b(t: &str, d: &str) -> (String, String) {
    (t.to_string(), d.to_string())
}

fn back(to: &str) -> Value {
    ikb(vec![vec![b("\u{27E8} Orqaga", to)]])
}

/// Xabarni tahrirlaydi (tugma bosilgan bo'lsa), bo'lmasa yangisini yuboradi.
async fn show(env: &Env, chat: i64, msg_id: i64, text: &str, kb: Value) {
    if msg_id > 0 {
        let r = encbot_api(env, "editMessageText", json!({
            "chat_id": chat, "message_id": msg_id, "text": text,
            "parse_mode": "HTML", "disable_web_page_preview": true, "reply_markup": kb,
        })).await;
        if r.is_ok() {
            return;
        }
    }
    encbot_send(env, chat, text, Some(kb)).await;
}

fn menu_kb() -> Value {
    ikb(vec![
        vec![b("\u{2795} Yangi kanal qo'shish", "ch:add")],
        vec![b("\u{1F5C2} Kanallar ro'yxati", "ch:list")],
    ])
}

const MENU_TEXT: &str = "\u{1F510} <b>Majburiy obunalar</b>\n\n\
    Bepul bo'limni ko'rishdan oldin ilova foydalanuvchidan ruxsat so'raydi va uning \
    Telegram hisobi bilan shu kanallarga o'zi qo'shiladi (yopiq kanalga so'rov yuboradi).";

pub(crate) async fn menu(env: &Env, chat: i64) {
    config_put(env, WAIT_KEY, "").await;
    encbot_send(env, chat, MENU_TEXT, Some(menu_kb())).await;
}

fn list_text(items: &[Value]) -> String {
    if items.is_empty() {
        return "\u{1F4ED} Hali hech qanday kanal qo'shilmagan.".into();
    }
    let mut t = "\u{1F5C2} <b>Kanallar ro'yxati</b>\n\n".to_string();
    for (i, c) in items.iter().enumerate() {
        t.push_str(&format!("{}. {}\n", i + 1, detail_lines(c)));
    }
    t
}

fn detail_lines(c: &Value) -> String {
    let kind = c["kind"].as_str().unwrap_or("");
    let title = html_escape(c["title"].as_str().unwrap_or(""));
    let mut t = format!("<b>Turi:</b> {}\n", kind_label(kind));
    if kind == "external" {
        t.push_str(&format!("   <b>Nomi:</b> {title}\n   <b>Havola:</b> {}\n", html_escape(&join_url(c))));
        return t;
    }
    t.push_str(&format!("   <b>Nomi:</b> {title}\n   <b>ID:</b> <code>{}</code>\n", jint(c, "chat_id")));
    if kind == "public" {
        t.push_str(&format!("   <b>Useri:</b> {}\n", html_escape(c["username"].as_str().unwrap_or(""))));
    } else {
        t.push_str(&format!("   <b>Havola:</b> {}\n", html_escape(c["link"].as_str().unwrap_or(""))));
    }
    let verb = if kind == "public" { "qo'shilgan" } else { "so'rov yuborgan" };
    let need = jint(c, "need");
    let lim = if need > 0 { need.to_string() } else { "\u{221E}".to_string() };
    let state = if active(c) { "" } else { " \u{2705} limit to'ldi" };
    t.push_str(&format!("   <b>Statistika:</b> {}/{} tasi {verb}{state}\n", jint(c, "joined"), lim));
    t
}

async fn show_list(env: &Env, chat: i64, msg_id: i64) {
    let items = all_channels(env).await;
    let mut rows: Vec<Vec<(String, String)>> = Vec::new();
    let mut row = Vec::new();
    for (i, c) in items.iter().enumerate() {
        row.push(((i + 1).to_string(), format!("ch:view:{}", jint(c, "id"))));
        if row.len() == 4 {
            rows.push(std::mem::take(&mut row));
        }
    }
    if !row.is_empty() {
        rows.push(row);
    }
    rows.push(vec![b("\u{27E8} Orqaga", "ch:menu")]);
    show(env, chat, msg_id, &list_text(&items), ikb(rows)).await;
}

async fn show_one(env: &Env, chat: i64, msg_id: i64, id: i64) {
    let Some(c) = channel_by_id(env, id).await else {
        show(env, chat, msg_id, "\u{274C} Topilmadi.", back("ch:list")).await;
        return;
    };
    let mut rows = Vec::new();
    if c["kind"] != "external" {
        rows.push(vec![b("\u{1F4C8} Limitni oshirish", &format!("ch:inc:{id}"))]);
        rows.push(vec![b("\u{1F4C9} Limitni kamaytirish", &format!("ch:dec:{id}"))]);
    }
    rows.push(vec![b("\u{1F5D1} O'chirish", &format!("ch:del:{id}"))]);
    rows.push(vec![b("\u{27E8} Orqaga", "ch:list")]);
    show(env, chat, msg_id, &detail_lines(&c), ikb(rows)).await;
}

const HOW_TO_ADD: &str = "1. Asosiy botni kanalga ADMIN qiling (\"Foydalanuvchi qo'shish\" va \
    \"Havola orqali taklif qilish\" huquqlari bilan).\n\
    2. Kanaldan istalgan postni shu yerga FORWARD qiling yoki kanal @username'ini / \
    <code>-100...</code> IDsini yuboring.";

/// Inline tugma (`ch:...`). `true` — shu yerda bajarildi.
pub(crate) async fn on_callback(env: &Env, chat: i64, msg_id: i64, data: &str) -> bool {
    let Some(rest) = data.strip_prefix("ch:") else { return false };
    let parts: Vec<&str> = rest.split(':').collect();
    let id = parts.get(1).and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
    match parts[0] {
        "menu" => {
            config_put(env, WAIT_KEY, "").await;
            show(env, chat, msg_id, MENU_TEXT, menu_kb()).await;
        }
        "add" => {
            config_put(env, WAIT_KEY, "").await;
            show(env, chat, msg_id,
                "Turini tanlang:\n\n\
                 \u{1F4E2} <b>Ochiq kanal</b> — ilova foydalanuvchini kanalga o'zi qo'shadi.\n\n\
                 \u{1F510} <b>Yopiq kanal</b> — ilova qo'shilish so'rovini yuboradi.\n\n\
                 \u{1F517} <b>Tashqi havola</b> — Instagram, YouTube kabi havolalar (faqat ko'rsatiladi).",
                ikb(vec![
                    vec![b("\u{1F4E2} Ochiq [ public ] kanal", "ch:new:public")],
                    vec![b("\u{1F510} Yopiq [ private ] kanal", "ch:new:private")],
                    vec![b("\u{1F517} Tashqi havola [ URL ]", "ch:new:external")],
                    vec![b("\u{27E8} Orqaga", "ch:menu")],
                ])).await;
        }
        "new" => {
            let kind = parts.get(1).copied().unwrap_or("");
            let (wait, text) = match kind {
                "public" => ("pub", format!("<b>Turi:</b> {}\n\n{HOW_TO_ADD}", kind_label("public"))),
                "private" => ("priv", format!("<b>Turi:</b> {}\n\n{HOW_TO_ADD}", kind_label("private"))),
                _ => ("ext_name", format!("<b>Turi:</b> {}\n\nHavola uchun NOM yuboring \
                    (masalan: <code>Instagram</code>):", kind_label("external"))),
            };
            config_put(env, WAIT_KEY, wait).await;
            show(env, chat, msg_id, &text, back("ch:add")).await;
        }
        "list" => {
            config_put(env, WAIT_KEY, "").await;
            show_list(env, chat, msg_id).await;
        }
        "view" => show_one(env, chat, msg_id, id).await,
        "inc" | "dec" => {
            config_put(env, WAIT_KEY, &format!("{}:{id}", parts[0])).await;
            let verb = if parts[0] == "inc" { "oshirmoqchisiz" } else { "kamaytirmoqchisiz" };
            show(env, chat, msg_id, &format!("Limitni qanchaga {verb}? Raqam yuboring (masalan: 500):"),
                back(&format!("ch:view:{id}"))).await;
        }
        "del" => {
            show(env, chat, msg_id, "\u{26A0}\u{FE0F} Rostdan o'chirilsinmi?",
                ikb(vec![vec![b("\u{2705} Ha, o'chirish", &format!("ch:delyes:{id}")),
                              b("\u{274C} Bekor", &format!("ch:view:{id}"))]])).await;
        }
        "delyes" => {
            delete(env, id).await;
            show_list(env, chat, msg_id).await;
        }
        _ => return false,
    }
    true
}

/// Kanaldan forward qilingan post bo'lsa — kanal IDsi.
fn forwarded_chat(msg: &Value) -> Option<String> {
    let fo = &msg["forward_origin"];
    if fo["type"] == "channel" {
        return fo["chat"]["id"].as_i64().map(|i| i.to_string());
    }
    msg["forward_from_chat"]["id"].as_i64().map(|i| i.to_string())
}

/// Kutilayotgan matn (kanal, limit, havola). `true` — shu yerda
/// qabul qilindi. Pastki tugmalar (boshqa bo'lim) holatni bekor qiladi.
pub(crate) async fn on_text(env: &Env, chat: i64, msg: &Value, text: &str) -> bool {
    let wait = config_get(env, WAIT_KEY).await.unwrap_or_default();
    if wait.is_empty() {
        return false;
    }
    let nav = text.starts_with('/')
        || ["\u{1F3AC}", "\u{1F4C2}", "\u{1F39E}", "\u{2795}", "\u{1F4CB}", "\u{2B05}", "\u{25C0}", "Keyingi", "\u{1F510}"]
            .iter().any(|p| text.starts_with(p));
    if nav && forwarded_chat(msg).is_none() {
        config_put(env, WAIT_KEY, "").await;
        return false;
    }
    let num = text.trim().parse::<i64>().ok().filter(|n| *n >= 1);
    let (head, arg) = wait.split_once(':').unwrap_or((wait.as_str(), ""));
    match head {
        "pub" | "priv" => {
            let input = forwarded_chat(msg).unwrap_or_else(|| text.trim().to_string());
            let kind = if head == "pub" { "public" } else { "private" };
            match add(env, kind, &input, "", 0).await {
                Ok(c) => {
                    let id = jint(&c, "id");
                    config_put(env, WAIT_KEY, &format!("limit:{id}")).await;
                    encbot_send(env, chat, &format!(
                        "\u{2705} Qo'shildi: <b>{}</b>\n\nEndi kanalga qo'shilishi kerak bo'lgan odamlar \
                         sonini RAQAM bilan yuboring (masalan: 1000).\n<code>0</code> — cheklovsiz.",
                        html_escape(c["title"].as_str().unwrap_or(""))), None).await;
                }
                Err(e) => encbot_send(env, chat, &format!("\u{274C} {}", html_escape(&e)), Some(back("ch:add"))).await,
            }
        }
        "limit" => {
            let Ok(n) = text.trim().parse::<i64>() else {
                encbot_send(env, chat, "\u{274C} Faqat raqam yuboring (0 — cheklovsiz).", None).await;
                return true;
            };
            let id = arg.parse().unwrap_or(0);
            let _ = turso_exec(env, "UPDATE channels_db SET need=? WHERE id=?",
                vec![TursoArg::int(n.max(0)), TursoArg::int(id)]).await;
            config_put(env, WAIT_KEY, "").await;
            encbot_send(env, chat, &format!("\u{2705} Saqlandi. Limit: {}",
                if n > 0 { n.to_string() } else { "cheklovsiz".into() }), Some(menu_kb())).await;
        }
        "inc" | "dec" => {
            let Some(n) = num else {
                encbot_send(env, chat, "\u{274C} Faqat musbat raqam yuboring.", None).await;
                return true;
            };
            let id = arg.parse().unwrap_or(0);
            config_put(env, WAIT_KEY, "").await;
            match change_limit(env, id, if head == "inc" { n } else { -n }).await {
                Some(c) => encbot_send(env, chat, &format!("\u{2705} Yangi limit: <b>{}</b>", jint(&c, "need")),
                    Some(back(&format!("ch:view:{id}")))).await,
                None => encbot_send(env, chat, "\u{274C} Topilmadi.", Some(back("ch:list"))).await,
            }
        }
        "ext_name" => {
            let name = text.trim();
            if name.is_empty() || name.chars().count() > 40 {
                encbot_send(env, chat, "\u{274C} Nom 1-40 belgi bo'lsin.", None).await;
                return true;
            }
            config_put(env, WAIT_KEY, &format!("ext_url:{name}")).await;
            encbot_send(env, chat, &format!("\u{2705} \"{}\" qabul qilindi.\n\nEndi havolani (URL) yuboring:",
                html_escape(name)), None).await;
        }
        "ext_url" => match add(env, "external", text, arg, 0).await {
            Ok(_) => {
                config_put(env, WAIT_KEY, "").await;
                encbot_send(env, chat, &format!("\u{2705} \"{}\" havolasi saqlandi.", html_escape(arg)),
                    Some(menu_kb())).await;
            }
            Err(e) => encbot_send(env, chat, &format!("\u{274C} {}", html_escape(&e)), None).await,
        },
        _ => {
            config_put(env, WAIT_KEY, "").await;
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kanal_kiritmasi_tushuniladi() {
        assert_eq!(chat_ref("@arugram_news"), Some(json!("@arugram_news")));
        assert_eq!(chat_ref("https://t.me/arugram_news"), Some(json!("@arugram_news")));
        assert_eq!(chat_ref("-1001234567890"), Some(json!(-1001234567890i64)));
        assert_eq!(chat_ref("abc"), None);
        assert!(valid_url("https://instagram.com/aru"));
        assert!(!valid_url("instagram.com"));
        assert!(active(&json!({"need": 0, "joined": 9})));
        assert!(active(&json!({"need": 10, "joined": 9})));
        assert!(!active(&json!({"need": 10, "joined": 10})));
    }
}
