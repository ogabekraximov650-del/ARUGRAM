// ══════════════════════════════════════════════════════════════
//  YANGI TELEGRAM SESSIYASI — kodlash botidagi "🔑 Yangi sessiya"
// ══════════════════════════════════════════════════════════════
//
// TALAB (foydalanuvchi): panel botga "Yangi sessiya" tugmasi; bosilganda bot raqamni
// so'raydi; raqam yuboriladi; raqamga kod keladi; kod botga yuboriladi; yangi sessiya
// yaratilib SAQLANADI.
//
// QANDAY ISHLAYDI (worker MTProto'ni o'zi bilmaydi, kirishni GitHub Actions bajaradi):
//   1. Tugma -> `sess_state=phone`; bot raqam so'raydi.
//   2. Raqam -> `wait_send`; worker `session.yml` ni ishga tushiradi (avtoencode repoda).
//   3. Run (`tool/session/session.py`) `/api/session/poll` bilan holatni o'qiydi, raqamga
//      Telegram kodini yuboradi va `/api/session/report code_sent` deydi -> `code`.
//   4. Kod botga -> `wait_sign`; run kirishni tugatadi (2FA bo'lsa parol so'raladi).
//   5. Run yangi sessiyani `ENCODE_TOKEN` bilan shifrlab `tool/encode/session.enc` ga
//      commit qiladi (hamma workflow shu fayldan oladi) va `done` deydi.
//
// Holat `app_config` da: `sess_state`, `sess_phone`, `sess_code`, `sess_pw`, `sess_at`.
// Kod/parol xabarlari chatdan O'CHIRILADI. Telegram kodni oddiy matnda yuborilganda
// uni bekor qiladi — shu sabab raqamlar orasiga probel qo'yish so'raladi (probellar olinadi).

use super::*;

pub(crate) const BTN: &str = "\u{1F511} Yangi sessiya";
/// Workerga yangi Anibla akkauntini ulash (login + parol, `anibla::connect_account`).
pub(crate) const BTN_ANIBLA_ACC: &str = "\u{1F517} Anibla akkaunt";
const BTN_CANCEL: &str = "\u{274C} Bekor qilish";
const WORKFLOW: &str = "session.yml";
/// Jarayon shu vaqtdan keyin eskirgan hisoblanadi.
const TTL_MS: i64 = 20 * 60 * 1000;

async fn get(env: &Env, key: &str) -> String {
    config_get(env, key).await.unwrap_or_default()
}

async fn reset(env: &Env) {
    for k in ["sess_state", "sess_phone", "sess_code", "sess_pw", "sess_at", "sess_target"] {
        config_put(env, k, "").await;
    }
}

fn keyboard() -> Value {
    encbot_keyboard(vec![vec![BTN_CANCEL.to_string()], vec![postbot::BTN_HOME.to_string()]])
}

fn digits(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_digit()).collect()
}

/// "🔑 Yangi sessiya" bosildi: avval qaysi workflow uchun ekani so'raladi (har sifat-workflow'ning O'Z sessiyasi bor).
pub(crate) async fn start(env: &Env, chat: i64) {
    reset(env).await;
    config_put(env, "sess_chat", &chat.to_string()).await;
    config_put(env, "encbot_mode", "sess").await;
    config_put(env, "sess_state", "target").await;
    config_put(env, "sess_at", &now_ms().to_string()).await;
    // Hamma workflow BITTA umumiy sessiyada ishlaydi — qaysi workflow so'ralmaydi, to'g'ri raqamga o'tiladi.
    if true {
        ask_phone(env, chat, "shared").await;
        return;
    }
    let mut lines = String::new();
    for q in QUALITIES.iter() {
        let ok = encode_sess_ok(env, q).await;
        lines.push_str(&format!("{} <b>{q}</b>: {}\n", if ok { "\u{2705}" } else { "\u{274C}" },
            if ok { "ulangan" } else { "ULANMAGAN \u{2014} workflow ishga tushmaydi" }));
    }
    let kb = encbot_keyboard(vec![
        vec![format!("\u{1F511} {}", QUALITIES[0]), format!("\u{1F511} {}", QUALITIES[1])],
        vec![format!("\u{1F511} {}", QUALITIES[2]), format!("\u{1F511} {}", QUALITIES[3])],
        vec![BTN_CANCEL.to_string()], vec![postbot::BTN_HOME.to_string()],
    ]);
    encbot_send(env, chat, &format!(
        "\u{1F511} <b>Workflow sessiyalari</b>\n\n{lines}\n\
         Qaysi workflow uchun <b>yangi Telegram sessiya</b> ulaysiz? Tugmani bosing. Workflow shu sessiyasiz ishga tushmaydi; \
         u kodlangan fayllarni shu akkaunt orqali shu botga yuboradi."), Some(kb)).await;
}

async fn ask_phone(env: &Env, chat: i64, q: &str) {
    config_put(env, "sess_target", q).await;
    config_put(env, "sess_state", "phone").await;
    config_put(env, "sess_at", &now_ms().to_string()).await;
    encbot_send(env, chat, &format!(
        "\u{1F511} <b>{q}</b> workflow sessiyasi\n\n\
         \u{1F4F1} Telegram akkaunt <b>telefon raqamini</b> yuboring (masalan <code>+998901234567</code>).\n\
         Shu raqamga Telegram'dan kirish kodi keladi. Akkaunt yopiq kanalda a'zo/admin bo'lishi va shu sifatning log kanalida admin bo'lishi kerak."),
        Some(keyboard())).await;
}

/// "🔗 Anibla akkaunt" bosildi: login (telefon) so'raladi.
pub(crate) async fn start_anibla(env: &Env, chat: i64) {
    reset(env).await;
    config_put(env, "encbot_mode", "sess").await;
    config_put(env, "sess_state", "al_phone").await;
    config_put(env, "sess_at", &now_ms().to_string()).await;
    encbot_send(env, chat,
        "\u{1F517} <b>Anibla akkaunt</b>\n\n\
         anibla.uz dagi akkauntingiz <b>telefon raqamini</b> (login) yuboring, masalan <code>998901234567</code>.\n\
         Keyin parol so'raladi. Worker saytga kirib tekshiradi va akkauntni <b>shifrlab</b> saqlaydi.",
        Some(keyboard())).await;
}

async fn delete_msg(env: &Env, chat: i64, msg: &Value) {
    if let Some(id) = msg["message_id"].as_i64() {
        let _ = encbot_api(env, "deleteMessage", json!({"chat_id": chat, "message_id": id})).await;
    }
}

/// Sessiya rejimidagi xabar. `true` — xabar shu yerda qayta ishlandi.
pub(crate) async fn on_message(env: &Env, msg: &Value, chat: i64) -> bool {
    let text = msg["text"].as_str().unwrap_or("").trim().to_string();
    if text == BTN_CANCEL {
        reset(env).await;
        config_put(env, "encbot_mode", "").await;
        encbot_send(env, chat, "\u{274C} Sessiya yaratish bekor qilindi.", None).await;
        postbot::main_menu(env, chat).await;
        return true;
    }
    let started: i64 = get(env, "sess_at").await.parse().unwrap_or(0);
    let mut state = get(env, "sess_state").await;
    if !state.is_empty() && now_ms() - started > TTL_MS {
        reset(env).await;
        state.clear();
    }
    match state.as_str() {
        "target" => {
            if text.is_empty() {
                return true;
            }
            match QUALITIES.iter().find(|q| text.contains(**q)) {
                Some(q) => ask_phone(env, chat, q).await,
                None => encbot_send(env, chat, "\u{26A0}\u{FE0F} Pastdagi tugmalardan birini tanlang.", None).await,
            }
        }
        "phone" => {
            let d = digits(&text);
            if d.len() < 8 || d.len() > 15 {
                encbot_send(env, chat, "\u{26A0}\u{FE0F} Raqam noto'g'ri. Masalan: <code>+998901234567</code>", Some(keyboard())).await;
                return true;
            }
            config_put(env, "sess_phone", &d).await;
            config_put(env, "sess_state", "wait_send").await;
            config_put(env, "sess_at", &now_ms().to_string()).await;
            let note = dispatch(env).await;
            encbot_send(env, chat, &format!(
                "\u{23F3} Kod so'ralmoqda (<code>+{d}</code>). GitHub Actions ishga tushishi 1\u{2013}2 daqiqa oladi.\n{note}"),
                Some(keyboard())).await;
        }
        "code" => {
            let d = digits(&text);
            // Kod xabari darhol o'chiriladi (Telegram ochiq yozilgan kodni bekor qiladi).
            delete_msg(env, chat, msg).await;
            if d.len() < 4 || d.len() > 6 {
                encbot_send(env, chat, "\u{26A0}\u{FE0F} Kod 5 ta raqam. Raqamlar orasiga probel qo'yib yuboring: <code>1 2 3 4 5</code>", Some(keyboard())).await;
                return true;
            }
            config_put(env, "sess_code", &d).await;
            config_put(env, "sess_state", "wait_sign").await;
            encbot_send(env, chat, "\u{23F3} Kod tekshirilmoqda...", Some(keyboard())).await;
        }
        "password" => {
            delete_msg(env, chat, msg).await;
            if text.is_empty() {
                encbot_send(env, chat, "\u{26A0}\u{FE0F} Parolni yozib yuboring.", Some(keyboard())).await;
                return true;
            }
            config_put(env, "sess_pw", &text).await;
            config_put(env, "sess_state", "wait_pw").await;
            encbot_send(env, chat, "\u{23F3} Parol tekshirilmoqda...", Some(keyboard())).await;
        }
        "al_phone" => {
            let d = digits(&text);
            if d.len() < 9 || d.len() > 15 {
                encbot_send(env, chat, "\u{26A0}\u{FE0F} Raqam noto'g'ri. Masalan: <code>998901234567</code>", Some(keyboard())).await;
                return true;
            }
            config_put(env, "sess_phone", &d).await;
            config_put(env, "sess_state", "al_pw").await;
            encbot_send(env, chat, "\u{1F510} Endi anibla.uz <b>parolini</b> yuboring (xabar darhol o'chiriladi).", Some(keyboard())).await;
        }
        "al_pw" => {
            delete_msg(env, chat, msg).await;
            if text.is_empty() {
                encbot_send(env, chat, "\u{26A0}\u{FE0F} Parolni yozib yuboring.", Some(keyboard())).await;
                return true;
            }
            let phone = get(env, "sess_phone").await;
            match anibla::connect_account(env, &phone, &text).await {
                Ok(()) => {
                    reset(env).await;
                    config_put(env, "encbot_mode", "").await;
                    encbot_send(env, chat, &format!(
                        "\u{2705} <b>Anibla akkaunt ulandi</b> (<code>+{phone}</code>).\n\
                         Worker saytga kirdi; login/parol shifrlangan holda saqlandi va avvalgisidan USTUN turadi."),
                        None).await;
                    postbot::main_menu(env, chat).await;
                }
                Err(e) if e.contains("parol noto'g'ri") => {
                    encbot_send(env, chat, "\u{274C} Parol noto'g'ri. Qayta yuboring yoki \u{00AB}\u{274C} Bekor qilish\u{00BB}.", Some(keyboard())).await;
                }
                Err(e) => {
                    reset(env).await;
                    config_put(env, "encbot_mode", "").await;
                    encbot_send(env, chat, &format!("\u{274C} Akkaunt ulanmadi: {}", html_escape(&e)), None).await;
                    postbot::main_menu(env, chat).await;
                }
            }
        }
        "" => {
            encbot_send(env, chat, "\u{1F511} Boshlash uchun \u{00AB}\u{1F511} Yangi sessiya\u{00BB} tugmasini bosing (Bosh menyu).", None).await;
        }
        _ => {
            encbot_send(env, chat, "\u{23F3} Jarayon ketmoqda, biroz kuting. To'xtatish: \u{00AB}\u{274C} Bekor qilish\u{00BB}.", Some(keyboard())).await;
        }
    }
    true
}

/// `session.yml` ni ishga tushiradi (avtoencode repoda). Natija matni.
async fn dispatch(env: &Env) -> String {
    let repo = tg_secret(env, "GH_REPO");
    if !repo.contains('/') {
        return "\u{26A0}\u{FE0F} GH_REPO o'rnatilmagan \u{2014} Actions ishga tushirilmadi.".into();
    }
    match gh_workflow_busy(env, &repo, WORKFLOW).await {
        Ok(true) => return "\u{26A0}\u{FE0F} Oldingi sessiya jarayoni hali ishlayapti. Bir necha daqiqadan keyin qayta urinib ko'ring.".into(),
        Ok(false) => {}
        Err(e) => return e,
    }
    match gh_api(env, Method::Post,
        &format!("/repos/{repo}/actions/workflows/{WORKFLOW}/dispatches"),
        Some(json!({"ref": "main"}))).await {
        Ok((204, _)) => String::new(),
        Ok((code, v)) => format!("\u{26A0}\u{FE0F} Actions ishga tushmadi (GitHub {code}): {}",
            html_escape(v["message"].as_str().unwrap_or(""))),
        Err(e) => format!("\u{26A0}\u{FE0F} Actions ishga tushmadi: {}", html_escape(&e.to_string())),
    }
}

/// Actions run'i uchun (`tool/session/session.py`): `poll` va `report`.
pub(crate) async fn route(mut req: Request, env: &Env, path: &str, method: Method) -> Result<Response> {
    if !encode_token_ok(&req, env) {
        return json_resp(&json!({"error": "unauthorized"}), 401);
    }
    if method != Method::Post {
        return err404("topilmadi");
    }
    let b: Value = req.json().await.unwrap_or(json!({}));
    // Run xabarlari jarayonni boshlagan adminga boradi (bir nechta admin bo'lishi mumkin).
    let admin: i64 = get(env, "sess_chat").await.parse().ok().filter(|c| is_encbot_admin(*c)).unwrap_or(ADMIN_TELEGRAM_ID);
    let started: i64 = get(env, "sess_at").await.parse().unwrap_or(0);
    let mut state = get(env, "sess_state").await;
    if !state.is_empty() && now_ms() - started > TTL_MS {
        reset(env).await;
        state.clear();
    }

    if path == "/api/session/poll" {
        return ok(json!({
            "state": state,
            "phone": get(env, "sess_phone").await,
            "code": get(env, "sess_code").await,
            "pw": get(env, "sess_pw").await,
            "target": get(env, "sess_target").await,
            "bot": bot_username(env).await,
            // Run'dan kelgan oxirgi xabar (tashxis uchun; `reset` tegmaydi).
            "last": get(env, "sess_last").await,
        }));
    }

    if path == "/api/session/report" {
        let event = b["event"].as_str().unwrap_or("");
        let text = html_escape(b["text"].as_str().unwrap_or("").trim());
        config_put(env, "sess_last", &format!("{} | {event} | {} | state={state}", now_ms() / 1000,
            b["text"].as_str().unwrap_or("").chars().take(300).collect::<String>())).await;
        match event {
            "code_sent" if state == "wait_send" => {
                config_put(env, "sess_state", "code").await;
                config_put(env, "sess_at", &now_ms().to_string()).await;
                encbot_send(env, admin, &format!(
                    "\u{1F4E9} <b>Kod yuborildi</b> {text}\n\n\
                     Telegram'da kelgan <b>kodni</b> shu yerga yuboring.\n\
                     \u{26A0}\u{FE0F} Kodni ulanayotgan akkauntning <b>o'zidan</b> yuborsangiz Telegram uni bloklaydi. Ikkita yo'l: \
                     (1) kodni <b>boshqa admin akkauntdan</b> yuboring; yoki (2) raqamlar orasiga <b>harf</b> qo'ying: <code>1a2b3c4d5e</code> \
                     (harflar olib tashlanadi)."), Some(keyboard())).await;
            }
            // Run bosqichlari haqida xabar (foydalanuvchi jarayonni ko'rib tursin).
            "info" if !state.is_empty() => {
                encbot_send(env, admin, &text, Some(keyboard())).await;
            }
            "bad_code" => {
                config_put(env, "sess_code", "").await;
                config_put(env, "sess_state", "code").await;
                encbot_send(env, admin, "\u{274C} Kod noto'g'ri. Qayta yuboring (probel bilan: <code>1 2 3 4 5</code>).", Some(keyboard())).await;
            }
            "need_password" => {
                config_put(env, "sess_state", "password").await;
                encbot_send(env, admin, "\u{1F510} Akkauntda <b>ikki bosqichli parol</b> (2FA) bor. Parolni yuboring (xabar o'chiriladi).", Some(keyboard())).await;
            }
            "bad_password" => {
                config_put(env, "sess_pw", "").await;
                config_put(env, "sess_state", "password").await;
                encbot_send(env, admin, "\u{274C} Parol noto'g'ri. Qayta yuboring.", Some(keyboard())).await;
            }
            "done" => {
                // Shu sifatning sessiyasi ulandi: yuboruvchi akkaunt ID'si (fayl qabul qilish) va "ulangan" belgisi.
                let target = get(env, "sess_target").await;
                let uid = b["uid"].as_i64().unwrap_or(0);
                if QUALITIES.contains(&target.as_str()) && uid > 0 {
                    config_put(env, &format!("sess_uid_{target}"), &uid.to_string()).await;
                    config_put(env, &format!("sess_ok_{target}"), "1").await;
                }
                reset(env).await;
                config_put(env, "encbot_mode", "").await;
                encbot_send(env, admin, &format!(
                    "\u{2705} <b>{target} sessiyasi ulandi</b> {text}\n\
                     Endi {target} workflow shu sessiya bilan ishlaydi (keyingi run'dan)."),
                    None).await;
                postbot::main_menu(env, admin).await;
            }
            "error" => {
                reset(env).await;
                config_put(env, "encbot_mode", "").await;
                encbot_send(env, admin, &format!("\u{274C} Sessiya yaratilmadi: {text}"), None).await;
                postbot::main_menu(env, admin).await;
            }
            _ => {}
        }
        return ok(json!({"ok": true}));
    }
    err404("topilmadi")
}
