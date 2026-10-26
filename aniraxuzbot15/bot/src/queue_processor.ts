/**
 * Post navbatini qayta ishlash — Cron trigger har daqiqada chaqiradi.
 * Cloudflare Workers: Cron triggerlar parallel ishlamaydi (sequential guarantee).
 * Shuning uchun har cron boshida barcha 'processing' itemlarni 'pending' ga qaytaramiz.
 */
import {
  getPendingQueueItems,
  markQueueItemDone,
  markQueueItemError,
  markQueueItemProcessing,
  markQueueItemSending,
  resetAllProcessingItems,
  getBatchStats,
} from "./db_post_queue.ts";
import {
  getEpisode,
  getAnimeInfo,
  getAnimeById,
  getAvailableQualities,
  type Episode,
  type AnimeInfo,
  type Quality,
} from "./db.ts";
import { config } from "./config.ts";

const MAX_PER_MINUTE = 8;
const DELAY_MS = 3000; // 8 ta * 3s = 24s + ~1.5s*8 qayta ishlash = ~36s (<60s)

function pickDefaultQuality(ep: Episode): Quality | null {
  const avail = getAvailableQualities(ep);
  return avail.length > 0 ? avail[0] : null;
}

function pad2(n: number): string {
  return n < 10 ? `0${n}` : String(n);
}

function buildCaption(
  ep: Episode,
  info: AnimeInfo,
  epName: string,
  channelUsername: string
): string {
  const seasonPart =
    info.kategoriya === "TV" && info.season_id ? ` {${info.season_id}-fasl}` : "";
  const num = ep.epizod_number ?? ep.film_id ?? ep.ova_id ?? 0;
  let line = "";
  if (info.kategoriya === "TV") line = `${epName}${seasonPart} [${pad2(num)}-qism]`;
  else if (info.kategoriya === "FILM") line = `${epName} [${num}-Film]`;
  else line = `${epName} [${pad2(num)}-qism]`;
  return `${line}\nKanal: ${channelUsername || "—"}`;
}

async function tgSend(method: string, body: object, maxRetries = 4): Promise<boolean> {
  for (let attempt = 0; attempt <= maxRetries; attempt++) {
    try {
      const r = await fetch(`https://api.telegram.org/bot${config.BOT_TOKEN}/${method}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });
      const d = (await r.json()) as {
        ok: boolean;
        parameters?: { retry_after?: number };
        description?: string;
        error_code?: number;
      };
      if (d.ok) return true;

      // Flood limit — kutib qayta urinish
      if (d.parameters?.retry_after) {
        const wait = (d.parameters.retry_after + 1) * 1000;
        console.log(`Flood limit (urinish ${attempt + 1}), kutilmoqda ${wait}ms`);
        await new Promise((res) => setTimeout(res, wait));
        continue;
      }

      // Qayta urinib bo'lmaydigan xatolar (channel topilmadi, bot admin emas va h.k.)
      const fatal = [400, 403, 404];
      if (d.error_code && fatal.includes(d.error_code)) {
        console.error(`tgSend ${method} fatal (${d.error_code}):`, d.description);
        return false;
      }

      console.error(`tgSend ${method} fail (urinish ${attempt + 1}):`, d.description);
      if (attempt < maxRetries) {
        await new Promise((res) => setTimeout(res, 2000 * (attempt + 1)));
      }
    } catch (e) {
      console.error(`tgSend ${method} network xatosi (urinish ${attempt + 1}):`, e);
      if (attempt < maxRetries) {
        await new Promise((res) => setTimeout(res, 2000 * (attempt + 1)));
      }
    }
  }
  return false;
}

/** Cron har daqiqada shu funksiyani chaqiradi */
export async function processPostQueue(): Promise<void> {
  // Kanalga post joylash vaqtincha o'chirilgan / posting to channels is temporarily off
  if (!config.CHANNEL_POSTING) return;
  // Cloudflare cron triggerlar sequential — avvalgi cron o'lgan bo'lsa,
  // uning 'processing' holati qolgan itemlarini 'pending' ga qaytaramiz
  await resetAllProcessingItems().catch(e => console.error("resetAllProcessingItems error:", e));

  const items = await getPendingQueueItems(MAX_PER_MINUTE);
  if (items.length === 0) return;

  console.log(`processPostQueue: ${items.length} ta navbatdagi qism`);

  const processedBatchIds = new Set<string>();

  for (const item of items) {
    // Atomik lock — agar boshqa cron/worker allaqachon olgan bo'lsa, o'tkazib yuboramiz
    const locked = await markQueueItemProcessing(item.id);
    if (!locked) {
      console.log(`Queue item ${item.id}: lock olinmadi (boshqa worker allaqachon ushlab olgan), o'tkazib yuborildi`);
      continue;
    }

    try {
      // DUPLICATE GUARD: agar xabar allaqachon yuborilgan bo'lsa (posted_at bor) — o'tkazib yuborish
      if (item.posted_at) {
        console.log(`Queue item ${item.id}: allaqachon yuborilgan (posted_at=${item.posted_at}), done qilinmoqda`);
        await markQueueItemDone(item.id);
        processedBatchIds.add(item.batch_id);
        continue;
      }

      const ep = await getEpisode(item.episode_rowid);
      if (!ep) {
        console.error(`Queue item ${item.id}: episode rowid ${item.episode_rowid} topilmadi`);
        await markQueueItemError(item.id);
        processedBatchIds.add(item.batch_id);
        continue;
      }

      const info = await getAnimeInfo(item.anime_info_id);
      if (!info) {
        console.error(`Queue item ${item.id}: anime_info ${item.anime_info_id} topilmadi`);
        await markQueueItemError(item.id);
        processedBatchIds.add(item.batch_id);
        continue;
      }

      const anime = await getAnimeById(info.id);
      const animeName = anime?.name ?? info.name;
      const epName = ep.name && ep.name.trim() ? ep.name.trim() : animeName;
      const cap = buildCaption(ep, info, epName, item.channel_username);
      const link = `https://t.me/${item.bot_username}?start=ep_${ep.epizod_id}`;

      const ok = await tgSend("sendMessage", {
        chat_id: item.channel_id,
        text: cap,
        reply_markup: {
          inline_keyboard: [[{ text: "• Yuklab Olish •", url: link }]],
        },
      });

      if (ok) {
        await markQueueItemDone(item.id); // posted_at ham shu yerda yoziladi
      } else {
        await markQueueItemError(item.id);
      }
    } catch (e) {
      console.error(`Queue item ${item.id} xatosi:`, e);
      await markQueueItemError(item.id).catch(() => {});
    }

    processedBatchIds.add(item.batch_id);

    // Postlar orasida kutish (flood limit uchun) — oxirgi itemdan keyin kutmaymiz
    if (items.indexOf(item) < items.length - 1) {
      await new Promise((r) => setTimeout(r, DELAY_MS));
    }
  }

  // Batch holati haqida admin ga xabar
  for (const batchId of processedBatchIds) {
    const stats = await getBatchStats(batchId);
    const adminItem = items.find((i) => i.batch_id === batchId);
    if (!adminItem) continue;

    if (stats.pending === 0 && stats.processing === 0) {
      const total = stats.done + stats.error;
      const msg =
        stats.error > 0
          ? `✅ Post yakunlandi: ${stats.done}/${total} ta joylandi. ❌ ${stats.error} ta xato.`
          : `✅ Barcha ${total} ta post muvaffaqiyatli joylandi!`;
      await tgSend("sendMessage", { chat_id: adminItem.admin_chat_id, text: msg });
    } else if (stats.done > 0 && stats.done % 10 === 0) {
      const done = stats.done + stats.error;
      const total = done + stats.pending + stats.processing;
      await tgSend("sendMessage", {
        chat_id: adminItem.admin_chat_id,
        text: `📤 Joylandi: ${done}/${total}`,
      });
    }
  }
}
