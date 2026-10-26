import { d1Query } from "./db.ts";
import { formatTashkent } from "./date_utils.ts";

export interface KonstruktorUser {
  user_id: number;
  user_phone: string;
  user_name: string;
  bot_id: number | null;
  bot_token: string | null;
  create_data: string;
  user_status: string;
  sorov_status: string;
  admin_1: string | null;
  admin_2: string | null;
  admin_3: string | null;
  admin_4: string | null;
  admin_5: string | null;
}

/** Jadval D1 (db_d1.ts:ensureD1Tables) orqali yaratiladi. Bu no-op. */
export async function createKonstruktorTable(): Promise<void> {}

export async function getKonstruktorUser(userId: number): Promise<KonstruktorUser | null> {
  const rows = await d1Query<KonstruktorUser>(
    `SELECT user_id, user_phone, user_name, bot_id, bot_token, create_data
     FROM kanstruktor_info_db WHERE user_id = ?`,
    [userId]
  );
  return rows[0] ?? null;
}

export async function upsertKonstruktorUser(
  userId: number,
  phone: string,
  name: string
): Promise<void> {
  const date = formatTashkent();
  await d1Query(
    `INSERT INTO kanstruktor_info_db (user_id, user_phone, user_name, create_data)
     VALUES (?, ?, ?, ?)
     ON CONFLICT(user_id) DO UPDATE SET
       user_phone = excluded.user_phone,
       user_name  = excluded.user_name`,
    [userId, phone, name, date]
  );
}

export async function setKonstruktorBot(
  userId: number,
  botId: number,
  botToken: string
): Promise<void> {
  await d1Query(
    `UPDATE kanstruktor_info_db SET bot_id = ?, bot_token = ? WHERE user_id = ?`,
    [botId, botToken, userId]
  );
}

export async function getKonstruktorByBotId(botId: number): Promise<KonstruktorUser | null> {
  const rows = await d1Query<KonstruktorUser>(
    `SELECT user_id, user_phone, user_name, bot_id, bot_token, create_data
     FROM kanstruktor_info_db WHERE bot_id = ?`,
    [botId]
  );
  return rows[0] ?? null;
}

export async function clearKonstruktorBot(userId: number): Promise<void> {
  await d1Query(
    `UPDATE kanstruktor_info_db SET bot_id = NULL, bot_token = NULL WHERE user_id = ?`,
    [userId]
  );
}

export async function getAllKonstruktorBots(): Promise<
  { bot_id: number; bot_token: string; user_id: number }[]
> {
  const rows = await d1Query<{ bot_id: number; bot_token: string; user_id: number }>(
    `SELECT bot_id, bot_token, user_id FROM kanstruktor_info_db
     WHERE bot_id IS NOT NULL AND bot_token IS NOT NULL`
  );
  return rows;
}
