/**
 * Konstruktor (child) bot uchun alohida admin panel.
 * Global subAdminSet ishlatilmaydi — isChildAdmin() DB dan o'qiydi.
 */
import { Bot, InlineKeyboard, Keyboard } from "grammy";
import type { MyContext } from "./admin.ts";
import {
  isChildAdmin,
  addChildAdmin,
  removeChildAdmin,
  getChildAdmins,
} from "../child_admin_check.ts";
import { findUserByIdentifier, getUsersByIds } from "../db_users.ts";

function styleAll(kb: InlineKeyboard): InlineKeyboard {
  for (const row of kb.inline_keyboard) {
    for (const btn of row) (btn as any).style = "primary";
  }
  return kb;
}

async function showAdminList(ctx: MyContext): Promise<void> {
  const botId = ctx.me.id;
  const admins = await getChildAdmins(botId);
  const kb = new InlineKeyboard();

  if (admins.length === 0) {
    styleAll(kb.text("➕ Admin qo'shish", "cadm_add"));
    const text = "🗂 <b>Adminlar ro'yxati</b>\n\nHali hech qanday sub-admin qo'shilmagan.";
    try { await ctx.editMessageText(text, { parse_mode: "HTML", reply_markup: kb }); }
    catch { await ctx.reply(text, { parse_mode: "HTML", reply_markup: kb }); }
    return;
  }

  const records = await getUsersByIds(admins).catch(() => []);
  const userMap = new Map(records.map((u) => [u.user_id, u]));

  let text = "🗂 <b>Adminlar ro'yxati</b>\n\n";
  for (let i = 0; i < admins.length; i++) {
    const id = admins[i];
    const u = userMap.get(id);
    const name = u?.username ? ` (@${u.username})` : "";
    text += `${i + 1}. <code>${id}</code>${name}\n`;
  }

  for (let i = 0; i < admins.length; i++) {
    kb.text(`🗑 ${i + 1}.`, `cadm_remove:${admins[i]}`);
    if ((i + 1) % 3 === 0) kb.row();
  }
  if (admins.length % 3 !== 0) kb.row();
  kb.text("➕ Admin qo'shish", "cadm_add");
  styleAll(kb);

  try { await ctx.editMessageText(text, { parse_mode: "HTML", reply_markup: kb }); }
  catch { await ctx.reply(text, { parse_mode: "HTML", reply_markup: kb }); }
}

export function setupChildAdminHandlers(
  bot: Bot<MyContext>,
  mainKb: () => Keyboard,
): void {
  // /admin buyrug'i
  bot.command("admin", async (ctx) => {
    if (!await isChildAdmin(ctx.me.id, ctx.from?.id)) return;
    ctx.session.state = "idle";
    await ctx.reply("Admin panel ochildi 👇", { reply_markup: mainKb() });
  });

  // "👤 Adminlar" reply-keyboard tugmasi
  bot.hears("👤 Adminlar", async (ctx) => {
    if (!await isChildAdmin(ctx.me.id, ctx.from?.id)) return;
    ctx.session.state = "idle";
    const kb = styleAll(
      new InlineKeyboard()
        .text("➕ Admin qo'shish", "cadm_add").row()
        .text("🗂 Adminlar ro'yxati", "cadm_list"),
    );
    await ctx.reply(
      "👤 <b>Admin boshqaruvi</b>\n\nJami 5 tagacha sub-admin qo'shish mumkin.",
      { parse_mode: "HTML", reply_markup: kb },
    );
  });

  // Adminlar ro'yxati
  bot.callbackQuery("cadm_list", async (ctx) => {
    if (!await isChildAdmin(ctx.me.id, ctx.from?.id)) { await ctx.answerCallbackQuery(); return; }
    await ctx.answerCallbackQuery();
    await showAdminList(ctx);
  });

  // Admin qo'shish — ID so'rash
  bot.callbackQuery("cadm_add", async (ctx) => {
    if (!await isChildAdmin(ctx.me.id, ctx.from?.id)) { await ctx.answerCallbackQuery(); return; }
    await ctx.answerCallbackQuery();
    const admins = await getChildAdmins(ctx.me.id);
    if (admins.length >= 5) {
      try {
        await ctx.editMessageText(
          "❌ Maksimal 5 ta sub-admin qo'shish mumkin. Avval birini o'chiring.",
          { reply_markup: styleAll(new InlineKeyboard().text("🗂 Ro'yxat", "cadm_list")) },
        );
      } catch { await ctx.reply("❌ Limit to'lib ketgan."); }
      return;
    }
    ctx.session.state = "waiting_admin_id";
    const msg =
      "➕ <b>Yangi admin qo'shish</b>\n\n" +
      "Admin qilmoqchi bo'lgan foydalanuvchining quyidagilaridan birini yuboring:\n\n" +
      "• Telegram ID raqami\n• @username\n• Telefon raqami\n\n" +
      "Bekor qilish: /cancel";
    try { await ctx.editMessageText(msg, { parse_mode: "HTML" }); }
    catch { await ctx.reply(msg, { parse_mode: "HTML" }); }
  });

  // Adminni o'chirish — tasdiqlash
  bot.callbackQuery(/^cadm_remove:(\d+)$/, async (ctx) => {
    if (!await isChildAdmin(ctx.me.id, ctx.from?.id)) { await ctx.answerCallbackQuery(); return; }
    await ctx.answerCallbackQuery();
    const targetId = parseInt(ctx.match[1], 10);
    const kb = styleAll(
      new InlineKeyboard()
        .text("✅ Ha, o'chirish", `cadm_remove_yes:${targetId}`)
        .text("❌ Bekor", "cadm_list"),
    );
    try {
      await ctx.editMessageText(
        `⚠️ <code>${targetId}</code> ni adminlikdan olasizmi?`,
        { parse_mode: "HTML", reply_markup: kb },
      );
    } catch { await ctx.reply("⚠️ Adminlikdan olasizmi?", { reply_markup: kb }); }
  });

  // Adminni o'chirish — tasdiqlangan
  bot.callbackQuery(/^cadm_remove_yes:(\d+)$/, async (ctx) => {
    if (!await isChildAdmin(ctx.me.id, ctx.from?.id)) { await ctx.answerCallbackQuery(); return; }
    await ctx.answerCallbackQuery();
    const targetId = parseInt(ctx.match[1], 10);
    await removeChildAdmin(ctx.me.id, targetId);
    try {
      await ctx.api.sendMessage(targetId, "⚠️ Ushbu botda adminlik huquqingiz olib tashlandi.");
    } catch {}
    await showAdminList(ctx);
  });

  // Message handler — FAQAT waiting_admin_id holati uchun
  bot.on("message", async (ctx, next) => {
    if (!await isChildAdmin(ctx.me.id, ctx.from?.id)) return next();
    const state = ctx.session.state;

    if (state !== "waiting_admin_id") {
      if (state !== "idle") ctx.session.state = "idle";
      return next();
    }

    const text = ctx.message.text?.trim() ?? "";

    if (!text || text.startsWith("/")) {
      ctx.session.state = "idle";
      await ctx.reply("✅ Bekor qilindi.", { reply_markup: mainKb() });
      return;
    }

    const user = await findUserByIdentifier(text).catch(() => null);
    if (!user) {
      await ctx.reply(
        "❌ Foydalanuvchi topilmadi.\n\n" +
        "ID, @username yoki telefon raqamni tekshiring va qayta yuboring.",
      );
      return;
    }
    if (user.user_id === ctx.from!.id) {
      await ctx.reply("❌ O'zingizni admin qila olmaysiz.");
      return;
    }
    const admins = await getChildAdmins(ctx.me.id);
    if (admins.includes(user.user_id)) {
      await ctx.reply(`❌ <code>${user.user_id}</code> allaqachon admin.`, { parse_mode: "HTML" });
      return;
    }

    const added = await addChildAdmin(ctx.me.id, user.user_id);
    if (!added) {
      await ctx.reply("❌ Maksimal 5 ta sub-admin qo'shish mumkin. Avval birini o'chiring.");
      return;
    }
    ctx.session.state = "idle";
    await ctx.reply(
      `✅ <code>${user.user_id}</code> sub-admin qilib qo'shildi!\n\n` +
      `Endi u /admin buyrug'ini ishlatishi mumkin.`,
      { parse_mode: "HTML", reply_markup: mainKb() },
    );
    try {
      await ctx.api.sendMessage(
        user.user_id,
        "✅ Siz ushbu botga admin sifatida qo'shildingiz!\n\n/admin buyrug'ini bosing.",
      );
    } catch {}
  });
}
