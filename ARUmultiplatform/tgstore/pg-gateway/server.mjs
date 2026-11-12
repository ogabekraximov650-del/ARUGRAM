// UZ: tgstore PG-DARVOZA (Ample): Worker'dan Turso formatidagi so'rovlarni qabul qilib, Postgres'da bajaradi va Turso formatida javob beradi.
//     SQL SQLite lahjasidan tarjima qilinadi (`translate.mjs`). Shuningdek: baza hajmi (`/usage`) va kesh belgilari (`/marks`).
// EN: tgstore PG GATEWAY (Ample): accepts Turso-shaped requests from the Worker, runs them on Postgres and answers in Turso's shape.
// RU: PG-shlyuz tgstore (Ample).
import http from 'node:http';
import crypto from 'node:crypto';
import pg from 'pg';
import { translate, finalSql } from './translate.mjs';
import { Archive, makeTelegram, THRESHOLD } from './archive.mjs';

const E = process.env;
const STORE_KEY = E.STORE_KEY || '';
const h256 = (x) => crypto.createHash('sha256').update(x).digest('hex');
let AUTHS = new Set([STORE_KEY && h256(STORE_KEY), E.ADMIN_KEY && h256(E.ADMIN_KEY)].filter(Boolean));
const LIMIT_MB = Number(E.DB_LIMIT_MB || 1024);
const log = (...a) => console.log(new Date().toISOString().slice(11, 23), ...a);

// int8 / numeric ni qator sifatida olamiz (aniqlik uchun), keyin o'zimiz aylantiramiz
pg.types.setTypeParser(20, (v) => v);
pg.types.setTypeParser(1700, (v) => v);
const pool = new pg.Pool({ connectionString: E.DATABASE_URL, max: Number(E.PG_POOL || 40), idleTimeoutMillis: 30000, ssl: E.PGSSL === '1' ? { rejectUnauthorized: false } : undefined });
pool.on('error', (e) => log('pool error', e.message));

// ── sxema keshi: jadval -> ustunlar va birlamchi kalit ───────────────────────────────────────────────────────────────────────────────────────
const schemaCache = new Map();
async function loadTable(client, table) {
  const cols = (await client.query('select column_name from information_schema.columns where table_schema=current_schema() and table_name=$1 order by ordinal_position', [table])).rows.map((r) => r.column_name);
  const pk = (await client.query(`select a.attname from pg_index i join pg_attribute a on a.attrelid=i.indrelid and a.attnum=any(i.indkey)
    where i.indrelid=to_regclass($1) and i.indisprimary order by array_position(i.indkey, a.attnum)`, [table])).rows.map((r) => r.attname);
  // UZ: lastval() faqat identity ustuni bor jadvalda kerak (aks holda har INSERT'ga ortiqcha so'rov). EN/RU: lastval only for tables with an identity column.
  const ident = (await client.query("select 1 from information_schema.columns where table_schema=current_schema() and table_name=$1 and is_identity='YES' limit 1", [table])).rows.length > 0;
  schemaCache.set(table, { cols, pk, ident });
}
const schemaView = { cols: (t) => schemaCache.get(t)?.cols || [], pk: (t) => schemaCache.get(t)?.pk || [] };
async function ensureTables(client, sql) {
  for (const m of sql.matchAll(/\b(?:INSERT\s+(?:OR\s+\w+\s+)?INTO|REPLACE\s+INTO)\s+([A-Za-z_]\w*)/gi)) {
    const t = m[1].toLowerCase(); if (!schemaCache.has(t)) await loadTable(client, t);
  }
}

// ── natijani Turso shakliga o'girish ────────────────────────────────────────────────────────────────────────────────────────────────────────
function cell(v, oid) {
  if (v === null || v === undefined) return { type: 'null' };
  if (oid === 20 || oid === 21 || oid === 23) return { type: 'integer', value: String(v) };
  if (oid === 700 || oid === 701) return { type: 'float', value: Number(v) };
  if (oid === 1700) { const s = String(v); return /^-?\d+$/.test(s) ? { type: 'integer', value: s } : { type: 'float', value: Number(s) }; }
  if (oid === 16) return { type: 'integer', value: v ? '1' : '0' };
  if (Buffer.isBuffer(v)) return { type: 'blob', base64: v.toString('base64') };
  if (v instanceof Date) return { type: 'text', value: v.toISOString() };
  if (typeof v === 'object') return { type: 'text', value: JSON.stringify(v) };
  return { type: 'text', value: String(v) };
}
const trCache = new Map();
async function runOne(client, st) {
  await ensureTables(client, st.sql);
  let tr = trCache.get(st.sql);
  if (!tr) { const r = translate(st.sql, schemaView); tr = { sql: finalSql(r), kind: r.kind, table: r.table }; if (tr.kind !== 'CREATE' && tr.kind !== 'ALTER') trCache.set(st.sql, tr); }
  const values = (st.args || []).map((a) => (a === null || a.type === 'null' || a.value === undefined ? null : a.type === 'blob' ? Buffer.from(a.base64 || a.value, 'base64') : String(a.value)));
  // UZ: `dry` — faqat tahlil (EXPLAIN): so'rov bajarilmaydi, sintaksis/ustun xatolari aniqlanadi (production SQL'ni tekshirish uchun). EN/RU: dry = EXPLAIN only.
  const text = st.dry && tr.kind !== 'CREATE' && tr.kind !== 'ALTER' ? `EXPLAIN ${tr.sql}` : tr.sql;
  const r = await client.query({ text, values, rowMode: 'array' });
  if (tr.kind === 'CREATE' || tr.kind === 'ALTER') schemaCache.clear();
  const fields = r.fields || [];
  const out = {
    cols: fields.map((f) => ({ name: f.name })),
    rows: (r.rows || []).map((row) => row.map((v, i) => cell(v, fields[i]?.dataTypeID))),
    affected_row_count: r.rowCount || 0,
    rows_read: (r.rows || []).length,
    rows_written: ['INSERT', 'UPDATE', 'DELETE'].includes(tr.kind) ? (r.rowCount || 0) : 0,
  };
  if (tr.kind === 'INSERT' && !(r.rows || []).length && schemaCache.get(tr.table)?.ident) {
    try { const l = await client.query('select lastval() as v'); out.last_insert_rowid = String(l.rows[0].v); } catch { /* ketma-ketlik ishlatilmagan */ }
  }
  return out;
}

async function sql(body) {
  const client = await pool.connect();
  const results = [];
  try {
    for (const st of body.stmts || []) {
      try { results.push({ ok: true, result: await runOne(client, st) }); }
      catch (e) { log('SQL XATO:', String(e.message).slice(0, 200), '|', st.sql.replace(/\s+/g, ' ').slice(0, 160)); results.push({ ok: false, error: String(e.message || e) }); break; }
    }
  } finally { client.release(); }
  return { results };
}

async function init() {
  await pool.query('create table if not exists kv_marks(k text primary key, v bigint not null default 0)');
  await pool.query('create table if not exists kv_conf(k text primary key, v text not null)');
  const r = await pool.query("select v from kv_conf where k='key_hash'");
  if (r.rows[0]) AUTHS.add(r.rows[0].v);
}
async function marks(b) {
  const at = Number(b.at || 0);
  for (const k of b.bump || []) await pool.query('insert into kv_marks(k,v) values($1,$2) on conflict(k) do update set v=greatest(excluded.v, kv_marks.v+1)', [k, Math.max(at, 1)]);
  const v = {}; if ((b.get || []).length) for (const r of (await pool.query('select k,v from kv_marks where k=any($1)', [b.get])).rows) v[r.k] = Number(r.v);
  return { v };
}
async function usage() {
  const size = Number((await pool.query('select pg_database_size(current_database()) as b')).rows[0].b);
  const top = (await pool.query(`select relname as t, pg_total_relation_size(c.oid) as b from pg_class c join pg_namespace n on n.oid=c.relnamespace
    where c.relkind='r' and n.nspname=current_schema() order by 2 desc limit 8`)).rows.map((r) => ({ table: r.t, bytes: Number(r.b) }));
  const limit = LIMIT_MB * 1024 * 1024;
  return { used_bytes: size, limit_bytes: limit, percent: limit ? (size / limit) * 100 : 0, top };
}

// UZ: doira tizimi shu serverda: TG_BOT_TOKEN + TG_CHANNEL_ID berilganda (yoki TG_MOCK=1 — sinov) yoqiladi, har 5 daqiqada bazani tekshiradi.
// EN/RU: the circle system runs here when TG_BOT_TOKEN + TG_CHANNEL_ID are set (or TG_MOCK=1 for tests).
const arch = (E.TG_BOT_TOKEN && E.TG_CHANNEL_ID) || E.TG_MOCK === '1'
  ? new Archive(pool, makeTelegram({ token: E.TG_BOT_TOKEN, channel: E.TG_CHANNEL_ID, mock: E.TG_MOCK === '1' }), { usage: async () => (await usage()).percent, cacheDir: E.ARCH_CACHE_DIR })
  : null;
if (arch && E.ARCHIVE_ON !== '0') {
  setInterval(() => { arch.run().then((r) => { if (r.active) log('doira:', JSON.stringify(r).slice(0, 300)); }).catch((e) => log('doira xato:', e.message)); }, 5 * 60_000).unref();
}

http.createServer((req, res) => {
  const send = (code, obj) => { res.writeHead(code, { 'content-type': 'application/json' }); res.end(JSON.stringify(obj)); };
  if (req.method === 'GET') return send(200, { ok: true, service: 'tgstore-pg-gateway' });
  let body = ''; req.on('data', (c) => { body += c; });
  req.on('end', async () => {
    try {
      const tok = (req.headers.authorization || '').replace(/^Bearer /, '');
      const fn = (req.url || '').replace(/^\//, '').split('?')[0];
      if (fn === 'setkey') { // UZ: kalit hali o'rnatilmagan bo'lsagina, birinchi chaqiruv o'rnatadi. EN: first call sets the key, only while none is set. RU: pervyj vyzov zadaet klyuch.
        const r = await pool.query("select 1 from kv_conf where k='key_hash'");
        if (r.rows[0]) return send(403, { message: 'already set' });
        const k = String(JSON.parse(body || '{}').key || ''); if (k.length < 16) return send(400, { message: 'bad key' });
        await pool.query("insert into kv_conf(k,v) values('key_hash',$1)", [h256(k)]); AUTHS.add(h256(k)); return send(200, { ok: true });
      }
      if (!AUTHS.has(h256(tok))) return send(401, { message: 'unauthorized' });
      const b = JSON.parse(body || '{}');
      if (fn === 'v2/pipeline') { // UZ: Turso "pipeline" formati — Worker'dagi mavjud kod o'zgarishsiz ishlaydi. EN: Turso pipeline shape so the Worker's existing code works as is. RU: format Turso pipeline.
        const reqs = b.requests || [];
        const r = await sql({ stmts: reqs.filter((q) => q.type === 'execute').map((q) => q.stmt) });
        let i = 0;
        const results = reqs.map((q) => {
          if (q.type !== 'execute') return { type: 'ok', response: { type: q.type } };
          const x = r.results[i++];
          if (!x) return { type: 'error', error: { message: 'skipped after error' } };
          return x.ok ? { type: 'ok', response: { type: 'execute', result: x.result } } : { type: 'error', error: { message: x.error } };
        });
        return send(200, { baton: null, base_url: null, results });
      }
      if (fn.startsWith('archive/')) {
        if (!arch) return send(503, { message: 'doira yoqilmagan (TG_BOT_TOKEN/TG_CHANNEL_ID yo\'q)' });
        const a = fn.slice(8);
        if (a === 'run') return send(200, await arch.run({ force: !!b.force, dry: !!b.dry, max: b.max || 5 }));
        if (a === 'files') return send(200, { files: await arch.files(), threshold: THRESHOLD });
        if (a === 'get') return send(200, { row: await arch.lookup(String(b.table), String(b.key)) });
        if (a === 'warm') return send(200, await arch.warm(String(b.table), String(b.key), { warmDays: b.days || 7 }));
        if (a === 'purge') return send(200, await arch.purge(Number(b.id)));
        if (a === 'restore') return send(200, await arch.restore(Number(b.id), { warmDays: b.days || 7 }));
        return send(404, { message: 'no such endpoint' });
      }
      if (fn === 'sql') return send(200, await sql(b));
      if (fn === 'usage') return send(200, await usage());
      if (fn === 'marks') return send(200, await marks(b));
      return send(404, { message: 'no such endpoint' });
    } catch (e) { send(500, { message: String(e.message || e) }); }
  });
}).listen(process.env.PORT || 8080, '0.0.0.0', async () => { await init().catch((e) => log('init xato', e.message)); log('pg-gateway listening'); });
