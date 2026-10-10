// Keshdagi kichik fayllar (rasm, ovoz) uchun objectURL — Telegram'ga umuman so'rovsiz,
// mtcute yuklanmagan holda ham (`tg/chunk-cache.js`). `images.js` birinchi shuni so'raydi.
import { fileGet, filePut } from './chunk-cache.js';

const urls = new Map(); // nom -> objectURL

/** Keshda bo'lsa — objectURL (darhol), bo'lmasa null. */
export async function cachedMediaUrl(name) {
  if (urls.has(name)) return urls.get(name);
  const hit = await fileGet(name);
  if (!hit) return null;
  const u = URL.createObjectURL(new Blob([hit.bytes], { type: hit.type || 'application/octet-stream' }));
  urls.set(name, u);
  return u;
}

/** Telegram'dan olingan faylni keshga yozadi. */
export function rememberFile(name, bytes, type) { return filePut(name, bytes, type); }
export function rememberUrl(name, url) { urls.set(name, url); }
