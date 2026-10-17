// worker/src/emojifont.rs — EMOJI SHRIFTI (`TgEmoji.ttf`, ~7,4 MB) KANALDA.
//
// TALAB (foydalanuvchi): shrift APK ichida turmasin (APK 7-8 MB ga kattalashyapti) — fayl bo'lib yopiq kanalda
// (AES-128-CTR bilan shifrlangan) tursin; ilova emoji oynasi ochilganda uni kanaldan yuklab olsin. Agar emojilar
// 5 ta urinishda ochilmasa — worker Actions'ni uyg'otsin, u faylni yangisiga almashtirsin.
//
// ── QANDAY ISHLAYDI ───────────────────────────────────────────
//   * Fayl `sys_emojifont_<versiya>.bin` nomi bilan kanalda (`tg_files`: xabar raqami + kalit). Ilova uni boshqa fayllar
//     kabi `/api/tg/deliver` orqali bot chatiga oladi (kalit ham shu javobda keladi).
//   * `config` `emoji_font` = `{"name","size","ver"}` — hozirgi fayl. Yo'q bo'lsa ilova so'raganda worker darhol
//     workflow'ni ishga tushiradi (birinchi yuklash).
//   * Ilova yuklay olmasa `POST /api/emojifont/fail` yuboradi. Worker urinishlarni sanaydi; `FAILS_KICK` ga yetganda
//     `emojifont.yml` ni ishga tushiradi (3 daqiqada bir martadan ko'p emas) va sanagichni nolga tushiradi.
//   * Actions (`tool/emojifont/run.py`): shriftni yangi kalit bilan shifrlab kanalga yuboradi va `job/commit` ni
//     chaqiradi — worker `tg_files` ga yozadi, `emoji_font` ni almashtiradi, eski faylni kanaldan o'chiradi.
//
// Fayl baytlari worker orqali O'TMAYDI.

use super::*;

const GH_EMOJIFONT_WORKFLOW: &str = "emojifont.yml";
/// Shuncha muvaffaqiyatsiz urinishdan keyin Actions uyg'otiladi.
const FAILS_KICK: i64 = 5;
const KICK_GAP_MS: i64 = 3 * 60 * 1000;

fn arg_i(v: i64) -> TursoArg { TursoArg::int(v) }
fn arg_s(v: &str) -> TursoArg { TursoArg::text(v) }

/// `sys_emojifont_<versiya>.bin`
fn font_name_ok(name: &str, ver: i64) -> bool {
    ver > 0 && name == format!("sys_emojifont_{ver}.bin")
}

async fn current(env: &Env) -> Option<Value> {
    let raw = config_get_db(env, "emoji_font").await?;
    let v: Value = serde_json::from_str(&raw).ok()?;
    let ver = jint(&v, "ver");
    let name = v["name"].as_str().unwrap_or("");
    if font_name_ok(name, ver) && jint(&v, "size") > 0 { Some(v) } else { None }
}

/// Actions'ni ishga tushiradi (3 daqiqada bir martadan ko'p emas, ishlayotgan bo'lsa tegmaydi). Natija matni.
async fn kick(env: &Env) -> String {
    let repo = tg_secret(env, "GH_REPO");
    if !repo.contains('/') {
        return "GH_REPO o'rnatilmagan".into();
    }
    for st in ["queued", "in_progress", "waiting", "requested", "pending"] {
        match gh_api(env, Method::Get,
            &format!("/repos/{repo}/actions/workflows/{GH_EMOJIFONT_WORKFLOW}/runs?status={st}&per_page=1"), None).await {
            Ok((200, v)) if v["total_count"].as_i64().unwrap_or(0) > 0 => {
                return "Shrift workflow'i ishlayapti yoki kutmoqda".into();
            }
            Ok((200, _)) => {}
            Ok((code, _)) => return format!("GitHub javobi {code}"),
            Err(e) => return format!("Actions ishga tushmadi: {e}"),
        }
    }
    let now = now_ms();
    let claim = turso_exec(env,
        "INSERT INTO app_config (cfg_key,cfg_value) VALUES ('emoji_font_kicked_at', ?)
         ON CONFLICT(cfg_key) DO UPDATE SET cfg_value=excluded.cfg_value
           WHERE CAST(app_config.cfg_value AS INTEGER) < ?
         RETURNING cfg_key",
        vec![arg_s(&now.to_string()), arg_i(now - KICK_GAP_MS)]).await;
    if !claim.ok().and_then(|r| first_row(&r)).is_some() {
        return "Shrift workflow'i hozirgina ishga tushirilgan".into();
    }
    config_put(env, "emoji_font_want", "1").await;
    match gh_api(env, Method::Post,
        &format!("/repos/{repo}/actions/workflows/{GH_EMOJIFONT_WORKFLOW}/dispatches"),
        Some(json!({"ref": "main"}))).await {
        Ok((204, _)) => "Shrift workflow'i ishga tushirildi".into(),
        Ok((code, _)) => format!("Actions ishga tushmadi (GitHub {code})"),
        Err(e) => format!("Actions ishga tushmadi: {e}"),
    }
}

pub(crate) async fn route(mut req: Request, env: &Env, path: &str, method: Method) -> Result<Response> {
    // ── ACTIONS (maxfiy kalit bilan) ─────────────────────────
    if path.starts_with("/api/emojifont/job/") {
        if !encode_token_ok(&req, env) {
            return json_resp(&json!({"error": "unauthorized"}), 401);
        }
        if method != Method::Post {
            return err404("topilmadi");
        }
        let b: Value = req.json().await.unwrap_or(json!({}));
        return match path {
            // Workflow ishlashi kerakmi: shrift hali yo'q yoki almashtirish so'ralgan.
            "/api/emojifont/job/peek" => {
                let need = current(env).await.is_none()
                    || config_get_db(env, "emoji_font_want").await.as_deref() == Some("1");
                ok(json!({"need": need, "channel": tg_channel_id(env)}))
            }
            "/api/emojifont/job/commit" => commit(env, &b).await,
            _ => err404("topilmadi"),
        };
    }

    // ── ILOVA (sessiya bilan) ────────────────────────────────
    let Some(u) = session_user(env, &bearer(&req)).await? else {
        return json_resp(&json!({"error": "unauthorized"}), 401);
    };
    if u["id"].as_i64().unwrap_or(0) <= 0 {
        return json_resp(&json!({"error": "unauthorized"}), 401);
    }
    match (method, path) {
        // Hozirgi shrift: `{name,size,ver}` yoki `{}` (hali yuklanmagan — worker Actions'ni uyg'otadi).
        (Method::Get, "/api/emojifont") => match current(env).await {
            Some(v) => ok(json!({"name": v["name"], "size": jint(&v, "size"), "ver": jint(&v, "ver")})),
            None => {
                let r = kick(env).await;
                ok(json!({"missing": true, "kick": r}))
            }
        },
        // Ilova yuklay olmadi: urinish sanaladi, `FAILS_KICK` ga yetsa Actions uyg'otiladi.
        (Method::Post, "/api/emojifont/fail") => {
            let b: Value = req.json().await.unwrap_or(json!({}));
            let ver = jint(&b, "ver");
            let key = "emoji_font_fails";
            let cur = config_get_db(env, key).await.unwrap_or_default();
            // `<versiya>:<son>` — versiya almashsa sanagich nolga tushadi.
            let n = match cur.split_once(':') {
                Some((v, c)) if v.parse::<i64>().ok() == Some(ver) => c.parse::<i64>().unwrap_or(0),
                _ => 0,
            } + 1;
            if n >= FAILS_KICK {
                config_put(env, key, &format!("{ver}:0")).await;
                let r = kick(env).await;
                return ok(json!({"kicked": true, "result": r}));
            }
            config_put(env, key, &format!("{ver}:{n}")).await;
            ok(json!({"kicked": false, "fails": n}))
        }
        _ => err404("topilmadi"),
    }
}

/// Actions yangi shriftni kanalga yubordi: `tg_files` ga yoziladi, `emoji_font` almashadi, eski fayl kanaldan o'chadi.
async fn commit(env: &Env, b: &Value) -> Result<Response> {
    let ver = jint(b, "ver");
    let name = b["name"].as_str().unwrap_or("").to_string();
    let key = b["key"].as_str().unwrap_or("").to_ascii_lowercase();
    let msg_id = jint(b, "msg_id");
    let size = jint(b, "size");
    if !font_name_ok(&name, ver) || !valid_file_key(&key) || msg_id <= 0 || size < 100_000 || size > 40_000_000 {
        return json_resp(&json!({"error": "fayl"}), 400);
    }
    let old = current(env).await;
    turso_exec(env,
        "INSERT INTO tg_files (file_name, msg_id, file_key) VALUES (?, ?, ?)
         ON CONFLICT(file_name) DO UPDATE SET msg_id=excluded.msg_id, file_key=excluded.file_key",
        vec![arg_s(&name), arg_i(msg_id), arg_s(&key)]).await?;
    config_put(env, "emoji_font", &json!({"name": name, "size": size, "ver": ver}).to_string()).await;
    config_put(env, "emoji_font_want", "0").await;
    config_put(env, "emoji_font_fails", &format!("{ver}:0")).await;
    if let Some(o) = old {
        let on = o["name"].as_str().unwrap_or("").to_string();
        if !on.is_empty() && on != name {
            tg_forget_file(env, &on).await;
        }
    }
    ok(json!({"ok": true}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nom_tekshiruvi() {
        assert!(font_name_ok("sys_emojifont_5.bin", 5));
        assert!(!font_name_ok("sys_emojifont_5.bin", 6));
        assert!(!font_name_ok("sys_emojifont_0.bin", 0));
        assert!(!font_name_ok("../sys_emojifont_5.bin", 5));
        assert!(!font_name_ok("sys_emojifont_5.ttf", 5));
    }
}
