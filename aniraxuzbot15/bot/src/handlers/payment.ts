/**
 * To'lov tizimi:
 * - Balans to'ldirish (admin orqali, avto to'lov tezcheck.uz)
 * - Hisobim (balans ko'rish)
 */
import { Bot, InlineKeyboard } from "grammy";
import type { MyContext } from "./admin.ts";
import { getUser, updateUserBalance } from "../db_users.ts";
import { d1Run, d1Batch, d1InsertReturningId } from "../db_d1.ts";
import { createBill, getBill, toMinor, tezcheckReady, PAY_MIN, PAY_MAX, PAY_LINK_TTL_MS } from "../tezcheck.ts";

function styleAll(kb: InlineKeyboard): InlineKeyboard {
  for (const row of kb.inline_keyboard)
    for (const btn of row) (btn as any).style = "primary";
  return kb;
}

// ─── To'lovni hisobga o'tkazish (ilovadagi `credit_payment` kabi: BIR MARTA) ────
//
// To'lov haqiqatan bo'lganini FAQAT tezcheck.uz tasdiqlaydi (summa ham solishtiriladi). Balans va holat bitta
// tranzaksiyada (D1 batch) o'zgaradi: avval `pending` bo'lsagina balans oshadi, keyin `paid` qilinadi — "Tekshirish"
// tugmasi necha marta bosilsa ham pul BIR marta qo'shiladi.

type Settle =
  | { status: "paid"; amount: number }
  | { status: "pending" }
  | { status: "missing" }
  | { status: "error"; message: string };

async function settlePayment(id: number, userId: number): Promise<Settle> {
  const rows = await d1Run<{ order_id: string; amount: number; status: string }>(
    `SELECT order_id, amount, status FROM bot_payments WHERE id = ? AND user_id = ?`,
    [id, userId]
  );
  const pay = rows[0];
  if (!pay) return { status: "missing" };
  if (pay.status === "paid") return { status: "paid", amount: pay.amount };

  let bill;
  try {
    bill = await getBill(pay.order_id);
  } catch (e) {
    return { status: "error", message: e instanceof Error ? e.message : String(e) };
  }
  if (!bill.paid) return { status: "pending" };
  if (bill.amountMinor !== toMinor(pay.amount)) {
    return { status: "error", message: "To'lov summasi mos kelmadi — adminga murojaat qiling" };
  }
  await d1Batch([
    {
      sql: `UPDATE user_db SET balance = balance + ?
             WHERE user_id = ? AND (SELECT status FROM bot_payments WHERE id = ?) = 'pending'`,
      params: [pay.amount, userId, id],
    },
    {
      sql: `UPDATE bot_payments SET status = 'paid' WHERE id = ? AND status = 'pending'`,
      params: [id],
    },
  ]);
  return { status: "paid", amount: pay.amount };
}

async function replyPaid(ctx: MyContext, userId: number, amount: number): Promise<void> {
  const user = await getUser(userId);
  await ctx.reply(
    `✅ <b>To'lov qabul qilindi!</b>\n\n` +
    `Summa: <b>${amount.toLocaleString("uz")} so'm</b>\n` +
    `Joriy balans: <b>${(user?.balance ?? 0).toLocaleString("uz")} so'm</b>`,
    { parse_mode: "HTML" }
  );
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

  // ── Eski (tezchek.uz) tugmalari: yangi tizimga o'tilgan ─────────────────────
  bot.callbackQuery(/^pay:check:\d+:\d+$/, async (ctx) => {
    await ctx.answerCallbackQuery({
      text: "Bu eski to'lov tugmasi. Yangi to'lov yarating yoki adminga murojaat qiling.",
      show_alert: true,
    });
  });

  // ── To'lovni tekshirish tugmasi ────────────────────────────────────────────
  bot.callbackQuery(/^pay:check:(\d+)$/, async (ctx) => {
    const id = Number(ctx.match[1]);
    const userId = ctx.from!.id;
    const res = await settlePayment(id, userId);

    if (res.status === "paid") {
      await ctx.answerCallbackQuery("✅ To'lov qabul qilindi");
      try { await ctx.deleteMessage(); } catch {}
      await replyPaid(ctx, userId, res.amount);
    } else if (res.status === "pending") {
      await ctx.answerCallbackQuery("⏳ To'lov hali amalga oshmagan");
    } else if (res.status === "missing") {
      await ctx.answerCallbackQuery({ text: "To'lov topilmadi", show_alert: true });
    } else {
      await ctx.answerCallbackQuery({ text: `Tekshirib bo'lmadi: ${res.message}`.slice(0, 190), show_alert: true });
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
      if (isNaN(amount) || amount < PAY_MIN) {
        await ctx.reply("❌ Noto'g'ri summa. Kamida 1 000 so'm kiriting.");
        return;
      }

      if (!tezcheckReady()) {
        await ctx.reply("❌ Avto to'lov hozircha sozlanmagan. Admin orqali to'lang.");
        return;
      }
      if (amount > PAY_MAX) {
        await ctx.reply(`❌ Summa ${PAY_MAX.toLocaleString("uz")} so'mdan oshmasligi kerak.`);
        return;
      }

      let bill;
      try {
        bill = await createBill(amount, userId);
      } catch (e) {
        await ctx.reply(
          `❌ To'lov yaratishda xato: ${e instanceof Error ? e.message : "Noma'lum xato"}\n` +
          `Qaytadan urinib ko'ring yoki admin orqali to'lang.`
        );
        return;
      }

      const now = Date.now();
      const payId = await d1InsertReturningId(
        `INSERT INTO bot_payments (order_id, user_id, amount, status, pay_url, created_at, expires_at)
         VALUES (?, ?, ?, 'pending', ?, ?, ?)`,
        [bill.id, userId, amount, bill.url, now, now + PAY_LINK_TTL_MS]
      );

      const kb = styleAll(
        new InlineKeyboard()
          .url("💳 To'lash", bill.url).row()
          .text("✅ To'lovni tekshirish", `pay:check:${payId}`)
      );

      await ctx.reply(
        `⚡ <b>Avto to'lov</b>\n\n` +
        `Summa: <b>${amount.toLocaleString("uz")} so'm</b>\n` +
        `Buyurtma: <code>#${payId}</code>\n\n` +
        `To'lov havolasi 1 soat faol. Havolani bosib to'lang, so'ng\n` +
        `<b>"To'lovni tekshirish"</b> tugmasini bosing:`,
        { parse_mode: "HTML", reply_markup: kb }
      );
      return;
    }

    return next();
  });
}
