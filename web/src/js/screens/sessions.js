// Kirgan qurilmalar — `lib/screens/sessions_screen.dart`.
//
//   GET    /api/auth/sessions       -> { sessions: [...] }
//   DELETE /api/auth/sessions/:id   -> boshqa qurilmani chiqarish

import { api } from '../api.js';
import { esc, toInt } from '../format.js';
import { push } from '../router.js';
import { C, icon, spinner, appBar, bindAppBar, toast } from '../ui.js';

const two = (n) => String(n).padStart(2, '0');

/** `12:36/01/01/2026` */
function when(ms) {
  const v = toInt(ms);
  if (v === 0) return '—';
  const d = new Date(v);
  return `${two(d.getHours())}:${two(d.getMinutes())}/${two(d.getDate())}/${two(d.getMonth() + 1)}/${d.getFullYear()}`;
}

function row(k, v) {
  return `<div class="se-kv"><span class="k">${k}</span><span class="v">${esc(v || '—')}</span></div>`;
}

function tileHtml(s) {
  const current = s.current === true;
  const device = `${s.device ?? ''}`;
  const platform = `${s.platform ?? ''}`;
  const version = `${s.app_version ?? ''}`;
  return `<div class="glass se-tile">
    <div class="se-head">
      ${icon(current ? 'phone_iphone' : 'devices_other', { size: 20, color: current ? C.success : 'rgba(255,255,255,0.7)' })}
      <span class="se-dev">${esc(device || "Noma'lum qurilma")}</span>
      ${current ? '<span class="se-cur">Shu qurilma</span>'
    : `<button class="icon-btn se-out" title="Hisobdan chiqarish" data-id="${toInt(s.id)}">${icon('logout', { size: 18, color: 'rgba(255,255,255,0.38)' })}</button>`}
    </div>
    <div style="height:10px"></div>
    ${row('Tizim', platform)}
    ${row('Ilova', version ? `v${version}` : '—')}
    ${row('Kirgan', when(s.created_at))}
    ${row('Oxirgi faollik', when(s.last_seen_at))}
  </div>`;
}

export function openSessions() {
  push((el) => {
    el.innerHTML = `${appBar({ title: 'Kirgan qurilmalar' })}<div class="scroll se"></div>`;
    bindAppBar(el);
    const body = el.querySelector('.scroll');
    let dead = false;

    async function load() {
      body.innerHTML = `<div class="se-center">${spinner(36, 2.4, 'rgba(255,255,255,0.54)')}</div>`;
      let items = null;
      try {
        const d = await api('/api/auth/sessions');
        items = Array.isArray(d?.sessions) ? d.sessions : null;
      } catch (_) { items = null; }
      if (dead) return;
      if (!items) {
        body.innerHTML = `<div class="se-center">
          ${icon('wifi_off', { size: 40, color: 'rgba(255,255,255,0.38)' })}
          <div class="se-fail">Ro'yxatni olib bo'lmadi</div>
          <button class="btn btn-text se-retry">Qayta urinish</button></div>`;
        body.querySelector('.se-retry').addEventListener('click', load);
        return;
      }
      body.innerHTML = `<div class="se-list">
        <div class="se-note">${items.length} / 4 qurilma. Chegara to'lganda yangi qurilma kirsa, eng oldin onlayn bo'lgani avtomatik chiqariladi.</div>
        ${items.map(tileHtml).join('')}</div>`;
      body.querySelectorAll('.se-out').forEach((b) => b.addEventListener('click', () => revoke(toInt(b.dataset.id))));
    }

    async function revoke(id) {
      if (id === 0) return;
      let ok = false;
      try { await api(`/api/auth/sessions/${id}`, { method: 'DELETE' }); ok = true; } catch (_) { ok = false; }
      if (dead) return;
      if (ok) await load();
      else toast("Chiqarib bo'lmadi — qaytadan urinib ko'ring");
    }

    load();
    return { dispose() { dead = true; } };
  });
}
