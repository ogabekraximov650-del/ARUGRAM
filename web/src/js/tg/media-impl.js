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
import { getClient, tgConfig, isAuthorized, authorizedCached } from './client.js';
import { openTelegramLogin } from './login.js';

const CHUNK = 1024 * 1024;
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
async function scanBotChat() {
  const cl = await getClient();
  let msgs;
  try {
    msgs = await cl.getHistory(await botPeer(), { limit: 100 });
  } catch (e) {
    // Sessiya almashgan/o'chirilgan bo'lsa peer access_hash'i eskirib qoladi — qayta aniqlab, bir marta urinamiz.
    if (!/PEER_ID_INVALID|ACCESS_HASH|USER_BANNED/.test(`${e?.text || e?.message || ''}`)) throw e;
    peerP = null;
    msgs = await cl.getHistory(await botPeer(), { limit: 100 });
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
      for (let i = 0; i < 12; i++) {
        await new Promise((r) => setTimeout(r, i === 0 ? 400 : 700));
        try { await scanBotChat(); } catch (e) { scanInfo = `scan:${e?.text || e?.message || e}`; }
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
  const MAX_CACHE = 24;

  async function chunk(idx) {
    if (cache.has(idx)) return cache.get(idx);
    const p = (async () => {
      const off = idx * CHUNK;
      const lim = Math.min(CHUNK, doc.size - off);
      for (let attempt = 0; attempt < 3; attempt++) {
        try {
          const raw = await cl.downloadChunk({ location: doc.media, offset: off, limit: CHUNK });
          const part = raw.length > lim ? raw.subarray(0, lim) : raw;
          return await ctrApply(hex, off, part);
        } catch (e) {
          const t = `${e?.text || e?.message || ''}`;
          if (/FILE_REFERENCE|MEDIA_EMPTY|not_in_chat/.test(t) && attempt < 2) {
            // Nusxa eskirgan yoki o'chirilgan — qayta yetkaziladi.
            docs.delete(name);
            await locate([name], { force: true });
            doc = docs.get(name);
            continue;
          }
          if (attempt === 2) throw e;
          await new Promise((r) => setTimeout(r, 500 * (attempt + 1)));
        }
      }
      throw new Error('download_failed');
    })();
    cache.set(idx, p);
    ORDER.push(idx);
    while (ORDER.length > MAX_CACHE) cache.delete(ORDER.shift());
    p.catch(() => cache.delete(idx));
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
      for (let i = Math.floor(offset / CHUNK); i <= Math.floor((end - 1) / CHUNK); i++) chunk(i).catch(() => {});
    },
    close() { cache.clear(); },
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
export async function uploadFile(file, name) {
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
  });
  if (sent?.id) delivered.add(sent.id);
  keys.set(name, hex);
  for (let i = 0; i < 40; i++) {
    await new Promise((r) => setTimeout(r, 750));
    const j = await api(`/api/tg/claim?file=${encodeURIComponent(name)}&key=${hex}`).catch(() => null);
    if (j?.ready) return { name, key: hex };
  }
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
  try {
    const cl = await getClient();
    await cl.joinChat(url);
    return { ok: true };
  } catch (e) {
    const t = `${e?.text || e?.message || e}`;
    // Yopiq kanal: so'rov yuborildi — bu muvaffaqiyat.
    if (t.includes('INVITE_REQUEST_SENT') || t.includes('USER_ALREADY_PARTICIPANT')) return { ok: true };
    const m = t.match(/FLOOD(?:_PREMIUM)?_WAIT_(\d+)/);
    return { ok: false, error: t, wait: m ? Number(m[1]) : undefined };
  }
}
