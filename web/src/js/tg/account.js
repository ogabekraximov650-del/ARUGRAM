// Profil -> "Telegram'ni ulash" (ilovada `PhoneLoginScreen(connectOnly: true)`):
// videolar uchun foydalanuvchining O'Z Telegram hisobi ulanadi.
import { ensureTelegram, isTelegramAuthorized, checkTelegram } from './media.js';

export async function openTelegramAccount() {
  return ensureTelegram();
}

/** Profil qatori ko'rinsinmi (ulanmagan bo'lsa). */
export { isTelegramAuthorized, checkTelegram };
