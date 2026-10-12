// Xotiradan foydalanish — `lib/screens/storage_screen.dart` (Cherrygram `CacheControlActivity`) ning web nusxasi:
// animatsiyali halqa (`storage-chart.js`), sarlavha + ulush chizig'i, toifa qatorlari (foiz va hajm), "Keshni tozalash",
// tasdiqlash, tozalash pardasi (jo'ja: `chick.js`) va tozalangach yashil belgi. Ma'lumot — brauzerdagi disk keshi.
// Kesh chegarasi YO'Q: qurilmaga sig'ganicha saqlanadi, joy tugaganda eng eskisi o'chadi (`tg/chunk-cache.js`).

import { push } from '../router.js';
import { icon, appBar, bindAppBar, toast, confirmDialog } from '../ui.js';
import { formatBytes } from './stats.js';
import { mountChick, chickDuration } from './chick.js';
import { Chart, pctText, roundPercents } from './storage-chart.js';
import {
  cacheBreakdown, cacheClearCats, cacheStats,
  CAT_VIDEO, CAT_PACKS, CAT_CHAT, CAT_IMG, CAT_OTHER,
} from '../tg/chunk-cache.js';

// Ilovadagi toifalar va ranglar (`storage_screen.dart` `_colors`).
const GROUPS = [
  { label: 'Videolar', cats: [CAT_VIDEO], color: '#3E8CF0' },
  { label: 'Posterlar', cats: [CAT_IMG], color: '#56B6F5' },
  { label: 'Emoji, GIF va stikerlar', cats: [CAT_PACKS], color: '#E08A3C' },
  { label: 'Vaqtinchalik fayllar', cats: [CAT_CHAT, CAT_OTHER], color: '#9A6FE2' },
];
const INFO = "Barcha videolar, rasmlar, stikerlar va emojilar serverda qoladi — kerak bo'lsa ularni qayta yuklab olishingiz mumkin.";
const clamp01 = (v) => Math.min(1, Math.max(0, v));
const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));

export function openStorage() {
  push((el) => {
    let disposed = false; let rows = []; let off = new Set(); let hi = null; let loading = true; let st = null; let drain = null; let chart = null;
    el.classList.add('sg-route');
    el.innerHTML = `<div class="sg-top">${appBar({ title: 'Xotiradan foydalanish' })}</div>
      <div class="scroll sg-body"><div class="sg-card"><div class="sg-chartbox"></div><div class="sg-head"></div><div class="sg-rows"></div><div class="sg-btnbox"></div></div><div class="sg-info"></div><div style="height:16px"></div></div>`;
    bindAppBar(el);
    const top = el.querySelector('.sg-top'); const body = el.querySelector('.sg-body');
    const head = el.querySelector('.sg-head'); const rowsEl = el.querySelector('.sg-rows');
    const btnBox = el.querySelector('.sg-btnbox'); const infoEl = el.querySelector('.sg-info');
    chart = new Chart(el.querySelector('.sg-chartbox'), {
      onPress: (l) => { hi = l; paintRows(); },
      font: getComputedStyle(document.body).fontFamily || 'sans-serif', format: formatBytes,
    });
    body.addEventListener('scroll', () => top.classList.toggle('pinned', body.scrollTop > 230), { passive: true });

    const total = () => rows.reduce((a, r) => a + r.bytes, 0);
    const selected = () => rows.filter((r) => !off.has(r.label));
    const selBytes = () => selected().reduce((a, r) => a + r.bytes, 0);

    const subtitle = (t) => {
      if (loading) return 'Hisoblanmoqda...';
      if (t <= 0) return "Keshdagi barcha fayllar o'chirildi. Ular kerak bo'lganda serverdan qayta yuklab olinadi.";
      const q = st?.quota || 0;
      if (q <= 0) return `ARUGRAM keshi qurilmada ${formatBytes(t)} joy egallagan.`;
      const pct = t / q * 100;
      if (pct < 1) return 'ARUGRAM qurilma xotirasining 1% dan kamroq qismini egallagan.';
      const v = pct < 10 ? pct.toFixed(1).replace('.', ',').replace(',0', '') : String(Math.round(pct));
      return `ARUGRAM qurilma xotirasining ${v}% qismini egallagan.`;
    };

    function paintHead() {
      const t = total(); const hasCache = loading || t > 0; const q = st?.quota || 0;
      const showBar = hasCache && (loading || q > 0);
      let bar = '<div style="height:14px"></div>';
      if (showBar) {
        let segs = '';
        if (loading) segs = '<i style="left:0;right:0;background:rgba(255,255,255,0.08);border-radius:2px"></i>';
        else {
          const w = Math.min(174, (head.clientWidth || 360) * 0.8);
          const a = Math.max(4, t / q * w); const b = Math.max(4, Math.min(1, (st?.usage || 0) / q) * w);
          segs = `<i style="left:${Math.max(a, b) + 1}px;right:0;background:rgba(255,255,255,0.08);border-radius:1px 2px 2px 1px"></i>
            <i style="left:${a + 1}px;width:${Math.max(0, b - a - 1)}px;background:rgb(${Math.round(0xE2 * 0.25 + 0x15 * 0.75)},${Math.round(0x62 * 0.25 + 0x15 * 0.75)},${Math.round(0x0F * 0.25 + 0x17 * 0.75)})"></i>
            <i style="left:0;width:${a}px;background:var(--accent2);border-radius:2px 1px 1px 2px"></i>`;
        }
        bar = `<div class="sg-bar">${segs}</div><div style="height:18px"></div>`;
      }
      head.innerHTML = `<div class="sg-h">${hasCache ? 'Xotiradan foydalanish' : 'Xotira tozalandi'}</div><div class="sg-sub">${subtitle(t)}</div>${bar}`;
    }

    function paintRows() {
      const t = total();
      if (loading) {
        rowsEl.innerHTML = Array.from({ length: 5 }, () => '<div class="sg-row ld"><span class="sg-chk ldb"></span><span class="ldb w140"></span><span class="sp"></span><span class="ldb w48"></span></div>').join('');
        return;
      }
      const pcts = roundPercents(rows.map((r) => (t > 0 ? r.bytes / t : 0)));
      rowsEl.innerHTML = rows.map((r, i) => `<div class="sg-row${hi === r.label ? ' hi' : ''}${i < rows.length - 1 ? ' dv' : ''}" data-l="${esc(r.label)}">
        <span class="sg-chk${off.has(r.label) ? '' : ' on'}" style="--c:${r.color}">${icon('check', { size: 15, color: '#fff' })}</span>
        <span class="sg-t">${esc(r.label)}<b>${pctText(pcts[i] || 0)}</b></span><span class="sg-s">${formatBytes(r.bytes)}</span></div>`).join('');
      rowsEl.querySelectorAll('.sg-row[data-l]').forEach((r) => r.addEventListener('click', () => {
        if (drain != null) return;
        const l = r.dataset.l; if (off.has(l)) off.delete(l); else off.add(l);
        refresh();
      }));
    }

    function paintBtn() {
      const t = total(); const sb = selBytes(); const all = off.size === 0; const hasCache = loading || t > 0;
      if (!hasCache) { btnBox.innerHTML = ''; infoEl.innerHTML = ''; return; }
      const enabled = !loading && sb > 0 && drain == null;
      btnBox.innerHTML = `<button class="sg-btn${enabled ? '' : ' dis'}" ${enabled ? '' : 'disabled'}>${all ? 'Keshni tozalash' : 'Tanlanganini tozalash'}${sb > 0 ? `<span>${formatBytes(sb)}</span>` : ''}</button>`;
      btnBox.querySelector('.sg-btn')?.addEventListener('click', clear);
      infoEl.innerHTML = `<div class="sg-note">${INFO}</div>`;
    }

    function refresh() {
      const t = total();
      chart.set({
        slices: selected().map((r) => ({ label: r.label, bytes: r.bytes, color: r.color })),
        selectedBytes: selBytes(), loading, complete: !loading && t <= 0, drain,
      });
      paintHead(); paintRows(); paintBtn();
    }

    async function clear() {
      const chosen = selected();
      const cats = new Set(chosen.flatMap((r) => r.cats));
      const size = selBytes();
      if (!cats.size || size <= 0) return;
      if (!(await confirmDialog(`Keshni tozalash (${formatBytes(size)})`, "Tanlangan toifalardagi saqlangan fayllar o'chiriladi. Ular keyin qayta yuklanadi.", { ok: "O'chirish", danger: true }))) return;
      // Jo'ja chiqindini qutiga tashlaydi (ilovadagi kabi): parda animatsiya oxirigacha ko'rinadi, halqa jo'ja bilan
      // bir sur'atda bo'shaydi, foiz vaqtga bog'langan (tozalash tez tugasa ham sekin o'sadi), ish tugamasa 95% da kutadi.
      const sheet = document.createElement('div');
      sheet.className = 'ch-sheet';
      sheet.innerHTML = `<div class="ch-host"></div><div class="ch-pct">0.00%</div>
        <div class="ch-bar"><i></i></div><div class="ch-t">Kesh tozalanmoqda</div><div class="ch-d">Kesh tozalanayotganda bu oynani yopmang.</div>`;
      el.appendChild(sheet);
      drain = 0; refresh();
      const stop = mountChick(sheet.querySelector('.ch-host'), size);
      const minMs = chickDuration(size); const t0 = performance.now(); let done = false; let ok = true;
      const pctEl = sheet.querySelector('.ch-pct'); const barEl = sheet.querySelector('.ch-bar i');
      const tick = () => {
        const r = (performance.now() - t0) / minMs;
        drain = done ? clamp01(r) : Math.min(0.95, clamp01(r));
        pctEl.textContent = pctText(drain * 100).replace('<0.01%', '0.00%'); barEl.style.width = `${drain * 100}%`;
        chart.set({ slices: [], selectedBytes: 0, loading: false, complete: false, drain });
        if (!(done && r >= 1) && !disposed) requestAnimationFrame(tick);
      };
      requestAnimationFrame(tick);
      try { await cacheClearCats(cats); } catch (_) { ok = false; }
      done = true;
      await new Promise((res) => { const w = () => ((performance.now() - t0 >= minMs || disposed) ? res() : setTimeout(w, 100)); w(); });
      stop(); sheet.remove(); drain = null; off = new Set(); hi = null;
      toast(ok ? `${formatBytes(size)} kesh tozalandi` : "Tozalab bo'lmadi");
      await load();
    }

    async function load() {
      try {
        const m = await cacheBreakdown();
        rows = GROUPS.map((g) => ({ ...g, bytes: g.cats.reduce((a, c) => a + (m.get(c) || 0), 0) })).filter((g) => g.bytes > 0).sort((a, b) => b.bytes - a.bytes);
        st = await cacheStats();
      } catch (_) { rows = []; }
      loading = false;
      if (!disposed) refresh();
    }
    refresh(); load();
    return { dispose() { disposed = true; chart?.destroy(); } };
  });
}
