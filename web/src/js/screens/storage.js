// Xotiradan foydalanish — `lib/screens/storage_screen.dart` (Cherrygram `CacheControlActivity`) ning web nusxasi:
// animatsiyali halqa (`storage-chart.js`), sarlavha + ulush chizig'i, toifa qatorlari (foiz va hajm), "Keshni tozalash",
// tasdiqlash, tozalash pardasi (jo'ja: `chick.js`) va tozalangach yashil belgi. Ma'lumot — brauzerdagi disk keshi.
// Kesh chegarasi YO'Q: qurilmaga sig'ganicha saqlanadi, joy tugaganda eng eskisi o'chadi (`tg/chunk-cache.js`).

import { push } from '../router.js';
import { icon, appBar, bindAppBar, toast, confirmDialog } from '../ui.js';
import { formatBytes } from './stats.js';
import { mountChick, chickDuration } from './chick.js';
import { Chart, pctText, roundPercents } from './storage-chart.js';
import { lib, loadLibrary } from './packs.js';
import {
  cacheBreakdown, cacheClearCats, cacheStats, cachePackBreakdown, cacheClearPackKinds,
  CAT_VIDEO, CAT_PACKS, CAT_CHAT, CAT_IMG, CAT_OTHER,
} from '../tg/chunk-cache.js';

// Ilovadagi toifalar va ranglar (`storage_screen.dart` `_colors`).
const GROUPS = [
  { label: 'Videolar', cats: [CAT_VIDEO], color: '#3E8CF0' },
  { label: 'Posterlar', cats: [CAT_IMG], color: '#56B6F5' },
  { label: 'Emoji, GIF va stikerlar', cats: [CAT_PACKS], color: '#E08A3C' },
  { label: 'Vaqtinchalik fayllar', cats: [CAT_CHAT, CAT_OTHER], color: '#9A6FE2' },
];
// "Emoji, GIF va stikerlar" ichki bo'limlari (`_kPackEmoji` ...).
const PACK_KIDS = [
  { label: 'Emoji', kind: 'emoji', color: '#F0B04A' },
  { label: 'GIF', kind: 'gif', color: '#E0703C' },
  { label: 'Stiker', kind: 'sticker', color: '#C95A8A' },
];
const PACKS = 'Emoji, GIF va stikerlar';
const INFO = "Barcha videolar, rasmlar, stikerlar va emojilar serverda qoladi — kerak bo'lsa ularni qayta yuklab olishingiz mumkin.";
const clamp01 = (v) => Math.min(1, Math.max(0, v));
const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));

export function openStorage() {
  push((el) => {
    let disposed = false; let rows = []; let off = new Set(); let hi = null; let loading = true; let st = null; let drain = null; let chart = null; let packsOpen = false; let packKinds = new Map();
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

    const kids = (r) => r.children || [];
    const total = () => rows.reduce((a, r) => a + r.bytes, 0);
    // Ilovadagi `_isOn` / `_fullyOn` / `_bytesOf` / `_labelsOf`.
    const isOn = (r) => (kids(r).length ? kids(r).some((c) => !off.has(c.label)) : !off.has(r.label));
    const fullyOn = (r) => (kids(r).length ? kids(r).every((c) => !off.has(c.label)) : !off.has(r.label));
    const bytesOf = (r) => {
      if (!kids(r).length) return off.has(r.label) ? 0 : r.bytes;
      const on = kids(r).filter((c) => !off.has(c.label));
      const rest = r.bytes - kids(r).reduce((a, c) => a + c.bytes, 0);
      return on.reduce((a, c) => a + c.bytes, 0) + (on.length === kids(r).length ? rest : 0);
    };
    const selected = () => rows.filter((r) => bytesOf(r) > 0);
    const selBytes = () => rows.reduce((a, r) => a + bytesOf(r), 0);
    const allSelected = () => rows.every(fullyOn);
    const toggleRow = (r) => {
      const labels = kids(r).length ? kids(r).map((c) => c.label) : [r.label];
      if (fullyOn(r)) labels.forEach((l) => off.add(l)); else labels.forEach((l) => off.delete(l));
    };

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
      const chk = (on, color) => `<span class="sg-chk${on ? ' on' : ''}" style="--c:${color}" data-chk="1">${icon('check', { size: 15, color: '#fff' })}</span>`;
      const html = [];
      rows.forEach((r, i) => {
        const open = kids(r).length && packsOpen;
        const dv = i < rows.length - 1 || open;
        html.push(`<div class="sg-row${hi === r.label ? ' hi' : ''}${dv ? ' dv' : ''}" data-l="${esc(r.label)}" data-i="${i}">
          ${chk(isOn(r), r.color)}
          <span class="sg-t">${esc(r.label)}<b>${pctText(pcts[i] || 0)}</b></span>
          ${kids(r).length ? `<span class="sg-chev" data-chev="1">${icon(packsOpen ? 'keyboard_arrow_up' : 'keyboard_arrow_down', { size: 22, color: '#8A8A8E' })}</span>` : ''}
          <span class="sg-s">${formatBytes(r.bytes)}</span></div>`);
        if (open) kids(r).forEach((c, k) => {
          const pc = t > 0 ? Math.round(c.bytes * 10000 / t) / 100 : 0;
          html.push(`<div class="sg-row kid${k < kids(r).length - 1 || i < rows.length - 1 ? ' dv' : ''}" data-kid="${esc(c.label)}">
            ${chk(!off.has(c.label), c.color)}<span class="sg-t">${esc(c.label)}<b>${pctText(pc)}</b></span><span class="sg-s">${formatBytes(c.bytes)}</span></div>`);
        });
      });
      rowsEl.innerHTML = html.join('');
      rowsEl.querySelectorAll('.sg-row[data-i]').forEach((rowEl) => {
        const r = rows[Number(rowEl.dataset.i)];
        rowEl.addEventListener('click', (e) => {
          if (drain != null) return;
          const onCheck = e.target.closest('[data-chk]');
          // Ichki bo'limli toifa: qatorga bosish ochadi/yopadi, doira — hammasini tanlaydi.
          if (kids(r).length && !onCheck) { packsOpen = !packsOpen; paintRows(); return; }
          toggleRow(r); refresh();
        });
      });
      rowsEl.querySelectorAll('.sg-row[data-kid]').forEach((rowEl) => rowEl.addEventListener('click', () => {
        if (drain != null) return;
        const l = rowEl.dataset.kid; if (off.has(l)) off.delete(l); else off.add(l);
        refresh();
      }));
    }

    function paintBtn() {
      const t = total(); const sb = selBytes(); const all = allSelected(); const hasCache = loading || t > 0;
      if (!hasCache) { btnBox.innerHTML = ''; infoEl.innerHTML = ''; return; }
      const enabled = !loading && sb > 0 && drain == null;
      btnBox.innerHTML = `<button class="sg-btn${enabled ? '' : ' dis'}" ${enabled ? '' : 'disabled'}>${all ? 'Keshni tozalash' : 'Tanlanganini tozalash'}${sb > 0 ? `<span>${formatBytes(sb)}</span>` : ''}</button>`;
      btnBox.querySelector('.sg-btn')?.addEventListener('click', clear);
      infoEl.innerHTML = `<div class="sg-note">${INFO}</div>`;
    }

    function refresh() {
      const t = total();
      chart.set({
        slices: rows.filter((r) => bytesOf(r) > 0).map((r) => ({ label: r.label, bytes: bytesOf(r), color: r.color })),
        selectedBytes: selBytes(), loading, complete: !loading && t <= 0, drain,
      });
      paintHead(); paintRows(); paintBtn();
    }

    async function clear() {
      const cats = new Set(); const wantKinds = new Set();
      for (const r of rows) {
        if (!kids(r).length) { if (!off.has(r.label)) r.cats.forEach((c) => cats.add(c)); continue; }
        const on = kids(r).filter((c) => !off.has(c.label));
        if (on.length === kids(r).length) r.cats.forEach((c) => cats.add(c)); else on.forEach((c) => wantKinds.add(c.kind));
      }
      const size = selBytes();
      if ((!cats.size && !wantKinds.size) || size <= 0) return;
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
      try { await cacheClearCats(cats); if (wantKinds.size) await cacheClearPackKinds(packKinds, wantKinds); } catch (_) { ok = false; }
      done = true;
      await new Promise((res) => { const w = () => ((performance.now() - t0 >= minMs || disposed) ? res() : setTimeout(w, 100)); w(); });
      stop(); sheet.remove(); drain = null; off = new Set(); hi = null;
      toast(ok ? `${formatBytes(size)} kesh tozalandi` : "Tozalab bo'lmadi");
      await load();
    }

    async function load() {
      try {
        const m = await cacheBreakdown();
        try { await loadLibrary(); } catch (_) { /* */ }
        packKinds = new Map([...(lib.mine || []), ...(lib.subs || [])].map((p) => [Number(p.id), p.kind]));
        const pk = await cachePackBreakdown(packKinds);
        rows = GROUPS.map((g) => {
          const row = { ...g, bytes: g.cats.reduce((a, c) => a + (m.get(c) || 0), 0) };
          if (g.label === PACKS) row.children = PACK_KIDS.filter((k) => (pk.get(k.kind) || 0) > 0).map((k) => ({ ...k, bytes: pk.get(k.kind) }));
          return row;
        }).filter((g) => g.bytes > 0).sort((a, b) => b.bytes - a.bytes);
        st = await cacheStats();
      } catch (_) { rows = []; }
      loading = false;
      if (!disposed) refresh();
    }
    refresh(); load();
    return { dispose() { disposed = true; chart?.destroy(); } };
  });
}
