/**
 * 📊 Statistika — faqat faol VIP foydalanuvchilar uchun
 * protect_content: true — nusxalash, uzatish, saqlash mumkin emas
 */
import { Bot, InlineKeyboard } from "grammy";
import type { MyContext } from "./admin.ts";
import { d1Query, d1QueryBatchRead } from "../db.ts";
import { getActiveVip } from "./vip.ts";

// ─── Parallel barcha bazalardan ma'lumot olish ────────────────────────────────

interface StatsResult {
  // Foydalanuvchilar
  totalUsers: number;
  monthlyUsers: number;
  weeklyUsers: number;
  dailyUsers: number;
  // Animelar
  totalAnimes: number;
  totalEpisodes: number;
  // Yuklamalar
  totalDownloads: number;
  monthlyDownloads: number;
  weeklyDownloads: number;
  dailyDownloads: number;
  // Reaksiyalar
  totalLikes: number;
  totalDislikes: number;
  totalShares: number;
  totalSaves: number;
  // Baza hajmi (bayt)
  dbBytes: number;
}

async function fetchStats(): Promise<StatsResult> {
  // Hamma sanash BITTA so'rovda (bitta HTTP safari, bitta tranzaksiyasiz o'qish).
  const one = (sql: string) => ({ sql });
  const [users, animes, episodes, downloads, reactions] = await d1QueryBatchRead<Record<string, number>>([
    one(`SELECT COUNT(*) AS total,
                COALESCE(SUM(last_active >= strftime('%Y-%m-%d %H:%M:%S', datetime('now', '-30 days', '+5 hours'))), 0) AS monthly,
                COALESCE(SUM(last_active >= strftime('%Y-%m-%d %H:%M:%S', datetime('now', '-7 days', '+5 hours'))), 0) AS weekly,
                COALESCE(SUM(last_active >= strftime('%Y-%m-%d %H:%M:%S', datetime('now', '-1 day', '+5 hours'))), 0) AS daily
           FROM user_db`),
    one(`SELECT COUNT(*) AS n FROM anime_db`),
    one(`SELECT COUNT(*) AS n FROM epizod_db`),
    one(`SELECT COUNT(*) AS total,
                COALESCE(SUM(created_at >= unixepoch() - 2592000), 0) AS monthly,
                COALESCE(SUM(created_at >= unixepoch() - 604800), 0) AS weekly,
                COALESCE(SUM(created_at >= unixepoch() - 86400), 0) AS daily
           FROM stat_downloads`),
    one(`SELECT COALESCE(SUM(reaction = 1), 0) AS likes, COALESCE(SUM(reaction = -1), 0) AS dislikes,
                COALESCE(SUM(shared), 0) AS shares, COALESCE(SUM(saved), 0) AS saves
           FROM stat_user_season`),
  ]);
  // Baza hajmi (SQLite page_count * page_size); xato bo'lsa 0.
  const size = await d1Query<{ bytes: number }>(
    "SELECT (SELECT page_count FROM pragma_page_count()) * (SELECT page_size FROM pragma_page_size()) AS bytes"
  ).catch(() => [{ bytes: 0 }]);

  const u = users[0] ?? {};
  const d = downloads[0] ?? {};
  const r = reactions[0] ?? {};
  return {
    totalUsers:   u.total ?? 0,
    monthlyUsers: u.monthly ?? 0,
    weeklyUsers:  u.weekly ?? 0,
    dailyUsers:   u.daily ?? 0,

    totalAnimes:   animes[0]?.n ?? 0,
    totalEpisodes: episodes[0]?.n ?? 0,

    totalDownloads:   d.total ?? 0,
    monthlyDownloads: d.monthly ?? 0,
    weeklyDownloads:  d.weekly ?? 0,
    dailyDownloads:   d.daily ?? 0,

    totalLikes:    r.likes ?? 0,
    totalDislikes: r.dislikes ?? 0,
    totalShares:   r.shares ?? 0,
    totalSaves:    r.saves ?? 0,

    dbBytes: size[0]?.bytes ?? 0,
  };
}

function n(val: number): string {
  return val.toLocaleString("uz");
}

function bytesToGb(bytes: number): string {
  return (bytes / 1_073_741_824).toFixed(2);
}

function buildStatsText(s: StatsResult): string {
  return (
    `📊 <b>@aniraxuzbot statistikasi:</b>\n\n` +

    `👥 <b>Foydalanuvchi statistikasi:</b>\n` +
    `👤 Jami foydalanuvchilar: <b>${n(s.totalUsers)} ta</b>\n` +
    `📅 Oylik (so'nggi 30 kun): <b>${n(s.monthlyUsers)} ta</b>\n` +
    `📆 Haftalik (so'nggi 7 kun): <b>${n(s.weeklyUsers)} ta</b>\n` +
    `🕐 Kunlik (so'nggi 24 soat): <b>${n(s.dailyUsers)} ta</b>\n\n` +

    `🎌 <b>Animelar statistikasi:</b>\n` +
    `🎬 Jami animelar: <b>${n(s.totalAnimes)} ta</b>\n` +
    `🎞 Jami epizodlar: <b>${n(s.totalEpisodes)} ta</b>\n\n` +

    `📥 <b>Yuklashlar statistikasi:</b>\n` +
    `📦 Jami yuklashlar: <b>${n(s.totalDownloads)} marta</b>\n` +
    `📅 Oylik yuklashlar: <b>${n(s.monthlyDownloads)} marta</b>\n` +
    `📆 Haftalik yuklashlar: <b>${n(s.weeklyDownloads)} marta</b>\n` +
    `🕐 Kunlik yuklashlar: <b>${n(s.dailyDownloads)} marta</b>\n\n` +

    `💬 <b>Qolgan jami statistika:</b>\n` +
    `❤️ Yoqtirishlar: <b>${n(s.totalLikes)} ta</b>\n` +
    `👎 Yoqmaganlar: <b>${n(s.totalDislikes)} ta</b>\n` +
    `📤 Ulashishlar: <b>${n(s.totalShares)} ta</b>\n` +
    `🔖 Saqlaganlar: <b>${n(s.totalSaves)} ta</b>\n\n` +

    `🗄 <b>Ma'lumotlar bazasi hajmi:</b>\n` +
    `💾 Baza: <b>${bytesToGb(s.dbBytes)} GB</b>`
  );
}

// ─── setupStats ───────────────────────────────────────────────────────────────

async function sendStats(ctx: MyContext, edit = false): Promise<void> {
  const userId = ctx.from!.id;

  const vip = await getActiveVip(userId);
  if (!vip) {
    const text = "❌ <b>Bu funksiya faqat VIP foydalanuvchilar uchun!</b>\n\n💎 Statistikani ko'rish uchun obuna oling.";
    if (edit && ctx.callbackQuery) {
      await ctx.editMessageText(text, { parse_mode: "HTML" }).catch(() => {});
      await ctx.answerCallbackQuery();
    } else {
      await ctx.reply(text, { parse_mode: "HTML" });
    }
    return;
  }

  if (!edit) {
    const loadingMsg = await ctx.reply("⏳ Statistika yuklanmoqda...").catch(() => null);
    try {
      const stats = await fetchStats();
      const text = buildStatsText(stats);
      const kb = new InlineKeyboard().text("🔄 Yangilash", "stats:refresh");
      if (loadingMsg) await ctx.api.deleteMessage(ctx.chat!.id, loadingMsg.message_id).catch(() => {});
      await ctx.reply(text, { parse_mode: "HTML", protect_content: true, reply_markup: kb });
    } catch (e) {
      console.error("Stats xatosi:", e);
      if (edit) await ctx.answerCallbackQuery("❌ Xato").catch(() => {});
      else await ctx.reply("❌ Statistika yuklanishda xato. Qaytadan urinib ko'ring.");
    }
  } else {
    try {
      await ctx.answerCallbackQuery("⏳ Yangilanmoqda...");
      const stats = await fetchStats();
      const text = buildStatsText(stats);
      const kb = new InlineKeyboard().text("🔄 Yangilash", "stats:refresh");
      await ctx.editMessageText(text, { parse_mode: "HTML", reply_markup: kb }).catch(() => {});
    } catch (e) {
      console.error("Stats refresh xatosi:", e);
      await ctx.answerCallbackQuery("❌ Yangilashda xato").catch(() => {});
    }
  }
}

export function setupStats(bot: Bot<MyContext>): void {
  bot.hears("📊 Statistika", (ctx) => sendStats(ctx, false));

  bot.callbackQuery("stats:refresh", (ctx) => sendStats(ctx, true));
}
