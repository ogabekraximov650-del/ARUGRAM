import { Bot, InlineKeyboard, InputFile } from "grammy";
import { config } from "../config.ts";
import { checkIsAdmin } from "../admin_check.ts";
import type { MyContext } from "./admin.ts";
import {
  getAnimeById,
  getSeasonsByAnime,
  getAnimeInfo,
  insertAnimeInfo,
  deleteAnimeInfo,
  updateAnimeInfoText,
  updateAnimeInfoNumber,
  getEpisodesByAnimeInfo,
  getEpisode,
  countEpisodesByQuality,
  upsertEpisodeQuality,
  deleteEpisode,
  insertEpisode,
  updateEpisodeText,
  updateEpisodeNumber,
  getNextDownloadId,
  getNextEpisodeNumber,
  checkAndRecordDownload,
  getEpisodeWithContextByEpisodeId,
  getSetting,
  getAvailableQualities,
  getEpisodeFileIds,
  setEpisodeQualityFiles,
  clearEpisodeQuality,
  QUALITIES,
  type AnimeInfo,
  type Episode,
  type Kategoriya,
  type Quality,
} from "../db.ts";

function buildSeasonNumberPart(info: AnimeInfo): string {
  if (info.kategoriya !== "TV") return "";
  const sn = info.season_id ?? null;
  return sn ? ` {${sn}-fasl}` : "";
}

const EP_PAGE_SIZE = 30;

function isAdmin(ctx: MyContext): boolean {
  return checkIsAdmin(ctx.from?.id);
}

function pad2(n: number): string {
  return n < 10 ? `0${n}` : String(n);
}

function styleAll(kb: InlineKeyboard): InlineKeyboard {
  for (const row of kb.inline_keyboard) {
    for (const btn of row) (btn as any).style = "primary";
  }
  return kb;
}

async function getMainChannelUsername(ctx: MyContext): Promise<string> {
  try {
    const id = await getSetting("main_channel_id");
    if (!id) return "";
    const chat = await ctx.api.getChat(id);
    if ("username" in chat && chat.username) return "@" + chat.username;
  } catch {}
  return "";
}

export function buildEpisodeCaption(
  ep: Episode,
  info: AnimeInfo,
  animeName: string,
  channelUsername: string,
  withDownloads: boolean,
  quality?: Quality | null
): string {
  const seasonPart = buildSeasonNumberPart(info);
  // Use ep.name if it differs from anime name (admin-customized), else fallback to animeName
  const displayName = (ep.name && ep.name.trim()) ? ep.name.trim() : animeName;
  let line = "";
  if (info.kategoriya === "TV") {
    line = `${displayName}${seasonPart} [${pad2(ep.epizod_number ?? 0)}-qism]`;
  } else if (info.kategoriya === "FILM") {
    line = `${displayName} [${ep.film_id}-Film]`;
  } else {
    line = `${displayName} [${pad2(ep.ova_id ?? 0)}-qism]`;
  }
  let out = "";
  if (quality) out += `Sifati: ${quality} formatda\n`;
  out += `${line}\nKanal: ${channelUsername || "—"}`;
  if (withDownloads) out += `\n📥Yuklangan: ${ep.downloads} marta`;
  return out;
}

function buildEpisodeCaptionFromInputs(
  info: AnimeInfo,
  animeName: string,
  episodeNum: number,
  channelUsername: string,
  downloads?: number,
  quality?: Quality | null
): string {
  const seasonPart = buildSeasonNumberPart(info);
  let line = "";
  if (info.kategoriya === "TV") {
    line = `${animeName}${seasonPart} [${pad2(episodeNum)}-qism]`;
  } else if (info.kategoriya === "FILM") {
    line = `${animeName} [${episodeNum}-Film]`;
  } else {
    line = `${animeName} [${pad2(episodeNum)}-qism]`;
  }
  let out = "";
  if (quality) out += `Sifati: ${quality} formatda\n`;
  out += `${line}\nKanal: ${channelUsername || "—"}`;
  if (downloads !== undefined) out += `\n📥Yuklangan: ${downloads} marta`;
  return out;
}

function pickDefaultQuality(ep: Episode, preferred?: Quality): Quality | null {
  if (preferred) return preferred;
  const avail = getAvailableQualities(ep);
  return avail.length > 0 ? avail[0] : null; // QUALITIES order: 1080p > 720p > 480p > 360p
}

async function showSeasons(ctx: MyContext, animeId: number, editMsgId?: number): Promise<void> {
  const seasons = await getSeasonsByAnime(animeId);
  const kb = new InlineKeyboard();
  for (const s of seasons) {
    kb.text(s.name, `season:${s.rowid}`).row();
  }
  kb.text("➕ Yangi qo'shish", `season_add:${animeId}`).row();
  kb.text("« Orqaga", `anime:${animeId}`);
  styleAll(kb);

  const text = seasons.length === 0
    ? "📭 Hali fasllar/film/ova yo'q.\n\nQuyidagilardan birini tanlang:"
    : "Quyidagilardan birini tanlang:";

  if (editMsgId) {
    try {
      await ctx.api.editMessageText(ctx.chat!.id, editMsgId, text, { reply_markup: kb });
      return;
    } catch {}
    try {
      await ctx.api.editMessageCaption(ctx.chat!.id, editMsgId, { caption: text, reply_markup: kb });
      return;
    } catch {}
    try { await ctx.api.deleteMessage(ctx.chat!.id, editMsgId); } catch {}
  }
  const msg = await ctx.reply(text, { reply_markup: kb });
  ctx.session.manageMsgId = msg.message_id;
}

async function showEpisodes(ctx: MyContext, animeInfoId: number, page: number, editMsgId?: number): Promise<void> {
  const info = await getAnimeInfo(animeInfoId);
  if (!info) {
    await ctx.reply("❌ Topilmadi.");
    return;
  }
  const eps = await getEpisodesByAnimeInfo(animeInfoId);
  const total = eps.length;
  const totalPages = Math.max(1, Math.ceil(total / EP_PAGE_SIZE));
  const cur = Math.min(Math.max(1, page), totalPages);
  const start = (cur - 1) * EP_PAGE_SIZE;
  const slice = eps.slice(start, start + EP_PAGE_SIZE);

  const kb = new InlineKeyboard();
  // 3 columns
  for (let i = 0; i < slice.length; i += 3) {
    const row = slice.slice(i, i + 3);
    for (const ep of row) {
      const num = ep.epizod_number ?? ep.film_id ?? ep.ova_id ?? 0;
      const label = info.kategoriya === "FILM" ? `${num}-Film` : `${num}-qism`;
      kb.text(label, `ep:${ep.rowid}`);
    }
    kb.row();
  }
  kb.text("➕ Yangi qism qo'shish", `ep_add:${animeInfoId}`).row();
  kb.text("🪧 Kanalga post joylash", `ep_post:${animeInfoId}`).row();
  kb.text("📝 Anime malumotlarini tahrirlash", `season_edit:${animeInfoId}`).row();
  kb.text("🗑 Animeni o'chirish", `season_delete:${animeInfoId}`).row();
  if (totalPages > 1) {
    if (cur > 1) kb.text("« oldingi", `ep_page:${animeInfoId}:${cur - 1}`);
    kb.text(`${cur}/${totalPages}`, "noop");
    if (cur < totalPages) kb.text("keyingi »", `ep_page:${animeInfoId}:${cur + 1}`);
    kb.row();
  }
  kb.text("« orqaga qaytish", `edit_episodes:${info.id}`);
  styleAll(kb);

  const text = `<b>${info.name}</b>\n\nJami qismlar soni: <b>${total}/${total}</b>`;

  ctx.session.currentAnimeInfoId = animeInfoId;
  ctx.session.episodesPage = cur;

  if (editMsgId) {
    try {
      await ctx.api.editMessageText(ctx.chat!.id, editMsgId, text, {
        parse_mode: "HTML",
        reply_markup: kb,
      });
      ctx.session.manageMsgId = editMsgId;
      return;
    } catch {}
    try { await ctx.api.deleteMessage(ctx.chat!.id, editMsgId); } catch {}
  }
  const msg = await ctx.reply(text, { parse_mode: "HTML", reply_markup: kb });
  ctx.session.manageMsgId = msg.message_id;
}

async function showEpisodeDetail(ctx: MyContext, episodeRowid: number, preferredQuality?: Quality): Promise<void> {
  const ep = await getEpisode(episodeRowid);
  if (!ep) { await ctx.reply("❌ Topilmadi."); return; }
  const info = await getAnimeInfo(ep.seasondb_id);
  if (!info) { await ctx.reply("❌ Anime info topilmadi."); return; }
  const anime = await getAnimeById(info.id);
  const animeName = anime?.name ?? info.name;

  const channelUsername = await getMainChannelUsername(ctx);
  const available = getAvailableQualities(ep);
  // Determine quality to display
  let displayQ: Quality | null = preferredQuality ?? ctx.session.currentEpisodeQuality ?? null;
  if (displayQ && !available.includes(displayQ) && preferredQuality === undefined) {
    displayQ = null; // fallback
  }
  if (!displayQ) {
    displayQ = pickDefaultQuality(ep) ?? "720p";
  }
  ctx.session.currentEpisodeQuality = displayQ;

  // Build keyboard
  const kb = new InlineKeyboard();
  // Row 1: 4 quality toggles
  for (const q of QUALITIES) {
    const has = available.includes(q);
    const mark = has ? "✅" : "❌";
    kb.text(`${mark}${q}`, `ep_q:${ep.rowid}:${q}`);
  }
  kb.row();
  kb.text("📝 Nomini tahrirlash", `ep_edit_name:${ep.rowid}`).row();
  kb.text(
    info.kategoriya === "TV" ? "📝 Qism raqamini tahrirlash" :
    info.kategoriya === "FILM" ? "📝 Film raqamini tahrirlash" :
    "📝 OVA raqamini tahrirlash",
    `ep_edit_num:${ep.rowid}`
  ).row();
  kb.text(`📝 ${displayQ} videoni tahrirlash`, `ep_edit_q:${ep.rowid}:${displayQ}`).row();
  kb.text(`🗑 ${displayQ} videoni o'chirish`, `ep_delete_q:${ep.rowid}:${displayQ}`).row();
  kb.text("🗑 Barchasini o'chirish", `ep_delete:${ep.rowid}`).row();
  kb.text("« Orqaga", `season:${ep.seasondb_id}`);
  styleAll(kb);

  if (ctx.session.manageMsgId) {
    try { await ctx.api.deleteMessage(ctx.chat!.id, ctx.session.manageMsgId); } catch {}
    ctx.session.manageMsgId = undefined;
  }

  if (available.includes(displayQ)) {
    const caption = buildEpisodeCaption(ep, info, animeName, channelUsername, true, displayQ);
    const ids = getEpisodeFileIds(ep, displayQ);
    let msg;
    try {
      msg = await ctx.api.sendVideo(ctx.chat!.id, ids.chanel ?? ids.bot ?? "", {
        caption, reply_markup: kb,
      });
    } catch {
      msg = await ctx.api.sendVideo(ctx.chat!.id, ids.bot ?? ids.chanel ?? "", {
        caption, reply_markup: kb,
      });
    }
    ctx.session.manageMsgId = msg.message_id;
  } else {
    const text =
      `Botga hali <b>${displayQ}</b> formatdagi video qo'shilmagan.\n\n` +
      `Agar qo'shmoqchi bo'lsangiz pastdagi 📝 ${displayQ} videoni tahrirlash tugmasini bosib qo'shishingiz mumkin.`;
    const msg = await ctx.reply(text, { parse_mode: "HTML", reply_markup: kb });
    ctx.session.manageMsgId = msg.message_id;
  }
}

export function setupEpisodeHandlers(bot: Bot<MyContext>): void {
  // Click "📝 Anime qismlarini tahrirlash" on anime detail (overrides placeholder)
  bot.callbackQuery(/^edit_episodes:(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    await ctx.answerCallbackQuery();
    const animeId = parseInt(ctx.match[1], 10);
    if (ctx.session.manageMsgId) {
      try { await ctx.api.deleteMessage(ctx.chat!.id, ctx.session.manageMsgId); } catch {}
      ctx.session.manageMsgId = undefined;
    }
    try {
      await showSeasons(ctx, animeId);
    } catch (e) {
      console.error("edit_episodes error:", e);
      await ctx.reply("❌ Xato: " + String(e));
    }
  });

  // Add new season/film/ova
  bot.callbackQuery(/^season_add:(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    const animeId = parseInt(ctx.match[1], 10);
    ctx.session.pendingSeason = { animeId };
    ctx.session.state = "waiting_season_name";
    await ctx.answerCallbackQuery();
    await ctx.reply("✏️ Anime uchun nom yuboring (masalan: Naruto 1-fasl):");
  });

  // Click on a season → show episodes
  bot.callbackQuery(/^season:(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    await ctx.answerCallbackQuery();
    const animeInfoId = parseInt(ctx.match[1], 10);
    try {
      await showEpisodes(ctx, animeInfoId, 1, ctx.session.manageMsgId);
    } catch (e) {
      console.error("season error:", e);
      await ctx.reply("❌ Xato: " + String(e));
    }
  });

  bot.callbackQuery(/^ep_page:(\d+):(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    await ctx.answerCallbackQuery();
    const animeInfoId = parseInt(ctx.match[1], 10);
    const page = parseInt(ctx.match[2], 10);
    try {
      await showEpisodes(ctx, animeInfoId, page, ctx.session.manageMsgId);
    } catch (e) {
      console.error("ep_page error:", e);
    }
  });

  // Add new episode — Step 1: quality selection
  bot.callbackQuery(/^ep_add:(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    const animeInfoId = parseInt(ctx.match[1], 10);
    const info = await getAnimeInfo(animeInfoId);
    if (!info) { await ctx.answerCallbackQuery("Topilmadi"); return; }
    ctx.session.currentAnimeInfoId = animeInfoId;
    await ctx.answerCallbackQuery();
    const kb = styleAll(new InlineKeyboard()
      .text("1080p", `ep_add_q:${animeInfoId}:1080p`)
      .text("720p", `ep_add_q:${animeInfoId}:720p`).row()
      .text("480p", `ep_add_q:${animeInfoId}:480p`)
      .text("360p", `ep_add_q:${animeInfoId}:360p`));
    await ctx.reply("✅ Sifatni tanlang:", { reply_markup: kb });
  });

  // Add new episode — Step 2: quality chosen → start auto-add for that quality
  bot.callbackQuery(/^ep_add_q:(\d+):(1080p|720p|480p|360p)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    const animeInfoId = parseInt(ctx.match[1], 10);
    const quality = ctx.match[2] as Quality;
    const info = await getAnimeInfo(animeInfoId);
    if (!info) { await ctx.answerCallbackQuery("Topilmadi"); return; }
    const [next, qualityCount] = await Promise.all([
      getNextEpisodeNumber(animeInfoId, info.kategoriya, quality),
      countEpisodesByQuality(animeInfoId, quality),
    ]);
    ctx.session.currentAnimeInfoId = animeInfoId;
    ctx.session.currentQuality = quality;
    ctx.session.state = "waiting_episode_video";
    await ctx.answerCallbackQuery(`Sifat: ${quality}`);
    try { await ctx.editMessageReplyMarkup({ reply_markup: undefined }); } catch {}
    const word = info.kategoriya === "FILM" ? "film" : "qism";
    await ctx.reply(
      `✅ Sifat tanlandi.\n\nSifat formati: <b>${quality}</b>\n\n${quality} sifatda qo'shilgan ${word}lar: <b>${qualityCount} ta</b>\n\n<b>${next}-${word}</b>dan boshlab video yuboring:`,
      { parse_mode: "HTML" }
    );
  });

  // Click on episode → show video (default = highest available quality)
  bot.callbackQuery(/^ep:(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    await ctx.answerCallbackQuery();
    const rowid = parseInt(ctx.match[1], 10);
    ctx.session.currentEpisodeQuality = undefined; // reset → use default
    try {
      await showEpisodeDetail(ctx, rowid);
    } catch (e) {
      console.error("ep detail error:", e);
      await ctx.reply("❌ Xato: " + String(e));
    }
  });

  // Switch displayed quality (✅/❌ click)
  bot.callbackQuery(/^ep_q:(\d+):(1080p|720p|480p|360p)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    const rowid = parseInt(ctx.match[1], 10);
    const quality = ctx.match[2] as Quality;
    await ctx.answerCallbackQuery(quality);
    try {
      await showEpisodeDetail(ctx, rowid, quality);
    } catch (e) {
      console.error("ep_q error:", e);
    }
  });

  // Edit episode video for a specific quality
  bot.callbackQuery(/^ep_edit_q:(\d+):(1080p|720p|480p|360p)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    const rowid = parseInt(ctx.match[1], 10);
    const quality = ctx.match[2] as Quality;
    ctx.session.editingEpisodeRowid = rowid;
    ctx.session.editingEpisodeQuality = quality;
    ctx.session.state = "waiting_edit_episode_video";
    await ctx.answerCallbackQuery();
    await ctx.reply(`✏️ <b>${quality}</b> formatdagi yangi videoni yuboring:`, { parse_mode: "HTML" });
  });

  // Delete a single quality from an episode
  bot.callbackQuery(/^ep_delete_q:(\d+):(1080p|720p|480p|360p)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    const rowid = parseInt(ctx.match[1], 10);
    const quality = ctx.match[2] as Quality;
    const ep = await getEpisode(rowid);
    if (!ep) { await ctx.answerCallbackQuery("Topilmadi"); return; }
    const ids = getEpisodeFileIds(ep, quality);
    if (!ids.chanel && !ids.bot) {
      await ctx.answerCallbackQuery(`${quality} mavjud emas`);
      return;
    }
    await ctx.answerCallbackQuery();
    const kb = styleAll(new InlineKeyboard()
      .text("✅ Ha", `ep_delete_q_yes:${rowid}:${quality}`)
      .text("❌ Yo'q", `ep:${rowid}`));
    await ctx.reply(`⚠️ <b>${quality}</b> formatdagi videoni o'chirmoqchimisiz?`, {
      parse_mode: "HTML", reply_markup: kb
    });
  });

  bot.callbackQuery(/^ep_delete_q_yes:(\d+):(1080p|720p|480p|360p)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    const rowid = parseInt(ctx.match[1], 10);
    const quality = ctx.match[2] as Quality;
    await clearEpisodeQuality(rowid, quality);
    await ctx.answerCallbackQuery("🗑 O'chirildi");
    try { await ctx.deleteMessage(); } catch {}
    ctx.session.currentEpisodeQuality = undefined;
    await showEpisodeDetail(ctx, rowid);
  });

  bot.callbackQuery(/^ep_edit_name:(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    ctx.session.editingEpisodeRowid = parseInt(ctx.match[1], 10);
    ctx.session.state = "waiting_edit_episode_name";
    await ctx.answerCallbackQuery();
    await ctx.reply("✏️ Yangi nomni yuboring:");
  });

  bot.callbackQuery(/^ep_edit_num:(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    ctx.session.editingEpisodeRowid = parseInt(ctx.match[1], 10);
    ctx.session.state = "waiting_edit_episode_number";
    await ctx.answerCallbackQuery();
    await ctx.reply("✏️ Yangi raqam yuboring:");
  });

  bot.callbackQuery(/^ep_delete:(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    const rowid = parseInt(ctx.match[1], 10);
    const ep = await getEpisode(rowid);
    if (!ep) { await ctx.answerCallbackQuery("Topilmadi"); return; }
    const num = ep.epizod_number ?? ep.film_id ?? ep.ova_id ?? 0;
    const info = await getAnimeInfo(ep.seasondb_id);
    const word = info?.kategoriya === "FILM" ? "film" : "qism";
    await ctx.answerCallbackQuery();
    const kb = styleAll(new InlineKeyboard()
      .text("✅ Ha", `ep_delete_yes:${rowid}`)
      .text("❌ Yo'q", `ep:${rowid}`));
    await ctx.reply(`⚠️ <b>${num}-${word}</b>ni o'chirmoqchimisiz?`, {
      parse_mode: "HTML", reply_markup: kb
    });
  });

  bot.callbackQuery(/^ep_delete_yes:(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    const rowid = parseInt(ctx.match[1], 10);
    const ep = await getEpisode(rowid);
    if (!ep) { await ctx.answerCallbackQuery("Topilmadi"); return; }
    await deleteEpisode(rowid);
    await ctx.answerCallbackQuery("🗑 O'chirildi");
    try { await ctx.deleteMessage(); } catch {}
    if (ctx.session.manageMsgId) {
      try { await ctx.api.deleteMessage(ctx.chat!.id, ctx.session.manageMsgId); } catch {}
      ctx.session.manageMsgId = undefined;
    }
    await showEpisodes(ctx, ep.seasondb_id, ctx.session.episodesPage ?? 1);
  });

  // Delete season (and all episodes)
  bot.callbackQuery(/^season_delete:(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    const rowid = parseInt(ctx.match[1], 10);
    const info = await getAnimeInfo(rowid);
    if (!info) { await ctx.answerCallbackQuery("Topilmadi"); return; }
    await ctx.answerCallbackQuery();
    const kb = styleAll(new InlineKeyboard()
      .text("✅ Ha", `season_delete_yes:${rowid}`)
      .text("❌ Yo'q", `season:${rowid}`));
    await ctx.reply(`⚠️ <b>${info.name}</b> va uning barcha qismlarini o'chirmoqchimisiz?`, {
      parse_mode: "HTML", reply_markup: kb
    });
  });

  bot.callbackQuery(/^season_delete_yes:(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    const rowid = parseInt(ctx.match[1], 10);
    const info = await getAnimeInfo(rowid);
    await deleteAnimeInfo(rowid);
    await ctx.answerCallbackQuery("O'chirildi");
    await ctx.editMessageText(`✅ O'chirildi.`);
    if (info) {
      if (ctx.session.manageMsgId) {
        try { await ctx.api.deleteMessage(ctx.chat!.id, ctx.session.manageMsgId); } catch {}
        ctx.session.manageMsgId = undefined;
      }
      await showSeasons(ctx, info.id);
    }
  });

  // ============ Anime info edit panel (📝 Anime malumotlarini tahrirlash) ============
  async function showInfoPanel(ctx: MyContext, infoRowid: number): Promise<void> {
    const info = await getAnimeInfo(infoRowid);
    if (!info) { await ctx.reply("❌ Topilmadi."); return; }
    const anime = await getAnimeById(info.id);
    const cap =
      `<b>Nomi:</b> ${info.name}\n` +
      `<b>Tavsifi:</b> ${info.info}\n` +
      `<b>Kategoriya:</b> ${info.kategoriya}\n` +
      `<b>Fasl raqami:</b> ${info.season_id ?? "—"}\n` +
      `<b>Yuklashlar soni:</b> ${info.downloads}\n` +
      `<b>Anime ID:</b> <code>${info.id}</code>${anime ? ` (${anime.name})` : ""}\n` +
      `<b>Epizoddb ID:</b> <code>${info.epizoddb_id ?? "—"}</code>\n` +
      `<b>Qo'shilgan sana:</b> ${info.data || "—"}`;

    const kb = styleAll(new InlineKeyboard()
      .text("📝 Suratni tahrirlash", `info_edit_photo:${info.rowid}`).row()
      .text("📝 Nomini tahrirlash", `info_edit_name:${info.rowid}`).row()
      .text("📝 Tavsifini tahrirlash", `info_edit_text:${info.rowid}`).row()
      .text("📝 Fasl raqamini tahrirlash", `info_edit_season:${info.rowid}`).row()
      .text("« Orqaga", `season:${info.rowid}`));

    if (ctx.session.manageMsgId) {
      try { await ctx.api.deleteMessage(ctx.chat!.id, ctx.session.manageMsgId); } catch {}
    }
    const msg = await ctx.api.sendPhoto(ctx.chat!.id, info.file_id, {
      caption: cap, parse_mode: "HTML", reply_markup: kb,
    });
    ctx.session.manageMsgId = msg.message_id;
  }

  bot.callbackQuery(/^season_edit:(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    await ctx.answerCallbackQuery();
    const rowid = parseInt(ctx.match[1], 10);
    try {
      await showInfoPanel(ctx, rowid);
    } catch (e) {
      console.error("season_edit error:", e);
      await ctx.reply("❌ Xato: " + String(e));
    }
  });

  bot.callbackQuery(/^info_edit_photo:(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    ctx.session.editingSeasonRowid = parseInt(ctx.match[1], 10);
    ctx.session.state = "waiting_edit_info_photo";
    await ctx.answerCallbackQuery();
    await ctx.reply("✏️ Yangi suratni yuboring:");
  });
  bot.callbackQuery(/^info_edit_name:(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    ctx.session.editingSeasonRowid = parseInt(ctx.match[1], 10);
    ctx.session.state = "waiting_edit_info_name";
    await ctx.answerCallbackQuery();
    await ctx.reply("✏️ Yangi nomni yuboring:");
  });
  bot.callbackQuery(/^info_edit_text:(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    ctx.session.editingSeasonRowid = parseInt(ctx.match[1], 10);
    ctx.session.state = "waiting_edit_info_text";
    await ctx.answerCallbackQuery();
    await ctx.reply("✏️ Yangi tavsifni yuboring:");
  });
  bot.callbackQuery(/^info_edit_season:(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    ctx.session.editingSeasonRowid = parseInt(ctx.match[1], 10);
    ctx.session.state = "waiting_edit_info_season";
    await ctx.answerCallbackQuery();
    await ctx.reply("✏️ Yangi fasl raqamini yuboring (faqat son):");
  });

  // ============ 🪧 Kanalga post joylash ============
  bot.callbackQuery(/^ep_post:(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    await ctx.answerCallbackQuery();
    const animeInfoId = parseInt(ctx.match[1], 10);
    const info = await getAnimeInfo(animeInfoId);
    if (!info) { await ctx.reply("❌ Topilmadi."); return; }
    const anime = await getAnimeById(info.id);
    if (!anime) { await ctx.reply("❌ Anime topilmadi."); return; }
    let username = "";
    try {
      const chat = await ctx.api.getChat(anime.chanel_id);
      if ("username" in chat && chat.username) username = "@" + chat.username;
    } catch {}
    const eps = await getEpisodesByAnimeInfo(animeInfoId);
    const kb = styleAll(new InlineKeyboard()
      .text("✅ Ha", `ep_post_yes:${animeInfoId}`)
      .text("❌ Bekor qilish", `season:${animeInfoId}`));
    await ctx.reply(
      `<b>Animega tegishli kanal IDsi:</b> <code>${anime.chanel_id}</code>\n` +
      `<b>Kanal useri:</b> ${username || "—"}\n` +
      `<b>Joylanadigan postlar:</b> ${eps.length} ta\n\n` +
      `Rostan ham post joylamoqchimisiz?`,
      { parse_mode: "HTML", reply_markup: kb }
    );
  });

  bot.callbackQuery(/^ep_post_yes:(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    await ctx.answerCallbackQuery("Joylanmoqda...");
    const animeInfoId = parseInt(ctx.match[1], 10);
    const info = await getAnimeInfo(animeInfoId);
    if (!info) { await ctx.reply("❌ Topilmadi."); return; }
    const anime = await getAnimeById(info.id);
    if (!anime) { await ctx.reply("❌ Anime topilmadi."); return; }
    const eps = await getEpisodesByAnimeInfo(animeInfoId);
    if (eps.length === 0) { await ctx.reply("❌ Qismlar yo'q."); return; }
    const channelUsername = await getMainChannelUsername(ctx);
    const botUsername = ctx.me.username ?? "";
    const adminChatId = ctx.from!.id;
    // Tartib bo'yicha saralash
    const sortedEps = [...eps].sort(
      (a, b) =>
        (a.epizod_number ?? a.film_id ?? a.ova_id ?? 0) -
        (b.epizod_number ?? b.film_id ?? b.ova_id ?? 0)
    );
    const episodeRowids = sortedEps.map((ep) => ep.rowid);
    // Navbatga qo'shish — Cron trigger har daqiqada qayta ishlaydi
    const batchId = `${animeInfoId}_${Date.now()}`;
    try {
      const { insertQueueBatch } = await import("../db_post_queue.ts");
      await insertQueueBatch(
        batchId,
        adminChatId,
        animeInfoId,
        episodeRowids,
        anime.chanel_id,
        botUsername,
        channelUsername
      );
      await ctx.reply(
        `📋 Post navbatga qo'shildi!\n\n` +
        `Jami: ${episodeRowids.length} ta qism\n` +
        `Har daqiqada ~15 ta joylanadi.\n` +
        `Tugatganda xabar keladi ✅`
      );
    } catch (e) {
      console.error("ep_post_yes insertQueueBatch failed:", e);
      await ctx.reply("❌ Navbatga qo'shishda xato: " + String(e).slice(0, 200));
    }
  });

  // Kategoriya selection (button-based)
  bot.callbackQuery(/^season_kat:(TV|FILM|OVA)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    if (ctx.session.state !== "waiting_season_kategoriya") {
      await ctx.answerCallbackQuery();
      return;
    }
    const kat = ctx.match[1] as Kategoriya;
    ctx.session.pendingSeason = ctx.session.pendingSeason ?? {};
    ctx.session.pendingSeason.kategoriya = kat;
    await ctx.answerCallbackQuery(`Tanlandi: ${kat}`);
    try { await ctx.editMessageReplyMarkup({ reply_markup: undefined }); } catch {}
    if (kat === "TV") {
      ctx.session.state = "waiting_season_number";
      await ctx.reply("✏️ Fasl raqamini yuboring (faqat son):");
    } else {
      ctx.session.state = "waiting_season_photo";
      await ctx.reply("✏️ Anime uchun surat yuboring:");
    }
  });

  // ============ Message handler for season/episode states ============
  const EPISODE_STATES = new Set([
    "waiting_season_name",
    "waiting_season_info",
    "waiting_season_kategoriya",
    "waiting_season_number",
    "waiting_season_photo",
    "waiting_episode_video",
    "waiting_edit_episode_video",
    "waiting_edit_episode_name",
    "waiting_edit_episode_number",
    "waiting_edit_info_photo",
    "waiting_edit_info_name",
    "waiting_edit_info_text",
    "waiting_edit_info_season",
  ]);

  bot.on("message", async (ctx, next) => {
    if (!isAdmin(ctx)) return next();
    const state = ctx.session.state;
    // Only handle episode states; let everything else (admin states, idle) pass through
    if (!EPISODE_STATES.has(state)) return next();
    // Escape stuck state via command/menu button
    const txt = ctx.message.text?.trim() ?? "";
    if (txt.startsWith("/") || txt === "📹 Animelarni boshqarish" || txt === "📢 Asosiy Kanal") {
      ctx.session.state = "idle";
      return next();
    }

    // ----- Season add flow -----
    if (state === "waiting_season_name") {
      const name = ctx.message.text?.trim() ?? "";
      if (!name) { await ctx.reply("❌ Nom yuboring."); return; }
      ctx.session.pendingSeason = ctx.session.pendingSeason ?? {};
      ctx.session.pendingSeason.name = name;
      ctx.session.state = "waiting_season_info";
      await ctx.reply("✅ Nom saqlandi.\n\nAnime uchun tavsif yuboring:");
      return;
    }

    if (state === "waiting_season_info") {
      const info = ctx.message.text?.trim() ?? "";
      if (!info) { await ctx.reply("❌ Tavsif yuboring."); return; }
      ctx.session.pendingSeason = ctx.session.pendingSeason ?? {};
      ctx.session.pendingSeason.info = info;
      ctx.session.state = "waiting_season_kategoriya";
      const kb = styleAll(new InlineKeyboard()
        .text("TV", "season_kat:TV")
        .text("FILM", "season_kat:FILM")
        .text("OVA", "season_kat:OVA"));
      await ctx.reply("Kategoriyani tanlang:", { reply_markup: kb });
      return;
    }

    if (state === "waiting_season_number") {
      const num = parseInt(ctx.message.text?.trim() ?? "", 10);
      if (isNaN(num) || num <= 0) { await ctx.reply("❌ Musbat son yuboring."); return; }
      ctx.session.pendingSeason = ctx.session.pendingSeason ?? {};
      ctx.session.pendingSeason.season_id = num;
      ctx.session.state = "waiting_season_photo";
      await ctx.reply("✅ Fasl saqlandi.\n\nAnime uchun surat yuboring:");
      return;
    }

    if (state === "waiting_season_photo") {
      const photo = ctx.message.photo;
      if (!photo || photo.length === 0) { await ctx.reply("❌ Rasm yuboring."); return; }
      const p = ctx.session.pendingSeason;
      if (!p || !p.animeId || !p.name || !p.info || !p.kategoriya) {
        await ctx.reply("❌ Ma'lumotlar to'liq emas. Boshidan boshlang.");
        ctx.session.state = "idle";
        return;
      }
      const fileId = photo[photo.length - 1].file_id;
      try {
        const result = await insertAnimeInfo({
          id: p.animeId,
          name: p.name,
          info: p.info,
          kategoriya: p.kategoriya,
          season_id: p.season_id ?? null,
          file_id: fileId,
        });
        ctx.session.state = "idle";
        ctx.session.pendingSeason = undefined;
        await ctx.reply(`✅ <b>${p.name}</b> saqlandi.\n\n<b>Epizoddb ID:</b> <code>${result.epizoddb_id}</code>`, { parse_mode: "HTML" });
        await showSeasons(ctx, p.animeId);
      } catch (e) {
        await ctx.reply("❌ Xato: " + String(e));
      }
      return;
    }

    // ----- Add new episode (quality-aware auto-add) -----
    if (state === "waiting_episode_video") {
      const video = ctx.message.video;
      if (!video) { await ctx.reply("❌ Video yuboring."); return; }
      const animeInfoId = ctx.session.currentAnimeInfoId;
      const quality = ctx.session.currentQuality;
      if (!animeInfoId || !quality) {
        await ctx.reply("❌ Sessiya buzildi. /cancel va qaytadan boshlang.");
        ctx.session.state = "idle";
        return;
      }
      const info = await getAnimeInfo(animeInfoId);
      if (!info) { await ctx.reply("❌ Topilmadi."); ctx.session.state = "idle"; return; }
      const anime = await getAnimeById(info.id);
      if (!anime) { await ctx.reply("❌ Anime topilmadi."); ctx.session.state = "idle"; return; }

      try {
        const botFileId = video.file_id;
        const epNum = await getNextEpisodeNumber(animeInfoId, info.kategoriya, quality);
        const downloadId = await getNextDownloadId();
        const channelUsername = await getMainChannelUsername(ctx);
        const captionSaved = buildEpisodeCaptionFromInputs(
          info, anime.name, epNum, channelUsername, undefined, quality
        );

        // 1) Send to saved channel (with video)
        const sentSaved = await ctx.api.sendVideo(config.SAVED_CHANNEL_ID, botFileId, {
          caption: captionSaved,
        });
        const chanelFileId = sentSaved.video?.file_id ?? botFileId;

        // 2) Save to DB — upsert: mavjud bo'lsa yangilaydi, aks holda yangi qator
        const upserted = await upsertEpisodeQuality({
          seasondb_id: animeInfoId,
          epizoddb_id: info.epizoddb_id,
          id: info.id,
          name: info.name,
          epizod_number: info.kategoriya === "TV" ? epNum : null,
          film_id: info.kategoriya === "FILM" ? epNum : null,
          ova_id: info.kategoriya === "OVA" ? epNum : null,
          epizod_id: downloadId,
          quality,
          chanel_id: chanelFileId,
          bot_id: botFileId,
        });

        // 3) Post to anime's chanel_id — faqat YANGI qism uchun
        if (upserted.isNew) {
          try {
            const link = `https://t.me/${ctx.me.username}?start=ep_${upserted.epizod_id}`;
            const kb = new InlineKeyboard().url("• Yuklab Olish •", link);
            // Kanal postida sifat ko'rsatilmaydi
            const captionChannel = buildEpisodeCaptionFromInputs(
              info, anime.name, epNum, channelUsername
            );
            await ctx.api.sendMessage(anime.chanel_id, captionChannel, { reply_markup: kb });
          } catch (e) {
            await ctx.reply(`⚠️ Anime kanaliga (<code>${anime.chanel_id}</code>) post yuborilmadi: ${String(e)}\n\nBot u kanalda admin bo'lishi kerak.`, { parse_mode: "HTML" });
          }
        }

        const word = info.kategoriya === "FILM" ? "film" : "qism";
        const action = upserted.isNew ? "qo'shildi" : "yangilandi";
        // KEEP state at waiting_episode_video for AUTO-ADD flow (same quality)
        await ctx.reply(
          `✅ <b>${epNum}-${word}</b> (${quality}) ${action}.\n\nKeyingi qismni yuboring yoki to'xtatish uchun /cancel buyrug'ini yuboring.`,
          { parse_mode: "HTML" }
        );
      } catch (e) {
        console.error("episode add error:", e);
        await ctx.reply("❌ Xato: " + String(e));
      }
      return;
    }

    // ----- Edit/add per-quality episode video -----
    if (state === "waiting_edit_episode_video") {
      const video = ctx.message.video;
      if (!video) { await ctx.reply("❌ Video yuboring."); return; }
      const rowid = ctx.session.editingEpisodeRowid;
      const quality = ctx.session.editingEpisodeQuality;
      if (!rowid || !quality) {
        await ctx.reply("❌ Sessiya buzildi. /cancel va qaytadan boshlang.");
        ctx.session.state = "idle";
        return;
      }
      const ep = await getEpisode(rowid);
      if (!ep) { await ctx.reply("❌ Topilmadi."); return; }
      const info = await getAnimeInfo(ep.seasondb_id);
      const anime = info ? await getAnimeById(info.id) : null;
      if (!info || !anime) { await ctx.reply("❌ Topilmadi."); return; }
      const channelUsername = await getMainChannelUsername(ctx);
      const num = ep.epizod_number ?? ep.film_id ?? ep.ova_id ?? 0;
      const cap = buildEpisodeCaptionFromInputs(info, anime.name, num, channelUsername, undefined, quality);
      const sent = await ctx.api.sendVideo(config.SAVED_CHANNEL_ID, video.file_id, { caption: cap });
      const newChanelId = sent.video?.file_id ?? video.file_id;
      await setEpisodeQualityFiles(rowid, quality, newChanelId, video.file_id);
      ctx.session.state = "idle";
      ctx.session.currentEpisodeQuality = quality;
      await ctx.reply(`✅ <b>${quality}</b> formatdagi video saqlandi.`, { parse_mode: "HTML" });
      await showEpisodeDetail(ctx, rowid, quality);
      return;
    }

    if (state === "waiting_edit_episode_name") {
      const name = ctx.message.text?.trim() ?? "";
      if (!name) { await ctx.reply("❌ Nom yuboring."); return; }
      const rowid = ctx.session.editingEpisodeRowid;
      if (!rowid) return;
      await updateEpisodeText(rowid, "name", name);
      ctx.session.state = "idle";
      await ctx.reply("✅ Nom yangilandi.");
      await showEpisodeDetail(ctx, rowid);
      return;
    }

    if (state === "waiting_edit_episode_number") {
      const num = parseInt(ctx.message.text?.trim() ?? "", 10);
      if (isNaN(num) || num <= 0) { await ctx.reply("❌ Musbat son yuboring."); return; }
      const rowid = ctx.session.editingEpisodeRowid;
      if (!rowid) return;
      const ep = await getEpisode(rowid);
      if (!ep) return;
      const info = await getAnimeInfo(ep.seasondb_id);
      if (!info) return;
      const field = info.kategoriya === "TV" ? "epizod_number" : info.kategoriya === "FILM" ? "film_id" : "ova_id";
      await updateEpisodeNumber(rowid, field, num);
      ctx.session.state = "idle";
      await ctx.reply("✅ Raqam yangilandi.");
      await showEpisodeDetail(ctx, rowid);
      return;
    }

    // ----- Edit anime_info (season_edit panel) -----
    if (state === "waiting_edit_info_photo") {
      const photo = ctx.message.photo;
      if (!photo || photo.length === 0) { await ctx.reply("❌ Rasm yuboring."); return; }
      const rowid = ctx.session.editingSeasonRowid;
      if (!rowid) { ctx.session.state = "idle"; return; }
      const fileId = photo[photo.length - 1].file_id;
      await updateAnimeInfoText(rowid, "file_id", fileId);
      ctx.session.state = "idle";
      await ctx.reply("✅ Surat yangilandi.");
      await showInfoPanel(ctx, rowid);
      return;
    }
    if (state === "waiting_edit_info_name") {
      const name = ctx.message.text?.trim() ?? "";
      if (!name) { await ctx.reply("❌ Nom yuboring."); return; }
      const rowid = ctx.session.editingSeasonRowid;
      if (!rowid) { ctx.session.state = "idle"; return; }
      await updateAnimeInfoText(rowid, "name", name);
      ctx.session.state = "idle";
      await ctx.reply("✅ Nom yangilandi.");
      await showInfoPanel(ctx, rowid);
      return;
    }
    if (state === "waiting_edit_info_text") {
      const text = ctx.message.text?.trim() ?? "";
      if (!text) { await ctx.reply("❌ Tavsif yuboring."); return; }
      const rowid = ctx.session.editingSeasonRowid;
      if (!rowid) { ctx.session.state = "idle"; return; }
      await updateAnimeInfoText(rowid, "info", text);
      ctx.session.state = "idle";
      await ctx.reply("✅ Tavsif yangilandi.");
      await showInfoPanel(ctx, rowid);
      return;
    }
    if (state === "waiting_edit_info_season") {
      const num = parseInt(ctx.message.text?.trim() ?? "", 10);
      if (isNaN(num) || num <= 0) { await ctx.reply("❌ Musbat son yuboring."); return; }
      const rowid = ctx.session.editingSeasonRowid;
      if (!rowid) { ctx.session.state = "idle"; return; }
      await updateAnimeInfoNumber(rowid, "season_id", num);
      ctx.session.state = "idle";
      await ctx.reply("✅ Fasl raqami yangilandi.");
      await showInfoPanel(ctx, rowid);
      return;
    }
  });
}

// ============ Public: handle /start ep_<id> deep link (for any user) ============
export function setupDeepLinkDownload(bot: Bot<MyContext>): void {
  bot.command("start", async (ctx, next) => {
    const arg = ctx.match?.toString().trim() ?? "";
    if (!arg.startsWith("ep_")) {
      return next();
    }
    const downloadId = parseInt(arg.slice(3), 10);
    if (!downloadId) {
      await ctx.reply("❌ Noto'g'ri yuklash linki.");
      return;
    }
    try {
      // 3 alohida so'rov o'rniga bitta JOIN so'rovi
      const ctx2 = await getEpisodeWithContextByEpisodeId(downloadId);
      if (!ctx2) {
        await ctx.reply("❌ Bu qism topilmadi.");
        return;
      }
      const { ep, info, animeName, animeId } = ctx2;
      const bestQ = pickDefaultQuality(ep);
      if (!bestQ) {
        await ctx.reply("❌ Bu qism uchun video hali yuklanmagan.");
        return;
      }
      const userId = ctx.from?.id;
      if (userId) {
        await checkAndRecordDownload(userId, animeId, ep.rowid, ep.epizod_id);
      }
      // Yangilangan yuklab olishlar sonini olish
      const fresh = await getEpisodeWithContextByEpisodeId(downloadId);
      const freshEp = fresh?.ep ?? ep;
      const channelUsername = await getMainChannelUsername(ctx);
      const cap = buildEpisodeCaption(freshEp, info, animeName, channelUsername, true, bestQ);
      const ids = getEpisodeFileIds(freshEp, bestQ);
      const primaryId = ids.chanel ?? ids.bot ?? "";
      const fallbackId = ids.bot ?? ids.chanel ?? "";
      try {
        await ctx.api.sendVideo(ctx.chat!.id, primaryId, { caption: cap, protect_content: true });
      } catch {
        if (fallbackId && fallbackId !== primaryId) {
          await ctx.api.sendVideo(ctx.chat!.id, fallbackId, { caption: cap, protect_content: true });
        }
      }
    } catch (e) {
      console.error("deep link error:", e);
      await ctx.reply("❌ Xato yuz berdi.");
    }
  });
}
