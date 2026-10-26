import { Bot, InlineKeyboard, Keyboard } from "grammy";
import type { MyContext } from "./admin.ts";
import {
  getBotChannels,
  getBotExternalLinks,
  hasJoinRequest,
  type ChannelInfo,
  type ExternalLink,
} from "../db_channels.ts";
import { getUser, upsertUserPhone, touchLastBot } from "../db_users.ts";

type OnComplete = (ctx: MyContext) => Promise<void>;

function styleAll(kb: InlineKeyboard): InlineKeyboard {
  for (const row of kb.inline_keyboard) {
    for (const btn of row) (btn as any).style = "primary";
  }
  return kb;
}

async function checkSubscribed(
  ctx: MyContext,
  ch: ChannelInfo,
  userId: number
): Promise<boolean> {
  // Ommaviy kanal: faqat getChatMember orqali
  // Yopiq kanal: getChatMember + so'rov yuborganmi (request_chanel_db)
  try {
    const m = await ctx.api.getChatMember(ch.chanel_id, userId);
    if (["creator", "administrator", "member", "restricted"].includes(m.status)) {
      return true;
    }
  } catch {
    /* bot kanalga kira olmasa — obuna emas deb hisoblanadi */
  }
  if (ch.chanel_type === "private") {
    if (await hasJoinRequest(ctx.me.id, ch.chanel_id, userId)) return true;
  }
  return false;
}

/**
 * Foydalanuvchi barcha majburiy kanallarga obuna bo'lgan bo'lsa true qaytaradi.
 * Aks holda obuna oynasini chiqaradi:
 *   [1 - kanal] [2 - kanal] ...   ← URL tugmalar
 *   [Instagram] [YouTube] ...     ← faqat kanallar mavjud bo'lsa pastda
 *   [✅ Tekshirish]                ← bot deep-link (?start=verify)
 *
 * MUHIM: bu yerda hech qanday hisoblagich/log yozilmaydi —
 * sof verifikatsiya. Hisoblagichlar chat_join_request va chat_member
 * eventlarida (channels.ts) yangilanadi.
 */
async function showSubscriptionGate(ctx: MyContext): Promise<boolean> {
  const botId = ctx.me.id;
  const userId = ctx.from!.id;

  // channels + links — PARALLEL (avval ketma-ket edi: +75ms)
  const [channels, links] = await Promise.all([
    getBotChannels(botId),
    getBotExternalLinks(botId),
  ]);

  if (channels.length === 0) return true;

  const results = await Promise.all(channels.map(async (ch) => {
    if (ch.jami_qoshish_kerak > 0 && ch.jami_qoshilgan >= ch.jami_qoshish_kerak) return true;
    return checkSubscribed(ctx, ch, userId);
  }));
  const unsub: ChannelInfo[] = channels.filter((_, i) => !results[i]);

  if (unsub.length === 0) return true;

  const kb = new InlineKeyboard();
  unsub.forEach((ch, i) => {
    let url = "";
    if (ch.chanel_type === "public" && ch.chanel_username) {
      url = `https://t.me/${ch.chanel_username.replace(/^@/, "")}`;
    } else if (ch.chanel_link) {
      url = ch.chanel_link;
    }
    if (!url) return;
    kb.url(`${i + 1} - kanal`, url).row();
  });

  for (const lnk of links) {
    if (!lnk.link || !lnk.link_name) continue;
    kb.url(lnk.link_name, lnk.link).row();
  }

  const botUsername = ctx.me.username || "";
  const verifyUrl = botUsername ? `https://t.me/${botUsername}?start=verify` : "";
  if (verifyUrl) kb.url("✅ Tekshirish", verifyUrl);

  styleAll(kb);
  await ctx.reply(
    "⚠️ Botdan foydalanish uchun quyida koʻrsatilgan kanallarga obuna boʻling " +
      "va ✅ Tekshirish tugmasini bosing.",
    { reply_markup: kb }
  );
  return false;
}

export function setupOnboarding(bot: Bot<MyContext>, onComplete: OnComplete): void {
  bot.command("start", async (ctx) => {
    if (!ctx.from) return;
    const username = ctx.from.username ? "@" + ctx.from.username : "";

    // touchLastBot + getUser — PARALLEL (avval ketma-ket edi: +75ms)
    const [, user] = await Promise.all([
      touchLastBot(ctx.from.id, ctx.me.id, username),
      getUser(ctx.from.id),
    ]);
    if (!user || !user.user_phone) {
      const kb = new Keyboard()
        .requestContact("📱 Kontaktni ulashish")
        .resized()
        .oneTime();
      await ctx.reply(
        "Assalomu alaykum hurmatli foydalanuvchi! 👋\n\n" +
          "Botdan toʻliq foydalanish uchun telefon raqamingizni yuboring. " +
          "Bu narsa soxta foydalanuvchilarni oldini olish uchun kerak.",
        { reply_markup: kb }
      );
      return;
    }

    const passed = await showSubscriptionGate(ctx);
    if (passed) await onComplete(ctx);
  });

  bot.on("message:contact", async (ctx) => {
    if (!ctx.from || !ctx.message?.contact) return;
    const c = ctx.message.contact;
    if (c.user_id !== ctx.from.id) {
      await ctx.reply("❌ Iltimos, faqat OʻZ kontaktingizni ulashing.");
      return;
    }
    const username = ctx.from.username ? "@" + ctx.from.username : "";
    await upsertUserPhone(ctx.from.id, username, c.phone_number);
    await touchLastBot(ctx.from.id, ctx.me.id, username);

    await ctx.reply("✅ Kontakt saqlandi.", {
      reply_markup: { remove_keyboard: true },
    });

    const passed = await showSubscriptionGate(ctx);
    if (passed) await onComplete(ctx);
  });
}
