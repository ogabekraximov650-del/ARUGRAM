import { Keyboard } from "grammy";
import type { MyContext } from "./admin.ts";

export function mainMenuKeyboard(): Keyboard {
  return new Keyboard()
    .text("🔍 Anime izlash").row()
    .text("💰 Balans to'ldirish").text("💳 Hisobim").row()
    .text("💎 Obuna olish").text("📊 Statistika")
    .resized();
}

export async function sendMainMenu(ctx: MyContext): Promise<void> {
  await ctx.reply("Botdan foydalanishingiz mumkin! 👇", {
    reply_markup: mainMenuKeyboard(),
  });
}
