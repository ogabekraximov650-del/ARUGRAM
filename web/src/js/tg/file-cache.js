// Keshdagi kichik fayllar (rasm, ovoz) uchun objectURL — mtcute YUKLANMAGAN holda, Telegram'ga
// va bot nusxasiga so'rovsiz. Diskda faqat Telegram'dan kelgan SHIFRLANGAN bo'laklar turadi
// (`chunk-cache.js`); ochish kaliti diskda yo'q — `ensureKeys` bilan onlayn olinadi (`ctr.js`).
import { cacheGet, metaGet } from './chunk-cache.js';
import { keys, ensureKeys, ctrApply, guessType } from './ctr.js';

const urls = new Map(); // nom -> objectURL
const CHUNK = 1024 * 1024;

/** Keshda to'liq bo'lsa — objectURL (kalit xotirada bo'lishi kerak: `ensureKeys` oldin), bo'lmasa null. */
export async function cachedMediaUrl(name) {
  if (urls.has(name)) return urls.get(name);
  const meta = await metaGet(name);
  const size = Number(meta?.size) || 0;
  if (size <= 0) return null;
  const n = Math.ceil(size / CHUNK);
  const parts = await Promise.all(Array.from({ length: n }, (_, i) => cacheGet(`${name}#${i}`, Math.min(CHUNK, size - i * CHUNK))));
  if (parts.some((p) => !p)) return null;
  const hex = keys.get(name) || '';
  const plain = await Promise.all(parts.map((p, i) => ctrApply(hex, i * CHUNK, p)));
  const u = URL.createObjectURL(new Blob(plain, { type: guessType(name) }));
  urls.set(name, u);
  return u;
}

export function rememberUrl(name, url) { urls.set(name, url); }
