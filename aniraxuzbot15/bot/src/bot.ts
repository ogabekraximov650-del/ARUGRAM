import { Bot } from "grammy";
import { config } from "./config.ts";
import { setupAdminHandlers, createSession, type MyContext } from "./handlers/admin.ts";
import { setupEpisodeHandlers, setupDeepLinkDownload } from "./handlers/episodes.ts";
import { setupOnboarding } from "./handlers/onboarding.ts";
import { setupChannels } from "./handlers/channels.ts";
import { setupBroadcast } from "./handlers/broadcast.ts";
import { setupSearch } from "./handlers/search.ts";
import { setupPlayer, setupDeepLinkPlayer } from "./handlers/player.ts";
import { setupResetData } from "./handlers/admin.ts";
import { setupShare } from "./handlers/share.ts";
import { setupPayment } from "./handlers/payment.ts";
import { setupVip } from "./handlers/vip.ts";
import { setupStats } from "./handlers/stats.ts";
import { setupUserManagement } from "./handlers/user_management.ts";
import { setupDatabaseAdmin } from "./handlers/database_admin.ts";
import { sendMainMenu } from "./handlers/main_menu.ts";

export function createBot(): Bot<MyContext> {
  const bot = new Bot<MyContext>(config.BOT_TOKEN);

  bot.use(createSession());

  // Order matters — specific handlers BEFORE catch-all on("message") handlers.
  // (1) Deep link /start ep_<id>  →  to'g'ridan-to'g'ri video yuborish
  setupDeepLinkDownload(bot);
  // (1b) Deep link /start {a}_{s} va {a}_{s}_{n}_{e}  →  player ochish
  setupDeepLinkPlayer(bot);
  // (2) /start onboarding (contact share + mandatory subscription gate).
  setupOnboarding(bot, sendMainMenu);
  // (3) Channel admin UI ("🔐 Majburiy obunalar") + chat_join_request capture.
  setupChannels(bot);
  // (4) Anime qidirish — "🔍 Anime izlash" tugmasi va barcha search oqimlari.
  setupSearch(bot);
  // (5) Pleyer — "Yuklab Olish" tugmasi, qismlar, like/dislike/save.
  setupPlayer(bot);
  // (6) Kamentaryalar — 💬 tugmasi, yozish, tahrirlash, o'chirish.
  // (7) Share — inline mode orqali epizod ulashish + anime qidirish.
  setupShare(bot);
  // (8) To'lov tizimi — balans, admin/avto to'lov, stars.
  setupPayment(bot);
  // (8b) VIP obuna tizimi — tariflar, balansdan to'lash.
  setupVip(bot);
  setupStats(bot);
  // (9) Foydalanuvchi boshqaruvi — admin uchun qidiruv, ro'yxat, balans tahrirlash.
  setupUserManagement(bot);
  // (10) Database boshqaruvi — D1 ↔ Turso almashtirish va sinxronlash.
  setupDatabaseAdmin(bot);
  // (10) Ma'lumotlarni tozalash — faqat superadmin /resetdata
  setupResetData(bot);

  // /cancel — escape any stuck state
  bot.command("cancel", async (ctx) => {
    ctx.session.state = "idle";
    await ctx.reply("✅ Bekor qilindi. /admin orqali davom eting.");
  });

  setupBroadcast(bot);
  setupEpisodeHandlers(bot);
  setupAdminHandlers(bot);

  bot.catch((err) => {
    console.error("Bot xatosi:", err.message, err.error, err.ctx?.update);
    if (err.ctx?.session) {
      err.ctx.session.state = "idle";
    }
  });

  return bot;
}
