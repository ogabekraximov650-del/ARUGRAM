/**
 * Post navbati (post_queue) jadvali — D1 (DB_MAIN) orqali ishlaydi.
 */
import { d1Query } from "./db.ts";
import { d1Update } from "./db_d1.ts";
import { nowSec } from "./date_utils.ts";

export interface QueueItem {
  id: number;
  batch_id: string;
  admin_chat_id: number;
  anime_info_id: number;
  episode_rowid: number;
  channel_id: string;
  bot_username: string;
  channel_username: string;
  status: "pending" | "processing" | "done" | "error";
  /** NULL bo'lsa — hali yuborilmagan. Agar to'ldirilgan bo'lsa — qayta yuborilmaydi. */
  posted_at: number | null;
}

/** Jadval D1 (db_d1.ts:ensureD1Tables) orqali yaratiladi. Bu no-op. */
export async function createPostQueueTable(): Promise<void> {}

export async function insertQueueBatch(
  batchId: string,
  adminChatId: number,
  animeInfoId: number,
  episodeRowids: number[],
  channelId: string,
  botUsername: string,
  channelUsername: string
): Promise<void> {
  for (const rowid of episodeRowids) {
    await d1Query(
      `INSERT INTO post_queue
        (batch_id, admin_chat_id, anime_info_id, episode_rowid, channel_id, bot_username, channel_username, status)
       VALUES (?, ?, ?, ?, ?, ?, ?, 'pending')`,
      [batchId, adminChatId, animeInfoId, rowid, channelId, botUsername, channelUsername]
    );
  }
}

export async function getPendingQueueItems(limit = 10): Promise<QueueItem[]> {
  const rows = await d1Query<QueueItem>(
    `SELECT id, batch_id, admin_chat_id, anime_info_id, episode_rowid,
            channel_id, bot_username, channel_username, status, posted_at
     FROM post_queue
     WHERE status = 'pending'
     ORDER BY id ASC
     LIMIT ?`,
    [limit]
  );
  return rows;
}

/**
 * Queue itemni "processing" holatiga o'tkazadi va lock olindi yoki yo'qligini qaytaradi.
 *
 * WHERE status='pending' sharti — agar boshqa cron/worker allaqachon olgan bo'lsa,
 * rowsAffected=0 bo'ladi va false qaytadi. Caller itemni o'tkazib yuborishi shart.
 */
export async function markQueueItemProcessing(id: number): Promise<boolean> {
  const now = nowSec();
  const affected = await d1Update(
    `UPDATE post_queue SET status = 'processing', processing_started_at = ? WHERE id = ? AND status = 'pending'`,
    [now, id]
  );
  return affected > 0;
}

/**
 * Yuborishdan OLDIN posted_at ni o'rnatadi.
 * Maqsad: agar tgSend muvaffaqiyatli bo'lsa lekin keyingi markQueueItemDone() DB xatosi bersa,
 * item reset qilinganda posted_at bor bo'ladi → qayta yuborilmaydi (duplicate prevention).
 */
export async function markQueueItemSending(id: number): Promise<void> {
  const now = nowSec();
  await d1Query(
    `UPDATE post_queue SET posted_at = ? WHERE id = ? AND posted_at IS NULL`,
    [now, id]
  ).catch(e => console.warn(`markQueueItemSending(${id}) xato (ignore):`, e));
}

export async function markQueueItemDone(id: number): Promise<void> {
  const now = nowSec();
  await d1Query(
    `UPDATE post_queue SET status = 'done', posted_at = COALESCE(posted_at, ?) WHERE id = ?`,
    [now, id]
  );
}

export async function markQueueItemError(id: number): Promise<void> {
  await d1Query(`UPDATE post_queue SET status = 'error' WHERE id = ?`, [id]);
}

/**
 * Faqat HAQIQATAN stuck bo'lgan itemlarni qaytaradi.
 *
 * MUHIM: Cloudflare cron parallel ishlaishi mumkin — agar barcha processing
 * itemlarni reset qilsak, parallel cronlar bir xil itemni ikki marta yuboradi
 * (DUPLICATE POST muammosi).
 *
 * Yechim: faqat 3 daqiqadan ko'p "processing" holatida qolgan itemlarni reset
 * qilamiz. Agar cron normal ishlasa, har item 3s delay bilan ~36s da tugaydi.
 * 3 daqiqa = 180 sekund — bu cron o'lib qolgan degan ma'no.
 */
export async function resetAllProcessingItems(): Promise<void> {
  const threeMinutesAgo = nowSec() - 3 * 60;

  // MUHIM: faqat posted_at IS NULL (hali yuborilmagan) itemlarni reset qilamiz.
  // Agar posted_at to'ldirilgan bo'lsa — xabar yuborilgan, lekin 'done' yozilmagan;
  // bunday itemni qayta pending qilsak xabar ikki marta yuboriladi (DUPLICATE bug).
  await d1Query(
    `UPDATE post_queue SET status = 'pending', processing_started_at = NULL
     WHERE status = 'processing'
       AND posted_at IS NULL
       AND (processing_started_at IS NULL OR processing_started_at < ?)`,
    [threeMinutesAgo]
  ).catch(e => console.error("resetAllProcessingItems error:", e));

  // posted_at bor lekin 'done' emas — xabar yuborilgan, faqat holat to'g'irlanmagan
  await d1Query(
    `UPDATE post_queue SET status = 'done'
     WHERE status = 'processing'
       AND posted_at IS NOT NULL`
  ).catch(() => {});

  // Xato bo'lgan itemlarni qayta urinish (24 soat davomida)
  await d1Query(`
    UPDATE post_queue SET status = 'pending'
    WHERE status = 'error'
      AND created_at > unixepoch() - 86400
  `).catch(() => {});
}

export async function getBatchStats(batchId: string): Promise<{ pending: number; processing: number; done: number; error: number }> {
  const rows = await d1Query<{ status: string; cnt: number }>(
    `SELECT status, COUNT(*) as cnt FROM post_queue WHERE batch_id = ? GROUP BY status`,
    [batchId]
  );
  const stats = { pending: 0, processing: 0, done: 0, error: 0 };
  for (const r of rows) {
    if (r.status === "pending") stats.pending = Number(r.cnt);
    else if (r.status === "processing") stats.processing = Number(r.cnt);
    else if (r.status === "done") stats.done = Number(r.cnt);
    else if (r.status === "error") stats.error = Number(r.cnt);
  }
  return stats;
}
