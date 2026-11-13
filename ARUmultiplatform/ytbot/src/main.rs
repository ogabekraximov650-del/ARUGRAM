// UZ: yt-dlp Telegram boti. Faqat BOT (token): suhbat, navbat va yuborish. So'rovlar libsql bazadagi navbatda turadi va BIRIN-KETIN bajariladi.
//     Video yuklanadi, VAQT bo'yicha ~45 MB li qismlarga bo'linadi (ffmpeg, qayta kodlamasdan) va har qism Bot API orqali yuboriladi (bot limiti 50 MB).
// EN: yt-dlp Telegram bot. BOT only (token): chat, queue and delivery. Requests wait in a libsql queue and run ONE BY ONE.
//     The video is downloaded, split by TIME into ~45 MB parts (ffmpeg, no re-encoding) and each part is sent through the Bot API (50 MB bot limit).
mod bot;
mod db;
mod dl;

use anyhow::Result;
use std::sync::Arc;

pub struct Cfg {
    pub bot_token: String,
    pub data_dir: String,
    pub ytdlp: String,
    pub cookies: Option<String>,
    pub owner_env: Option<i64>,
    /// Ample'ning umumiy manzili (https://...ample.run). Berilsa bot WEBHOOK rejimida ishlaydi va `setWebhook` ni o'zi o'rnatadi; aks holda long polling.
    pub public_url: Option<String>,
    /// Webhook so'rovlarini tekshirish siri (manzilda va `X-Telegram-Bot-Api-Secret-Token` sarlavhasida).
    pub webhook_secret: String,
    pub port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let cfg = Arc::new(Cfg {
        bot_token: env("BOT_TOKEN").ok_or_else(|| anyhow::anyhow!("BOT_TOKEN yo'q"))?,
        data_dir: env("DATA_DIR").unwrap_or_else(|| "./data".into()),
        ytdlp: env("YTDLP").unwrap_or_else(|| "yt-dlp".into()),
        cookies: env("COOKIES"),
        owner_env: env("OWNER_ID").and_then(|v| v.parse().ok()),
        public_url: env("PUBLIC_URL").map(|u| u.trim_end_matches('/').to_string()),
        webhook_secret: env("WEBHOOK_SECRET").unwrap_or_default(),
        port: env("PORT").and_then(|v| v.parse().ok()).unwrap_or(8080),
    });
    if cfg.public_url.is_some() && cfg.webhook_secret.len() < 16 {
        return Err(anyhow::anyhow!("WEBHOOK_SECRET kamida 16 belgi bo'lishi kerak (harf, raqam, _ va -)"));
    }
    std::fs::create_dir_all(&cfg.data_dir)?;
    eprintln!("ytbot ishga tushdi");
    bot::run(cfg).await
}
