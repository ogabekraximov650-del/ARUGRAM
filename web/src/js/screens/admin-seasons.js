// Bo'limlar boshqaruvi — `season_management_screen.dart`, `add_season_screen.dart`.
// Server: GET /api/seasons/anime/:a, POST /api/seasons, PUT/DELETE /api/seasons/:a/:s
// (`season_id` ni server beradi; foydalanuvchi faqat "N-bo'lim" raqamini kiritadi).

import { api, imageUrl } from '../api.js';
import { esc } from '../format.js';
import { push } from '../router.js';
import { icon, appBar, bindAppBar, bindTap, toast, confirmDialog, spinner, C } from '../ui.js';
import { fileOf, toJpeg, uploadImage } from './admin-util.js';
import { openAdminEpisodes } from './admin-episodes.js';

const JANRLAR = ['Bolalar uchun', 'Detektiv', 'Drama', 'Ekshen', 'Etti', 'Fantastika', 'Fantaziya', "G'ayrioddiy", 'Garem', 'Harbiy',
  'Iblislar', 'Ish', "Jang san'ati", 'Jangari', 'Komediya', 'Kosmos', 'Kundalik hayot', 'Maktab', 'Mashinalar', 'Mexa', 'Musiqiy',
  "O'yinlar", "O'zga dunyo", 'Psixologik', "Qo'rqinchli", 'Romantika', 'Sarguzasht', 'Sehr', 'Shafqatsizlik', 'Sir', 'Sport',
  'Super kuch', 'Syodze', 'Syonen', 'Tarixiy', 'Tragediya', 'Triller'];
const TURLAR = ['TV', 'FILM', 'OVA'];
const HOLATLAR = ['Davom etmoqda', 'Tugallangan'];

export function openAdminSeasons(anime) {
  push((el) => {
    let list = [];
    let closed = false;
    el.innerHTML = `${appBar({ title: esc(anime.name || 'Bo\'limlar'), sub: "Bo'limlarni boshqarish", actions: `<button class="icon-btn ad-add" aria-label="Qo'shish">${icon('add', { size: 26 })}</button>` })}
      <div class="scroll"><div class="ss ad-list"></div></div>`;
    bindAppBar(el);
    const host = el.querySelector('.ad-list');

    function paint() {
      if (!list.length) { host.innerHTML = '<div class="ss-note" style="text-align:center;margin-top:48px">Hali bo\'lim qo\'shilmagan</div>'; return; }
      host.innerHTML = list.map((s, i) => `<div class="glass ad-row" data-i="${i}">
        <div class="ad-ph">${s.photo_url ? `<img alt="" decoding="async" loading="lazy" src="${esc(imageUrl(s.photo_url))}">` : icon('movie', { fill: false, size: 26, color: 'rgba(255,255,255,0.4)' })}</div>
        <div class="ss-main"><div class="ss-t">${esc(`${s.bolim_id ?? s.season_id ?? '?'}-bo'lim — ${s.nomi || ''}`)}</div>
          <div class="ss-h">${esc([s.turi, s.holati].filter(Boolean).join(' · '))}</div></div>
        <button class="icon-btn ad-edit" aria-label="Tahrirlash">${icon('edit', { fill: false, size: 20, color: 'rgba(255,255,255,0.7)' })}</button>
        <button class="icon-btn ad-del" aria-label="O'chirish">${icon('delete', { fill: false, size: 20, color: C.danger })}</button>
      </div>`).join('');
      host.querySelectorAll('.ad-row').forEach((r) => {
        const s = list[+r.dataset.i];
        r.querySelector('.ad-edit').addEventListener('click', (e) => { e.stopPropagation(); openSeasonForm(anime.id, s, load); });
        r.querySelector('.ad-del').addEventListener('click', (e) => { e.stopPropagation(); remove(s); });
        r.addEventListener('click', () => openAdminEpisodes(anime, s));
      });
    }

    async function load() {
      if (!list.length) host.innerHTML = `<div style="display:flex;justify-content:center;margin-top:48px">${spinner(30, 2, 'rgba(255,255,255,0.54)')}</div>`;
      try {
        const rows = await api(`/api/seasons/anime/${anime.id}`);
        if (closed) return;
        list = Array.isArray(rows) ? rows : [];
        paint();
      } catch (_) {
        if (closed) return;
        host.innerHTML = '<div class="ss-note" style="text-align:center;margin-top:48px">Yuklab bo\'lmadi<br><button class="btn btn-filled ad-retry" style="margin-top:14px">Qayta urinish</button></div>';
        host.querySelector('.ad-retry').addEventListener('click', load);
      }
    }

    async function remove(s) {
      const ok = await confirmDialog("Bo'limni o'chirish?", `"${esc(s.nomi || '')}" bo'limini rostdan o'chirmoqchimisiz? Barcha qismlari ham o'chadi.`, { ok: "Ha, o'chirish", cancel: "Yo'q", danger: true });
      if (!ok) return;
      try {
        await api(`/api/seasons/${anime.id}/${s.season_id}`, { method: 'DELETE' });
        toast("Bo'lim o'chirildi");
        load();
      } catch (e) { toast(`Xato: ${e?.body?.error || e?.message || e}`); }
    }

    el.querySelector('.ad-add').addEventListener('click', () => openSeasonForm(anime.id, null, load));
    load();
    return { dispose() { closed = true; } };
  });
}

function seg(items, cur, cls) {
  return `<div class="ad-seg ${cls}">${items.map((t) => `<button type="button" class="ad-chip${t === cur ? ' on' : ''}" data-v="${esc(t)}">${esc(t)}</button>`).join('')}</div>`;
}

function openSeasonForm(animeId, season, onSaved) {
  push((el) => {
    const edit = !!season;
    let photoName = fileOf(season?.photo_url);
    let picked = null;
    let preview = '';
    let busy = false;
    const janrs = new Set(`${season?.janri ?? ''}`.split(',').map((x) => x.trim()).filter(Boolean));
    let turi = TURLAR.includes(season?.turi) ? season.turi : TURLAR[0];
    let holati = HOLATLAR.includes(season?.holati) ? season.holati : HOLATLAR[0];
    const v = (k) => esc(season?.[k] ?? '');
    const yosh = Number(season?.yosh) > 0 ? Number(season.yosh) : '';
    el.innerHTML = `${appBar({ title: edit ? "Bo'limni tahrirlash" : "Bo'lim qo'shish" })}
      <div class="scroll"><div class="ss">
        <div class="ad-pick glass"><div class="ad-pv"></div><div class="ss-note" style="margin:8px 0 0">Rasm tanlash uchun bosing (ixtiyoriy)</div></div>
        <input type="file" accept="image/*" hidden class="ad-file">
        <div class="ad-err ss-note" style="color:#ff6b6b;display:none"></div>
        <input class="field ad-b" type="number" inputmode="numeric" placeholder="Bo'lim raqami (N-bo'lim)" value="${v('bolim_id')}" style="margin-top:12px">
        <input class="field ad-n" placeholder="Nomi" value="${v('nomi')}" style="margin-top:12px">
        <input class="field ad-s" placeholder="Studiya" value="${v('studio')}" style="margin-top:12px">
        <input class="field ad-tj" placeholder="Tarjimon" value="${v('tarjimon')}" style="margin-top:12px">
        <input class="field ad-dv" placeholder="Davlat" value="${v('davlat')}" style="margin-top:12px">
        <input class="field ad-rj" placeholder="Rejissyor" value="${v('rejissyor')}" style="margin-top:12px">
        <input class="field ad-y" inputmode="numeric" placeholder="Yili" value="${v('yili')}" style="margin-top:12px">
        <input class="field ad-yo" type="number" inputmode="numeric" placeholder="Yosh chegarasi (masalan 18)" value="${yosh}" style="margin-top:12px">
        <div class="ad-lbl">Janri</div>
        <div class="ad-seg ad-janr">${JANRLAR.map((j) => `<button type="button" class="ad-chip${janrs.has(j) ? ' on' : ''}" data-v="${esc(j)}">${esc(j)}</button>`).join('')}</div>
        <div class="ad-lbl">Turi</div>${seg(TURLAR, turi, 'ad-turi')}
        <div class="ad-lbl">Holat</div>${seg(HOLATLAR, holati, 'ad-holat')}
        <textarea class="field ad-t" rows="4" placeholder="Tavsif" style="margin-top:16px">${v('tavsif')}</textarea>
        <button class="btn btn-filled ad-save" style="width:100%;height:48px;margin-top:20px">${edit ? 'Saqlash' : "Qo'shish"}</button>
      </div></div>`;
    bindAppBar(el);
    const $ = (s) => el.querySelector(s);
    const pv = $('.ad-pv');
    const paintPhoto = () => {
      const src = preview || (photoName ? imageUrl(season?.photo_url) : '');
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
    $('.ad-janr').addEventListener('click', (e) => {
      const b = e.target.closest('.ad-chip'); if (!b) return;
      const j = b.dataset.v;
      if (janrs.has(j)) janrs.delete(j); else janrs.add(j);
      b.classList.toggle('on', janrs.has(j));
    });
    const single = (sel, set) => $(sel).addEventListener('click', (e) => {
      const b = e.target.closest('.ad-chip'); if (!b) return;
      set(b.dataset.v);
      $(sel).querySelectorAll('.ad-chip').forEach((x) => x.classList.toggle('on', x === b));
    });
    single('.ad-turi', (x) => { turi = x; });
    single('.ad-holat', (x) => { holati = x; });

    const save = $('.ad-save');
    save.addEventListener('click', async () => {
      if (busy) return;
      busy = true; err('');
      save.disabled = true; save.textContent = picked ? 'Rasm yuklanmoqda…' : 'Saqlanmoqda…';
      try {
        if (picked) { photoName = await uploadImage(picked, 'season'); save.textContent = 'Saqlanmoqda…'; }
        const jl = [...janrs].sort();
        const body = {
          anime_id: animeId,
          bolim_id: parseInt($('.ad-b').value, 10) || 0,
          photo_url: photoName || '',
          nomi: $('.ad-n').value, studio: $('.ad-s').value, tarjimon: $('.ad-tj').value, yili: $('.ad-y').value,
          davlat: $('.ad-dv').value.trim(), rejissyor: $('.ad-rj').value.trim(),
          janrlar: jl, janri: jl.join(', '), turi, holati, tavsif: $('.ad-t').value,
          yosh: parseInt($('.ad-yo').value, 10) || 0,
        };
        if (edit) await api(`/api/seasons/${animeId}/${season.season_id}`, { method: 'PUT', body });
        else await api('/api/seasons', { method: 'POST', body });
        toast(edit ? 'Saqlandi' : "Bo'lim qo'shildi");
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
