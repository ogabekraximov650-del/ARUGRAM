import { Bot, Context, session, SessionFlavor, InlineKeyboard, Keyboard } from "grammy";
import { config } from "../config.ts";
import { D1SessionAdapter } from "../db_sessions.ts";
import {
  idExists,
  getSmallestAvailableId,
  insertAnime,
  getAnimesPaginated,
  getAnimeById,
  updateAnimeField,
  updateAnimeId,
  deleteAnime,
  getSetting,
  setSetting,
  clearAllData,
} from "../db.ts";
import {
  chatPickerKb,
  chatPickerHelpText,
  BTN_PICK_BACK,
} from "./chat_picker.ts";
import {
  checkIsAdmin,
  reloadSubAdmins,
  addSubAdmin,
  removeSubAdmin,
  getSubAdmins,
} from "../admin_check.ts";
import { findUserByIdentifier, getUsersByIds } from "../db_users.ts";

/** Admin paneldagi asosiy reply-keyboard. Boshqa fayllar ham ishlatadi. */
export function adminMainKeyboard(): Keyboard {
  return new Keyboard()
    .text("📹 Animelarni boshqarish").row()
    .text("🎞 Anibla").row()
    .text("📢 Asosiy Kanal").row()
    .text("🔐 Majburiy obunalar").row()
    .text("🔍 Foydalanuvchi boshqaruvi").row()
    .text("🗄 Database").row()
    .text("👤 Adminlar").text("✉️ Xabar yuborish")
    .resized();
}

/** Konstruktor child-botlar uchun cheklangan admin keyboard: faqat 2 tugma + orqaga. */
export function childAdminMainKeyboard(): Keyboard {
  return new Keyboard()
    .text("🔐 Majburiy obunalar").row()
    .text("👤 Adminlar").row()
    .text("🏠 Foydalanuvchi paneli")
    .resized();
}

const PAGE_SIZE = 10;

export type AdminState =
  | "idle"
  | "waiting_id"
  | "waiting_name"
  | "waiting_info"
  | "waiting_channel"
  | "waiting_photo"
  | "waiting_edit_name"
  | "waiting_edit_info"
  | "waiting_edit_channel"
  | "waiting_edit_id"
  | "waiting_edit_photo"
  | "waiting_main_channel"
  | "waiting_season_name"
  | "waiting_season_info"
  | "waiting_season_kategoriya"
  | "waiting_season_number"
  | "waiting_season_photo"
  | "waiting_episode_video"
  | "waiting_anibla_query"
  | "waiting_edit_episode_video"
  | "waiting_edit_episode_name"
  | "waiting_edit_episode_number"
  | "waiting_edit_info_photo"
  | "waiting_edit_info_name"
  | "waiting_edit_info_text"
  | "waiting_edit_info_season"
  | "waiting_admin_id"
  | "waiting_broadcast_msg"
  | "waiting_search_code"
  | "waiting_search_name"
  | "waiting_comment_text"
  | "waiting_edit_comment"
  | "waiting_pay_amount_admin"
  | "waiting_pay_amount_auto"
  | "waiting_pay_amount_stars"
  | "um_search"
  | "um_balance_amount";

export type Quality = "1080p" | "720p" | "480p" | "360p";

export interface SessionData {
  state: AdminState;
  searchMode?: "code" | "name";
  searchPage?: number;
  searchQuery?: string;
  pendingAnime: {
    id?: number;
    name?: string;
    info?: string;
    chanel_id?: string;
  };
  editingAnimeId?: number;
  listPage: number;
  manageMsgId?: number;
  // Season/episode flow
  pendingSeason?: {
    animeId?: number;
    name?: string;
    info?: string;
    kategoriya?: "TV" | "FILM" | "OVA";
    season_id?: number;
  };
  currentAnimeInfoId?: number;
  /** Anibla orqali qism qo'shish: qaysi anime (season_db.id) ga qo'shiladi. */
  aniblaTarget?: number;
  /** Anibla bo'limi "tekshirish" rejimida (🎞 Anibla): hech narsa yuklanmaydi, faqat anime/qism/sifatlar ko'riladi. */
  aniblaCheck?: boolean;
  episodesPage?: number;
  editingEpisodeRowid?: number;
  editingSeasonRowid?: number;
  // Quality-aware episode flow
  currentQuality?: Quality;
  editingEpisodeQuality?: Quality;
  currentEpisodeQuality?: Quality;
  // Admin management
  pendingAdminTelegramId?: number;
  // Broadcast
  broadcastType?: "text" | "file" | "antiforward" | "forward";
  broadcastChatId?: number;
  broadcastMsgId?: number;
  broadcastText?: string;
  broadcastAudience?: "bot" | "server";
  // Player
  playerAnimeId?: number;
  playerAnimeInfoId?: number;
  playerEpRowid?: number;
  playerEpNumber?: number;
  playerMsgId?: number;
  playerQuality?: string;
  playerPage?: number;
  playerSection?: number;
  playerTotalEps?: number;
  // Comment
  commentAnimeinfoId?: number;
  commentAnimeId?: number;
  commentPage?: number;
  commentMsgId?: number;
  editingCommentRowid?: number;
  // User management
  umTargetUserId?: number;
  umPage?: number;
  umListType?: "registered" | "active";
  umEditMode?: "add" | "subtract";
  umBalanceType?: "main" | "anime";
  umMsgId?: number;
}

export type MyContext = Context & SessionFlavor<SessionData>;

function isAdmin(ctx: MyContext): boolean {
  return checkIsAdmin(ctx.from?.id);
}

function styleAll(kb: InlineKeyboard, style: "primary" | "success" | "danger" = "primary"): InlineKeyboard {
  for (const row of kb.inline_keyboard) {
    for (const btn of row) {
      (btn as any).style = style;
    }
  }
  return kb;
}

function mainInlineKeyboard(): InlineKeyboard {
  return styleAll(
    new InlineKeyboard()
      .text("➕ Anime qo'shish", "add_anime").row()
      .text("📁 Animelar Ro'yxati", "list_anime")
  );
}

async function sendListPage(ctx: MyContext, page: number, editMsgId?: number): Promise<void> {
  const { animes, total } = await getAnimesPaginated(page, PAGE_SIZE);
  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE));

  if (total === 0) {
    const keyboard = styleAll(new InlineKeyboard()
      .text("« Orqaga qaytish", "back_to_main"));
    if (editMsgId) {
      const edited = await ctx.api.editMessageText(
        ctx.chat!.id,
        editMsgId,
        "📭 Bazada hech qanday anime yo'q.",
        { reply_markup: keyboard }
      ).catch(() => null);
      if (!edited) {
        try { await ctx.api.deleteMessage(ctx.chat!.id, editMsgId); } catch {}
        const msg = await ctx.reply("📭 Bazada hech qanday anime yo'q.", { reply_markup: keyboard });
        ctx.session.manageMsgId = msg.message_id;
      }
    } else {
      const msg = await ctx.reply("📭 Bazada hech qanday anime yo'q.", { reply_markup: keyboard });
      ctx.session.manageMsgId = msg.message_id;
    }
    return;
  }

  const kb = new InlineKeyboard();
  for (const anime of animes) {
    kb.text(`🎬 ${anime.name}`, `anime:${anime.id}`).row();
  }

  const paginationRow: string[] = [];
  if (page > 1) paginationRow.push("prev");
  paginationRow.push("counter");
  if (page < totalPages) paginationRow.push("next");

  if (page > 1) {
    kb.text("« Oldingi", `list_page:${page - 1}`);
  }
  kb.text(`${page}/${totalPages}`, "noop");
  if (page < totalPages) {
    kb.text("Keyingi »", `list_page:${page + 1}`);
  }
  kb.row().text("« Orqaga qaytish", "back_to_main");
  styleAll(kb);

  const text = `📁 <b>Animelar ro'yxati</b>\n\nJami: <b>${total}</b> ta anime`;

  if (editMsgId) {
    const edited = await ctx.api.editMessageText(ctx.chat!.id, editMsgId, text, {
      parse_mode: "HTML",
      reply_markup: kb,
    }).catch(() => null);
    if (!edited) {
      // Rasm xabarini editlashga urinish muvaffaqiyatsiz — o'chirib yangisini yubor
      try { await ctx.api.deleteMessage(ctx.chat!.id, editMsgId); } catch {}
      const msg = await ctx.reply(text, { parse_mode: "HTML", reply_markup: kb });
      ctx.session.manageMsgId = msg.message_id;
    }
  } else {
    const msg = await ctx.reply(text, {
      parse_mode: "HTML",
      reply_markup: kb,
    });
    ctx.session.manageMsgId = msg.message_id;
  }
  ctx.session.listPage = page;
}

async function sendAnimeDetail(ctx: MyContext, animeId: number): Promise<void> {
  const anime = await getAnimeById(animeId);
  if (!anime) {
    await ctx.answerCallbackQuery("Anime topilmadi!");
    return;
  }

  const botUsername = ctx.me.username ?? "aniraxuzbot";

  const caption =
    `<b>Anime nomi:</b> ${anime.name}\n\n` +
    `<b>Tavsif:</b> ${anime.info}\n\n` +
    `<b>ID:</b> <code>${anime.id}</code>\n` +
    `<b>Kanal IDsi:</b> <code>${anime.chanel_id}</code>\n` +
    `<b>Botimiz:</b> @${botUsername}\n` +
    `<b>Yuklash linki:</b> https://t.me/${botUsername}?start=${anime.id}\n` +
    `<b>Qo'shilgan sana:</b> ${anime.data || "—"}`;

  const kb = styleAll(new InlineKeyboard()
    .text("📝 Suratni tahrirlash", `edit_photo:${anime.id}`).row()
    .text("📝 Nomini tahrirlash", `edit_name:${anime.id}`).row()
    .text("📝 Tavsifini tahrirlash", `edit_info:${anime.id}`).row()
    .text("📝 ID kodni tahrirlash", `edit_id:${anime.id}`).row()
    .text("📝 Kanal idsini tahrirlash", `edit_channel:${anime.id}`).row()
    .text("📝 Anime qismlarini tahrirlash", `edit_episodes:${anime.id}`).row()
    .text("🗑 Animeni o'chirish", `delete_anime:${anime.id}`).row()
    .text("« Orqaga qaytish", "back_to_list"));

  if (ctx.session.manageMsgId) {
    try {
      await ctx.api.deleteMessage(ctx.chat!.id, ctx.session.manageMsgId);
    } catch (_) {}
  }

  const msg = await ctx.api.sendPhoto(ctx.chat!.id, anime.file_id, {
    caption,
    parse_mode: "HTML",
    reply_markup: kb,
  });
  ctx.session.manageMsgId = msg.message_id;
}

/** Pending admin invites: candidateId → { inviterId, botName } */
const pendingAdminInvites = new Map<number, { inviterId: number; botName: string }>();

/** Pending ownership transfers: candidateId → { ownerId, botName } */
const pendingOwnershipTransfers = new Map<number, { ownerId: number; botName: string }>();

async function showAdminList(ctx: MyContext): Promise<void> {
  const admins = getSubAdmins(); // sync — D1 call yo'q
  const kb = new InlineKeyboard();

  if (admins.length === 0) {
    const text = "🗂 <b>Adminlar ro'yxati</b>\n\nHali hech qanday sub-admin qo'shilmagan.";
    kb.text("➕ Admin qo'shish", "adm_add").row();
    kb.text("⟨ Orqaga", "adm_back");
    styleAll(kb);
    try {
      await ctx.editMessageText(text, { parse_mode: "HTML", reply_markup: kb });
    } catch {
      await ctx.reply(text, { parse_mode: "HTML", reply_markup: kb });
    }
    return;
  }

  // Foydalanuvchi nomlarini user_db dan olamiz (1 ta D1 call)
  const userRecords = await getUsersByIds(admins);
  const userMap = new Map(userRecords.map(u => [u.user_id, u]));

  let text = "🗂 <b>Adminlar ro'yxati</b>\n\n";
  for (let i = 0; i < admins.length; i++) {
    const id = admins[i];
    const user = userMap.get(id);
    const uname = user?.username ? `\n   Admin @${user.username}` : "";
    text += `${i + 1}. <code>${id}</code>${uname}\n\n`;
  }
  text += "Quyidagilardan birini tanlang:";

  // Har bir admin uchun raqamli tugma
  for (let i = 0; i < admins.length; i++) {
    kb.text(`${i + 1}. ${admins[i]}`, `adm_view:${admins[i]}`);
    if (i % 2 === 1) kb.row();
  }
  if (admins.length % 2 !== 0) kb.row();
  kb.text("➕ Admin qo'shish", "adm_add").row();
  kb.text("⟨ Orqaga", "adm_back");
  styleAll(kb);

  try {
    await ctx.editMessageText(text, { parse_mode: "HTML", reply_markup: kb });
  } catch {
    await ctx.reply(text, { parse_mode: "HTML", reply_markup: kb });
  }
}

async function showAdminDetail(ctx: MyContext, adminId: number, index: number): Promise<void> {
  const userRecords = await getUsersByIds([adminId]);
  const user = userRecords[0];
  const uname = user?.username ? `\nUsername: @${user.username}` : "";

  const text = `👤 <b>${index}. Admin ma'lumotlari</b>\n\nAdmin id: <code>${adminId}</code>${uname}`;

  const kb = new InlineKeyboard();
  if (ctx.from?.id === config.ADMIN_ID) {
    kb.text("🔄 Egalikni o'tkazish", `adm_transfer:${adminId}`).row();
  }
  kb.text("🗑 Adminlikdan olish", `adm_remove:${adminId}`).row();
  kb.text("⟨ Orqaga", "adm_list");
  styleAll(kb);

  try {
    await ctx.editMessageText(text, { parse_mode: "HTML", reply_markup: kb });
  } catch {
    await ctx.reply(text, { parse_mode: "HTML", reply_markup: kb });
  }
}

export function setupAdminHandlers(
  bot: Bot<MyContext>,
  options?: { keyboard?: () => Keyboard }
): void {
  // Sub-adminlarni DB dan yuklab olish
  reloadSubAdmins().catch(e => console.error("reloadSubAdmins error:", e));

  const mainKb = options?.keyboard ?? adminMainKeyboard;

  bot.command("admin", async (ctx) => {
    if (!isAdmin(ctx)) return;
    ctx.session.state = "idle";
    await ctx.reply("Admin panel ochildi 👇", { reply_markup: mainKb() });
  });

  bot.hears("📹 Animelarni boshqarish", async (ctx) => {
    if (!isAdmin(ctx)) return;
    ctx.session.state = "idle";
    const msg = await ctx.reply("Quyidagilardan birini tanlang:", {
      reply_markup: mainInlineKeyboard(),
    });
    ctx.session.manageMsgId = msg.message_id;
  });

  bot.hears("📢 Asosiy Kanal", async (ctx) => {
    if (!isAdmin(ctx)) return;
    ctx.session.state = "waiting_main_channel";
    const current = await getSetting("main_channel_id");
    let info = "";
    if (current) {
      let username = "";
      try {
        const chat = await ctx.api.getChat(current);
        if ("username" in chat && chat.username) username = "@" + chat.username;
      } catch {}
      info = `\n\nHozirgi: <code>${current}</code>${username ? ` (${username})` : ""}`;
    }
    await ctx.reply(
      `📢 <b>Asosiy kanal</b>${info}\n\n` + chatPickerHelpText() + "\n\nBekor qilish: /admin",
      { parse_mode: "HTML" }
    );
    await ctx.reply("👇 Pastdagi tugmalardan foydalaning:", {
      reply_markup: chatPickerKb({ reqAdminId: 200, reqIdOnlyId: 201 }),
    });
  });

  // ===== 👤 Adminlar =====
  bot.hears("👤 Adminlar", async (ctx) => {
    if (!isAdmin(ctx)) return;
    ctx.session.state = "idle";
    const kb = styleAll(
      new InlineKeyboard()
        .text("➕ Admin qo'shish", "adm_add").row()
        .text("🗂 Adminlar ro'yxati", "adm_list")
    );
    await ctx.reply(
      "👤 <b>Admin boshqaruvi</b>\n\nJami 5 tagacha sub-admin qo'shish mumkin.",
      { parse_mode: "HTML", reply_markup: kb }
    );
  });

  bot.callbackQuery("adm_back", async (ctx) => {
    if (!isAdmin(ctx)) { ctx.answerCallbackQuery(); return; }
    ctx.answerCallbackQuery();
    const kb = styleAll(
      new InlineKeyboard()
        .text("➕ Admin qo'shish", "adm_add").row()
        .text("🗂 Adminlar ro'yxati", "adm_list")
    );
    try {
      await ctx.editMessageText(
        "👤 <b>Admin boshqaruvi</b>\n\nJami 5 tagacha sub-admin qo'shish mumkin.",
        { parse_mode: "HTML", reply_markup: kb }
      );
    } catch {
      await ctx.reply("👤 Admin boshqaruvi", { reply_markup: kb });
    }
  });

  bot.callbackQuery("adm_add", async (ctx) => {
    if (!isAdmin(ctx)) { ctx.answerCallbackQuery(); return; }
    ctx.answerCallbackQuery();
    const admins = getSubAdmins();
    if (admins.length >= 5) {
      try {
        await ctx.editMessageText(
          "❌ Maksimal 5 ta sub-admin qo'shish mumkin. Avval birini o'chiring.",
          { reply_markup: styleAll(new InlineKeyboard().text("⟨ Orqaga", "adm_back")) }
        );
      } catch {
        await ctx.reply("❌ Maksimal 5 ta sub-admin qo'shish mumkin.");
      }
      return;
    }
    ctx.session.state = "waiting_admin_id";
    const addMsg =
      "➕ <b>Yangi admin qo'shish</b>\n\n" +
      "Admin qilmoqchi bo'lgan do'stingizni Quyidagilaridan birini yuboring:\n\n" +
      "• Botdagi ID raqamini\n" +
      "• Telegram ID raqamini\n" +
      "• Telefon raqamini\n\n" +
      "Bekor qilish: /admin";
    try {
      await ctx.editMessageText(addMsg, { parse_mode: "HTML" });
    } catch {
      await ctx.reply(addMsg, { parse_mode: "HTML" });
    }
  });

  bot.callbackQuery("adm_list", async (ctx) => {
    if (!isAdmin(ctx)) { ctx.answerCallbackQuery(); return; }
    ctx.answerCallbackQuery();
    await showAdminList(ctx);
  });

  bot.callbackQuery(/^adm_view:(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { ctx.answerCallbackQuery(); return; }
    ctx.answerCallbackQuery();
    const adminId = parseInt(ctx.match[1], 10);
    const admins = getSubAdmins();
    const idx = admins.indexOf(adminId);
    await showAdminDetail(ctx, adminId, idx + 1);
  });

  bot.callbackQuery(/^adm_remove:(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { ctx.answerCallbackQuery(); return; }
    ctx.answerCallbackQuery();
    const targetId = parseInt(ctx.match[1], 10);
    const kb = styleAll(
      new InlineKeyboard()
        .text("✅ Ha", `adm_remove_yes:${targetId}`)
        .text("❌ Yo'q", "adm_list")
    );
    try {
      await ctx.editMessageText(
        `⚠️ Bu do'stingizni rostanham adminlikdan olmoqchimisiz?\n\nID: <code>${targetId}</code>`,
        { parse_mode: "HTML", reply_markup: kb }
      );
    } catch {
      await ctx.reply(`⚠️ Adminlikdan olasizmi?`, { parse_mode: "HTML", reply_markup: kb });
    }
  });

  bot.callbackQuery(/^adm_remove_yes:(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { ctx.answerCallbackQuery(); return; }
    ctx.answerCallbackQuery();
    const targetId = parseInt(ctx.match[1], 10);
    await removeSubAdmin(targetId);
    try { await ctx.api.sendMessage(targetId, "⚠️ Adminlik huquqingiz olib tashlandi."); } catch {}
    await showAdminList(ctx);
  });

  // 🔄 Egalik o'tkazish — 1-bosqich
  bot.callbackQuery(/^adm_transfer:(\d+)$/, async (ctx) => {
    if (!ctx.from || ctx.from.id !== config.ADMIN_ID) { ctx.answerCallbackQuery(); return; }
    ctx.answerCallbackQuery();
    const targetId = parseInt(ctx.match[1], 10);
    const kb = styleAll(
      new InlineKeyboard()
        .text("✅ Ha", `adm_transfer2:${targetId}`)
        .text("❌ Yo'q", "adm_list")
    );
    try {
      await ctx.editMessageText(
        "🔄 <b>Egalikni o'tkazish</b>\n\nRostanham egalikni o'tkazmoqchimisiz?",
        { parse_mode: "HTML", reply_markup: kb }
      );
    } catch {
      await ctx.reply("Rostanham egalikni o'tkazmoqchimisiz?", { reply_markup: kb });
    }
  });

  // 🔄 Egalik o'tkazish — 2-bosqich (ogohlantirish)
  bot.callbackQuery(/^adm_transfer2:(\d+)$/, async (ctx) => {
    if (!ctx.from || ctx.from.id !== config.ADMIN_ID) { ctx.answerCallbackQuery(); return; }
    ctx.answerCallbackQuery();
    const targetId = parseInt(ctx.match[1], 10);
    const kb = styleAll(
      new InlineKeyboard()
        .text("✅ Ha, ishonchim komil", `adm_transfer3:${targetId}`).row()
        .text("❌ Yo'q, fikrimdan qaytdim", "adm_list")
    );
    try {
      await ctx.editMessageText(
        "⚠️ <b>Rostanham ishonchingiz komilmi!</b>\n\n" +
        "Ha ishonchim komil tugmasini bossangiz do'stingizga xabar yuboriladi. " +
        "Do'stingiz egalikni olishga rozi bo'lsa botning egalik huquqi do'stingizga o'tadi " +
        "va buni ortga qaytarib bo'lmaydi.\n\n" +
        "<b>Eslatma!</b> Do'stingiz egalik huquqini olmasidan oldin uni adminlar ro'yxatidan " +
        "o'chirsangiz, do'stingiz egalik huquqini ola olmaydi.",
        { parse_mode: "HTML", reply_markup: kb }
      );
    } catch {
      await ctx.reply("Ishonchingiz komilmi?", { reply_markup: kb });
    }
  });

  // 🔄 Egalik o'tkazish — 3-bosqich (taklif yuborish)
  bot.callbackQuery(/^adm_transfer3:(\d+)$/, async (ctx) => {
    if (!ctx.from || ctx.from.id !== config.ADMIN_ID) { ctx.answerCallbackQuery(); return; }
    ctx.answerCallbackQuery();
    const targetId = parseInt(ctx.match[1], 10);
    if (!getSubAdmins().includes(targetId)) {
      try {
        await ctx.editMessageText(
          "❌ Bu foydalanuvchi adminlar ro'yxatida emas. Egalikni o'tkazib bo'lmaydi.",
          { reply_markup: styleAll(new InlineKeyboard().text("⟨ Orqaga", "adm_list")) }
        );
      } catch { await ctx.reply("❌ Foydalanuvchi adminlar ro'yxatida emas."); }
      return;
    }
    const botUsername = ctx.me.username ?? "bot";
    pendingOwnershipTransfers.set(targetId, { ownerId: ctx.from.id, botName: botUsername });
    const inviteKb = styleAll(
      new InlineKeyboard()
        .text("✅ Ha, tayyorman", `adm_take_own:${targetId}`).row()
        .text("❌ Yo'q, shart emas", `adm_no_own:${targetId}`)
    );
    try {
      await ctx.api.sendMessage(
        targetId,
        `🎁 Sizga @${botUsername} egalik huquqini o'tkazishmoqchi.\n\n` +
        `Egalik huquqini olishga tayyormisiz? Agar tayyor bo'lsangiz, ha tayyorman tugmasini bosing.`,
        { reply_markup: inviteKb }
      );
      try {
        await ctx.editMessageText(
          `✅ Egalik o'tkazish taklifi <code>${targetId}</code> ga yuborildi. U tasdiqlashini kuting.`,
          { parse_mode: "HTML" }
        );
      } catch { await ctx.reply("✅ Taklif yuborildi."); }
    } catch {
      pendingOwnershipTransfers.delete(targetId);
      try {
        await ctx.editMessageText("❌ Foydalanuvchiga xabar yuborib bo'lmadi. U botga /start bosishi kerak.");
      } catch { await ctx.reply("❌ Xabar yuborib bo'lmadi."); }
    }
  });

  // Candidate: admin taklifini qabul qilish (har qanday foydalanuvchi)
  bot.callbackQuery(/^adm_invite_yes:(\d+)$/, async (ctx) => {
    ctx.answerCallbackQuery();
    const candidateId = parseInt(ctx.match[1], 10);
    if (ctx.from?.id !== candidateId) {
      try { await ctx.editMessageText("❌ Bu taklif siz uchun emas."); } catch {}
      return;
    }
    const invite = pendingAdminInvites.get(candidateId);
    if (!invite) {
      try { await ctx.editMessageText("❌ Taklif muddati o'tib ketgan yoki bekor qilingan."); } catch {}
      return;
    }
    pendingAdminInvites.delete(candidateId);
    const added = await addSubAdmin(candidateId);
    if (!added) {
      try { await ctx.editMessageText("❌ Admin qo'shib bo'lmadi (maksimal limit to'lib ketgan)."); } catch {}
      return;
    }
    try {
      await ctx.editMessageText(
        `✅ Siz @${invite.botName} ga admin sifatida qo'shildingiz!\n\n/admin buyrug'ini bosing.`
      );
    } catch {}
    try {
      await ctx.api.sendMessage(invite.inviterId, `✅ <code>${candidateId}</code> admin bo'lishga rozi bo'ldi!`, { parse_mode: "HTML" });
    } catch {}
  });

  // Candidate: admin taklifini rad etish
  bot.callbackQuery(/^adm_invite_no:(\d+)$/, async (ctx) => {
    ctx.answerCallbackQuery();
    const candidateId = parseInt(ctx.match[1], 10);
    if (ctx.from?.id !== candidateId) {
      try { await ctx.editMessageText("❌ Bu taklif siz uchun emas."); } catch {}
      return;
    }
    const invite = pendingAdminInvites.get(candidateId);
    pendingAdminInvites.delete(candidateId);
    try { await ctx.editMessageText("❌ Admin taklifini rad ettingiz."); } catch {}
    if (invite) {
      try {
        await ctx.api.sendMessage(invite.inviterId, `❌ <code>${candidateId}</code> admin bo'lishni rad etdi.`, { parse_mode: "HTML" });
      } catch {}
    }
  });

  // Candidate: egalik taklifini qabul qilish
  bot.callbackQuery(/^adm_take_own:(\d+)$/, async (ctx) => {
    ctx.answerCallbackQuery();
    const candidateId = parseInt(ctx.match[1], 10);
    if (ctx.from?.id !== candidateId) {
      try { await ctx.editMessageText("❌ Bu taklif siz uchun emas."); } catch {}
      return;
    }
    const transfer = pendingOwnershipTransfers.get(candidateId);
    if (!transfer) {
      try { await ctx.editMessageText("❌ Taklif muddati o'tib ketgan yoki bekor qilingan."); } catch {}
      return;
    }
    if (!getSubAdmins().includes(candidateId)) {
      pendingOwnershipTransfers.delete(candidateId);
      try { await ctx.editMessageText("❌ Siz adminlar ro'yxatidan o'chirib tashlangansiz. Egalik huquqini ola olmaysiz."); } catch {}
      try {
        await ctx.api.sendMessage(transfer.ownerId,
          `ℹ️ <code>${candidateId}</code> adminlar ro'yxatidan o'chirilganligi sababli egalik huquqini ola olmadi.`,
          { parse_mode: "HTML" }
        );
      } catch {}
      return;
    }
    pendingOwnershipTransfers.delete(candidateId);
    await removeSubAdmin(candidateId);
    await addSubAdmin(transfer.ownerId);
    await reloadSubAdmins();
    try {
      await ctx.editMessageText(
        `🎉 Tabriklaymiz! @${transfer.botName} egalik huquqi sizga o'tkazildi!\n\n/admin buyrug'ini bosing.`
      );
    } catch {}
    try {
      await ctx.api.sendMessage(transfer.ownerId,
        `✅ Bot egaligi <code>${candidateId}</code> ga muvaffaqiyatli o'tkazildi.\n\nSiz endi sub-admin sifatida ro'yxatdasisiz.`,
        { parse_mode: "HTML" }
      );
    } catch {}
  });

  // Candidate: egalik taklifini rad etish
  bot.callbackQuery(/^adm_no_own:(\d+)$/, async (ctx) => {
    ctx.answerCallbackQuery();
    const candidateId = parseInt(ctx.match[1], 10);
    if (ctx.from?.id !== candidateId) {
      try { await ctx.editMessageText("❌ Bu taklif siz uchun emas."); } catch {}
      return;
    }
    const transfer = pendingOwnershipTransfers.get(candidateId);
    pendingOwnershipTransfers.delete(candidateId);
    try { await ctx.editMessageText("❌ Egalik taklifini rad ettingiz."); } catch {}
    if (transfer) {
      try {
        await ctx.api.sendMessage(transfer.ownerId,
          `❌ <code>${candidateId}</code> egalik huquqini olishni rad etdi.`,
          { parse_mode: "HTML" }
        );
      } catch {}
    }
  });

  bot.callbackQuery("add_anime", async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    ctx.session.state = "waiting_id";
    ctx.session.pendingAnime = {};
    await ctx.answerCallbackQuery();
    await ctx.reply("✅ Iltimos, anime uchun raqam ko'rinishida ID kod yuboring:");
  });

  bot.callbackQuery("list_anime", async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    await ctx.answerCallbackQuery();
    try {
      const page = ctx.session.listPage || 1;
      await sendListPage(ctx, page, ctx.session.manageMsgId);
    } catch (e) {
      console.error("list_anime error:", e);
      await ctx.reply("❌ Xato: " + String(e));
    }
  });

  bot.callbackQuery(/^list_page:(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    await ctx.answerCallbackQuery();
    const page = parseInt(ctx.match[1], 10);
    try {
      await sendListPage(ctx, page, ctx.session.manageMsgId);
    } catch (e) {
      console.error("list_page error:", e);
    }
  });

  bot.callbackQuery("noop", async (ctx) => {
    await ctx.answerCallbackQuery();
  });

  bot.callbackQuery("back_to_main", async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    await ctx.answerCallbackQuery();
    try {
      if (ctx.session.manageMsgId) {
        await ctx.api.editMessageText(
          ctx.chat!.id,
          ctx.session.manageMsgId,
          "Quyidagilardan birini tanlang:",
          { reply_markup: mainInlineKeyboard() }
        );
      } else {
        const msg = await ctx.reply("Quyidagilardan birini tanlang:", {
          reply_markup: mainInlineKeyboard(),
        });
        ctx.session.manageMsgId = msg.message_id;
      }
    } catch (e) {
      console.error("back_to_main error:", e);
    }
  });

  bot.callbackQuery("back_to_list", async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    await ctx.answerCallbackQuery();
    try {
      if (ctx.session.manageMsgId) {
        try {
          await ctx.api.deleteMessage(ctx.chat!.id, ctx.session.manageMsgId);
        } catch (_) {}
        ctx.session.manageMsgId = undefined;
      }
      await sendListPage(ctx, ctx.session.listPage || 1);
    } catch (e) {
      console.error("back_to_list error:", e);
    }
  });

  bot.callbackQuery(/^anime:(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    await ctx.answerCallbackQuery();
    const animeId = parseInt(ctx.match[1], 10);
    try {
      await sendAnimeDetail(ctx, animeId);
    } catch (e) {
      console.error("anime detail error:", e);
      await ctx.reply("❌ Xato: " + String(e));
    }
  });

  bot.callbackQuery(/^edit_name:(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    const animeId = parseInt(ctx.match[1], 10);
    ctx.session.editingAnimeId = animeId;
    ctx.session.state = "waiting_edit_name";
    await ctx.answerCallbackQuery();
    await ctx.reply("✏️ Yangi nomni yuboring:");
  });

  bot.callbackQuery(/^edit_info:(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    const animeId = parseInt(ctx.match[1], 10);
    ctx.session.editingAnimeId = animeId;
    ctx.session.state = "waiting_edit_info";
    await ctx.answerCallbackQuery();
    await ctx.reply("✏️ Yangi tavsifni yuboring:");
  });

  bot.callbackQuery(/^edit_channel:(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    const animeId = parseInt(ctx.match[1], 10);
    ctx.session.editingAnimeId = animeId;
    ctx.session.state = "waiting_edit_channel";
    await ctx.answerCallbackQuery();
    await ctx.reply(
      "✏️ <b>Anime kanalini almashtirish</b>\n\n" + chatPickerHelpText(),
      { parse_mode: "HTML" }
    );
    await ctx.reply("👇 Pastdagi tugmalardan foydalaning:", {
      reply_markup: chatPickerKb({ reqAdminId: 204, reqIdOnlyId: 205 }),
    });
  });

  bot.callbackQuery(/^edit_id:(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    const animeId = parseInt(ctx.match[1], 10);
    ctx.session.editingAnimeId = animeId;
    ctx.session.state = "waiting_edit_id";
    await ctx.answerCallbackQuery();
    await ctx.reply("✏️ Yangi ID kodni yuboring (musbat son):");
  });

  bot.callbackQuery(/^edit_photo:(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    const animeId = parseInt(ctx.match[1], 10);
    ctx.session.editingAnimeId = animeId;
    ctx.session.state = "waiting_edit_photo";
    await ctx.answerCallbackQuery();
    await ctx.reply("✏️ Yangi suratni yuboring:");
  });

  bot.callbackQuery(/^delete_anime:(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    const animeId = parseInt(ctx.match[1], 10);
    await ctx.answerCallbackQuery();
    const anime = await getAnimeById(animeId);
    if (!anime) { await ctx.reply("Anime topilmadi."); return; }
    const kb = styleAll(new InlineKeyboard()
      .text("✅ Ha, o'chirish", `confirm_delete:${animeId}`)
      .text("❌ Bekor qilish", `cancel_delete:${animeId}`));
    await ctx.reply(
      `⚠️ <b>${anime.name}</b> animesini o'chirmoqchimisiz?`,
      { parse_mode: "HTML", reply_markup: kb }
    );
  });

  bot.callbackQuery(/^confirm_delete:(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    const animeId = parseInt(ctx.match[1], 10);
    await ctx.answerCallbackQuery();
    try {
      const anime = await getAnimeById(animeId);
      await deleteAnime(animeId);
      await ctx.editMessageText(`✅ <b>${anime?.name ?? "Anime"}</b> o'chirildi.`, { parse_mode: "HTML" });
      if (ctx.session.manageMsgId) {
        try { await ctx.api.deleteMessage(ctx.chat!.id, ctx.session.manageMsgId); } catch (_) {}
        ctx.session.manageMsgId = undefined;
      }
      await sendListPage(ctx, 1);
    } catch (e) {
      await ctx.reply("❌ Xato: " + String(e));
    }
  });

  bot.callbackQuery(/^cancel_delete:(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    const animeId = parseInt(ctx.match[1], 10);
    await ctx.answerCallbackQuery("Bekor qilindi");
    await ctx.editMessageText("❌ O'chirish bekor qilindi.");
    await sendAnimeDetail(ctx, animeId);
  });

  // ⟨ Orqaga reply-keyboard tugmasi (chat picker oqimidan qaytish, admin uchun)
  bot.hears(BTN_PICK_BACK, async (ctx, next) => {
    if (!isAdmin(ctx)) return next();
    const st = ctx.session.state;
    if (
      st !== "waiting_main_channel" &&
      st !== "waiting_channel" &&
      st !== "waiting_edit_channel"
    ) {
      return next();
    }
    ctx.session.state = "idle";
    await ctx.reply("⟨ Orqaga qaytildi.", { reply_markup: mainKb() });
  });

  bot.on("message", async (ctx, next) => {
    if (!isAdmin(ctx)) return next();
    const state = ctx.session.state;
    if (state === "idle") return next();

    // ===== chat_shared (request_chat orqali kanal tanlangan) =====
    const chatShared = (ctx.message as any).chat_shared as
      | { request_id: number; chat_id: number; title?: string; username?: string }
      | undefined;
    if (chatShared) {
      // request_id mapping:  even = admin qilish, odd = ID olish
      //  200/201 = main_channel,  202/203 = waiting_channel (new anime),  204/205 = waiting_edit_channel
      const isAdminBtn = [200, 202, 204].includes(chatShared.request_id);
      const isIdBtn = [201, 203, 205].includes(chatShared.request_id);
      const matchesState =
        (state === "waiting_main_channel" && (chatShared.request_id === 200 || chatShared.request_id === 201)) ||
        (state === "waiting_channel" && (chatShared.request_id === 202 || chatShared.request_id === 203)) ||
        (state === "waiting_edit_channel" && (chatShared.request_id === 204 || chatShared.request_id === 205));

      if (!matchesState) return next();

      if (isAdminBtn) {
        await ctx.reply(
          "✅ Bot kanalga admin qilindi.\n\n" +
            "Endi 🆔 <b>Kanal IDsi botga yuborish</b> tugmasini bosib, oʻsha kanalni tanlang.",
          { parse_mode: "HTML" }
        );
        return;
      }

      if (isIdBtn) {
        const chanelId = String(chatShared.chat_id);
        if (state === "waiting_main_channel") {
          try {
            await setSetting("main_channel_id", chanelId);
            ctx.session.state = "idle";
            let username = "";
            try {
              const chat = await ctx.api.getChat(chanelId);
              if ("username" in chat && chat.username) username = "@" + chat.username;
            } catch {}
            await ctx.reply(
              `✅ Asosiy kanal saqlandi.\n\nID: <code>${chanelId}</code>${username ? `\nUsername: ${username}` : ""}`,
              { parse_mode: "HTML", reply_markup: mainKb() }
            );
          } catch (e) {
            ctx.session.state = "idle";
            console.error("set main channel error:", e);
            await ctx.reply("❌ Xato: " + String(e), { reply_markup: mainKb() });
          }
          return;
        }
        if (state === "waiting_channel") {
          ctx.session.pendingAnime.chanel_id = chanelId;
          ctx.session.state = "waiting_photo";
          await ctx.reply(
            `✅ Kanal IDsi olindi: <code>${chanelId}</code>\n\nAnime uchun surat (photo) yuboring:`,
            { parse_mode: "HTML", reply_markup: mainKb() }
          );
          return;
        }
        if (state === "waiting_edit_channel") {
          const animeId = ctx.session.editingAnimeId;
          if (!animeId) {
            ctx.session.state = "idle";
            return;
          }
          await updateAnimeField(animeId, "chanel_id", chanelId);
          ctx.session.state = "idle";
          await ctx.reply(
            `✅ Kanal IDsi yangilandi: <code>${chanelId}</code>`,
            { parse_mode: "HTML", reply_markup: mainKb() }
          );
          await sendAnimeDetail(ctx, animeId);
          return;
        }
      }
    }

    if (state === "waiting_main_channel") {
      const text = ctx.message.text?.trim() ?? "";
      // Allow user to escape stuck state by sending a command or menu button
      if (text.startsWith("/") || text === "📹 Animelarni boshqarish" || text === "📢 Asosiy Kanal") {
        ctx.session.state = "idle";
        return next();
      }
      let chanelId: string | undefined;
      const fwd: any = (ctx.message as any).forward_origin;
      if (fwd && fwd.type === "channel" && fwd.chat?.id) {
        chanelId = String(fwd.chat.id);
      } else if ((ctx.message as any).forward_from_chat?.id) {
        chanelId = String((ctx.message as any).forward_from_chat.id);
      } else if (text.startsWith("-100")) {
        chanelId = text;
      }
      if (!chanelId) {
        await ctx.reply("❌ Kanal ID topilmadi. -100 bilan boshlangan ID yuboring yoki kanaldan post forward qiling.\n\nBekor qilish: /admin");
        return;
      }
      try {
        await setSetting("main_channel_id", chanelId);
        ctx.session.state = "idle";
        let username = "";
        try {
          const chat = await ctx.api.getChat(chanelId);
          if ("username" in chat && chat.username) username = "@" + chat.username;
        } catch {}
        await ctx.reply(
          `✅ Asosiy kanal saqlandi.\n\nID: <code>${chanelId}</code>${username ? `\nUsername: ${username}` : ""}`,
          { parse_mode: "HTML" }
        );
      } catch (e) {
        ctx.session.state = "idle";
        console.error("set main channel error:", e);
        await ctx.reply("❌ Xato: " + String(e));
      }
      return;
    }

    if (state === "waiting_id") {
      const text = ctx.message.text?.trim() ?? "";
      const id = parseInt(text, 10);
      if (isNaN(id) || id <= 0) {
        await ctx.reply("❌ Iltimos, musbat butun son yuboring.");
        return;
      }
      try {
        const exists = await idExists(id);
        if (exists) {
          const smallest = await getSmallestAvailableId();
          await ctx.reply(
            `⚠️ Bu kod bazada mavjud. Iltimos yangi kod yuboring yoki mana bu kodni yuboring: <b>${smallest}</b>`,
            { parse_mode: "HTML" }
          );
          return;
        }
        ctx.session.pendingAnime.id = id;
        ctx.session.state = "waiting_name";
        await ctx.reply("✅ ID qabul qilindi.\n\nAnime uchun nom yuboring:");
      } catch (e) {
        console.error("waiting_id error:", e);
        await ctx.reply("❌ Xato: " + String(e));
      }
      return;
    }

    if (state === "waiting_name") {
      const name = ctx.message.text?.trim() ?? "";
      if (!name) { await ctx.reply("❌ Iltimos, nom yuboring."); return; }
      ctx.session.pendingAnime.name = name;
      ctx.session.state = "waiting_info";
      await ctx.reply("✅ Nom saqlandi.\n\nAnime uchun tavsif yuboring:");
      return;
    }

    if (state === "waiting_info") {
      const info = ctx.message.text?.trim() ?? "";
      if (!info) { await ctx.reply("❌ Iltimos, tavsif yuboring."); return; }
      ctx.session.pendingAnime.info = info;
      ctx.session.state = "waiting_channel";
      await ctx.reply(
        "✅ Tavsif saqlandi.\n\n📡 <b>Animening oʻz kanali</b>\n\n" + chatPickerHelpText(),
        { parse_mode: "HTML" }
      );
      await ctx.reply("👇 Pastdagi tugmalardan foydalaning:", {
        reply_markup: chatPickerKb({ reqAdminId: 202, reqIdOnlyId: 203 }),
      });
      return;
    }

    if (state === "waiting_channel") {
      let chanelId: string | undefined;

      if (ctx.message.forward_origin) {
        const origin = ctx.message.forward_origin;
        if (origin.type === "channel") {
          chanelId = String(origin.chat.id);
          await ctx.reply(`✅ Kanal ID aniqlandi: <code>${chanelId}</code>`, { parse_mode: "HTML" });
        }
      }

      if (!chanelId) {
        const text = ctx.message.text?.trim() ?? "";
        if (text.startsWith("-100")) {
          chanelId = text;
          await ctx.reply(`✅ Kanal ID qabul qilindi: <code>${chanelId}</code>`, { parse_mode: "HTML" });
        }
      }

      if (!chanelId) {
        await ctx.reply("❌ Kanal ID topilmadi. Iltimos -100 bilan boshlanadigan ID yoki kanaldan post yuboring.");
        return;
      }

      ctx.session.pendingAnime.chanel_id = chanelId;
      ctx.session.state = "waiting_photo";
      await ctx.reply("✅ Kanal ID saqlandi.\n\nAnime uchun surat (photo) yuboring:");
      return;
    }

    if (state === "waiting_photo") {
      const photo = ctx.message.photo;
      if (!photo || photo.length === 0) {
        await ctx.reply("❌ Iltimos, rasm yuboring.");
        return;
      }
      const pending = ctx.session.pendingAnime;
      if (pending.id === undefined || !pending.name || !pending.info || !pending.chanel_id) {
        await ctx.reply("❌ Ma'lumotlar to'liq emas. /admin orqali qaytadan boshlang.");
        ctx.session.state = "idle";
        return;
      }
      try {
        const caption =
          `🎬 <b>${pending.name}</b>\n\n` +
          `📖 <i>${pending.info}</i>\n\n` +
          `🔢 ID: <code>${pending.id}</code>`;

        const sent = await ctx.api.sendPhoto(
          config.SAVED_CHANNEL_ID,
          photo[photo.length - 1].file_id,
          { caption, parse_mode: "HTML" }
        );
        const fileId = sent.photo[sent.photo.length - 1].file_id;

        await insertAnime({
          id: pending.id,
          name: pending.name,
          info: pending.info,
          chanel_id: pending.chanel_id,
          file_id: fileId,
        });

        ctx.session.state = "idle";
        ctx.session.pendingAnime = {};

        await ctx.reply(
          `✅ <b>${pending.name}</b> muvaffaqiyatli saqlandi!\n\n🔢 ID: <code>${pending.id}</code>\n📁 File ID: <code>${fileId}</code>`,
          { parse_mode: "HTML" }
        );
      } catch (e) {
        console.error("waiting_photo error:", e);
        await ctx.reply("❌ Xato yuz berdi: " + String(e));
      }
      return;
    }

    if (state === "waiting_edit_name") {
      const name = ctx.message.text?.trim() ?? "";
      if (!name) { await ctx.reply("❌ Iltimos, nom yuboring."); return; }
      const animeId = ctx.session.editingAnimeId;
      if (!animeId) return;
      await updateAnimeField(animeId, "name", name);
      ctx.session.state = "idle";
      await ctx.reply(`✅ Nom yangilandi: <b>${name}</b>`, { parse_mode: "HTML" });
      await sendAnimeDetail(ctx, animeId);
      return;
    }

    if (state === "waiting_edit_info") {
      const info = ctx.message.text?.trim() ?? "";
      if (!info) { await ctx.reply("❌ Iltimos, tavsif yuboring."); return; }
      const animeId = ctx.session.editingAnimeId;
      if (!animeId) return;
      await updateAnimeField(animeId, "info", info);
      ctx.session.state = "idle";
      await ctx.reply("✅ Tavsif yangilandi.");
      await sendAnimeDetail(ctx, animeId);
      return;
    }

    if (state === "waiting_edit_channel") {
      const text = ctx.message.text?.trim() ?? "";
      if (!text.startsWith("-100")) {
        await ctx.reply("❌ Iltimos -100 bilan boshlanadigan kanal ID yuboring.");
        return;
      }
      const animeId = ctx.session.editingAnimeId;
      if (!animeId) return;
      await updateAnimeField(animeId, "chanel_id", text);
      ctx.session.state = "idle";
      await ctx.reply(`✅ Kanal ID yangilandi: <code>${text}</code>`, { parse_mode: "HTML" });
      await sendAnimeDetail(ctx, animeId);
      return;
    }

    if (state === "waiting_edit_id") {
      const text = ctx.message.text?.trim() ?? "";
      const newId = parseInt(text, 10);
      if (isNaN(newId) || newId <= 0) {
        await ctx.reply("❌ Iltimos, musbat butun son yuboring.");
        return;
      }
      const animeId = ctx.session.editingAnimeId;
      if (!animeId) return;
      const exists = await idExists(newId);
      if (exists && newId !== animeId) {
        const smallest = await getSmallestAvailableId();
        await ctx.reply(
          `⚠️ Bu ID bazada mavjud. Bo'sh ID: <b>${smallest}</b>`,
          { parse_mode: "HTML" }
        );
        return;
      }
      await updateAnimeId(animeId, newId);
      ctx.session.state = "idle";
      ctx.session.editingAnimeId = newId;
      await ctx.reply(`✅ ID yangilandi: <code>${newId}</code>`, { parse_mode: "HTML" });
      await sendAnimeDetail(ctx, newId);
      return;
    }

    if (state === "waiting_edit_photo") {
      const photo = ctx.message.photo;
      if (!photo || photo.length === 0) {
        await ctx.reply("❌ Iltimos, rasm yuboring.");
        return;
      }
      const animeId = ctx.session.editingAnimeId;
      if (!animeId) return;
      try {
        const fileId = photo[photo.length - 1].file_id;
        const anime = await getAnimeById(animeId);
        if (!anime) { await ctx.reply("Anime topilmadi."); return; }

        const sent = await ctx.api.sendPhoto(
          config.SAVED_CHANNEL_ID,
          fileId,
          {
            caption: `🎬 <b>${anime.name}</b>\n\n📖 <i>${anime.info}</i>\n\n🔢 ID: <code>${anime.id}</code>`,
            parse_mode: "HTML",
          }
        );
        const newFileId = sent.photo[sent.photo.length - 1].file_id;
        await updateAnimeField(animeId, "file_id", newFileId);
        ctx.session.state = "idle";
        await ctx.reply("✅ Surat yangilandi.");
        await sendAnimeDetail(ctx, animeId);
      } catch (e) {
        await ctx.reply("❌ Xato: " + String(e));
      }
      return;
    }

    // ===== waiting_admin_id =====
    if (state === "waiting_admin_id") {
      const text = ctx.message.text?.trim() ?? "";
      if (!text) return;

      const admins = getSubAdmins();
      if (admins.length >= 5) {
        ctx.session.state = "idle";
        await ctx.reply("❌ Maksimal 5 ta sub-admin qo'shish mumkin.", { reply_markup: mainKb() });
        return;
      }

      // Foydalanuvchini botdagi ID, Telegram ID yoki telefon orqali qidirish
      const user = await findUserByIdentifier(text);
      if (!user) {
        await ctx.reply(
          "❌ Foydalanuvchi topilmadi.\n\n" +
          "Iltimos, to'g'ri ma'lumot yuboring:\n" +
          "• Botdagi ID raqami\n" +
          "• Telegram ID raqami\n" +
          "• Telefon raqami (+998...)\n\n" +
          "Bekor qilish: /admin"
        );
        return;
      }

      if (user.user_id === config.ADMIN_ID) {
        await ctx.reply("⚠️ Bu asosiy admin. Bekor qilish: /admin");
        return;
      }

      if (admins.includes(user.user_id)) {
        const uname = user.username ? ` (@${user.username})` : "";
        await ctx.reply(
          `⚠️ <code>${user.user_id}</code>${uname} allaqachon admin.\n\nBekor qilish: /admin`,
          { parse_mode: "HTML" }
        );
        return;
      }

      ctx.session.state = "idle";
      const botUsername = ctx.me.username ?? "bot";

      // Taklifni xotirada saqlash
      pendingAdminInvites.set(user.user_id, { inviterId: ctx.from!.id, botName: botUsername });

      // Kandidatga taklif xabari yuborish
      const inviteKb = styleAll(
        new InlineKeyboard()
          .text("✅ Ha, admin bo'laman", `adm_invite_yes:${user.user_id}`).row()
          .text("❌ Yo'q, hohlamayman", `adm_invite_no:${user.user_id}`)
      );
      try {
        await ctx.api.sendMessage(
          user.user_id,
          `📨 Sizni @${botUsername} ga admin sifatida taklif qilishyabdi.\n\nQuyidagilardan birini tanlang:`,
          { reply_markup: inviteKb }
        );
        const displayName = user.username ? `@${user.username}` : `<code>${user.user_id}</code>`;
        await ctx.reply(
          `✅ Taklif ${displayName} ga yuborildi.\n\nU tasdiqlasa, adminlar ro'yxatiga qo'shiladi.`,
          { parse_mode: "HTML", reply_markup: mainKb() }
        );
      } catch {
        pendingAdminInvites.delete(user.user_id);
        await ctx.reply(
          `❌ Foydalanuvchiga xabar yuborib bo'lmadi.\n\nFoydalanuvchi avval botga /start bosishi kerak.\n\nBekor qilish: /admin`
        );
      }
      return;
    }
  });
}

/** Faqat superadmin: barcha test ma'lumotlarini tozalash */
export function setupResetData(bot: Bot<MyContext>): void {
  bot.command("resetdata", async (ctx) => {
    if (ctx.from?.id !== config.ADMIN_ID) return;
    const kb = new InlineKeyboard()
      .text("✅ Ha, o'chirish", "resetdata:confirm")
      .text("❌ Bekor qilish", "resetdata:cancel");
    await ctx.reply(
      "⚠️ <b>TO'LIQ TOZALASH!</b>\n\nQuyidagilar <b>batamom o'chiriladi</b>:\n• Barcha anime, fasl va qismlar\n• Barcha statistika (like, save, yuklash...)\n• Foydalanuvchi ma'lumotlari va sessiyalar\n• Navbat (post_queue)\n\n⚠️ Bu amalni <b>qaytarib bo'lmaydi!</b>\n\nDavom etasizmi?",
      { parse_mode: "HTML", reply_markup: kb }
    );
  });

  bot.callbackQuery("resetdata:confirm", async (ctx) => {
    if (ctx.from?.id !== config.ADMIN_ID) { await ctx.answerCallbackQuery(); return; }
    await ctx.answerCallbackQuery("O'chirilmoqda...");
    try {
      await clearAllData();
      await ctx.editMessageText("✅ Barcha ma'lumotlar o'chirildi. Baza tozalandi.");
    } catch (e) {
      await ctx.editMessageText("❌ Xato: " + String(e));
    }
  });

  bot.callbackQuery("resetdata:cancel", async (ctx) => {
    await ctx.answerCallbackQuery("Bekor qilindi");
    await ctx.deleteMessage().catch(() => {});
  });
}

export function createSession() {
  return session<SessionData, MyContext>({
    initial: (): SessionData => ({
      state: "idle",
      pendingAnime: {},
      listPage: 1,
    }),
    // bot_id ni ham kalitga qo'shamiz — har bir bot o'z sessioniga ega bo'ladi
    getSessionKey: (ctx) => {
      const chatId = ctx.chat?.id;
      if (chatId === undefined) return undefined;
      return `${ctx.me.id}:${chatId}`;
    },
    storage: new D1SessionAdapter<SessionData>(),
  });
}
