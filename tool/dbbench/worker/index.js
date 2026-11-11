// UZ: Test worker: Ample'dagi Postgres va libSQL'ga qaysi biri tezroq / barqarorroq javob berishini o'lchaydi.
// EN: Test worker: measures which of Postgres / libSQL (on Ample) is faster / more stable.
// RU: Тестовый воркер: измеряет, кто быстрее и стабильнее — Postgres или libSQL (на Ample).
//
// GET /run?engine=ping|pg|lite&op=read|write|hot|heavy&n=45&seed=1
//   -> { ms: [...], ok: "1101..." }   (ok[i] = '1' if request i succeeded)
// Free plan: 50 subrequests per invocation, 6 simultaneous connections => n <= 45, concurrency 6.
export default {
  async fetch(req, env) {
    const u = new URL(req.url);
    if (u.pathname !== '/run') return new Response('dbbench ok\n');
    const engine = u.searchParams.get('engine') || 'ping';
    const op = u.searchParams.get('op') || 'read';
    const n = Math.min(45, Math.max(1, Number(u.searchParams.get('n')) || 45));
    const seed = Number(u.searchParams.get('seed')) || 1;
    if (!['ping', 'pg', 'lite'].includes(engine) || !['read', 'write', 'hot', 'heavy'].includes(op)) {
      return new Response('bad engine/op', { status: 400 });
    }
    const path = engine === 'ping' ? '/ping' : `/${engine}/${op}`;
    const ms = new Array(n);
    const ok = new Array(n).fill('0');
    let next = 0;
    const lane = async () => {
      while (next < n) {
        const i = next++;
        const id = ((seed * 7919 + i * 104729) % 500000) + 1;
        const t0 = Date.now();
        try {
          const r = await fetch(`${env.AMPLE_URL}${path}?id=${id}`, { headers: { 'cache-control': 'no-cache' } });
          await r.text();
          if (r.ok) ok[i] = '1';
        } catch (_) { /* counted as failure */ }
        ms[i] = Date.now() - t0;
      }
    };
    await Promise.all(Array.from({ length: 6 }, lane));
    return Response.json({ engine, op, n, ms, ok: ok.join(''), colo: req.cf && req.cf.colo });
  },
};
