// Mini App video oqimi (Service Worker): oddiy MP4 (`url_<sifat>`) ni brauzer <video> ga
// "Range" so'rovlari bilan beradi. Baytlarni sahifaning o'zi Telegram'dan olib ochadi
// (`player/mp4-stream.js`); worker/server orqali bayt o'tmaydi.
//   /__aru/mp4/<id>.mp4   — virtual manzil; so'rov -> sahifaga xabar -> baytlar -> 206 javob.
const PART = 4 * 1024 * 1024; // bitta javobda ko'pi bilan (ochiq `bytes=N-` so'rovlari uchun)

self.addEventListener('install', () => self.skipWaiting());
self.addEventListener('activate', (e) => e.waitUntil(self.clients.claim()));

async function ask(client, msg) {
  return new Promise((resolve, reject) => {
    const ch = new MessageChannel();
    const t = setTimeout(() => reject(new Error('timeout')), 45000);
    ch.port1.onmessage = (ev) => { clearTimeout(t); ev.data?.error ? reject(new Error(ev.data.error)) : resolve(ev.data); };
    client.postMessage(msg, [ch.port2]);
  });
}

async function pickClient(event, id) {
  if (event.clientId) { const c = await self.clients.get(event.clientId); if (c) return c; }
  const all = await self.clients.matchAll({ type: 'window', includeUncontrolled: true });
  return all.find((c) => c.visibilityState === 'visible') || all[0] || null;
}

self.addEventListener('fetch', (event) => {
  const u = new URL(event.request.url);
  const m = /^\/__aru\/mp4\/([A-Za-z0-9_-]+)\.mp4$/.exec(u.pathname);
  if (!m) return;
  event.respondWith((async () => {
    const id = m[1];
    const client = await pickClient(event, id);
    if (!client) return new Response('no client', { status: 503 });
    try {
      const { size } = await ask(client, { t: 'aru-stat', id });
      const hdr = event.request.headers.get('Range');
      let start = 0; let end = size - 1; let partial = false;
      const r = hdr && /bytes=(\d*)-(\d*)/.exec(hdr);
      if (r) {
        partial = true;
        if (r[1] === '' && r[2] !== '') { start = Math.max(0, size - parseInt(r[2], 10)); } else {
          start = parseInt(r[1] || '0', 10);
          if (r[2] !== '') end = Math.min(size - 1, parseInt(r[2], 10));
        }
      }
      if (start >= size) return new Response('', { status: 416, headers: { 'Content-Range': `bytes */${size}` } });
      end = Math.min(end, start + PART - 1, size - 1);
      const { buf } = await ask(client, { t: 'aru-read', id, offset: start, length: end - start + 1 });
      const headers = {
        'Content-Type': 'video/mp4', 'Accept-Ranges': 'bytes', 'Content-Length': `${buf.byteLength}`,
        'Cache-Control': 'no-store',
      };
      if (partial || end < size - 1) {
        headers['Content-Range'] = `bytes ${start}-${start + buf.byteLength - 1}/${size}`;
        return new Response(buf, { status: 206, headers });
      }
      return new Response(buf, { status: 200, headers });
    } catch (e) {
      return new Response(`${e?.message || e}`, { status: 502 });
    }
  })());
});
