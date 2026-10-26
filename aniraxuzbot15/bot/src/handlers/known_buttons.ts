/**
 * Reply-keyboard tugma matnlari va commandlarning yagona ro'yxati.
 * State-based message handlerlar (channels.ts state, konstruktor.ts token-input)
 * ushbu matnlardan birini ko'rsa o'z statelarini tozalab next() ga yo'l beradi —
 * shunda foydalanuvchi qaysi tugmani bossa bot o'sha tugmaga javob beradi.
 */
export const KNOWN_REPLY_BUTTONS = new Set<string>([
  // ── Foydalanuvchi paneli ───────────────────────────────────────────────────
  "🔍 Anime izlash",
  "💰 Balans to'ldirish",
  "💳 Hisobim",
  "💎 Obuna olish",
  // ── Konstruktor paneli ────────────────────────────────────────────────────
  "🤖 Botlarni boshqarish",
  "🛠 Anime bot yaratish",
  "📋 Mening botlarim",
  "🏠 Foydalanuvchi paneli",
  "⟨ Orqaga",
  // ── Admin asosiy paneli (BARCHASI) ────────────────────────────────────────
  "📹 Animelarni boshqarish",
  "📢 Asosiy Kanal",
  "🔐 Majburiy obunalar",
  "🔍 Foydalanuvchi boshqaruvi",
  "🗄 Database",
  "👤 Adminlar",
  "✉️ Xabar yuborish",
  // ── Child-admin paneli ────────────────────────────────────────────────────
  "👨‍💻 Admin",
  // ── Kanal tanlash ─────────────────────────────────────────────────────────
  "🤖 Botni kanalga admin qilish",
  "🆔 Kanal IDsi botga yuborish",
]);

/**
 * Matn rezervlangan bo'lsa true qaytaradi:
 *  - "/" bilan boshlanadigan har qanday Telegram command
 *  - Bot tomonidan tanigan reply-keyboard tugma matni
 */
export function isReservedText(text: string | undefined | null): boolean {
  if (!text) return false;
  if (text.startsWith("/")) return true;
  return KNOWN_REPLY_BUTTONS.has(text);
}
