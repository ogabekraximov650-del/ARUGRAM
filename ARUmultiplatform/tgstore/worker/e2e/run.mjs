// UZ: To'liq sinov (haqiqiy worker + Telegram). Ishlatish: URL=... KEY=... node e2e/run.mjs
// EN: Full end-to-end test (real worker + Telegram). Usage: URL=... KEY=... node e2e/run.mjs
// RU: Polnyy end-to-end test (realnyy worker + Telegram). Zapusk: URL=... KEY=... node e2e/run.mjs
const URL_ = process.env.URL, KEY = process.env.KEY;
if (!URL_ || !KEY) { console.error('URL/KEY kerak'); process.exit(2); }
const H = { authorization: 'Bearer ' + KEY, 'user-agent': 'tgstore-e2e/1' };
const table = 'e2e' + Date.now().toString(36);
const rows = [];
let failed = 0;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function req(method, path, body) {
  const t = Date.now();
  const r = await fetch(URL_ + path, { method, headers: { ...H, ...(body ? { 'content-type': 'application/json' } : {}) }, body: body ? JSON.stringify(body) : undefined });
  const text = await r.text();
  let j; try { j = JSON.parse(text); } catch { j = null; }
  return { status: r.status, j, ms: Date.now() - t, src: r.headers.get('x-src'), len: text.length };
}
async function step(name, fn) {
  const t = Date.now();
  try {
    const note = await fn();
    rows.push(`| ✅ | ${name} | ${Date.now() - t} ms | ${note || ''} |`);
  } catch (e) {
    failed++;
    rows.push(`| ❌ | ${name} | ${Date.now() - t} ms | ${String(e.message || e).slice(0, 200)} |`);
  }
}
const ok = (c, m) => { if (!c) throw new Error(m); };

const big = (i) => ({ i, name: 'Anime ' + i, text: 'x'.repeat(9000) + i }); // ~9 KB: 150 ta ≈ 1.4 MB -> 2 bo'lak / 2 chunks / 2 chanka

await step('health', async () => { const r = await fetch(URL_ + '/health'); ok(r.ok, 'health ' + r.status); });
await step('auth: kalitsiz 401', async () => { const r = await fetch(URL_ + '/stats/' + table); ok(r.status === 401, 'status ' + r.status); });
await step('bot: getMe', async () => { const r = await req('GET', '/me'); ok(r.j?.is_bot, JSON.stringify(r.j)); return '@' + r.j.username; });

await step('yozish: 150 yozuv (30 ta parallel so\'rov = guruh-commit)', async () => {
  const all = [...Array(150).keys()];
  const groups = []; for (let g = 0; g < 30; g++) groups.push(all.slice(g * 5, g * 5 + 5));
  const res = await Promise.all(groups.map((ids) => req('POST', '/w/' + table, { ops: ids.map((i) => ({ op: 'put', k: 'k' + i, v: big(i) })) })));
  for (const r of res) ok(r.status === 200 && r.j.ok, 'write ' + r.status + JSON.stringify(r.j));
  return 'commit guruhlari: ' + [...new Set(res.map((r) => r.j.batched))].join(',');
});
await step('statistika: 2+ bo\'lak, 150 kalit, muhrlangan', async () => {
  const r = await req('GET', '/stats/' + table);
  ok(r.j.keys === 150, 'keys ' + r.j.keys); ok(r.j.chunks >= 2, 'chunks ' + r.j.chunks); ok(r.j.sealed >= 1, 'sealed ' + r.j.sealed);
  return JSON.stringify(r.j);
});
await step('o\'qish: har bir yozuv to\'g\'ri', async () => {
  for (let i = 0; i < 150; i += 7) { const r = await req('GET', `/r/${table}/k${i}`); ok(r.status === 200 && r.j.v.i === i, 'k' + i + ' ' + r.status); }
});
await step('kesh: ikkinchi o\'qish Telegram\'ga bormaydi', async () => {
  const a = await req('GET', `/r/${table}/k1`); const b = await req('GET', `/r/${table}/k1`);
  ok(b.src === 'isolate' || b.src === 'cache', 'src=' + b.src);
  return `1-o'qish: ${a.src} ${a.ms}ms; 2-o'qish: ${b.src} ${b.ms}ms`;
});
await step('bo\'lakni to\'g\'ridan-to\'g\'ri olish (ilova indeksi) + kesh', async () => {
  const k = await req('GET', `/k/${table}?limit=1`); const { no, ver } = k.j.keys[0];
  const t = Date.now(); const r = await fetch(`${URL_}/c/${table}/${no}/${ver}`, { headers: H }); const n = (await r.arrayBuffer()).byteLength;
  ok(r.ok && n > 1000, 'chunk ' + r.status);
  return `${no}/${ver}: ${n} bayt, x-src=${r.headers.get('x-src')}, ${Date.now() - t}ms`;
});
await step('tahrir: kalit yangilanadi, eski versiya o\'qilmaydi', async () => {
  const w = await req('POST', '/w/' + table, { ops: [{ op: 'put', k: 'k3', v: { i: 3, edited: true } }] });
  ok(w.j.results.k3.ver >= 2, 'ver ' + JSON.stringify(w.j));
  const r = await req('GET', `/r/${table}/k3`); ok(r.j.v.edited === true, 'edit not visible');
});
await step('tahrir: kattalashgan yozuv ko\'chadi, boshqa kalitlar saqlanadi', async () => {
  await req('POST', '/w/' + table, { ops: [{ op: 'put', k: 'k0', v: { i: 0, text: 'y'.repeat(120000) } }] });
  const r = await req('GET', `/r/${table}/k0`); ok(r.j.v.text.length === 120000, 'grown');
  const o = await req('GET', `/r/${table}/k1`); ok(o.j.v.i === 1, 'neighbor lost');
});
await step('o\'chirish', async () => {
  const d = await req('POST', '/w/' + table, { ops: [{ op: 'del', k: 'k5' }, { op: 'del', k: 'yoq-kalit' }] });
  ok(d.status === 200, 'del ' + d.status);
  const r = await req('GET', `/r/${table}/k5`); ok(r.status === 404, 'still there ' + r.status);
  const s = await req('GET', '/stats/' + table); ok(s.j.keys === 149, 'keys ' + s.j.keys);
});
await step('xato: juda katta yozuv rad etiladi, baza buzilmaydi', async () => {
  const w = await req('POST', '/w/' + table, { ops: [{ op: 'put', k: 'huge', v: 'z'.repeat(1100 * 1024) }] });
  ok(w.status >= 400, 'accepted ' + w.status);
  const s = await req('GET', '/stats/' + table); ok(s.j.keys === 149, 'keys ' + s.j.keys);
});
await step('GC: eski versiyalar Telegram\'dan o\'chadi', async () => {
  const before = (await req('GET', '/stats/' + table)).j.gc_pending;
  const g = await req('POST', '/gc/' + table); ok(g.status === 200, 'gc ' + g.status);
  return `kutilgan ${before} -> o'chirildi ${g.j.deleted}, qoldi ${g.j.left}`;
});
await step('zaxira: indeks zerkal kanalga yuboriladi va qadaladi', async () => {
  const b = await req('POST', '/backup/' + table);
  if (b.j?.skipped) return 'o\'tkazib yuborildi: ' + b.j.skipped;
  ok(b.status === 200 && b.j.ok, JSON.stringify(b.j)); return JSON.stringify(b.j);
});
await step('tiklash: qadalgan indeksdan holat qayta quriladi', async () => {
  const s0 = (await req('GET', '/stats/' + table)).j;
  const b = await req('POST', '/restore/' + table);
  if (b.status === 500 && /no mirror/.test(b.j?.error || '')) return 'o\'tkazib yuborildi (zerkal yo\'q)';
  ok(b.status === 200 && b.j.ok, JSON.stringify(b.j));
  const s1 = (await req('GET', '/stats/' + table)).j; ok(s1.keys === s0.keys, `keys ${s0.keys} -> ${s1.keys}`);
  const r = await req('GET', `/r/${table}/k3`); ok(r.j.v.edited === true, 'edit lost after restore');
  return `${b.j.chunks} bo'lak, ${b.j.keys} kalit`;
});
await step('tezlik: 40 ta ketma-ket o\'qish (kesh)', async () => {
  const t = Date.now(); for (let i = 0; i < 40; i++) { const r = await req('GET', `/r/${table}/k${10 + (i % 20)}`); ok(r.status === 200, 'r ' + r.status); }
  return `o'rtacha ${((Date.now() - t) / 40).toFixed(0)} ms`;
});

const md = `## tgstore E2E (jadval \`${table}\`)\n\n| | Sinov | Vaqt | Izoh |\n|---|---|---|---|\n${rows.join('\n')}\n`;
console.log(md);
if (process.env.GITHUB_STEP_SUMMARY) { const fs = await import('node:fs'); fs.appendFileSync(process.env.GITHUB_STEP_SUMMARY, md); }
process.exit(failed ? 1 : 0);
