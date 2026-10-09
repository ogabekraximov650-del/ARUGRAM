// Boshqa odamning profili — `lib/screens/public_profile_screen.dart`.
//
// GET /api/user/:id (sessiya bilan: admin bo'lsa server qo'shimcha
// maydonlarni ham beradi — `admin_view`). Egasi yashirgan statistika
// javobda UMUMAN yo'q, ya'ni "maydon bormi" = "ko'rinadimi".
// Ilovadagi disk nusxasi o'rnida localStorage (faqat o'qish keshi).

import { api, ApiError, currentUser, imageUrl } from '../api.js';
import { esc, formatCount, toInt } from '../format.js';
import { push } from '../router.js';
import { C, icon, spinner, appBar, bindAppBar, bindTap, toast } from '../ui.js';
import { STAT_KINDS, formatBytes, formatHours, formatMoment, openStatDetail } from './stats.js';
import { copyText } from './profile.js';

const key = (id) => `aru_profile_${id}`;
function readCache(id) { try { return JSON.parse(localStorage.getItem(key(id)) || 'null'); } catch (_) { return null; } }
function writeCache(id, v) { try { localStorage.setItem(key(id), JSON.stringify(v)); } catch (_) { /* */ } }

/** `15 000 so'm` */
function formatSum(amount) {
  const n = String(Math.abs(toInt(amount)));
  let out = '';
  for (let i = 0; i < n.length; i++) {
    if (i > 0 && (n.length - i) % 3 === 0) out += ' ';
    out += n[i];
  }
  return `${out} so'm`;
}

function statBox(ic, label, value, { kind = '', fill = true } = {}) {
  return `<div class="glass pp-stat${kind ? ' open' : ''}"${kind ? ` data-k="${kind}"` : ''}>
    <div class="pp-stat-h">${icon(ic, { fill, size: 17, color: C.accent })}<span>${label}</span></div>
    <div class="pp-stat-v">${esc(value)}</div>
  </div>`;
}

function adminBox(d) {
  const until = toInt(d.subscription_until);
  const now = Date.now();
  const active = until > now;
  const left = active ? Math.ceil((until - now) / 86400000) : 0;
  const r = (l, v) => `<div class="pp-ar"><span class="l">${l}</span><span class="v">${esc(v)}</span></div>`;
  return `<div class="glass pp-admin">
    <div class="pp-admin-h">${icon('admin_panel_settings', { size: 17, color: C.accent })}<span>Admin uchun</span></div>
    ${r('Telegram ID', `${toInt(d.telegram_id)}`)}
    ${r('Obuna', active ? `Faol · yana ${left} kun` : "Yo'q")}
    ${r("Ro'yxatdan o'tgan", formatMoment(d.created_at))}
    ${r('Oxirgi kirish', formatMoment(d.last_login_at))}
  </div>`;
}

export function openPublicProfile(userId) {
  const id = toInt(userId);
  if (id <= 0) return;
  push((el) => {
    el.innerHTML = `${appBar({ title: 'Profil' })}<div class="scroll"></div>`;
    bindAppBar(el);
    const body = el.querySelector('.scroll');
    let data = readCache(id);
    let loading = data == null;
    let error = null;
    let dead = false;

    function render() {
      if (loading) {
        body.innerHTML = `<div class="pp-center">${spinner(36, 2)}</div>`;
        return;
      }
      const d = data;
      if (!d) {
        body.innerHTML = `<div class="pp-center"><span class="pp-msg">${esc(error ?? "Ma'lumot yo'q")}</span></div>`;
        return;
      }
      const first = `${d.first_name ?? ''}`.trim();
      const last = `${d.last_name ?? ''}`.trim();
      const name = `${first} ${last}`.trim();
      const username = `${d.username ?? ''}`.trim();
      const photo = imageUrl(`${d.photo_url ?? ''}`);
      const premium = d.premium === true;
      const shown = name || (username ? `@${username}` : `Foydalanuvchi ${id}`);
      const letterSrc = (name || username).trim();
      const letter = letterSrc ? [...letterSrc][0].toUpperCase() : '?';
      const has = (k) => d[k] != null;
      const n = (k) => toInt(d[k]);

      const tiles = [];
      if (has('animes')) tiles.push(statBox('movie_filter', STAT_KINDS.anime.label, formatCount(n('animes'))));
      if (has('episodes')) tiles.push(statBox('play_circle', STAT_KINDS.episodes.label, formatCount(n('episodes')), { kind: 'episodes', fill: false }));
      if (has('seasons')) tiles.push(statBox('grid_view', STAT_KINDS.seasons.label, formatCount(n('seasons')), { kind: 'seasons' }));
      if (has('favorites')) tiles.push(statBox('bookmark', STAT_KINDS.favorites.label, formatCount(n('favorites')), { kind: 'favorites' }));
      if (has('rated')) tiles.push(statBox('star', STAT_KINDS.rated.label, formatCount(n('rated')), { kind: 'rated' }));
      if (has('comments')) tiles.push(statBox('mode_comment', STAT_KINDS.comments.label, formatCount(n('comments')), { kind: 'comments' }));
      if (has('watch_ms')) tiles.push(statBox('schedule', STAT_KINDS.watch.label, `${formatHours(n('watch_ms'))} soat`));

      const admin = d.admin_view === true;
      body.innerHTML = `<div class="pp">
        <div class="glass pp-card">
          <div class="pp-av-wrap">
            <div class="pp-av"><span class="pp-av-l">${esc(letter)}</span>${photo ? `<img alt="" src="${esc(photo)}" onerror="this.remove()">` : ''}</div>
            ${premium ? `<span class="pp-prem">${icon('workspace_premium', { size: 12, color: 'rgba(0,0,0,0.87)' })}<span>PREMIUM</span></span>` : ''}
          </div>
          <div class="pp-info">
            <div class="pp-name">${esc(shown)}</div>
            ${username && name ? `<div class="pp-user">@${esc(username)}</div>` : ''}
            <div class="pp-id"><span>ID: ${id}</span>${icon('content_copy', { size: 14, color: 'rgba(255,255,255,0.5)' })}</div>
            ${admin ? `<div class="pp-bal">Balans: ${formatSum(d.balance)}</div>` : ''}
          </div>
        </div>
        <div style="height:14px"></div>
        ${tiles.length === 0 ? `<div class="glass pp-locked">${icon('lock', { fill: false, size: 30, color: 'rgba(255,255,255,0.3)' })}
          <div>Bu foydalanuvchi statistikasini yashirgan</div></div>`
    : `<div class="pp-grid">${tiles.join('')}</div>`}
        ${admin ? `${statBox('download', 'Trafik', formatBytes(n('traffic')))}<div style="height:12px"></div>${adminBox(d)}` : ''}
      </div>`;

      bindTap(body.querySelector('.pp-id'), async () => {
        await copyText(`${id}`);
        toast('ID nusxalandi');
      }, { scale: false });
      body.querySelectorAll('.pp-stat.open').forEach((t) => bindTap(t, () => openStatDetail({
        userId: id, kind: t.dataset.k, owner: shown, isMe: currentUser()?.id === id,
      })));
    }

    (async () => {
      try {
        const j = await api(`/api/user/${id}`);
        if (dead) return;
        if (j && typeof j === 'object') { data = j; writeCache(id, j); }
      } catch (e) {
        if (dead) return;
        if (data == null) error = e instanceof ApiError && e.status ? 'Foydalanuvchi topilmadi' : "Internet yo'q";
      }
      loading = false;
      render();
    })();
    render();
    return { dispose() { dead = true; } };
  });
}
