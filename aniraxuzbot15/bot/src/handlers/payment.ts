/**
 * To'lov tizimi:
 * - Balans to'ldirish (admin orqali, avto to'lov tezcheck.uz)
 * - Hisobim (balans ko'rish)
 */
import { Bot, InlineKeyboard } from "grammy";
import type { MyContext } from "./admin.ts";
import { getUser, updateUserBalance } from "../db_users.ts";
import { config } from "../config.ts";

function styleAll(kb: InlineKeyboard): InlineKeyboard {
  for (const row of kb.inline_keyboard)
    for (const btn of row) (btn as any).style = "primary";
  return kb;
}

// ─── TezCheck API ──────────────────────────────────────────────────────────────

interface TezCheckCreateResponse {
  ok: boolean;
  order_id?: number;
  pay_url?: string;
  error?: { message: string };
}

interface TezCheckStatusResponse {
  ok: boolean;
  payment?: { id: string; amount: string; status: "pending" | "paid" | "canceled" };
  error?: { message: string };
}

/** PHP notice bilan kelgan javobdan JSON qismini ajratib oladi */
function extractJson(raw: string): unknown {
  const idx = raw.indexOf("{");
  if (idx === -1) throw new Error("JSON topilmadi");
  return JSON.parse(raw.slice(idx));
}

async function tezCheckCreate(amountSom: number): Promise<TezCheckCreateResponse> {
  try {
    const r = await fetch(`https://tezcheck.uz/api/create_invoice`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ api_key: config.TEZCHECK_API_KEY, amount: amountSom }),
    });
    return extractJson(await r.text()) as TezCheckCreateResponse;
  } catch (e) {
    return { ok: false, error: { message: String(e) } };
  }
}

async function tezCheckStatus(orderId: number): Promise<TezCheckStatusResponse> {
  try {
    const r = await fetch(`https://tezcheck.uz/api/status_invoice`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ api_key: config.TEZCHECK_API_KEY, order_id: String(orderId) }),
    });
    return extractJson(await r.text()) as TezCheckStatusResponse;
  } catch (e) {
    return { ok: false, error: { message: String(e) } };
  }
}

// ─── Admin to'lov matni ────────────────────────────────────────────────────────

function buildAdminPayLink(userId: number, amount: number): string {
  const text = `Salom men botdagi balansimni ${amount} so'mga to'ldirmoqchiman\nId raqam: ${userId}`;
  return `https://t.me/aniraxuzadmin?text=${encodeURIComponent(text)}`;
}

// ─── Hisobim ───────────────────────────────────────────────────────────────────

async function showBalance(ctx: MyContext): Promise<void> {
  const userId = ctx.from!.id;
  const user = await getUser(userId);

  const botId = user?.bot_user_id ? `#${user.bot_user_id}` : "—";
  const phone = user?.user_phone || "—";
  const balance = user?.balance ?? 0;
  const animeBalance = user?.anime_balance ?? 0;

  await ctx.reply(
    `💰 <b>Hisobingiz</b>\n\n` +
    `🆔 Bot ID: <code>${botId}</code>\n` +
    `📱 Telegram ID: <code>${userId}</code>\n` +
    `📞 Telefon: <code>${phone}</code>\n` +
    `💵 Asosiy balans: <b>${balance.toLocaleString("uz")} so'm</b>\n` +
    `🎌 Anime balans: <b>${animeBalance.toLocaleString("uz")} so'm</b>`,
    { parse_mode: "HTML" }
  );
}

// ─── Balans to'ldirish menyusi ─────────────────────────────────────────────────

async function showTopUpMenu(ctx: MyContext): Promise<void> {
  const kb = styleAll(
    new InlineKeyboard()
      .text("👤 Admin orqali to'lov", "pay:method:admin").row()
      .text("⚡ Avto to'lov", "pay:method:auto")
  );

  const text = "💳 Quyidagi to'lov usullaridan birini tanlang:";
  if (ctx.callbackQuery) {
    await ctx.editMessageText(text, { reply_markup: kb })
      .catch(() => ctx.reply(text, { reply_markup: kb }));
    await ctx.answerCallbackQuery();
  } else {
    await ctx.reply(text, { reply_markup: kb });
  }
}

// ─── Miqdor so'rash ────────────────────────────────────────────────────────────

async function askAutoAmount(ctx: MyContext, edit = false): Promise<void> {
  ctx.session.state = "waiting_pay_amount_auto";
  const kb = styleAll(new InlineKeyboard().text("❌ Bekor qilish", "pay:cancel"));
  const text =
    "⚡ <b>Avto to'lov</b>\n\n" +
    "Hisobingizni qancha so'mga to'ldirmoqchisiz?\n" +
    "(Faqat son kiriting, masalan: <code>10000</code>)\n\n" +
    "💳 To'lov usullari: Payme, Click, karta va boshqalar.";

  if (edit && ctx.callbackQuery) {
    await ctx.editMessageText(text, { parse_mode: "HTML", reply_markup: kb })
      .catch(() => ctx.reply(text, { parse_mode: "HTML", reply_markup: kb }));
  } else {
    await ctx.reply(text, { parse_mode: "HTML", reply_markup: kb });
  }
}

// ─── To'lov holati tekshiruvi (polling) ───────────────────────────────────────

async function pollPaymentStatus(
  ctx: MyContext,
  orderId: number,
  amount: number,
  userId: number
): Promise<void> {
  const MAX_ATTEMPTS = 30; // 5 daqiqa
  const INTERVAL_MS = 10_000;

  for (let i = 0; i < MAX_ATTEMPTS; i++) {
    await new Promise(r => setTimeout(r, INTERVAL_MS));
    const res = await tezCheckStatus(orderId);

    if (res.ok && res.payment?.status === "paid") {
      await updateUserBalance(userId, amount);
      const user = await getUser(userId);
      await ctx.reply(
        `✅ <b>To'lov qabul qilindi!</b>\n\n` +
        `Qo'shildi: <b>${amount.toLocaleString("uz")} so'm</b>\n` +
        `Joriy balans: <b>${(user?.balance ?? 0).toLocaleString("uz")} so'm</b>`,
        { parse_mode: "HTML" }
      ).catch(() => {});
      return;
    }

    if (res.ok && res.payment?.status === "canceled") {
      await ctx.reply("❌ To'lov bekor qilindi.").catch(() => {});
      return;
    }
  }
  // Timeout
  await ctx.reply(
    `⚠️ To'lov vaqti tugadi. Agar to'lagan bo'lsangiz adminga murojaat qiling.\n` +
    `Buyurtma ID: <code>${orderId}</code>`,
    { parse_mode: "HTML" }
  ).catch(() => {});
}

// ─── setupPayment ──────────────────────────────────────────────────────────────

export function setupPayment(bot: Bot<MyContext>): void {

  bot.hears("💳 Hisobim", showBalance);
  bot.hears("💰 Balans to'ldirish", showTopUpMenu);

  // Balans to'ldirish menyusiga qaytish (VIP sahifasidan)
  bot.callbackQuery("pay:topup", async (ctx) => {
    await ctx.answerCallbackQuery();
    try { await ctx.deleteMessage(); } catch {}
    await showTopUpMenu(ctx);
  });

  // ── Admin orqali to'lov ───────────────────────────────────────────────────
  bot.callbackQuery("pay:method:admin", async (ctx) => {
    await ctx.answerCallbackQuery();
    ctx.session.state = "waiting_pay_amount_admin";
    const kb = styleAll(new InlineKeyboard().text("❌ Bekor qilish", "pay:cancel"));
    await ctx.editMessageText(
      "👤 <b>Admin orqali to'lov</b>\n\n" +
      "Hisobingizni qancha so'mga to'ldirmoqchisiz?\n" +
      "(Faqat son kiriting, masalan: <code>10000</code>)",
      { parse_mode: "HTML", reply_markup: kb }
    ).catch(() =>
      ctx.reply(
        "Hisobingizni qancha so'mga to'ldirmoqchisiz? (Faqat son kiriting):",
        { reply_markup: kb }
      )
    );
  });

  // ── Avto to'lov ───────────────────────────────────────────────────────────
  bot.callbackQuery("pay:method:auto", async (ctx) => {
    await ctx.answerCallbackQuery();
    await askAutoAmount(ctx, true);
  });

  // ── Bekor qilish ──────────────────────────────────────────────────────────
  bot.callbackQuery("pay:cancel", async (ctx) => {
    ctx.session.state = "idle";
    await ctx.answerCallbackQuery("Bekor qilindi");
    try { await ctx.deleteMessage(); } catch {}
  });

  // ── To'lovni tekshirish tugmasi ────────────────────────────────────────────
  bot.callbackQuery(/^pay:check:(\d+):(\d+)$/, async (ctx) => {
    const orderId = Number(ctx.match[1]);
    const amount  = Number(ctx.match[2]);
    const userId  = ctx.from!.id;
    await ctx.answerCallbackQuery("Tekshirilmoqda...");

    const res = await tezCheckStatus(orderId);
    if (res.ok && res.payment?.status === "paid") {
      await updateUserBalance(userId, amount);
      const user = await getUser(userId);
      try { await ctx.deleteMessage(); } catch {}
      await ctx.reply(
        `✅ <b>To'lov qabul qilindi!</b>\n\n` +
        `Qo'shildi: <b>${amount.toLocaleString("uz")} so'm</b>\n` +
        `Joriy balans: <b>${(user?.balance ?? 0).toLocaleString("uz")} so'm</b>`,
        { parse_mode: "HTML" }
      );
    } else if (res.ok && res.payment?.status === "canceled") {
      try { await ctx.deleteMessage(); } catch {}
      await ctx.reply("❌ To'lov bekor qilindi.");
    } else {
      // Hali to'lanmagan
      await ctx.answerCallbackQuery("⏳ To'lov hali amalga oshmagan").catch(() => {});
    }
  });

  // ── Matn orqali miqdor kiritish ────────────────────────────────────────────
  bot.on("message:text", async (ctx, next) => {
    const state = ctx.session.state;
    const userId = ctx.from!.id;

    // Admin orqali to'lov — miqdor kiritildi
    if (state === "waiting_pay_amount_admin") {
      const amount = parseInt(ctx.message.text.trim(), 10);
      ctx.session.state = "idle";
      if (isNaN(amount) || amount < 1000) {
        await ctx.reply("❌ Noto'g'ri summa. Kamida 1 000 so'm kiriting.");
        return;
      }
      const link = buildAdminPayLink(userId, amount);
      const kb = styleAll(new InlineKeyboard().url("👤 Adminga yozish", link));
      await ctx.reply(
        `👤 <b>Admin orqali to'lov</b>\n\n` +
        `Summa: <b>${amount.toLocaleString("uz")} so'm</b>\n\n` +
        `Quyidagi tugmani bosib adminga xabar yuboring:`,
        { parse_mode: "HTML", reply_markup: kb }
      );
      return;
    }

    // Avto to'lov — miqdor kiritildi
    if (state === "waiting_pay_amount_auto") {
      const amount = parseInt(ctx.message.text.trim(), 10);
      ctx.session.state = "idle";
      if (isNaN(amount) || amount < 1000) {
        await ctx.reply("❌ Noto'g'ri summa. Kamida 1 000 so'm kiriting.");
        return;
      }

      const res = await tezCheckCreate(amount);
      if (!res.ok || !res.pay_url || !res.order_id) {
        await ctx.reply(
          `❌ To'lov yaratishda xato: ${res.error?.message ?? "Noma'lum xato"}\n` +
          `Qaytadan urinib ko'ring yoki admin orqali to'lang.`
        );
        return;
      }

      const kb = styleAll(
        new InlineKeyboard()
          .url("💳 To'lash", res.pay_url).row()
          .text("✅ To'lovni tekshirish", `pay:check:${res.order_id}:${amount}`)
      );

      await ctx.reply(
        `⚡ <b>Avto to'lov</b>\n\n` +
        `Summa: <b>${amount.toLocaleString("uz")} so'm</b>\n` +
        `Buyurtma ID: <code>${res.order_id}</code>\n\n` +
        `To'lov linkini bosing va to'lovni amalga oshiring.\n` +
        `To'lovdan so'ng <b>"To'lovni tekshirish"</b> tugmasini bosing:`,
        { parse_mode: "HTML", reply_markup: kb }
      );
      return;
    }

    return next();
  });
}
