/**
 * "➕ Yangi qism qo'shish" -> "🌐 Anibla orqali": ARUMEDIATV kodlash botidagi "Ilova uchun — Anibla orqali" bilan BIR XIL tizim
 * (`worker/src/anibla.rs`): pastki panel (🆕 Oxirgi yuklanganlar, 📂 Bo'limlar, 📅 Yil bo'yicha, 🔍 Izlash, 📋 Yuklash navbati),
 * anime nomi yozib qidiriladi, anime rasm va to'liq ma'lumot bilan chiqadi, serial bo'lsa 🗂 fasl, keyin ▶️ qismlar (3 tadan, eng
 * yangisi birinchi). Qism tanlanganda sifat SO'RALMAYDI:
 *   1. Anibla'dagi BARCHA sifatlar uchun GitHub Actions'da ALOHIDA run (`tool/anibla/bot_download.py`): video yuklanadi,
 *      2 GB dan katta bo'lsa H.265 (CRF 30, sig'masa 31, 32, ...) bilan siqiladi va Telegram akkaunti orqali BOTGA yuboriladi
 *      (izoh: `ANIBLA:<ish>:<token>`);
 *   2. bot videoni yopiq kanalga saqlaydi, kanaldagi `file_id` ni oladi va bazaga yozadi (qo'lda qo'shishdagi kabi).
 * Keyingi so'rovda video to'g'ridan-to'g'ri kanaldagi `file_id` dan yuboriladi.
 */
import { Bot, InlineKeyboard, Keyboard } from "grammy";
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
import { sendMainMenu } from "./main_menu.ts";
import {
  aniblaReady,
  categories,
  describe,
  dispatchRun,
  episodes,
  ghReady,
  imgUrl,
  isMovie,
  itemLabel,
  listing,
  pickPerQuality,
  seasonLabel,
  seasons,
  variants,
  videoOf,
  yearCounts,
  type AEpisode,
  type AItem,
  type ASeason,
} from "../anibla.ts";

const EP_PAGE = 50;
/** Telegram paneliga sig'adigan tugmalar soni (ARUMEDIATV botida sinalgan: 150 tagacha). */
const PANEL_MAX = 140;
const DUP_WINDOW_MS = 8 * 60 * 60 * 1000;

const BTN_NEW = "🆕 Oxirgi yuklanganlar";
const BTN_PREV = "◀️ Oldingi";
const BTN_NEXT = "Keyingi ▶️";
const BTN_BACK = "⬅️ Orqaga";
const BTN_HOME = "🏠 Bosh menyu";
const BTN_SEARCH = "🔍 Izlash";
const BTN_QUEUE = "📋 Yuklash navbati";
const BTN_CATS = "📂 Bo'limlar";
const BTN_YEARS = "📅 Yil bo'yicha";
const YEAR_PREFIX = "📅 ";

interface Nav {
  /** Botdagi anime (season_db.id), qism shunga qo'shiladi. */
  target: number;
  /** Qidiruv matni yoki bo'lim (`c`: ID, `new`, `y:2026`) va uning nomi. */
  q: string;
  c: string;
  lt: string;
  p: number;
  pages: number;
  total: number;
  items: AItem[];
  /** Ochiq ro'yxat: list | seasons | eps. */
  v: string;
  /** Tanlangan anime (items ichida), fasl, qismlar sahifasi. */
  i: number;
  j: number;
  ep: number;
  seasons?: ASeason[];
  /** Tanlangan faslning qismlari (eng yangisi birinchi). */
  eps?: AEpisode[];
}

const navKey = (chat: number) => `anibla_nav_${chat}`;

async function navGet(chat: number, target: number): Promise<Nav> {
  try {
    const raw = await getSetting(navKey(chat));
    if (raw) {
      const n = JSON.parse(raw) as Nav;
      n.target = target;
      return n;
    }
  } catch { /* yangisi */ }
  return { target, q: "", c: "", lt: "", p: 1, pages: 1, total: 0, items: [], v: "", i: -1, j: -1, ep: 0 };
}
const navPut = (chat: number, nav: Nav) => setSetting(navKey(chat), JSON.stringify(nav));

const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
const errText = (e: unknown) => (e instanceof Error ? e.message : String(e));

function stripHtml(s: string): string {
  return s.replace(/<[^>]*>/g, "").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&amp;/g, "&");
}

/** Telegram hisoblaydigan uzunlik (HTML teglari va `&amp;` kabilar hisobga kirmaydi). */
function visibleLen(html: string): number {
  return [...stripHtml(html)].length;
}

function randomHex(bytes: number): string {
  const a = new Uint8Array(bytes);
  crypto.getRandomValues(a);
  return [...a].map((b) => b.toString(16).padStart(2, "0")).join("");
}

// ── Pastki panel (ARUMEDIATV botidagi `panel()` bilan bir xil) ──

/** Doimiy boshqaruv tugmalari TEPADA (uzun ro'yxatda pastga aylantirmasdan ko'rinsin), keyin berilgan qatorlar. */
function panel(rows: string[][], back: boolean): Keyboard {
  const head = back
    ? [[BTN_BACK, BTN_HOME], [BTN_SEARCH, BTN_QUEUE]]
    : [[BTN_CATS, BTN_YEARS, BTN_SEARCH], [BTN_QUEUE, BTN_HOME]];
  const kb = new Keyboard();
  for (const r of [...head, ...rows]) {
    for (const t of r) kb.text(t);
    kb.row();
  }
  return kb.resized();
}

async function homePanel(): Promise<Keyboard> {
  const rows: string[][] = [[BTN_NEW]];
  const cats = await categories();
  for (let i = 0; i < cats.length; i += 2) rows.push(cats.slice(i, i + 2).map((c) => `📂 ${c.name}`));
  return panel(rows, false);
}

const HTML = { parse_mode: "HTML" as const, link_preview_options: { is_disabled: true } };

/** HTML xato bersa — oddiy matn bilan qayta uriniladi. */
async function sendText(ctx: MyContext, text: string, kb?: Keyboard): Promise<void> {
  try {
    await ctx.reply(text, { ...HTML, reply_markup: kb });
  } catch (e) {
    if (errText(e).includes("parse entities")) {
      await ctx.reply(stripHtml(text), { reply_markup: kb, link_preview_options: { is_disabled: true } });
      return;
    }
    throw e;
  }
}

function itemHead(it: AItem, desc: boolean): string {
  let head = `${isMovie(it) ? "🎬" : "📺"} <b>${esc(it.t)}</b>`;
  if (it.y > 0) head += ` (${it.y})`;
  if (desc && it.info) head += `\n\n${it.info}`;
  if (desc && it.about) head += `\n\n${it.about}`;
  return head;
}

/** Rasm (muqova) bilan karta: izoh 1024 belgidan oshsa — rasmga sarlavha, qolgani keyingi xabarga (panel o'sha xabarda). */
async function sendCard(ctx: MyContext, it: AItem, text: string, kb: Keyboard): Promise<void> {
  const photo = imgUrl(it.img);
  if (photo) {
    try {
      if (visibleLen(text) <= 1024) {
        await ctx.replyWithPhoto(photo, { caption: text, ...HTML, reply_markup: kb });
        return;
      }
      let cap = itemHead({ ...it, about: "" }, true);
      if (!text.startsWith(cap) || visibleLen(cap) > 1024) cap = itemHead(it, false);
      const rest = (text.startsWith(cap) ? text.slice(cap.length) : text).trimStart();
      await ctx.replyWithPhoto(photo, { caption: cap, ...HTML });
      await sendText(ctx, rest, kb);
      return;
    } catch (e) {
      console.error("anibla: rasm yuborilmadi:", errText(e));
    }
  }
  await sendText(ctx, text, kb);
}

/** Ro'yxat + panel. Panelga sig'masa (Telegram cheklovi) qisqartirib qayta uriniladi. */
async function sendList(ctx: MyContext, text: string, top: string[][], items: string[][], back: boolean, card?: AItem): Promise<void> {
  const total = items.reduce((n, r) => n + r.length, 0);
  let lastErr = "";
  for (const cap of [PANEL_MAX, 40]) {
    const rows = [...top];
    let shown = 0;
    for (const r of items) {
      if (shown + r.length > cap) break;
      shown += r.length;
      rows.push(r);
    }
    const t = shown < total
      ? `${text}\n\n⚠️ Telegram paneli cheklovi: ${total} tadan faqat birinchi ${shown} tasi ko'rsatildi. Qolganini nom yozib qidiring.`
      : text;
    try {
      if (card) await sendCard(ctx, card, t, panel(rows, back));
      else await sendText(ctx, t, panel(rows, back));
      return;
    } catch (e) {
      lastErr = errText(e);
      if (lastErr.includes("Too Many Requests")) break;
    }
  }
  await sendText(ctx, `❌ Ro'yxatni yuborib bo'lmadi: ${esc(lastErr)}\nNom yozib qidiring.`);
}

// ── Ro'yxatlar ───────────────────────────────────────────────

async function showListing(ctx: MyContext, chat: number, target: number, q: string, cat: string, label: string, page: number): Promise<void> {
  let res;
  try {
    res = await listing(q, cat, page);
  } catch (e) {
    await sendText(ctx, `❌ Ro'yxatni olib bo'lmadi: ${esc(errText(e))}`);
    return;
  }
  if (res.items.length === 0) {
    await sendText(
      ctx,
      q
        ? `🤷 <b>«${esc(q)}»</b> bo'yicha hech narsa topilmadi.\nBoshqacha yozib ko'ring (masalan, o'zbekcha nomining bir qismi).`
        : `🤷 <b>${esc(label)}</b> bo'limida hozircha hech narsa yo'q.`
    );
    return;
  }
  const nav: Nav = {
    target, q, c: cat, lt: label, p: page, pages: res.pages, total: res.total, items: res.items,
    v: "list", i: -1, j: -1, ep: 0,
  };
  await navPut(chat, nav);
  await listView(ctx, nav);
}

async function listView(ctx: MyContext, nav: Nav): Promise<void> {
  nav.v = "list";
  const top: string[][] = [];
  const nr: string[] = [];
  if (nav.p > 1) nr.push(BTN_PREV);
  if (nav.p < nav.pages) nr.push(BTN_NEXT);
  if (nr.length) top.push(nr);
  const items = nav.items.map((it) => [itemLabel(it)]);
  const head = nav.q
    ? `🔍 <b>«${esc(nav.q)}»</b> — ${nav.total} ta natija`
    : `📂 <b>${esc(nav.lt)}</b> — ${nav.total} ta`;
  const page = nav.pages > 1 ? ` · ${nav.p}/${nav.pages}-sahifa` : "";
  await navPut(ctx.chat!.id, nav);
  await sendList(ctx, `${head}${page}\n📺 serial · 🎬 film\n\n⬇️ Pastdan tanlang:`, top, items, false);
}

async function yearsPanel(ctx: MyContext): Promise<void> {
  const { list, why } = await yearCounts();
  if (list.length === 0) {
    await sendText(ctx, `❌ Yillar ro'yxatini olib bo'lmadi, birozdan keyin qayta urinib ko'ring.\n<code>${esc(why)}</code>`);
    return;
  }
  const total = list.reduce((n, x) => n + x[1], 0);
  const rows: string[][] = [];
  for (let i = 0; i < list.length; i += 2) rows.push(list.slice(i, i + 2).map(([y, n]) => `${YEAR_PREFIX}${y}-yil · ${n} ta anime`));
  await sendText(
    ctx,
    `📅 <b>Yil bo'yicha</b> — saytda jami ${total} ta anime, ${list.length} ta yil.\n\n⬇️ Yilni tanlang:`,
    panel(rows, false)
  );
}

// ── Anime -> fasl -> qismlar ─────────────────────────────────

async function ensureInfo(nav: Nav): Promise<void> {
  const it = nav.items[nav.i];
  if (it && !it.info && !it.about) {
    const d = await describe(it);
    it.info = d.info;
    it.about = d.about;
  }
}

async function showItem(ctx: MyContext, chat: number, nav: Nav): Promise<void> {
  const it = nav.items[nav.i];
  if (!it) return;
  await ensureInfo(nav);
  nav.j = -1;
  nav.ep = 0;
  nav.seasons = undefined;
  nav.eps = undefined;
  if (isMovie(it)) {
    await navPut(chat, nav);
    await sendCard(ctx, it, `${itemHead(it, true)}\n\n⏳ Anibla'dagi sifatlar tekshirilmoqda...`, panel([], true));
    await startJobs(ctx, chat, nav, it);
    return;
  }
  let ss: ASeason[];
  try {
    ss = await seasons(it.s);
  } catch (e) {
    await sendText(ctx, `${itemHead(it, false)}\n\n❌ ${esc(errText(e))}`);
    return;
  }
  if (ss.length === 0) {
    await sendText(ctx, `${itemHead(it, false)}\n\n🤷 Fasllar topilmadi.`);
    return;
  }
  nav.seasons = ss;
  if (ss.length > 1) {
    await showSeasons(ctx, chat, nav);
    return;
  }
  nav.j = 0;
  await showEps(ctx, chat, nav);
}

async function showSeasons(ctx: MyContext, chat: number, nav: Nav): Promise<void> {
  const it = nav.items[nav.i];
  const ss = nav.seasons ?? [];
  if (!it) return;
  await ensureInfo(nav);
  nav.v = "seasons";
  await navPut(chat, nav);
  const rows: string[][] = [];
  for (let i = 0; i < ss.length; i += 2) rows.push(ss.slice(i, i + 2).map((s) => `🗂 ${seasonLabel(s)}`));
  await sendCard(ctx, it, `${itemHead(it, true)}\n\n${ss.length} ta fasl — pastdan tanlang:`, panel(rows, true));
}

/** Qismlar pastki panelda: qatorga 3 tadan, 50 tadan sahifa, muqova bilan; eng yangisi birinchi. */
async function showEps(ctx: MyContext, chat: number, nav: Nav): Promise<void> {
  const it = nav.items[nav.i];
  const season = nav.seasons?.[nav.j];
  if (!it || !season) return;
  if (!nav.eps) {
    try {
      nav.eps = (await episodes(it.s, season.slug)).reverse();
    } catch (e) {
      await sendText(ctx, `❌ ${esc(errText(e))}`);
      return;
    }
  }
  const eps = nav.eps;
  if (eps.length === 0) {
    await sendText(ctx, "❌ Bu faslda qismlar topilmadi.");
    return;
  }
  await ensureInfo(nav);
  const pages = Math.max(1, Math.ceil(eps.length / EP_PAGE));
  nav.ep = Math.min(Math.max(nav.ep, 0), pages - 1);
  nav.v = "eps";
  await navPut(chat, nav);
  const part = eps.slice(nav.ep * EP_PAGE, (nav.ep + 1) * EP_PAGE);
  const top: string[][] = [];
  const nr: string[] = [];
  if (nav.ep > 0) nr.push(BTN_PREV);
  if (nav.ep + 1 < pages) nr.push(BTN_NEXT);
  if (nr.length) top.push(nr);
  const rows: string[][] = [];
  for (let i = 0; i < part.length; i += 3) rows.push(part.slice(i, i + 3).map((e) => `▶️ ${e.num}-qism`));
  const range = pages > 1 ? ` · ${part[0].num}–${part[part.length - 1].num}-qismlar` : "";
  const text =
    `${itemHead(it, true)}\n${esc(seasonLabel(season))} · ${eps.length} ta qism${range}\n\n` +
    `⬇️ Qismni pastdagi paneldan tanlang (eng yangisi birinchi). Anibla'dagi barcha sifatlar yuklanadi:`;
  await sendList(ctx, text, top, rows, true, it);
}

// ── Yuklash navbati ──────────────────────────────────────────

async function queueList(ctx: MyContext): Promise<void> {
  const rows = await d1Run<{ id: number; title: string; quality: string; state: string; error: string }>(
    `SELECT id, title, quality, state, error FROM anibla_jobs ORDER BY id DESC LIMIT 20`
  );
  if (rows.length === 0) {
    await sendText(ctx, "📋 Yuklash navbati bo'sh.");
    return;
  }
  const icon = (st: string) => (st === "done" ? "✅ tayyor" : st === "error" ? "❌ xato" : st === "saving" ? "💾 saqlanmoqda" : "⏳ navbatda / yuklanmoqda");
  let text = `📋 <b>Yuklash navbati</b> — oxirgi ${rows.length} ta\n`;
  const kb = new InlineKeyboard();
  for (const r of rows) {
    text += `\n#${r.id} ${esc(r.title)} · ${r.quality} — ${icon(r.state)}`;
    if (r.state === "error" && r.error) text += `\n   <i>${esc(r.error)}</i>`;
    if (r.state === "queued" || r.state === "error") kb.text(`🗑 #${r.id} ni olib tashlash`, `anq:${r.id}`).row();
  }
  await ctx.reply(text, { ...HTML, reply_markup: kb.inline_keyboard.length ? kb : undefined });
}

// ── Ishlarni boshlash: har bir sifat uchun alohida run ───────

async function startJobs(ctx: MyContext, chat: number, nav: Nav, it: AItem, ep?: AEpisode, seasonSlug?: string): Promise<void> {
  const info = await getAnimeInfo(nav.target);
  const anime = info ? await getAnimeById(info.id) : null;
  if (!info || !anime) {
    await sendText(ctx, "❌ Anime topilmadi (o'chirilgan bo'lishi mumkin).");
    return;
  }
  if (!ghReady()) {
    await sendText(ctx, "❌ GitHub Actions ulanmagan (GH_ACTIONS_TOKEN / GH_REPO). Deploy loglarini tekshiring.");
    return;
  }
  const head = `📺 <b>${esc(it.t)}</b>${ep ? ` · ${ep.num}-qism` : ""}`;
  const status = await ctx.reply(`${head}\n\n⏳ Video manzili tekshirilmoqda...`, HTML);
  const edit = (t: string) =>
    ctx.api.editMessageText(chat, status.message_id, t, HTML).catch(() => ctx.reply(t, HTML));

  let picks;
  try {
    const video = await videoOf(it, seasonSlug, ep);
    picks = pickPerQuality(await variants(video));
  } catch (e) {
    await edit(`${head}\n\n❌ ${esc(errText(e))}`);
    return;
  }
  if (picks.length === 0) {
    await edit(`${head}\n\n❌ Sifatlar topilmadi.`);
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
  await edit(lines.join("\n"));
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

// ── Pastki panel matnlarini yo'naltirish (ARUMEDIATV `anibla::on_message` kabi) ──

async function handleText(ctx: MyContext, text: string, target: number): Promise<void> {
  const chat = ctx.chat!.id;
  if (text === BTN_HOME) {
    ctx.session.state = "idle";
    await sendMainMenu(ctx);
    return;
  }
  if (text === BTN_QUEUE) return queueList(ctx);
  if (text === BTN_CATS) {
    await sendText(ctx, "📂 <b>Bo'limlar</b> (saytdagidek) — pastdan tanlang:", await homePanel());
    return;
  }
  if (!text || text === BTN_SEARCH) {
    await sendText(ctx, "✍️ Anime yoki film nomini yozing:");
    return;
  }
  if (text === BTN_NEW) return showListing(ctx, chat, target, "", "new", "Oxirgi yuklanganlar", 1);
  if (text === BTN_YEARS) return yearsPanel(ctx);

  // "📅 2026-yil · 53 ta anime" — shu yilning animelari.
  if (text.startsWith(YEAR_PREFIX)) {
    const rest = text.slice(YEAR_PREFIX.length);
    const y = parseInt(rest.split("-yil")[0], 10);
    if (rest.includes("-yil") && y >= 1900 && y <= 2200) {
      return showListing(ctx, chat, target, "", `y:${y}`, `${y}-yil`, 1);
    }
  }
  // "📂 Bo'lim nomi".
  if (text.startsWith("📂 ")) {
    const name = text.slice(3).trim();
    const c = (await categories()).find((x) => x.name === name);
    if (c) return showListing(ctx, chat, target, "", c.id, c.name, 1);
  }

  const nav = await navGet(chat, target);
  if (text === BTN_PREV || text === BTN_NEXT) {
    const d = text === BTN_NEXT ? 1 : -1;
    if (nav.v === "eps") {
      nav.ep = Math.max(0, nav.ep + d);
      return showEps(ctx, chat, nav);
    }
    if (nav.items.length) {
      const page = Math.min(Math.max(nav.p + d, 1), Math.max(nav.pages, 1));
      return showListing(ctx, chat, target, nav.q, nav.c, nav.lt, page);
    }
    return;
  }
  if (text === BTN_BACK) {
    if (nav.v === "eps" && (nav.seasons?.length ?? 0) > 1) return showSeasons(ctx, chat, nav);
    if ((nav.v === "eps" || nav.v === "seasons") && nav.items.length) return listView(ctx, nav);
    await sendText(ctx, "📂 Bo'limlar:", await homePanel());
    return;
  }
  // Ro'yxatdagi anime/film.
  const idx = nav.items.findIndex((it) => itemLabel(it) === text);
  if (idx >= 0) {
    nav.i = idx;
    return showItem(ctx, chat, nav);
  }
  // Fasl: "🗂 2-fasl".
  if (text.startsWith("🗂 ") && nav.seasons) {
    const k = nav.seasons.findIndex((s) => seasonLabel(s) === text.slice(3).trim());
    if (k >= 0) {
      nav.j = k;
      nav.ep = 0;
      nav.eps = undefined;
      return showEps(ctx, chat, nav);
    }
  }
  // Qism: "▶️ 12-qism" — sifat so'ralmaydi, hamma sifatlar yuklanadi.
  if (text.startsWith("▶️ ") && text.endsWith("-qism")) {
    const num = parseInt(text.slice(3), 10);
    const it = nav.items[nav.i];
    const ep = nav.eps?.find((e) => e.num === num);
    const season = nav.seasons?.[nav.j];
    if (it && ep && season) return startJobs(ctx, chat, nav, it, ep, season.slug);
    await sendText(ctx, `🤷 ${num}-qism topilmadi — ro'yxatdan qaytadan tanlang.`);
    return;
  }
  if ([...text].length < 3) {
    await sendText(ctx, "Kamida 3 ta harf yozing.");
    return;
  }
  return showListing(ctx, chat, target, [...text].slice(0, 80).join(""), "", "", 1);
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
    const text = ctx.message.text.trim();

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

    // Anibla rejimida (ARUMEDIATV botidagi `encbot_mode='anibla'` kabi) hamma matn shu yerga: tugmalar ham, qidiruv ham.
    if (ctx.session.state !== "waiting_anibla_query") return next();
    if (text.startsWith("/")) return next();
    if (!isAdmin(ctx)) return next();
    const target = ctx.session.aniblaTarget;
    if (!target) {
      ctx.session.state = "idle";
      return next();
    }
    try {
      await handleText(ctx, text, target);
    } catch (e) {
      console.error("anibla handleText:", e);
      await sendText(ctx, `❌ Xato: ${esc(errText(e))}`).catch(() => {});
    }
  });

  // Boshlash: "➕ Yangi qism qo'shish" -> "🌐 Anibla orqali".
  bot.callbackQuery(/^ep_add_an:(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    const animeInfoId = parseInt(ctx.match[1], 10);
    const info = await getAnimeInfo(animeInfoId);
    if (!info) { await ctx.answerCallbackQuery("Topilmadi"); return; }
    ctx.session.currentAnimeInfoId = animeInfoId;
    ctx.session.aniblaTarget = animeInfoId;
    ctx.session.state = "waiting_anibla_query";
    await ctx.answerCallbackQuery();
    await navPut(ctx.chat?.id ?? ctx.from.id, { target: animeInfoId, q: "", c: "", lt: "", p: 1, pages: 1, total: 0, items: [], v: "", i: -1, j: -1, ep: 0 });
    const warn = aniblaReady() ? "" : "\n\n⚠️ anibla.uz login/paroli sozlanmagan (<code>tool/anibla/creds.enc</code>).";
    await ctx.reply(
      `📱 <b>Anibla orqali</b>\n🎯 ${esc(info.name)}\n\n` +
        `📂 Pastdagi bo'limlardan tanlang yoki anime nomini yozing. Anime → fasl → qism: sifat so'ralmaydi — ` +
        `Anibla'dagi barcha sifatlar alohida run bilan yuklanadi, 2 GB dan katta bo'lsa H.265 bilan siqiladi.\n` +
        `↩️ Chiqish: «Bosh menyu».${warn}`,
      { ...HTML, reply_markup: await homePanel() }
    );
  });

  // Yuklash navbatidan olib tashlash (video kelsa ham saqlanmaydi).
  bot.callbackQuery(/^anq:(\d+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    const id = parseInt(ctx.match[1], 10);
    const n = await d1Update(`UPDATE anibla_jobs SET state = 'cancelled' WHERE id = ? AND state IN ('queued','error')`, [id]);
    await ctx.answerCallbackQuery(n === 1 ? "🗑 Olib tashlandi" : "Allaqachon tugagan");
  });
}
