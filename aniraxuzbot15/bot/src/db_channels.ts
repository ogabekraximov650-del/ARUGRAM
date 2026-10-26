import { d1Query, insertReturningId } from "./db.ts";
import { formatTashkent } from "./date_utils.ts";

export type ChannelType = "public" | "private" | "external";

export interface ChannelInfo {
  id: number;
  user_id: number;
  bot_id: number;
  chanel_type: ChannelType;
  chanel_id: string;
  chanel_username: string;
  chanel_link: string;
  chanel_title: string;
  jami_qoshish_kerak: number;
  jami_qoshilgan: number;
  data: string;
}

export interface ExternalLink {
  id: number;
  user_id: number;
  bot_id: number;
  link_name: string;
  link: string;
  data: string;
}

/** Jadvallar D1 (db_d1.ts:ensureD1Tables) orqali yaratiladi. Bu no-op. */
export async function createChannelTables(): Promise<void> {}

// =================== chanel_info_db ===================

export async function countBotChannels(botId: number): Promise<number> {
  const rows = await d1Query<{ c: number }>(
    `SELECT COUNT(*) AS c FROM chanel_info_db WHERE bot_id = ?`,
    [botId]
  );
  return rows[0]?.c ?? 0;
}

export async function getChannelByBotAndChatId(botId: number, chatId: string): Promise<ChannelInfo | null> {
  const rows = await d1Query<ChannelInfo>(
    `SELECT * FROM chanel_info_db WHERE bot_id = ? AND chanel_id = ?`,
    [botId, chatId]
  );
  return rows[0] ?? null;
}

export async function insertChannel(
  data: Omit<ChannelInfo, "id" | "data" | "jami_qoshilgan">
): Promise<number> {
  const ts = formatTashkent();
  return insertReturningId(
    `INSERT INTO chanel_info_db
     (user_id, bot_id, chanel_type, chanel_id, chanel_username, chanel_link, chanel_title, jami_qoshish_kerak, jami_qoshilgan, data)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0, ?)`,
    [
      data.user_id,
      data.bot_id,
      data.chanel_type,
      data.chanel_id,
      data.chanel_username,
      data.chanel_link,
      data.chanel_title,
      data.jami_qoshish_kerak,
      ts,
    ]
  );
}

export async function getBotChannels(botId: number): Promise<ChannelInfo[]> {
  return d1Query<ChannelInfo>(
    `SELECT * FROM chanel_info_db WHERE bot_id = ? ORDER BY id ASC`,
    [botId]
  );
}

export async function getChannelById(id: number): Promise<ChannelInfo | null> {
  const rows = await d1Query<ChannelInfo>(
    `SELECT * FROM chanel_info_db WHERE id = ?`,
    [id]
  );
  return rows[0] ?? null;
}

export async function deleteChannelById(id: number): Promise<void> {
  await d1Query(`DELETE FROM chanel_info_db WHERE id = ?`, [id]);
}

/** Private kanalning invite linkini yangilash */
export async function updateChannelLink(id: number, link: string): Promise<void> {
  await d1Query(`UPDATE chanel_info_db SET chanel_link = ? WHERE id = ?`, [link, id]);
}

/** Barcha private kanallarni olish (invite link yangilash uchun) */
export async function getAllPrivateChannels(): Promise<ChannelInfo[]> {
  return d1Query<ChannelInfo>(
    `SELECT * FROM chanel_info_db WHERE chanel_type = 'private' ORDER BY id ASC`
  );
}

export async function increaseChannelLimit(id: number, addBy: number): Promise<void> {
  await d1Query(
    `UPDATE chanel_info_db SET jami_qoshish_kerak = jami_qoshish_kerak + ? WHERE id = ?`,
    [addBy, id]
  );
}

/** Limitni kamaytiradi, 0 dan past tushmaydi (CLAMP). */
export async function decreaseChannelLimit(id: number, subBy: number): Promise<void> {
  await d1Query(
    `UPDATE chanel_info_db
     SET jami_qoshish_kerak = MAX(0, jami_qoshish_kerak - ?)
     WHERE id = ?`,
    [subBy, id]
  );
}

export async function incrementJoinedCount(id: number): Promise<void> {
  await d1Query(
    `UPDATE chanel_info_db SET jami_qoshilgan = jami_qoshilgan + 1 WHERE id = ?`,
    [id]
  );
}

// =================== tashqi_link_db ===================

export async function insertExternalLink(
  data: Omit<ExternalLink, "id" | "data">
): Promise<number> {
  const ts = formatTashkent();
  return insertReturningId(
    `INSERT INTO tashqi_link_db (user_id, bot_id, link_name, link, data)
     VALUES (?, ?, ?, ?, ?)`,
    [data.user_id, data.bot_id, data.link_name, data.link, ts]
  );
}

export async function getBotExternalLinks(botId: number): Promise<ExternalLink[]> {
  return d1Query<ExternalLink>(
    `SELECT * FROM tashqi_link_db WHERE bot_id = ? ORDER BY id ASC`,
    [botId]
  );
}

export async function getExternalLinkById(id: number): Promise<ExternalLink | null> {
  const rows = await d1Query<ExternalLink>(
    `SELECT * FROM tashqi_link_db WHERE id = ?`,
    [id]
  );
  return rows[0] ?? null;
}

export async function deleteExternalLinkById(id: number): Promise<void> {
  await d1Query(`DELETE FROM tashqi_link_db WHERE id = ?`, [id]);
}

// =================== request_chanel_db ===================

export async function hasJoinRequest(
  botId: number,
  chanelId: string,
  userId: number
): Promise<boolean> {
  const rows = await d1Query<{ id: number }>(
    `SELECT id FROM request_chanel_db
     WHERE bot_id = ? AND chanel_id = ? AND request_user_id = ?`,
    [botId, chanelId, userId]
  );
  return rows.length > 0;
}

/** Returns true if a new row was inserted (i.e. first time we recorded this user for this channel). */
export async function recordJoinOrSubscribe(
  botId: number,
  chanelId: string,
  userId: number,
  chanelLink: string
): Promise<boolean> {
  if (await hasJoinRequest(botId, chanelId, userId)) return false;
  const ts = formatTashkent();
  await d1Query(
    `INSERT INTO request_chanel_db (bot_id, chanel_id, request_user_id, chanel_link, data)
     VALUES (?, ?, ?, ?, ?)`,
    [botId, chanelId, userId, chanelLink, ts]
  );
  return true;
}

/** Yopiq kanal o'chirilganda unga tegishli barcha so'rovlarni o'chiradi */
export async function deleteRequestsByChannel(botId: number, chanelId: string): Promise<void> {
  await d1Query(
    `DELETE FROM request_chanel_db WHERE bot_id = ? AND chanel_id = ?`,
    [botId, chanelId]
  );
}
