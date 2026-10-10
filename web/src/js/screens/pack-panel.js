// Emoji panelining GIF va Stiker sahifalari — `lib/widgets/tg_composer.dart` (`_PackPage`).
//   Stiker: yuqorida qator (⭐ saralangan, 🕒 yaqinda, to'plam muqovalari, ➕), qidiruv, bo'limlar, katakchalar (min 72, kamida 4 ustun).
//   GIF: qator yo'q; qidiruv + zich "devor" (qatorlar kenglikka to'liq, elementlar o'z nisbatida).
//   Bosish — yuborish; bosib turish — katta ko'rinish + menyu.

import { icon, promptDialog, toast } from '../ui.js';
import { esc } from '../format.js';
import { currentUser } from '../api.js';
import {
  lib, loadLibrary, packHeader, packThumb, usablePacks, packFavorites, packRecents, notePackRecent,
  togglePackFavorite, isPackFavorite, bindPress, showPackPreview, openMyPacks, openPackDetail, packOp, packSingle,
} from './packs.js';

const CHIPS = ['❤️', '👍', '👎', '🎉', '😊', '😢', '😂', '🔥', '🙏', '😡'];
const base = (e) => `${e || ''}`.replace(/️/g, '');
const STRIP_ITEM = 33;

export function createPackPage(kind, { onPick }) {
  const gif = kind === 'gif';
  const el = document.createElement('div');
  el.className = 'tgp';
  el.innerHTML = `${gif ? '' : '<div class="tge-strip tgp-strip"><div class="tge-strip-in"><div class="tge-sel"></div></div></div>'}
    <div class="tgp-scroll${gif ? ' gif' : ''}"><div class="tgp-q"></div><div class="tgp-secs"></div><div style="height:70px"></div></div>`;
  const scroll = el.querySelector('.tgp-scroll');
  const qEl = el.querySelector('.tgp-q');
  const secsEl = el.querySelector('.tgp-secs');
  const strip = el.querySelector('.tgp-strip');
  const stripIn = el.querySelector('.tge-strip-in');
  const sel = el.querySelector('.tge-sel');
  const hdr = new Map(); // to'plam id -> {h, f, base}
  let query = ''; let chip = '';
  let packs = []; let loaded = false; let secs = []; let current = 0;
  let io = null;

  const keep = (emoji) => !chip || base(emoji).includes(base(chip));
  const matches = (title, emoji) => (!query || title.toLowerCase().includes(query.toLowerCase()) || `${emoji}`.includes(query)) && keep(emoji);

  // ── Qidiruv qatori ──
  function paintQ() {
    qEl.innerHTML = `<div class="tgp-qin"><div class="s">${icon('search', { fill: false, size: 22, color: '#8E8E93' })}<span class="${query ? 'has' : ''}">${esc(query || 'Qidiruv')}</span></div>
      <div class="ch">${CHIPS.map((c) => `<div class="c${chip === c ? ' on' : ''}" data-c="${c}">${c}</div>`).join('')}</div></div>`;
    qEl.querySelector('.s').addEventListener('click', async () => {
      const r = await promptDialog('Qidiruv', { value: query, placeholder: "To'plam nomi yoki emoji", ok: 'Qidirish' });
      if (r != null) { query = `${r}`.trim(); paintQ(); paintSecs(); }
    });
    qEl.querySelectorAll('.c').forEach((n) => n.addEventListener('click', () => { chip = chip === n.dataset.c ? '' : n.dataset.c; paintQ(); paintSecs(); }));
  }

  const itemOf = (pack, item) => (hdr.get(pack)?.h.items || []).find((x) => x.i === item);
  const packOf = (id) => usablePacks(kind).find((p) => p.id === id) || lib.mine.concat(lib.subs).find((p) => p.id === id);
  const aspectOf = (pick) => { const it = itemOf(pick.pack, pick.item); return it && it.w > 0 && it.h > 0 ? Math.min(3.5, Math.max(0.4, it.w / it.h)) : 1; };

  function pickOf(p, it) { return { kind, pack: p.id, item: it.i, emoji: it.e || '' }; }

  function sections() {
    const favs = query ? [] : packFavorites(kind).filter((r) => keep(r.emoji));
    const rec = query ? [] : packRecents(kind).filter((r) => keep(r.emoji));
    const out = [
      { title: 'Saralanganlar', ic: 'star', cells: favs },
      { title: 'Yaqinda ishlatilgan', ic: 'access_time', cells: rec },
    ];
    packs.forEach((p) => {
      const hd = hdr.get(p.id);
      out.push({ title: p.title, pack: p, cells: hd ? (hd.h.items || []).filter((it) => matches(p.title, it.e || '')).map((it) => pickOf(p, it)) : [] });
    });
    return out;
  }

  const thumbInto = (cell, pick) => {
    const p = packOf(pick.pack); const hd = hdr.get(pick.pack); const it = itemOf(pick.pack, pick.item);
    if (!p || !hd || !it) return;
    packThumb(p, hd, it).then((b) => { if (cell.isConnected) cell.innerHTML = `<img src="${b.url}" alt="" draggable="false">`; }).catch(() => {});
  };

  function press(cellEl, pick, aspect) {
    bindPress(cellEl, {
      scale: gif ? 0.96 : 0.85,
      onTap: () => { notePackRecent(pick); onPick?.(pick); },
      onLong: () => preview(pick, aspect),
    });
  }

  function preview(pick, aspect) {
    const info = packOf(pick.pack);
    const me = currentUser()?.id || 0;
    const what = gif ? 'GIF' : 'Stiker';
    const fav = isPackFavorite(pick);
    showPackPreview({
      pick, aspect,
      actions: [
        { icon: 'send', text: `${what} yuborish`, run: () => { notePackRecent(pick); onPick?.(pick); } },
        { icon: fav ? 'star' : 'star', text: fav ? "Saralanganlardan o'chirish" : "Saralanganlarga qo'shish", run: () => { togglePackFavorite(pick); paintSecs(); } },
        ...(info && me > 0 && info.owner_id === me ? [{ icon: 'delete', text: "To'plamdan o'chirish", danger: true, run: () => { packOp(`p:rm:${pick.pack}:${pick.item}`, { op: 'remove', pack: pick.pack, item: pick.item }); toast("O'chirish navbatga qo'yildi"); } }] : []),
      ],
    });
  }

  function paintSecs() {
    secs = sections();
    if (!packs.length && !secs[0].cells.length && !secs[1].cells.length) {
      secsEl.innerHTML = `<div class="tgp-empty">${loaded ? `${icon(gif ? 'gif_box' : 'emoji_emotions', { fill: false, size: 56, color: 'rgba(255,255,255,0.3)' })}
        <div class="t">${packSingle(kind)} to'plami yo'q</div><div class="s">Ommaviy to'plamlardan qo'shing yoki o'zingiznikini yarating</div><div class="b">To'plamlarni boshqarish</div>`
        : '<div class="spinner" style="width:30px;height:30px"></div>'}</div>`;
      secsEl.querySelector('.b')?.addEventListener('click', () => openMyPacks(kind));
      paintStrip(); return;
    }
    const w = (scroll.clientWidth || window.innerWidth) - 0;
    const total = secs.reduce((a, s) => a + s.cells.length, 0);
    if (!total && (query || chip)) { secsEl.innerHTML = `<div class="tgp-empty"><div class="t">Hech narsa topilmadi</div></div>`; paintStrip(); return; }
    let html = '';
    secs.forEach((s, si) => {
      if (!s.cells.length) return;
      html += `<div class="tgp-sec" data-s="${si}"><div class="tge-head${s.pack ? ' tap' : ''}" data-s="${si}">${esc(s.title)}</div>`;
      if (gif) {
        const gap = 2; const target = 118; let row = []; let sum = 0; const rows = [];
        const flush = (full) => {
          if (!row.length) return;
          const gaps = gap * (row.length - 1);
          const h = Math.min(260, Math.max(60, full ? (w - gaps) / sum : target));
          rows.push({ cells: row, h, full, sum }); row = []; sum = 0;
        };
        s.cells.forEach((c) => { const a = aspectOf(c); row.push({ c, a }); sum += a; if (sum * target + gap * (row.length - 1) >= w) flush(true); });
        flush(false);
        html += rows.map((r) => {
          const gaps = gap * (r.cells.length - 1);
          return `<div class="tgp-row" style="height:${r.h}px">${r.cells.map((x) => `<div class="tgp-gc" data-p="${x.c.pack}" data-i="${x.c.item}" style="width:${r.full ? ((w - gaps) * x.a / r.sum) : r.h * x.a}px;height:${r.h}px"></div>`).join('')}</div>`;
        }).join('');
      } else {
        const cols = Math.max(4, Math.floor(w / 72)); const cell = w / cols;
        html += `<div class="tgp-grid" style="grid-template-columns:repeat(${cols},${cell}px);grid-auto-rows:${cell}px">${s.cells.map((c) => `<div class="tgp-cell" data-p="${c.pack}" data-i="${c.item}"><div class="in"></div></div>`).join('')}</div>`;
      }
      html += '</div>';
    });
    secsEl.innerHTML = html;
    if (io) io.disconnect();
    io = new IntersectionObserver((ents) => ents.forEach((en) => {
      if (!en.isIntersecting) return;
      io.unobserve(en.target);
      const pick = { kind, pack: +en.target.dataset.p, item: +en.target.dataset.i };
      const inner = gif ? en.target : en.target.querySelector('.in');
      thumbInto(inner, pick);
    }), { root: scroll, rootMargin: '300px' });
    secsEl.querySelectorAll(gif ? '.tgp-gc' : '.tgp-cell').forEach((n) => {
      const pick = secs.flatMap((s) => s.cells).find((c) => c.pack === +n.dataset.p && c.item === +n.dataset.i);
      if (!pick) return;
      press(n, pick, aspectOf(pick));
      io.observe(n);
    });
    secsEl.querySelectorAll('.tge-head.tap').forEach((n) => n.addEventListener('click', () => { const p = secs[+n.dataset.s].pack; if (p) openPackDetail(p, { mine: lib.mine.some((x) => x.id === p.id) }, () => {}); }));
    paintStrip();
  }

  // ── Yuqoridagi qator (Stiker sahifasi) ──
  function paintStrip() {
    if (!strip) return;
    const items = [{ ic: 'star' }, { ic: 'access_time' }, ...packs.map((p) => ({ p })), { ic: 'add_circle', add: true }];
    stripIn.style.width = `${items.length * STRIP_ITEM}px`;
    stripIn.querySelectorAll('.tge-tab').forEach((n) => n.remove());
    stripIn.insertAdjacentHTML('beforeend', items.map((it, i) => `<div class="tge-tab" data-i="${i}">${it.p ? '<span class="cv"></span>' : icon(it.ic, { fill: false, size: 22 })}</div>`).join(''));
    stripIn.querySelectorAll('.tge-tab').forEach((n, i) => {
      const it = items[i];
      if (it.p) {
        const hd = hdr.get(it.p.id); const first = hd?.h.items?.[0];
        if (first) packThumb(it.p, hd, first).then((b) => { n.querySelector('.cv').innerHTML = `<img src="${b.url}" alt="" draggable="false">`; }).catch(() => {});
      }
      n.addEventListener('click', () => {
        if (it.add) { openMyPacks(kind); return; }
        const t = secsEl.querySelector(`.tgp-sec[data-s="${i}"]`);
        if (t) { current = i; paintSel(); scroll.scrollTo({ top: t.offsetTop + qEl.offsetHeight - 2, behavior: 'smooth' }); }
      });
    });
    paintSel();
  }
  function paintSel() {
    if (!strip) return;
    sel.style.transform = `translateX(${current * STRIP_ITEM + 1.5}px)`;
    stripIn.querySelectorAll('.tge-tab').forEach((n, i) => { const ic = n.querySelector('.ic'); if (ic) ic.style.color = i === current ? '#fff' : '#9A9AA0'; n.style.opacity = i === current ? '1' : '0.8'; });
  }
  scroll.addEventListener('scroll', () => {
    if (!strip) return;
    const y = scroll.scrollTop - qEl.offsetHeight + 6;
    let at = current;
    secsEl.querySelectorAll('.tgp-sec').forEach((s) => { if (s.offsetTop <= y + 4) at = +s.dataset.s; });
    if (at !== current) { current = at; paintSel(); }
  }, { passive: true });

  async function load() {
    if (!lib.loaded) await loadLibrary();
    packs = usablePacks(kind); loaded = true;
    paintSecs();
    for (const p of packs) {
      if (hdr.has(p.id)) continue;
      try { hdr.set(p.id, await packHeader(p)); paintSecs(); } catch (_) { /* */ }
    }
  }

  paintQ();
  let started = false;
  return {
    el,
    show() {
      if (!started) { started = true; paintSecs(); load(); return; }
      packs = usablePacks(kind); paintSecs();
      if (packs.some((p) => !hdr.has(p.id))) load();
    },
  };
}
