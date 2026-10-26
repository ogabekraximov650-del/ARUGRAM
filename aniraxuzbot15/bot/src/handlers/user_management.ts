/**
 * Admin uchun foydalanuvchi boshqaruvi:
 * - Qidirish (bot_id, tg_id, phone, @username)
 * - Ro'yxat (oxirgi ro'yxatdan o'tgan / oxirgi foydalangan)
 * - Foydalanuvchi ma'lumotlari va balans tahrirlash
 */
import { Bot, InlineKeyboard } from "grammy";
import type { MyContext } from "./admin.ts";
import { checkIsAdmin } from "../admin_check.ts";
import {
  getUser,
  findUserByIdentifier,
  getUserActivityStats,
  getUsersPaginated,
  updateUserBalance,
  updateAnimeBalance,
  type UserRecord,
} from "../db_users.ts";

const PAGE_SIZE = 10;

function isAdmin(ctx: MyContext): boolean {
  return checkIsAdmin(ctx.from?.id);
}

function styleAll(kb: InlineKeyboard): InlineKeyboard {
  for (const row of kb.inline_keyboard)
    for (const btn of row) (btn as any).style = "primary";
  return kb;
}

/** Sana allaqachon "YYYY-MM-DD HH:MM:SS" formatida saqlanadi — o'zgarishsiz ko'rsatiladi. */
function formatDate(s: string): string {
  return s || "—";
}

// ─── Foydalanuvchi ro'yxati ────────────────────────────────────────────────────

async function showUserList(
  ctx: MyContext,
  page: number,
  listType: "registered" | "active"
): Promise<void> {
  const { users, total } = await getUsersPaginated(page, PAGE_SIZE, listType);
  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE));

  if (total === 0) {
    const kb = styleAll(new InlineKeyboard().text("« Orqaga", "um:list:back"));
    const text = "📭 Hozircha foydalanuvchilar yo'q.";
    await ctx.editMessageText(text, { reply_markup: kb }).catch(() => ctx.reply(text, { reply_markup: kb }));
    return;
  }

  const title = listType === "registered" ? "📋 Oxirgi ro'yxatdan o'tganlar" : "🟢 Oxirgi foydalanganlar";
  const prefix = listType === "registered" ? "ro" : "ac";

  let text = `${title}\n\nJami: <b>${total}</b> ta | Sahifa: <b>${page}/${totalPages}</b>\n\n`;

  const offset = (page - 1) * PAGE_SIZE;
  const kb = new InlineKeyboard();
  for (let i = 0; i < users.length; i++) {
    const u = users[i];
    const rowNum = offset + i + 1;
    const botId = u.bot_user_id != null ? `#${u.bot_user_id}` : "—";
    const un = u.username ? ("@" + u.username).replace(/^@+/, "@") : "—";
    const dateRaw = listType === "registered" ? u.creat_at : u.last_bot_data;
    const dateStr = formatDate(dateRaw);
    text += `${rowNum}. Bot id: <b>${botId}</b> | TG: <code>${u.user_id}</code>\n   ${un} | ${dateStr}\n`;
    kb.text(`${rowNum}. ${botId}`, `um:user:${u.user_id}`);
    if ((i + 1) % 5 === 0) kb.row();
  }
  if (users.length % 5 !== 0) kb.row();

  if (page > 1) kb.text("« Oldingi", `um:list:${prefix}:${page - 1}`);
  kb.text(`${page}/${totalPages}`, "um:noop");
  if (page < totalPages) kb.text("Keyingi »", `um:list:${prefix}:${page + 1}`);
  kb.row().text("« Orqaga", "um:list:back");
  styleAll(kb);

  await ctx.editMessageText(text, { parse_mode: "HTML", reply_markup: kb })
    .catch(() => ctx.reply(text, { parse_mode: "HTML", reply_markup: kb }));
}

// ─── Foydalanuvchi ma'lumotlari ───────────────────────────────────────────────

async function showUserDetail(ctx: MyContext, userId: number, editMsg = true): Promise<void> {
  const user = await getUser(userId);
  if (!user) {
    await ctx.reply("❌ Foydalanuvchi topilmadi.");
    return;
  }

  const stats = await getUserActivityStats(userId);

  const botId = user.bot_user_id != null ? `#${user.bot_user_id}` : "—";
  const un = user.username ? `@${user.username}` : "—";
  const phone = user.user_phone || "—";
  const balance = (user.balance ?? 0).toLocaleString("uz");
  const animeBalance = (user.anime_balance ?? 0).toLocaleString("uz");
  const regDate = formatDate(user.creat_at);
  const lastDate = formatDate(user.last_bot_data);

  const text =
    `👤 <b>Foydalanuvchi ma'lumotlari</b>\n\n` +
    `🆔 Bot ID: <b>${botId}</b>\n` +
    `📱 Telegram ID: <code>${user.user_id}</code>\n` +
    `📞 Telefon: <code>${phone}</code>\n` +
    `👤 Username: ${un}\n` +
    `📅 Ro'yxatdan o'tgan: <b>${regDate}</b>\n` +
    `🕐 So'nggi faollik: <b>${lastDate}</b>\n\n` +
    `💰 Asosiy balans: <b>${balance} so'm</b>\n` +
    `🎌 Anime balans: <b>${animeBalance} so'm</b>\n\n` +
    `❤️ Layk: <b>${stats.likes}</b>\n` +
    `💔 Dislike: <b>${stats.dislikes}</b>\n` +
    `🗂 Saqlagan: <b>${stats.saves}</b>\n` +
    `↗️ Ulashgan: <b>${stats.shares}</b>\n` +
    `💬 Izoh: <b>${stats.comments}</b>\n` +
    `📥 Yuklab olgan: <b>${stats.downloads}</b>`;

  const kb = styleAll(
    new InlineKeyboard()
      .text("🚫 Bloklash", `um:block:${user.user_id}`).row()
      .text("💰 Balansni tahrirlash", `um:bal:${user.user_id}`).row()
      .text("👑 VIP tahrirlash", `um:vip:${user.user_id}`).row()
      .text("« Orqaga", "um:main")
  );

  if (editMsg) {
    await ctx.editMessageText(text, { parse_mode: "HTML", reply_markup: kb })
      .catch(() => ctx.reply(text, { parse_mode: "HTML", reply_markup: kb }));
  } else {
    const msg = await ctx.reply(text, { parse_mode: "HTML", reply_markup: kb });
    ctx.session.umMsgId = msg.message_id;
  }
}

// ─── setupUserManagement ──────────────────────────────────────────────────────

export function setupUserManagement(bot: Bot<MyContext>): void {

  // ── Asosiy menyu tugmasi ──────────────────────────────────────────────────
  bot.hears("🔍 Foydalanuvchi boshqaruvi", async (ctx) => {
    if (!isAdmin(ctx)) return;
    ctx.session.state = "idle";
    const kb = styleAll(
      new InlineKeyboard()
        .text("🔍 Izlash", "um:search").row()
        .text("🗂 Ro'yxat", "um:list:back")
    );
    const msg = await ctx.reply("👥 <b>Foydalanuvchi boshqaruvi</b>\n\nQuyidagilardan birini tanlang:", {
      parse_mode: "HTML",
      reply_markup: kb,
    });
    ctx.session.umMsgId = msg.message_id;
  });

  // ── Asosiy menyuga qaytish ────────────────────────────────────────────────
  bot.callbackQuery("um:main", async (ctx) => {
    await ctx.answerCallbackQuery();
    if (!isAdmin(ctx)) return;
    ctx.session.state = "idle";
    const kb = styleAll(
      new InlineKeyboard()
        .text("🔍 Izlash", "um:search").row()
        .text("🗂 Ro'yxat", "um:list:back")
    );
    await ctx.editMessageText("👥 <b>Foydalanuvchi boshqaruvi</b>\n\nQuyidagilardan birini tanlang:", {
      parse_mode: "HTML",
      reply_markup: kb,
    }).catch(() => {});
  });

  bot.callbackQuery("um:noop", async (ctx) => {
    await ctx.answerCallbackQuery();
  });

  // ── Izlash ────────────────────────────────────────────────────────────────
  bot.callbackQuery("um:search", async (ctx) => {
    await ctx.answerCallbackQuery();
    if (!isAdmin(ctx)) return;
    ctx.session.state = "um_search";
    await ctx.editMessageText(
      "🔍 <b>Foydalanuvchi qidirish</b>\n\n" +
      "Quyidagi ma'lumotlardan birini yuboring:\n" +
      "• Bot ID (masalan: <code>42</code>)\n" +
      "• Telegram ID (masalan: <code>123456789</code>)\n" +
      "• Telefon raqam (masalan: <code>+998991234567</code>)\n" +
      "• @username (masalan: <code>@user</code>)\n\n" +
      "Bekor qilish: /admin",
      { parse_mode: "HTML", reply_markup: styleAll(new InlineKeyboard().text("« Orqaga", "um:main")) }
    ).catch(() => {});
  });

  // ── Ro'yxat turi tanlash ──────────────────────────────────────────────────
  bot.callbackQuery("um:list:back", async (ctx) => {
    await ctx.answerCallbackQuery();
    if (!isAdmin(ctx)) return;
    ctx.session.state = "idle";
    const kb = styleAll(
      new InlineKeyboard()
        .text("📋 Oxirgi ro'yxatdan o'tgan", "um:list:ro:1").row()
        .text("🟢 Oxirgi foydalangan", "um:list:ac:1").row()
        .text("« Orqaga", "um:main")
    );
    await ctx.editMessageText(
      "🗂 <b>Ro'yxat</b>\n\nQaysi tartibda ko'rmoqchisiz?",
      { parse_mode: "HTML", reply_markup: kb }
    ).catch(() => {});
  });

  // ── Pagination: ro'yxatdan o'tgan ─────────────────────────────────────────
  bot.callbackQuery(/^um:list:ro:(\d+)$/, async (ctx) => {
    await ctx.answerCallbackQuery();
    if (!isAdmin(ctx)) return;
    const page = Number(ctx.match[1]);
    ctx.session.umListType = "registered";
    ctx.session.umPage = page;
    await showUserList(ctx, page, "registered");
  });

  // ── Pagination: oxirgi foydalangan ────────────────────────────────────────
  bot.callbackQuery(/^um:list:ac:(\d+)$/, async (ctx) => {
    await ctx.answerCallbackQuery();
    if (!isAdmin(ctx)) return;
    const page = Number(ctx.match[1]);
    ctx.session.umListType = "active";
    ctx.session.umPage = page;
    await showUserList(ctx, page, "active");
  });

  // ── Foydalanuvchi detali ──────────────────────────────────────────────────
  bot.callbackQuery(/^um:user:(\d+)$/, async (ctx) => {
    await ctx.answerCallbackQuery();
    if (!isAdmin(ctx)) return;
    const userId = Number(ctx.match[1]);
    ctx.session.umTargetUserId = userId;
    await showUserDetail(ctx, userId);
  });

  // ── Bloklash (keyinroq) ───────────────────────────────────────────────────
  bot.callbackQuery(/^um:block:(\d+)$/, async (ctx) => {
    await ctx.answerCallbackQuery("Tez orada qo'shiladi ⏳");
  });

  // ── VIP tahrirlash (keyinroq) ─────────────────────────────────────────────
  bot.callbackQuery(/^um:vip:(\d+)$/, async (ctx) => {
    await ctx.answerCallbackQuery("Tez orada qo'shiladi ⏳");
  });

  // ── Balans tahrirlash: asosiy vs anime tanlash ────────────────────────────
  bot.callbackQuery(/^um:bal:(\d+)$/, async (ctx) => {
    await ctx.answerCallbackQuery();
    if (!isAdmin(ctx)) return;
    const userId = Number(ctx.match[1]);
    ctx.session.umTargetUserId = userId;
    const user = await getUser(userId);
    const kb = styleAll(
      new InlineKeyboard()
        .text("💵 Asosiy balans", `um:bal:type:main:${userId}`).row()
        .text("🎌 Anime balans", `um:bal:type:anime:${userId}`).row()
        .text("« Orqaga", `um:user:${userId}`)
    );
    await ctx.editMessageText(
      `💰 <b>Balans tahrirlash</b>\n\n` +
      `Foydalanuvchi: <code>${userId}</code>\n` +
      `💵 Asosiy: <b>${(user?.balance ?? 0).toLocaleString("uz")} so'm</b>\n` +
      `🎌 Anime: <b>${(user?.anime_balance ?? 0).toLocaleString("uz")} so'm</b>\n\n` +
      `Qaysi balansni tahrirlaysiz?`,
      { parse_mode: "HTML", reply_markup: kb }
    ).catch(() => {});
  });

  // ── Balans turi tanlash ───────────────────────────────────────────────────
  bot.callbackQuery(/^um:bal:type:(main|anime):(\d+)$/, async (ctx) => {
    await ctx.answerCallbackQuery();
    if (!isAdmin(ctx)) return;
    const balType = ctx.match[1] as "main" | "anime";
    const userId = Number(ctx.match[2]);
    ctx.session.umTargetUserId = userId;
    ctx.session.umBalanceType = balType;

    const label = balType === "main" ? "💵 Asosiy balans" : "🎌 Anime balans";
    const kb = styleAll(
      new InlineKeyboard()
        .text("➕ Pul qo'shish", `um:bal:op:add:${balType}:${userId}`).row()
        .text("➖ Pul ayirish", `um:bal:op:sub:${balType}:${userId}`).row()
        .text("« Orqaga", `um:bal:${userId}`)
    );
    await ctx.editMessageText(
      `${label}\n\nAmal tanlang:`,
      { parse_mode: "HTML", reply_markup: kb }
    ).catch(() => {});
  });

  // ── Qo'shish / ayirish ────────────────────────────────────────────────────
  bot.callbackQuery(/^um:bal:op:(add|sub):(main|anime):(\d+)$/, async (ctx) => {
    await ctx.answerCallbackQuery();
    if (!isAdmin(ctx)) return;
    const op = ctx.match[1] as "add" | "subtract";
    const balType = ctx.match[2] as "main" | "anime";
    const userId = Number(ctx.match[3]);
    ctx.session.umEditMode = op;
    ctx.session.umBalanceType = balType;
    ctx.session.umTargetUserId = userId;
    ctx.session.state = "um_balance_amount";

    const actionLabel = op === "add" ? "➕ Qo'shish" : "➖ Ayirish";
    const balLabel = balType === "main" ? "asosiy" : "anime";
    await ctx.editMessageText(
      `${actionLabel} — ${balLabel} balans\n\n` +
      `Necha so'm? (Faqat raqam kiriting, masalan: <code>5000</code>)`,
      { parse_mode: "HTML", reply_markup: styleAll(new InlineKeyboard().text("« Bekor qilish", `um:bal:type:${balType}:${userId}`)) }
    ).catch(() => {});
  });

  // ── Matn xabarlari (qidiruv va balans miqdori) ────────────────────────────
  bot.on("message:text", async (ctx, next) => {
    const state = ctx.session.state as string;

    // ─ Qidirish ─
    if (state === "um_search") {
      if (!isAdmin(ctx)) return next();
      ctx.session.state = "idle";
      const user = await findUserByIdentifier(ctx.message.text);
      if (!user) {
        const kb = styleAll(
          new InlineKeyboard()
            .text("🔍 Qaytadan izlash", "um:search").row()
            .text("« Orqaga", "um:main")
        );
        await ctx.reply("❌ Foydalanuvchi topilmadi.", { reply_markup: kb });
        return;
      }
      ctx.session.umTargetUserId = user.user_id;
      await showUserDetail(ctx, user.user_id, false);
      return;
    }

    // ─ Balans miqdori ─
    if (state === "um_balance_amount") {
      if (!isAdmin(ctx)) return next();
      const raw = ctx.message.text.trim().replace(/\D/g, "");
      const amount = parseInt(raw, 10);
      if (isNaN(amount) || amount <= 0) {
        await ctx.reply("❌ Noto'g'ri miqdor. Faqat musbat son kiriting. Qaytadan:");
        return; // state o'zgarmaydi
      }
      ctx.session.state = "idle";
      const userId = ctx.session.umTargetUserId;
      const balType = ctx.session.umBalanceType;
      const op = ctx.session.umEditMode;

      if (!userId || !balType || !op) {
        await ctx.reply("❌ Sessiya xatosi. Qaytadan boshlang: /admin");
        return;
      }

      const delta = op === "add" ? amount : -amount;

      if (balType === "main") {
        await updateUserBalance(userId, delta);
      } else {
        await updateAnimeBalance(userId, delta);
      }

      const user = await getUser(userId);
      const newMain = (user?.balance ?? 0).toLocaleString("uz");
      const newAnime = (user?.anime_balance ?? 0).toLocaleString("uz");
      const actionLabel = op === "add" ? "Qo'shildi" : "Ayirildi";
      const balLabel = balType === "main" ? "💵 Asosiy" : "🎌 Anime";

      const kb = styleAll(
        new InlineKeyboard()
          .text("👤 Foydalanuvchi", `um:user:${userId}`).row()
          .text("💰 Balans tahrirlash", `um:bal:${userId}`)
      );

      await ctx.reply(
        `✅ <b>${actionLabel}: ${amount.toLocaleString("uz")} so'm</b>\n\n` +
        `${balLabel} balans: <b>${balType === "main" ? newMain : newAnime} so'm</b>\n` +
        `💵 Asosiy balans: <b>${newMain} so'm</b>\n` +
        `🎌 Anime balans: <b>${newAnime} so'm</b>`,
        { parse_mode: "HTML", reply_markup: kb }
      );
      return;
    }

    return next();
  });
}
