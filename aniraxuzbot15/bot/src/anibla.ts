/**
 * anibla.uz bilan ishlash (ARUMEDIATV workeridagi `worker/src/anibla.rs` ning TypeScript nusxasi, "Anibla yuklash" bo'limi).
 *
 * SAYT: `<sayt>/api/backend/api/v1/...`; kirish `POST auth/login-password` -> `token.accessToken` (cookie `access_token`);
 * qidiruv `media/mobile` (loginsiz); serial: `seasons/<slug>`, `episodes/<slug>/<fasl>?view=compact` (loginsiz);
 * qism videosi `episodes/<slug>/<fasl>/<qism>` yoki film `movies/<slug>` (LOGIN bilan) -> `video` = `<sayt>/stream/<id>`;
 * `<video>?format=api` -> `{"file": ".../index.m3u8"}` — HLS master (sifatlar). Playlist va bo'laklar loginsiz ochiladi.
 * Video baytlari worker'dan O'TMAYDI: worker faqat JSON va m3u8 matnini o'qiydi, yuklashni GitHub Actions bajaradi.
 */
import { config } from "./config.ts";
import { getSetting, setSetting } from "./db.ts";

const DEFAULT_SITE = "https://anibla.uz";
const UA = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0 Safari/537.36";
/** Har bir sifat uchun alohida workflow fayli: anibla_bot_1080p.yml, anibla_bot_720p.yml, ... */
export const workflowOf = (q: string): string => `anibla_bot_${q}.yml`;

// deno-lint-ignore no-explicit-any
type Json = any;

export interface AItem {
  s: string; t: string; m: string; y: number; e: number;
  /** Muqova (poster) yo'li — `imgUrl()` bilan to'liq manzilga aylanadi. */
  img: string;
  /** Tanlanganda `describe()` to'ldiradi (tayyor HTML): ma'lumot va tavsif. */
  info?: string;
  about?: string;
}
export interface ASeason { slug: string; title: string }
export interface AEpisode { slug: string; num: number; title: string }
export interface AVariant { height: number; bandwidth: number; url: string; audio: string; audioName: string }

export const isMovie = (it: AItem): boolean => !it.m.toLowerCase().includes("serie");

export function itemLabel(it: AItem): string {
  let l = `${isMovie(it) ? "🎬" : "📺"} ${it.t}`;
  if (it.y > 0) l += ` · ${it.y}`;
  if (!isMovie(it) && it.e > 0) l += ` · ${it.e} qism`;
  return l;
}

interface Creds { site: string; phone: number; password: string }

function creds(): Creds | null {
  try {
    const v = JSON.parse(config.ANIBLA_CREDS || "{}");
    const phone = String(v.login ?? "").replace(/\D/g, "");
    const password = String(v.password ?? "");
    if (!phone || !password) return null;
    const site = String(v.site ?? "").trim().replace(/\/+$/, "");
    return { site: site.startsWith("http") ? site : DEFAULT_SITE, phone: Number(phone), password };
  } catch {
    return null;
  }
}

export const aniblaReady = (): boolean => creds() !== null;
const site = (): string => creds()?.site ?? DEFAULT_SITE;
const apiBase = (): string => `${site()}/api/backend/api/v1`;
const enc = (s: string): string => encodeURIComponent(s);

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Sayt 502/503/504 bersa 3 martagacha qayta uriniladi. */
async function http(url: string, init: RequestInit = {}): Promise<{ code: number; text: string }> {
  let last = { code: 0, text: "" };
  for (let i = 0; i < 4; i++) {
    const r = await fetch(url, {
      ...init,
      headers: { "User-Agent": UA, Accept: "application/json, text/plain, */*", ...(init.headers ?? {}) },
    });
    last = { code: r.status, text: await r.text() };
    if (![502, 503, 504].includes(last.code)) return last;
    await sleep(600 * (i + 1));
  }
  return last;
}

let memToken = "";

async function login(): Promise<string> {
  const c = creds();
  if (!c) throw new Error("anibla login/paroli sozlanmagan (ANIBLA_CREDS)");
  const { code, text } = await http(`${c.site}/api/backend/api/v1/auth/login-password`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ phone_number: c.phone, password: c.password }),
  });
  let v: Json = {};
  try { v = JSON.parse(text); } catch { /* matn emas */ }
  const tok = String(v?.token?.accessToken ?? v?.data?.token?.accessToken ?? "");
  if (!tok) {
    const why = String(v?.code ?? v?.message ?? "");
    if (code >= 500) throw new Error(`sayt vaqtincha ishlamayapti (HTTP ${code}) — birozdan keyin urinib ko'ring`);
    throw new Error(`saytga kirib bo'lmadi (HTTP ${code} ${why})`);
  }
  memToken = tok;
  await setSetting("anibla_token", tok).catch(() => {});
  return tok;
}

async function token(fresh: boolean): Promise<string> {
  if (!fresh) {
    if (memToken) return memToken;
    const saved = await getSetting("anibla_token").catch(() => null);
    if (saved) return (memToken = saved);
  }
  return login();
}

async function get(path: string, auth: boolean): Promise<Json> {
  const url = `${apiBase()}/${path.replace(/^\/+/, "")}`;
  for (let attempt = 0; attempt < 2; attempt++) {
    const headers: Record<string, string> = {};
    if (auth) headers.Cookie = `access_token=${await token(attempt > 0)}`;
    const { code, text } = await http(url, { headers });
    if (code === 401 && auth && attempt === 0) continue;
    let v: Json;
    try {
      v = JSON.parse(text);
    } catch {
      if ([502, 503, 504].includes(code)) throw new Error(`sayt vaqtincha ishlamayapti (HTTP ${code}) — birozdan keyin urinib ko'ring`);
      throw new Error(`sayt javobi tushunarsiz (HTTP ${code})`);
    }
    if (code >= 400 || v?.success === false) throw new Error(`sayt: ${v?.message ?? "xato"} (HTTP ${code})`);
    return v;
  }
  throw new Error("saytga kirib bo'lmadi");
}

const dataList = (v: Json): Json[] => (Array.isArray(v?.data) ? v.data : []);
const titleOf = (v: Json): string => {
  const t = String(v?.uz?.title ?? v?.ru?.title ?? "").trim();
  return t || String(v?.slug ?? "?");
};
const jint = (v: Json, k: string): number => {
  const n = Number(v?.[k]);
  return Number.isFinite(n) ? Math.trunc(n) : 0;
};

/** Rasmlar (poster) anibla.uz'da EMAS, ortidagi serverda turadi (sayt JS: `NEXT_PUBLIC_API_BASE_URL`). */
const IMG_BASE = "https://amediatv.up-it.uz";
export function imgUrl(img: string): string {
  const i = (img ?? "").trim();
  if (!i) return "";
  return /^https?:\/\//.test(i) ? i : `${IMG_BASE}/${i.replace(/^\/+/, "")}`;
}

export interface Listing { items: AItem[]; pages: number; total: number }

/** Ro'yxat sahifasi: qidiruv (`q`) yoki bo'lim (`cat`: ID yoki `new`) yoki yil (`y:2026`). Sahifada 40 tadan (inline tugmalar chegarasi: 100 ta). */
export async function listing(q: string, cat: string, page: number): Promise<Listing> {
  let path = `media/mobile?limit=40&page=${page}`;
  if (q) path += `&search=${enc(q)}`;
  else if (cat.startsWith("y:")) path += `&years=${enc(cat.slice(2))}`;
  else if (cat && cat !== "new") path += `&categories=${enc(cat)}`;
  const v = await get(path, false);
  const items: AItem[] = dataList(v)
    .map((x) => ({
      s: String(x.slug ?? ""),
      t: titleOf(x),
      m: String(x.mediaType ?? ""),
      y: jint(x, "published_year"),
      e: jint(x, "available_episodes"),
      img: String(x.thumbnail ?? "").replace(/^\/+/, ""),
    }))
    .filter((i) => i.s);
  const pages = Math.max(1, jint(v?.pagination, "pages"));
  const total = Math.max(jint(v?.pagination, "total"), items.length);
  return { items, pages, total };
}

export interface Category { id: string; name: string }

/** Saytdagi bo'limlar (`GET categories`): Ongoing, Hamma animelar, Anime filmlar, ... — saytdan olinadi. */
export async function categories(): Promise<Category[]> {
  try {
    const v = await get("categories?sortBy=name.uz&sortDirection=-1", false);
    const list: Json[] = Array.isArray(v?.data?.categories) ? v.data.categories : Array.isArray(v?.data) ? v.data : [];
    return list
      .filter((c) => c?.status !== false)
      .map((c) => ({ id: String(c?._id ?? ""), name: String(c?.name?.uz ?? (typeof c?.name === "string" ? c.name : "")).trim() }))
      .filter((c) => c.id && c.name);
  } catch {
    return [];
  }
}

/** Saytdagi animelar yillar bo'yicha: [yil, soni], yangi yil tepada; 24 soat saqlanadi (xatoda eskisi ko'rsatiladi). */
export async function yearCounts(): Promise<{ list: [number, number][]; why: string }> {
  const TTL = 24 * 3600 * 1000;
  const readCache = async (): Promise<{ at: number; l: [number, number][] } | null> => {
    try {
      const raw = await getSetting("anibla_years");
      return raw ? JSON.parse(raw) : null;
    } catch {
      return null;
    }
  };
  const c = await readCache();
  if (c && Date.now() - (c.at ?? 0) < TTL && c.l?.length) return { list: c.l, why: "" };
  let why = "";
  const counts = new Map<number, number>();
  for (let page = 1; page <= 4; page++) {
    let v: Json;
    try {
      v = await get(`media/mobile?limit=500&page=${page}`, false);
    } catch (e) {
      why = e instanceof Error ? e.message : String(e);
      break;
    }
    for (const x of dataList(v)) {
      const y = jint(x, "published_year");
      if (y > 0) counts.set(y, (counts.get(y) ?? 0) + 1);
    }
    if (page >= Math.max(1, jint(v?.pagination, "pages"))) break;
  }
  const list = [...counts.entries()].sort((a, b) => b[0] - a[0]);
  if (list.length) {
    await setSetting("anibla_years", JSON.stringify({ at: Date.now(), l: list })).catch(() => {});
    return { list, why: "" };
  }
  if (c?.l?.length) return { list: c.l, why: "" };
  return { list: [], why: why || "sayt ro'yxati bo'sh qaytdi" };
}

const escH = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
const namesOf = (v: Json): string[] =>
  Array.isArray(v)
    ? v.map((x) => {
        const n = x?.name;
        return String(n?.uz ?? n?.ru ?? (typeof n === "string" ? n : "")).trim();
      }).filter(Boolean)
    : [];
const numOf = (v: Json): number => {
  const n = typeof v === "number" ? v : parseInt(String(v ?? "").trim(), 10);
  return Number.isFinite(n) ? Math.trunc(n) : 0;
};
const DESC_MAX = 2600;

/** Tanlangan anime/film haqida saytdagi TO'LIQ ma'lumot (tayyor HTML): `{info, about}`. */
export async function describe(it: AItem): Promise<{ info: string; about: string }> {
  let v: Json;
  try {
    v = await get(`${isMovie(it) ? "movies" : "series"}/${enc(it.s)}`, false);
  } catch {
    return { info: "", about: "" };
  }
  const d: Json = Array.isArray(v?.data) ? v.data[0] : v?.data;
  const lines: string[] = [];
  const add = (icon: string, key: string, val: string) => {
    if (val.trim()) lines.push(`${icon} <b>${key}:</b> ${escH(val.trim())}`);
  };
  const alt = String(d?.ru?.title ?? "").trim();
  if (alt && alt !== it.t) add("🔤", "Boshqa nomi", alt);
  add("🌍", "Davlat", String(d?.country?.name?.uz ?? d?.country?.name?.ru ?? ""));
  const year = numOf(d?.published_year);
  if (year > 0) add("📅", "Yili", String(year));
  add("🎭", "Janrlar", namesOf(d?.genres).join(", "));
  add("🏢", "Studiya", String(d?.studio?.name ?? ""));
  add("🎬", "Rejissyor", String(d?.director?.name ?? ""));
  add("🎙", "Ovoz berganlar", namesOf(d?.creators).join(", "));
  const age = numOf(d?.age);
  if (age > 0) add("🔞", "Yosh chegarasi", `${age}+`);
  if (isMovie(it)) {
    const dur = numOf(d?.duration);
    if (dur > 0) add("⏱", "Davomiyligi", `${dur} daqiqa`);
  } else {
    const total = numOf(d?.total_episodes);
    let eps = "";
    if (it.e > 0 && total > 0 && it.e !== total) eps = `${it.e} ta chiqqan / jami ${total} ta`;
    else if (it.e > 0) eps = `${it.e} ta`;
    else if (total > 0) eps = `${total} ta`;
    add("📺", "Qismlar", eps);
  }
  if (d?.type === "paid") add("💰", "Turi", "Pullik");
  else if (d?.type === "free") add("🆓", "Turi", "Bepul");
  add("📂", "Bo'limlar", namesOf(d?.categories).filter((n) => !n.toLowerCase().startsWith("hamma")).join(", "));
  let info = lines.join("\n");
  const tr = String(d?.trailer ?? "").trim();
  if (tr.startsWith("http")) {
    const t = tr.replace("youtube.com/embed/", "youtu.be/").replace("www.youtu.be/", "youtu.be/");
    info += `\n▶️ <a href="${escH(t)}">Treyler</a>`;
  }
  let about = "";
  const desc = String(d?.uz?.description ?? "").trim() || String(d?.ru?.description ?? "").trim();
  if (desc) {
    const t = [...desc].slice(0, DESC_MAX).join("") + ([...desc].length > DESC_MAX ? "…" : "");
    about = `📝 <b>Tavsif:</b>\n${escH(t)}`;
  }
  return { info, about };
}

export async function seasons(slug: string): Promise<ASeason[]> {
  const v = await get(`seasons/${enc(slug)}`, false);
  return dataList(v).map((s) => ({ slug: String(s.slug ?? ""), title: titleOf(s) })).filter((s) => s.slug);
}

export const seasonLabel = (s: ASeason): string => (/^\d+$/.test(s.title) ? `${s.title}-fasl` : s.title);

export async function episodes(slug: string, season: string): Promise<AEpisode[]> {
  const v = await get(`episodes/${enc(slug)}/${enc(season)}?view=compact`, false);
  const eps = dataList(v)
    .map((e, i) => ({
      slug: String(e.slug ?? ""),
      num: jint(e, "episode_number") > 0 ? jint(e, "episode_number") : i + 1,
      title: String(e?.uz?.title ?? "").trim(),
    }))
    .filter((e) => e.slug);
  eps.sort((a, b) => a.num - b.num);
  return eps;
}

/** Qism (yoki film) videosi manzili — LOGIN bilan. */
export async function videoOf(it: AItem, season?: string, ep?: AEpisode): Promise<string> {
  let v: Json;
  if (ep && season) {
    try {
      v = await get(`episodes/${enc(it.s)}/${enc(season)}/${enc(ep.slug)}`, true);
    } catch {
      v = await get(`episodes/${enc(ep.slug)}`, true);
    }
  } else {
    v = await get(`movies/${enc(it.s)}`, true);
  }
  const d: Json = Array.isArray(v?.data) ? v.data[0] : v?.data && typeof v.data === "object" ? v.data : v;
  for (const k of ["video", "video_url", "url", "stream"]) {
    const s = typeof d?.[k] === "string" ? d[k].trim() : "";
    if (s) return s;
  }
  return "";
}

function m3uAttr(attrs: string, name: string): string {
  const key = `${name}=`;
  let rest = attrs;
  for (;;) {
    const pos = rest.indexOf(key);
    if (pos < 0) return "";
    const ok = pos === 0 || rest[pos - 1] === ",";
    const after = rest.slice(pos + key.length);
    if (ok) return after.startsWith('"') ? after.slice(1).split('"')[0] : after.split(",")[0].trim();
    rest = after;
  }
}

/** Nisbiy manzilni to'liqqa aylantiradi (m3u8 ichidagi `./720/index.m3u8`). */
function joinUrl(base: string, rel: string): string {
  rel = rel.trim();
  if (/^https?:\/\//.test(rel)) return rel;
  try {
    return new URL(rel, base.split("?")[0]).toString();
  } catch {
    return rel;
  }
}

/** `video` -> HLS master -> sifatlar (balanddan pastga), o'zbekcha ovoz yo'li (bo'lsa) bilan. */
export async function variants(video: string): Promise<AVariant[]> {
  if (!video) throw new Error("bu qismda video yo'q (hali yuklanmagan yoki obuna kerak)");
  const sep = video.includes("?") ? "&" : "?";
  const r1 = await http(`${video}${sep}format=api`);
  let master = "";
  try {
    const f = JSON.parse(r1.text)?.file;
    if (typeof f === "string" && f) master = joinUrl(video, f);
  } catch { /* JSON emas */ }
  if (!master) throw new Error(`video manzili ochilmadi (HTTP ${r1.code})`);
  const r2 = await http(master);
  if (r2.code >= 400 || !r2.text.includes("#EXTM3U")) throw new Error(`playlist ochilmadi (HTTP ${r2.code})`);
  const lines = r2.text.split(/\r?\n/).map((l) => l.trim());

  // Ovoz yo'llari: ba'zi qismlarda o'zbekcha ovoz ALOHIDA playlist (`#EXT-X-MEDIA:TYPE=AUDIO,LANGUAGE="uz"`).
  const groups: { gid: string; uri: string; name: string; rank: number }[] = [];
  for (const l of lines) {
    if (!l.startsWith("#EXT-X-MEDIA:")) continue;
    const a = l.slice("#EXT-X-MEDIA:".length);
    if (m3uAttr(a, "TYPE") !== "AUDIO") continue;
    const uri = m3uAttr(a, "URI");
    if (!uri) continue;
    const lang = m3uAttr(a, "LANGUAGE").toLowerCase();
    const rank = lang.startsWith("uz") ? 3 : m3uAttr(a, "DEFAULT") === "YES" ? 2 : 1;
    groups.push({ gid: m3uAttr(a, "GROUP-ID"), uri: joinUrl(master, uri), name: m3uAttr(a, "NAME"), rank });
  }
  const pickAudio = (gid: string) =>
    groups.filter((g) => g.gid === gid).sort((a, b) => b.rank - a.rank)[0];

  let out: AVariant[] = [];
  lines.forEach((l, k) => {
    if (!l.startsWith("#EXT-X-STREAM-INF:")) return;
    const attrs = l.slice("#EXT-X-STREAM-INF:".length);
    const uri = lines.slice(k + 1).find((x) => x && !x.startsWith("#"));
    if (!uri) return;
    const height = parseInt(m3uAttr(attrs, "RESOLUTION").split("x")[1] ?? "", 10) || 0;
    const bandwidth = parseInt(m3uAttr(attrs, "BANDWIDTH"), 10) || 0;
    const gid = m3uAttr(attrs, "AUDIO");
    const au = gid ? pickAudio(gid) : undefined;
    out.push({ height, bandwidth, url: joinUrl(master, uri), audio: au?.uri ?? "", audioName: au?.name ?? "" });
  });
  if (out.length === 0) out = [{ height: 0, bandwidth: 0, url: master, audio: "", audioName: "" }];
  out.sort((a, b) => b.height - a.height || b.bandwidth - a.bandwidth);
  return out.filter((v, i) => out.findIndex((x) => x.height === v.height) === i);
}

/** Variant playlist'idagi bo'laklar uzunligi yig'indisi (soniya). */
export async function duration(url: string): Promise<number> {
  try {
    const { code, text } = await http(url);
    if (code !== 200) return 0;
    let sum = 0;
    for (const l of text.split(/\r?\n/)) {
      const t = l.trim();
      if (t.startsWith("#EXTINF:")) sum += parseFloat(t.slice(8).split(",")[0]) || 0;
    }
    return sum;
  } catch {
    return 0;
  }
}

export const qualityName = (h: number): string => (h > 0 ? `${h}p` : "asl sifat");

export function hms(s: number): string {
  const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), x = Math.floor(s % 60);
  const p = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${p(m)}:${p(x)}` : `${p(m)}:${p(x)}`;
}

/** Anibla balandligi -> botdagi standart sifat (1080p / 720p / 480p / 360p). */
export type Q = "1080p" | "720p" | "480p" | "360p";
export function bucket(height: number): Q {
  if (height <= 0) return "720p";
  if (height >= 900) return "1080p";
  if (height >= 600) return "720p";
  if (height >= 400) return "480p";
  return "360p";
}

/** Har standart sifat uchun eng katta variant (balanddan pastga tartibda). */
export function pickPerQuality(vs: AVariant[]): { q: Q; v: AVariant }[] {
  const seen = new Set<Q>();
  const out: { q: Q; v: AVariant }[] = [];
  for (const v of vs) {
    const q = bucket(v.height);
    if (seen.has(q)) continue;
    seen.add(q);
    out.push({ q, v });
  }
  return out;
}

// ── GitHub Actions ───────────────────────────────────────────

export function ghReady(): boolean {
  return !!config.GH_ACTIONS_TOKEN.trim() && config.GH_REPO.includes("/");
}

/** Bitta sifat — o'z workflow'i (`workflowOf(sifat)`): har bir sifat alohida `workflow_dispatch`. */
export async function dispatchRun(inputs: Record<string, string>): Promise<void> {
  const r = await fetch(`https://api.github.com/repos/${config.GH_REPO.trim()}/actions/workflows/${workflowOf(inputs.quality)}/dispatches`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${config.GH_ACTIONS_TOKEN.trim()}`,
      Accept: "application/vnd.github+json",
      "X-GitHub-Api-Version": "2022-11-28",
      "User-Agent": "aniraxuzbot",
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ ref: "main", inputs }),
  });
  if (r.status !== 204) {
    const t = await r.text().catch(() => "");
    let m = "";
    try { m = JSON.parse(t)?.message ?? ""; } catch { /* matn */ }
    throw new Error(`Actions ishga tushmadi (GitHub ${r.status}) ${m}`.trim());
  }
}
