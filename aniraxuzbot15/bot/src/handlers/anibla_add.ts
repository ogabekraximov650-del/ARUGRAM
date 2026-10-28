/**
 * "➕ Yangi qism qo'shish" -> "🌐 Anibla orqali": ARUMEDIATV kodlash botidagi "Ilova uchun — Anibla orqali" bilan BIR XIL tizim
 * (`worker/src/anibla.rs`), faqat INLINE tugmalar ko'rinishida: panel (🆕 Oxirgi yuklanganlar, 📂 Bo'limlar, 📅 Yil bo'yicha, 🔍 Izlash, 📋 Yuklash navbati),
 * anime nomi yozib qidiriladi, anime rasm va to'liq ma'lumot bilan chiqadi, serial bo'lsa 🗂 fasl, keyin ▶️ qismlar (3 tadan, eng
 * yangisi birinchi). Qism tanlanganda sifat SO'RALMAYDI:
 *   1. Anibla'dagi BARCHA sifatlar uchun GitHub Actions'da ALOHIDA run (`tool/anibla/bot_download.py`): video yuklanadi,
 *      2 GB dan katta bo'lsa qayta kodlamasdan aniq o'lchab qismlarga bo'linadi va Telegram akkaunti orqali BOTGA yuboriladi
 *      (izoh: `ANIBLA:<ish>:<token>`; 2 GB dan katta bo'lsa aniq o'lchab qismlarga bo'linadi — `:<i>/<n>`); fayl R2 orqali yuklovchi (qo'shimcha akkauntlar) tomonidan yuboriladi;
 *   2. bot videoni yopiq kanalga saqlaydi, kanaldagi `file_id` ni oladi va bazaga yozadi (qo'lda qo'shishdagi kabi).
 * Keyingi so'rovda video to'g'ridan-to'g'ri kanaldagi `file_id` dan yuboriladi.
 */
import { Api, Bot, InlineKeyboard } from "grammy";
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
  duration,
  hms,
  qualityName,
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
const MENU_TEXTS = new Set([
  // Foydalanuvchi menyusi
  "🔍 Anime izlash", "💰 Balans to'ldirish", "💳 Hisobim", "💎 Obuna olish",
  // Admin paneli (`adminMainKeyboard`)
  "📹 Animelarni boshqarish", "🎞 Anibla", "📢 Asosiy Kanal", "🔐 Majburiy obunalar",
  "🔍 Foydalanuvchi boshqaruvi", "🗄 Database", "👤 Adminlar", "✉️ Xabar yuborish",
]);

const BTN_NEW = "🆕 Oxirgi yuklanganlar";
const BTN_PREV = "◀️ Oldingi";
const BTN_NEXT = "Keyingi ▶️";
const BTN_BACK = "⬅️ Orqaga";
const BTN_HOME = "🏠 Bosh menyu";
const BTN_SEARCH = "🔍 Izlash";
const BTN_QUEUE = "📋 Yuklash navbati";
const BTN_STATUS = "📊 Holat";
const BTN_CATS = "📂 Bo'limlar";
const BTN_YEARS = "📅 Yil bo'yicha";
const YEAR_PREFIX = "📅 ";

interface Nav {
  /** Ro'yxat nonce'i: eskirgan tugmalar (boshqa ro'yxatdan qolgan) rad etiladi. */
  n: string;
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
  return { n: "0", target, q: "", c: "", lt: "", p: 1, pages: 1, total: 0, items: [], v: "", i: -1, j: -1, ep: 0 };
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

// ── Inline panel (ARUMEDIATV botidagi pastki panel o'rnida) ────

type Btn = [string, string];

function ik(rows: Btn[][]): InlineKeyboard {
  const kb = new InlineKeyboard();
  for (const r of rows) {
    for (const [t, d] of r) kb.text(t, d);
    kb.row();
  }
  return kb;
}

/** Doimiy boshqaruv tugmalari TEPADA, keyin berilgan qatorlar (panel botdagi `panel()` kabi). */
function controls(back: boolean, n: string): Btn[][] {
  return back
    ? [[[BTN_BACK, `an:${n}:b`], [BTN_HOME, "an:home"]], [[BTN_SEARCH, "an:s"], [BTN_QUEUE, "an:q"], [BTN_STATUS, "an:st"]]]
    : [[[BTN_CATS, "an:h"], [BTN_YEARS, "an:y"], [BTN_SEARCH, "an:s"]], [[BTN_QUEUE, "an:q"], [BTN_STATUS, "an:st"], [BTN_HOME, "an:home"]]];
}

async function homePanel(): Promise<InlineKeyboard> {
  const rows: Btn[][] = [...controls(false, "0"), [[BTN_NEW, "an:new"]]];
  const cats = await categories();
  for (let i = 0; i < cats.length; i += 2) {
    rows.push(cats.slice(i, i + 2).map((c, k): Btn => [`📂 ${c.name}`.slice(0, 40), `an:c:${i + k}`]));
  }
  return ik(rows);
}

const HTML = { parse_mode: "HTML" as const, link_preview_options: { is_disabled: true } };

/** HTML xato bersa — oddiy matn bilan qayta uriniladi. */
async function sendText(ctx: MyContext, text: string, kb?: InlineKeyboard): Promise<void> {
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

/** Rasm (muqova) bilan karta: izoh 1024 belgidan oshsa — rasmga sarlavha, qolgani keyingi xabarga (tugmalar o'sha xabarda). */
async function sendCard(ctx: MyContext, it: AItem, text: string, kb: InlineKeyboard): Promise<boolean> {
  const photo = imgUrl(it.img);
  if (!photo) return false;
  try {
    if (visibleLen(text) <= 1024) {
      await ctx.replyWithPhoto(photo, { caption: text, ...HTML, reply_markup: kb });
      return true;
    }
    let cap = itemHead({ ...it, about: "" }, true);
    if (!text.startsWith(cap) || visibleLen(cap) > 1024) cap = itemHead(it, false);
    const rest = (text.startsWith(cap) ? text.slice(cap.length) : text).trimStart();
    await ctx.replyWithPhoto(photo, { caption: cap, ...HTML });
    await sendText(ctx, rest, kb);
    return true;
  } catch (e) {
    console.error("anibla: rasm yuborilmadi:", errText(e));
    return false;
  }
}

/**
 * Xabarni ko'rsatadi: tugma bosilgan bo'lsa shu matn xabari TAHRIRLANADI (chat to'lib ketmasin), karta (rasm) bo'lsa yangi
 * xabar yuboriladi va eski xabar o'chiriladi.
 */
async function present(ctx: MyContext, text: string, kb: InlineKeyboard, card?: AItem): Promise<void> {
  const old = ctx.callbackQuery?.message as { message_id: number; photo?: unknown } | undefined;
  const chat = ctx.chat!.id;
  if (card && (await sendCard(ctx, card, text, kb))) {
    if (old) await ctx.api.deleteMessage(chat, old.message_id).catch(() => {});
    return;
  }
  if (old && !old.photo) {
    try {
      await ctx.editMessageText(text, { ...HTML, reply_markup: kb });
      return;
    } catch (e) {
      if (errText(e).includes("not modified")) return;
    }
  }
  if (old) await ctx.api.deleteMessage(chat, old.message_id).catch(() => {});
  await sendText(ctx, text, kb);
}

// ── Ro'yxatlar ───────────────────────────────────────────────

async function showListing(ctx: MyContext, chat: number, target: number, q: string, cat: string, label: string, page: number): Promise<void> {
  let res;
  try {
    res = await listing(q, cat, page);
  } catch (e) {
    await present(ctx, `❌ Ro'yxatni olib bo'lmadi: ${esc(errText(e))}`, ik(controls(false, "0")));
    return;
  }
  if (res.items.length === 0) {
    await present(
      ctx,
      q
        ? `🤷 <b>«${esc(q)}»</b> bo'yicha hech narsa topilmadi.\nBoshqacha yozib ko'ring (masalan, o'zbekcha nomining bir qismi).`
        : `🤷 <b>${esc(label)}</b> bo'limida hozircha hech narsa yo'q.`,
      ik(controls(false, "0"))
    );
    return;
  }
  const nav: Nav = {
    n: String(Date.now() % 1_000_000), target, q, c: cat, lt: label, p: page, pages: res.pages, total: res.total, items: res.items,
    v: "list", i: -1, j: -1, ep: 0,
  };
  await listView(ctx, chat, nav);
}

async function listView(ctx: MyContext, chat: number, nav: Nav): Promise<void> {
  nav.v = "list";
  const rows: Btn[][] = [...controls(false, nav.n)];
  const nr: Btn[] = [];
  if (nav.p > 1) nr.push([BTN_PREV, `an:${nav.n}:pg:${nav.p - 1}`]);
  if (nav.p < nav.pages) nr.push([BTN_NEXT, `an:${nav.n}:pg:${nav.p + 1}`]);
  if (nr.length) rows.push(nr);
  nav.items.forEach((it, i) => rows.push([[itemLabel(it).slice(0, 60), `an:${nav.n}:i:${i}`]]));
  const head = nav.q
    ? `🔍 <b>«${esc(nav.q)}»</b> — ${nav.total} ta natija`
    : `📂 <b>${esc(nav.lt)}</b> — ${nav.total} ta`;
  const page = nav.pages > 1 ? ` · ${nav.p}/${nav.pages}-sahifa` : "";
  await navPut(chat, nav);
  await present(ctx, `${head}${page}\n📺 serial · 🎬 film\n\n⬇️ Tanlang:`, ik(rows));
}

async function yearsPanel(ctx: MyContext): Promise<void> {
  const { list, why } = await yearCounts();
  if (list.length === 0) {
    await present(ctx, `❌ Yillar ro'yxatini olib bo'lmadi, birozdan keyin qayta urinib ko'ring.\n<code>${esc(why)}</code>`, ik(controls(false, "0")));
    return;
  }
  const total = list.reduce((n, x) => n + x[1], 0);
  const rows: Btn[][] = [...controls(false, "0")];
  for (let i = 0; i < list.length; i += 2) {
    rows.push(list.slice(i, i + 2).map(([y, n]): Btn => [`📅 ${y}-yil · ${n} ta`, `an:yr:${y}`]));
  }
  await present(ctx, `📅 <b>Yil bo'yicha</b> — saytda jami ${total} ta anime, ${list.length} ta yil.\n\n⬇️ Yilni tanlang:`, ik(rows));
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
    nav.v = "movie";
    await navPut(chat, nav);
    await present(ctx, `${itemHead(it, true)}\n\n⏳ Anibla'dagi sifatlar tekshirilmoqda...`, ik(controls(true, nav.n)), it);
    if (nav.target === 0) await showQualities(ctx, it);
    else await startJobs(ctx, chat, nav, it);
    return;
  }
  let ss: ASeason[];
  try {
    ss = await seasons(it.s);
  } catch (e) {
    await present(ctx, `${itemHead(it, false)}\n\n❌ ${esc(errText(e))}`, ik(controls(true, nav.n)));
    return;
  }
  if (ss.length === 0) {
    await present(ctx, `${itemHead(it, false)}\n\n🤷 Fasllar topilmadi.`, ik(controls(true, nav.n)));
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
  const rows: Btn[][] = [...controls(true, nav.n)];
  for (let i = 0; i < ss.length; i += 2) {
    rows.push(ss.slice(i, i + 2).map((s, k): Btn => [`🗂 ${seasonLabel(s)}`.slice(0, 40), `an:${nav.n}:z:${i + k}`]));
  }
  await present(ctx, `${itemHead(it, true)}\n\n${ss.length} ta fasl — tanlang:`, ik(rows), it);
}

/** Qismlar: qatorga 3 tadan, 45 tadan sahifa, muqova bilan; eng yangisi birinchi. */
async function showEps(ctx: MyContext, chat: number, nav: Nav): Promise<void> {
  const it = nav.items[nav.i];
  const season = nav.seasons?.[nav.j];
  if (!it || !season) return;
  if (!nav.eps) {
    try {
      nav.eps = (await episodes(it.s, season.slug)).reverse();
    } catch (e) {
      await present(ctx, `❌ ${esc(errText(e))}`, ik(controls(true, nav.n)));
      return;
    }
  }
  const eps = nav.eps;
  if (eps.length === 0) {
    await present(ctx, "❌ Bu faslda qismlar topilmadi.", ik(controls(true, nav.n)));
    return;
  }
  await ensureInfo(nav);
  const pages = Math.max(1, Math.ceil(eps.length / EP_PAGE));
  nav.ep = Math.min(Math.max(nav.ep, 0), pages - 1);
  nav.v = "eps";
  await navPut(chat, nav);
  const part = eps.slice(nav.ep * EP_PAGE, (nav.ep + 1) * EP_PAGE);
  const rows: Btn[][] = [...controls(true, nav.n)];
  const nr: Btn[] = [];
  if (nav.ep > 0) nr.push([BTN_PREV, `an:${nav.n}:ep:${nav.ep - 1}`]);
  if (nav.ep + 1 < pages) nr.push([BTN_NEXT, `an:${nav.n}:ep:${nav.ep + 1}`]);
  if (nr.length) rows.push(nr);
  for (let i = 0; i < part.length; i += 3) {
    rows.push(part.slice(i, i + 3).map((e): Btn => [`▶️ ${e.num}-qism`, `an:${nav.n}:e:${e.num}`]));
  }
  const range = pages > 1 ? ` · ${part[0].num}–${part[part.length - 1].num}-qismlar` : "";
  const text =
    `${itemHead(it, true)}\n${esc(seasonLabel(season))} · ${eps.length} ta qism${range}\n\n` +
    (nav.target === 0
      ? `⬇️ Qismni tanlang (eng yangisi birinchi) — sifatlari ko'rsatiladi (tekshirish rejimi, hech narsa yuklanmaydi):`
      : `⬇️ Qismni tanlang (eng yangisi birinchi). Anibla'dagi barcha sifatlar yuklanadi:`);
  await present(ctx, text, ik(rows), it);
}

// ── Yuklash navbati ──────────────────────────────────────────

async function queueList(ctx: MyContext): Promise<void> {
  const rows = await d1Run<{ id: number; title: string; quality: string; state: string; error: string }>(
    `SELECT id, title, quality, state, error FROM anibla_jobs ORDER BY id DESC LIMIT 20`
  );
  if (rows.length === 0) {
    await present(ctx, "📋 Yuklash navbati bo'sh.", ik(controls(false, "0")));
    return;
  }
  const icon = (st: string) =>
    st === "done" ? "✅ tayyor" : st === "error" ? "❌ xato" : st === "saving" ? "💾 saqlanmoqda" : st === "cancelled" ? "🗑 bekor" : st === "waiting" ? "🕓 navbatda" : st === "sending" ? "📤 R2'da (botga yuboriladi)" : "⚙️ Actions'da ishlayapti";
  let text = `📋 <b>Yuklash navbati</b> — oxirgi ${rows.length} ta\n`;
  const kbRows: Btn[][] = [...controls(false, "0")];
  for (const r of rows) {
    text += `\n#${r.id} ${esc(r.title)} · ${r.quality} — ${icon(r.state)}`;
    if (r.state === "error" && r.error) text += `\n   <i>${esc(r.error)}</i>`;
    if (r.state === "waiting" || r.state === "queued" || r.state === "sending" || r.state === "error") kbRows.push([[`🗑 #${r.id} ni olib tashlash`, `an:x:${r.id}`]]);
  }
  await present(ctx, text, ik(kbRows));
}

// ── Tekshirish rejimi: sifatlar ko'rsatiladi, hech narsa yuklanmaydi ──

async function showQualities(ctx: MyContext, it: AItem, ep?: AEpisode, seasonSlug?: string): Promise<void> {
  const head = `${isMovie(it) ? "🎬" : "📺"} <b>${esc(it.t)}</b>${ep ? ` · ${ep.num}-qism${ep.title && ep.title !== String(ep.num) ? `: ${esc(ep.title)}` : ""}` : ""}`;
  const st = await ctx.reply(`${head}\n\n⏳ Sifatlar tekshirilmoqda...`, HTML);
  const edit = (t: string) => ctx.api.editMessageText(ctx.chat!.id, st.message_id, t, HTML).catch(() => ctx.reply(t, HTML));
  try {
    const video = await videoOf(it, seasonSlug, ep);
    const vs = await variants(video);
    const secs = await duration(vs[0].url);
    const voice = vs[0].audioName ? `🔊 Ovoz: ${esc(vs[0].audioName)}\n` : "";
    const len = secs > 0 ? `⏱ ${hms(Math.round(secs))} · ` : "";
    const lines = vs.map((v) => {
      const mb = secs > 0 && v.bandwidth > 0 ? ` · ~${Math.round((v.bandwidth * secs) / 8 / 1048576)} MB` : "";
      return `⬇️ ${qualityName(v.height)}${mb}`;
    });
    await edit(`${head}\n\n${voice}${len}Mavjud sifatlar (tekshirish rejimi — yuklanmaydi):\n${lines.join("\n")}`);
  } catch (e) {
    await edit(`${head}\n\n❌ ${esc(errText(e))}`);
  }
}

// ── 📊 Holat: panel botdagi "Holat" bilan bir xil ko'rinish ──
// Har bir ishlayotgan video: qalin sarlavha ("⬇️ Yuklash #4: Anime"), ostida "qism · sifat", keyin runner yuborgan bosqichlar
// va log (oddiy matn). Holat — chat OXIRIGA yangi xabar; Actions har ~5 soniyada `/anibla/progress` ga yuboradi va shu xabar
// darhol tahrirlanadi (`aniblaPush`); cron har daqiqada zaxira sifatida yangilaydi. Tugmasiz, panel botdagidek.

const WATCH_PREFIX = "anibla_watch_";
const WATCH_TTL_MS = 8 * 3600 * 1000;
const EDIT_GAP_MS = 3000;
let lastEditAt = 0;
const lastText = new Map<string, string>();

interface Watch { mid: number; until: number }

function jobHead(j: { id: number; title: string; quality: string }): string {
  const i = j.title.lastIndexOf(" · ");
  const name = i > 0 ? j.title.slice(0, i) : j.title;
  const rest = i > 0 ? `${j.title.slice(i + 3)} · ${j.quality}` : j.quality;
  return `⬇️ <b>Yuklash #${j.id}: ${esc(name)}</b>\n${esc(rest)}`;
}

/** Matn `budget` belgiga sig'masa avval log qismi tashlanadi, keyin oxiri qoladi. */
function fitBody(text: string, budget: number): string {
  if (text.length <= budget) return text;
  const i = text.indexOf("\n\n📜 Log:");
  const j = text.lastIndexOf("\n\n⏱");
  if (i >= 0 && j > i) text = text.slice(0, i) + text.slice(j);
  return text.length <= budget ? text : text.slice(-budget);
}

// Tayyorlash tartibi: 360p dan boshlab (kichigidan kattasiga).
const QRANK: Record<string, number> = { "360p": 0, "480p": 1, "720p": 2, "1080p": 3 };
const STALE_MS = 30 * 60 * 1000;

/** Navbatni tozalaydi: tayyor ishlar o'chadi; 30 daqiqa holat bermagan (run to'xtagan/boshlanmagan) ishlar xatoga o'tadi; eski xatolar o'chadi. */
async function aniblaCleanup(): Promise<void> {
  const now = Date.now();
  await d1Update(`DELETE FROM anibla_jobs WHERE state = 'done'`);
  // Run'ga olingan ('queued') ishlar: run tirik bo'lsa uning joriy sifati har ~5 soniyada holat yuboradi. Olingandan 15 daqiqa o'tib,
  // hech bir 'queued' ish oxirgi 30 daqiqada holat bermagan bo'lsa — run boshlanmadi/to'xtadi: hammasi xato, navbat davom etadi.
  // (Navbatini kutayotgan keyingi sifatlarning o'z holati bo'lmaydi — ularni run'ning boshqa sifati "tirikligi" saqlaydi.)
  await d1Update(
    `UPDATE anibla_jobs SET state = 'error', error = ?
      WHERE state = 'queued'
        AND COALESCE((SELECT NULLIF(d.started_at, 0) FROM anibla_queue_data d WHERE d.job_id = anibla_jobs.id), created_at) < ?
        AND NOT EXISTS (
          SELECT 1 FROM anibla_progress p JOIN anibla_jobs q ON q.id = p.job_id
           WHERE q.state = 'queued' AND p.updated_at > ?
        )`,
    ["Actions run boshlanmadi yoki to'xtadi (holat kelmadi)", now - 15 * 60 * 1000, now - STALE_MS]
  );
  // R2'ga yozilgan, lekin 12 soatda botga yetib kelmagan ish (yuklovchi ishlamagan) — xato.
  await d1Update(
    `UPDATE anibla_jobs SET state = 'error', error = ?
      WHERE state = 'sending'
        AND COALESCE((SELECT NULLIF(d.started_at, 0) FROM anibla_queue_data d WHERE d.job_id = anibla_jobs.id), created_at) < ?`,
    ["R2'dan botga 12 soatda yetib kelmadi (yuklovchi ishlamadimi?)", now - 12 * 3600 * 1000]
  );
  await d1Update(`UPDATE anibla_jobs SET state = 'error', error = ? WHERE state = 'waiting' AND created_at < ?`, ["Navbatda 48 soatdan oshdi", now - 48 * 3600 * 1000]);
  await d1Update(`DELETE FROM anibla_jobs WHERE state IN ('error','cancelled') AND created_at < ?`, [now - 24 * 3600 * 1000]);
  await d1Update(`DELETE FROM anibla_queue_data WHERE job_id NOT IN (SELECT id FROM anibla_jobs)`);
  await d1Update(`DELETE FROM anibla_progress WHERE job_id NOT IN (SELECT id FROM anibla_jobs)`);
  await d1Update(`DELETE FROM anibla_part_files WHERE job_id NOT IN (SELECT id FROM anibla_jobs)`);
}

/** Bir nechta video bir vaqtda ishlasa log qismi tashlanadi (xabar aralash bo'lib ketmasin). */
const stripLog = (t: string): string => {
  const i = t.indexOf("\n\n📜 Log:");
  const j = t.lastIndexOf("\n\n⏱");
  return i >= 0 && j > i ? t.slice(0, i) + t.slice(j) : t;
};

async function statusText(): Promise<string> {
  const now = Date.now();
  const jobs = await d1Run<{ id: number; title: string; quality: string; state: string; text: string | null }>(
    `SELECT j.id, j.title, j.quality, j.state, p.text
       FROM anibla_jobs j LEFT JOIN anibla_progress p ON p.job_id = j.id
      WHERE j.state IN ('waiting','queued','sending','saving') AND j.created_at > ?
      ORDER BY j.id ASC LIMIT 40`,
    [now - 48 * 3600 * 1000]
  );
  const bad = await d1Run<{ id: number; title: string; quality: string; error: string }>(
    `SELECT id, title, quality, error FROM anibla_jobs WHERE state = 'error' ORDER BY id DESC LIMIT 3`
  );
  const order = (a: { title: string; quality: string; id: number }, b: { title: string; quality: string; id: number }) =>
    a.title.localeCompare(b.title) || (QRANK[a.quality] ?? 9) - (QRANK[b.quality] ?? 9) || a.id - b.id;
  const running = jobs.filter((j) => j.state === "saving" || (j.state === "queued" && j.text)).sort(order);
  const starting = jobs.filter((j) => j.state === "queued" && !j.text).sort(order);
  const sending = jobs.filter((j) => j.state === "sending").sort(order);
  const waiting = jobs.filter((j) => j.state === "waiting");  // id bo'yicha: avval navbatga qo'yilgan qism birinchi

  const parts: string[] = [];
  if (running.length) {
    const budget = Math.floor(3300 / running.length) - 160;
    const multi = running.length > 1;
    parts.push(
      running
        .map((j) => {
          const body = j.state === "saving"
            ? "💾 Bot kanalga saqlayapti..."
            : esc(fitBody(multi ? stripLog(j.text!) : j.text!, budget));
          return `${jobHead(j)}\n\n${body}`;
        })
        .join("\n\n━━━━━━━━━━\n\n")
    );
  }
  if (starting.length) {
    parts.push(
      `🚀 <b>Run'da, navbatini kutmoqda (${starting.length} ta):</b>\n` +
        starting.map((j) => `• #${j.id} ${esc(j.title)} · ${j.quality}`).join("\n")
    );
  }
  if (sending.length) {
    parts.push(
      `📤 <b>R2'da — yuklovchi botga yuboradi (${sending.length} ta):</b>\n` +
        sending.map((j) => `• #${j.id} ${esc(j.title)} · ${j.quality}`).join("\n")
    );
  }
  if (waiting.length) {
    parts.push(
      `🕓 <b>Navbatda — eng eski qismdan boshlab, har qismda 360p dan (${waiting.length} ta):</b>\n` +
        waiting.slice(0, 15).map((j) => `• #${j.id} ${esc(j.title)} · ${j.quality}`).join("\n") +
        (waiting.length > 15 ? `\n… va yana ${waiting.length - 15} ta` : "")
    );
  }
  if (bad.length) {
    parts.push(
      "❌ <b>Xatolar:</b>\n" +
        bad.map((r) => `• #${r.id} ${esc(r.title)} · ${r.quality}${r.error ? ` — <i>${esc(r.error.slice(0, 120))}</i>` : ""}`).join("\n")
    );
  }
  if (!running.length && !starting.length && !sending.length && !waiting.length) {
    parts.unshift("🎞 <b>Anibla yuklash</b>\n\n💤 Hozir yuklanayotgan video yo'q.\n\nYuklash boshlansa, shu xabar o'zi yangilanadi.");
  }
  return parts.join("\n\n━━━━━━━━━━\n\n").slice(0, 4000);
}

/** Xabarni tahrirlaydi. Telegram cheklasa ("retry after N") — N soniyani qaytaradi; xabar yo'qolgan bo'lsa kuzatuv o'chadi. */
async function editWatch(api: Api, key: string, chat: number, w: Watch, text: string): Promise<number> {
  if (lastText.get(key) === text) return 0;
  try {
    await api.editMessageText(chat, w.mid, text, HTML);
    lastText.set(key, text);
    return 0;
  } catch (e) {
    const m = errText(e);
    if (m.includes("not modified")) {
      lastText.set(key, text);
      return 0;
    }
    const r = /retry after (\d+)/i.exec(m);
    if (r) return Number(r[1]);
    await d1Update(`DELETE FROM bot_settings WHERE key = ?`, [key]);
    lastText.delete(key);
    return 0;
  }
}

function parseWatch(raw: string | null): Watch | null {
  if (!raw) return null;
  try {
    const w = JSON.parse(raw) as Watch;
    return w && w.mid && Date.now() <= w.until ? w : null;
  } catch {
    return null;
  }
}

/** Actions yangi holat yuborganda (`/anibla/progress`): ochiq "Holat" xabari darhol yangilanadi. `retry_after` runner'ga qaytadi. */
export async function aniblaPush(api: Api, chat: number): Promise<number> {
  const key = `${WATCH_PREFIX}${chat}`;
  const w = parseWatch(await getSetting(key));
  if (!w) return 0;
  const now = Date.now();
  if (now - lastEditAt < EDIT_GAP_MS) return 0;
  lastEditAt = now;
  return editWatch(api, key, chat, w, await statusText());
}

async function showStatus(ctx: MyContext): Promise<void> {
  const chat = ctx.chat!.id;
  await aniblaCleanup().catch((e) => console.error("anibla cleanup:", e));
  await dispatchAll().catch((e) => console.error("anibla dispatchAll:", e));
  const text = await statusText();
  const m = await ctx.reply(text, HTML);
  const key = `${WATCH_PREFIX}${chat}`;
  lastText.set(key, text);
  await setSetting(key, JSON.stringify({ mid: m.message_id, until: Date.now() + WATCH_TTL_MS }));
}

/** Cron (har daqiqa): ochiq "📊 Holat" xabarlarini zaxira sifatida yangilaydi. */
export async function aniblaTick(api: Api): Promise<void> {
  const rows = await d1Run<{ key: string; value: string }>(`SELECT key, value FROM bot_settings WHERE key LIKE ?`, [`${WATCH_PREFIX}%`]);
  const any = await d1Run(`SELECT id FROM anibla_jobs LIMIT 1`);
  if (rows.length === 0 && any.length === 0) return;
  // Ochiq "Holat" bo'lmasa ham: qotib qolgan ishlar xatoga o'tadi va shu sifatning navbatdagi ishi ishga tushadi.
  await aniblaCleanup().catch((e) => console.error("anibla cleanup:", e));
  await dispatchAll().catch((e) => console.error("anibla dispatchAll:", e));
  if (rows.length === 0) return;
  const text = await statusText();
  for (const r of rows) {
    const chat = Number(r.key.slice(WATCH_PREFIX.length));
    const w = parseWatch(r.value);
    if (!chat || !w) {
      await d1Update(`DELETE FROM bot_settings WHERE key = ?`, [r.key]);
      continue;
    }
    await editWatch(api, r.key, chat, w, text);
  }
}

// ── Navbat: BITTA Actions run = BITTA to'liq qism ──
// Qism navbatga qo'yilganda uning hamma sifati 'waiting' bo'lib yoziladi. Bot Actions'da `anibla_bot.yml` ni ishga tushiradi (bir vaqtda
// faqat bitta run). Run `POST /anibla/claim` bilan navbatdagi ENG ESKI qismning hamma sifatini oladi ('queued') va 360p dan boshlab
// birin-ketin tayyorlaydi; har tayyor fayl R2'ga yoziladi, R2 yuklovchisi uni botga yuboradi. Sifat R2'ga to'liq yozilgach ish
// 'sending' bo'ladi (`/anibla/r2done`); bot videoni olib kanalga saqlagach ish o'chadi. Hamma sifat 'queued' dan chiqqach keyingi run ochiladi.

const DISPATCH_GAP_MS = 3 * 60 * 1000;

/** Run ishlamayotgan bo'lsa va navbat bo'lsa — Actions run'ini ishga tushiradi (3 daqiqada ko'pi bilan bir marta). */
export async function dispatchAll(): Promise<void> {
  const active = await d1Run(`SELECT id FROM anibla_jobs WHERE state = 'queued' LIMIT 1`);
  if (active.length > 0) return; // run ishlayapti
  const waiting = await d1Run(`SELECT id FROM anibla_jobs WHERE state = 'waiting' LIMIT 1`);
  if (waiting.length === 0 || !ghReady()) return;
  const last = parseInt((await getSetting("anibla_last_dispatch")) ?? "", 10) || 0;
  if (Date.now() - last < DISPATCH_GAP_MS) return; // yaqinda ishga tushirilgan — run boshlanishini kutamiz
  await setSetting("anibla_last_dispatch", String(Date.now()));
  await dispatchRun({});
}

export interface ClaimedJob { job: number; token: string; quality: string; video: string; audio: string; name: string }

/** Run chaqiradi: navbatdagi eng eski qismning hamma sifatini (360p dan) oladi. Boshqa run ishlayotgan bo'lsa yoki navbat bo'sh bo'lsa — null. */
export async function aniblaClaim(): Promise<{ jobs: ClaimedJob[]; bot: string } | null> {
  await aniblaCleanup().catch((e) => console.error("anibla cleanup:", e));
  const first = await d1Run<{ id: number; anime_info_id: number; ep_number: number }>(
    `SELECT id, anime_info_id, ep_number FROM anibla_jobs WHERE state = 'waiting' ORDER BY id ASC LIMIT 1`
  );
  if (first.length === 0) return null;
  const f = first[0];
  // Atomik: boshqa ish 'queued' bo'lsa (boshqa run ishlayapti) hech narsa olinmaydi.
  const got = await d1Update(
    `UPDATE anibla_jobs SET state = 'queued'
      WHERE id = ? AND state = 'waiting' AND NOT EXISTS (SELECT 1 FROM anibla_jobs x WHERE x.state = 'queued')`,
    [f.id]
  );
  if (got !== 1) return null;
  await d1Update(`UPDATE anibla_jobs SET state = 'queued' WHERE state = 'waiting' AND anime_info_id = ? AND ep_number = ?`, [f.anime_info_id, f.ep_number]);
  const rows = await d1Run<{ id: number; token: string; quality: string; video: string; name: string; bot: string }>(
    `SELECT j.id, j.token, j.quality, d.video, d.name, d.bot
       FROM anibla_jobs j JOIN anibla_queue_data d ON d.job_id = j.id
      WHERE j.state = 'queued' AND j.anime_info_id = ? AND j.ep_number = ?`,
    [f.anime_info_id, f.ep_number]
  );
  rows.sort((a, b) => (QRANK[a.quality] ?? 9) - (QRANK[b.quality] ?? 9) || a.id - b.id);
  await d1Update(
    `UPDATE anibla_queue_data SET started_at = ? WHERE job_id IN (SELECT id FROM anibla_jobs WHERE state = 'queued' AND anime_info_id = ? AND ep_number = ?)`,
    [Date.now(), f.anime_info_id, f.ep_number]
  );
  const fail = async (id: number, msg: string) =>
    d1Update(`UPDATE anibla_jobs SET state = 'error', error = ? WHERE id = ?`, [msg.slice(0, 300), id]);
  if (rows.length === 0) return null;

  // Havola vaqt o'tib eskirgan bo'lishi mumkin — run boshlanganda qayta aniqlanadi.
  let picks: ReturnType<typeof pickPerQuality>;
  try {
    picks = pickPerQuality(await variants(rows[0].video));
  } catch (e) {
    for (const r of rows) await fail(r.id, errText(e));
    return null;
  }
  const jobs: ClaimedJob[] = [];
  for (const r of rows) {
    const pick = picks.find((p) => p.q === r.quality);
    if (!pick) {
      await fail(r.id, `${r.quality} sifati endi topilmadi`);
      continue;
    }
    jobs.push({ job: r.id, token: r.token, quality: r.quality, video: pick.v.url, audio: pick.v.audio, name: r.name });
  }
  if (jobs.length === 0) return null;
  return { jobs, bot: rows[0].bot };
}

/** Run: sifat R2'ga to'liq yozildi — ish 'sending' (R2 yuklovchisi botga yuboradi); keyingi run ochilishi mumkin. */
export async function aniblaR2Done(jobId: number, token: string): Promise<boolean> {
  const n = await d1Update(`UPDATE anibla_jobs SET state = 'sending' WHERE id = ? AND token = ? AND state = 'queued'`, [jobId, token]);
  if (n === 1) await dispatchAll().catch((e) => console.error("anibla dispatchAll:", e));
  return n === 1;
}

// ── Ishlarni boshlash: qism sifatlari navbatga qo'yiladi ─────

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
  let video = "";
  try {
    video = await videoOf(it, seasonSlug, ep);
    picks = pickPerQuality(await variants(video));
  } catch (e) {
    await edit(`${head}\n\n❌ ${esc(errText(e))}`);
    return;
  }
  if (picks.length === 0) {
    await edit(`${head}\n\n❌ Sifatlar topilmadi.`);
    return;
  }

  // Film: har safar yangi raqam berilsa sifatlar turli "film"larga tarqalib ketadi (masalan 1080p alohida yozuvda qoladi).
  // Shuning uchun bitta Anibla filmi (slug) shu bo'lim ichida DOIM bir xil raqamni oladi.
  let epNum: number;
  if (ep) {
    epNum = ep.num;
  } else {
    const filmKey = `anibla_film_${nav.target}_${it.s}`;
    const saved = parseInt((await getSetting(filmKey)) ?? "", 10);
    epNum = saved > 0 ? saved : await getNextEpisodeNumber(nav.target, info.kategoriya);
    if (!(saved > 0)) await setSetting(filmKey, String(epNum));
  }
  const title = `${it.t} · ${ep ? `${ep.num}-qism` : "film"}`;
  const added: { id: number; q: string }[] = [];
  const skipped: string[] = [];

  // Tartib muhim: id o'sish bo'yicha — avval navbatga qo'yilgan qismning sifatlari birinchi tayyorlanadi.
  for (const { q } of picks) {
    const dup = await d1Run(
      `SELECT id FROM anibla_jobs WHERE anime_info_id = ? AND ep_number = ? AND quality = ?
         AND state IN ('waiting','queued','sending','saving')`,
      [nav.target, epNum, q]
    );
    if (dup.length > 0) {
      skipped.push(q);
      continue;
    }
    const token = randomHex(16);
    const id = await d1InsertReturningId(
      `INSERT INTO anibla_jobs (token, admin_chat, anime_info_id, ep_number, quality, title, state, created_at)
       VALUES (?, ?, ?, ?, ?, ?, 'waiting', ?)`,
      [token, chat, nav.target, epNum, q, title, Date.now()]
    );
    await d1Update(
      `INSERT INTO anibla_queue_data (job_id, video, name, bot) VALUES (?, ?, ?, ?)`,
      [id, video, `${title} ${q}`, ctx.me.username ?? ""]
    );
    added.push({ id, q });
  }
  let dispatchErr = "";
  if (added.length > 0) {
    await dispatchAll().catch((e) => { dispatchErr = errText(e); console.error("anibla dispatchAll:", e); });
  }

  const lines = [head, ""];
  if (added.length) {
    const qs = added.map((a) => a.q).sort((x, y) => (QRANK[x] ?? 9) - (QRANK[y] ?? 9));
    lines.push(
      `🕓 Navbatga qo'yildi: <b>${qs.join(", ")}</b>.`,
      "Actions bitta run'da eng eski qismdan boshlab sifatlarni 360p dan ketma-ket tayyorlaydi; tayyor fayl R2 orqali botga keladi.",
      "Har sifat tayyor bo'lgach alohida xabar keladi. 2 GB dan katta bo'lsa qismlarga bo'linadi."
    );
  }
  if (skipped.length) lines.push(`⏭ Allaqachon navbatda/ishlayapti: ${skipped.join(", ")}`);
  if (dispatchErr) lines.push(`⚠️ Actions hozir ishga tushmadi (${esc(dispatchErr)}) — navbat saqlandi, 3 daqiqada qayta uriniladi.`);
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

/** Workflow qadami yiqilsa (akkaunt yo'q, yuklash xatosi...) `/anibla/fail` chaqiradi: ish xatoga o'tadi, navbat davom etadi. */
export async function aniblaRunFailed(api: Api, jobId: number, token: string, reason: string): Promise<boolean> {
  const job = await loadJob(jobId, token);
  if (!job || (job.state !== "queued" && job.state !== "sending")) return false;
  await d1Update(`UPDATE anibla_jobs SET state = 'error', error = ? WHERE id = ? AND state IN ('queued','sending')`, [reason.slice(0, 300), job.id]);
  await dispatchAll().catch((e) => console.error("anibla dispatchAll:", e));
  await api.sendMessage(job.admin_chat, `❌ <b>${esc(job.title)}</b> (${job.quality}): ${esc(reason.slice(0, 300))}`, { parse_mode: "HTML" }).catch(() => {});
  return true;
}

/** Qism/sifat yozuvini bazaga yozadi (bor bo'lsa yangilaydi). Bir qismning sifatlari bir vaqtda kelsa, UNIQUE to'qnashuvida qayta uriniladi. */
async function upsertForJob(
  job: Job,
  info: NonNullable<Awaited<ReturnType<typeof getAnimeInfo>>>,
  chanelFileId: string,
  botFileId: string
): Promise<Awaited<ReturnType<typeof upsertEpisodeQuality>>> {
  let lastErr: unknown;
  for (let attempt = 0; attempt < 3; attempt++) {
    try {
      return await upsertEpisodeQuality({
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
      lastErr = e;
      await new Promise((r) => setTimeout(r, 400 * (attempt + 1)));
    }
  }
  throw lastErr;
}

/**
 * 2 GB dan katta video qismi (`ANIBLA:<ish>:<token>:<i>/<n>`) kelishi bilan: kanalga saqlanadi va bazadagi yozuvga
 * VERGUL bilan qo'shiladi ("id1" -> "id1,id2" -> "id1,id2,id3"). Hamma qism yozilgach ish navbatdan o'chadi.
 * Yozuv `anibla_part_files` dan qismlar tartibida qayta yig'iladi — takror kelgan qism ikki marta yozilmaydi.
 */
async function savePart(ctx: MyContext, job: Job, idx: number, total: number, botFileId: string): Promise<void> {
  if (job.state !== "queued" && job.state !== "sending") return;
  const ex = await d1Run<{ chanel_id: string }>(`SELECT chanel_id FROM anibla_part_files WHERE job_id = ? AND idx = ?`, [job.id, idx]);
  if (ex[0]?.chanel_id) return; // takror yuborilgan
  try {
    const info = await getAnimeInfo(job.anime_info_id);
    const anime = info ? await getAnimeById(info.id) : null;
    if (!info || !anime) throw new Error("anime topilmadi");
    const channelUsername = await getMainChannelUsername(ctx);
    const caption = buildEpisodeCaptionFromInputs(info, anime.name, job.ep_number, channelUsername, undefined, job.quality);
    const sent = await ctx.api.sendVideo(config.SAVED_CHANNEL_ID, botFileId, { caption: `${caption}\n\n📦 ${idx}/${total}-qism` });
    const chanelId = sent.video?.file_id ?? botFileId;
    await d1Update(
      `INSERT INTO anibla_part_files (job_id, idx, total, bot_id, chanel_id) VALUES (?, ?, ?, ?, ?)
       ON CONFLICT(job_id, idx) DO UPDATE SET bot_id = excluded.bot_id, chanel_id = excluded.chanel_id, total = excluded.total`,
      [job.id, idx, total, botFileId, chanelId]
    );

    // Bazaga: oldingi yozuv + vergul + yangi. Parallel kelgan qism yozuvini yo'qotmaslik uchun natija qayta tekshiriladi.
    let written = "";
    let count = 0;
    for (let pass = 0; pass < 2; pass++) {
      const rows = await d1Run<{ bot_id: string; chanel_id: string }>(
        `SELECT bot_id, chanel_id FROM anibla_part_files WHERE job_id = ? AND chanel_id != '' ORDER BY idx ASC`,
        [job.id]
      );
      const chanel = rows.map((r) => r.chanel_id).join(",");
      if (chanel === written) break;
      await upsertForJob(job, info, chanel, rows.map((r) => r.bot_id).join(","));
      written = chanel;
      count = rows.length;
    }

    if (count >= total) {
      await d1Update(`DELETE FROM anibla_jobs WHERE id = ?`, [job.id]);
      await d1Update(`DELETE FROM anibla_progress WHERE job_id = ?`, [job.id]);
      await d1Update(`DELETE FROM anibla_part_files WHERE job_id = ?`, [job.id]);
      await d1Update(`DELETE FROM anibla_queue_data WHERE job_id = ?`, [job.id]);
      // Run o'z ishini tugatdi (oxirgi qism keldi) — shu sifat workflow'i navbatdagi keyingi qismga o'tadi.
      await dispatchAll().catch((e) => console.error("anibla dispatchAll:", e));
      await ctx.api.sendMessage(
        job.admin_chat,
        `✅ <b>${esc(job.title)}</b> (${job.quality}) — ${total} qismli video kanalga saqlandi, <code>file_id</code> lar bazaga vergul bilan yozildi.`,
        { parse_mode: "HTML" }
      ).catch(() => {});
    }
  } catch (e) {
    console.error("anibla savePart:", e);
    await d1Update(`UPDATE anibla_jobs SET state = 'error', error = ? WHERE id = ?`, [`${idx}/${total}-qism: ${errText(e)}`.slice(0, 300), job.id]);
    await dispatchAll().catch((e2) => console.error("anibla dispatchAll:", e2));
    await ctx.api.sendMessage(
      job.admin_chat,
      `❌ <b>${esc(job.title)}</b> (${job.quality}) ${idx}/${total}-qism saqlanmadi: ${esc(errText(e))}\nOldingi qismlar bazada qolgan bo'lishi mumkin — videoni qayta yuklang.`,
      { parse_mode: "HTML" }
    ).catch(() => {});
  }
}

async function finishJob(ctx: MyContext, job: Job, botFileIds: string[]): Promise<void> {
  // Atomik: bir xil video ikki marta saqlanmasin.
  const claimed = await d1Update(`UPDATE anibla_jobs SET state = 'saving' WHERE id = ? AND state IN ('queued','sending')`, [job.id]);
  if (claimed !== 1) return;
  // Run o'z ishini tugatdi (video botga keldi) — shu sifat workflow'i navbatdagi keyingi qismga o'tadi.
  await dispatchAll().catch((e) => console.error("anibla dispatchAll:", e));
  try {
    const info = await getAnimeInfo(job.anime_info_id);
    const anime = info ? await getAnimeById(info.id) : null;
    if (!info || !anime) throw new Error("anime topilmadi");

    const channelUsername = await getMainChannelUsername(ctx);
    const caption = buildEpisodeCaptionFromInputs(info, anime.name, job.ep_number, channelUsername, undefined, job.quality);
    // 1) Yopiq kanalga saqlash; 2) kanaldagi file_id ni olish.
    // 2 GB dan katta video bir nechta qism bo'lsa — har biri kanalga saqlanadi; bazaga vergul bilan yoziladi ("id1,id2,id3").
    const chanelIds: string[] = [];
    for (let i = 0; i < botFileIds.length; i++) {
      const cap = botFileIds.length > 1 ? `${caption}\n\n📦 ${i + 1}/${botFileIds.length}-qism` : caption;
      const sent = await ctx.api.sendVideo(config.SAVED_CHANNEL_ID, botFileIds[i], { caption: cap });
      chanelIds.push(sent.video?.file_id ?? botFileIds[i]);
    }
    const chanelFileId = chanelIds.join(",");
    const botFileId = botFileIds.join(",");

    // 3) Bazaga yozish.
    const upserted = await upsertForJob(job, info, chanelFileId, botFileId);

    // Kanalga post joylash vaqtincha o'chirilgan (`CHANNEL_POSTING`).
    if (upserted.isNew && config.CHANNEL_POSTING) {
      try {
        const link = `https://t.me/${ctx.me.username}?start=ep_${upserted.epizod_id}`;
        const kb = new InlineKeyboard().url("• Yuklab Olish •", link);
        const cap = buildEpisodeCaptionFromInputs(info, anime.name, job.ep_number, channelUsername);
        await ctx.api.sendMessage(anime.chanel_id, cap, { reply_markup: kb });
      } catch (e) {
        console.error("anibla: kanal posti yuborilmadi:", e);
      }
    }

    // Tayyor ish navbatdan butunlay o'chiriladi (video allaqachon bazada).
    await d1Update(`DELETE FROM anibla_jobs WHERE id = ?`, [job.id]);
    await d1Update(`DELETE FROM anibla_progress WHERE job_id = ?`, [job.id]);
    await d1Update(`DELETE FROM anibla_queue_data WHERE job_id = ?`, [job.id]);
    await ctx.api.sendMessage(
      job.admin_chat,
      `✅ <b>${esc(job.title)}</b> (${job.quality}) ${upserted.isNew ? "qo'shildi" : "yangilandi"} — kanalga saqlandi, <code>file_id</code> bazaga yozildi.`,
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

// ── Tugmalar va matn ─────────────────────────────────────────

/** Anibla rejimida yozilgan matn: nom bo'yicha qidiruv (tugmalar inline — `an:` callback'lari). */
async function handleText(ctx: MyContext, text: string, target: number): Promise<void> {
  const chat = ctx.chat!.id;
  if ([...text].length < 3) {
    await sendText(ctx, "Kamida 3 ta harf yozing.");
    return;
  }
  return showListing(ctx, chat, target, [...text].slice(0, 80).join(""), "", "", 1);
}

/** Rejim: `0` — tekshirish (🎞 Anibla), `>0` — qism qo'shiladigan anime, `null` — Anibla rejimi yo'q. */
function modeTarget(ctx: MyContext): number | null {
  if (ctx.session.aniblaCheck) return 0;
  return ctx.session.aniblaTarget ?? null;
}

async function goHome(ctx: MyContext): Promise<void> {
  await present(ctx, "📂 <b>Bo'limlar</b> (saytdagidek) — tanlang yoki anime nomini yozing:", await homePanel());
}

async function onCallback(ctx: MyContext, data: string): Promise<void> {
  const chat = ctx.chat?.id ?? ctx.from!.id;
  const parts = data.split(":");
  const target = modeTarget(ctx);
  if (target === null) {
    ctx.session.state = "idle";
    await ctx.answerCallbackQuery({ text: "Seans tugagan — '🎞 Anibla' yoki '➕ Yangi qism qo'shish' dan qaytadan boshlang", show_alert: true });
    return;
  }
  ctx.session.state = "waiting_anibla_query";
  const act = parts[1];

  // Nonce'siz amallar.
  if (!/^\d+$/.test(act)) {
    await ctx.answerCallbackQuery();
    switch (act) {
      case "home":
        ctx.session.state = "idle";
        try { await ctx.deleteMessage(); } catch { /* eski */ }
        await sendMainMenu(ctx);
        return;
      case "h":
        return goHome(ctx);
      case "q":
        return queueList(ctx);
      case "st":
        return showStatus(ctx);
      case "y":
        return yearsPanel(ctx);
      case "s":
        await sendText(ctx, "✍️ Anime yoki film nomini yozing (kamida 3 harf):");
        return;
      case "new":
        return showListing(ctx, chat, target, "", "new", "Oxirgi yuklanganlar", 1);
      case "yr": {
        const y = parseInt(parts[2], 10);
        if (y >= 1900 && y <= 2200) return showListing(ctx, chat, target, "", `y:${y}`, `${y}-yil`, 1);
        return;
      }
      case "c": {
        const c = (await categories())[parseInt(parts[2], 10)];
        if (c) return showListing(ctx, chat, target, "", c.id, c.name, 1);
        return goHome(ctx);
      }
      case "x": {
        const n = await d1Update(`UPDATE anibla_jobs SET state = 'cancelled' WHERE id = ? AND state IN ('waiting','queued','sending','error')`, [parseInt(parts[2], 10)]);
        await dispatchAll().catch((e) => console.error("anibla dispatchAll:", e));
        await ctx.api.sendMessage(chat, n === 1 ? "🗑 Navbatdan olib tashlandi (video kelsa ham saqlanmaydi)." : "Allaqachon tugagan.").catch(() => {});
        return;
      }
    }
    return;
  }

  // Ro'yxatga bog'liq amallar: nonce mos kelishi shart.
  const nav = await navGet(chat, target);
  if (nav.n !== act) {
    await ctx.answerCallbackQuery({ text: "Ro'yxat eskirgan — qaytadan tanlang", show_alert: true });
    return;
  }
  await ctx.answerCallbackQuery();
  const kind = parts[2];
  const arg = parseInt(parts[3] ?? "", 10);
  switch (kind) {
    case "pg":
      return showListing(ctx, chat, target, nav.q, nav.c, nav.lt, Math.min(Math.max(arg, 1), Math.max(nav.pages, 1)));
    case "i":
      nav.i = arg;
      return showItem(ctx, chat, nav);
    case "z":
      nav.j = arg;
      nav.ep = 0;
      nav.eps = undefined;
      return showEps(ctx, chat, nav);
    case "ep":
      nav.ep = Math.max(0, arg);
      return showEps(ctx, chat, nav);
    case "b":
      if (nav.v === "eps" && (nav.seasons?.length ?? 0) > 1) return showSeasons(ctx, chat, nav);
      if ((nav.v === "eps" || nav.v === "seasons" || nav.v === "movie") && nav.items.length) return listView(ctx, chat, nav);
      return goHome(ctx);
    case "e": {
      const it = nav.items[nav.i];
      const ep = nav.eps?.find((e) => e.num === arg);
      const season = nav.seasons?.[nav.j];
      if (it && ep && season) {
        if (nav.target === 0) return showQualities(ctx, it, ep, season.slug);
        return startJobs(ctx, chat, nav, it, ep, season.slug);
      }
      await sendText(ctx, `🤷 ${arg}-qism topilmadi — ro'yxatdan qaytadan tanlang.`);
      return;
    }
  }
}

// ── Ulash ────────────────────────────────────────────────────

export function setupAniblaAdd(bot: Bot<MyContext>): void {
  const isAdmin = (ctx: MyContext) => checkIsAdmin(ctx.from?.id);

  // Actions (Telegram akkaunti) yuborgan video: izoh `ANIBLA:<ish>:<token>`. Onboarding'dan OLDIN ishlaydi.
  // 2 GB dan katta video qismlarga bo'lingan bo'lsa izoh: `ANIBLA:<ish>:<token>:<i>/<n>` — hamma qism kelgach saqlanadi.
  bot.on("message:video", async (ctx, next) => {
    const m = (ctx.message.caption ?? "").match(/^ANIBLA:(\d+):([a-f0-9]{32})(?::(\d+)\/(\d+))?$/);
    if (!m || ctx.chat.type !== "private") return next();
    const job = await loadJob(Number(m[1]), m[2]);
    if (!job) return;
    const fileId = ctx.message.video.file_id;
    if (!m[3]) {
      await finishJob(ctx, job, [fileId]);
      return;
    }
    const idx = Number(m[3]);
    const total = Number(m[4]);
    if (!(idx >= 1 && idx <= total && total >= 2 && total <= 30)) return;
    await savePart(ctx, job, idx, total, fileId);
  });

  bot.on("message:text", async (ctx, next) => {
    const text = ctx.message.text.trim();

    // Actions xato haqida xabar beradi: `ANIBLA_FAIL:<ish>:<token>:<sabab>`.
    const f = text.match(/^ANIBLA_FAIL:(\d+):([a-f0-9]{32}):([\s\S]*)$/);
    if (f && ctx.chat.type === "private") {
      const job = await loadJob(Number(f[1]), f[2]);
      if (job && (job.state === "queued" || job.state === "sending")) {
        await d1Update(`UPDATE anibla_jobs SET state = 'error', error = ? WHERE id = ? AND state IN ('queued','sending')`, [f[3].slice(0, 300), job.id]);
        await dispatchAll().catch((e) => console.error("anibla dispatchAll:", e));
        await ctx.api.sendMessage(
          job.admin_chat,
          `❌ <b>${esc(job.title)}</b> (${job.quality}): ${esc(f[3].slice(0, 300))}`,
          { parse_mode: "HTML" }
        ).catch(() => {});
      }
      return;
    }

    // Anibla rejimida yozilgan matn — qidiruv.
    if (ctx.session.state !== "waiting_anibla_query") return next();
    // Buyruq (/start, /admin ...) yoki asosiy menyu tugmasi — Anibla rejimidan chiqiladi, odatdagidek ishlaydi.
    if (text.startsWith("/") || MENU_TEXTS.has(text)) {
      ctx.session.state = "idle";
      return next();
    }
    if (!isAdmin(ctx)) return next();
    const target = modeTarget(ctx);
    if (target === null) {
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
    ctx.session.aniblaCheck = false;
    ctx.session.state = "waiting_anibla_query";
    await ctx.answerCallbackQuery();
    await navPut(ctx.chat?.id ?? ctx.from.id, {
      n: "0", target: animeInfoId, q: "", c: "", lt: "", p: 1, pages: 1, total: 0, items: [], v: "", i: -1, j: -1, ep: 0,
    });
    const warn = aniblaReady() ? "" : "\n\n⚠️ anibla.uz login/paroli sozlanmagan (<code>tool/anibla/creds.enc</code>).";
    await ctx.reply(
      `📱 <b>Anibla orqali</b>\n🎯 ${esc(info.name)}\n\n` +
        `📂 Pastdagi bo'limlardan tanlang yoki anime nomini yozing. Anime → fasl → qism: sifat so'ralmaydi — ` +
        `Anibla'dagi barcha sifatlar alohida run bilan yuklanadi, 2 GB dan katta bo'lsa qismlarga bo'linadi (kodlanmaydi).\n` +
        `↩️ Chiqish: «Bosh menyu».${warn}`,
      { ...HTML, reply_markup: await homePanel() }
    );
  });

  // "🎞 Anibla" (admin panelida): tekshirish rejimi — anime/qism/sifatlar ko'riladi, hech narsa yuklanmaydi.
  bot.hears("🎞 Anibla", async (ctx) => {
    if (!isAdmin(ctx)) return;
    ctx.session.aniblaCheck = true;
    ctx.session.aniblaTarget = undefined;
    ctx.session.state = "waiting_anibla_query";
    await navPut(ctx.chat.id, {
      n: "0", target: 0, q: "", c: "", lt: "", p: 1, pages: 1, total: 0, items: [], v: "", i: -1, j: -1, ep: 0,
    });
    const warn = aniblaReady() ? "" : "\n\n⚠️ anibla.uz login/paroli sozlanmagan (<code>tool/anibla/creds.enc</code>).";
    await ctx.reply(
      `🎞 <b>Anibla — tekshirish</b>\n\n` +
        `📂 Bo'limlardan tanlang yoki anime nomini yozing. Anime → fasl → qism: qismning sifatlari va taxminiy hajmi ko'rsatiladi. ` +
        `Hech narsa yuklanmaydi.\n📊 Holat — yuklanayotgan videolarning jonli logi.\n↩️ Chiqish: «Bosh menyu».${warn}`,
      { ...HTML, reply_markup: await homePanel() }
    );
  });

  // Barcha Anibla tugmalari: `an:...` (qo'lda tugmalar — admin uchun).
  bot.callbackQuery(/^an:(.+)$/, async (ctx) => {
    if (!isAdmin(ctx)) { await ctx.answerCallbackQuery(); return; }
    try {
      await onCallback(ctx, ctx.match[0]);
    } catch (e) {
      console.error("anibla onCallback:", e);
      await ctx.answerCallbackQuery().catch(() => {});
      await sendText(ctx, `❌ Xato: ${esc(errText(e))}`).catch(() => {});
    }
  });
}
