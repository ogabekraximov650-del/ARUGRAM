/**
 * Turso (libSQL) — BITTA baza (`aniraxuzbot-arumedia`), HTTP (Hrana) orqali.
 *
 * Oldin: 3 ta Cloudflare D1 (anime / stat / main). Endi hamma jadval bitta bazada. Eski D1 bazalar tegilmagan holda turadi
 * (`wrangler.toml` oxiridagi izohga qarang) — bot ularni ishlatmaydi.
 *
 * Funksiya nomlari (d1Run, d1Update, d1InsertReturningId, d1Batch, d1BatchRead, ensureD1Tables) saqlangan — ularga bog'liq
 * kod o'zgarmadi. Kalitlar: `TURSO_URL` va `TURSO_TOKEN` (Cloudflare secret; manba — `turso.enc`, ENCODE_TOKEN bilan shifrlangan).
 *
 * Jadval tuzilmasi qoidalari (jurnal/statistika jadvallari):
 *  - vaqt — `created_at INTEGER` (UTC unix soniya, `unixepoch()`); matn sanadan ~4 baravar ixcham va indeksga qulay;
 *  - ustun nomlari bir xil: `user_id`, `anime_id`, `seasondb_id`, `episode_id`/`epizod_id`, `created_at`;
 *  - kompozit kalitli jadvallar `WITHOUT ROWID` (kalit — yagona B-daraxt, ortiqcha indeks kerak emas);
 *  - indeks faqat haqiqiy so'rovlar uchun (kalit prefiksi yetarli bo'lsa — alohida indeks yo'q).
 */

import { SQL_TASHKENT_NOW } from "./date_utils.ts";

// ─── Ulanish ──────────────────────────────────────────────────────────────────

export interface D1Env {
  TURSO_URL?: string;
  TURSO_TOKEN?: string;
}

let _url = "";
let _token = "";

/** worker.ts da env dan Turso kalitlari o'rnatiladi. */
export function initD1Bindings(env: D1Env): void {
  if (env.TURSO_URL) _url = String(env.TURSO_URL).trim().replace(/\/+$/, "");
  if (env.TURSO_TOKEN) _token = String(env.TURSO_TOKEN).trim();
}

type HranaValue =
  | { type: "null" }
  | { type: "integer"; value: string }
  | { type: "float"; value: number }
  | { type: "text"; value: string }
  | { type: "blob"; base64: string };

function encodeArg(v: unknown): HranaValue {
  if (v === null || v === undefined) return { type: "null" };
  if (typeof v === "boolean") return { type: "integer", value: v ? "1" : "0" };
  if (typeof v === "bigint") return { type: "integer", value: v.toString() };
  if (typeof v === "number") {
    return Number.isInteger(v) ? { type: "integer", value: String(v) } : { type: "float", value: v };
  }
  return { type: "text", value: String(v) };
}

function decodeValue(v: { type: string; value?: string | number; base64?: string }): unknown {
  switch (v.type) {
    case "null": return null;
    case "integer": return Number(v.value);
    case "float": return Number(v.value);
    case "blob": return v.base64 ?? "";
    default: return v.value ?? "";
  }
}

interface HranaResult {
  cols: { name: string }[];
  rows: { type: string; value?: string | number; base64?: string }[][];
  affected_row_count?: number;
  last_insert_rowid?: string | null;
}

function toObjects<T>(r: HranaResult): T[] {
  const names = r.cols.map((c) => c.name);
  return r.rows.map((row) => {
    const o: Record<string, unknown> = {};
    row.forEach((cell, i) => { o[names[i]] = decodeValue(cell); });
    return o as T;
  });
}

const stmtOf = (sql: string, params: unknown[] = []) => ({ sql, args: params.map(encodeArg) });

/** Bitta HTTP so'rovi (pipeline). Tarmoq/5xx/429 xatolarida 2 marta qayta uriniladi. */
async function pipeline(requests: unknown[]): Promise<any[]> {
  if (!_url || !_token) throw new Error("Turso kalitlari yo'q — TURSO_URL / TURSO_TOKEN (initD1Bindings)");
  const body = JSON.stringify({ requests: [...requests, { type: "close" }] });
  let lastErr: unknown;
  for (let attempt = 0; attempt < 3; attempt++) {
    try {
      const r = await fetch(`${_url}/v2/pipeline`, {
        method: "POST",
        headers: { Authorization: `Bearer ${_token}`, "Content-Type": "application/json" },
        body,
      });
      if (r.status === 429 || r.status >= 500) throw new Error(`Turso HTTP ${r.status}`);
      const j = (await r.json()) as { results?: any[]; error?: string };
      if (!r.ok || !j.results) throw Object.assign(new Error(`Turso xato (${r.status}): ${j.error ?? JSON.stringify(j).slice(0, 200)}`), { fatal: true });
      return j.results;
    } catch (e) {
      lastErr = e;
      if ((e as { fatal?: boolean }).fatal) throw e;
      await new Promise((res) => setTimeout(res, 150 * (attempt + 1)));
    }
  }
  throw lastErr;
}

function failIfError(r: any): void {
  if (r?.type === "error") throw new Error(`Turso SQL xato: ${r.error?.message ?? "?"}`);
}

/** Bitta so'rovni bajaradi. */
async function execute(sql: string, params: unknown[] = []): Promise<HranaResult> {
  const res = await pipeline([{ type: "execute", stmt: stmtOf(sql, params) }]);
  failIfError(res[0]);
  return res[0].response.result as HranaResult;
}

/**
 * Bir nechta so'rov — BITTA HTTP safari. `atomic` bo'lsa tranzaksiya (birortasi xato bersa — hammasi bekor), aks holda
 * ketma-ket mustaqil. Har so'rov natijasi qaytadi.
 */
async function batch(queries: { sql: string; params?: unknown[] }[], atomic: boolean): Promise<HranaResult[]> {
  if (queries.length === 0) return [];
  const steps: any[] = [];
  if (atomic) {
    steps.push({ stmt: { sql: "BEGIN" } });
    queries.forEach((q, i) => {
      steps.push({ stmt: stmtOf(q.sql, q.params), condition: { type: "ok", step: i } });
    });
    const last = queries.length; // oxirgi so'rov bosqichi
    steps.push({ stmt: { sql: "COMMIT" }, condition: { type: "ok", step: last } });
    steps.push({ stmt: { sql: "ROLLBACK" }, condition: { type: "not", cond: { type: "ok", step: last + 1 } } });
  } else {
    queries.forEach((q) => steps.push({ stmt: stmtOf(q.sql, q.params) }));
  }
  const res = await pipeline([{ type: "batch", batch: { steps } }]);
  failIfError(res[0]);
  const { step_results, step_errors } = res[0].response.result as {
    step_results: (HranaResult | null)[];
    step_errors: ({ message: string } | null)[];
  };
  const off = atomic ? 1 : 0;
  for (let i = 0; i < step_errors.length; i++) {
    if (step_errors[i]) throw new Error(`Turso batch xato: ${step_errors[i]!.message}`);
  }
  return queries.map((_, i) => step_results[i + off] as HranaResult);
}

/** "no such table" xatosimi? */
function isMissingTableError(e: unknown): boolean {
  const msg = String(e).toLowerCase();
  return msg.includes("no such table") || msg.includes("no such column");
}

// ─── O'qish keshi (10 daqiqa) ─────────────────────────────────────────────────
// So'rovlar sonini kamaytirish uchun kam o'zgaradigan kontent jadvallaridan o'qish natijalari 10 daqiqagacha keshlanadi.
// Keshlanadigan jadvallar: anime/fasl/qism va kanal/tashqi havola ro'yxatlari. Foydalanuvchi, to'lov, sessiya, navbat (anibla_*),
// statistika va sozlamalar KESHLANMAYDI — ularning to'g'riligi muhim.
//  • Shu jadvallarga yozuv bo'lsa kesh darhol tozalanadi (yuklash hisoblagichi `downloads = downloads + 1` bundan mustasno).
//  • Worker isolate'lari orasida muvofiqlik: kontent yozilganda `bot_settings.content_ver` oshiriladi; har isolate uni
//    ≤ 30 soniyada bir marta tekshiradi va o'zgargan bo'lsa keshni tozalaydi. Ya'ni boshqa isolate'da eskirish ≤ 30 soniya.
const READ_CACHE_TTL_MS = 10 * 60 * 1000;
const READ_CACHE_MAX = 2000;
const VERSION_CHECK_MS = 30 * 1000;
const CACHED_TABLES = new Set(["anime_db", "season_db", "epizod_db", "chanel_info_db", "tashqi_link_db"]);
const _readCache = new Map<string, { rows: unknown[]; expires: number }>();
let _verCheckedAt = 0;
let _ver: string | null = null;

const isWriteSql = (sql: string) => /^\s*(INSERT|UPDATE|DELETE|REPLACE)\b/i.test(sql);
const isCounterOnly = (sql: string) => /^\s*UPDATE\b[^;]*\bdownloads\s*=\s*downloads\s*\+\s*1\b/i.test(sql);

function tablesOf(sql: string, write: boolean): string[] {
  const re = write ? /(?:UPDATE|INTO|FROM)\s+"?(\w+)"?/gi : /(?:FROM|JOIN)\s+"?(\w+)"?/gi;
  return [...sql.matchAll(re)].map((m) => m[1]);
}

/** Barcha jadvallari keshlanadigan SELECT'mi? */
function isCacheableRead(sql: string): boolean {
  if (!/^\s*(SELECT|WITH)\b/i.test(sql)) return false;
  const t = tablesOf(sql, false);
  return t.length > 0 && t.every((x) => CACHED_TABLES.has(x));
}

async function syncContentVersion(): Promise<void> {
  const now = Date.now();
  if (now - _verCheckedAt < VERSION_CHECK_MS) return;
  _verCheckedAt = now;
  try {
    const r = await execute(`SELECT value FROM bot_settings WHERE key = 'content_ver'`);
    const v = String(r.rows[0] ? decodeValue(r.rows[0][0]) : "0");
    if (_ver !== null && v !== _ver) _readCache.clear();
    _ver = v;
  } catch {
    /* jadval hali yo'q yoki xato — kesh qisqa muddatli, davom etamiz */
  }
}

/** Kontent jadvaliga yozuv: mahalliy kesh tozalanadi va boshqa isolate'lar uchun versiya oshiriladi. */
async function onWrite(sql: string): Promise<void> {
  if (isCounterOnly(sql)) return;
  if (!tablesOf(sql, true).some((x) => CACHED_TABLES.has(x))) return;
  _readCache.clear();
  try {
    await execute(
      `INSERT INTO bot_settings (key, value) VALUES ('content_ver', '1')
       ON CONFLICT(key) DO UPDATE SET value = CAST(CAST(value AS INTEGER) + 1 AS TEXT)`
    );
    _verCheckedAt = 0;
  } catch {
    /* versiya yozilmasa ham mahalliy kesh tozalangan; boshqa isolate'larda kesh TTL bilan tugaydi */
  }
}

// ─── Asosiy so'rov funksiyalari ───────────────────────────────────────────────

async function withTables<R>(run: () => Promise<R>): Promise<R> {
  try {
    return await run();
  } catch (e) {
    if (isMissingTableError(e) && !_tablesReady) {
      await ensureD1Tables();
      return await run();
    }
    throw e;
  }
}

/** SELECT (yoki natija qaytaradigan so'rov). Kontent jadvallari bo'lsa 10 daqiqa keshlanadi. */
export async function d1Run<T = Record<string, unknown>>(sql: string, params: unknown[] = []): Promise<T[]> {
  if (isWriteSql(sql)) {
    const rows = toObjects<T>(await withTables(() => execute(sql, params)));
    await onWrite(sql);
    return rows;
  }
  if (!isCacheableRead(sql)) return toObjects<T>(await withTables(() => execute(sql, params)));

  await syncContentVersion();
  const key = sql + "\u0000" + JSON.stringify(params);
  const hit = _readCache.get(key);
  if (hit && hit.expires > Date.now()) return structuredClone(hit.rows) as T[];
  const rows = toObjects<T>(await withTables(() => execute(sql, params)));
  if (_readCache.size >= READ_CACHE_MAX) {
    const first = _readCache.keys().next().value;
    if (first !== undefined) _readCache.delete(first);
  }
  _readCache.set(key, { rows: structuredClone(rows), expires: Date.now() + READ_CACHE_TTL_MS });
  return rows;
}

/** DML (INSERT/UPDATE/DELETE) — o'zgargan qatorlar sonini qaytaradi (lock tekshirish uchun). */
export async function d1Update(sql: string, params: unknown[] = []): Promise<number> {
  const r = await withTables(() => execute(sql, params));
  await onWrite(sql);
  return r.affected_row_count ?? 0;
}

/** INSERT ... RETURNING — yangi qator primary keyini qaytaradi. */
export async function d1InsertReturningId(
  sql: string,
  params: unknown[],
  pkColumn: string = "id"
): Promise<number> {
  const r = await withTables(() => execute(`${sql} RETURNING ${pkColumn}`, params));
  await onWrite(sql);
  const row = toObjects<Record<string, number>>(r)[0];
  return Number(row?.[pkColumn] ?? 0);
}

export interface BatchQuery {
  sql: string;
  params?: unknown[];
}

/** Bir nechta DML so'rov — BITTA HTTP safari va BITTA tranzaksiya (hammasi yoki hech biri). */
export async function d1Batch(queries: BatchQuery[]): Promise<void> {
  if (queries.length === 0) return;
  await withTables(() => batch(queries, true));
  for (const q of queries) await onWrite(q.sql);
}

/** Bir nechta SELECT — BITTA HTTP safari. Natijalar so'rovlar tartibida. */
export async function d1BatchRead<T = Record<string, unknown>>(
  queries: { sql: string; params?: unknown[] }[]
): Promise<T[][]> {
  if (queries.length === 0) return [];
  const res = await withTables(() => batch(queries, false));
  return res.map((r) => toObjects<T>(r));
}

// ─── Jadvallar (sxema) ────────────────────────────────────────────────────────

const SCHEMA_VERSION = "2";
let _tablesReady = false;
let _initPromise: Promise<void> | null = null;

/** Jadvallar bir marta tekshiriladi (isolate boshiga 1 ta kichik so'rov); sxema versiyasi mos bo'lmasa yaratiladi. */
export async function ensureD1Tables(): Promise<void> {
  if (_tablesReady) return;
  if (_initPromise) return _initPromise;
  _initPromise = (async () => {
    try {
      const r = await execute(`SELECT value FROM bot_settings WHERE key = 'schema_ver'`);
      if (r.rows[0] && String(decodeValue(r.rows[0][0])) === SCHEMA_VERSION) return;
    } catch {
      /* jadval yo'q — yaratiladi */
    }
    await batch(SCHEMA.map((sql) => ({ sql })), false);
    await execute(
      `INSERT INTO bot_settings (key, value) VALUES ('schema_ver', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value`,
      [SCHEMA_VERSION]
    );
    console.log("Turso — jadvallar tayyor.");
  })()
    .then(() => { _tablesReady = true; })
    .catch((e) => { _initPromise = null; throw e; });
  return _initPromise;
}

export function resetD1TablesFlag(): void {
  _tablesReady = false;
  _initPromise = null;
}

const TS = `INTEGER NOT NULL DEFAULT (unixepoch())`;

const SCHEMA: string[] = [
  // Eski (birlashtirilgan) jadvallar — bo'sh bazada qolgan bo'lishi mumkin.
  `DROP TABLE IF EXISTS stat_like`,
  `DROP TABLE IF EXISTS stat_dislike`,
  `DROP TABLE IF EXISTS stat_save`,
  `DROP TABLE IF EXISTS stat_share`,
  // ── Kontent: anime → fasl/bo'lim → qism ──────────────────────────────────────
  `CREATE TABLE IF NOT EXISTS anime_db (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    info TEXT NOT NULL,
    chanel_id TEXT NOT NULL,
    file_id TEXT NOT NULL,
    downloads INTEGER NOT NULL DEFAULT 0,
    data TEXT NOT NULL DEFAULT '',
    updated_at TEXT NOT NULL DEFAULT ${SQL_TASHKENT_NOW},
    created_at TEXT NOT NULL DEFAULT ${SQL_TASHKENT_NOW}
  )`,
  `CREATE TABLE IF NOT EXISTS season_db (
    rowid INTEGER PRIMARY KEY AUTOINCREMENT,
    id INTEGER NOT NULL,
    epizoddb_id INTEGER,
    name TEXT NOT NULL,
    info TEXT NOT NULL,
    kategoriya TEXT NOT NULL,
    season_id INTEGER,
    file_id TEXT NOT NULL DEFAULT '',
    downloads INTEGER NOT NULL DEFAULT 0,
    data TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT ${SQL_TASHKENT_NOW}
  )`,
  `CREATE TABLE IF NOT EXISTS epizod_db (
    rowid INTEGER PRIMARY KEY AUTOINCREMENT,
    seasondb_id INTEGER NOT NULL,
    epizoddb_id INTEGER,
    id INTEGER NOT NULL,
    name TEXT NOT NULL,
    epizod_number INTEGER,
    film_id INTEGER,
    ova_id INTEGER,
    epizod_id INTEGER UNIQUE,
    q1080p_file_chanel_id TEXT,
    q1080p_file_bot_id TEXT,
    q720p_file_chanel_id TEXT,
    q720p_file_bot_id TEXT,
    q480p_file_chanel_id TEXT,
    q480p_file_bot_id TEXT,
    q360p_file_chanel_id TEXT,
    q360p_file_bot_id TEXT,
    downloads INTEGER NOT NULL DEFAULT 0,
    data TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT ${SQL_TASHKENT_NOW}
  )`,
  `CREATE INDEX IF NOT EXISTS idx_anime_updated      ON anime_db (updated_at DESC, id DESC)`,
  `CREATE INDEX IF NOT EXISTS idx_anime_downloads    ON anime_db (downloads DESC)`,
  `CREATE INDEX IF NOT EXISTS idx_season_anime       ON season_db (id)`,
  `CREATE INDEX IF NOT EXISTS idx_season_epizoddb    ON season_db (epizoddb_id)`,
  `CREATE INDEX IF NOT EXISTS idx_epizod_season      ON epizod_db (seasondb_id)`,
  `CREATE INDEX IF NOT EXISTS idx_epizod_data        ON epizod_db (data DESC)`,

  // ── Jurnallar / statistika (vaqt — INTEGER unix soniya) ──────────────────────
  // Yuklash: bir foydalanuvchi + bir qism = bitta qator (noyob hisoblagich). So'rovlar: foydalanuvchi bo'yicha (kalit prefiksi),
  // umumiy son va vaqt oynalari (created_at indeksi).
  `CREATE TABLE IF NOT EXISTS stat_downloads (
    user_id INTEGER NOT NULL,
    episode_id INTEGER NOT NULL,
    anime_id INTEGER NOT NULL,
    created_at ${TS},
    PRIMARY KEY (user_id, episode_id)
  ) WITHOUT ROWID`,
  `CREATE INDEX IF NOT EXISTS idx_dl_created ON stat_downloads (created_at)`,
  // Foydalanuvchi + bo'lim bo'yicha BITTA qator: reaksiya (1 yoqdi / -1 yoqmadi / 0), saqlagan, ulashgan. Avval stat_like,
  // stat_dislike, stat_save, stat_share alohida jadval edi — endi har amal bitta UPSERT, statistika bitta so'rov.
  `CREATE TABLE IF NOT EXISTS stat_user_season (
    user_id INTEGER NOT NULL,
    seasondb_id INTEGER NOT NULL,
    anime_id INTEGER NOT NULL,
    reaction INTEGER NOT NULL DEFAULT 0,
    saved INTEGER NOT NULL DEFAULT 0,
    shared INTEGER NOT NULL DEFAULT 0,
    created_at ${TS},
    PRIMARY KEY (user_id, seasondb_id)
  ) WITHOUT ROWID`,
  `CREATE INDEX IF NOT EXISTS idx_us_season ON stat_user_season (seasondb_id)`,
  `CREATE TABLE IF NOT EXISTS stat_coment (
    rowid INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    anime_id INTEGER NOT NULL,
    seasondb_id INTEGER NOT NULL,
    username TEXT NOT NULL DEFAULT '',
    comment TEXT NOT NULL,
    created_at ${TS}
  )`,
  `CREATE INDEX IF NOT EXISTS idx_coment_season ON stat_coment (seasondb_id)`,
  `CREATE INDEX IF NOT EXISTS idx_coment_user   ON stat_coment (user_id, seasondb_id)`,
  // Ko'rish tarixi: bir foydalanuvchi + bir anime = bitta qator (oxirgi ko'rilgan qism). Kalit so'rovlarni qoplaydi.
  `CREATE TABLE IF NOT EXISTS stat_watch_history (
    user_id INTEGER NOT NULL,
    anime_id INTEGER NOT NULL,
    epizod_id INTEGER NOT NULL,
    created_at ${TS},
    PRIMARY KEY (user_id, anime_id)
  ) WITHOUT ROWID`,

  // ── Foydalanuvchilar ─────────────────────────────────────────────────────────
  `CREATE TABLE IF NOT EXISTS user_db (
    user_id INTEGER PRIMARY KEY,
    bot_user_id INTEGER,
    username TEXT NOT NULL DEFAULT '',
    user_phone TEXT NOT NULL DEFAULT '',
    friends_phone TEXT NOT NULL DEFAULT '',
    balance INTEGER NOT NULL DEFAULT 0,
    anime_balance INTEGER NOT NULL DEFAULT 0,
    last_bot_id INTEGER,
    last_bot_data TEXT NOT NULL DEFAULT '',
    downloads INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT ${SQL_TASHKENT_NOW},
    last_active TEXT NOT NULL DEFAULT ${SQL_TASHKENT_NOW}
  )`,
  `CREATE INDEX IF NOT EXISTS idx_user_last_bot    ON user_db (last_bot_id)`,
  `CREATE INDEX IF NOT EXISTS idx_user_bot_user_id ON user_db (bot_user_id)`,
  `CREATE INDEX IF NOT EXISTS idx_user_created     ON user_db (created_at DESC)`,
  `CREATE TABLE IF NOT EXISTS vip_user (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    duration_days INTEGER NOT NULL,
    purchased_at TEXT NOT NULL DEFAULT ${SQL_TASHKENT_NOW},
    expires_at TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL DEFAULT ${SQL_TASHKENT_NOW}
  )`,
  `CREATE INDEX IF NOT EXISTS idx_vip_user   ON vip_user (user_id)`,
  `CREATE INDEX IF NOT EXISTS idx_vip_status ON vip_user (status, expires_at)`,

  // ── Tizim: sessiya, sozlamalar, to'lovlar ────────────────────────────────────
  `CREATE TABLE IF NOT EXISTS sessions (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at ${TS}
  ) WITHOUT ROWID`,
  `CREATE INDEX IF NOT EXISTS idx_sessions_updated ON sessions (updated_at)`,
  `CREATE TABLE IF NOT EXISTS bot_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
  ) WITHOUT ROWID`,
  `CREATE TABLE IF NOT EXISTS bot_payments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    order_id TEXT NOT NULL UNIQUE,
    user_id INTEGER NOT NULL,
    amount INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    pay_url TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL
  )`,
  `CREATE INDEX IF NOT EXISTS idx_pay_user ON bot_payments (user_id, status)`,

  // ── Anibla navbati (vaqt — millisekund; qisqa muddatli ishlar) ───────────────
  `CREATE TABLE IF NOT EXISTS anibla_jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    token TEXT NOT NULL,
    admin_chat INTEGER NOT NULL,
    anime_info_id INTEGER NOT NULL,
    ep_number INTEGER NOT NULL,
    quality TEXT NOT NULL,
    title TEXT NOT NULL DEFAULT '',
    state TEXT NOT NULL DEFAULT 'waiting',
    error TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL
  )`,
  `CREATE INDEX IF NOT EXISTS idx_aj_queue ON anibla_jobs (quality, state, id)`,
  `CREATE INDEX IF NOT EXISTS idx_aj_dup   ON anibla_jobs (anime_info_id, ep_number, quality)`,
  `CREATE TABLE IF NOT EXISTS anibla_queue_data (
    job_id INTEGER PRIMARY KEY,
    video TEXT NOT NULL DEFAULT '',
    name TEXT NOT NULL DEFAULT '',
    bot TEXT NOT NULL DEFAULT '',
    started_at INTEGER NOT NULL DEFAULT 0
  )`,
  `CREATE TABLE IF NOT EXISTS anibla_progress (
    job_id INTEGER PRIMARY KEY,
    text TEXT NOT NULL DEFAULT '',
    updated_at INTEGER NOT NULL
  )`,
  `CREATE TABLE IF NOT EXISTS anibla_part_files (
    job_id INTEGER NOT NULL,
    idx INTEGER NOT NULL,
    total INTEGER NOT NULL,
    bot_id TEXT NOT NULL DEFAULT '',
    chanel_id TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (job_id, idx)
  ) WITHOUT ROWID`,
  `CREATE TABLE IF NOT EXISTS player_parts (
    chat_id INTEGER NOT NULL,
    msg_id INTEGER NOT NULL,
    parts TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL,
    PRIMARY KEY (chat_id, msg_id)
  ) WITHOUT ROWID`,

  // ── Post navbati (vaqt — unix soniya) ────────────────────────────────────────
  `CREATE TABLE IF NOT EXISTS post_queue (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    batch_id TEXT NOT NULL,
    admin_chat_id INTEGER NOT NULL,
    anime_info_id INTEGER NOT NULL,
    episode_rowid INTEGER NOT NULL,
    channel_id TEXT NOT NULL,
    bot_username TEXT NOT NULL DEFAULT '',
    channel_username TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'pending',
    processing_started_at INTEGER,
    posted_at INTEGER,
    created_at ${TS}
  )`,
  `CREATE INDEX IF NOT EXISTS idx_pq_status ON post_queue (status, id)`,
  `CREATE INDEX IF NOT EXISTS idx_pq_batch  ON post_queue (batch_id)`,

  // ── Kanallar, havolalar, konstruktor ─────────────────────────────────────────
  `CREATE TABLE IF NOT EXISTS chanel_info_db (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    bot_id INTEGER NOT NULL,
    chanel_type TEXT NOT NULL,
    chanel_id TEXT NOT NULL,
    chanel_username TEXT NOT NULL DEFAULT '',
    chanel_link TEXT NOT NULL DEFAULT '',
    chanel_title TEXT NOT NULL DEFAULT '',
    jami_qoshish_kerak INTEGER NOT NULL DEFAULT 0,
    jami_qoshilgan INTEGER NOT NULL DEFAULT 0,
    data TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT ${SQL_TASHKENT_NOW}
  )`,
  `CREATE UNIQUE INDEX IF NOT EXISTS idx_chanel_bot_chanel ON chanel_info_db (bot_id, chanel_id)`,
  `CREATE TABLE IF NOT EXISTS request_chanel_db (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    bot_id INTEGER NOT NULL,
    chanel_id TEXT NOT NULL,
    request_user_id INTEGER NOT NULL,
    chanel_link TEXT NOT NULL DEFAULT '',
    data TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT ${SQL_TASHKENT_NOW}
  )`,
  `CREATE UNIQUE INDEX IF NOT EXISTS idx_req_unique ON request_chanel_db (bot_id, chanel_id, request_user_id)`,
  `CREATE TABLE IF NOT EXISTS tashqi_link_db (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    bot_id INTEGER NOT NULL,
    link_name TEXT NOT NULL,
    link TEXT NOT NULL,
    data TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT ${SQL_TASHKENT_NOW}
  )`,
  `CREATE INDEX IF NOT EXISTS idx_link_bot ON tashqi_link_db (bot_id)`,
  `CREATE TABLE IF NOT EXISTS kanstruktor_info_db (
    user_id INTEGER PRIMARY KEY,
    user_phone TEXT NOT NULL DEFAULT '',
    user_name TEXT NOT NULL DEFAULT '',
    bot_id INTEGER,
    bot_token TEXT,
    create_data TEXT NOT NULL DEFAULT ${SQL_TASHKENT_NOW},
    user_status TEXT NOT NULL DEFAULT '',
    sorov_status TEXT NOT NULL DEFAULT '',
    admin_1 TEXT,
    admin_2 TEXT,
    admin_3 TEXT,
    admin_4 TEXT,
    admin_5 TEXT
  )`,
  `CREATE INDEX IF NOT EXISTS idx_kanstruktor_bot ON kanstruktor_info_db (bot_id)`,
];

// ─── Statistika (admin "🗄 Database") ─────────────────────────────────────────

export interface D1DbStats {
  name: string;
  tables: { table: string; rows: number }[];
  totalRows: number;
}

/** Jadvallar va qatorlar soni. */
export async function getD1Stats(): Promise<D1DbStats> {
  try {
    const t = await execute(
      `SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '_litestream%' ORDER BY name`
    );
    const names = toObjects<{ name: string }>(t).map((r) => r.name);
    if (names.length === 0) return { name: "turso", tables: [], totalRows: 0 };
    const res = await batch(names.map((n) => ({ sql: `SELECT COUNT(*) AS cnt FROM "${n}"` })), false);
    const tables = names.map((n, i) => ({ table: n, rows: Number(toObjects<{ cnt: number }>(res[i])[0]?.cnt ?? 0) }));
    return { name: "turso", tables, totalRows: tables.reduce((s, x) => s + x.rows, 0) };
  } catch (e) {
    console.error("getD1Stats xato:", e);
    return { name: "turso", tables: [], totalRows: 0 };
  }
}

export async function pingD1(): Promise<boolean> {
  try {
    await execute("SELECT 1");
    return true;
  } catch {
    return false;
  }
}

/** Barcha jadvallardagi ma'lumotlarni eksport qiladi. */
export async function getAllD1TablesData(): Promise<Record<string, Record<string, unknown>[]>> {
  const stats = await getD1Stats();
  const names = stats.tables.map((t) => t.table);
  const result: Record<string, Record<string, unknown>[]> = {};
  if (names.length === 0) return result;
  const res = await batch(names.map((n) => ({ sql: `SELECT * FROM "${n}"` })), false);
  names.forEach((n, i) => { result[n] = toObjects<Record<string, unknown>>(res[i]); });
  return result;
}
