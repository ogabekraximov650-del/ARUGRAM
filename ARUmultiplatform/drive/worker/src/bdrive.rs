// ══════════════════════════════════════════════════════════════
//  BDRIVE: Bot API orqali fayl/video ombori (kichik alohida ilova uchun)
// ══════════════════════════════════════════════════════════════
//
// TALAB (foydalanuvchi): ilova faylni R2'ga yuklaydi -> ikkinchi akkauntdagi Actions (`bdrive.yml`) uni R2'dan olib 18 MB
// bo'laklarga bo'ladi va Bot API orqali ARUMEDIATV kanaliga saqlaydi (KODLASH YO'Q). Ko'rish: bo'lak Telegram'dan TO'G'RIDAN-TO'G'RI
// Cloudflare keshiga yoziladi (R2 orqali o'tmaydi) va FAQAT keshga tushgandan keyin ilovaga uzatiladi (video worker orqali o'tadi). Kesh tizimi ARUMEDIATV'dagi bilan BIR XIL: 480 MiB'lik "oyna" yozuvi, `/api/play/...` (b2_play) uni
// kesib beradi, muddat — 1000 kun (`CHUNK_CACHE_SECONDS`).
//
// Baza — alohida Turso (`BD_TURSO_URL` / `BD_TURSO_TOKEN`), asosiy bazaga tegilmaydi. Bot tokeni — `BD_BOT_TOKEN` (ilovaga
// HECH QACHON berilmaydi; faqat `ENCODE_TOKEN` bilan kirgan Actions run'iga `claim` javobida beriladi).
//
//   Ilova (imzo `X-App-Sig`):  POST /api/bd/init, PUT /api/bd/part, POST /api/bd/finish, POST /api/bd/thumb,
//                              GET /api/bd/list, GET /api/bd/thumb/<id>, GET /api/warm/bd~<id>?w=N, GET /api/play/bd~<id>
//   Actions (`ENCODE_TOKEN`):  POST /api/bd/claim, GET /api/bd/read, POST /api/bd/chunkdone, POST /api/bd/done, POST /api/bd/fail

use crate::cache::*;
use crate::*;
use core::sync::atomic::{AtomicBool, Ordering};
use worker::{Bucket, FixedLengthStream, Range, UploadedPart};

/// Bitta Telegram bo'lagi: bot 20 MB gacha yuklab oladi, 50 MB gacha yuklaydi.
pub(crate) const CHUNK: u64 = 18 * 1024 * 1024;
/// Ilovadan R2 ga yuklash qismi (ko'p qismli yuklash): 64 MiB. Cloudflare so'rov tanasi 100 MB bilan cheklangan, shu sabab bitta so'rovda
/// butun faylni yuborib bo'lmaydi; R2 da baribir BITTA butun obyekt bo'ladi, Telegram bo'laklari (18 MiB) esa worker tomonida `Range` bilan olinadi.
pub(crate) const PART: u64 = 64 * 1024 * 1024;
/// Bitta qism uchun qat'iy chegara (100 MB so'rov chegarasidan past).
const PART_MAX: u64 = 96 * 1024 * 1024;
const UP: &str = "bd/up/";
/// Fayl hajmining yuqori chegarasi (R2 multipart: 10 000 bo'lak x 18 MiB ≈ 175 GB; amalda 4 GB).
/// Eng katta fayl: 50 GiB (R2 ko'p qismli yuklash 10 000 qismgacha: 18 MiB × 10 000 ≈ 175 GiB, zaxira bilan).
const MAX_SIZE: i64 = 50 * 1024 * 1024 * 1024;
const LEASE_MS: i64 = 30 * 60 * 1000;
/// Worker o'zi saqlayotganda ijara qisqa: har bo'lakdan keyin yangilanadi, worker o'lsa 2 daqiqada qayta navbatga qaytadi.
const WORKER_LEASE_MS: i64 = 2 * 60 * 1000;
const MAX_TRIES: i64 = 3;
const WORKFLOW: &str = "bdrive.yml";

static SCHEMA_OK: AtomicBool = AtomicBool::new(false);

fn bucket(env: &Env) -> Option<Bucket> {
    env.bucket("R2").ok()
}

// ── Baza (alohida Turso) ─────────────────────────────────────

async fn sql_many(env: &Env, stmts: &[(&str, Vec<TursoArg>)]) -> Result<Vec<Value>> {
    let url = tg_secret(env, "BD_TURSO_URL").replacen("libsql://", "https://", 1);
    let token = tg_secret(env, "BD_TURSO_TOKEN");
    if url.is_empty() || token.is_empty() {
        return Err(Error::RustError("BD_TURSO sozlanmagan".into()));
    }
    let mut reqs: Vec<Value> = stmts
        .iter()
        .map(|(s, a)| json!({"type": "execute", "stmt": {"sql": s, "args": a}}))
        .collect();
    reqs.push(json!({"type": "close"}));
    let h = Headers::new();
    h.set("Authorization", &format!("Bearer {token}"))?;
    h.set("Content-Type", "application/json")?;
    let req = Request::new_with_init(
        &format!("{url}/v2/pipeline"),
        RequestInit::new().with_method(Method::Post).with_headers(h).with_body(Some(json!({"requests": reqs}).to_string().into())),
    )?;
    let mut r = Fetch::Request(req).send().await?;
    let d: Value = r.json().await.unwrap_or(json!({}));
    let list = d["results"].as_array().cloned().unwrap_or_default();
    let mut out = Vec::new();
    for item in list.iter().take(stmts.len()) {
        if item["type"] == json!("error") {
            return Err(Error::RustError(format!("baza: {}", item["error"]["message"].as_str().unwrap_or("xato"))));
        }
        out.push(item["response"]["result"].clone());
    }
    if out.len() != stmts.len() {
        return Err(Error::RustError("baza javobi to'liq emas".into()));
    }
    Ok(out)
}

async fn sql(env: &Env, q: &str, args: Vec<TursoArg>) -> Result<Value> {
    Ok(sql_many(env, &[(q, args)]).await?.remove(0))
}

async fn ensure(env: &Env) -> Result<()> {
    if SCHEMA_OK.load(Ordering::Relaxed) {
        return Ok(());
    }
    sql_many(env, &[
        ("CREATE TABLE IF NOT EXISTS bd_files (id TEXT PRIMARY KEY, name TEXT NOT NULL, kind TEXT NOT NULL DEFAULT 'file', mime TEXT NOT NULL DEFAULT '', \
          size INTEGER NOT NULL, w INTEGER NOT NULL DEFAULT 0, h INTEGER NOT NULL DEFAULT 0, dur INTEGER NOT NULL DEFAULT 0, \
          state TEXT NOT NULL, upload_id TEXT NOT NULL DEFAULT '', chunks INTEGER NOT NULL DEFAULT 0, lease INTEGER NOT NULL DEFAULT 0, \
          tries INTEGER NOT NULL DEFAULT 0, created INTEGER NOT NULL)", vec![]),
    ]).await?;
    // Telegram file_id lar bitta ustunda, vergul bilan: `chunk0_fileid,chunk1_fileid,...` (indeks = bo'lak raqami).
    // Eski jadvalda ustun yo'q bo'lsa qo'shiladi (bor bo'lsa xato e'tiborsiz).
    let _ = sql(env, "ALTER TABLE bd_files ADD COLUMN chunk_ids TEXT NOT NULL DEFAULT ''", vec![]).await;
    let _ = sql(env, "ALTER TABLE bd_files ADD COLUMN thumb_id TEXT NOT NULL DEFAULT ''", vec![]).await;
    // INDEKSLAR (Turso "o'qilgan qatorlar" bo'yicha hisoblaydi, LIMIT bunga yordam bermaydi): indeksiz har so'rov BUTUN jadvalni
    // skanerlardi — 1 milliard qatorda bitta so'rov butun oylik chegarani yeb qo'yadi.
    //   bd_list  — ro'yxat: eng yangi 200 qatorni indeks bo'yicha (saralashsiz) o'qiydi, jadval qancha katta bo'lmasin;
    //   bd_state — navbat/qayta navbat/tozalash: faqat kerakli holatdagi qatorlarni o'qiydi.
    let _ = sql(env, "CREATE INDEX IF NOT EXISTS bd_list ON bd_files(created DESC) WHERE state IN ('ready','uploaded','storing')", vec![]).await;
    let _ = sql(env, "CREATE INDEX IF NOT EXISTS bd_state ON bd_files(state, created)", vec![]).await;
    // DELTA SINXRONLASH: har qatorda so'nggi o'zgarish vaqti (`updated`, ms, BAZA soati — ma'lumot markazlari soati farqi bo'lmaydi).
    let _ = sql(env, "ALTER TABLE bd_files ADD COLUMN updated INTEGER NOT NULL DEFAULT 0", vec![]).await;
    let _ = sql(env, "CREATE INDEX IF NOT EXISTS bd_updated ON bd_files(updated)", vec![]).await;
    let _ = sql(env, "UPDATE bd_files SET updated=created WHERE updated=0", vec![]).await;
    // O'ZGARISHLAR JURNALI (DO'siz delta): har yozuvda (worker, qo'lda SQL — farqi yo'q) trigger bir tranzaksiyada jurnalga `seq` bilan
    // yozadi; bir qator uchun jurnalda BITTA yozuv (eskisi o'chadi), shu sabab u o'smaydi. `bd_meta.floor` — tozalangan eng katta `seq`.
    let _ = sql(env, "CREATE TABLE IF NOT EXISTS bd_journal (seq INTEGER PRIMARY KEY AUTOINCREMENT, id TEXT NOT NULL, op TEXT NOT NULL, at INTEGER NOT NULL)", vec![]).await;
    let _ = sql(env, "CREATE INDEX IF NOT EXISTS bd_journal_id ON bd_journal(id)", vec![]).await;
    let _ = sql(env, "CREATE TABLE IF NOT EXISTS bd_meta (k TEXT PRIMARY KEY, v INTEGER NOT NULL)", vec![]).await;
    let _ = sql(env, "CREATE TRIGGER IF NOT EXISTS bd_files_ai AFTER INSERT ON bd_files BEGIN DELETE FROM bd_journal WHERE id=NEW.id; INSERT INTO bd_journal(id,op,at) VALUES (NEW.id,'u',CAST((julianday('now')-2440587.5)*86400000 AS INTEGER)); END", vec![]).await;
    let _ = sql(env, "CREATE TRIGGER IF NOT EXISTS bd_files_au AFTER UPDATE ON bd_files BEGIN DELETE FROM bd_journal WHERE id=NEW.id; INSERT INTO bd_journal(id,op,at) VALUES (NEW.id,'u',CAST((julianday('now')-2440587.5)*86400000 AS INTEGER)); END", vec![]).await;
    let _ = sql(env, "CREATE TRIGGER IF NOT EXISTS bd_files_ad AFTER DELETE ON bd_files BEGIN DELETE FROM bd_journal WHERE id=OLD.id; INSERT INTO bd_journal(id,op,at) VALUES (OLD.id,'d',CAST((julianday('now')-2440587.5)*86400000 AS INTEGER)); END", vec![]).await;
    SCHEMA_OK.store(true, Ordering::Relaxed);
    Ok(())
}

/// Bot API `sendDocument` (qo'lda yig'ilgan multipart): (file_id, message_id). Hajm Telegram javobi bilan solishtiriladi.
async fn tg_send_document(token: &str, channel: i64, name: &str, data: &[u8], expect: u64) -> Result<(String, i64)> {
    if token.is_empty() {
        return Err(Error::RustError("BD_BOT_TOKEN yo'q".into()));
    }
    let boundary = format!("----arudrive{}", now_ms());
    let safe: String = name.chars().map(|c| if c.is_ascii_graphic() && c != '"' && c != '\\' { c } else { '_' }).collect();
    let mut body: Vec<u8> = Vec::with_capacity(data.len() + 512);
    for (k, v) in [("chat_id", channel.to_string()), ("disable_notification", "true".to_string())] {
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
    if d["ok"] != json!(true) {
        return Err(Error::RustError(format!("Telegram rad etdi: {} (retry_after {})", d["description"].as_str().unwrap_or("noma'lum"), d["parameters"]["retry_after"])));
    }
    let doc = &d["result"]["document"];
    if doc["file_size"].as_u64().unwrap_or(expect) != expect {
        return Err(Error::RustError("Telegram hajmi mos emas".into()));
    }
    let fid = doc["file_id"].as_str().unwrap_or("").to_string();
    if fid.is_empty() {
        return Err(Error::RustError("Telegram file_id bermadi".into()));
    }
    Ok((fid, d["result"]["message_id"].as_i64().unwrap_or(0)))
}

fn thumb_resp(bytes: Vec<u8>) -> Result<Response> {
    let mut resp = Response::from_bytes(bytes)?;
    set_cors(&mut resp);
    let h = resp.headers_mut();
    h.set("Content-Type", "image/jpeg")?;
    h.set("Cache-Control", &format!("public, max-age={CHUNK_CACHE_SECONDS}, immutable"))?;
    Ok(resp)
}

/// ── WORKER O'ZI: R2 -> Telegram ────────────────────────────────
///
/// `uploaded` holatidagi fayllarni birma-bir oladi (ijara bilan), har 18 MiB bo'lakni R2'dan `Range` bilan o'qib Bot API'ga
/// yuboradi va `file_id` ni `chunk_ids` ga qo'shadi. Vaqt byudjeti (`budget_ms`) tugasa ijara qaytariladi va davomini keyingi
/// chaqiruv (cron yoki yangi yuklash) oladi — tayyor bo'laklar QAYTA yuborilmaydi. Hammasi saqlansa fayl `ready`, R2 nusxasi o'chadi.
pub(crate) async fn process(env: &Env, budget_ms: i64) -> Result<String> {
    Ok(process_ex(env, budget_ms).await?.0)
}

/// Katta fayl (GB'lab) bitta chaqiruvda tugamaydi, shu sabab `process_ex` ish qolgan bo'lsa shuni (`more`) bildiradi va
/// worker O'ZINI qayta chaqiradi (zanjir): har bo'g'in ~20 soniya ishlaydi. Zanjir faqat PROGRESS bo'lganda davom etadi
/// (xato bo'lsa to'xtaydi — cron keyinroq oladi), shu sabab cheksiz aylanib qolmaydi.
pub(crate) async fn process_and_chain(env: &Env) {
    if let Ok((_, progressed, more)) = process_ex(env, 21_000).await {
        if more && progressed {
            // O'ZIGA xizmat ulanishi (`SELF`): worker o'z `workers.dev` manziliga so'rov yubora olmaydi (Cloudflare sikl deb to'sadi).
            let h = Headers::new();
            let _ = h.set("X-Encode-Token", &tg_secret(env, "ENCODE_TOKEN"));
            if let (Ok(svc), Ok(req)) = (
                env.service("SELF"),
                Request::new_with_init(
                    "https://self/api/bd/process?bg=1",
                    RequestInit::new().with_method(Method::Post).with_headers(h),
                ),
            ) {
                let _ = svc.fetch_request(req).await;
            }
        }
    }
}

/// (jurnal, progress bo'ldimi, ish qoldimi)
pub(crate) async fn process_ex(env: &Env, budget_ms: i64) -> Result<(String, bool, bool)> {
    ensure(env).await?;
    let Some(bk) = bucket(env) else { return Ok(("r2 yo'q".into(), false, false)) };
    let mut progressed = false;
    let mut drained = false;
    let t0 = now_ms();
    let mut out = String::new();
    while now_ms() - t0 < budget_ms - 6_000 {
        let now = now_ms();
        let r = sql_many(env, &[
            ("UPDATE bd_files SET updated=CAST((julianday('now')-2440587.5)*86400000 AS INTEGER), state='uploaded' WHERE state='storing' AND lease<? AND tries<?", vec![TursoArg::int(now), TursoArg::int(MAX_TRIES)]),
            ("UPDATE bd_files SET updated=CAST((julianday('now')-2440587.5)*86400000 AS INTEGER), state='failed' WHERE state='storing' AND lease<? AND tries>=?", vec![TursoArg::int(now), TursoArg::int(MAX_TRIES)]),
            ("SELECT id,name,size,chunks,chunk_ids FROM bd_files WHERE state='uploaded' ORDER BY created LIMIT 1", vec![]),
        ]).await?;
        let Some(f) = first(&r[2]) else {
            drained = true;
            break;
        };
        let id = f["id"].as_str().unwrap_or("").to_string();
        let got = sql(env, "UPDATE bd_files SET updated=CAST((julianday('now')-2440587.5)*86400000 AS INTEGER), state='storing', lease=?, tries=tries+1 WHERE id=? AND state='uploaded' RETURNING id",
            vec![TursoArg::int(now + WORKER_LEASE_MS), TursoArg::text(&id)]).await?;
        if first(&got).is_none() {
            continue; // boshqa chaqiruv oldi
        }
        list_invalidate(env).await; // uploaded -> storing
        let name = f["name"].as_str().unwrap_or("fayl").to_string();
        let size = f["size"].as_i64().unwrap_or(0).max(0) as u64;
        let chunks = f["chunks"].as_i64().unwrap_or(0).max(0) as usize;
        let mut list: Vec<String> = split_ids(f["chunk_ids"].as_str().unwrap_or("")).into_iter().map(String::from).collect();
        let key = format!("{UP}{id}");
        let token = tg_secret(env, "BD_BOT_TOKEN");
        let channel = tg_channel_id(env);
        let mut stopped = false;
        let mut failed: Option<String> = None;
        while list.len() < chunks {
            if now_ms() - t0 > budget_ms - 6_000 {
                stopped = true;
                break;
            }
            let n = list.len() as u64;
            let offset = n * CHUNK;
            let length = CHUNK.min(size.saturating_sub(offset));
            let res: Result<String> = async {
                let obj = bk.get(&key).range(Range::OffsetWithLength { offset, length }).execute().await?
                    .ok_or_else(|| Error::RustError("R2'da fayl yo'q".into()))?;
                let bytes = obj.body().ok_or_else(|| Error::RustError("R2 tanasi yo'q".into()))?.bytes().await?;
                if bytes.len() as u64 != length {
                    return Err(Error::RustError(format!("R2 hajmi mos emas ({} != {length})", bytes.len())));
                }
                let (fid, _) = tg_send_document(&token, channel, &format!("{name}.part{n:04}"), &bytes, length).await?;
                Ok(fid)
            }.await;
            match res {
                Ok(fid) => {
                    list.push(fid);
                    progressed = true;
                    list_invalidate(env).await; // "Saqlanmoqda x/y" yangilanadi
                    // Bo'lak saqlandi: urinishlar hisobi nollanadi (katta fayl bir necha chaqiruvda tugashi mumkin), ijara yangilanadi.
                    sql(env, "UPDATE bd_files SET updated=CAST((julianday('now')-2440587.5)*86400000 AS INTEGER), chunk_ids=?, lease=?, tries=0 WHERE id=? AND state='storing'",
                        vec![TursoArg::text(&list.join(",")), TursoArg::int(now_ms() + WORKER_LEASE_MS), TursoArg::text(&id)]).await?;
                }
                Err(e) => {
                    failed = Some(e.to_string());
                    break;
                }
            }
        }
        if let Some(e) = failed {
            let _ = sql(env, "UPDATE bd_files SET updated=CAST((julianday('now')-2440587.5)*86400000 AS INTEGER), state=CASE WHEN tries>=? THEN 'failed' ELSE 'uploaded' END, lease=0 WHERE id=? AND state='storing'",
                vec![TursoArg::int(MAX_TRIES), TursoArg::text(&id)]).await;
            list_invalidate(env).await;
            out.push_str(&format!("{id}: xato: {e}\n"));
            drained = true; // xatoda zanjir to'xtaydi (cron keyin oladi)
            break;
        }
        if stopped {
            let _ = sql(env, "UPDATE bd_files SET updated=CAST((julianday('now')-2440587.5)*86400000 AS INTEGER), state='uploaded', lease=0 WHERE id=? AND state='storing'", vec![TursoArg::text(&id)]).await;
            list_invalidate(env).await;
            out.push_str(&format!("{id}: vaqt tugadi, {}/{chunks}\n", list.len()));
            break;
        }
        sql(env, "UPDATE bd_files SET updated=CAST((julianday('now')-2440587.5)*86400000 AS INTEGER), state='ready' WHERE id=? AND state='storing'", vec![TursoArg::text(&id)]).await?;
        list_invalidate(env).await;
        let _ = bk.delete(&key).await;
        out.push_str(&format!("{id}: tayyor\n"));
    }
    Ok((out, progressed, !drained))
}

/// ── TOZALASH: tashlab ketilgan yuklashlar ───────────────────────
///
/// 24 soatdan eski `uploading` (yuklash yarmida uzilgan), `failed` (saqlab bo'lmadi) va `direct` (eski sinov yo'li) yozuvlari:
/// R2'dagi yarim ko'p qismli yuklash bekor qilinadi, R2 nusxasi o'chadi, yozuv bazadan olib tashlanadi. `ready` fayllarga TEGILMAYDI.
pub(crate) async fn cleanup(env: &Env) -> Result<String> {
    ensure(env).await?;
    let Some(bk) = bucket(env) else { return Ok("r2 yo'q".into()) };
    let cutoff = now_ms() - 24 * 3600 * 1000;
    let r = sql(env, "SELECT id,state,upload_id FROM bd_files WHERE state IN ('uploading','failed','direct') AND created<? LIMIT 50",
        vec![TursoArg::int(cutoff)]).await?;
    let mut n = 0;
    for row in rows_of(&r) {
        let id = row["id"].as_str().unwrap_or("").to_string();
        if !id_ok(&id) {
            continue;
        }
        let state = row["state"].as_str().unwrap_or("").to_string();
        let uid = row["upload_id"].as_str().unwrap_or("").to_string();
        if state == "uploading" && !uid.is_empty() {
            if let Ok(m) = bk.resume_multipart_upload(format!("{UP}{id}"), uid) {
                let _ = m.abort().await;
            }
        }
        let _ = bk.delete(format!("{UP}{id}")).await;
        sql(env, "DELETE FROM bd_files WHERE id=? AND state IN ('uploading','failed','direct')", vec![TursoArg::text(&id)]).await?;
        n += 1;
    }
    // Jurnal: 30 kundan eski yozuvlar o'chadi, eng katta o'chirilgan `seq` `floor` ga yoziladi (shundan eski keshlar to'liq qayta o'qiladi).
    let cut = now_ms() - 30 * 24 * 3600 * 1000;
    let mx = sql(env, "SELECT MAX(seq) AS m FROM bd_journal WHERE at < ?", vec![TursoArg::int(cut)]).await?;
    if let Some(m) = first(&mx).and_then(|r| r["m"].as_i64()) {
        sql(env, "INSERT INTO bd_meta (k,v) VALUES ('floor', ?) ON CONFLICT(k) DO UPDATE SET v=MAX(v, excluded.v)", vec![TursoArg::int(m)]).await?;
        sql(env, "DELETE FROM bd_journal WHERE seq <= ? AND at < ?", vec![TursoArg::int(m), TursoArg::int(cut)]).await?;
    }
    Ok(format!("tozalandi: {n}"))
}

/// `YYYYMMDDTHHMMSSZ` (UTC) — S3 imzosi uchun.
fn amz_now() -> String {
    let secs = now_ms() / 1000;
    let (days, rem) = (secs.div_euclid(86400), secs.rem_euclid(86400));
    // fuqarolik sanasi (Howard Hinnant algoritmi)
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
    format!("{y:04}{m:02}{d:02}T{:02}{:02}{:02}Z", rem / 3600, (rem % 3600) / 60, rem % 60)
}

/// Bitta so'rovda (S3 PUT) yuboriladigan eng katta hajm: 5 GiB dan kichikroq (zaxira bilan).
const SINGLE_MAX: i64 = 5 * 1024 * 1024 * 1024 - 16 * 1024 * 1024;
/// 5 GiB dan katta fayl: har qism 1 GiB (S3 qism chegarasi 5 GiB), qismlar kam bo'ladi.
const BIG_PART: i64 = 1024 * 1024 * 1024;
const R2_BUCKET: &str = "arumediatv";
const URL_TTL: u64 = 24 * 3600;

/// `/api/bd/presign` va `/api/bd/complete` uchun R2 S3 kalitlari (worker sirlari).
fn r2_creds(env: &Env) -> Option<(String, String, String)> {
    let (a, k, s) = (tg_secret(env, "R2_ACCOUNT_ID"), tg_secret(env, "R2_ACCESS_KEY_ID"), tg_secret(env, "R2_SECRET_ACCESS_KEY"));
    if a.is_empty() || k.is_empty() || s.is_empty() { None } else { Some((a, k, s)) }
}

/// Delta shundan ko'p qator bo'lsa — to'liq qayta o'qiladi.
const LIST_MAX_DELTA: usize = 500;
const LIST_N: usize = 200;
const LIST_COLS: &str = "id,name,kind,mime,size,w,h,dur,state,created,chunks,updated,CASE WHEN chunk_ids='' THEN 0 ELSE length(chunk_ids)-length(replace(chunk_ids,',',''))+1 END AS stored";

fn listed(state: &str) -> bool {
    matches!(state, "ready" | "uploaded" | "storing")
}

/// To'liq o'qish: eng yangi 200 qator, indeks bo'yicha (indeks yo'q bo'lsa — oddiy so'rov).
async fn list_full(env: &Env) -> Result<Vec<Value>> {
    let q_fast = format!("SELECT {LIST_COLS} FROM bd_files INDEXED BY bd_list WHERE state IN ('ready','uploaded','storing') ORDER BY created DESC LIMIT {LIST_N}");
    let q_plain = format!("SELECT {LIST_COLS} FROM bd_files WHERE state IN ('ready','uploaded','storing') ORDER BY created DESC LIMIT {LIST_N}");
    let res = match sql(env, &q_fast, vec![]).await {
        Ok(r) => r,
        Err(_) => sql(env, &q_plain, vec![]).await?,
    };
    Ok(rows_of(&res))
}

/// Ro'yxat keshi — ikki yozuv (DO'siz): (1) ASOSIY — qatorlar + `seq` (qaysi jurnal raqamigacha yangilangan), 1000 kun;
/// (2) JURNAL BELGISI — bazadagi eng oxirgi `seq` va `floor`, 10 daqiqa. Ro'yxat so'ralganda belgi (10 daqiqada bir marta Turso'dan
/// 1 qator o'qib) yangilanadi; asosiy yozuvning `seq` si eskirgan bo'lsa — jurnaldan FAQAT o'zgargan `id`lar olinib, o'sha qatorlargina
/// Turso'dan o'qiladi va keshdagi eskilari almashtiriladi. O'zgarishni yozgan mintaqa belgini darhol o'chiradi (o'z o'zgarishini kutmasdan ko'radi);
/// boshqa mintaqalar eng ko'pi bilan 10 daqiqada bilib oladi.
const LIST_MAIN_URL: &str = "https://arudrive.cache/list/main";
const LIST_SEQ_URL: &str = "https://arudrive.cache/list/seq";
const LIST_MAIN_TTL: u64 = 1000 * 24 * 3600;
const LIST_SEQ_TTL: u64 = 600;

/// Jurnal belgisini (shu mintaqada) o'chiradi: keyingi ro'yxat so'rovi eng oxirgi `seq` ni Turso'dan qayta o'qiydi.
pub(crate) async fn list_invalidate(_env: &Env) {
    if let Ok(req) = Request::new(LIST_SEQ_URL, Method::Get) {
        let _ = Cache::default().delete(&req, false).await;
    }
}

async fn cache_json(url: &str) -> Option<Value> {
    let req = Request::new(url, Method::Get).ok()?;
    let mut hit = Cache::default().get(&req, false).await.ok()??;
    hit.json::<Value>().await.ok()
}

async fn cache_put_json(url: &str, v: &Value, ttl: u64) {
    if let (Ok(req), Ok(mut r)) = (Request::new(url, Method::Get), Response::ok(v.to_string())) {
        let _ = r.headers_mut().set("Content-Type", "application/json");
        let _ = r.headers_mut().set("Cache-Control", &format!("public, max-age={ttl}"));
        let _ = Cache::default().put(&req, r).await;
    }
}

/// Bazadagi eng oxirgi `seq` va `floor` (tozalangan eng katta `seq`): bitta so'rov, indeks bo'yicha 1–2 qator.
async fn list_head(env: &Env) -> Result<(i64, i64)> {
    let r = sql(env, "SELECT COALESCE(MAX(seq),0) AS latest, COALESCE((SELECT v FROM bd_meta WHERE k='floor'),0) AS floor FROM bd_journal", vec![]).await?;
    let row = first(&r).unwrap_or(json!({}));
    Ok((row["latest"].as_i64().unwrap_or(0), row["floor"].as_i64().unwrap_or(0)))
}

/// To'liq o'qiladi va asosiy yozuvga `seq` bilan saqlaydi. `seq` — o'qishdan OLDIN bilingan `latest` (eskiroq bo'lishi mumkin, lekin
/// xavfsiz: keyingi delta ba'zi o'zgarishlarni qayta qo'llaydi, hech narsa o'tib ketmaydi).
async fn list_full_cached(env: &Env, latest: i64) -> Result<Vec<Value>> {
    let rows = list_full(env).await?;
    cache_put_json(LIST_MAIN_URL, &json!({"files": rows, "seq": latest}), LIST_MAIN_TTL).await;
    Ok(rows)
}

/// Jurnaldan `seq` dan keyin o'zgargan (seq, id) lar (eng ko'pi bilan `LIST_MAX_DELTA + 1` ta — oshsa to'liq o'qiladi).
async fn list_journal(env: &Env, after: i64) -> Result<Vec<(i64, String)>> {
    let q = format!("SELECT seq,id FROM bd_journal WHERE seq > ? ORDER BY seq LIMIT {}", LIST_MAX_DELTA + 1);
    let r = sql(env, &q, vec![TursoArg::int(after)]).await?;
    Ok(rows_of(&r).iter().filter_map(|x| Some((x["seq"].as_i64()?, x["id"].as_str()?.to_string()))).collect())
}

/// Berilgan `id`li qatorlarni o'qiydi (faqat shular, indeks bo'yicha).
async fn list_rows_by_ids(env: &Env, ids: &[String]) -> Result<Vec<Value>> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let marks = vec!["?"; ids.len()].join(",");
    let q = format!("SELECT {LIST_COLS} FROM bd_files WHERE id IN ({marks})");
    let args = ids.iter().map(|i| TursoArg::text(i)).collect();
    Ok(rows_of(&sql(env, &q, args).await?))
}

/// Delta'ni keshdagi qatorlarga qo'llaydi: o'zgargan `id`lar keshdan olib tashlanadi, bazadan qaytganlari (ro'yxatda bo'lsa) qo'yiladi
/// (o'chirilgan qator qaytmaydi — shu bilan ketadi); tartib va 200 chegarasi saqlanadi.
fn list_apply(rows: &mut Vec<Value>, changed: &[String], fetched: Vec<Value>) {
    rows.retain(|r| !changed.iter().any(|c| r["id"].as_str() == Some(c.as_str())));
    for d in fetched {
        if listed(d["state"].as_str().unwrap_or("")) {
            rows.push(d);
        }
    }
    rows.sort_by(|a, b| b["created"].as_i64().cmp(&a["created"].as_i64()));
    rows.truncate(LIST_N);
}

/// `a,b,c` -> ["a","b","c"] (bo'sh matn — bo'sh ro'yxat).
pub(crate) fn split_ids(s: &str) -> Vec<&str> {
    if s.is_empty() { Vec::new() } else { s.split(',').collect() }
}

fn first(res: &Value) -> Option<Value> {
    first_row(res)
}

fn qs(url: &Url, k: &str) -> String {
    url.query_pairs().find(|(a, _)| a == k).map(|(_, v)| v.to_string()).unwrap_or_default()
}

/// Fayl identifikatori: faqat harf/raqam (R2 kaliti va keshdagi nom shunga tayanadi).
fn id_ok(id: &str) -> bool {
    !id.is_empty() && id.len() <= 32 && id.bytes().all(|b| b.is_ascii_alphanumeric())
}

fn new_id() -> String {
    let r = (js_sys::Math::random() * 1e15) as u64;
    format!("{:x}{:x}", now_ms() as u64, r)
}

/// Fayl nomidagi xavfli belgilar olib tashlanadi (Telegram'ga va ro'yxatga ketadi).
fn clean_name(s: &str) -> String {
    let t: String = s.chars().filter(|c| !c.is_control() && *c != '/' && *c != '\\').take(120).collect();
    let t = t.trim().to_string();
    if t.is_empty() { "fayl".into() } else { t }
}

pub(crate) fn chunks_of(size: i64) -> i64 {
    (size.max(0) as u64).div_ceil(CHUNK) as i64
}

// ── Asosiy yo'l ──────────────────────────────────────────────

pub(crate) async fn route(mut req: Request, env: &Env, ctx: &Context, path: &str) -> Result<Response> {
    let method = req.method();
    let actions = matches!(path, "/api/bd/claim" | "/api/bd/read" | "/api/bd/chunkdone" | "/api/bd/done" | "/api/bd/fail");
    if actions && !encode_token_ok(&req, env) {
        return json_resp(&json!({"error": "forbidden"}), 403);
    }
    if let Err(e) = ensure(env).await {
        return json_resp(&json!({"error": e.to_string()}), 503);
    }
    let url = req.url()?;
    match (method, path) {
        // ── Ilova: yuklash ──
        (Method::Post, "/api/bd/init") => {
            let b: Value = req.json().await.unwrap_or(json!({}));
            let name = clean_name(b["name"].as_str().unwrap_or(""));
            let size = jint(&b, "size");
            if size <= 0 || size > MAX_SIZE {
                return json_resp(&json!({"error": "hajm"}), 400);
            }
            let kind = if b["kind"].as_str() == Some("video") { "video" } else { "file" };
            let mime: String = b["mime"].as_str().unwrap_or("").chars().filter(|c| c.is_ascii_graphic()).take(60).collect();
            let Some(bk) = bucket(env) else { return json_resp(&json!({"error": "r2"}), 503) };
            let id = new_id();
            let up = bk.create_multipart_upload(format!("{UP}{id}")).execute().await?;
            let upload_id = up.upload_id().await;
            sql(env, "INSERT INTO bd_files (id,name,kind,mime,size,w,h,dur,state,upload_id,chunks,created,updated) VALUES (?,?,?,?,?,?,?,?,'uploading',?,?,?,CAST((julianday('now')-2440587.5)*86400000 AS INTEGER))",
                vec![TursoArg::text(&id), TursoArg::text(&name), TursoArg::text(kind), TursoArg::text(&mime), TursoArg::int(size),
                     TursoArg::int(jint(&b, "w")), TursoArg::int(jint(&b, "h")), TursoArg::int(jint(&b, "dur")),
                     TursoArg::text(&upload_id), TursoArg::int(chunks_of(size)), TursoArg::int(now_ms())]).await?;
            ok(json!({"id": id, "upload_id": upload_id, "part_size": PART}))
        }
        (Method::Put, "/api/bd/part") => {
            let (id, uid, n) = (qs(&url, "id"), qs(&url, "uid"), qs(&url, "n").parse::<u16>().unwrap_or(0));
            let Some(bk) = bucket(env) else { return json_resp(&json!({"error": "r2"}), 503) };
            if !id_ok(&id) || uid.is_empty() || n == 0 {
                return json_resp(&json!({"error": "so'rov"}), 400);
            }
            let len = req.headers().get("Content-Length").ok().flatten().and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
            if len == 0 || len > PART_MAX {
                return json_resp(&json!({"error": "hajm"}), 400);
            }
            // OQIM: tana xotiraga yig'ilmaydi, R2'ga to'g'ridan-to'g'ri uzatiladi.
            let stream = FixedLengthStream::wrap(req.stream()?, len);
            let part = bk.resume_multipart_upload(format!("{UP}{id}"), uid)?.upload_part(n, stream).await?;
            ok(json!({"n": part.part_number(), "etag": part.etag()}))
        }
        (Method::Post, "/api/bd/finish") => {
            let b: Value = req.json().await.unwrap_or(json!({}));
            let id = b["id"].as_str().unwrap_or("").to_string();
            let uid = b["upload_id"].as_str().unwrap_or("").to_string();
            let Some(bk) = bucket(env) else { return json_resp(&json!({"error": "r2"}), 503) };
            if !id_ok(&id) || uid.is_empty() {
                return json_resp(&json!({"error": "so'rov"}), 400);
            }
            let Some(row) = first(&sql(env, "SELECT size,state FROM bd_files WHERE id=?", vec![TursoArg::text(&id)]).await?) else {
                return err404("fayl yo'q");
            };
            if row["state"].as_str() != Some("uploading") {
                return json_resp(&json!({"error": "holat"}), 409);
            }
            let size = row["size"].as_i64().unwrap_or(0);
            let parts: Vec<UploadedPart> = b["parts"].as_array().map(|a| a.iter().filter_map(|p| {
                Some(UploadedPart::new(p["n"].as_u64()? as u16, p["etag"].as_str()?.to_string()))
            }).collect()).unwrap_or_default();
            if parts.is_empty() {
                return json_resp(&json!({"error": "bo'laklar"}), 400);
            }
            let key = format!("{UP}{id}");
            bk.resume_multipart_upload(key.clone(), uid)?.complete(parts).await.map_err(|e| Error::RustError(format!("yig'ish: {e}")))?;
            let got = bk.head(&key).await.ok().flatten().map(|o| o.size() as i64);
            if got != Some(size) {
                let _ = bk.delete(&key).await;
                let _ = sql(env, "UPDATE bd_files SET updated=CAST((julianday('now')-2440587.5)*86400000 AS INTEGER), state='failed' WHERE id=?", vec![TursoArg::text(&id)]).await;
                return json_resp(&json!({"error": format!("hajm mos emas ({got:?} != {size})")}), 422);
            }
            sql(env, "UPDATE bd_files SET updated=CAST((julianday('now')-2440587.5)*86400000 AS INTEGER), state='uploaded' WHERE id=? AND state='uploading'", vec![TursoArg::text(&id)]).await?;
            list_invalidate(env).await;
            // Telegramga saqlashni WORKER O'ZI boshlaydi (javobdan keyin fonda, ~30 soniya); davomini har daqiqadagi cron oladi.
            let e2 = env.clone();
            ctx.wait_until(async move {
                process_and_chain(&e2).await;
            });
            ok(json!({"ok": true}))
        }
        // ── Ilova -> R2 TO'G'RIDAN-TO'G'RI: worker faqat vaqtinchalik imzolangan havola yasaydi (baytlar worker'dan o'tmaydi) ──
        (Method::Post, "/api/bd/presign") => {
            let Some((acct, ak, sk)) = r2_creds(env) else {
                return json_resp(&json!({"error": "r2 kalitlari yo'q"}), 501);
            };
            let b: Value = req.json().await.unwrap_or(json!({}));
            let name = clean_name(b["name"].as_str().unwrap_or(""));
            let size = jint(&b, "size");
            if size <= 0 || size > MAX_SIZE {
                return json_resp(&json!({"error": "hajm"}), 400);
            }
            let kind = if b["kind"].as_str() == Some("video") { "video" } else { "file" };
            let mime: String = b["mime"].as_str().unwrap_or("").chars().filter(|c| c.is_ascii_graphic()).take(60).collect();
            let Some(bk) = bucket(env) else { return json_resp(&json!({"error": "r2"}), 503) };
            let id = new_id();
            let key = format!("{UP}{id}");
            let host = format!("{acct}.r2.cloudflarestorage.com");
            let path = format!("/{R2_BUCKET}/{key}");
            let now = amz_now();
            let (mode, upload_id, part_size, urls): (&str, String, i64, Vec<String>) = if size <= SINGLE_MAX {
                ("single", String::new(), size, vec![crate::s3sign::presign("PUT", &host, &path, &[], &ak, &sk, "auto", &now, URL_TTL)])
            } else {
                let up = bk.create_multipart_upload(key.clone()).execute().await?;
                let uid = up.upload_id().await;
                let n = (size + BIG_PART - 1) / BIG_PART;
                let urls = (1..=n).map(|i| crate::s3sign::presign("PUT", &host, &path, &[("partNumber", &i.to_string()), ("uploadId", &uid)], &ak, &sk, "auto", &now, URL_TTL)).collect();
                ("multi", uid, BIG_PART, urls)
            };
            sql(env, "INSERT INTO bd_files (id,name,kind,mime,size,w,h,dur,state,upload_id,chunks,created,updated) VALUES (?,?,?,?,?,?,?,?,'uploading',?,?,?,CAST((julianday('now')-2440587.5)*86400000 AS INTEGER))",
                vec![TursoArg::text(&id), TursoArg::text(&name), TursoArg::text(kind), TursoArg::text(&mime), TursoArg::int(size),
                     TursoArg::int(jint(&b, "w")), TursoArg::int(jint(&b, "h")), TursoArg::int(jint(&b, "dur")),
                     TursoArg::text(&upload_id), TursoArg::int(chunks_of(size)), TursoArg::int(now_ms())]).await?;
            ok(json!({"id": id, "mode": mode, "upload_id": upload_id, "part_size": part_size, "urls": urls}))
        }
        // Bitta so'rovda yuklangan fayl: R2 dagi hajm tekshiriladi va saqlash navbatiga qo'yiladi (ko'p qismli yuklash uchun `finish`).
        (Method::Post, "/api/bd/complete") => {
            let b: Value = req.json().await.unwrap_or(json!({}));
            let id = b["id"].as_str().unwrap_or("").to_string();
            let Some(bk) = bucket(env) else { return json_resp(&json!({"error": "r2"}), 503) };
            if !id_ok(&id) {
                return json_resp(&json!({"error": "so'rov"}), 400);
            }
            let Some(row) = first(&sql(env, "SELECT size,state FROM bd_files WHERE id=?", vec![TursoArg::text(&id)]).await?) else {
                return err404("fayl yo'q");
            };
            if row["state"].as_str() != Some("uploading") {
                return json_resp(&json!({"error": "holat"}), 409);
            }
            let size = row["size"].as_i64().unwrap_or(0);
            let got = bk.head(&format!("{UP}{id}")).await.ok().flatten().map(|o| o.size() as i64);
            if got != Some(size) {
                let _ = sql(env, "UPDATE bd_files SET updated=CAST((julianday('now')-2440587.5)*86400000 AS INTEGER), state='failed' WHERE id=?", vec![TursoArg::text(&id)]).await;
                return json_resp(&json!({"error": format!("hajm mos emas ({got:?} != {size})")}), 422);
            }
            sql(env, "UPDATE bd_files SET updated=CAST((julianday('now')-2440587.5)*86400000 AS INTEGER), state='uploaded' WHERE id=? AND state='uploading'", vec![TursoArg::text(&id)]).await?;
            list_invalidate(env).await;
            let e2 = env.clone();
            ctx.wait_until(async move {
                process_and_chain(&e2).await;
            });
            ok(json!({"ok": true}))
        }
        // Kichik rasm Telegram'ga (kanalga) saqlanadi; file_id `thumb_id` ustunida (R2 faqat asl fayl uchun).
        (Method::Post, "/api/bd/thumb") => {
            let id = qs(&url, "id");
            if !id_ok(&id) {
                return json_resp(&json!({"error": "so'rov"}), 400);
            }
            let bytes = req.bytes().await?;
            if bytes.is_empty() || bytes.len() > 1024 * 1024 {
                return json_resp(&json!({"error": "hajm"}), 400);
            }
            let (fid, _) = tg_send_document(&tg_secret(env, "BD_BOT_TOKEN"), tg_channel_id(env), &format!("{id}.thumb.jpg"), &bytes, bytes.len() as u64).await?;
            sql(env, "UPDATE bd_files SET updated=CAST((julianday('now')-2440587.5)*86400000 AS INTEGER), thumb_id=? WHERE id=?", vec![TursoArg::text(&fid), TursoArg::text(&id)]).await?;
            ok(json!({"ok": true}))
        }
        // Qo'lda: R2 -> Telegram saqlashni HOZIR ishga tushiradi va natijani (yoki xatoni) qaytaradi (tashxis).
        (Method::Post, "/api/bd/process") if qs(&url, "bg") == "1" => {
            // Zanjir bo'g'ini: ish fonda, javob darhol (chaqiruvchi kutib qolmaydi).
            let e2 = env.clone();
            ctx.wait_until(async move {
                process_and_chain(&e2).await;
            });
            json_resp(&json!({"started": true}), 202)
        }
        // Zanjir ishlashini tekshirish: worker o'z manziliga so'rov yubora oladimi.
        (Method::Get, "/api/bd/chain-test") => {
            let svc = match env.service("SELF") {
                Ok(s) => s,
                Err(e) => return json_resp(&json!({"ok": false, "error": format!("SELF ulanishi yo'q: {e}")}), 200),
            };
            let st = match svc.fetch_request(Request::new("https://self/health", Method::Get)?).await {
                Ok(r) => r.status_code() as i64,
                Err(e) => return json_resp(&json!({"ok": false, "error": e.to_string()}), 200),
            };
            ok(json!({"ok": st == 200, "status": st}))
        }
        (Method::Post, "/api/bd/process") => match process(env, 22_000).await {
            Ok(log) => ok(json!({"ok": true, "log": log})),
            Err(e) => json_resp(&json!({"ok": false, "error": e.to_string()}), 500),
        },
        // Qo'lda: bazani qo'lda o'zgartirgandan keyin ro'yxat versiyasini oshiradi (kesh keyingi so'rovda faqat o'zgargan qatorlarni oladi).
        (Method::Post, "/api/bd/bump") => {
            list_invalidate(env).await;
            ok(json!({"ok": true}))
        }
        // Qo'lda: tashlab ketilgan (24 soatdan eski) yuklashlarni tozalash.
        (Method::Post, "/api/bd/cleanup") => match cleanup(env).await {
            Ok(log) => ok(json!({"ok": true, "log": log})),
            Err(e) => json_resp(&json!({"ok": false, "error": e.to_string()}), 500),
        },
        // Qo'lda: ikkinchi akkauntdagi Actions'ni ishga tushirish (zaxira yo'l).
        (Method::Post, "/api/bd/kick") => ok(json!({"kick": kick(env).await})),
        (Method::Get, "/api/bd/list") => {
            // DO'SIZ JURNALLI KESH (foydalanuvchi rejasi): asosiy yozuv (qatorlar + `seq`, 1000 kun) va jurnal belgisi (`latest`, 10 daqiqa).
            //   * belgi eskirgan/yo'q  -> Turso'dan eng oxirgi `seq` (1 qator) o'qiladi;
            //   * asosiy `seq` >= latest -> keshdan (HIT-LIST);
            //   * asosiy `seq` eskirgan  -> jurnaldan o'zgargan `id`lar, so'ng FAQAT shu qatorlar o'qilib almashtiriladi (DELTA-LIST);
            //   * asosiy yo'q / `seq` jurnal tozalangan chegaradan eski / o'zgarish 500 dan ko'p / `?full=1` -> to'liq (MISS/FULL-LIST).
            // `?fresh=1` — jurnal belgisini chetlab o'tadi (darhol tekshiradi).
            if qs(&url, "fresh") == "1" {
                list_invalidate(env).await;
            }
            let (latest, floor) = match cache_json(LIST_SEQ_URL).await {
                Some(j) if j["latest"].is_i64() => (j["latest"].as_i64().unwrap_or(0), j["floor"].as_i64().unwrap_or(0)),
                _ => {
                    let (l, f) = list_head(env).await?;
                    cache_put_json(LIST_SEQ_URL, &json!({"latest": l, "floor": f}), LIST_SEQ_TTL).await;
                    (l, f)
                }
            };
            let main = if qs(&url, "full") == "1" { None } else { cache_json(LIST_MAIN_URL).await };
            let cached = main.and_then(|j| {
                let rows = j["files"].as_array()?.clone();
                Some((rows, j["seq"].as_i64()?))
            });
            let (rows, how) = match cached {
                Some((rows, mseq)) if mseq >= latest => (rows, "HIT-LIST"),
                Some((mut rows, mseq)) if mseq >= floor => {
                    let j = list_journal(env, mseq).await?;
                    if j.len() > LIST_MAX_DELTA {
                        let r = list_full_cached(env, latest).await?;
                        (r, "FULL-LIST")
                    } else {
                        let ids: Vec<String> = j.iter().map(|(_, id)| id.clone()).collect();
                        let new_seq = j.iter().map(|(s, _)| *s).max().unwrap_or(mseq).max(latest);
                        let fetched = list_rows_by_ids(env, &ids).await?;
                        list_apply(&mut rows, &ids, fetched);
                        cache_put_json(LIST_MAIN_URL, &json!({"files": rows, "seq": new_seq}), LIST_MAIN_TTL).await;
                        (rows, "DELTA-LIST")
                    }
                }
                _ => {
                    let r = list_full_cached(env, latest).await?;
                    (r, "MISS-LIST")
                }
            };
            let mut r = Response::ok(json!({"files": rows}).to_string())?;
            set_cors(&mut r);
            let _ = r.headers_mut().set("Content-Type", "application/json");
            let _ = r.headers_mut().set("Cache-Control", "no-store");
            let _ = r.headers_mut().set("X-Cache", how);
            Ok(r)
        }
        // Kichik rasm: Telegram'dan olinib Cloudflare keshiga (1000 kun) saqlanadi.
        (Method::Get, p) if p.starts_with("/api/bd/thumb/") => {
            let id = p["/api/bd/thumb/".len()..].trim_end_matches(".jpg").to_string();
            if !id_ok(&id) {
                return err404("yo'q");
            }
            let cache = Cache::default();
            let key = Request::new(&format!("https://arudrive.cache/thumb/{id}"), Method::Get)?;
            if let Some(mut hit) = cache.get(&key, false).await? {
                let bytes = hit.bytes().await?;
                return thumb_resp(bytes);
            }
            let Some(row) = first(&sql(env, "SELECT thumb_id FROM bd_files WHERE id=?", vec![TursoArg::text(&id)]).await?) else { return err404("yo'q") };
            let fid = row["thumb_id"].as_str().unwrap_or("").to_string();
            if fid.is_empty() {
                return err404("rasm yo'q");
            }
            let token = tg_secret(env, "BD_BOT_TOKEN");
            let f = tg_api_tok(&token, "getFile", json!({"file_id": fid})).await?;
            let path = f["file_path"].as_str().unwrap_or("").to_string();
            let mut r = Fetch::Request(Request::new(&format!("https://api.telegram.org/file/bot{token}/{path}"), Method::Get)?).send().await?;
            if r.status_code() != 200 {
                return err404("rasm yo'q");
            }
            let bytes = r.bytes().await?;
            let _ = cache.put(&key, thumb_resp(bytes.clone())?).await;
            thumb_resp(bytes)
        }

        // ── Actions (ENCODE_TOKEN) ──
        (Method::Post, "/api/bd/claim") => {
            let now = now_ms();
            let r = sql_many(env, &[
                // Ijarasi tugagan (run o'lgan) ishlar qayta navbatga qaytadi.
                ("UPDATE bd_files SET updated=CAST((julianday('now')-2440587.5)*86400000 AS INTEGER), state='uploaded' WHERE state='storing' AND lease<? AND tries<?", vec![TursoArg::int(now), TursoArg::int(MAX_TRIES)]),
                ("UPDATE bd_files SET updated=CAST((julianday('now')-2440587.5)*86400000 AS INTEGER), state='failed' WHERE state='storing' AND lease<? AND tries>=?", vec![TursoArg::int(now), TursoArg::int(MAX_TRIES)]),
                ("SELECT id,name,size,chunks FROM bd_files WHERE state='uploaded' ORDER BY created LIMIT 1", vec![]),
            ]).await?;
            let Some(f) = first(&r[2]) else { return ok(json!({"none": true})) };
            let id = f["id"].as_str().unwrap_or("").to_string();
            let got = sql(env, "UPDATE bd_files SET updated=CAST((julianday('now')-2440587.5)*86400000 AS INTEGER), state='storing', lease=?, tries=tries+1 WHERE id=? AND state='uploaded' RETURNING id",
                vec![TursoArg::int(now + LEASE_MS), TursoArg::text(&id)]).await?;
            if first(&got).is_none() {
                return ok(json!({"none": true}));
            }
            let ids = sql(env, "SELECT chunk_ids FROM bd_files WHERE id=?", vec![TursoArg::text(&id)]).await?;
            let done: Vec<i64> = (0..split_ids(first(&ids).and_then(|r| r["chunk_ids"].as_str().map(String::from)).unwrap_or_default().as_str()).len() as i64).collect();
            ok(json!({
                "id": id, "name": f["name"], "size": f["size"], "chunks": f["chunks"], "chunk": CHUNK, "done": done,
                "bot_token": tg_secret(env, "BD_BOT_TOKEN"), "channel": tg_channel_id(env),
            }))
        }
        (Method::Get, "/api/bd/read") => {
            let (id, n) = (qs(&url, "id"), qs(&url, "n").parse::<u64>().unwrap_or(0));
            let Some(bk) = bucket(env) else { return json_resp(&json!({"error": "r2"}), 503) };
            if !id_ok(&id) {
                return err404("yo'q");
            }
            let key = format!("{UP}{id}");
            let Some(head) = bk.head(&key).await.ok().flatten() else { return err404("yo'q") };
            let total = head.size();
            let offset = n * CHUNK;
            if offset >= total {
                return json_resp(&json!({"error": "oraliq"}), 416);
            }
            let length = CHUNK.min(total - offset);
            let Some(obj) = bk.get(&key).range(Range::OffsetWithLength { offset, length }).execute().await? else { return err404("yo'q") };
            let Some(body) = obj.body() else { return err404("tana") };
            let resp = Response::from_body(body.response_body()?)?;
            let h = Headers::new();
            h.set("Content-Length", &length.to_string())?;
            h.set("Content-Type", "application/octet-stream")?;
            h.set("Cache-Control", "no-store")?;
            Ok(resp.with_headers(h))
        }
        (Method::Post, "/api/bd/chunkdone") => {
            let b: Value = req.json().await.unwrap_or(json!({}));
            let id = b["id"].as_str().unwrap_or("").to_string();
            let tg = b["tg_file_id"].as_str().unwrap_or("").to_string();
            if !id_ok(&id) || tg.is_empty() || tg.len() > 300 {
                return json_resp(&json!({"error": "so'rov"}), 400);
            }
            let n = jint(&b, "n");
            if n < 0 || !tg.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
                return json_resp(&json!({"error": "so'rov"}), 400);
            }
            let cur = sql(env, "SELECT chunk_ids FROM bd_files WHERE id=?", vec![TursoArg::text(&id)]).await?;
            let Some(row) = first(&cur) else { return err404("fayl yo'q") };
            let mut list: Vec<String> = split_ids(row["chunk_ids"].as_str().unwrap_or("")).into_iter().map(String::from).collect();
            let n = n as usize;
            if n > list.len() {
                return json_resp(&json!({"error": "bo'laklar tartib bilan kelishi shart"}), 409);
            }
            if n == list.len() { list.push(tg) } else { list[n] = tg }
            sql(env, "UPDATE bd_files SET updated=CAST((julianday('now')-2440587.5)*86400000 AS INTEGER), chunk_ids=?, lease=? WHERE id=? AND state='storing'",
                vec![TursoArg::text(&list.join(",")), TursoArg::int(now_ms() + LEASE_MS), TursoArg::text(&id)]).await?;
            ok(json!({"ok": true}))
        }
        (Method::Post, "/api/bd/done") => {
            let b: Value = req.json().await.unwrap_or(json!({}));
            let id = b["id"].as_str().unwrap_or("").to_string();
            if !id_ok(&id) {
                return json_resp(&json!({"error": "so'rov"}), 400);
            }
            let r = sql(env, "SELECT chunks,chunk_ids FROM bd_files WHERE id=?", vec![TursoArg::text(&id)]).await?;
            let row = first(&r);
            let want = row.as_ref().and_then(|x| x["chunks"].as_i64()).unwrap_or(-1);
            let have = row.as_ref().map(|x| split_ids(x["chunk_ids"].as_str().unwrap_or("")).len() as i64).unwrap_or(0);
            if want <= 0 || have != want {
                return json_resp(&json!({"error": format!("bo'laklar yetarli emas ({have}/{want})")}), 409);
            }
            sql(env, "UPDATE bd_files SET updated=CAST((julianday('now')-2440587.5)*86400000 AS INTEGER), state='ready' WHERE id=? AND state='storing'", vec![TursoArg::text(&id)]).await?;
            // Asl fayl R2'dan o'chadi (bo'laklar Telegram'da).
            if let Some(bk) = bucket(env) {
                let _ = bk.delete(format!("{UP}{id}")).await;
            }
            ok(json!({"ok": true}))
        }
        (Method::Post, "/api/bd/fail") => {
            let b: Value = req.json().await.unwrap_or(json!({}));
            let id = b["id"].as_str().unwrap_or("").to_string();
            if !id_ok(&id) {
                return json_resp(&json!({"error": "so'rov"}), 400);
            }
            sql(env, "UPDATE bd_files SET updated=CAST((julianday('now')-2440587.5)*86400000 AS INTEGER), state=CASE WHEN tries>=? THEN 'failed' ELSE 'uploaded' END, lease=0 WHERE id=? AND state='storing'",
                vec![TursoArg::int(MAX_TRIES), TursoArg::text(&id)]).await?;
            ok(json!({"ok": true}))
        }
        _ => err404("topilmadi"),
    }
}

/// Yuklangan fayl tayyor bo'lgach (yoki cron) ikkinchi akkauntdagi `bdrive.yml` ni ishga tushiradi.
pub(crate) async fn kick(env: &Env) -> String {
    kick_workflow(env, WORKFLOW).await
}

/// Ikkinchi akkauntdagi repoda `wf` workflow'ini ishga tushiradi (u allaqachon ishlayotgan/navbatda bo'lsa — tegmaydi).
pub(crate) async fn kick_workflow(env: &Env, wf: &str) -> String {
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

/// Telegram webhook sirli kaliti (ENCODE_TOKEN dan hosil qilinadi, alohida sir kerak emas): `setWebhook` da `secret_token` shu.
pub(crate) fn tg_hook_secret(env: &Env) -> String {
    use sha2::{Digest, Sha256};
    let d = Sha256::digest(format!("arudrive-tghook:{}", tg_secret(env, "ENCODE_TOKEN")).as_bytes());
    hex_of(&d)[..32].to_string()
}

/// `POST /api/tg/hook` — `@arudrivebot` ga xabar kelganda Telegram shu yerga yuboradi; worker qabul botining Actions'ini
/// (`tgbot.yml`) ishga tushiradi (ishlab turgan bo'lsa tegmaydi). Xabarning o'zi bu yerda saqlanmaydi: bot uni sessiya orqali
/// chat tarixidan oladi, shu sabab webhook faqat "uyg'otuvchi".
pub(crate) async fn tg_hook(req: Request, env: &Env) -> Result<Response> {
    let got = req.headers().get("X-Telegram-Bot-Api-Secret-Token").ok().flatten().unwrap_or_default();
    if tg_secret(env, "ENCODE_TOKEN").len() < 16 || !ct_eq(got.trim(), &tg_hook_secret(env)) {
        return json_resp(&json!({"error": "forbidden"}), 403);
    }
    let msg = kick_workflow(env, "tgbot.yml").await;
    ok(json!({"ok": true, "kick": msg}))
}

// ── Ko'rish: Telegram -> Cloudflare keshi (R2'siz) ─────────────
//
// `/api/warm/bd~<id>?w=N` N-oynani (26 bo'lak ≈ 468 MiB) keshga isitadi: bo'laklar Telegram'dan (Bot API `getFile`) to'g'ridan-to'g'ri
// BITTA kesh yozuviga (`warm_window_url("bd~<id>", N)`) oqim bilan yoziladi — R2 orqali O'TMAYDI. Javob keshga HAQIQATAN tushgandan keyingina qaytadi. `/api/play/bd~<id>` (`b2_play`) shu yozuvdan Range bilan
// kesib beradi; kesh tayyor bo'lmasa 503 — ilova avval `warm` ni chaqirib kutadi.

/// Bo'lakni Telegram'dan TO'G'RIDAN-TO'G'RI oqim bilan beradi (R2 orqali o'tmaydi, xotiraga yig'ilmaydi). Oyna chegarasi
/// bo'laklarga to'g'ri keladi (`WARM_WINDOW` = 26 bo'lak), shu sabab bo'lakning bir qismini kesish shart emas.
async fn chunk_stream(env: &Env, tg_file_id: &str, expect: u64) -> Result<web_sys::ReadableStream> {
    let token = tg_secret(env, "BD_BOT_TOKEN");
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

pub(crate) async fn warm(env: &Env, id: &str, widx: u64, force: bool) -> Result<Response> {
    let name = format!("bd~{id}");
    let reply = |status: &str, total: u64| -> Result<Response> {
        let status: String = status.chars().filter(|c| c.is_ascii_alphanumeric() || " :-_.,()".contains(*c)).collect();
        let mut r = Response::ok(format!("{{\"status\":\"{status}\",\"total\":{total}}}"))?;
        set_cors(&mut r);
        r.headers_mut().set("Content-Type", "application/json")?;
        r.headers_mut().set("Cache-Control", "no-store")?;
        Ok(r)
    };
    if !id_ok(id) {
        return reply("error nom", 0);
    }
    if !force {
        if let Some(t) = warm_window_total(&name, widx).await {
            if t > 0 {
                return reply("cached", t);
            }
        }
    }
    ensure(env).await?;
    // BIRINCHI BO'LAK SAQLANISHI BILAN video ochiladi: fayl hali saqlanayotgan bo'lsa ham (state='storing'), kerakli oyna
    // (= bitta bo'lak) allaqachon saqlangan bo'lsa isitiladi; aks holda "pending" (xato emas) — ilova kutib qayta so'raydi.
    let r = sql(env, "SELECT size,mime,chunk_ids,state FROM bd_files WHERE id=? AND state IN ('ready','storing','uploaded')", vec![TursoArg::text(id)]).await?;
    let Some(f) = first(&r) else { return reply("error fayl topilmadi", 0) };
    let total = f["size"].as_i64().unwrap_or(0).max(0) as u64;
    let mime = f["mime"].as_str().filter(|m| !m.is_empty()).unwrap_or("application/octet-stream").to_string();
    let chunks: Vec<(i64, String)> = split_ids(f["chunk_ids"].as_str().unwrap_or("")).into_iter().enumerate().map(|(i, s)| (i as i64, s.to_string())).collect();
    if total == 0 || widx * WARM_WINDOW >= total {
        return reply("error oyna", total);
    }
    let ready = f["state"].as_str() == Some("ready");
    if ready && chunks.len() as i64 != chunks_of(total as i64) {
        return reply("error bo'laklar to'liq emas", total);
    }
    if !ready && (chunks.len() as u64) <= widx {
        return reply(&format!("pending saqlanmoqda {}/{}", chunks.len(), chunks_of(total as i64)), total);
    }
    // Boshqa birov shu oynani isitayotgan bo'lsa — kutamiz (Telegram'ga ortiqcha so'rov ketmasin).
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
    if win_start >= total {
        return reply("error oyna", total);
    }
    let win_end = (win_start + WARM_WINDOW).min(total) - 1; // oxirgi bayt (kiritilgan)
    let win_len = win_end - win_start + 1;
    let (readable, writable) = fixed_length_pipe(win_len)?;

    // Bo'laklar Telegram'dan ketma-ket (R2'siz) quvurga yoziladi (fon vazifasi, so'rov ochiq turganda ishlaydi); kesh yozuvi
    // shu quvurdan o'qiydi. Har bir bo'lak OLDINDAN e'lon qilingan hajmda bo'lishi shart (oxirgisi qisqa bo'lishi mumkin).
    let env2 = env.clone();
    let first_n = win_start / CHUNK;
    let last_n = win_end / CHUNK;
    // TEZLIK: joriy bo'lak quvurga oqayotgan paytda KEYINGI bo'lak ulanishi (`getFile` + Telegram ulanishi) ALLAQACHON
    // parallel ochiladi (`join`) — bo'laklar orasidagi kutish yo'qoladi. Xotira o'smaydi: keyingi bo'lakning tanasi
    // quvurga o'tgunga qadar o'qilmaydi (oqim to'xtab turadi).
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
                pending = nr.ok(); // xato bo'lsa keyingi aylanishda qayta uriniladi
            } else if pipe_fut.await.is_err() {
                abort_pipe(&writable);
                return;
            }
        }
        close_pipe(&writable);
    });

    let headers = Headers::new();
    headers.set("Content-Type", &mime)?;
    headers.set("Accept-Ranges", "bytes")?;
    headers.set("Cache-Control", &format!("public, max-age={CHUNK_CACHE_SECONDS}"))?;
    headers.set("X-Total-Size", &total.to_string())?;
    headers.set("X-Window-Start", &win_start.to_string())?;
    let to_cache = Response::from_body(ResponseBody::Stream(readable))?.with_headers(headers).with_status(200);
    let key = Request::new(&warm_window_url(&name, widx), Method::Get)?;
    if cache.put(&key, to_cache).await.is_err() {
        return reply("error keshga yozilmadi", total);
    }
    // Yozuv HAQIQATAN o'qiladigan bo'lganini tekshiramiz (ARUMEDIATV'dagi kabi).
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
    fn vergulli_royxat() {
        assert!(split_ids("").is_empty());
        assert_eq!(split_ids("a_1,b-2"), vec!["a_1", "b-2"]);
    }

    fn row(id: &str, state: &str, created: i64, updated: i64) -> Value {
        json!({"id": id, "state": state, "created": created, "updated": updated, "stored": 0})
    }

    #[test]
    fn delta_qatorni_almashtiradi() {
        let mut rows = vec![row("b", "storing", 20, 20), row("a", "ready", 10, 10)];
        list_apply(&mut rows, &["b".to_string()], vec![row("b", "ready", 20, 99)]);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["id"], "b");
        assert_eq!(rows[0]["state"], "ready");
    }

    #[test]
    fn delta_royxatdan_chiqqan_va_ochirilganni_olib_tashlaydi() {
        let mut rows = vec![row("b", "storing", 20, 20), row("a", "ready", 10, 10), row("c", "ready", 5, 5)];
        // b failed bo'ldi (ro'yxatda ko'rinmaydi); c o'chirildi (bazadan qaytmadi)
        list_apply(&mut rows, &["b".to_string(), "c".to_string()], vec![row("b", "failed", 20, 50)]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["id"], "a");
        // hali yuklanayotgan (uploading) qator ro'yxatga qo'shilmaydi
        list_apply(&mut rows, &["z".to_string()], vec![row("z", "uploading", 30, 60)]);
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn delta_yangi_qatorni_tartib_bilan_qoshadi_va_200_dan_oshirmaydi() {
        let mut rows: Vec<Value> = (0..LIST_N as i64).map(|i| row(&format!("r{i}"), "ready", i, i)).collect();
        list_apply(&mut rows, &["yangi".to_string()], vec![row("yangi", "uploaded", 10_000, 10_000)]);
        assert_eq!(rows.len(), LIST_N);
        assert_eq!(rows[0]["id"], "yangi"); // eng yangi tepada
        assert!(rows.iter().all(|r| r["id"] != "r0")); // eng eskisi chiqib ketdi
    }

    #[test]
    fn delta_o_zgarmagan_qatorlarga_tegmaydi() {
        let mut rows = vec![row("b", "ready", 20, 20), row("a", "ready", 10, 10)];
        list_apply(&mut rows, &["a".to_string()], vec![row("a", "ready", 10, 77)]);
        assert_eq!(rows[0]["id"], "b");
        assert_eq!(rows[0]["updated"], 20); // b tegilmadi
        assert_eq!(rows[1]["updated"], 77);
    }

    #[test]
    fn nom_va_id() {
        assert!(id_ok("a1B2c3"));
        assert!(!id_ok(""));
        assert!(!id_ok("a/b"));
        assert!(!id_ok(&"x".repeat(40)));
        assert_eq!(clean_name("  a/b\\c.mp4 "), "abc.mp4");
        assert_eq!(clean_name("///"), "fayl");
    }
}
