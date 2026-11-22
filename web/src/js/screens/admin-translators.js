// Admin: tarjimonlar — `ui/admin/AdminContentScreens.kt` (`AdminTranslatorsScreen`, `AdminTranslatorEditScreen`).
// GET /api/translators; POST /api/translators; PUT/DELETE /api/translators/:id. Rasm `translator_<vaqt>.jpg` (vd).

import { api, imageUrl } from '../api.js';
import { esc } from '../format.js';
import { push } from '../router.js';
import { icon, appBar, bindAppBar, bindTap, toast, confirmDialog, spinner, C } from '../ui.js';
import { fileOf, toJpeg, uploadImage } from './admin-util.js';
import { openAdminAnimes } from './admin.js';

export function openAdminTranslators() {
  push((el) => {
    let list = []; let closed = false;
    el.innerHTML = `${appBar({ title: 'Animelarni boshqarish', actions: `<button class="icon-btn ad-add" aria-label="Qo'shish">${icon('add', { size: 26 })}</button>` })}
      <div class="scroll"><div class="ss ad-list"></div></div>`;
    bindAppBar(el);
    const host = el.querySelector('.ad-list');
    const paint = () => {
      if (!list.length) { host.innerHTML = '<div class="ss-note" style="text-align:center;margin-top:48px">Hali tarjimon qo\'shilmagan<br>Yuqoridagi + tugmasi bilan qo\'shing</div>'; return; }
      host.innerHTML = list.map((t, i) => {
        const real = Number(t.id) > 0;
        return `<div class="ad-row" data-i="${i}">
          <div class="ad-ph" style="width:64px;height:64px;border-radius:50%">${t.photo_url ? `<img alt="" loading="lazy" src="${esc(imageUrl(t.photo_url))}" onerror="this.remove()">` : icon('translate', { fill: false, size: 26, color: 'rgba(255,255,255,0.4)' })}</div>
          <div class="ss-main"><div class="ss-t" style="font-size:15px">${esc(t.name)}</div><div class="ss-h" style="font-size:12px">${Number(t.anime_count) || 0} ta anime</div></div>
          ${real ? `<button class="icon-btn ad-edit">${icon('edit', { fill: false, size: 20, color: 'rgba(255,255,255,0.54)' })}</button><button class="icon-btn ad-del">${icon('delete', { fill: false, size: 20, color: '#FF5252' })}</button>` : ''}
        </div>`;
      }).join('');
      host.querySelectorAll('.ad-row').forEach((r) => {
        const t = list[+r.dataset.i];
        r.addEventListener('click', () => openAdminAnimes(t));
        r.querySelector('.ad-edit')?.addEventListener('click', (e) => { e.stopPropagation(); openForm(t, load); });
        r.querySelector('.ad-del')?.addEventListener('click', async (e) => {
          e.stopPropagation();
          const ok = await confirmDialog("Tarjimonni o'chirish?", `"${esc(t.name)}" o'chiriladi. Animelari o'chmaydi, "Tarjimonsiz" bo'lib qoladi.`, { ok: "Ha, o'chirish", cancel: "Yo'q", danger: true });
          if (!ok) return;
          try { await api(`/api/translators/${t.id}`, { method: 'DELETE' }); toast("Tarjimon o'chirildi"); load(); } catch (er) { toast(`Xato: ${er?.body?.error || er?.message || er}`); }
        });
      });
    };
    async function load() {
      if (!list.length) host.innerHTML = `<div style="display:flex;justify-content:center;margin-top:48px">${spinner(30, 2, 'rgba(255,255,255,0.54)')}</div>`;
      try { const r = await api('/api/translators'); if (closed) return; list = Array.isArray(r) ? r : []; paint(); } catch (e) {
        if (closed) return;
        host.innerHTML = `<div class="ss-note" style="text-align:center;margin-top:48px">Tarjimonlar yuklab olib bo'lmadi<br><button class="btn btn-filled ad-retry" style="margin-top:14px">Qayta urinish</button></div>`;
        host.querySelector('.ad-retry').addEventListener('click', load);
      }
    }
    el.querySelector('.ad-add').addEventListener('click', () => openForm(null, load));
    load();
    return { dispose() { closed = true; } };
  });
}

function openForm(tr, onSaved) {
  push((el) => {
    const edit = !!tr;
    let photoName = fileOf(tr?.photo_url); let picked = null; let preview = ''; let busy = false;
    el.innerHTML = `${appBar({ title: edit ? 'Tarjimonni tahrirlash' : "Tarjimon qo'shish" })}
      <div class="scroll"><div class="ss">
        <div class="ad-pick glass"><div class="ad-pv" style="height:180px"></div><div class="ss-note" style="margin:8px 0 0">Logotip tanlash uchun bosing</div></div>
        <input type="file" accept="image/*" hidden class="ad-file">
        <div class="ad-err ss-note" style="color:#ff6b6b;display:none"></div>
        <input class="field ad-n" placeholder="Tarjimon nomi" value="${esc(tr?.name ?? '')}" style="margin-top:12px">
        <textarea class="field ad-b" rows="6" placeholder="Tarjimon haqida" style="margin-top:12px">${esc(tr?.bio ?? '')}</textarea>
        <button class="btn btn-filled ad-save" style="width:100%;height:48px;margin-top:20px">Saqlash</button>
      </div></div>`;
    bindAppBar(el);
    const $ = (s) => el.querySelector(s);
    const pv = $('.ad-pv');
    const paintPhoto = () => {
      const src = preview || (photoName ? imageUrl(tr?.photo_url) : '');
      pv.innerHTML = src ? `<img alt="" src="${esc(src)}">` : icon('add_photo_alternate', { fill: false, size: 40, color: 'rgba(255,255,255,0.45)' });
    };
    paintPhoto();
    const err = (t) => { const e = $('.ad-err'); e.textContent = t || ''; e.style.display = t ? 'block' : 'none'; };
    bindTap($('.ad-pick'), () => { if (!busy) $('.ad-file').click(); }, { scale: false });
    $('.ad-file').addEventListener('change', async (e) => {
      const f = e.target.files?.[0]; e.target.value = ''; if (!f) return;
      try { picked = await toJpeg(f); if (preview) URL.revokeObjectURL(preview); preview = URL.createObjectURL(picked); err(''); paintPhoto(); } catch (_) { err("Rasmni ochib bo'lmadi"); }
    });
    const save = $('.ad-save');
    save.addEventListener('click', async () => {
      if (busy) return;
      const name = $('.ad-n').value.trim();
      if (!name) { err('Tarjimon nomini kiriting'); return; }
      busy = true; err(''); save.disabled = true; save.textContent = 'Saqlanmoqda…';
      try {
        if (picked) photoName = await uploadImage(picked, 'translator');
        const body = { photo_url: photoName || '', name, bio: $('.ad-b').value.trim() };
        if (edit) await api(`/api/translators/${tr.id}`, { method: 'PUT', body }); else await api('/api/translators', { method: 'POST', body });
        toast(edit ? 'Saqlandi' : "Tarjimon qo'shildi"); onSaved?.(); el.querySelector('.appbar-back').click();
      } catch (e) { err(`Xato: ${e?.body?.error || e?.message || e}`); busy = false; save.disabled = false; save.textContent = 'Saqlash'; }
    });
    return { dispose() { if (preview) URL.revokeObjectURL(preview); } };
  });
}
