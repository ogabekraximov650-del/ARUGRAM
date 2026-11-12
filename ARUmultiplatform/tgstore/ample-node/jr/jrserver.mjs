// UZ: JURNAL-BAZA serveri (Ample, qo'lda xizmat): Turso `pipeline` formatidagi so'rovlarni SQLite'da bajaradi; yozuvlar `db_N_vV` jurnal papkalariga tushadi va
//     Telegram'ga nusxalanadi (`jstore.mjs`). Hech qanday tashqi kutubxona kerak emas (faqat Node 22: `node:sqlite`, `fetch`).
// EN: journal-store server (Ample hands-on service): runs Turso-pipeline requests on SQLite; writes land in `db_N_vV` journal folders and replicate to Telegram.
// RU: сервер журнальной базы (Ample): запросы формата Turso pipeline на SQLite, журнал в папках db_N_vV и копия в Telegram.
import http from 'node:http';
import crypto from 'node:crypto';
import fs from 'node:fs';
import { JStore } from './jstore.mjs';
import { makeTelegram } from '../archive.mjs';

const E = process.env;
const log = (...a) => console.log(new Date().toISOString().slice(11, 23), ...a);
const h256 = (x) => crypto.createHash('sha256').update(x).digest('hex');
const AUTHS = new Set([E.STORE_KEY && h256(E.STORE_KEY), E.ADMIN_KEY && h256(E.ADMIN_KEY)].filter(Boolean));
const DIR = E.JR_DIR || '/var/lib/tgstore-data';
const LIMIT_MB = Number(E.DB_LIMIT_MB || 1024);
const tg = (E.TG_BOT_TOKEN && E.TG_CHANNEL_ID) ? makeTelegram({ token: E.TG_BOT_TOKEN, channel: E.TG_CHANNEL_ID, mock: false }) : (E.TG_MOCK === '1' ? makeTelegram({ mock: true }) : null);
const cfg = { dir: DIR, tg, key: E.JR_KEY || null, chunkBytes: Number(E.JR_CHUNK_BYTES || 1048576), limitMb: LIMIT_MB, log };

let jr = null;
async function start() {
  if (!fs.existsSync(`${DIR}/main.db`) && tg && cfg.key) {
    try { const r = await JStore.recover(cfg); jr = r.store; log('Telegram\'dan tiklandi, seq', r.seq, 'qayta o\'ynaldi', r.replayed); }
    catch (e) { log('tiklab bo\'lmadi (yangi bo\'sh baza ochiladi):', e.message); }
  }
  if (!jr) jr = new JStore(cfg).init();
  jr.exec([{ sql: 'create table if not exists kv_conf(k text primary key, v text not null)' }], { journal: false });
  const r = jr.exec([{ sql: "select v from kv_conf where k='key_hash'" }])[0];
  if (r.ok && r.result.rows[0]) AUTHS.add(r.result.rows[0][0].value);
  if (tg && cfg.key) {
    setInterval(() => { jr.replicate().catch((e) => log('replicate xato:', e.message)); }, 20_000).unref();
    setInterval(() => { if (jr.needSnapshot()) jr.snapshot().catch((e) => log('snapshot xato:', e.message)); }, 5 * 60_000).unref();
  }
}
const ready = start().catch((e) => { log('BOSHLASH XATO', e.stack || e.message); process.exit(1); });

function applySchema() {
  const schema = fs.readFileSync(new URL('./schema.sql', import.meta.url), 'utf8').split(/;\s*\n/).map((x) => x.trim()).filter(Boolean);
  let n = 0;
  for (const q of schema) { const r = jr.exec([{ sql: q }]); if (r[0].ok === false && !/already exists|duplicate column/.test(r[0].error)) throw new Error('sxema: ' + r[0].error); n++; }
  return { statements: n };
}

http.createServer((req, res) => {
  const send = (code, obj) => { if (res.headersSent) return; const s = JSON.stringify(obj, (_, v) => (typeof v === 'bigint' ? v.toString() : v)); res.writeHead(code, { 'content-type': 'application/json' }); res.end(s); };
  if (req.method === 'GET') return send(200, { ok: true, service: 'tgstore-jr' });
  const chunks = []; req.on('data', (c) => chunks.push(c));
  req.on('end', async () => {
    try {
      await ready;
      const body = Buffer.concat(chunks).toString('utf8');
      const tok = (req.headers.authorization || '').replace(/^Bearer /, '');
      const fn = (req.url || '').replace(/^\//, '').split('?')[0];
      if (fn === 'setkey') { // UZ: kalit hali o'rnatilmagan bo'lsagina birinchi chaqiruv o'rnatadi. EN/RU: first call sets the key while none is set.
        const have = jr.exec([{ sql: "select 1 from kv_conf where k='key_hash'" }])[0].result.rows.length;
        if (have) return send(403, { message: 'already set' });
        const k = String(JSON.parse(body || '{}').key || ''); if (k.length < 16) return send(400, { message: 'bad key' });
        jr.exec([{ sql: "insert into kv_conf(k,v) values('key_hash',?)", args: [{ type: 'text', value: h256(k) }] }]); AUTHS.add(h256(k)); return send(200, { ok: true });
      }
      if (!AUTHS.has(h256(tok))) return send(401, { message: 'unauthorized' });
      const b = JSON.parse(body || '{}');
      if (fn === 'v2/pipeline') {
        const reqs = b.requests || [];
        const r = jr.exec(reqs.filter((q) => q.type === 'execute').map((q) => q.stmt));
        let i = 0;
        const results = reqs.map((q) => {
          if (q.type !== 'execute') return { type: 'ok', response: { type: q.type } };
          const x = r[i++];
          if (!x) return { type: 'error', error: { message: 'skipped after error' } };
          return x.ok ? { type: 'ok', response: { type: 'execute', result: x.result } } : { type: 'error', error: { message: x.error } };
        });
        return send(200, { baton: null, base_url: null, results });
      }
      if (fn === 'sql') return send(200, { results: jr.exec(b.stmts || [], { journal: b.journal !== false }) });
      if (fn === 'usage') return send(200, jr.usage());
      if (fn === 'marks') return send(200, jr.marks(b));
      if (fn === 'status') return send(200, { ...jr.usage(), snapshot: jr.snap ? { seq: jr.snap.seq, at: jr.snap.at, parts: jr.snap.parts.length } : null, dirty: [...jr.dirty], telegram: !!(tg && cfg.key) });
      if (fn === 'schema') return send(200, applySchema());
      if (fn === 'replicate') return send(200, await jr.replicate());
      if (fn === 'snapshot') return send(200, await jr.snapshot());
      return send(404, { message: 'no such endpoint' });
    } catch (e) { send(500, { message: String((e && e.message) || e) }); }
  });
}).listen(Number(E.PORT || 8090), '0.0.0.0', () => log('tgstore-jr listening', E.PORT || 8090));
