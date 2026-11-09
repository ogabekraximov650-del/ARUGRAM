//! UZ: ARUDRIVE uslubidagi video ombori (`vd`): har sifat fayli 18 MiB bo'laklarga bo'linadi, hamma bo'lak BITTA kalit bilan
//!     (AES-128-CTR, hisoblagich fayl bo'ylab davom etadi — bo'lak boshi `n * 18 MiB / 16` blokka to'g'ri keladi) shifrlanadi.
//!     Tayyorlangan bo'laklar Actions orqali R2 ga yoziladi (`vd/c/<nom>/<n>`); ALOHIDA yuklovchi workflow (`vdstore.yml`) hammasini
//!     Bot API bilan Telegram kanaliga saqlaydi va `file_id` larni bazaga yozadi. Ko'rish: bo'lak Telegram -> Cloudflare kesh
//!     (`vcache.rs`, oyna = 1 bo'lak), ilova shifrni o'zi ochadi. Worker kalitni faqat kirgan foydalanuvchiga beradi.
//! EN: ARUDRIVE-style video store: every quality file is split into 18 MiB chunks, all chunks share ONE AES-128-CTR key (the
//!     counter continues across the file, so a chunk is decryptable on its own). Actions stage chunks in R2, a separate uploader
//!     workflow stores them in Telegram via the Bot API and records `file_id`s. Playback: Telegram -> Cloudflare cache window
//!     (one chunk), the app decrypts. The key is only handed to signed-in users.
//! RU: Hranilishche video v stile ARUDRIVE: fayl rezhetsya na kuski po 18 MiB, vse kuski shifruyutsya ODNIM klyuchom AES-128-CTR.
//!     Actions kladut kuski v R2, otdelnyy workflow-zagruzchik sohranyaet ih v Telegram cherez Bot API. Vosproizvedenie: Telegram ->
//!     kesh Cloudflare (okno = 1 kusok), rasshifrovku delaet prilozhenie.
//!
//! Holatlar (`vd_files.state`): `staging` (Actions bo'laklarni R2 ga yozyapti) / `uploading` (ilova butun faylni R2 ga yozyapti) ->
//! `uploaded` (R2 da to'liq, saqlash navbatida) -> `storing` (yuklovchi Telegram'ga yozyapti) -> `ready`. `failed` — saqlab bo'lmadi.
//! Migratsiya navbati `vd_mig`: eski (`tg_files`) sifat fayllari -> yangi kalit va bo'laklar.

use super::*;
use crate::vcache::{
    abort_pipe, close_pipe, fixed_length_pipe, pipe_keep_open, warm_marker_url, warm_window_total, warm_window_url, CHUNK_CACHE_SECONDS,
    WARM_MARKER_SECONDS, WARM_VERIFY_TICKS, WARM_WAIT_STEP_MS, WARM_WAIT_TICKS, WARM_WINDOW,
};
use core::sync::atomic::{AtomicBool, Ordering};
use worker::{Bucket, Range};

pub(crate) const CHUNK: u64 = 18 * 1024 * 1024;
const WORKFLOW: &str = "vdstore.yml";
const MIG_WORKFLOW: &str = "vdmig.yml";
const LEASE_MS: i64 = 45 * 60 * 1000;
const MAX_TRIES: i64 = 4;
const MAX_SIZE: i64 = 50 * 1024 * 1024 * 1024;
/// Ilova BUTUN faylni shu kalitga yozadi (`vd/up/<nom>`).
const SINGLE: &str = "vd/up/";
/// Actions bo'laklarni shu yerga yozadi (`vd/c/<nom>/<n>`).
const PARTS: &str = "vd/c/";

static SCHEMA_OK: AtomicBool = AtomicBool::new(false);

fn bucket(env: &Env) -> Option<Bucket> {
    env.bucket("R2").ok()
}

pub(crate) fn chunks_of(size: i64) -> i64 {
    ((size.max(0) as u64).div_ceil(CHUNK)) as i64
}

fn ids(s: &str) -> Vec<&str> {
    if s.is_empty() { Vec::new() } else { s.split(',').collect() }
}

fn qs(url: &Url, k: &str) -> String {
    url.query_pairs().find(|(a, _)| a == k).map(|(_, v)| v.to_string()).unwrap_or_default()
}

fn part_key(name: &str, n: u64) -> String {
    format!("{PARTS}{name}/{n}")
}

fn bad(msg: &str) -> Result<Response> {
    json_resp(&json!({"error": msg}), 400)
}

// ── Turso o'qishlarini kamaytirish (ARUDRIVE kesh tizimi kabi) ───────────────────────────
//
// `ready` fayl ma'lumoti (hajm, kalit, `file_id` lar) o'zgarmaydi — Cloudflare keshida (Cache API) turadi: `info` va `warm` bazaga faqat
// keshda yo'q bo'lsagina boradi. Fayl nomi HECH QACHON qayta ishlatilmaydi (nomda vaqt tamg'asi bor), shu sabab eskirish faqat o'chirishda
// mumkin (`forget` o'z mintaqasida keshni o'chiradi; boshqa mintaqada eskirgan nusxa META_TTL dan keyin ketadi, `warm` esa baribir xato
// qaytaradi va ilova eski yo'lga o'tadi). Muddat — 1000 kun, ARUDRIVE bilan bir xil.
/// 1000 kun (ARUDRIVE `LIST_TTL`/`CHUNK_CACHE_SECONDS` bilan bir xil). `META_V` — kesh kalitining versiyasi: o'zgartirib qayta deploy qilinsa
/// eski yozuvlarning hammasi ko'rinmay qoladi (masalan, bazani noldan boshlaganda).
const META_TTL: u64 = 1000 * 24 * 3600;
const META_V: &str = "1";

fn meta_url(name: &str) -> String {
    cache_key_url(&format!("vd~{name}"), &format!("meta{META_V}"))
}

async fn meta_get(name: &str) -> Option<Value> {
    let req = Request::new(&meta_url(name), Method::Get).ok()?;
    let mut hit = Cache::default().get(&req, false).await.ok()??;
    hit.json::<Value>().await.ok()
}

async fn meta_put(name: &str, v: &Value) {
    let Ok(mut resp) = Response::from_json(v) else { return };
    let _ = resp.headers_mut().set("Cache-Control", &format!("public, max-age={META_TTL}"));
    if let Ok(req) = Request::new(&meta_url(name), Method::Get) {
        let _ = Cache::default().put(&req, resp).await;
    }
}

async fn meta_drop(name: &str) {
    let _ = Cache::default().delete(&meta_url(name), false).await;
}

/// Qisqa muddatli "bo'sh" belgisi (Cloudflare keshida): cron navbat bo'sh deb bilgan bo'lsa keyingi daqiqalarda bazaga bormaydi.
async fn marker_has(key: &str) -> bool {
    let Ok(req) = Request::new(&cache_key_url("vd~marker", key), Method::Get) else { return false };
    matches!(Cache::default().get(&req, false).await, Ok(Some(_)))
}

async fn marker_set(key: &str, secs: u64) {
    if let (Ok(req), Ok(mut resp)) = (Request::new(&cache_key_url("vd~marker", key), Method::Get), Response::ok("1")) {
        let _ = resp.headers_mut().set("Cache-Control", &format!("public, max-age={secs}"));
        let _ = Cache::default().put(&req, resp).await;
    }
}

/// R2 da bo'laklar uchun ajratilgan sig'im (bayt): `VD_R2_MAX_GB` (standart 4 GB). Qolgani kodlovchilarniki (`r2.rs`).
fn cap_bytes(env: &Env) -> i64 {
    tg_secret(env, "VD_R2_MAX_GB").parse::<i64>().unwrap_or(4).max(1) * 1024 * 1024 * 1024
}

/// Telegram'ga hali saqlanmagan (R2 da turgan) fayllar hajmi.
async fn staged_bytes(env: &Env) -> i64 {
    turso_exec(env, "SELECT COALESCE(SUM(size),0) AS n FROM vd_files WHERE state IN ('staging','uploading','uploaded','storing')", vec![]).await
        .ok().and_then(|r| first_row(&r)).map(|r| jint(&r, "n")).unwrap_or(0)
}

pub(crate) async fn ensure(env: &Env) -> Result<()> {
    if SCHEMA_OK.load(Ordering::Relaxed) {
        return Ok(());
    }
    turso_many(env, &[
        ("CREATE TABLE IF NOT EXISTS vd_files (name TEXT PRIMARY KEY, size INTEGER NOT NULL, chunks INTEGER NOT NULL, vkey TEXT NOT NULL, \
          state TEXT NOT NULL, layout TEXT NOT NULL DEFAULT 'chunks', chunk_ids TEXT NOT NULL DEFAULT '', msg_ids TEXT NOT NULL DEFAULT '', \
          lease INTEGER NOT NULL DEFAULT 0, tries INTEGER NOT NULL DEFAULT 0, created INTEGER NOT NULL)", vec![]),
        ("CREATE INDEX IF NOT EXISTS vd_state ON vd_files(state, created)", vec![]),
        ("CREATE TABLE IF NOT EXISTS vd_mig (name TEXT PRIMARY KEY, state TEXT NOT NULL, lease INTEGER NOT NULL DEFAULT 0, \
          tries INTEGER NOT NULL DEFAULT 0, err TEXT NOT NULL DEFAULT '', queued INTEGER NOT NULL)", vec![]),
        ("CREATE INDEX IF NOT EXISTS vd_mig_state ON vd_mig(state, queued)", vec![]),
    ]).await?;
    SCHEMA_OK.store(true, Ordering::Relaxed);
    Ok(())
}

// ── Workflow'larni ishga tushirish ───────────────────────────

async fn kick_wf(env: &Env, wf: &str) -> String {
    let repo = tg_secret(env, "GH_REPO");
    if !repo.contains('/') {
        return "GH_REPO yo'q".into();
    }
    match gh_workflow_busy(env, &repo, wf).await {
        Ok(true) => return String::new(),
        Ok(false) => {}
        Err(e) => return e,
    }
    match gh_api(env, Method::Post, &format!("/repos/{repo}/actions/workflows/{wf}/dispatches"), Some(json!({"ref": "main"}))).await {
        Ok((204, _)) => format!("{wf} ishga tushirildi"),
        Ok((code, v)) => format!("{wf} ishga tushmadi (GitHub {code}): {}", v["message"].as_str().unwrap_or("")),
        Err(e) => format!("{wf} ishga tushmadi: {e}"),
    }
}

/// Cron: saqlash yoki migratsiya navbati bo'lsa tegishli workflow uyg'otiladi (bo'sh bo'lsa GitHub'ga so'rov yo'q), eskirganlar tozalanadi.
pub(crate) async fn cron(env: &Env) {
    if bucket(env).is_none() {
        return;
    }
    // Navbat bo'sh bo'lsa (50 daqiqalik belgi) bazaga umuman borilmaydi; yangi ish kelganda (`finish`, `up_complete`, `enqueue_one`)
    // workflow to'g'ridan-to'g'ri ishga tushiriladi, shu sabab belgi ishni kechiktirmaydi.
    if !marker_has("idle").await {
        if ensure(env).await.is_err() {
            return;
        }
        let now = now_ms();
        let Some(r) = turso_exec(env,
            "SELECT (SELECT COUNT(*) FROM vd_files WHERE state='uploaded' OR (state='storing' AND lease<?)) AS store,
                    (SELECT COUNT(*) FROM vd_mig WHERE state='queued' OR (state='running' AND lease<?)) AS mig,
                    (SELECT COUNT(*) FROM vd_files WHERE state IN ('storing','staging','uploading') AND lease>=?) AS busy",
            vec![TursoArg::int(now), TursoArg::int(now), TursoArg::int(now)]).await.ok().and_then(|r| first_row(&r)) else { return };
        if jint(&r, "store") > 0 {
            let _ = kick_wf(env, WORKFLOW).await;
        }
        if jint(&r, "mig") > 0 && staged_bytes(env).await < cap_bytes(env) {
            let _ = kick_wf(env, MIG_WORKFLOW).await;
        }
        if jint(&r, "store") == 0 && jint(&r, "mig") == 0 && jint(&r, "busy") == 0 {
            marker_set("idle", 50 * 60).await;
        }
    }
    // Har 6 soatda: hali yangi tizimda yo'q hamma eski sifat fayllari migratsiya navbatiga qo'yiladi (`tg_files` skaneri — qimmat, shu sabab kamdan-kam).
    if !marker_has("enq").await {
        marker_set("enq", 6 * 3600).await;
        if ensure(env).await.is_ok() && enqueue_all(env).await.unwrap_or(0) > 0 && staged_bytes(env).await < cap_bytes(env) {
            marker_set("idle", 1).await;
            let _ = kick_wf(env, MIG_WORKFLOW).await;
        }
    }
    // Tozalash (bazaga bitta so'rov) 6 soatlik davr bilan bir xil.
    if !marker_has("clean").await {
        marker_set("clean", 6 * 3600).await;
        cleanup(env).await;
    }
}

/// 24 soatdan eski, tugamagan `staging`/`uploading` yuklashlar R2 dan va bazadan o'chadi. `ready`/`storing` ga tegilmaydi.
async fn cleanup(env: &Env) {
    let Some(bk) = bucket(env) else { return };
    let cutoff = now_ms() - 24 * 3600 * 1000;
    let Some(rows) = turso_exec(env, "SELECT name FROM vd_files WHERE state IN ('staging','uploading','failed') AND created<? LIMIT 20",
        vec![TursoArg::int(cutoff)]).await.ok().map(|r| rows_of(&r)) else { return };
    for row in rows {
        let name = row["name"].as_str().unwrap_or("").to_string();
        if tg_safe_name(&name) {
            r2_delete_all(&bk, &name).await;
            let _ = turso_exec(env, "DELETE FROM vd_files WHERE name=? AND state IN ('staging','uploading','failed')", vec![TursoArg::text(&name)]).await;
        }
    }
}

/// Faylning R2 dagi hamma bo'laklarini (va butun nusxasini) o'chiradi.
async fn r2_delete_all(bk: &Bucket, name: &str) {
    let _ = bk.delete(format!("{SINGLE}{name}")).await;
    if let Ok(list) = bk.list().prefix(format!("{PARTS}{name}/")).limit(1000).execute().await {
        for o in list.objects() {
            let _ = bk.delete(o.key()).await;
        }
    }
}

// ── Migratsiya navbati ───────────────────────────────────────

/// Eski (`tg_files`) sifat fayllarini navbatga qo'yadi: faqat `ep_` kodlangan MP4 (fMP4 `_f.mp4` — Mini App uchun, tegilmaydi),
/// kalitli va hali `vd_files` da yo'qlari. Qaytadi: navbatdagi (queued) fayllar soni.
pub(crate) async fn enqueue_all(env: &Env) -> Result<i64> {
    ensure(env).await?;
    turso_exec(env,
        "INSERT OR IGNORE INTO vd_mig (name, state, queued) \
         SELECT file_name, 'queued', ? FROM tg_files \
         WHERE file_name LIKE 'ep\\_%' ESCAPE '\\' AND file_name NOT LIKE '%\\_f.mp4' ESCAPE '\\' AND file_key<>'' AND msg_id>0 \
           AND file_name NOT IN (SELECT name FROM vd_files)",
        vec![TursoArg::int(now_ms())]).await?;
    let r = turso_exec(env, "SELECT COUNT(*) AS n FROM vd_mig WHERE state IN ('queued','running')", vec![]).await?;
    Ok(first_row(&r).map(|r| jint(&r, "n")).unwrap_or(0))
}

/// Yangi kodlangan sifat (`encupload::on_file`) — shu faylni ham yangi tizimga o'tkazish uchun navbatga qo'yadi (xato e'tiborsiz).
pub(crate) async fn enqueue_one(env: &Env, name: &str) {
    if !tg_safe_name(name) || name.ends_with("_f.mp4") || !name.starts_with("ep_") || ensure(env).await.is_err() {
        return;
    }
    let _ = turso_exec(env,
        "INSERT OR IGNORE INTO vd_mig (name, state, queued) SELECT ?, 'queued', ? WHERE NOT EXISTS (SELECT 1 FROM vd_files WHERE name=?)",
        vec![TursoArg::text(name), TursoArg::int(now_ms()), TursoArg::text(name)]).await;
    let _ = kick_wf(env, MIG_WORKFLOW).await;
}

/// Qism/sifat o'chirilganda: R2 bo'laklari, kanal xabarlari va baza yozuvi ketadi.
pub(crate) async fn forget(env: &Env, name: &str) {
    if !tg_safe_name(name) {
        return;
    }
    meta_drop(name).await;
    if ensure(env).await.is_err() {
        return;
    }
    let row = turso_exec(env, "SELECT msg_ids FROM vd_files WHERE name=?", vec![TursoArg::text(name)]).await.ok().and_then(|r| first_row(&r));
    let _ = turso_exec(env, "DELETE FROM vd_mig WHERE name=?", vec![TursoArg::text(name)]).await;
    let Some(row) = row else { return };
    if let Some(bk) = bucket(env) {
        r2_delete_all(&bk, name).await;
    }
    let channel = tg_channel_id(env);
    if channel != 0 {
        for id in ids(row["msg_ids"].as_str().unwrap_or("")).into_iter().take(400) {
            if let Ok(m) = id.parse::<i64>() {
                let _ = tg_api(env, "deleteMessage", json!({"chat_id": channel, "message_id": m})).await;
            }
        }
    }
    let _ = turso_exec(env, "DELETE FROM vd_files WHERE name=?", vec![TursoArg::text(name)]).await;
    meta_drop(name).await;
}

// ── Actions yo'llari (ENCODE_TOKEN) ──────────────────────────

pub(crate) fn is_actions_path(path: &str) -> bool {
    matches!(path, "/api/vd/mig/enqueue" | "/api/vd/mig/claim" | "/api/vd/mig/fail" | "/api/vd/init" | "/api/vd/put" | "/api/vd/finish"
        | "/api/vd/peek" | "/api/vd/verify" | "/api/vd/claim" | "/api/vd/read" | "/api/vd/chunkdone" | "/api/vd/done" | "/api/vd/fail")
}

pub(crate) async fn route(mut req: Request, env: &Env, path: &str) -> Result<Response> {
    if !encode_token_ok(&req, env) {
        return json_resp(&json!({"error": "forbidden"}), 403);
    }
    let Some(bk) = bucket(env) else { return json_resp(&json!({"error": "r2"}), 503) };
    ensure(env).await?;
    let url = req.url()?;
    let now = now_ms();
    if path == "/api/vd/put" && req.method() == Method::Put {
        let (name, n) = (qs(&url, "name"), qs(&url, "n").parse::<u64>().unwrap_or(u64::MAX));
        if !tg_safe_name(&name) {
            return bad("nom");
        }
        let Some(row) = turso_exec(env, "SELECT size,chunks FROM vd_files WHERE name=? AND state='staging'", vec![TursoArg::text(&name)]).await?
            .pipe_first() else { return json_resp(&json!({"error": "holat"}), 409) };
        let (size, chunks) = (jint(&row, "size") as u64, jint(&row, "chunks") as u64);
        if n >= chunks {
            return bad("bo'lak raqami");
        }
        let want = CHUNK.min(size - n * CHUNK);
        let bytes = req.bytes().await?;
        if bytes.len() as u64 != want {
            return json_resp(&json!({"error": format!("hajm mos emas ({} != {want})", bytes.len())}), 422);
        }
        bk.put(part_key(&name, n), bytes).execute().await?;
        return ok(json!({"ok": true}));
    }
    if path == "/api/vd/read" && req.method() == Method::Get {
        let (name, n) = (qs(&url, "name"), qs(&url, "n").parse::<u64>().unwrap_or(u64::MAX));
        if !tg_safe_name(&name) {
            return err404("yo'q");
        }
        let Some(row) = turso_exec(env, "SELECT size,layout FROM vd_files WHERE name=?", vec![TursoArg::text(&name)]).await?.pipe_first() else {
            return err404("yo'q");
        };
        let size = jint(&row, "size") as u64;
        let offset = n.saturating_mul(CHUNK);
        if offset >= size {
            return json_resp(&json!({"error": "oraliq"}), 416);
        }
        let length = CHUNK.min(size - offset);
        let obj = if row["layout"].as_str() == Some("single") {
            bk.get(format!("{SINGLE}{name}")).range(Range::OffsetWithLength { offset, length }).execute().await?
        } else {
            bk.get(part_key(&name, n)).execute().await?
        };
        let Some(obj) = obj else { return err404("yo'q") };
        let Some(body) = obj.body() else { return err404("tana") };
        let resp = Response::from_body(body.response_body()?)?;
        let h = Headers::new();
        h.set("Content-Length", &length.to_string())?;
        h.set("Content-Type", "application/octet-stream")?;
        h.set("Cache-Control", "no-store")?;
        return Ok(resp.with_headers(h));
    }
    if req.method() != Method::Post {
        return err404("topilmadi");
    }
    let b: Value = req.json().await.unwrap_or(json!({}));
    let name = b["name"].as_str().unwrap_or("").to_string();
    match path {
        "/api/vd/mig/enqueue" => {
            let n = enqueue_all(env).await?;
            let k = if n > 0 { kick_wf(env, MIG_WORKFLOW).await } else { String::new() };
            ok(json!({"queued": n, "kick": k}))
        }
        "/api/vd/mig/claim" => {
            if staged_bytes(env).await >= cap_bytes(env) {
                return ok(json!({"wait": true}));
            }
            turso_many(env, &[
                ("UPDATE vd_mig SET state='queued' WHERE state='running' AND lease<? AND tries<?", vec![TursoArg::int(now), TursoArg::int(MAX_TRIES)]),
                ("UPDATE vd_mig SET state='failed' WHERE state='running' AND lease<? AND tries>=?", vec![TursoArg::int(now), TursoArg::int(MAX_TRIES)]),
            ]).await?;
            for _ in 0..8 {
                let Some(m) = turso_exec(env, "SELECT name FROM vd_mig WHERE state='queued' ORDER BY queued LIMIT 1", vec![]).await?.pipe_first() else {
                    return ok(json!({"none": true}));
                };
                let nm = m["name"].as_str().unwrap_or("").to_string();
                let got = turso_exec(env, "UPDATE vd_mig SET state='running', lease=?, tries=tries+1 WHERE name=? AND state='queued' RETURNING name",
                    vec![TursoArg::int(now + LEASE_MS), TursoArg::text(&nm)]).await?;
                if first_row(&got).is_none() {
                    continue;
                }
                let f = turso_exec(env, "SELECT msg_id,file_key FROM tg_files WHERE file_name=?", vec![TursoArg::text(&nm)]).await?.pipe_first();
                match f {
                    Some(f) if jint(&f, "msg_id") > 0 && valid_file_key(f["file_key"].as_str().unwrap_or("")) => {
                        return ok(json!({"name": nm, "msg_id": f["msg_id"], "key": f["file_key"], "channel": tg_channel_id(env)}));
                    }
                    // Fayl o'chirilgan yoki kalitsiz (ochiq) — migratsiya qilinmaydi.
                    _ => {
                        let _ = turso_exec(env, "DELETE FROM vd_mig WHERE name=?", vec![TursoArg::text(&nm)]).await;
                    }
                }
            }
            ok(json!({"none": true}))
        }
        "/api/vd/mig/fail" => {
            if !tg_safe_name(&name) {
                return bad("nom");
            }
            let why: String = b["error"].as_str().unwrap_or("").chars().take(200).collect();
            turso_exec(env, "UPDATE vd_mig SET state=CASE WHEN tries>=? THEN 'failed' ELSE 'queued' END, lease=0, err=? WHERE name=? AND state='running'",
                vec![TursoArg::int(MAX_TRIES), TursoArg::text(&why), TursoArg::text(&name)]).await?;
            ok(json!({"ok": true}))
        }
        "/api/vd/init" => {
            let size = jint(&b, "size");
            let key = b["key"].as_str().unwrap_or("").to_ascii_lowercase();
            if !tg_safe_name(&name) || size <= 0 || size > MAX_SIZE || !valid_file_key(&key) {
                return bad("so'rov");
            }
            let cur = turso_exec(env, "SELECT state FROM vd_files WHERE name=?", vec![TursoArg::text(&name)]).await?.pipe_first();
            if cur.as_ref().and_then(|r| r["state"].as_str()) == Some("ready") {
                return ok(json!({"ready": true}));
            }
            if staged_bytes(env).await + size > cap_bytes(env) && cur.is_none() {
                return ok(json!({"wait": true}));
            }
            r2_delete_all(&bk, &name).await;
            let chunks = chunks_of(size);
            turso_exec(env,
                "INSERT INTO vd_files (name,size,chunks,vkey,state,layout,created) VALUES (?,?,?,?,'staging','chunks',?) \
                 ON CONFLICT(name) DO UPDATE SET size=excluded.size, chunks=excluded.chunks, vkey=excluded.vkey, state='staging', layout='chunks', \
                   chunk_ids='', msg_ids='', lease=0, tries=0, created=excluded.created",
                vec![TursoArg::text(&name), TursoArg::int(size), TursoArg::int(chunks), TursoArg::text(&key), TursoArg::int(now)]).await?;
            ok(json!({"ok": true, "chunks": chunks, "chunk": CHUNK}))
        }
        "/api/vd/finish" => {
            let Some(row) = turso_exec(env, "SELECT size,chunks FROM vd_files WHERE name=? AND state='staging'", vec![TursoArg::text(&name)]).await?.pipe_first() else {
                return json_resp(&json!({"error": "holat"}), 409);
            };
            let (size, chunks) = (jint(&row, "size") as u64, jint(&row, "chunks") as u64);
            for n in 0..chunks {
                let want = CHUNK.min(size - n * CHUNK);
                let got = bk.head(part_key(&name, n)).await.ok().flatten().map(|o| o.size() as u64);
                if got != Some(want) {
                    return json_resp(&json!({"error": format!("bo'lak {n} R2 da yo'q yoki hajmi mos emas")}), 422);
                }
            }
            turso_many(env, &[
                ("UPDATE vd_files SET state='uploaded' WHERE name=? AND state='staging'", vec![TursoArg::text(&name)]),
                ("UPDATE vd_mig SET state='done', lease=0 WHERE name=?", vec![TursoArg::text(&name)]),
            ]).await?;
            let k = kick_wf(env, WORKFLOW).await;
            ok(json!({"ok": true, "kick": k}))
        }
        "/api/vd/peek" => {
            let r = turso_exec(env,
                "SELECT (SELECT COUNT(*) FROM vd_files WHERE state='uploaded' OR (state='storing' AND lease<?)) AS store,
                        (SELECT COUNT(*) FROM vd_mig WHERE state='queued' OR (state='running' AND lease<?)) AS mig",
                vec![TursoArg::int(now), TursoArg::int(now)]).await?;
            let r = first_row(&r).unwrap_or(json!({}));
            ok(json!({"store": jint(&r, "store"), "mig": jint(&r, "mig"), "full": staged_bytes(env).await >= cap_bytes(env)}))
        }
        // Tekshiruv (ENCODE_TOKEN): tayyor faylning 0-bo'lagini Telegram'dan olib, boshidagi 64 baytni (shifrlangan) va kalitni qaytaradi —
        // Actions shifr ochilib MP4 chiqishini tekshiradi (kalit logga chiqmaydi). `name` berilmasa — eng so'nggi tayyor fayl.
        "/api/vd/verify" => {
            let row = if name.is_empty() {
                turso_exec(env, "SELECT name,size,chunks,vkey,chunk_ids FROM vd_files WHERE state='ready' ORDER BY created DESC LIMIT 1", vec![]).await?.pipe_first()
            } else {
                turso_exec(env, "SELECT name,size,chunks,vkey,chunk_ids FROM vd_files WHERE name=? AND state='ready'", vec![TursoArg::text(&name)]).await?.pipe_first()
            };
            let Some(r) = row else { return ok(json!({"none": true})) };
            let first = ids(r["chunk_ids"].as_str().unwrap_or("")).first().map(|s| s.to_string()).unwrap_or_default();
            let size = jint(&r, "size") as u64;
            let rs = chunk_stream(env, &first, CHUNK.min(size)).await?;
            let resp = Response::from_body(ResponseBody::Stream(rs))?;
            let mut resp = resp;
            let bytes = resp.bytes().await?;
            if bytes.len() < 64 {
                return json_resp(&json!({"error": "bo'lak qisqa"}), 502);
            }
            ok(json!({"name": r["name"], "size": r["size"], "chunks": r["chunks"], "key": r["vkey"], "head": hex_of(&bytes[..64]), "chunk0_len": bytes.len()}))
        }
        "/api/vd/claim" => {
            turso_many(env, &[
                ("UPDATE vd_files SET state='uploaded' WHERE state='storing' AND lease<? AND tries<?", vec![TursoArg::int(now), TursoArg::int(MAX_TRIES)]),
                ("UPDATE vd_files SET state='failed' WHERE state='storing' AND lease<? AND tries>=?", vec![TursoArg::int(now), TursoArg::int(MAX_TRIES)]),
            ]).await?;
            let Some(f) = turso_exec(env, "SELECT name FROM vd_files WHERE state='uploaded' ORDER BY created LIMIT 1", vec![]).await?.pipe_first() else {
                return ok(json!({"none": true}));
            };
            let nm = f["name"].as_str().unwrap_or("").to_string();
            let got = turso_exec(env, "UPDATE vd_files SET state='storing', lease=?, tries=tries+1 WHERE name=? AND state='uploaded' \
                RETURNING size,chunks,chunk_ids,layout", vec![TursoArg::int(now + LEASE_MS), TursoArg::text(&nm)]).await?;
            let Some(r) = first_row(&got) else { return ok(json!({"none": true})) };
            let have = ids(r["chunk_ids"].as_str().unwrap_or("")).len() as i64;
            let done: Vec<i64> = (0..have).collect();
            ok(json!({
                "name": nm, "size": r["size"], "chunks": r["chunks"], "chunk": CHUNK, "layout": r["layout"], "done": done,
                "bot_token": tg_secret(env, "TELEGRAM_BOT_TOKEN"), "channel": tg_channel_id(env),
            }))
        }
        "/api/vd/chunkdone" => {
            // Bitta bo'lak (`n`, `tg_file_id`, `msg_id`) yoki bir nechta (`items: [...]`) — guruhlash bazaga so'rovlarni kamaytiradi.
            let items: Vec<Value> = match b["items"].as_array() {
                Some(a) => a.iter().take(40).cloned().collect(),
                None => vec![b.clone()],
            };
            if !tg_safe_name(&name) || items.is_empty() {
                return bad("so'rov");
            }
            let Some(row) = turso_exec(env, "SELECT chunk_ids,msg_ids,layout FROM vd_files WHERE name=? AND state='storing'", vec![TursoArg::text(&name)]).await?.pipe_first() else {
                return json_resp(&json!({"error": "holat"}), 409);
            };
            let mut fids: Vec<String> = ids(row["chunk_ids"].as_str().unwrap_or("")).into_iter().map(String::from).collect();
            let mut mids: Vec<String> = ids(row["msg_ids"].as_str().unwrap_or("")).into_iter().map(String::from).collect();
            let mut done_n: Vec<u64> = Vec::new();
            for it in &items {
                let tg = it["tg_file_id"].as_str().unwrap_or("").to_string();
                let (msg, n) = (jint(it, "msg_id"), jint(it, "n"));
                if n < 0 || tg.is_empty() || tg.len() > 300 || msg <= 0 || !tg.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
                    return bad("so'rov");
                }
                let n = n as usize;
                if n > fids.len() || n > mids.len() {
                    return json_resp(&json!({"error": "bo'laklar tartib bilan kelishi shart"}), 409);
                }
                if n == fids.len() { fids.push(tg); mids.push(msg.to_string()) } else { fids[n] = tg; mids[n] = msg.to_string() }
                done_n.push(n as u64);
            }
            turso_exec(env, "UPDATE vd_files SET chunk_ids=?, msg_ids=?, lease=?, tries=0 WHERE name=? AND state='storing'",
                vec![TursoArg::text(&fids.join(",")), TursoArg::text(&mids.join(",")), TursoArg::int(now + LEASE_MS), TursoArg::text(&name)]).await?;
            // Saqlangan bo'laklar R2 dan darhol o'chadi (joy bo'shaydi — migratsiya davom etadi).
            if row["layout"].as_str() != Some("single") {
                for n in done_n {
                    let _ = bk.delete(part_key(&name, n)).await;
                }
            }
            ok(json!({"ok": true}))
        }
        "/api/vd/done" => {
            let Some(row) = turso_exec(env, "SELECT chunks,chunk_ids,layout FROM vd_files WHERE name=? AND state='storing'", vec![TursoArg::text(&name)]).await?.pipe_first() else {
                return json_resp(&json!({"error": "holat"}), 409);
            };
            let (want, have) = (jint(&row, "chunks"), ids(row["chunk_ids"].as_str().unwrap_or("")).len() as i64);
            if want <= 0 || have != want {
                return json_resp(&json!({"error": format!("bo'laklar yetarli emas ({have}/{want})")}), 409);
            }
            turso_exec(env, "UPDATE vd_files SET state='ready', lease=0 WHERE name=? AND state='storing'", vec![TursoArg::text(&name)]).await?;
            r2_delete_all(&bk, &name).await;
            ok(json!({"ok": true}))
        }
        "/api/vd/fail" => {
            turso_exec(env, "UPDATE vd_files SET state=CASE WHEN tries>=? THEN 'failed' ELSE 'uploaded' END, lease=0 WHERE name=? AND state='storing'",
                vec![TursoArg::int(MAX_TRIES), TursoArg::text(&name)]).await?;
            ok(json!({"ok": true}))
        }
        _ => err404("topilmadi"),
    }
}

/// `Result<Value>` (Turso javobi) dan birinchi qatorni olish uchun qisqa yordamchi.
trait PipeFirst {
    fn pipe_first(&self) -> Option<Value>;
}
impl PipeFirst for Value {
    fn pipe_first(&self) -> Option<Value> {
        first_row(self)
    }
}

// ── Foydalanuvchi yo'llari (sessiya bilan) ───────────────────

/// `POST /api/vd/info {names:[...]}` -> `{files:{nom:{state,size,chunks,stored,key}}}`. Kalit faqat kirgan foydalanuvchiga
/// beriladi (`/api/tg/deliver` bilan bir xil qoida); o'qish bitta so'rov. Yangi tizimda bo'lmagan nomlar javobda yo'q — ilova
/// ularni eski yo'l (Telegram/MTProto) bilan ochadi.
pub(crate) async fn info(env: &Env, body: &Value) -> Result<Response> {
    let mut names: Vec<String> = body["names"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
    names.retain(|n| tg_safe_name(n));
    names.sort();
    names.dedup();
    names.truncate(100);
    if names.is_empty() {
        return bad("nom");
    }
    let mut files = serde_json::Map::new();
    let mut miss: Vec<String> = Vec::new();
    for n in &names {
        match meta_get(n).await {
            Some(m) => {
                files.insert(n.clone(), m);
            }
            None => miss.push(n.clone()),
        }
    }
    if !miss.is_empty() {
        ensure(env).await?;
        let marks = vec!["?"; miss.len()].join(",");
        let r = turso_exec(env,
            &format!("SELECT name,size,chunks,vkey,state,chunk_ids FROM vd_files WHERE state IN ('ready','storing','uploaded') AND name IN ({marks})"),
            miss.iter().map(|n| TursoArg::text(n)).collect()).await?;
        for row in rows_of(&r) {
            let n = row["name"].as_str().unwrap_or("").to_string();
            let v = json!({
                "state": row["state"], "size": row["size"], "chunks": row["chunks"], "key": row["vkey"],
                "stored": ids(row["chunk_ids"].as_str().unwrap_or("")).len(),
            });
            // Faqat TUGAGAN (`ready`) fayl keshlanadi — u o'zgarmaydi; saqlanayotgan fayl holati o'zgarib turadi.
            if row["state"].as_str() == Some("ready") {
                meta_put(&n, &v).await;
            }
            files.insert(n, v);
        }
    }
    ok_nostore(json!({"files": files}))
}

/// `POST /api/vd/up/init` (admin): ilova BUTUN (qurilmada shifrlangan) faylni R2 ga to'g'ridan-to'g'ri yozadi; worker faqat imzolangan
/// PUT havolasini beradi. `{name, size, key}` -> `{url}`.
pub(crate) async fn up_init(env: &Env, body: &Value) -> Result<Response> {
    ensure(env).await?;
    let name = body["name"].as_str().unwrap_or("").to_string();
    let size = jint(body, "size");
    let key = body["key"].as_str().unwrap_or("").to_ascii_lowercase();
    if !name.starts_with("ep_") || !tg_safe_name(&name) || size <= 0 || size > 5 * 1024 * 1024 * 1024 - CHUNK as i64 || !valid_file_key(&key) {
        return bad("so'rov");
    }
    let Some(bk) = bucket(env) else { return json_resp(&json!({"error": "r2"}), 503) };
    let Some(url) = r2::signed_key(env, "PUT", &format!("{SINGLE}{name}"), 6 * 3600) else {
        return json_resp(&json!({"error": "R2 kalitlari sozlanmagan"}), 501);
    };
    r2_delete_all(&bk, &name).await;
    turso_exec(env,
        "INSERT INTO vd_files (name,size,chunks,vkey,state,layout,created) VALUES (?,?,?,?,'uploading','single',?) \
         ON CONFLICT(name) DO UPDATE SET size=excluded.size, chunks=excluded.chunks, vkey=excluded.vkey, state='uploading', layout='single', \
           chunk_ids='', msg_ids='', lease=0, tries=0, created=excluded.created",
        vec![TursoArg::text(&name), TursoArg::int(size), TursoArg::int(chunks_of(size)), TursoArg::text(&key), TursoArg::int(now_ms())]).await?;
    ok(json!({"url": url, "chunk": CHUNK}))
}

/// `POST /api/vd/up/complete {name}` (admin): R2 dagi fayl hajmi tekshiriladi va saqlash navbatiga qo'yiladi (yuklovchi workflow uyg'otiladi).
pub(crate) async fn up_complete(env: &Env, body: &Value) -> Result<Response> {
    ensure(env).await?;
    let name = body["name"].as_str().unwrap_or("").to_string();
    if !tg_safe_name(&name) {
        return bad("nom");
    }
    let Some(bk) = bucket(env) else { return json_resp(&json!({"error": "r2"}), 503) };
    let Some(row) = turso_exec(env, "SELECT size FROM vd_files WHERE name=? AND state='uploading'", vec![TursoArg::text(&name)]).await?.pipe_first() else {
        return json_resp(&json!({"error": "holat"}), 409);
    };
    let got = bk.head(format!("{SINGLE}{name}")).await.ok().flatten().map(|o| o.size() as i64);
    if got != Some(jint(&row, "size")) {
        let _ = turso_exec(env, "UPDATE vd_files SET state='failed' WHERE name=?", vec![TursoArg::text(&name)]).await;
        return json_resp(&json!({"error": format!("hajm mos emas ({got:?})")}), 422);
    }
    turso_exec(env, "UPDATE vd_files SET state='uploaded' WHERE name=? AND state='uploading'", vec![TursoArg::text(&name)]).await?;
    let k = kick_wf(env, WORKFLOW).await;
    ok(json!({"ok": true, "kick": k}))
}

// ── Ko'rish: Telegram -> Cloudflare kesh (oyna = 1 bo'lak) ──

async fn chunk_stream(env: &Env, tg_file_id: &str, expect: u64) -> Result<web_sys::ReadableStream> {
    let token = tg_secret(env, "TELEGRAM_BOT_TOKEN");
    let f = tg_api_tok(&token, "getFile", json!({"file_id": tg_file_id})).await?;
    let path = f["file_path"].as_str().unwrap_or("").to_string();
    if path.is_empty() {
        return Err(Error::RustError("Telegram fayl yo'lini bermadi".into()));
    }
    let r = Fetch::Request(Request::new(&format!("https://api.telegram.org/file/bot{token}/{path}"), Method::Get)?).send().await?;
    if r.status_code() != 200 {
        return Err(Error::RustError(format!("Telegram yuklash {}", r.status_code())));
    }
    let len = r.headers().get("Content-Length").ok().flatten().and_then(|v| v.parse::<u64>().ok());
    if len.is_some_and(|l| l != expect) {
        return Err(Error::RustError("bo'lak hajmi mos emas".into()));
    }
    match r.body() {
        ResponseBody::Stream(rs) => Ok(rs.clone()),
        _ => Err(Error::RustError("Telegram oqimi yo'q".into())),
    }
}

/// `GET /api/warm/vd~<nom>?w=N` — N-bo'lakni Telegram'dan Cloudflare keshiga isitadi (ARUDRIVE `bdrive::warm` nusxasi).
pub(crate) async fn warm(env: &Env, file: &str, widx: u64, force: bool) -> Result<Response> {
    let name = format!("vd~{file}");
    let reply = |status: &str, total: u64| -> Result<Response> {
        let status: String = status.chars().filter(|c| c.is_ascii_alphanumeric() || " :-_.,()".contains(*c)).collect();
        let mut r = Response::ok(format!("{{\"status\":\"{status}\",\"total\":{total}}}"))?;
        set_cors(&mut r);
        r.headers_mut().set("Content-Type", "application/json")?;
        r.headers_mut().set("Cache-Control", "no-store")?;
        Ok(r)
    };
    if !tg_safe_name(file) {
        return reply("error nom", 0);
    }
    if !force {
        if let Some(t) = warm_window_total(&name, widx).await {
            if t > 0 {
                return reply("cached", t);
            }
        }
    }
    // Fayl ma'lumoti: `ready` bo'lsa Cloudflare keshidan (bazaga bormaydi), aks holda bazadan.
    let f = match meta_get(file).await.filter(|m| m["chunk_ids"].is_string()) {
        Some(m) => m,
        None => {
            ensure(env).await?;
            let r = turso_exec(env, "SELECT size,chunk_ids,state,chunks FROM vd_files WHERE name=? AND state IN ('ready','storing','uploaded')", vec![TursoArg::text(file)]).await?;
            let Some(f) = first_row(&r) else { return reply("error fayl topilmadi", 0) };
            f
        }
    };
    let total = jint(&f, "size").max(0) as u64;
    let chunks: Vec<(i64, String)> = ids(f["chunk_ids"].as_str().unwrap_or("")).into_iter().enumerate().map(|(i, s)| (i as i64, s.to_string())).collect();
    if total == 0 || widx * WARM_WINDOW >= total {
        return reply("error oyna", total);
    }
    let all = chunks_of(total as i64);
    let is_ready = f["state"].as_str() == Some("ready");
    if is_ready && chunks.len() as i64 != all {
        return reply("error bo'laklar to'liq emas", total);
    }
    if !is_ready && (chunks.len() as u64) <= widx {
        return reply(&format!("pending saqlanmoqda {}/{}", chunks.len(), all), total);
    }
    let cache = Cache::default();
    let marker_key = Request::new(&warm_marker_url(&name, widx), Method::Get)?;
    if !force && cache.get(&marker_key, false).await?.is_some() {
        for _ in 0..WARM_WAIT_TICKS {
            Delay::from(core::time::Duration::from_millis(WARM_WAIT_STEP_MS)).await;
            if let Some(t) = warm_window_total(&name, widx).await {
                if t > 0 {
                    return reply("cached", t);
                }
            }
        }
    }
    let mut marker = Response::ok("1")?;
    marker.headers_mut().set("Cache-Control", &format!("public, max-age={WARM_MARKER_SECONDS}"))?;
    let _ = cache.put(&marker_key, marker).await;

    let win_start = widx * WARM_WINDOW;
    let win_end = (win_start + WARM_WINDOW).min(total) - 1;
    let win_len = win_end - win_start + 1;
    let (readable, writable) = fixed_length_pipe(win_len)?;
    let env2 = env.clone();
    let first_n = win_start / CHUNK;
    let last_n = win_end / CHUNK;
    worker::wasm_bindgen_futures::spawn_local(async move {
        let open = |n: u64| {
            let env3 = env2.clone();
            let tg = chunks.iter().find(|(k, _)| *k as u64 == n).map(|(_, t)| t.clone());
            async move {
                let Some(tg) = tg else { return Err(Error::RustError("bo'lak yo'q".into())) };
                chunk_stream(&env3, &tg, CHUNK.min(total - n * CHUNK)).await
            }
        };
        let mut pending: Option<web_sys::ReadableStream> = None;
        for n in first_n..=last_n {
            let rs = match pending.take() {
                Some(s) => s,
                None => match open(n).await {
                    Ok(s) => s,
                    Err(_) => {
                        abort_pipe(&writable);
                        return;
                    }
                },
            };
            let p = match pipe_keep_open(&rs, &writable) {
                Ok(p) => p,
                Err(_) => {
                    abort_pipe(&writable);
                    return;
                }
            };
            let pipe_fut = worker::wasm_bindgen_futures::JsFuture::from(p);
            if n < last_n {
                let (pr, nr) = futures_util::future::join(pipe_fut, open(n + 1)).await;
                if pr.is_err() {
                    abort_pipe(&writable);
                    return;
                }
                pending = nr.ok();
            } else if pipe_fut.await.is_err() {
                abort_pipe(&writable);
                return;
            }
        }
        close_pipe(&writable);
    });

    let headers = Headers::new();
    headers.set("Content-Type", "application/octet-stream")?;
    headers.set("Accept-Ranges", "bytes")?;
    headers.set("Cache-Control", &format!("public, max-age={CHUNK_CACHE_SECONDS}"))?;
    headers.set("X-Total-Size", &total.to_string())?;
    headers.set("X-Window-Start", &win_start.to_string())?;
    let to_cache = Response::from_body(ResponseBody::Stream(readable))?.with_headers(headers).with_status(200);
    let key = Request::new(&warm_window_url(&name, widx), Method::Get)?;
    if cache.put(&key, to_cache).await.is_err() {
        return reply("error keshga yozilmadi", total);
    }
    for _ in 0..WARM_VERIFY_TICKS {
        if let Some(t) = warm_window_total(&name, widx).await {
            if t > 0 {
                return reply("warmed", t);
            }
        }
        Delay::from(core::time::Duration::from_millis(WARM_WAIT_STEP_MS)).await;
    }
    reply("error kesh tasdiqlanmadi", total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bolaklar_soni() {
        assert_eq!(chunks_of(1), 1);
        assert_eq!(chunks_of(CHUNK as i64), 1);
        assert_eq!(chunks_of(CHUNK as i64 + 1), 2);
        assert_eq!(chunks_of(0), 0);
    }

    #[test]
    fn id_ajratish() {
        assert!(ids("").is_empty());
        assert_eq!(ids("a,b,c"), vec!["a", "b", "c"]);
    }

    #[test]
    fn actions_yollari() {
        assert!(is_actions_path("/api/vd/claim"));
        assert!(is_actions_path("/api/vd/put"));
        assert!(!is_actions_path("/api/vd/info"));
        assert!(!is_actions_path("/api/vd/up/init"));
    }
}
