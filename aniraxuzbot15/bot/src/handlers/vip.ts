/**
 * VIP obuna tizimi
 * - 💎 Obuna olish tugmasi
 * - 1-bosqich: VIP ma'lumotlari + "Tarif tanlash" tugmasi
 * - 2-bosqich: 5 ta tarif tugmasi + "Orqaga qaytish" tugmasi
 * - Faol obuna bo'lsa: sotib olish vaqti, tugash vaqti, qolgan vaqt ko'rsatiladi
 */
import { Bot, InlineKeyboard } from "grammy";
import type { MyContext } from "./admin.ts";
import { getUser } from "../db_users.ts";
import { d1Query } from "../db.ts";
import { formatTashkent } from "../date_utils.ts";
import { SQL_TASHKENT_NOW } from "../date_utils.ts";

function styleAll(kb: InlineKeyboard): InlineKeyboard {
  for (const row of kb.inline_keyboard)
    for (const btn of row) (btn as any).style = "primary";
  return kb;
}

// ─── Tariflar ──────────────────────────────────────────────────────────────────
interface VipTariff {
  days: number;
  price: number;
  label: string;
  pricePerDay: number;
}

const VIP_TARIFFS: VipTariff[] = [
  { days: 1,  price: 1000,  label: "1 kunlik",  pricePerDay: 1000 },
  { days: 5,  price: 4000,  label: "5 kunlik",  pricePerDay: 800  },
  { days: 10, price: 7000,  label: "10 kunlik", pricePerDay: 700  },
  { days: 20, price: 12000, label: "20 kunlik", pricePerDay: 600  },
  { days: 30, price: 15000, label: "30 kunlik", pricePerDay: 500  },
];

// ─── VIP holati ────────────────────────────────────────────────────────────────

export interface VipRecord {
  id: number;
  user_id: number;
  duration_days: number;
  purchased_at: string;
  expires_at: string;
  status: string;
  created_at: string;
}

export async function getActiveVip(userId: number): Promise<VipRecord | null> {
  const now = formatTashkent();
  const rows = await d1Query<VipRecord>(
    `SELECT * FROM vip_user WHERE user_id = ? AND status = 'active' AND expires_at > ? ORDER BY expires_at DESC LIMIT 1`,
    [userId, now]
  );
  return rows[0] ?? null;
}

async function insertVip(userId: number, days: number): Promise<void> {
  const now = formatTashkent();
  const expiresDate = new Date(Date.now() + days * 24 * 60 * 60 * 1000);
  const expiresAt = formatTashkent(expiresDate);

  await d1Query(
    `INSERT INTO vip_user (user_id, duration_days, purchased_at, expires_at, status, created_at)
     VALUES (?, ?, ?, ?, 'active', ?)`,
    [userId, days, now, expiresAt, now]
  );
}

// ─── Vaqt yordamchi funksiyalari ───────────────────────────────────────────────

/**
 * "YYYY-MM-DD HH:MM:SS" (Toshkent UTC+5) → Date (UTC)
 */
function parseTashkentDate(dateStr: string): Date {
  const [datePart, timePart] = dateStr.split(" ");
  const [year, month, day] = datePart.split("-").map(Number);
  const [hour, minute, second] = timePart.split(":").map(Number);
  // Toshkent vaqti = UTC+5, shuning uchun UTC ga o'tkazamiz
  return new Date(Date.UTC(year, month - 1, day, hour - 5, minute, second));
}

/**
 * "YYYY-MM-DD HH:MM:SS" → "YYYY-MM-DD.HH:MM:SS"
 */
function formatDateTimeUz(dateStr: string): string {
  return dateStr.replace(" ", ".");
}

/**
 * Tugash sanasiga qadar qolgan vaqtni yil/oy/kun/soat/daqiqa/soniya ko'rinishida qaytaradi
 */
function getTimeRemaining(expiresAt: string): string {
  const now = new Date();
  const expiry = parseTashkentDate(expiresAt);
  const diffMs = expiry.getTime() - now.getTime();

  if (diffMs <= 0) return "Obuna muddati tugagan";

  const totalSeconds = Math.floor(diffMs / 1000);
  const seconds = totalSeconds % 60;
  const totalMinutes = Math.floor(totalSeconds / 60);
  const minutes = totalMinutes % 60;
  const totalHours = Math.floor(totalMinutes / 60);
  const hours = totalHours % 24;
  const totalDays = Math.floor(totalHours / 24);
  const years = Math.floor(totalDays / 365);
  const remAfterYears = totalDays % 365;
  const months = Math.floor(remAfterYears / 30);
  const days = remAfterYears % 30;

  const parts: string[] = [];
  if (years > 0)   parts.push(`${years} yil`);
  if (months > 0)  parts.push(`${months} oy`);
  if (days > 0)    parts.push(`${days} kun`);
  if (hours > 0)   parts.push(`${hours} soat`);
  if (minutes > 0) parts.push(`${minutes} daqiqa`);
  parts.push(`${seconds} soniya`);

  return parts.join(", ");
}

// ─── 1-bosqich: VIP ma'lumotlari ──────────────────────────────────────────────

async function showVipInfo(ctx: MyContext): Promise<void> {
  const userId = ctx.from!.id;
  const activeVip = await getActiveVip(userId);

  if (activeVip) {
    // Faol obuna bor — vaqt ma'lumotlarini ko'rsat, sotib olishni to'sib qo'y
    const remaining = getTimeRemaining(activeVip.expires_at);
    const text =
      `💎 <b>Sizning VIP obunangiz</b>\n\n` +
      `✅ <b>Obuna faol!</b>\n\n` +
      `🛒 Sotib olingan vaqt:\n` +
      `<b>${formatDateTimeUz(activeVip.purchased_at)}</b>\n\n` +
      `⏰ Tugash vaqti:\n` +
      `<b>${formatDateTimeUz(activeVip.expires_at)}</b>\n\n` +
      `⏳ <b>Qolgan vaqt:</b>\n` +
      `${remaining}`;

    const kb = styleAll(new InlineKeyboard().text("« Orqaga qaytish", "vip:back"));

    if (ctx.callbackQuery) {
      await ctx.editMessageText(text, { parse_mode: "HTML", reply_markup: kb })
        .catch(() => ctx.reply(text, { parse_mode: "HTML", reply_markup: kb }));
      await ctx.answerCallbackQuery();
    } else {
      await ctx.reply(text, { parse_mode: "HTML", reply_markup: kb });
    }
    return;
  }

  // Faol obuna yo'q — 1-bosqich: ma'lumot ko'rsat
  const text =
    `💎 <b>VIP Obuna</b>\n\n` +
    `VIP obuna afzalliklari:\n` +
    `• 🎌 Cheksiz anime ko'rish\n` +
    `• 🎥 Yuqori sifatli videolar\n` +
    `• ⚡ Tezkor yuklash\n` +
    `• 🔓 Barcha qismlar ochiq\n\n` +
    `📋 <b>Tariflar:</b>\n` +
    `• 1 kunlik  — 1 000 so'm  (1 000/kun)\n` +
    `• 5 kunlik  — 4 000 so'm  (800/kun)\n` +
    `• 10 kunlik — 7 000 so'm  (700/kun)\n` +
    `• 20 kunlik — 12 000 so'm (600/kun)\n` +
    `• 30 kunlik — 15 000 so'm (500/kun)`;

  const kb = styleAll(new InlineKeyboard()
    .text("💎 Tarif tanlash", "vip:list").row()
    .text("« Orqaga qaytish", "vip:back")
  );

  if (ctx.callbackQuery) {
    await ctx.editMessageText(text, { parse_mode: "HTML", reply_markup: kb })
      .catch(() => ctx.reply(text, { parse_mode: "HTML", reply_markup: kb }));
    await ctx.answerCallbackQuery();
  } else {
    await ctx.reply(text, { parse_mode: "HTML", reply_markup: kb });
  }
}

// ─── 2-bosqich: Tarif tugmalari ───────────────────────────────────────────────

async function showTariffList(ctx: MyContext): Promise<void> {
  const kb = new InlineKeyboard();
  for (const t of VIP_TARIFFS) {
    kb.text(`${t.label} — ${t.price.toLocaleString("uz")} so'm`, `vip:select:${t.days}`).row();
  }
  kb.text("« Orqaga qaytish", "vip:info");
  styleAll(kb);

  const text = `💎 <b>Quyidagi tariflardan birini tanlang:</b>`;

  if (ctx.callbackQuery) {
    await ctx.editMessageText(text, { parse_mode: "HTML", reply_markup: kb })
      .catch(() => ctx.reply(text, { parse_mode: "HTML", reply_markup: kb }));
    await ctx.answerCallbackQuery();
  } else {
    await ctx.reply(text, { parse_mode: "HTML", reply_markup: kb });
  }
}

// ─── Tarif tafsiloti ───────────────────────────────────────────────────────────

async function showTariffDetail(ctx: MyContext, days: number): Promise<void> {
  const tariff = VIP_TARIFFS.find((t) => t.days === days);
  if (!tariff) {
    await ctx.answerCallbackQuery("❌ Tarif topilmadi").catch(() => {});
    return;
  }

  const userId = ctx.from!.id;
  const user = await getUser(userId);
  const mainBalance = user?.balance ?? 0;
  const animeBalance = user?.anime_balance ?? 0;
  const totalBalance = mainBalance + animeBalance;

  const kb = styleAll(
    new InlineKeyboard()
      .text("✅ Obuna bo'lish", `vip:buy:${days}`).row()
      .text("« Orqaga", "vip:list")
  );

  await ctx.editMessageText(
    `💎 <b>${tariff.label} VIP obuna</b>\n\n` +
    `📅 Muddat: <b>${tariff.days} kun</b>\n` +
    `💰 Narx: <b>${tariff.price.toLocaleString("uz")} so'm</b>\n` +
    `📊 Kunlik narx: <b>${tariff.pricePerDay.toLocaleString("uz")} so'm/kun</b>\n\n` +
    `💵 Sizning balansingiz:\n` +
    `  • Asosiy: ${mainBalance.toLocaleString("uz")} so'm\n` +
    `  • Anime: ${animeBalance.toLocaleString("uz")} so'm\n` +
    `  • Jami: <b>${totalBalance.toLocaleString("uz")} so'm</b>\n\n` +
    (totalBalance >= tariff.price
      ? `✅ Balans yetarli!`
      : `❌ Balans yetarli emas! Yetishmaydi: <b>${(tariff.price - totalBalance).toLocaleString("uz")} so'm</b>`),
    { parse_mode: "HTML", reply_markup: kb }
  ).catch(() =>
    ctx.reply(
      `${tariff.label} — ${tariff.price.toLocaleString("uz")} so'm\nBalans: ${totalBalance.toLocaleString("uz")} so'm`,
      { parse_mode: "HTML", reply_markup: kb }
    )
  );
  await ctx.answerCallbackQuery();
}

// ─── Obuna sotib olish ────────────────────────────────────────────────────────

async function buyVip(ctx: MyContext, days: number): Promise<void> {
  const tariff = VIP_TARIFFS.find((t) => t.days === days);
  if (!tariff) {
    await ctx.answerCallbackQuery("❌ Tarif topilmadi").catch(() => {});
    return;
  }

  const userId = ctx.from!.id;

  // Faol obuna tekshiruvi
  const activeVip = await getActiveVip(userId);
  if (activeVip) {
    await ctx.answerCallbackQuery("❌ Faol obunangiz mavjud!").catch(() => {});
    await showVipInfo(ctx);
    return;
  }

  const user = await getUser(userId);
  const mainBalance = user?.balance ?? 0;
  const animeBalance = user?.anime_balance ?? 0;
  const totalBalance = mainBalance + animeBalance;

  if (totalBalance < tariff.price) {
    await ctx.answerCallbackQuery("❌ Balans yetarli emas!").catch(() => {});
    const kb = styleAll(
      new InlineKeyboard()
        .text("💰 Balans to'ldirish", "pay:topup").row()
        .text("« Orqaga", `vip:select:${days}`)
    );
    await ctx.editMessageText(
      `❌ <b>Balans yetarli emas!</b>\n\n` +
      `Tarif narxi: ${tariff.price.toLocaleString("uz")} so'm\n` +
      `Sizning balansingiz: ${totalBalance.toLocaleString("uz")} so'm\n` +
      `Yetishmaydi: <b>${(tariff.price - totalBalance).toLocaleString("uz")} so'm</b>`,
      { parse_mode: "HTML", reply_markup: kb }
    ).catch(() => {});
    return;
  }

  // Balansdan ayirish (avval anime_balance, keyin asosiy)
  const animeDeduct = Math.min(animeBalance, tariff.price);
  const mainDeduct = tariff.price - animeDeduct;

  if (mainDeduct > 0) {
    await d1Query(
      `UPDATE user_db SET balance = MAX(0, balance - ?) WHERE user_id = ?`,
      [mainDeduct, userId]
    );
  }
  if (animeDeduct > 0) {
    await d1Query(
      `UPDATE user_db SET anime_balance = MAX(0, anime_balance - ?) WHERE user_id = ?`,
      [animeDeduct, userId]
    );
  }

  // VIP yozuvini qo'shish
  await insertVip(userId, tariff.days);

  await ctx.answerCallbackQuery("✅ VIP obuna faollashtirildi!").catch(() => {});
  await ctx.editMessageText(
    `✅ <b>VIP obuna muvaffaqiyatli sotib olindi!</b>\n\n` +
    `💎 Tarif: <b>${tariff.label}</b>\n` +
    `📅 Muddat: <b>${tariff.days} kun</b>\n` +
    `💸 To'langan: <b>${tariff.price.toLocaleString("uz")} so'm</b>\n\n` +
    `VIP obuna faollashtirildi! Yaxshi tomosha qiling 🎌`,
    { parse_mode: "HTML" }
  ).catch(() => {});
}

// ─── setupVip ──────────────────────────────────────────────────────────────────

export function setupVip(bot: Bot<MyContext>): void {

  // Tugma: 💎 Obuna olish → 1-bosqich (ma'lumot + "Tarif tanlash")
  bot.hears("💎 Obuna olish", async (ctx) => {
    await showVipInfo(ctx);
  });

  // "vip:info" → 1-bosqichga qaytish
  bot.callbackQuery("vip:info", async (ctx) => {
    await showVipInfo(ctx);
  });

  // "Tarif tanlash" bosildi → 2-bosqich (tarif tugmalari)
  bot.callbackQuery("vip:list", async (ctx) => {
    await showTariffList(ctx);
  });

  // Tarif tanlash (tarif tafsiloti)
  bot.callbackQuery(/^vip:select:(\d+)$/, async (ctx) => {
    const days = Number(ctx.match[1]);
    await showTariffDetail(ctx, days);
  });

  // Obuna sotib olish
  bot.callbackQuery(/^vip:buy:(\d+)$/, async (ctx) => {
    const days = Number(ctx.match[1]);
    await buyVip(ctx, days);
  });

  // Orqaga qaytish (asosiy menyuga)
  bot.callbackQuery("vip:back", async (ctx) => {
    await ctx.answerCallbackQuery();
    try { await ctx.deleteMessage(); } catch {}
  });
}
