/**
 * Cloudflare Workers entry point.
 * Ma'lumotlar bazasi: Turso (bitta baza); db_d1.ts ga qarang
 *
 * TUZATMALAR:
 *  1. ensureReady() — parallel so'rovlarda xavfsiz (race-condition yo'q)
 *  2. Bot + DB init xatolari qayta urinib ko'riladi
 *  3. handleUpdate tezlashtirilgan — javob darhol "ok" qaytaradi
 */
import { Hono } from "hono";
import { createBot } from "./bot.ts";
import { createTable } from "./db.ts";
import { config } from "./config.ts";
import { processPostQueue } from "./queue_processor.ts";
import { aniblaPush, aniblaRunFailed, aniblaTick } from "./handlers/anibla_add.ts";
import { d1Run, d1Update } from "./db_d1.ts";
import { initD1Bindings, ensureD1Tables, type D1Env } from "./db_d1.ts";
import { createSessionTable } from "./db_sessions.ts";

type CfEnv = D1Env & Record<string, string | undefined>;

function applyEnv(env: CfEnv) {
  if (env.BOT_TOKEN)          config.BOT_TOKEN          = env.BOT_TOKEN as string;
  if (env.ADMIN_ID)           config.ADMIN_ID           = Number(env.ADMIN_ID);
  if (env.SAVED_CHANNEL_ID)   config.SAVED_CHANNEL_ID   = env.SAVED_CHANNEL_ID as string;
  if (env.WORKER_URL)         config.WORKER_URL         = env.WORKER_URL as string;
  if (env.CHANNEL_POSTING)    config.CHANNEL_POSTING    = env.CHANNEL_POSTING === "1";
  if (env.ANIBLA_CREDS)       config.ANIBLA_CREDS       = env.ANIBLA_CREDS as string;
  if (env.GH_ACTIONS_TOKEN)   config.GH_ACTIONS_TOKEN   = env.GH_ACTIONS_TOKEN as string;
  if (env.GH_REPO)            config.GH_REPO            = env.GH_REPO as string;
  if (env.TEZCHECK_TOKEN)     config.TEZCHECK_TOKEN     = env.TEZCHECK_TOKEN as string;
  if (env.TEZCHECK_DESK)      config.TEZCHECK_DESK      = env.TEZCHECK_DESK as string;
  if (env.TEZCHECK_API_KEY)   config.TEZCHECK_API_KEY   = env.TEZCHECK_API_KEY as string;

  // D1 bindinglari — modul darajasida saqlanadi
  initD1Bindings(env);
}

const app = new Hono();

let bot: ReturnType<typeof createBot> | null = null;
let _botInitPromise: Promise<void> | null = null;
let _dbReady = false;
let _dbInitPromise: Promise<void> | null = null;

/**
 * Bot + DB ni ishga tushiradi.
 * DB init BIRINCHI safar AWAIT qilinadi — jadvallar yo'q bo'lganda xavfsizlik.
 */
async function ensureReady(): Promise<void> {
  // ── 1. Bot init ──────────────────────────────────────────────────────────────
  if (!_botInitPromise) {
    bot = createBot();
    _botInitPromise = bot.init().catch((e) => {
      console.error("Bot init xatosi, qayta uriniladi:", e);
      _botInitPromise = null;
      bot = null;
      throw e;
    });
  }
  await _botInitPromise;

  // ── 2. DB init — birinchi safar await, keyin background ──────────────────────
  if (!_dbReady) {
    if (!_dbInitPromise) {
      _dbInitPromise = Promise.all([
        ensureD1Tables(),
        createTable(),
      ])
        .then(() => {
          console.log("D1 jadvallar tayyor.");
          _dbReady = true;
        })
        .catch((e) => {
          console.error("DB init xatosi:", e);
          _dbInitPromise = null;
          throw e;
        });
    }
    await _dbInitPromise.catch((e) => {
      console.error("DB init kutilmadi (bot ishlashda davom etadi):", e);
    });
  }
}

// ── Sog'liq tekshiruvi ──────────────────────────────────────────────────────
app.get("/health", (c) => c.json({ status: "ok", bot: "@aniraxuzbot", db: "D1 x3" }));

// ── Anibla Actions jonli holati (bot_download.py → "📊 Holat") ───────────────
app.post("/anibla/progress", async (c) => {
  let b: { job?: number; token?: string; text?: string };
  try {
    b = await c.req.json();
  } catch {
    return c.json({ ok: false }, 400);
  }
  const job = Number(b.job);
  const token = String(b.token ?? "");
  if (!job || !/^[a-f0-9]{32}$/.test(token)) return c.json({ ok: false }, 400);
  try {
    await ensureReady();
    const rows = await d1Run<{ admin_chat: number }>(`SELECT admin_chat FROM anibla_jobs WHERE id = ? AND token = ?`, [job, token]);
    if (rows.length === 0) return c.json({ ok: false }, 403);
    await d1Update(
      `INSERT INTO anibla_progress (job_id, text, updated_at) VALUES (?, ?, ?)
       ON CONFLICT(job_id) DO UPDATE SET text = excluded.text, updated_at = excluded.updated_at`,
      [job, String(b.text ?? "").slice(0, 3500), Date.now()]
    );
    // Ochiq "📊 Holat" xabari darhol yangilanadi; Telegram cheklasa runner `retry_after` soniya kutadi.
    const retry = await aniblaPush(bot!.api, Number(rows[0].admin_chat)).catch(() => 0);
    return c.json(retry ? { ok: false, retry_after: retry } : { ok: true });
  } catch (e) {
    console.error("anibla/progress:", e);
    return c.json({ ok: false }, 500);
  }
});

// ── Actions xatosi (bot_download.py / workflow): ish xatoga o'tadi, shu sifat navbati davom etadi ──
app.post("/anibla/fail", async (c) => {
  let b: { job?: number; token?: string; error?: string };
  try {
    b = await c.req.json();
  } catch {
    return c.json({ ok: false }, 400);
  }
  const job = Number(b.job);
  const token = String(b.token ?? "");
  if (!job || !/^[a-f0-9]{32}$/.test(token)) return c.json({ ok: false }, 400);
  try {
    await ensureReady();
    const done = await aniblaRunFailed(bot!.api, job, token, String(b.error ?? "Actions xatosi").slice(0, 300));
    return c.json({ ok: done });
  } catch (e) {
    console.error("anibla/fail:", e);
    return c.json({ ok: false }, 500);
  }
});

// ── Webhook o'rnatish ───────────────────────────────────────────────────────
app.get("/setup", async (c) => {
  try {
    await ensureReady();
    const url = new URL(c.req.url);
    const webhookUrl = `${url.protocol}//${url.host}${config.WEBHOOK_PATH}`;
    await bot!.api.setWebhook(webhookUrl, {
      drop_pending_updates: true,
      max_connections: 100,
      allowed_updates: [
        "message", "edited_message", "callback_query",
        "chat_join_request", "chat_member", "my_chat_member",
        "inline_query", "chosen_inline_result",
        "pre_checkout_query",
      ],
    });
    return c.json({ ok: true, webhook: webhookUrl });
  } catch (e) {
    return c.json({ ok: false, error: String(e) }, 500);
  }
});

// ── Telegram webhook ─────────────────────────────────────────────────────────
app.post(config.WEBHOOK_PATH, async (c) => {
  let update: unknown;
  try {
    update = await c.req.json();
  } catch {
    return c.text("bad json", 400);
  }

  c.executionCtx.waitUntil(
    ensureReady()
      .then(() => bot!.handleUpdate(update as any))
      .catch((e) => console.error("handleUpdate xatosi:", e))
  );

  return c.text("ok");
});

// ── Cloudflare Worker export ──────────────────────────────────────────────────
export default {
  fetch(request: Request, env: CfEnv, ctx: ExecutionContext) {
    applyEnv(env);
    return app.fetch(request, env as any, ctx);
  },

  async scheduled(
    _event: { scheduledTime: number; cron: string },
    env: CfEnv,
    ctx: { waitUntil(p: Promise<unknown>): void }
  ) {
    applyEnv(env);
    if (!bot) {
      bot = createBot();
      await bot.init().catch(() => {});
    }
    ctx.waitUntil(
      processPostQueue().catch((e) => console.error("processPostQueue:", e))
    );
    // Soatda bir marta: 30 kundan eski sessiyalar tozalanadi.
    if (new Date().getUTCMinutes() === 0) {
      ctx.waitUntil(createSessionTable().catch((e) => console.error("createSessionTable:", e)));
    }
    // Ochiq "📊 Holat" xabarlari (Anibla) — har daqiqada yangilanadi.
    ctx.waitUntil(
      aniblaTick(bot.api).catch((e) => console.error("aniblaTick:", e))
    );
  },
};
