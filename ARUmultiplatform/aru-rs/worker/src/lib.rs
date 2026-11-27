//! ARU API (Rust -> WASM, Cloudflare Worker). Videolar Telegram kanaliga <=19 MiB bo'laklarda yoziladi (bot `getFile` 20 MB gacha beradi),
//! ro'yxat va bo'laklar jadvali Supabase (Postgres, funksiyalar orqali) bazasida. Bot tokeni FAQAT shu yerda (secret `BOT_TOKEN`, `CHANNEL_ID`); baza: `SB_URL`, `SB_ANON`, `DB_KEY`.
//!
//! GET    /api/health
//! GET    /api/videos?kind=&q=&author=&followed_by=&sort=new|views&limit=&offset=&me=   — tayyor videolar
//! POST   /api/user {id,name} · GET /api/channel/<id>?me= · POST /api/follow {uid,target,on}
//! POST   /api/video/<id>/like|view · GET|POST /api/video/<id>/notes · GET /api/video/<id>/thumb
//! GET    /api/video/<id>                           — video va bo'laklar hajmi
//! GET    /api/video/<id>/stream                    — bo'laklarni ulab beradi (Range qo'llanadi)
//! POST   /api/upload      {title,channel,size,parts,mime}  -> {id}        (X-App-Token)
//! PUT    /api/upload/<id>/<n>   (bayt, <=19 MiB)                           (X-App-Token)
//! POST   /api/upload/<id>/done  {duration}                                 (X-App-Token)
//! DELETE /api/video/<id>                                                   (X-App-Token)

use serde_json::{json, Value};
use worker::wasm_bindgen::JsValue;
use worker::*;

const TG: &str = "https://api.telegram.org";
/// Bitta bo'lakning eng katta hajmi: Telegram botlari 20 MB gacha faylni qaytara oladi.
const MAX_PART: usize = 19 * 1024 * 1024;

fn rerr(m: impl Into<String>) -> Error {
    Error::RustError(m.into())
}

fn cors(mut r: Response) -> Result<Response> {
    let h = r.headers_mut();
    h.set("Access-Control-Allow-Origin", "*")?;
    h.set("Access-Control-Allow-Headers", "X-App-Token, Content-Type, Range")?;
    h.set("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS")?;
    h.set("Access-Control-Expose-Headers", "Content-Range, Accept-Ranges, Content-Length")?;
    Ok(r)
}

fn reply(v: &Value, status: u16) -> Result<Response> {
    cors(Response::from_json(v)?.with_status(status))
}

fn secret(env: &Env, name: &str) -> Result<String> {
    Ok(env.secret(name).map_err(|_| rerr(format!("{name} sozlanmagan")))?.to_string())
}

/// Yozish so'rovlari faqat `APP_TOKEN` bilan; token sozlanmagan bo'lsa hech kimga ruxsat yo'q.
fn authorized(req: &Request, env: &Env) -> bool {
    match secret(env, "APP_TOKEN") {
        Ok(want) if !want.trim().is_empty() => req.headers().get("X-App-Token").ok().flatten().as_deref() == Some(want.trim()),
        _ => false,
    }
}

// ───────────────────────── sof mantiq (testlanadi) ─────────────────────────

/// `Range: bytes=a-b` / `bytes=a-` / `bytes=-n` -> (boshi, oxiri) (oxiri ham kiradi); noto'g'ri bo'lsa `None`.
pub fn parse_range(h: &str, total: u64) -> Option<(u64, u64)> {
    if total == 0 {
        return None;
    }
    let spec = h.trim().strip_prefix("bytes=")?;
    let (a, b) = spec.split_once('-')?;
    let (start, end) = if a.is_empty() {
        let n: u64 = b.trim().parse().ok()?;
        (total.saturating_sub(n), total - 1)
    } else {
        let s: u64 = a.trim().parse().ok()?;
        let e: u64 = if b.trim().is_empty() { total - 1 } else { b.trim().parse().ok()? };
        (s, e.min(total - 1))
    };
    if start > end || start >= total {
        None
    } else {
        Some((start, end))
    }
}

/// Umumiy `pos` baytiga mos bo'lak: (bo'lak raqami, bo'lak ichidagi joy, bo'lak boshining umumiy joyi).
pub fn locate(sizes: &[u64], pos: u64) -> Option<(usize, u64, u64)> {
    let mut base = 0u64;
    for (i, s) in sizes.iter().enumerate() {
        if pos < base + s {
            return Some((i, pos - base, base));
        }
        base += s;
    }
    None
}

fn new_id() -> String {
    let n = (worker::js_sys::Math::random() * 4_294_967_296.0) as u64;
    format!("{:x}{:08x}", Date::now().as_millis(), n)
}

// ───────────────────────── Telegram ─────────────────────────

async fn tg_call(token: &str, method: &str, body: &Value) -> Result<Value> {
    let mut h = Headers::new();
    h.set("Content-Type", "application/json")?;
    let mut init = RequestInit::new();
    init.with_method(Method::Post).with_headers(h).with_body(Some(JsValue::from_str(&body.to_string())));
    let mut r = Fetch::Request(Request::new_with_init(&format!("{TG}/bot{token}/{method}"), &init)?).send().await?;
    let v: Value = r.json().await?;
    if v["ok"].as_bool() == Some(true) {
        Ok(v["result"].clone())
    } else {
        Err(rerr(format!("Telegram: {}", v["description"].as_str().unwrap_or("xato"))))
    }
}

/// Bo'lakni kanalga hujjat sifatida yozadi; (file_id, message_id) qaytadi.
async fn tg_put(token: &str, chat: &str, name: &str, bytes: &[u8]) -> Result<(String, i64)> {
    let b = "----aruBoundary7MA4YWxk";
    let mut body: Vec<u8> = Vec::with_capacity(bytes.len() + 512);
    for (k, v) in [("chat_id", chat), ("disable_notification", "true")] {
        body.extend_from_slice(format!("--{b}\r\nContent-Disposition: form-data; name=\"{k}\"\r\n\r\n{v}\r\n").as_bytes());
    }
    body.extend_from_slice(format!("--{b}\r\nContent-Disposition: form-data; name=\"document\"; filename=\"{name}\"\r\nContent-Type: application/octet-stream\r\n\r\n").as_bytes());
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{b}--\r\n").as_bytes());
    let mut h = Headers::new();
    h.set("Content-Type", &format!("multipart/form-data; boundary={b}"))?;
    let mut init = RequestInit::new();
    init.with_method(Method::Post).with_headers(h).with_body(Some(worker::js_sys::Uint8Array::from(&body[..]).into()));
    let mut r = Fetch::Request(Request::new_with_init(&format!("{TG}/bot{token}/sendDocument"), &init)?).send().await?;
    let v: Value = r.json().await?;
    if v["ok"].as_bool() != Some(true) {
        return Err(rerr(format!("Telegram: {}", v["description"].as_str().unwrap_or("yuborilmadi"))));
    }
    let fid = v["result"]["document"]["file_id"].as_str().ok_or_else(|| rerr("Telegram file_id bermadi"))?.to_string();
    Ok((fid, v["result"]["message_id"].as_i64().unwrap_or(0)))
}

async fn tg_get(token: &str, file_id: &str) -> Result<Vec<u8>> {
    let f = tg_call(token, "getFile", &json!({ "file_id": file_id })).await?;
    let path = f["file_path"].as_str().ok_or_else(|| rerr("Telegram file_path bermadi"))?;
    let mut r = Fetch::Url(Url::parse(&format!("{TG}/file/bot{token}/{path}")).map_err(|e| rerr(e.to_string()))?).send().await?;
    if r.status_code() != 200 {
        return Err(rerr(format!("Telegram fayl HTTP {}", r.status_code())));
    }
    r.bytes().await
}

// ───────────────────────── Supabase (PostgREST RPC) ─────────────────────────

/// Postgres funksiyasini chaqiradi. Ochiq funksiyalar faqat anon kalit bilan; yozish/maxfiy funksiyalar qo'shimcha `k` (DB_KEY) so'raydi.
async fn rpc(env: &Env, f: &str, body: Value) -> Result<Value> {
    let (base, anon) = (secret(env, "SB_URL")?, secret(env, "SB_ANON")?);
    let mut h = Headers::new();
    h.set("Content-Type", "application/json")?;
    h.set("apikey", anon.trim())?;
    h.set("Authorization", &format!("Bearer {}", anon.trim()))?;
    let mut init = RequestInit::new();
    init.with_method(Method::Post).with_headers(h).with_body(Some(JsValue::from_str(&body.to_string())));
    let mut r = Fetch::Request(Request::new_with_init(&format!("{}/rest/v1/rpc/{f}", base.trim()), &init)?).send().await?;
    let ok = (200..300).contains(&r.status_code());
    let text = r.text().await?;
    if !ok {
        return Err(rerr(format!("baza: {}", text.chars().take(300).collect::<String>())));
    }
    if text.trim().is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(&text).map_err(|_| rerr("bazadan tushunarsiz javob"))
}

fn dbkey(env: &Env) -> Result<String> {
    Ok(secret(env, "DB_KEY")?.trim().to_string())
}

fn int(v: &Value) -> i64 {
    v.as_i64().unwrap_or(0)
}

// ───────────────────────── marshrutlar ─────────────────────────

fn q_str(q: &std::collections::HashMap<String, String>, k: &str) -> String {
    q.get(k).cloned().unwrap_or_default().chars().take(100).collect()
}

fn q_int(q: &std::collections::HashMap<String, String>, k: &str, d: i64) -> i64 {
    q.get(k).and_then(|v| v.parse().ok()).unwrap_or(d)
}

/// Telegram'dan fayl olib, kesh sarlavhalari bilan beradi (muqova rasmi uchun).
async fn file_response(bytes: Vec<u8>, mime: &str, cache: &str) -> Result<Response> {
    let mut r = Response::from_bytes(bytes)?;
    r.headers_mut().set("Content-Type", mime)?;
    r.headers_mut().set("Cache-Control", cache)?;
    cors(r)
}

async fn route(mut req: Request, env: &Env, url: &Url) -> Result<Response> {
    let path = url.path().to_string();
    let seg: Vec<&str> = path.trim_matches('/').split('/').collect();
    let method = req.method();
    let writes = matches!(method, Method::Post | Method::Put | Method::Delete);
    if writes && !authorized(&req, env) {
        return reply(&json!({ "error": "ruxsat yo'q" }), 401);
    }
    let q: std::collections::HashMap<String, String> = url.query_pairs().into_owned().collect();
    let me = q_str(&q, "me");
    match (method, seg.as_slice()) {
        (Method::Get, ["api", "health"]) => reply(&json!({ "ok": true, "name": "aru-api" }), 200),

        // Kanal (foydalanuvchi) yaratish / nomini yangilash.
        (Method::Post, ["api", "user"]) => {
            let b: Value = req.json().await.map_err(|_| rerr("noto'g'ri JSON"))?;
            let r = rpc(env, "aru_user", json!({ "k": dbkey(env)?, "me": b["id"].as_str().unwrap_or(""), "nm": b["name"].as_str().unwrap_or("") })).await?;
            reply(&r, 200)
        }

        (Method::Get, ["api", "channel", id]) => {
            let r = rpc(env, "aru_channel", json!({ "cid": id, "me": me })).await?;
            if r.is_null() {
                return reply(&json!({ "error": "kanal topilmadi" }), 404);
            }
            reply(&r, 200)
        }

        (Method::Get, ["api", "videos"]) => {
            let rows = rpc(
                env,
                "aru_videos",
                json!({
                    "me": me, "kd": q_str(&q, "kind"), "q": q_str(&q, "q"), "author": q_str(&q, "author"),
                    "followed_by": q_str(&q, "followed_by"), "srt": q_str(&q, "sort"),
                    "lim": q_int(&q, "limit", 24).clamp(1, 100), "off": q_int(&q, "offset", 0).max(0),
                }),
            )
            .await?;
            reply(&json!({ "videos": rows }), 200)
        }

        (Method::Post, ["api", "upload"]) => {
            let b: Value = req.json().await.map_err(|_| rerr("noto'g'ri JSON"))?;
            let title = b["title"].as_str().unwrap_or("").trim().to_string();
            let (size, n) = (b["size"].as_i64().unwrap_or(-1), b["parts"].as_i64().unwrap_or(0));
            if title.is_empty() || title.chars().count() > 200 || size < 0 || !(1..=100_000).contains(&n) {
                return reply(&json!({ "error": "title, size va parts to'g'ri emas" }), 400);
            }
            let id = new_id();
            rpc(
                env,
                "aru_video_new",
                json!({
                    "k": dbkey(env)?, "vid": id, "ttl": title, "me": b["uid"].as_str().unwrap_or(""), "kd": b["kind"].as_str().unwrap_or("video"),
                    "mm": b["mime"].as_str().unwrap_or("video/mp4"), "sz": size, "np": n, "ts": Date::now().as_millis() as i64,
                }),
            )
            .await?;
            reply(&json!({ "id": id }), 200)
        }

        (Method::Put, ["api", "upload", id, "thumb"]) => {
            let bytes = req.bytes().await?;
            if bytes.is_empty() || bytes.len() > 2 * 1024 * 1024 {
                return reply(&json!({ "error": "muqova 1 bayt..2 MiB bo'lishi kerak" }), 413);
            }
            let (token, chat) = (secret(env, "BOT_TOKEN")?, secret(env, "CHANNEL_ID")?);
            let (fid, msg) = tg_put(&token, chat.trim(), &format!("{id}_thumb.jpg"), &bytes).await?;
            rpc(env, "aru_thumb_set", json!({ "k": dbkey(env)?, "vid": id, "fid": fid, "mid": msg })).await?;
            reply(&json!({ "ok": true }), 200)
        }

        (Method::Put, ["api", "upload", id, n]) => {
            let n: i64 = n.parse().map_err(|_| rerr("bo'lak raqami noto'g'ri"))?;
            let v = rpc(env, "aru_video", json!({ "vid": id })).await?;
            if v.is_null() {
                return reply(&json!({ "error": "video topilmadi" }), 404);
            }
            if v["done"].as_bool() == Some(true) || n < 0 || n >= int(&v["parts"]) {
                return reply(&json!({ "error": "bo'lak raqami yoki holat noto'g'ri" }), 400);
            }
            let bytes = req.bytes().await?;
            if bytes.is_empty() || bytes.len() > MAX_PART {
                return reply(&json!({ "error": format!("bo'lak 1..{MAX_PART} bayt bo'lishi kerak") }), 413);
            }
            let (token, chat) = (secret(env, "BOT_TOKEN")?, secret(env, "CHANNEL_ID")?);
            let (fid, msg) = tg_put(&token, chat.trim(), &format!("{id}_{n}.bin"), &bytes).await?;
            rpc(env, "aru_part_put", json!({ "k": dbkey(env)?, "vid": id, "pn": n, "fid": fid, "mid": msg, "sz": bytes.len() as i64 })).await?;
            reply(&json!({ "ok": true, "n": n, "size": bytes.len() }), 200)
        }

        (Method::Post, ["api", "upload", id, "done"]) => {
            let b: Value = req.json().await.unwrap_or(Value::Null);
            let r = rpc(env, "aru_video_done", json!({ "k": dbkey(env)?, "vid": id, "dur": b["duration"].as_i64().unwrap_or(0).clamp(0, 1_000_000) })).await?;
            let ok = r["ok"].as_bool() == Some(true);
            reply(&r, if ok { 200 } else { 409 })
        }

        (Method::Get, ["api", "video", id]) => {
            let v = rpc(env, "aru_video", json!({ "vid": id, "me": me })).await?;
            if v.is_null() {
                return reply(&json!({ "error": "video topilmadi" }), 404);
            }
            reply(&v, 200)
        }

        (Method::Get, ["api", "video", id, "thumb"]) => {
            let fid = rpc(env, "aru_thumb_get", json!({ "k": dbkey(env)?, "vid": id })).await?;
            let Some(fid) = fid.as_str() else { return reply(&json!({ "error": "muqova yo'q" }), 404) };
            let data = tg_get(&secret(env, "BOT_TOKEN")?, fid).await?;
            file_response(data, "image/jpeg", "public, max-age=604800").await
        }

        (Method::Post, ["api", "video", id, "like"]) => {
            let b: Value = req.json().await.map_err(|_| rerr("noto'g'ri JSON"))?;
            let n = rpc(env, "aru_like", json!({ "k": dbkey(env)?, "vid": id, "me": b["uid"].as_str().unwrap_or(""), "flag": b["on"].as_bool().unwrap_or(true) })).await?;
            reply(&json!({ "likes": n }), 200)
        }

        (Method::Post, ["api", "video", id, "view"]) => {
            rpc(env, "aru_view", json!({ "k": dbkey(env)?, "vid": id })).await?;
            reply(&json!({ "ok": true }), 200)
        }

        (Method::Get, ["api", "video", id, "notes"]) => {
            let n = rpc(env, "aru_notes", json!({ "vid": id })).await?;
            reply(&json!({ "notes": n }), 200)
        }

        (Method::Post, ["api", "video", id, "notes"]) => {
            let b: Value = req.json().await.map_err(|_| rerr("noto'g'ri JSON"))?;
            rpc(
                env,
                "aru_note_add",
                json!({ "k": dbkey(env)?, "vid": id, "me": b["uid"].as_str().unwrap_or(""), "tm": b["at"].as_i64().unwrap_or(0), "txt": b["text"].as_str().unwrap_or(""), "ts": Date::now().as_millis() as i64 }),
            )
            .await?;
            reply(&json!({ "ok": true }), 200)
        }

        (Method::Post, ["api", "follow"]) => {
            let b: Value = req.json().await.map_err(|_| rerr("noto'g'ri JSON"))?;
            let n = rpc(
                env,
                "aru_follow",
                json!({ "k": dbkey(env)?, "me": b["uid"].as_str().unwrap_or(""), "target": b["target"].as_str().unwrap_or(""), "flag": b["on"].as_bool().unwrap_or(true) }),
            )
            .await?;
            reply(&json!({ "followers": n }), 200)
        }

        (Method::Get, ["api", "video", id, "stream"]) => {
            let v = rpc(env, "aru_video", json!({ "vid": id })).await?;
            if v.is_null() {
                return reply(&json!({ "error": "video topilmadi" }), 404);
            }
            if v["done"].as_bool() != Some(true) {
                return reply(&json!({ "error": "video hali tayyor emas" }), 409);
            }
            let ps = rpc(env, "aru_parts", json!({ "k": dbkey(env)?, "vid": id })).await?;
            let ps = ps.as_array().cloned().unwrap_or_default();
            let sizes: Vec<u64> = ps.iter().map(|p| int(&p["size"]) as u64).collect();
            let total: u64 = sizes.iter().sum();
            let mime = v["mime"].as_str().unwrap_or("video/mp4").to_string();
            let range = req.headers().get("Range").ok().flatten();
            let (start, end, partial) = match range.as_deref().and_then(|h| parse_range(h, total)) {
                Some((a, b)) => (a, b, true),
                None if range.is_some() => {
                    let mut r = Response::empty()?.with_status(416);
                    r.headers_mut().set("Content-Range", &format!("bytes */{total}"))?;
                    return cors(r);
                }
                None => (0, total.saturating_sub(1), false),
            };
            let Some((k, off, base)) = locate(&sizes, start) else { return reply(&json!({ "error": "video bo'sh" }), 404) };
            // Bitta so'rovda faqat shu bo'lak beriladi (206 kamroq bayt qaytarishi mumkin); mijoz keyingisini o'zi so'raydi.
            let last = end.min(base + sizes[k] - 1);
            let (rel_start, rel_end) = (off, last - base);
            let token = secret(env, "BOT_TOKEN")?;
            let f = tg_call(&token, "getFile", &json!({ "file_id": ps[k]["file_id"].as_str().unwrap_or("") })).await?;
            let tpath = f["file_path"].as_str().ok_or_else(|| rerr("Telegram file_path bermadi"))?;
            // Telegram fayl serveri Range'ni qo'llaydi: kerakli qismi so'raladi va mijozga oqim bilan uzatiladi (to'liq bo'lak kutilmaydi).
            let mut rh = Headers::new();
            rh.set("Range", &format!("bytes={rel_start}-{rel_end}"))?;
            let mut init = RequestInit::new();
            init.with_headers(rh);
            let mut up = Fetch::Request(Request::new_with_init(&format!("{TG}/file/bot{token}/{tpath}"), &init)?).send().await?;
            let code = up.status_code();
            if !(200..300).contains(&code) {
                return reply(&json!({ "error": format!("Telegram fayl HTTP {code}") }), 502);
            }
            let len = rel_end - rel_start + 1;
            let mut r = if code == 206 {
                Response::from_stream(up.stream()?)?
            } else {
                // Zaxira: Telegram Range'ni e'tiborsiz qoldirsa, to'liq bo'lakdan kerakli qismi kesiladi.
                let data = up.bytes().await?;
                if (data.len() as u64) != sizes[k] {
                    return reply(&json!({ "error": "bo'lak hajmi mos emas" }), 502);
                }
                Response::from_bytes(data[rel_start as usize..=rel_end as usize].to_vec())?
            };
            r = r.with_status(if partial { 206 } else { 200 });
            let h = r.headers_mut();
            h.set("Content-Type", &mime)?;
            h.set("Accept-Ranges", "bytes")?;
            h.set("Content-Length", &len.to_string())?;
            h.set("Cache-Control", "public, max-age=86400")?;
            if partial || last < total.saturating_sub(1) {
                h.set("Content-Range", &format!("bytes {start}-{last}/{total}"))?;
                if !partial {
                    // Range so'ralmagan bo'lsa ham to'liq fayl bitta javobga sig'maydi: 206 bilan boshlanadi.
                    r = r.with_status(206);
                }
            }
            cors(r)
        }

        (Method::Delete, ["api", "video", id]) => {
            if rpc(env, "aru_video", json!({ "vid": id })).await?.is_null() {
                return reply(&json!({ "error": "video topilmadi" }), 404);
            }
            let (token, chat) = (secret(env, "BOT_TOKEN")?, secret(env, "CHANNEL_ID")?);
            let msgs = rpc(env, "aru_video_del", json!({ "k": dbkey(env)?, "vid": id, "me": me })).await?;
            let msgs = msgs.as_array().cloned().unwrap_or_default();
            // Bitta so'rovda subrequest cheklovi bor: 40 tagacha xabar o'chiriladi (qolganlari kanalda qoladi).
            let mut gone = 0;
            for m in msgs.iter().take(40) {
                if tg_call(&token, "deleteMessage", &json!({ "chat_id": chat.trim(), "message_id": int(m) })).await.is_ok() {
                    gone += 1;
                }
            }
            reply(&json!({ "ok": true, "o_chirilgan_xabar": gone, "bo_laklar": msgs.len() }), 200)
        }

        _ => reply(&json!({ "error": "topilmadi" }), 404),
    }
}

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    if req.method() == Method::Options {
        return cors(Response::empty()?.with_status(204));
    }
    let url = req.url()?;
    match route(req, &env, &url).await {
        Ok(r) => Ok(r),
        Err(e) => reply(&json!({ "error": e.to_string() }), 502),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_turlari() {
        assert_eq!(parse_range("bytes=0-99", 1000), Some((0, 99)));
        assert_eq!(parse_range("bytes=500-", 1000), Some((500, 999)));
        assert_eq!(parse_range("bytes=-100", 1000), Some((900, 999)));
        assert_eq!(parse_range("bytes=900-5000", 1000), Some((900, 999)));
        assert_eq!(parse_range("bytes=1000-", 1000), None);
        assert_eq!(parse_range("bytes=50-10", 1000), None);
        assert_eq!(parse_range("items=0-1", 1000), None);
        assert_eq!(parse_range("bytes=0-1", 0), None);
    }

    #[test]
    fn bolak_topish() {
        let s = [100u64, 100, 50];
        assert_eq!(locate(&s, 0), Some((0, 0, 0)));
        assert_eq!(locate(&s, 99), Some((0, 99, 0)));
        assert_eq!(locate(&s, 100), Some((1, 0, 100)));
        assert_eq!(locate(&s, 249), Some((2, 49, 200)));
        assert_eq!(locate(&s, 250), None);
        assert_eq!(locate(&[], 0), None);
    }
}
