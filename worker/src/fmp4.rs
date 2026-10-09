// worker/src/fmp4.rs — QISMLARNING fMP4 NUSXASI (Telegram Mini App uchun)
//
// TALAB (foydalanuvchi): ilova (Android) oddiy MP4 bilan ishlayveradi, unga
// TEGILMAYDI. Mini App (`web/`) esa videoni brauzerda bo'laklab ko'rsatadi
// (MSE / iPhone'da ManagedMediaSource) — buning uchun fayl "bo'laklangan
// MP4" (fMP4) bo'lishi kerak. Shu sabab har sifatning YONIDA fMP4 nusxasi:
//
//   epizod_db.fmp4_url_<q>  — fayl nomi (`ep_<a>_<s>_<e>_<q>_<vaqt>_f.mp4`);
//   epizod_db.fmp4_size_<q> — hajmi;
//   epizod_db.fmp4_key_<q>  — AES-128-CTR kaliti (ro'yxatlarda berilmaydi,
//                             `hide_keys`; Mini App uni `/api/tg/deliver`
//                             javobidan oladi — `tg_files.file_key`).
//
// Nom `ep_` bilan boshlanadi — `/api/tg/deliver` dagi obuna tekshiruvi
// (`season_of_file`) fMP4 ga ham AYNAN shunday ishlaydi.
//
// ── QAYERDAN PAYDO BO'LADI ─────────────────────────────────────
//
//   1. Yangi qismlar: avto-kodlash (`tool/encode/run.py`) har sifatning
//      MP4 i tayyor bo'lishi bilan undan fMP4 yasab (`ffmpeg -c copy`,
//      qayta siqilmaydi) kanalga yuklaydi va `/api/fmp4/done` ga yozadi.
//   2. Eski qismlar: Mini App fMP4'i yo'q qismni so'rasa
//      (`POST /api/fmp4/request`) — tayyor sifatlar NAVBATGA (`fmp4_jobs`)
//      qo'yiladi, worker ALOHIDA workflow `fmp4.yml` ni (avtoencode repo)
//      ishga tushiradi: u MP4 ni kanaldan olib, fMP4 qilib qaytaradi.
//
// Yozuvlar: navbatga faqat YO'Q narsa qo'yiladi (bitta batch), tayyor
// bo'lganda bitta batch — Turso yozuvi kam.

use super::*;

pub(crate) const WORKFLOW: &str = "fmp4.yml";
/// Bitta ish ijarasi (yuklab olish + qayta joylash + yuklash): 30 daqiqa.
const LEASE_MS: i64 = 30 * 60 * 1000;
const MAX_ATTEMPTS: i64 = 3;

pub(crate) const DDL: &str = "CREATE TABLE IF NOT EXISTS fmp4_jobs (
    anime_id INTEGER NOT NULL,
    season_id INTEGER NOT NULL,
    epizod_id INTEGER NOT NULL,
    quality TEXT NOT NULL,
    state TEXT DEFAULT 'queued',
    attempts INTEGER DEFAULT 0,
    lease_until INTEGER DEFAULT 0,
    runner TEXT DEFAULT '',
    error TEXT DEFAULT '',
    queued_at INTEGER,
    PRIMARY KEY (anime_id, season_id, epizod_id, quality)
) WITHOUT ROWID";

/// Bir martalik: `epizod_db` ga 12 ta ustun va navbat jadvali.
pub(crate) async fn migrate(env: &Env) {
    if config_get(env, "mig_fmp4").await.is_some() {
        return;
    }
    for q in QUALITIES {
        for sql in [
            format!("ALTER TABLE epizod_db ADD COLUMN fmp4_url_{q} TEXT DEFAULT ''"),
            format!("ALTER TABLE epizod_db ADD COLUMN fmp4_size_{q} INTEGER DEFAULT 0"),
            format!("ALTER TABLE epizod_db ADD COLUMN fmp4_key_{q} TEXT DEFAULT ''"),
        ] {
            let _ = turso_exec(env, &sql, vec![]).await;
        }
    }
    let _ = turso_exec(env, DDL, vec![]).await;
    config_put(env, "mig_fmp4", "1").await;
}

/// fMP4 fayl nomi to'g'rimi (shu qism va sifatga tegishli).
fn valid_name(name: &str, a: i64, s: i64, e: i64, q: &str) -> bool {
    tg_safe_name(name) && name.starts_with(&format!("ep_{a}_{s}_{e}_{q}_")) && name.ends_with("_f.mp4")
}

fn bad(msg: &str) -> Result<Response> {
    json_resp(&json!({"error": msg}), 400)
}

/// `POST /api/fmp4/request` (Mini App, sessiya bilan):
/// `{anime_id, season_id, epizod_id}` -> `{ready: [...], queued: [...]}`.
pub(crate) async fn request(mut req: Request, env: &Env) -> Result<Response> {
    if session_user(env, &bearer(&req)).await?.is_none() {
        return json_resp(&json!({"error": "unauthorized"}), 401);
    }
    migrate(env).await;
    let b: Value = req.json().await.unwrap_or(json!({}));
    let (a, s, e) = (jint(&b, "anime_id"), jint(&b, "season_id"), jint(&b, "epizod_id"));
    let cols: Vec<String> = QUALITIES.iter()
        .map(|q| format!("url_{q}, fmp4_url_{q}"))
        .collect();
    let res = turso_exec(env,
        &format!("SELECT {} FROM epizod_db WHERE anime_id=? AND season_id=? AND epizod_id=?", cols.join(", ")),
        vec![TursoArg::int(a), TursoArg::int(s), TursoArg::int(e)]).await?;
    let Some(row) = first_row(&res) else {
        return json_resp(&json!({"error": "not_found"}), 404);
    };
    let mut ready = Vec::new();
    let mut missing = Vec::new();
    for q in QUALITIES {
        let mp4 = row[format!("url_{q}")].as_str().unwrap_or("");
        let fm = row[format!("fmp4_url_{q}")].as_str().unwrap_or("");
        if !fm.is_empty() {
            ready.push(q);
        } else if bare_name(mp4).starts_with("ep_") {
            missing.push(q);
        }
    }
    if !missing.is_empty() {
        let now = now_ms();
        let stmts: Vec<(&str, Vec<TursoArg>)> = missing.iter().map(|q| (
            "INSERT INTO fmp4_jobs (anime_id, season_id, epizod_id, quality, queued_at)
             VALUES (?,?,?,?,?)
             ON CONFLICT DO UPDATE SET state='queued', attempts=0, error=''
               WHERE fmp4_jobs.state='error'",
            vec![TursoArg::int(a), TursoArg::int(s), TursoArg::int(e), TursoArg::text(q), TursoArg::int(now)],
        )).collect();
        let _ = turso_batch(env, &stmts).await;
        let _ = kick(env).await;
    }
    ok_nostore(json!({"ready": ready, "queued": missing}))
}

/// Navbatda ish bo'lsa va workflow ishlamayotgan bo'lsa — ishga tushiradi.
pub(crate) async fn kick(env: &Env) -> String {
    let now = now_ms();
    let res = turso_exec(env,
        "SELECT
           (SELECT COUNT(*) FROM fmp4_jobs WHERE state='queued' OR (state='running' AND lease_until<=?)) AS pending,
           (SELECT COUNT(*) FROM fmp4_jobs WHERE state='running' AND lease_until>?) AS active",
        vec![TursoArg::int(now), TursoArg::int(now)]).await;
    let Some(r) = res.ok().and_then(|r| first_row(&r)) else {
        return String::new();
    };
    if jint(&r, "active") > 0 || jint(&r, "pending") == 0 {
        return String::new();
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
    // Cron va so'rov bir vaqtda ikkita run ochmasin — atomik belgi.
    let claim = turso_exec(env,
        "INSERT INTO app_config (cfg_key,cfg_value) VALUES ('fmp4_kicked_at', ?)
         ON CONFLICT(cfg_key) DO UPDATE SET cfg_value=excluded.cfg_value
           WHERE CAST(app_config.cfg_value AS INTEGER) < ?
         RETURNING cfg_key",
        vec![TursoArg::text(&now.to_string()), TursoArg::int(now - 3 * 60 * 1000)]).await;
    if claim.ok().and_then(|r| first_row(&r)).is_none() {
        return String::new();
    }
    match gh_api(env, Method::Post,
        &format!("/repos/{repo}/actions/workflows/{WORKFLOW}/dispatches"),
        Some(json!({"ref": "main"}))).await {
        Ok((204, _)) => "fMP4 workflow ishga tushirildi".into(),
        Ok((code, v)) => format!("fMP4 workflow ishga tushmadi (GitHub {code}): {}", v["message"].as_str().unwrap_or("")),
        Err(e) => format!("fMP4 workflow ishga tushmadi: {e}"),
    }
}

/// Actions yo'llari (`X-Encode-Token`): `/api/fmp4/claim`, `/api/fmp4/done`.
pub(crate) async fn route(mut req: Request, env: &Env, path: &str) -> Result<Response> {
    if !encode_token_ok(&req, env) {
        return json_resp(&json!({"error": "forbidden"}), 403);
    }
    migrate(env).await;
    let b: Value = req.json().await.unwrap_or(json!({}));
    let now = now_ms();

    if path == "/api/fmp4/claim" {
        let runner = b["runner"].as_str().unwrap_or("").chars().take(64).collect::<String>();
        let res = turso_exec(env,
            "UPDATE fmp4_jobs SET state='running', runner=?, lease_until=?, attempts=attempts+1
              WHERE (anime_id, season_id, epizod_id, quality) = (
                SELECT anime_id, season_id, epizod_id, quality FROM fmp4_jobs
                 WHERE state='queued' OR (state='running' AND lease_until<=?)
                 ORDER BY queued_at ASC LIMIT 1)
             RETURNING *",
            vec![TursoArg::text(&runner), TursoArg::int(now + LEASE_MS), TursoArg::int(now)]).await?;
        let Some(job) = first_row(&res) else {
            return ok(json!({"job": null}));
        };
        let (a, s, e) = (jint(&job, "anime_id"), jint(&job, "season_id"), jint(&job, "epizod_id"));
        let q = job["quality"].as_str().unwrap_or("").to_string();
        if !QUALITIES.contains(&q.as_str()) {
            let _ = turso_exec(env, "DELETE FROM fmp4_jobs WHERE anime_id=? AND season_id=? AND epizod_id=? AND quality=?",
                vec![TursoArg::int(a), TursoArg::int(s), TursoArg::int(e), TursoArg::text(&q)]).await;
            return ok(json!({"job": null, "retry": true}));
        }
        let src = turso_exec(env,
            &format!("SELECT d.url_{q} AS name, d.key_{q} AS dkey, t.msg_id AS msg_id, t.file_key AS tkey
                        FROM epizod_db d LEFT JOIN tg_files t ON t.file_name = d.url_{q}
                       WHERE d.anime_id=? AND d.season_id=? AND d.epizod_id=?"),
            vec![TursoArg::int(a), TursoArg::int(s), TursoArg::int(e)]).await?;
        let row = first_row(&src).unwrap_or(json!({}));
        let name = bare_name(row["name"].as_str().unwrap_or(""));
        let msg_id = jint(&row, "msg_id");
        if name.is_empty() || msg_id <= 0 {
            // Manba yo'q (Telegram'da emas) — ish tashlanadi.
            let _ = turso_exec(env, "DELETE FROM fmp4_jobs WHERE anime_id=? AND season_id=? AND epizod_id=? AND quality=?",
                vec![TursoArg::int(a), TursoArg::int(s), TursoArg::int(e), TursoArg::text(&q)]).await;
            return ok(json!({"job": null, "retry": true}));
        }
        let key = [row["tkey"].as_str().unwrap_or(""), row["dkey"].as_str().unwrap_or("")]
            .into_iter().find(|k| valid_file_key(k)).unwrap_or("").to_string();
        let stem = name.trim_end_matches(".mp4");
        return ok(json!({"job": {
            "anime_id": a, "season_id": s, "epizod_id": e, "quality": q,
            "src_name": name, "src_msg_id": msg_id, "src_key": key,
            "name": format!("{stem}_f.mp4"),
            "channel": tg_channel_id(env),
        }}));
    }

    if path == "/api/fmp4/done" {
        let (a, s, e) = (jint(&b, "anime_id"), jint(&b, "season_id"), jint(&b, "epizod_id"));
        let q = b["quality"].as_str().unwrap_or("").to_string();
        if !QUALITIES.contains(&q.as_str()) {
            return bad("sifat");
        }
        let pk = || vec![TursoArg::int(a), TursoArg::int(s), TursoArg::int(e), TursoArg::text(&q)];
        if b["ok"].as_bool() != Some(true) {
            let why: String = b["error"].as_str().unwrap_or("xato").chars().take(300).collect();
            let mut args = vec![TursoArg::int(MAX_ATTEMPTS), TursoArg::text(&why)];
            args.extend(pk());
            let _ = turso_exec(env,
                "UPDATE fmp4_jobs SET state = CASE WHEN attempts >= ? THEN 'error' ELSE 'queued' END,
                        lease_until=0, runner='', error=?
                  WHERE anime_id=? AND season_id=? AND epizod_id=? AND quality=?", args).await;
            return ok(json!({"ok": true}));
        }
        let file = b["file"].as_str().unwrap_or("").to_string();
        let key = b["key"].as_str().unwrap_or("").to_ascii_lowercase();
        let size = jint(&b, "size");
        let msg_id = jint(&b, "msg_id");
        if !valid_name(&file, a, s, e, &q) || !valid_file_key(&key) || size <= 0 || msg_id <= 0 {
            return bad("fayl");
        }
        let old = turso_exec(env,
            &format!("SELECT fmp4_url_{q} AS u FROM epizod_db WHERE anime_id=? AND season_id=? AND epizod_id=?"),
            vec![TursoArg::int(a), TursoArg::int(s), TursoArg::int(e)]).await.ok()
            .and_then(|r| first_row(&r)).and_then(|r| r["u"].as_str().map(String::from)).unwrap_or_default();
        turso_batch(env, &[
            ("INSERT INTO tg_files (file_name, msg_id, file_key) VALUES (?, ?, ?)
              ON CONFLICT(file_name) DO UPDATE SET msg_id=excluded.msg_id, file_key=excluded.file_key",
             vec![TursoArg::text(&file), TursoArg::int(msg_id), TursoArg::text(&key)]),
            (&format!("UPDATE epizod_db SET fmp4_url_{q}=?, fmp4_size_{q}=?, fmp4_key_{q}=?
                        WHERE anime_id=? AND season_id=? AND epizod_id=?"),
             vec![TursoArg::text(&file), TursoArg::int(size), TursoArg::text(&key),
                  TursoArg::int(a), TursoArg::int(s), TursoArg::int(e)]),
            ("DELETE FROM fmp4_jobs WHERE anime_id=? AND season_id=? AND epizod_id=? AND quality=?", pk()),
        ]).await?;
        if !old.is_empty() && old != file {
            tg_forget_file(env, &old).await;
        }
        return ok(json!({"ok": true}));
    }

    err404("topilmadi")
}

/// MP4 almashtirildi (qayta kodlandi) — shu sifatning eski fMP4'i endi
/// boshqa videoniki: ustunlar tozalanadi, kanaldagi fayl o'chadi.
pub(crate) async fn forget_quality(env: &Env, a: i64, s: i64, e: i64, q: &str) {
    if !QUALITIES.contains(&q) || config_get(env, "mig_fmp4").await.is_none() {
        return;
    }
    let old = turso_exec(env,
        &format!("SELECT fmp4_url_{q} AS u FROM epizod_db WHERE anime_id=? AND season_id=? AND epizod_id=?"),
        vec![TursoArg::int(a), TursoArg::int(s), TursoArg::int(e)]).await.ok()
        .and_then(|r| first_row(&r)).and_then(|r| r["u"].as_str().map(String::from)).unwrap_or_default();
    if old.is_empty() {
        return;
    }
    let _ = turso_exec(env,
        &format!("UPDATE epizod_db SET fmp4_url_{q}='', fmp4_size_{q}=0, fmp4_key_{q}=''
                   WHERE anime_id=? AND season_id=? AND epizod_id=?"),
        vec![TursoArg::int(a), TursoArg::int(s), TursoArg::int(e)]).await;
    tg_forget_file(env, &old).await;
}
