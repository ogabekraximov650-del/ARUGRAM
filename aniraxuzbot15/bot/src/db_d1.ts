/**
 * Cloudflare D1 — uch bazali arxitektura.
 *
 * DB_ANIME : anime_db, season_db, epizod_db
 * DB_STAT  : stat_downloads, stat_like, stat_dislike, stat_save, stat_coment, stat_share
 * DB_MAIN  : user_db, sessions, bot_settings, post_queue,
 *            chanel_info_db, request_chanel_db, tashqi_link_db, kanstruktor_info_db
 *
 * Har bir so'rov SQL dagi jadval nomiga qarab avtomatik yo'naltiriladi.
 * Jadvallar mavjud bo'lmasa — avtomatik qayta yaratiladi (no-op IF NOT EXISTS).
 */

import { SQL_TASHKENT_NOW } from "./date_utils.ts";

// ─── D1Database minimal type (CF Workers global) ─────────────────────────────
interface D1PreparedStatement {
  bind(...values: unknown[]): D1PreparedStatement;
  all<T = Record<string, unknown>>(): Promise<{ results: T[]; success: boolean; error?: string; meta: { changes: number; last_row_id: number } }>;
  run(): Promise<{ success: boolean; error?: string; meta: { changes: number; last_row_id: number } }>;
}
interface D1Database {
  prepare(query: string): D1PreparedStatement;
  batch(statements: D1PreparedStatement[]): Promise<Array<{ results: unknown[]; success: boolean; error?: string; meta: { changes: number; last_row_id: number } }>>;
}

// ─── Binding holatlari ────────────────────────────────────────────────────────
let _dbAnime: D1Database | null = null;
let _dbStat: D1Database | null = null;
let _dbMain: D1Database | null = null;

// Jadvallar bir marta yaratildi
let _tablesReady = false;
let _initPromise: Promise<void> | null = null;

export interface D1Env {
  DB_ANIME?: D1Database;
  DB_STAT?: D1Database;
  DB_MAIN?: D1Database;
}

/** worker.ts da env dan D1 bindinglari o'rnatiladi. */
export function initD1Bindings(env: D1Env): void {
  if (env.DB_ANIME) _dbAnime = env.DB_ANIME;
  if (env.DB_STAT)  _dbStat  = env.DB_STAT;
  if (env.DB_MAIN)  _dbMain  = env.DB_MAIN;
}

// ─── Jadval → DB xaritasi ─────────────────────────────────────────────────────
const ANIME_TABLES = new Set([
  "anime_db", "season_db", "epizod_db",
]);
const STAT_TABLES = new Set([
  "stat_downloads", "stat_like", "stat_dislike",
  "stat_save", "stat_coment", "stat_share",
  "stat_watch_history",
]);

type DbKey = "ANIME" | "STAT" | "MAIN";

function detectDbKey(sql: string): DbKey {
  // FROM, INTO, UPDATE, JOIN, TABLE, DELETE FROM — birinchi jadval nomi
  const m = sql.match(/(?:FROM|INTO|UPDATE|JOIN|TABLE)\s+"?(\w+)"?/i);
  const table = m?.[1] ?? "";
  if (ANIME_TABLES.has(table)) return "ANIME";
  if (STAT_TABLES.has(table))  return "STAT";
  return "MAIN";
}

function getDb(key: DbKey): D1Database {
  switch (key) {
    case "ANIME": return _dbAnime ?? (() => { throw new Error("DB_ANIME bind qilinmagan"); })();
    case "STAT":  return _dbStat  ?? (() => { throw new Error("DB_STAT bind qilinmagan"); })();
    case "MAIN":  return _dbMain  ?? (() => { throw new Error("DB_MAIN bind qilinmagan"); })();
  }
}

function getDbForSql(sql: string): D1Database {
  return getDb(detectDbKey(sql));
}

/** "no such table" xatosimi? */
function isMissingTableError(e: unknown): boolean {
  const msg = String(e).toLowerCase();
  return (
    msg.includes("no such table") ||
    msg.includes("table not found") ||
    msg.includes("does not exist")
  );
}

/** D1 natijasini tekshiradi va xato bo'lsa throw qiladi */
function checkError<T extends { success: boolean; error?: string }>(r: T): T {
  if (!r.success && r.error) throw new Error(`D1 xato: ${r.error}`);
  return r;
}

// ─── Asosiy so'rov funksiyalari ───────────────────────────────────────────────

/**
 * SELECT so'rovi — jadval nomiga qarab avtomatik DB tanlanadi.
 * "no such table" → jadvallar qayta yaratiladi va bir marta qayta uriniladi.
 */
export async function d1Run<T = Record<string, unknown>>(
  sql: string,
  params: unknown[] = []
): Promise<T[]> {
  const db = getDbForSql(sql);
  const run = async () => {
    const stmt = params.length > 0 ? db.prepare(sql).bind(...params) : db.prepare(sql);
    const r = await stmt.all<T>();
    checkError(r);
    return r.results;
  };

  try {
    return await run();
  } catch (e) {
    if (isMissingTableError(e)) {
      console.warn(`d1Run: "no such table" — jadvallar qayta yaratilmoqda (${sql.slice(0, 50)})`);
      await ensureD1Tables();
      return await run();
    }
    throw e;
  }
}

/**
 * DML (INSERT/UPDATE/DELETE) — rowsAffected qaytaradi.
 * Lock tekshirish uchun: agar rowsAffected=0 bo'lsa, boshqa worker olgan.
 */
export async function d1Update(sql: string, params: unknown[] = []): Promise<number> {
  const db = getDbForSql(sql);
  const run = async () => {
    const stmt = params.length > 0 ? db.prepare(sql).bind(...params) : db.prepare(sql);
    const r = await stmt.run();
    checkError(r);
    return r.meta.changes ?? 0;
  };

  try {
    return await run();
  } catch (e) {
    if (isMissingTableError(e)) {
      await ensureD1Tables();
      return await run();
    }
    throw e;
  }
}

/**
 * INSERT RETURNING rowid/id — yangi qator primary keyini qaytaradi.
 */
export async function d1InsertReturningId(
  sql: string,
  params: unknown[],
  pkColumn: string = "id"
): Promise<number> {
  const db = getDbForSql(sql);
  const run = async () => {
    const fullSql = `${sql} RETURNING ${pkColumn}`;
    const stmt = params.length > 0 ? db.prepare(fullSql).bind(...params) : db.prepare(fullSql);
    const r = await stmt.all<Record<string, number>>();
    checkError(r);
    return Number(r.results[0]?.[pkColumn] ?? 0);
  };

  try {
    return await run();
  } catch (e) {
    if (isMissingTableError(e)) {
      await ensureD1Tables();
      return await run();
    }
    throw e;
  }
}

export interface BatchQuery {
  sql: string;
  params?: unknown[];
}

/**
 * Bir nechta DML so'rovni BITTA network safari bilan yuboradi.
 * Agar so'rovlar turli bazalarga tegishli bo'lsa — guruhlab parallel yuboriladi.
 */
export async function d1Batch(queries: BatchQuery[]): Promise<void> {
  if (queries.length === 0) return;

  // Bazalar bo'yicha guruhlash
  const groups = new Map<DbKey, BatchQuery[]>();
  for (const q of queries) {
    const key = detectDbKey(q.sql);
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key)!.push(q);
  }

  // Har guruhni parallel yuborish
  await Promise.all(
    [...groups.entries()].map(async ([key, qs]) => {
      const db = getDb(key);
      const run = async () => {
        const stmts = qs.map(q =>
          q.params && q.params.length > 0
            ? db.prepare(q.sql).bind(...q.params)
            : db.prepare(q.sql)
        );
        const results = await db.batch(stmts);
        for (const r of results) {
          if (!r.success && r.error) throw new Error(`D1 batch xato: ${r.error}`);
        }
      };
      try {
        await run();
      } catch (e) {
        if (isMissingTableError(e)) {
          await ensureD1Tables();
          await run();
        } else {
          throw e;
        }
      }
    })
  );
}

/**
 * Bir nechta SELECT so'rovini BITTA batch bilan yuboradi.
 * MUHIM: barcha so'rovlar BIR XIL bazaga tegishli bo'lishi kerak!
 * Aks holda har biri alohida d1Run orqali chaqiriladi.
 */
export async function d1BatchRead<T = Record<string, unknown>>(
  queries: { sql: string; params?: unknown[] }[]
): Promise<T[][]> {
  if (queries.length === 0) return [];

  // Bir xil bazaga tegishli so'rovlar guruhlanadi
  const firstKey = detectDbKey(queries[0].sql);
  const allSameDb = queries.every(q => detectDbKey(q.sql) === firstKey);

  if (allSameDb) {
    const db = getDb(firstKey);
    const run = async () => {
      const stmts = queries.map(q =>
        q.params && q.params.length > 0
          ? db.prepare(q.sql).bind(...q.params)
          : db.prepare(q.sql)
      );
      const results = await db.batch(stmts);
      return results.map(r => {
        if (!r.success && (r as any).error) throw new Error(`D1 batchRead xato: ${(r as any).error}`);
        return r.results as T[];
      });
    };
    try {
      return await run();
    } catch (e) {
      if (isMissingTableError(e)) {
        await ensureD1Tables();
        return await run();
      }
      throw e;
    }
  }

  // Turli bazalar — parallel alohida so'rovlar
  return Promise.all(queries.map(q => d1Run<T>(q.sql, q.params)));
}

// ─── Jadval yaratish (schema) ─────────────────────────────────────────────────

/** Jadvallar mavjud bo'lmasa yaratadi — faqat birinchi marta (yoki xatoda). */
export async function ensureD1Tables(): Promise<void> {
  if (_tablesReady) return;
  if (_initPromise) return _initPromise;
  _initPromise = _initAllD1Tables()
    .then(() => { _tablesReady = true; })
    .catch(e => { _initPromise = null; throw e; });
  return _initPromise;
}

export function resetD1TablesFlag(): void {
  _tablesReady = false;
  _initPromise = null;
}

async function batchDdl(db: D1Database, sqls: string[]): Promise<void> {
  // D1 batch faqat prepared statement qabul qiladi
  const stmts = sqls.map(sql => db.prepare(sql));
  const results = await db.batch(stmts);
  for (const r of results) {
    if (!r.success && (r as any).error) {
      const err = String((r as any).error);
      // "already exists" — OK
      if (!err.includes("already exists") && !err.includes("duplicate")) {
        console.warn("DDL warning:", err);
      }
    }
  }
}

async function _initAllD1Tables(): Promise<void> {
  if (!_dbAnime || !_dbStat || !_dbMain) {
    throw new Error("D1 bindings hali o'rnatilmagan — initD1Bindings(env) chaqiring");
  }

  await Promise.all([
    _initAnimeTables(_dbAnime),
    _initStatTables(_dbStat),
    _initMainTables(_dbMain),
  ]);
  console.log("D1 — barcha 3 ta baza jadvallar tayyor.");
}

async function _initAnimeTables(db: D1Database): Promise<void> {
  const ddl = [
    `CREATE TABLE IF NOT EXISTS anime_db (
      id INTEGER PRIMARY KEY,
      name TEXT NOT NULL,
      info TEXT NOT NULL,
      chanel_id TEXT NOT NULL,
      file_id TEXT NOT NULL,
      downloads INTEGER NOT NULL DEFAULT 0,
      data TEXT NOT NULL DEFAULT '',
      updated_at TEXT NOT NULL DEFAULT (${SQL_TASHKENT_NOW}),
      created_at TEXT NOT NULL DEFAULT (${SQL_TASHKENT_NOW})
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
      created_at TEXT NOT NULL DEFAULT (${SQL_TASHKENT_NOW})
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
      created_at TEXT NOT NULL DEFAULT (${SQL_TASHKENT_NOW})
    )`,
    `CREATE INDEX IF NOT EXISTS idx_anime_updated     ON anime_db (updated_at DESC, id DESC)`,
    `CREATE INDEX IF NOT EXISTS idx_anime_downloads   ON anime_db (downloads DESC)`,
    `CREATE INDEX IF NOT EXISTS idx_seasondb_id       ON season_db (id)`,
    `CREATE INDEX IF NOT EXISTS idx_anime_info_epizoddb ON season_db (epizoddb_id)`,
    `CREATE INDEX IF NOT EXISTS idx_epizod_anime_info ON epizod_db (seasondb_id)`,
    `CREATE INDEX IF NOT EXISTS idx_epizod_data       ON epizod_db (data DESC)`,
  ];
  await batchDdl(db, ddl);
}

async function _initStatTables(db: D1Database): Promise<void> {
  const ddl = [
    `CREATE TABLE IF NOT EXISTS stat_downloads (
      user_id INTEGER NOT NULL,
      anime_id INTEGER NOT NULL,
      episode_id INTEGER NOT NULL,
      downloaded_at TEXT NOT NULL DEFAULT (${SQL_TASHKENT_NOW}),
      PRIMARY KEY (user_id, episode_id)
    )`,
    `CREATE TABLE IF NOT EXISTS stat_like (
      user_id INTEGER NOT NULL,
      anime_id INTEGER NOT NULL,
      seasondb_id INTEGER NOT NULL,
      created_at TEXT NOT NULL DEFAULT (${SQL_TASHKENT_NOW}),
      PRIMARY KEY (user_id, seasondb_id)
    )`,
    `CREATE TABLE IF NOT EXISTS stat_dislike (
      user_id INTEGER NOT NULL,
      anime_id INTEGER NOT NULL,
      seasondb_id INTEGER NOT NULL,
      created_at TEXT NOT NULL DEFAULT (${SQL_TASHKENT_NOW}),
      PRIMARY KEY (user_id, seasondb_id)
    )`,
    `CREATE TABLE IF NOT EXISTS stat_save (
      user_id INTEGER NOT NULL,
      anime_id INTEGER NOT NULL,
      seasondb_id INTEGER NOT NULL,
      created_at TEXT NOT NULL DEFAULT (${SQL_TASHKENT_NOW}),
      PRIMARY KEY (user_id, seasondb_id)
    )`,
    `CREATE TABLE IF NOT EXISTS stat_coment (
      rowid INTEGER PRIMARY KEY AUTOINCREMENT,
      user_id INTEGER NOT NULL,
      username TEXT NOT NULL DEFAULT '',
      anime_id INTEGER NOT NULL,
      seasondb_id INTEGER NOT NULL,
      comment TEXT NOT NULL,
      data TEXT NOT NULL DEFAULT (${SQL_TASHKENT_NOW})
    )`,
    `CREATE TABLE IF NOT EXISTS stat_share (
      rowid INTEGER PRIMARY KEY AUTOINCREMENT,
      user_id INTEGER NOT NULL,
      anime_id INTEGER NOT NULL,
      seasondb_id INTEGER NOT NULL,
      epizod_id INTEGER NOT NULL,
      data TEXT NOT NULL DEFAULT (${SQL_TASHKENT_NOW})
    )`,
    `CREATE INDEX IF NOT EXISTS idx_dl_episode        ON stat_downloads (episode_id)`,
    `CREATE INDEX IF NOT EXISTS idx_dl_anime          ON stat_downloads (anime_id)`,
    `CREATE INDEX IF NOT EXISTS idx_dl_user           ON stat_downloads (user_id)`,
    `CREATE INDEX IF NOT EXISTS idx_like_animeinfo    ON stat_like (seasondb_id)`,
    `CREATE INDEX IF NOT EXISTS idx_like_anime        ON stat_like (anime_id)`,
    `CREATE INDEX IF NOT EXISTS idx_dislike_animeinfo ON stat_dislike (seasondb_id)`,
    `CREATE INDEX IF NOT EXISTS idx_dislike_anime     ON stat_dislike (anime_id)`,
    `CREATE INDEX IF NOT EXISTS idx_save_animeinfo    ON stat_save (seasondb_id)`,
    `CREATE INDEX IF NOT EXISTS idx_save_user         ON stat_save (user_id)`,
    `CREATE INDEX IF NOT EXISTS idx_coment_seasondb   ON stat_coment (seasondb_id)`,
    `CREATE INDEX IF NOT EXISTS idx_coment_user       ON stat_coment (user_id)`,
    `CREATE INDEX IF NOT EXISTS idx_coment_anime      ON stat_coment (anime_id)`,
    `CREATE INDEX IF NOT EXISTS idx_share_seasondb    ON stat_share (seasondb_id)`,
    `CREATE INDEX IF NOT EXISTS idx_share_user        ON stat_share (user_id)`,
    `CREATE INDEX IF NOT EXISTS idx_share_epizod      ON stat_share (epizod_id)`,
    `CREATE INDEX IF NOT EXISTS idx_share_anime       ON stat_share (anime_id)`,
    // Ko'rish tarixi — foydalanuvchi qaysi anime, qaysi qismda qolganini saqlaydi
    `CREATE TABLE IF NOT EXISTS stat_watch_history (
      user_id  INTEGER NOT NULL,
      anime_id INTEGER NOT NULL,
      epizod_id INTEGER NOT NULL,
      create_at TEXT NOT NULL DEFAULT (${SQL_TASHKENT_NOW}),
      PRIMARY KEY (user_id, anime_id)
    )`,
    `CREATE INDEX IF NOT EXISTS idx_watch_history_user  ON stat_watch_history (user_id)`,
    `CREATE INDEX IF NOT EXISTS idx_watch_history_anime ON stat_watch_history (anime_id)`,
  ];
  await batchDdl(db, ddl);
}

async function _initMainTables(db: D1Database): Promise<void> {
  const ddl = [
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
      created_at TEXT NOT NULL DEFAULT (${SQL_TASHKENT_NOW}),
      last_active TEXT NOT NULL DEFAULT (${SQL_TASHKENT_NOW})
    )`,
    `CREATE TABLE IF NOT EXISTS vip_user (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      user_id INTEGER NOT NULL,
      duration_days INTEGER NOT NULL,
      purchased_at TEXT NOT NULL DEFAULT (${SQL_TASHKENT_NOW}),
      expires_at TEXT NOT NULL,
      status TEXT NOT NULL DEFAULT 'active',
      created_at TEXT NOT NULL DEFAULT (${SQL_TASHKENT_NOW})
    )`,
    `CREATE TABLE IF NOT EXISTS sessions (
      key TEXT PRIMARY KEY,
      value TEXT NOT NULL,
      updated_at TEXT NOT NULL DEFAULT (${SQL_TASHKENT_NOW})
    )`,
    `CREATE TABLE IF NOT EXISTS bot_settings (
      key TEXT PRIMARY KEY,
      value TEXT NOT NULL
    )`,
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
    `CREATE TABLE IF NOT EXISTS anibla_jobs (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      token TEXT NOT NULL,
      admin_chat INTEGER NOT NULL,
      anime_info_id INTEGER NOT NULL,
      ep_number INTEGER NOT NULL,
      quality TEXT NOT NULL,
      title TEXT NOT NULL DEFAULT '',
      state TEXT NOT NULL DEFAULT 'queued',
      error TEXT NOT NULL DEFAULT '',
      created_at INTEGER NOT NULL
    )`,
    `CREATE TABLE IF NOT EXISTS anibla_queue_data (
      job_id INTEGER PRIMARY KEY,
      video TEXT NOT NULL DEFAULT '',
      name TEXT NOT NULL DEFAULT '',
      bot TEXT NOT NULL DEFAULT '',
      started_at INTEGER NOT NULL DEFAULT 0
    )`,
    `CREATE TABLE IF NOT EXISTS anibla_parts (
      job_id INTEGER NOT NULL,
      idx INTEGER NOT NULL,
      total INTEGER NOT NULL,
      file_id TEXT NOT NULL,
      PRIMARY KEY (job_id, idx)
    )`,
    `CREATE TABLE IF NOT EXISTS anibla_progress (
      job_id INTEGER PRIMARY KEY,
      text TEXT NOT NULL DEFAULT '',
      updated_at INTEGER NOT NULL
    )`,
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
      processing_started_at TEXT,
      posted_at TEXT,
      created_at TEXT NOT NULL DEFAULT (${SQL_TASHKENT_NOW})
    )`,
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
      created_at TEXT NOT NULL DEFAULT (${SQL_TASHKENT_NOW})
    )`,
    `CREATE TABLE IF NOT EXISTS request_chanel_db (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      bot_id INTEGER NOT NULL,
      chanel_id TEXT NOT NULL,
      request_user_id INTEGER NOT NULL,
      chanel_link TEXT NOT NULL DEFAULT '',
      data TEXT NOT NULL DEFAULT '',
      created_at TEXT NOT NULL DEFAULT (${SQL_TASHKENT_NOW})
    )`,
    `CREATE TABLE IF NOT EXISTS tashqi_link_db (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      user_id INTEGER NOT NULL,
      bot_id INTEGER NOT NULL,
      link_name TEXT NOT NULL,
      link TEXT NOT NULL,
      data TEXT NOT NULL DEFAULT '',
      created_at TEXT NOT NULL DEFAULT (${SQL_TASHKENT_NOW})
    )`,
    `CREATE TABLE IF NOT EXISTS kanstruktor_info_db (
      user_id INTEGER PRIMARY KEY,
      user_phone TEXT NOT NULL DEFAULT '',
      user_name TEXT NOT NULL DEFAULT '',
      bot_id INTEGER,
      bot_token TEXT,
      create_data TEXT NOT NULL DEFAULT (${SQL_TASHKENT_NOW}),
      user_status TEXT NOT NULL DEFAULT '',
      sorov_status TEXT NOT NULL DEFAULT '',
      admin_1 TEXT,
      admin_2 TEXT,
      admin_3 TEXT,
      admin_4 TEXT,
      admin_5 TEXT
    )`,
    // Indexlar
    `CREATE INDEX IF NOT EXISTS idx_vip_user_id ON vip_user (user_id)`,
    `CREATE INDEX IF NOT EXISTS idx_vip_status ON vip_user (status, expires_at)`,
    `CREATE UNIQUE INDEX IF NOT EXISTS idx_chanel_bot_chanel ON chanel_info_db (bot_id, chanel_id)`,
    `CREATE UNIQUE INDEX IF NOT EXISTS idx_req_unique ON request_chanel_db (bot_id, chanel_id, request_user_id)`,
    `CREATE INDEX IF NOT EXISTS idx_link_bot ON tashqi_link_db (bot_id)`,
    `CREATE INDEX IF NOT EXISTS idx_pq_status ON post_queue (status, id ASC)`,
    `CREATE INDEX IF NOT EXISTS idx_pq_batch ON post_queue (batch_id)`,
    `CREATE INDEX IF NOT EXISTS idx_sessions_updated ON sessions (updated_at)`,
    `CREATE INDEX IF NOT EXISTS idx_user_last_bot ON user_db (last_bot_id)`,
    `CREATE INDEX IF NOT EXISTS idx_user_bot_user_id ON user_db (bot_user_id)`,
    `CREATE INDEX IF NOT EXISTS idx_user_created ON user_db (created_at DESC)`,
    `CREATE INDEX IF NOT EXISTS idx_kanstruktor_bot_id ON kanstruktor_info_db (bot_id)`,
  ];
  await batchDdl(db, ddl);

  // ALTER TABLE — mavjud bazada yangi ustunlar (xato bo'lsa — allaqachon bor)
  for (const sql of [
    `ALTER TABLE post_queue ADD COLUMN processing_started_at TEXT`,
    `ALTER TABLE post_queue ADD COLUMN posted_at TEXT`,
    `ALTER TABLE user_db DROP COLUMN creat_at`,
    `ALTER TABLE user_db ADD COLUMN last_active TEXT NOT NULL DEFAULT '1970-01-01 00:00:00'`,
  ]) {
    await db.prepare(sql).run().catch(() => {});
  }
}

// ─── D1 statistika ───────────────────────────────────────────────────────────

export interface D1DbStats {
  name: string;
  tables: { table: string; rows: number }[];
  totalRows: number;
}

/** Har uch bazaning jadval statistikasi — bitta chaqiruv. */
export async function getD1Stats(): Promise<{
  anime: D1DbStats;
  stat: D1DbStats;
  main: D1DbStats;
}> {
  const [anime, stat, main] = await Promise.all([
    _getDbStats("anime", _dbAnime!),
    _getDbStats("stat",  _dbStat!),
    _getDbStats("main",  _dbMain!),
  ]);
  return { anime, stat, main };
}

async function _getDbStats(name: string, db: D1Database): Promise<D1DbStats> {
  if (!db) return { name, tables: [], totalRows: 0 };
  try {
    // Jadval ro'yxati
    const tablesRes = await db.prepare(
      `SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '_cf_%' ORDER BY name`
    ).all<{ name: string }>();
    const tableNames = tablesRes.results.map(r => r.name);
    if (tableNames.length === 0) return { name, tables: [], totalRows: 0 };

    // Har jadval uchun COUNT — bitta batch
    const countStmts = tableNames.map(t => db.prepare(`SELECT COUNT(*) as cnt FROM "${t}"`));
    const countResults = await db.batch(countStmts);

    const tables = tableNames.map((t, i) => ({
      table: t,
      rows: Number((countResults[i].results as any[])[0]?.cnt ?? 0),
    }));
    const totalRows = tables.reduce((s, t) => s + t.rows, 0);
    return { name, tables, totalRows };
  } catch (e) {
    console.error(`_getDbStats(${name}) xato:`, e);
    return { name, tables: [], totalRows: 0 };
  }
}

/** D1 ga ulanishni tekshiradi */
export async function pingD1(): Promise<{ anime: boolean; stat: boolean; main: boolean }> {
  const check = async (db: D1Database | null) => {
    if (!db) return false;
    try {
      await db.prepare("SELECT 1").all();
      return true;
    } catch { return false; }
  };
  const [anime, stat, main] = await Promise.all([
    check(_dbAnime), check(_dbStat), check(_dbMain),
  ]);
  return { anime, stat, main };
}

/** Barcha jadvallardagi ma'lumotlarni eksport qiladi */
export async function getAllD1TablesData(): Promise<Record<string, Record<string, unknown>[]>> {
  const [animeStats, statStats, mainStats] = await Promise.all([
    _getDbStats("anime", _dbAnime!),
    _getDbStats("stat",  _dbStat!),
    _getDbStats("main",  _dbMain!),
  ]);

  const result: Record<string, Record<string, unknown>[]> = {};

  const fetchDb = async (db: D1Database, tableNames: string[]) => {
    if (tableNames.length === 0) return;
    const stmts = tableNames.map(t => db.prepare(`SELECT * FROM "${t}"`));
    const results = await db.batch(stmts);
    tableNames.forEach((t, i) => {
      result[t] = results[i].results as Record<string, unknown>[];
    });
  };

  await Promise.all([
    fetchDb(_dbAnime!, animeStats.tables.map(t => t.table)),
    fetchDb(_dbStat!,  statStats.tables.map(t => t.table)),
    fetchDb(_dbMain!,  mainStats.tables.map(t => t.table)),
  ]);

  return result;
}
