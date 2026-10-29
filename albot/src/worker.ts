/**
 * albot — Telegram bot (webhook) + Actions'dagi mahalliy AI model (gpt-oss-20b) o'rtasidagi ko'prik.
 *
 * Oqim: foydalanuvchi botga yozadi -> worker xabarni Turso'ga yozadi -> bazada modelga O'QILMAGAN yoki JAVOBSIZ xabar bo'lsa
 * (va Actions run'i yo'q bo'lsa) worker ikkinchi akkauntdagi repoda `albot.yml` ni ishga tushiradi. Actions FAQAT worker orqali
 * ishlaydi, kutib turmaydi (polling yo'q): run modelni yuklaydi, navbatdagi javobsiz xabarlarni birin-ketin oladi (`/ai/poll` — bitta
 * olish), javoblarni `/ai/reply` bilan botga yuboradi, navbat bo'shagach `/ai/done` deydi va to'xtaydi. Navbatda yana xabar qolgan
 * bo'lsa, worker o'zi yangi run ochadi. Yozishmalar Turso'da; o'qilgan tarix Cloudflare KV keshida 1 kun.
 *
 * Tarix keshi: KV'da {xulosa, oxirgi id, xabarlar}; har so'rovda bazadan FAQAT yangi (id > oxirgi) qatorlar o'qiladi.
 * (Cache API workers.dev manzilida ishlamaydi, shuning uchun KV; KV bo'lmasa kesh isolate xotirasida.)
 */

export interface Env {
  BOT_TOKEN: string;
  TURSO_URL: string;
  TURSO_TOKEN: string;
  RUN_KEY: string;
  GH_ACTIONS_TOKEN?: string;
  GH_REPO?: string;
  ADMIN_ID: string;
  CACHE?: KVNamespace;
}

const WORKFLOW = "albot.yml";
const CACHE_TTL = 86400;          // 1 kun
const HIST_MAX = 400;             // keshdagi xabarlar soni chegarasi
const STALE_CLAIM = 900;          // modelda 15 daqiqa tursa — qaytadan olinadi
const RUN_SPAN = 3900;            // run ko'pi bilan 1 soat (+5 daqiqa zaxira): shu vaqt ichida "run bor" deb hisoblanadi
const WAKE_GAP = 60;              // uyg'otish xatosidan keyin qayta urinish oralig'i
const MAX_TRIES = 3;              // javobsiz xabar uchun ketma-ket muvaffaqiyatsiz run'lar soni

// ─── Turso (Hrana HTTP) ───────────────────────────────────────────────────────

type Row = Record<string, unknown>;
type Stmt = [string, unknown[]?];

function arg(v: unknown) {
  if (v === null || v === undefined) return { type: "null" };
  if (typeof v === "number") return Number.isInteger(v) ? { type: "integer", value: String(v) } : { type: "float", value: v };
  return { type: "text", value: String(v) };
}

/** Bir nechta buyruq — BITTA HTTP so'rovi. Natija: har buyruq uchun qatorlar. */
async function db(env: Env, stmts: Stmt[]): Promise<Row[][]> {
  const base = env.TURSO_URL.trim().replace(/^libsql:/, "https:").replace(/\/+$/, "");
  const body = {
    requests: [
      ...stmts.map(([sql, a]) => ({ type: "execute", stmt: { sql, args: (a ?? []).map(arg) } })),
      { type: "close" },
    ],
  };
  let last = "";
  for (let i = 0; i < 3; i++) {
    try {
      const r = await fetch(`${base}/v2/pipeline`, {
        method: "POST",
        headers: { Authorization: `Bearer ${env.TURSO_TOKEN.trim()}`, "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });
      if (r.status >= 500 || r.status === 429) { last = `HTTP ${r.status}`; await sleep(300 * (i + 1)); continue; }
      const j = (await r.json()) as { results?: { type: string; response?: { result?: any }; error?: { message: string } }[]; error?: string };
      if (!r.ok) throw new Error(`Turso ${r.status}: ${j.error ?? ""}`);
      const out: Row[][] = [];
      for (let k = 0; k < stmts.length; k++) {
        const res = j.results?.[k];
        if (!res || res.type === "error") throw new Error(`Turso xato: ${res?.error?.message ?? "noma'lum"}`);
        const rs = res.response!.result;
        const names: string[] = rs.cols.map((c: { name: string }) => c.name);
        out.push(rs.rows.map((row: { type: string; value?: string | number }[]) => {
          const o: Row = {};
          row.forEach((c, n) => { o[names[n]] = c.type === "null" ? null : c.type === "integer" || c.type === "float" ? Number(c.value) : c.value; });
          return o;
        }));
      }
      return out;
    } catch (e) {
      last = String(e);
      if (String(e).includes("Turso xato") || String(e).includes("Turso 4")) throw e;
      await sleep(300 * (i + 1));
    }
  }
  throw new Error(`Turso ulanmadi: ${last}`);
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const nowSec = () => Math.floor(Date.now() / 1000);

let schemaReady = false;
async function ensureSchema(env: Env): Promise<void> {
  if (schemaReady) return;
  await db(env, [
    // role: 0 = foydalanuvchi, 1 = model. state (faqat role=0): 0 o'qilmagan, 1 modelda, 2 javob berilgan.
    [`CREATE TABLE IF NOT EXISTS ai_msg(
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        chat_id INTEGER NOT NULL,
        role INTEGER NOT NULL,
        text TEXT NOT NULL,
        state INTEGER NOT NULL DEFAULT 0,
        created_at INTEGER NOT NULL DEFAULT (unixepoch()),
        claimed_at INTEGER)`],
    ["CREATE INDEX IF NOT EXISTS idx_ai_chat ON ai_msg(chat_id, id)"],
    ["CREATE INDEX IF NOT EXISTS idx_ai_open ON ai_msg(id) WHERE role = 0 AND state < 2"],
    ["CREATE TABLE IF NOT EXISTS ai_sum(chat_id INTEGER PRIMARY KEY, summary TEXT NOT NULL, upto_id INTEGER NOT NULL) WITHOUT ROWID"],
    ["CREATE TABLE IF NOT EXISTS ai_state(k TEXT PRIMARY KEY, v INTEGER NOT NULL) WITHOUT ROWID"],
    // Foydalanuvchi yuborgan fayllar (Telegram file_id); model `get_attachment` vositasi bilan oladi.
    ["CREATE TABLE IF NOT EXISTS ai_file(id INTEGER PRIMARY KEY AUTOINCREMENT, chat_id INTEGER NOT NULL, tg_file_id TEXT NOT NULL, name TEXT NOT NULL, size INTEGER NOT NULL DEFAULT 0, created_at INTEGER NOT NULL DEFAULT (unixepoch()))"],
  ]);
  schemaReady = true;
}

// ─── Telegram ─────────────────────────────────────────────────────────────────

async function tg(env: Env, method: string, body: unknown): Promise<any> {
  const r = await fetch(`https://api.telegram.org/bot${env.BOT_TOKEN.trim()}/${method}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  return r.json().catch(() => ({}));
}

async function say(env: Env, chat: number, text: string): Promise<void> {
  const t = text.trim() || "…";
  for (let i = 0; i < t.length; i += 4000) {
    await tg(env, "sendMessage", { chat_id: chat, text: t.slice(i, i + 4000), disable_web_page_preview: true });
  }
}

async function secretToken(env: Env): Promise<string> {
  const d = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(`${env.BOT_TOKEN.trim()}:albot`));
  return [...new Uint8Array(d)].map((b) => b.toString(16).padStart(2, "0")).join("").slice(0, 32);
}

// ─── Actions'ni uyg'otish ─────────────────────────────────────────────────────

/**
 * Faqat bazada o'qilmagan/javobsiz xabar bo'lsa va run yo'q bo'lsa run ochiladi (`until` — run ishlashi mumkin bo'lgan oxirgi vaqt).
 * Run xabar olmasdan 3 marta ketma-ket tugasa (model yuklanmayapti) — xabarlar yopiladi va foydalanuvchiga aytiladi.
 * Natija: none | alive | recent | stuck | dispatched | error:<sabab>
 */
async function wakeIfNeeded(env: Env): Promise<string> {
  const [[s]] = await db(env, [[
    `SELECT (SELECT COUNT(*) FROM ai_msg WHERE role = 0 AND state < 2) AS open,
            COALESCE((SELECT v FROM ai_state WHERE k = 'until'), 0) AS until,
            COALESCE((SELECT v FROM ai_state WHERE k = 'wake'), 0) AS wake,
            COALESCE((SELECT v FROM ai_state WHERE k = 'tries'), 0) AS tries`,
  ]]);
  const now = nowSec();
  if (!Number(s.open)) return "none";
  if (Number(s.until) > now) return "alive";
  if (Number(s.wake) > now - WAKE_GAP) return "recent";
  if (Number(s.tries) >= MAX_TRIES) {
    const [rows] = await db(env, [["SELECT DISTINCT chat_id FROM ai_msg WHERE role = 0 AND state < 2"]]);
    await db(env, [["UPDATE ai_msg SET state = 2 WHERE role = 0 AND state < 2"], ["DELETE FROM ai_state WHERE k = 'tries'"]]);
    for (const r of rows) await say(env, Number(r.chat_id), "⚠️ Model 3 marta ishga tushmadi — xabaringiz javobsiz qoldi. Birozdan keyin qayta yozing.");
    return "stuck";
  }
  if (!env.GH_ACTIONS_TOKEN || !env.GH_REPO) return "error:GH_ACTIONS_TOKEN/GH_REPO yo'q";
  await db(env, [
    ["INSERT INTO ai_state(k, v) VALUES('until', ?) ON CONFLICT(k) DO UPDATE SET v = excluded.v", [now + RUN_SPAN]],
    ["INSERT INTO ai_state(k, v) VALUES('wake', ?) ON CONFLICT(k) DO UPDATE SET v = excluded.v", [now]],
    ["INSERT INTO ai_state(k, v) VALUES('tries', 1) ON CONFLICT(k) DO UPDATE SET v = v + 1"],
  ]);
  const r = await fetch(`https://api.github.com/repos/${env.GH_REPO.trim()}/actions/workflows/${WORKFLOW}/dispatches`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${env.GH_ACTIONS_TOKEN.trim()}`,
      Accept: "application/vnd.github+json",
      "X-GitHub-Api-Version": "2022-11-28",
      "User-Agent": "albot",
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ ref: "main" }),
  });
  if (r.status === 204) return "dispatched";
  // Xatoda run ochilmagan: "run bor" belgisi olib tashlanadi, urinish hisobi qaytariladi, ~1 daqiqadan keyin cron qayta uriniladi.
  await db(env, [["UPDATE ai_state SET v = 0 WHERE k = 'until'"], ["UPDATE ai_state SET v = MAX(v - 1, 0) WHERE k = 'tries'"]]);
  const t = await r.text().catch(() => "");
  return `error:GitHub ${r.status} ${t.slice(0, 120)}`;
}

// ─── Tarix keshi (KV, 1 kun) ──────────────────────────────────────────────────

interface Msg { id: number; r: number; t: string }
interface Hist { sum: string; sumUpto: number; last: number; msgs: Msg[] }

const memCache = new Map<string, { at: number; h: Hist }>();
const hkey = (chat: number) => `h:${chat}`;

async function cacheGet(env: Env, chat: number): Promise<Hist | null> {
  const m = memCache.get(hkey(chat));
  if (m && Date.now() - m.at < CACHE_TTL * 1000) return m.h;
  if (env.CACHE) {
    try { const h = await env.CACHE.get<Hist>(hkey(chat), "json"); if (h) return h; } catch { /* kesh ixtiyoriy */ }
  }
  return null;
}

async function cachePut(env: Env, chat: number, h: Hist): Promise<void> {
  memCache.set(hkey(chat), { at: Date.now(), h });
  if (env.CACHE) {
    try { await env.CACHE.put(hkey(chat), JSON.stringify(h), { expirationTtl: CACHE_TTL }); } catch { /* kesh ixtiyoriy */ }
  }
}

async function cacheDrop(env: Env, chat: number): Promise<void> {
  memCache.delete(hkey(chat));
  if (env.CACHE) { try { await env.CACHE.delete(hkey(chat)); } catch { /* ixtiyoriy */ } }
}

/** Suhbat tarixi: xulosa + undan keyingi xabarlar (maxId gacha). Bazadan faqat keshda yo'q yangi qatorlar o'qiladi. */
async function loadHistory(env: Env, chat: number, maxId: number): Promise<Hist> {
  let h = await cacheGet(env, chat);
  const stmts: Stmt[] = [];
  if (!h) {
    stmts.push(["SELECT summary, upto_id FROM ai_sum WHERE chat_id = ?", [chat]]);
    stmts.push(["SELECT id, role, text FROM ai_msg WHERE chat_id = ? AND id <= ? AND id > COALESCE((SELECT upto_id FROM ai_sum WHERE chat_id = ?), 0) ORDER BY id DESC LIMIT ?",
      [chat, maxId, chat, HIST_MAX]]);
    const [sumRows, msgRows] = await db(env, stmts);
    h = {
      sum: sumRows[0] ? String(sumRows[0].summary) : "",
      sumUpto: sumRows[0] ? Number(sumRows[0].upto_id) : 0,
      last: 0,
      msgs: msgRows.reverse().map((x) => ({ id: Number(x.id), r: Number(x.role), t: String(x.text) })),
    };
    h.last = h.msgs.length ? h.msgs[h.msgs.length - 1].id : h.sumUpto;
    await cachePut(env, chat, h);
  } else if (h.last < maxId) {
    const [rows] = await db(env, [["SELECT id, role, text FROM ai_msg WHERE chat_id = ? AND id > ? AND id <= ? ORDER BY id", [chat, h.last, maxId]]]);
    for (const x of rows) h.msgs.push({ id: Number(x.id), r: Number(x.role), t: String(x.text) });
    if (rows.length) h.last = Number(rows[rows.length - 1].id);
    if (h.msgs.length > HIST_MAX) h.msgs = h.msgs.slice(-HIST_MAX);
    await cachePut(env, chat, h);
  }
  return { ...h, msgs: h.msgs.filter((m) => m.id <= maxId) };
}

// ─── Marshrutlar ──────────────────────────────────────────────────────────────

const json = (o: unknown, status = 200) =>
  new Response(JSON.stringify(o), { status, headers: { "Content-Type": "application/json" } });

async function handleUpdate(env: Env, u: any, ctx: ExecutionContext): Promise<void> {
  const m = u?.message;
  if (!m?.chat) return;
  // Botdan faqat egasi foydalanadi.
  if (String(m.from?.id) !== String(env.ADMIN_ID)) return;
  const chat = Number(m.chat.id);
  let text: string = m.text ?? m.caption ?? "";

  // Biriktirilgan fayl (hujjat, rasm, video, audio): file_id saqlanadi, xabarga uning raqami yoziladi.
  const att = m.document ?? m.video ?? m.audio ?? m.voice ?? m.animation
    ?? (Array.isArray(m.photo) && m.photo.length ? { ...m.photo[m.photo.length - 1], file_name: `photo_${m.message_id}.jpg` } : null);
  if (att?.file_id) {
    const name = String(att.file_name ?? `${m.video ? "video" : m.audio || m.voice ? "audio" : "file"}_${m.message_id}`).slice(0, 120);
    const [[r]] = await db(env, [["INSERT INTO ai_file(chat_id, tg_file_id, name, size) VALUES(?, ?, ?, ?) RETURNING id", [chat, String(att.file_id), name, Number(att.file_size ?? 0)]]]);
    text = `${text}\n[Biriktirilgan fayl #${r.id}: ${name}, ${Number(att.file_size ?? 0)} bayt — get_attachment(${r.id}) bilan olinadi]`.trim();
  }
  if (!text) return;

  if (text.startsWith("/start")) {
    await say(env, chat, "Salom! Yozing — model Actions'da (gpt-oss-20b) javob beradi.\n/new — yangi suhbat\n/status — holat");
    return;
  }
  if (text.startsWith("/new")) {
    await db(env, [["DELETE FROM ai_msg WHERE chat_id = ?", [chat]], ["DELETE FROM ai_sum WHERE chat_id = ?", [chat]]]);
    await cacheDrop(env, chat);
    await say(env, chat, "🧹 Yangi suhbat boshlandi.");
    return;
  }
  if (text.startsWith("/status")) {
    const [[s]] = await db(env, [[
      `SELECT (SELECT COUNT(*) FROM ai_msg WHERE role = 0 AND state < 2) AS open,
              COALESCE((SELECT v FROM ai_state WHERE k = 'until'), 0) AS until`,
    ]]);
    const alive = Number(s.until) > nowSec();
    await say(env, chat, `Model run'i: ${alive ? "ishlayapti" : "o'chiq"}\nJavobsiz xabarlar: ${s.open}`);
    return;
  }

  await db(env, [
    ["INSERT INTO ai_msg(chat_id, role, text) VALUES(?, 0, ?)", [chat, text]],
    ["DELETE FROM ai_state WHERE k = 'tries'"],  // yangi xabar — yangi urinishlar
  ]);
  const w = await wakeIfNeeded(env);
  if (w === "dispatched") await say(env, chat, "🧠 Model uyg'onyapti — birinchi javobgacha taxminan 3–8 daqiqa.");
  else if (w.startsWith("error:")) await say(env, chat, `⚠️ Actions ishga tushmadi (${w.slice(6)}). Bir daqiqadan keyin yana uriniladi.`);
  else if (w === "alive" || w === "recent") ctx.waitUntil(tg(env, "sendChatAction", { chat_id: chat, action: "typing" }));
}

export default {
  async fetch(req: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
    const url = new URL(req.url);
    const p = url.pathname;

    if (p === "/health") {
      try { await ensureSchema(env); return json({ ok: true, db: "turso ok" }); }
      catch (e) { return json({ ok: false, error: String(e) }, 500); }
    }

    if (p === "/setup") {
      const r = await tg(env, "setWebhook", {
        url: `${url.origin}/webhook`,
        secret_token: await secretToken(env),
        allowed_updates: ["message"],
        max_connections: 2,
      });
      await tg(env, "setMyCommands", { commands: [
        { command: "new", description: "Yangi suhbat" },
        { command: "status", description: "Model holati" },
      ] });
      return json(r);
    }

    if (p === "/webhook" && req.method === "POST") {
      if (req.headers.get("X-Telegram-Bot-Api-Secret-Token") !== (await secretToken(env))) return json({ ok: false }, 403);
      try { await ensureSchema(env); await handleUpdate(env, await req.json(), ctx); }
      catch (e) { console.log("webhook xato:", String(e)); }
      return json({ ok: true });
    }

    if (p.startsWith("/ai/")) {
      if (req.headers.get("X-Run-Key")?.trim() !== env.RUN_KEY.trim()) return json({ ok: false }, 403);
      await ensureSchema(env);
      const b: any = req.method === "POST" ? await req.json().catch(() => ({})) : {};

      // Run navbatni bo'shatib tugadi (yoki yiqildi): "run bor" belgisi o'chadi, navbatda xabar qolgan bo'lsa worker yangi run ochadi.
      if (p === "/ai/done") {
        await db(env, [["INSERT INTO ai_state(k, v) VALUES('until', 0) ON CONFLICT(k) DO UPDATE SET v = 0"]]);
        if (b.ok === false) {
          await db(env, [["INSERT INTO ai_state(k, v) VALUES('wake', ?) ON CONFLICT(k) DO UPDATE SET v = excluded.v", [nowSec()]]]);
          // Run yiqilgan sabab egasiga ko'rsatiladi (3 urinishgacha har safar).
          if (b.err) ctx.waitUntil(say(env, Number(env.ADMIN_ID), `⚠️ Actions run'i yiqildi:\n${String(b.err).slice(0, 1500)}`));
        }
        return json({ ok: true, next: await wakeIfNeeded(env) });
      }

      if (p === "/ai/typing") {
        await tg(env, "sendChatAction", { chat_id: Number(b.chat), action: "typing" });
        return json({ ok: true });
      }

      // Eng eski o'qilmagan xabarni atomik olish (15 daqiqadan beri modelda tursa — qaytadan).
      if (p === "/ai/poll") {
        const [rows] = await db(env, [[
          `UPDATE ai_msg SET state = 1, claimed_at = unixepoch()
           WHERE id = (SELECT id FROM ai_msg WHERE role = 0 AND (state = 0 OR (state = 1 AND claimed_at < unixepoch() - ?)) ORDER BY id LIMIT 1)
           RETURNING id, chat_id, text`, [STALE_CLAIM],
        ]]);
        if (!rows.length) return json({ ok: true, msg: null });
        const id = Number(rows[0].id);
        const chat = Number(rows[0].chat_id);
        ctx.waitUntil(db(env, [["DELETE FROM ai_state WHERE k = 'tries'"]]));  // run ishlayapti — urinish hisobi nolga
        ctx.waitUntil(tg(env, "sendChatAction", { chat_id: chat, action: "typing" }));
        const h = await loadHistory(env, chat, id);
        return json({ ok: true, msg: { id, chat, text: String(rows[0].text) }, summary: h.sum, history: h.msgs });
      }

      if (p === "/ai/reply") {
        const id = Number(b.id);
        const [[m]] = await db(env, [["SELECT chat_id FROM ai_msg WHERE id = ? AND role = 0", [id]]]);
        if (!m) return json({ ok: false, error: "xabar yo'q" }, 404);
        const chat = Number(m.chat_id);
        const text = String(b.text ?? "").trim() || "(model bo'sh javob berdi)";
        await db(env, [
          ["INSERT INTO ai_msg(chat_id, role, text, state) VALUES(?, 1, ?, 2)", [chat, text]],
          ["UPDATE ai_msg SET state = 2 WHERE id = ?", [id]],
        ]);
        await say(env, chat, text);
        return json({ ok: true });
      }

      // Uzun tarixni siqish: eski qism xulosaga aylanadi (xabarlar bazada qoladi, modelga faqat xulosa + yangilari beriladi).
      if (p === "/ai/summary") {
        const chat = Number(b.chat);
        await db(env, [[
          "INSERT INTO ai_sum(chat_id, summary, upto_id) VALUES(?, ?, ?) ON CONFLICT(chat_id) DO UPDATE SET summary = excluded.summary, upto_id = excluded.upto_id",
          [chat, String(b.summary ?? ""), Number(b.upto)],
        ]]);
        await cacheDrop(env, chat);
        return json({ ok: true });
      }

      // Javobni yopmasdan foydalanuvchiga oraliq xabar (agent qadamlari haqida).
      if (p === "/ai/say") {
        await say(env, Number(b.chat), String(b.text ?? "").slice(0, 3500));
        return json({ ok: true });
      }

      // Run -> foydalanuvchi: fayl yuborish (xom bayt; sarlavhalarda chat, nom, izoh). Telegram chegarasi 50 MB.
      if (p === "/ai/file") {
        const chat = Number(req.headers.get("X-Chat"));
        const name = decodeURIComponent(req.headers.get("X-Name") ?? "file");
        const caption = decodeURIComponent(req.headers.get("X-Caption") ?? "").slice(0, 1000);
        const buf = await req.arrayBuffer();
        if (!chat || !buf.byteLength) return json({ ok: false, error: "bo'sh fayl yoki chat" }, 400);
        if (buf.byteLength > 49 * 1024 * 1024) return json({ ok: false, error: "fayl 49 MB dan katta (Telegram bot chegarasi 50 MB)" }, 413);
        const fd = new FormData();
        fd.append("chat_id", String(chat));
        if (caption) fd.append("caption", caption);
        fd.append("document", new Blob([buf]), name);
        const r = await fetch(`https://api.telegram.org/bot${env.BOT_TOKEN.trim()}/sendDocument`, { method: "POST", body: fd });
        const j: any = await r.json().catch(() => ({}));
        return json({ ok: !!j.ok, error: j.ok ? undefined : String(j.description ?? r.status) }, j.ok ? 200 : 502);
      }

      // Foydalanuvchi yuborgan faylni olish (Bot API chegarasi 20 MB): javob — xom bayt.
      if (p === "/ai/getfile") {
        const [[f]] = await db(env, [["SELECT tg_file_id, name, size FROM ai_file WHERE id = ?", [Number(b.id)]]]);
        if (!f) return json({ ok: false, error: "bunday fayl yo'q" }, 404);
        const info: any = await tg(env, "getFile", { file_id: String(f.tg_file_id) });
        if (!info.ok) return json({ ok: false, error: String(info.description ?? "getFile xato") + " (Bot API 20 MB dan kattani bermaydi)" }, 413);
        const r = await fetch(`https://api.telegram.org/file/bot${env.BOT_TOKEN.trim()}/${info.result.file_path}`);
        if (!r.ok) return json({ ok: false, error: `Telegram ${r.status}` }, 502);
        return new Response(r.body, { headers: { "Content-Type": "application/octet-stream", "X-Name": encodeURIComponent(String(f.name)) } });
      }

      if (p === "/ai/fail") {
        const id = Number(b.id);
        const [[m]] = await db(env, [["SELECT chat_id FROM ai_msg WHERE id = ? AND role = 0", [id]]]);
        if (m) {
          await db(env, [["UPDATE ai_msg SET state = 2 WHERE id = ?", [id]]]);
          await say(env, Number(m.chat_id), `⚠️ Model xato berdi: ${String(b.error ?? "").slice(0, 300)}`);
        }
        return json({ ok: true });
      }
    }

    return new Response("albot", { status: 200 });
  },

  async scheduled(_ev: ScheduledController, env: Env, ctx: ExecutionContext): Promise<void> {
    ctx.waitUntil((async () => {
      try { await ensureSchema(env); const r = await wakeIfNeeded(env); if (r.startsWith("error:")) console.log("uyg'otish:", r); }
      catch (e) { console.log("cron xato:", String(e)); }
    })());
  },
};
