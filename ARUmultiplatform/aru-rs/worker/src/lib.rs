//! ARU API (Rust -> WASM, Cloudflare Worker). Videolar Telegram kanaliga <=19 MiB bo'laklarda yoziladi (bot `getFile` 20 MB gacha beradi),
//! ro'yxat va bo'laklar jadvali D1 bazasida. Bot tokeni FAQAT shu yerda (secret `BOT_TOKEN`, `CHANNEL_ID`).
//!
//! GET    /api/health
//! GET    /api/videos?limit=24&before=<ms>          — tayyor videolar (yangisi birinchi)
//! GET    /api/video/<id>                           — video va bo'laklar hajmi
//! GET    /api/video/<id>/stream                    — bo'laklarni ulab beradi (Range qo'llanadi)
//! POST   /api/upload      {title,channel,size,parts,mime}  -> {id}        (X-App-Token)
//! PUT    /api/upload/<id>/<n>   (bayt, <=19 MiB)                           (X-App-Token)
//! POST   /api/upload/<id>/done  {duration}                                 (X-App-Token)
//! DELETE /api/video/<id>                                                   (X-App-Token)

use serde::Deserialize;
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

// ───────────────────────── D1 ─────────────────────────

fn js(s: &str) -> JsValue {
    JsValue::from_str(s)
}

fn jn(n: i64) -> JsValue {
    JsValue::from_f64(n as f64)
}

#[derive(Deserialize)]
struct VideoRow {
    id: String,
    title: String,
    channel: String,
    mime: String,
    size: f64,
    parts: f64,
    duration: f64,
    views: f64,
    done: f64,
    created: f64,
}

impl VideoRow {
    fn json(&self) -> Value {
        json!({
            "id": self.id, "title": self.title, "channel": self.channel, "mime": self.mime,
            "size": self.size as i64, "parts": self.parts as i64, "duration": self.duration as i64,
            "views": self.views as i64, "created": self.created as i64,
        })
    }
}

#[derive(Deserialize)]
struct PartRow {
    n: f64,
    file_id: String,
    msg: f64,
    size: f64,
}

async fn video(db: &D1Database, id: &str) -> Result<Option<VideoRow>> {
    db.prepare("SELECT id,title,channel,mime,size,parts,duration,views,done,created FROM videos WHERE id=?1").bind(&[js(id)])?.first::<VideoRow>(None).await
}

async fn parts(db: &D1Database, id: &str) -> Result<Vec<PartRow>> {
    db.prepare("SELECT n,file_id,msg,size FROM parts WHERE video=?1 ORDER BY n").bind(&[js(id)])?.all().await?.results::<PartRow>()
}

// ───────────────────────── marshrutlar ─────────────────────────

async fn route(mut req: Request, env: &Env, url: &Url) -> Result<Response> {
    let path = url.path().to_string();
    let seg: Vec<&str> = path.trim_matches('/').split('/').collect();
    let method = req.method();
    let writes = matches!(method, Method::Post | Method::Put | Method::Delete);
    if writes && !authorized(&req, env) {
        return reply(&json!({ "error": "ruxsat yo'q" }), 401);
    }
    let db = env.d1("DB")?;
    match (method, seg.as_slice()) {
        (Method::Get, ["api", "health"]) => reply(&json!({ "ok": true, "name": "aru-api" }), 200),

        (Method::Get, ["api", "videos"]) => {
            let q: std::collections::HashMap<String, String> = url.query_pairs().into_owned().collect();
            let limit = q.get("limit").and_then(|v| v.parse::<i64>().ok()).unwrap_or(24).clamp(1, 100);
            let before = q.get("before").and_then(|v| v.parse::<i64>().ok()).unwrap_or(i64::MAX / 4);
            let rows = db
                .prepare("SELECT id,title,channel,mime,size,parts,duration,views,done,created FROM videos WHERE done=1 AND created<?1 ORDER BY created DESC LIMIT ?2")
                .bind(&[jn(before), jn(limit)])?
                .all()
                .await?
                .results::<VideoRow>()?;
            let mut r = reply(&json!({ "videos": rows.iter().map(VideoRow::json).collect::<Vec<_>>() }), 200)?;
            r.headers_mut().set("Cache-Control", "public, max-age=15")?;
            Ok(r)
        }

        (Method::Post, ["api", "upload"]) => {
            let b: Value = req.json().await.map_err(|_| rerr("noto'g'ri JSON"))?;
            let title = b["title"].as_str().unwrap_or("").trim().to_string();
            let (size, n) = (b["size"].as_i64().unwrap_or(-1), b["parts"].as_i64().unwrap_or(0));
            if title.is_empty() || title.chars().count() > 200 || size < 0 || !(1..=100_000).contains(&n) {
                return reply(&json!({ "error": "title, size va parts to'g'ri emas" }), 400);
            }
            let id = new_id();
            let now = Date::now().as_millis() as i64;
            db.prepare("INSERT INTO videos(id,title,channel,mime,size,parts,done,created) VALUES(?1,?2,?3,?4,?5,?6,0,?7)")
                .bind(&[js(&id), js(&title), js(b["channel"].as_str().unwrap_or("")), js(b["mime"].as_str().unwrap_or("video/mp4")), jn(size), jn(n), jn(now)])?
                .run()
                .await?;
            reply(&json!({ "id": id }), 200)
        }

        (Method::Put, ["api", "upload", id, n]) => {
            let n: i64 = n.parse().map_err(|_| rerr("bo'lak raqami noto'g'ri"))?;
            let Some(v) = video(&db, id).await? else { return reply(&json!({ "error": "video topilmadi" }), 404) };
            if v.done != 0.0 || n < 0 || n as f64 >= v.parts {
                return reply(&json!({ "error": "bo'lak raqami yoki holat noto'g'ri" }), 400);
            }
            let bytes = req.bytes().await?;
            if bytes.is_empty() || bytes.len() > MAX_PART {
                return reply(&json!({ "error": format!("bo'lak 1..{MAX_PART} bayt bo'lishi kerak") }), 413);
            }
            let (token, chat) = (secret(env, "BOT_TOKEN")?, secret(env, "CHANNEL_ID")?);
            let (fid, msg) = tg_put(&token, chat.trim(), &format!("{id}_{n}.bin"), &bytes).await?;
            db.prepare("INSERT OR REPLACE INTO parts(video,n,file_id,msg,size) VALUES(?1,?2,?3,?4,?5)").bind(&[js(id), jn(n), js(&fid), jn(msg), jn(bytes.len() as i64)])?.run().await?;
            reply(&json!({ "ok": true, "n": n, "size": bytes.len() }), 200)
        }

        (Method::Post, ["api", "upload", id, "done"]) => {
            let b: Value = req.json().await.unwrap_or(Value::Null);
            let Some(v) = video(&db, id).await? else { return reply(&json!({ "error": "video topilmadi" }), 404) };
            let ps = parts(&db, id).await?;
            let sum: f64 = ps.iter().map(|p| p.size).sum();
            if ps.len() as f64 != v.parts || sum != v.size {
                return reply(&json!({ "error": "bo'laklar to'liq emas", "yuklangan": ps.len(), "kerak": v.parts as i64 }), 409);
            }
            db.prepare("UPDATE videos SET done=1, duration=?2 WHERE id=?1").bind(&[js(id), jn(b["duration"].as_i64().unwrap_or(0).max(0))])?.run().await?;
            reply(&json!({ "ok": true }), 200)
        }

        (Method::Get, ["api", "video", id]) => {
            let Some(v) = video(&db, id).await? else { return reply(&json!({ "error": "video topilmadi" }), 404) };
            let ps = parts(&db, id).await?;
            let mut m = v.json();
            m["done"] = json!(v.done != 0.0);
            m["part_sizes"] = json!(ps.iter().map(|p| p.size as i64).collect::<Vec<_>>());
            reply(&m, 200)
        }

        (Method::Get, ["api", "video", id, "stream"]) => {
            let Some(v) = video(&db, id).await? else { return reply(&json!({ "error": "video topilmadi" }), 404) };
            if v.done == 0.0 {
                return reply(&json!({ "error": "video hali tayyor emas" }), 409);
            }
            let ps = parts(&db, id).await?;
            let sizes: Vec<u64> = ps.iter().map(|p| p.size as u64).collect();
            let total: u64 = sizes.iter().sum();
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
            let data = tg_get(&secret(env, "BOT_TOKEN")?, &ps[k].file_id).await?;
            if data.len() as u64 != sizes[k] {
                return reply(&json!({ "error": "bo'lak hajmi mos emas" }), 502);
            }
            // Bitta so'rovda faqat shu bo'lak beriladi (206 kamroq bayt qaytarishi mumkin); mijoz keyingisini o'zi so'raydi.
            let last = end.min(base + sizes[k] - 1);
            let body = data[off as usize..=(last - base) as usize].to_vec();
            let mut r = Response::from_bytes(body)?.with_status(if partial { 206 } else { 200 });
            let h = r.headers_mut();
            h.set("Content-Type", &v.mime)?;
            h.set("Accept-Ranges", "bytes")?;
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
            if video(&db, id).await?.is_none() {
                return reply(&json!({ "error": "video topilmadi" }), 404);
            }
            let (token, chat) = (secret(env, "BOT_TOKEN")?, secret(env, "CHANNEL_ID")?);
            let ps = parts(&db, id).await?;
            // Bitta so'rovda subrequest cheklovi bor: 40 tagacha xabar o'chiriladi (qolganlari kanalda qoladi).
            let mut gone = 0;
            for p in ps.iter().take(40) {
                if tg_call(&token, "deleteMessage", &json!({ "chat_id": chat.trim(), "message_id": p.msg as i64 })).await.is_ok() {
                    gone += 1;
                }
            }
            db.prepare("DELETE FROM parts WHERE video=?1").bind(&[js(id)])?.run().await?;
            db.prepare("DELETE FROM videos WHERE id=?1").bind(&[js(id)])?.run().await?;
            reply(&json!({ "ok": true, "o_chirilgan_xabar": gone, "bo_laklar": ps.len() }), 200)
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
