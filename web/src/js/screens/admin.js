// Admin paneli — `lib/screens/admin_screen.dart`. Hozircha faqat "Animelarni boshqarish"
// (anime -> bo'limlar -> qismlar: `admin-seasons.js`, `admin-episodes.js`); qolgan bo'limlar keyinroq.
//
// Server so'rovlari ilovadagi bilan BIR XIL: GET/POST /api/anime, PUT/DELETE /api/anime/:id.
// Rasm ilovadagidek Telegram orqali yuklanadi (`anime_<vaqt>.jpg`), bazaga FAQAT fayl nomi yoziladi.

import { api, imageUrl } from '../api.js';
import { esc } from '../format.js';
import { push } from '../router.js';
import { icon, appBar, bindAppBar, bindTap, ripple, toast, confirmDialog, spinner, C } from '../ui.js';
import { fileOf, toJpeg, uploadImage } from './admin-util.js';
import { openAdminSeasons } from './admin-seasons.js';
import { openAdminUsers } from './admin-users.js';
import { openAdminTranslators } from './admin-translators.js';
import { openAdminReports, openAdminChannels, openAdminStats, openAdminEncode, openAdminDb, openAdminApp } from './admin-more.js';

const SECTIONS = [
  ['anime', 'movie_filter', 'Animelarni boshqarish', "Qo'shish, tahrirlash, o'chirish"],
  ['users', 'people_alt', 'Foydalanuvchilar', 'Balans, obuna, bloklash'],
  ['chats', 'chat', 'Suhbatlar', 'Foydalanuvchilar yozishmalari'],
  ['reports', 'flag', 'Shikoyatlar', 'Izohlarga kelgan shikoyatlar'],
  ['channels', 'campaign', 'Majburiy obunalar', 'Kanallar, limit va statistika'],
  ['stats', 'insights', 'Umumiy statistika', "Kontent, foydalanuvchilar, ko'rishlar, trafik"],
  ['encode', 'movie', 'Kodlash navbati', 'Kodlanayotgan va navbatdagi qismlar, log'],
  ['db', 'insights', "Ma'lumotlar bazasi", "Jurnallar, ustunlar va yozuvlarni ko'rish (faqat o'qish)"],
  ['app', 'security', 'Ilova va xavfsizlik', "Eng past versiya, ulanish kaliti"],
];

function adminTile(id, ic, title, sub) {
  return `<div class="ad-tile" data-t="${id}">
    <div class="ad-ic">${icon(ic, { size: 26, color: C.accent2 })}</div>
    <div class="ss-main"><div class="ad-tt">${title}</div><div class="ad-ts">${sub}</div></div>
    ${icon('chevron_right', { size: 24, color: 'rgba(255,255,255,0.38)' })}
  </div>`;
}

export function openAdmin() {
  push((el) => {
    el.innerHTML = `${appBar({ title: 'Admin paneli', actions: icon('admin_panel_settings', { size: 24, color: 'rgba(255,255,255,0.54)' }) })}
      <div class="scroll"><div class="ss ad-hub">${SECTIONS.map((x) => adminTile(...x)).join('')}</div></div>`;
    bindAppBar(el);
    const go = { anime: openAdminTranslators, users: () => openAdminUsers('new'), chats: () => openAdminUsers('chats'), reports: openAdminReports,
      channels: openAdminChannels, stats: openAdminStats, encode: openAdminEncode, db: openAdminDb, app: openAdminApp };
    el.querySelectorAll('.ad-tile').forEach((t) => ripple(t, () => go[t.dataset.t]?.()));
    return {};
  });
}

// ── Animelar ro'yxati ─────────────────────────────────────────────
export function openAdminAnimes(translator = { id: 0, name: 'Animelar' }) {
  const trId = Number(translator.id) || 0;
  push((el) => {
    let list = [];
    let closed = false;
    el.innerHTML = `${appBar({ title: translator.name || 'Animelar', actions: `<button class="icon-btn ad-add" aria-label="Qo'shish">${icon('add', { size: 26 })}</button>` })}
      <div class="scroll"><div class="ss ad-list"></div></div>`;
    bindAppBar(el);
    const host = el.querySelector('.ad-list');

    function paint() {
      if (!list.length) {
        host.innerHTML = '<div class="ss-note" style="text-align:center;margin-top:48px">Hali anime qo\'shilmagan</div>';
        return;
      }
      host.innerHTML = list.map((a, i) => `<div class="glass ad-row" data-i="${i}">
        <div class="ad-ph">${a.photo_url ? `<img alt="" decoding="async" loading="lazy" src="${esc(imageUrl(a.photo_url))}">` : icon('movie', { fill: false, size: 26, color: 'rgba(255,255,255,0.4)' })}</div>
        <div class="ss-main"><div class="ss-t">${esc(a.name || '(nomsiz)')}</div>
          <div class="ss-h">${esc([a.davlat, a.studiya, a.janri].filter(Boolean).join(' · '))}</div></div>
        <button class="icon-btn ad-edit" aria-label="Tahrirlash">${icon('edit', { fill: false, size: 20, color: 'rgba(255,255,255,0.7)' })}</button>
        <button class="icon-btn ad-del" aria-label="O'chirish">${icon('delete', { fill: false, size: 20, color: C.danger })}</button>
      </div>`).join('');
      host.querySelectorAll('.ad-row').forEach((r) => {
        const a = list[+r.dataset.i];
        r.querySelector('.ad-edit').addEventListener('click', (e) => { e.stopPropagation(); openAnimeForm(a, load, trId); });
        r.querySelector('.ad-del').addEventListener('click', (e) => { e.stopPropagation(); remove(a); });
        r.addEventListener('click', () => openAdminSeasons(a));
      });
    }

    async function load() {
      if (!list.length) host.innerHTML = `<div style="display:flex;justify-content:center;margin-top:48px">${spinner(30, 2, 'rgba(255,255,255,0.54)')}</div>`;
      try {
        const rows = await api('/api/anime');
        if (closed) return;
        list = (Array.isArray(rows) ? rows : []).filter((a) => (Number(a.translator_id) || 0) === trId);
        paint();
      } catch (e) {
        if (closed) return;
        host.innerHTML = `<div class="ss-note" style="text-align:center;margin-top:48px">Yuklab bo'lmadi<br><button class="btn btn-filled ad-retry" style="margin-top:14px">Qayta urinish</button></div>`;
        host.querySelector('.ad-retry').addEventListener('click', load);
      }
    }

    async function remove(a) {
      const ok = await confirmDialog("Animeni o'chirish?", `"${esc(a.name || '')}" animeni rostdan o'chirmoqchimisiz? Barcha bo'limlari va qismlari ham o'chadi.`, { ok: "Ha, o'chirish", cancel: "Yo'q", danger: true });
      if (!ok) return;
      try {
        await api(`/api/anime/${a.id}`, { method: 'DELETE' });
        toast("Anime o'chirildi");
        load();
      } catch (e) { toast(`Xato: ${e?.body?.error || e?.message || e}`); }
    }

    el.querySelector('.ad-add').addEventListener('click', () => openAnimeForm(null, load, trId));
    load();
    return { dispose() { closed = true; } };
  });
}

// ── Anime qo'shish / tahrirlash ───────────────────────────────────
function openAnimeForm(anime, onSaved, translatorId = 0) {
  push((el) => {
    const edit = !!anime;
    let photoName = fileOf(anime?.photo_url);
    let picked = null; // Blob
    let preview = '';
    let busy = false;
    let trId = anime ? (Number(anime.translator_id) || 0) : translatorId;
    let trs = [];
    const f = (k) => esc(anime?.[k] ?? '');
    el.innerHTML = `${appBar({ title: edit ? 'Animeni tahrirlash' : "Anime qo'shish" })}
      <div class="scroll"><div class="ss">
        <div class="ad-pick glass"><div class="ad-pv"></div><div class="ss-note" style="margin:8px 0 0">Rasm tanlash uchun bosing</div></div>
        <input type="file" accept="image/*" hidden class="ad-file">
        <div class="ad-err ss-note" style="color:#ff6b6b;display:none"></div>
        <div class="ad-lbl" style="margin-top:14px">Tarjimon</div><div class="ad-seg ad-trs"></div>
        <input class="field ad-n" placeholder="Anime nomi (ixtiyoriy)" value="${f('name')}" style="margin-top:12px">
        <input class="field ad-d" placeholder="Davlat (ixtiyoriy)" value="${f('davlat')}" style="margin-top:12px">
        <input class="field ad-s" placeholder="Studiya (ixtiyoriy)" value="${f('studiya')}" style="margin-top:12px">
        <input class="field ad-j" placeholder="Janri (ixtiyoriy)" value="${f('janri')}" style="margin-top:12px">
        <textarea class="field ad-t" rows="4" placeholder="Tavsif (ixtiyoriy)" style="margin-top:12px">${f('tavsif')}</textarea>
        <button class="btn btn-filled ad-save" style="width:100%;height:48px;margin-top:20px">${edit ? 'Saqlash' : "Qo'shish"}</button>
      </div></div>`;
    bindAppBar(el);
    const $ = (s) => el.querySelector(s);
    const pv = $('.ad-pv');
    const paintPhoto = () => {
      const src = preview || (photoName ? imageUrl(anime?.photo_url) : '');
      pv.innerHTML = src ? `<img alt="" src="${esc(src)}">` : icon('add_photo_alternate', { fill: false, size: 40, color: 'rgba(255,255,255,0.45)' });
    };
    paintPhoto();
    const err = (t) => { const e = $('.ad-err'); e.textContent = t || ''; e.style.display = t ? 'block' : 'none'; };
    bindTap($('.ad-pick'), () => { if (!busy) $('.ad-file').click(); }, { scale: false });
    $('.ad-file').addEventListener('change', async (e) => {
      const file = e.target.files?.[0];
      e.target.value = '';
      if (!file) return;
      try {
        picked = await toJpeg(file);
        if (preview) URL.revokeObjectURL(preview);
        preview = URL.createObjectURL(picked);
        err(''); paintPhoto();
      } catch (_) { err("Rasmni ochib bo'lmadi"); }
    });

    const paintTrs = () => {
      $('.ad-trs').innerHTML = [{ id: 0, name: 'Tarjimonsiz' }, ...trs].map((t) => `<button class="ad-chip${Number(t.id) === trId ? ' on' : ''}" data-id="${t.id}">${esc(t.name)}</button>`).join('');
      $('.ad-trs').querySelectorAll('.ad-chip').forEach((b) => b.addEventListener('click', () => { trId = Number(b.dataset.id); paintTrs(); }));
    };
    paintTrs();
    api('/api/translators').then((r) => { trs = (Array.isArray(r) ? r : []).filter((t) => Number(t.id) > 0); paintTrs(); }).catch(() => {});
    const save = $('.ad-save');
    save.addEventListener('click', async () => {
      if (busy) return;
      busy = true; err('');
      save.disabled = true; save.textContent = picked ? 'Rasm yuklanmoqda…' : 'Saqlanmoqda…';
      try {
        if (picked) {
          photoName = await uploadImage(picked, 'anime');
          save.textContent = 'Saqlanmoqda…';
        }
        const body = {
          photo_url: photoName || '',
          name: $('.ad-n').value, davlat: $('.ad-d').value, studiya: $('.ad-s').value,
          janri: $('.ad-j').value, tavsif: $('.ad-t').value, translator_id: trId,
        };
        if (edit) await api(`/api/anime/${anime.id}`, { method: 'PUT', body });
        else await api('/api/anime', { method: 'POST', body });
        toast(edit ? 'Saqlandi' : "Anime qo'shildi");
        onSaved?.();
        el.querySelector('.appbar-back').click();
      } catch (e) {
        err(`Xato: ${e?.body?.error || e?.message || e}`);
        busy = false; save.disabled = false; save.textContent = edit ? 'Saqlash' : "Qo'shish";
      }
    });
    return { dispose() { if (preview) URL.revokeObjectURL(preview); } };
  });
}
