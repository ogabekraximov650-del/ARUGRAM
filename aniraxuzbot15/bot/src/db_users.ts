import { d1Query } from "./db.ts";
import { d1BatchRead } from "./db_d1.ts";
import { formatTashkent } from "./date_utils.ts";

export interface UserRecord {
  bot_user_id: number | null;
  user_id: number;
  username: string;
  user_phone: string;
  friends_phone: string;
  balance: number;
  anime_balance: number;
  last_bot_id: number | null;
  last_bot_data: string;
  downloads: number;
  creat_at: string;  // eski ustun (o'chirilgan, moslik uchun qoldirilgan)
  created_at: string;
}

export interface UserStats {
  likes: number;
  dislikes: number;
  saves: number;
  comments: number;
  shares: number;
  downloads: number;
}

/** Jadval D1 (db_d1.ts:ensureD1Tables) orqali yaratiladi. Bu no-op. */
export async function createUserTable(): Promise<void> {}

export async function getUser(userId: number): Promise<UserRecord | null> {
  const rows = await d1Query<UserRecord>(
    `SELECT * FROM user_db WHERE user_id = ?`,
    [userId]
  );
  return rows[0] ?? null;
}

export async function upsertUserPhone(
  userId: number,
  username: string,
  phone: string
): Promise<void> {
  const ts = formatTashkent();
  const rows = await d1Query<{ max_id: number | null }>(
    `SELECT MAX(bot_user_id) AS max_id FROM user_db`
  );
  const nextBotUserId = (rows[0]?.max_id ?? 0) + 1;

  await d1Query(
    `INSERT INTO user_db (bot_user_id, user_id, username, user_phone, created_at, last_bot_data)
     VALUES (?, ?, ?, ?, ?, ?)
     ON CONFLICT(user_id) DO UPDATE SET
       bot_user_id = CASE WHEN user_db.bot_user_id IS NULL THEN excluded.bot_user_id ELSE user_db.bot_user_id END,
       username    = excluded.username,
       user_phone  = excluded.user_phone`,
    [nextBotUserId, userId, username, phone, ts, ts]
  );
}

export async function touchLastBot(userId: number, botId: number, username: string): Promise<void> {
  const ts = formatTashkent();
  await d1Query(
    `INSERT INTO user_db (user_id, username, last_bot_id, last_bot_data, created_at, last_active)
     VALUES (?, ?, ?, ?, ?, ?)
     ON CONFLICT(user_id) DO UPDATE SET
       last_bot_id   = excluded.last_bot_id,
       last_bot_data = excluded.last_bot_data,
       username      = excluded.username,
       last_active   = excluded.last_active`,
    [userId, username, botId, ts, ts, ts]
  );
}

export async function incrementDownloads(userId: number): Promise<void> {
  await d1Query(`UPDATE user_db SET downloads = downloads + 1 WHERE user_id = ?`, [userId]);
}

/** Asosiy balansni oshirish/kamaytirishh (manfiy qiymat ayiradi) */
export async function updateUserBalance(userId: number, amount: number): Promise<void> {
  await d1Query(
    `UPDATE user_db SET balance = MAX(0, balance + ?) WHERE user_id = ?`,
    [amount, userId]
  );
}

/** Anime balansni oshirish/kamaytirish (manfiy qiymat ayiradi) */
export async function updateAnimeBalance(userId: number, amount: number): Promise<void> {
  await d1Query(
    `UPDATE user_db SET anime_balance = MAX(0, anime_balance + ?) WHERE user_id = ?`,
    [amount, userId]
  );
}

/** Foydalanuvchini qidirish (bot_id, telegram_id, phone yoki username) */
export async function findUserByIdentifier(identifier: string): Promise<UserRecord | null> {
  const trimmed = identifier.trim().replace(/^@/, "");
  const digitsOnly = trimmed.replace(/\D/g, "");

  // @username orqali
  if (identifier.trim().startsWith("@") || (!identifier.trim().startsWith("+") && digitsOnly.length < 6)) {
    const rows = await d1Query<UserRecord>(
      `SELECT * FROM user_db WHERE username = ? COLLATE NOCASE`,
      [trimmed]
    );
    if (rows[0]) return rows[0];
  }

  // Telefon raqam orqali
  if (digitsOnly.length >= 9 && digitsOnly.length <= 12) {
    const last9 = digitsOnly.slice(-9);
    const rows = await d1Query<UserRecord>(
      `SELECT * FROM user_db WHERE user_phone LIKE ?`,
      [`%${last9}`]
    );
    if (rows[0]) return rows[0];
  }

  // Raqam orqali (bot_id yoki telegram_id)
  if (digitsOnly) {
    const asNum = parseInt(digitsOnly, 10);
    if (asNum > 0 && asNum <= 999999) {
      const rows = await d1Query<UserRecord>(
        `SELECT * FROM user_db WHERE bot_user_id = ?`,
        [asNum]
      );
      if (rows[0]) return rows[0];
    }
    if (asNum > 0) {
      const rows = await d1Query<UserRecord>(
        `SELECT * FROM user_db WHERE user_id = ?`,
        [asNum]
      );
      if (rows[0]) return rows[0];
    }
  }

  return null;
}

/**
 * Foydalanuvchi faollik statistikasi.
 * 3 ta so'rov — BITTA batch round-trip.
 */
export async function getUserActivityStats(userId: number): Promise<UserStats> {
  // 3 ta so'rov — BITTA HTTP safari (oldin 6 ta).
  const [react, comments, downloads] = await d1BatchRead<Record<string, number>>([
    {
      sql: `SELECT COALESCE(SUM(reaction = 1), 0) AS likes, COALESCE(SUM(reaction = -1), 0) AS dislikes,
                   COALESCE(SUM(saved), 0) AS saves, COALESCE(SUM(shared), 0) AS shares
            FROM stat_user_season WHERE user_id = ?`,
      params: [userId],
    },
    { sql: `SELECT COUNT(*) AS cnt FROM stat_coment WHERE user_id = ?`, params: [userId] },
    { sql: `SELECT COUNT(*) AS cnt FROM stat_downloads WHERE user_id = ?`, params: [userId] },
  ]);
  const r = react[0] ?? {};
  return {
    likes:     Number(r.likes ?? 0),
    dislikes:  Number(r.dislikes ?? 0),
    saves:     Number(r.saves ?? 0),
    comments:  Number(comments[0]?.cnt ?? 0),
    shares:    Number(r.shares ?? 0),
    downloads: Number(downloads[0]?.cnt ?? 0),
  };
}

export interface PaginatedUsers {
  users: UserRecord[];
  total: number;
}

/** Sahifalangan foydalanuvchilar ro'yxati */
export async function getUsersPaginated(
  page: number,
  limit: number,
  orderBy: "registered" | "active"
): Promise<PaginatedUsers> {
  const offset = (page - 1) * limit;
  const order = orderBy === "registered"
    ? "COALESCE(bot_user_id, 9999999) DESC"
    : "last_bot_data DESC";

  const [users, countRows] = await Promise.all([
    d1Query<UserRecord>(
      `SELECT * FROM user_db ORDER BY ${order} LIMIT ? OFFSET ?`,
      [limit, offset]
    ),
    d1Query<{ cnt: number }>(`SELECT COUNT(*) as cnt FROM user_db`),
  ]);

  return { users, total: countRows[0]?.cnt ?? 0 };
}

export async function getUsersByIds(ids: number[]): Promise<UserRecord[]> {
  if (ids.length === 0) return [];
  const placeholders = ids.map(() => "?").join(",");
  return await d1Query<UserRecord>(
    `SELECT * FROM user_db WHERE user_id IN (${placeholders})`,
    ids
  );
}

export async function getAllBotUserIds(botId: number): Promise<number[]> {
  const rows = await d1Query<{ user_id: number }>(
    `SELECT user_id FROM user_db WHERE last_bot_id = ?`,
    [botId]
  );
  return rows.map(r => r.user_id);
}

export async function getAllServerUserIds(): Promise<number[]> {
  const rows = await d1Query<{ user_id: number }>(
    `SELECT user_id FROM user_db`
  );
  return rows.map(r => r.user_id);
}
