// Telegram orqali fayllar (videolar, rasmlar, yozishma fayllari).
//
// Ilovadagi yo'l bilan AYNAN bir xil (`rust/src/telegram.rs`):
//   1. `POST /api/tg/deliver {files}` — bot fayllarni yopiq kanaldan
//      foydalanuvchining bot chatiga nusxalaydi va
//      ochish kalitlarini (`keys`) qaytaradi;
//   2. sayt foydalanuvchining O'Z Telegram hisobi bilan (mtcute) bot
//      chatining oxirgi 100 xabaridan faylni NOMI bo'yicha topadi;
//   3. kerakli bo'laklarni (`upload.getFile`, 1 MB) oladi va xotirada
//      AES-128-CTR bilan ochadi (IV nol, hisoblagich = bayt / 16).
// Worker orqali bayt o'tmaydi; diskka hech narsa yozilmaydi.
// Bot chati FAQAT Mini App ochilganda tozalanadi (`clearOldBotChat`).
//
// Eksport:
//   ensureTelegram()                 — kirilmagan bo'lsa kirish oynasi; bool
//   isTelegramAuthorized()           — keshlangan holat
//   openFile(name, {key})            — {size, read(offset, len), close()}
//   fetchFile(name, {key})           — Blob (kichik fayllar)
//   mediaUrl(name, {key})            — objectURL (keshlanadi)
//   uploadFile(file, name)           — {name, key} (bot chati -> kanal)
//   clearOldBotChat()                — ochilganda eski nusxalarni o'chirish

import { api, apiPost, ApiError } from '../api.js';
import { downloadChunk } from '@mtcute/web/methods.js';
import { getClient, tgConfig, isAuthorized, authorizedCached } from './client.js';
import { openTelegramLogin } from './login.js';
import { cacheGet, cachePut } from './chunk-cache.js';

const CHUNK = 1024 * 1024; // Telegram upload.getFile: bir so'rovda ko'pi bilan 1 MB
const MAX_PAR = 16; // bir vaqtda ketadigan upload.getFile so'rovlari (4 ta ulanishga taqsimlanadi)
const REQ_TIMEOUT = 25000; // javobsiz qolgan so'rov shu vaqtdan keyin qayta yuboriladi
const NET_TRIES = 30; // tarmoq xatosida urinishlar (sekin/uzilgan internetda video to'xtab qolmasin)

function withTimeout(p, ms) {
  let t = 0;
  return Promise.race([
    p,
    new Promise((_, j) => { t = setTimeout(() => j(new Error('chunk_timeout')), ms); }),
  ]).finally(() => clearTimeout(t));
}
let parActive = 0;
// Navbat: `prio` 0 — pleyer HOZIR kutayotgan bo'lak, 1 — oldindan yuklash. Surishdan keyin
// eski joyning hali boshlanmagan oldindan yuklashlari bekor qilinadi (yangi joy ularni
// kutib o'tirmasin).
const parQ = [];
function pump() {
  while (parActive < MAX_PAR && parQ.length) {
    let bi = 0;
    for (let i = 1; i < parQ.length; i++) if (parQ[i].prio < parQ[bi].prio) bi = i;
    const it = parQ.splice(bi, 1)[0];
    if (it.cancelled) { it.reject(new Error('cancelled')); continue; }
    parActive++;
    it.fn().then(it.resolve, it.reject).finally(() => { parActive--; pump(); });
  }
}
function limited(fn, prio = 0, hold) {
  return new Promise((resolve, reject) => {
    const it = { fn, prio, resolve, reject, cancelled: false };
    if (hold) hold(it);
    parQ.push(it);
    pump();
  });
}
function cancelPrefetches() { for (const it of parQ) if (it.prio > 0) it.cancelled = true; }
/** Pleyer ulanishi: `downloadChunk` ni "download" turidagi ulanishlar orqali yuboradi. */
function dlClient(cl) {
  if (cl.__aruDl) return cl.__aruDl;
  cl.__aruDl = new Proxy(cl, {
    get(t, k) {
      if (k === 'call') return (m, o) => t.call(m, { ...(o || {}), kind: 'download' });
      const v = t[k];
      return typeof v === 'function' ? v.bind(t) : v;
    },
  });
  return cl.__aruDl;
}
const docs = new Map(); // nom -> {media, size, msgId}
const keys = new Map(); // nom -> hex
const urls = new Map(); // nom -> objectURL
let scanInfo = '';
const keyChecked = new Set(); // kaliti serverdan so'ralgan nomlar
const delivered = new Set(); // bot chatiga yuborilgan xabarlar (o'chirish uchun)

export function isTelegramAuthorized() { return authorizedCached(); }

/** Telegram'ga kirilgan bo'lishi kerak — bo'lmasa kirish oynasi. */
export async function ensureTelegram() {
  if (await isAuthorized()) return true;
  return openTelegramLogin();
}

let peerP = null;
/** Bot chatining peer'i: avval @username bilan, bo'lmasa `contacts.resolveUsername` orqali. */
async function botPeer() {
  if (peerP) return peerP;
  peerP = (async () => {
    const c = await tgConfig();
    const name = `${c.bot || ''}`.replace(/^@/, '');
    if (!name) throw new Error('bot_unknown');
    const cl = await getClient();
    // Keshdagi (eskirgan access_hash'li) peer'ga ishonmaymiz: avval serverdan yangisini olamiz.
    const r = await cl.call({ _: 'contacts.resolveUsername', username: name });
    const u = (r.users || []).find((x) => `${x.username || ''}`.toLowerCase() === name.toLowerCase()) || r.users?.[0];
    if (!u) throw new Error('bot_unresolved');
    return { _: 'inputPeerUser', userId: u.id, accessHash: u.accessHash };
  })().catch(async (e) => {
    // Tarmoq/FLOOD xatosi bo'lsa — keshdagi peer'ga qaytamiz.
    try { return await (await getClient()).resolvePeer(`@${`${(await tgConfig()).bot || ''}`.replace(/^@/, '')}`); } catch (_) { throw e; }
  });
  peerP.catch(() => { peerP = null; });
  return peerP;
}

function docName(msg) {
  const m = msg?.media;
  if (!m) return '';
  return m.fileName || m.raw?.attributes?.find?.((a) => a._ === 'documentAttributeFilename')?.fileName || '';
}

/** Bot chatining oxirgi 100 xabaridan fayllarni nomi bo'yicha eslab qoladi. */
async function scanBotChat(limit = 100) {
  const cl = await getClient();
  let msgs;
  try {
    msgs = await cl.getHistory(await botPeer(), { limit });
  } catch (e) {
    // Sessiya almashgan/o'chirilgan bo'lsa peer access_hash'i eskirib qoladi — qayta aniqlab, bir marta urinamiz.
    if (!/PEER_ID_INVALID|ACCESS_HASH|USER_BANNED/.test(`${e?.text || e?.message || ''}`)) throw e;
    peerP = null;
    msgs = await cl.getHistory(await botPeer(), { limit });
  }
  scanInfo = `msgs=${msgs.length}`;
  const kinds = [];
  for (const m of [...msgs].reverse()) {
    const n = docName(m);
    if (!n) { if (kinds.length < 3) kinds.push(`${m.media?.type || 'none'}`); continue; }
    docs.set(n, { media: m.media, size: Number(m.media.fileSize || 0), msgId: m.id });
    delivered.add(m.id);
  }
  if (kinds.length) scanInfo += ` unnamed=${kinds.join(',')}`;
}

/** Fayllarni bot chatiga yetkazadi (kerak bo'lsa) va topadi. */
async function locate(names, { force = false } = {}) {
  const need = names.filter((n) => force || !docs.has(n));
  if (need.length) {
    if (!force) {
      // Avval chatning o'zida bormi (oldingi nusxa) — bot'dan qayta so'ramaslik.
      try { await scanBotChat(); } catch (e) { scanInfo = `scan:${e?.text || e?.message || e}`; }
    }
    const still = names.filter((n) => force || !docs.has(n));
    if (still.length) {
      let j;
      try {
        j = await apiPost('/api/tg/deliver', { files: still });
      } catch (e) {
        if (e instanceof ApiError) throw e;
        throw e;
      }
      for (const [k, v] of Object.entries(j?.keys || {})) keys.set(k, `${v}`);
      still.forEach((n) => keyChecked.add(n));
      // Bot nusxani bir-ikki soniyada yuboradi.
      for (let i = 0; i < 20; i++) {
        await new Promise((r) => setTimeout(r, i === 0 ? 250 : 400));
        try { await scanBotChat(Math.min(100, 12 + still.length * 6)); } catch (e) { scanInfo = `scan:${e?.text || e?.message || e}`; }
        if (still.every((n) => docs.has(n))) break;
      }
    }
  }
  for (const n of names) if (!docs.has(n)) throw new Error(`not_in_chat (${scanInfo}; bot=${await botPeer().then(() => 'ok', (e) => e?.text || e?.message || '?')})`);
}

function hexToBytes(hex) {
  const out = new Uint8Array(hex.length / 2);
  for (let i = 0; i < out.length; i++) out[i] = parseInt(hex.substr(i * 2, 2), 16);
  return out;
}

const cryptoKeys = new Map();
async function aesKey(hex) {
  if (!cryptoKeys.has(hex)) {
    cryptoKeys.set(hex, crypto.subtle.importKey('raw', hexToBytes(hex), { name: 'AES-CTR' }, false, ['encrypt', 'decrypt']));
  }
  return cryptoKeys.get(hex);
}

/** AES-128-CTR: `data` fayldagi `offset` dan boshlanadi (offset % 16 == 0). */
export async function ctrApply(hex, offset, data) {
  if (!hex) return data;
  const k = await aesKey(hex);
  const counter = new Uint8Array(16);
  let block = BigInt(Math.floor(offset / 16));
  for (let i = 15; i >= 0 && block > 0n; i--) { counter[i] = Number(block & 0xffn); block >>= 8n; }
  const head = offset % 16;
  if (head === 0) {
    return new Uint8Array(await crypto.subtle.decrypt({ name: 'AES-CTR', counter, length: 128 }, k, data));
  }
  // Blok o'rtasidan — oldiga to'ldiruvchi qo'shib, keyin kesiladi.
  const pad = new Uint8Array(head + data.length);
  pad.set(data, head);
  const outp = new Uint8Array(await crypto.subtle.decrypt({ name: 'AES-CTR', counter, length: 128 }, k, pad));
  return outp.subarray(head);
}

/**
 * Faylni ochadi: `read(offset, length)` kerakli baytlarni (ochilgan holda)
 * qaytaradi. Bo'laklar 1 MB chegarasiga tekislanib olinadi.
 */
export async function openFile(name, { key = '' } = {}) {
  await locate([name]);
  const cl = await getClient();
  let doc = docs.get(name);
  // Fayl bot chatida allaqachon bor edi (deliver chaqirilmagan) — kalit yo'q bo'lishi mumkin: serverdan olinadi.
  if (!key && !keys.get(name) && !keyChecked.has(name)) {
    keyChecked.add(name);
    try {
      const j = await apiPost('/api/tg/deliver', { files: [name] });
      for (const [k, v] of Object.entries(j?.keys || {})) keys.set(k, `${v}`);
    } catch (_) { /* kalitsiz (shifrlanmagan) fayl */ }
  }
  const hex = key || keys.get(name) || '';
  const cache = new Map(); // bo'lak raqami -> Promise<Uint8Array>
  const ORDER = [];
  const MAX_CACHE = 64;
  let closed = false;
  const done = []; // [vaqt, bayt] — tezlikni o'lchash uchun (oxirgi 20 s)
  const opened = Date.now();

  const items = new Map(); // bo'lak -> navbatdagi so'rov (oldindan yuklash talabga aylansa — ustuvorlik oshadi)
  async function chunk(idx, prio = 0) {
    if (cache.has(idx)) { if (prio === 0) { const it = items.get(idx); if (it) it.prio = 0; } return cache.get(idx); }
    const p = (async () => {
      const off = idx * CHUNK;
      const lim = Math.min(CHUNK, doc.size - off);
      let refreshes = 0;
      for (let attempt = 0; attempt < NET_TRIES; attempt++) {
        if (closed) throw new Error('aborted');
        try {
          // Taymaut `limited` ichida: osilib qolgan so'rov navbatdagi o'rinni band qilib turmaydi.
          // Diskda bor bo'lsa — tarmoqsiz (kalit: fayl nomi + bo'lak; nom har yuklashda yangi).
          const hit = await cacheGet(`${name}#${idx}`, lim);
          if (hit) {
            done.push([Date.now(), hit.length]);
            return await ctrApply(hex, off, hit);
          }
          const dc = Number.isInteger(doc.media?.dcId) ? doc.media.dcId : undefined;
          const raw = await limited(
            () => withTimeout(downloadChunk(dlClient(cl), { location: doc.media, offset: off, limit: CHUNK, ...(dc ? { dcId: dc } : {}) }), REQ_TIMEOUT),
            prio, (it) => items.set(idx, it));
          const part = raw.length > lim ? raw.subarray(0, lim) : raw;
          done.push([Date.now(), part.length]);
          if (done.length > 200) done.splice(0, done.length - 200);
          if (part.length === lim && doc.size > 4 * 1024 * 1024) cachePut(`${name}#${idx}`, part); // faqat katta fayllar (video)
          return await ctrApply(hex, off, part);
        } catch (e) {
          const t = `${e?.text || e?.message || ''}`;
          if (/FILE_REFERENCE|MEDIA_EMPTY|not_in_chat/.test(t) && refreshes < 4) {
            // Nusxa eskirgan yoki o'chirilgan — qayta yetkaziladi.
            refreshes += 1;
            docs.delete(name);
            await locate([name], { force: true });
            doc = docs.get(name);
            continue;
          }
          // Tuzalmaydigan xatolar (hisob/ruxsat) — darhol; tarmoq xatolari — qayta-qayta.
          if (/AUTH_KEY|SESSION_REVOKED|USER_DEACTIVATED|FILE_ID_INVALID|LIMIT_INVALID|OFFSET_INVALID/.test(t)) throw e;
          if (attempt === NET_TRIES - 1) throw e;
          const fw = /FLOOD(?:_PREMIUM)?_WAIT_(\d+)/.exec(t);
          await new Promise((r) => setTimeout(r, fw ? Math.min(15, +fw[1]) * 1000 : Math.min(5000, 400 * 2 ** Math.min(attempt, 4))));
        }
      }
      throw new Error('download_failed');
    })();
    cache.set(idx, p);
    ORDER.push(idx);
    while (ORDER.length > MAX_CACHE) cache.delete(ORDER.shift());
    p.catch(() => cache.delete(idx));
    p.finally(() => items.delete(idx)).catch(() => {});
    return p;
  }

  return {
    get size() { return doc.size; },
    name,
    /** `offset` dan `length` bayt (fayl oxirida qisqaroq). */
    async read(offset, length) {
      const end = Math.min(doc.size, offset + length);
      if (end <= offset) return new Uint8Array(0);
      const first = Math.floor(offset / CHUNK);
      const last = Math.floor((end - 1) / CHUNK);
      const parts = await Promise.all(Array.from({ length: last - first + 1 }, (_, i) => chunk(first + i)));
      const out = new Uint8Array(end - offset);
      let pos = 0;
      parts.forEach((p, i) => {
        const base = (first + i) * CHUNK;
        const s = Math.max(0, offset - base);
        const e = Math.min(p.length, end - base);
        out.set(p.subarray(s, e), pos);
        pos += e - s;
      });
      return out;
    },
    /** Oldindan yuklab qo'yish (pleyer buferi uchun). */
    prefetch(offset, length) {
      const end = Math.min(doc.size, offset + length);
      for (let i = Math.floor(offset / CHUNK); i <= Math.floor((end - 1) / CHUNK); i++) chunk(i, 1).catch(() => {});
    },
    /** Surishdan keyin: hali boshlanmagan oldindan yuklashlarni bekor qiladi. */
    cancelPrefetch() { cancelPrefetches(); },
    /** So'nggi ~20 s dagi yuklash tezligi (kbit/s). */
    rateKbps() {
      const now = Date.now();
      const win = Math.max(3000, Math.min(20000, now - opened));
      let b = 0;
      for (const [t, n] of done) if (now - t <= win) b += n;
      return Math.round((b * 8) / win);
    },
    close() { closed = true; cache.clear(); },
  };
}

/** Bir nechta faylni BITTA so'rov bilan yetkazadi (xato bo'lsa jim). */
export async function prefetchNames(names) {
  const need = names.filter((n) => !docs.has(n));
  if (!need.length) return;
  try { await locate(need); } catch (_) { /* topilganlari docs'da qoladi */ }
}

/** Kichik fayl — butunlay (rasm, ovozli xabar). */
export async function fetchFile(name, { key = '', type = '' } = {}) {
  const f = await openFile(name, { key });
  const bytes = await f.read(0, f.size);
  f.close();
  return new Blob([bytes], { type: type || guessType(name) });
}

export async function mediaUrl(name, opts = {}) {
  if (urls.has(name)) return urls.get(name);
  const ok = await isAuthorized();
  if (!ok) throw new Error('tg_not_ready');
  const blob = await fetchFile(name, opts);
  const u = URL.createObjectURL(blob);
  urls.set(name, u);
  return u;
}

function guessType(name) {
  const n = name.toLowerCase();
  if (/\.(jpe?g)$/.test(n)) return 'image/jpeg';
  if (n.endsWith('.png')) return 'image/png';
  if (n.endsWith('.webp')) return 'image/webp';
  if (n.endsWith('.gif')) return 'image/gif';
  if (n.endsWith('.mp4')) return 'video/mp4';
  if (n.endsWith('.webm')) return 'video/webm';
  if (/\.(m4a|aac)$/.test(n)) return 'audio/mp4';
  if (n.endsWith('.ogg') || n.endsWith('.oga')) return 'audio/ogg';
  return 'application/octet-stream';
}

/**
 * Fayl yuborish (yozishma, avatar va h.k.) — ilovadagidek: fayl SHIFRLANIB
 * foydalanuvchining bot chatiga yuboriladi, bot uni kanalga ko'chiradi,
 * sayt esa `/api/tg/claim` orqali tayyor bo'lishini kutadi.
 */
export async function uploadFile(file, name, { waitForClaim = true, onProgress } = {}) {
  if (!(await ensureTelegram())) throw new Error('tg_not_ready');
  const cl = await getClient();
  const keyBytes = crypto.getRandomValues(new Uint8Array(16));
  const hex = [...keyBytes].map((b) => b.toString(16).padStart(2, '0')).join('');
  const plain = new Uint8Array(await file.arrayBuffer());
  // CTR: shifrlash va ochish bir xil amal.
  const sealed = await ctrApply(hex, 0, plain);
  const peer = await botPeer();
  const sent = await cl.sendMedia(peer, {
    type: 'document',
    file: sealed,
    fileName: name,
    fileMime: 'application/octet-stream',
  }, onProgress ? { progressCallback: (a, b) => { try { onProgress(a, b || sealed.length); } catch (_) { /* */ } } } : undefined);
  if (sent?.id) delivered.add(sent.id);
  keys.set(name, hex);
  // Fayl Telegram'da — kanalga ko'chirishni bot o'zi qiladi. Ilovadagidek
  // (`waitForClaim: false`) kutish fonda: sekin internetda "yuklab bo'lmadi" chiqmaydi.
  const claim = async () => {
    for (const w of [1, 1, 2, 2, 3, 4, 5, 8, 10, 15]) {
      await new Promise((r) => setTimeout(r, w * 1000));
      const j = await api(`/api/tg/claim?file=${encodeURIComponent(name)}&key=${hex}`).catch(() => null);
      if (j?.ready) return true;
    }
    return false;
  };
  if (!waitForClaim) { claim(); return { name, key: hex }; }
  if (await claim()) return { name, key: hex };
  throw new Error('upload_not_claimed');
}

/**
 * Bot chatidagi ESKI nusxalarni o'chiradi — faqat Mini App ochilganda
 * (`app.js`). Chatning hamma xabarlari sahifalab o'qilib o'chiriladi.
 */
export async function clearOldBotChat() {
  try {
    const cl = await getClient();
    const peer = await botPeer();
    for (let page = 0; page < 20; page++) {
      const msgs = await cl.getHistory(peer, { limit: 100 });
      if (!msgs.length) break;
      await cl.deleteMessagesById(peer, msgs.map((m) => m.id), { revoke: true });
      if (msgs.length < 100) break;
    }
  } catch (_) { /* keyingi ochilishda */ }
  docs.clear();
  delivered.clear();
}

/** Tarmoq bilan qayta tekshiradi. */
export async function isAuthorizedFresh() { return isAuthorized({ fresh: true }); }

/** Telegram hisobidan chiqish (faqat shu brauzer sessiyasi). */
export async function logout() {
  const { tgLogout } = await import('./client.js');
  await tgLogout();
}

/** Ulangan Telegram hisobi (ism, raqam, username) yoki null. */
export async function me() {
  if (!(await isAuthorized())) return null;
  const cl = await getClient();
  const u = await cl.getMe();
  return { id: u.id, firstName: u.firstName, lastName: u.lastName, username: u.username, phone: u.phoneNumber };
}

/**
 * Majburiy obuna kanaliga qo'shilish (ilovadagi `rust_tg_join_channel`).
 * `kind`: public (`https://t.me/nom`) — a'zo bo'ladi; private (qo'shilish
 * so'rovi bilan havola) — so'rov yuboradi. `{ok}` yoki `{error, wait?}`.
 */
export async function joinChannel(kind, url) {
  // Ilovadagi `join_one` (rust/src/telegram.rs) bilan bir xil: ochiq — resolveUsername + joinChannel,
  // yopiq — checkChatInvite + importChatInvite. (`joinChat(url)` to'liq t.me havolani tushunmasdi.)
  try {
    const cl = await getClient();
    const raw = `${url || ''}`.trim();
    const hashM = /(?:t\.me\/\+|t\.me\/joinchat\/|^\+)([A-Za-z0-9_-]+)/i.exec(raw);
    if (kind === 'private' || hashM) {
      const hash = hashM?.[1];
      if (!hash) return { ok: false, error: 'invite_hash_invalid' };
      const chk = await cl.call({ _: 'messages.checkChatInvite', hash });
      if (chk?._ === 'chatInviteAlready') return { ok: true };
      await cl.call({ _: 'messages.importChatInvite', hash });
      return { ok: true };
    }
    const name = raw.replace(/^https?:\/\//i, '').replace(/^(?:www\.)?t\.me\//i, '').replace(/^@/, '').split(/[/?#]/)[0];
    if (!name) return { ok: false, error: 'USERNAME_INVALID' };
    const r = await cl.call({ _: 'contacts.resolveUsername', username: name });
    const ch = (r.chats || []).find((c) => c._ === 'channel' && c.accessHash != null);
    if (!ch) return { ok: false, error: 'USERNAME_NOT_OCCUPIED' };
    await cl.call({ _: 'channels.joinChannel', channel: { _: 'inputChannel', channelId: ch.id, accessHash: ch.accessHash } });
    return { ok: true };
  } catch (e) {
    const t = `${e?.text || e?.message || e}`;
    // Yopiq kanal: so'rov yuborildi — bu muvaffaqiyat.
    if (t.includes('INVITE_REQUEST_SENT') || t.includes('USER_ALREADY_PARTICIPANT')) return { ok: true };
    const m = t.match(/FLOOD(?:_PREMIUM)?_WAIT_(\d+)/);
    return { ok: false, error: t, wait: m ? Number(m[1]) : undefined };
  }
}
