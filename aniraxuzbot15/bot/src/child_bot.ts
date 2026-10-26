/**
 * Child (konstruktor) bot — asosiy botdan to'liq mustaqil tizim.
 * Global subAdminSet ishlatilmaydi. isChildAdmin() DB dan o'qiydi.
 */
import { Bot, Keyboard, InlineKeyboard } from "grammy";
import { getKonstruktorByBotId } from "./db_konstruktor.ts";
import {
  createSession,
  childAdminMainKeyboard,
  type MyContext,
} from "./handlers/admin.ts";
import { setupChildAdminHandlers } from "./handlers/child_admin.ts";
import { isChildAdmin } from "./child_admin_check.ts";
import { setupDeepLinkDownload } from "./handlers/episodes.ts";
import { setupOnboarding } from "./handlers/onboarding.ts";
import { setupChannels } from "./handlers/channels.ts";
import { setupSearch } from "./handlers/search.ts";
import { setupPlayer, setupDeepLinkPlayer } from "./handlers/player.ts";
import { setupShare } from "./handlers/share.ts";
import {
  getBotChannels,
  getBotExternalLinks,
  hasJoinRequest,
  type ChannelInfo,
} from "./db_channels.ts";
import { getUser } from "./db_users.ts";

// ─── Cache: har bir child bot faqat bir marta quriladi ───────────────────────
const childCache = new Map<number, Bot<MyContext>>();

export function evictChildBot(botId: number): void {
  childCache.delete(botId);
}

// ─── Reply-keyboard tugmalari ────────────────────────────────────────────────
const BTN_ADMIN_PANEL = "🛡 Admin paneli";
const BTN_SEARCH     = "🔍 Anime izlash";
const BTN_BOT_MANAGE = "🤖 Botlarni boshqarish";
const BTN_USER_PANEL = "🏠 Foydalanuvchi paneli";

function userMainKeyboard(): Keyboard {
  return new Keyboard()
    .text(BTN_SEARCH).row()
    .text(BTN_BOT_MANAGE)
    .resized();
}

function ownerMainKeyboard(): Keyboard {
  return new Keyboard()
    .text(BTN_SEARCH).row()
    .text(BTN_BOT_MANAGE).row()
    .text(BTN_ADMIN_PANEL)
    .resized();
}

// ─── Majburiy obuna tekshiruvi (child bot uchun, asosiy botdan mustaqil) ─────

async function childCheckSubscribed(
  ctx: MyContext,
  ch: ChannelInfo,
  userId: number
): Promise<boolean> {
  try {
    const m = await ctx.api.getChatMember(ch.chanel_id, userId);
    if (["creator", "administrator", "member", "restricted"].includes(m.status)) return true;
  } catch { /* bot kanalga kira olmasa — obuna emas */ }
  if (ch.chanel_type === "private") {
    if (await hasJoinRequest(ctx.me.id, ch.chanel_id, userId)) return true;
  }
  return false;
}

async function childShowGate(ctx: MyContext): Promise<boolean> {
  if (!ctx.from) return true;
  const botId  = ctx.me.id;
  const userId = ctx.from.id;
  const channels = await getBotChannels(botId);
  if (channels.length === 0) return true;

  const results = await Promise.all(channels.map(async (ch) => {
    // Limit to'lgan bo'lsa — bu kanalga yangi odam qo'shilmaydi, ko'rsatilmasin
    if (ch.jami_qoshish_kerak > 0 && ch.jami_qoshilgan >= ch.jami_qoshish_kerak) return true;
    return childCheckSubscribed(ctx, ch, userId);
  }));
  const unsub   = channels.filter((_, i) => !results[i]);
  if (unsub.length === 0) return true;

  const kb = new InlineKeyboard();
  unsub.forEach((ch, i) => {
    let url = "";
    if (ch.chanel_type === "public" && ch.chanel_username)
      url = `https://t.me/${ch.chanel_username.replace(/^@/, "")}`;
    else if (ch.chanel_link)
      url = ch.chanel_link;
    if (url) kb.url(`${i + 1} - kanal`, url).row();
  });
  const links = await getBotExternalLinks(botId);
  for (const lnk of links) {
    if (lnk.link && lnk.link_name) kb.url(lnk.link_name, lnk.link).row();
  }
  const botUsername = ctx.me.username ?? "";
  if (botUsername) kb.url("✅ Tekshirish", `https://t.me/${botUsername}?start=verify`);

  await ctx.reply(
    "⚠️ Botdan foydalanish uchun quyida ko'rsatilgan kanallarga obuna bo'ling " +
    "va ✅ Tekshirish tugmasini bosing.",
    { reply_markup: kb }
  );
  return false;
}

// ─── Bot qurilishi ────────────────────────────────────────────────────────────

async function buildChildBot(token: string, ownerId: number): Promise<Bot<MyContext>> {
  const bot = new Bot<MyContext>(token);

  // 1. Session — har bir bot alohida namespace da (kaliti: botId:chatId)
  bot.use(createSession());

  // 2. Majburiy obuna middleware — callback query va /start ga tegmaydi
  bot.use(async (ctx, next) => {
    if (!ctx.from) return next();
    // Faqat private chatlarda obuna tekshiruvi ishlaydi.
    // chat_join_request / chat_member kabi kanal eventlarida ctx.chat = kanal bo'ladi,
    // shuning uchun bu yerda return next() — aks holda bot kanalga xabar yuborib qo'yadi.
    if (ctx.chat?.type !== "private") return next();
    const userId = ctx.from.id;
    const botId  = ctx.me.id;

    // Admin va egalar har doim o'tadi
    if (await isChildAdmin(botId, userId)) return next();

    // Barcha buyruqlar (/start, /cancel ...) o'tadi
    if (ctx.message?.text?.startsWith("/")) return next();

    // Callback query (inline tugmalar) o'tadi — player, player:start va boshqalar
    if (ctx.callbackQuery) return next();

    // Inline query o'tadi
    if (ctx.inlineQuery) return next();

    // Foydalanuvchi telefon kiritmaganmi? — onboarding hal qiladi
    let user: Awaited<ReturnType<typeof getUser>>;
    try {
      user = await getUser(userId);
    } catch {
      return next();
    }
    if (!user || !user.user_phone) return next();

    // Majburiy obuna tekshiruvi
    try {
      const passed = await childShowGate(ctx);
      if (!passed) return;
    } catch (e) {
      console.error("childShowGate error:", e);
    }
    return next();
  });

  // 3. Deep-link orqali to'g'ridan-to'g'ri yuklab olish
  setupDeepLinkDownload(bot);
  // 3b. Deep link player (season/episode share linki)
  setupDeepLinkPlayer(bot);

  // 4. Onboarding (/start + kontakt)
  setupOnboarding(bot, async (ctx) => {
    if (!ctx.from) return;
    ctx.session.state = "idle";
    if (ctx.from.id === ownerId) {
      await ctx.reply(
        "Salom! 🎌\n\nBu sizning shaxsiy anime botingiz.",
        { parse_mode: "HTML", reply_markup: ownerMainKeyboard() }
      );
    } else {
      await ctx.reply(
        "Salom! 🎌\n\nAnime qidirish uchun pastdagi tugmalardan foydalaning.",
        { reply_markup: userMainKeyboard() }
      );
    }
  });

  // 5. "🛡 Admin paneli" — faqat egaga
  bot.hears(BTN_ADMIN_PANEL, async (ctx) => {
    if (!ctx.from || ctx.from.id !== ownerId) return;
    ctx.session.state = "idle";
    await ctx.reply("Admin panel ochildi 👇", { reply_markup: childAdminMainKeyboard() });
  });

  // 6. "🏠 Foydalanuvchi paneli" — admin paneldan orqaga qaytish
  bot.hears(BTN_USER_PANEL, async (ctx) => {
    if (!ctx.from) return;
    ctx.session.state = "idle";
    const kb = ctx.from.id === ownerId ? ownerMainKeyboard() : userMainKeyboard();
    await ctx.reply("Foydalanuvchi paneli 👇", { reply_markup: kb });
  });

  // 7. "🤖 Botlarni boshqarish" — asosiy botga yo'naltiradi
  bot.hears(BTN_BOT_MANAGE, async (ctx) => {
    await ctx.reply(
      "🤖 Bot yaratish funksiyasi faqat asosiy botda mavjud.\n\n" +
      "Asosiy botga o'ting: @aniraxuzbot"
    );
  });

  // 8. /cancel buyrug'i
  bot.command("cancel", async (ctx) => {
    ctx.session.state = "idle";
    const kb = ctx.from?.id === ownerId ? ownerMainKeyboard() : userMainKeyboard();
    await ctx.reply("✅ Bekor qilindi.", { reply_markup: kb });
  });

  // 9. Majburiy obunalar boshqaruvi (child bot admin checker bilan)
  setupChannels(bot, {
    keyboard: childAdminMainKeyboard,
    adminChecker: (ctx) => isChildAdmin(ctx.me.id, ctx.from?.id),
  });

  // 10. Sub-admin boshqaruvi
  setupChildAdminHandlers(bot, childAdminMainKeyboard);

  // 11. Qidiruv tizimi
  setupSearch(bot);

  // 12. Player (asosiy funksiya)
  setupPlayer(bot);

  // 13. Izohlar

  // 14. Inline query (ulashish + qidirish)
  setupShare(bot);

  // 14. Debug buyrug'i
  bot.command("test", async (ctx) => {
    try {
      const userId  = ctx.from?.id ?? 0;
      const botId   = ctx.me.id;
      const botName = ctx.me.first_name;
      const channels = await getBotChannels(botId).catch(() => []);
      const user     = await getUser(userId).catch(() => null);
      const admin    = await isChildAdmin(botId, userId).catch(() => false);
      await ctx.reply(
        `✅ Bot ishlayapti!\n\n` +
        `🤖 Bot: ${botName} (${botId})\n` +
        `👤 User: ${userId}\n` +
        `🛡 Admin: ${admin ? "ha" : "yo'q"}\n` +
        `📞 Telefon: ${user?.user_phone || "yo'q"}\n` +
        `🔐 Kanallar: ${channels.length} ta\n` +
        `📊 Session holat: ${ctx.session.state}`
      );
    } catch (e) {
      await ctx.reply(`❌ Test xatosi: ${String(e)}`);
    }
  });

  bot.catch((err) => {
    console.error("Child bot xatosi:", err.message, err.error);
    if (err.ctx?.session) err.ctx.session.state = "idle";
  });

  await bot.init();
  return bot;
}

// ─── Webhook handler ──────────────────────────────────────────────────────────

export async function handleChildUpdate(botId: number, update: unknown): Promise<boolean> {
  let bot = childCache.get(botId);
  if (!bot) {
    const rec = await getKonstruktorByBotId(botId);
    if (!rec || !rec.bot_token) return false;
    bot = await buildChildBot(rec.bot_token, rec.user_id);
    childCache.set(botId, bot);
  }
  await bot.handleUpdate(update as any);
  return true;
}
