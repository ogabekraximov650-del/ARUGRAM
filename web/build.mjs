// Mini App yig'ish: `src/js/app.js` (va undan import qilinganlar, mtcute
// ham) bitta `dist/app.js` ga; `src/css/*.css` bitta `dist/app.css` ga;
// `index.html` va `assets/` o'zicha ko'chiriladi. Deploy — `dist/`.
import { build } from 'esbuild';
import fs from 'node:fs';
import path from 'node:path';

const root = path.dirname(new URL(import.meta.url).pathname);
const src = path.join(root, 'src');
const dist = path.join(root, 'dist');
fs.rmSync(dist, { recursive: true, force: true });
fs.mkdirSync(dist, { recursive: true });

await build({
  entryPoints: [path.join(src, 'js', 'app.js')],
  bundle: true,
  format: 'esm',
  splitting: true,
  outdir: dist,
  entryNames: 'app',
  chunkNames: 'chunks/[name]-[hash]',
  assetNames: 'assets/[name]-[hash]',
  minify: true,
  sourcemap: false,
  target: ['es2020', 'safari15'],
  loader: { '.wasm': 'file' },
  logLevel: 'info',
});

const css = fs.readdirSync(path.join(src, 'css'))
  .filter((f) => f.endsWith('.css'))
  .sort((a, b) => (a === 'app.css' ? -1 : b === 'app.css' ? 1 : a.localeCompare(b)))
  .map((f) => `/* ${f} */\n` + fs.readFileSync(path.join(src, 'css', f), 'utf8'))
  .join('\n');
fs.writeFileSync(path.join(dist, 'app.css'), css);

const v = Date.now().toString(36);
const html = fs.readFileSync(path.join(src, 'index.html'), 'utf8')
  .replace('css/app.css', `app.css?v=${v}`)
  .replace('js/app.js', `app.js?v=${v}`);
fs.writeFileSync(path.join(dist, 'index.html'), html);
fs.cpSync(path.join(src, 'assets'), path.join(dist, 'assets'), { recursive: true });
fs.copyFileSync(path.join(root, '../fonts/TgEmoji.ttf'), path.join(dist, 'assets/TgEmoji.ttf'));
// mtcute shifrlash yadrosi (WASM) — `tg/client.js` shu manzildan yuklaydi.
fs.copyFileSync(path.join(root, 'node_modules/@mtcute/wasm/mtcute.wasm'), path.join(dist, 'assets/mtcute.wasm'));
console.log('dist tayyor');
