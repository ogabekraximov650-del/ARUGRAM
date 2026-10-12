// "Kesh tozalanmoqda" animatsiyasi — `lib/widgets/trash_chick.dart` ning web nusxasi (soddalashtirilgan):
// Telegram jo'jasi (`utyan`) chiqindini qutiga tashlaydi; sahna chiqindi HAJMIGA qarab 4 xil
// (500 MB gacha / 2 GB gacha / 5 GB gacha / undan katta). Jo'ja shakllari ilovadagi bilan bir xil vektor.
import { CHICK } from './chick-parts.js';

const MB = 1024 * 1024;
const tierOf = (b) => (b < 500 * MB ? 1 : b < 2048 * MB ? 2 : b < 5120 * MB ? 3 : 4);
const DUR = [0, 7, 13.5, 16.5, 22.5];
export const chickDuration = (bytes) => DUR[tierOf(bytes)] * 1000;

const lerp = (a, b, t) => a + (b - a) * t;
const clamp = (v, a = 0, b = 1) => Math.min(b, Math.max(a, v));
const ease = (t) => t * t * (3 - 2 * t);
/** keyframes [[t, v], ...] bo'yicha silliq qiymat */
function kf(frames, t) {
  if (t <= frames[0][0]) return frames[0][1];
  for (let i = 1; i < frames.length; i++) {
    if (t <= frames[i][0]) {
      const [t0, v0] = frames[i - 1]; const [t1, v1] = frames[i];
      return lerp(v0, v1, ease((t - t0) / (t1 - t0 || 1)));
    }
  }
  return frames[frames.length - 1][1];
}

function bagSize(bytes) {
  const g = bytes / MB;
  const h = g < 500 ? lerp(34, 52, clamp(g / 500)) : g < 2048 ? lerp(58, 92, clamp((g - 500) / 1548)) : g < 5120 ? lerp(158, 190, clamp((g - 2048) / 3072)) : 290;
  return h / 2; // sahna 680 birlik (Dart'da 1360)
}

const GROUND = 292; const DOOR = 96; const BIN = 596; const S = 0.27;

function pathOf(a, t, k) {
  let d = ''; let i = 0;
  while (i < a.length) {
    const c = a[i++];
    const n = c === 1 ? 6 : c === 3 ? 0 : 2;
    const v = []; for (let j = 0; j < n; j++, i++) v.push((lerp(a[i], t[i], k)).toFixed(2));
    d += c === 0 ? `M${v[0]} ${v[1]}` : c === 1 ? `C${v.join(' ')}` : c === 2 ? `L${v[0]} ${v[1]}` : 'Z';
  }
  return d;
}

function scene(bytes) {
  const tier = tierOf(bytes);
  const bh = bagSize(bytes); const bw = bh * 0.82;
  return `<svg viewBox="0 0 680 340" class="ch-svg" preserveAspectRatio="xMidYMax meet">
    <rect x="0" y="${GROUND}" width="680" height="48" fill="#2b2118" opacity=".55"/>
    <g class="ch-house"><path d="M-10 120 L96 52 L202 120 Z" fill="#7b3f2a"/><rect x="0" y="120" width="190" height="172" fill="#c98f58"/>
      <rect x="62" y="186" width="70" height="106" rx="4" fill="#5b3522"/><circle cx="118" cy="244" r="4" fill="#ffd527"/></g>
    <g stroke="#8a6a4a" stroke-width="5">${Array.from({ length: 16 }, (_, i) => `<line x1="${200 + i * 26}" y1="236" x2="${200 + i * 26}" y2="${GROUND}"/>`).join('')}
      <line x1="196" y1="250" x2="420" y2="250"/><line x1="196" y1="274" x2="420" y2="274"/></g>
    <g class="ch-bin"><path d="M${BIN - 36} 214 L${BIN + 36} 214 L${BIN + 28} ${GROUND} L${BIN - 28} ${GROUND} Z" fill="#4a8f6a"/>
      <rect x="${BIN - 42}" y="204" width="84" height="12" rx="4" fill="#3b7556"/></g>
    <g id="ch-bag" style="display:none"><rect x="${-bw / 2}" y="${-bh}" width="${bw}" height="${bh}" rx="${bw * 0.28}" fill="#b9b2a4"/>
      <path d="M${-bw * 0.18} ${-bh} L0 ${-bh - bh * 0.18} L${bw * 0.18} ${-bh}" fill="#9a9384"/></g>
    <g id="ch-chick">${CHICK.map((p, i) => `<path data-i="${i}" fill="${p.f || 'none'}" stroke="${p.s || 'none'}" stroke-width="${p.w}" stroke-linecap="${p.b ? 'butt' : 'round'}" stroke-linejoin="round"/>`).join('')}</g>
    ${tier ? '' : ''}</svg>`;
}

/** Animatsiyani [host] ichiga qo'yadi; to'xtatish funksiyasini qaytaradi. */
export function mountChick(host, bytes) {
  const tier = tierOf(bytes); const total = DUR[tier];
  host.innerHTML = scene(bytes);
  const svg = host.querySelector('svg'); const chick = svg.querySelector('#ch-chick'); const bag = svg.querySelector('#ch-bag');
  const paths = [...chick.querySelectorAll('path')];
  const bh = bagSize(bytes);
  let raf = 0; let t0 = 0; let lastTurn = -1;

  const pose = (t) => {
    // chick: x, turn(0 oldga..1 o'ngga), bob, dy, shake; bag: x, y(poyabzal pastki), rot, show
    let x = DOOR + 34; let turn = 0; let walk = 0; let dy = 0; let shake = 0; let alpha = 1;
    let b = { show: true, x: 0, y: GROUND, r: 0 };
    if (tier === 1) {
      turn = t < 1.4 ? kf([[0, 0], [1.2, 1]], t) : t < 3 ? kf([[1.4, 1], [2.6, 0]], t) : kf([[3, 0], [3.6, 1]], t);
      const k = clamp((t - 3.8) / 1.1);
      b = k <= 0 ? { show: true, x: x + 30, y: GROUND - 60, r: 0 } : { show: k < 1, x: lerp(x + 30, BIN, k), y: lerp(GROUND - 60, GROUND - 30, k) - Math.sin(k * Math.PI) * 120, r: k * 360 };
      shake = t > 5 ? Math.sin(t * 5) * 1.5 : 0;
    } else if (tier === 2) {
      x = kf([[0, DOOR + 34], [1, DOOR + 34], [9, BIN - 70], [13.5, BIN - 70]], t);
      turn = kf([[0, 0], [0.8, 1], [10.2, 1], [11.2, 0]], t);
      walk = t > 1 && t < 9 ? 1 : 0;
      const k = clamp((t - 9.2) / 1.2);
      b = k <= 0 ? { show: true, x: x + 26, y: GROUND - 56, r: 0 } : { show: k < 1, x: lerp(x + 26, BIN, k), y: lerp(GROUND - 56, GROUND - 40, k) - Math.sin(k * Math.PI) * 90, r: k * 300 };
    } else if (tier === 3) {
      x = kf([[0, DOOR + 40], [1, DOOR + 40], [13, BIN - 120], [16.5, BIN - 120]], t);
      turn = kf([[0, 0], [1, 0.9]], t) - (t > 13.2 ? kf([[13.2, 0], [14, 0.9]], t) : 0);
      walk = t > 1 && t < 13 ? 1 : 0;
      b = { show: true, x: x + 50, y: GROUND, r: walk ? Math.sin(t * 6) * 4 : 0 };
      shake = t > 13.5 ? Math.sin(t * 7) * 2 : 0;
    } else {
      // 1) eshikda tortadi, 2) qop otilib chiqadi, jo'ja yiqiladi va yig'laydi, 3) sudrab boradi, 4) uyiga kiradi
      const pop = 4.6;
      x = t < pop ? DOOR + 34 + Math.sin(t * 9) * 3 : t < 6 ? kf([[pop, DOOR + 34], [pop + 0.5, DOOR - 18]], t)
        : t < 9 ? DOOR - 18 : t < 19 ? kf([[9, DOOR - 18], [19, BIN - 150]], t) : kf([[19, BIN - 150], [22.5, DOOR + 30]], t);
      turn = t < 0.8 ? kf([[0, 0], [0.8, 1]], t) : t < 19.5 ? 1 : kf([[19.5, 1], [20.5, 0]], t);
      walk = (t > 9 && t < 19) || t > 19.8 ? 1 : 0;
      dy = t >= pop && t < 9 ? 8 : 0;
      shake = (t >= pop + 0.6 && t < 9) || (t > 9 && t < 19) ? Math.sin(t * 14) * 1.6 : 0;
      alpha = t > 22 ? clamp(1 - (t - 22) / 0.5) : 1;
      b = t < pop ? { show: true, x: DOOR + 20, y: GROUND, r: 0, clip: true }
        : { show: true, x: t < 9 ? kf([[pop, DOOR + 20], [pop + 0.5, DOOR + 130]], t) : kf([[9, DOOR + 130], [19, BIN - 60]], t), y: GROUND, r: t < 9 ? 0 : -3 + Math.sin(t * 6) * 3 };
    }
    return { x, turn, walk, dy, shake, alpha, b };
  };

  const draw = (el) => {
    const t = clamp(el / 1000, 0, total);
    const p = pose(t);
    const bob = p.walk ? Math.abs(Math.sin(t * 9)) * 7 : 0;
    const cx = p.x + p.shake; const cy = GROUND - bob + p.dy;
    chick.setAttribute('transform', `translate(${(cx - 256 * S).toFixed(1)} ${(cy - 392 * S).toFixed(1)}) scale(${S})`);
    chick.setAttribute('opacity', p.alpha.toFixed(2));
    const k = Math.round(p.turn * 50) / 50;
    if (k !== lastTurn) { lastTurn = k; paths.forEach((el2, i) => el2.setAttribute('d', pathOf(CHICK[i].a, CHICK[i].t, k))); }
    if (p.b.show) { bag.style.display = ''; bag.setAttribute('transform', `translate(${p.b.x.toFixed(1)} ${p.b.y.toFixed(1)}) rotate(${p.b.r.toFixed(1)} 0 ${-bh / 2})`); } else bag.style.display = 'none';
  };

  const loop = (now) => { if (!t0) t0 = now; draw(now - t0); raf = requestAnimationFrame(loop); };
  draw(0); raf = requestAnimationFrame(loop);
  return () => cancelAnimationFrame(raf);
}
