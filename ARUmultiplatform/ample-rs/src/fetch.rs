// UZ: "O'tkazgich" — Worker (Cloudflare IP) ba'zi saytlarga kira olmaydi (anibla.uz nginx 403 beradi). Shu yo'l orqali so'rov Ample'ning o'z IP'sidan ketadi.
//     Faqat ruxsat etilgan hostlarga (`PROXY_HOSTS`, standart `anibla.uz`), faqat https, yo'naltirishsiz, javob 8 MiB gacha.
// EN: pass-through fetch — the Worker's Cloudflare IPs are blocked by some sites (anibla.uz returns an nginx 403). Requests leave from Ample's own IP.
//     Allow-listed hosts only (`PROXY_HOSTS`, default `anibla.uz`), https only, no redirects, response up to 8 MiB.
// RU: proksi-zapros: IP Cloudflare blokiruyutsya nekotorymi saytami; zapros uhodit s IP Ample. Tolko razreshennye hosty, https, bez redirektov, otvet do 8 MiB.
use anyhow::{anyhow, Result};
use serde_json::{json, Value as J};
use std::time::Duration;

const MAX_BODY: usize = 8 * 1024 * 1024;

pub fn host_allowed(host: &str, allow: &str) -> bool {
    let h = host.to_ascii_lowercase();
    allow.split(',').map(|x| x.trim().to_ascii_lowercase()).filter(|x| !x.is_empty()).any(|a| h == a || h.ends_with(&format!(".{a}")))
}

pub async fn run(b: &J) -> Result<J> {
    let url = b["url"].as_str().ok_or_else(|| anyhow!("url yo'q"))?;
    let u = reqwest::Url::parse(url).map_err(|e| anyhow!("url noto'g'ri: {e}"))?;
    if u.scheme() != "https" { return Err(anyhow!("faqat https")); }
    let allow = std::env::var("PROXY_HOSTS").unwrap_or_else(|_| "anibla.uz".to_string());
    if !host_allowed(u.host_str().unwrap_or(""), &allow) { return Err(anyhow!("host ruxsat etilmagan")); }
    let method = reqwest::Method::from_bytes(b["method"].as_str().unwrap_or("GET").to_ascii_uppercase().as_bytes()).map_err(|_| anyhow!("method noto'g'ri"))?;
    let client = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).timeout(Duration::from_secs(20)).build()?;
    let mut rq = client.request(method, u);
    if let Some(h) = b["headers"].as_object() {
        for (k, v) in h { if let Some(v) = v.as_str() { rq = rq.header(k.as_str(), v); } }
    }
    if let Some(body) = b["body"].as_str() { rq = rq.body(body.to_string()); }
    let r = rq.send().await.map_err(|e| anyhow!("so'rov: {e}"))?;
    let status = r.status().as_u16();
    let bytes = r.bytes().await.map_err(|e| anyhow!("javob: {e}"))?;
    if bytes.len() > MAX_BODY { return Err(anyhow!("javob juda katta")); }
    Ok(json!({"status": status, "body": String::from_utf8_lossy(&bytes)}))
}

#[cfg(test)]
mod tests {
    use super::host_allowed;
    #[test]
    fn hosts() {
        assert!(host_allowed("anibla.uz", "anibla.uz"));
        assert!(host_allowed("API.anibla.uz", "anibla.uz"));
        assert!(!host_allowed("evilanibla.uz", "anibla.uz"));
        assert!(!host_allowed("anibla.uz.evil.com", "anibla.uz"));
        assert!(!host_allowed("x.com", ""));
    }
}
