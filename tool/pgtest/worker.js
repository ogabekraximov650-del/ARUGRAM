// Sinov: Cloudflare Worker -> Ample ko'prigi (HTTPS) -> Postgres. Manzil va token worker sirlaridan (AMPLE_URL, AMPLE_TOKEN) olinadi.
async function rpc(env, body) {
  const t0 = Date.now();
  const r = await fetch(env.AMPLE_URL + '/rpc', {
    method: 'POST',
    headers: { Authorization: 'Bearer ' + env.AMPLE_TOKEN, 'content-type': 'application/json' },
    body: JSON.stringify(body),
  });
  const j = await r.json().catch(() => ({}));
  return { status: r.status, ms: Date.now() - t0, db_ms: j.ms, data: j.data, error: j.error };
}

export default {
  async fetch(req, env) {
    const out = { colo: req.cf && req.cf.colo, steps: [] };
    out.steps.push({ op: 'init', ...(await rpc(env, { op: 'init' })) });
    for (let i = 0; i < 5; i++) {
      const r = await rpc(env, { op: 'ping' });
      out.steps.push({ op: 'ping#' + (i + 1), status: r.status, ms: r.ms, db_ms: r.db_ms });
    }
    const key = 'w' + Date.now();
    out.steps.push({ op: 'put', ...(await rpc(env, { op: 'put', k: key, v: 'worker->ample->postgres' })) });
    out.steps.push({ op: 'get', ...(await rpc(env, { op: 'get', k: key })) });
    // 10 ketma-ket yozuv (ko'prik orqali) — o'rtacha vaqt.
    const t0 = Date.now();
    for (let i = 0; i < 10; i++) await rpc(env, { op: 'put', k: key + '_' + i, v: 'x' });
    out.put10_avg_ms = (Date.now() - t0) / 10;
    // 10 parallel o'qish.
    const t1 = Date.now();
    await Promise.all(Array.from({ length: 10 }, () => rpc(env, { op: 'list', limit: 5 })));
    out.list10_parallel_total_ms = Date.now() - t1;
    out.steps.push({ op: 'cleanup', ...(await rpc(env, { op: 'cleanup' })) });
    return new Response(JSON.stringify(out, null, 1), { headers: { 'content-type': 'application/json' } });
  },
};
