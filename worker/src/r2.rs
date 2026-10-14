// ══════════════════════════════════════════════════════════════
//  R2 ORALIQ OMBORI: kodlovchi -> worker -> R2 -> yuklovchi workflow -> Telegram (bot) -> baza
// ══════════════════════════════════════════════════════════════
//
// TALAB (foydalanuvchi): tayyor fayllar avval Cloudflare R2 (`arumediatv` bucket) ga yuklansin; ALOHIDA workflow
// (`upload.yml`) ularni R2'dan olib Telegram'ga (asosiy bot chatiga) yuklasin; baza to'liq yozilgach fayl R2'dan o'chsin.
// Shunda kodlash Telegram yuklashini kutmaydi, yuklash qotib qolsa fayl R2'da turaveradi va qayta uriniladi.
//
// R2 BILAN ALOQA — WORKER'NING O'Z BINDING'I (`R2`, deploy paytida `wrangler.toml` ga qo'shiladi): hech qanday R2/S3 kaliti
// yoki token KERAK EMAS. Run'lar faqat worker manziliga so'rov yuboradi (`ENCODE_TOKEN` — worker yo'llarini himoyalaydi):
//   * kodlovchi: `/api/encode/r2put` ("shuncha MB sig'adimi?") -> `PUT /api/r2/part` (32 MB bo'laklar, multipart)
//     -> `/api/encode/r2ready` (yig'ish va hajmni tekshirish);
//   * yuklovchi: `/api/r2/claim` -> `GET /api/r2/get` (Range bo'laklar) -> Telegram -> `/api/r2/done`.
// Bo'laklar kichik (worker so'rov tanasi chegarasi 100 MB va xotira 128 MB dan past), shu sabab katta fayl ham o'tadi.
//
// Holat jadvali `r2_files`: `uploading` (kodlovchi yozyapti) -> `ready` (R2'da, navbatda) -> `sending` (yuklovchi oldi).
// Bot faylni yopiq kanalga saqlab bazaga yozgach (`encupload::on_file`) `finish` R2 obyekti va qatorni o'chiradi.

use super::*;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use worker::{Bucket, FixedLengthStream, Range, UploadedPart};

// ── IKKI REJIM ───────────────────────────────────────────────
//   1. TO'G'RIDAN-TO'G'RI (S3 kalitlari bor: `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY`, `R2_ACCOUNT_ID`): worker imzolangan
//      (AWS SigV4 presigned) PUT/GET havolalarini beradi, run R2 bilan to'g'ridan-to'g'ri gaplashadi — fayl baytlari worker'dan
//      O'TMAYDI, so'rovlar soni minimal. Kalitlar `tool/r2/r2_keys.enc` da (deploy `ENCODE_TOKEN` bilan ochib secret qiladi).
//   2. WORKER ORQALI (faqat `R2` binding): kalit kerak emas, baytlar worker'dan oqim bilan o'tadi (quyida).
// Kalitlar bor bo'lsa 1-rejim ishlatiladi.

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

/// S3 kalitlari bormi (1-rejim).
fn direct(env: &Env) -> bool {
    !tg_secret(env, "R2_ACCESS_KEY_ID").is_empty() && !tg_secret(env, "R2_SECRET_ACCESS_KEY").is_empty()
        && !tg_secret(env, "R2_ACCOUNT_ID").is_empty()
}

/// Imzolangan havola (`method` — GET/PUT/HEAD/DELETE) `arumediatv/<file>` uchun.
fn signed_url(env: &Env, method: &str, file: &str, expires: i64) -> Option<String> {
    if !direct(env) || !tg_safe_name(file) {
        return None;
    }
    let host = format!("{}.r2.cloudflarestorage.com", tg_secret(env, "R2_ACCOUNT_ID"));
    Some(presign(method, &host, &format!("/arumediatv/{file}"), &tg_secret(env, "R2_ACCESS_KEY_ID"),
        &tg_secret(env, "R2_SECRET_ACCESS_KEY"), "auto", &amz_date(now_ms()), expires))
}

async fn signed_call(env: &Env, method: &str, file: &str) -> Option<(u16, Option<i64>)> {
    let u = signed_url(env, method, file, 300)?;
    let mut init = RequestInit::new();
    init.with_method(if method == "HEAD" { Method::Head } else { Method::Delete });
    let r = Fetch::Request(Request::new_with_init(&u, &init).ok()?).send().await.ok()?;
    let len = r.headers().get("content-length").ok().flatten().and_then(|v| v.parse::<i64>().ok());
    Some((r.status_code(), len))
}

/// Faylning R2'dagi hajmi (binding bo'lsa u, aks holda imzolangan HEAD).
async fn head_size(env: &Env, file: &str) -> Option<i64> {
    if let Some(b) = bucket(env) {
        return b.head(file).await.ok().flatten().map(|o| o.size() as i64);
    }
    match signed_call(env, "HEAD", file).await {
        Some((200, len)) => len,
        _ => None,
    }
}

const WORKFLOW: &str = "upload.yml";
/// Kodlovchi bo'lak hajmi (multipart: oxirgisidan boshqalari bir xil va kamida 5 MiB bo'lishi shart). Bo'lak xotiraga
/// YIGILMAYDI — so'rov tanasi oqim (stream) bilan to'g'ridan-to'g'ri R2'ga uzatiladi, shu sabab chegara faqat Cloudflare'ning
/// so'rov tanasi limiti (Free/Pro 100 MB): standart 90 MB (`R2_PART_MB` bilan o'zgartiriladi; Business/Enterprise'da kattaroq mumkin).
pub(crate) fn part_size(env: &Env) -> i64 {
    tg_secret(env, "R2_PART_MB").parse::<i64>().unwrap_or(90).clamp(5, 5000) * 1024 * 1024
}
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
    if config_get(env, "mig_r2b").await.is_some() {
        return;
    }
    let _ = turso_exec(env, DDL, vec![]).await;
    // Multipart yuklash identifikatori (birinchi variantda yo'q edi).
    let _ = turso_exec(env, "ALTER TABLE r2_files ADD COLUMN upload_id TEXT DEFAULT ''", vec![]).await;
    config_put(env, "mig_r2b", "1").await;
}

fn bucket(env: &Env) -> Option<Bucket> {
    env.bucket("R2").ok()
}

/// R2 ishlatsa bo'ladimi: binding (worker orqali) YOKI S3 kalitlari (to'g'ridan-to'g'ri). Yo'q bo'lsa run'lar eski usulda yuklaydi.
pub(crate) fn ready(env: &Env) -> bool {
    bucket(env).is_some() || direct(env)
}

// ── Kodlovchi tomoni ─────────────────────────────────────────

/// Kodlovchi faylni R2'ga yozmoqchi ("shuncha MB sig'adimi?"): joy bo'lsa qator ochiladi, multipart yuklash boshlanadi va
/// `upload_id` qaytadi; joy yo'q bo'lsa `Err("full")` — kodlovchi R2 bo'shaguncha KUTADI; R2 yo'q yoki fayl butunlay
/// sig'maydigan bo'lsa `Err("na")` — kodlovchi to'g'ridan-to'g'ri bot chatiga yuklaydi. Sig'im: `R2_MAX_GB` (standart 8).
pub(crate) async fn register(env: &Env, (a, s, e): (i64, i64, i64), queued_at: i64, q: &str, kind: &str, file: &str,
        key: &str, size: i64) -> std::result::Result<Value, &'static str> {
    migrate(env).await;
    if !ready(env) {
        return Err("na");
    }
    let cap: i64 = tg_secret(env, "R2_MAX_GB").parse::<i64>().unwrap_or(8).max(1) * 1024 * 1024 * 1024;
    if size > cap || !tg_safe_name(file) {
        return Err("na");
    }
    // Sig'im tekshiruvi va qator ochish BITTA so'rovda (4 ta run bir vaqtda so'rasa ham chegara oshmaydi).
    let r = turso_exec(env,
        "INSERT INTO r2_files (file_name, kind, anime_id, season_id, epizod_id, queued_at, quality, file_key, size, state, created_at)
         SELECT ?,?,?,?,?,?,?,?,?,'uploading',?
          WHERE (SELECT COALESCE(SUM(size),0) FROM r2_files WHERE file_name<>?) + ? <= ?
         ON CONFLICT(file_name) DO UPDATE SET kind=excluded.kind, file_key=excluded.file_key, size=excluded.size,
            state='uploading', runner='', lease_until=0, attempts=0, error='', created_at=excluded.created_at
         RETURNING file_name",
        vec![TursoArg::text(file), TursoArg::text(kind), TursoArg::int(a), TursoArg::int(s), TursoArg::int(e),
             TursoArg::int(queued_at), TursoArg::text(q), TursoArg::text(key), TursoArg::int(size), TursoArg::int(now_ms()),
             TursoArg::text(file), TursoArg::int(size), TursoArg::int(cap)]).await.map_err(|_| "na")?;
    if first_row(&r).is_none() {
        return Err("full");
    }
    // 1-rejim: imzolangan PUT havolasi (2 soat) — run R2'ga to'g'ridan-to'g'ri yozadi.
    if let Some(put) = signed_url(env, "PUT", file, 7200) {
        return Ok(json!({"url": put}));
    }
    // 2-rejim: worker orqali multipart (binding).
    let Some(b) = bucket(env) else { return Err("na") };
    let up = b.create_multipart_upload(file).execute().await.map_err(|_| "na")?;
    let id = up.upload_id().await;
    let _ = turso_exec(env, "UPDATE r2_files SET upload_id=? WHERE file_name=?", vec![TursoArg::text(&id), TursoArg::text(file)]).await;
    Ok(json!({"upload_id": id, "part_size": part_size(env)}))
}

/// `PUT /api/r2/part?file=..&n=..` — bitta bo'lak (so'rov tanasi, OQIM bilan) R2 multipart yuklashiga yoziladi; `etag` qaytadi.
async fn put_part(mut req: Request, env: &Env) -> Result<Response> {
    let url = req.url()?;
    let q = |k: &str| url.query_pairs().find(|(a, _)| a == k).map(|(_, v)| v.to_string()).unwrap_or_default();
    let (file, n) = (q("file"), q("n").parse::<u16>().unwrap_or(0));
    let Some(row) = row(env, &file).await.filter(|r| r["state"].as_str() == Some("uploading")) else {
        return json_resp(&json!({"error": "qator"}), 404);
    };
    let id = row["upload_id"].as_str().unwrap_or("").to_string();
    let Some(b) = bucket(env) else { return json_resp(&json!({"error": "r2"}), 503) };
    if n == 0 || id.is_empty() {
        return json_resp(&json!({"error": "bolak"}), 400);
    }
    // OQIM: tana xotiraga yig'ilmaydi, R2'ga to'g'ridan-to'g'ri uzatiladi (Content-Length shart).
    let len = req.headers().get("Content-Length").ok().flatten().and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
    if len == 0 || len as i64 > part_size(env) {
        return json_resp(&json!({"error": "hajm"}), 400);
    }
    let stream = FixedLengthStream::wrap(req.stream()?, len);
    let part = b.resume_multipart_upload(&file, id)?.upload_part(n, stream).await?;
    ok(json!({"ok": true, "etag": part.etag(), "n": part.part_number()}))
}

/// Kodlovchi hamma bo'lakni yozdi: yig'iladi (`complete`), hajm tekshiriladi, qator `ready` (yuklovchi olishi mumkin).
pub(crate) async fn mark_ready(env: &Env, file: &str, size: i64, parts: &Value) -> std::result::Result<(), String> {
    let Some(row) = row(env, file).await.filter(|r| r["state"].as_str() == Some("uploading")) else {
        return Err("qator".into());
    };
    let id = row["upload_id"].as_str().unwrap_or("").to_string();
    // 2-rejim (multipart): bo'laklar yig'iladi. 1-rejimda (to'g'ridan-to'g'ri PUT) `upload_id` bo'sh — faqat hajm tekshiriladi.
    if !id.is_empty() {
        let Some(b) = bucket(env) else { return Err("r2_yoq".into()) };
        let list: Vec<UploadedPart> = parts.as_array().map(|a| a.iter().filter_map(|p| {
            Some(UploadedPart::new(p["n"].as_u64()? as u16, p["etag"].as_str()?.to_string()))
        }).collect()).unwrap_or_default();
        if list.is_empty() {
            return Err("bolaklar".into());
        }
        let up = b.resume_multipart_upload(file, id).map_err(|e| e.to_string())?;
        up.complete(list).await.map_err(|e| format!("yigish: {e}"))?;
    }
    let got = head_size(env, file).await;
    if got != Some(size) {
        finish(env, file).await;
        return Err(format!("r2_hajm ({got:?} != {size})"));
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
    if let Some(b) = bucket(env) {
        let _ = b.delete(file).await;
    } else {
        let _ = signed_call(env, "DELETE", file).await;
    }
    let _ = turso_exec(env, "DELETE FROM r2_files WHERE file_name=?", vec![TursoArg::text(file)]).await;
}

pub(crate) async fn row(env: &Env, file: &str) -> Option<Value> {
    turso_exec(env, "SELECT * FROM r2_files WHERE file_name=?", vec![TursoArg::text(file)]).await.ok().and_then(|r| first_row(&r))
}

// ── Yuklovchi workflow ───────────────────────────────────────

/// Navbatda tayyor fayl bo'lsa va `upload.yml` ishlamayotgan bo'lsa — ishga tushiradi.
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

/// Cron: eskirgan qatorlar (yuklab bo'lmagan `uploading` 3 soatdan, `ready`/`sending` 24 soatdan eski) R2'dan o'chadi;
/// Telegram'ga 6 urinishda ham yetmagan fayl R2'dan o'chib, sifat qayta kodlanadi (kodlash urinishi tugamagan bo'lsa).
pub(crate) async fn cleanup(env: &Env) {
    if !ready(env) || config_get(env, "mig_r2b").await.is_none() {
        return;
    }
    let now = now_ms();
    if let Ok(res) = turso_exec(env,
        "SELECT file_name FROM r2_files WHERE (state='uploading' AND created_at<?) OR created_at<? LIMIT 10",
        vec![TursoArg::int(now - 3 * 3600 * 1000), TursoArg::int(now - 24 * 3600 * 1000)]).await {
        for r in rows_of(&res) {
            if let Some(f) = r["file_name"].as_str() {
                finish(env, f).await;
            }
        }
    }
    let Ok(res) = turso_exec(env,
        "SELECT file_name, anime_id, season_id, epizod_id, queued_at, quality FROM r2_files WHERE state='error' LIMIT 5",
        vec![]).await else { return };
    for r in rows_of(&res) {
        let (a, s, e, qa) = (jint(&r, "anime_id"), jint(&r, "season_id"), jint(&r, "epizod_id"), jint(&r, "queued_at"));
        if let Some(f) = r["file_name"].as_str() {
            finish(env, f).await;
        }
        let _ = turso_exec(env,
            "UPDATE encode_q SET state='queued', runner='', lease_until=0, progress='', error=''
              WHERE anime_id=? AND season_id=? AND epizod_id=? AND queued_at=? AND quality=? AND attempts<?",
            vec![TursoArg::int(a), TursoArg::int(s), TursoArg::int(e), TursoArg::int(qa),
                 TursoArg::text(r["quality"].as_str().unwrap_or("")), TursoArg::int(ENCODE_MAX_ATTEMPTS)]).await;
    }
}

/// `GET /api/r2/get?file=..&offset=..&length=..` — R2'dan bo'lak (Range) beradi (yuklovchi).
async fn get_part(req: Request, env: &Env) -> Result<Response> {
    let url = req.url()?;
    let q = |k: &str| url.query_pairs().find(|(a, _)| a == k).map(|(_, v)| v.to_string()).unwrap_or_default();
    let file = q("file");
    if !tg_safe_name(&file) {
        return json_resp(&json!({"error": "nom"}), 400);
    }
    let Some(b) = bucket(env) else { return json_resp(&json!({"error": "r2"}), 503) };
    let (offset, length) = (q("offset").parse::<u64>().unwrap_or(0), q("length").parse::<u64>().unwrap_or(0));
    // Butun fayl BITTA oqim (stream) bilan; uzilsa yuklovchi `offset` dan davom ettiradi (`length` — ixtiyoriy).
    let mut g = b.get(&file);
    if length > 0 {
        g = g.range(Range::OffsetWithLength { offset, length });
    } else if offset > 0 {
        g = g.range(Range::OffsetToEnd { offset });
    }
    let Some(obj) = g.execute().await? else { return json_resp(&json!({"error": "yoq"}), 404) };
    let total = obj.size();
    let want = if length > 0 { length.min(total.saturating_sub(offset)) } else { total.saturating_sub(offset) };
    let Some(body) = obj.body() else { return json_resp(&json!({"error": "tana"}), 404) };
    let resp = Response::from_body(body.response_body()?)?;
    let h = Headers::new();
    h.set("Content-Length", &want.to_string())?;
    h.set("Content-Type", "application/octet-stream")?;
    h.set("Cache-Control", "no-store")?;
    Ok(resp.with_headers(h))
}

/// `X-Encode-Token` bilan: `PUT /api/r2/part`, `GET /api/r2/get`, `POST /api/r2/{claim,beat,fail,done,diag}`.
pub(crate) async fn route(mut req: Request, env: &Env, path: &str) -> Result<Response> {
    if !encode_token_ok(&req, env) {
        return json_resp(&json!({"error": "forbidden"}), 403);
    }
    migrate(env).await;
    match (req.method(), path) {
        (Method::Put, "/api/r2/part") => return put_part(req, env).await,
        (Method::Get, "/api/r2/get") => return get_part(req, env).await,
        (Method::Post, _) => {}
        _ => return err404("topilmadi"),
    }
    let b: Value = req.json().await.unwrap_or(json!({}));
    let file = b["file"].as_str().unwrap_or("").to_string();
    let now = now_ms();
    match path {
        "/api/r2/claim" => {
            if !ready(env) {
                return ok(json!({"none": true}));
            }
            // Yuklovchi ESKI umumiy sessiya bilan ishlaydi: akkaunt ID'si shu yerda ro'yxatga olinadi (bot undan fayl qabul qiladi).
            if let Some(uid) = b["uid"].as_i64().filter(|u| *u > 0) {
                if config_get(env, "sess_uid_upload").await.as_deref() != Some(uid.to_string().as_str()) {
                    config_put(env, "sess_uid_upload", &uid.to_string()).await;
                }
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
            let (a, s, e, qa) = (jint(&r, "anime_id"), jint(&r, "season_id"), jint(&r, "epizod_id"), jint(&r, "queued_at"));
            let (kind, q, key, size) = (r["kind"].as_str().unwrap_or(""), r["quality"].as_str().unwrap_or(""),
                r["file_key"].as_str().unwrap_or(""), jint(&r, "size"));
            return ok(json!({
                // 1-rejimda R2'dan to'g'ridan-to'g'ri olish havolasi (2 soat); aks holda yuklovchi worker'dan oqim bilan oladi.
                "url": signed_url(env, "GET", &name, 7200),
                "file": name, "size": size, "bot": bot_username(env).await,
                "caption": format!("ARUENC|{kind}|{a}|{s}|{e}|{qa}|{q}|{key}|{size}"),
                "attempt": jint(&r, "attempts"),
            }));
        }
        // Tashxis: R2 binding'i bormi va bucket'ga yetib boradimi.
        "/api/r2/diag" => {
            if !ready(env) {
                return ok(json!({"ready": false}));
            }
            let binding = match bucket(env) {
                Some(bk) => Some(bk.list().limit(1).execute().await.is_ok()),
                None => None,
            };
            let head = if direct(env) {
                let host = format!("{}.r2.cloudflarestorage.com", tg_secret(env, "R2_ACCOUNT_ID"));
                let u = presign("HEAD", &host, "/arumediatv", &tg_secret(env, "R2_ACCESS_KEY_ID"),
                    &tg_secret(env, "R2_SECRET_ACCESS_KEY"), "auto", &amz_date(now), 300);
                let mut init = RequestInit::new();
                init.with_method(Method::Head);
                match Request::new_with_init(&u, &init) {
                    Ok(rq) => Some(Fetch::Request(rq).send().await.map(|r| r.status_code() as i64).unwrap_or(-1)),
                    Err(_) => Some(-2),
                }
            } else {
                None
            };
            return ok(json!({"ready": true, "direct": direct(env), "binding_ok": binding, "s3_bucket_status": head}));
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
