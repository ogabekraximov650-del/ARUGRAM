// Qismlar boshqaruvi — `epizod_management_screen.dart`, `add_epizod_screen.dart`.
// Server: GET /api/epizods/:a/:s?all=1, POST /api/epizods, PUT/DELETE /api/epizods/:a/:s/:e,
// DELETE .../:e/:sifat; kodlash: /api/encode/{status,queue,delete} (admin sessiyasi bilan).
// Videolar ilovadagidek Telegram orqali yuklanadi (bot chati -> bot kanalga ko'chiradi), worker orqali bayt o'tmaydi.

import { api, currentUser } from '../api.js';
import { esc } from '../format.js';
import { push } from '../router.js';
import { icon, appBar, bindAppBar, toast, confirmDialog, spinner, C } from '../ui.js';
import * as tgMedia from '../tg/media.js';
import { fileOf, fmtSize } from './admin-util.js';

const QS = ['360p', '480p', '720p', '1080p'];
const INTRO_ROWS = 5; // `lib/services/intro_times.dart` -> kIntroRows

// Intro vaqtlari — yozilgan ko'rinishida ("5:14"); raqam bo'lsa "0:ss"/"m:ss" ga keltiriladi.
function normIntro(raw) {
  const t = `${raw ?? ''}`.trim();
  if (!t) return '';
  if (t.includes(':')) return t;
  const d = t.replace(/[^0-9]/g, '');
  if (!d || !parseInt(d, 10)) return '';
  if (d.length <= 2) return `0:${d.padStart(2, '0')}`;
  return `${d.slice(0, -2)}:${d.slice(-2)}`;
}
function introText(raw) {
  const t = `${raw ?? ''}`.trim();
  if (!t || t === '0') return '';
  if (t.includes(':')) return t;
  const s = parseInt(t, 10);
  return !s || s <= 0 ? '' : `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
}
const errText = (e) => `${e?.body?.error || e?.message || e}`;

export function openAdminEpisodes(anime, season) {
  push((el) => {
    let list = [];
    let closed = false;
    const title = `${season.bolim_id ?? season.season_id ?? ''}-bo'lim`;
    el.innerHTML = `${appBar({ title, sub: esc(anime.name || ''), actions: `<button class="icon-btn ad-add" aria-label="Qo'shish">${icon('add', { size: 26 })}</button>` })}
      <div class="scroll"><div class="ss ad-list"></div></div>`;
    bindAppBar(el);
    const host = el.querySelector('.ad-list');

    function paint() {
      if (!list.length) { host.innerHTML = '<div class="ss-note" style="text-align:center;margin-top:48px">Hali qism qo\'shilmagan</div>'; return; }
      host.innerHTML = list.map((e, i) => {
        const o = `${e.origin_video ?? ''}`;
        const qs = QS.filter((k) => { const u = fileOf(e[`url_${k}`]); return u && u !== o; });
        const sub = [qs.join(', '), o ? 'asl video' : ''].filter(Boolean).join(' · ') || 'Fayl yo\'q';
        return `<div class="glass ad-row" data-i="${i}">
          <div class="ad-num">${esc(e.epizod_number ?? '?')}</div>
          <div class="ss-main"><div class="ss-t">${esc(e.epizod_name || `${e.epizod_number ?? ''}-qism`)}</div><div class="ss-h">${esc(sub)}</div></div>
          <button class="icon-btn ad-post" aria-label="Kanalga joylash">${icon('send', { fill: false, size: 20, color: 'rgba(255,255,255,0.7)' })}</button>
          <button class="icon-btn ad-edit" aria-label="Tahrirlash">${icon('edit', { fill: false, size: 20, color: 'rgba(255,255,255,0.7)' })}</button>
          <button class="icon-btn ad-del" aria-label="O'chirish">${icon('delete', { fill: false, size: 20, color: C.danger })}</button>
        </div>`;
      }).join('');
      host.querySelectorAll('.ad-row').forEach((r) => {
        const e = list[+r.dataset.i];
        r.querySelector('.ad-post').addEventListener('click', () => postToChannel(e));
        r.querySelector('.ad-edit').addEventListener('click', () => openEpisodeForm(anime.id, season.season_id, e, load));
        r.querySelector('.ad-del').addEventListener('click', () => remove(e));
        r.addEventListener('click', (ev) => { if (!ev.target.closest('button')) openEpisodeForm(anime.id, season.season_id, e, load); });
      });
    }

    async function load() {
      if (!list.length) host.innerHTML = `<div style="display:flex;justify-content:center;margin-top:48px">${spinner(30, 2, 'rgba(255,255,255,0.54)')}</div>`;
      try {
        const rows = await api(`/api/epizods/${anime.id}/${season.season_id}?all=1`);
        if (closed) return;
        list = Array.isArray(rows) ? rows : (rows?.items || []);
        // Oxirgi qism eng yuqorida.
        list = list.slice().sort((a, b) => (Number(b.epizod_number) || 0) - (Number(a.epizod_number) || 0));
        paint();
      } catch (_) {
        if (closed) return;
        host.innerHTML = '<div class="ss-note" style="text-align:center;margin-top:48px">Yuklab bo\'lmadi<br><button class="btn btn-filled ad-retry" style="margin-top:14px">Qayta urinish</button></div>';
        host.querySelector('.ad-retry').addEventListener('click', load);
      }
    }

    async function postToChannel(e) {
      const ok = await confirmDialog('Kanalga joylash?', `${esc(e.epizod_number ?? '')}-qism kanalga post qilib joylansinmi?`, { ok: 'Ha, joylash', cancel: "Yo'q" });
      if (!ok) return;
      try {
        await api(`/api/epizods/${anime.id}/${season.season_id}/${e.epizod_id}/announce`, { method: 'POST' });
        toast('Kanalga joylandi');
      } catch (er) { toast(`Xato: ${errText(er)}`); }
    }

    async function remove(e) {
      const ok = await confirmDialog("Qismni o'chirish?", `${esc(e.epizod_number ?? '')}-qismni rostdan o'chirmoqchimisiz?`, { ok: "Ha, o'chirish", cancel: "Yo'q", danger: true });
      if (!ok) return;
      try {
        await api(`/api/epizods/${anime.id}/${season.season_id}/${e.epizod_id}`, { method: 'DELETE' });
        toast("Qism o'chirildi");
        load();
      } catch (er) { toast(`Xato: ${errText(er)}`); }
    }

    el.querySelector('.ad-add').addEventListener('click', () => openEpisodeForm(anime.id, season.season_id, null, load));
    load();
    return { dispose() { closed = true; } };
  });
}

function pickVideo() {
  return new Promise((resolve) => {
    const inp = document.createElement('input');
    inp.type = 'file'; inp.accept = 'video/*,.mkv';
    inp.onchange = () => resolve(inp.files?.[0] || null);
    inp.oncancel = () => resolve(null);
    inp.click();
  });
}
const extOf = (f) => {
  const d = f.name.lastIndexOf('.');
  const e = d > 0 ? f.name.slice(d + 1).toLowerCase() : '';
  return e && e.length <= 5 && /^[a-z0-9]+$/.test(e) ? e : 'mp4';
};

function openEpisodeForm(animeId, seasonId, ep, onSaved) {
  push((el) => {
    const edit = !!ep;
    const me = currentUser()?.id ?? 0;
    let closed = false;
    let busy = false;
    let origin = `${ep?.origin_video ?? ''}` || null;
    let originNew = false;
    let originKey = '';
    let originSize = 0;
    let job = null;
    const qs = QS.map((label) => ({ label, url: null, size: 0, key: '', up: null }));
    const originUp = { up: null };
    if (ep) {
      qs.forEach((q) => {
        const u = fileOf(ep[`url_${q.label}`]);
        if (origin && u === origin) return;
        q.url = u || null; q.size = Number(ep[`size_${q.label}`]) || 0;
      });
    }
    const intro = (i, n) => esc(introText(ep?.[`intro_${i * 2 + n}`]));
    el.innerHTML = `${appBar({ title: edit ? 'Qismni tahrirlash' : "Qism qo'shish" })}
      <div class="scroll"><div class="ss">
        <div class="ad-err ss-note" style="color:#ff6b6b;display:none"></div>
        <input class="field ad-n" type="number" inputmode="numeric" placeholder="Qism raqami" value="${esc(ep?.epizod_number ?? '')}">
        <input class="field ad-nm" placeholder="Qism nomi (ixtiyoriy)" value="${esc(ep?.epizod_name ?? '')}" style="margin-top:12px">

        <div class="ad-lbl">Intro (o'tkazib yuborish)</div>
        <div class="ss-note" style="margin:0 0 8px">Video 5:14 ga kelganda pleyerda "Introni o'tkazish" tugmasi chiqadi.</div>
        ${Array.from({ length: INTRO_ROWS }, (_, i) => `<div class="ad-intro"><input class="field ad-i" data-k="${i * 2 + 1}" placeholder="Boshi (5:14)" value="${intro(i, 1)}"><span>—</span><input class="field ad-i" data-k="${i * 2 + 2}" placeholder="Oxiri (6:40)" value="${intro(i, 2)}"></div>`).join('')}

        <div class="ad-lbl">Asl video (avto-kodlash)</div>
        <div class="glass ad-card ad-origin"></div>

        <div class="ad-lbl">Sifatlar (qo'lda yuklash)</div>
        ${qs.map((q, i) => `<div class="glass ad-card ad-q" data-i="${i}"></div>`).join('')}

        <button class="btn btn-filled ad-save" style="width:100%;height:48px;margin-top:20px">${edit ? 'Saqlash' : "Qo'shish"}</button>
      </div></div>`;
    bindAppBar(el);
    const $ = (s) => el.querySelector(s);
    const err = (t) => { const e = $('.ad-err'); e.textContent = t || ''; e.style.display = t ? 'block' : 'none'; if (t) e.scrollIntoView({ block: 'nearest' }); };
    const uploading = () => !!originUp.up || qs.some((q) => q.up);
    const progText = (u) => `${(u.p * 100).toFixed(1)}% (${fmtSize(u.sent)} / ${fmtSize(u.total)})`;
    const progBar = (u) => `<div class="ad-prog"><div style="width:${(u.p * 100).toFixed(1)}%"></div></div><div class="ss-h">${progText(u)}</div>`;

    async function upload(u, file, name, onDone) {
      if (!(await tgMedia.ensureTelegram())) { toast("Video Telegram orqali yuklanadi: Profil → \"Telegram'ni ulash\""); return; }
      u.up = { p: 0, sent: 0, total: file.size };
      paint();
      try {
        const res = await tgMedia.uploadFile(file, name, {
          onProgress: (sent, total) => { u.up = { p: total > 0 ? sent / total : 0, sent, total: total || file.size }; if (!closed) paintProgress(); },
        });
        u.up = null;
        onDone(res?.name || name, res?.key || '', file.size);
      } catch (e) {
        u.up = null;
        toast(`Yuklanmadi: ${errText(e)}`);
      }
      if (!closed) paint();
    }

    function statusText() {
      if (!job) return originNew ? "Saqlangach kodlash navbatiga qo'yiladi" : '';
      const done = `${job.done ?? ''}`.split(',').filter(Boolean).join(', ');
      switch (job.state) {
        case 'queued': return done ? `Navbatda (tayyor: ${done})` : 'Navbatda';
        case 'running': return done ? `Kodlanmoqda... tayyor: ${done}` : 'Kodlanmoqda...';
        case 'done': return `Tayyor: ${done}`;
        case 'error': return `Xato: ${job.error ?? ''}`;
        default: return '';
      }
    }

    function paintProgress() {
      const o = $('.ad-origin .ad-pg'); if (o && originUp.up) o.innerHTML = progBar(originUp.up);
      qs.forEach((q, i) => { const n = el.querySelector(`.ad-q[data-i="${i}"] .ad-pg`); if (n && q.up) n.innerHTML = progBar(q.up); });
    }

    function paint() {
      const oc = $('.ad-origin');
      const st = statusText();
      oc.innerHTML = `<div class="ss-h" style="margin:0 0 8px">Bitta asl video yuklang — 1080p/720p/480p/360p (manbadan baland emas) H.265 bilan o'zi tayyorlanadi va yoziladi.</div>
        ${originUp.up ? `<div class="ad-pg">${progBar(originUp.up)}</div>` : `${origin ? `<div class="ss-t" style="font-size:13px;word-break:break-all">${esc(origin)}${originSize ? ` · ${fmtSize(originSize)}` : ''}</div>` : ''}
        ${st ? `<div class="ss-h" style="${job?.state === 'error' ? 'color:#ff6b6b' : ''}">${esc(st)}</div>` : ''}`}
        <div class="ad-btns"><button class="btn ad-bt ad-opick"${originUp.up ? ' disabled' : ''}>${origin ? 'Boshqa asl video yuklash' : 'Original video yuklash'}</button>
        ${origin && !originUp.up ? '<button class="btn ad-bt ad-odel">O\'chirish</button>' : ''}
        ${edit && job?.state === 'error' && !originNew ? '<button class="btn ad-bt ad-oretry">Qayta urinish</button>' : ''}</div>`;
      oc.querySelector('.ad-opick').addEventListener('click', async () => {
        const f = await pickVideo(); if (!f) return;
        await upload(originUp, f, `orig_${me}_${animeId}_${seasonId}_${Date.now()}.${extOf(f)}`, (name, key, size) => {
          origin = name; originKey = key; originSize = size; originNew = true; job = null;
          toast('Asl video yuklandi — "Saqlash"dan keyin kodlash navbatiga qo\'yiladi');
        });
      });
      oc.querySelector('.ad-odel')?.addEventListener('click', deleteOrigin);
      oc.querySelector('.ad-oretry')?.addEventListener('click', async () => {
        try { await queueEncode(ep.epizod_id); toast("Qayta navbatga qo'yildi"); loadStatus(); } catch (e) { toast(`Xato: ${errText(e)}`); }
      });
      qs.forEach((q, i) => {
        const c = el.querySelector(`.ad-q[data-i="${i}"]`);
        c.innerHTML = `<div class="ss-t">${q.label}</div>
          ${q.up ? `<div class="ad-pg">${progBar(q.up)}</div>` : `<div class="ss-h" style="word-break:break-all">${q.url ? `${esc(q.url)}${q.size ? ` · ${fmtSize(q.size)}` : ''}` : 'Fayl yuklanmagan'}</div>`}
          <div class="ad-btns"><button class="btn ad-bt ad-qpick"${q.up ? ' disabled' : ''}>${q.url ? 'Almashtirish' : 'Video yuklash'}</button>
          ${q.url && !q.up ? '<button class="btn ad-bt ad-qdel">O\'chirish</button>' : ''}</div>`;
        c.querySelector('.ad-qpick').addEventListener('click', async () => {
          const f = await pickVideo(); if (!f) return;
          await upload(q, f, `ep_${animeId}_${seasonId}_${q.label}_${Date.now()}.${extOf(f)}`, (name, key, size) => {
            q.url = name; q.key = key; q.size = size;
            toast(`${q.label} yuklandi — endi "Saqlash"ni bosing`);
          });
        });
        c.querySelector('.ad-qdel')?.addEventListener('click', () => deleteQuality(q));
      });
    }

    async function deleteOrigin() {
      if (!(await confirmDialog("Asl videoni o'chirish?", "Asl video Telegram'dan ham, bazadan ham o'chiriladi va kodlash navbatdan olinadi. Tayyor bo'lgan sifatlar qoladi. Davom etasizmi?", { ok: "Ha, o'chirish", cancel: "Yo'q", danger: true }))) return;
      try {
        if (edit) {
          await api('/api/encode/delete', { method: 'POST', body: { anime_id: animeId, season_id: seasonId, epizod_id: ep.epizod_id ?? 0, origin } });
        }
        origin = null; originNew = false; originKey = ''; originSize = 0; job = null;
        toast("Asl video o'chirildi");
        paint();
      } catch (e) { toast(`Asl video o'chirilmadi: ${errText(e)}`); }
    }

    async function deleteQuality(q) {
      if (!(await confirmDialog(`${q.label} faylni o'chirish?`, "Fayl bazadan o'chiriladi. Davom etasizmi?", { ok: "Ha, o'chirish", cancel: "Yo'q", danger: true }))) return;
      try {
        if (edit) await api(`/api/epizods/${animeId}/${seasonId}/${ep.epizod_id}/${q.label}`, { method: 'DELETE' });
        q.url = null; q.size = 0; q.key = '';
        paint();
      } catch (e) { toast(`O'chirishda xato: ${errText(e)}`); }
    }

    function queueEncode(epizodId) {
      return api('/api/encode/queue', { method: 'POST', body: { anime_id: animeId, season_id: seasonId, epizod_id: epizodId, origin, key: originKey } });
    }

    async function loadStatus() {
      if (!edit) return;
      try {
        const j = await api(`/api/encode/status?anime_id=${animeId}&season_id=${seasonId}`);
        const it = (j?.items || []).find((x) => `${x.epizod_id}` === `${ep.epizod_id}`);
        if (it && !closed) { job = it; paint(); }
      } catch (_) { /* */ }
    }

    const save = $('.ad-save');
    save.addEventListener('click', async () => {
      if (busy) return;
      if (uploading()) { err('Yuklash tugashini kuting'); return; }
      busy = true; err('');
      save.disabled = true; save.textContent = 'Saqlanmoqda…';
      try {
        const body = {
          anime_id: animeId, season_id: seasonId,
          epizod_number: parseInt($('.ad-n').value, 10) || 0,
          epizod_name: $('.ad-nm').value,
        };
        qs.forEach((q) => { body[`url_${q.label}`] = q.url || ''; body[`size_${q.label}`] = q.size || 0; body[`key_${q.label}`] = q.key || ''; });
        el.querySelectorAll('.ad-i').forEach((i) => { body[`intro_${i.dataset.k}`] = normIntro(i.value); });
        const res = edit
          ? await api(`/api/epizods/${animeId}/${seasonId}/${ep.epizod_id}`, { method: 'PUT', body })
          : await api('/api/epizods', { method: 'POST', body });
        if (originNew && origin) {
          const id = ep?.epizod_id ?? res?.epizod_id;
          try {
            if (id == null) throw new Error('qism raqami olinmadi');
            await queueEncode(id);
            toast("Kodlash navbatiga qo'yildi — GitHub Actions'da \"Avto-kodlash\" ni ishga tushiring", 4000);
          } catch (e) { toast(`Qism saqlandi, lekin kodlash navbatiga qo'yilmadi: ${errText(e)}`, 4000); }
        } else toast(edit ? 'Saqlandi' : "Qism qo'shildi");
        onSaved?.();
        el.querySelector('.appbar-back').click();
      } catch (e) {
        err(`Xato: ${errText(e)}`);
        busy = false; save.disabled = false; save.textContent = edit ? 'Saqlash' : "Qo'shish";
      }
    });

    paint();
    loadStatus();
    return { dispose() { closed = true; } };
  });
}
