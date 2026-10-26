import { Bot, InlineKeyboard } from "grammy";
import type { MyContext } from "./admin.ts";
import {
  getEpisodeByEpisodeId,
  getAnimeInfo,
  getAnimeById,
  getAnimeInfoStats,
  getEpisodesByAnimeInfo,
  insertShare,
  searchSeasonsByAnimeName,
  getSetting,
  type Episode,
  type AnimeInfo,
} from "../db.ts";

function styleAll(kb: InlineKeyboard): InlineKeyboard {
  for (const row of kb.inline_keyboard)
    for (const btn of row) (btn as any).style = "primary";
  return kb;
}

function pad2(n: number): string {
  return n < 10 ? `0${n}` : String(n);
}

function getEpNum(ep: Episode): number {
  return ep.epizod_number ?? ep.film_id ?? ep.ova_id ?? 0;
}

function getEpLabel(ep: Episode, info: AnimeInfo): string {
  const n = getEpNum(ep);
  if (info.kategoriya === "FILM") return `[${n}-Film]`;
  const seasonPart = (info.kategoriya === "TV" && info.season_id)
    ? `{${info.season_id}-fasl} ` : "";
  return `${seasonPart}[${pad2(n)}-qism]`;
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

function calcRating(likes: number, dislikes: number): string {
  const total = likes + dislikes;
  if (total === 0) return "—";
  return (likes / total * 10).toFixed(2);
}

// ─── Inline query parsers ─────────────────────────────────────────────────────
function parseEpisodeQuery(q: string): { animeId: number; seasonId: number; epNum: number; epId: number } | null {
  const m = q.trim().match(/^(\d+)_(\d+)_(\d+)_(\d+)$/);
  if (!m) return null;
  return { animeId: +m[1], seasonId: +m[2], epNum: +m[3], epId: +m[4] };
}

function parseSeasonQuery(q: string): { animeId: number; seasonId: number } | null {
  const m = q.trim().match(/^(\d+)_(\d+)$/);
  if (!m) return null;
  return { animeId: +m[1], seasonId: +m[2] };
}

// ─── setupShare ───────────────────────────────────────────────────────────────

export function setupShare(bot: Bot<MyContext>): void {

  bot.on("inline_query", async (ctx) => {
    const query = ctx.inlineQuery.query ?? "";
    const botUsername = ctx.me.username;

    // ── 1. Episode share: anime_id_season_id_ep_num_ep_id ────────────────────
    const epP = parseEpisodeQuery(query);
    if (epP) {
      try {
        const { animeId, seasonId, epNum, epId } = epP;
        const [ep, info] = await Promise.all([
          getEpisodeByEpisodeId(epId),
          getAnimeInfo(seasonId),
        ]);
        if (!ep || !info) {
          await ctx.answerInlineQuery([], { cache_time: 5 });
          return;
        }

        const userId = ctx.from.id;
        const [anime, stats, channelUsername] = await Promise.all([
          getAnimeById(info.id),
          getAnimeInfoStats(info.rowid, userId),
          getChannelUsername(ctx),
        ]);
        const animeName = anime?.name ?? info.name;
        const epLabel = getEpLabel(ep, info);
        const rating = calcRating(stats.likes, stats.dislikes);

        const caption =
          `Anime nomi: ${animeName}\n` +
          `${epLabel}\n` +
          (channelUsername ? `${channelUsername}\n` : "") +
          `Yuklangan: ${ep.downloads} marta\n` +
          `Reyting: ${rating}`;

        const deepLink = `https://t.me/${botUsername}?start=${animeId}_${seasonId}_${epNum}_${epId}`;
        const kb = styleAll(new InlineKeyboard().url("Tomosha qilish", deepLink));

        // Agar anime rasmi bo'lsa — rasm bilan ko'rsatish
        if (info.file_id) {
          await ctx.answerInlineQuery([
            {
              type: "photo",
              id: `ep_${animeId}_${seasonId}_${epNum}_${epId}`,
              photo_file_id: info.file_id,
              title: `${animeName} — ${epLabel}`,
              description: `${ep.downloads} ta yuklandi • ${rating} reyting`,
              caption,
              input_message_content: { message_text: caption },
              reply_markup: kb,
            } as any,
          ], { cache_time: 60 });
        } else {
          await ctx.answerInlineQuery([
            {
              type: "article",
              id: `ep_${animeId}_${seasonId}_${epNum}_${epId}`,
              title: `${animeName} — ${epLabel}`,
              description: `${ep.downloads} ta yuklandi • ${rating} reyting`,
              input_message_content: { message_text: caption },
              reply_markup: kb,
            },
          ], { cache_time: 60 });
        }
      } catch (e) {
        console.error("inline_query episode error:", e);
        await ctx.answerInlineQuery([], { cache_time: 5 });
      }
      return;
    }

    // ── 2. Season share: anime_id_season_id ──────────────────────────────────
    const sP = parseSeasonQuery(query);
    if (sP) {
      try {
        const { animeId, seasonId } = sP;
        const info = await getAnimeInfo(seasonId);
        if (!info) {
          await ctx.answerInlineQuery([], { cache_time: 5 });
          return;
        }

        const userId = ctx.from.id;
        const [anime, eps, stats] = await Promise.all([
          getAnimeById(info.id),
          getEpisodesByAnimeInfo(info.rowid),
          getAnimeInfoStats(info.rowid, userId),
        ]);
        const animeName = anime?.name ?? info.name;
        const totalDownloads = eps.reduce((acc, e) => acc + (e.downloads ?? 0), 0);
        const rating = calcRating(stats.likes, stats.dislikes);

        const caption =
          `Anime nomi: ${animeName}\n` +
          (info.info ? `${info.info}\n\n` : "\n") +
          `Botimiz: @${botUsername}\n` +
          `Yuklangan: ${totalDownloads} marta\n` +
          `Reyting: ${rating}\n\n` +
          `👍 ${stats.likes} | 👎 ${stats.dislikes} | 💬 ${stats.comments} | ↗️ ${stats.shares} | 🗂 ${stats.saves}`;

        const deepLink = `https://t.me/${botUsername}?start=${animeId}_${seasonId}`;
        const kb = styleAll(new InlineKeyboard().url("Tomosha qilish", deepLink));

        if (info.file_id) {
          await ctx.answerInlineQuery([
            {
              type: "photo",
              id: `s_${animeId}_${seasonId}`,
              photo_file_id: info.file_id,
              title: animeName,
              description: `${totalDownloads} ta yuklandi • ${rating} reyting`,
              caption,
              input_message_content: { message_text: caption },
              reply_markup: kb,
            } as any,
          ], { cache_time: 60 });
        } else {
          await ctx.answerInlineQuery([
            {
              type: "article",
              id: `s_${animeId}_${seasonId}`,
              title: animeName,
              description: `${totalDownloads} ta yuklandi • ${rating} reyting`,
              input_message_content: { message_text: caption },
              reply_markup: kb,
            },
          ], { cache_time: 60 });
        }
      } catch (e) {
        console.error("inline_query season error:", e);
        await ctx.answerInlineQuery([], { cache_time: 5 });
      }
      return;
    }

    // ── 3. Anime qidirish (erkin matn) — rasm bilan ───────────────────────────
    const trimmed = query.trim();
    if (trimmed.length >= 2) {
      try {
        const seasons = await searchSeasonsByAnimeName(trimmed, 20);
        const results = seasons.map(s => {
          const deepLink = `https://t.me/${botUsername}?start=${s.id}_${s.rowid}`;
          const kb = styleAll(new InlineKeyboard().url("Tomosha qilish", deepLink));

          const caption =
            `Anime nomi: ${s.anime_name}\n` +
            (s.info ? `${s.info}\n\n` : "\n") +
            `Botimiz: @${botUsername}`;

          // Rasm bo'lsa — InlineQueryResultCachedPhoto
          if (s.file_id) {
            return {
              type: "photo" as const,
              id: `s_${s.id}_${s.rowid}`,
              photo_file_id: s.file_id,
              title: s.anime_name,
              description: s.name,
              caption: s.anime_name,
              input_message_content: { message_text: caption },
              reply_markup: kb,
            } as any;
          }

          // Rasm yo'q bo'lsa — article
          return {
            type: "article" as const,
            id: `s_${s.id}_${s.rowid}`,
            title: s.anime_name,
            description: s.name,
            input_message_content: { message_text: caption },
            reply_markup: kb,
          };
        });

        await ctx.answerInlineQuery(results, { cache_time: 30 });
      } catch (e) {
        console.error("inline_query search error:", e);
        await ctx.answerInlineQuery([], { cache_time: 5 });
      }
      return;
    }

    await ctx.answerInlineQuery([], { cache_time: 5 });
  });

  // ── chosen_inline_result → stat_share ga yoz ────────────────────────────────
  bot.on("chosen_inline_result", async (ctx) => {
    const resultId = ctx.chosenInlineResult.result_id ?? "";
    const userId = ctx.from.id;

    const epM = resultId.match(/^ep_(\d+)_(\d+)_(\d+)_(\d+)$/);
    if (epM) {
      try {
        await insertShare(userId, +epM[1], +epM[2], +epM[4]);
      } catch (e) {
        console.error("chosen_inline_result ep share error:", e);
      }
      return;
    }

    const sM = resultId.match(/^s_(\d+)_(\d+)$/);
    if (sM) {
      try {
        await insertShare(userId, +sM[1], +sM[2], 0);
      } catch (e) {
        console.error("chosen_inline_result season share error:", e);
      }
      return;
    }
  });
}
