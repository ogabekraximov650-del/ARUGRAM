// Emoji / GIF / stiker to'plamlari — `my_packs_screen.dart`, `pack_detail_screen.dart`,
// `pack_service.dart` ning onlayn nusxasi.
//
// To'plam = kanaldagi bitta shifrlangan `.arp` fayl (`tool/packs/arupack.py`):
// avval sarlavha (elementlar ro'yxati), keyin kichik rasmlar, keyin elementlar.
// Yozuvlar (yangi to'plam, obuna, rasm qo'shish, o'chirish) faqat `sync.js` navbati orqali.

import { api, currentUser } from '../api.js';
import { push, back as routerBack } from '../router.js';
import { icon, spinner, bindTap, toast, appBar, bindAppBar, confirmDialog, promptDialog, dialog, emptyGlass } from '../ui.js';
import { esc } from '../format.js';
import { putPack, flushNow } from '../sync.js';
import { ensureTelegram, openFile, uploadFile, isTelegramAuthorized } from '../tg/media.js';

const KINDS = [['sticker', 'Stikerlar'], ['emoji', 'Emojilar'], ['gif', 'GIFlar']];
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

export { lib, loadLibrary, header as packHeader, thumbOf as packThumb, itemOf as packItem };

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

/** Xabar/izoh ichiga stiker yoki GIF qo'yadi (`PackMessage`: GIF 200, stiker 150). */
export async function renderPackMedia(el, file, type) {
  const m = /^pk_(\d{1,16})_(\d{1,9})$/.exec(file || '');
  const size = type === 'gif' ? 200 : 150;
  const ph = (ic) => `<div class="pk-msg" style="width:${size}px;height:${size}px">${ic ? icon(type === 'gif' ? 'gif_box' : 'emoji_emotions', { fill: false, size: 40, color: 'rgba(255,255,255,0.38)' }) : ''}</div>`;
  el.innerHTML = ph(true);
  if (!m || !isTelegramAuthorized()) return;
  try {
    const p = await packInfo(m[1]);
    if (!p?.file) return;
    const hd = await header(p);
    const it = (hd.h.items || []).find((x) => `${x.i}` === m[2]);
    if (!it || !el.isConnected) return;
    const th = await thumbOf(p, hd, it);
    if (el.isConnected) el.innerHTML = `<div class="pk-msg" style="width:${size}px;height:${size}px"><img src="${th.url}" alt=""></div>`;
    const full = await itemOf(p, hd, it);
    if (!el.isConnected) return;
    const vid = full.type.startsWith('video');
    el.innerHTML = `<div class="pk-msg" style="width:${size}px;height:${size}px">${vid ? `<video src="${full.url}" autoplay loop muted playsinline></video>` : `<img src="${full.url}" alt="" draggable="false">`}</div>`;
    // Video/GIF ustiga bosilsa ovozi yoqiladi (ilovadagi PackSoundHub: bir vaqtda bittasida).
    if (vid) {
      const v = el.querySelector('video');
      el.querySelector('.pk-msg').addEventListener('click', (e) => {
        e.stopPropagation();
        const on = v.muted;
        document.querySelectorAll('.pk-msg video').forEach((x) => { x.muted = true; });
        v.muted = !on ? true : false;
        if (!v.muted) v.play().catch(() => {});
      });
    }
  } catch (_) { /* belgi qoladi */ }
}

function packRow(p, { sub, owner } = {}) {
  return `<div class="pk-row" data-id="${p.id}">
    <div class="pk-ic">${icon(p.kind === 'gif' ? 'gif_box' : p.kind === 'emoji' ? 'mood' : 'sticky_note_2', { size: 26, color: '#E2620F' })}</div>
    <div class="pk-tx"><div class="n1">${esc(p.title)}</div>
      <div class="n2">${kindLabel(p.kind)} · ${p.items} ta${owner && p.owner_name ? ` · ${esc(p.owner_name)}` : ''}</div></div>
    ${sub ? icon('check_circle', { size: 20, color: '#4ADE80' }) : ''}${icon('chevron_right', { size: 22, color: 'rgba(255,255,255,0.4)' })}</div>`;
}

export function openMyPacks() {
  push((el) => {
    let page = 0; let pub = null; let disposed = false;
    el.innerHTML = `${appBar({ title: "To'plamlarim", actions: `<button class="icon-btn pk-new">${icon('add', { size: 24 })}</button>` })}
      <div class="pk-tabs">${['Mening', 'Obunalar', 'Ommaviy'].map((t, i) => `<div class="pk-tab" data-i="${i}">${t}</div>`).join('')}</div>
      <div class="scroll pk-body"></div>`;
    bindAppBar(el);
    const body = el.querySelector('.pk-body');
    const paint = () => {
      el.querySelectorAll('.pk-tab').forEach((t) => t.classList.toggle('on', +t.dataset.i === page));
      const list = page === 0 ? lib.mine : page === 1 ? lib.subs : pub;
      if (list == null || (page < 2 && !lib.loaded)) { body.innerHTML = `<div class="center-box">${spinner(36, 3)}</div>`; return; }
      if (!list.length) { body.innerHTML = emptyGlass('folder_open', page === 0 ? "Sizda hali to'plam yo'q" : page === 1 ? "Obuna bo'lgan to'plamlar yo'q" : "Ommaviy to'plamlar yo'q", page === 0 ? "Yuqoridagi + tugmasi bilan yangisini yarating" : ''); return; }
      body.innerHTML = `<div class="pk-list">${list.map((p) => packRow(p, { sub: page === 2 && p.sub, owner: page === 2 })).join('')}</div><div class="bottom-space"></div>`;
      body.querySelectorAll('.pk-row').forEach((n) => bindTap(n, () => {
        const p = list.find((x) => `${x.id}` === n.dataset.id);
        if (p) openPackDetail(p, { mine: page === 0 }, () => refresh());
      }));
    };
    const refresh = async () => { await loadLibrary(); if (page === 2) pub = (await api('/api/packs/public').catch(() => ({ packs: [] }))).packs; if (!disposed) paint(); };
    el.querySelectorAll('.pk-tab').forEach((t) => t.addEventListener('click', async () => {
      page = +t.dataset.i; paint();
      if (page === 2 && pub == null) { pub = (await api('/api/packs/public').catch(() => ({ packs: [] }))).packs; if (!disposed) paint(); }
    }));
    el.querySelector('.pk-new').addEventListener('click', async () => {
      let kind = 'sticker';
      const ok = await dialog({
        title: "Yangi to'plam",
        content: (b) => {
          b.innerHTML = `<div class="pk-kinds">${KINDS.map(([k, l]) => `<div class="pk-kind${k === kind ? ' on' : ''}" data-k="${k}">${l}</div>`).join('')}</div>`;
          b.querySelectorAll('.pk-kind').forEach((n) => n.addEventListener('click', () => { kind = n.dataset.k; b.querySelectorAll('.pk-kind').forEach((m) => m.classList.toggle('on', m === n)); }));
        },
        actions: [{ text: 'Bekor qilish', value: false }, { text: 'Davom etish', value: true, primary: true }],
      });
      if (ok !== true) return;
      const title = ((await promptDialog("To'plam nomi", { maxLength: 40, ok: 'Yaratish' })) || '').trim();
      if (!title) return;
      const id = newId();
      op(`p:new:${id}`, { op: 'new', id, kind, title });
      lib.mine = [{ id, kind, title, file: '', version: 0, items: 0, bytes: 0, owner_id: currentUser()?.id }, ...lib.mine];
      page = 0; paint(); toast("To'plam yaratildi");
    });
    refresh(); paint();
    return { dispose() { disposed = true; } };
  });
}

export function openPackDetail(p, { mine = false } = {}, onChange = () => {}) {
  push((el) => {
    let disposed = false; let sub = !!p.sub || lib.subs.some((x) => x.id === p.id);
    const myOps = () => lib.ops.filter((o) => o.pack === p.id);
    el.innerHTML = `${appBar({ title: esc(p.title), sub: `${kindLabel(p.kind)} · ${p.items} ta`, actions: mine
      ? `<button class="icon-btn pk-add">${icon('add_photo_alternate', { size: 24 })}</button><button class="icon-btn pk-del">${icon('delete', { fill: false, size: 24 })}</button>`
      : `<button class="icon-btn pk-sub"></button>` })}
      <div class="scroll pk-body"></div><input type="file" accept="image/*,video/mp4,video/webm" hidden class="pk-file">`;
    bindAppBar(el);
    const body = el.querySelector('.pk-body');
    const subBtn = el.querySelector('.pk-sub');
    const paintSub = () => { if (subBtn) subBtn.innerHTML = icon(sub ? 'bookmark_added' : 'bookmark_add', { size: 24, color: sub ? '#4ADE80' : '#fff' }); };
    subBtn?.addEventListener('click', () => { sub = !sub; op(`p:sub:${p.id}`, { op: 'sub', pack: p.id, on: sub }); paintSub(); toast(sub ? "Obuna bo'ldingiz" : 'Obuna bekor qilindi'); onChange(); });
    paintSub();

    async function paint() {
      const ops = myOps();
      const opsHtml = mine && ops.length ? `<div class="pk-ops">${ops.map((o) => `<div class="pk-op ${o.state}">${icon(o.state === 'rejected' ? 'cancel' : 'hourglass_top', { size: 16, color: o.state === 'rejected' ? '#E5484D' : '#FFC93C' })}
        <span>${o.state === 'rejected' ? `Rad etildi${o.reason ? `: ${esc(o.reason)}` : ''}` : "Ko'rib chiqilmoqda"}</span>
        ${o.state === 'rejected' ? `<button data-o="${o.id}">Tozalash</button>` : ''}</div>`).join('')}</div>` : '';
      if (!p.file) { body.innerHTML = `${opsHtml}${emptyGlass('image', "Bu to'plamda hali element yo'q", mine ? "Yuqoridagi tugma bilan rasm qo'shing — admin tasdiqlagach paydo bo'ladi" : '')}`; bindOps(); return; }
      body.innerHTML = `${opsHtml}<div class="center-box">${spinner(36, 3)}</div>`; bindOps();
      let hd;
      try { hd = await header(p); } catch (e) {
        if (!disposed) body.innerHTML = `${opsHtml}${emptyGlass('error', "To'plamni ochib bo'lmadi", `${e?.message === 'tg_login_cancelled' ? "Avval Telegram hisobini ulang" : "Internetni tekshirib qayta urinib ko'ring"}`)}`;
        return;
      }
      if (disposed) return;
      const items = hd.h.items || [];
      body.innerHTML = `${opsHtml}<div class="pk-grid pk-${p.kind}">${items.map((it) => `<div class="pk-cell" data-i="${it.i}"><div class="ph"></div></div>`).join('')}</div><div class="bottom-space"></div>`;
      bindOps();
      body.querySelectorAll('.pk-cell').forEach((cell) => {
        const it = items.find((x) => `${x.i}` === cell.dataset.i);
        thumbOf(p, hd, it).then((b) => { if (!disposed) cell.querySelector('.ph').innerHTML = `<img src="${b.url}" alt="">`; }).catch(() => {});
        bindTap(cell, () => preview(p, hd, it, mine));
      });
    }
    function bindOps() {
      body.querySelectorAll('.pk-op button').forEach((b) => b.addEventListener('click', () => {
        const id = +b.dataset.o; op(`p:clear:${id}`, { op: 'clear', pack: p.id, id });
        lib.ops = lib.ops.filter((o) => o.id !== id); paint();
      }));
    }

    async function preview(pk, hd, it, canRemove) {
      const full = await itemOf(pk, hd, it).catch(() => null);
      dialog({
        title: it.e ? esc(it.e) : '',
        cls: 'pk-prev',
        content: (b, close) => {
          b.innerHTML = `<div class="pk-big">${!full ? spinner(36, 3) : full.type.startsWith('video') ? `<video src="${full.url}" autoplay loop muted playsinline></video>` : `<img src="${full.url}" alt="">`}</div>`;
        },
        actions: [{ text: 'Yopish', value: 'x' }, ...(canRemove ? [{ text: "O'chirish", value: 'rm', danger: true }] : [])],
      }).then(async (v) => {
        if (v !== 'rm') return;
        if (await confirmDialog("Elementni o'chirish", "Bu elementni to'plamdan o'chirasizmi?", { ok: "O'chirish", danger: true })) {
          op(`p:rm:${p.id}:${it.i}`, { op: 'remove', pack: p.id, item: it.i }); toast("O'chirish navbatga qo'yildi");
        }
      });
    }

    el.querySelector('.pk-del')?.addEventListener('click', async () => {
      if (!(await confirmDialog("To'plamni o'chirish", `"${esc(p.title)}" to'plamini o'chirasizmi?`, { ok: "O'chirish", danger: true }))) return;
      op(`p:del:${p.id}`, { op: 'delete', pack: p.id });
      lib.mine = lib.mine.filter((x) => x.id !== p.id); onChange(); routerBack();
    });
    const fileEl = el.querySelector('.pk-file');
    el.querySelector('.pk-add')?.addEventListener('click', () => fileEl.click());
    fileEl.addEventListener('change', async () => {
      const f = fileEl.files?.[0]; fileEl.value = '';
      if (!f) return;
      if (f.size > MAX_ITEM) { toast(`Fayl 5 MB dan katta (${(f.size / 1048576).toFixed(1)} MB)`); return; }
      const head = new Uint8Array(await f.slice(0, 32).arrayBuffer());
      if (sniff(head) === 'application/octet-stream') { toast('Faqat rasm (PNG, JPG, GIF, WebP) yoki video (MP4, WebM) mumkin'); return; }
      const emoji = Array.from(((await promptDialog('Mos emoji', { placeholder: '😀', maxLength: 8 })) || '').replace(/[\[\]]/g, '')).slice(0, 3).join('');
      const uid = currentUser()?.id || 0;
      const name = `pki_${uid}_${Date.now()}_${Math.floor(Math.random() * 0xffff).toString(16)}.bin`;
      toast('Yuklanmoqda...', 8000);
      try { await uploadFile(f, name); } catch (e) { toast(e?.message === 'tg_not_ready' ? 'Avval Telegram hisobini ulang' : 'Yuklab bo\'lmadi — qayta urinib ko\'ring'); return; }
      op(`p:add:${name}`, { op: 'add', pack: p.id, file: name, emoji, size: f.size });
      lib.ops = [{ id: -Date.now(), pack: p.id, op: 'add', file: name, state: 'pending', reason: '' }, ...lib.ops];
      toast("Yuborildi — admin ko'rib chiqadi"); paint();
    });
    paint();
    return { dispose() { disposed = true; } };
  });
}
