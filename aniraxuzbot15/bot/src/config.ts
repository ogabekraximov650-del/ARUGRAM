export const config = {
  BOT_TOKEN: process.env.BOT_TOKEN ?? "",
  ADMIN_ID: Number(process.env.ADMIN_ID ?? "6805215964"),
  SAVED_CHANNEL_ID: process.env.SAVED_CHANNEL_ID ?? "-1002889216626",
  PORT: Number(process.env.PORT ?? "5000"),
  WEBHOOK_PATH: process.env.WEBHOOK_PATH ?? "/webhook",
  WORKER_URL: process.env.WORKER_URL ?? "https://aniraxuzbot.ogabekraximov650.workers.dev",
  // Kanalga post joylash vaqtincha O'CHIRILGAN (yoqish: CHANNEL_POSTING=1) / posting to channels is temporarily OFF
  CHANNEL_POSTING: process.env.CHANNEL_POSTING === "1",
  TEZCHECK_TOKEN: process.env.TEZCHECK_TOKEN ?? "",
  TEZCHECK_DESK: process.env.TEZCHECK_DESK ?? "",
  TEZCHECK_API_KEY: process.env.TEZCHECK_API_KEY ?? "",
  TEZCHECK_PAY_URL: process.env.TEZCHECK_PAY_URL ?? "https://tezcheck.uz/merchant/pay?token=",
};
