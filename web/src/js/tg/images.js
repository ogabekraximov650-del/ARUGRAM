// Rasmlarni Telegram orqali yuklash — ilovadagi `_TelegramFileService`
// (`image_cache.dart`) nusxasi: `/api/image/...` rasmlari worker'dan emas,
// foydalanuvchining Telegram hisobi bilan bot chatidan olinadi.
//
// `api.js -> imageUrl` Telegram ulangan bo'lsa `<img>` ga 1x1 rasm va
// fayl nomini beradi; bu modul uni kuzatib, haqiqiy rasmni qo'yadi.
// Telegram'dan olib bo'lmasa — oddiy (worker) manzilga qaytadi.

import { mediaUrl, prefetchNames, isTelegramAuthorized } from './media.js';
import { startup, withTimeout } from './startup.js';
import { cachedMediaUrl } from './file-cache.js';
import { ensureKeys } from './ctr.js';

const queue = new Map(); // nom -> [{img, orig}]
let timer = 0;
let shown = false;

const nameOf = (src) => { const m = /\/api\/(?:image|media)\/([^?#]+)/.exec(src); return m ? decodeURIComponent(m[1]) : ''; };
const tgOn = () => { try { return localStorage.getItem('aru_tg_on') === '1'; } catch (_) { return false; } };

function consider(img) {
  if (!(img instanceof HTMLImageElement) || img.dataset.aru) return;
  const src = img.getAttribute('src') || '';
  let name = ''; let orig = '';
  const i = src.indexOf('#aru=');
  if (src.startsWith('data:') && i > 0) {
    const p = new URLSearchParams(src.slice(i + 1));
    name = p.get('aru') || ''; orig = p.get('o') || '';
  } else if (tgOn() && /\/api\/(image|media)\//.test(src)) {
    name = nameOf(src); orig = src;
  }
  if (!name) return;
  img.dataset.aru = '1';
  img.decoding = 'async';
  if (!queue.has(name)) queue.set(name, []);
  queue.get(name).push({ img, orig });
  clearTimeout(timer);
  timer = setTimeout(flush, 120);
}

function setSrc(list, url) { list.forEach(({ img }) => { if (img.isConnected) img.src = url; }); }

async function flush() {
  const batch = [...queue.entries()];
  queue.clear();
  if (!batch.length) return;
  // Avval DISK keshi: bor rasmlar Telegram'ga umuman so'rovsiz, darhol chiqadi.
  const rest = [];
  // Kalitlar onlayn (bitta so'rov, bot nusxasiz); tarmoq yo'q bo'lsa kesh ishlatilmaydi.
  let keyOk = true;
  try { await ensureKeys(batch.map((b) => b[0])); } catch (_) { keyOk = false; }
  await Promise.all(batch.map(async (b) => {
    if (!keyOk) { rest.push(b); return; }
    const u = await cachedMediaUrl(b[0]).catch(() => null);
    if (u) setSrc(b[1], u); else rest.push(b);
  }));
  batch.length = 0; batch.push(...rest);
  if (!batch.length) return;
  await startup();
  const names = batch.map(([n]) => n);
  await prefetchNames(names).catch(() => {});
  let i = 0;
  const worker = async () => {
    while (i < batch.length) {
      const [name, list] = batch[i++];
      try { setSrc(list, await withTimeout(mediaUrl(name), 60000, 'img_timeout')); } catch (e) {
        if (!shown) { shown = true; try { window.Telegram?.WebApp?.showAlert?.(`Rasm yuklanmadi: ${e?.text || e?.message || e}`); } catch (_) { /* */ } }
        list.forEach(({ img, orig }) => { if (img.isConnected && orig) img.src = orig; });
      }
    }
  };
  await Promise.all([worker(), worker(), worker(), worker()]);
}

export function scanImages(root = document) { root.querySelectorAll('img').forEach(consider); }

export function watchImages(root) {
  new MutationObserver((muts) => {
    for (const m of muts) {
      if (m.type === 'attributes') consider(m.target);
      m.addedNodes.forEach((n) => {
        if (n.nodeType !== 1) return;
        if (n.tagName === 'IMG') consider(n); else n.querySelectorAll?.('img').forEach(consider);
      });
    }
  }).observe(root, { childList: true, subtree: true, attributes: true, attributeFilter: ['src'] });
  scanImages(root);
}

/** Telegram'ga kirilgach — hozir ekrandagi worker rasmlari qayta yuklanadi. */
export function rescanImages() {
  document.querySelectorAll('img').forEach((img) => { if (!img.dataset.aru) consider(img); });
}

export { isTelegramAuthorized };
