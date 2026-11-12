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
//! Holatlar (`vd_queue.state`): `staging` (Actions bo'laklarni R2 ga yozyapti) / `uploading` (ilova butun faylni R2 ga yozyapti) ->
//! `uploaded` (R2 da to'liq, saqlash navbatida) -> `storing` (yuklovchi Telegram'ga yozyapti) -> `ready`. `failed` — saqlab bo'lmadi.
//! Migratsiya navbati `vd_mig`: eski (`tg_files`) sifat fayllari -> yangi kalit va bo'laklar.

use super::*;
use crate::vcache::{
    abort_pipe, close_pipe, fixed_length_pipe, pipe_keep_open, warm_marker_url, warm_window_total, warm_window_url, CHUNK_CACHE_SECONDS,
    WARM_MARKER_SECONDS, WARM_VERIFY_TICKS, WARM_WAIT_STEP_MS, WARM_WAIT_TICKS, WARM_WINDOW,
};
use aes::Aes128;
use ctr::cipher::{KeyIvInit, StreamCipher};
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

/// Bo'laklar saqlanadigan kanal: `VD_CHANNEL_ID` (deploy `ci/VD_CHANNEL_ID` dan qo'yadi), bo'lmasa umumiy `TG_CHANNEL_ID`.
pub(crate) fn vd_channel(env: &Env) -> i64 {
    tg_secret(env, "VD_CHANNEL_ID").trim().parse::<i64>().ok().filter(|c| *c != 0).unwrap_or_else(|| tg_channel_id(env))
}

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
const META_V: &str = "3";

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

/// "O'CHIRILGAN" BELGISI (butun dunyo bo'yicha): `forget` global `MarkHub` (Durable Object) da `vddel_<nom>` belgisini yangilaydi; keshdan o'qiladigan
/// har joy (`meta_many`, `img`, `warm`) belgini tekshiradi — o'chirilgan fayl hech bir mintaqada qaytarilmaydi (keshdagi baytlar muddati
/// tugaguncha yoki siqib chiqarilguncha qolishi mumkin, lekin ularsiz kalit yo'q). DO javob bermasa tekshiruv o'tkazib yuboriladi (hamma narsa
/// "o'chirilgan" bo'lib qolmasin).
fn del_key(name: &str) -> String {
    format!("vddel_{name}")
}

async fn tombstoned_many(env: &Env, names: &[String]) -> std::collections::HashSet<String> {
    let mut out = std::collections::HashSet::new();
    if names.is_empty() {
        return out;
    }
    let keys: Vec<String> = names.iter().map(|n| del_key(n)).collect();
    let refs: Vec<&str> = keys.iter().map(|k| k.as_str()).collect();
    let (each, reliable) = marks_each(env, &refs).await;
    if reliable {
        for (n, t) in names.iter().zip(each) {
            if t > 0 {
                out.insert(n.clone());
            }
        }
    }
    out
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
    turso_exec(env, "SELECT COALESCE(SUM(size),0) AS n FROM vd_queue WHERE state IN ('staging','uploading','uploaded','storing')", vec![]).await
        .ok().and_then(|r| first_row(&r)).map(|r| jint(&r, "n")).unwrap_or(0)
}

pub(crate) async fn ensure(env: &Env) -> Result<()> {
    if SCHEMA_OK.load(Ordering::Relaxed) {
        return Ok(());
    }
    // `vd_queue` — FAQAT yuklash navbati (vaqtinchalik): tayyor (`ready`) fayl ma'lumoti egasining jadvaliga ko'chadi va navbat qatori o'chadi
    // (`try_attach`). Egasi hali yo'q fayl (masalan yozishma rasmi xabardan oldin yuklanadi) shu yerda turadi va egasi paydo bo'lgach ko'chiriladi.
    turso_many(env, &[
        ("CREATE TABLE IF NOT EXISTS vd_queue (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL UNIQUE, size INTEGER NOT NULL, chunks INTEGER NOT NULL, \
          vkey TEXT NOT NULL, state TEXT NOT NULL, layout TEXT NOT NULL DEFAULT 'chunks', chunk_ids TEXT NOT NULL DEFAULT '', \
          msg_ids TEXT NOT NULL DEFAULT '', lease INTEGER NOT NULL DEFAULT 0, tries INTEGER NOT NULL DEFAULT 0, created INTEGER NOT NULL)", vec![]),
        ("CREATE INDEX IF NOT EXISTS vd_queue_state ON vd_queue(state, created)", vec![]),
        ("CREATE TABLE IF NOT EXISTS vd_mig (name TEXT PRIMARY KEY, state TEXT NOT NULL, lease INTEGER NOT NULL DEFAULT 0, \
          tries INTEGER NOT NULL DEFAULT 0, err TEXT NOT NULL DEFAULT '', queued INTEGER NOT NULL)", vec![]),
        ("CREATE INDEX IF NOT EXISTS vd_mig_state ON vd_mig(state, queued)", vec![]),
    ]).await?;
    // Eski `vd_files` jadvali bo'lsa: qatorlari navbatga ko'chiriladi (egasi bo'lganlar o'qishda egasiga o'tadi), jadval o'chadi.
    let old = turso_exec(env, "SELECT 1 AS x FROM sqlite_master WHERE type='table' AND name='vd_files'", vec![]).await?;
    if first_row(&old).is_some() {
        turso_batch(env, &[
            ("INSERT OR IGNORE INTO vd_queue (name,size,chunks,vkey,state,layout,chunk_ids,msg_ids,lease,tries,created) \
              SELECT name,size,chunks,vkey,state,layout,chunk_ids,msg_ids,lease,tries,created FROM vd_files", vec![]),
            ("DROP TABLE vd_files", vec![]),
        ]).await?;
    }
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
            "SELECT (SELECT COUNT(*) FROM vd_queue WHERE state='uploaded' OR (state='storing' AND lease<?)) AS store,
                    (SELECT COUNT(*) FROM vd_mig WHERE state='queued' OR (state='running' AND lease<?)) AS mig,
                    (SELECT COUNT(*) FROM vd_queue WHERE state IN ('storing','staging','uploading') AND lease>=?) AS busy",
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
    let Some(rows) = turso_exec(env, "SELECT name FROM vd_queue WHERE state IN ('staging','uploading','failed') AND created<? LIMIT 20",
        vec![TursoArg::int(cutoff)]).await.ok().map(|r| rows_of(&r)) else { return };
    for row in rows {
        let name = row["name"].as_str().unwrap_or("").to_string();
        if tg_safe_name(&name) {
            r2_delete_all(&bk, &name).await;
            let _ = turso_exec(env, "DELETE FROM vd_queue WHERE name=? AND state IN ('staging','uploading','failed')", vec![TursoArg::text(&name)]).await;
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
/// kalitli va hali `vd_queue` da yo'qlari. Qaytadi: navbatdagi (queued) fayllar soni.
pub(crate) async fn enqueue_all(env: &Env) -> Result<i64> {
    ensure(env).await?;
    // Sifat fayllari (kalitli `ep_*.mp4`) VA rasmlar (Anibla/Post/to'plam botlari yetkazgan, kalitsiz ochiq rasmlar ham): `.jpg/.jpeg/.png/.webp`,
    // yozishma (`chat_`) va asl video (`orig_`) dan tashqari. Kalitsiz fayl migratsiyada yangi kalit bilan shifrlanadi.
    turso_exec(env,
        "INSERT OR IGNORE INTO vd_mig (name, state, queued) \
         SELECT file_name, 'queued', ? FROM tg_files \
         WHERE msg_id>0 AND file_name NOT IN (SELECT name FROM vd_queue) AND ( \
            (file_name LIKE 'ep\\_%' ESCAPE '\\' AND file_name NOT LIKE '%\\_f.mp4' ESCAPE '\\' AND file_key<>'') \
            OR (file_name LIKE 'sys\\_emojifont\\_%' ESCAPE '\\' AND file_key<>'') \
            OR (file_name NOT LIKE 'chat\\_%' ESCAPE '\\' AND file_name NOT LIKE 'orig\\_%' ESCAPE '\\' AND file_name NOT LIKE 'ep\\_%' ESCAPE '\\' \
                AND (file_name LIKE '%.jpg' OR file_name LIKE '%.jpeg' OR file_name LIKE '%.png' OR file_name LIKE '%.webp')))",
        vec![TursoArg::int(now_ms())]).await?;
    let r = turso_exec(env, "SELECT COUNT(*) AS n FROM vd_mig WHERE state IN ('queued','running')", vec![]).await?;
    Ok(first_row(&r).map(|r| jint(&r, "n")).unwrap_or(0))
}

/// Yangi kodlangan sifat (`encupload::on_file`) — shu faylni ham yangi tizimga o'tkazish uchun navbatga qo'yadi (xato e'tiborsiz).
pub(crate) async fn enqueue_one(env: &Env, name: &str) {
    let img = [".jpg", ".jpeg", ".png", ".webp"].iter().any(|e| name.ends_with(e));
    if !tg_safe_name(name) || name.ends_with("_f.mp4") || !(name.starts_with("ep_") || name.starts_with("sys_emojifont_") || (img && !name.starts_with("chat_") && !name.starts_with("orig_")))
        || ensure(env).await.is_err()
    {
        return;
    }
    let _ = turso_exec(env,
        "INSERT OR IGNORE INTO vd_mig (name, state, queued) SELECT ?, 'queued', ? WHERE NOT EXISTS (SELECT 1 FROM vd_queue WHERE name=?)",
        vec![TursoArg::text(name), TursoArg::int(now_ms()), TursoArg::text(name)]).await;
    let _ = kick_wf(env, MIG_WORKFLOW).await;
}

/// Kodlash navbati uchun asl video yangi tizimda (`vd`) tayyormi: `{"msg_id":0,"file_key":<kalit>,"vd":true}` yoki `None` (yo'q / hali saqlanmoqda).
pub(crate) async fn origin_src(env: &Env, name: &str) -> Option<Value> {
    if !tg_safe_name(name) || ensure(env).await.is_err() {
        return None;
    }
    let m = meta_many(env, &[name.to_string()]).await.ok()?;
    let m = m.get(name)?;
    if m["state"].as_str() != Some("ready") {
        return None;
    }
    Some(json!({"msg_id": 0, "file_key": m["key"], "vd": true}))
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
    // Ma'lumot egasining jadvalida (tayyor fayl) yoki navbatda (saqlanayotgan / egasi hali yo'q) bo'lishi mumkin.
    let owned = owner_load_many(env, &[name.to_string()]).await.ok().and_then(|m| m.get(name).cloned());
    let row = match owned {
        Some(o) => Some(json!({"msg_ids": o["msg_ids"], "chunks": o["chunks"]})),
        None => turso_exec(env, "SELECT msg_ids,chunks FROM vd_queue WHERE name=?", vec![TursoArg::text(name)]).await.ok().and_then(|r| first_row(&r)),
    };
    let _ = turso_exec(env, "DELETE FROM vd_mig WHERE name=?", vec![TursoArg::text(name)]).await;
    let Some(row) = row else { return };
    if let Some(bk) = bucket(env) {
        r2_delete_all(&bk, name).await;
    }
    // Cloudflare keshidagi bo'laklar (oynalar): shu mintaqada darhol o'chadi; boshqa mintaqalardagi nusxalar hech kim so'ramasa o'zi ketadi.
    let wname = format!("vd~{name}");
    for n in 0..jint(&row, "chunks").clamp(0, 400) as u64 {
        let _ = Cache::default().delete(&warm_window_url(&wname, n), false).await;
        let _ = Cache::default().delete(&warm_marker_url(&wname, n), false).await;
    }
    let channel = vd_channel(env);
    if channel != 0 {
        for id in ids(row["msg_ids"].as_str().unwrap_or("")).into_iter().take(400) {
            if let Ok(m) = id.parse::<i64>() {
                let _ = tg_api(env, "deleteMessage", json!({"chat_id": channel, "message_id": m})).await;
            }
        }
    }
    let _ = turso_exec(env, "DELETE FROM vd_queue WHERE name=?", vec![TursoArg::text(name)]).await;
    owner_clear(env, name).await;
    meta_drop(name).await;
    // Butun dunyo bo'yicha: boshqa mintaqalardagi kesh nusxalari endi "o'chirilgan" deb hisoblanadi (`tombstoned_many`).
    marks_bump(env, &[del_key(name)]).await;
}

// ── Actions yo'llari (ENCODE_TOKEN) ──────────────────────────

pub(crate) fn is_actions_path(path: &str) -> bool {
    matches!(path, "/api/vd/mig/enqueue" | "/api/vd/mig/claim" | "/api/vd/mig/fail" | "/api/vd/init" | "/api/vd/put" | "/api/vd/finish"
        | "/api/vd/peek" | "/api/vd/verify" | "/api/vd/wipe" | "/api/vd/channel" | "/api/vd/forget" | "/api/vd/selftest" | "/api/vd/dl" | "/api/vd/claim" | "/api/vd/read" | "/api/vd/chunkdone" | "/api/vd/done" | "/api/vd/fail")
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
        let Some(row) = turso_exec(env, "SELECT size,chunks FROM vd_queue WHERE name=? AND state='staging'", vec![TursoArg::text(&name)]).await?
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
        let Some(row) = turso_exec(env, "SELECT size,layout FROM vd_queue WHERE name=?", vec![TursoArg::text(&name)]).await?.pipe_first() else {
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
                    // Kalitli fayl (eski kalit bilan shifrlangan) yoki kalitsiz ochiq RASM (kalit bo'sh — yangi kalit bilan shifrlanadi).
                    Some(f) if jint(&f, "msg_id") > 0 && (valid_file_key(f["file_key"].as_str().unwrap_or("")) || (f["file_key"].as_str().unwrap_or("").is_empty() && !nm.starts_with("ep_"))) => {
                        return ok(json!({"name": nm, "msg_id": f["msg_id"], "key": f["file_key"], "channel": tg_channel_id(env)}));
                    }
                    // Fayl o'chirilgan yoki kalitsiz video (ochiq) — migratsiya qilinmaydi.
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
            let cur = turso_exec(env, "SELECT state FROM vd_queue WHERE name=?", vec![TursoArg::text(&name)]).await?.pipe_first();
            if cur.as_ref().and_then(|r| r["state"].as_str()) == Some("ready") {
                return ok(json!({"ready": true}));
            }
            if cur.is_none() && owner_load_many(env, &[name.clone()]).await.map(|m| m.contains_key(&name)).unwrap_or(false) {
                return ok(json!({"ready": true}));
            }
            if staged_bytes(env).await + size > cap_bytes(env) && cur.is_none() {
                return ok(json!({"wait": true}));
            }
            r2_delete_all(&bk, &name).await;
            let chunks = chunks_of(size);
            turso_exec(env,
                "INSERT INTO vd_queue (name,size,chunks,vkey,state,layout,created) VALUES (?,?,?,?,'staging','chunks',?) \
                 ON CONFLICT(name) DO UPDATE SET size=excluded.size, chunks=excluded.chunks, vkey=excluded.vkey, state='staging', layout='chunks', \
                   chunk_ids='', msg_ids='', lease=0, tries=0, created=excluded.created",
                vec![TursoArg::text(&name), TursoArg::int(size), TursoArg::int(chunks), TursoArg::text(&key), TursoArg::int(now)]).await?;
            ok(json!({"ok": true, "chunks": chunks, "chunk": CHUNK}))
        }
        "/api/vd/finish" => {
            let Some(row) = turso_exec(env, "SELECT size,chunks FROM vd_queue WHERE name=? AND state='staging'", vec![TursoArg::text(&name)]).await?.pipe_first() else {
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
                ("UPDATE vd_queue SET state='uploaded' WHERE name=? AND state='staging'", vec![TursoArg::text(&name)]),
                ("UPDATE vd_mig SET state='done', lease=0 WHERE name=?", vec![TursoArg::text(&name)]),
            ]).await?;
            let k = kick_wf(env, WORKFLOW).await;
            ok(json!({"ok": true, "kick": k}))
        }
        "/api/vd/peek" => {
            let r = turso_exec(env,
                "SELECT (SELECT COUNT(*) FROM vd_queue WHERE state='uploaded' OR (state='storing' AND lease<?)) AS store,
                        (SELECT COUNT(*) FROM vd_mig WHERE state='queued' OR (state='running' AND lease<?)) AS mig",
                vec![TursoArg::int(now), TursoArg::int(now)]).await?;
            let r = first_row(&r).unwrap_or(json!({}));
            ok(json!({"store": jint(&r, "store"), "mig": jint(&r, "mig"), "full": staged_bytes(env).await >= cap_bytes(env)}))
        }
        // ── NOLDAN BOSHLASH (foydalanuvchi talabi): kanal postlari va R2 obyektlarini o'chirish. Jadvallarni Actions (`vd-wipe.yml`) tozalaydi. ──
        "/api/vd/wipe" => match b["stage"].as_str().unwrap_or("") {
            "cancel" => ok(json!({"cancelled": cancel_runs(env).await})),
            "files" => ok(wipe_files(env, &bk).await?),
            // Baza Actions orqali to'g'ridan-to'g'ri tozalangach: "o'zgarish bormi?" belgilari yangilanadi (ilova/Mini App eski ro'yxatni ushlab qolmasin).
            "marks" => {
                let t: Vec<String> = b["tables"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
                marks_bump(env, &t).await;
                free_seasons_forget().await;
                ok(json!({"ok": true, "tables": t.len()}))
            }
            _ => bad("stage"),
        },
        // Tekshiruv (ENCODE_TOKEN): tayyor faylning 0-bo'lagini Telegram'dan olib, boshidagi 64 baytni (shifrlangan) va kalitni qaytaradi —
        // Actions shifr ochilib MP4 chiqishini tekshiradi (kalit logga chiqmaydi). `name` berilmasa — eng so'nggi tayyor fayl.
        "/api/vd/verify" => {
            // `name` berilsa — fayl ma'lumoti egasidan (yoki navbatdan); berilmasa — navbatdagi eng so'nggi tayyor fayl (tayyorlar egasiga ko'chib ketadi).
            let row = if name.is_empty() {
                turso_exec(env, "SELECT name,size,chunks,vkey,chunk_ids FROM vd_queue WHERE state='ready' ORDER BY created DESC LIMIT 1", vec![]).await?.pipe_first()
            } else {
                meta_one(env, &name).await.filter(|m| m["state"].as_str() == Some("ready"))
                    .map(|m| json!({"name": name, "size": m["size"], "chunks": m["chunks"], "vkey": m["key"], "chunk_ids": m["chunk_ids"]}))
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
        // Actions yuklab olishi (kodlash uchun asl video): hajm, kalit, bo'laklar `file_id` lari va bot tokeni (faqat ENCODE_TOKEN).
        "/api/vd/dl" => {
            let m = meta_many(env, &[name.clone()]).await?;
            let Some(m) = m.get(&name) else { return err404("topilmadi") };
            if m["state"].as_str() != Some("ready") {
                return json_resp(&json!({"error": "tayyor emas"}), 409);
            }
            ok(json!({
                "size": m["size"], "chunk": CHUNK, "key": m["key"],
                "chunk_ids": ids(m["chunk_ids"].as_str().unwrap_or("")),
                "bot_token": tg_secret(env, "TELEGRAM_BOT_TOKEN"),
            }))
        }
        // Faylni to'liq o'chirish (R2, kanal xabarlari, kesh, baza) — Actions/ENCODE_TOKEN (sinov fayllarini tozalash uchun).
        // O'z-o'zini tekshirish: 4 KiB faylni XUDDI `/api/vd/small` yo'li bilan saqlaydi (shifrlash -> Telegram -> baza -> kesh), vaqtlarni qaytaradi, so'ng o'chiradi.
        "/api/vd/selftest" => {
            // Cloudflare bepul rejasi bitta so'rovda 50 ta tashqi chaqiruvga ruxsat beradi: oddiy sinov va egasi yo'li sinovi ALOHIDA chaqiriladi
            // (`?owner=1` — faqat egasi yo'li).
            let owner_only = qs(&url, "owner") == "1";
            let t0 = now_ms();
            let mut out = json!({"ok": true});
            if !owner_only {
                let tname = format!("selftest_{t0}.bin");
                let r = store_small(env, &tname, vec![7u8; 4096]).await;
                let t1 = now_ms();
                out = match &r {
                    Ok((_, size)) => json!({"ok": true, "size": size, "ms": t1 - t0}),
                    Err(e) => json!({"ok": false, "error": e.to_string(), "ms": t1 - t0}),
                };
                forget(env, &tname).await;
                return ok(out);
            }
            // Egasi yo'li (vd_files rejasi): yozishma xabari + `chat_0_<vaqt>.bin` fayli. Ma'lumot egasining ustunlariga ko'chishi, navbat qatori
            // o'chishi, egasidan o'qilishi va `forget` bilan tozalanishi tekshiriladi.
            let (oname, mid) = (format!("chat_0_{t0}.bin"), format!("selftest_{t0}"));
            let mut owner = json!({"ok": false});
            let ins = turso_exec(env,
                "INSERT INTO chat_messages (id,user_id,from_admin,body,media_file,media_type,media_ms,created_at) VALUES (?,0,0,'',?,'',0,?)",
                vec![TursoArg::text(&mid), TursoArg::text(&oname), TursoArg::int(t0)]).await;
            if ins.is_ok() {
                let stored = store_small(env, &oname, vec![9u8; 2048]).await;
                let in_owner = owner_load_many(env, &[oname.clone()]).await.map(|m| m.get(&oname).map(|v| v["chunks"].clone())).ok().flatten();
                let queued = turso_exec(env, "SELECT COUNT(*) AS n FROM vd_queue WHERE name=?", vec![TursoArg::text(&oname)]).await.ok().and_then(|r| first_row(&r)).map(|r| jint(&r, "n")).unwrap_or(-1);
                let read = meta_one(env, &oname).await.map(|m| m["state"].clone());
                forget(env, &oname).await;
                let left = owner_load_many(env, &[oname.clone()]).await.map(|m| m.len()).unwrap_or(99);
                let _ = turso_exec(env, "DELETE FROM chat_messages WHERE id=?", vec![TursoArg::text(&mid)]).await;
                let good = stored.is_ok() && in_owner.as_ref().and_then(|c| c.as_i64().or_else(|| c.as_str().and_then(|x| x.parse().ok()))) == Some(1)
                    && queued == 0 && read.as_ref().and_then(|v| v.as_str()) == Some("ready") && left == 0;
                owner = json!({"ok": good, "stored": stored.is_ok(), "in_owner": in_owner, "queue_rows": queued, "read": read, "left_after_forget": left});
            }
            out["owner"] = owner;
            ok(out)
        }
        "/api/vd/forget" => {
            forget(env, &name).await;
            ok(json!({"ok": true}))
        }
        // Kanal tekshiruvi: bot shu kanalda adminmi va xabar yubora/o'chira oladimi.
        "/api/vd/channel" => {
            // Ikkala kanal: `vd` (yangi) va eski (`TG_CHANNEL_ID`): bot shu kanalda adminmi, xabar yubora/o'chira oladimi.
            let me = tg_api(env, "getMe", json!({})).await.ok().and_then(|m| m["id"].as_i64()).unwrap_or(0);
            let mut out = serde_json::Map::new();
            for (label, ch) in [("vd", vd_channel(env)), ("legacy", tg_channel_id(env))] {
                let chat = tg_api(env, "getChat", json!({"chat_id": ch})).await;
                let member = tg_api(env, "getChatMember", json!({"chat_id": ch, "user_id": me})).await;
                out.insert(label.to_string(), json!({
                    "channel": ch,
                    "chat": chat.as_ref().ok().map(|c| json!({"type": c["type"], "title": c["title"]})).unwrap_or(json!(null)),
                    "chat_error": chat.as_ref().err().map(|e| e.to_string()),
                    "bot_status": member.as_ref().ok().map(|m| m["status"].clone()),
                    "can_post": member.as_ref().ok().map(|m| m["can_post_messages"].clone()),
                    "can_delete": member.as_ref().ok().map(|m| m["can_delete_messages"].clone()),
                    "member_error": member.as_ref().err().map(|e| e.to_string()),
                }));
            }
            ok(Value::Object(out))
        }
        "/api/vd/claim" => {
            turso_many(env, &[
                ("UPDATE vd_queue SET state='uploaded' WHERE state='storing' AND lease<? AND tries<?", vec![TursoArg::int(now), TursoArg::int(MAX_TRIES)]),
                ("UPDATE vd_queue SET state='failed' WHERE state='storing' AND lease<? AND tries>=?", vec![TursoArg::int(now), TursoArg::int(MAX_TRIES)]),
            ]).await?;
            let Some(f) = turso_exec(env, "SELECT name FROM vd_queue WHERE state='uploaded' ORDER BY created LIMIT 1", vec![]).await?.pipe_first() else {
                return ok(json!({"none": true}));
            };
            let nm = f["name"].as_str().unwrap_or("").to_string();
            let got = turso_exec(env, "UPDATE vd_queue SET state='storing', lease=?, tries=tries+1 WHERE name=? AND state='uploaded' \
                RETURNING id AS vid,size,chunks,chunk_ids,layout", vec![TursoArg::int(now + LEASE_MS), TursoArg::text(&nm)]).await?;
            let Some(r) = first_row(&got) else { return ok(json!({"none": true})) };
            let have = ids(r["chunk_ids"].as_str().unwrap_or("")).len() as i64;
            let done: Vec<i64> = (0..have).collect();
            ok(json!({
                "name": nm, "vid": r["vid"], "kind": doc_prefix(&nm), "label": label(&nm), "size": r["size"], "chunks": r["chunks"], "chunk": CHUNK, "layout": r["layout"], "done": done,
                "bot_token": tg_secret(env, "TELEGRAM_BOT_TOKEN"), "channel": vd_channel(env),
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
            let Some(row) = turso_exec(env, "SELECT chunk_ids,msg_ids,layout FROM vd_queue WHERE name=? AND state='storing'", vec![TursoArg::text(&name)]).await?.pipe_first() else {
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
            turso_exec(env, "UPDATE vd_queue SET chunk_ids=?, msg_ids=?, lease=?, tries=0 WHERE name=? AND state='storing'",
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
            let Some(row) = turso_exec(env, "SELECT chunks,chunk_ids,layout FROM vd_queue WHERE name=? AND state='storing'", vec![TursoArg::text(&name)]).await?.pipe_first() else {
                return json_resp(&json!({"error": "holat"}), 409);
            };
            let (want, have) = (jint(&row, "chunks"), ids(row["chunk_ids"].as_str().unwrap_or("")).len() as i64);
            if want <= 0 || have != want {
                return json_resp(&json!({"error": format!("bo'laklar yetarli emas ({have}/{want})")}), 409);
            }
            turso_exec(env, "UPDATE vd_queue SET state='ready', lease=0 WHERE name=? AND state='storing'", vec![TursoArg::text(&name)]).await?;
            // Egasi bo'lsa ma'lumot unga ko'chadi va navbat qatori o'chadi; bo'lmasa navbatda qoladi (`try_attach`).
            let _ = try_attach(env, &name).await;
            r2_delete_all(&bk, &name).await;
            ok(json!({"ok": true}))
        }
        "/api/vd/fail" => {
            turso_exec(env, "UPDATE vd_queue SET state=CASE WHEN tries>=? THEN 'failed' ELSE 'uploaded' END, lease=0 WHERE name=? AND state='storing'",
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

/// Saqlash va migratsiya workflow'larining ishlab turgan (yoki navbatdagi) run'larini bekor qiladi (noldan boshlashdan oldin).
async fn cancel_runs(env: &Env) -> i64 {
    let repo = tg_secret(env, "GH_REPO");
    if !repo.contains('/') {
        return 0;
    }
    let mut n = 0;
    for wf in [WORKFLOW, MIG_WORKFLOW] {
        if let Ok((200, v)) = gh_api(env, Method::Get, &format!("/repos/{repo}/actions/workflows/{wf}/runs?per_page=20"), None).await {
            for r in v["workflow_runs"].as_array().cloned().unwrap_or_default() {
                if matches!(r["status"].as_str(), Some("in_progress") | Some("queued") | Some("waiting")) {
                    if let Some(id) = r["id"].as_i64() {
                        if matches!(gh_api(env, Method::Post, &format!("/repos/{repo}/actions/runs/{id}/cancel"), None).await, Ok((202, _))) {
                            n += 1;
                        }
                    }
                }
            }
        }
    }
    n
}

/// Kanaldan xabarlarni o'chiradi (asosiy bot, bo'lmasa kodlash boti). `true` — so'rov muvaffaqiyatli.
async fn del_msgs(env: &Env, channel: i64, msgs: &[i64]) -> bool {
    if msgs.is_empty() || channel == 0 {
        return true;
    }
    let body = json!({"chat_id": channel, "message_ids": msgs});
    if tg_api(env, "deleteMessages", body.clone()).await.is_ok() {
        return true;
    }
    !encbot_token(env).is_empty() && encbot_api(env, "deleteMessages", body).await.is_ok()
}

/// Bir chaqiruvda cheklangan ish (worker so'rovlar chegarasi): `tg_files` / `vd_queue` dagi kanal xabarlari o'chadi, qatorlar olib tashlanadi,
/// R2 dagi `vd/` va `r2_files` obyektlari o'chadi. Qolgan ish soni qaytadi — Actions `left == 0` bo'lguncha qayta chaqiradi.
/// `bd/` (ARUDRIVE) prefiksi va boshqa kalitlarga TEGILMAYDI.
async fn wipe_files(env: &Env, bk: &Bucket) -> Result<Value> {
    ensure(env).await?;
    let channel = tg_channel_id(env);
    let (mut deleted, mut failed) = (0i64, 0i64);
    for _ in 0..10 {
        let r = turso_exec(env, "SELECT file_name,msg_id FROM tg_files LIMIT 100", vec![]).await?;
        let rows = rows_of(&r);
        if rows.is_empty() {
            break;
        }
        let msgs: Vec<i64> = rows.iter().map(|x| jint(x, "msg_id")).filter(|m| *m > 0).collect();
        if del_msgs(env, channel, &msgs).await { deleted += msgs.len() as i64 } else { failed += msgs.len() as i64 }
        let names: Vec<String> = rows.iter().filter_map(|x| x["file_name"].as_str().map(String::from)).collect();
        let marks = vec!["?"; names.len()].join(",");
        turso_exec(env, &format!("DELETE FROM tg_files WHERE file_name IN ({marks})"), names.iter().map(|n| TursoArg::text(n)).collect()).await?;
    }
    for _ in 0..5 {
        let r = turso_exec(env, "SELECT name,msg_ids FROM vd_queue LIMIT 1", vec![]).await?;
        let Some(row) = first_row(&r) else { break };
        let name = row["name"].as_str().unwrap_or("").to_string();
        let msgs: Vec<i64> = ids(row["msg_ids"].as_str().unwrap_or("")).iter().filter_map(|x| x.parse().ok()).collect();
        for part in msgs.chunks(100) {
            if del_msgs(env, vd_channel(env), part).await { deleted += part.len() as i64 } else { failed += part.len() as i64 }
        }
        r2_delete_all(bk, &name).await;
        meta_drop(&name).await;
        turso_exec(env, "DELETE FROM vd_queue WHERE name=?", vec![TursoArg::text(&name)]).await?;
    }
    // Yetkazilgan (anibla/to'plam) fayllarning kanal xabarlari: `dlv_msg_<nom>` -> xabar raqami.
    let r = turso_exec(env, "SELECT cfg_key,cfg_value FROM app_config WHERE cfg_key LIKE 'dlv\\_msg\\_%' ESCAPE '\\' LIMIT 100", vec![]).await?;
    let rows = rows_of(&r);
    if !rows.is_empty() {
        let msgs: Vec<i64> = rows.iter().filter_map(|x| x["cfg_value"].as_str().and_then(|v| v.parse().ok())).collect();
        if del_msgs(env, channel, &msgs).await { deleted += msgs.len() as i64 } else { failed += msgs.len() as i64 }
        let keys: Vec<String> = rows.iter().filter_map(|x| x["cfg_key"].as_str().map(String::from)).collect();
        let marks = vec!["?"; keys.len()].join(",");
        turso_exec(env, &format!("DELETE FROM app_config WHERE cfg_key IN ({marks})"), keys.iter().map(|n| TursoArg::text(n)).collect()).await?;
    }
    // R2: `vd/` prefiksi (yarim yuklangan bo'laklar) va eski oraliq ombor (`r2_files`).
    if let Ok(list) = bk.list().prefix("vd/".to_string()).limit(500).execute().await {
        for o in list.objects() {
            let _ = bk.delete(o.key()).await;
        }
    }
    let r = turso_exec(env, "SELECT file_name FROM r2_files LIMIT 100", vec![]).await?;
    let rows = rows_of(&r);
    for x in &rows {
        if let Some(f) = x["file_name"].as_str() {
            if tg_safe_name(f) {
                let _ = bk.delete(f.to_string()).await;
            }
        }
    }
    if !rows.is_empty() {
        let names: Vec<String> = rows.iter().filter_map(|x| x["file_name"].as_str().map(String::from)).collect();
        let marks = vec!["?"; names.len()].join(",");
        turso_exec(env, &format!("DELETE FROM r2_files WHERE file_name IN ({marks})"), names.iter().map(|n| TursoArg::text(n)).collect()).await?;
    }
    let left = turso_exec(env,
        "SELECT (SELECT COUNT(*) FROM tg_files) AS tg, (SELECT COUNT(*) FROM vd_queue) AS vd, (SELECT COUNT(*) FROM r2_files) AS r2,
                (SELECT COUNT(*) FROM app_config WHERE cfg_key LIKE 'dlv\\_msg\\_%' ESCAPE '\\') AS dlv", vec![]).await?;
    let l = first_row(&left).unwrap_or(json!({}));
    let total = jint(&l, "tg") + jint(&l, "vd") + jint(&l, "r2") + jint(&l, "dlv");
    Ok(json!({"deleted": deleted, "failed": failed, "left": total, "tg_files": l["tg"], "vd_queue": l["vd"], "r2_files": l["r2"]}))
}

// ── Foydalanuvchi yo'llari (sessiya bilan) ───────────────────

/// Fayl ma'lumoti (to'liq): Cloudflare keshidan, yo'q bo'lsa bazadan (tugagan `ready` fayl keshlanadi). `chunk_ids` ichki — mijozga berilmaydi.
async fn meta_many(env: &Env, names: &[String]) -> Result<serde_json::Map<String, Value>> {
    let mut out = serde_json::Map::new();
    let mut miss: Vec<String> = Vec::new();
    for n in names {
        match meta_get(n).await.filter(|m| m["chunk_ids"].is_string()) {
            Some(m) => {
                out.insert(n.clone(), m);
            }
            None => miss.push(n.clone()),
        }
    }
    // Keshdan kelganlar eskirgan bo'lishi mumkin (boshqa mintaqada o'chirilgan) — global "o'chirilgan" belgisi tekshiriladi (bazadan kelganlar — haqiqat).
    let hits: Vec<String> = out.keys().cloned().collect();
    for n in tombstoned_many(env, &hits).await {
        out.remove(&n);
        meta_drop(&n).await;
    }
    if !miss.is_empty() {
        ensure(env).await?;
        // 1) tayyor fayl ma'lumoti egasining jadvalida; 2) saqlanayotgan (yoki egasi hali yo'q) fayllar — navbat jadvalida.
        let mut left: Vec<String> = miss.clone();
        if let Ok(found) = owner_load_many(env, &miss).await {
            for (n, v) in found {
                meta_put(&n, &v).await;
                left.retain(|x| *x != n);
                out.insert(n, v);
            }
        }
        if !left.is_empty() {
            let marks = vec!["?"; left.len()].join(",");
            let r = turso_exec(env,
                &format!("SELECT name,size,chunks,vkey,state,chunk_ids,msg_ids FROM vd_queue WHERE state IN ('ready','storing','uploaded') AND name IN ({marks})"),
                left.iter().map(|n| TursoArg::text(n)).collect()).await?;
            for row in rows_of(&r) {
                let n = row["name"].as_str().unwrap_or("").to_string();
                let v = json!({"state": row["state"], "size": row["size"], "chunks": row["chunks"], "key": row["vkey"], "chunk_ids": row["chunk_ids"], "msg_ids": row["msg_ids"]});
                // Faqat TUGAGAN (`ready`) fayl keshlanadi — u o'zgarmaydi; saqlanayotgan fayl holati o'zgarib turadi.
                if row["state"].as_str() == Some("ready") {
                    meta_put(&n, &v).await;
                    // Egasi endi bor bo'lsa ma'lumot unga ko'chadi (fayl egasidan oldin yuklangan edi).
                    let _ = try_attach(env, &n).await;
                }
                out.insert(n, v);
            }
        }
    }
    Ok(out)
}


// ── Egasi jadvali bilan ishlash (ma'lumot egasining qatorida turadi; navbat jadvali — vaqtinchalik) ───────────────────────────

fn owner_arg(a: &OwnerArg) -> TursoArg {
    match a {
        OwnerArg::Int(i) => TursoArg::int(*i),
        OwnerArg::Text(t) => TursoArg::text(t),
    }
}

/// Fayl ma'lumotini egasining ustunlariga yozadi. Egasining qatori topilib yangilangan bo'lsa `true` (egasi hali yo'q bo'lsa `false`).
async fn owner_save(env: &Env, name: &str, size: i64, chunks: i64, key: &str, ids: &str, msgs: &str) -> bool {
    let mut saved = false;
    for o in owners_of(name) {
        let r = &o.r;
        let sql = format!(
            "UPDATE {t} SET {r}_vd_name=?, {r}_vd_size=?, {r}_vd_chunks=?, {r}_vd_key=?, {r}_vd_ids=?, {r}_vd_msgs=? WHERE {w}",
            t = o.table, w = o.where_sql
        );
        let mut args = vec![TursoArg::text(name), TursoArg::int(size), TursoArg::int(chunks), TursoArg::text(key), TursoArg::text(ids), TursoArg::text(msgs)];
        args.extend(o.args.iter().map(owner_arg));
        if let Ok(res) = turso_exec(env, &sql, args).await {
            if jint(&res, "affected_row_count") > 0 {
                saved = true;
            }
        }
    }
    saved
}

/// Faylning egasidagi ma'lumotini tozalaydi (fayl o'chirilganda); faqat ma'lumot AYNAN shu nomga tegishli bo'lsa.
async fn owner_clear(env: &Env, name: &str) {
    for o in owners_of(name) {
        let r = &o.r;
        let sql = format!(
            "UPDATE {t} SET {r}_vd_name='', {r}_vd_size=0, {r}_vd_chunks=0, {r}_vd_key='', {r}_vd_ids='', {r}_vd_msgs='' WHERE ({w}) AND {r}_vd_name=?",
            t = o.table, w = o.where_sql
        );
        let mut args: Vec<TursoArg> = o.args.iter().map(owner_arg).collect();
        args.push(TursoArg::text(name));
        let _ = turso_exec(env, &sql, args).await;
    }
}

/// Egalarining jadvallaridan tayyor fayllar ma'lumoti (bitta so'rov: `UNION ALL`). Faqat ma'lumoti to'liq (`chunks>0`) va nomi mos fayllar.
async fn owner_load_many(env: &Env, names: &[String]) -> Result<serde_json::Map<String, Value>> {
    let mut out = serde_json::Map::new();
    let mut parts: Vec<String> = Vec::new();
    let mut args: Vec<TursoArg> = Vec::new();
    for n in names {
        for o in owners_of(n) {
            let r = &o.r;
            parts.push(format!(
                "SELECT ? AS n, {r}_vd_size AS size, {r}_vd_chunks AS chunks, {r}_vd_key AS vkey, {r}_vd_ids AS chunk_ids, {r}_vd_msgs AS msg_ids \
                 FROM {t} WHERE ({w}) AND {r}_vd_name=? AND {r}_vd_chunks>0",
                t = o.table, w = o.where_sql
            ));
            args.push(TursoArg::text(n));
            args.extend(o.args.iter().map(owner_arg));
            args.push(TursoArg::text(n));
        }
    }
    if parts.is_empty() {
        return Ok(out);
    }
    let res = turso_exec(env, &parts.join(" UNION ALL "), args).await?;
    for row in rows_of(&res) {
        let n = row["n"].as_str().unwrap_or("").to_string();
        if n.is_empty() || out.contains_key(&n) {
            continue;
        }
        out.insert(n, json!({"state": "ready", "size": row["size"], "chunks": row["chunks"], "key": row["vkey"], "chunk_ids": row["chunk_ids"], "msg_ids": row["msg_ids"]}));
    }
    Ok(out)
}

/// Navbatdagi TAYYOR (`ready`) fayl ma'lumotini egasiga ko'chiradi va navbat qatorini o'chiradi. Egasi hali yo'q bo'lsa hech narsa o'zgarmaydi
/// (ma'lumot navbatda turadi va egasi paydo bo'lgach birinchi o'qishda ko'chiriladi). `true` — ko'chirildi.
async fn try_attach(env: &Env, name: &str) -> bool {
    let Some(row) = turso_exec(env, "SELECT size,chunks,vkey,chunk_ids,msg_ids FROM vd_queue WHERE name=? AND state='ready'", vec![TursoArg::text(name)]).await
        .ok().and_then(|r| first_row(&r)) else { return false };
    let saved = owner_save(env, name, jint(&row, "size"), jint(&row, "chunks"), row["vkey"].as_str().unwrap_or(""),
        row["chunk_ids"].as_str().unwrap_or(""), row["msg_ids"].as_str().unwrap_or("")).await;
    if saved {
        let _ = turso_exec(env, "DELETE FROM vd_queue WHERE name=? AND state='ready'", vec![TursoArg::text(name)]).await;
    }
    saved
}

/// Bitta fayl ma'lumoti (`state/size/chunks/key/chunk_ids`) yoki `None`.
pub(crate) async fn meta_one(env: &Env, name: &str) -> Option<Value> {
    meta_many(env, &[name.to_string()]).await.ok()?.remove(name)
}

/// Mijozga beriladigan ko'rinish: `{state,size,chunks,key,stored}` (`file_id` lar yashirin).
fn client_view(m: &Value) -> Value {
    json!({
        "state": m["state"], "size": m["size"], "chunks": m["chunks"], "key": m["key"],
        "stored": ids(m["chunk_ids"].as_str().unwrap_or("")).len(),
    })
}

fn clean_names(body: &Value) -> Vec<String> {
    let mut names: Vec<String> = body["names"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
    names.retain(|n| tg_safe_name(n));
    names.sort();
    names.dedup();
    names.truncate(100);
    names
}

/// `info` + RUXSAT (kalit faqat egasiga): `orig_` — admin; `chat_<id>_` — egasi, admin yoki shu suhbatdagi xabar ishtirokchisi; `pki_<id>_` — egasi yoki admin.
pub(crate) async fn info_for(env: &Env, me: i64, admin: bool, body: &Value) -> Result<Response> {
    let mut names = clean_names(body);
    names.retain(|n| {
        if admin { return true }
        if n.starts_with("orig_") { return false }
        if n.starts_with("pki_") { return n.starts_with(&format!("pki_{me}_")) }
        true
    });
    // Boshqa odamning `chat_` fayli: faqat shu foydalanuvchi suhbatidagi xabarda bo'lsa (admin yuborgan fayl).
    let foreign: Vec<String> = if admin { Vec::new() } else {
        names.iter().filter(|n| n.starts_with("chat_") && !n.starts_with(&format!("chat_{me}_"))).cloned().collect()
    };
    if !foreign.is_empty() {
        let marks = vec!["?"; foreign.len()].join(",");
        let mut args = vec![TursoArg::int(me)];
        args.extend(foreign.iter().map(|n| TursoArg::text(n)));
        let r = turso_exec(env, &format!("SELECT DISTINCT media_file FROM chat_messages WHERE user_id=? AND media_file IN ({marks})"), args).await?;
        let allowed: std::collections::HashSet<String> = rows_of(&r).iter().filter_map(|x| x["media_file"].as_str().map(String::from)).collect();
        names.retain(|n| !foreign.contains(n) || allowed.contains(n));
    }
    if names.is_empty() {
        return ok_nostore(json!({"files": {}}));
    }
    let m = meta_many(env, &names).await?;
    let files: serde_json::Map<String, Value> = m.iter().map(|(k, v)| (k.clone(), client_view(v))).collect();
    ok_nostore(json!({"files": files}))
}

/// `POST /api/vd/info {names:[...]}` -> `{files:{nom:{state,size,chunks,stored,key}}}`. Kalit faqat kirgan foydalanuvchiga beriladi
/// (`/api/tg/deliver` bilan bir xil qoida). Yangi tizimda bo'lmagan nomlar javobda yo'q — ilova ularni eski yo'l bilan ochadi.
pub(crate) async fn info(env: &Env, body: &Value) -> Result<Response> {
    let names = clean_names(body);
    if names.is_empty() {
        return bad("nom");
    }
    let m = meta_many(env, &names).await?;
    let files: serde_json::Map<String, Value> = m.iter().map(|(k, v)| (k.clone(), client_view(v))).collect();
    ok_nostore(json!({"files": files}))
}

// ── Rasmlar (poster, bo'lim rasmi, profil rasmi): kirmagan foydalanuvchiga ham ko'rinadi ──

/// Rasm nomi ommaviymi: qism (`ep_`), asl video (`orig_`), yozishma (`chat_`) va kutilayotgan to'plam (`pki_`) fayllari — YO'Q.
fn public_image_name(n: &str) -> bool {
    tg_safe_name(n) && !["ep_", "orig_", "chat_", "pki_"].iter().any(|p| n.starts_with(p)) && n.len() > 4
}

/// `POST /api/vd/imginfo {names}` (faqat ilova imzosi, sessiyasiz): tayyor (`ready`) ommaviy rasmlar uchun hajm va kalit.
pub(crate) async fn img_info(env: &Env, body: &Value) -> Result<Response> {
    let names: Vec<String> = clean_names(body).into_iter().filter(|n| public_image_name(n)).collect();
    if names.is_empty() {
        return bad("nom");
    }
    let m = meta_many(env, &names).await?;
    let files: serde_json::Map<String, Value> = m.iter().filter(|(_, v)| v["state"].as_str() == Some("ready"))
        .map(|(k, v)| (k.clone(), client_view(v))).collect();
    ok_nostore(json!({"files": files}))
}

/// Telegram'dan bo'lakni to'liq baytlar bilan oladi (kichik fayllar uchun).
pub(crate) async fn chunk_bytes(env: &Env, tg_file_id: &str, expect: u64) -> Result<Vec<u8>> {
    let token = tg_secret(env, "TELEGRAM_BOT_TOKEN");
    let f = tg_api_tok(&token, "getFile", json!({"file_id": tg_file_id})).await?;
    let path = f["file_path"].as_str().unwrap_or("").to_string();
    if path.is_empty() {
        return Err(Error::RustError("Telegram fayl yo'lini bermadi".into()));
    }
    let mut r = Fetch::Request(Request::new(&format!("https://api.telegram.org/file/bot{token}/{path}"), Method::Get)?).send().await?;
    if r.status_code() != 200 {
        return Err(Error::RustError(format!("Telegram yuklash {}", r.status_code())));
    }
    let b = r.bytes().await?;
    if b.len() as u64 != expect {
        return Err(Error::RustError("hajm mos emas".into()));
    }
    Ok(b)
}

/// `GET /api/vd/img/<nom>` (faqat ilova imzosi): shifrlangan rasm baytlari. Bir so'rovda: Cloudflare keshidan, yo'q bo'lsa Telegram'dan olib
/// keshga (1000 kun, `warm` bilan bir xil yozuv) yoziladi. Faqat bitta bo'lakka sig'adigan (<= 18 MiB) tayyor rasm.
pub(crate) async fn img(env: &Env, name: &str) -> Result<Response> {
    // Shifrlangan baytlar: yozishma (`chat_`) va to'plam (`pki_`) fayllari ham beriladi — ularning KALITI sessiya va ruxsat bilan (`info_for`).
    if !tg_safe_name(name) || name.starts_with("ep_") || name.starts_with("orig_") || name.len() <= 4 {
        return err404("topilmadi");
    }
    let wname = format!("vd~{name}");
    let cache = Cache::default();
    let key = Request::new(&warm_window_url(&wname, 0), Method::Get)?;
    let bytes: Vec<u8> = match cache.get(&key, false).await? {
        Some(mut hit) => {
            if !tombstoned_many(env, &[name.to_string()]).await.is_empty() {
                let _ = cache.delete(&key, false).await;
                return err404("topilmadi");
            }
            hit.bytes().await?
        }
        None => {
            let m = meta_many(env, &[name.to_string()]).await?;
            let Some(m) = m.get(name) else { return err404("topilmadi") };
            let size = jint(m, "size").max(0) as u64;
            let first = ids(m["chunk_ids"].as_str().unwrap_or("")).first().map(|s| s.to_string()).unwrap_or_default();
            if m["state"].as_str() != Some("ready") || size == 0 || size > CHUNK || first.is_empty() {
                return err404("topilmadi");
            }
            let b = chunk_bytes(env, &first, size).await?;
            let headers = Headers::new();
            headers.set("Content-Type", "application/octet-stream")?;
            headers.set("Cache-Control", &format!("public, max-age={CHUNK_CACHE_SECONDS}"))?;
            headers.set("X-Total-Size", &size.to_string())?;
            headers.set("X-Window-Start", "0")?;
            let _ = cache.put(&key, Response::from_bytes(b.clone())?.with_headers(headers)).await;
            b
        }
    };
    let mut resp = Response::from_bytes(bytes)?;
    set_cors(&mut resp);
    resp.headers_mut().set("Content-Type", "application/octet-stream")?;
    resp.headers_mut().set("Cache-Control", "no-store")?;
    Ok(resp)
}

// ── KICHIK FAYLLAR (<= 18 MiB): worker O'ZI shifrlab Telegram'ga saqlaydi (R2 va Actions kerak emas) ──

/// Bot API `sendDocument` (qo'lda yig'ilgan multipart): `(file_id, message_id)`. Telegram 429 bersa bir marta qisqa kutib qayta uriniladi.
async fn tg_send_document(token: &str, channel: i64, name: &str, caption: &str, data: &[u8]) -> Result<(String, i64)> {
    if token.is_empty() || channel == 0 {
        return Err(Error::RustError("bot tokeni yoki kanal yo'q".into()));
    }
    let boundary = format!("----vd{}", now_ms());
    let safe: String = name.chars().map(|c| if c.is_ascii_graphic() && c != '"' && c != '\\' { c } else { '_' }).collect();
    let mut last = String::new();
    for attempt in 0..2 {
        let mut body: Vec<u8> = Vec::with_capacity(data.len() + 512);
        for (k, v) in [("chat_id", channel.to_string()), ("disable_notification", "true".to_string()), ("caption", caption.to_string())].into_iter().filter(|(_, v)| !v.is_empty()) {
            body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{k}\"\r\n\r\n{v}\r\n").as_bytes());
        }
        body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"document\"; filename=\"{safe}\"\r\nContent-Type: application/octet-stream\r\n\r\n").as_bytes());
        body.extend_from_slice(data);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
        let h = Headers::new();
        h.set("Content-Type", &format!("multipart/form-data; boundary={boundary}"))?;
        let arr = worker::js_sys::Uint8Array::from(&body[..]);
        drop(body);
        let treq = Request::new_with_init(
            &format!("https://api.telegram.org/bot{token}/sendDocument"),
            RequestInit::new().with_method(Method::Post).with_headers(h).with_body(Some(arr.into())),
        )?;
        let mut r = Fetch::Request(treq).send().await?;
        let d: Value = r.json().await?;
        if d["ok"] == json!(true) {
            let doc = &d["result"]["document"];
            let fid = doc["file_id"].as_str().unwrap_or("").to_string();
            if doc["file_size"].as_u64().unwrap_or(data.len() as u64) != data.len() as u64 || fid.is_empty() {
                return Err(Error::RustError("Telegram javobi mos emas".into()));
            }
            return Ok((fid, d["result"]["message_id"].as_i64().unwrap_or(0)));
        }
        last = d["description"].as_str().unwrap_or("noma'lum").to_string();
        let wait = d["parameters"]["retry_after"].as_u64().unwrap_or(0);
        if attempt == 0 && wait > 0 && wait <= 5 {
            Delay::from(core::time::Duration::from_secs(wait + 1)).await;
            continue;
        }
        break;
    }
    Err(Error::RustError(format!("Telegram rad etdi: {last}")))
}

fn hex_decode16(s: &str) -> Option<[u8; 16]> {
    if s.len() != 32 {
        return None;
    }
    let mut out = [0u8; 16];
    for i in 0..16 {
        out[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// Fayl nomining odam o'qiydigan tavsifi (kanal izohi va bot xabarlari uchun): `ep_1_2_3_720p_...` -> "Qism 1-2-3 (720p)".
/// Kanal hujjatining nomi boshi: `<tur>_<raqam>_<bo'lak>.bin`. `vid` — FAQAT video (qism, asl video, video fayl); rasm — `img`, ovoz — `audio`,
/// emoji shrifti — `font`, qolgan hujjatlar — `file`.
pub(crate) fn doc_kind(name: &str) -> &'static str {
    let n = name.to_ascii_lowercase();
    let ext = n.rsplit('.').next().unwrap_or("");
    if n.starts_with("sys_emojifont_") {
        "font"
    } else if n.starts_with("ep_") || n.starts_with("orig_") || matches!(ext, "mp4" | "mov" | "mkv" | "webm" | "avi" | "m4v" | "3gp") {
        "vid"
    } else if matches!(ext, "jpg" | "jpeg" | "png" | "webp" | "gif" | "bmp" | "heic") {
        "img"
    } else if matches!(ext, "ogg" | "oga" | "opus" | "mp3" | "m4a" | "aac" | "wav" | "amr") {
        "audio"
    } else {
        "file"
    }
}

/// Kanal hujjati nomining boshi (sifat bilan): qism fayli (`ep_<a>_<s>_<e>_<sifat>_...`) uchun `vid_1080p`, `vid_720p` ...; boshqalar uchun `doc_kind`.
/// To'liq nom: `<bosh>_<raqam>_<bo'lak>.bin` (masalan `vid_1080p_17_1.bin`).
pub(crate) fn doc_prefix(name: &str) -> String {
    let kind = doc_kind(name);
    if kind == "vid" && name.starts_with("ep_") {
        if let Some(q) = name.split('_').nth(4) {
            let digits = q.strip_suffix('p').unwrap_or("");
            if !digits.is_empty() && digits.len() <= 4 && digits.bytes().all(|b| b.is_ascii_digit()) {
                return format!("vid_{q}");
            }
        }
    }
    kind.to_string()
}

pub(crate) fn label(name: &str) -> String {
    let p: Vec<&str> = name.trim_end_matches(".mp4").split('_').collect();
    if name.starts_with("sys_emojifont_") {
        return "Emoji shrifti".into();
    }
    if p.first() == Some(&"ep") && p.len() >= 5 {
        return format!("Qism {}-{}-{} ({})", p[1], p[2], p[3], p[4]);
    }
    let uid = p.get(1).copied().unwrap_or("");
    match p.first().copied().unwrap_or("") {
        "orig" => "Asl video".into(),
        "avatar" => format!("Profil rasmi (foydalanuvchi {uid})"),
        "chat" => format!("Yozishma fayli (foydalanuvchi {uid})"),
        "anime" => "Anime rasmi".into(),
        "season" => "Bo'lim rasmi".into(),
        _ => name.to_string(),
    }
}

/// Kichik faylni shifrlab Telegram'ga saqlaydi, bazaga `ready` yozadi va keshni to'ldiradi: `(kalit_hex, hajm)`.
async fn store_small(env: &Env, name: &str, mut bytes: Vec<u8>) -> Result<(String, u64)> {
    let size = bytes.len() as u64;
    let key_hex = random_hex(32);
    let Some(key) = hex_decode16(&key_hex) else { return Err(Error::RustError("kalit".into())) };
    ctr::Ctr128BE::<Aes128>::new(&key.into(), &[0u8; 16].into()).apply_keystream(&mut bytes);
    // Fayl RAQAMI (`vid`): jadvalning ketma-ket `rowid` si — kanalda hujjat nomi `<tur>_<raqam>_<bo'lak>.bin` bo'ladi (`doc_kind`: vid/img/audio/font/file) (o'qish oson, qism raqami kabi).
    let r = turso_exec(env,
        "INSERT INTO vd_queue (name,size,chunks,vkey,state,layout,chunk_ids,msg_ids,created) VALUES (?,?,1,?,'staging','chunks','','',?) \
         ON CONFLICT(name) DO UPDATE SET size=excluded.size, chunks=1, vkey=excluded.vkey, state='staging', layout='chunks', \
           chunk_ids='', msg_ids='', lease=0, tries=0, created=excluded.created RETURNING id AS vid",
        vec![TursoArg::text(name), TursoArg::int(size as i64), TursoArg::text(&key_hex), TursoArg::int(now_ms())]).await?;
    let vid = first_row(&r).map(|x| jint(&x, "vid")).unwrap_or(0);
    let sent = tg_send_document(&tg_secret(env, "TELEGRAM_BOT_TOKEN"), vd_channel(env), &format!("{}_{vid}_1.bin", doc_prefix(name)),
        "", &bytes).await;
    let (fid, mid) = match sent {
        Ok(v) => v,
        Err(e) => {
            let _ = turso_exec(env, "DELETE FROM vd_queue WHERE name=? AND state='staging' AND chunk_ids=''", vec![TursoArg::text(name)]).await;
            return Err(e);
        }
    };
    turso_exec(env, "UPDATE vd_queue SET state='ready', chunk_ids=?, msg_ids=? WHERE name=?",
        vec![TursoArg::text(&fid), TursoArg::text(&mid.to_string()), TursoArg::text(name)]).await?;
    let _ = try_attach(env, name).await;
    // Write-through kesh: ma'lumot va shifrlangan bo'lak (oyna 0) shu mintaqada darrov tayyor.
    meta_put(name, &json!({"state": "ready", "size": size, "chunks": 1, "key": key_hex, "chunk_ids": fid})).await;
    let wname = format!("vd~{name}");
    if let (Ok(creq), Ok(resp)) = (Request::new(&warm_window_url(&wname, 0), Method::Get), Response::from_bytes(bytes)) {
        let headers = Headers::new();
        let _ = headers.set("Content-Type", "application/octet-stream");
        let _ = headers.set("Cache-Control", &format!("public, max-age={CHUNK_CACHE_SECONDS}"));
        let _ = headers.set("X-Total-Size", &size.to_string());
        let _ = headers.set("X-Window-Start", "0");
        let _ = Cache::default().put(&creq, resp.with_headers(headers)).await;
    }
    Ok((key_hex, size))
}

/// `PUT /api/vd/small?name=<nom>` (sessiya + ruxsat, tana = OCHIQ fayl, <= 18 MiB): worker yangi tasodifiy kalit yasaydi, AES-128-CTR bilan shifrlaydi,
/// `.bin` hujjat sifatida YANGI kanalga yuboradi va bazaga `ready` yozadi (bitta bo'lak). Kesh ham shu zahoti to'ldiriladi (ma'lumot va
/// shifrlangan bo'lak) — birinchi ochish ham tez. Qaytadi: `{ok, key, size}`; katta fayl uchun 413 (ilova R2 + Actions yo'liga o'tadi).
pub(crate) async fn small(env: &Env, me: i64, admin: bool, name: &str, mut req: Request) -> Result<Response> {
    if !upload_allowed(name, me, admin) {
        return json_resp(&json!({"error": "ruxsat yo'q"}), 403);
    }
    let declared = req.headers().get("Content-Length").ok().flatten().and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
    if declared > CHUNK {
        return json_resp(&json!({"error": "katta fayl", "limit": CHUNK}), 413);
    }
    ensure(env).await?;
    // Xuddi shu nom allaqachon tayyor (qayta yuborilgan so'rov) — mavjud kalit qaytadi.
    if let Some(m) = meta_one(env, name).await.filter(|m| m["state"].as_str() == Some("ready")) {
        return ok(json!({"ok": true, "key": m["key"], "size": m["size"], "existing": true}));
    }
    let mut bytes = req.bytes().await?;
    if bytes.is_empty() || bytes.len() as u64 > CHUNK {
        return json_resp(&json!({"error": if bytes.is_empty() { "bo'sh fayl" } else { "katta fayl" }, "limit": CHUNK}), if bytes.is_empty() { 400 } else { 413 });
    }
    let (key_hex, size) = match store_small(env, name, bytes).await {
        Ok(v) => v,
        // Xato matni ilovaga qaytadi (ilova uni ko'rsatadi) — jim 500 emas.
        Err(e) => return json_resp(&json!({"error": e.to_string()}), 502),
    };
    ok_nostore(json!({"ok": true, "key": key_hex, "size": size}))
}

/// Kim nimani yuklay oladi: admin — qism (`ep_*`) va rasm fayllari (`.jpg/.jpeg/.png/.webp`); oddiy foydalanuvchi — faqat O'Z profil rasmi
/// (`avatar_<id>_*.jpg`).
fn upload_allowed(name: &str, me: i64, admin: bool) -> bool {
    if !tg_safe_name(name) {
        return false;
    }
    let img = [".jpg", ".jpeg", ".png", ".webp"].iter().any(|e| name.ends_with(e));
    if me > 0 && name.starts_with(&format!("avatar_{me}_")) && img {
        return true;
    }
    // Yozishma fayli: o'z `chat_<id>_*` fayli (rasm, video, ovoz, hujjat — hammasi).
    if me > 0 && name.starts_with(&format!("chat_{me}_")) {
        return true;
    }
    // Admin: qism, asl video (kodlash uchun), yozishma va rasm fayllari.
    admin && (name.starts_with("ep_") || name.starts_with("orig_") || name.starts_with("chat_") || img)
}

/// `POST /api/vd/up/init` (admin): ilova BUTUN (qurilmada shifrlangan) faylni R2 ga to'g'ridan-to'g'ri yozadi; worker faqat imzolangan
/// PUT havolasini beradi. `{name, size, key}` -> `{url}`.
pub(crate) async fn up_init(env: &Env, me: i64, admin: bool, body: &Value) -> Result<Response> {
    ensure(env).await?;
    let name = body["name"].as_str().unwrap_or("").to_string();
    let size = jint(body, "size");
    let key = body["key"].as_str().unwrap_or("").to_ascii_lowercase();
    if !upload_allowed(&name, me, admin) || size <= 0 || size > 5 * 1024 * 1024 * 1024 - CHUNK as i64 || !valid_file_key(&key) {
        return bad("so'rov");
    }
    let Some(bk) = bucket(env) else { return json_resp(&json!({"error": "r2"}), 503) };
    let Some(url) = r2::signed_key(env, "PUT", &format!("{SINGLE}{name}"), 6 * 3600) else {
        return json_resp(&json!({"error": "R2 kalitlari sozlanmagan"}), 501);
    };
    r2_delete_all(&bk, &name).await;
    turso_exec(env,
        "INSERT INTO vd_queue (name,size,chunks,vkey,state,layout,created) VALUES (?,?,?,?,'uploading','single',?) \
         ON CONFLICT(name) DO UPDATE SET size=excluded.size, chunks=excluded.chunks, vkey=excluded.vkey, state='uploading', layout='single', \
           chunk_ids='', msg_ids='', lease=0, tries=0, created=excluded.created",
        vec![TursoArg::text(&name), TursoArg::int(size), TursoArg::int(chunks_of(size)), TursoArg::text(&key), TursoArg::int(now_ms())]).await?;
    ok(json!({"url": url, "chunk": CHUNK}))
}

/// `POST /api/vd/up/complete {name}` (admin): R2 dagi fayl hajmi tekshiriladi va saqlash navbatiga qo'yiladi (yuklovchi workflow uyg'otiladi).
pub(crate) async fn up_complete(env: &Env, me: i64, admin: bool, body: &Value) -> Result<Response> {
    ensure(env).await?;
    let name = body["name"].as_str().unwrap_or("").to_string();
    if !upload_allowed(&name, me, admin) {
        return bad("nom");
    }
    let Some(bk) = bucket(env) else { return json_resp(&json!({"error": "r2"}), 503) };
    let Some(row) = turso_exec(env, "SELECT size FROM vd_queue WHERE name=? AND state='uploading'", vec![TursoArg::text(&name)]).await?.pipe_first() else {
        return json_resp(&json!({"error": "holat"}), 409);
    };
    let got = bk.head(format!("{SINGLE}{name}")).await.ok().flatten().map(|o| o.size() as i64);
    if got != Some(jint(&row, "size")) {
        let _ = turso_exec(env, "UPDATE vd_queue SET state='failed' WHERE name=?", vec![TursoArg::text(&name)]).await;
        return json_resp(&json!({"error": format!("hajm mos emas ({got:?})")}), 422);
    }
    turso_exec(env, "UPDATE vd_queue SET state='uploaded' WHERE name=? AND state='uploading'", vec![TursoArg::text(&name)]).await?;
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
                if !tombstoned_many(env, &[file.to_string()]).await.is_empty() {
                    return reply("error fayl topilmadi", 0);
                }
                return reply("cached", t);
            }
        }
    }
    // Fayl ma'lumoti: `ready` bo'lsa Cloudflare keshidan (bazaga bormaydi), aks holda bazadan.
    let mm = meta_many(env, &[file.to_string()]).await?;
    let Some(f) = mm.get(file).cloned() else { return reply("error fayl topilmadi", 0) };
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
    fn tavsif_nomi() {
        assert_eq!(label("ep_3_1_12_720p_1794221272.mp4"), "Qism 3-1-12 (720p)");
        assert_eq!(label("sys_emojifont_1794223126.bin"), "Emoji shrifti");
        assert_eq!(label("avatar_7_1794.jpg"), "Profil rasmi (foydalanuvchi 7)");
        assert_eq!(label("chat_7_1794.jpg"), "Yozishma fayli (foydalanuvchi 7)");
        assert_eq!(label("orig_5.mp4"), "Asl video");
        assert_eq!(label("boshqa.bin"), "boshqa.bin");
        assert_eq!(doc_prefix("ep_3_1_12_1080p_1794221272.mp4"), "vid_1080p");
        assert_eq!(doc_prefix("ep_3_1_12_720p_1794221272.mp4"), "vid_720p");
        assert_eq!(doc_prefix("orig_5.mp4"), "vid");
        assert_eq!(doc_prefix("avatar_7_1794.jpg"), "img");
    }

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
    fn yuklash_ruxsati() {
        assert!(upload_allowed("avatar_7_1.jpg", 7, false));
        assert!(!upload_allowed("avatar_8_1.jpg", 7, false));
        assert!(!upload_allowed("ep_1_2_720p_3.mp4", 7, false));
        assert!(upload_allowed("ep_1_2_720p_3.mp4", 7, true));
        assert!(upload_allowed("poster_5.jpg", 7, true));
        assert!(!upload_allowed("poster_5.jpg", 7, false));
        assert!(upload_allowed("orig_9_1_2_3.mp4", 7, true));
        assert!(!upload_allowed("orig_9_1_2_3.mp4", 7, false));
        assert!(upload_allowed("chat_7_5.m4a", 7, false));
        assert!(!upload_allowed("chat_8_5.m4a", 7, false));
        assert!(upload_allowed("chat_8_5.m4a", 7, true));
    }

    #[test]
    fn kalit_hex() {
        assert_eq!(hex_decode16("00112233445566778899aabbccddeeff").map(|k| k[15]), Some(0xff));
        assert!(hex_decode16("zz").is_none());
        assert!(hex_decode16(&"g".repeat(32)).is_none());
    }

    #[test]
    fn ommaviy_rasm() {
        assert!(public_image_name("avatar_7_1.jpg"));
        assert!(!public_image_name("ep_1_2_720p_3.mp4"));
        assert!(!public_image_name("chat_7_1.jpg"));
    }

    #[test]
    fn actions_yollari() {
        assert!(is_actions_path("/api/vd/claim"));
        assert!(is_actions_path("/api/vd/put"));
        assert!(!is_actions_path("/api/vd/info"));
        assert!(!is_actions_path("/api/vd/up/init"));
    }
}

// ── Egalar jadvallaridagi fayl ma'lumoti ustunlari (vd_queue ni olib tashlash rejasining 1-qadami) ─────────────────────────────
//
// Nomga havola qiluvchi har ustun (`ref`) uchun oltita ustun qo'shiladi: `<ref>_vd_name`, `_size`, `_chunks`, `_key`, `_ids` (Telegram `file_id` lar,
// vergul bilan), `_msgs` (kanal xabar raqamlari). `ref` — jadval ichida noyob qisqa nom. Hozircha ular bo'sh: keyingi qadamlarda `ready`
// bo'lgan fayl ma'lumoti shu yerga yoziladi.
/// (jadval, ref nomlari). `epizod_db` da har sifat alohida ref (hajm/kalit eski ustunlari avvaldan bor, lekin yangi kalit shu yerda).
pub(crate) const OWNER_REFS: [(&str, &[&str]); 6] = [
    ("anime_db", &["photo"]),
    ("season_db", &["photo"]),
    ("epizod_db", &["q360", "q480", "q720", "q1080"]),
    ("users_db", &["avatar"]),
    ("comments_db", &["media"]),
    ("chat_messages", &["media"]),
];

/// Egalar ustunlari: (qo'shimcha, SQL turi va standart qiymati).
const OWNER_COLS: [(&str, &str); 6] = [
    // `name` — ma'lumot QAYSI fayl nomiga tegishli (egasining havolasi almashsa eski fayl ma'lumoti yangisiga o'tib ketmasin).
    ("name", "TEXT DEFAULT ''"),
    ("size", "INTEGER DEFAULT 0"),
    ("chunks", "INTEGER DEFAULT 0"),
    ("key", "TEXT DEFAULT ''"),
    ("ids", "TEXT DEFAULT ''"),
    ("msgs", "TEXT DEFAULT ''"),
];

pub(crate) fn owner_col(r: &str, suffix: &str) -> String {
    format!("{r}_vd_{suffix}")
}

/// Jadvalda hali yo'q ustunlar uchun `ALTER TABLE` buyruqlari (`have` — jadvalning hozirgi ustunlari).
pub(crate) fn owner_alter_sqls(table: &str, refs: &[&str], have: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for r in refs {
        for (suffix, ty) in OWNER_COLS {
            let col = owner_col(r, suffix);
            if !have.iter().any(|h| *h == col) {
                out.push(format!("ALTER TABLE {table} ADD COLUMN {col} {ty}"));
            }
        }
    }
    out
}

#[cfg(test)]
mod owner_cols_tests {
    use super::*;

    #[test]
    fn adds_only_missing_columns() {
        let have: Vec<String> = ["id", "photo_vd_name", "photo_vd_size", "photo_vd_chunks"].iter().map(|s| s.to_string()).collect();
        let q = owner_alter_sqls("anime_db", &["photo"], &have);
        assert_eq!(q, vec![
            "ALTER TABLE anime_db ADD COLUMN photo_vd_key TEXT DEFAULT ''".to_string(),
            "ALTER TABLE anime_db ADD COLUMN photo_vd_ids TEXT DEFAULT ''".to_string(),
            "ALTER TABLE anime_db ADD COLUMN photo_vd_msgs TEXT DEFAULT ''".to_string(),
        ]);
    }

    #[test]
    fn full_set_for_fresh_table_and_all_present() {
        let q = owner_alter_sqls("epizod_db", &["q360", "q1080"], &[]);
        assert_eq!(q.len(), 12);
        assert!(q.iter().any(|s| s.contains("q1080_vd_msgs")));
        let have: Vec<String> = q.iter().map(|s| s.split_whitespace().nth(5).unwrap().to_string()).collect();
        assert!(owner_alter_sqls("epizod_db", &["q360", "q1080"], &have).is_empty());
    }
}

// ── Fayl nomi -> egasi (vd_queue rejasining 2-qadami; hali hech qayerda ishlatilmaydi) ────────────────────────────────────────
//
// Nom formatlari (kodda va bazada tekshirilgan):
//   `ep_<anime>_<sezon>_<qism>_<sifat>p_<vaqt>.mp4`  -> `epizod_db` qatori (anime_id, season_id, epizod_id), ref `q<sifat>`
//   `anime_<vaqt>.<kengaytma>`                       -> `anime_db.photo_url`, ref `photo`
//   `season_<vaqt>.<kengaytma>`                      -> `season_db.photo_url`, ref `photo`
//   `avatar_<user>_<vaqt>.jpg`                       -> `users_db.avatar_file`, ref `avatar`
//   `chat_<user>_<vaqt>.<kengaytma>`                 -> `chat_messages.media_file` yoki `comments_db.media_file`, ref `media`
// Boshqalar (`sys_emojifont_*`, `selftest_*`, `orig_*`, ...) egasiz: ma'lumoti navbat jadvalida qoladi.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum OwnerArg {
    Int(i64),
    Text(String),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OwnerRef {
    pub table: &'static str,
    /// Ustun to'plamining nomi (`OWNER_REFS` dagi ref): ustunlar `<ref>_vd_size` ...
    pub r: String,
    /// `WHERE` sharti (`?` belgilari bilan) va uning argumentlari.
    pub where_sql: String,
    pub args: Vec<OwnerArg>,
}

fn all_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// `<bosh>_<vaqt>.<kengaytma>` ko'rinishidagi nom (bosh — `anime` / `season`).
fn stamped(name: &str, head: &str) -> bool {
    let Some(rest) = name.strip_prefix(head).and_then(|r| r.strip_prefix('_')) else { return false };
    let Some((stamp, ext)) = rest.split_once('.') else { return false };
    all_digits(stamp) && !ext.is_empty() && ext.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// Nom bo'yicha mumkin bo'lgan egalar (odatda bitta; `chat_` nomi yozishma yoki izohga tegishli bo'lishi mumkin). Egasiz nom — bo'sh ro'yxat.
pub(crate) fn owners_of(name: &str) -> Vec<OwnerRef> {
    if !tg_safe_name(name) {
        return Vec::new();
    }
    if let Some(rest) = name.strip_prefix("ep_") {
        let p: Vec<&str> = rest.split('.').next().unwrap_or("").split('_').collect();
        if name.ends_with(".mp4") && p.len() == 5 && p[..3].iter().all(|x| all_digits(x)) && all_digits(p[4]) {
            if let Some(q) = p[3].strip_suffix('p').filter(|d| all_digits(d) && matches!(*d, "360" | "480" | "720" | "1080")) {
                let (a, s, e) = (p[0].parse::<i64>(), p[1].parse::<i64>(), p[2].parse::<i64>());
                if let (Ok(a), Ok(s), Ok(e)) = (a, s, e) {
                    return vec![OwnerRef {
                        table: "epizod_db",
                        r: format!("q{q}"),
                        where_sql: format!("anime_id=? AND season_id=? AND epizod_id=? AND url_{q}p=?"),
                        args: vec![OwnerArg::Int(a), OwnerArg::Int(s), OwnerArg::Int(e), OwnerArg::Text(name.to_string())],
                    }];
                }
            }
        }
        return Vec::new();
    }
    if stamped(name, "anime") {
        return vec![OwnerRef { table: "anime_db", r: "photo".into(), where_sql: "photo_url=?".into(), args: vec![OwnerArg::Text(name.to_string())] }];
    }
    if stamped(name, "season") {
        return vec![OwnerRef { table: "season_db", r: "photo".into(), where_sql: "photo_url=?".into(), args: vec![OwnerArg::Text(name.to_string())] }];
    }
    if let Some(rest) = name.strip_prefix("avatar_") {
        let mut it = rest.split('_');
        if let (Some(u), Some(t), None) = (it.next(), it.next(), it.next()) {
            if all_digits(u) && t.strip_suffix(".jpg").map_or(false, all_digits) {
                return vec![OwnerRef { table: "users_db", r: "avatar".into(), where_sql: "avatar_file=?".into(), args: vec![OwnerArg::Text(name.to_string())] }];
            }
        }
        return Vec::new();
    }
    if let Some(rest) = name.strip_prefix("chat_") {
        let mut it = rest.split('_');
        if let (Some(u), Some(t), None) = (it.next(), it.next(), it.next()) {
            if all_digits(u) && t.split_once('.').map_or(false, |(st, ext)| all_digits(st) && !ext.is_empty()) {
                return ["chat_messages", "comments_db"].iter().map(|tb| OwnerRef {
                    table: tb, r: "media".into(), where_sql: "media_file=?".into(), args: vec![OwnerArg::Text(name.to_string())],
                }).collect();
            }
        }
    }
    Vec::new()
}

#[cfg(test)]
mod owner_of_tests {
    use super::*;

    #[test]
    fn episode_names() {
        let o = owners_of("ep_11_1_10_480p_1794297649616.mp4");
        assert_eq!(o.len(), 1);
        assert_eq!((o[0].table, o[0].r.as_str()), ("epizod_db", "q480"));
        assert_eq!(o[0].where_sql, "anime_id=? AND season_id=? AND epizod_id=? AND url_480p=?");
        assert_eq!(o[0].args[..3], [OwnerArg::Int(11), OwnerArg::Int(1), OwnerArg::Int(10)]);
        assert!(owners_of("ep_11_1_10_999p_1.mp4").is_empty()); // noma'lum sifat
        assert!(owners_of("ep_11_1_10_480p_1794297649616_f.mp4").is_empty()); // fMP4 / ortiqcha bo'lak
        assert!(owners_of("ep_a_1_10_480p_1.mp4").is_empty());
    }

    #[test]
    fn photo_avatar_chat() {
        assert_eq!(owners_of("anime_1794231437322.jpg")[0].table, "anime_db");
        assert_eq!(owners_of("season_1794287206244.png")[0].table, "season_db");
        assert_eq!(owners_of("avatar_1_1794246570946.jpg")[0].table, "users_db");
        assert!(owners_of("avatar_1_1794246570946.png").is_empty()); // faqat .jpg
        let c = owners_of("chat_1_1794232801099.jpg");
        assert_eq!(c.iter().map(|x| x.table).collect::<Vec<_>>(), vec!["chat_messages", "comments_db"]);
        assert!(c.iter().all(|x| x.r == "media"));
    }

    #[test]
    fn no_owner() {
        for n in ["sys_emojifont_1794244734.bin", "selftest_1.bin", "orig_5_1_1_1.mp4", "anime_x.jpg", "", "../ep_1_1_1_480p_1.mp4"] {
            assert!(owners_of(n).is_empty(), "{n}");
        }
    }

    #[test]
    fn every_owner_has_columns() {
        // Har egaga qaytadigan (jadval, ref) juftligi OWNER_REFS da bo'lishi shart (1-qadamdagi ustunlar).
        for n in ["ep_1_1_1_360p_1.mp4", "ep_1_1_1_720p_1.mp4", "ep_1_1_1_1080p_1.mp4", "anime_1.jpg", "season_1.jpg", "avatar_1_1.jpg", "chat_1_1.jpg"] {
            for o in owners_of(n) {
                assert!(OWNER_REFS.iter().any(|(t, rs)| *t == o.table && rs.contains(&o.r.as_str())), "{n} -> {} {}", o.table, o.r);
            }
        }
    }
}
