// ══════════════════════════════════════════════════════════════
//  SINOV: Telegram "Rich Message" (Bot API 10.x) — bir nechta rasm + tugma BITTA xabarda chiqadimi?
// ══════════════════════════════════════════════════════════════
//
// Faqat admin: kodlash botiga `/richtest` yuboriladi. Bot admin chatiga bir necha variantni yuboradi (3 ta kichik sinov rasmi bilan, kanalga HECH NIMA
// chiqmaydi) va har birining natijasini (OK yoki Telegram xato matni) o'sha chatga hamda `app_config.richtest` ga yozadi.
// Natija ma'qul bo'lsa kanal posti (`announce.rs`) rich message'ga o'tkaziladi; bo'lmasa bu fayl o'chiriladi.

use super::*;

const IMGS: [&[u8]; 3] = [
    include_bytes!("../assets/rt0.jpg"),
    include_bytes!("../assets/rt1.jpg"),
    include_bytes!("../assets/rt2.jpg"),
];

fn photo_blocks() -> Vec<Value> {
    (0..IMGS.len()).map(|i| json!({"type": "photo", "photo": {"type": "photo", "media": format!("attach://f{i}")}})).collect()
}

fn all_blocks(bot: &str) -> Vec<Value> {
    let mut v = photo_blocks();
    v.push(buttons_block(bot));
    v
}

fn buttons_block(bot: &str) -> Value {
    json!({"type": "buttons", "buttons": [{"text": "Tomosha qilish", "url": format!("https://t.me/{bot}")}]})
}

/// Bitta variantni yuboradi: `rich` — InputRichMessage; rasmlar `attach://f0..f2` bilan multipart. Qaytadi: (ok, matn).
async fn send_variant(env: &Env, chat: i64, rich: &Value) -> (bool, String) {
    let token = encbot_token(env);
    let go = || -> std::result::Result<web_sys::FormData, String> {
        let fd = web_sys::FormData::new().map_err(|_| "FormData".to_string())?;
        fd.append_with_str("chat_id", &chat.to_string()).map_err(|_| "FormData".to_string())?;
        fd.append_with_str("rich_message", &rich.to_string()).map_err(|_| "FormData".to_string())?;
        for (i, f) in IMGS.iter().enumerate() {
            let arr = js_sys::Uint8Array::from(*f);
            let parts = js_sys::Array::new();
            parts.push(&arr);
            let blob = web_sys::Blob::new_with_u8_array_sequence(&parts).map_err(|_| "Blob".to_string())?;
            fd.append_with_blob_and_filename(&format!("f{i}"), &blob, &format!("rt{i}.jpg")).map_err(|_| "FormData".to_string())?;
        }
        Ok(fd)
    };
    let fd = match go() { Ok(f) => f, Err(e) => return (false, e) };
    let req = match Request::new_with_init(
        &format!("https://api.telegram.org/bot{token}/sendRichMessage"),
        RequestInit::new().with_method(Method::Post).with_body(Some(fd.into())),
    ) { Ok(r) => r, Err(e) => return (false, e.to_string()) };
    let mut r = match Fetch::Request(req).send().await { Ok(r) => r, Err(e) => return (false, e.to_string()) };
    match r.json::<Value>().await {
        Ok(v) if v["ok"] == json!(true) => (true, format!("OK (xabar {})", v["result"]["message_id"])),
        Ok(v) => (false, v["description"].as_str().unwrap_or("noma'lum xato").to_string()),
        Err(e) => (false, e.to_string()),
    }
}

pub(crate) async fn run(env: &Env, chat: i64) {
    if chat != ADMIN_TELEGRAM_ID {
        return;
    }
    let bot = bot_username(env).await;
    let bot = if bot.is_empty() { "telegram".to_string() } else { bot };
    let lines = ["\u{2022} Sinov anime nomi", "\u{2022} 5-qism", "\u{2022} Ovoz berdi: Sinov tarjimon"];
    let text_blocks = || -> Vec<Value> { lines.iter().map(|l| json!({"type": "paragraph", "text": l})).collect() };
    // Haqiqiy post tartibi: rasmlar -> matn qatorlari -> tugma.
    let real = |kind: &str| {
        let mut b = vec![json!({"type": kind, "blocks": photo_blocks()})];
        b.extend(text_blocks());
        b.push(buttons_block(&bot));
        json!({"blocks": b})
    };
    let variants: Vec<(&str, Value)> = vec![
        ("A) slideshow -> 3 qator matn -> tugma (haqiqiy post tartibi)", real("slideshow")),
        ("B) collage -> 3 qator matn -> tugma", real("collage")),
        ("C) slideshow -> bitta matn (qatorlar \\n bilan) -> tugma", json!({"blocks": [
            {"type": "slideshow", "blocks": photo_blocks()},
            {"type": "paragraph", "text": lines.join("\n")}, buttons_block(&bot)]})),
    ];
    let mut report = String::from("Rich Message sinovi:\n");
    for (name, rich) in &variants {
        let (ok, msg) = send_variant(env, chat, rich).await;
        report.push_str(&format!("{} {name}: {msg}\n", if ok { "✅" } else { "❌" }));
    }
    config_put(env, "richtest", &report).await;
    encbot_send(env, chat, &html_escape(&report), None).await;
}
