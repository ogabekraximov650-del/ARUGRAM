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

/// Bitta Telegram bo'lagi: bot 20 MB gacha yuklab oladi, 50 MB gacha yuklaydi.
pub(crate) const CHUNK: u64 = 18 * 1024 * 1024;
/// Fayl hajmining yuqori chegarasi (R2 multipart: 10 000 bo'lak x 18 MiB ≈ 175 GB; amalda 4 GB).
const MAX_SIZE: i64 = 4 * 1024 * 1024 * 1024;

static SCHEMA_OK: AtomicBool = AtomicBool::new(false);

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

pub(crate) async fn route(mut req: Request, env: &Env, path: &str) -> Result<Response> {
    let method = req.method();
    if let Err(e) = ensure(env).await {
        return json_resp(&json!({"error": e.to_string()}), 503);
    }
    let url = req.url()?;
    match (method, path) {
        // ── Ilova: to'g'ridan-to'g'ri (R2 va Actions'siz): bo'lak worker orqali Bot API'ga ketadi, token faqat shu yerda ──
        (Method::Post, "/api/bd/new") => {
            let b: Value = req.json().await.unwrap_or(json!({}));
            let name = clean_name(b["name"].as_str().unwrap_or(""));
            let size = jint(&b, "size");
            if size <= 0 || size > MAX_SIZE {
                return json_resp(&json!({"error": "hajm"}), 400);
            }
            let kind = if b["kind"].as_str() == Some("video") { "video" } else { "file" };
            let mime: String = b["mime"].as_str().unwrap_or("").chars().filter(|c| c.is_ascii_graphic()).take(60).collect();
            let id = new_id();
            sql(env, "INSERT INTO bd_files (id,name,kind,mime,size,w,h,dur,state,chunks,created) VALUES (?,?,?,?,?,?,?,?,'direct',?,?)",
                vec![TursoArg::text(&id), TursoArg::text(&name), TursoArg::text(kind), TursoArg::text(&mime), TursoArg::int(size),
                     TursoArg::int(jint(&b, "w")), TursoArg::int(jint(&b, "h")), TursoArg::int(jint(&b, "dur")),
                     TursoArg::int(chunks_of(size)), TursoArg::int(now_ms())]).await?;
            ok(json!({"id": id, "part_size": CHUNK}))
        }
        (Method::Put, "/api/bd/chunk") => {
            let (id, n) = (qs(&url, "id"), qs(&url, "n").parse::<usize>().ok());
            let Some(n) = n else { return json_resp(&json!({"error": "so'rov"}), 400) };
            if !id_ok(&id) {
                return json_resp(&json!({"error": "so'rov"}), 400);
            }
            let Some(row) = first(&sql(env, "SELECT name,size,chunks,state,chunk_ids FROM bd_files WHERE id=?", vec![TursoArg::text(&id)]).await?) else {
                return err404("fayl yo'q");
            };
            if row["state"].as_str() != Some("direct") {
                return json_resp(&json!({"error": "holat"}), 409);
            }
            let size = row["size"].as_i64().unwrap_or(0) as u64;
            let mut list: Vec<String> = split_ids(row["chunk_ids"].as_str().unwrap_or("")).into_iter().map(String::from).collect();
            if n as i64 >= row["chunks"].as_i64().unwrap_or(0) || n > list.len() {
                return json_resp(&json!({"error": "bo'lak raqami"}), 409);
            }
            let expect = CHUNK.min(size - n as u64 * CHUNK);
            let bytes = req.bytes().await?;
            if bytes.len() as u64 != expect {
                return json_resp(&json!({"error": format!("hajm mos emas ({} != {expect})", bytes.len())}), 400);
            }
            let fname = format!("{}.part{:04}", row["name"].as_str().unwrap_or("fayl"), n);
            let (fid, _mid) = tg_send_document(&tg_secret(env, "BD_BOT_TOKEN"), tg_channel_id(env), &fname, &bytes, expect).await?;
            if n == list.len() { list.push(fid.clone()) } else { list[n] = fid.clone() }
            sql(env, "UPDATE bd_files SET chunk_ids=? WHERE id=? AND state='direct'", vec![TursoArg::text(&list.join(",")), TursoArg::text(&id)]).await?;
            ok(json!({"ok": true, "n": n}))
        }
        (Method::Post, "/api/bd/seal") => {
            let b: Value = req.json().await.unwrap_or(json!({}));
            let id = b["id"].as_str().unwrap_or("").to_string();
            if !id_ok(&id) {
                return json_resp(&json!({"error": "so'rov"}), 400);
            }
            let Some(row) = first(&sql(env, "SELECT chunks,chunk_ids FROM bd_files WHERE id=? AND state='direct'", vec![TursoArg::text(&id)]).await?) else {
                return err404("fayl yo'q");
            };
            let want = row["chunks"].as_i64().unwrap_or(-1);
            let have = split_ids(row["chunk_ids"].as_str().unwrap_or("")).len() as i64;
            if want <= 0 || have != want {
                return json_resp(&json!({"error": format!("bo'laklar yetarli emas ({have}/{want})")}), 409);
            }
            sql(env, "UPDATE bd_files SET state='ready' WHERE id=? AND state='direct'", vec![TursoArg::text(&id)]).await?;
            ok(json!({"ok": true}))
        }
        // Kichik rasm ham Telegram'ga (kanalga) saqlanadi; file_id `thumb_id` ustunida.
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
            sql(env, "UPDATE bd_files SET thumb_id=? WHERE id=? AND state='direct'", vec![TursoArg::text(&fid), TursoArg::text(&id)]).await?;
            ok(json!({"ok": true}))
        }
        (Method::Get, "/api/bd/list") => {
            let res = sql(env,
                "SELECT id,name,kind,mime,size,w,h,dur,state,created FROM bd_files WHERE state='ready' ORDER BY created DESC LIMIT 200",
                vec![]).await?;
            let mut r = ok(json!({"files": rows_of(&res)}))?;
            let _ = r.headers_mut().set("Cache-Control", "no-store");
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
        _ => err404("topilmadi"),
    }
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

    ensure(env).await?;
    let r = sql(env, "SELECT size,mime,chunk_ids FROM bd_files WHERE id=? AND state='ready'", vec![TursoArg::text(id)]).await?;
    let Some(f) = first(&r) else { return reply("error fayl tayyor emas", 0) };
    let total = f["size"].as_i64().unwrap_or(0).max(0) as u64;
    let mime = f["mime"].as_str().filter(|m| !m.is_empty()).unwrap_or("application/octet-stream").to_string();
    let chunks: Vec<(i64, String)> = split_ids(f["chunk_ids"].as_str().unwrap_or("")).into_iter().enumerate().map(|(i, s)| (i as i64, s.to_string())).collect();
    if total == 0 || chunks.len() as i64 != chunks_of(total as i64) {
        return reply("error bo'laklar to'liq emas", total);
    }
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
    worker::wasm_bindgen_futures::spawn_local(async move {
        for n in first_n..=last_n {
            let Some((_, tg)) = chunks.iter().find(|(k, _)| *k as u64 == n) else {
                abort_pipe(&writable);
                return;
            };
            let c_start = n * CHUNK;
            let expect = CHUNK.min(total - c_start);
            let rs = match chunk_stream(&env2, tg, expect).await {
                Ok(s) => s,
                Err(_) => {
                    abort_pipe(&writable);
                    return;
                }
            };
            match pipe_keep_open(&rs, &writable) {
                Ok(p) => {
                    if worker::wasm_bindgen_futures::JsFuture::from(p).await.is_err() {
                        abort_pipe(&writable);
                        return;
                    }
                }
                Err(_) => {
                    abort_pipe(&writable);
                    return;
                }
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
