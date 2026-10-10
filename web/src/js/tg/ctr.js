// AES-128-CTR (fayl ochish), fayl kalitlari va fayl turi — mtcute'ga bog'liq emas.
// Kalitlar DISKKA YOZILMAYDI: faqat shu sessiya xotirasida; keshdagi fayllarni ochish uchun
// har safar serverdan ONLAYN olinadi (`ensureKeys`, `keys_only` — bot nusxasiz, tez).

import { apiPost } from '../api.js';

export const keys = new Map(); // nom -> hex (faqat xotirada)

/** Kalitlari hali yo'q nomlar uchun bitta so'rov (yuzgacha). Tarmoq yo'q bo'lsa — xato. */
export async function ensureKeys(names) {
  const need = [...new Set(names)].filter((n) => !keys.has(n));
  for (let i = 0; i < need.length; i += 100) {
    const j = await apiPost('/api/tg/deliver', { files: need.slice(i, i + 100), keys_only: true });
    for (const [k, v] of Object.entries(j?.keys || {})) keys.set(k, `${v}`);
  }
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

export function guessType(name) {
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
