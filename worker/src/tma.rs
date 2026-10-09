// worker/src/tma.rs — TELEGRAM MINI APP (arumediatv.pages.dev) UCHUN KIRISH
//
// TALAB (foydalanuvchi): ilovaning onlayn nusxasi Telegram Mini App
// bo'lib ishlasin (`web/`), asosiy kirish botiga /start yuborilganda
// uni ochadigan tugma chiqsin.
//
// ── MUAMMO ─────────────────────────────────────────────────────
//
// Worker faqat ilovaga javob beradi (`app_gate`): har so'rovda
// `X-App-Sig` — ilova ichidagi sir bilan yasalgan imzo. Brauzerdagi
// sahifada bu sir bo'lishi MUMKIN EMAS (sahifa kodi hammaga ochiq).
//
// ── YECHIM ─────────────────────────────────────────────────────
//
// Telegram Mini App'ni ochganda unga `initData` beradi — foydalanuvchi
// ma'lumoti va BOT TOKENI bilan yasalgan imzo. Uni soxtalashtirib
// bo'lmaydi (bot tokeni faqat bizda). Shu sabab:
//
//   1. Sahifa `POST /api/tma/auth` ga `initData` ni yuboradi.
//   2. Worker imzoni tekshiradi (Telegram hujjatidagi usul) va
//      12 soatlik QISQA TOKEN beradi: `<uid>.<muddat>.<hex HMAC>`.
//   3. Sahifa keyingi so'rovlarda tokenni `X-Tma` sarlavhasida,
//      rasmlarda esa manzilda (`?tma=`) yuboradi — `<img>` ga
//      sarlavha qo'shib bo'lmaydi.
//
// Token kaliti — `APP_SIGN_SECRET` (bo'lmasa bot tokeni). U faqat
// worker'da turadi.

use super::*;

/// Mini App manzili (bot tugmasi shu yerni ochadi).
pub(crate) const WEB_URL: &str = "https://arumediatv.pages.dev";

/// Token muddati: 12 soat.
const TOKEN_SECS: i64 = 12 * 3600;

/// `initData` shundan eski bo'lsa qabul qilinmaydi (24 soat).
const INIT_MAX_AGE_SECS: i64 = 24 * 3600;

fn mac_hex(key: &[u8], msg: &[u8]) -> Option<String> {
    let mut h = <hmac::Hmac<sha2::Sha256> as hmac::Mac>::new_from_slice(key).ok()?;
    hmac::Mac::update(&mut h, msg);
    Some(hex_of(&hmac::Mac::finalize(h).into_bytes()))
}

fn mac_raw(key: &[u8], msg: &[u8]) -> Option<Vec<u8>> {
    let mut h = <hmac::Hmac<sha2::Sha256> as hmac::Mac>::new_from_slice(key).ok()?;
    hmac::Mac::update(&mut h, msg);
    Some(hmac::Mac::finalize(h).into_bytes().to_vec())
}

fn token_key(env: &Env) -> String {
    let s = env.secret("APP_SIGN_SECRET").map(|s| s.to_string().trim().to_string()).unwrap_or_default();
    if !s.is_empty() {
        return s;
    }
    env.secret("TELEGRAM_BOT_TOKEN").map(|s| s.to_string().trim().to_string()).unwrap_or_default()
}

/// Telegram `initData` ni tekshiradi; to'g'ri bo'lsa — foydalanuvchi.
///
/// Usul (core.telegram.org/bots/webapps): `hash` dan boshqa juftliklar
/// kalit bo'yicha saralanib `\n` bilan qo'shiladi; kalit =
/// HMAC_SHA256("WebAppData", bot_token); imzo = HMAC_SHA256(kalit, matn).
fn check_init_data(bot_token: &str, init: &str) -> Option<Value> {
    let url = Url::parse(&format!("http://x/?{init}")).ok()?;
    let mut pairs: Vec<(String, String)> = url.query_pairs().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    let hash = pairs.iter().find(|(k, _)| k == "hash")?.1.clone();
    pairs.retain(|(k, _)| k != "hash");
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    let dcs = pairs.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("\n");
    let secret = mac_raw(b"WebAppData", bot_token.as_bytes())?;
    let want = mac_hex(&secret, dcs.as_bytes())?;
    if want.len() != hash.len() || !want.eq_ignore_ascii_case(&hash) {
        return None;
    }
    let date = pairs.iter().find(|(k, _)| k == "auth_date")?.1.parse::<i64>().ok()?;
    if now_ms() / 1000 - date > INIT_MAX_AGE_SECS {
        return None;
    }
    let user: Value = serde_json::from_str(&pairs.iter().find(|(k, _)| k == "user")?.1).ok()?;
    user["id"].as_i64().filter(|id| *id > 0)?;
    Some(user)
}

fn make_token(env: &Env, uid: i64) -> Option<String> {
    let exp = now_ms() / 1000 + TOKEN_SECS;
    let mac = mac_hex(token_key(env).as_bytes(), format!("tma.{uid}.{exp}").as_bytes())?;
    Some(format!("{uid}.{exp}.{mac}"))
}

/// Token to'g'ri va muddati o'tmagan bo'lsa — Telegram ID.
pub(crate) fn token_uid(env: &Env, token: &str) -> Option<i64> {
    let mut p = token.split('.');
    let (uid, exp, mac) = (p.next()?, p.next()?, p.next()?);
    if p.next().is_some() {
        return None;
    }
    let uid_n = uid.parse::<i64>().ok()?;
    let exp_n = exp.parse::<i64>().ok()?;
    if now_ms() / 1000 > exp_n {
        return None;
    }
    let key = token_key(env);
    if key.is_empty() {
        return None;
    }
    let want = mac_hex(key.as_bytes(), format!("tma.{uid}.{exp}").as_bytes())?;
    (want.len() == mac.len() && want.eq_ignore_ascii_case(mac)).then_some(uid_n)
}

/// So'rovda Mini App tokeni bormi (sarlavhada yoki `?tma=`) va to'g'rimi.
pub(crate) fn request_ok(req: &Request, env: &Env) -> bool {
    let mut t = req.headers().get("X-Tma").ok().flatten().unwrap_or_default();
    if t.is_empty() {
        t = req.url().ok()
            .and_then(|u| u.query_pairs().find(|(k, _)| k == "tma").map(|(_, v)| v.to_string()))
            .unwrap_or_default();
    }
    !t.is_empty() && token_uid(env, t.trim()).is_some()
}

/// `POST /api/tma/auth` — `{"init_data": "..."}` -> `{"token", "user"}`.
pub(crate) async fn auth(mut req: Request, env: &Env) -> Result<Response> {
    let b: Value = req.json().await.unwrap_or(json!({}));
    let init = b["init_data"].as_str().unwrap_or("").trim().to_string();
    let bot = env.secret("TELEGRAM_BOT_TOKEN").map(|s| s.to_string().trim().to_string()).unwrap_or_default();
    if init.is_empty() || bot.is_empty() {
        return json_resp(&json!({"error": "bad_request"}), 400);
    }
    let Some(user) = check_init_data(&bot, &init) else {
        return json_resp(&json!({"error": "forbidden"}), 403);
    };
    let uid = user["id"].as_i64().unwrap_or(0);
    let Some(token) = make_token(env, uid) else {
        return err500("token");
    };
    ok(json!({
        "token": token,
        "expires_in": TOKEN_SECS,
        "user": {
            "id": uid,
            "first_name": user["first_name"],
            "last_name": user["last_name"],
            "username": user["username"],
            "photo_url": user["photo_url"],
        },
    }))
}

/// Mini App'ni ochadigan tugma (xabar ostida).
pub(crate) fn open_button() -> Value {
    json!({"inline_keyboard": [[{
        "text": "\u{1F4FA} ARUmediaTV'ni ochish",
        "web_app": {"url": WEB_URL},
    }]]})
}
