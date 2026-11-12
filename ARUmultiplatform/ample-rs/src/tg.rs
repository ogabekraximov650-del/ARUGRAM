//! UZ: Telegram bot API (fayl yuklash/olish/o'chirish, pin). 429 va 5xx da qayta uriniladi. Hamma so'rov va trafik sanaladi (`stats`). EN/RU: Telegram bot API client with retries and request/traffic counters.
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::time::Duration;

static REQS: AtomicU64 = AtomicU64::new(0);
static UP: AtomicU64 = AtomicU64::new(0);
static DOWN: AtomicU64 = AtomicU64::new(0);

/// Ishga tushgandan beri: Telegram'ga ketgan so'rovlar soni va baytlar. EN: requests and bytes since start.
pub fn stats() -> Value { json!({"requests": REQS.load(Relaxed), "bytes_up": UP.load(Relaxed), "bytes_down": DOWN.load(Relaxed)}) }

#[derive(Clone)]
pub struct Tg { client: reqwest::Client, token: String, channel: String, base: String }

impl Tg {
    pub fn new(token: String, channel: String) -> Self {
        // `TG_API_BASE` — faqat sinov uchun (soxta Telegram serveri). EN: test-only override.
        let base = std::env::var("TG_API_BASE").ok().filter(|v| !v.is_empty()).unwrap_or_else(|| "https://api.telegram.org".to_string());
        Self { client: reqwest::Client::builder().timeout(Duration::from_secs(120)).build().unwrap(), token, channel, base }
    }
    fn url(&self, m: &str) -> String { format!("{}/bot{}/{}", self.base, self.token, m) }

    async fn call(&self, method: &str, body: Value) -> Result<Value> {
        for a in 0..6u64 {
            REQS.fetch_add(1, Relaxed);
            let r = self.client.post(self.url(method)).json(&body).send().await;
            let Ok(r) = r else { tokio::time::sleep(Duration::from_secs(a + 1)).await; continue };
            let j: Value = r.json().await.unwrap_or(json!({}));
            if j["ok"] == json!(true) { return Ok(j["result"].clone()); }
            match j["error_code"].as_i64().unwrap_or(0) {
                429 => { tokio::time::sleep(Duration::from_secs(j["parameters"]["retry_after"].as_u64().unwrap_or(5) + 1)).await; }
                c if c >= 500 => { tokio::time::sleep(Duration::from_secs(a + 1)).await; }
                _ => return Err(anyhow!("telegram {}: {}", method, j["description"].as_str().unwrap_or("xato"))),
            }
        }
        Err(anyhow!("telegram {}: qayta urinishlar tugadi", method))
    }

    /// Hujjat yuboradi: `(file_id, msg_id)`. EN/RU: sends a document.
    pub async fn send(&self, name: &str, data: Vec<u8>) -> Result<(String, i64)> {
        for a in 0..6u64 {
            let part = reqwest::multipart::Part::bytes(data.clone()).file_name(name.to_string());
            let form = reqwest::multipart::Form::new().text("chat_id", self.channel.clone()).part("document", part);
            REQS.fetch_add(1, Relaxed); UP.fetch_add(data.len() as u64, Relaxed);
            let r = self.client.post(self.url("sendDocument")).multipart(form).send().await;
            let Ok(r) = r else { tokio::time::sleep(Duration::from_secs(a + 1)).await; continue };
            let j: Value = r.json().await.unwrap_or(json!({}));
            if j["ok"] == json!(true) {
                return Ok((j["result"]["document"]["file_id"].as_str().unwrap_or("").to_string(), j["result"]["message_id"].as_i64().unwrap_or(0)));
            }
            match j["error_code"].as_i64().unwrap_or(0) {
                429 => tokio::time::sleep(Duration::from_secs(j["parameters"]["retry_after"].as_u64().unwrap_or(5) + 1)).await,
                c if c >= 500 => tokio::time::sleep(Duration::from_secs(a + 1)).await,
                _ => return Err(anyhow!("telegram sendDocument: {}", j["description"].as_str().unwrap_or("xato"))),
            }
        }
        Err(anyhow!("telegram sendDocument: qayta urinishlar tugadi"))
    }

    /// Mavjud hujjatni joyida almashtiradi (pin saqlanadi, "pin qildi" xizmat xabari chiqmaydi). EN/RU: replace a document in place (keeps the pin, no new service message).
    pub async fn edit(&self, msg_id: i64, name: &str, data: Vec<u8>) -> Result<()> {
        let n = data.len() as u64;
        let part = reqwest::multipart::Part::bytes(data).file_name(name.to_string());
        let media = json!({"type": "document", "media": "attach://f"}).to_string();
        let form = reqwest::multipart::Form::new().text("chat_id", self.channel.clone()).text("message_id", msg_id.to_string()).text("media", media).part("f", part);
        REQS.fetch_add(1, Relaxed); UP.fetch_add(n, Relaxed);
        let j: Value = self.client.post(self.url("editMessageMedia")).multipart(form).send().await?.json().await.unwrap_or(json!({}));
        if j["ok"] == json!(true) { Ok(()) } else { Err(anyhow!("telegram editMessageMedia: {}", j["description"].as_str().unwrap_or("xato"))) }
    }

    pub async fn get(&self, file_id: &str) -> Result<Vec<u8>> {
        let f = self.call("getFile", json!({"file_id": file_id})).await?;
        let path = f["file_path"].as_str().ok_or_else(|| anyhow!("telegram fayl yo'lini bermadi"))?;
        REQS.fetch_add(1, Relaxed);
        let r = self.client.get(format!("{}/file/bot{}/{}", self.base, self.token, path)).send().await?;
        if !r.status().is_success() { return Err(anyhow!("telegram yuklash {}", r.status())); }
        let b = r.bytes().await?.to_vec();
        DOWN.fetch_add(b.len() as u64, Relaxed);
        Ok(b)
    }

    pub async fn del(&self, msg_id: i64) -> bool { self.call("deleteMessage", json!({"chat_id": self.channel, "message_id": msg_id})).await.is_ok() }

    /// Ko'p xabarni bitta so'rovda o'chiradi (Bot API: 100 tagacha). Xato bo'lsa birma-bir. EN: batch delete, 100 per call.
    pub async fn del_many(&self, ids: &[i64]) {
        for chunk in ids.chunks(100) {
            if chunk.is_empty() { continue; }
            if self.call("deleteMessages", json!({"chat_id": self.channel, "message_ids": chunk})).await.is_err() {
                for m in chunk { let _ = self.del(*m).await; }
            }
        }
    }

    pub async fn pin(&self, msg_id: i64) -> bool { self.call("pinChatMessage", json!({"chat_id": self.channel, "message_id": msg_id, "disable_notification": true})).await.is_ok() }
    /// Kanalning pin qilingan hujjati: `(msg_id, file_id)`. EN/RU: the pinned document of the channel.
    pub async fn get_pinned(&self) -> Result<Option<(i64, String)>> {
        let c = self.call("getChat", json!({"chat_id": self.channel})).await?;
        let m = &c["pinned_message"];
        Ok(m["document"]["file_id"].as_str().map(|f| (m["message_id"].as_i64().unwrap_or(0), f.to_string())))
    }
}
