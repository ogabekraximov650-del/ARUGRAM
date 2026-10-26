/**
 * Child bot uchun alohida admin tekshiruv tizimi.
 * Global subAdminSet ishlatilmaydi — barcha ma'lumot D1 dan o'qiladi.
 * kanstruktor_info_db.admin_1 … admin_5 ustunlarida saqlanadi.
 */
import { d1Query } from "./db.ts";
import { config } from "./config.ts";

const ADMIN_COLS = ["admin_1", "admin_2", "admin_3", "admin_4", "admin_5"] as const;

interface AdminRow {
  user_id: number;
  admin_1: string | null;
  admin_2: string | null;
  admin_3: string | null;
  admin_4: string | null;
  admin_5: string | null;
}

async function getRow(botId: number): Promise<AdminRow | null> {
  const rows = await d1Query<AdminRow>(
    `SELECT user_id, admin_1, admin_2, admin_3, admin_4, admin_5
     FROM kanstruktor_info_db WHERE bot_id = ?`,
    [botId]
  );
  return rows[0] ?? null;
}

function extractAdmins(row: AdminRow): number[] {
  const result: number[] = [];
  for (const col of ADMIN_COLS) {
    const val = row[col];
    if (val) {
      const n = parseInt(val, 10);
      if (!isNaN(n) && n > 0) result.push(n);
    }
  }
  return result;
}

/**
 * Foydalanuvchi child bot admini, egasi yoki asosiy admin ekanligini tekshiradi.
 * DB ga murojaat qiladi — global state yo'q.
 */
export async function isChildAdmin(botId: number, userId: number | undefined): Promise<boolean> {
  if (!userId) return false;
  if (userId === config.ADMIN_ID) return true;
  const row = await getRow(botId);
  if (!row) return false;
  if (row.user_id === userId) return true; // bot egasi
  return extractAdmins(row).includes(userId);
}

/** Child bot sub-adminlar ro'yxati (egasi kirmaydi) */
export async function getChildAdmins(botId: number): Promise<number[]> {
  const row = await getRow(botId);
  if (!row) return [];
  return extractAdmins(row);
}

/**
 * Sub-admin qo'shadi (max 5 ta).
 * true = muvaffaqiyat, false = limit to'lib ketgan
 */
export async function addChildAdmin(botId: number, userId: number): Promise<boolean> {
  const row = await getRow(botId);
  if (!row) return false;
  const current = extractAdmins(row);
  if (current.includes(userId)) return true;
  if (current.length >= 5) return false;

  for (const col of ADMIN_COLS) {
    if (!row[col]) {
      await d1Query(
        `UPDATE kanstruktor_info_db SET ${col} = ? WHERE bot_id = ?`,
        [String(userId), botId]
      );
      return true;
    }
  }
  return false;
}

/** Sub-adminni o'chiradi */
export async function removeChildAdmin(botId: number, userId: number): Promise<void> {
  const row = await getRow(botId);
  if (!row) return;
  const current = extractAdmins(row).filter(a => a !== userId);

  await d1Query(
    `UPDATE kanstruktor_info_db
     SET admin_1 = ?, admin_2 = ?, admin_3 = ?, admin_4 = ?, admin_5 = ?
     WHERE bot_id = ?`,
    [
      current[0] != null ? String(current[0]) : null,
      current[1] != null ? String(current[1]) : null,
      current[2] != null ? String(current[2]) : null,
      current[3] != null ? String(current[3]) : null,
      current[4] != null ? String(current[4]) : null,
      botId,
    ]
  );
}
