//! ARUTUBE worker (Rust → WASM): DoodStream API kaliti FAQAT shu yerda (Cloudflare secret `DOOD_KEY`), ilovada emas.
//! Bazasiz: har so'rov DoodStream'dan jonli olinadi. Fayl baytlari worker'dan o'tmaydi.
//!
//! GET  /api/videos?page=1&per_page=24  — videolar ro'yxati (muqova, davomiylik, ko'rishlar, iframe manzili)
//! GET  /api/video/<kod>                — bitta video
//! GET  /api/upload-ticket              — yuklash manzili + kalit (faqat ilova tokeni bilan)
//! POST /api/remote {"url","title"}     — havoladan yuklash (DoodStream o'zi oladi)
//! Himoya: `APP_TOKEN` secret qo'yilgan bo'lsa, `X-App-Token` sarlavhasi unga teng bo'lishi shart.
use serde_json::{json, Value};
use std::collections::HashMap;
use worker::*;

const API: &str = "https://doodapi.co/api";
const DEFAULT_HOST: &str = "https://playmogo.com";

fn cors(mut r: Response) -> Result<Response> {
    let h = r.headers_mut();
    h.set("Access-Control-Allow-Origin", "*")?;
    h.set("Access-Control-Allow-Headers", "X-App-Token, Content-Type")?;
    h.set("Access-Control-Allow-Methods", "GET, POST, OPTIONS")?;
    Ok(r)
}

fn reply(v: &Value, status: u16) -> Result<Response> {
    cors(Response::from_json(v)?.with_status(status))
}

fn num(v: &Value) -> i64 {
    v.as_i64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok())).unwrap_or(0)
}

fn st(v: &Value) -> String {
    v.as_str().unwrap_or("").to_string()
}

/// DoodStream fayl yozuvi -> ilova modeli.
pub fn map_file(f: &Value) -> Value {
    let code = if f["file_code"].is_string() { st(&f["file_code"]) } else { st(&f["filecode"]) };
    let dl = st(&f["download_url"]);
    let host = dl
        .split("/d/")
        .next()
        .filter(|h| h.starts_with("http"))
        .unwrap_or(DEFAULT_HOST)
        .to_string();
    json!({
        "code": code,
        "title": st(&f["title"]),
        "duration": num(&f["length"]),
        "uploaded": st(&f["uploaded"]),
        "views": num(&f["views"]),
        "thumb": st(&f["single_img"]),
        "splash": st(&f["splash_img"]),
        "embed": format!("{host}/e/{code}"),
        "canplay": num(&f["canplay"]),
    })
}

fn rerr(m: impl Into<String>) -> Error {
    Error::RustError(m.into())
}

async fn dood(env: &Env, path: &str, extra: &[(&str, &str)]) -> Result<Value> {
    let key = env.secret("DOOD_KEY").map_err(|_| rerr("DOOD_KEY sozlanmagan"))?.to_string();
    let mut url = Url::parse(&format!("{API}/{path}")).map_err(|e| rerr(e.to_string()))?;
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("key", &key);
        for (k, v) in extra {
            q.append_pair(k, v);
        }
    }
    let mut resp = Fetch::Url(url).send().await?;
    let code = resp.status_code();
    let text = resp.text().await?;
    serde_json::from_str::<Value>(&text).map_err(|_| rerr(format!("DoodStream javobi tushunarsiz (HTTP {code})")))
}

fn check_ok(v: &Value) -> Result<()> {
    if num(&v["status"]) == 200 {
        Ok(())
    } else {
        Err(rerr(format!("DoodStream: {}", st(&v["msg"]))))
    }
}

fn authorized(req: &Request, env: &Env) -> bool {
    match env.secret("APP_TOKEN") {
        Ok(s) => {
            let want = s.to_string();
            let got = req.headers().get("X-App-Token").ok().flatten().unwrap_or_default();
            want.is_empty() || got == want
        }
        Err(_) => true,
    }
}

async fn route(mut req: Request, env: &Env, url: &Url, path: &str) -> Result<Response> {
    let q: HashMap<String, String> = url.query_pairs().into_owned().collect();
    let method = req.method();
    match (method, path) {
        (Method::Get, "/api/videos") => {
            let page = q.get("page").map(String::as_str).unwrap_or("1");
            let per = q.get("per_page").map(String::as_str).unwrap_or("24");
            let v = dood(env, "file/list", &[("page", page), ("per_page", per)]).await?;
            check_ok(&v)?;
            let files = v["result"]["files"].as_array().cloned().unwrap_or_default();
            let videos: Vec<Value> = files.iter().map(map_file).collect();
            let mut r = reply(
                &json!({
                    "videos": videos,
                    "page": page.parse::<i64>().unwrap_or(1),
                    "pages": num(&v["result"]["total_pages"]).max(1),
                    "total": num(&v["result"]["results_total"]),
                }),
                200,
            )?;
            r.headers_mut().set("Cache-Control", "public, max-age=30")?;
            Ok(r)
        }
        (Method::Get, p) if p.starts_with("/api/video/") => {
            let code = &p["/api/video/".len()..];
            let v = dood(env, "file/info", &[("file_code", code)]).await?;
            check_ok(&v)?;
            let f = v["result"].as_array().and_then(|a| a.first()).cloned().unwrap_or(Value::Null);
            if f.is_null() {
                return reply(&json!({"error": "video topilmadi"}), 404);
            }
            let mut m = map_file(&f);
            m["size"] = json!(num(&f["size"]));
            reply(&m, 200)
        }
        (Method::Get, "/api/upload-ticket") => {
            let v = dood(env, "upload/server", &[]).await?;
            check_ok(&v)?;
            let key = env.secret("DOOD_KEY").map_err(|_| rerr("DOOD_KEY sozlanmagan"))?.to_string();
            reply(&json!({"url": st(&v["result"]), "key": key}), 200)
        }
        (Method::Post, "/api/remote") => {
            let body: Value = req.json().await.map_err(|_| rerr("noto'g'ri JSON"))?;
            let u = st(&body["url"]);
            if !u.starts_with("http") {
                return reply(&json!({"error": "havola http(s) bilan boshlanishi kerak"}), 400);
            }
            let t = st(&body["title"]);
            let mut extra: Vec<(&str, &str)> = vec![("url", u.as_str())];
            if !t.is_empty() {
                extra.push(("new_title", t.as_str()));
            }
            let v = dood(env, "upload/url", &extra).await?;
            check_ok(&v)?;
            reply(&json!({"ok": true, "code": st(&v["result"]["filecode"])}), 200)
        }
        _ => reply(&json!({"error": "topilmadi"}), 404),
    }
}

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    if req.method() == Method::Options {
        return cors(Response::empty()?.with_status(204));
    }
    let url = req.url()?;
    let path = url.path().to_string();
    if path == "/" || path == "/health" {
        return reply(&json!({"ok": true, "name": "arutube-api"}), 200);
    }
    if !authorized(&req, &env) {
        return reply(&json!({"error": "ruxsat yo'q"}), 401);
    }
    match route(req, &env, &url, &path).await {
        Ok(r) => Ok(r),
        Err(e) => reply(&json!({"error": e.to_string()}), 502),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_file_ro_yxat_yozuvi() {
        let f = json!({"file_code":"abc","title":"T","length":"6001","uploaded":"2026-11-06","views":"5",
            "single_img":"https://i/x.jpg","download_url":"https://playmogo.com/d/abc","canplay":1});
        let m = map_file(&f);
        assert_eq!(m["code"], "abc");
        assert_eq!(m["duration"], 6001);
        assert_eq!(m["views"], 5);
        assert_eq!(m["embed"], "https://playmogo.com/e/abc");
    }

    #[test]
    fn map_file_info_yozuvi_va_standart_host() {
        let f = json!({"filecode":"zz","length":12});
        let m = map_file(&f);
        assert_eq!(m["code"], "zz");
        assert_eq!(m["embed"], "https://playmogo.com/e/zz");
    }
}
