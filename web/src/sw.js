// Mini App qobig'i keshi (Service Worker): ilova (HTML/JS/CSS), shriftlar va Telegram skripti keshda saqlanadi —
// keyingi ochilishda tarmoqsiz DARHOL yuklanadi, yangilanish orqa fonda (stale-while-revalidate).
// API (boshqa domen: worker) va video/fayllarga TEGMAYDI. `__V__` — `build.mjs` har yig'ishda almashtiradi.

const V = '__V__';
const CACHE = `aru-${V}`;
const EXTRA = new Set(['fonts.googleapis.com', 'fonts.gstatic.com', 'telegram.org']);

self.addEventListener('install', () => { self.skipWaiting(); });

self.addEventListener('activate', (e) => {
  e.waitUntil((async () => {
    for (const k of await caches.keys()) if (k.startsWith('aru-') && k !== CACHE) await caches.delete(k);
    await self.clients.claim();
  })());
});

const okToStore = (res) => res && (res.ok || res.type === 'opaque');

async function cacheFirst(req) {
  const c = await caches.open(CACHE);
  const hit = await c.match(req);
  if (hit) return hit;
  const res = await fetch(req);
  if (okToStore(res)) c.put(req, res.clone()).catch(() => {});
  return res;
}

// Keshdan darhol; orqa fonda yangilanadi. `key` — keshdagi kalit (sahifa `?w=...` bilan ochilsa ham bitta nusxa).
async function swr(req, key, e) {
  const c = await caches.open(CACHE);
  const hit = await c.match(key);
  const update = fetch(req).then((res) => {
    if (okToStore(res)) c.put(key, res.clone()).catch(() => {});
    return res;
  });
  if (hit) { e.waitUntil(update.catch(() => {})); return hit; }
  return update;
}

self.addEventListener('fetch', (e) => {
  const r = e.request;
  if (r.method !== 'GET' || r.headers.has('range')) return;
  const u = new URL(r.url);
  const same = u.origin === self.location.origin;
  if (!same && !EXTRA.has(u.hostname)) return;
  if (same && u.pathname.endsWith('/sw.js')) return;
  try {
    if (r.mode === 'navigate') { e.respondWith(swr(r, new Request(self.registration.scope), e).catch(() => fetch(r))); return; }
    const immutable = (same && /\/(chunks|assets)\//.test(u.pathname)) || u.hostname === 'fonts.gstatic.com';
    e.respondWith((immutable ? cacheFirst(r) : swr(r, r, e)).catch(() => fetch(r)));
  } catch (_) { /* tarmoqqa */ }
});
