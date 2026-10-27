/**
 * Ma'lumotlar bazasi qatlami — 3 ta Cloudflare D1 baza.
 *
 * DB_ANIME : anime_db, season_db, epizod_db
 * DB_STAT  : stat_downloads, stat_like, stat_dislike, stat_save, stat_coment, stat_share
 * DB_MAIN  : user_db, sessions, bot_settings, post_queue, chanel_info_db, ...
 */
import { d1Run, d1Update, d1InsertReturningId, d1Batch, d1BatchRead, ensureD1Tables } from "./db_d1.ts";
export { d1Update };
import { formatTashkent } from "./date_utils.ts";

// ─── Asosiy so'rov funksiyalari ───────────────────────────────────────────────

/** Barcha SQL so'rovlar shu orqali o'tadi — bazaga avtomatik yo'naltiriladi. */
export async function d1Query<T = Record<string, unknown>>(
  sql: string,
  params: unknown[] = []
): Promise<T[]> {
  return d1Run<T>(sql, params);
}

/** Batch DML — bitta network safari, turli bazalarga guruhlangan. */
export async function d1QueryBatch(
  queries: { sql: string; params?: unknown[] }[]
): Promise<void> {
  await d1Batch(queries);
}

/** INSERT → primary key qaytaradi. */
export async function insertReturningId(
  sql: string,
  params: unknown[],
  pkColumn: string = "id"
): Promise<number> {
  return d1InsertReturningId(sql, params, pkColumn);
}

// ─── Moslik uchun stubs ───────────────────────────────────────────────────────
export async function loadActiveDb(): Promise<void> {}
export function setActiveDbCache(_db: string): void {}
export function getActiveDbCache(): string { return "d1"; }

export interface AnimeRecord {
  id: number;
  name: string;
  info: string;
  chanel_id: string;
  file_id: string;
  downloads: number;
  data: string;
  updated_at: string;
}

/**
 * Jadval yaratish — yagona schema manbai db_d1.ts:ensureD1Tables() da.
 */
export async function createTable(): Promise<void> {
  await ensureD1Tables();
}

// ─── stat_downloads — noyob yuklash hisoblagichi ──────────────────────────────
/**
 * Foydalanuvchi qismni yuklagan bo'lsa false qaytaradi.
 * Birinchi marta yuklab olsa true qaytaradi va statistikalarni yangilaydi.
 *
 * MUHIM: stat_downloads → DB_STAT, lekin epizod/season/anime → DB_ANIME.
 * Cross-DB bo'lgani uchun alohida parallel await ishlatiladi (d1Batch emas).
 */
export async function checkAndRecordDownload(
  userId: number,
  animeId: number,
  episodeRowid: number,
  episodeId: number
): Promise<boolean> {
  const now = formatTashkent();
  const existing = await d1Query<{ user_id: number }>(
    `SELECT user_id FROM stat_downloads WHERE user_id = ? AND episode_id = ?`,
    [userId, episodeId]
  );
  if (existing.length > 0) {
    // Allaqachon yuklab olingan — faqat ko'rish tarixini yangilaymiz
    await upsertWatchHistory(userId, animeId, episodeId);
    return false;
  }

  // Cross-DB: STAT (stat_downloads + watch_history) + ANIME (epizod/season/anime) + MAIN (user_db) — parallel
  await Promise.all([
    // DB_STAT — yuklab olish yozuvi
    d1Query(
      `INSERT OR IGNORE INTO stat_downloads (user_id, anime_id, episode_id, downloaded_at) VALUES (?, ?, ?, ?)`,
      [userId, animeId, episodeId, now]
    ),
    // DB_STAT — ko'rish tarixi (UPSERT: bir foydalanuvchi + bir anime = bitta qator)
    upsertWatchHistory(userId, animeId, episodeId),
    // DB_ANIME — bitta batch (3 ta UPDATE)
    d1QueryBatch([
      {
        sql: `UPDATE epizod_db SET downloads = downloads + 1 WHERE rowid = ?`,
        params: [episodeRowid],
      },
      {
        sql: `UPDATE season_db SET downloads = downloads + 1 WHERE rowid = (SELECT seasondb_id FROM epizod_db WHERE rowid = ?)`,
        params: [episodeRowid],
      },
      {
        sql: `UPDATE anime_db SET downloads = downloads + 1 WHERE id = ?`,
        params: [animeId],
      },
    ]),
    // DB_MAIN — foydalanuvchi yuklamalar hisoblagichi
    d1Query(
      `UPDATE user_db SET downloads = downloads + 1 WHERE user_id = ?`,
      [userId]
    ),
  ]);
  return true;
}

// ─── Ko'rish tarixi (stat_watch_history) ──────────────────────────────────────

export interface WatchHistoryRecord {
  user_id: number;
  anime_id: number;
  epizod_id: number;
  create_at: string;
}

/**
 * Foydalanuvchi ko'rish tarixini UPSERT qiladi.
 * Har bir (user_id, anime_id) juftligi uchun faqat bitta qator saqlanadi —
 * keyingi marta yuklasa, epizod_id va create_at yangilanadi.
 */
export async function upsertWatchHistory(
  userId: number,
  animeId: number,
  epizodId: number
): Promise<void> {
  const now = formatTashkent();
  await d1Query(
    `INSERT INTO stat_watch_history (user_id, anime_id, epizod_id, create_at)
     VALUES (?, ?, ?, ?)
     ON CONFLICT(user_id, anime_id) DO UPDATE SET
       epizod_id = excluded.epizod_id,
       create_at = excluded.create_at`,
    [userId, animeId, epizodId, now]
  );
}

/**
 * Foydalanuvchining berilgan anime uchun oxirgi ko'rgan qismini qaytaradi.
 * Agar tarix yo'q bo'lsa — null.
 */
export async function getWatchHistory(
  userId: number,
  animeId: number
): Promise<WatchHistoryRecord | null> {
  const rows = await d1Query<WatchHistoryRecord>(
    `SELECT user_id, anime_id, epizod_id, create_at
     FROM stat_watch_history
     WHERE user_id = ? AND anime_id = ?`,
    [userId, animeId]
  );
  return rows[0] ?? null;
}

export type Quality = "1080p" | "720p" | "480p" | "360p";
export const QUALITIES: Quality[] = ["1080p", "720p", "480p", "360p"];

export async function getNextEpizoddbId(): Promise<number> {
  const rows = await d1Query<{ max: number | null }>(
    "SELECT MAX(epizoddb_id) as max FROM season_db"
  );
  return (rows[0]?.max ?? 0) + 1;
}

// ─── bot_settings — in-memory kesh bilan ─────────────────────────────────────
interface _SettingCacheEntry { value: string | null; expires: number }
const _settingsCache = new Map<string, _SettingCacheEntry>();
const _SETTINGS_TTL = 5 * 60 * 1000; // 5 daqiqa

export function invalidateSettingCache(key?: string): void {
  if (key) _settingsCache.delete(key);
  else _settingsCache.clear();
}

export async function setSetting(key: string, value: string): Promise<void> {
  await d1Query(
    "INSERT INTO bot_settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    [key, value]
  );
  _settingsCache.set(key, { value, expires: Date.now() + _SETTINGS_TTL });
}

export async function getSetting(key: string): Promise<string | null> {
  const cached = _settingsCache.get(key);
  if (cached && cached.expires > Date.now()) return cached.value;

  const rows = await d1Query<{ value: string }>(
    "SELECT value FROM bot_settings WHERE key = ?",
    [key]
  );
  const value = rows[0]?.value ?? null;
  _settingsCache.set(key, { value, expires: Date.now() + _SETTINGS_TTL });
  return value;
}

// ─── season_db ────────────────────────────────────────────────────────────────
export type Kategoriya = "TV" | "FILM" | "OVA";

export interface AnimeInfo {
  rowid: number;
  id: number;
  epizoddb_id: number | null;
  name: string;
  info: string;
  kategoriya: Kategoriya;
  season_id: number | null;
  file_id: string;
  downloads: number;
  data: string;
}

export async function getSeasonsByAnime(animeId: number): Promise<AnimeInfo[]> {
  return d1Query<AnimeInfo>(
    "SELECT rowid, id, epizoddb_id, name, info, kategoriya, season_id, file_id, downloads, data FROM season_db WHERE id = ? ORDER BY rowid ASC",
    [animeId]
  );
}

export async function getAnimeInfo(rowid: number): Promise<AnimeInfo | null> {
  const rows = await d1Query<AnimeInfo>(
    "SELECT rowid, id, epizoddb_id, name, info, kategoriya, season_id, file_id, downloads, data FROM season_db WHERE rowid = ?",
    [rowid]
  );
  return rows[0] ?? null;
}

export async function getAnimeInfoByEpizoddbId(epizoddbId: number): Promise<AnimeInfo | null> {
  const rows = await d1Query<AnimeInfo>(
    "SELECT rowid, id, epizoddb_id, name, info, kategoriya, season_id, file_id, downloads, data FROM season_db WHERE epizoddb_id = ?",
    [epizoddbId]
  );
  return rows[0] ?? null;
}

export async function insertAnimeInfo(
  data: Omit<AnimeInfo, "rowid" | "downloads" | "data" | "epizoddb_id">
): Promise<{ rowid: number; epizoddb_id: number }> {
  const date = formatTashkent();
  const epizoddbId = await getNextEpizoddbId();

  const rowid = await insertReturningId(
    `INSERT INTO season_db (id, epizoddb_id, name, info, kategoriya, season_id, file_id, downloads, data)
     VALUES (?, ?, ?, ?, ?, ?, ?, 0, ?)`,
    [data.id, epizoddbId, data.name, data.info, data.kategoriya, data.season_id, data.file_id, date],
    "rowid"
  );
  return { rowid, epizoddb_id: epizoddbId };
}

export async function incrementAnimeInfoDownloads(rowid: number): Promise<void> {
  await d1Query("UPDATE season_db SET downloads = downloads + 1 WHERE rowid = ?", [rowid]);
}

export async function deleteAnimeInfo(rowid: number): Promise<void> {
  // Ikkala jadval ham DB_ANIME — batch mumkin
  await d1QueryBatch([
    { sql: "DELETE FROM epizod_db WHERE seasondb_id = ?", params: [rowid] },
    { sql: "DELETE FROM season_db WHERE rowid = ?", params: [rowid] },
  ]);
}

export async function updateAnimeInfoText(
  rowid: number,
  field: "name" | "info" | "file_id",
  value: string
): Promise<void> {
  await d1Query(`UPDATE season_db SET ${field} = ? WHERE rowid = ?`, [value, rowid]);
}

export async function updateAnimeInfoNumber(
  rowid: number,
  field: "season_id",
  value: number
): Promise<void> {
  await d1Query(`UPDATE season_db SET ${field} = ? WHERE rowid = ?`, [value, rowid]);
}

// ─── epizod_db ────────────────────────────────────────────────────────────────
export interface Episode {
  rowid: number;
  seasondb_id: number;
  epizoddb_id: number | null;
  id: number;
  name: string;
  epizod_number: number | null;
  film_id: number | null;
  ova_id: number | null;
  epizod_id: number;
  q1080p_file_chanel_id: string | null;
  q1080p_file_bot_id: string | null;
  q720p_file_chanel_id: string | null;
  q720p_file_bot_id: string | null;
  q480p_file_chanel_id: string | null;
  q480p_file_bot_id: string | null;
  q360p_file_chanel_id: string | null;
  q360p_file_bot_id: string | null;
  downloads: number;
  data: string;
}

const EP_FIELDS = `rowid, seasondb_id, epizoddb_id, id, name, epizod_number, film_id, ova_id, epizod_id,
  q1080p_file_chanel_id, q1080p_file_bot_id,
  q720p_file_chanel_id, q720p_file_bot_id,
  q480p_file_chanel_id, q480p_file_bot_id,
  q360p_file_chanel_id, q360p_file_bot_id,
  downloads, data`;

export function getAvailableQualities(ep: Episode): Quality[] {
  return QUALITIES.filter((q) => {
    const chanelId = ep[`q${q}_file_chanel_id` as keyof Episode];
    const botId    = ep[`q${q}_file_bot_id`    as keyof Episode];
    return !!chanelId || !!botId;
  });
}

/** Bazadagi `file_id` maydoni: bitta id yoki 2 GB dan katta video qismlari — vergul bilan ("id1,id2,id3"). */
export function splitFileIds(v: string | null | undefined): string[] {
  return (v ?? "").split(",").map((x) => x.trim()).filter(Boolean);
}

export function getEpisodeFileIds(ep: Episode, quality: Quality): { chanel: string | null; bot: string | null } {
  return {
    chanel: ep[`q${quality}_file_chanel_id` as keyof Episode] as string | null,
    bot: ep[`q${quality}_file_bot_id` as keyof Episode] as string | null,
  };
}

export async function setEpisodeQualityFiles(
  rowid: number,
  quality: Quality,
  chanelId: string,
  botId: string
): Promise<void> {
  await d1Query(
    `UPDATE epizod_db SET q${quality}_file_chanel_id = ?, q${quality}_file_bot_id = ? WHERE rowid = ?`,
    [chanelId, botId, rowid]
  );
}

export async function clearEpisodeQuality(rowid: number, quality: Quality): Promise<void> {
  await d1Query(
    `UPDATE epizod_db SET q${quality}_file_chanel_id = NULL, q${quality}_file_bot_id = NULL WHERE rowid = ?`,
    [rowid]
  );
}

export async function getEpisodesByAnimeInfo(animeInfoId: number): Promise<Episode[]> {
  return d1Query<Episode>(
    `SELECT ${EP_FIELDS} FROM epizod_db WHERE seasondb_id = ? ORDER BY COALESCE(epizod_number, film_id, ova_id) ASC, rowid ASC`,
    [animeInfoId]
  );
}

export async function getEpisode(rowid: number): Promise<Episode | null> {
  const rows = await d1Query<Episode>(
    `SELECT ${EP_FIELDS} FROM epizod_db WHERE rowid = ?`,
    [rowid]
  );
  return rows[0] ?? null;
}

export async function getEpisodeByEpisodeId(episodeId: number): Promise<Episode | null> {
  const rows = await d1Query<Episode>(
    `SELECT ${EP_FIELDS} FROM epizod_db WHERE epizod_id = ?`,
    [episodeId]
  );
  return rows[0] ?? null;
}

export async function getEpisodeByAnimeInfoAndNum(
  animeInfoId: number,
  field: "epizod_number" | "film_id" | "ova_id",
  num: number
): Promise<Episode | null> {
  const rows = await d1Query<Episode>(
    `SELECT ${EP_FIELDS} FROM epizod_db WHERE seasondb_id = ? AND ${field} = ?`,
    [animeInfoId, num]
  );
  return rows[0] ?? null;
}

export async function getNextDownloadId(): Promise<number> {
  const rows = await d1Query<{ max: number | null }>(
    "SELECT MAX(epizod_id) as max FROM epizod_db"
  );
  return (rows[0]?.max ?? 0) + 1;
}

export async function getNextEpisodeNumber(
  animeInfoId: number,
  kategoriya: Kategoriya,
  quality?: Quality
): Promise<number> {
  const col = kategoriya === "TV" ? "epizod_number" : kategoriya === "FILM" ? "film_id" : "ova_id";
  let rows: { max: number | null }[];
  if (quality) {
    rows = await d1Query<{ max: number | null }>(
      `SELECT MAX(${col}) as max FROM epizod_db WHERE seasondb_id = ? AND (q${quality}_file_chanel_id IS NOT NULL OR q${quality}_file_bot_id IS NOT NULL)`,
      [animeInfoId]
    );
  } else {
    rows = await d1Query<{ max: number | null }>(
      `SELECT MAX(${col}) as max FROM epizod_db WHERE seasondb_id = ?`,
      [animeInfoId]
    );
  }
  return (rows[0]?.max ?? 0) + 1;
}

export interface InsertEpisodeData {
  seasondb_id: number;
  epizoddb_id: number | null;
  id: number;
  name: string;
  epizod_number: number | null;
  film_id: number | null;
  ova_id: number | null;
  epizod_id: number;
  quality: Quality;
  chanel_id: string;
  bot_id: string;
}

export async function countEpisodesByQuality(animeInfoId: number, quality: Quality): Promise<number> {
  const rows = await d1Query<{ cnt: number }>(
    `SELECT COUNT(*) as cnt FROM epizod_db WHERE seasondb_id = ? AND (q${quality}_file_chanel_id IS NOT NULL OR q${quality}_file_bot_id IS NOT NULL)`,
    [animeInfoId]
  );
  return rows[0]?.cnt ?? 0;
}

export async function upsertEpisodeQuality(data: InsertEpisodeData): Promise<{
  rowid: number;
  epizod_id: number;
  isNew: boolean;
}> {
  const numField =
    data.epizod_number !== null ? "epizod_number" :
    data.film_id !== null       ? "film_id" :
                                  "ova_id";
  const epNum = data.epizod_number ?? data.film_id ?? data.ova_id ?? 0;

  const existing = await d1Query<{ rowid: number; epizod_id: number }>(
    `SELECT rowid, epizod_id FROM epizod_db WHERE seasondb_id = ? AND ${numField} = ?`,
    [data.seasondb_id, epNum]
  );

  if (existing.length > 0) {
    const row = existing[0];
    await d1Query(
      `UPDATE epizod_db SET q${data.quality}_file_chanel_id = ?, q${data.quality}_file_bot_id = ? WHERE rowid = ?`,
      [data.chanel_id, data.bot_id, row.rowid]
    );
    return { rowid: row.rowid, epizod_id: row.epizod_id, isNew: false };
  }

  const date = formatTashkent();
  const cChanel = `q${data.quality}_file_chanel_id`;
  const cBot = `q${data.quality}_file_bot_id`;
  const rowid = await insertReturningId(
    `INSERT INTO epizod_db (seasondb_id, epizoddb_id, id, name, epizod_number, film_id, ova_id, epizod_id, ${cChanel}, ${cBot}, downloads, data)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?)`,
    [
      data.seasondb_id, data.epizoddb_id, data.id, data.name,
      data.epizod_number, data.film_id, data.ova_id, data.epizod_id,
      data.chanel_id, data.bot_id, date,
    ],
    "rowid"
  );
  return { rowid, epizod_id: data.epizod_id, isNew: true };
}

export async function insertEpisode(data: InsertEpisodeData): Promise<number> {
  const date = formatTashkent();
  const cChanel = `q${data.quality}_file_chanel_id`;
  const cBot = `q${data.quality}_file_bot_id`;
  return insertReturningId(
    `INSERT INTO epizod_db (seasondb_id, epizoddb_id, id, name, epizod_number, film_id, ova_id, epizod_id, ${cChanel}, ${cBot}, downloads, data)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?)`,
    [
      data.seasondb_id, data.epizoddb_id, data.id, data.name,
      data.epizod_number, data.film_id, data.ova_id, data.epizod_id,
      data.chanel_id, data.bot_id, date,
    ],
    "rowid"
  );
}

/** Barcha ma'lumotlarni tozalash — turli bazalar, parallel */
export async function clearAllData(): Promise<void> {
  await Promise.all([
    // DB_STAT
    d1QueryBatch([
      { sql: "DELETE FROM stat_like" },
      { sql: "DELETE FROM stat_dislike" },
      { sql: "DELETE FROM stat_save" },
      { sql: "DELETE FROM stat_downloads" },
      { sql: "DELETE FROM stat_coment" },
      { sql: "DELETE FROM stat_share" },
    ]),
    // DB_ANIME
    d1QueryBatch([
      { sql: "DELETE FROM epizod_db" },
      { sql: "DELETE FROM season_db" },
      { sql: "DELETE FROM anime_db" },
    ]),
    // DB_MAIN
    d1QueryBatch([
      { sql: "DELETE FROM sessions" },
      { sql: "DELETE FROM user_db" },
      { sql: "DELETE FROM post_queue" },
    ]),
  ]);
}

export async function incrementEpisodeDownloads(rowid: number): Promise<void> {
  await d1QueryBatch([
    { sql: "UPDATE epizod_db SET downloads = downloads + 1 WHERE rowid = ?", params: [rowid] },
    { sql: "UPDATE season_db SET downloads = downloads + 1 WHERE rowid = (SELECT seasondb_id FROM epizod_db WHERE rowid = ?)", params: [rowid] },
    { sql: "UPDATE anime_db SET downloads = downloads + 1 WHERE id = (SELECT id FROM epizod_db WHERE rowid = ?)", params: [rowid] },
  ]);
}

export async function deleteEpisode(rowid: number): Promise<void> {
  await d1Query("DELETE FROM epizod_db WHERE rowid = ?", [rowid]);
}

export async function updateEpisodeText(
  rowid: number,
  field: "name",
  value: string
): Promise<void> {
  await d1Query(`UPDATE epizod_db SET ${field} = ? WHERE rowid = ?`, [value, rowid]);
}

export async function updateEpisodeNumber(
  rowid: number,
  field: "epizod_number" | "film_id" | "ova_id",
  value: number
): Promise<void> {
  await d1Query(`UPDATE epizod_db SET ${field} = ? WHERE rowid = ?`, [value, rowid]);
}

export async function idExists(id: number): Promise<boolean> {
  const rows = await d1Query<{ cnt: number }>(
    "SELECT COUNT(*) as cnt FROM anime_db WHERE id = ?",
    [id]
  );
  return (rows[0]?.cnt ?? 0) > 0;
}

export async function getSmallestAvailableId(): Promise<number> {
  const rows = await d1Query<{ id: number }>(
    "SELECT id FROM anime_db ORDER BY id ASC"
  );
  const ids = new Set(rows.map((r) => r.id));
  let candidate = 1;
  while (ids.has(candidate)) candidate++;
  return candidate;
}

export async function insertAnime(
  anime: Omit<AnimeRecord, "downloads" | "data" | "updated_at">
): Promise<void> {
  const now = formatTashkent();
  await d1Query(
    "INSERT INTO anime_db (id, name, info, chanel_id, file_id, downloads, data, updated_at) VALUES (?, ?, ?, ?, ?, 0, ?, ?)",
    [anime.id, anime.name, anime.info, anime.chanel_id, anime.file_id, now, now]
  );
}

export async function getAllAnime(): Promise<AnimeRecord[]> {
  return d1Query<AnimeRecord>("SELECT * FROM anime_db ORDER BY id ASC");
}

export async function getAnimeById(id: number): Promise<AnimeRecord | null> {
  const rows = await d1Query<AnimeRecord>(
    "SELECT * FROM anime_db WHERE id = ?",
    [id]
  );
  return rows[0] ?? null;
}

export async function getAnimesPaginated(
  page: number,
  pageSize: number
): Promise<{ animes: AnimeRecord[]; total: number }> {
  const offset = (page - 1) * pageSize;
  // Ikkala so'rov ham DB_ANIME — bitta batch
  const [[...animes], [...countRows]] = await d1BatchRead<any>([
    {
      sql: "SELECT * FROM anime_db ORDER BY updated_at DESC, id DESC LIMIT ? OFFSET ?",
      params: [pageSize, offset],
    },
    { sql: "SELECT COUNT(*) as cnt FROM anime_db" },
  ]);
  return {
    animes: animes as AnimeRecord[],
    total: Number((countRows as any[])[0]?.cnt ?? 0),
  };
}

export async function updateAnimeField(
  id: number,
  field: "name" | "info" | "chanel_id" | "file_id",
  value: string
): Promise<void> {
  const now = formatTashkent();
  await d1Query(
    `UPDATE anime_db SET ${field} = ?, updated_at = ? WHERE id = ?`,
    [value, now, id]
  );
}

export async function updateAnimeId(oldId: number, newId: number): Promise<void> {
  const now = formatTashkent();
  await d1Query(
    "UPDATE anime_db SET id = ?, updated_at = ? WHERE id = ?",
    [newId, now, oldId]
  );
}

export async function touchAnime(id: number): Promise<void> {
  const now = formatTashkent();
  await d1Query("UPDATE anime_db SET updated_at = ? WHERE id = ?", [now, id]);
}

export async function deleteAnime(id: number): Promise<void> {
  await d1Query("DELETE FROM anime_db WHERE id = ?", [id]);
}

export async function getAnimesPopularPaginated(
  page: number,
  pageSize: number
): Promise<{ animes: AnimeRecord[]; total: number }> {
  const offset = (page - 1) * pageSize;
  const [animesRows, countRows] = await d1BatchRead<any>([
    {
      sql: "SELECT * FROM anime_db ORDER BY downloads DESC, id DESC LIMIT ? OFFSET ?",
      params: [pageSize, offset],
    },
    { sql: "SELECT COUNT(*) as cnt FROM anime_db" },
  ]);
  return {
    animes: animesRows as AnimeRecord[],
    total: Number((countRows as any[])[0]?.cnt ?? 0),
  };
}

export async function searchAnimeByName(query: string): Promise<AnimeRecord[]> {
  return d1Query<AnimeRecord>(
    "SELECT * FROM anime_db WHERE LOWER(name) LIKE LOWER(?) ORDER BY name ASC",
    [`%${query}%`]
  );
}

export interface SeasonSearchRow {
  rowid: number;
  id: number;
  name: string;
  info: string;
  kategoriya: Kategoriya;
  season_id: number | null;
  file_id: string;
  downloads: number;
  anime_name: string;
}

export async function searchSeasonsByAnimeName(query: string, limit = 20): Promise<SeasonSearchRow[]> {
  return d1Query<SeasonSearchRow>(
    `SELECT s.rowid, s.id, s.name, s.info, s.kategoriya, s.season_id, s.file_id, s.downloads,
            a.name AS anime_name
     FROM season_db s
     JOIN anime_db a ON s.id = a.id
     WHERE LOWER(a.name) LIKE LOWER(?)
     ORDER BY a.name ASC, s.rowid ASC
     LIMIT ?`,
    [`%${query}%`, limit]
  );
}

// ─── JOIN: deep link uchun bir so'rovda ep + info + anime ────────────────────

export interface EpisodeWithContext {
  ep: Episode;
  info: AnimeInfo;
  animeName: string;
  animeId: number;
}

export async function getEpisodeWithContextByEpisodeId(
  episodeId: number
): Promise<EpisodeWithContext | null> {
  const rows = await d1Query<{
    ep_rowid: number; seasondb_id: number; epizoddb_id: number | null;
    ep_id: number; ep_name: string; epizod_number: number | null;
    film_id: number | null; ova_id: number | null; epizod_id: number;
    q1080p_file_chanel_id: string | null; q1080p_file_bot_id: string | null;
    q720p_file_chanel_id: string | null; q720p_file_bot_id: string | null;
    q480p_file_chanel_id: string | null; q480p_file_bot_id: string | null;
    q360p_file_chanel_id: string | null; q360p_file_bot_id: string | null;
    ep_downloads: number; ep_data: string;
    info_rowid: number; info_id: number; info_epizoddb_id: number | null;
    info_name: string; info_info: string; info_kategoriya: string;
    info_season_id: number | null; info_file_id: string;
    info_downloads: number; info_data: string;
    anime_name: string;
  }>(
    `SELECT
      e.rowid AS ep_rowid, e.seasondb_id, e.epizoddb_id,
      e.id AS ep_id, e.name AS ep_name,
      e.epizod_number, e.film_id, e.ova_id, e.epizod_id,
      e.q1080p_file_chanel_id, e.q1080p_file_bot_id,
      e.q720p_file_chanel_id, e.q720p_file_bot_id,
      e.q480p_file_chanel_id, e.q480p_file_bot_id,
      e.q360p_file_chanel_id, e.q360p_file_bot_id,
      e.downloads AS ep_downloads, e.data AS ep_data,
      i.rowid AS info_rowid, i.id AS info_id, i.epizoddb_id AS info_epizoddb_id,
      i.name AS info_name, i.info AS info_info,
      i.kategoriya AS info_kategoriya, i.season_id AS info_season_id,
      i.file_id AS info_file_id, i.downloads AS info_downloads, i.data AS info_data,
      a.name AS anime_name
    FROM epizod_db e
    JOIN season_db i ON i.rowid = e.seasondb_id
    JOIN anime_db a ON a.id = i.id
    WHERE e.epizod_id = ?`,
    [episodeId]
  );
  const r = rows[0];
  if (!r) return null;

  const ep: Episode = {
    rowid: r.ep_rowid, seasondb_id: r.seasondb_id, epizoddb_id: r.epizoddb_id,
    id: r.ep_id, name: r.ep_name,
    epizod_number: r.epizod_number, film_id: r.film_id, ova_id: r.ova_id,
    epizod_id: r.epizod_id,
    q1080p_file_chanel_id: r.q1080p_file_chanel_id, q1080p_file_bot_id: r.q1080p_file_bot_id,
    q720p_file_chanel_id: r.q720p_file_chanel_id, q720p_file_bot_id: r.q720p_file_bot_id,
    q480p_file_chanel_id: r.q480p_file_chanel_id, q480p_file_bot_id: r.q480p_file_bot_id,
    q360p_file_chanel_id: r.q360p_file_chanel_id, q360p_file_bot_id: r.q360p_file_bot_id,
    downloads: r.ep_downloads, data: r.ep_data,
  };
  const info: AnimeInfo = {
    rowid: r.info_rowid, id: r.info_id, epizoddb_id: r.info_epizoddb_id,
    name: r.info_name, info: r.info_info,
    kategoriya: r.info_kategoriya as Kategoriya,
    season_id: r.info_season_id, file_id: r.info_file_id,
    downloads: r.info_downloads, data: r.info_data,
  };
  return { ep, info, animeName: r.anime_name, animeId: r.info_id };
}

// ─── JOIN: har bir anime uchun oxirgi qism ────────────────────────────────────

export interface RecentEpisodeRow {
  anime_id: number;
  anime_name: string;
  info_name: string;
  kategoriya: Kategoriya;
  season_id: number | null;
  epizod_number: number | null;
  film_id: number | null;
  ova_id: number | null;
  ep_data: string;
}

export async function getRecentEpisodesPerAnime(
  page: number,
  pageSize: number
): Promise<{ rows: RecentEpisodeRow[]; total: number }> {
  const offset = (page - 1) * pageSize;
  const [rowsRes, countRes] = await d1BatchRead<any>([
    {
      sql: `SELECT
        a.id AS anime_id,
        a.name AS anime_name,
        i.name AS info_name,
        i.kategoriya,
        i.season_id,
        e.epizod_number,
        e.film_id,
        e.ova_id,
        e.data AS ep_data
      FROM anime_db a
      JOIN (
        SELECT i2.id AS aid, MAX(e2.rowid) AS max_ep
        FROM epizod_db e2
        JOIN season_db i2 ON i2.rowid = e2.seasondb_id
        GROUP BY i2.id
      ) latest ON latest.aid = a.id
      JOIN epizod_db e ON e.rowid = latest.max_ep
      JOIN season_db i ON i.rowid = e.seasondb_id
      ORDER BY e.data DESC, a.id DESC
      LIMIT ? OFFSET ?`,
      params: [pageSize, offset],
    },
    {
      sql: `SELECT COUNT(DISTINCT i2.id) AS cnt
       FROM epizod_db e2
       JOIN season_db i2 ON i2.rowid = e2.seasondb_id`,
    },
  ]);
  return {
    rows: rowsRes as RecentEpisodeRow[],
    total: Number((countRes as any[])[0]?.cnt ?? 0),
  };
}

// ─── stat_like / stat_dislike / stat_save ────────────────────────────────────

export async function toggleLike(
  userId: number,
  animeId: number,
  animeinfoId: number
): Promise<"added" | "removed"> {
  const now = formatTashkent();
  const existing = await d1Query<{ user_id: number }>(
    `SELECT user_id FROM stat_like WHERE user_id = ? AND seasondb_id = ?`,
    [userId, animeinfoId]
  );
  if (existing.length > 0) {
    await d1Query(`DELETE FROM stat_like WHERE user_id = ? AND seasondb_id = ?`, [userId, animeinfoId]);
    return "removed";
  }
  // Ikkala jadval ham DB_STAT — batch mumkin
  await d1QueryBatch([
    {
      sql: `INSERT OR IGNORE INTO stat_like (user_id, anime_id, seasondb_id, created_at) VALUES (?, ?, ?, ?)`,
      params: [userId, animeId, animeinfoId, now],
    },
    {
      sql: `DELETE FROM stat_dislike WHERE user_id = ? AND seasondb_id = ?`,
      params: [userId, animeinfoId],
    },
  ]);
  return "added";
}

export async function toggleDislike(
  userId: number,
  animeId: number,
  animeinfoId: number
): Promise<"added" | "removed"> {
  const now = formatTashkent();
  const existing = await d1Query<{ user_id: number }>(
    `SELECT user_id FROM stat_dislike WHERE user_id = ? AND seasondb_id = ?`,
    [userId, animeinfoId]
  );
  if (existing.length > 0) {
    await d1Query(`DELETE FROM stat_dislike WHERE user_id = ? AND seasondb_id = ?`, [userId, animeinfoId]);
    return "removed";
  }
  await d1QueryBatch([
    {
      sql: `INSERT OR IGNORE INTO stat_dislike (user_id, anime_id, seasondb_id, created_at) VALUES (?, ?, ?, ?)`,
      params: [userId, animeId, animeinfoId, now],
    },
    {
      sql: `DELETE FROM stat_like WHERE user_id = ? AND seasondb_id = ?`,
      params: [userId, animeinfoId],
    },
  ]);
  return "added";
}

export async function toggleSave(
  userId: number,
  animeId: number,
  animeinfoId: number
): Promise<"added" | "removed"> {
  const now = formatTashkent();
  const existing = await d1Query<{ user_id: number }>(
    `SELECT user_id FROM stat_save WHERE user_id = ? AND seasondb_id = ?`,
    [userId, animeinfoId]
  );
  if (existing.length > 0) {
    await d1Query(`DELETE FROM stat_save WHERE user_id = ? AND seasondb_id = ?`, [userId, animeinfoId]);
    return "removed";
  }
  await d1Query(
    `INSERT OR IGNORE INTO stat_save (user_id, anime_id, seasondb_id, created_at) VALUES (?, ?, ?, ?)`,
    [userId, animeId, animeinfoId, now]
  );
  return "added";
}

// ─── Animeinfo uchun statistika ───────────────────────────────────────────────

export interface AnimeInfoStats {
  likes: number;
  dislikes: number;
  saves: number;
  comments: number;
  shares: number;
  userLiked: boolean;
  userDisliked: boolean;
  userSaved: boolean;
}

export async function getAnimeInfoStats(
  animeinfoId: number,
  userId: number
): Promise<AnimeInfoStats> {
  // Barcha 8 so'rov DB_STAT — bitta batch
  const results = await d1BatchRead<{ cnt?: number; user_id?: number }>([
    { sql: `SELECT COUNT(*) AS cnt FROM stat_like WHERE seasondb_id = ?`,    params: [animeinfoId] },
    { sql: `SELECT COUNT(*) AS cnt FROM stat_dislike WHERE seasondb_id = ?`, params: [animeinfoId] },
    { sql: `SELECT COUNT(*) AS cnt FROM stat_save WHERE seasondb_id = ?`,    params: [animeinfoId] },
    { sql: `SELECT COUNT(*) AS cnt FROM stat_coment WHERE seasondb_id = ?`,  params: [animeinfoId] },
    { sql: `SELECT COUNT(*) AS cnt FROM stat_share WHERE seasondb_id = ?`,   params: [animeinfoId] },
    { sql: `SELECT user_id FROM stat_like WHERE user_id = ? AND seasondb_id = ?`,    params: [userId, animeinfoId] },
    { sql: `SELECT user_id FROM stat_dislike WHERE user_id = ? AND seasondb_id = ?`, params: [userId, animeinfoId] },
    { sql: `SELECT user_id FROM stat_save WHERE user_id = ? AND seasondb_id = ?`,    params: [userId, animeinfoId] },
  ]);
  const [likes, dislikes, saves, comments, shares, userLike, userDislike, userSave] = results;
  return {
    likes:        Number((likes[0] as any)?.cnt ?? 0),
    dislikes:     Number((dislikes[0] as any)?.cnt ?? 0),
    saves:        Number((saves[0] as any)?.cnt ?? 0),
    comments:     Number((comments[0] as any)?.cnt ?? 0),
    shares:       Number((shares[0] as any)?.cnt ?? 0),
    userLiked:    userLike.length > 0,
    userDisliked: userDislike.length > 0,
    userSaved:    userSave.length > 0,
  };
}

// ─── stat_share ───────────────────────────────────────────────────────────────

export async function insertShare(
  userId: number,
  animeId: number,
  seasondbId: number,
  epizodId: number
): Promise<void> {
  const data = formatTashkent();
  await d1Query(
    `INSERT INTO stat_share (user_id, anime_id, seasondb_id, epizod_id, data) VALUES (?, ?, ?, ?, ?)`,
    [userId, animeId, seasondbId, epizodId, data]
  );
}

export async function getShareCount(seasondbId: number): Promise<number> {
  const rows = await d1Query<{ cnt: number }>(
    `SELECT COUNT(*) AS cnt FROM stat_share WHERE seasondb_id = ?`,
    [seasondbId]
  );
  return rows[0]?.cnt ?? 0;
}

export async function insertShareIfNew(
  userId: number,
  animeId: number,
  seasondbId: number,
  epizodId: number
): Promise<void> {
  const data = formatTashkent();
  await d1Query(
    `INSERT INTO stat_share (user_id, anime_id, seasondb_id, epizod_id, data)
     SELECT ?, ?, ?, ?, ?
     WHERE NOT EXISTS (
       SELECT 1 FROM stat_share WHERE user_id = ? AND anime_id = ? AND seasondb_id = ?
     )`,
    [userId, animeId, seasondbId, epizodId, data, userId, animeId, seasondbId]
  );
}

// ─── stat_coment ──────────────────────────────────────────────────────────────

export interface CommentRecord {
  rowid: number;
  user_id: number;
  username: string;
  anime_id: number;
  seasondb_id: number;
  comment: string;
  data: string;
}

export async function countComments(seasondbId: number): Promise<number> {
  const rows = await d1Query<{ cnt: number }>(
    `SELECT COUNT(*) AS cnt FROM stat_coment WHERE seasondb_id = ?`,
    [seasondbId]
  );
  return rows[0]?.cnt ?? 0;
}

export async function getCommentsPaginated(
  seasondbId: number,
  page: number,
  pageSize: number
): Promise<{ rows: CommentRecord[]; total: number }> {
  const offset = (page - 1) * pageSize;
  const [rows, cnt] = await d1BatchRead<any>([
    {
      sql: `SELECT rowid, user_id, username, anime_id, seasondb_id, comment, data FROM stat_coment WHERE seasondb_id = ? ORDER BY rowid DESC LIMIT ? OFFSET ?`,
      params: [seasondbId, pageSize, offset],
    },
    { sql: `SELECT COUNT(*) AS cnt FROM stat_coment WHERE seasondb_id = ?`, params: [seasondbId] },
  ]);
  return { rows: rows as CommentRecord[], total: Number((cnt as any[])[0]?.cnt ?? 0) };
}

export async function getUserCommentsPaginated(
  userId: number,
  seasondbId: number,
  page: number,
  pageSize: number
): Promise<{ rows: CommentRecord[]; total: number }> {
  const offset = (page - 1) * pageSize;
  const [rows, cnt] = await d1BatchRead<any>([
    {
      sql: `SELECT rowid, user_id, username, anime_id, seasondb_id, comment, data FROM stat_coment WHERE user_id = ? AND seasondb_id = ? ORDER BY rowid DESC LIMIT ? OFFSET ?`,
      params: [userId, seasondbId, pageSize, offset],
    },
    {
      sql: `SELECT COUNT(*) AS cnt FROM stat_coment WHERE user_id = ? AND seasondb_id = ?`,
      params: [userId, seasondbId],
    },
  ]);
  return { rows: rows as CommentRecord[], total: Number((cnt as any[])[0]?.cnt ?? 0) };
}

export async function getComment(rowid: number): Promise<CommentRecord | null> {
  const rows = await d1Query<CommentRecord>(
    `SELECT rowid, user_id, username, anime_id, seasondb_id, comment, data FROM stat_coment WHERE rowid = ?`,
    [rowid]
  );
  return rows[0] ?? null;
}

export async function insertComment(
  userId: number,
  username: string,
  animeId: number,
  seasondbId: number,
  comment: string
): Promise<void> {
  const data = formatTashkent();
  await d1Query(
    `INSERT INTO stat_coment (user_id, username, anime_id, seasondb_id, comment, data) VALUES (?, ?, ?, ?, ?, ?)`,
    [userId, username, animeId, seasondbId, comment, data]
  );
}

export async function updateComment(rowid: number, comment: string): Promise<void> {
  await d1Query(`UPDATE stat_coment SET comment = ? WHERE rowid = ?`, [comment, rowid]);
}

export async function deleteComment(rowid: number): Promise<void> {
  await d1Query(`DELETE FROM stat_coment WHERE rowid = ?`, [rowid]);
}
