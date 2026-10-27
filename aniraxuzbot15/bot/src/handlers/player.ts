import { Bot, InlineKeyboard } from "grammy";
import type { MyContext } from "./admin.ts";
import { sendParts } from "./video_parts.ts";
import {
  getSeasonsByAnime,
  getAnimeById,
  getAnimeInfo,
  getEpisodesByAnimeInfo,
  getEpisode,
  getEpisodeFileIds,
  splitFileIds,
  getAvailableQualities,
  checkAndRecordDownload,
  getSetting,
  toggleLike,
  toggleDislike,
  toggleSave,
  getAnimeInfoStats,
  insertShareIfNew,
  getWatchHistory,
  getEpisodeByEpisodeId,
  QUALITIES,
  type AnimeInfo,
  type Episode,
  type Quality,
  type AnimeInfoStats,
} from "../db.ts";

// ─── Sozlamalar ──────────────────────────────────────────────────────────────
const EP_PAGE_SIZE = 20;      // bir sahifada ko'rinadigan qismlar soni
const PAGES_PER_SECTION = 3;  // bir bo'limdagi sahifalar soni (= 60 qism)

// ─── Stil ────────────────────────────────────────────────────────────────────
function styleAll(kb: InlineKeyboard): InlineKeyboard {
  for (const row of kb.inline_keyboard)
    for (const btn of row) (btn as any).style = "primary";
  return kb;
}

// ─── Yordamchi funksiyalar ────────────────────────────────────────────────────
function pad2(n: number): string {
  return n < 10 ? `0${n}` : String(n);
}

function getEpNum(ep: Episode): number {
  return ep.epizod_number ?? ep.film_id ?? ep.ova_id ?? 0;
}

function getEpLabel(ep: Episode, info: AnimeInfo): string {
  const n = getEpNum(ep);
  if (info.kategoriya === "FILM") return `${n}-Film`;
  return `${pad2(n)}-qism`;
}

function buildSeasonPart(info: AnimeInfo): string {
  if (info.kategoriya !== "TV") return "";
  return info.season_id ? ` {${info.season_id}-fasl}` : "";
}

function getPageForEp(eps: Episode[], epRowid: number): number {
  const idx = eps.findIndex((e) => e.rowid === epRowid);
  if (idx < 0) return 1;
  return Math.floor(idx / EP_PAGE_SIZE) + 1;
}

function getSectionForPage(page: number): number {
  return Math.floor((page - 1) / PAGES_PER_SECTION) + 1;
}

async function getChannelUsername(ctx: MyContext): Promise<string> {
  try {
    const id = await getSetting("main_channel_id");
    if (!id) return "";
    const chat = await ctx.api.getChat(id);
    if ("username" in chat && chat.username) return "@" + chat.username;
  } catch {}
  return "";
}

// ─── Caption qurilishi ────────────────────────────────────────────────────────

/** To'liq caption — statistika bilan (player xabari) */
function buildPlayerCaption(
  ep: Episode,
  info: AnimeInfo,
  animeName: string,
  channelUsername: string,
  stats: AnimeInfoStats,
  quality: Quality
): string {
  const seasonPart = buildSeasonPart(info);
  const epLabel = getEpLabel(ep, info);
  const displayName = ep.name?.trim() || animeName;

  return (
    `Sifati: ${quality} formatda\n` +
    `${displayName}${seasonPart} [${epLabel}]\n` +
    `Kanal: ${channelUsername || "—"}\n` +
    `📥Yuklangan: ${ep.downloads} marta\n\n` +
    `Yoqqan 👍 ${stats.likes.toLocaleString("uz")} | ` +
    `yoqmagan 👎 ${stats.dislikes.toLocaleString("uz")} | ` +
    `ulashilgan ↗️ ${stats.shares.toLocaleString("uz")} | ` +
    `saqlangan 🗂 ${stats.saves.toLocaleString("uz")}`
  );
}

/** Sodda caption — statistikasiz (arxiv xabari) */
function buildSimpleCaption(
  ep: Episode,
  info: AnimeInfo,
  animeName: string,
  channelUsername: string,
  quality: Quality
): string {
  const seasonPart = buildSeasonPart(info);
  const epLabel = getEpLabel(ep, info);
  const displayName = ep.name?.trim() || animeName;

  return (
    `Sifati: ${quality} formatda\n` +
    `${displayName}${seasonPart} [${epLabel}]\n` +
    `Kanal: ${channelUsername || "—"}\n` +
    `📥Yuklangan: ${ep.downloads} marta`
  );
}

// ─── Keyboard qurilishi ───────────────────────────────────────────────────────
function buildPlayerKeyboard(
  ep: Episode,
  info: AnimeInfo,
  eps: Episode[],
  page: number,
  section: number,
  stats: AnimeInfoStats,
  quality: Quality,
  allSeasons: AnimeInfo[] = []
): InlineKeyboard {
  const kb = new InlineKeyboard();
  const animeinfoId = info.rowid;
  const totalEps = eps.length;
  const totalPages = Math.max(1, Math.ceil(totalEps / EP_PAGE_SIZE));
  const totalSections = Math.max(1, Math.ceil(totalPages / PAGES_PER_SECTION));
  const curPage = Math.min(Math.max(1, page), totalPages);
  const curSection = Math.min(Math.max(1, section), totalSections);

  // ── Qator 1: Reaktsiya tugmalari (✈️ = 4-chi, season share) ──────────────
  kb.text(stats.userLiked ? "✓👍" : "👍",     `player:like:${animeinfoId}`)
    .text(stats.userDisliked ? "✓👎" : "👎",  `player:dislike:${animeinfoId}`)
    .switchInline("↗️",                          `${info.id}_${info.rowid}`)
    .text(stats.userSaved ? "✓🗂" : "🗂",     `player:save:${animeinfoId}`)
    .row();

  // ── Qator 2: Sifat tugmalari (faqat mavjud sifatlar) ─────────────────────
  const availableQ = getAvailableQualities(ep);
  if (availableQ.length > 0) {
    for (const q of availableQ) {
      kb.text(q === quality ? `✓${q}` : q, `player:q:${ep.rowid}:${q}`);
    }
    kb.row();
  }

  // ── Qatorlar 3–6: Qismlar gridi (joriy sahifa) ───────────────────────────
  const start = (curPage - 1) * EP_PAGE_SIZE;
  const slice = eps.slice(start, start + EP_PAGE_SIZE);

  for (let i = 0; i < slice.length; i++) {
    const sliceEp = slice[i];
    const num = getEpNum(sliceEp);
    const isCurrent = sliceEp.rowid === ep.rowid;
    if (isCurrent) {
      // Joriy qism tugmasi: bosish → inline share (episode share, chat picker ochiladi)
      const shareQ = `${info.id}_${info.rowid}_${num}_${sliceEp.epizod_id}`;
      kb.switchInline(`•${num}`, shareQ);
    } else {
      kb.text(String(num), `player:ep:${sliceEp.rowid}`);
    }
    if ((i + 1) % 5 === 0) kb.row();
  }
  if (slice.length % 5 !== 0) kb.row();

  // ── Qator 7: Oldingi / joriy / keyingi qism ──────────────────────────────
  const epIdx = eps.findIndex((e) => e.rowid === ep.rowid);
  const prevEp = epIdx > 0 ? eps[epIdx - 1] : null;
  const nextEp = epIdx >= 0 && epIdx < eps.length - 1 ? eps[epIdx + 1] : null;
  const epNum = getEpNum(ep);

  if (prevEp) kb.text(`‹ ${pad2(getEpNum(prevEp))}-qism`, `player:ep:${prevEp.rowid}`);
  kb.text(`${epNum}/${totalEps}`, "player:noop");
  if (nextEp) kb.text(`${pad2(getEpNum(nextEp))}-qism ›`, `player:ep:${nextEp.rowid}`);
  kb.row();

  // ── Qator 8: Sahifa navigatsiyasi ────────────────────────────────────────
  if (totalPages > 1) {
    if (curPage > 1)          kb.text(`« ${curPage - 1}-sahifa`, `player:page:${animeinfoId}:${curPage - 1}`);
    kb.text(`${curPage}/${totalPages}`, "player:noop");
    if (curPage < totalPages) kb.text(`${curPage + 1}-sahifa »`, `player:page:${animeinfoId}:${curPage + 1}`);
    kb.row();
  }

  // ── Qator 9: Bo'lim navigatsiyasi ────────────────────────────────────────
  if (totalSections > 1) {
    if (curSection > 1)             kb.text(`⟨ ${curSection - 1}-bo'lim`, `player:section:${animeinfoId}:${curSection - 1}`);
    kb.text(`${curSection}/${totalSections}`, "player:noop");
    if (curSection < totalSections) kb.text(`${curSection + 1}-bo'lim ⟩`, `player:section:${animeinfoId}:${curSection + 1}`);
    kb.row();
  }

  // ── Qator 10: Fasl navigatsiyasi (1 dan ko'p fasl bo'lsa) ────────────────
  if (allSeasons.length > 1) {
    const seasonIdx = allSeasons.findIndex((s) => s.rowid === info.rowid);
    const prevSeason = seasonIdx > 0 ? allSeasons[seasonIdx - 1] : null;
    const nextSeason = seasonIdx >= 0 && seasonIdx < allSeasons.length - 1 ? allSeasons[seasonIdx + 1] : null;
    const curSeasonNum = seasonIdx + 1;
    const totalSeasons = allSeasons.length;

    if (prevSeason) kb.text(`« ${curSeasonNum - 1}-bo'lim`, `player:swseason:${prevSeason.rowid}`);
    kb.text(`${curSeasonNum}/${totalSeasons}`, "player:noop");
    if (nextSeason) kb.text(`${curSeasonNum + 1}-bo'lim »`, `player:swseason:${nextSeason.rowid}`);
    kb.row();
  }

  return styleAll(kb);
}

// ─── Video yuborish ───────────────────────────────────────────────────────────

/** Arxiv ko'rinishda yuborish (keyboard yo'q) */
async function sendArchivedEpisode(
  ctx: MyContext,
  ep: Episode,
  info: AnimeInfo,
  animeName: string,
  channelUsername: string,
  quality: Quality
): Promise<void> {
  const caption = buildSimpleCaption(ep, info, animeName, channelUsername, quality);
  const ids = getEpisodeFileIds(ep, quality);
  if (!ids.chanel && !ids.bot) return;
  try {
    await sendParts(ctx, ids.chanel || ids.bot, ids.bot || ids.chanel, caption);
  } catch (e) {
    console.error(`sendArchivedEpisode failed (ep=${ep.rowid} q=${quality}):`, String(e));
  }
}

/** To'liq player ko'rinishda yuborish */
async function sendPlayerEpisode(
  ctx: MyContext,
  ep: Episode,
  info: AnimeInfo,
  animeName: string,
  channelUsername: string,
  quality: Quality,
  stats: AnimeInfoStats,
  eps: Episode[],
  page: number,
  section: number,
  allSeasons: AnimeInfo[] = []
): Promise<number | null> {
  const caption = buildPlayerCaption(ep, info, animeName, channelUsername, stats, quality);
  const kb = buildPlayerKeyboard(ep, info, eps, page, section, stats, quality, allSeasons);
  const ids = getEpisodeFileIds(ep, quality);

  if (!ids.chanel && !ids.bot) {
    await ctx.reply("❌ Bu qism uchun video topilmadi.").catch(() => {});
    return null;
  }

  try {
    return await sendParts(ctx, ids.chanel || ids.bot, ids.bot || ids.chanel, caption, kb);
  } catch (e1) {
    console.error(`sendPlayerEpisode failed (ep=${ep.rowid} q=${quality}):`, String(e1));
    await ctx.reply(
      `❌ Video yuborishda xato yuz berdi.\n\nXato: ${String(e1).slice(0, 200)}`
    ).catch(() => {});
    return null;
  }
}

// ─── Asosiy navigatsiya ───────────────────────────────────────────────────────

/** Birinchi marta faslni ochish */
async function openSeason(ctx: MyContext, animeinfoRowid: number): Promise<void> {
  const userId = ctx.from!.id;

  const info = await getAnimeInfo(animeinfoRowid);
  if (!info) { await ctx.reply("❌ Fasl topilmadi."); return; }

  const eps = await getEpisodesByAnimeInfo(animeinfoRowid);
  if (eps.length === 0) { await ctx.reply("❌ Bu faslda hali qism qo'shilmagan."); return; }

  const firstEp = eps[0];
  const available = getAvailableQualities(firstEp);

  if (available.length === 0) {
    await ctx.reply("❌ Bu qism uchun hali video qo'shilmagan.").catch(() => {});
    return;
  }

  const quality = available[0];

  const [anime, channelUsername, allSeasons] = await Promise.all([
    getAnimeById(info.id),
    getChannelUsername(ctx),
    getSeasonsByAnime(info.id),
  ]);
  const animeName = anime?.name ?? info.name;

  // Download statistikasini yoz
  await checkAndRecordDownload(userId, info.id, firstEp.rowid, firstEp.epizod_id).catch(() => {});

  // Statistikani yangilab ol
  const stats = await getAnimeInfoStats(animeinfoRowid, userId);

  const msgId = await sendPlayerEpisode(ctx, firstEp, info, animeName, channelUsername, quality, stats, eps, 1, 1, allSeasons);

  // Sessiyani yangilash
  ctx.session.playerAnimeId = info.id;
  ctx.session.playerAnimeInfoId = animeinfoRowid;
  ctx.session.playerEpRowid = firstEp.rowid;
  ctx.session.playerEpNumber = getEpNum(firstEp);
  ctx.session.playerMsgId = msgId ?? undefined;
  ctx.session.playerQuality = quality;
  ctx.session.playerPage = 1;
  ctx.session.playerSection = 1;
  ctx.session.playerTotalEps = eps.length;
}

/** Yangi qismga o'tish — tezlashtirilgan va bardoshli versiya */
async function navigateToEpisode(ctx: MyContext, newEpRowid: number): Promise<void> {
  const userId = ctx.from!.id;

  // Joriy epizod bilan bir xil bo'lsa — hech narsa qilma
  if (ctx.session.playerEpRowid === newEpRowid) {
    await ctx.answerCallbackQuery().catch(() => {});
    return;
  }

  const newEp = await getEpisode(newEpRowid);

  if (!newEp) {
    await ctx.answerCallbackQuery("❌ Qism topilmadi.").catch(() => {});
    return;
  }

  const info = await getAnimeInfo(newEp.seasondb_id);
  if (!info) {
    await ctx.answerCallbackQuery("❌ Xato yuz berdi.").catch(() => {});
    return;
  }

  const available = getAvailableQualities(newEp);
  if (available.length === 0) {
    await ctx.answerCallbackQuery("❌ Bu qism uchun video yo'q.").catch(() => {});
    return;
  }

  let quality: Quality = (ctx.session.playerQuality as Quality) ?? available[0];
  if (!available.includes(quality)) quality = available[0];

  // Barcha kerakli ma'lumotlarni parallel olamiz
  const oldEpRowid = ctx.session.playerEpRowid;
  const oldMsgId = ctx.session.playerMsgId;

  const [anime, eps, channelUsername, oldEp, allSeasons] = await Promise.all([
    getAnimeById(info.id),
    getEpisodesByAnimeInfo(info.rowid),
    getChannelUsername(ctx),
    oldEpRowid ? getEpisode(oldEpRowid) : Promise.resolve(null),
    getSeasonsByAnime(info.id),
  ]);
  const animeName = anime?.name ?? info.name;

  const page = getPageForEp(eps, newEpRowid);
  const section = getSectionForPage(page);

  await ctx.answerCallbackQuery().catch(() => {});

  // Download va yangi qism statistikasini parallel olamiz
  const [, stats] = await Promise.all([
    checkAndRecordDownload(userId, info.id, newEp.rowid, newEp.epizod_id).catch(() => {}),
    getAnimeInfoStats(info.rowid, userId),
  ]);

  // 1. Yangi qismni yuboramiz
  const newMsgId = await sendPlayerEpisode(ctx, newEp, info, animeName, channelUsername, quality, stats, eps, page, section, allSeasons);

  // 2. Eski player caption ni soddalashtiramiz:
  //    faqat Sifat / Nom / Kanal / Yuklamalar — statistika va tugmalar yo'q
  if (oldMsgId && oldEp) {
    const oldQuality = (ctx.session.playerQuality as Quality) ?? quality;
    const simpleCaption = buildSimpleCaption(oldEp, info, animeName, channelUsername, oldQuality);
    await ctx.api.editMessageCaption(ctx.chat!.id, oldMsgId, {
      caption: simpleCaption,
      reply_markup: new InlineKeyboard(),
    }).catch(() => {});
  }

  // 3. Sessiyani yangilash
  ctx.session.playerAnimeInfoId = info.rowid;
  ctx.session.playerEpRowid = newEpRowid;
  ctx.session.playerEpNumber = getEpNum(newEp);
  ctx.session.playerMsgId = newMsgId ?? ctx.session.playerMsgId;
  ctx.session.playerQuality = quality;
  ctx.session.playerPage = page;
  ctx.session.playerSection = section;
  ctx.session.playerTotalEps = eps.length;
}

/** Caption va keyboardni yangilash (like/dislike/save bosilganda) */
async function refreshPlayerCaption(ctx: MyContext): Promise<void> {
  const userId = ctx.from!.id;
  const epRowid = ctx.session.playerEpRowid;
  const animeinfoId = ctx.session.playerAnimeInfoId;
  const msgId = ctx.session.playerMsgId;
  const quality = (ctx.session.playerQuality as Quality) ?? "720p";
  const page = ctx.session.playerPage ?? 1;
  const section = ctx.session.playerSection ?? 1;

  if (!epRowid || !animeinfoId || !msgId) return;

  const [ep, info, eps, stats, channelUsername] = await Promise.all([
    getEpisode(epRowid),
    getAnimeInfo(animeinfoId),
    getEpisodesByAnimeInfo(animeinfoId),
    getAnimeInfoStats(animeinfoId, userId),
    getChannelUsername(ctx),
  ]);

  if (!ep || !info) return;

  const [anime, allSeasons] = await Promise.all([
    getAnimeById(info.id),
    getSeasonsByAnime(info.id),
  ]);
  const animeName = anime?.name ?? info.name;

  const caption = buildPlayerCaption(ep, info, animeName, channelUsername, stats, quality);
  const kb = buildPlayerKeyboard(ep, info, eps, page, section, stats, quality, allSeasons);

  await ctx.api.editMessageCaption(ctx.chat!.id, msgId, { caption, reply_markup: kb }).catch(() => {});
}

/** Faqat keyboardni yangilash (sahifa/bo'lim o'zgarganda) */
async function refreshPlayerKeyboard(ctx: MyContext): Promise<void> {
  const epRowid = ctx.session.playerEpRowid;
  const animeinfoId = ctx.session.playerAnimeInfoId;
  const msgId = ctx.session.playerMsgId;
  const quality = (ctx.session.playerQuality as Quality) ?? "720p";
  const page = ctx.session.playerPage ?? 1;
  const section = ctx.session.playerSection ?? 1;
  const userId = ctx.from!.id;

  if (!epRowid || !animeinfoId || !msgId) return;

  const [ep, info, eps, stats] = await Promise.all([
    getEpisode(epRowid),
    getAnimeInfo(animeinfoId),
    getEpisodesByAnimeInfo(animeinfoId),
    getAnimeInfoStats(animeinfoId, userId),
  ]);

  if (!ep || !info) return;

  const allSeasons = await getSeasonsByAnime(info.id);
  const kb = buildPlayerKeyboard(ep, info, eps, page, section, stats, quality, allSeasons);
  await ctx.api.editMessageReplyMarkup(ctx.chat!.id, msgId, { reply_markup: kb }).catch(() => {});
}

/** Deep link orqali aniq epizodni pleyrda ochish */
async function openSeasonAtEpisode(ctx: MyContext, animeinfoRowid: number, epizodId: number): Promise<void> {
  const userId = ctx.from!.id;

  const info = await getAnimeInfo(animeinfoRowid);
  if (!info) { await ctx.reply("❌ Fasl topilmadi."); return; }

  const eps = await getEpisodesByAnimeInfo(animeinfoRowid);
  if (eps.length === 0) { await ctx.reply("❌ Bu faslda hali qism qo'shilmagan."); return; }

  const targetEp = eps.find((e) => e.epizod_id === epizodId) ?? eps[0];
  const available = getAvailableQualities(targetEp);
  if (available.length === 0) { await ctx.reply("❌ Bu qism uchun video hali yuklanmagan."); return; }

  const quality = available[0];

  const [anime, channelUsername, allSeasons] = await Promise.all([
    getAnimeById(info.id),
    getChannelUsername(ctx),
    getSeasonsByAnime(info.id),
  ]);
  const animeName = anime?.name ?? info.name;

  await checkAndRecordDownload(userId, info.id, targetEp.rowid, targetEp.epizod_id).catch(() => {});

  const stats = await getAnimeInfoStats(animeinfoRowid, userId);

  const page = getPageForEp(eps, targetEp.rowid);
  const section = getSectionForPage(page);

  const msgId = await sendPlayerEpisode(ctx, targetEp, info, animeName, channelUsername, quality, stats, eps, page, section, allSeasons);

  ctx.session.playerAnimeId = info.id;
  ctx.session.playerAnimeInfoId = animeinfoRowid;
  ctx.session.playerEpRowid = targetEp.rowid;
  ctx.session.playerEpNumber = getEpNum(targetEp);
  ctx.session.playerMsgId = msgId ?? undefined;
  ctx.session.playerQuality = quality;
  ctx.session.playerPage = page;
  ctx.session.playerSection = section;
  ctx.session.playerTotalEps = eps.length;
}

// ─── Handler ro'yxatdan o'tkazish ─────────────────────────────────────────────
export function setupPlayer(bot: Bot<MyContext>): void {

  // Hech narsa qilmaydigan tugma
  bot.callbackQuery("player:noop", (ctx) => ctx.answerCallbackQuery());

  // ── "Yuklab Olish" bosildi → ko'rish tarixi + fasllar ──────────────────────
  bot.callbackQuery(/^player:start:(\d+)$/, async (ctx) => {
    const animeId = Number(ctx.match[1]);
    const userId = ctx.from!.id;

    const [seasons, history] = await Promise.all([
      getSeasonsByAnime(animeId),
      getWatchHistory(userId, animeId),
    ]);

    if (seasons.length === 0) {
      await ctx.answerCallbackQuery("❌ Bu anime uchun hali qism qo'shilmagan");
      return;
    }

    ctx.session.playerAnimeId = animeId;

    // Ko'rish tarixi bor — "davom etish" tugmasini ko'rsat
    if (history) {
      const kb = new InlineKeyboard();

      // Qaysi fasl va qism ekanini topamiz
      let resumeLabel = "▶️ Davom etish";
      for (const s of seasons) {
        // Bu yerda aniq qism nomini topish uchun taxminiy label
        if (s.rowid) {
          // epizod_id bo'yicha qaysi fasl ekanligini tekshirib ko'rmaymiz (heavy query),
          // faqat umumiy "N-qismdan davom etish" yozuvini ko'rsatamiz
        }
      }
      // Epizod raqamini topish uchun epizod_id orqali so'rov
      const resumeEp = history.epizod_id
        ? await getEpisodeByEpisodeId(history.epizod_id).catch(() => null)
        : null;

      if (resumeEp) {
        const epNum = resumeEp.epizod_number ?? resumeEp.film_id ?? resumeEp.ova_id ?? 0;
        resumeLabel = `▶️ ${epNum}-qismdan davom etish`;
      }

      kb.text(resumeLabel, `player:resume:${animeId}`).row();

      // Anime nomi uchun
      const anime = await getAnimeById(animeId);
      const animeName = anime?.name ?? "Anime";

      // Fasllar ro'yxati
      if (seasons.length === 1) {
        kb.text(animeName, `player:season:${seasons[0].rowid}`).row();
      } else {
        for (const s of seasons) {
          kb.text(s.name, `player:season:${s.rowid}`).row();
        }
      }
      styleAll(kb);

      await ctx.answerCallbackQuery();
      await ctx.reply("Quyidagilardan birini tanlang:", { reply_markup: kb });
      return;
    }

    // Tarix yo'q — odatdagidek davom et
    if (seasons.length === 1) {
      await ctx.answerCallbackQuery();
      await openSeason(ctx, seasons[0].rowid);
      return;
    }

    const kb = new InlineKeyboard();
    for (const s of seasons) {
      kb.text(s.name, `player:season:${s.rowid}`).row();
    }
    styleAll(kb);

    await ctx.answerCallbackQuery();
    await ctx.reply("Quyidagilardan birini tanlang:", { reply_markup: kb });
  });

  // ── Davom etish bosildi — oxirgi ko'rilgan qismni ochish ─────────────────
  bot.callbackQuery(/^player:resume:(\d+)$/, async (ctx) => {
    const animeId = Number(ctx.match[1]);
    const userId = ctx.from!.id;
    await ctx.answerCallbackQuery();

    const history = await getWatchHistory(userId, animeId);
    if (!history) {
      // Tarix yo'q — birinchi faslni ochish
      const seasons = await getSeasonsByAnime(animeId);
      if (seasons.length === 0) { await ctx.reply("❌ Qism topilmadi."); return; }
      ctx.session.playerAnimeId = animeId;
      try { await ctx.deleteMessage(); } catch {}
      await openSeason(ctx, seasons[0].rowid);
      return;
    }

    // Epizod_id orqali qaysi faslda ekanligini topamiz
    const ep = await getEpisodeByEpisodeId(history.epizod_id).catch(() => null);
    if (!ep) {
      await ctx.reply("❌ Qism topilmadi.");
      return;
    }

    ctx.session.playerAnimeId = animeId;
    try { await ctx.deleteMessage(); } catch {}
    // openSeasonAtEpisode ichki funksiyasi — to'g'ri fasl + qismni ochadi
    await openSeasonAtEpisode(ctx, ep.seasondb_id, history.epizod_id);
  });

  // ── Fasl tanlandi ──────────────────────────────────────────────────────────
  bot.callbackQuery(/^player:season:(\d+)$/, async (ctx) => {
    const animeinfoRowid = Number(ctx.match[1]);
    await ctx.answerCallbackQuery();
    try { await ctx.deleteMessage(); } catch {}
    await openSeason(ctx, animeinfoRowid);
  });

  // ── Raqam (episode) tugmasi bosildi ───────────────────────────────────────
  bot.callbackQuery(/^player:ep:(\d+)$/, async (ctx) => {
    await navigateToEpisode(ctx, Number(ctx.match[1]));
  });

  // ── 👍 Like ────────────────────────────────────────────────────────────────
  bot.callbackQuery(/^player:like:(\d+)$/, async (ctx) => {
    const animeinfoId = Number(ctx.match[1]);
    const userId = ctx.from!.id;
    const animeId = ctx.session.playerAnimeId ?? 0;

    const result = await toggleLike(userId, animeId, animeinfoId);
    await ctx.answerCallbackQuery(result === "added" ? "👍 Yoqdi!" : "👍 Olib tashlandi");
    await refreshPlayerCaption(ctx);
  });

  // ── 👎 Dislike ─────────────────────────────────────────────────────────────
  bot.callbackQuery(/^player:dislike:(\d+)$/, async (ctx) => {
    const animeinfoId = Number(ctx.match[1]);
    const userId = ctx.from!.id;
    const animeId = ctx.session.playerAnimeId ?? 0;

    const result = await toggleDislike(userId, animeId, animeinfoId);
    await ctx.answerCallbackQuery(result === "added" ? "👎 Yoqmadi!" : "👎 Olib tashlandi");
    await refreshPlayerCaption(ctx);
  });

  // ── 🗂 Saqlash ─────────────────────────────────────────────────────────────
  bot.callbackQuery(/^player:save:(\d+)$/, async (ctx) => {
    const animeinfoId = Number(ctx.match[1]);
    const userId = ctx.from!.id;
    const animeId = ctx.session.playerAnimeId ?? 0;

    const result = await toggleSave(userId, animeId, animeinfoId);
    await ctx.answerCallbackQuery(result === "added" ? "🗂 Saqlandi!" : "🗂 Saqlashdan olib tashlandi");
    await refreshPlayerCaption(ctx);
  });

  // ── Sifat o'zgartirish ─────────────────────────────────────────────────────
  bot.callbackQuery(/^player:q:(\d+):(1080p|720p|480p|360p)$/, async (ctx) => {
    const epRowid = Number(ctx.match[1]);
    const newQuality = ctx.match[2] as Quality;
    const msgId = ctx.session.playerMsgId;
    const userId = ctx.from!.id;
    const animeinfoId = ctx.session.playerAnimeInfoId;
    const page = ctx.session.playerPage ?? 1;
    const section = ctx.session.playerSection ?? 1;

    if (!msgId || !animeinfoId) {
      await ctx.answerCallbackQuery().catch(() => {});
      return;
    }

    // Avval xuddi shunday sifat bo'lsa
    if (ctx.session.playerQuality === newQuality) {
      await ctx.answerCallbackQuery(`✓ Allaqachon ${newQuality}`);
      return;
    }

    const ep = await getEpisode(epRowid);
    if (!ep) { await ctx.answerCallbackQuery("❌ Xato"); return; }

    const available = getAvailableQualities(ep);
    if (!available.includes(newQuality)) {
      await ctx.answerCallbackQuery(`❌ ${newQuality} sifat mavjud emas`);
      return;
    }

    const ids = getEpisodeFileIds(ep, newQuality);
    const fileId = ids.chanel || ids.bot;
    if (!fileId) {
      await ctx.answerCallbackQuery(`❌ ${newQuality} fayl topilmadi`);
      return;
    }

    const [info, eps, stats, channelUsername] = await Promise.all([
      getAnimeInfo(animeinfoId),
      getEpisodesByAnimeInfo(animeinfoId),
      getAnimeInfoStats(animeinfoId, userId),
      getChannelUsername(ctx),
    ]);

    if (!info) { await ctx.answerCallbackQuery("❌ Xato"); return; }

    const [anime, allSeasons] = await Promise.all([
      getAnimeById(info.id),
      getSeasonsByAnime(info.id),
    ]);
    const animeName = anime?.name ?? info.name;
    const caption = buildPlayerCaption(ep, info, animeName, channelUsername, stats, newQuality);
    const kb = buildPlayerKeyboard(ep, info, eps, page, section, stats, newQuality, allSeasons);

    await ctx.answerCallbackQuery(`✅ ${newQuality}`);

    // Bir nechta qismli video (2 GB dan katta): joyida almashtirib bo'lmaydi — eski xabar o'chadi, qismlar qayta yuboriladi.
    if (splitFileIds(fileId).length > 1 || splitFileIds(ids.bot).length > 1) {
      try {
        const newId = await sendParts(ctx, ids.chanel || ids.bot, ids.bot || ids.chanel, caption, kb);
        await ctx.api.deleteMessage(ctx.chat!.id, msgId).catch(() => {});
        ctx.session.playerMsgId = newId;
        ctx.session.playerQuality = newQuality;
      } catch (e) {
        console.error("player:quality parts failed:", String(e));
      }
      return;
    }

    // Video va captionni bir vaqtda almashtiramiz
    try {
      await ctx.api.editMessageMedia(
        ctx.chat!.id,
        msgId,
        { type: "video", media: fileId, caption },
        { reply_markup: kb }
      );
    } catch {
      // editMessageMedia ishlamasa — eski o'chir, yangisini yuborma
      // (faqat caption va keyboard yangilanadi)
      await ctx.api.editMessageCaption(ctx.chat!.id, msgId, { caption, reply_markup: kb }).catch(() => {});
    }

    ctx.session.playerQuality = newQuality;
  });

  // ── Sahifa o'zgartirildi ───────────────────────────────────────────────────
  bot.callbackQuery(/^player:page:(\d+):(\d+)$/, async (ctx) => {
    const animeinfoId = Number(ctx.match[1]);
    const newPage = Number(ctx.match[2]);

    await ctx.answerCallbackQuery(`${newPage}-sahifa`);

    ctx.session.playerAnimeInfoId = animeinfoId;
    ctx.session.playerPage = newPage;
    ctx.session.playerSection = getSectionForPage(newPage);

    await refreshPlayerKeyboard(ctx);
  });

  // ── Bo'lim o'zgartirildi ───────────────────────────────────────────────────
  bot.callbackQuery(/^player:section:(\d+):(\d+)$/, async (ctx) => {
    const animeinfoId = Number(ctx.match[1]);
    const newSection = Number(ctx.match[2]);

    // Bo'limning birinchi sahifasiga o'tish
    const newPage = (newSection - 1) * PAGES_PER_SECTION + 1;

    await ctx.answerCallbackQuery(`${newSection}-bo'lim`);

    ctx.session.playerAnimeInfoId = animeinfoId;
    ctx.session.playerSection = newSection;
    ctx.session.playerPage = newPage;

    await refreshPlayerKeyboard(ctx);
  });

  // ── Fasl almashtirildi (player ichidan) ────────────────────────────────────
  bot.callbackQuery(/^player:swseason:(\d+)$/, async (ctx) => {
    const animeinfoRowid = Number(ctx.match[1]);
    await ctx.answerCallbackQuery();
    await openSeason(ctx, animeinfoRowid);
  });

}

// ─── Deep link handlerlari (setupOnboarding DAN OLDIN ro'yxatdan o'tishi kerak) ────
export function setupDeepLinkPlayer(bot: Bot<MyContext>): void {

  // Fasl ulashish linki: /start {anime_id}_{season_id}
  bot.command("start", async (ctx, next) => {
    const arg = ctx.match?.toString().trim() ?? "";
    const m = arg.match(/^(\d+)_(\d+)$/);
    if (!m || !ctx.from) return next();
    const animeId = parseInt(m[1], 10);
    const seasonId = parseInt(m[2], 10);
    if (!seasonId) return next();
    // Ulashish statistikasini yoz (faqat birinchi marta)
    await insertShareIfNew(ctx.from.id, animeId, seasonId, 0).catch(() => {});
    await openSeason(ctx, seasonId);
  });

  // Qism ulashish linki: /start {anime_id}_{season_id}_{ep_num}_{ep_id}
  bot.command("start", async (ctx, next) => {
    const arg = ctx.match?.toString().trim() ?? "";
    const m = arg.match(/^(\d+)_(\d+)_(\d+)_(\d+)$/);
    if (!m || !ctx.from) return next();
    const animeId = parseInt(m[1], 10);
    const seasonId = parseInt(m[2], 10);
    const epId    = parseInt(m[4], 10);
    if (!seasonId || !epId) return next();
    // Ulashish statistikasini yoz (faqat birinchi marta, season darajasida)
    await insertShareIfNew(ctx.from.id, animeId, seasonId, epId).catch(() => {});
    await openSeasonAtEpisode(ctx, seasonId, epId);
  });
}
