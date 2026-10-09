// Mahalliy ko'rish (deploy qilinmaydi): `node build.mjs` dan keyin
//   node tools/preview.mjs <mocks.mjs> <out-dir>
// `mocks.mjs` — `export default { '/api/seasons': [...], 'POST /api/x': {...},
//   steps: async (page, shot) => { ...; await shot('nom'); } }`.
// Telegram va worker soxta (mock) qilinadi; brauzer — Playwright Chromium,
// telefon o'lchami 393x851 (Android), DPR 2.
import { chromium } from '/opt/node-tools/node_modules/playwright/index.mjs';
import path from 'node:path';
import fs from 'node:fs';

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname), '..');
const DIST = path.join(root, 'dist');
const mocksPath = path.resolve(process.argv[2]);
const out = path.resolve(process.argv[3] || '.');
fs.mkdirSync(out, { recursive: true });
const mocks = (await import(mocksPath)).default;

const API = 'https://arugram.uzcom.workers.dev';
const user = mocks.tgUser || { id: 777, first_name: 'Test', username: 'test' };
const b = await chromium.launch();
const ctx = await b.newContext({ viewport: { width: 393, height: 851 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true });
const page = await ctx.newPage();
page.on('console', (m) => console.log('console:', m.text()));
page.on('pageerror', (e) => console.log('pageerror:', e.message));
await page.route('https://telegram.org/**', (r) => r.fulfill({ contentType: 'text/javascript', body:
  `window.Telegram={WebApp:{initData:'user=x',initDataUnsafe:{user:${JSON.stringify(user)}},ready(){},expand(){},
   BackButton:{show(){},hide(){},onClick(){}},HapticFeedback:{impactOccurred(){}},openLink(u){console.log('openLink',u)},
   openTelegramLink(u){console.log('openTelegramLink',u)},showConfirm(t,c){c(true)},showAlert(t,c){c&&c()}}}` }));
await page.route(`${API}/**`, async (r) => {
  const u = new URL(r.request().url());
  const m = r.request().method();
  if (u.pathname === '/api/tma/auth') return r.fulfill({ json: { token: '1.9999999999.ab', expires_in: 43200, session: 'sess', user } });
  if (u.pathname.startsWith('/api/image/')) return r.fulfill({ path: path.join(root, '../branding/aru-logo-512.png') });
  const key = `${m} ${u.pathname}`;
  let v = mocks[key] ?? (m === 'GET' ? mocks[u.pathname] : undefined);
  if (typeof v === 'function') v = await v(u, r.request());
  if (v === undefined) { console.log('MOCK YOQ:', key); return r.fulfill({ status: 404, json: { error: 'not_found' } }); }
  return r.fulfill({ json: v });
});
await page.route('http://site/**', (r) => {
  const p = new URL(r.request().url()).pathname;
  const f = path.join(DIST, p === '/' ? 'index.html' : p);
  if (!fs.existsSync(f)) return r.fulfill({ status: 404, body: '' });
  r.fulfill({ path: f });
});
await page.goto('http://site/');
await page.waitForTimeout(1500);
const shot = async (name) => { await page.screenshot({ path: path.join(out, `${name}.png`) }); console.log('shot', name); };
if (mocks.steps) await mocks.steps(page, shot); else await shot('home');
await b.close();
