// Profil — `lib/screens/profile_screen.dart` (kirilgan holat).
//
// Mini App'da foydalanuvchi DOIM kirgan: hisob Telegram'dan avtomatik
// keladi (`currentUser()`), shu sabab:
//   * "Telegram orqali kirish" (`_LoginBody`) — YO'Q;
//   * "Accountdan chiqish" — YO'Q: chiqilsa ham keyingi so'rovdayoq
//     Telegram `initData` orqali o'sha hisobga qayta kiriladi, ya'ni
//     tugma hech narsa qilmasdi (ilovadagi "sinxronlash" ham kerak
//     emas — sayt hamma narsani darhol serverga yuboradi).
// Ataylab tashlab ketilganlar (onlayn emas yoki admin):
//   * "Xotira" katagi va Xotira ekrani (faqat telefonga tegishli);
//   * Admin paneli tugmasi;
//   * Trafik toifalari (Videolar/Rasmlar/Ma'lumotlar) — ular faqat
//     telefonda sanaladi; saytda serverdagi jami raqam ko'rsatiladi
//     (ilova ham toifa bo'lmasa shunday ko'rsatadi: "Oldingi hisob").
// Profil rasmi ilovadagidek: fayl Telegram orqali yuklanadi
// (`tg/media.js` -> `uploadFile`), workerga faqat FAYL NOMI aytiladi
// (`POST /api/auth/avatar`).

import { api, ApiError, currentUser, onUser, imageUrl, logout } from '../api.js';
import { esc, formatCount } from '../format.js';
import { C, icon, spinner, bindTap, ripple, toast } from '../ui.js';
import { myStats, formatBytes, formatHours, STAT_KINDS, openStatDetail } from './stats.js';
import { openProfileEdit } from './profile-edit.js';
import { openSettings } from './settings.js';
import { openSessions } from './sessions.js';
import { openAdmin, isAdminUser } from './admin.js';
import { billing, formatLeft } from '../services/billing.js';
import {
  openBilling, openSupport, openMyPacks, openTelegramAccount, isTelegramAuthorized, checkTelegram, uploadFile,
} from './profile-links.js';

const two = (n) => String(n).padStart(2, '0');

/** `21.09.2026 14:30` */
function subUntilText(ms) {
  if (ms <= 0) return '—';
  const d = new Date(ms);
  return `${two(d.getDate())}.${two(d.getMonth() + 1)}.${d.getFullYear()} ${two(d.getHours())}:${two(d.getMinutes())}`;
}

// ── O'qilmagan xabar (`UnreadBadge`, GET /api/chat/unread) ────────

const unread = { count: 0, admin: false, srv: -1, mk: -1, at: 0, busy: false };
async function refreshUnread() {
  if (unread.busy) return;
  unread.busy = true;
  try {
    const j = await api(`/api/chat/unread?u=${unread.srv}&mk=${unread.mk}&at=${unread.at}`);
    if (j) {
      const n = Number(j.unread) || 0;
      unread.srv = n;
      unread.mk = j.mk != null ? Number(j.mk) : -1;
      unread.at = Number(j.at) || 0;
      unread.count = n;
      unread.admin = j.admin === true;
    }
  } catch (_) { /* jim */ }
  unread.busy = false;
}

// ── Yordamchilar ─────────────────────────────────────────────────

function fullName(u) {
  const n = `${u.first_name ?? ''} ${u.last_name ?? ''}`.trim();
  if (n) return n;
  if (u.username) return `@${u.username}`;
  return `Foydalanuvchi ${u.id}`;
}

function initials(u) {
  const a = `${u.first_name ?? ''}`.trim();
  const b = `${u.last_name ?? ''}`.trim();
  const first = (s) => [...s][0] || '';
  if (a && b) return `${first(a)}${first(b)}`.toUpperCase();
  if (a) return first(a).toUpperCase();
  if (u.username) return first(`${u.username}`).toUpperCase();
  return '#';
}

/** Matnni buferga (Telegram WebView'da `clipboard` bo'lmasligi mumkin). */
export async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch (_) {
    try {
      const ta = document.createElement('textarea');
      ta.value = text;
      ta.style.cssText = 'position:fixed;opacity:0;top:0;left:0';
      document.body.appendChild(ta);
      ta.select();
      const ok = document.execCommand('copy');
      ta.remove();
      return ok;
    } catch (_) { return false; }
  }
}

function premiumBadge(compact) {
  return `<span class="pf-prem${compact ? ' compact' : ''}">${icon('workspace_premium', { size: compact ? 11 : 13, color: C.accentTint })}<span>PREMIUM</span></span>`;
}

/** Rasmni eng ko'pi `max` px qilib JPEG'ga aylantiradi (`image_picker` maxWidth/imageQuality). */
async function shrinkImage(file, max, quality) {
  let bmp;
  try {
    bmp = await createImageBitmap(file);
  } catch (_) {
    throw new Error('pick');
  }
  const k = Math.min(1, max / Math.max(bmp.width, bmp.height));
  const w = Math.max(1, Math.round(bmp.width * k));
  const h = Math.max(1, Math.round(bmp.height * k));
  const cv = document.createElement('canvas');
  cv.width = w; cv.height = h;
  cv.getContext('2d').drawImage(bmp, 0, 0, w, h);
  try { bmp.close?.(); } catch (_) { /* */ }
  const blob = await new Promise((res) => cv.toBlob(res, 'image/jpeg', quality));
  if (!blob) throw new Error('pick');
  return blob;
}

// ── Hisobni o'chirish: progress oynasi (`_TaskDialog`) ───────────

function taskDialog({ title, doneTitle, doneText, run }) {
  return new Promise((resolve) => {
    const app = document.getElementById('app');
    const wrap = document.createElement('div');
    wrap.className = 'dlg-wrap';
    wrap.innerHTML = `<div class="dlg pf-task">
      <div class="dlg-title pf-task-t"></div>
      <div class="pf-task-bar"><div></div></div>
      <div class="pf-task-msg"></div>
      <div class="pf-task-pct"></div>
      <div class="dlg-actions"></div>
    </div>`;
    const tEl = wrap.querySelector('.pf-task-t');
    const bar = wrap.querySelector('.pf-task-bar > div');
    const msg = wrap.querySelector('.pf-task-msg');
    const pctEl = wrap.querySelector('.pf-task-pct');
    const acts = wrap.querySelector('.dlg-actions');
    let progress = 0; let step = ''; let error = null; let busy = true;

    function close(v) {
      wrap.classList.remove('in');
      setTimeout(() => wrap.remove(), 180);
      resolve(v);
    }
    function btn(text, color, fn) {
      const b = document.createElement('button');
      b.className = 'btn btn-text';
      b.textContent = text;
      if (color) b.style.color = color;
      b.addEventListener('click', fn);
      acts.appendChild(b);
    }
    function paint() {
      const ok = !busy && error == null;
      tEl.textContent = ok ? doneTitle : title;
      bar.style.width = `${(progress * 100).toFixed(2)}%`;
      bar.style.background = error != null ? '#FFAB40' : C.accent;
      msg.textContent = error ?? (ok ? doneText : step);
      pctEl.style.display = !ok && error == null ? '' : 'none';
      pctEl.textContent = `${Math.round(progress * 100)}%`;
      acts.innerHTML = '';
      if (busy) acts.innerHTML = `<div class="pf-task-spin">${spinner(18, 2, 'rgba(255,255,255,0.38)')}</div>`;
      if (ok) btn('Yopish', C.accent, () => close('done'));
      if (!busy && error != null) {
        btn('Bekor qilish', '', () => close('cancel'));
        btn('Qayta urinish', 'rgba(255,255,255,0.7)', start);
      }
    }
    async function start() {
      busy = true; error = null; progress = 0; step = '';
      paint();
      let err = null;
      try {
        err = await run((s, p) => { step = s; if (p > progress) progress = Math.min(1, Math.max(0, p)); paint(); });
      } catch (e) {
        err = `Kutilmagan xato: ${e}`;
      }
      busy = false; error = err;
      if (err == null) progress = 1;
      paint();
    }
    app.appendChild(wrap);
    requestAnimationFrame(() => wrap.classList.add('in'));
    start();
  });
}

function plainDialog(title, text, cancel, ok) {
  return new Promise((resolve) => {
    const app = document.getElementById('app');
    const wrap = document.createElement('div');
    wrap.className = 'dlg-wrap';
    wrap.innerHTML = `<div class="dlg"><div class="dlg-title pf-dlg-t">${title}</div>
      <div class="pf-dlg-x">${text}</div>
      <div class="dlg-actions"><button class="btn btn-text" data-v="0">${cancel}</button>
      <button class="btn btn-text" data-v="1" style="color:${C.accent}">${ok}</button></div></div>`;
    const close = (v) => { wrap.classList.remove('in'); setTimeout(() => wrap.remove(), 180); resolve(v); };
    wrap.querySelectorAll('button').forEach((b) => b.addEventListener('click', () => close(b.dataset.v === '1')));
    wrap.addEventListener('click', (e) => { if (e.target === wrap) close(false); });
    app.appendChild(wrap);
    requestAnimationFrame(() => wrap.classList.add('in'));
  });
}

async function deleteAccount() {
  const first = await plainDialog("Accountni o'chirish", "Accountingizni butunlay o'chirmoqchimisiz?",
    'Bekor qilish', 'Davom etish');
  if (!first) return;
  const second = await plainDialog('Ishonchingiz komilmi?',
    "Bu amalni orqaga qaytarib bo'lmaydi.<br><br>• Hisobingiz o'chiriladi<br>• Barcha qurilmalardan chiqarilasiz<br>"
    + "• Profil rasmingiz o'chiriladi<br>• Balansingiz yo'qoladi",
    "Yo'q, bekor qilish", "Ha, o'chirilsin");
  if (!second) return;
  const r = await taskDialog({
    title: "Ma'lumotlar tozalanmoqda",
    doneTitle: "Hisob o'chirildi",
    doneText: "Barcha ma'lumotlaringiz tozalandi.\nYaxshi qoling!",
    run: async (onStep) => {
      onStep("Server ma'lumotlari o'chirilmoqda", 0.15);
      try {
        await api('/api/auth/delete-account', { method: 'POST', noSessionRetry: true });
      } catch (e) {
        if (e instanceof ApiError && e.status) {
          const m = `${e.body?.error ?? ''}`;
          return m || `O'chirib bo'lmadi (${e.status})`;
        }
        return "Tarmoq xatosi — qaytadan urinib ko'ring";
      }
      onStep('Tayyor', 0.95);
      try {
        Object.keys(localStorage).filter((k) => k.startsWith('aru_')).forEach((k) => localStorage.removeItem(k));
      } catch (_) { /* */ }
      return null;
    },
  });
  if (r === 'done') {
    // Hisob o'chdi — Mini App yopiladi (qayta ochilsa yangi hisob ochiladi).
    await logout();
    try { window.Telegram?.WebApp?.close(); } catch (_) { /* */ }
  }
}

// ══════════════════════════════════════════════════════════════
//  SAHIFA
// ══════════════════════════════════════════════════════════════

export function createProfile(page) {
  page.innerHTML = '<div class="pf"></div>';
  const root = page.querySelector('.pf');
  let timer = 0;
  let avatarBusy = false;

  /** Rasm tanlash -> 720 px gacha kichraytirish (JPEG 88%) -> Telegram -> worker. */
  function changeAvatar() {
    if (avatarBusy) return;
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = 'image/*';
    input.addEventListener('change', async () => {
      const f = input.files?.[0];
      if (!f) return;
      const u = currentUser();
      if (!u) return;
      avatarBusy = true;
      root.querySelector('.pf-av-busy')?.removeAttribute('hidden');
      let msg;
      try {
        const blob = await shrinkImage(f, 720, 0.88);
        const name = `avatar_${u.id}_${Date.now()}.jpg`;
        try {
          await uploadFile(blob, name);
        } catch (e) {
          throw new Error(`${e?.message}` === 'tg_not_ready' ? 'Telegram hisobi ulanmagan' : "Tarmoq xatosi — qaytadan urinib ko'ring");
        }
        let j;
        try {
          j = await api('/api/auth/avatar', { method: 'POST', body: { file: name } });
        } catch (e) {
          throw new Error(e instanceof ApiError && e.status ? 'Rasm saqlanmadi' : "Tarmoq xatosi — qaytadan urinib ko'ring");
        }
        if (j?.user) Object.assign(u, j.user);
        msg = 'Profil rasmi yangilandi';
      } catch (e) {
        msg = e?.message === 'pick' ? "Galereyani ochib bo'lmadi" : (e?.message || "Tarmoq xatosi — qaytadan urinib ko'ring");
      }
      avatarBusy = false;
      render();
      toast(msg);
    });
    input.click();
  }

  function cardHtml(u) {
    const photo = imageUrl(u.photo_url);
    return `<div class="glass pf-card">
      <div class="pf-card-row">
        <div class="pf-av">
          <div class="pf-av-in"><span class="pf-av-l">${esc(initials(u))}</span>
            ${photo ? `<img alt="" src="${esc(photo)}" onerror="this.remove()">` : ''}</div>
          <div class="pf-av-busy"${avatarBusy ? '' : ' hidden'}>${spinner(26, 2.4, '#fff')}</div>
          <div class="pf-av-prem">${billing.active ? premiumBadge(true) : ''}</div>
          <div class="pf-av-cam">${icon('photo_camera', { size: 13, color: '#fff' })}</div>
        </div>
        <div class="pf-info">
          <div class="pf-name">${esc(fullName(u))}</div>
          ${u.username ? `<div class="pf-user">@${esc(u.username)}</div>` : ''}
          <div class="pf-id"><span>ID: ${esc(u.id)}</span>${icon('content_copy', { size: 16, color: 'rgba(255,255,255,0.6)' })}</div>
          <div class="pf-bal">Balans: ${esc(u.balance ?? 0)}</div>
        </div>
      </div>
      <div class="pf-edit">${icon('edit', { size: 17, color: 'rgba(255,255,255,0.85)' })}</div>
    </div>`;
  }

  function billingHtml() {
    return `<div class="pf-bill">
      ${icon('workspace_premium', { size: 20, color: '#fff' })}
      <div class="pf-bill-main">
        <div class="pf-bill-t">Obuna olish va Balans to'ldirish</div>
        <div class="pf-bill-s">${billing.active ? `Obuna ${subUntilText(billing.until)} gacha` : "Obuna yo'q"}</div>
        ${billing.active ? `<div class="pf-bill-c">${formatLeft(billing.until - Date.now())}</div>` : ''}
      </div>
      ${icon('chevron_right', { size: 20, color: 'rgba(255,255,255,0.7)' })}
    </div>`;
  }

  function statBox(kind, ic, value, fill = true) {
    const k = STAT_KINDS[kind];
    const open = k.openable;
    return `<div class="glass pf-stat${open ? ' open' : ''}"${open ? ` data-k="${kind}"` : ''}>
      <div class="pf-stat-h">${icon(ic, { fill, size: 16, color: C.accent })}<span>${k.label}</span></div>
      <div class="pf-stat-v">${esc(value)}</div>
    </div>`;
  }

  function statsHtml() {
    const s = myStats.stats;
    return `<div class="pf-stats">
      ${statBox('anime', 'movie_filter', formatCount(s.animes))}
      ${statBox('episodes', 'play_circle', formatCount(s.episodes), false)}
      ${statBox('seasons', 'grid_view', formatCount(s.seasons))}
      ${statBox('favorites', 'bookmark', formatCount(s.favorites))}
      ${statBox('rated', 'star', formatCount(s.rated))}
      ${statBox('comments', 'mode_comment', formatCount(s.comments))}
      ${statBox('watch', 'schedule', `${formatHours(s.watchMs)} soat`)}
    </div>`;
  }

  function tile(id, ic, label, { fill = true, trailing = '' } = {}) {
    return `<div class="pf-tile" data-t="${id}">${icon(ic, { fill, size: 24, color: 'rgba(255,255,255,0.7)' })}
      <span class="pf-tile-l">${label}</span>${trailing}${icon('chevron_right', { size: 24, color: 'rgba(255,255,255,0.38)' })}</div>`;
  }

  function menuHtml() {
    const items = [
      isAdminUser() ? tile('admin', 'admin_panel_settings', 'Admin paneli') : '',
      tile('notif', 'notifications', 'Bildirishnoma', { fill: false }),
      tile('settings', 'settings', 'Sozlamalar'),
      isTelegramAuthorized() ? '' : tile('telegram', 'send', "Telegram'ni ulash"),
      tile('sessions', 'devices', 'Qurilmalar'),
      tile('packs', 'emoji_emotions', 'Emoji, GIF va stikerlar'),
      tile('support', 'support_agent', "Admin bilan bog'lanish",
        { trailing: unread.count > 0 && !unread.admin ? '<span class="pf-dot"></span>' : '' }),
      tile('about', 'info', 'Ilova haqida', { fill: false }),
    ].filter(Boolean);
    return `<div class="glass pf-menu">${items.join('<div class="pf-div"></div>')}</div>`;
  }

  function render() {
    const u = currentUser();
    if (!u) { root.innerHTML = ''; return; }
    root.innerHTML = `
      ${cardHtml(u)}
      <div class="pf-bill-slot">${billingHtml()}</div>
      <div class="pf-stats-slot">${statsHtml()}</div>
      ${menuHtml()}
      <div class="glass pf-danger" data-t="delete">
        <span class="pf-danger-ic">${icon('delete_forever', { size: 22, color: C.accent })}</span>
        <span class="pf-danger-l">Accountni o'chirish</span>
      </div>`;
    bind();
  }

  function renderBilling() {
    const slot = root.querySelector('.pf-bill-slot');
    if (!slot) return;
    slot.innerHTML = billingHtml();
    bindTap(slot.firstElementChild, () => openBilling({ startPage: billing.balance > 0 ? 1 : 0 }), { scale: false });
    const prem = root.querySelector('.pf-av-prem');
    if (prem) prem.innerHTML = billing.active ? premiumBadge(true) : '';
  }

  function renderStats() {
    const s1 = root.querySelector('.pf-stats-slot');
    if (!s1) return;
    s1.innerHTML = statsHtml();
    const me = currentUser()?.id || 0;
    s1.querySelectorAll('.pf-stat.open').forEach((el) => bindTap(el, () => {
      if (me > 0) openStatDetail({ userId: me, kind: el.dataset.k, isMe: true });
    }));
  }

  function bind() {
    const u = currentUser();
    root.querySelector('.pf-av').addEventListener('click', changeAvatar);
    bindTap(root.querySelector('.pf-id'), async () => {
      await copyText(`${u.id}`);
      toast(`ID nusxalandi: ${u.id}`, 1400);
    }, { scale: false });
    bindTap(root.querySelector('.pf-edit'), () => openProfileEdit({ onSaved: render }), { scale: false });
    renderBilling();
    renderStats();
    bindMenu();
    bindTap(root.querySelector('.pf-danger'), () => deleteAccount());
  }

  function tick() {
    const c = root.querySelector('.pf-bill-c');
    if (!c) return;
    if (billing.active) c.textContent = formatLeft(billing.until - Date.now());
    else renderBilling();
  }

  billing.listen(renderBilling);
  myStats.listen(renderStats);
  onUser(render);
  render();

  return {
    onShow() {
      myStats.loadFromDisk();
      myStats.load();
      billing.load();
      refreshUnread().then(redrawMenu);
      Promise.resolve(checkTelegram()).catch(() => {}).then(redrawMenu);
      clearInterval(timer);
      timer = setInterval(tick, 1000);
    },
    onHide() { clearInterval(timer); timer = 0; },
  };

  function redrawMenu() {
    const m = root.querySelector('.pf-menu');
    if (m) { m.outerHTML = menuHtml(); bindMenu(); }
  }

  function bindMenu() {
    root.querySelectorAll('.pf-tile').forEach((el) => {
      const label = el.querySelector('.pf-tile-l').textContent;
      ripple(el, () => {
        switch (el.dataset.t) {
          case 'admin': openAdmin(); break;
          case 'settings': openSettings(); break;
          case 'telegram': Promise.resolve(openTelegramAccount()).catch(() => {}).then(redrawMenu); break;
          case 'sessions': openSessions(); break;
          case 'packs': openMyPacks(); break;
          case 'support': openSupport(); break;
          default: toast(`«${label}» bo'limi tez orada qo'shiladi`, 1600);
        }
      });
    });
  }
}
