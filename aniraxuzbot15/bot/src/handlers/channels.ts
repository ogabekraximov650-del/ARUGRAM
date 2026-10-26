import { Bot, InlineKeyboard, Keyboard } from "grammy";
import type { MyContext } from "./admin.ts";
import { adminMainKeyboard } from "./admin.ts";
import { checkIsAdmin } from "../admin_check.ts";
import {
  chatPickerKb,
  chatPickerHelpText,
  BTN_PICK_BACK,
} from "./chat_picker.ts";
import {
  countBotChannels,
  insertChannel,
  getChannelByBotAndChatId,
  getBotChannels,
  getChannelById,
  deleteChannelById,
  increaseChannelLimit,
  decreaseChannelLimit,
  insertExternalLink,
  getBotExternalLinks,
  getExternalLinkById,
  deleteExternalLinkById,
  recordJoinOrSubscribe,
  incrementJoinedCount,
  deleteRequestsByChannel,
  type ChannelInfo,
  type ExternalLink,
} from "../db_channels.ts";
import { isReservedText } from "./known_buttons.ts";

const MAX_CHANNELS = 8;
const BTN_OBUNA = "🔐 Majburiy obunalar";

type ChState =
  | { kind: "menu_open"; msgId: number }
  | { kind: "wait_post_public"; msgId: number }
  | { kind: "wait_post_private"; msgId: number }
  | { kind: "wait_limit_for_new"; channelDbId: number }
  | { kind: "wait_link_name"; msgId: number }
  | { kind: "wait_link_url"; linkName: string }
  | { kind: "wait_increase_limit"; channelDbId: number }
  | { kind: "wait_decrease_limit"; channelDbId: number };

const chState = new Map<string, ChState>();

function key(ctx: MyContext): string {
  return `${ctx.me.id}:${ctx.from?.id ?? 0}`;
}

function styleAll(kb: InlineKeyboard): InlineKeyboard {
  for (const row of kb.inline_keyboard) {
    for (const btn of row) (btn as any).style = "primary";
  }
  return kb;
}

// ====== UI builders ======

function submenuKb(): InlineKeyboard {
  return styleAll(
    new InlineKeyboard()
      .text("➕ Yangi kanal qoʻshish", "ch_add").row()
      .text("🗂  Kanallar roʻyxati", "ch_list")
  );
}

function chooseTypeKb(): InlineKeyboard {
  return styleAll(
    new InlineKeyboard()
      .text("📢 Ochiq [ public ] kanal", "ch_add_public").row()
      .text("🔐 Yopiq [ private ] kanal", "ch_add_private").row()
      .text("🔗 Tashqi havola [ URL ] link", "ch_add_external").row()
      .text("⟨ Orqaga qaytish", "ch_back_submenu")
  );
}

function backOnlyKb(target: string): InlineKeyboard {
  return styleAll(new InlineKeyboard().text("⟨ Orqaga qaytish", target));
}

async function openSubmenu(ctx: MyContext): Promise<void> {
  const msg = await ctx.reply("Quyidagilardan birini tanlang:", {
    reply_markup: submenuKb(),
  });
  chState.set(key(ctx), { kind: "menu_open", msgId: msg.message_id });
}

async function showChannelsList(ctx: MyContext): Promise<void> {
  const botId = ctx.me.id;
  const channels = await getBotChannels(botId);
  const links = await getBotExternalLinks(botId);

  let text = "🗂 <b>Kanallar roʻyxati</b>\n\n";
  let idx = 0;
  type Item = { kind: "channel"; data: ChannelInfo } | { kind: "link"; data: ExternalLink };
  const items: Item[] = [];

  for (const ch of channels) {
    idx++;
    items.push({ kind: "channel", data: ch });
    const typeLabel = ch.chanel_type === "public" ? "📢 public" : "🔐 private";
    text += `${idx}. <b>Kanal turi:</b> ${typeLabel}\n`;
    text += `   <b>Kanal IDsi:</b> <code>${ch.chanel_id}</code>\n`;
    if (ch.chanel_type === "public" && ch.chanel_username) {
      text += `   <b>Kanal useri:</b> ${ch.chanel_username}\n`;
    } else if (ch.chanel_type === "private" && ch.chanel_link) {
      text += `   <b>Kanal linki:</b> ${ch.chanel_link}\n`;
    }
    const verb = ch.chanel_type === "public" ? "qoʻshilgan" : "soʻrov yuborgan";
    text += `   <b>statistika:</b> ${ch.jami_qoshish_kerak}/${ch.jami_qoshilgan} tasi ${verb}\n\n`;
  }
  for (const lnk of links) {
    idx++;
    items.push({ kind: "link", data: lnk });
    text += `${idx}. <b>Kanal turi:</b> 🔗 tashqi havola\n`;
    text += `   <b>Nomi:</b> ${lnk.link_name}\n`;
    text += `   <b>Havola:</b> ${lnk.link}\n\n`;
  }

  if (idx === 0) {
    text = "📭 Hali hech qanday kanal qoʻshilmagan.";
  }

  const kb = new InlineKeyboard();
  let col = 0;
  for (let i = 0; i < items.length; i++) {
    const it = items[i];
    const cbData = it.kind === "channel" ? `ch_view:${it.data.id}` : `ch_view_link:${it.data.id}`;
    kb.text(String(i + 1), cbData);
    col++;
    if (col === 4) {
      kb.row();
      col = 0;
    }
  }
  if (col !== 0) kb.row();
  kb.text("⟨ Orqaga qaytish", "ch_back_submenu");
  styleAll(kb);

  const ks = chState.get(key(ctx));
  const msgId = ks && "msgId" in ks ? ks.msgId : undefined;
  if (msgId) {
    try {
      await ctx.api.editMessageText(ctx.chat!.id, msgId, text, {
        parse_mode: "HTML",
        reply_markup: kb,
        link_preview_options: { is_disabled: true },
      });
      return;
    } catch {
      /* fall through to send new */
    }
  }
  const sent = await ctx.reply(text, {
    parse_mode: "HTML",
    reply_markup: kb,
    link_preview_options: { is_disabled: true },
  });
  chState.set(key(ctx), { kind: "menu_open", msgId: sent.message_id });
}

async function showChannelDetail(ctx: MyContext, dbId: number): Promise<void> {
  const ch = await getChannelById(dbId);
  if (!ch) {
    await ctx.answerCallbackQuery({ text: "Topilmadi", show_alert: true });
    return;
  }
  const typeLabel = ch.chanel_type === "public" ? "📢 public" : "🔐 private";
  let text = `<b>Kanal turi:</b> ${typeLabel}\n`;
  text += `<b>Kanal IDsi:</b> <code>${ch.chanel_id}</code>\n`;
  if (ch.chanel_type === "public" && ch.chanel_username) {
    text += `<b>Kanal useri:</b> ${ch.chanel_username}\n`;
  } else if (ch.chanel_type === "private" && ch.chanel_link) {
    text += `<b>Kanal linki:</b> ${ch.chanel_link}\n`;
  }
  const verb = ch.chanel_type === "public" ? "qoʻshilgan" : "soʻrov yuborgan";
  text += `<b>statistika:</b> ${ch.jami_qoshish_kerak}/${ch.jami_qoshilgan} tasi ${verb}\n`;
  text += `<b>Sana:</b> ${ch.data}`;

  const kb = styleAll(
    new InlineKeyboard()
      .text("📈 Limitni oshirish", `ch_inc:${ch.id}`).row()
      .text("📉 Limitni kamaytirish", `ch_dec:${ch.id}`).row()
      .text("🗑 Kanalni oʻchirish", `ch_del:${ch.id}`).row()
      .text("⟨ Orqaga qaytish", "ch_list")
  );

  await ctx.editMessageText(text, {
    parse_mode: "HTML",
    reply_markup: kb,
    link_preview_options: { is_disabled: true },
  });
}

async function showLinkDetail(ctx: MyContext, linkId: number): Promise<void> {
  const lnk = await getExternalLinkById(linkId);
  if (!lnk) {
    await ctx.answerCallbackQuery({ text: "Topilmadi", show_alert: true });
    return;
  }
  let text = `<b>Kanal turi:</b> 🔗 tashqi havola\n`;
  text += `<b>Nomi:</b> ${lnk.link_name}\n`;
  text += `<b>Havola:</b> ${lnk.link}\n`;
  text += `<b>Sana:</b> ${lnk.data}`;

  const kb = styleAll(
    new InlineKeyboard()
      .text("🗑 Linkni oʻchirish", `ch_del_link:${lnk.id}`).row()
      .text("⟨ Orqaga qaytish", "ch_list")
  );

  await ctx.editMessageText(text, {
    parse_mode: "HTML",
    reply_markup: kb,
    link_preview_options: { is_disabled: true },
  });
}

// ====== Public-channel addition flow helpers ======

/**
 * Kanal chat_id'sini olib, kanalni DB ga qoʻshib, limit soʻraydi.
 * Bir nechta joyda chaqiriladi: forward'dan, chat_shared'dan.
 */
async function processChannelIdAndSave(
  ctx: MyContext,
  kind: "wait_post_public" | "wait_post_private",
  chanelId: string,
  title?: string,
  kbFn: () => import("grammy").Keyboard = adminMainKeyboard
): Promise<void> {
  if (!ctx.from) return;
  const k = key(ctx);
  const botId = ctx.me.id;
  const exists = await getChannelByBotAndChatId(botId, chanelId);
  if (exists) {
    await ctx.reply("❌ Bu kanal allaqachon qoʻshilgan.", {
      reply_markup: kbFn(),
    });
    chState.delete(k);
    return;
  }

  let inviteLink = "";
  let publicUsername = "";
  let chanTitle = title ?? "";
  try {
    const chat = await ctx.api.getChat(chanelId);
    if ((chat as any).username) {
      publicUsername = "@" + (chat as any).username;
    }
    if (!chanTitle && (chat as any).title) {
      chanTitle = (chat as any).title;
    }
    if (kind === "wait_post_private") {
      try {
        const link = await ctx.api.createChatInviteLink(chanelId, {
          creates_join_request: true,
          name: `bot ${ctx.me.username} — majburiy obuna`,
        });
        inviteLink = link.invite_link;
      } catch {
        await ctx.reply(
          "❌ Botga 'havola yaratish' (Invite users via link) ruxsatini bering, " +
            "soʻng qaytadan urinib koʻring."
        );
        return;
      }
    }
  } catch {
    await ctx.reply(
      "❌ Botni kanalga admin qilganingizga ishonch hosil qiling, soʻng qaytadan urinib koʻring."
    );
    return;
  }

  const newId = await insertChannel({
    user_id: ctx.from.id,
    bot_id: botId,
    chanel_type: kind === "wait_post_public" ? "public" : "private",
    chanel_id: chanelId,
    chanel_username: publicUsername,
    chanel_link: inviteLink,
    chanel_title: chanTitle,
    jami_qoshish_kerak: 0,
  });

  chState.set(k, { kind: "wait_limit_for_new", channelDbId: newId });
  // Pastki request_chat tugmalarini olib, admin keyboard'ni qaytaramiz
  await ctx.reply(
    `✅ Kanal IDsi olindi:\n<code>${chanelId}</code>\n\n` +
      `Endi kanalga qoʻshilishi kerak boʻlgan odamlar sonini RAQAM koʻrinishida yuboring (masalan: 1000):`,
    { parse_mode: "HTML", reply_markup: kbFn() }
  );
}

function extractForwardChat(ctx: MyContext): { id: string; title: string; username: string } | null {
  const m = ctx.message;
  if (!m) return null;
  const fo = (m as any).forward_origin;
  if (fo && fo.type === "channel" && fo.chat) {
    return {
      id: String(fo.chat.id),
      title: fo.chat.title ?? "",
      username: fo.chat.username ? "@" + fo.chat.username : "",
    };
  }
  const ffc = (m as any).forward_from_chat;
  if (ffc && ffc.type === "channel") {
    return {
      id: String(ffc.id),
      title: ffc.title ?? "",
      username: ffc.username ? "@" + ffc.username : "",
    };
  }
  return null;
}

// ====== Setup ======

export function setupChannels(
  bot: Bot<MyContext>,
  options?: {
    keyboard?: () => import("grammy").Keyboard;
    adminChecker?: (ctx: MyContext) => Promise<boolean> | boolean;
  }
): void {
  const mainKb = options?.keyboard ?? adminMainKeyboard;
  const isAdmin = (ctx: MyContext): Promise<boolean> =>
    Promise.resolve(
      options?.adminChecker ? options.adminChecker(ctx) : checkIsAdmin(ctx.from?.id)
    );

  // Reply-keyboard entrypoint from admin panel
  bot.hears(BTN_OBUNA, async (ctx) => {
    if (!await isAdmin(ctx)) return;
    chState.delete(key(ctx));
    await openSubmenu(ctx);
  });

  // Helper: pastdagi chat_picker reply-keyboardni olib tashlab admin keyboard'ga qaytaradi.
  // Faqat aloqador state'larda ishlaydi.
  async function maybeRestoreAdminKb(ctx: MyContext): Promise<void> {
    const st = chState.get(key(ctx));
    if (st && (st.kind === "wait_post_public" || st.kind === "wait_post_private")) {
      try {
        await ctx.reply("👇", { reply_markup: mainKb() });
      } catch {
        /* ignore */
      }
    }
  }

  // Submenu nav
  bot.callbackQuery("ch_back_submenu", async (ctx) => {
    if (!await isAdmin(ctx)) {
      await ctx.answerCallbackQuery();
      return;
    }
    await ctx.answerCallbackQuery();
    await maybeRestoreAdminKb(ctx);
    const ks = chState.get(key(ctx));
    const msgId = ks && "msgId" in ks ? ks.msgId : undefined;
    if (msgId) {
      try {
        await ctx.api.editMessageText(ctx.chat!.id, msgId, "Quyidagilardan birini tanlang:", {
          reply_markup: submenuKb(),
        });
        chState.set(key(ctx), { kind: "menu_open", msgId });
        return;
      } catch {
        /* fall through */
      }
    }
    await openSubmenu(ctx);
  });

  // ➕ Yangi kanal qoʻshish → choose type
  bot.callbackQuery("ch_add", async (ctx) => {
    if (!await isAdmin(ctx)) {
      await ctx.answerCallbackQuery();
      return;
    }
    await ctx.answerCallbackQuery();
    await maybeRestoreAdminKb(ctx);
    const text =
      "Quyidagilardan birini tanlang:\n\n" +
      "📢 <b>Ochiq kanal</b> = hammaga ochiq boʻlgan kanal va guruhlar uchun.\n\n" +
      "🔐 <b>Yopiq kanal</b> = qoʻshilish soʻrovi yuboriladigan yopiq kanallar uchun.\n\n" +
      "🔗 <b>Tashqi havola</b> = Instagram, va YouTube'ga oʻxshash oʻzingiz hohlagan havolalar.";
    await ctx.editMessageText(text, {
      parse_mode: "HTML",
      reply_markup: chooseTypeKb(),
    });
  });

  // 📢 Public flow
  bot.callbackQuery("ch_add_public", async (ctx) => {
    if (!await isAdmin(ctx)) {
      await ctx.answerCallbackQuery();
      return;
    }
    await ctx.answerCallbackQuery();
    const count = await countBotChannels(ctx.me.id);
    if (count >= MAX_CHANNELS) {
      await ctx.editMessageText(
        `❌ Maksimal ${MAX_CHANNELS} ta kanalga ruxsat bor. Hozir ${count} ta qoʻshilgan.`,
        { reply_markup: backOnlyKb("ch_add") }
      );
      return;
    }
    const text =
      "<b>Kanal turi:</b> 📢 Ochiq [ public ] kanal\n\n" + chatPickerHelpText();
    const sent = await ctx.editMessageText(text, {
      parse_mode: "HTML",
      reply_markup: backOnlyKb("ch_add"),
    });
    const msgId = typeof sent === "object" ? (sent as any).message_id : undefined;
    chState.set(key(ctx), { kind: "wait_post_public", msgId: msgId ?? 0 });
    await ctx.reply("👇 Pastdagi tugmalardan foydalaning:", {
      reply_markup: chatPickerKb({ reqAdminId: 100, reqIdOnlyId: 101 }),
    });
  });

  // 🔐 Private flow
  bot.callbackQuery("ch_add_private", async (ctx) => {
    if (!await isAdmin(ctx)) {
      await ctx.answerCallbackQuery();
      return;
    }
    await ctx.answerCallbackQuery();
    const count = await countBotChannels(ctx.me.id);
    if (count >= MAX_CHANNELS) {
      await ctx.editMessageText(
        `❌ Maksimal ${MAX_CHANNELS} ta kanalga ruxsat bor. Hozir ${count} ta qoʻshilgan.`,
        { reply_markup: backOnlyKb("ch_add") }
      );
      return;
    }
    const text =
      "<b>Kanal turi:</b> 🔐 Yopiq [ private ] kanal\n\n" +
      "Botni admin qilayotganda <b>havola yaratish</b> ruxsatini bering — aks holda bot foydalanuvchilarga kanalni koʻrsata olmaydi.\n\n" +
      chatPickerHelpText();
    const sent = await ctx.editMessageText(text, {
      parse_mode: "HTML",
      reply_markup: backOnlyKb("ch_add"),
    });
    const msgId = typeof sent === "object" ? (sent as any).message_id : undefined;
    chState.set(key(ctx), { kind: "wait_post_private", msgId: msgId ?? 0 });
    await ctx.reply("👇 Pastdagi tugmalardan foydalaning:", {
      reply_markup: chatPickerKb({ reqAdminId: 110, reqIdOnlyId: 111 }),
    });
  });

  // 🔗 External link flow — namuna URL'lar <code> ichida (tap-to-copy)
  bot.callbackQuery("ch_add_external", async (ctx) => {
    if (!await isAdmin(ctx)) {
      await ctx.answerCallbackQuery();
      return;
    }
    await ctx.answerCallbackQuery();
    const text =
      "<b>Kanal turi:</b> 🔗 Tashqi havola [ URL ] link\n\n" +
      "1. Link uchun nom yuboring (masalan: <code>Instagram</code>, <code>YouTube</code> yoki <code>Feysbuk</code>)\n" +
      "2. Keyin esa havola URL yuboring (masalan: <code>https://instagram.com/</code>, <code>https://youtube.com/</code>)\n\n" +
      "Link uchun nom yuboring:";
    const sent = await ctx.editMessageText(text, {
      parse_mode: "HTML",
      reply_markup: backOnlyKb("ch_add"),
    });
    const msgId = typeof sent === "object" ? (sent as any).message_id : undefined;
    chState.set(key(ctx), { kind: "wait_link_name", msgId: msgId ?? 0 });
  });

  // 🗂 List
  bot.callbackQuery("ch_list", async (ctx) => {
    if (!await isAdmin(ctx)) {
      await ctx.answerCallbackQuery();
      return;
    }
    await ctx.answerCallbackQuery();
    await showChannelsList(ctx);
  });

  // View channel detail
  bot.callbackQuery(/^ch_view:(\d+)$/, async (ctx) => {
    if (!await isAdmin(ctx)) {
      await ctx.answerCallbackQuery();
      return;
    }
    await ctx.answerCallbackQuery();
    await showChannelDetail(ctx, parseInt(ctx.match![1], 10));
  });

  bot.callbackQuery(/^ch_view_link:(\d+)$/, async (ctx) => {
    if (!await isAdmin(ctx)) {
      await ctx.answerCallbackQuery();
      return;
    }
    await ctx.answerCallbackQuery();
    await showLinkDetail(ctx, parseInt(ctx.match![1], 10));
  });

  // 📈 Increase limit
  bot.callbackQuery(/^ch_inc:(\d+)$/, async (ctx) => {
    if (!await isAdmin(ctx)) {
      await ctx.answerCallbackQuery();
      return;
    }
    await ctx.answerCallbackQuery();
    const id = parseInt(ctx.match![1], 10);
    chState.set(key(ctx), { kind: "wait_increase_limit", channelDbId: id });
    await ctx.editMessageText(
      "📈 Limitni qanchaga oshirmoqchisiz? Raqam koʻrinishida yuboring (masalan: 500):",
      { reply_markup: backOnlyKb(`ch_view:${id}`) }
    );
  });

  // 📉 Decrease limit
  bot.callbackQuery(/^ch_dec:(\d+)$/, async (ctx) => {
    if (!await isAdmin(ctx)) {
      await ctx.answerCallbackQuery();
      return;
    }
    await ctx.answerCallbackQuery();
    const id = parseInt(ctx.match![1], 10);
    const ch = await getChannelById(id);
    if (!ch) {
      await ctx.editMessageText("❌ Topilmadi.", { reply_markup: backOnlyKb("ch_list") });
      return;
    }
    chState.set(key(ctx), { kind: "wait_decrease_limit", channelDbId: id });
    await ctx.editMessageText(
      `📉 Limitni qanchaga kamaytirmoqchisiz?\n\n` +
        `Hozirgi limit: <b>${ch.jami_qoshish_kerak}</b>\n\n` +
        `Raqam koʻrinishida yuboring (masalan: 100):`,
      { parse_mode: "HTML", reply_markup: backOnlyKb(`ch_view:${id}`) }
    );
  });

  // Delete channel — confirm
  bot.callbackQuery(/^ch_del:(\d+)$/, async (ctx) => {
    if (!await isAdmin(ctx)) {
      await ctx.answerCallbackQuery();
      return;
    }
    await ctx.answerCallbackQuery();
    const id = parseInt(ctx.match![1], 10);
    const ch = await getChannelById(id);
    if (!ch) {
      await ctx.editMessageText("❌ Topilmadi.", { reply_markup: backOnlyKb("ch_list") });
      return;
    }
    const kb = styleAll(
      new InlineKeyboard()
        .text("✅ Ha, oʻchirish", `ch_del_yes:${id}`)
        .text("❌ Bekor", `ch_view:${id}`)
    );
    await ctx.editMessageText(
      `⚠️ <code>${ch.chanel_id}</code> kanalini oʻchirmoqchimisiz?`,
      { parse_mode: "HTML", reply_markup: kb }
    );
  });

  bot.callbackQuery(/^ch_del_yes:(\d+)$/, async (ctx) => {
    if (!await isAdmin(ctx)) {
      await ctx.answerCallbackQuery();
      return;
    }
    await ctx.answerCallbackQuery({ text: "✅ Oʻchirildi" });
    const id = parseInt(ctx.match![1], 10);
    // Yopiq kanal bo'lsa, unga tegishli barcha so'rovlarni ham o'chiramiz
    const ch = await getChannelById(id);
    if (ch && ch.chanel_type === "private") {
      await deleteRequestsByChannel(ctx.me.id, ch.chanel_id);
    }
    await deleteChannelById(id);
    await showChannelsList(ctx);
  });

  // Delete external link
  bot.callbackQuery(/^ch_del_link:(\d+)$/, async (ctx) => {
    if (!await isAdmin(ctx)) {
      await ctx.answerCallbackQuery();
      return;
    }
    await ctx.answerCallbackQuery({ text: "✅ Oʻchirildi" });
    const id = parseInt(ctx.match![1], 10);
    await deleteExternalLinkById(id);
    await showChannelsList(ctx);
  });

  // ========= "⟨ Orqaga" reply-keyboard tugmasi (chat_picker dan qaytish) =========
  bot.hears(BTN_PICK_BACK, async (ctx, next) => {
    if (!await isAdmin(ctx)) return next();
    const k = key(ctx);
    const state = chState.get(k);
    // Faqat kanal qoʻshish jarayonida bosilsa biz qabul qilamiz
    if (!state || (state.kind !== "wait_post_public" && state.kind !== "wait_post_private")) {
      return next();
    }
    chState.delete(k);
    await ctx.reply("⟨ Orqaga qaytildi.", { reply_markup: mainKb() });
    await openSubmenu(ctx);
  });

  // ========= state-based message handler =========
  // MUHIM: agar foydalanuvchi commanda yoki tanigan reply-keyboard tugmasi
  // matnini yuborsa — state tozalanib next() ga o'tadi va keyingi handler
  // (bot.hears, bot.command) o'sha tugmaga to'g'ri javob beradi.
  bot.on("message", async (ctx, next) => {
    if (!ctx.from || !ctx.chat) return next();
    if (!await isAdmin(ctx)) return next();
    const k = key(ctx);
    const state = chState.get(k);
    if (!state) return next();

    // Komandalar va tanigan reply-keyboard tugmalari uchun state'ni tashlab next() ga o'tamiz
    // (chat_shared message'ida text bo'lmaydi, shuning uchun bu chat_shared'ga ta'sir qilmaydi)
    const incomingText = (ctx.message as any).text;
    if (isReservedText(incomingText)) {
      chState.delete(k);
      return next();
    }

    // ===== chat_shared event (request_chat tugmasi orqali kanal tanlangan) =====
    const chatShared = (ctx.message as any).chat_shared as
      | { request_id: number; chat_id: number; title?: string; username?: string }
      | undefined;
    if (chatShared && (state.kind === "wait_post_public" || state.kind === "wait_post_private")) {
      const isAdminBtn =
        chatShared.request_id === 100 || chatShared.request_id === 110;
      const isIdBtn =
        chatShared.request_id === 101 || chatShared.request_id === 111;

      if (isAdminBtn) {
        // "Botni kanalga admin qilish" tugmasi — chat_id'ni ishlatmaymiz, faqat tasdiq.
        await ctx.reply(
          "✅ Bot kanalga admin qilindi.\n\n" +
            "Endi 🆔 <b>Kanal IDsi botga yuborish</b> tugmasini bosib, oʻsha kanalni tanlang.",
          { parse_mode: "HTML" }
        );
        return;
      }
      if (isIdBtn) {
        // "Kanal IDsi botga yuborish" — bu yerdan chat_id ni qabul qilamiz va saqlaymiz
        await processChannelIdAndSave(ctx, state.kind, String(chatShared.chat_id), undefined, mainKb);
        return;
      }
      // boshqa request_id — bizning oqimimizga tegishli emas
      return next();
    }

    // wait_post_public / wait_post_private — extract channel from forward
    if (state.kind === "wait_post_public" || state.kind === "wait_post_private") {
      const fwd = extractForwardChat(ctx);
      if (!fwd) {
        await ctx.reply(
          "❌ Iltimos, kanaldan postni FORWARD qiling yoki pastdagi tugmalardan foydalaning."
        );
        return;
      }
      await processChannelIdAndSave(ctx, state.kind, fwd.id, fwd.title, mainKb);
      return;
    }

    if (state.kind === "wait_limit_for_new") {
      const text = (incomingText ?? "").trim();
      const n = parseInt(text, 10);
      if (!Number.isFinite(n) || n < 1) {
        await ctx.reply("❌ Iltimos, faqat musbat raqam yuboring.");
        return;
      }
      await increaseChannelLimit(state.channelDbId, n);
      chState.delete(k);
      await ctx.reply(`✅ Saqlandi. Limit: ${n} ta foydalanuvchi.`);
      await openSubmenu(ctx);
      return;
    }

    if (state.kind === "wait_increase_limit") {
      const text = (incomingText ?? "").trim();
      const n = parseInt(text, 10);
      if (!Number.isFinite(n) || n < 1) {
        await ctx.reply("❌ Iltimos, faqat musbat raqam yuboring.");
        return;
      }
      await increaseChannelLimit(state.channelDbId, n);
      const id = state.channelDbId;
      chState.delete(k);
      const ch = await getChannelById(id);
      await ctx.reply(
        `✅ Limit +${n} ga oshirildi.\n` +
          `Yangi limit: <b>${ch?.jami_qoshish_kerak ?? "?"}</b>`,
        { parse_mode: "HTML" }
      );
      const sent = await ctx.reply("Quyidagilardan birini tanlang:", { reply_markup: submenuKb() });
      chState.set(k, { kind: "menu_open", msgId: sent.message_id });
      return;
    }

    if (state.kind === "wait_decrease_limit") {
      const text = (incomingText ?? "").trim();
      const n = parseInt(text, 10);
      if (!Number.isFinite(n) || n < 1) {
        await ctx.reply("❌ Iltimos, faqat musbat raqam yuboring.");
        return;
      }
      await decreaseChannelLimit(state.channelDbId, n);
      const id = state.channelDbId;
      chState.delete(k);
      const ch = await getChannelById(id);
      await ctx.reply(
        `✅ Limit −${n} ga kamaytirildi.\n` +
          `Yangi limit: <b>${ch?.jami_qoshish_kerak ?? "?"}</b>`,
        { parse_mode: "HTML" }
      );
      const sent = await ctx.reply("Quyidagilardan birini tanlang:", { reply_markup: submenuKb() });
      chState.set(k, { kind: "menu_open", msgId: sent.message_id });
      return;
    }

    if (state.kind === "wait_link_name") {
      const text = (incomingText ?? "").trim();
      if (!text) {
        await ctx.reply("❌ Iltimos, link uchun nom yozib yuboring.");
        return;
      }
      chState.set(k, { kind: "wait_link_url", linkName: text });
      await ctx.reply(`✅ "${text}" qabul qilindi.\n\nEndi havola (URL) yuboring:`);
      return;
    }

    if (state.kind === "wait_link_url") {
      const text = (incomingText ?? "").trim();
      if (!/^https?:\/\/.+/i.test(text)) {
        await ctx.reply("❌ Iltimos, http:// yoki https:// bilan boshlanadigan toʻliq URL yuboring.");
        return;
      }
      await insertExternalLink({
        user_id: ctx.from.id,
        bot_id: ctx.me.id,
        link_name: state.linkName,
        link: text,
      });
      chState.delete(k);
      await ctx.reply(`✅ "${state.linkName}" tashqi havolasi saqlandi.`);
      await openSubmenu(ctx);
      return;
    }

    return next();
  });

  // ========= chat_join_request — FAQAT yopiq kanallar uchun =========
  // (Telegram public kanallar uchun bu eventni umuman yubormaydi.)
  // Hisoblagich bu yerda dedupe bilan +1 qilinadi.
  bot.on("chat_join_request", async (ctx) => {
    try {
      const botId = ctx.me.id;
      const chanelId = String(ctx.chatJoinRequest.chat.id);
      const userId = ctx.chatJoinRequest.from.id;
      const link = ctx.chatJoinRequest.invite_link?.invite_link ?? "";
      const ch = await getChannelByBotAndChatId(botId, chanelId);
      if (!ch || ch.chanel_type !== "private") return;
      const isNew = await recordJoinOrSubscribe(botId, chanelId, userId, link);
      if (isNew) await incrementJoinedCount(ch.id);
    } catch (e) {
      console.error("chat_join_request handler error:", e);
    }
  });

  // ========= chat_member — FAQAT ommaviy kanallar uchun =========
  // Foydalanuvchi public kanalga qo'shilganda hisoblagich +1.
  // request_chanel_db ga YOZILMAYDI — bu jurnal faqat private uchun.
  bot.on("chat_member", async (ctx) => {
    try {
      const upd = ctx.chatMember;
      const oldS = upd.old_chat_member.status;
      const newS = upd.new_chat_member.status;
      const wasIn = ["member", "administrator", "creator", "restricted"].includes(oldS);
      const nowIn = ["member", "administrator", "creator", "restricted"].includes(newS);
      // faqat "qo'shilish" eventi: oldin a'zo emas, hozir a'zo
      if (wasIn || !nowIn) return;
      const chanelId = String(upd.chat.id);
      const ch = await getChannelByBotAndChatId(ctx.me.id, chanelId);
      if (!ch || ch.chanel_type !== "public") return;
      await incrementJoinedCount(ch.id);
    } catch (e) {
      console.error("chat_member handler error:", e);
    }
  });
}
