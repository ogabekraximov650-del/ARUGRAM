// Admin paneli — `lib/screens/admin_screen.dart` va unga ulangan ekranlarning onlayn nusxasi.
//
// Bo'limlar (ilovadagi tartibda):
//   Animelarni boshqarish (`anime_management_screen`, `add_anime_screen`, `season_management_screen`,
//   `add_season_screen`), Foydalanuvchilar (`admin_users_screen`), Shikoyatlar (`admin_reports_screen`),
//   Emoji/GIF/stikerlar (`admin_packs_screen`), Majburiy obunalar (`admin_channels_screen`),
//   Umumiy statistika (`stats.js -> openStats`), Kodlash navbati (`admin_encode_screen`),
//   Ilova va xavfsizlik (`admin_app_screen`).
//
// Ataylab YO'Q: qism qo'shish (video yuklash — kodlash botida), foydalanuvchi qurilmalari/statistikasi
// oynalari, APK imzosi (saytda imzo yo'q), "B2 tozalash" (olib tashlangan).
// Haqiqiy to'siq serverda: har admin so'rovi worker'da qayta tekshiriladi.

import { api, apiPost, currentUser, imageUrl } from '../api.js';
import { push, back as routerBack } from '../router.js';
import {
  icon, spinner, bindTap, toast, appBar, bindAppBar, confirmDialog, promptDialog, dialog, emptyGlass,
} from '../ui.js';
import { esc } from '../format.js';
import { uploadFile, mediaUrl } from '../tg/media.js';
import { openStats } from './stats.js';
import { openPublicProfile } from './public-profile.js';

export const isAdminUser = () => currentUser()?.is_admin === true || currentUser()?.isAdmin === true;

const put = (path, body) => api(path, { method: 'PUT', body });
const del = (path) => api(path, { method: 'DELETE' });
const errText = (e) => `${e?.body?.error || e?.message || 'Xato'}`;

const JANRLAR = ['Bolalar uchun', 'Detektiv', 'Drama', 'Ekshen', 'Etti', 'Fantastika', 'Fantaziya', "G'ayrioddiy", 'Garem', 'Harbiy',
  'Iblislar', 'Ish', "Jang san'ati", 'Jangari', 'Komediya', 'Kosmos', 'Kundalik hayot', 'Maktab', 'Mashinalar', 'Mexa', 'Musiqiy',
  "O'yinlar", "O'zga dunyo", 'Psixologik', "Qo'rqinchli", 'Romantika', 'Sarguzasht', 'Sehr', 'Shafqatsizlik', 'Sir', 'Sport',
  'Super kuch', 'Syodze', 'Syonen', 'Tarixiy', 'Tragediya', 'Triller'];

const fmtDate = (ms) => {
  if (!ms) return '—';
  const d = new Date(ms + 5 * 3600 * 1000); // UTC+5 (Toshkent)
  const p = (n) => `${n}`.padStart(2, '0');
  return `${p(d.getUTCDate())}.${p(d.getUTCMonth() + 1)}.${d.getUTCFullYear()} ${p(d.getUTCHours())}:${p(d.getUTCMinutes())}`;
};
const ago = (ms) => {
  if (!ms) return '';
  const s = Math.max(0, Math.floor((Date.now() - ms) / 1000));
  if (s < 60) return `${s} soniya oldin`;
  if (s < 3600) return `${Math.floor(s / 60)} daqiqa oldin`;
  if (s < 86400) return `${Math.floor(s / 3600)} soat oldin`;
  return `${Math.floor(s / 86400)} kun oldin`;
};

/** Sahifa: sarlavha + aylanadigan tana. `load(body, ctx)` tanani to'ldiradi; `ctx.reload()` qayta yuklaydi. */
function page({ title, actions = '', load, onAction, dispose, poll = 0 }) {
  push((el) => {
    let disposed = false;
    el.innerHTML = `${appBar({ title, actions })}<div class="scroll ad-body"></div>`;
    bindAppBar(el);
    const body = el.querySelector('.ad-body');
    const ctx = {
      el, body, get disposed() { return disposed; },
      reload: async (quiet = false) => {
        if (!quiet) body.innerHTML = `<div class="center-box" style="height:200px">${spinner(34, 3)}</div>`;
        try { await load(body, ctx); } catch (e) {
          if (!disposed) body.innerHTML = emptyGlass('error', "Yuklab bo'lmadi", errText(e));
        }
      },
    };
    el.querySelector('.ad-act')?.addEventListener('click', () => onAction?.(ctx));
    ctx.reload();
    let t = 0;
    if (poll) t = setInterval(() => { if (!document.hidden && !disposed) ctx.reload(true); }, poll);
    return { dispose() { disposed = true; clearInterval(t); dispose?.(); } };
  });
}

/** Matn/tanlov maydonlari bilan dialog. fields: [{k, label, type, value, options, placeholder, rows}] -> qiymatlar yoki null. */
function form(title, fields, { ok = 'Saqlash' } = {}) {
  let wrap;
  return dialog({
    title,
    content: (el) => {
      wrap = el;
      el.innerHTML = fields.map((f) => {
        const l = f.label ? `<div class="ad-fl">${esc(f.label)}</div>` : '';
        if (f.type === 'select') {
          return `${l}<select class="field" data-k="${f.k}">${f.options.map((o) => `<option value="${esc(o)}"${o === f.value ? ' selected' : ''}>${esc(o)}</option>`).join('')}</select>`;
        }
        if (f.type === 'area') {
          return `${l}<textarea class="field" rows="${f.rows || 4}" data-k="${f.k}" placeholder="${esc(f.placeholder || '')}">${esc(f.value ?? '')}</textarea>`;
        }
        return `${l}<input class="field" data-k="${f.k}" type="${f.type || 'text'}" value="${esc(f.value ?? '')}" placeholder="${esc(f.placeholder || '')}">`;
      }).join('');
    },
    actions: [{ text: 'Bekor qilish', value: null }, { text: ok, value: '__ok', primary: true }],
  }).then((v) => {
    if (v !== '__ok') return null;
    const out = {};
    wrap.querySelectorAll('[data-k]').forEach((n) => { out[n.dataset.k] = n.value; });
    return out;
  });
}

function pickFile(accept = 'image/*') {
  return new Promise((res) => {
    const i = document.createElement('input');
    i.type = 'file'; i.accept = accept;
    i.onchange = () => res(i.files?.[0] || null);
    i.click();
  });
}

const tile = (id, ic, label, sub = '', badge = 0) => `<div class="ad-tile" data-t="${id}">
  <div class="ad-ic">${icon(ic, { size: 26, color: '#fff' })}</div>
  <div class="ad-tx"><div class="n1">${label}</div>${sub ? `<div class="n2">${sub}</div>` : ''}</div>
  ${badge > 0 ? `<span class="ad-badge">${badge > 99 ? '99+' : badge}</span>` : ''}
  ${icon('chevron_right', { size: 24, color: 'rgba(255,255,255,0.38)' })}</div>`;

// ══════════════════════════════════════════════════════════════
//  ASOSIY MENYU
// ══════════════════════════════════════════════════════════════

export function openAdmin() {
  page({
    title: 'Admin paneli',
    async load(body) {
      const [badges, reports, users] = await Promise.all([
        api('/api/admin/badges').catch(() => ({})),
        api('/api/admin/reports?page=0').catch(() => ({})),
        api('/api/admin/users?sort=new&page=0').catch(() => ({})),
      ]);
      const nr = reports.total || 0;
      body.innerHTML = `<div class="ad-in">
        ${tile('anime', 'movie_filter', 'Animelarni boshqarish', "Qo'shish, tahrirlash, o'chirish")}
        ${tile('users', 'group', 'Foydalanuvchilar', users.total ? `Jami ${users.total} ta` : '', badges.chat > 0 ? 0 : 0)}
        ${tile('reports', 'flag', 'Shikoyatlar', nr > 0 ? `${nr} ta shikoyat` : "Shikoyat yo'q", 0)}
        ${tile('packs', 'emoji_emotions', 'Emoji, GIF va stikerlar', "Yuklangan rasmlarni ko'rib chiqish")}
        ${tile('channels', 'campaign', 'Majburiy obunalar', 'Kanallar, limit va statistika')}
        ${tile('stats', 'bar_chart', 'Umumiy statistika', "Kontent, foydalanuvchilar, ko'rishlar, trafik")}
        ${tile('encode', 'terminal', 'Kodlash navbati', 'Kodlanayotgan va navbatdagi qismlar, log')}
        ${tile('app', 'shield', 'Ilova va xavfsizlik', 'Eng past versiya')}
        <div style="height:40px"></div></div>`;
      const go = { anime: openAnimeAdmin, users: openUsersAdmin, reports: openReportsAdmin, packs: openPacksAdmin,
        channels: openChannelsAdmin, stats: openStats, encode: openEncodeAdmin, app: openAppAdmin };
      body.querySelectorAll('.ad-tile').forEach((n) => bindTap(n, () => go[n.dataset.t]?.(), { scale: false }));
    },
  });
}

// ══════════════════════════════════════════════════════════════
//  ANIMELARNI BOSHQARISH
// ══════════════════════════════════════════════════════════════

function openAnimeAdmin() {
  page({
    title: 'Animelarni boshqarish',
    actions: `<button class="icon-btn ad-act">${icon('add', { size: 26 })}</button>`,
    onAction: (ctx) => animeForm(null, () => ctx.reload()),
    async load(body, ctx) {
      const list = await api('/api/anime');
      body.innerHTML = `<div class="ad-in">${list.length ? list.map((a) => `<div class="ad-row" data-id="${a.id}">
          <img class="ad-poster" alt="" data-u="${esc(a.photo_url || '')}">
          <div class="ad-tx"><div class="n1">${esc(a.name || `#${a.id}`)}</div><div class="n2">${esc([a.davlat, a.janri].filter(Boolean).join(' · '))}</div></div>
          <button class="ad-mini" data-a="edit">${icon('edit', { size: 20, fill: false })}</button>
          <button class="ad-mini danger" data-a="del">${icon('delete', { size: 20, fill: false })}</button>
          ${icon('chevron_right', { size: 22, color: 'rgba(255,255,255,0.38)' })}</div>`).join('')
    : emptyGlass('movie_filter', "Hali anime qo'shilmagan")}<div style="height:40px"></div></div>`;
      body.querySelectorAll('.ad-poster').forEach((im) => {
        const u = im.dataset.u; if (u) im.src = imageUrl(u);
      });
      body.querySelectorAll('.ad-row').forEach((row) => {
        const a = list.find((x) => `${x.id}` === row.dataset.id);
        bindTap(row, () => openSeasonsAdmin(a), { scale: false });
        row.querySelector('[data-a=edit]').addEventListener('click', (e) => { e.stopPropagation(); animeForm(a, () => ctx.reload()); });
        row.querySelector('[data-a=del]').addEventListener('click', async (e) => {
          e.stopPropagation();
          if (!(await confirmDialog("Animeni o'chirish?", `"${esc(a.name)}" va uning barcha bo'limlari o'chiriladi.`, { ok: "Ha, o'chirish", danger: true }))) return;
          try { await del(`/api/anime/${a.id}`); toast("Anime o'chirildi"); ctx.reload(); } catch (er) { toast(errText(er)); }
        });
      });
    },
  });
}

/** Rasm tanlanib yuklanadi (Telegram orqali, ilovadagidek); bazaga FAQAT fayl nomi yoziladi. */
async function uploadPoster(file, prefix) {
  const name = `${prefix}_${Date.now()}.jpg`;
  await uploadFile(file, name);
  return name;
}

async function animeForm(a, done) {
  let poster = null;
  const v = await form(a ? 'Animeni tahrirlash' : "Anime qo'shish", [
    { k: 'name', label: 'Nomi', value: a?.name },
    { k: 'davlat', label: 'Davlat', value: a?.davlat },
    { k: 'studiya', label: 'Studiya', value: a?.studiya },
    { k: 'janri', label: 'Janri', value: a?.janri },
    { k: 'tavsif', label: 'Tavsif', type: 'area', value: a?.tavsif },
  ]);
  if (!v) return;
  try {
    let photo = a?.photo_url ? `${a.photo_url}`.split('?')[0].split('/').pop() : '';
    if (await confirmDialog('Poster', a ? "Poster almashtirilsinmi?" : "Poster tanlaysizmi?", { ok: 'Tanlash', cancel: 'Yo\'q' })) {
      poster = await pickFile('image/*');
      if (poster) { toast('Poster yuklanmoqda...', 8000); photo = await uploadPoster(poster, 'anime'); }
    }
    const body = { photo_url: photo, ...v };
    if (a) await put(`/api/anime/${a.id}`, body); else await apiPost('/api/anime', body);
    toast('Saqlandi'); done();
  } catch (e) { toast(errText(e)); }
}

function openSeasonsAdmin(anime) {
  page({
    title: anime.name || 'Bo\'limlar',
    actions: `<button class="icon-btn ad-act">${icon('add', { size: 26 })}</button>`,
    onAction: (ctx) => seasonForm(anime, null, () => ctx.reload()),
    async load(body, ctx) {
      const list = await api(`/api/seasons/anime/${anime.id}`);
      body.innerHTML = `<div class="ad-in">${list.length ? list.map((s) => `<div class="ad-row" data-id="${s.season_id}">
          <img class="ad-poster" alt="" data-u="${esc(s.photo_url || '')}">
          <div class="ad-tx"><div class="n1">${esc(s.nomi || `${s.bolim_id}-bo'lim`)}</div>
            <div class="n2">${esc(`${s.bolim_id}-bo'lim · ${s.epizod_count ?? 0} ta qism · ${s.turi || ''} · ${s.holati || ''}`)}</div></div>
          <button class="ad-mini" data-a="edit">${icon('edit', { size: 20, fill: false })}</button>
          <button class="ad-mini danger" data-a="del">${icon('delete', { size: 20, fill: false })}</button></div>`).join('')
    : emptyGlass('folder_open', "Bo'limlar yo'q")}
        <div class="ad-note">Qism qo'shish va video almashtirish kodlash botida ("Ilova uchun" → "Yangi qism qo'shish" yoki "Anibla orqali").</div>
        <div style="height:40px"></div></div>`;
      body.querySelectorAll('.ad-poster').forEach((im) => { if (im.dataset.u) im.src = imageUrl(im.dataset.u); });
      body.querySelectorAll('.ad-row').forEach((row) => {
        const s = list.find((x) => `${x.season_id}` === row.dataset.id);
        row.querySelector('[data-a=edit]').addEventListener('click', () => seasonForm(anime, s, () => ctx.reload()));
        row.querySelector('[data-a=del]').addEventListener('click', async () => {
          if (!(await confirmDialog("Bo'limni o'chirish?", `"${esc(s.nomi || '')}" va uning qismlari o'chiriladi.`, { ok: "Ha, o'chirish", danger: true }))) return;
          try { await del(`/api/seasons/${anime.id}/${s.season_id}`); toast("Bo'lim o'chirildi"); ctx.reload(); } catch (er) { toast(errText(er)); }
        });
      });
    },
  });
}

async function seasonForm(anime, s, done) {
  const sel = new Set(`${s?.janri || ''}`.split(',').map((x) => x.trim()).filter(Boolean));
  const v = await form(s ? "Bo'limni tahrirlash" : "Bo'lim qo'shish", [
    { k: 'nomi', label: 'Nomi', value: s?.nomi },
    { k: 'bolim_id', label: "Bo'lim raqami", type: 'number', value: s?.bolim_id ?? '' },
    { k: 'studio', label: 'Studiya', value: s?.studio },
    { k: 'tarjimon', label: 'Tarjimon', value: s?.tarjimon },
    { k: 'yili', label: 'Yili', value: s?.yili },
    { k: 'janri', label: `Janrlar (vergul bilan). Mavjud: ${JANRLAR.join(', ')}`, type: 'area', rows: 3, value: [...sel].join(', ') },
    { k: 'turi', label: 'Turi', type: 'select', options: ['TV', 'FILM', 'OVA'], value: s?.turi || 'TV' },
    { k: 'holati', label: 'Holati', type: 'select', options: ['Davom etmoqda', 'Tugallangan'], value: s?.holati || 'Davom etmoqda' },
    { k: 'yosh', label: 'Yosh chegarasi', type: 'number', value: s?.yosh || '' },
    { k: 'tavsif', label: 'Tavsif', type: 'area', value: s?.tavsif },
  ]);
  if (!v) return;
  try {
    let photo = s?.photo_url ? `${s.photo_url}`.split('?')[0].split('/').pop() : '';
    if (await confirmDialog('Poster', s ? 'Poster almashtirilsinmi?' : 'Poster tanlaysizmi?', { ok: 'Tanlash', cancel: "Yo'q" })) {
      const f = await pickFile('image/*');
      if (f) { toast('Poster yuklanmoqda...', 8000); photo = await uploadPoster(f, 'season'); }
    }
    const janrlar = v.janri.split(',').map((x) => x.trim()).filter(Boolean);
    const body = {
      anime_id: anime.id, bolim_id: parseInt(v.bolim_id, 10) || 0, photo_url: photo, nomi: v.nomi, studio: v.studio,
      tarjimon: v.tarjimon, yili: v.yili, janrlar, janri: janrlar.join(', '), turi: v.turi, holati: v.holati,
      tavsif: v.tavsif, yosh: parseInt(v.yosh, 10) || 0,
    };
    if (s) await put(`/api/seasons/${anime.id}/${s.season_id}`, body); else await apiPost('/api/seasons', body);
    toast('Saqlandi'); done();
  } catch (e) { toast(errText(e)); }
}

// ══════════════════════════════════════════════════════════════
//  FOYDALANUVCHILAR
// ══════════════════════════════════════════════════════════════

function userRow(u) {
  const name = `${u.first_name || ''} ${u.last_name || ''}`.trim() || u.username || `#${u.id}`;
  const days = u.sub_until > Date.now() ? Math.ceil((u.sub_until - Date.now()) / 86400000) : 0;
  const sub = [`ID ${u.id}`, u.username ? `@${u.username}` : '', `${(u.balance || 0).toLocaleString('ru-RU')} so'm`,
    days ? `obuna ${days} kun` : '', u.banned ? 'bloklangan' : ''].filter(Boolean).join(' · ');
  return `<div class="ad-row${u.banned ? ' banned' : ''}" data-id="${u.id}">
    <div class="ad-ava">${esc(name.slice(0, 1).toUpperCase())}</div>
    <div class="ad-tx"><div class="n1">${esc(name)}</div><div class="n2">${esc(sub)}</div>
      <div class="n3">Qo'shilgan ${fmtDate(u.created_at)}${u.last_login_at ? ` · kirgan ${ago(u.last_login_at)}` : ''}</div></div>
    ${icon('chevron_right', { size: 22, color: 'rgba(255,255,255,0.38)' })}</div>`;
}

function openUsersAdmin() {
  push((el) => {
    let disposed = false; let sort = 'new'; let q = ''; let pg = 0; let more = false; let loading = false; let list = []; let total = 0;
    el.innerHTML = `${appBar({ title: 'Foydalanuvchilar' })}
      <div class="ad-search"><input class="field" placeholder="Ism, @username yoki ID"></div>
      <div class="pkh-kinds"><div class="pkh-kind on" data-s="new">Yangilar</div><div class="pkh-kind" data-s="online">Oxirgi kirganlar</div></div>
      <div class="scroll ad-body"></div>`;
    bindAppBar(el);
    const body = el.querySelector('.ad-body');
    const paint = () => {
      body.innerHTML = `<div class="ad-in"><div class="ad-note">Jami: ${total}</div>${list.map(userRow).join('')}
        ${loading ? `<div class="center-box" style="height:70px">${spinner(28, 3)}</div>` : ''}
        ${!loading && !list.length ? emptyGlass('person_search', 'Hech kim topilmadi') : ''}
        ${more && !loading ? '<div class="ad-more">Yana yuklash</div>' : ''}<div style="height:40px"></div></div>`;
      body.querySelectorAll('.ad-row').forEach((r) => bindTap(r, () => userSheet(list.find((x) => `${x.id}` === r.dataset.id), (nu) => {
        list = list.map((x) => (x.id === nu.id ? nu : x)); paint();
      }), { scale: false }));
      body.querySelector('.ad-more')?.addEventListener('click', () => load(false));
    };
    async function load(reset) {
      if (loading) return;
      if (reset) { pg = 0; list = []; }
      loading = true; paint();
      try {
        const j = await api(`/api/admin/users?sort=${sort}&q=${encodeURIComponent(q)}&page=${pg}`);
        list = list.concat(j.items || []); total = j.total ?? list.length; more = !!j.has_more; pg += 1;
      } catch (e) { toast(errText(e)); }
      loading = false; if (!disposed) paint();
    }
    let t = 0;
    el.querySelector('.ad-search input').addEventListener('input', (e) => { clearTimeout(t); t = setTimeout(() => { q = e.target.value.trim(); load(true); }, 400); });
    el.querySelectorAll('.pkh-kind').forEach((k) => k.addEventListener('click', () => {
      sort = k.dataset.s; el.querySelectorAll('.pkh-kind').forEach((n) => n.classList.toggle('on', n === k)); load(true);
    }));
    body.addEventListener('scroll', () => { if (more && body.scrollTop + body.clientHeight > body.scrollHeight - 300) load(false); }, { passive: true });
    load(true);
    return { dispose() { disposed = true; clearTimeout(t); } };
  });
}

/** Foydalanuvchi ustidagi amallar (`POST /api/admin/user/:id`). */
async function userAction(id, body) {
  return apiPost(`/api/admin/user/${id}`, body);
}

function userSheet(u, onChange) {
  const name = `${u.first_name || ''} ${u.last_name || ''}`.trim() || u.username || `#${u.id}`;
  const days = u.sub_until > Date.now() ? Math.ceil((u.sub_until - Date.now()) / 86400000) : 0;
  const items = [
    ['balance', 'add_card', "Balans qo'shish / yechish"],
    ['set_balance', 'edit', 'Balansni tenglashtirish'],
    ['sub', 'workspace_premium', "Obunaga kun qo'shish / ayirish"],
    ...(days ? [['sub_clear', 'cancel', 'Obunani bekor qilish']] : []),
    u.banned ? ['unban', 'lock_open', 'Blokdan chiqarish'] : ['ban', 'block', 'Bloklash'],
    ['open', 'person', 'Profilni ochish'],
  ];
  dialog({
    title: esc(name),
    content: (b, close) => {
      b.innerHTML = `<div class="ad-note" style="margin-top:0">ID ${u.id}${u.username ? ` · @${esc(u.username)}` : ''} · Telegram ${u.telegram_id || '—'}<br>
        Balans: ${(u.balance || 0).toLocaleString('ru-RU')} so'm · ${days ? `obuna ${days} kun qoldi` : "obuna yo'q"}<br>
        ${u.banned ? `Bloklangan${u.ban_until ? ` (${fmtDate(u.ban_until)} gacha)` : ' (muddatsiz)'}${u.ban_reason ? `: ${esc(u.ban_reason)}` : ''}<br>` : ''}
        Qo'shilgan: ${fmtDate(u.created_at)}</div>
        ${items.map(([k, ic, l]) => `<div class="ad-act-row" data-k="${k}">${icon(ic, { size: 22, fill: false, color: k === 'ban' ? '#E5484D' : '#fff' })}<span>${l}</span></div>`).join('')}`;
      b.querySelectorAll('.ad-act-row').forEach((r) => r.addEventListener('click', () => close(r.dataset.k)));
    },
    actions: [{ text: 'Yopish', value: null }],
  }).then(async (k) => {
    if (!k) return;
    try {
      if (k === 'open') { openPublicProfile(u.id); return; }
      let body = { action: k };
      if (k === 'balance' || k === 'set_balance') {
        const t = await promptDialog(k === 'balance' ? "Summa (manfiy — yechish)" : "Yangi balans", { type: 'number', ok: 'Saqlash' });
        const n = parseInt(t, 10);
        if (t == null || !Number.isFinite(n)) return;
        body.amount = n;
      } else if (k === 'sub') {
        const t = await promptDialog('Necha kun (manfiy — ayirish)', { type: 'number', ok: 'Saqlash' });
        const n = parseInt(t, 10);
        if (t == null || !Number.isFinite(n)) return;
        body.days = n;
      } else if (k === 'ban') {
        const v = await form('Bloklash', [
          { k: 'days', label: 'Necha kun (0 — muddatsiz)', type: 'number', value: 0 },
          { k: 'reason', label: 'Sabab', placeholder: 'Ixtiyoriy' },
        ], { ok: 'Bloklash' });
        if (!v) return;
        body.days = parseInt(v.days, 10) || 0; body.reason = v.reason.slice(0, 300);
      } else if (k === 'sub_clear' && !(await confirmDialog('Obunani bekor qilish', 'Obuna butunlay bekor qilinsinmi?', { ok: 'Ha', danger: true }))) return;
      const r = await userAction(u.id, body);
      toast('Bajarildi');
      // Yangi holatni qayta o'qiymiz (server javobi har amalda har xil).
      const j = await api(`/api/admin/users?q=${u.id}&page=0`).catch(() => null);
      const nu = j?.items?.find((x) => x.id === u.id);
      onChange(nu || { ...u, ...(r?.balance != null ? { balance: r.balance } : {}) });
    } catch (e) { toast(errText(e)); }
  });
}

// ══════════════════════════════════════════════════════════════
//  SHIKOYATLAR
// ══════════════════════════════════════════════════════════════

function openReportsAdmin() {
  push((el) => {
    let disposed = false; let pg = 0; let more = false; let loading = false; let list = []; let total = 0;
    el.innerHTML = `${appBar({ title: 'Shikoyatlar' })}<div class="scroll ad-body"></div>`;
    bindAppBar(el);
    const body = el.querySelector('.ad-body');
    const who = (u) => (u?.first_name || u?.username ? `${esc(u.first_name || '')}${u.username ? ` @${esc(u.username)}` : ''}` : `#${u?.id || '?'}`);
    const paint = () => {
      body.innerHTML = `<div class="ad-in"><div class="ad-note">Jami: ${total}</div>${list.map((r) => `<div class="ad-card" data-id="${esc(r.id)}">
        <div class="ad-card-h"><b>${r.kind === 'comment' ? 'Izoh' : esc(r.kind)}</b><span>${fmtDate(r.created_at)}</span></div>
        <div class="ad-quote">${esc(r.target_body || '(matn yo\'q)')}${r.target_alive ? '' : '<i> — izoh o\'chirilgan</i>'}</div>
        <div class="ad-meta">Yozgan: <a data-u="${r.target_user?.id}">${who(r.target_user)}</a> · Shikoyatchi: <a data-u="${r.reporter?.id}">${who(r.reporter)}</a></div>
        ${r.reason ? `<div class="ad-meta">Sabab: ${esc(r.reason)}</div>` : ''}
        <div class="ad-btns"><button data-a="ban">${icon('block', { size: 18, fill: false })} Bloklash</button>
          <button data-a="del">${icon('delete', { size: 18, fill: false })} Shikoyatni o'chirish</button></div></div>`).join('')}
        ${loading ? `<div class="center-box" style="height:70px">${spinner(28, 3)}</div>` : ''}
        ${!loading && !list.length ? emptyGlass('flag', "Shikoyat yo'q") : ''}
        ${more && !loading ? '<div class="ad-more">Yana yuklash</div>' : ''}<div style="height:40px"></div></div>`;
      body.querySelectorAll('.ad-card').forEach((c) => {
        const r = list.find((x) => x.id === c.dataset.id);
        c.querySelector('[data-a=del]').addEventListener('click', async () => {
          try { await del(`/api/admin/report/${encodeURIComponent(r.id)}`); list = list.filter((x) => x !== r); total -= 1; paint(); } catch (e) { toast(errText(e)); }
        });
        c.querySelector('[data-a=ban]').addEventListener('click', async () => {
          const uid = r.target_user?.id; if (!uid) return;
          const v = await form('Bloklash', [{ k: 'days', label: 'Necha kun (0 — muddatsiz)', type: 'number', value: 0 }, { k: 'reason', label: 'Sabab', value: r.reason || '' }], { ok: 'Bloklash' });
          if (!v) return;
          try { await userAction(uid, { action: 'ban', days: parseInt(v.days, 10) || 0, reason: v.reason.slice(0, 300) }); toast('Bloklandi'); } catch (e) { toast(errText(e)); }
        });
      });
      body.querySelector('.ad-more')?.addEventListener('click', () => load());
    };
    async function load() {
      if (loading) return;
      loading = true; paint();
      try {
        const j = await api(`/api/admin/reports?page=${pg}`);
        list = list.concat(j.items || []); total = j.total ?? list.length; more = !!j.has_more; pg += 1;
      } catch (e) { toast(errText(e)); }
      loading = false; if (!disposed) paint();
    }
    load();
    return { dispose() { disposed = true; } };
  });
}

// ══════════════════════════════════════════════════════════════
//  EMOJI, GIF VA STIKERLAR (tasdiqlash)
// ══════════════════════════════════════════════════════════════

function openPacksAdmin() {
  page({
    title: 'Emoji, GIF va stikerlar',
    async load(body, ctx) {
      const j = await api('/api/packs/admin/pending');
      const ops = j.ops || [];
      body.innerHTML = `<div class="ad-in"><div class="ad-note">Ko'rib chiqilishi kutilayotgan rasmlar: ${ops.length}</div>
        ${ops.map((o) => `<div class="ad-card" data-id="${o.id}">
          <div class="ad-pk"><div class="ad-pk-img" data-f="${esc(o.file)}">${spinner(26, 3)}</div>
            <div class="ad-tx"><div class="n1">${esc(o.title || `#${o.pack}`)}</div>
              <div class="n2">${esc({ sticker: 'Stiker', emoji: 'Emoji', gif: 'GIF' }[o.kind] || o.kind)} · ${(o.size / 1024).toFixed(0)} KB${o.emoji ? ` · ${esc(o.emoji)}` : ''}</div>
              <div class="n3">${esc(o.owner || `#${o.owner_id}`)} · ${ago(o.at)}</div></div></div>
          <div class="ad-btns"><button data-a="ok">${icon('check_circle', { size: 18, fill: false })} Tasdiqlash</button>
            <button data-a="no">${icon('cancel', { size: 18, fill: false })} Rad etish</button>
            <button data-a="pack">${icon('delete', { size: 18, fill: false })} To'plamni o'chirish</button></div></div>`).join('')}
        ${ops.length ? '' : emptyGlass('task_alt', "Ko'rib chiqiladigan rasm yo'q")}<div style="height:40px"></div></div>`;
      body.querySelectorAll('.ad-pk-img').forEach(async (box) => {
        try {
          const u = await mediaUrl(box.dataset.f);
          if (!ctx.disposed) box.innerHTML = /\.(mp4|webm)/i.test(box.dataset.f) ? `<video src="${u}" muted loop autoplay playsinline></video>` : `<img src="${u}" alt="">`;
        } catch (_) { box.innerHTML = icon('broken_image', { size: 28, fill: false, color: 'rgba(255,255,255,0.4)' }); }
      });
      body.querySelectorAll('.ad-card').forEach((c) => {
        const o = ops.find((x) => `${x.id}` === c.dataset.id);
        const review = async (approve) => {
          let reason = '';
          if (!approve) { reason = (await promptDialog('Rad etish sababi', { maxLength: 120, placeholder: 'Ixtiyoriy' })); if (reason == null) return; }
          try { await apiPost('/api/packs/admin/review', { ids: [o.id], approve, reason }); toast(approve ? 'Tasdiqlandi' : 'Rad etildi'); ctx.reload(true); } catch (e) { toast(errText(e)); }
        };
        c.querySelector('[data-a=ok]').addEventListener('click', () => review(true));
        c.querySelector('[data-a=no]').addEventListener('click', () => review(false));
        c.querySelector('[data-a=pack]').addEventListener('click', async () => {
          if (!(await confirmDialog("To'plamni o'chirish", `"${esc(o.title || '')}" to'plami butunlay o'chirilsinmi?`, { ok: "O'chirish", danger: true }))) return;
          try { await apiPost('/api/packs/admin/delete', { pack: o.pack }); toast("To'plam o'chirildi"); ctx.reload(true); } catch (e) { toast(errText(e)); }
        });
      });
    },
  });
}

// ══════════════════════════════════════════════════════════════
//  MAJBURIY OBUNALAR
// ══════════════════════════════════════════════════════════════

function openChannelsAdmin() {
  page({
    title: 'Majburiy obunalar',
    actions: `<button class="icon-btn ad-act">${icon('add', { size: 26 })}</button>`,
    onAction: async (ctx) => {
      const v = await form("Kanal qo'shish", [
        { k: 'kind', label: 'Turi', type: 'select', options: ['public', 'private'], value: 'public' },
        { k: 'input', label: '@username yoki kanal ID', placeholder: '@kanal' },
        { k: 'need', label: 'Limit (nechta kishi qo\'shilishi kerak, 0 — cheksiz)', type: 'number', value: 0 },
      ], { ok: "Qo'shish" });
      if (!v) return;
      try { await apiPost('/api/admin/channels', { op: 'add', kind: v.kind, input: v.input.trim(), need: parseInt(v.need, 10) || 0 }); toast("Kanal qo'shildi"); ctx.reload(true); } catch (e) { toast(errText(e)); }
    },
    async load(body, ctx) {
      const j = await api('/api/admin/channels');
      const items = j.items || [];
      body.innerHTML = `<div class="ad-in"><div class="ad-note">Bot kanalda ADMIN bo'lishi shart. Yopiq kanal uchun kanal ID'si kerak.</div>
        ${items.map((c) => `<div class="ad-card" data-id="${c.id}">
          <div class="ad-card-h"><b>${esc(c.title || c.username || `#${c.chat_id}`)}</b><span>${c.kind === 'private' ? 'yopiq' : 'ochiq'}${c.active ? '' : ' · to\'lgan'}</span></div>
          <div class="ad-meta">${esc(c.url || '')}</div>
          <div class="ad-meta">Qo'shilgan: <b>${c.joined}</b> / ${c.need || '∞'}</div>
          <div class="ad-btns"><button data-a="m">${icon('remove', { size: 18 })} 100</button><button data-a="p">${icon('add', { size: 18 })} 100</button>
            <button data-a="set">${icon('edit', { size: 18, fill: false })} Limit</button><button data-a="del">${icon('delete', { size: 18, fill: false })} O'chirish</button></div></div>`).join('')}
        ${items.length ? '' : emptyGlass('campaign', "Majburiy obuna kanallari yo'q")}<div style="height:40px"></div></div>`;
      body.querySelectorAll('.ad-card').forEach((c) => {
        const id = +c.dataset.id; const it = items.find((x) => x.id === id);
        const op = async (b) => { try { await apiPost('/api/admin/channels', { id, ...b }); ctx.reload(true); } catch (e) { toast(errText(e)); } };
        c.querySelector('[data-a=m]').addEventListener('click', () => op({ op: 'limit', delta: -100 }));
        c.querySelector('[data-a=p]').addEventListener('click', () => op({ op: 'limit', delta: 100 }));
        c.querySelector('[data-a=set]').addEventListener('click', async () => {
          const t = await promptDialog('Yangi limit', { type: 'number', value: `${it.need}` }); const n = parseInt(t, 10);
          if (t != null && Number.isFinite(n)) op({ op: 'set', need: n });
        });
        c.querySelector('[data-a=del]').addEventListener('click', async () => {
          if (await confirmDialog("Kanalni o'chirish", "Majburiy obunalar ro'yxatidan olib tashlansinmi?", { ok: "O'chirish", danger: true })) op({ op: 'del' });
        });
      });
    },
  });
}

// ══════════════════════════════════════════════════════════════
//  KODLASH NAVBATI
// ══════════════════════════════════════════════════════════════

function openEncodeAdmin() {
  const row = (r, st) => `<div class="ad-row"><div class="ad-tx"><div class="n1">${esc(r.anime_name || `#${r.anime_id}`)} · ${r.epizod_number}-qism</div>
    <div class="n2">${esc(r.nomi || '')}${r.done?.length ? ` · tayyor: ${esc(r.done.join(', '))}` : ''}${r.attempts > 1 ? ` · ${r.attempts}-urinish` : ''}</div>
    ${r.error ? `<div class="n3" style="color:#E5484D">${esc(r.error)}</div>` : ''}</div><span class="ad-st ${st}">${st === 'run' ? 'kodlanmoqda' : st === 'err' ? 'xato' : 'navbatda'}</span></div>`;
  page({
    title: 'Kodlash navbati',
    poll: 4000,
    async load(body) {
      const [q, live] = await Promise.all([api('/api/encode/admin'), api('/api/encode/live').catch(() => ({}))]);
      const lines = (live?.status?.lines || []).map((x) => `${x}`).filter((x) => x.trim());
      const running = q.running || []; const queue = q.queue || []; const errors = q.errors || [];
      body.innerHTML = `<div class="ad-in">
        <div class="ad-sec">Hozir kodlanmoqda</div>${running.length ? running.map((r) => row(r, 'run')).join('') : '<div class="ad-note">Hozir hech narsa kodlanmayapti.</div>'}
        <div class="ad-sec">Encode log${live?.status?.updated_at ? ` · ${ago(live.status.updated_at)}` : ''}</div>
        <pre class="ad-log">${lines.length ? esc(lines.join('\n')) : esc(live?.status_error || "Log yo'q")}</pre>
        <div class="ad-sec">Navbatda (${queue.length})</div>${queue.length ? queue.slice(0, 60).map((r) => row(r, 'q')).join('') : '<div class="ad-note">Navbat bo\'sh.</div>'}
        ${errors.length ? `<div class="ad-sec">Xato bilan to'xtagan (${errors.length})</div>${errors.map((r) => row(r, 'err')).join('')}` : ''}
        <div style="height:40px"></div></div>`;
    },
  });
}

// ══════════════════════════════════════════════════════════════
//  ILOVA VA XAVFSIZLIK
// ══════════════════════════════════════════════════════════════

function openAppAdmin() {
  page({
    title: 'Ilova va xavfsizlik',
    async load(body, ctx) {
      const j = await api('/api/admin/app');
      body.innerHTML = `<div class="ad-in">
        <div class="ad-card"><div class="ad-card-h"><b>Eng past versiya</b><span>${esc(j.min_version || 'cheklov yo\'q')}</span></div>
          <div class="ad-meta">Bundan eski ilovalar (APK) ishlamaydi va yangilashni so'raydi. Masalan: 0.0.9 yoki 0.0.9+9. Bo'sh qoldirsangiz cheklov olinadi.</div>
          <div class="ad-btns"><button data-a="ver">${icon('edit', { size: 18, fill: false })} O'zgartirish</button></div></div>
        <div class="ad-card"><div class="ad-card-h"><b>Ulanish kaliti (APK imzosi)</b><span>${j.gate_on ? 'yoqilgan' : "o'chiq"}</span></div>
          <div class="ad-meta">Ishonchli imzolar soni: ${j.sig_count ?? 0}. Imzoni qo'shish yoki tozalash faqat ilovadan (APK imzosi saytda yo'q).</div></div>
        <div style="height:40px"></div></div>`;
      body.querySelector('[data-a=ver]').addEventListener('click', async () => {
        const t = await promptDialog('Eng past versiya', { value: j.min_version || '', placeholder: '0.0.9+9' });
        if (t == null) return;
        try { await apiPost('/api/admin/app', { min_version: t.trim() }); toast('Saqlandi'); ctx.reload(true); } catch (e) { toast(errText(e)); }
      });
    },
  });
}
