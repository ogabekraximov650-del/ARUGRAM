/**
 * Admin: Database statistikasi — Cloudflare D1 × 3 baza.
 *
 * "🗄 Database" tugmasi bosilganda statistika HAR SAFAR jonli (live) qayta
 * hisoblanadi. D1 da SQL dbstat yo'q — hajm qator soniga ko'ra taxminiy (~500 B/row).
 */
import { Bot, InlineKeyboard, InputFile } from "grammy";
import type { MyContext } from "./admin.ts";
import { checkIsAdmin } from "../admin_check.ts";
import { getD1Stats, pingD1, getAllD1TablesData } from "../db_d1.ts";
import { formatTashkent } from "../date_utils.ts";

// D1 bepul cheklov: har bir baza 10 GB
const D1_LIMIT_BYTES = 10 * 1024 * 1024 * 1024;

function isAdmin(ctx: MyContext): boolean {
  return checkIsAdmin(ctx.from?.id);
}

/** Qatorlar soniga qarab taxminiy hajm (D1 da SQL orqali aniq hajm yo'q) */
function estimateBytes(rows: number): number {
  // O'rtacha bir qator ~500 bayt
  return rows * 500;
}

/** Baytni inson o'qiy oladigan ko'rinishga o'tkazadi */
function fmtSize(bytes: number): string {
  if (bytes >= 1024 * 1024 * 1024) return `${(bytes / (1024 ** 3)).toFixed(2)} GB`;
  if (bytes >= 1024 * 1024) return `${(bytes / (1024 ** 2)).toFixed(1)} MB`;
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${bytes} B`;
}

/** 0–10 blokli progress bar */
function progressBar(fraction: number): string {
  const n = Math.min(10, Math.round(Math.max(0, fraction) * 10));
  return "█".repeat(n) + "░".repeat(10 - n);
}

/** Har bosilganda QAYTA hisoblangan D1 statistikasi. */
async function buildStatsText(): Promise<string> {
  const [ping, stats] = await Promise.all([pingD1(), getD1Stats()]);

  const allDbs = [
    { label: "🎬 DB_ANIME", ok: ping.anime, data: stats.anime },
    { label: "📊 DB_STAT",  ok: ping.stat,  data: stats.stat  },
    { label: "🗄 DB_MAIN",  ok: ping.main,  data: stats.main  },
  ];

  let text = `📊 <b>Database statistikasi (Cloudflare D1 × 3)</b>\n🕒 ${formatTashkent()}\n\n`;

  // ── Jami 3 baza birlashgan statistika ─────────────────────────────────────
  const grandTotalRows  = stats.anime.totalRows + stats.stat.totalRows + stats.main.totalRows;
  const grandTotalBytes = estimateBytes(grandTotalRows);
  const grandLimit      = 3 * D1_LIMIT_BYTES; // 30 GB jami
  const grandPct        = grandTotalBytes / grandLimit;

  text += `<b>📦 Jami 3 baza birlashgan:</b>\n`;
  text += `📝 Jami: <b>${grandTotalRows.toLocaleString()}</b> qator\n`;
  text += `📦 ~${fmtSize(grandTotalBytes)} / 30 GB\n`;
  text += `<code>${progressBar(grandPct)}</code> ${(grandPct * 100).toFixed(3)}%\n\n`;

  // ── Har bir baza alohida ───────────────────────────────────────────────────
  for (const db of allDbs) {
    const totalRows = db.data.totalRows;
    const estBytes  = estimateBytes(totalRows);
    const pct       = estBytes / D1_LIMIT_BYTES;

    text += `${db.label} ${db.ok ? "✅" : "🔴"}\n`;
    text += `📝 Jami: <b>${totalRows.toLocaleString()}</b> qator\n`;
    text += `📦 ~${fmtSize(estBytes)} / 10 GB\n`;
    text += `<code>${progressBar(pct)}</code> ${(pct * 100).toFixed(3)}%\n`;

    for (const t of db.data.tables) {
      if (t.rows > 0) {
        text += `  <code>${t.table}</code>: ${t.rows.toLocaleString()} qator\n`;
      }
    }
    text += "\n";
  }

  return text.length > 4000 ? text.slice(0, 3990) + "\n<i>...qisqartirildi</i>" : text;
}

function statsKeyboard(): InlineKeyboard {
  return new InlineKeyboard()
    .text("📥 JSON yuklab olish", "dbexport:json")
    .text("📥 SQL yuklab olish", "dbexport:sql")
    .row()
    .text("🔄 Yangilash", "dbexport:refresh");
}

/** SQL qiymatini xavfsiz literal ko'rinishga o'tkazadi. */
function sqlLiteral(v: unknown): string {
  if (v === null || v === undefined) return "NULL";
  if (typeof v === "number") return Number.isFinite(v) ? String(v) : "NULL";
  if (typeof v === "boolean") return v ? "1" : "0";
  return `'${String(v).replace(/'/g, "''")}'`;
}

/** Barcha jadvallar ma'lumotlarini .sql (INSERT statements) ko'rinishida quradi. */
function buildSqlDump(data: Record<string, Record<string, unknown>[]>): string {
  const lines: string[] = [
    `-- D1 ma'lumotlar bazasi eksporti`,
    `-- Yaratilgan: ${formatTashkent()}`,
    ``,
  ];
  for (const [table, rows] of Object.entries(data)) {
    if (rows.length === 0) continue;
    lines.push(`-- Jadval: ${table} (${rows.length} qator)`);
    const columns = Object.keys(rows[0]);
    for (const row of rows) {
      const values = columns.map(c => sqlLiteral(row[c]));
      lines.push(`INSERT INTO "${table}" (${columns.map(c => `"${c}"`).join(", ")}) VALUES (${values.join(", ")});`);
    }
    lines.push("");
  }
  return lines.join("\n");
}

// ─── Handlers ────────────────────────────────────────────────────────────────

export function setupDatabaseAdmin(bot: Bot<MyContext>): void {
  // ── "🗄 Database" tugmasi ──────────────────────────────────────────────────
  bot.hears("🗄 Database", async (ctx) => {
    if (!isAdmin(ctx)) return;
    ctx.session.state = "idle";
    const msg = await ctx.reply("⏳ Statistika yuklanmoqda...");
    try {
      const text = await buildStatsText();
      await ctx.api
        .editMessageText(ctx.chat!.id, msg.message_id, text, { parse_mode: "HTML", reply_markup: statsKeyboard() })
        .catch(() => ctx.reply(text, { parse_mode: "HTML", reply_markup: statsKeyboard() }));
    } catch (e) {
      await ctx.api
        .editMessageText(
          ctx.chat!.id, msg.message_id,
          `❌ Xato: <code>${String(e).slice(0, 300)}</code>`,
          { parse_mode: "HTML" }
        ).catch(() => ctx.reply(`❌ Xato: ${String(e).slice(0, 300)}`).catch(() => {}));
    }
  });

  // ── /dbstats — alias ───────────────────────────────────────────────────────
  bot.command("dbstats", async (ctx) => {
    if (!isAdmin(ctx)) return;
    const msg = await ctx.reply("⏳ Statistika yuklanmoqda...");
    try {
      const text = await buildStatsText();
      await ctx.api
        .editMessageText(ctx.chat!.id, msg.message_id, text, { parse_mode: "HTML", reply_markup: statsKeyboard() })
        .catch(() => ctx.reply(text, { parse_mode: "HTML", reply_markup: statsKeyboard() }));
    } catch (e) {
      await ctx.api
        .editMessageText(
          ctx.chat!.id, msg.message_id,
          `❌ Xato: <code>${String(e).slice(0, 300)}</code>`,
          { parse_mode: "HTML" }
        ).catch(() => ctx.reply(`❌ Xato: ${String(e).slice(0, 300)}`).catch(() => {}));
    }
  });

  // ── "🔄 Yangilash" tugmasi — statistikani qayta hisoblab yangilaydi ────────
  bot.callbackQuery("dbexport:refresh", async (ctx) => {
    if (!isAdmin(ctx)) return ctx.answerCallbackQuery();
    await ctx.answerCallbackQuery("Yangilanmoqda...");
    try {
      const text = await buildStatsText();
      await ctx.editMessageText(text, { parse_mode: "HTML", reply_markup: statsKeyboard() }).catch(() => {});
    } catch (e) {
      await ctx.answerCallbackQuery({ text: `Xato: ${String(e).slice(0, 150)}`, show_alert: true }).catch(() => {});
    }
  });

  // ── "📥 JSON yuklab olish" ──────────────────────────────────────────────────
  bot.callbackQuery("dbexport:json", async (ctx) => {
    if (!isAdmin(ctx)) return ctx.answerCallbackQuery();
    await ctx.answerCallbackQuery("Tayyorlanmoqda...");
    try {
      const data = await getAllD1TablesData();
      const json = JSON.stringify(data, null, 2);
      const fileName = `d1_export_${formatTashkent().replace(/[: ]/g, "-")}.json`;
      await ctx.replyWithDocument(
        new InputFile(new TextEncoder().encode(json), fileName),
        { caption: `📥 JSON eksport — ${formatTashkent()}` }
      );
    } catch (e) {
      await ctx.reply(`❌ JSON eksport xatosi: ${String(e).slice(0, 300)}`).catch(() => {});
    }
  });

  // ── "📥 SQL yuklab olish" ───────────────────────────────────────────────────
  bot.callbackQuery("dbexport:sql", async (ctx) => {
    if (!isAdmin(ctx)) return ctx.answerCallbackQuery();
    await ctx.answerCallbackQuery("Tayyorlanmoqda...");
    try {
      const data = await getAllD1TablesData();
      const sql = buildSqlDump(data);
      const fileName = `d1_export_${formatTashkent().replace(/[: ]/g, "-")}.sql`;
      await ctx.replyWithDocument(
        new InputFile(new TextEncoder().encode(sql), fileName),
        { caption: `📥 SQL eksport — ${formatTashkent()}` }
      );
    } catch (e) {
      await ctx.reply(`❌ SQL eksport xatosi: ${String(e).slice(0, 300)}`).catch(() => {});
    }
  });
}
