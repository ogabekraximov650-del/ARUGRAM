/**
 * Yagona sana/vaqt formatlash utilitasi — butun loyihada bitta format ishlatiladi:
 *   "YYYY-MM-DD HH:MM:SS"  (Toshkent vaqti, UTC+5)
 *
 * Bu format:
 *  - DB ustunlarida TEXT sifatida saqlanadi (lexicographic sort = chronological sort)
 *  - Botda ko'rsatiladigan barcha sanalarda ishlatiladi
 *  - SQLite tomonida ham bir xil natija: strftime('%Y-%m-%d %H:%M:%S','now','+5 hours')
 */

const TASHKENT_OFFSET_MS = 5 * 60 * 60 * 1000;

/** Hozirgi Toshkent vaqtini "YYYY-MM-DD HH:MM:SS" formatida qaytaradi. */
export function formatTashkent(date: Date = new Date()): string {
  const t = new Date(date.getTime() + TASHKENT_OFFSET_MS);
  const y = t.getUTCFullYear();
  const mo = String(t.getUTCMonth() + 1).padStart(2, "0");
  const d = String(t.getUTCDate()).padStart(2, "0");
  const h = String(t.getUTCHours()).padStart(2, "0");
  const mi = String(t.getUTCMinutes()).padStart(2, "0");
  const s = String(t.getUTCSeconds()).padStart(2, "0");
  return `${y}-${mo}-${d} ${h}:${mi}:${s}`;
}

/** SQL DEFAULT ifodasi — jadval yaratishda created_at/updated_at ustunlari uchun. */
export const SQL_TASHKENT_NOW = `(strftime('%Y-%m-%d %H:%M:%S','now','+5 hours'))`;
