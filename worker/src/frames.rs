// ══════════════════════════════════════════════════════════════
//  KADRLAR (post uchun afisha albomi) — ALOHIDA workflow
// ══════════════════════════════════════════════════════════════
//
// UZ: Sifat workflow'lari (`encode.yml`) faqat qismni tayyorlaydi. Qismning kadrlari (2..10 ta to'liq o'lchamli JPEG) ALOHIDA
//     workflow'da (`frames.yml`, `avtoencode` repoda, shablon `tool/frames/frames.workflow.yml`) olinadi:
//   * qism TO'LIQ tayyor bo'lgach (barcha sifatlar yozilgan, asl video o'chirilgan: `encode_jobs` qatori yo'q) — "Kodlash tugadi" dan keyin, `vd/done` da yoki cron (har 10 daqiqa) `kick` workflow'ni ishga tushiradi;
//   * `tool/frames/frames.py` navbat bo'shaguncha `/api/frames/claim` qiladi: kadrlari YO'Q (`epizod_db.collage_file_id` bo'sh), kodlash ishi yakunlangan,
//     video video omborida (`vd`) tayyor qismlar — yangisidan eskisiga (oxirgi 14 kun);
//   * eng yuqori sifatni yuklab kadrlarni oladi va MAVJUD `/api/encode/collage` ga yuboradi (shifrlab saqlaydi); xato bo'lsa `/api/frames/fail`.
//   Shu tariqa kadrsiz qolgan qism (masalan, kadr olish xato bergan) o'zi tuzaladi.
// EN: Quality workflows only build the episode. Its frames are taken by a SEPARATE workflow, woken as soon as a quality is ready
//     (and by cron); it fetches the best quality from the video store and posts the frames to the existing collage endpoint.

use super::*;

pub(crate) const DDL: [&str; 1] = [
    "CREATE TABLE IF NOT EXISTS frames_q (
        anime_id INTEGER NOT NULL, season_id INTEGER NOT NULL, epizod_id INTEGER NOT NULL,
        runner TEXT DEFAULT '', lease_until INTEGER DEFAULT 0, attempts INTEGER DEFAULT 0, error TEXT DEFAULT '',
        PRIMARY KEY (anime_id, season_id, epizod_id)
    )",
];

const FRAMES_WORKFLOW: &str = "frames.yml";
/// Bitta qismga ijara (run o'lsa shundan keyin qayta olinadi) va urinishlar soni.
const LEASE_MS: i64 = 60 * 60 * 1000;
const RETRY_MS: i64 = 10 * 60 * 1000;
const MAX_ATTEMPTS: i64 = 5;
/// Faqat shu muddatdan yangi qismlar (eski qismlar hammasini bir yo'la navbatga qo'ymaslik uchun).
const WINDOW_MS: i64 = 14 * 24 * 60 * 60 * 1000;

/// Navbatdagi (kadrsiz, videosi omborda tayyor) qismlar shartini hosil qiladi; `?` lar: oyna boshi, hozir, hozir (ijara).
const CANDIDATE_WHERE: &str =
    "COALESCE(e.collage_file_id,'')='' AND e.created_at > ?
     AND NOT EXISTS (SELECT 1 FROM encode_jobs j WHERE j.anime_id=e.anime_id AND j.season_id=e.season_id AND j.epizod_id=e.epizod_id)
     AND (COALESCE(e.q1080_vd_ids,'')<>'' OR COALESCE(e.q720_vd_ids,'')<>'' OR COALESCE(e.q480_vd_ids,'')<>'' OR COALESCE(e.q360_vd_ids,'')<>'')
     AND (f.anime_id IS NULL OR (f.attempts < 5 AND f.lease_until <= ?))";

const JOIN: &str = "FROM epizod_db e LEFT JOIN frames_q f
        ON f.anime_id=e.anime_id AND f.season_id=e.season_id AND f.epizod_id=e.epizod_id";

/// Eng yuqori sifatdagi (omborda tayyor) fayl nomi va sifat belgisi.
fn best_quality(r: &Value) -> Option<(String, &'static str)> {
    for (q, ids, name) in [("1080p", "q1080_vd_ids", "url_1080p"), ("720p", "q720_vd_ids", "url_720p"),
                           ("480p", "q480_vd_ids", "url_480p"), ("360p", "q360_vd_ids", "url_360p")] {
        let n = r[name].as_str().unwrap_or("").trim();
        if !n.is_empty() && !r[ids].as_str().unwrap_or("").trim().is_empty() {
            return Some((n.to_string(), q));
        }
    }
    None
}

/// Navbatda qism bo'lsa-yu, uni hech kim ishlamayotgan bo'lsa — `frames.yml` ni ishga tushiradi. Natija matni (log uchun).
pub(crate) async fn kick(env: &Env) -> String {
    let now = now_ms();
    let res = turso_exec(env,
        &format!("SELECT COUNT(*) AS n {JOIN} WHERE {CANDIDATE_WHERE}"),
        vec![TursoArg::int(now - WINDOW_MS), TursoArg::int(now)]).await;
    let Some(r) = res.ok().and_then(|r| first_row(&r)) else {
        return "kadrlar navbatini o'qib bo'lmadi".into();
    };
    if jint(&r, "n") == 0 {
        return String::new();
    }
    let repo = tg_secret(env, "GH_REPO");
    if !repo.contains('/') {
        return "GH_REPO o'rnatilmagan".into();
    }
    match gh_workflow_busy(env, &repo, FRAMES_WORKFLOW).await {
        Ok(true) => return "kadrlar workflow'i ishlayapti".into(),
        Ok(false) => {}
        Err(e) => return e,
    }
    // Ikki joydan bir vaqtda ikkita run ochilmasin — atomik belgi.
    let claim = turso_exec(env,
        "INSERT INTO app_config (cfg_key,cfg_value) VALUES ('frames_kicked_at', ?)
         ON CONFLICT(cfg_key) DO UPDATE SET cfg_value=excluded.cfg_value
           WHERE CAST(app_config.cfg_value AS INTEGER) < ?
         RETURNING cfg_key",
        vec![TursoArg::text(&now.to_string()), TursoArg::int(now - 3 * 60 * 1000)]).await;
    if claim.ok().and_then(|r| first_row(&r)).is_none() {
        return "kadrlar workflow'i hozirgina ishga tushirilgan".into();
    }
    match gh_api(env, Method::Post, &format!("/repos/{repo}/actions/workflows/{FRAMES_WORKFLOW}/dispatches"),
        Some(json!({"ref": "main"}))).await {
        Ok((204, _)) => "kadrlar workflow'i ishga tushirildi".into(),
        Ok((code, v)) => format!("kadrlar workflow'i ishga tushmadi (GitHub {code}): {}", v["message"].as_str().unwrap_or("")),
        Err(e) => format!("kadrlar workflow'i ishga tushmadi: {e}"),
    }
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

    if path == "/api/frames/claim" {
        let runner = b["runner"].as_str().unwrap_or("").trim().to_string();
        if runner.is_empty() || runner.len() > 64 {
            return json_resp(&json!({"error": "runner"}), 400);
        }
        for _ in 0..5 {
            let res = turso_exec(env,
                &format!("SELECT e.anime_id, e.season_id, e.epizod_id, e.epizod_number,
                    e.url_1080p, e.url_720p, e.url_480p, e.url_360p,
                    e.q1080_vd_ids, e.q720_vd_ids, e.q480_vd_ids, e.q360_vd_ids
                    {JOIN} WHERE {CANDIDATE_WHERE} ORDER BY e.created_at DESC LIMIT 1"),
                vec![TursoArg::int(now - WINDOW_MS), TursoArg::int(now)]).await?;
            let Some(e) = first_row(&res) else {
                return ok(json!({"none": true}));
            };
            let (a, s, ep) = (jint(&e, "anime_id"), jint(&e, "season_id"), jint(&e, "epizod_id"));
            let Some((name, quality)) = best_quality(&e) else {
                // Omborda tayyor sifat yo'q (nomi bo'sh): bu qismni vaqtincha chetlab o'tamiz.
                let _ = turso_exec(env,
                    "INSERT INTO frames_q (anime_id,season_id,epizod_id,lease_until,attempts,error) VALUES (?,?,?,?,1,'sifat nomi yo''q')
                     ON CONFLICT(anime_id,season_id,epizod_id) DO UPDATE SET lease_until=excluded.lease_until, attempts=frames_q.attempts+1",
                    vec![TursoArg::int(a), TursoArg::int(s), TursoArg::int(ep), TursoArg::int(now + RETRY_MS)]).await;
                continue;
            };
            let got = turso_exec(env,
                "INSERT INTO frames_q (anime_id,season_id,epizod_id,runner,lease_until,attempts) VALUES (?,?,?,?,?,1)
                 ON CONFLICT(anime_id,season_id,epizod_id) DO UPDATE
                   SET runner=excluded.runner, lease_until=excluded.lease_until, attempts=frames_q.attempts+1
                   WHERE frames_q.lease_until <= ?
                 RETURNING attempts",
                vec![TursoArg::int(a), TursoArg::int(s), TursoArg::int(ep), TursoArg::text(&runner),
                     TursoArg::int(now + LEASE_MS), TursoArg::int(now)]).await?;
            let Some(got) = first_row(&got) else { continue };
            return ok(json!({"job": {
                "anime_id": a, "season_id": s, "epizod_id": ep, "epizod_number": jint(&e, "epizod_number"),
                "name": name, "quality": quality, "attempt": jint(&got, "attempts"),
            }}));
        }
        return ok(json!({"none": true}));
    }

    if path == "/api/frames/fail" {
        let (a, s, ep) = (jint(&b, "anime_id"), jint(&b, "season_id"), jint(&b, "epizod_id"));
        let why: String = b["error"].as_str().unwrap_or("").chars().take(300).collect();
        let r = turso_exec(env,
            "UPDATE frames_q SET lease_until=?, error=? WHERE anime_id=? AND season_id=? AND epizod_id=? RETURNING attempts",
            vec![TursoArg::int(now + RETRY_MS), TursoArg::text(&why), TursoArg::int(a), TursoArg::int(s), TursoArg::int(ep)]).await?;
        if let Some(r) = first_row(&r) {
            if jint(&r, "attempts") >= MAX_ATTEMPTS {
                tg_send(env, ADMIN_TELEGRAM_ID, &format!(
                    "\u{26A0}\u{FE0F} Kadrlar olinmadi ({a}/{s}/{ep}): {MAX_ATTEMPTS} marta urinildi.\n<code>{}</code>", html_escape(&why))).await;
            }
        }
        return ok(json!({"ok": true}));
    }

    err404("topilmadi")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eng_yuqori_tayyor_sifat_tanlanadi() {
        let r = json!({"url_1080p": "a_1080.mp4", "q1080_vd_ids": "", "url_720p": "a_720.mp4", "q720_vd_ids": "X,Y",
                       "url_480p": "a_480.mp4", "q480_vd_ids": "Z"});
        assert_eq!(best_quality(&r), Some(("a_720.mp4".to_string(), "720p")));
        let r = json!({"url_1080p": "a_1080.mp4", "q1080_vd_ids": "X"});
        assert_eq!(best_quality(&r), Some(("a_1080.mp4".to_string(), "1080p")));
        assert_eq!(best_quality(&json!({})), None);
    }
}
