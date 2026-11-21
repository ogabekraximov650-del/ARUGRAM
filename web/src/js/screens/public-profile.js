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

function statBox(label, value, { kind = '' } = {}) {
  const muted = value == null;
  return `<div class="pp-stat${kind ? ' open' : ''}"${kind ? ` data-k="${kind}"` : ''}>
    <div class="pp-stat-v${muted ? ' muted' : ''}">${esc(muted ? 'Yashirilgan' : value)}</div>
    <div class="pp-stat-l">${label}${kind ? ' ›' : ''}</div>
  </div>`;
}

function adminBox(d) {
  const until = toInt(d.subscription_until);
  const active = until > Date.now();
  const r = (l, v) => `<div class="pp-ar"><span class="l">${l}</span><span class="v">${esc(v)}</span></div>`;
  return `<div class="pp-admin">
    <div class="pp-admin-h">ADMIN KO'RINISHI</div>
    ${r('Telegram ID', `${toInt(d.telegram_id)}`)}
    ${r('Balans', formatSum(d.balance))}
    ${r('Trafik', formatBytes(toInt(d.traffic)))}
    ${r('Obuna', active ? `${formatMoment(until)} gacha` : "yo'q")}
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
      const add = (k, label, fmt, kind) => { if (has(k)) tiles.push(statBox(label, fmt(n(k)), { kind })); };
      add('animes', STAT_KINDS.anime.label, formatCount);
      add('episodes', STAT_KINDS.episodes.label, formatCount, 'episodes');
      add('seasons', STAT_KINDS.seasons.label, formatCount, 'seasons');
      add('watch_ms', STAT_KINDS.watch.label, (v) => `${formatHours(v)} soat`);
      add('favorites', STAT_KINDS.favorites.label, formatCount, 'favorites');
      add('rated', STAT_KINDS.rated.label, formatCount, 'rated');
      add('comments', STAT_KINDS.comments.label, formatCount, 'comments');

      const admin = d.admin_view === true;
      const isMe = currentUser()?.id === id;
      body.innerHTML = `<div class="pp">
        <div class="pp-card">
          <div class="pp-av"><span class="pp-av-l">${esc(letter)}</span>${photo ? `<img alt="" src="${esc(photo)}" onerror="this.remove()">` : ''}</div>
          <div class="pp-info">
            <div class="pp-name">${esc(shown)}</div>
            ${username && name ? `<div class="pp-user">@${esc(username)}</div>` : ''}
            <div class="pp-chips"><span class="pp-chip pp-id">ID ${id}</span>${premium ? '<span class="pp-chip pp-prem">Premium</span>' : ''}</div>
          </div>
        </div>
        ${admin ? adminBox(d) : ''}
        ${tiles.length === 0 ? `<div class="pp-note">Bu foydalanuvchi statistikasini yashirgan</div>`
    : `<div class="pp-grid">${tiles.join('')}</div>`}
        ${isMe ? '<div class="pp-note">Bu sizning profilingiz: balans va trafik boshqalarga ko\'rinmaydi.</div>' : ''}
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
