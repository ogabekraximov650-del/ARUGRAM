// UZ: `login_demo.html` ni 60 fps video (mp4) yoki tekshiruv rasmlariga aylantiradi (virtual soat bilan, har kadr aniq).
// EN: Renders `login_demo.html` into a 60 fps video (mp4) or check screenshots (virtual clock, frame-exact).
// RU: Превращает `login_demo.html` в видео 60 fps (mp4) или в проверочные снимки (виртуальные часы, покадрово точно).
//
// Ishlatish / Usage:  node render.mjs video out.mp4
//                     node render.mjs shots outDir 1500 6000 12000
import { chromium } from '/opt/node-tools/node_modules/playwright/index.mjs';
import { spawn } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const here = path.dirname(fileURLToPath(import.meta.url));
const [mode, out, ...rest] = process.argv.slice(2);
const FPS = 60, DT = 1000 / FPS;

const browser = await chromium.launch({ executablePath: process.env.CHROME || undefined, args: ['--font-render-hinting=none', '--disable-lcd-text'] });
const page = await browser.newPage({ viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1 });
page.on('pageerror', (e) => console.error('PAGEERROR', e.message));
page.on('console', (m) => { if (m.type() === 'error') console.error('CONSOLE', m.text()); });
await page.goto('file://' + path.join(here, 'login_demo.html') + '?manual=1');
await page.evaluate(() => document.fonts.ready);
await page.evaluate(() => window.start());

if (mode === 'shots') {
  mkdirSync(out, { recursive: true });
  const times = rest.map(Number).sort((a, b) => a - b);
  let t = 0;
  for (const target of times) {
    while (t < target) { await page.evaluate((dt) => window.step(dt), DT); t += DT; }
    await page.screenshot({ path: path.join(out, `t${String(target).padStart(6, '0')}.png`) });
    console.log('shot', target);
  }
} else {
  const ff = spawn('ffmpeg', ['-y', '-loglevel', 'error', '-f', 'image2pipe', '-framerate', String(FPS), '-i', '-',
    '-c:v', 'libx264', '-preset', 'medium', '-crf', '17', '-pix_fmt', 'yuv420p', '-movflags', '+faststart', out], { stdio: ['pipe', 'inherit', 'inherit'] });
  let n = 0;
  while (!(await page.evaluate(() => window.__finished === true))) {
    await page.evaluate((dt) => window.step(dt), DT);
    const buf = await page.screenshot({ type: 'png' });
    if (!ff.stdin.write(buf)) await new Promise((r) => ff.stdin.once('drain', r));
    n++;
    if (n % 300 === 0) console.log('kadr', n, (n / FPS).toFixed(1) + ' s');
    if (n > FPS * 120) { console.error('juda uzoq'); break; }
  }
  ff.stdin.end();
  await new Promise((r) => ff.on('close', r));
  writeFileSync(out + '.txt', `kadrlar: ${n}, davomiyligi: ${(n / FPS).toFixed(1)} s\n`);
  console.log('tayyor', n, 'kadr');
}
await browser.close();
