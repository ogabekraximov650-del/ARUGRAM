import type { InlineKeyboard } from "grammy";
import type { MyContext } from "./admin.ts";
import { splitFileIds } from "../db.ts";

/**
 * `file_id` maydoni vergul bilan bir nechta qism bo'lishi mumkin (2 GB dan katta video aniq o'lchab bo'lingan).
 * Qismlar ketma-ket yuboriladi: oxirgisi izoh va klaviatura bilan, oldingilari "📦 i/n-qism" izohi bilan.
 * Har qism uchun avval `primary`, bo'lmasa `fallback` id. Oxirgi xabar id qaytadi; `sent` berilsa — yuborilgan HAMMA xabar id'lari
 * (qismlar tartibida) shu massivga yig'iladi (keyin ularni o'chirish uchun).
 */
export async function sendParts(
  ctx: MyContext,
  primaryRaw: string | null,
  fallbackRaw: string | null,
  caption: string,
  kb?: InlineKeyboard,
  sent: number[] = []
): Promise<number> {
  const prim = splitFileIds(primaryRaw);
  const fall = splitFileIds(fallbackRaw);
  const n = Math.max(prim.length, fall.length);
  let last = 0;
  for (let i = 0; i < n; i++) {
    const isLast = i === n - 1;
    const opts = isLast
      ? { caption, reply_markup: kb, protect_content: true }
      : { caption: `📦 ${i + 1}/${n}-qism`, protect_content: true };
    const a = (prim[i] ?? fall[i])!;
    const b = (fall[i] ?? prim[i])!;
    try {
      last = (await ctx.api.sendVideo(ctx.chat!.id, a, opts)).message_id;
    } catch (e1) {
      console.error(`sendParts primary failed (qism ${i + 1}/${n}):`, String(e1));
      if (b && b !== a) last = (await ctx.api.sendVideo(ctx.chat!.id, b, opts)).message_id;
      else throw e1;
    }
    sent.push(last);
  }
  return last;
}

