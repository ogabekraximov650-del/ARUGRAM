import { Bot, InlineKeyboard, Keyboard } from "grammy";
import type { MyContext } from "./admin.ts";
import {
  getKonstruktorUser,
  setKonstruktorBot,
  getKonstruktorByBotId,
  clearKonstruktorBot,
  upsertKonstruktorUser,
} from "../db_konstruktor.ts";
import { getUser } from "../db_users.ts";
import { evictChildBot } from "../child_bot.ts";
import { isReservedText } from "./known_buttons.ts";
import { config } from "../config.ts";

const flowState = new Map<number, "waiting_bot_token">();

const BTN_BOT_MANAGE = "🤖 Botlarni boshqarish";
const BTN_CREATE = "🛠 Anime bot yaratish";
const BTN_MY_BOTS = "📋 Mening botlarim";
const BTN_BACK = "⟨ Orqaga";

function styleAll(kb: InlineKeyboard): InlineKeyboard {
  for (const row of kb.inline_keyboard) {
    for (const btn of row) (btn as any).style = "primary";
  }
  return kb;
}

/** TOP-LEVEL menyu — "🔍 Anime izlash" yuqorida, "🤖 Botlarni boshqarish" pastda. */
function topMenuKeyboard(): Keyboard {
  return new Keyboard()
    .text("🔍 Anime izlash")
    .row()
    .text(BTN_BOT_MANAGE)
    .resized();
}

/** SUB menyu — bot yaratish/ro'yxat + Orqaga. */
function botManageKeyboard(): Keyboard {
  return new Keyboard()
    .text(BTN_CREATE)
    .row()
    .text(BTN_MY_BOTS)
    .row()
    .text(BTN_BACK)
    .resized();
}

/** Ommaviy — main bot onboarding tomonidan onComplete sifatida ishlatiladi. */
export async function sendKonstruktorMenu(
  ctx: MyContext,
  text = "📋 Asosiy menyu"
): Promise<void> {
  await ctx.reply(text, { reply_markup: topMenuKeyboard() });
}

export function setupKonstruktor(bot: Bot<MyContext>): void {
  bot.command("cancel", async (ctx, next) => {
    if (ctx.from && flowState.has(ctx.from.id)) {
      flowState.delete(ctx.from.id);
      await ctx.reply("✅ Bot yaratish bekor qilindi.");
      await sendKonstruktorMenu(ctx);
      return;
    }
    await next();
  });

  // 🤖 Botlarni boshqarish — sub menyuga tushish
  bot.hears(BTN_BOT_MANAGE, async (ctx) => {
    if (!ctx.from) return;
    flowState.delete(ctx.from.id);
    await ctx.reply("🤖 <b>Botlarni boshqarish</b>", {
      parse_mode: "HTML",
      reply_markup: botManageKeyboard(),
    });
  });

  // ⟨ Orqaga — top menyuga qaytish
  bot.hears(BTN_BACK, async (ctx) => {
    if (!ctx.from) return;
    flowState.delete(ctx.from.id);
    await sendKonstruktorMenu(ctx);
  });

  bot.hears(BTN_CREATE, async (ctx) => {
    if (!ctx.from) return;
    flowState.set(ctx.from.id, "waiting_bot_token");
    await ctx.reply(
      "🤖 Yangi anime botingizni yaratish uchun:\n\n" +
        "1) Telegramda @BotFather ga kiring\n" +
        "2) /newbot buyrugʻini yuboring\n" +
        "3) Bot nomi va usernameni belgilang\n" +
        "4) BotFather bergan TOKEN ni shu yerga yuboring\n\n" +
        "Bekor qilish uchun /cancel."
    );
  });

  bot.hears(BTN_MY_BOTS, async (ctx) => {
    if (!ctx.from) return;
    flowState.delete(ctx.from.id);

    const user = await getKonstruktorUser(ctx.from.id);
    if (!user || !user.bot_id || !user.bot_token) {
      await ctx.reply("📭 Siz hali hech qanday bot yaratmagansiz.");
      return;
    }

    let username = "";
    let firstName = "";
    try {
      const res = await fetch(`https://api.telegram.org/bot${user.bot_token}/getMe`);
      const d = (await res.json()) as {
        ok: boolean;
        result?: { username: string; first_name: string };
      };
      if (d.ok && d.result) {
        username = "@" + d.result.username;
        firstName = d.result.first_name;
      }
    } catch {
      /* ignore */
    }

    const kb = styleAll(
      new InlineKeyboard().text("🗑 Botni oʻchirish", `konst_delete_${user.bot_id}`)
    );
    await ctx.reply(
      `🤖 Sizning botingiz:\n\n` +
        `📛 ${firstName || "—"}\n` +
        `🔗 ${username || "—"}\n` +
        `🆔 ID: ${user.bot_id}\n` +
        `📅 ${user.create_data || "—"}`,
      { reply_markup: kb }
    );
  });

  bot.callbackQuery(/^konst_delete_(\d+)$/, async (ctx) => {
    if (!ctx.from) return;
    const botId = Number(ctx.match![1]);
    const user = await getKonstruktorUser(ctx.from.id);
    if (!user || user.bot_id !== botId) {
      await ctx.answerCallbackQuery({
        text: "❌ Bot topilmadi yoki sizga tegishli emas.",
        show_alert: true,
      });
      return;
    }
    if (user.bot_token) {
      try {
        await fetch(`https://api.telegram.org/bot${user.bot_token}/deleteWebhook`, {
          method: "POST",
        });
      } catch {
        /* ignore */
      }
    }
    evictChildBot(botId);
    await clearKonstruktorBot(ctx.from.id);
    await ctx.answerCallbackQuery({ text: "✅ Oʻchirildi" });
    try {
      await ctx.editMessageText("✅ Bot muvaffaqiyatli oʻchirildi.");
    } catch {
      await ctx.reply("✅ Bot muvaffaqiyatli oʻchirildi.");
    }
  });

  // Token-input handler — faqat waiting_bot_token state'da ishlaydi.
  // MUHIM: agar foydalanuvchi commanda yoki tanigan tugma yuborsa,
  // state tozalanib next() ga o'tadi — shunda boshqa handler javob beradi.
  bot.on("message:text", async (ctx, next) => {
    if (!ctx.from) return next();
    const state = flowState.get(ctx.from.id);
    if (state !== "waiting_bot_token") return next();

    const text = ctx.message.text.trim();

    if (isReservedText(text)) {
      flowState.delete(ctx.from.id);
      return next();
    }

    if (!/^\d+:[A-Za-z0-9_-]{30,}$/.test(text)) {
      await ctx.reply(
        "❌ Token formati notoʻgʻri.\n\nMisol: `123456789:AAH...`\n\nQaytadan yuboring yoki /cancel."
      );
      return;
    }

    try {
      const res = await fetch(`https://api.telegram.org/bot${text}/getMe`);
      const data = (await res.json()) as {
        ok: boolean;
        result?: { id: number; username: string; first_name: string };
      };
      if (!data.ok || !data.result) {
        await ctx.reply("❌ Token Telegram tomonidan rad etildi. Qaytadan yuboring.");
        return;
      }

      const botId = data.result.id;
      const botUsername = data.result.username;

      // Faqat bot yaratgan odamni konstruktor jadvaliga yozamiz
      const dbUser = await getUser(ctx.from.id);
      const phone = dbUser?.user_phone ?? "";
      const fullName =
        [ctx.from.first_name, ctx.from.last_name].filter(Boolean).join(" ") ||
        ctx.from.username ||
        String(ctx.from.id);
      await upsertKonstruktorUser(ctx.from.id, phone, fullName);
      await setKonstruktorBot(ctx.from.id, botId, text);
      flowState.delete(ctx.from.id);

      // Webhook har doim Cloudflare Worker ga o'rnatiladi
      const childWebhookUrl = `${config.WORKER_URL}/child/${botId}`;
      const wh = await fetch(`https://api.telegram.org/bot${text}/setWebhook`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          url: childWebhookUrl,
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
          ],
        }),
      });
      const whData = (await wh.json()) as { ok: boolean; description?: string };
      if (!whData.ok) console.error("Child webhook fail:", whData);

      await ctx.reply(
        `✅ Botingiz muvaffaqiyatli yaratildi!\n\n` +
          `🤖 @${botUsername}\n` +
          `🆔 ID: ${botId}\n\n` +
          `Endi @${botUsername} ga oʻtib /start yuboring.`
      );
      await sendKonstruktorMenu(ctx);
    } catch (e) {
      console.error("Token validate error:", e);
      await ctx.reply("❌ Tokenni tekshirishda xato. Qaytadan urinib koʻring.");
    }
  });
}

export { getKonstruktorByBotId };
