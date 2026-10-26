import { config } from "./config.ts";
import { getSetting, setSetting } from "./db.ts";

/** In-memory sub-admin IDlar set (DB dan yuklanadi, keyin xotirada saqlanadi) */
export const subAdminSet = new Set<number>();

/** Foydalanuvchi admin yoki sub-admin ekanligini tekshiradi (sync, tez) */
export function checkIsAdmin(userId: number | undefined): boolean {
  if (!userId) return false;
  return userId === config.ADMIN_ID || subAdminSet.has(userId);
}

/** Sub-adminlarni bot_settings dan yuklaydi. Bot startida va o'zgarishdan so'ng chaqiriladi. */
export async function reloadSubAdmins(): Promise<void> {
  const stored = (await getSetting("sub_admins")) ?? "";
  subAdminSet.clear();
  if (stored) {
    for (const part of stored.split(",")) {
      const n = parseInt(part.trim(), 10);
      if (!isNaN(n) && n > 0) subAdminSet.add(n);
    }
  }
}

/**
 * Sub-admin qo'shadi.
 * In-memory set ishlatadi — faqat 1 ta D1 call (setSetting).
 * true = muvaffaqiyat, false = limit to'lib ketgan
 */
export async function addSubAdmin(userId: number): Promise<boolean> {
  if (subAdminSet.has(userId)) return true;
  if (subAdminSet.size >= 5) return false;
  subAdminSet.add(userId);
  await setSetting("sub_admins", Array.from(subAdminSet).join(","));
  return true;
}

/**
 * Sub-adminni o'chiradi.
 * In-memory set ishlatadi — faqat 1 ta D1 call (setSetting).
 */
export async function removeSubAdmin(userId: number): Promise<void> {
  subAdminSet.delete(userId);
  await setSetting("sub_admins", Array.from(subAdminSet).join(","));
}

/**
 * Barcha sub-adminlar IDlari ro'yxatini qaytaradi.
 * Sync — D1 call yo'q, xotiradan o'qiladi.
 */
export function getSubAdmins(): number[] {
  return Array.from(subAdminSet);
}
