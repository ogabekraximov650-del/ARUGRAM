// Xotiradan foydalanish — `lib/screens/storage_screen.dart` (Cherrygram `CacheControlActivity`) nusxasi:
// halqa diagrammasi, toifalar (belgilanadi), "Keshni tozalash / Tanlanganini tozalash", tasdiqlash,
// tozalangach yashil halqa. Ma'lumot — brauzerdagi disk keshi (`tg/chunk-cache.js`).
// Qo'shimcha: "Kesh chegarasi" (Avtomatik / 500 MB ... 5 GB).

import { push } from '../router.js';
import { icon, appBar, bindAppBar, toast, confirmDialog } from '../ui.js';
import { formatBytes } from './stats.js';
import {
  cacheBreakdown, cacheClearCats, cacheStats, cacheLimitSetting, setCacheLimitSetting,
  CAT_VIDEO, CAT_PACKS, CAT_CHAT, CAT_IMG, CAT_OTHER,
} from '../tg/chunk-cache.js';

const COLORS = { [CAT_VIDEO]: '#E2620F', [CAT_IMG]: '#4AA3FF', [CAT_PACKS]: '#B26BFF', [CAT_CHAT]: '#2ECC71', [CAT_OTHER]: '#E8B730' };
const ICONS = { [CAT_VIDEO]: 'movie', [CAT_IMG]: 'image', [CAT_PACKS]: 'emoji_emotions', [CAT_CHAT]: 'forum', [CAT_OTHER]: 'folder' };
const ORDER = [CAT_VIDEO, CAT_IMG, CAT_PACKS, CAT_CHAT, CAT_OTHER];

function ring(slices, total, hi, empty) {
  const R = 86; const C = 2 * Math.PI * R; const gap = 2 / 360 * C;
  let off = 0;
  const arcs = total > 0 ? slices.map((s) => {
    const len = Math.max(0.5, (s.bytes / total) * C - gap);
    const el = `<circle cx="100" cy="100" r="${R}" fill="none" stroke="${COLORS[s.label]}" stroke-width="${hi === s.label ? 22 : 18}"
      stroke-dasharray="${len} ${C - len}" stroke-dashoffset="${-off}" transform="rotate(-90 100 100)" stroke-linecap="butt"></circle>`;
    off += (s.bytes / total) * C;
    return el;
  }).join('') : `<circle cx="100" cy="100" r="${R}" fill="none" stroke="${empty ? '#2ECC71' : 'rgba(255,255,255,0.1)'}" stroke-width="18"></circle>`;
  return `<svg viewBox="0 0 200 200" class="sg-ring">${arcs}</svg>`;
}

export function openStorage() {
  push((el) => {
    let disposed = false; let rows = []; let off = new Set(); let hi = null; let cleared = false; let busy = false; let st = null;
    el.innerHTML = `${appBar({ title: 'Xotiradan foydalanish' })}<div class="scroll sg-body"></div>`;
    bindAppBar(el);
    const body = el.querySelector('.sg-body');
    const sel = () => rows.filter((r) => !off.has(r.label));
    const selBytes = () => sel().reduce((a, r) => a + r.bytes, 0);
    const total = () => rows.reduce((a, r) => a + r.bytes, 0);

    const paint = () => {
      const t = total(); const sb = selBytes(); const all = off.size === 0;
      const GB = 1024 * 1024 * 1024;
      const opts = [['auto', "Avtomatik (qurilma sig'ganicha)"], [`${500 * 1024 * 1024}`, '500 MB'], [`${GB}`, '1 GB'], [`${2 * GB}`, '2 GB'], [`${5 * GB}`, '5 GB']];
      const cur = opts.some(([v]) => v === cacheLimitSetting()) ? cacheLimitSetting() : 'auto';
      body.innerHTML = `<div class="sg-in">
        <div class="sg-chart">${ring(rows, t, hi, cleared && t === 0)}
          <div class="sg-center">${t > 0 ? `<b>${formatBytes(t)}</b><span>Kesh</span>` : `<b>${cleared ? 'Xotira tozalandi' : "Kesh bo'sh"}</b>`}</div></div>
        ${st?.quota ? `<div class="sg-dev">Brauzer ruxsati: <b>${formatBytes(st.quota)}</b> · ishlatilgan ${formatBytes(st.usage)}</div>` : ''}
        <div class="sg-sec">Kesh</div>
        <div class="glass sg-list">${rows.length ? rows.map((r) => `<div class="sg-row${hi === r.label ? ' hi' : ''}" data-l="${r.label}">
          <span class="sg-chk${off.has(r.label) ? '' : ' on'}" style="--c:${COLORS[r.label]}">${off.has(r.label) ? '' : icon('check', { size: 14, color: '#fff' })}</span>
          <span class="sg-ic">${icon(ICONS[r.label], { size: 20, fill: false, color: 'rgba(255,255,255,0.7)' })}</span>
          <span class="sg-t">${r.label}</span><span class="sg-s">${formatBytes(r.bytes)}</span></div>`).join('')
    : '<div class="sg-empty">Saqlangan narsa yo\'q</div>'}</div>
        ${rows.length ? `<button class="sg-btn" ${busy || !sb ? 'disabled' : ''}>${busy ? 'Kesh tozalanmoqda...' : `${all ? 'Keshni tozalash' : 'Tanlanganini tozalash'} (${formatBytes(sb)})`}</button>` : ''}
        <div class="sg-note">Ko'rilgan video bo'laklari, rasmlar va fayllar shu qurilmada shifrlangan holda saqlanadi: qayta ochish va orqaga surish tezroq bo'ladi. Joy tugagandagina eng eskisi o'chadi. Hisobdan chiqsangiz kesh to'liq tozalanadi.</div>
        <div class="sg-sec">Kesh chegarasi</div>
        <div class="glass sg-list"><div class="sg-row" style="cursor:default"><span class="sg-t">Eng katta hajm</span>
          <select class="sg-limit field">${opts.map(([v, l]) => `<option value="${v}"${v === cur ? ' selected' : ''}>${l}</option>`).join('')}</select></div></div>
        <div style="height:40px"></div></div>`;
      body.querySelectorAll('.sg-row[data-l]').forEach((r) => r.addEventListener('click', () => {
        const l = r.dataset.l; if (off.has(l)) off.delete(l); else off.add(l); hi = l; paint();
      }));
      body.querySelector('.sg-btn')?.addEventListener('click', clear);
      body.querySelector('.sg-limit')?.addEventListener('change', async (e) => { await setCacheLimitSetting(e.target.value); toast('Saqlandi'); await load(); });
    };

    async function clear() {
      const labels = new Set(sel().map((r) => r.label));
      const size = selBytes();
      if (!labels.size || size <= 0) return;
      if (!(await confirmDialog(`Keshni tozalash (${formatBytes(size)})`, "Tanlangan toifalardagi saqlangan fayllar o'chiriladi. Ular keyin qayta yuklanadi.", { ok: "O'chirish", danger: true }))) return;
      busy = true; paint();
      try { await cacheClearCats(labels); cleared = true; toast('Kesh tozalandi'); } catch (_) { toast("Tozalab bo'lmadi"); }
      busy = false; off = new Set(); hi = null;
      await load();
    }

    async function load() {
      try {
        const m = await cacheBreakdown();
        rows = ORDER.filter((l) => (m.get(l) || 0) > 0).map((l) => ({ label: l, bytes: m.get(l) }));
        st = await cacheStats();
      } catch (_) { rows = []; }
      if (!disposed) paint();
    }
    paint(); load();
    return { dispose() { disposed = true; } };
  });
}
