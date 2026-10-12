// Xotiradan foydalanish — `lib/screens/storage_screen.dart` (Cherrygram `CacheControlActivity`) nusxasi:
// halqa diagrammasi, toifalar (belgilanadi), "Keshni tozalash / Tanlanganini tozalash", tasdiqlash,
// tozalangach yashil halqa. Ma'lumot — brauzerdagi disk keshi (`tg/chunk-cache.js`).
// Kesh chegarasi YO'Q: qurilmaga sig'ganicha saqlanadi, joy tugaganda eng eskisi o'chadi (`tg/chunk-cache.js`).

import { push } from '../router.js';
import { icon, appBar, bindAppBar, toast, confirmDialog } from '../ui.js';
import { formatBytes } from './stats.js';
import { mountChick, chickDuration } from './chick.js';
import {
  cacheBreakdown, cacheClearCats, cacheStats,
  CAT_VIDEO, CAT_PACKS, CAT_CHAT, CAT_IMG, CAT_OTHER,
} from '../tg/chunk-cache.js';

// Ilovadagi (Flutter) ekran bilan bir xil toifalar: Videolar, Posterlar, Emoji/GIF/stikerlar, Vaqtinchalik fayllar.
const GROUPS = [
  { label: 'Videolar', cats: [CAT_VIDEO], color: '#3E8CF0' },
  { label: 'Posterlar', cats: [CAT_IMG], color: '#56B6F5' },
  { label: 'Emoji, GIF va stikerlar', cats: [CAT_PACKS], color: '#E08A3C' },
  { label: 'Vaqtinchalik fayllar', cats: [CAT_CHAT, CAT_OTHER], color: '#9A6FE2' },
];
const INFO = "Barcha videolar, rasmlar, stikerlar va emojilar serverda qoladi \u2014 kerak bo'lsa ularni qayta yuklab olishingiz mumkin.";

/** Foizlar yuzdan bir aniqlikda, jami 100 (ilovadagi `_roundPercents`). */
function roundPercents(shares) {
  const raw = shares.map((s) => s * 10000); const out = raw.map((r) => Math.floor(r));
  let left = Math.round(raw.reduce((a, b) => a + b, 0)) - out.reduce((a, b) => a + b, 0);
  raw.map((r, i) => i).sort((a, b) => (raw[b] - out[b]) - (raw[a] - out[a])).forEach((i) => { if (left > 0) { out[i]++; left--; } });
  return out.map((o) => o / 100);
}
const clamp01 = (v) => Math.min(1, Math.max(0, v));
const pctText = (p) => (p > 0 && p < 0.005 ? '<0.01%' : `${p.toFixed(2)}%`);

function ring(slices, total, empty) {
  const R = 86; const C = 2 * Math.PI * R; const gap = total > 0 && slices.length > 1 ? 2 / 360 * C : 0;
  let off = 0;
  const arcs = total > 0 ? slices.map((s) => {
    const len = Math.max(0.5, (s.bytes / total) * C - gap);
    const el = `<circle cx="100" cy="100" r="${R}" fill="none" stroke="${s.color}" stroke-width="40"
      stroke-dasharray="${len} ${C - len}" stroke-dashoffset="${-off}" transform="rotate(-90 100 100)" stroke-linecap="butt"></circle>`;
    off += (s.bytes / total) * C;
    return el;
  }).join('') : `<circle cx="100" cy="100" r="${R}" fill="none" stroke="${empty ? '#41BA71' : 'rgba(255,255,255,0.1)'}" stroke-width="40"></circle>`;
  return `<svg viewBox="0 0 200 200" class="sg-ring">${arcs}</svg>`;
}

/** Bo'lak o'rtasidagi foiz yorlig'i (halqa tashqarisida). */
function labels(slices, total, pcts) {
  if (total <= 0) return '';
  let acc = 0;
  return slices.map((s, i) => {
    const mid = (acc + s.bytes / 2) / total; acc += s.bytes;
    const ang = mid * 2 * Math.PI - Math.PI / 2;
    const x = 50 + Math.cos(ang) * 58; const y = 50 + Math.sin(ang) * 58;
    return `<span class="sg-pl" style="left:${x}%;top:${y}%;color:${s.color}">${pctText(pcts[i]).replace('<0.01%', '0.00%')}</span>`;
  }).join('');
}

export function openStorage() {
  push((el) => {
    let disposed = false; let rows = []; let off = new Set(); let hi = null; let cleared = false; let busy = false; let st = null; let drain = 0;
    el.innerHTML = `${appBar({ title: '' })}<div class="scroll sg-body"></div>`;
    bindAppBar(el);
    const body = el.querySelector('.sg-body');
    const sel = () => rows.filter((r) => !off.has(r.label));
    const selBytes = () => sel().reduce((a, r) => a + r.bytes, 0);
    const total = () => rows.reduce((a, r) => a + r.bytes, 0);

    const subtitle = (t) => {
      if (!t) return cleared ? "Keshdagi barcha fayllar o'chirildi. Ular kerak bo'lganda serverdan qayta yuklab olinadi." : '';
      const q = st?.quota || 0;
      if (q <= 0) return `ARUGRAM keshi qurilmada ${formatBytes(t)} joy egallagan.`;
      const pct = t / q * 100;
      if (pct < 1) return "ARUGRAM qurilma xotirasining 1% dan kamroq qismini egallagan.";
      const v = pct < 10 ? pct.toFixed(1).replace('.', ',').replace(',0', '') : String(Math.round(pct));
      return `ARUGRAM qurilma xotirasining ${v}% qismini egallagan.`;
    };

    const paint = () => {
      // Tozalash paytida tanlangan toifalar halqada jo'ja bilan birga bo'shaydi.
      const eff = rows.map((r) => (drain > 0 && !off.has(r.label) ? { ...r, bytes: r.bytes * (1 - drain) } : r));
      const t = eff.reduce((a, r) => a + r.bytes, 0); const sb = selBytes(); const all = off.size === 0;
      const pcts = roundPercents(eff.map((r) => (t > 0 ? r.bytes / t : 0)));
      const q = st?.quota || 0; const share = q > 0 && t > 0 ? Math.min(1, Math.max(0.03, t / q)) : 0;
      const sz = formatBytes(t).split(' ');
      body.innerHTML = `<div class="sg-in">
        <div class="sg-chart">${ring(eff, t, cleared && t === 0)}${labels(eff, t, pcts)}
          <div class="sg-center">${t > 0 ? `<b>${sz[0]}</b><span>${sz[1] || ''}</span>` : `<b>${cleared ? 'Xotira tozalandi' : "Kesh bo'sh"}</b>`}</div></div>
        <div class="sg-h">${t > 0 ? 'Xotiradan foydalanish' : (cleared ? 'Xotira tozalandi' : 'Xotiradan foydalanish')}</div>
        <div class="sg-sub">${subtitle(t)}</div>
        ${t > 0 && q > 0 ? `<div class="sg-bar"><i style="width:${(share * 100).toFixed(1)}%"></i></div>` : '<div style="height:14px"></div>'}
        <div class="sg-list">${rows.length ? rows.map((r, i) => `<div class="sg-row${hi === r.label ? ' hi' : ''}" data-l="${r.label}">
          <span class="sg-chk${off.has(r.label) ? '' : ' on'}" style="--c:${r.color}">${off.has(r.label) ? '' : icon('check', { size: 14, color: '#fff' })}</span>
          <span class="sg-t">${r.label}<b>${pctText(pcts[i])}</b></span><span class="sg-s">${formatBytes(r.bytes)}</span></div>`).join('')
    : ''}</div>
        ${rows.length ? `<button class="sg-btn" ${busy || !sb ? 'disabled' : ''}>${busy ? 'Kesh tozalanmoqda...' : `${all ? 'Keshni tozalash' : 'Tanlanganini tozalash'} ${formatBytes(sb)}`}</button>` : ''}
        <div class="sg-note">${INFO}</div>
        <div style="height:40px"></div></div>`;
      body.querySelectorAll('.sg-row[data-l]').forEach((r) => r.addEventListener('click', () => {
        const l = r.dataset.l; if (off.has(l)) off.delete(l); else off.add(l); hi = l; paint();
      }));
      body.querySelector('.sg-btn')?.addEventListener('click', clear);
    };

    async function clear() {
      const chosen = sel();
      const labels = new Set(chosen.flatMap((r) => r.cats));
      const size = selBytes();
      if (!labels.size || size <= 0) return;
      if (!(await confirmDialog(`Keshni tozalash (${formatBytes(size)})`, "Tanlangan toifalardagi saqlangan fayllar o'chiriladi. Ular keyin qayta yuklanadi.", { ok: "O'chirish", danger: true }))) return;
      busy = true; paint();
      // Jo'ja chiqindini qutiga tashlaydi (ilovadagi kabi): animatsiya oxirigacha parda ko'rinadi,
      // foiz vaqtga bog'langan (tozalash tez tugasa ham sekin o'sadi), ish tugamasa 95% da kutadi.
      const sheet = document.createElement('div');
      sheet.className = 'ch-sheet';
      sheet.innerHTML = `<div class="ch-host"></div><div class="ch-pct">0.00%</div>
        <div class="ch-bar"><i></i></div><div class="ch-t">Kesh tozalanmoqda</div><div class="ch-d">Kesh tozalanayotganda bu oynani yopmang.</div>`;
      el.appendChild(sheet);
      const stop = mountChick(sheet.querySelector('.ch-host'), size);
      const minMs = chickDuration(size); const t0 = performance.now(); let done = false; let ok = true;
      const pctEl = sheet.querySelector('.ch-pct'); const barEl = sheet.querySelector('.ch-bar i');
      const tick = () => {
        const r = (performance.now() - t0) / minMs;
        drain = done ? clamp01(r) : Math.min(0.95, clamp01(r));
        pctEl.textContent = `${(drain * 100).toFixed(2)}%`; barEl.style.width = `${drain * 100}%`;
        paint();
        if (!(done && r >= 1) && !disposed) requestAnimationFrame(tick);
      };
      requestAnimationFrame(tick);
      try { await cacheClearCats(labels); cleared = true; } catch (_) { ok = false; }
      done = true;
      await new Promise((res) => { const w = () => ((performance.now() - t0 >= minMs || disposed) ? res() : setTimeout(w, 100)); w(); });
      stop(); sheet.remove(); drain = 0;
      toast(ok ? 'Kesh tozalandi' : "Tozalab bo'lmadi");
      busy = false; off = new Set(); hi = null;
      await load();
    }

    async function load() {
      try {
        const m = await cacheBreakdown();
        rows = GROUPS.map((g) => ({ ...g, bytes: g.cats.reduce((a, c) => a + (m.get(c) || 0), 0) })).filter((g) => g.bytes > 0);
        st = await cacheStats();
      } catch (_) { rows = []; }
      if (!disposed) paint();
    }
    paint(); load();
    return { dispose() { disposed = true; } };
  });
}
