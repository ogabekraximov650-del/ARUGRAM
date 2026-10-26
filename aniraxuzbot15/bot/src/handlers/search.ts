import { Bot, InlineKeyboard } from "grammy";
import type { MyContext } from "./admin.ts";
import {
  getAnimeById,
  getAnimesPaginated,
  getAnimesPopularPaginated,
  getRecentEpisodesPerAnime,
  searchAnimeByName,
  type AnimeRecord,
  type RecentEpisodeRow,
  type Kategoriya,
} from "../db.ts";
import { config } from "../config.ts";

const PAGE_SIZE = 20;

function styleAll(kb: InlineKeyboard): InlineKeyboard {
  for (const row of kb.inline_keyboard)
    for (const btn of row) (btn as any).style = "primary";
  return kb;
}

function pad2(n: number): string {
  return n < 10 ? `0${n}` : String(n);
}

// ============================================================
// Anime post matni
// ============================================================
async function buildAnimePost(anime: AnimeRecord, botUsername: string): Promise<string> {
  const link = `https://t.me/${botUsername}?start=${anime.id}`;
  return (
    `Anime nomi: ${anime.name}\n\n` +
    `${anime.info}\n\n` +
    `Botimiz: @${botUsername}\n` +
    `Anime kodi: ${anime.id}\n` +
    `Link: ${link}\n` +
    `📥 Yuklangan: ${anime.downloads} marta`
  );
}

function animePostKeyboard(anime: AnimeRecord): InlineKeyboard {
  return styleAll(new InlineKeyboard().text("• Yuklab Olish •", `player:start:${anime.id}`));
}

// ============================================================
// Navigatsiya: ‹ N-sahifa   N/N   N-sahifa ›
// ============================================================
function buildNavRow(
  kb: InlineKeyboard,
  page: number,
  totalPages: number,
  cbPrefix: string
): void {
  if (page > 1) kb.text(`‹ ${page - 1}-sahifa`, `${cbPrefix}:page:${page - 1}`);
  kb.text(`${page}/${totalPages}`, `${cbPrefix}:noop`);
  if (page < totalPages) kb.text(`${page + 1}-sahifa ›`, `${cbPrefix}:page:${page + 1}`);
  kb.row().text("⟨ Orqaga qaytish", `${cbPrefix}:back`);
  // buildNavRow oxirida styleAll qilib barcha tugmalar (shu jumladan yangi qo'shilganlar) ham ranglanadi
  styleAll(kb);
}

// ============================================================
// Raqamli tugmalar: 5 ustun, max 4 qator = 20 ta
// ============================================================
function buildNumberedButtons(
  items: { animeId: number }[],
  cbPrefix: string
): InlineKeyboard {
  const kb = new InlineKeyboard();
  items.forEach((item, i) => {
    kb.text(String(i + 1), `${cbPrefix}:open:${item.animeId}`);
    if ((i + 1) % 5 === 0) kb.row();
  });
  if (items.length % 5 !== 0) kb.row();
  return styleAll(kb);
}

// ============================================================
// Asosiy qidiruv menyusi
// ============================================================
async function sendSearchMenu(ctx: MyContext) {
  ctx.session.state = "idle";
  const kb = styleAll(new InlineKeyboard()
    .text("🔍 Kod orqali izlash", "search:mode:code").row()
    .text("🔍 Nom orqali izlash", "search:mode:name").row()
    .text("🔍 Oxirgi yuklanganlar", "search:mode:recent").row()
    .text("🔍 Eng ko'p yuklanganlar", "search:mode:popular"));

  const text = "Qanday qidirmoqchisiz?";
  if (ctx.callbackQuery) {
    await ctx.editMessageText(text, { reply_markup: kb }).catch(() =>
      ctx.reply(text, { reply_markup: kb })
    );
    await ctx.answerCallbackQuery();
  } else {
    await ctx.reply(text, { reply_markup: kb });
  }
}

// ============================================================
// 2.1 — Kod orqali: anime postini ko'rsatish
// ============================================================
async function sendAnimePost(ctx: MyContext, animeId: number) {
  const botInfo = await ctx.api.getMe();
  const botUsername = botInfo.username ?? "";
  const anime = await getAnimeById(animeId);
  if (!anime) {
    await ctx.answerCallbackQuery("Anime topilmadi").catch(() => {});
    if (!ctx.callbackQuery) await ctx.reply("❌ Anime topilmadi.");
    return;
  }
  const caption = await buildAnimePost(anime, botUsername);
  const kb = animePostKeyboard(anime);

  if (ctx.callbackQuery) await ctx.answerCallbackQuery();

  if (anime.file_id) {
    try {
      await ctx.replyWithPhoto(anime.file_id, { caption, reply_markup: kb, protect_content: true });
      return;
    } catch {
      // Rasm ishlamasa matn ko'rinishiga o'tamiz
    }
  }
  await ctx.reply(caption, { reply_markup: kb });
}

// ============================================================
// 2.2 — Nom orqali: raqamli ro'yxat
// ============================================================
async function sendNameSearchResults(ctx: MyContext, query: string, page: number) {
  const all = await searchAnimeByName(query);
  const total = all.length;
  if (total === 0) {
    const kb = styleAll(new InlineKeyboard().text("⟨ Orqaga qaytish", "searchname:back"));
    const msg = `"${query}" bo'yicha hech narsa topilmadi.`;
    if (ctx.callbackQuery) {
      await ctx.editMessageText(msg, { reply_markup: kb }).catch(() =>
        ctx.reply(msg, { reply_markup: kb })
      );
      await ctx.answerCallbackQuery();
    } else {
      await ctx.reply(msg, { reply_markup: kb });
    }
    return;
  }

  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE));
  const cur = Math.min(Math.max(1, page), totalPages);
  const slice = all.slice((cur - 1) * PAGE_SIZE, cur * PAGE_SIZE);

  const listText = slice.map((a, i) => `${i + 1}. ${a.name}`).join("\n");
  const text = `"${query}" — ${total} ta natija\n\n${listText}`;

  const kb = buildNumberedButtons(slice.map(a => ({ animeId: a.id })), "searchname");
  buildNavRow(kb, cur, totalPages, "searchname");

  if (ctx.callbackQuery) {
    await ctx.editMessageText(text, { reply_markup: kb }).catch(() =>
      ctx.reply(text, { reply_markup: kb })
    );
    await ctx.answerCallbackQuery();
  } else {
    await ctx.reply(text, { reply_markup: kb });
  }
}

// ============================================================
// 2.3 — Oxirgi yuklanganlar: JOIN bilan, matnli ko'rinish
// ============================================================
function buildRecentEpLabel(row: RecentEpisodeRow): string {
  const seasonPart = row.kategoriya === "TV" && row.season_id
    ? ` { ${row.season_id}-fasl }`
    : "";
  let epPart = "";
  if (row.kategoriya === "TV") {
    epPart = ` [ ${pad2(row.epizod_number ?? 0)}-qism ]`;
  } else if (row.kategoriya === "FILM") {
    epPart = ` [ ${row.film_id}-film ]`;
  } else {
    epPart = ` [ ${pad2(row.ova_id ?? 0)}-qism ]`;
  }
  return `${row.info_name}${seasonPart}${epPart}`;
}

async function sendRecentList(ctx: MyContext, page: number) {
  const { rows, total } = await getRecentEpisodesPerAnime(page, PAGE_SIZE);
  if (total === 0) {
    const kb = styleAll(new InlineKeyboard().text("⟨ Orqaga qaytish", "searchrec:back"));
    const msg = "Hali hech qanday qism qo'shilmagan.";
    if (ctx.callbackQuery) {
      await ctx.editMessageText(msg, { reply_markup: kb }).catch(() =>
        ctx.reply(msg, { reply_markup: kb })
      );
      await ctx.answerCallbackQuery();
    } else {
      await ctx.reply(msg, { reply_markup: kb });
    }
    return;
  }

  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE));
  const cur = Math.min(Math.max(1, page), totalPages);

  const listText = rows
    .map(
      (r, i) =>
        `${i + 1}. Anime nomi: ${r.anime_name}\n` +
        `Oxirgi yuklangan qism: ${buildRecentEpLabel(r)}\n` +
        `Yuklangan sana: ${r.ep_data || "—"}`
    )
    .join("\n\n");

  const text = `Oxirgi yuklanganlar — ${total} ta\n\n${listText}`;

  const kb = buildNumberedButtons(rows.map(r => ({ animeId: r.anime_id })), "searchrec");
  buildNavRow(kb, cur, totalPages, "searchrec");

  if (ctx.callbackQuery) {
    await ctx.editMessageText(text, { reply_markup: kb }).catch(() =>
      ctx.reply(text, { reply_markup: kb })
    );
    await ctx.answerCallbackQuery();
  } else {
    await ctx.reply(text, { reply_markup: kb });
  }
}

// ============================================================
// 2.4 — Eng ko'p yuklanganlar: matnli ko'rinish
// ============================================================
async function sendPopularList(ctx: MyContext, page: number) {
  const { animes, total } = await getAnimesPopularPaginated(page, PAGE_SIZE);
  if (total === 0) {
    const kb = styleAll(new InlineKeyboard().text("⟨ Orqaga qaytish", "searchpop:back"));
    const msg = "Hali hech qanday anime qo'shilmagan.";
    if (ctx.callbackQuery) {
      await ctx.editMessageText(msg, { reply_markup: kb }).catch(() =>
        ctx.reply(msg, { reply_markup: kb })
      );
      await ctx.answerCallbackQuery();
    } else {
      await ctx.reply(msg, { reply_markup: kb });
    }
    return;
  }

  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE));
  const cur = Math.min(Math.max(1, page), totalPages);
  const slice = animes;

  const listText = slice
    .map(
      (a, i) =>
        `${i + 1}. Anime nomi: ${a.name}\n` +
        `   Yuklashlar soni: 📥 ${a.downloads} marta`
    )
    .join("\n\n");

  const text = `Eng ko'p yuklangan animelar — ${total} ta\n\n${listText}`;

  const kb = buildNumberedButtons(slice.map(a => ({ animeId: a.id })), "searchpop");
  buildNavRow(kb, cur, totalPages, "searchpop");

  if (ctx.callbackQuery) {
    await ctx.editMessageText(text, { reply_markup: kb }).catch(() =>
      ctx.reply(text, { reply_markup: kb })
    );
    await ctx.answerCallbackQuery();
  } else {
    await ctx.reply(text, { reply_markup: kb });
  }
}

// ============================================================
// setupSearch
// ============================================================
export function setupSearch(bot: Bot<MyContext>) {
  bot.hears("🔍 Anime izlash", sendSearchMenu);

  bot.callbackQuery("search:mode:code", async (ctx) => {
    ctx.session.state = "waiting_search_code";
    ctx.session.searchMode = "code";
    await ctx.answerCallbackQuery();
    const kb = styleAll(new InlineKeyboard().text("⟨ Orqaga qaytish", "search:back"));
    await ctx.reply("Anime kodini yuboring (masalan: 42):", { reply_markup: kb });
  });

  bot.callbackQuery("search:mode:name", async (ctx) => {
    ctx.session.state = "waiting_search_name";
    ctx.session.searchMode = "name";
    await ctx.answerCallbackQuery();
    const kb = styleAll(new InlineKeyboard().text("⟨ Orqaga qaytish", "search:back"));
    await ctx.reply("Izlayotgan anime nomini yuboring:", { reply_markup: kb });
  });

  bot.callbackQuery("search:mode:recent", async (ctx) => {
    ctx.session.searchPage = 1;
    await sendRecentList(ctx, 1);
  });

  bot.callbackQuery("search:mode:popular", async (ctx) => {
    ctx.session.searchPage = 1;
    await sendPopularList(ctx, 1);
  });

  bot.callbackQuery("search:back", sendSearchMenu);

  // ---- Nom qidiruv ----
  bot.callbackQuery(/^searchname:page:(\d+)$/, async (ctx) => {
    const page = Number(ctx.match![1]);
    ctx.session.searchPage = page;
    await sendNameSearchResults(ctx, ctx.session.searchQuery ?? "", page);
  });
  bot.callbackQuery("searchname:back", sendSearchMenu);
  bot.callbackQuery("searchname:noop", (ctx) => ctx.answerCallbackQuery());
  bot.callbackQuery(/^searchname:open:(\d+)$/, async (ctx) => {
    await sendAnimePost(ctx, Number(ctx.match![1]));
  });

  // ---- Oxirgi yuklanganlar ----
  bot.callbackQuery(/^searchrec:page:(\d+)$/, async (ctx) => {
    const page = Number(ctx.match![1]);
    ctx.session.searchPage = page;
    await sendRecentList(ctx, page);
  });
  bot.callbackQuery("searchrec:back", sendSearchMenu);
  bot.callbackQuery("searchrec:noop", (ctx) => ctx.answerCallbackQuery());
  bot.callbackQuery(/^searchrec:open:(\d+)$/, async (ctx) => {
    await sendAnimePost(ctx, Number(ctx.match![1]));
  });

  // ---- Eng ko'p yuklanganlar ----
  bot.callbackQuery(/^searchpop:page:(\d+)$/, async (ctx) => {
    const page = Number(ctx.match![1]);
    ctx.session.searchPage = page;
    await sendPopularList(ctx, page);
  });
  bot.callbackQuery("searchpop:back", sendSearchMenu);
  bot.callbackQuery("searchpop:noop", (ctx) => ctx.answerCallbackQuery());
  bot.callbackQuery(/^searchpop:open:(\d+)$/, async (ctx) => {
    await sendAnimePost(ctx, Number(ctx.match![1]));
  });

  // ---- Matn orqali qidiruv ----
  bot.on("message:text", async (ctx, next) => {
    const state = ctx.session.state;

    if (state === "waiting_search_code") {
      const code = parseInt(ctx.message.text.trim(), 10);
      ctx.session.state = "idle";
      if (isNaN(code) || code <= 0) {
        await ctx.reply("Noto'g'ri kod. Faqat son kiriting.");
        return;
      }
      await sendAnimePost(ctx, code);
      return;
    }

    if (state === "waiting_search_name") {
      const query = ctx.message.text.trim();
      ctx.session.state = "idle";
      ctx.session.searchQuery = query;
      ctx.session.searchPage = 1;
      await sendNameSearchResults(ctx, query, 1);
      return;
    }

    return next();
  });
}
