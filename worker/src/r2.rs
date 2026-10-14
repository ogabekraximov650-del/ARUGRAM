// ══════════════════════════════════════════════════════════════
//  R2 ORALIQ OMBORI: kodlovchi -> R2 -> yuklovchi workflow -> Telegram (bot) -> baza
// ══════════════════════════════════════════════════════════════
//
// TALAB (foydalanuvchi): tayyor fayllar avval Cloudflare R2 (`arumediatv` bucket) ga yuklansin; ALOHIDA workflow
// (`upload.yml`) ularni R2'dan olib Telegram'ga (asosiy bot chatiga) yuklasin; baza to'liq yozilgach fayl R2'dan o'chsin.
// Shunda kodlash Telegram yuklashini kutmaydi, yuklash qotib qolsa fayl R2'da turaveradi va qayta uriniladi.
//
// FAYL BAYTLARI WORKER ORQALI O'TMAYDI: worker faqat imzolangan (AWS SigV4 presigned) havolalar beradi:
//   * kodlovchi `PUT` havolasi bilan R2'ga to'g'ridan-to'g'ri yozadi (`/api/encode/r2put`, `/api/encode/r2ready`);
//   * yuklovchi `GET` havolasi bilan R2'dan oladi (`/api/r2/claim`).
// Kalitlar: `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY` (R2 -> Manage API tokens, "Object Read & Write"), `R2_ACCOUNT_ID`.
// Ular yo'q bo'lsa `ready()` false — run'lar eski usulda (bot chatiga to'g'ridan-to'g'ri) yuklaydi.
//
// Holat jadvali `r2_files`: `uploading` (kodlovchi yozyapti) -> `ready` (R2'da, navbatda) -> `sending` (yuklovchi oldi).
// Bot faylni yopiq kanalga saqlab bazaga yozgach (`encupload::on_file`) `finish` R2 obyekti va qatorni o'chiradi.

use super::*;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

const BUCKET: &str = "arumediatv";
const REGION: &str = "auto";
const WORKFLOW: &str = "upload.yml";
/// Yuklovchi ijarasi: shu vaqtda `beat` kelmasa fayl boshqa run'ga o'tadi.
const LEASE_MS: i64 = 15 * 60 * 1000;
const MAX_ATTEMPTS: i64 = 6;

const DDL: &str = "CREATE TABLE IF NOT EXISTS r2_files (
    file_name TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    anime_id INTEGER NOT NULL,
    season_id INTEGER NOT NULL,
    epizod_id INTEGER NOT NULL,
    queued_at INTEGER NOT NULL,
    quality TEXT NOT NULL,
    file_key TEXT NOT NULL,
    size INTEGER NOT NULL,
    state TEXT NOT NULL DEFAULT 'uploading',
    runner TEXT NOT NULL DEFAULT '',
    lease_until INTEGER NOT NULL DEFAULT 0,
    attempts INTEGER NOT NULL DEFAULT 0,
    error TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL
) WITHOUT ROWID";

pub(crate) async fn migrate(env: &Env) {
    if config_get(env, "mig_r2").await.is_some() {
        return;
    }
    let _ = turso_exec(env, DDL, vec![]).await;
    config_put(env, "mig_r2", "1").await;
}

// ── AWS SigV4 (presigned URL) ────────────────────────────────

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn hmac256(key: &[u8], data: &str) -> Vec<u8> {
    let mut m = <Hmac<Sha256> as Mac>::new_from_slice(key).expect("hmac kaliti");
    m.update(data.as_bytes());
    m.finalize().into_bytes().to_vec()
}

fn pct(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// `YYYYMMDDTHHMMSSZ` (UTC).
fn amz_date(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Fuqaro sanasi (Howard Hinnant algoritmi).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}{m:02}{d:02}T{:02}{:02}{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

/// Imzolangan havola (faqat `host` sarlavhasi imzolanadi, tana — `UNSIGNED-PAYLOAD`).
fn presign(method: &str, host: &str, path: &str, akid: &str, secret: &str, region: &str, amzdate: &str, expires: i64) -> String {
    let date = &amzdate[..8];
    let scope = format!("{date}/{region}/s3/aws4_request");
    let mut q = vec![
        ("X-Amz-Algorithm", "AWS4-HMAC-SHA256".to_string()),
        ("X-Amz-Credential", format!("{akid}/{scope}")),
        ("X-Amz-Date", amzdate.to_string()),
        ("X-Amz-Expires", expires.to_string()),
        ("X-Amz-SignedHeaders", "host".to_string()),
    ];
    q.sort_by(|a, b| a.0.cmp(b.0));
    let cq = q.iter().map(|(k, v)| format!("{}={}", pct(k), pct(v))).collect::<Vec<_>>().join("&");
    let creq = format!("{method}\n{path}\n{cq}\nhost:{host}\n\nhost\nUNSIGNED-PAYLOAD");
    let sts = format!("AWS4-HMAC-SHA256\n{amzdate}\n{scope}\n{}", hex(&Sha256::digest(creq.as_bytes())));
    let mut k = hmac256(format!("AWS4{secret}").as_bytes(), date);
    for part in [region, "s3", "aws4_request"] {
        k = hmac256(&k, part);
    }
    let sig = hex(&hmac256(&k, &sts));
    format!("https://{host}{path}?{cq}&X-Amz-Signature={sig}")
}

/// R2 sozlanganmi (kalitlar va hisob ID'si bor).
pub(crate) fn ready(env: &Env) -> bool {
    !tg_secret(env, "R2_ACCESS_KEY_ID").is_empty() && !tg_secret(env, "R2_SECRET_ACCESS_KEY").is_empty()
        && !tg_secret(env, "R2_ACCOUNT_ID").is_empty()
}

fn url(env: &Env, method: &str, file: &str, expires: i64) -> Option<String> {
    if !ready(env) || !tg_safe_name(file) {
        return None;
    }
    let host = format!("{}.r2.cloudflarestorage.com", tg_secret(env, "R2_ACCOUNT_ID"));
    Some(presign(method, &host, &format!("/{BUCKET}/{file}"), &tg_secret(env, "R2_ACCESS_KEY_ID"),
        &tg_secret(env, "R2_SECRET_ACCESS_KEY"), REGION, &amz_date(now_ms()), expires))
}

async fn r2_fetch(env: &Env, method: &str, file: &str) -> Option<(u16, Option<i64>)> {
    let u = url(env, method, file, 300)?;
    let mut init = RequestInit::new();
    init.with_method(if method == "HEAD" { Method::Head } else { Method::Delete });
    let req = Request::new_with_init(&u, &init).ok()?;
    let r = Fetch::Request(req).send().await.ok()?;
    let len = r.headers().get("content-length").ok().flatten().and_then(|v| v.parse::<i64>().ok());
    Some((r.status_code(), len))
}

// ── Kodlovchi tomoni ─────────────────────────────────────────

/// Kodlovchi faylni R2'ga yozmoqchi: qator ochiladi, `PUT` havolasi qaytadi (2 soat amal qiladi).
pub(crate) async fn register(env: &Env, (a, s, e): (i64, i64, i64), queued_at: i64, q: &str, kind: &str, file: &str,
        key: &str, size: i64) -> Option<String> {
    migrate(env).await;
    let put = url(env, "PUT", file, 7200)?;
    turso_exec(env,
        "INSERT INTO r2_files (file_name, kind, anime_id, season_id, epizod_id, queued_at, quality, file_key, size, state, created_at)
         VALUES (?,?,?,?,?,?,?,?,?,'uploading',?)
         ON CONFLICT(file_name) DO UPDATE SET kind=excluded.kind, file_key=excluded.file_key, size=excluded.size,
            state='uploading', runner='', lease_until=0, attempts=0, error='', created_at=excluded.created_at",
        vec![TursoArg::text(file), TursoArg::text(kind), TursoArg::int(a), TursoArg::int(s), TursoArg::int(e),
             TursoArg::int(queued_at), TursoArg::text(q), TursoArg::text(key), TursoArg::int(size), TursoArg::int(now_ms())]).await.ok()?;
    Some(put)
}

/// Kodlovchi yuklab bo'ldi: R2'da fayl bor va hajmi to'g'rimi tekshiriladi, keyin `ready` (yuklovchi olishi mumkin).
pub(crate) async fn mark_ready(env: &Env, file: &str, size: i64) -> std::result::Result<(), String> {
    let Some((code, len)) = r2_fetch(env, "HEAD", file).await else { return Err("r2_yetib_bormadi".into()) };
    if code != 200 || len != Some(size) {
        return Err(format!("r2_hajm ({code}, {len:?})"));
    }
    turso_exec(env, "UPDATE r2_files SET state='ready' WHERE file_name=? AND state='uploading'",
        vec![TursoArg::text(file)]).await.map_err(|e| e.to_string())?;
    let _ = kick(env).await;
    Ok(())
}

/// Shu qism uchun R2'da hali Telegram'ga yetib bormagan fayl bormi (qism "tugadi" deb hisoblanmasin).
pub(crate) async fn pending_for(env: &Env, a: i64, s: i64, e: i64, queued_at: i64) -> bool {
    turso_exec(env,
        "SELECT 1 AS x FROM r2_files WHERE anime_id=? AND season_id=? AND epizod_id=? AND queued_at=? LIMIT 1",
        vec![TursoArg::int(a), TursoArg::int(s), TursoArg::int(e), TursoArg::int(queued_at)]).await
        .ok().and_then(|r| first_row(&r)).is_some()
}

/// Bot faylni saqlab bazaga yozdi (yoki fayl kerak emas) — R2 obyekti va qator o'chadi.
pub(crate) async fn finish(env: &Env, file: &str) {
    let _ = r2_fetch(env, "DELETE", file).await;
    let _ = turso_exec(env, "DELETE FROM r2_files WHERE file_name=?", vec![TursoArg::text(file)]).await;
}

pub(crate) async fn row(env: &Env, file: &str) -> Option<Value> {
    turso_exec(env, "SELECT * FROM r2_files WHERE file_name=?", vec![TursoArg::text(file)]).await.ok().and_then(|r| first_row(&r))
}

// ── Yuklovchi workflow ───────────────────────────────────────

/// Navbatda tayyor fayl bo'lsa va `upload.yml` ishlamayotgan bo'lsa — ishga tushiradi. Eski/yarim fayllarni ham tozalaydi.
pub(crate) async fn kick(env: &Env) -> String {
    if !ready(env) {
        return String::new();
    }
    migrate(env).await;
    let now = now_ms();
    let Some(r) = turso_exec(env,
        "SELECT (SELECT COUNT(*) FROM r2_files WHERE state='ready' OR (state='sending' AND lease_until<=? AND attempts<?)) AS pending,
                (SELECT COUNT(*) FROM r2_files WHERE state='sending' AND lease_until>?) AS active",
        vec![TursoArg::int(now), TursoArg::int(MAX_ATTEMPTS), TursoArg::int(now)]).await.ok().and_then(|r| first_row(&r))
    else { return String::new() };
    if jint(&r, "active") > 0 || jint(&r, "pending") == 0 {
        return String::new();
    }
    if !encode_sess_ok(env, "upload").await {
        return "\u{1F511} Yuklovchi workflow sessiyasi ulanmagan (botda \u{00AB}Yangi sessiya\u{00BB} \u{2192} upload)".into();
    }
    let repo = tg_secret(env, "GH_REPO");
    if !repo.contains('/') {
        return "GH_REPO yo'q".into();
    }
    match gh_workflow_busy(env, &repo, WORKFLOW).await {
        Ok(true) => return String::new(),
        Ok(false) => {}
        Err(e) => return e,
    }
    // Atomik belgi: cron va `ready` bir vaqtda ikkita run ochmasin.
    let claim = turso_exec(env,
        "INSERT INTO app_config (cfg_key,cfg_value) VALUES ('r2_kicked_at', ?)
         ON CONFLICT(cfg_key) DO UPDATE SET cfg_value=excluded.cfg_value
           WHERE CAST(app_config.cfg_value AS INTEGER) < ?
         RETURNING cfg_key",
        vec![TursoArg::text(&now.to_string()), TursoArg::int(now - 90 * 1000)]).await;
    if claim.ok().and_then(|r| first_row(&r)).is_none() {
        return String::new();
    }
    match gh_api(env, Method::Post, &format!("/repos/{repo}/actions/workflows/{WORKFLOW}/dispatches"),
        Some(json!({"ref": "main"}))).await {
        Ok((204, _)) => "Yuklovchi workflow ishga tushirildi".into(),
        Ok((code, v)) => format!("Yuklovchi workflow ishga tushmadi (GitHub {code}): {}", v["message"].as_str().unwrap_or("")),
        Err(e) => format!("Yuklovchi workflow ishga tushmadi: {e}"),
    }
}

/// Cron: eskirgan qatorlar (yuklab bo'lmagan `uploading` 3 soatdan, `ready`/`sending` 24 soatdan eski) R2'dan o'chadi.
pub(crate) async fn cleanup(env: &Env) {
    if !ready(env) || config_get(env, "mig_r2").await.is_none() {
        return;
    }
    let now = now_ms();
    let Ok(res) = turso_exec(env,
        "SELECT file_name FROM r2_files WHERE (state='uploading' AND created_at<?) OR created_at<? LIMIT 10",
        vec![TursoArg::int(now - 3 * 3600 * 1000), TursoArg::int(now - 24 * 3600 * 1000)]).await else { return };
    for r in rows_of(&res) {
        if let Some(f) = r["file_name"].as_str() {
            finish(env, f).await;
        }
    }
}

/// `X-Encode-Token` bilan: `/api/r2/claim`, `/api/r2/beat`, `/api/r2/fail`, `/api/r2/done`.
pub(crate) async fn route(mut req: Request, env: &Env, path: &str) -> Result<Response> {
    if !encode_token_ok(&req, env) {
        return json_resp(&json!({"error": "forbidden"}), 403);
    }
    migrate(env).await;
    let b: Value = req.json().await.unwrap_or(json!({}));
    let file = b["file"].as_str().unwrap_or("").to_string();
    let now = now_ms();
    match path {
        "/api/r2/claim" => {
            if !ready(env) || !encode_sess_ok(env, "upload").await {
                return ok(json!({"none": true, "nosession": ready(env)}));
            }
            let runner: String = b["runner"].as_str().unwrap_or("").chars().take(64).collect();
            let res = turso_exec(env,
                "UPDATE r2_files SET state='sending', runner=?, lease_until=?, attempts=attempts+1
                  WHERE file_name = (SELECT file_name FROM r2_files
                                      WHERE state='ready' OR (state='sending' AND lease_until<=? AND attempts<?)
                                      ORDER BY created_at ASC LIMIT 1)
                  RETURNING *",
                vec![TursoArg::text(&runner), TursoArg::int(now + LEASE_MS), TursoArg::int(now), TursoArg::int(MAX_ATTEMPTS)]).await?;
            let Some(r) = first_row(&res) else { return ok(json!({"none": true})) };
            let name = r["file_name"].as_str().unwrap_or("").to_string();
            let Some(get) = url(env, "GET", &name, 7200) else { return ok(json!({"none": true})) };
            let (a, s, e, qa) = (jint(&r, "anime_id"), jint(&r, "season_id"), jint(&r, "epizod_id"), jint(&r, "queued_at"));
            let (kind, q, key, size) = (r["kind"].as_str().unwrap_or(""), r["quality"].as_str().unwrap_or(""),
                r["file_key"].as_str().unwrap_or(""), jint(&r, "size"));
            return ok(json!({
                "file": name, "url": get, "size": size, "bot": bot_username(env).await,
                "caption": format!("ARUENC|{kind}|{a}|{s}|{e}|{qa}|{q}|{key}|{size}"),
                "attempt": jint(&r, "attempts"),
            }));
        }
        "/api/r2/beat" => {
            let _ = turso_exec(env, "UPDATE r2_files SET lease_until=? WHERE file_name=? AND state='sending'",
                vec![TursoArg::int(now + LEASE_MS), TursoArg::text(&file)]).await;
            return ok(json!({"ok": true}));
        }
        "/api/r2/fail" => {
            let why: String = b["error"].as_str().unwrap_or("xato").chars().take(300).collect();
            let r = turso_exec(env,
                "UPDATE r2_files SET state=CASE WHEN attempts>=? THEN 'error' ELSE 'ready' END, runner='', lease_until=0, error=?
                  WHERE file_name=? AND state='sending' RETURNING state, attempts",
                vec![TursoArg::int(MAX_ATTEMPTS), TursoArg::text(&why), TursoArg::text(&file)]).await?;
            if let Some(r) = first_row(&r) {
                if r["state"].as_str() == Some("error") {
                    encode_notify(env, &format!("\u{274C} R2 \u{2192} Telegram: <code>{}</code> {} urinishda yuklanmadi: {}",
                        html_escape(&file), MAX_ATTEMPTS, html_escape(&why))).await;
                }
            }
            return ok(json!({"ok": true}));
        }
        "/api/r2/done" => {
            // Faqat bazaga haqiqatan yozilgan bo'lsa (tg_files) R2'dan o'chadi.
            if encupload::committed(env, &file).await {
                finish(env, &file).await;
                return ok(json!({"ok": true, "deleted": true}));
            }
            return ok(json!({"ok": false}));
        }
        _ => {}
    }
    err404("topilmadi")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aws_presign_vektori() {
        // AWS hujjatidagi rasmiy namuna (GET /test.txt).
        let u = presign("GET", "examplebucket.s3.amazonaws.com", "/test.txt", "AKIAIOSFODNN7EXAMPLE",
            "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY", "us-east-1", "20130524T000000Z", 86400);
        assert!(u.ends_with("X-Amz-Signature=aeeed9bbccd4d02ee5c0109b86d86835f995330da4c265957d157751f604d404"));
    }

    #[test]
    fn sana_formati() {
        assert_eq!(amz_date(1_369_353_600_000), "20130524T000000Z");
        assert_eq!(amz_date(0), "19700101T000000Z");
    }
}
