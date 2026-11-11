// UZ: Test worker: Ample'dagi Postgres va libSQL'ga qaysi biri tezroq javob berishini o'lchaydi.
// EN: Test worker: measures which of Postgres / libSQL (on Ample) answers faster.
// RU: Тестовый воркер: измеряет, кто быстрее отвечает — Postgres или libSQL (на Ample).
//
// GET /run?engine=ping|pg|lite&op=read|write&n=40&seed=1  -> { ms: [...], errors: N }
// Free plan limit: 50 subrequests per invocation, so n <= 40 (the workflow calls it several times).
export default {
  async fetch(req, env) {
    const u = new URL(req.url);
    if (u.pathname !== '/run') return new Response('dbbench ok\n');
    const engine = u.searchParams.get('engine') || 'ping';
    const op = u.searchParams.get('op') || 'read';
    const n = Math.min(40, Math.max(1, Number(u.searchParams.get('n')) || 40));
    const seed = Number(u.searchParams.get('seed')) || 1;
    if (!['ping', 'pg', 'lite'].includes(engine) || !['read', 'write'].includes(op)) {
      return new Response('bad engine/op', { status: 400 });
    }
    const path = engine === 'ping' ? '/ping' : `/${engine}/${op}`;
    const ms = [];
    let errors = 0;
    for (let i = 0; i < n; i++) {
      const id = ((seed * 7919 + i * 104729) % 500000) + 1;
      const t0 = Date.now();
      try {
        const r = await fetch(`${env.AMPLE_URL}${path}?id=${id}`, { headers: { 'cache-control': 'no-cache' } });
        await r.text();
        if (!r.ok) errors++;
      } catch (_) {
        errors++;
      }
      ms.push(Date.now() - t0);
    }
    return Response.json({ engine, op, n, seed, errors, ms, colo: req.cf && req.cf.colo });
  },
};
