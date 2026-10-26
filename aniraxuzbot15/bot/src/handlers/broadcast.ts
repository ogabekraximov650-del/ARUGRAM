import { Bot, InlineKeyboard } from "grammy";
import type { MyContext } from "./admin.ts";
import { checkIsAdmin } from "../admin_check.ts";
import { adminMainKeyboard } from "./admin.ts";
import { getAllBotUserIds, getAllServerUserIds } from "../db_users.ts";

function isAdmin(ctx: MyContext): boolean {
  return checkIsAdmin(ctx.from?.id);
}

function styleAll(kb: InlineKeyboard): InlineKeyboard {
  for (const row of kb.inline_keyboard) {
    for (const btn of row) {
      (btn as any).style = "primary";
    }
  }
  return kb;
}

/** Broadcast type bo'yicha yo'riqnoma matni */
function broadcastGuide(type: "text" | "file" | "antiforward" | "forward"): string {
  switch (type) {
    case "text":
      return (
        "✉️ <b>Matnli xabar yuborish</b>\n\n" +
        "Iltimos, faqatgina harf, raqam va har qanday belgilardan iborat bo'lgan " +
        "matnli xabar yuboring.\n\n" +
        "⚠️ Fayl, rasm yoki video yuborish mumkin emas.\n\n" +
        "Bekor qilish: /admin"
      );
    case "file":
      return (
        "📎 <b>Fayl xabar yuborish</b>\n\n" +
        "Iltimos, faqatgina fayl turdagi xabarlarni yuboring:\n" +
        "(Video, surat, hujjat, audio va boshqalar)\n\n" +
        "⚠️ Agar kanal postini yuborsangiz, bot matnni tozalab faqat faylni yuboradi.\n\n" +
        "Bekor qilish: /admin"
      );
    case "antiforward":
      return (
        "🔒 <b>Anti-forward xabar yuborish</b>\n\n" +
        "Iltimos, biron bir kanaldagi postni yuboring — men uni qaysi kanaldan " +
        "yuborilganini yashirib foydalanuvchilarga yuboraman.\n\n" +
        "Bekor qilish: /admin"
      );
    case "forward":
      return (
        "↗️ <b>Forward xabar yuborish</b>\n\n" +
        "Biron bir kanaldagi postni yuboring — men bu xabarni forward xabar " +
        "ko'rinishida yuboraman.\n\n" +
        "Bekor qilish: /admin"
      );
  }
}

/** Broadcast type uchun asosiy tugmalar klaviaturasi */
function broadcastTypeKeyboard(): InlineKeyboard {
  return styleAll(
    new InlineKeyboard()
      .text("✉️ Matnli xabar yuborish", "bc_text").row()
      .text("📎 Fayl xabar yuborish", "bc_file").row()
      .text("🔒 Anti-forward xabar yuborish", "bc_antiforward").row()
      .text("↗️ Forward xabar yuborish", "bc_forward")
  );
}

/** Audience tugmalari */
function audienceKeyboard(botUsername: string): InlineKeyboard {
  return styleAll(
    new InlineKeyboard()
      .text(`@${botUsername}`, "bc_aud_bot").row()
      .text("🌐 Server bo'yicha", "bc_aud_server")
  );
}

export function setupBroadcast(bot: Bot<MyContext>): void {
  // ===== Asosiy tugma =====
  bot.hears("✉️ Xabar yuborish", async (ctx) => {
    if (!isAdmin(ctx)) return;
    ctx.session.state = "idle";
    await ctx.reply("Quyidagilardan birini tanlang:", {
      reply_markup: broadcastTypeKeyboard(),
    });
  });

  // ===== Tur tanlash =====
  const typeMap: Record<string, "text" | "file" | "antiforward" | "forward"> = {
    bc_text: "text",
    bc_file: "file",
    bc_antiforward: "antiforward",
    bc_forward: "forward",
  };

  for (const [cbData, bcType] of Object.entries(typeMap)) {
    bot.callbackQuery(cbData, async (ctx) => {
      if (!isAdmin(ctx)) { ctx.answerCallbackQuery(); return; }
      ctx.answerCallbackQuery();
      ctx.session.state = "waiting_broadcast_msg";
      ctx.session.broadcastType = bcType;
      ctx.session.broadcastChatId = undefined;
      ctx.session.broadcastMsgId = undefined;
      ctx.session.broadcastText = undefined;
      try {
        await ctx.editMessageText(broadcastGuide(bcType), { parse_mode: "HTML" });
      } catch {
        await ctx.reply(broadcastGuide(bcType), { parse_mode: "HTML" });
      }
    });
  }

  // ===== Xabarni qabul qilish =====
  bot.on("message", async (ctx, next) => {
    if (!isAdmin(ctx)) return next();
    if (ctx.session.state !== "waiting_broadcast_msg") return next();

    const bcType = ctx.session.broadcastType;
    if (!bcType) return next();

    const msg = ctx.message;

    if (bcType === "text") {
      if (!msg.text) {
        await ctx.reply("❌ Iltimos, faqatgina matnli xabar yuboring. Fayl yoki rasm qabul qilinmaydi.\n\nBekor qilish: /admin");
        return;
      }
      ctx.session.broadcastText = msg.text;
      ctx.session.broadcastChatId = ctx.chat.id;
      ctx.session.broadcastMsgId = msg.message_id;
    } else if (bcType === "file") {
      const hasFile = !!(msg.photo || msg.video || msg.document || msg.audio || msg.animation || msg.voice || msg.video_note || msg.sticker);
      if (!hasFile) {
        await ctx.reply("❌ Iltimos, fayl (rasm, video, hujjat va h.k.) yuboring.\n\nBekor qilish: /admin");
        return;
      }
      ctx.session.broadcastChatId = ctx.chat.id;
      ctx.session.broadcastMsgId = msg.message_id;
    } else if (bcType === "antiforward" || bcType === "forward") {
      ctx.session.broadcastChatId = ctx.chat.id;
      ctx.session.broadcastMsgId = msg.message_id;
    }

    ctx.session.state = "idle";

    // Audience tanlash
    const kb = audienceKeyboard(ctx.me.username ?? "bot");
    await ctx.reply("✅ Xabar qabul qilindi!\n\nQuyidagilardan birini tanlang — kimga yuborilsin:", {
      reply_markup: kb,
    });
  });

  // ===== Audience: @bot =====
  bot.callbackQuery("bc_aud_bot", async (ctx) => {
    if (!isAdmin(ctx)) { ctx.answerCallbackQuery(); return; }
    ctx.answerCallbackQuery();
    ctx.session.broadcastAudience = "bot";
    await showBroadcastConfirm(ctx);
  });

  // ===== Audience: server =====
  bot.callbackQuery("bc_aud_server", async (ctx) => {
    if (!isAdmin(ctx)) { ctx.answerCallbackQuery(); return; }
    ctx.answerCallbackQuery();
    ctx.session.broadcastAudience = "server";
    await showBroadcastConfirm(ctx);
  });

  // ===== Tasdiqlash =====
  bot.callbackQuery("bc_confirm_yes", async (ctx) => {
    if (!isAdmin(ctx)) { ctx.answerCallbackQuery(); return; }
    ctx.answerCallbackQuery();
    await startBroadcast(ctx);
  });

  bot.callbackQuery("bc_confirm_no", async (ctx) => {
    if (!isAdmin(ctx)) { ctx.answerCallbackQuery(); return; }
    ctx.answerCallbackQuery();
    ctx.session.broadcastType = undefined;
    ctx.session.broadcastChatId = undefined;
    ctx.session.broadcastMsgId = undefined;
    ctx.session.broadcastText = undefined;
    ctx.session.broadcastAudience = undefined;
    try {
      await ctx.editMessageText("❌ Xabar yuborish bekor qilindi.");
    } catch {
      await ctx.reply("❌ Bekor qilindi.", { reply_markup: adminMainKeyboard() });
    }
  });
}

async function showBroadcastConfirm(ctx: MyContext): Promise<void> {
  const audience = ctx.session.broadcastAudience;
  const botId = ctx.me.id;

  let userIds: number[];
  if (audience === "bot") {
    userIds = await getAllBotUserIds(botId);
  } else {
    userIds = await getAllServerUserIds();
  }

  const count = userIds.length;
  const audienceLabel = audience === "bot"
    ? `@${ctx.me.username ?? "bot"} foydalanuvchilari`
    : "Barcha server foydalanuvchilari";

  const kb = styleAll(
    new InlineKeyboard()
      .text("✅ Ha, yuborish", "bc_confirm_yes").row()
      .text("❌ Yo'q, bekor qilish", "bc_confirm_no")
  );

  try {
    await ctx.editMessageText(
      `📊 <b>Xabar yuborish tasdiqlash</b>\n\n` +
      `Auditoriya: <b>${audienceLabel}</b>\n` +
      `Jami foydalanuvchilar: <b>${count} ta</b>\n\n` +
      `Rostanham ${count} ta foydalanuvchiga xabar yubormoqchimisiz?`,
      { parse_mode: "HTML", reply_markup: kb }
    );
  } catch {
    await ctx.reply(
      `📊 Jami <b>${count} ta</b> foydalanuvchiga yuboriladi.\n\nTasdiqlaysizmi?`,
      { parse_mode: "HTML", reply_markup: kb }
    );
  }
}

async function startBroadcast(ctx: MyContext): Promise<void> {
  const { broadcastType, broadcastChatId, broadcastMsgId, broadcastText, broadcastAudience } = ctx.session;

  if (!broadcastType || !broadcastAudience) {
    try { await ctx.editMessageText("❌ Xabar ma'lumotlari topilmadi. Qaytadan boshlang."); } catch {}
    return;
  }

  const botId = ctx.me.id;

  let userIds: number[];
  if (broadcastAudience === "bot") {
    userIds = await getAllBotUserIds(botId);
  } else {
    userIds = await getAllServerUserIds();
  }

  const total = userIds.length;
  if (total === 0) {
    try { await ctx.editMessageText("❌ Foydalanuvchilar topilmadi."); } catch {}
    return;
  }

  // Progress xabarini yuborish
  let progressMsg: { message_id: number; chat: { id: number } } | null = null;
  try {
    progressMsg = await ctx.api.sendMessage(
      ctx.chat!.id,
      `📤 <b>Xabar yuborilmoqda...</b>\n\n0 tadan / ${total} tasiga   0%`,
      { parse_mode: "HTML" }
    );
    await ctx.deleteMessage().catch(() => {});
  } catch {
    return;
  }

  let sent = 0;
  let failed = 0;
  let lastUpdate = Date.now();
  const chatId = progressMsg.chat.id;
  const msgId = progressMsg.message_id;

  const updateProgress = async () => {
    const done = sent + failed;
    const pct = total > 0 ? Math.floor((done / total) * 100) : 0;
    try {
      await ctx.api.editMessageText(
        chatId, msgId,
        `📤 <b>Xabar yuborilmoqda...</b>\n\n${done} tadan / ${total} tasiga   ${pct}%`,
        { parse_mode: "HTML" }
      );
    } catch {}
  };

  for (const userId of userIds) {
    try {
      if (broadcastType === "text" && broadcastText) {
        await ctx.api.sendMessage(userId, broadcastText);
      } else if (broadcastType === "file" && broadcastChatId && broadcastMsgId) {
        await ctx.api.copyMessage(userId, broadcastChatId, broadcastMsgId, { caption: "" });
      } else if (broadcastType === "antiforward" && broadcastChatId && broadcastMsgId) {
        await ctx.api.copyMessage(userId, broadcastChatId, broadcastMsgId);
      } else if (broadcastType === "forward" && broadcastChatId && broadcastMsgId) {
        await ctx.api.forwardMessage(userId, broadcastChatId, broadcastMsgId);
      }
      sent++;
    } catch {
      failed++;
    }

    // Har 5 sekundda yangilanish
    if (Date.now() - lastUpdate >= 5000) {
      lastUpdate = Date.now();
      await updateProgress();
    }
  }

  // Yakuniy natija
  const pct = Math.floor((sent / total) * 100);
  try {
    await ctx.api.editMessageText(
      chatId, msgId,
      `✅ <b>Xabar yuborish yakunlandi!</b>\n\n` +
      `Jami: <b>${total}</b> ta\n` +
      `✅ Yetkazildi: <b>${sent}</b> ta\n` +
      `❌ Yetmadi: <b>${failed}</b> ta\n` +
      `📊 Samaradorlik: <b>${pct}%</b>`,
      { parse_mode: "HTML" }
    );
  } catch {}

  // Session tozalash
  ctx.session.broadcastType = undefined;
  ctx.session.broadcastChatId = undefined;
  ctx.session.broadcastMsgId = undefined;
  ctx.session.broadcastText = undefined;
  ctx.session.broadcastAudience = undefined;
}
