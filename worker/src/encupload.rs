// ══════════════════════════════════════════════════════════════
//  KODLASH FAYLLARI BOT ORQALI: workflow -> bot chati -> yopiq kanal -> baza
// ══════════════════════════════════════════════════════════════
//
// TALAB (foydalanuvchi): sifat-workflow'lar tayyor faylni KANALGA emas, ASOSIY fayl uzatuvchi botning CHATIGA yuboradi; bot uni yopiq
// kanalga saqlaydi va bazaga yozadi; shundan keyin workflow o'chadi.
//
// QANDAY ISHLAYDI:
//   1. Run (o'z Telegram sessiyasi bilan) shifrlangan faylni bot chatiga hujjat qilib yuboradi. Izoh (caption):
//        `ARUENC|<mp4|fmp4>|anime|bo'lim|qism|queued_at|sifat|kalit|hajm`
//      Fayl nomi esa `ep_<a>_<s>_<e>_<q>_<queued_at>.mp4` (fMP4 uchun `..._f.mp4`).
//   2. Asosiy botning webhook'i xabarni oladi (`handle_tg_webhook`) (`on_file`). Yuboruvchi shu sifatning RO'YXATDAN O'TGAN sessiyasi bo'lishi shart
//      (`sess_uid_<q>`, "🔑 Yangi sessiya" da yoziladi) — boshqa odam fayl yubora olmaydi.
//   3. Fayl yopiq kanalga `copyMessage` bilan ko'chadi (qayta yuklanmaydi), `tg_files` va qism yozuviga yoziladi
//      (`encode_commit_quality` / `fmp4::commit`), chatdagi xabar o'chadi.
//   4. Run `/api/encode/committed?file=` bilan tasdiqni kutadi va tugaydi.
// Fayl baytlari worker orqali O'TMAYDI — faqat Telegram ichida ko'chadi.

use super::*;

/// Faylning bazaga yozilganini tekshirish: `tg_files` da shu nom bor (nom har `queued_at` uchun noyob).
pub(crate) async fn committed(env: &Env, file: &str) -> bool {
    if !tg_safe_name(file) {
        return false;
    }
    turso_exec(env, "SELECT 1 AS x FROM tg_files WHERE file_name=? AND msg_id>0", vec![TursoArg::text(file)]).await
        .ok().and_then(|r| first_row(&r)).is_some()
}

/// Bot chatiga kelgan xabar `ARUENC|` izohli hujjatmi — shuni qayta ishlaydi. `true` — xabar shu yerda tugadi.
pub(crate) async fn on_file(env: &Env, msg: &Value) -> bool {
    let caption = msg["caption"].as_str().unwrap_or("");
    if caption.starts_with("ARUDLV|") {
        return on_deliver(env, msg).await;
    }
    if !caption.starts_with("ARUENC|") || !msg["document"].is_object() {
        return false;
    }
    let chat = msg["chat"]["id"].as_i64().unwrap_or(0);
    let from = msg["from"]["id"].as_i64().unwrap_or(0);
    let msg_id = msg["message_id"].as_i64().unwrap_or(0);
    let p: Vec<&str> = caption.split('|').collect();
    if p.len() != 9 || chat == 0 || from == 0 || chat != from || msg_id == 0 {
        return true;
    }
    let (kind, key, q) = (p[1], p[7].to_ascii_lowercase(), p[6]);
    let n = |i: usize| p[i].parse::<i64>().unwrap_or(0);
    let (a, s, e, queued_at, size) = (n(2), n(3), n(4), n(5), n(8));
    let name = msg["document"]["file_name"].as_str().unwrap_or("").to_string();
    if !QUALITIES.contains(&q) || !valid_file_key(&key) || size <= 0 || a <= 0 || s <= 0 || e <= 0 || queued_at <= 0 {
        return true;
    }
    // Yuboruvchi — shu sifatning yoki R2'dan yuklovchi workflow'ning ro'yxatdan o'tgan sessiyasi.
    let me = from.to_string();
    let by_q = config_get(env, &format!("sess_uid_{q}")).await;
    let by_up = if by_q.as_deref() == Some(me.as_str()) { None } else { config_get(env, "sess_uid_upload").await };
    let in_all = match config_get(env, "sess_uid_upload_all").await {
        Some(l) => l.split(',').any(|x| x == me),
        None => false,
    };
    if by_q.as_deref() != Some(me.as_str()) && by_up.as_deref() != Some(me.as_str()) && !in_all {
        return true;
    }
    let name_ok = match kind {
        "mp4" => name == encode_file_name(a, s, e, q, queued_at),
        "fmp4" => fmp4::valid_name(&name, a, s, e, q),
        _ => false,
    };
    // Ish hali mavjudmi (o'chirilgan/almashtirilgan qismga fayl yozilmasin).
    let job = turso_exec(env,
        "SELECT 1 AS x FROM encode_jobs WHERE anime_id=? AND season_id=? AND epizod_id=? AND queued_at=?",
        vec![TursoArg::int(a), TursoArg::int(s), TursoArg::int(e), TursoArg::int(queued_at)]).await
        .ok().and_then(|r| first_row(&r)).is_some();
    // R2 orqali kelgan fayl: ish allaqachon tugagan (qator o'chgan) bo'lishi mumkin — ruxsat R2 qatoridan.
    let r2row = r2::row(env, &name).await.is_some();
    let channel = tg_channel_id(env);
    if !name_ok || !(job || r2row) || channel == 0 {
        let _ = tg_api(env, "deleteMessage", json!({"chat_id": chat, "message_id": msg_id})).await;
        return true;
    }
    let copied = tg_api(env, "copyMessage", json!({
        "chat_id": channel, "from_chat_id": chat, "message_id": msg_id,
        "caption": name, "disable_notification": true,
    })).await;
    let ch_msg = copied.ok().and_then(|c| c["message_id"].as_i64()).unwrap_or(0);
    if ch_msg <= 0 {
        // Run tasdiqni kutib, vaqt tugagach qayta yuboradi; xabar chatda qoladi (qayta nusxalash mumkin).
        tg_send(env, ADMIN_TELEGRAM_ID, &format!(
            "\u{274C} {q}: <code>{}</code> yopiq kanalga ko'chmadi (bot kanalda admin ekanini tekshiring).",
            html_escape(&name))).await;
        return true;
    }
    let res = if kind == "mp4" {
        let done = turso_exec(env, "SELECT done FROM encode_jobs WHERE anime_id=? AND season_id=? AND epizod_id=? AND queued_at=?",
            vec![TursoArg::int(a), TursoArg::int(s), TursoArg::int(e), TursoArg::int(queued_at)]).await
            .ok().and_then(|r| first_row(&r)).and_then(|r| r["done"].as_str().map(String::from)).unwrap_or_default();
        encode_commit_quality(env, (a, s, e), queued_at, q, &name, &key, size, ch_msg, true, &done).await.map(|_| ())
    } else {
        fmp4::commit(env, (a, s, e), q, &name, &key, size, ch_msg).await
    };
    match res {
        Ok(()) => {
            // Baza to'liq yozildi: chatdagi fayl, R2 obyekti va qator o'chadi; qism tugagan bo'lsa tozalanadi.
            let _ = tg_api(env, "deleteMessage", json!({"chat_id": chat, "message_id": msg_id})).await;
            r2::finish(env, &name).await;
            encode_settle(env, a, s, e, queued_at).await;
        }
        Err(err) => {
            // Bazaga yozilmadi: kanaldagi nusxa yetim qolmasin.
            let _ = tg_api(env, "deleteMessage", json!({"chat_id": channel, "message_id": ch_msg})).await;
            tg_send(env, ADMIN_TELEGRAM_ID, &format!(
                "\u{274C} {q}: <code>{}</code> bazaga yozilmadi: {}", html_escape(&name), html_escape(&err.to_string()))).await;
        }
    }
    true
}

/// Anibla/To'plam fayli yopiq kanalga: yuklovchi (asosiy akkauntsiz) faylni shu botning chatiga `ARUDLV|<r2 nomi>` izohi bilan yuboradi;
/// bot uni yopiq kanalga `copyMessage` qiladi (media turi va tavsif saqlanadi), xabar raqamini `dlv_msg_<nom>` ga yozadi (yuklovchi
/// `/api/r2/dmsg` bilan oladi va egasining yo'lini chaqiradi), chatdagi nusxa o'chadi.
async fn on_deliver(env: &Env, msg: &Value) -> bool {
    let name = msg["caption"].as_str().unwrap_or("").trim_start_matches("ARUDLV|").trim().to_string();
    let chat = msg["chat"]["id"].as_i64().unwrap_or(0);
    let from = msg["from"]["id"].as_i64().unwrap_or(0);
    let msg_id = msg["message_id"].as_i64().unwrap_or(0);
    if chat == 0 || from == 0 || chat != from || msg_id == 0 || !tg_safe_name(&name)
        || !(msg["document"].is_object() || msg["video"].is_object()) {
        return true;
    }
    let me = from.to_string();
    let mut ok_sender = config_get(env, "sess_uid_upload").await.as_deref() == Some(me.as_str());
    if !ok_sender {
        ok_sender = config_get(env, "sess_uid_upload_all").await.map(|l| l.split(',').any(|x| x == me)).unwrap_or(false);
    }
    if !ok_sender {
        return true;
    }
    let channel = tg_channel_id(env);
    let Some(r) = r2::row(env, &name).await else {
        let _ = tg_api(env, "deleteMessage", json!({"chat_id": chat, "message_id": msg_id})).await;
        return true;
    };
    let dest: Value = serde_json::from_str(r["dest"].as_str().unwrap_or("{}")).unwrap_or(json!({}));
    // Faqat bizning yopiq kanalga (boshqa manzilga nusxalanmaydi).
    if channel == 0 || r["kind"].as_str() != Some("deliver") || dest["to"].as_i64() != Some(channel) {
        let _ = tg_api(env, "deleteMessage", json!({"chat_id": chat, "message_id": msg_id})).await;
        return true;
    }
    let caption: String = dest["caption"].as_str().unwrap_or("").chars().take(1024).collect();
    let copied = tg_api(env, "copyMessage", json!({
        "chat_id": channel, "from_chat_id": chat, "message_id": msg_id,
        "caption": caption, "disable_notification": true,
    })).await;
    let ch_msg = copied.ok().and_then(|c| c["message_id"].as_i64()).unwrap_or(0);
    if ch_msg <= 0 {
        tg_send(env, ADMIN_TELEGRAM_ID, &format!(
            "\u{274C} <code>{}</code> yopiq kanalga ko'chmadi (bot kanalda admin ekanini tekshiring).", html_escape(&name))).await;
        return true;
    }
    config_put(env, &format!("dlv_msg_{name}"), &ch_msg.to_string()).await;
    let _ = tg_api(env, "deleteMessage", json!({"chat_id": chat, "message_id": msg_id})).await;
    true
}
