/**
 * Grammy sessiya adapteri — Cloudflare D1 (DB_MAIN), in-memory cache bilan tezlashtirilgan.
 *
 * - In-memory LRU cache (per Worker isolate) → D1 ga so'rov kamayadi
 * - Sessiya o'qish: cache hit → 0ms, cache miss → D1 (~1-3ms)
 * - Sessiya yozish: cache yangilanadi + D1 ga yoziladi
 */
import type { StorageAdapter } from "grammy";
import { d1Query } from "./db.ts";
import { formatTashkent } from "./date_utils.ts";

// ─── In-memory sessiya keshi (Worker isolate doirasida) ──────────────────────
// CF Workers: har bir isolate o'z keshiga ega (shared memory yo'q)
// Node.js: global process keshi (barcha so'rovlar uchun umumiy)

interface CacheEntry {
  value: unknown;
  expires: number;
}

const _cache = new Map<string, CacheEntry>();
const SESSION_TTL_MS = 10 * 60 * 1000; // 10 daqiqa
const MAX_CACHE_SIZE = 500; // maksimal kesh o'lchami

function cacheGet<T>(key: string): T | undefined {
  const entry = _cache.get(key);
  if (!entry) return undefined;
  if (entry.expires < Date.now()) {
    _cache.delete(key);
    return undefined;
  }
  return entry.value as T;
}

function cacheSet(key: string, value: unknown): void {
  // Kesh to'lgan bo'lsa eng eski yozuvni o'chirish (LRU approx)
  if (_cache.size >= MAX_CACHE_SIZE) {
    const firstKey = _cache.keys().next().value;
    if (firstKey) _cache.delete(firstKey);
  }
  _cache.set(key, { value, expires: Date.now() + SESSION_TTL_MS });
}

function cacheDel(key: string): void {
  _cache.delete(key);
}

// ─── Jadval yaratish ──────────────────────────────────────────────────────────

/**
 * Jadval D1 (db_d1.ts:ensureD1Tables) orqali yaratiladi.
 * Bu yerda faqat 30 kundan eski sessiyalarni tozalash bajariladi.
 */
export async function createSessionTable(): Promise<void> {
  // 30 kundan eski sessiyalarni tozalash — TEXT formatdagi sana bilan taqqoslash
  await d1Query(
    `DELETE FROM sessions WHERE updated_at < strftime('%Y-%m-%d %H:%M:%S', 'now', '+5 hours', '-30 days')`
  ).catch(() => {});
}

// ─── Sessiya adapteri ─────────────────────────────────────────────────────────

export class D1SessionAdapter<T> implements StorageAdapter<T> {
  async read(key: string): Promise<T | undefined> {
    // 1. In-memory kesh (eng tez yo'l)
    const cached = cacheGet<T>(key);
    if (cached !== undefined) return cached;

    // 2. D1 dan o'qish
    try {
      const rows = await d1Query<{ value: string }>(
        `SELECT value FROM sessions WHERE key = ?`,
        [key]
      );
      if (!rows[0]) return undefined;
      const val = JSON.parse(rows[0].value) as T;
      cacheSet(key, val);
      return val;
    } catch {
      return undefined;
    }
  }

  async write(key: string, value: T): Promise<void> {
    // 1. Keshni DARHOL yangilash — keyingi read(key) 0ms da keshdan keladi
    cacheSet(key, value);

    // 2. D1 ga yozish — AWAIT bilan (xavfsiz).
    //    CF Workers da waitUntil ichida ishlaydi, shuning uchun await zarur.
    const now = formatTashkent();
    await d1Query(
      `INSERT INTO sessions (key, value, updated_at)
       VALUES (?, ?, ?)
       ON CONFLICT(key) DO UPDATE
         SET value      = excluded.value,
             updated_at = excluded.updated_at`,
      [key, JSON.stringify(value), now]
    ).catch(() => {/* D1 xatosi — kesh saqlanib qoladi */});
  }

  async delete(key: string): Promise<void> {
    cacheDel(key);
    await d1Query(`DELETE FROM sessions WHERE key = ?`, [key]).catch(() => {});
  }
}
