// UZ: Test worker: Neon (Hyperdrive va HTTP drayver), D1 va Turso javob tezligini o'lchaydi.
// EN: Test worker: measures latency of Neon (Hyperdrive and HTTP driver), D1 and Turso.
// RU: Тестовый воркер: измеряет задержку Neon (Hyperdrive и HTTP-драйвер), D1 и Turso.
//
// GET /run?engine=neon_hd|neon_http|d1|turso&op=read|write|heavy&n=40&seed=1
import { Client } from 'pg';
import { neon } from '@neondatabase/serverless';

const ROWS = 200000;
const idOf = (seed, i) => ((seed * 7919 + i * 104729) % ROWS) + 1;

async function turso(env, sql, args) {
  const url = env.TURSO_URL.replace(/^libsql:\/\//, 'https://');
  const r = await fetch(`${url}/v2/pipeline`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${env.TURSO_TOKEN}`, 'content-type': 'application/json' },
    body: JSON.stringify({ requests: [{ type: 'execute', stmt: { sql, args: args.map((v) => ({ type: 'integer', value: String(v) })) } }, { type: 'close' }] }),
  });
  const j = await r.json();
  if (j.results && j.results[0] && j.results[0].type === 'error') throw new Error(j.results[0].error.message);
  return j;
}

export default {
  async fetch(req, env, ctx) {
    const u = new URL(req.url);
    if (u.pathname !== '/run') return new Response('dbbench2 ok\n');
    const engine = u.searchParams.get('engine') || 'd1';
    const op = u.searchParams.get('op') || 'read';
    const n = Math.min(40, Math.max(1, Number(u.searchParams.get('n')) || 40));
    const seed = Number(u.searchParams.get('seed')) || 1;
    if (!['neon_hd', 'neon_http', 'd1', 'turso'].includes(engine) || !['read', 'write', 'heavy'].includes(op)) {
      return new Response('bad engine/op', { status: 400 });
    }
    if (engine === 'turso' && op === 'write') return Response.json({ engine, op, skipped: 'production DB: write not tested' });

    const ms = [];
    let errors = 0, lastError = '', connectMs = 0;
    let client = null, sql = null;
    try {
      if (engine === 'neon_hd') {
        if (!env.HD) return Response.json({ engine, op, skipped: 'Hyperdrive not configured' });
        const t0 = Date.now();
        client = new Client({ connectionString: env.HD.connectionString });
        await client.connect();
        connectMs = Date.now() - t0;
      } else if (engine === 'neon_http') {
        sql = neon(env.NEON_URL);
      }
    } catch (e) {
      return Response.json({ engine, op, n, errors: n, ms: [], connect_ms: connectMs, error: String(e && e.message || e) });
    }

    for (let i = 0; i < n; i++) {
      const id = idOf(seed, i);
      const t0 = Date.now();
      try {
        if (engine === 'neon_hd') {
          if (op === 'read') await client.query('select v from t where id = $1', [id]);
          else if (op === 'write') await client.query('update t set v = v + 1 where id = $1', [id]);
          else await client.query('select count(*), sum(v) from t where id between $1 and $1 + 10000', [id]);
        } else if (engine === 'neon_http') {
          if (op === 'read') await sql.query('select v from t where id = $1', [id]);
          else if (op === 'write') await sql.query('update t set v = v + 1 where id = $1', [id]);
          else await sql.query('select count(*), sum(v) from t where id between $1 and $1 + 10000', [id]);
        } else if (engine === 'd1') {
          if (op === 'read') await env.DB.prepare('select v from t where id = ?').bind(id).first();
          else if (op === 'write') await env.DB.prepare('update t set v = v + 1 where id = ?').bind(id).run();
          else await env.DB.prepare('select count(*) c, sum(v) s from t where id between ? and ? + 10000').bind(id, id).first();
        } else {
          if (op === 'read') await turso(env, 'select id, name from anime_db where id = ?', [(i % 11) + 1]);
          else await turso(env, 'select count(*) from season_db', []);
        }
      } catch (e) {
        errors++;
        lastError = String(e && e.message || e).slice(0, 160);
      }
      ms.push(Date.now() - t0);
    }
    if (client) ctx.waitUntil(client.end().catch(() => {}));
    return Response.json({ engine, op, n, ms, errors, connect_ms: connectMs, last_error: lastError, colo: req.cf && req.cf.colo });
  },
};
