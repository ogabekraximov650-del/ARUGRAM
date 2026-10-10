// Emoji / GIF / stiker to'plamlari — `my_packs_screen.dart`, `pack_detail_screen.dart`,
// `pack_service.dart` ning onlayn nusxasi.
//
// To'plam = kanaldagi bitta shifrlangan `.arp` fayl (`tool/packs/arupack.py`):
// avval sarlavha (elementlar ro'yxati), keyin kichik rasmlar, keyin elementlar.
// Yozuvlar (yangi to'plam, obuna, rasm qo'shish, o'chirish) faqat `sync.js` navbati orqali.

import { api, currentUser } from '../api.js';
import { push, back as routerBack } from '../router.js';
import { icon, spinner, bindTap, toast, appBar, bindAppBar, confirmDialog, promptDialog, dialog, emptyGlass, haptic } from '../ui.js';
import { esc } from '../format.js';
import { putPack, flushNow } from '../sync.js';
import { ensureTelegram, openFile, uploadFile, isTelegramAuthorized } from '../tg/media.js';

const KINDS = [['sticker', 'Stikerlar'], ['emoji', 'Emojilar'], ['gif', 'GIFlar']];
const SINGLE = { sticker: 'Stiker', emoji: 'Emoji', gif: 'GIF' };
const kindLabel = (k) => (KINDS.find((x) => x[0] === k) || [0, k])[1];
const MAX_ITEM = 5 * 1024 * 1024;

// ── To'plam fayli ────────────────────────────────────────────────
const headers = new Map();
async function header(p) {
  if (!p.file) return null;
  if (headers.has(p.file)) return headers.get(p.file);
  const task = (async () => {
    if (!(await ensureTelegram())) throw new Error('tg_login_cancelled');
    const f = await openFile(p.file);
    const pre = await f.read(0, 16);
    const dv = new DataView(pre.buffer, pre.byteOffset, 16);
    if (String.fromCharCode(...pre.slice(0, 4)) !== 'ARUP') throw new Error('bad_pack');
    const hl = dv.getUint32(8, false); // big-endian (arupack.py: ">HHII")
    const raw = await f.read(16, hl);
    const h = JSON.parse(new TextDecoder().decode(raw));
    return { f, h, base: 16 + hl };
  })();
  headers.set(p.file, task);
  task.catch(() => headers.delete(p.file));
  return task;
}

function sniff(b) {
  if (b[0] === 0x89 && b[1] === 0x50) return 'image/png';
  if (b[0] === 0xff && b[1] === 0xd8) return 'image/jpeg';
  if (b[0] === 0x47 && b[1] === 0x49) return 'image/gif';
  if (b[0] === 0x52 && b[1] === 0x49) return 'image/webp';
  if (b[4] === 0x66 && b[5] === 0x74) return 'video/mp4';
  return 'application/octet-stream';
}
const urls = new Map();
async function blobOf(key, reader) {
  if (urls.has(key)) return urls.get(key);
  const task = (async () => {
    const b = await reader();
    const type = sniff(b);
    return { url: URL.createObjectURL(new Blob([b], { type })), type };
  })();
  urls.set(key, task);
  task.catch(() => urls.delete(key));
  return task;
}
const thumbOf = (p, hd, it) => blobOf(`${p.file}#t${it.i}`, () => hd.f.read(hd.base + it.to, it.tl));
const itemOf = (p, hd, it) => blobOf(`${p.file}#i${it.i}`, () => hd.f.read(hd.base + it.o, it.l));

// ── Ro'yxatlar ───────────────────────────────────────────────────
const lib = { mine: [], subs: [], ops: [], loaded: false };
async function loadLibrary() {
  try {
    const j = await api('/api/packs/library');
    lib.mine = j.mine || []; lib.subs = j.subs || []; lib.ops = j.ops || []; lib.loaded = true;
  } catch (_) { /* */ }
}
const newId = () => (Math.floor(Math.random() * 0x7ffffffe) + 1) * 2097152 + Math.floor(Math.random() * 2097152);
function op(key, data) { putPack(key, data); flushNow?.(); }
export const packOp = op;

export { lib, loadLibrary, header as packHeader, thumbOf as packThumb, itemOf as packItem, packInfo };

// ── Xabar ichidagi stiker / GIF (`pk_<to'plam>_<element>`) ─────────
const infos = new Map();
let libTask = null;
async function packInfo(id) {
  if (!lib.loaded) { libTask = libTask || loadLibrary(); await libTask; }
  const f = [...lib.mine, ...lib.subs].find((p) => `${p.id}` === `${id}`);
  if (f) return f;
  if (!infos.has(`${id}`)) {
    const t = api(`/api/packs/info?ids=${id}`).then((j) => (j.packs || [])[0] || null);
    infos.set(`${id}`, t); t.catch(() => infos.delete(`${id}`));
  }
  return infos.get(`${id}`);
}

window.addEventListener('aru-playerplay', () => {
  document.querySelectorAll('.pk-msg video').forEach((x) => {
    x.muted = true;
    const o = x.parentElement?.querySelector('.pk-spk');
    if (o) o.innerHTML = icon('volume_off', { size: 16, color: '#fff' });
  });
});

// Chat/izohlar ro'yxati qayta chizilganda stiker/GIF "o'chib yonmasin": yuklangan natija keshlanadi va
// yangi katakka DARHOL (sinxron) qo'yiladi. Bir xil element uchun yuklash bir marta ketadi.
const mediaCache = new Map(); // kalit -> {th, full, task, waiters:Set}
let soundKey = null; // ovozi yoqilgan element kaliti (qayta chizilganda saqlanadi)

function mediaBox(size, inner) { return `<div class="pk-msg" style="width:${size}px;height:${size}px">${inner}</div>`; }

function paintMedia(el, key, type, size) {
  const c = mediaCache.get(key);
  const phIc = icon(type === 'gif' ? 'gif_box' : 'emoji_emotions', { fill: false, size: 40, color: 'rgba(255,255,255,0.38)' });
  if (!c || (!c.th && !c.full)) { el.innerHTML = mediaBox(size, phIc); return; }
  if (!c.full) { el.innerHTML = mediaBox(size, `<img src="${c.th.url}" alt="" draggable="false">`); return; }
  const vid = c.full.type.startsWith('video');
  el.innerHTML = mediaBox(size, vid ? `<video src="${c.full.url}" autoplay loop ${soundKey === key ? '' : 'muted'} playsinline></video>` : `<img src="${c.full.url}" alt="" draggable="false">`);
  if (!vid) return;
  const box = el.querySelector('.pk-msg');
  const v = box.querySelector('video');
  box.insertAdjacentHTML('beforeend', `<div class="pk-spk">${icon(soundKey === key ? 'volume_up' : 'volume_off', { size: 16, color: '#fff' })}</div>`);
  const spk = box.querySelector('.pk-spk');
  v.play().catch(() => {});
  box.addEventListener('click', (e) => {
    e.stopPropagation();
    const turnOn = soundKey !== key;
    document.querySelectorAll('.pk-msg video').forEach((x) => { x.muted = true; const o = x.parentElement?.querySelector('.pk-spk'); if (o) o.innerHTML = icon('volume_off', { size: 16, color: '#fff' }); });
    soundKey = turnOn ? key : null;
    v.muted = !turnOn;
    spk.innerHTML = icon(turnOn ? 'volume_up' : 'volume_off', { size: 16, color: '#fff' });
    if (turnOn) v.play().catch(() => {});
    window.dispatchEvent(new CustomEvent('aru-packsound', { detail: { on: turnOn } }));
  });
}

window.addEventListener('aru-playerplay', () => {
  soundKey = null;
  document.querySelectorAll('.pk-msg video').forEach((x) => {
    x.muted = true;
    const o = x.parentElement?.querySelector('.pk-spk');
    if (o) o.innerHTML = icon('volume_off', { size: 16, color: '#fff' });
  });
});

/** Xabar/izoh ichiga stiker yoki GIF qo'yadi (`PackMessage`: GIF 200, stiker 150). */
export function renderPackMedia(el, file, type) {
  const m = /^pk_(\d{1,16})_(\d{1,9})$/.exec(file || '');
  const size = type === 'gif' ? 200 : 150;
  const key = `${file}|${type}`;
  let c = mediaCache.get(key);
  paintMedia(el, key, type, size);
  if (!m || (c && c.full) || !isTelegramAuthorized()) return;
  if (!c) {
    c = { th: null, full: null, waiters: new Set() };
    mediaCache.set(key, c);
    c.task = (async () => {
      try {
        const p = await packInfo(m[1]);
        if (!p?.file) throw new Error('no_pack');
        const hd = await header(p);
        const it = (hd.h.items || []).find((x) => `${x.i}` === m[2]);
        if (!it) throw new Error('no_item');
        c.th = await thumbOf(p, hd, it);
        c.waiters.forEach((w) => { if (w.isConnected) paintMedia(w, key, type, size); else c.waiters.delete(w); });
        c.full = await itemOf(p, hd, it);
        c.waiters.forEach((w) => { if (w.isConnected) paintMedia(w, key, type, size); });
      } catch (_) { mediaCache.delete(key); }
    })();
  }
  c.waiters.add(el);
}

// ── Sevimlilar / yaqinda ishlatilganlar (mahalliy) ─────────────
const LS_FAV = 'aru_pk_fav'; const LS_REC = 'aru_pk_rec';
const lsGet = (k) => { try { return JSON.parse(localStorage.getItem(k) || '[]'); } catch (_) { return []; } };
const lsSet = (k, v) => { try { localStorage.setItem(k, JSON.stringify(v)); } catch (_) { /* */ } };
const same = (a, b) => a.kind === b.kind && a.pack === b.pack && a.item === b.item;
export const packFavorites = (kind) => lsGet(LS_FAV).filter((x) => x.kind === kind);
export const packRecents = (kind) => lsGet(LS_REC).filter((x) => x.kind === kind);
export const isPackFavorite = (p) => lsGet(LS_FAV).some((x) => same(x, p));
export function togglePackFavorite(p) {
  const l = lsGet(LS_FAV); const i = l.findIndex((x) => same(x, p));
  if (i >= 0) l.splice(i, 1); else l.unshift({ kind: p.kind, pack: p.pack, item: p.item, emoji: p.emoji || '' });
  lsSet(LS_FAV, l.slice(0, 120));
}
export function notePackRecent(p) {
  const l = lsGet(LS_REC).filter((x) => !same(x, p));
  l.unshift({ kind: p.kind, pack: p.pack, item: p.item, emoji: p.emoji || '' });
  lsSet(LS_REC, l.slice(0, 80));
}
export function usablePacks(kind) {
  const seen = new Set();
  return [...lib.mine, ...lib.subs].filter((p) => p.kind === kind && p.file && !seen.has(p.id) && seen.add(p.id));
}
export const packSingle = (k) => SINGLE[k] || k;

/** Bosib turish tugmasi: qisqa bosish — onTap, uzoq (450 ms) — onLong. */
export function bindPress(el, { onTap, onLong, scale = 0.9 }) {
  let t = 0; let sx = 0; let sy = 0; let live = false; let long = false;
  const clear = () => { clearTimeout(t); el.style.transform = ''; };
  el.addEventListener('pointerdown', (e) => {
    sx = e.clientX; sy = e.clientY; live = true; long = false;
    el.style.transition = 'transform 90ms ease-out'; el.style.transform = `scale(${scale})`;
    t = setTimeout(() => { if (live && onLong) { long = true; live = false; clear(); haptic('light'); onLong(); } }, 450);
  });
  el.addEventListener('pointermove', (e) => { if (live && (Math.abs(e.clientX - sx) > 10 || Math.abs(e.clientY - sy) > 10)) { live = false; clear(); } });
  el.addEventListener('pointerup', () => { const was = live; live = false; clear(); if (was && !long) onTap?.(); });
  el.addEventListener('pointercancel', () => { live = false; clear(); });
  el.addEventListener('contextmenu', (e) => e.preventDefault());
}

/** Katta ko'rinish + menyu (Telegram'dagidek bosib turganda). */
export async function showPackPreview({ pick, aspect = 1, actions = [] }) {
  const app = document.getElementById('app');
  const wrap = document.createElement('div');
  wrap.className = 'pkp-wrap';
  const w = Math.min(window.innerWidth * 0.72, 300);
  const h = Math.min(w / (aspect || 1), window.innerHeight * 0.42);
  wrap.innerHTML = `<div class="pkp"><div class="pkp-img" style="width:${aspect >= 1 ? w : h * aspect}px;height:${aspect >= 1 ? w / aspect : h}px"></div>
    <div class="pkp-menu">${actions.map((a, i) => `<div class="pkp-it${a.danger ? ' danger' : ''}" data-i="${i}">${icon(a.icon, { size: 22, fill: false, color: a.danger ? '#E5484D' : '#fff' })}<span>${esc(a.text)}</span></div>`).join('')}</div></div>`;
  app.appendChild(wrap);
  requestAnimationFrame(() => wrap.classList.add('in'));
  const close = () => { wrap.classList.remove('in'); setTimeout(() => wrap.remove(), 180); };
  wrap.addEventListener('click', (e) => {
    const it = e.target.closest('.pkp-it');
    if (it) { const a = actions[+it.dataset.i]; close(); a?.run?.(); return; }
    if (!e.target.closest('.pkp-img')) close();
  });
  wrap.addEventListener('contextmenu', (e) => e.preventDefault());
  try {
    const p = await packInfo(pick.pack);
    const hd = await header(p);
    const it = (hd.h.items || []).find((x) => x.i === pick.item);
    const full = await itemOf(p, hd, it);
    const box = wrap.querySelector('.pkp-img');
    if (box?.isConnected) box.innerHTML = full.type.startsWith('video') ? `<video src="${full.url}" autoplay loop muted playsinline></video>` : `<img src="${full.url}" alt="" draggable="false">`;
  } catch (_) { /* */ }
}

// ── Qutilar ──────────────────────────────────────────────────────
const fmtSize = (b) => (b <= 0 ? '0 KB' : b < 1048576 ? `${Math.ceil(b / 1024)} KB` : `${(b / 1048576).toFixed(1)} MB`);

function coverHtml(size) { return `<div class="pkh-cover" style="width:${size}px;height:${size}px"></div>`; }
async function fillCover(box, p, size) {
  box.innerHTML = icon(p.kind === 'gif' ? 'gif_box' : 'emoji_emotions', { fill: false, size: 24, color: 'rgba(255,255,255,0.38)' });
  if (!p.file || !isTelegramAuthorized()) return;
  try {
    const hd = await header(p);
    const it = (hd.h.items || [])[0];
    if (!it) return;
    const th = await thumbOf(p, hd, it);
    if (box.isConnected) box.innerHTML = `<img src="${th.url}" alt="" style="width:${size - 12}px;height:${size - 12}px">`;
  } catch (_) { /* */ }
}

function tileHtml(p, { mine, ops = [] } = {}) {
  const waiting = ops.filter((o) => o.op === 'add' && o.state !== 'rejected').length;
  const rejected = ops.filter((o) => o.op === 'add' && o.state === 'rejected').length;
  return `<div class="pkh-tile" data-id="${p.id}">${coverHtml(52)}
    <div class="tx"><div class="n1">${esc(p.title)}</div>
      <div class="n2${rejected ? ' warn' : ''}">${p.items} ta · ${fmtSize(p.bytes)}${waiting ? ` · ${waiting} ta kutmoqda` : ''}${rejected ? ` · ${rejected} ta rad etilgan` : ''}</div></div>
    ${icon('chevron_right', { size: 24, color: 'rgba(255,255,255,0.38)' })}</div>`;
}

// ── "Emoji, GIF va stikerlar" ekrani (`my_packs_screen.dart`) ─────
export function openMyPacks(initialKind = 'sticker') {
  push((el) => {
    let kind = KINDS.some((k) => k[0] === initialKind) ? initialKind : 'sticker';
    let disposed = false;
    el.innerHTML = `${appBar({ title: 'Emoji, GIF va stikerlar' })}
      <div class="pkh-kinds">${KINDS.map(([k, l]) => `<div class="pkh-kind" data-k="${k}">${l}</div>`).join('')}</div>
      <div class="scroll pkh-body"></div>
      <div class="pkh-fab">${icon('add', { size: 24, color: '#fff' })}<span>Yangi to'plam</span></div>`;
    bindAppBar(el);
    const body = el.querySelector('.pkh-body');
    const paint = () => {
      el.querySelectorAll('.pkh-kind').forEach((n) => n.classList.toggle('on', n.dataset.k === kind));
      const uid = currentUser()?.id;
      const mine = lib.mine.filter((p) => p.kind === kind);
      const subs = lib.subs.filter((p) => p.kind === kind);
      const plural = kindLabel(kind).toLowerCase();
      body.innerHTML = `<div class="pkh-in">${!uid ? `<div class="pkh-hint">To'plam yaratish uchun hisobingizga kiring.</div>` : `
        <div class="pkh-sec">Mening to'plamlarim</div>
        ${mine.length ? mine.map((p) => tileHtml(p, { mine: true, ops: lib.ops.filter((o) => o.pack === p.id) })).join('')
    : `<div class="pkh-hint">${!lib.loaded ? 'Yuklanmoqda...' : `Hali to'plam yo'q. "Yangi to'plam" tugmasi bilan o'zingiznikini yarating: rasmlar admin ko'rib chiqqach to'plamga qo'shiladi.`}</div>`}
        <div style="height:14px"></div>
        <div class="pkh-sec">Qo'shilgan to'plamlar</div>
        ${subs.length ? subs.map((p) => tileHtml(p)).join('') : `<div class="pkh-hint">Boshqalarning to'plamlarini pastdagi tugma orqali qo'shing.</div>`}
        <div class="pkh-browse">${icon('explore', { size: 24, color: '#E2620F' })}<span>Ommaviy ${esc(plural)}ni ko'rish</span>${icon('chevron_right', { size: 24, color: 'rgba(255,255,255,0.38)' })}</div>`}
        <div style="height:110px"></div></div>`;
      body.querySelectorAll('.pkh-tile').forEach((n) => {
        const p = [...mine, ...subs].find((x) => `${x.id}` === n.dataset.id);
        if (!p) return;
        fillCover(n.querySelector('.pkh-cover'), p, 52);
        bindTap(n, () => openPackDetail(p, { mine: mine.includes(p) }, refresh), { scale: false });
      });
      const br = body.querySelector('.pkh-browse');
      if (br) bindTap(br, () => openPackBrowse(kind, refresh), { scale: false });
      el.querySelector('.pkh-fab').style.display = uid ? '' : 'none';
    };
    const refresh = async () => { await loadLibrary(); if (!disposed) paint(); };
    el.querySelectorAll('.pkh-kind').forEach((n) => n.addEventListener('click', () => { kind = n.dataset.k; paint(); }));
    el.querySelector('.pkh-fab').addEventListener('click', async () => {
      if (lib.mine.length >= 30) { toast("30 tadan ko'p to'plam yaratib bo'lmaydi"); return; }
      const title = ((await promptDialog(`Yangi ${packSingle(kind).toLowerCase()} to'plami`, { maxLength: 40, ok: 'Yaratish', placeholder: "To'plam nomi" })) || '').trim();
      if (!title) return;
      const id = newId();
      op(`p:new:${id}`, { op: 'new', id, kind, title });
      const np = { id, kind, title, file: '', version: 0, items: 0, bytes: 0, owner_id: currentUser()?.id };
      lib.mine = [np, ...lib.mine];
      paint(); openPackDetail(np, { mine: true }, refresh);
    });
    refresh(); paint();
    return { dispose() { disposed = true; } };
  });
}

// ── Ommaviy to'plamlar (`PackBrowseScreen`) ──────────────────────
export function openPackBrowse(kind, onChange = () => {}) {
  push((el) => {
    let disposed = false; let list = []; let more = true; let loading = false; let before = 0; let failed = false;
    el.innerHTML = `${appBar({ title: `Ommaviy ${kindLabel(kind).toLowerCase()}` })}<div class="scroll pkh-body"></div>`;
    bindAppBar(el);
    const body = el.querySelector('.pkh-body');
    const paint = () => {
      body.innerHTML = `<div class="pkh-in">${list.map((p) => tileHtml(p)).join('')}
        ${loading ? `<div class="center-box" style="height:80px">${spinner(30, 3)}</div>` : ''}
        ${!loading && !list.length ? emptyGlass('folder_open', failed ? "Yuklab bo'lmadi" : "Ommaviy to'plamlar yo'q") : ''}
        <div style="height:40px"></div></div>`;
      body.querySelectorAll('.pkh-tile').forEach((n) => {
        const p = list.find((x) => `${x.id}` === n.dataset.id);
        if (!p) return;
        fillCover(n.querySelector('.pkh-cover'), p, 52);
        bindTap(n, () => openPackDetail(p, { mine: false }, () => { onChange(); }), { scale: false });
      });
    };
    async function next() {
      if (!more || loading) return;
      loading = true; failed = false; paint();
      try {
        const j = await api(`/api/packs/public?kind=${kind}${before ? `&before=${before}` : ''}`);
        const r = j.packs || [];
        list = list.concat(r.filter((p) => !list.some((x) => x.id === p.id)));
        more = r.length >= 30;
        if (r.length) before = r[r.length - 1].created_at || 0;
      } catch (_) { failed = true; more = false; }
      loading = false; if (!disposed) paint();
    }
    body.addEventListener('scroll', () => { if (body.scrollTop + body.clientHeight > body.scrollHeight - 300) next(); }, { passive: true });
    next();
    return { dispose() { disposed = true; } };
  });
}

// ── To'plam ekrani (`pack_detail_screen.dart`) ───────────────────
export function openPackDetail(p, { mine = false } = {}, onChange = () => {}) {
  push((el) => {
    let disposed = false; let sub = !!p.sub || lib.subs.some((x) => x.id === p.id);
    const admin = currentUser()?.is_admin === true || currentUser()?.isAdmin === true;
    const myOps = () => lib.ops.filter((o) => o.pack === p.id && o.op === 'add');
    el.innerHTML = `${appBar({ title: esc(p.title), actions: (mine || admin) ? `<button class="icon-btn pk-del">${icon('delete', { fill: false, size: 24 })}</button>` : '' })}
      <div class="scroll pk-body"></div>
      ${mine ? `<div class="pkh-fab">${icon('add_photo_alternate', { size: 24, color: '#fff' })}<span>Qo'shish</span></div>` : ''}
      <input type="file" accept="image/*,video/mp4,video/webm" hidden class="pk-file">`;
    bindAppBar(el);
    const body = el.querySelector('.pk-body');
    const cols = p.kind === 'gif' ? 3 : 4;

    async function paint() {
      const ops = mine ? myOps() : [];
      const subline = `${packSingle(p.kind)} · ${p.items} ta${p.owner_name ? ` · ${esc(p.owner_name)}` : ''}`;
      const subBar = !mine && p.owner_id ? `<div class="pkd-sub${sub ? ' on' : ''}">${sub ? "Qo'shilgan — olib tashlash" : "To'plamni qo'shish"}</div>` : '';
      const label = (o) => ({ queued: 'Yuborilmoqda...', pending: "Admin ko'rib chiqmoqda", approved: "Tasdiqlandi — to'plamga qo'shilmoqda", rejected: `Rad etildi: ${o.reason || "sabab ko'rsatilmagan"}` }[o.state] || o.state);
      const opsHtml = ops.length ? `<div class="pkd-ops"><div class="hd"><span>Yuborilgan rasmlar</span>${ops.some((o) => o.state === 'rejected') ? `<b class="clr">Rad etilganlarni tozalash</b>` : ''}</div>
        ${ops.map((o) => `<div class="it"><i class="dot ${o.state}"></i><span>${esc(label(o))}</span></div>`).join('')}</div>` : '';
      const head = `<div class="pkd-in"><div class="pkd-line">${subline}</div>${subBar}${opsHtml}`;
      const bindTop = () => {
        body.querySelector('.pkd-sub')?.addEventListener('click', () => {
          sub = !sub; op(`p:sub:${p.id}`, { op: 'sub', pack: p.id, on: sub });
          if (sub) lib.subs = [p, ...lib.subs.filter((x) => x.id !== p.id)]; else lib.subs = lib.subs.filter((x) => x.id !== p.id);
          toast(sub ? "Obuna bo'ldingiz" : 'Obuna bekor qilindi'); onChange(); paint();
        });
        body.querySelector('.clr')?.addEventListener('click', () => {
          ops.filter((o) => o.state === 'rejected').forEach((o) => op(`p:clear:${o.id}`, { op: 'clear', pack: p.id, id: o.id }));
          lib.ops = lib.ops.filter((o) => !(o.pack === p.id && o.state === 'rejected')); paint();
        });
      };
      if (!p.file) {
        body.innerHTML = `${head}<div class="pkh-hint c" style="margin-top:30px">${mine ? `To'plam bo'sh. "Qo'shish" tugmasi bilan rasm yoki video yuboring — admin tasdiqlagach shu yerda ko'rinadi.` : "To'plam hali bo'sh."}</div></div>`;
        bindTop(); return;
      }
      body.innerHTML = `${head}<div class="center-box" style="height:140px">${spinner(34, 3)}</div></div>`; bindTop();
      let hd;
      try { hd = await header(p); } catch (e) {
        if (!disposed) body.innerHTML = `${head}${emptyGlass('error', "To'plamni ochib bo'lmadi", `${e?.message === 'tg_login_cancelled' ? 'Avval Telegram hisobini ulang' : `${e?.message || ''}`}`)}</div>`;
        return;
      }
      if (disposed) return;
      const items = hd.h.items || [];
      body.innerHTML = `${head}<div class="pkd-grid" style="grid-template-columns:repeat(${cols},1fr)">${items.map((it) => `<div class="pkd-cell" data-i="${it.i}"></div>`).join('')}</div><div style="height:110px"></div></div>`;
      bindTop();
      body.querySelectorAll('.pkd-cell').forEach((cell) => {
        const it = items.find((x) => `${x.i}` === cell.dataset.i);
        thumbOf(p, hd, it).then((b) => { if (!disposed) cell.innerHTML = `<img src="${b.url}" alt="" draggable="false">`; }).catch(() => {});
        bindTap(cell, () => openItem(hd, it), { scale: false });
      });
    }

    async function openItem(hd, it) {
      const full = await itemOf(p, hd, it).catch(() => null);
      const act = await dialog({
        cls: 'pk-prev',
        content: (b) => {
          b.innerHTML = `<div class="pk-big">${!full ? spinner(36, 3) : full.type.startsWith('video') ? `<video src="${full.url}" autoplay loop playsinline></video>` : `<img src="${full.url}" alt="" draggable="false">`}</div>${it.e ? `<div class="pk-em">${esc(it.e)}</div>` : ''}`;
        },
        actions: [{ text: 'Yopish', value: 'x' }, ...(mine ? [{ text: "To'plamdan olib tashlash", value: 'rm', danger: true }] : [])],
      });
      if (act === 'rm' && await confirmDialog("Elementni o'chirish", "Bu element to'plamdan olib tashlansinmi?", { ok: 'Olib tashlash', danger: true })) {
        op(`p:rm:${p.id}:${it.i}`, { op: 'remove', pack: p.id, item: it.i }); toast("O'chirish navbatga qo'yildi");
      }
    }

    el.querySelector('.pk-del')?.addEventListener('click', async () => {
      if (!(await confirmDialog("To'plamni o'chirish", "To'plam butunlay o'chirilsinmi? Uni qo'shgan foydalanuvchilarda ham yo'qoladi.", { ok: "O'chirish", danger: true }))) return;
      op(`p:del:${p.id}`, { op: 'delete', pack: p.id });
      lib.mine = lib.mine.filter((x) => x.id !== p.id); onChange(); routerBack();
    });
    const fileEl = el.querySelector('.pk-file');
    el.querySelector('.pkh-fab')?.addEventListener('click', () => fileEl.click());
    fileEl.addEventListener('change', async () => {
      const f = fileEl.files?.[0]; fileEl.value = '';
      if (!f) return;
      if (f.size > MAX_ITEM) { toast(`Fayl 5 MB dan katta (${(f.size / 1048576).toFixed(1)} MB)`); return; }
      const head = new Uint8Array(await f.slice(0, 32).arrayBuffer());
      if (sniff(head) === 'application/octet-stream') { toast("Faqat rasm (PNG, JPG, GIF, WebP) yoki video (MP4, WebM) mumkin"); return; }
      const emoji = Array.from(((await promptDialog('Mos emoji', { placeholder: '😀', maxLength: 8 })) || '').replace(/[\[\]]/g, '')).slice(0, 3).join('');
      const uid = currentUser()?.id || 0;
      const name = `pki_${uid}_${Date.now()}_${Math.floor(Math.random() * 0xffff).toString(16)}.bin`;
      toast('Yuklanmoqda...', 8000);
      try { await uploadFile(f, name); } catch (e) { toast(e?.message === 'tg_not_ready' ? 'Avval Telegram hisobini ulang' : "Yuklab bo'lmadi — qayta urinib ko'ring"); return; }
      op(`p:add:${name}`, { op: 'add', pack: p.id, file: name, emoji, size: f.size });
      lib.ops = [{ id: -Date.now(), pack: p.id, op: 'add', file: name, state: 'pending', reason: '' }, ...lib.ops];
      toast("Yuborildi — admin ko'rib chiqadi"); paint();
    });
    paint();
    return { dispose() { disposed = true; } };
  });
}
