/**
 * Zanjir post tizimi — har bir Worker invokatsiyasi bir qism joylaydi,
 * keyin keyingi invokatsiyani triggerlaiydi.
 * Bu Cloudflare Workers timeout muammosini hal qiladi.
 */
import { config } from "./config.ts";
import {
  getEpisode,
  getAnimeInfo,
  getAnimeById,
  getAvailableQualities,
  type AnimeInfo,
  type Episode,
  type Quality,
} from "./db.ts";

export interface PostChainParams {
  botToken: string;
  adminChatId: number;
  animeInfoId: number;
  channelId: string;
  botUsername: string;
  channelUsername: string;
  episodeRowids: number[];
  currentIndex: number;
  workerUrl: string;
}

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
  channelUsername: string,
  quality: Quality | null
): string {
  const seasonPart =
    info.kategoriya === "TV" && info.season_id ? ` {${info.season_id}-fasl}` : "";
  const num = ep.epizod_number ?? ep.film_id ?? ep.ova_id ?? 0;
  let line = "";
  if (info.kategoriya === "TV") line = `${epName}${seasonPart} [${pad2(num)}-qism]`;
  else if (info.kategoriya === "FILM") line = `${epName} [${num}-Film]`;
  else line = `${epName} [${pad2(num)}-qism]`;
  let out = "";
  if (quality) out += `Sifati: ${quality} formatda\n`;
  out += `${line}\nKanal: ${channelUsername || "—"}`;
  return out;
}

async function tgSend(botToken: string, method: string, body: object): Promise<boolean> {
  try {
    const r = await fetch(`https://api.telegram.org/bot${botToken}/${method}`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    const d = (await r.json()) as { ok: boolean; description?: string };
    if (!d.ok) console.error(`tgSend ${method} fail:`, d.description);
    return d.ok ?? false;
  } catch (e) {
    console.error(`tgSend ${method} error:`, e);
    return false;
  }
}

/** Bir qismni joylaydi, keyin keyingi /post-next invokatsiyasini trigger qiladi */
export async function runPostChain(params: PostChainParams): Promise<void> {
  const {
    botToken,
    adminChatId,
    animeInfoId,
    channelId,
    botUsername,
    channelUsername,
    episodeRowids,
    currentIndex,
    workerUrl,
  } = params;

  // Kanalga post joylash vaqtincha o'chirilgan / posting to channels is temporarily off
  if (!config.CHANNEL_POSTING) return;

  // Validatsiya — botToken yoki boshqa zaruriy maydonlar yo'q bo'lsa to'xta
  if (!botToken || !channelId || !episodeRowids?.length) {
    console.error("runPostChain: invalid params", {
      hasToken: !!botToken,
      channelId,
      count: episodeRowids?.length,
    });
    return;
  }

  const total = episodeRowids.length;

  // Hammasi joylandi
  if (currentIndex >= total) {
    await tgSend(botToken, "sendMessage", {
      chat_id: adminChatId,
      text: `✅ Barcha ${total} ta post joylandi.`,
    });
    return;
  }

  // Joriy qismni joyla
  const rowid = episodeRowids[currentIndex];
  console.log(`runPostChain: posting index=${currentIndex}/${total} rowid=${rowid}`);

  const ep = await getEpisode(rowid);
  if (!ep) {
    console.error(`runPostChain: episode rowid=${rowid} topilmadi, o'tkazib yuboriladi`);
  } else {
    const info = await getAnimeInfo(animeInfoId);
    if (info) {
      const anime = await getAnimeById(info.id);
      const animeName = anime?.name ?? info.name;
      const epName = ep.name && ep.name.trim() ? ep.name.trim() : animeName;
      const bestQ = pickDefaultQuality(ep);
      const cap = buildCaption(ep, info, epName, channelUsername, bestQ);
      const link = `https://t.me/${botUsername}?start=ep_${ep.epizod_id}`;

      const ok = await tgSend(botToken, "sendMessage", {
        chat_id: channelId,
        text: cap,
        reply_markup: {
          inline_keyboard: [[{ text: "• Yuklab Olish •", url: link }]],
        },
      });

      // Agar Telegram xato qaytarsa (masalan, 429 flood) — 5 sekund kutib qayta urinish
      if (!ok) {
        await new Promise((r) => setTimeout(r, 5000));
        await tgSend(botToken, "sendMessage", {
          chat_id: channelId,
          text: cap,
          reply_markup: {
            inline_keyboard: [[{ text: "• Yuklab Olish •", url: link }]],
          },
        });
      }
    }
  }

  // Har 10 tadan bir yoki oxirida — admin ga progress
  const done = currentIndex + 1;
  if (done % 10 === 0 || done === total) {
    await tgSend(botToken, "sendMessage", {
      chat_id: adminChatId,
      text: `📤 Joylandi: ${done}/${total}`,
    });
  }

  // 3 sekund kutib keyingi qismni trigger qil
  await new Promise((r) => setTimeout(r, 3000));

  const nextParams: PostChainParams = { ...params, currentIndex: currentIndex + 1 };
  try {
    const res = await fetch(`${workerUrl}/post-next`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(nextParams),
    });
    if (!res.ok) {
      console.error(`runPostChain: /post-next trigger failed: ${res.status}`);
    }
  } catch (e) {
    console.error("runPostChain: post-next trigger error:", e);
  }
}
