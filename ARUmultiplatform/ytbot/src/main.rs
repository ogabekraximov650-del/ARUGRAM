// UZ: yt-dlp Telegram boti. Bot (token) — suhbat va navbat; Telegram AKKAUNT (MTProto, grammers) — katta fayllarni (2 GB gacha) yuklaydi.
//     So'rovlar libsql bazadagi navbatda turadi va BIRIN-KETIN bajariladi. Fayl hajmi oldindan aniq bo'lsa video diskka yozilmasdan oqim bilan yuboriladi;
//     video+audio birlashtirilsa (hajm noma'lum) vaqtincha diskka yoziladi va yuborilgach darhol o'chiriladi.
// EN: yt-dlp Telegram bot. The bot (token) handles chat and the queue; a Telegram ACCOUNT (MTProto, grammers) uploads large files (up to 2 GB).
//     Requests wait in a libsql queue and run ONE BY ONE. When the exact size is known the video is streamed without touching the disk;
//     merged video+audio (unknown size) goes to a temp file that is deleted right after the upload.
mod bot;
mod db;
mod dl;

use anyhow::Result;
use std::sync::Arc;

pub struct Cfg {
    pub bot_token: String,
    pub api_id: i32,
    pub api_hash: String,
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
        api_id: env("API_ID").and_then(|v| v.parse().ok()).ok_or_else(|| anyhow::anyhow!("API_ID yo'q"))?,
        api_hash: env("API_HASH").ok_or_else(|| anyhow::anyhow!("API_HASH yo'q"))?,
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
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(|a| a == "sendcode").unwrap_or(false) {
        let phone = args.get(2).cloned().unwrap_or_default();
        return bot::cli_sendcode(cfg, &phone, args.get(3).map(|a| a == "resend").unwrap_or(false)).await;
    }
    eprintln!("ytbot ishga tushdi");
    bot::run(cfg).await
}
