/**
 * "➕ Yangi qism qo'shish" -> "🌐 Anibla orqali": ARUMEDIATV kodlash botidagi "Ilova uchun — Anibla orqali" bilan BIR XIL tizim
 * (`worker/src/anibla.rs`), faqat INLINE tugmalar ko'rinishida: panel (🆕 Oxirgi yuklanganlar, 📂 Bo'limlar, 📅 Yil bo'yicha, 🔍 Izlash, 📋 Yuklash navbati),
 * anime nomi yozib qidiriladi, anime rasm va to'liq ma'lumot bilan chiqadi, serial bo'lsa 🗂 fasl, keyin ▶️ qismlar (3 tadan, eng
 * yangisi birinchi). Qism tanlanganda sifat SO'RALMAYDI:
 *   1. Anibla'dagi BARCHA sifatlar uchun GitHub Actions'da ALOHIDA run (`tool/anibla/bot_download.py`): video yuklanadi,
 *      2 GB dan katta bo'lsa H.265 (CRF 30, sig'masa 31, 32, ...) bilan siqiladi va Telegram akkaunti orqali BOTGA yuboriladi
 *      (izoh: `ANIBLA:<ish>:<token>`);
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
const DUP_WINDOW_MS = 8 * 60 * 60 * 1000;

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
    st === "done" ? "✅ tayyor" : st === "error" ? "❌ xato" : st === "saving" ? "💾 saqlanmoqda" : st === "cancelled" ? "🗑 bekor" : "⏳ navbatda / yuklanmoqda";
  let text = `📋 <b>Yuklash navbati</b> — oxirgi ${rows.length} ta\n`;
  const kbRows: Btn[][] = [...controls(false, "0")];
  for (const r of rows) {
    text += `\n#${r.id} ${esc(r.title)} · ${r.quality} — ${icon(r.state)}`;
    if (r.state === "error" && r.error) text += `\n   <i>${esc(r.error)}</i>`;
    if (r.state === "queued" || r.state === "error") kbRows.push([[`🗑 #${r.id} ni olib tashlash`, `an:x:${r.id}`]]);
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

// ── 📊 Holat: yuklanayotgan videolarning jonli holati va logi ──

const WATCH_PREFIX = "anibla_watch_";
const WATCH_TTL_MS = 20 * 60 * 1000;

async function statusText(): Promise<string> {
  const now = Date.now();
  const active = await d1Run<{ id: number; title: string; quality: string; state: string; created_at: number; text: string | null; updated_at: number | null }>(
    `SELECT j.id, j.title, j.quality, j.state, j.created_at, p.text, p.updated_at
       FROM anibla_jobs j LEFT JOIN anibla_progress p ON p.job_id = j.id
      WHERE j.state IN ('queued','saving') AND j.created_at > ?
      ORDER BY j.id ASC LIMIT 6`,
    [now - 12 * 3600 * 1000]
  );
  const recent = await d1Run<{ id: number; title: string; quality: string; state: string; error: string }>(
    `SELECT id, title, quality, state, error FROM anibla_jobs WHERE state IN ('done','error') ORDER BY id DESC LIMIT 3`
  );
  let out = "🎞 <b>Anibla yuklash — holat</b>\n";
  if (active.length === 0) {
    out += "\n💤 Hozir yuklanayotgan video yo'q.";
  } else {
    out += `\n⚙️ Ishlayapti: ${active.length} ta\n`;
    for (const j of active) {
      out += `\n⬇️ <b>${esc(j.title)}</b> · ${j.quality} (#${j.id})\n`;
      if (j.state === "saving") {
        out += "💾 Bot kanalga saqlayapti...\n";
      } else if (j.text) {
        const age = Math.max(0, Math.round((now - (j.updated_at ?? now)) / 1000));
        out += `<pre>${esc(j.text.slice(-700))}</pre>🕒 yangilandi ${age < 90 ? `${age} soniya` : `${Math.round(age / 60)} daqiqa`} oldin\n`;
      } else {
        out += "⏳ Actions ishga tushmoqda (run navbatda)...\n";
      }
    }
  }
  if (recent.length) {
    out += "\n<b>Oxirgilari:</b>";
    for (const r of recent) {
      out += `\n${r.state === "done" ? "✅" : "❌"} #${r.id} ${esc(r.title)} · ${r.quality}${r.state === "error" && r.error ? ` — <i>${esc(r.error.slice(0, 120))}</i>` : ""}`;
    }
  }
  return out.slice(0, 4000);
}

async function showStatus(ctx: MyContext): Promise<void> {
  const chat = ctx.chat!.id;
  const text = await statusText();
  const kb = ik([[["🔄 Yangilash", "an:st"]], ...controls(false, "0")]);
  const old = ctx.callbackQuery?.message as { message_id: number; photo?: unknown } | undefined;
  let mid = 0;
  if (old && !old.photo) {
    try {
      await ctx.editMessageText(text, { ...HTML, reply_markup: kb });
      mid = old.message_id;
    } catch (e) {
      if (errText(e).includes("not modified")) mid = old.message_id;
    }
  }
  if (!mid) {
    if (old) await ctx.api.deleteMessage(chat, old.message_id).catch(() => {});
    const m = await ctx.reply(text, { ...HTML, reply_markup: kb });
    mid = m.message_id;
  }
  // Cron (har daqiqa) bu xabarni 20 daqiqa davomida o'zi yangilab turadi.
  await setSetting(`${WATCH_PREFIX}${chat}`, JSON.stringify({ mid, until: Date.now() + WATCH_TTL_MS, last: "" }));
}

/** Cron (har daqiqa): ochiq "📊 Holat" xabarlarini yangilaydi. */
export async function aniblaTick(api: Api): Promise<void> {
  const rows = await d1Run<{ key: string; value: string }>(`SELECT key, value FROM bot_settings WHERE key LIKE ?`, [`${WATCH_PREFIX}%`]);
  if (rows.length === 0) return;
  const text = await statusText();
  const kb = ik([[["🔄 Yangilash", "an:st"]], ...controls(false, "0")]);
  for (const r of rows) {
    const chat = Number(r.key.slice(WATCH_PREFIX.length));
    let w: { mid: number; until: number; last: string };
    try {
      w = JSON.parse(r.value);
    } catch {
      await d1Update(`DELETE FROM bot_settings WHERE key = ?`, [r.key]);
      continue;
    }
    if (!chat || Date.now() > w.until) {
      await d1Update(`DELETE FROM bot_settings WHERE key = ?`, [r.key]);
      continue;
    }
    if (text === w.last) continue;
    try {
      await api.editMessageText(chat, w.mid, text, { ...HTML, reply_markup: kb });
      await setSetting(r.key, JSON.stringify({ ...w, last: text }));
    } catch (e) {
      if (errText(e).includes("not modified")) continue;
      await d1Update(`DELETE FROM bot_settings WHERE key = ?`, [r.key]);
    }
  }
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
        api: config.WORKER_URL,
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
        const n = await d1Update(`UPDATE anibla_jobs SET state = 'cancelled' WHERE id = ? AND state IN ('queued','error')`, [parseInt(parts[2], 10)]);
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
        `Anibla'dagi barcha sifatlar alohida run bilan yuklanadi, 2 GB dan katta bo'lsa H.265 bilan siqiladi.\n` +
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
