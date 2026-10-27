/**
 * "➕ Yangi qism qo'shish" -> "🌐 Anibla orqali" (ARUMEDIATV kodlash botidagi "Anibla yuklash" bo'limi kabi):
 *   1. nom bo'yicha anibla.uz'dan qidiriladi;
 *   2. anime (serial bo'lsa fasl va qism) tanlanadi — qism tanlanishi bilan Anibla'dagi BARCHA sifatlar uchun ish boshlanadi;
 *   3. har bir sifat uchun GitHub Actions'da ALOHIDA run (`tool/anibla/bot_download.py`): video yuklanadi, 2 GB dan katta bo'lsa
 *      H.265 (CRF 30, sig'masa 31, 32, ...) bilan siqiladi va Telegram akkaunti orqali BOTGA yuboriladi (izoh: `ANIBLA:<ish>:<token>`);
 *   4. bot videoni yopiq kanalga saqlaydi, kanaldagi `file_id` ni oladi va bazaga yozadi (qo'lda qo'shishdagi kabi).
 * Keyingi so'rovda video to'g'ridan-to'g'ri kanaldagi `file_id` dan yuboriladi.
 */
import { Bot, InlineKeyboard } from "grammy";
import { config } from "../config.ts";
import { checkIsAdmin } from "../admin_check.ts";
import type { MyContext } from "./admin.ts";
import {
  getAnimeById,
  getAnimeInfo,
  getNextDownloadId,
  getNextEpisodeNumber,
  getSetting,
  setSetting,
  upsertEpisodeQuality,
  type Quality,
} from "../db.ts";
import { d1InsertReturningId, d1Run, d1Update } from "../db_d1.ts";
import { buildEpisodeCaptionFromInputs, getMainChannelUsername } from "./episodes.ts";
import {
  aniblaReady,
  dispatchRun,
  episodes,
  ghReady,
  isMovie,
  itemLabel,
  pickPerQuality,
  search,
  seasonLabel,
  seasons,
  variants,
  videoOf,
  type AEpisode,
  type AItem,
  type ASeason,
} from "../anibla.ts";

const EP_PAGE = 45;
const MENU_TEXTS = new Set(["🔍 Anime izlash", "💰 Balans to'ldirish", "💳 Hisobim", "💎 Obuna olish"]);
const DUP_WINDOW_MS = 8 * 60 * 60 * 1000;

interface Nav {
  /** Botdagi anime (season_db.id), qism shunga qo'shiladi. */
  target: number;
  q: string;
  items: AItem[];
  item?: number;
  seasons?: ASeason[];
  season?: string;
  eps?: AEpisode[];
}

const navKey = (chat: number) => `anibla_nav_${chat}`;

async function navGet(chat: number): Promise<Nav | null> {
  try {
    const raw = await getSetting(navKey(chat));
    return raw ? (JSON.parse(raw) as Nav) : null;
  } catch {
    return null;
  }
}
const navPut = (chat: number, nav: Nav) => setSetting(navKey(chat), JSON.stringify(nav));

const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
const errText = (e: unknown) => (e instanceof Error ? e.message : String(e));

function styleAll(kb: InlineKeyboard): InlineKeyboard {
  for (const row of kb.inline_keyboard) for (const btn of row) (btn as any).style = "primary";
  return kb;
}

function randomHex(bytes: number): string {
  const a = new Uint8Array(bytes);
  crypto.getRandomValues(a);
  return [...a].map((b) => b.toString(16).padStart(2, "0")).join("");
}

async function show(ctx: MyContext, text: string, kb?: InlineKeyboard): Promise<void> {
  const opts = { parse_mode: "HTML" as const, reply_markup: kb, link_preview_options: { is_disabled: true } };
  if (ctx.callbackQuery) {
    try {
      await ctx.editMessageText(text, opts);
      return;
    } catch { /* o'zgarmagan yoki eski xabar */ }
  }
  await ctx.reply(text, opts);
}

// ── Ro'yxatlar ───────────────────────────────────────────────

function listKb(nav: Nav): InlineKeyboard {
  const kb = new InlineKeyboard();
  nav.items.forEach((it, i) => kb.text(itemLabel(it).slice(0, 60), `an:i:${i}`).row());
  kb.text("❌ Bekor qilish", "an:x");
  return styleAll(kb);
}

function seasonsKb(nav: Nav): InlineKeyboard {
  const kb = new InlineKeyboard();
  (nav.seasons ?? []).forEach((s, i) => kb.text(seasonLabel(s).slice(0, 60), `an:z:${i}`).row());
  kb.text("« Orqaga", "an:b:l");
  return styleAll(kb);
}

function epsKb(nav: Nav, page: number): InlineKeyboard {
  const eps = nav.eps ?? [];
  const pages = Math.max(1, Math.ceil(eps.length / EP_PAGE));
  const cur = Math.min(Math.max(1, page), pages);
  const slice = eps.slice((cur - 1) * EP_PAGE, cur * EP_PAGE);
  const kb = new InlineKeyboard();
  for (let i = 0; i < slice.length; i += 3) {
    for (let j = i; j < Math.min(i + 3, slice.length); j++) {
      kb.text(`${slice[j].num}-qism`, `an:e:${(cur - 1) * EP_PAGE + j}`);
    }
    kb.row();
  }
  if (pages > 1) {
    if (cur > 1) kb.text("« oldingi", `an:p:${cur - 1}`);
    kb.text(`${cur}/${pages}`, "noop");
    if (cur < pages) kb.text("keyingi »", `an:p:${cur + 1}`);
    kb.row();
  }
  kb.text("« Orqaga", (nav.seasons?.length ?? 0) > 1 ? "an:b:s" : "an:b:l");
  return styleAll(kb);
}

async function showEpisodes(ctx: MyContext, chat: number, nav: Nav, page = 1): Promise<void> {
  const it = nav.items[nav.item ?? -1];
  await navPut(chat, nav);
  await show(ctx, `📺 <b>${esc(it?.t ?? "")}</b>\n\nQismni tanlang — Anibla'dagi BARCHA sifatlar yuklanadi:`, epsKb(nav, page));
}

async function openItem(ctx: MyContext, chat: number, nav: Nav, idx: number): Promise<void> {
  const it = nav.items[idx];
  if (!it) return;
  nav.item = idx;
  nav.seasons = undefined;
  nav.season = undefined;
  nav.eps = undefined;
  if (isMovie(it)) {
    await startJobs(ctx, chat, nav, it);
    return;
  }
  await show(ctx, "⏳ Fasllar olinmoqda...");
  let ss: ASeason[];
  try {
    ss = await seasons(it.s);
  } catch (e) {
    await show(ctx, `❌ Fasllarni olib bo'lmadi: ${esc(errText(e))}`, new InlineKeyboard().text("« Orqaga", "an:b:l"));
    return;
  }
  if (ss.length === 0) {
    await show(ctx, "❌ Bu serialda fasl topilmadi.", new InlineKeyboard().text("« Orqaga", "an:b:l"));
    return;
  }
  nav.seasons = ss;
  if (ss.length === 1) {
    await openSeason(ctx, chat, nav, 0);
    return;
  }
  await navPut(chat, nav);
  await show(ctx, `📺 <b>${esc(it.t)}</b>\n\nFaslni tanlang:`, seasonsKb(nav));
}

async function openSeason(ctx: MyContext, chat: number, nav: Nav, k: number): Promise<void> {
  const it = nav.items[nav.item ?? -1];
  const s = nav.seasons?.[k];
  if (!it || !s) return;
  await show(ctx, "⏳ Qismlar olinmoqda...");
  try {
    nav.eps = await episodes(it.s, s.slug);
  } catch (e) {
    await show(ctx, `❌ Qismlarni olib bo'lmadi: ${esc(errText(e))}`, new InlineKeyboard().text("« Orqaga", "an:b:l"));
    return;
  }
  nav.season = s.slug;
  if (nav.eps.length === 0) {
    await show(ctx, "❌ Bu faslda qism topilmadi.", new InlineKeyboard().text("« Orqaga", "an:b:l"));
    return;
  }
  await showEpisodes(ctx, chat, nav);
}

// ── Ishlarni boshlash: har bir sifat uchun alohida run ───────

async function startJobs(ctx: MyContext, chat: number, nav: Nav, it: AItem, ep?: AEpisode): Promise<void> {
  const info = await getAnimeInfo(nav.target);
  const anime = info ? await getAnimeById(info.id) : null;
  if (!info || !anime) {
    await show(ctx, "❌ Anime topilmadi (o'chirilgan bo'lishi mumkin).");
    return;
  }
  if (!ghReady()) {
    await show(ctx, "❌ GitHub Actions ulanmagan (GH_ACTIONS_TOKEN / GH_REPO). Deploy loglarini tekshiring.");
    return;
  }
  const head = `📺 <b>${esc(it.t)}</b>${ep ? ` · ${ep.num}-qism` : ""}`;
  await show(ctx, `${head}\n\n⏳ Video manzili tekshirilmoqda...`);

  let picks;
  try {
    const video = await videoOf(it, nav.season, ep);
    picks = pickPerQuality(await variants(video));
  } catch (e) {
    await show(ctx, `${head}\n\n❌ ${esc(errText(e))}`, new InlineKeyboard().text("« Orqaga", "an:b:l"));
    return;
  }
  if (picks.length === 0) {
    await show(ctx, `${head}\n\n❌ Sifatlar topilmadi.`);
    return;
  }

  const epNum = ep ? ep.num : await getNextEpisodeNumber(nav.target, info.kategoriya);
  const title = `${it.t} · ${ep ? `${ep.num}-qism` : "film"}`;
  const started: string[] = [];
  const skipped: string[] = [];
  const failed: string[] = [];

  for (const { q, v } of picks) {
    const dup = await d1Run(
      `SELECT id FROM anibla_jobs WHERE anime_info_id = ? AND ep_number = ? AND quality = ?
         AND state IN ('queued','saving') AND created_at > ?`,
      [nav.target, epNum, q, Date.now() - DUP_WINDOW_MS]
    );
    if (dup.length > 0) {
      skipped.push(q);
      continue;
    }
    const token = randomHex(16);
    const id = await d1InsertReturningId(
      `INSERT INTO anibla_jobs (token, admin_chat, anime_info_id, ep_number, quality, title, state, created_at)
       VALUES (?, ?, ?, ?, ?, ?, 'queued', ?)`,
      [token, chat, nav.target, epNum, q, title, Date.now()]
    );
    try {
      await dispatchRun({
        job: String(id),
        token,
        bot: ctx.me.username ?? "",
        quality: q,
        video: v.url,
        audio: v.audio,
        name: `${title} ${q}`,
      });
      started.push(q);
    } catch (e) {
      await d1Update(`UPDATE anibla_jobs SET state = 'error', error = ? WHERE id = ?`, [errText(e).slice(0, 300), id]);
      failed.push(`${q}: ${errText(e)}`);
    }
  }

  const lines = [head, ""];
  if (started.length) {
    lines.push(
      `▶️ Actions ishga tushirildi: <b>${started.join(", ")}</b> (${started.length} ta run, har sifat alohida).`,
      "Har bir sifat tayyor bo'lgach alohida xabar keladi. 2 GB dan katta bo'lsa H.265 bilan siqiladi."
    );
  }
  if (skipped.length) lines.push(`⏭ Allaqachon ishlayapti: ${skipped.join(", ")}`);
  if (failed.length) lines.push(`⚠️ Ishga tushmadi:\n${failed.map((f) => esc(f)).join("\n")}`);
  await show(ctx, lines.join("\n"), new InlineKeyboard().text("➕ Yana qism", `an:b:l`));
}

// ── Actions yuborgan videoni qabul qilish ────────────────────

interface Job {
  id: number;
  token: string;
  admin_chat: number;
  anime_info_id: number;
  ep_number: number;
  quality: Quality;
  title: string;
  state: string;
}

async function loadJob(id: number, token: string): Promise<Job | null> {
  const rows = await d1Run<Job>(`SELECT * FROM anibla_jobs WHERE id = ? AND token = ?`, [id, token]);
  return rows[0] ?? null;
}

async function finishJob(ctx: MyContext, job: Job, botFileId: string): Promise<void> {
  // Atomik: bir xil video ikki marta saqlanmasin.
  const claimed = await d1Update(`UPDATE anibla_jobs SET state = 'saving' WHERE id = ? AND state = 'queued'`, [job.id]);
  if (claimed !== 1) return;
  try {
    const info = await getAnimeInfo(job.anime_info_id);
    const anime = info ? await getAnimeById(info.id) : null;
    if (!info || !anime) throw new Error("anime topilmadi");

    const channelUsername = await getMainChannelUsername(ctx);
    const caption = buildEpisodeCaptionFromInputs(info, anime.name, job.ep_number, channelUsername, undefined, job.quality);
    // 1) Yopiq kanalga saqlash; 2) kanaldagi file_id ni olish.
    const sent = await ctx.api.sendVideo(config.SAVED_CHANNEL_ID, botFileId, { caption });
    const chanelFileId = sent.video?.file_id ?? botFileId;

    // 3) Bazaga yozish. Bir qismning sifatlari bir vaqtda kelsa, UNIQUE to'qnashuvida qayta uriniladi.
    let upserted: Awaited<ReturnType<typeof upsertEpisodeQuality>> | null = null;
    for (let attempt = 0; attempt < 3 && !upserted; attempt++) {
      try {
        upserted = await upsertEpisodeQuality({
          seasondb_id: job.anime_info_id,
          epizoddb_id: info.epizoddb_id,
          id: info.id,
          name: info.name,
          epizod_number: info.kategoriya === "TV" ? job.ep_number : null,
          film_id: info.kategoriya === "FILM" ? job.ep_number : null,
          ova_id: info.kategoriya === "OVA" ? job.ep_number : null,
          epizod_id: await getNextDownloadId(),
          quality: job.quality,
          chanel_id: chanelFileId,
          bot_id: botFileId,
        });
      } catch (e) {
        if (attempt === 2) throw e;
        await new Promise((r) => setTimeout(r, 400 * (attempt + 1)));
      }
    }

    // Kanalga post joylash vaqtincha o'chirilgan (`CHANNEL_POSTING`).
    if (upserted?.isNew && config.CHANNEL_POSTING) {
      try {
        const link = `https://t.me/${ctx.me.username}?start=ep_${upserted.epizod_id}`;
        const kb = new InlineKeyboard().url("• Yuklab Olish •", link);
        const cap = buildEpisodeCaptionFromInputs(info, anime.name, job.ep_number, channelUsername);
        await ctx.api.sendMessage(anime.chanel_id, cap, { reply_markup: kb });
      } catch (e) {
        console.error("anibla: kanal posti yuborilmadi:", e);
      }
    }

    await d1Update(`UPDATE anibla_jobs SET state = 'done' WHERE id = ?`, [job.id]);
    await ctx.api.sendMessage(
      job.admin_chat,
      `✅ <b>${esc(job.title)}</b> (${job.quality}) ${upserted?.isNew ? "qo'shildi" : "yangilandi"} — kanalga saqlandi, <code>file_id</code> bazaga yozildi.`,
      { parse_mode: "HTML" }
    ).catch(() => {});
  } catch (e) {
    console.error("anibla finishJob:", e);
    await d1Update(`UPDATE anibla_jobs SET state = 'error', error = ? WHERE id = ?`, [errText(e).slice(0, 300), job.id]);
    await ctx.api.sendMessage(
      job.admin_chat,
      `❌ <b>${esc(job.title)}</b> (${job.quality}) saqlanmadi: ${esc(errText(e))}`,
      { parse_mode: "HTML" }
    ).catch(() => {});
  }
}

// ── Ulash ────────────────────────────────────────────────────

export function setupAniblaAdd(bot: Bot<MyContext>): void {
  const isAdmin = (ctx: MyContext) => checkIsAdmin(ctx.from?.id);

  // Actions (Telegram akkaunti) yuborgan video: izoh `ANIBLA:<ish>:<token>`. Onboarding'dan OLDIN ishlaydi.
  bot.on("message:video", async (ctx, next) => {
    const m = (ctx.message.caption ?? "").match(/^ANIBLA:(\d+):([a-f0-9]{32})$/);
    if (!m || ctx.chat.type !== "private") return next();
    const job = await loadJob(Number(m[1]), m[2]);
    if (job) await finishJob(ctx, job, ctx.message.video.file_id);
  });

  bot.on("message:text", async (ctx, next) => {
    const text = ctx.message.text;

    // Actions xato haqida xabar beradi: `ANIBLA_FAIL:<ish>:<token>:<sabab>`.
    const f = text.match(/^ANIBLA_FAIL:(\d+):([a-f0-9]{32}):([\s\S]*)$/);
    if (f && ctx.chat.type === "private") {
      const job = await loadJob(Number(f[1]), f[2]);
      if (job && job.state === "queued") {
        await d1Update(`UPDATE anibla_jobs SET state = 'error', error = ? WHERE id = ? AND state = 'queued'`, [f[3].slice(0, 300), job.id]);
        await ctx.api.sendMessage(
          job.admin_chat,
          `❌ <b>${esc(job.title)}</b> (${job.quality}): ${esc(f[3].slice(0, 300))}`,
          { parse_mode: "HTML" }
        ).catch(() => {});
      }
      return;
    }

    if (ctx.session.state !== "waiting_anibla_query") return next();
    if (text.startsWith("/")) return next();
    if (MENU_TEXTS.has(text)) {
      ctx.session.state = "idle";
      return next();
    }
    if (!isAdmin(ctx)) return next();

    const target = ctx.session.aniblaTarget;
    if (!target) {
      ctx.session.state = "idle";
      return next();
    }
    const wait = await ctx.reply("🔎 Anibla'dan qidirilmoqda...");
    let items: AItem[];
    try {
      items = await search(text.trim());
    } catch (e) {
      await ctx.api.editMessageText(ctx.chat.id, wait.message_id, `❌ Qidirib bo'lmadi: ${esc(errText(e))}`, { parse_mode: "HTML" });
      return;
    }
    if (items.length === 0) {
      await ctx.api.editMessageText(
        ctx.chat.id,
        wait.message_id,
        `🤷 «${esc(text)}» bo'yicha hech narsa topilmadi.\nBoshqacha yozib ko'ring (o'zbekcha nomining bir qismi) yoki /cancel.`,
        { parse_mode: "HTML" }
      );
      return;
    }
    const nav: Nav = { target, q: text.trim(), items };
    await navPut(ctx.chat.id, nav);
    await ctx.api.editMessageText(
      ctx.chat.id,
      wait.message_id,
      `🔍 <b>«${esc(nav.q)}»</b> — ${items.length} ta natija\n📺 serial · 🎬 film\n\nTanlang:`,
      { parse_mode: "HTML", reply_markup: listKb(nav) }
    );
  });

  // Boshlash
  bot.callbackQuery(/^ep_add_an:(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    const animeInfoId = parseInt(ctx.match[1], 10);
    const info = await getAnimeInfo(animeInfoId);
    if (!info) { await ctx.answerCallbackQuery("Topilmadi"); return; }
    if (!aniblaReady()) {
      await ctx.answerCallbackQuery({ text: "Anibla login/paroli sozlanmagan (ANIBLA_CREDS)", show_alert: true });
      return;
    }
    ctx.session.currentAnimeInfoId = animeInfoId;
    ctx.session.aniblaTarget = animeInfoId;
    ctx.session.state = "waiting_anibla_query";
    await ctx.answerCallbackQuery();
    await ctx.reply(
      `🌐 <b>Anibla orqali</b>\n\n<b>${esc(info.name)}</b> ga qism qo'shiladi.\nAnibla'dan qidiriladigan nomni yozing (bekor qilish: /cancel):`,
      { parse_mode: "HTML" }
    );
  });

  bot.callbackQuery(/^an:(i|z|e|p):(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    const chat = ctx.chat?.id ?? ctx.from.id;
    const nav = await navGet(chat);
    if (!nav) {
      await ctx.answerCallbackQuery({ text: "Ro'yxat eskirgan — qaytadan qidiring", show_alert: true });
      return;
    }
    const n = parseInt(ctx.match[2], 10);
    await ctx.answerCallbackQuery();
    try {
      switch (ctx.match[1]) {
        case "i":
          await openItem(ctx, chat, nav, n);
          break;
        case "z":
          await openSeason(ctx, chat, nav, n);
          break;
        case "p":
          await showEpisodes(ctx, chat, nav, n);
          break;
        case "e": {
          const it = nav.items[nav.item ?? -1];
          const ep = nav.eps?.[n];
          if (it && ep) await startJobs(ctx, chat, nav, it, ep);
          break;
        }
      }
    } catch (e) {
      console.error("anibla nav:", e);
      await ctx.reply(`❌ Xato: ${esc(errText(e))}`, { parse_mode: "HTML" });
    }
  });

  bot.callbackQuery(/^an:(b:l|b:s|x)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    const chat = ctx.chat?.id ?? ctx.from.id;
    await ctx.answerCallbackQuery();
    if (ctx.match[1] === "x") {
      ctx.session.state = "idle";
      try { await ctx.deleteMessage(); } catch { /* eski */ }
      return;
    }
    const nav = await navGet(chat);
    if (!nav) return;
    if (ctx.match[1] === "b:s" && nav.seasons?.length) {
      await show(ctx, `📺 <b>${esc(nav.items[nav.item ?? 0]?.t ?? "")}</b>\n\nFaslni tanlang:`, seasonsKb(nav));
      return;
    }
    ctx.session.state = "waiting_anibla_query";
    await show(
      ctx,
      `🔍 <b>«${esc(nav.q)}»</b> — ${nav.items.length} ta natija\n📺 serial · 🎬 film\n\nTanlang (yoki yangi nom yozing):`,
      listKb(nav)
    );
  });
}
