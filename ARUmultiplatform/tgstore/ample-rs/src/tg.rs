//! UZ: Telegram bot API (fayl yuklash/olish/o'chirish, pin). 429 va 5xx da qayta uriniladi. EN/RU: Telegram bot API client with retries.
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::time::Duration;

#[derive(Clone)]
pub struct Tg { client: reqwest::Client, token: String, channel: String }

impl Tg {
    pub fn new(token: String, channel: String) -> Self { Self { client: reqwest::Client::builder().timeout(Duration::from_secs(120)).build().unwrap(), token, channel } }
    fn url(&self, m: &str) -> String { format!("https://api.telegram.org/bot{}/{}", self.token, m) }

    async fn call(&self, method: &str, body: Value) -> Result<Value> {
        for a in 0..6u64 {
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

    pub async fn get(&self, file_id: &str) -> Result<Vec<u8>> {
        let f = self.call("getFile", json!({"file_id": file_id})).await?;
        let path = f["file_path"].as_str().ok_or_else(|| anyhow!("telegram fayl yo'lini bermadi"))?;
        let r = self.client.get(format!("https://api.telegram.org/file/bot{}/{}", self.token, path)).send().await?;
        if !r.status().is_success() { return Err(anyhow!("telegram yuklash {}", r.status())); }
        Ok(r.bytes().await?.to_vec())
    }

    pub async fn del(&self, msg_id: i64) -> bool { self.call("deleteMessage", json!({"chat_id": self.channel, "message_id": msg_id})).await.is_ok() }
    pub async fn pin(&self, msg_id: i64) -> bool { self.call("pinChatMessage", json!({"chat_id": self.channel, "message_id": msg_id, "disable_notification": true})).await.is_ok() }
    /// Kanalning pin qilingan hujjati: `(msg_id, file_id)`. EN/RU: the pinned document of the channel.
    pub async fn get_pinned(&self) -> Result<Option<(i64, String)>> {
        let c = self.call("getChat", json!({"chat_id": self.channel})).await?;
        let m = &c["pinned_message"];
        Ok(m["document"]["file_id"].as_str().map(|f| (m["message_id"].as_i64().unwrap_or(0), f.to_string())))
    }
}
