import { initD1Bindings } from "./db_d1.ts";
import { serve } from "@hono/node-server";
import { Hono } from "hono";
import { webhookCallback } from "grammy";
import { config } from "./config.ts";
import { createBot } from "./bot.ts";
import { createTable } from "./db.ts";
import { createSessionTable } from "./db_sessions.ts";
import { createChannelTables } from "./db_channels.ts";
import { createUserTable } from "./db_users.ts";
import { createPostQueueTable } from "./db_post_queue.ts";

function getWebhookUrl(): string {
  const domains = process.env.REPLIT_DOMAINS;
  if (domains) {
    const domain = domains.split(",")[0].trim();
    return `https://${domain}${config.WEBHOOK_PATH}`;
  }
  return `${config.WORKER_URL}${config.WEBHOOK_PATH}`;
}

const app = new Hono();
const bot = createBot();

app.get("/health", (c) => {
  return c.json({ status: "ok", bot: "@aniraxuzbot", webhook: getWebhookUrl() });
});

app.post(config.WEBHOOK_PATH, webhookCallback(bot, "hono"));

async function main() {
  // Turso kalitlari (Node rejimi): TURSO_URL / TURSO_TOKEN muhit o'zgaruvchilaridan.
  initD1Bindings(process.env as { TURSO_URL?: string; TURSO_TOKEN?: string });
  // D1 jadvallarini yaratish (ensureD1Tables createTable ichida chaqiriladi)
  await createTable();
  await createSessionTable();
  await createChannelTables();
  await createUserTable();
  await createPostQueueTable();

  await bot.init();

  const webhookUrl = getWebhookUrl();
  try {
    await bot.api.setWebhook(webhookUrl, {
      drop_pending_updates: true,
      allowed_updates: [
        "message",
        "edited_message",
        "callback_query",
        "chat_join_request",
        "chat_member",
        "my_chat_member",
        "inline_query",
        "chosen_inline_result",
        "pre_checkout_query",
      ],
    });
    console.log(`Webhook o'rnatildi: ${webhookUrl}`);
  } catch (e) {
    console.error("Webhook o'rnatishda xato:", e);
  }

  serve({ fetch: app.fetch, port: config.PORT }, (info) => {
    console.log(`Bot serveri ishga tushdi: http://localhost:${info.port}`);
  });
}

main().catch(console.error);
