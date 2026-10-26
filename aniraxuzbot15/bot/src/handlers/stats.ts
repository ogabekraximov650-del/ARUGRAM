/**
 * 📊 Statistika — faqat faol VIP foydalanuvchilar uchun
 * protect_content: true — nusxalash, uzatish, saqlash mumkin emas
 */
import { Bot, InlineKeyboard } from "grammy";
import type { MyContext } from "./admin.ts";
import { d1Query } from "../db.ts";
import { getActiveVip } from "./vip.ts";

// ─── Parallel barcha bazalardan ma'lumot olish ────────────────────────────────

interface StatsResult {
  // Foydalanuvchilar
  totalUsers: number;
  monthlyUsers: number;
  weeklyUsers: number;
  dailyUsers: number;
  // Animelar (DB_ANIME)
  totalAnimes: number;
  totalEpisodes: number;
  // Yuklamalar (DB_STAT)
  totalDownloads: number;
  monthlyDownloads: number;
  weeklyDownloads: number;
  dailyDownloads: number;
  // Boshqa (DB_STAT)
  totalLikes: number;
  totalDislikes: number;
  totalShares: number;
  totalSaves: number;
  // DB hajmi (MB → GB)
  dbMainBytes: number;
  dbAnimeBytes: number;
  dbStatBytes: number;
}

async function fetchStats(): Promise<StatsResult> {
  // Barcha so'rovlarni parallel bajartirish
  const [
    totalUsersR,
    monthlyUsersR,
    weeklyUsersR,
    dailyUsersR,
    totalAnimesR,
    totalEpisodesR,
    totalDownloadsR,
    monthlyDownloadsR,
    weeklyDownloadsR,
    dailyDownloadsR,
    totalLikesR,
    totalDislikesR,
    totalSharesR,
    totalSavesR,
    dbMainSizeR,
    dbAnimeSizeR,
    dbStatSizeR,
  ] = await Promise.all([
    // user_db → DB_MAIN
    d1Query<{ n: number }>("SELECT COUNT(*) AS n FROM user_db"),
    d1Query<{ n: number }>("SELECT COUNT(*) AS n FROM user_db WHERE last_active >= strftime('%Y-%m-%d %H:%M:%S', datetime('now', '-30 days', '+5 hours'))"),
    d1Query<{ n: number }>("SELECT COUNT(*) AS n FROM user_db WHERE last_active >= strftime('%Y-%m-%d %H:%M:%S', datetime('now', '-7 days', '+5 hours'))"),
    d1Query<{ n: number }>("SELECT COUNT(*) AS n FROM user_db WHERE last_active >= strftime('%Y-%m-%d %H:%M:%S', datetime('now', '-1 day', '+5 hours'))"),
    // anime_db → DB_ANIME
    d1Query<{ n: number }>("SELECT COUNT(*) AS n FROM anime_db"),
    d1Query<{ n: number }>("SELECT COUNT(*) AS n FROM epizod_db"),
    // stat_downloads → DB_STAT
    d1Query<{ n: number }>("SELECT COUNT(*) AS n FROM stat_downloads"),
    d1Query<{ n: number }>("SELECT COUNT(*) AS n FROM stat_downloads WHERE downloaded_at >= strftime('%Y-%m-%d %H:%M:%S', datetime('now', '-30 days', '+5 hours'))"),
    d1Query<{ n: number }>("SELECT COUNT(*) AS n FROM stat_downloads WHERE downloaded_at >= strftime('%Y-%m-%d %H:%M:%S', datetime('now', '-7 days', '+5 hours'))"),
    d1Query<{ n: number }>("SELECT COUNT(*) AS n FROM stat_downloads WHERE downloaded_at >= strftime('%Y-%m-%d %H:%M:%S', datetime('now', '-1 day', '+5 hours'))"),
    // Boshqa stat jadvallar → DB_STAT
    d1Query<{ n: number }>("SELECT COUNT(*) AS n FROM stat_like"),
    d1Query<{ n: number }>("SELECT COUNT(*) AS n FROM stat_dislike"),
    d1Query<{ n: number }>("SELECT COUNT(*) AS n FROM stat_share"),
    d1Query<{ n: number }>("SELECT COUNT(*) AS n FROM stat_save"),
    // DB hajmlari (SQLite page_count * page_size)
    d1Query<{ bytes: number }>("SELECT (SELECT page_count FROM pragma_page_count()) * (SELECT page_size FROM pragma_page_size()) AS bytes FROM user_db LIMIT 1").catch(() => [{ bytes: 0 }]),
    d1Query<{ bytes: number }>("SELECT (SELECT page_count FROM pragma_page_count()) * (SELECT page_size FROM pragma_page_size()) AS bytes FROM anime_db LIMIT 1").catch(() => [{ bytes: 0 }]),
    d1Query<{ bytes: number }>("SELECT (SELECT page_count FROM pragma_page_count()) * (SELECT page_size FROM pragma_page_size()) AS bytes FROM stat_downloads LIMIT 1").catch(() => [{ bytes: 0 }]),
  ]);

  return {
    totalUsers:    totalUsersR[0]?.n ?? 0,
    monthlyUsers:  monthlyUsersR[0]?.n ?? 0,
    weeklyUsers:   weeklyUsersR[0]?.n ?? 0,
    dailyUsers:    dailyUsersR[0]?.n ?? 0,

    totalAnimes:   totalAnimesR[0]?.n ?? 0,
    totalEpisodes: totalEpisodesR[0]?.n ?? 0,

    totalDownloads:   totalDownloadsR[0]?.n ?? 0,
    monthlyDownloads: monthlyDownloadsR[0]?.n ?? 0,
    weeklyDownloads:  weeklyDownloadsR[0]?.n ?? 0,
    dailyDownloads:   dailyDownloadsR[0]?.n ?? 0,

    totalLikes:    totalLikesR[0]?.n ?? 0,
    totalDislikes: totalDislikesR[0]?.n ?? 0,
    totalShares:   totalSharesR[0]?.n ?? 0,
    totalSaves:    totalSavesR[0]?.n ?? 0,

    dbMainBytes:  dbMainSizeR[0]?.bytes ?? 0,
    dbAnimeBytes: dbAnimeSizeR[0]?.bytes ?? 0,
    dbStatBytes:  dbStatSizeR[0]?.bytes ?? 0,
  };
}

function n(val: number): string {
  return val.toLocaleString("uz");
}

function bytesToGb(bytes: number): string {
  return (bytes / 1_073_741_824).toFixed(2);
}

function buildStatsText(s: StatsResult): string {
  const totalDb = s.dbMainBytes + s.dbAnimeBytes + s.dbStatBytes;

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
    `💾 Asosiy baza: <b>${bytesToGb(s.dbMainBytes)} GB</b>\n` +
    `💾 Anime baza: <b>${bytesToGb(s.dbAnimeBytes)} GB</b>\n` +
    `💾 Stat baza: <b>${bytesToGb(s.dbStatBytes)} GB</b>\n` +
    `📊 Jami: <b>${bytesToGb(totalDb)} GB</b>`
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
