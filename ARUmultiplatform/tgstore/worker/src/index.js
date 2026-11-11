// UZ: tgstore-test worker: Telegram bo'laklar bazasi + Cloudflare kesh. R2 YO'Q.
//     Yozish/tahrir/o'chirish -> Durable Object (guruh-commit). O'qish -> kesh (o'zgarmas versiyali URL) -> Telegram.
// EN: tgstore-test worker: Telegram chunk database + Cloudflare cache. NO R2.
//     Write/edit/delete -> Durable Object (group commit). Read -> cache (immutable versioned URL) -> Telegram.
// RU: worker tgstore-test: baza iz chankov Telegram + kesh Cloudflare. BEZ R2.
//     Zapis/pravka/udalenie -> Durable Object (gruppovoy kommit). Chtenie -> kesh (neizmennyy URL s versiey) -> Telegram.
import { decodeChunk, validTable, timingSafeEq } from './core.js';
import * as tg from './tg.js';
export { TableDO } from './table_do.js';

const J = (o, status = 200, h = {}) => new Response(JSON.stringify(o), { status, headers: { 'content-type': 'application/json', ...h } });
const lru = new Map(); // UZ: izolyat ichidagi 2-qatlam / EN: in-isolate 2nd layer / RU: vtoroy sloy v izolyate
const LRU_MAX = 24;
const lruGet = (k) => { const v = lru.get(k); if (v) { lru.delete(k); lru.set(k, v); } return v; };
const lruSet = (k, v) => { lru.set(k, v); while (lru.size > LRU_MAX) lru.delete(lru.keys().next().value); };

const stub = (env, table) => env.TABLE.get(env.TABLE.idFromName(table));
async function doCall(env, table, path, init = {}) {
  const r = await stub(env, table).fetch('https://do' + path, { ...init, headers: { 'x-table': table, ...(init.headers || {}) } });
  const body = await r.json();
  if (!r.ok) throw Object.assign(new Error(body.error || 'do error'), { status: r.status });
  return body;
}

/** UZ: o'zgarmas versiyali bo'lak baytlari: kesh -> izolyat -> Telegram / EN: immutable chunk bytes / RU: neizmennye bayty chanka */
async function chunkBytes(env, ctx, table, no, ver) {
  const ck = new Request(`https://tgstore.invalid/c/${table}/${no}/${ver}`);
  const lk = `${table}/${no}/${ver}`;
  const mem = lruGet(lk);
  if (mem) return { bytes: mem, src: 'isolate' };
  const hit = await caches.default.match(ck);
  if (hit) { const b = new Uint8Array(await hit.arrayBuffer()); lruSet(lk, b); return { bytes: b, src: 'cache' }; }
  const info = await doCall(env, table, `/chunk?no=${no}&ver=${ver}`);
  if (!info.found) { const e = new Error('chunk version gone'); e.status = 404; throw e; }
  const bytes = await tg.download(env, info.file_id);
  lruSet(lk, bytes);
  const resp = new Response(bytes, { headers: { 'cache-control': 'public, max-age=31536000, immutable', 'content-type': 'application/octet-stream' } });
  ctx.waitUntil(caches.default.put(ck, resp));
  return { bytes, src: 'telegram' };
}

export default {
  async fetch(req, env, ctx) {
    const u = new URL(req.url);
    if (u.pathname === '/health') return J({ ok: true, time: Date.now() });
    if (!env.STORE_KEY || !timingSafeEq(req.headers.get('authorization') || '', 'Bearer ' + env.STORE_KEY)) return J({ error: 'unauthorized' }, 401);
    const p = u.pathname.split('/').filter(Boolean); // [kind, table, ...]
    const [kind, table] = p;
    try {
      if (kind === 'me') return J(await tg.getMe(env));
      if (!validTable(table)) return J({ error: 'bad table' }, 400);

      // O'QISH: bitta yozuv / READ: one record / CHTENIE: odna zapis
      if (kind === 'r' && req.method === 'GET' && p[2]) {
        const key = decodeURIComponent(p.slice(2).join('/'));
        const loc = await doCall(env, table, '/locate?k=' + encodeURIComponent(key));
        if (!loc.found) return J({ error: 'not found' }, 404);
        const { bytes, src } = await chunkBytes(env, ctx, table, loc.no, loc.ver);
        const map = decodeChunk(bytes);
        if (!map.has(key)) return J({ error: 'not found' }, 404);
        return J({ k: key, v: map.get(key), no: loc.no, ver: loc.ver }, 200, { 'x-src': src });
      }
      // O'QISH: butun bo'lak (ilova o'zi eslab qoladi) / READ: whole chunk (app remembers) / CHTENIE: ves chank
      if (kind === 'c' && req.method === 'GET') {
        const { bytes, src } = await chunkBytes(env, ctx, table, +p[2], +p[3]);
        return new Response(bytes, { headers: { 'content-type': 'application/octet-stream', 'x-src': src, 'cache-control': 'private, max-age=31536000, immutable' } });
      }
      if (kind === 'w' && req.method === 'POST') return J(await doCall(env, table, '/write', { method: 'POST', body: await req.text() }));
      if (kind === 'i') return J(await doCall(env, table, '/chunks'));
      if (kind === 'k') return J(await doCall(env, table, '/keys' + u.search));
      if (kind === 'stats') return J(await doCall(env, table, '/stats'));
      if (kind === 'gc' && req.method === 'POST') return J(await doCall(env, table, '/gc'));
      if (kind === 'backup' && req.method === 'POST') return J(await doCall(env, table, '/backup'));
      if (kind === 'restore' && req.method === 'POST') return J(await doCall(env, table, '/restore'));
      return J({ error: 'not found' }, 404);
    } catch (e) {
      return J({ error: String(e?.message || e) }, e?.status || 500);
    }
  },
};
