// Izohlar oynasi — `lib/widgets/comments_tab.dart` + `lib/services/comments_service.dart`
// (+ shikoyat: `lib/services/reports_service.dart` -> `sendCommentReport`).
//
// YouTube'dagidek: rasm, ism, qachon yozilgani, matn, layk, "Javob berish",
// o'z izohini o'chirish, boshqalarnikiga shikoyat (⋮). Javoblar yig'ilgan
// holda, "N ta javob" bosilganda ochiladi. Tartib: Yangilar / Layklar /
// Javoblar (serverda). Ro'yxat 30 tadan sahifalab keladi; oyna ochiq
// turganda yangi izohlar `/api/comments/wait` orqali o'zi chiqadi.
//
// Pleyerda (`video_player_screen.dart` -> `_buildCommentsTab`) ilova
// shunday quradi: `CommentsTab(controller: CommentsController(animeId,
// seasonId), expanded: _commentsExpanded, onExpanded: _setCommentsExpanded)`.
// Bu yerda ham xuddi o'sha parametrlar:
//
//   const tab = createCommentsTab(container, {
//     animeId, seasonId,         // bo'lim (season.anime_id / season_id)
//     expanded = false,          // izohlar butun ekranga kattalashganmi
//     onExpanded(v) {},          // ro'yxat surildi -> kattalashtirish/yig'ish so'rovi
//   });
//   tab.setExpanded(v)           // pleyer holatni o'zgartirganda (tugma belgisi)
//   tab.total                    // izohlar soni (javoblar bilan) — tab sarlavhasi uchun
//   tab.onChange(fn)             // ro'yxat o'zgarganda (total yangilanishi uchun)
//   tab.dispose()
//
// `container` — bo'sh element; u flex-ustun bo'lib, butun joyni egallaydi.
//
// Stiker/GIF izohlar (ilova to'plamlari) bu yerda faqat belgi bilan
// ko'rinadi va yuborilmaydi — to'plamlar Mini App'da hali yo'q.

import { api, currentUser, imageUrl } from './api.js';
import { icon, spinner, toast, C } from './ui.js';
import { esc } from './format.js';
import { hooks } from './hooks.js';
import { richText, emojiButtonHtml, bindEmojiInput } from './screens/tg-emoji.js';
import { renderPackMedia } from './screens/packs.js';

const SORTS = [
  { code: 'yangi', label: 'Yangilar' },
  { code: 'layk', label: 'Layklar' },
  { code: 'javob', label: 'Javoblar' },
];
const REPORT_MIN = 10;
const REPORT_MAX = 2000;
const RED300 = '#E57373';
const RED600 = '#E53935';

const num = (v) => { const n = Number(v); return Number.isFinite(n) ? Math.trunc(n) : 0; };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const two = (n) => String(n).padStart(2, '0');
function lsRead(k) { try { return JSON.parse(localStorage.getItem(k) || 'null'); } catch (_) { return null; } }
function lsWrite(k, v) { try { localStorage.setItem(k, JSON.stringify(v)); } catch (_) { /* */ } }

function fromJson(j) {
  return {
    id: `${j?.id ?? ''}`,
    parentId: `${j?.parent_id ?? ''}`,
    userId: num(j?.user_id),
    firstName: `${j?.first_name ?? ''}`,
    username: `${j?.username ?? ''}`,
    photoUrl: `${j?.photo_url ?? ''}`,
    body: `${j?.body ?? ''}`,
    mediaFile: `${j?.media_file ?? ''}`,
    mediaType: `${j?.media_type ?? ''}`,
    createdAt: num(j?.created_at),
    deleted: j?.deleted === true,
    likes: num(j?.likes),
    replyCount: num(j?.reply_count),
    liked: j?.liked === true,
  };
}
const toJson = (c) => ({
  id: c.id, parent_id: c.parentId, user_id: c.userId, first_name: c.firstName, username: c.username,
  photo_url: c.photoUrl, body: c.body, media_file: c.mediaFile, media_type: c.mediaType,
  created_at: c.createdAt, deleted: c.deleted, likes: c.likes, reply_count: c.replyCount, liked: c.liked,
});
const alive = (j) => j?.deleted !== true;
function nameOf(c) {
  const n = c.firstName.trim();
  if (n) return n;
  const u = c.username.trim();
  return u ? `@${u}` : 'Foydalanuvchi';
}
function preview(c) {
  if (c.body) return c.body;
  return c.mediaType === 'sticker' ? 'Stiker' : c.mediaType === 'gif' ? 'GIF' : c.body;
}

/** "3 daqiqa oldin" (`commentAgo`). */
export function commentAgo(ms) {
  if (ms <= 0) return '';
  const d = Date.now() - ms;
  if (d < 60000) return 'hozir';
  const min = Math.floor(d / 60000);
  if (min < 60) return `${min} daqiqa oldin`;
  const h = Math.floor(min / 60);
  if (h < 24) return `${h} soat oldin`;
  const days = Math.floor(h / 24);
  if (days < 30) return `${days} kun oldin`;
  const mo = Math.floor(days / 30);
  if (mo < 12) return `${mo} oy oldin`;
  return `${Math.floor(days / 365)} yil oldin`;
}

/** Bugun — soat, aks holda to'liq sana (`commentClock`). */
export function commentClock(ms) {
  if (ms <= 0) return '';
  const d = new Date(ms);
  const now = new Date();
  const hm = `${two(d.getHours())}:${two(d.getMinutes())}`;
  if (d.getFullYear() === now.getFullYear() && d.getMonth() === now.getMonth() && d.getDate() === now.getDate()) return hm;
  return `${two(d.getDate())}.${two(d.getMonth() + 1)}.${d.getFullYear()} ${hm}`;
}

// ══════════════════════════════════════════════════════════════
//  NAZORATCHI (`CommentsController`)
// ══════════════════════════════════════════════════════════════

class Comments {
  constructor(animeId, seasonId) {
    this.animeId = animeId;
    this.seasonId = seasonId;
    this.items = [];
    this.replies = new Map();
    this.loadingReplies = new Set();
    this.likeBusy = new Set();
    this.loading = false;
    this.loadingMore = false;
    this.hasMore = false;
    this.page = 0;
    this.error = null;
    this.loaded = false;
    this.mk = -1;
    this.at = 0;
    this.sort = 'yangi';
    this.watching = false;
    this.subs = new Set();
  }

  get base() { return '/api/comments'; }
  get diskKey() { return `aru_comments_${this.animeId}_${this.seasonId}`; }
  get mkKey() { return `${this.diskKey}_mk`; }
  get total() { return this.items.reduce((n, c) => n + c.replyCount, this.items.length); }
  emit() { this.subs.forEach((f) => { try { f(); } catch (e) { console.error(e); } }); }

  setSort(code) {
    if (this.sort === code) return;
    this.sort = code;
    this.items = []; this.replies.clear(); this.page = 0; this.hasMore = false; this.loaded = false;
    this.emit();
    this.load({ force: true });
  }

  loadFromDisk() {
    if (this.items.length || this.sort !== 'yangi') return;
    const rows = lsRead(this.diskKey);
    if (!Array.isArray(rows)) return;
    this.items = rows.filter(alive).map(fromJson);
    this.loaded = true;
    const m = lsRead(this.mkKey);
    if (m && m.u === (currentUser()?.id ?? 0)) { this.mk = num(m.mk); this.at = num(m.at); }
    this.emit();
  }

  saveDisk() {
    if (this.sort !== 'yangi') return;
    lsWrite(this.diskKey, this.items.map(toJson));
  }

  async load({ force = false } = {}) {
    if (this.loading) return;
    if (this.loaded && !force) return;
    this.loading = true;
    this.error = null;
    this.emit();
    try {
      const canSame = this.sort === 'yangi' && this.items.length > 0 && this.mk >= 0;
      const j = await api(`${this.base}/${this.animeId}/${this.seasonId}?page=0&sort=${this.sort}${canSame ? `&mk=${this.mk}&at=${this.at}` : ''}`);
      if (canSame && j?.same === true) {
        this.loaded = true;
      } else {
        this.mk = j?.mk == null ? -1 : num(j.mk);
        this.at = num(j?.at);
        if (this.sort === 'yangi') lsWrite(this.mkKey, { mk: this.mk, at: this.at, u: currentUser()?.id ?? 0 });
        const rows = Array.isArray(j?.items) ? j.items : [];
        this.items = rows.filter(alive).map(fromJson);
        this.page = 0;
        this.hasMore = j?.has_more === true;
        this.loaded = true;
        this.replies.clear();
        if (this.sort === 'yangi') lsWrite(this.diskKey, rows);
      }
    } catch (e) {
      this.error = e && e.status > 0 ? `Izohlar yuklanmadi (${e.status})` : "Internet yo'q";
    }
    this.loading = false;
    this.emit();
  }

  startWatching() {
    if (this.watching) return;
    this.watching = true;
    this.watchLoop();
  }
  stopWatching() { this.watching = false; try { this.abort?.abort(); } catch (_) { /* */ } }

  async watchLoop() {
    while (this.watching) {
      if (document.visibilityState === 'hidden' || this.mk < 0 || this.loading) { await sleep(2000); continue; }
      try {
        const ctl = typeof AbortController !== 'undefined' ? new AbortController() : null;
        this.abort = ctl;
        const t = setTimeout(() => { try { ctl?.abort(); } catch (_) { /* */ } }, 35000);
        let j;
        try { j = await api(`${this.base}/wait?mk=${this.mk}`, ctl ? { signal: ctl.signal } : {}); } finally { clearTimeout(t); }
        if (!this.watching) return;
        if (j?.new === true) {
          await this.load({ force: true });
          const mk = j?.mk == null ? null : num(j.mk);
          if (mk != null && mk > this.mk) this.mk = mk;
        }
      } catch (e) {
        if (!this.watching) return;
        await sleep(e && e.status > 0 ? 5000 : 3000);
      }
    }
  }

  async loadMore() {
    if (this.loadingMore || this.loading || !this.hasMore) return;
    this.loadingMore = true;
    this.emit();
    const next = this.page + 1;
    try {
      const j = await api(`${this.base}/${this.animeId}/${this.seasonId}?page=${next}&sort=${this.sort}`);
      const have = new Set(this.items.map((c) => c.id));
      const rows = (Array.isArray(j?.items) ? j.items : []).filter(alive).map(fromJson);
      this.items = this.items.concat(rows.filter((c) => !have.has(c.id)));
      this.page = next;
      this.hasMore = j?.has_more === true;
    } catch (_) { /* keyingi urinishda */ }
    this.loadingMore = false;
    this.emit();
  }

  async toggleReplies(id) {
    if (this.replies.has(id)) { this.replies.delete(id); this.emit(); return; }
    if (this.loadingReplies.has(id)) return;
    this.loadingReplies.add(id);
    this.emit();
    try {
      const j = await api(`${this.base}/${this.animeId}/${this.seasonId}/${id}`);
      this.replies.set(id, (Array.isArray(j?.items) ? j.items : []).filter(alive).map(fromJson).reverse());
    } catch (_) { /* */ }
    this.loadingReplies.delete(id);
    this.emit();
  }

  async add(body, { parentId = '', mediaFile = '', mediaType = '' } = {}) {
    const text = body.trim();
    if (!text && !mediaFile) return "Izoh bo'sh";
    try {
      const j = await api(this.base, {
        method: 'POST',
        body: { anime_id: this.animeId, season_id: this.seasonId, parent_id: parentId, body: text, ...(mediaFile ? { media_file: mediaFile, media_type: mediaType } : {}) },
      });
      const c = fromJson(j);
      if (c.parentId) {
        const list = this.replies.get(c.parentId);
        if (list) list.push(c);
        const p = this.items.find((x) => x.id === c.parentId);
        if (p) p.replyCount += 1;
      } else {
        this.items.unshift(c);
      }
      this.emit();
      return null;
    } catch (e) {
      if (e && e.status > 0) return `${e.body?.error ?? 'Izoh yuborilmadi'}`;
      return "Internet yo'q — qaytadan urinib ko'ring";
    }
  }

  async remove(id) {
    const snapItems = this.items.slice();
    const snapReplies = new Map([...this.replies].map(([k, v]) => [k, v.slice()]));
    this.items = this.items.filter((c) => c.id !== id);
    for (const [k, v] of this.replies) this.replies.set(k, v.filter((c) => c.id !== id));
    this.emit();
    const restore = () => { this.items = snapItems; this.replies = snapReplies; this.emit(); };
    try {
      const j = await api(`${this.base}/${id}`, { method: 'DELETE' });
      const parent = `${j?.parent_id ?? ''}`;
      if (parent) {
        const p = this.items.find((x) => x.id === parent);
        if (p && p.replyCount > 0) p.replyCount -= 1;
      }
      this.replies.delete(id);
      this.saveDisk();
      this.emit();
      return null;
    } catch (e) {
      restore();
      if (e && e.status > 0) return `${e.body?.error ?? "O'chirilmadi"}`;
      return "Internet yo'q";
    }
  }

  find(id) {
    for (const c of this.items) if (c.id === id) return c;
    for (const l of this.replies.values()) for (const c of l) if (c.id === id) return c;
    return null;
  }

  /** Layk: ekranda darhol, server bilan oxirida tenglashadi. */
  async toggleLike(id) {
    const c = this.find(id);
    if (!c) return null;
    if (!currentUser()) return 'Layk bosish uchun hisobingizga kiring';
    c.liked = !c.liked;
    c.likes = c.liked ? c.likes + 1 : Math.max(0, c.likes - 1);
    this.emit();
    if (this.likeBusy.has(id)) return null;
    const wasLiked = !c.liked;
    const wasLikes = c.liked ? c.likes - 1 : c.likes + 1;
    this.likeBusy.add(id);
    let error = null;
    try {
      for (let i = 0; i < 5; i++) {
        const j = await api(`${this.base}/like`, { method: 'POST', body: { id } });
        const liked = j?.liked === true;
        const likes = j?.likes == null ? c.likes : num(j.likes);
        if (liked === c.liked) { c.likes = likes; this.emit(); break; }
      }
    } catch (e) {
      error = e && e.status > 0 ? `${e.body?.error ?? 'Layk yuborilmadi'}` : "Internet yo'q";
    }
    if (error) { c.liked = wasLiked; c.likes = wasLikes; this.emit(); }
    this.likeBusy.delete(id);
    return error;
  }
}

async function sendCommentReport(commentId, reason) {
  if (!currentUser()) return 'Shikoyat yuborish uchun hisobingizga kiring';
  const text = reason.trim();
  if (text.length < REPORT_MIN) return 'Iltimos, shikoyat sababini batafsilroq yozing';
  try {
    await api('/api/reports', { method: 'POST', body: { kind: 'comment', target_id: commentId, reason: text } });
    return null;
  } catch (e) {
    if (e && e.status > 0) return `${e.body?.error ?? 'Shikoyat yuborilmadi'}`;
    return "Internet yo'q — qaytadan urinib ko'ring";
  }
}

// ══════════════════════════════════════════════════════════════
//  OYNA
// ══════════════════════════════════════════════════════════════

function avatar(url, name, size) {
  const u = imageUrl(url);
  const letter = esc((`${name}`.trim()[0] || '?').toUpperCase());
  return `<span class="cm-ava" style="width:${size}px;height:${size}px;font-size:${(size * 0.42).toFixed(1)}px">
    <span class="l">${letter}</span>${u ? `<img src="${esc(u)}" alt="" loading="lazy" onerror="this.remove()">` : ''}</span>`;
}

/** Pastdan chiqadigan oyna (o'z ko'rinishi bilan). */
function bottomPanel(builder, cls) {
  return new Promise((resolve) => {
    const app = document.getElementById('app');
    const wrap = document.createElement('div');
    wrap.className = `cm-sheet-wrap ${cls}`;
    wrap.innerHTML = '<div class="cm-sheet"></div>';
    const sh = wrap.querySelector('.cm-sheet');
    const close = (v) => { wrap.classList.remove('in'); setTimeout(() => wrap.remove(), 220); resolve(v ?? null); };
    wrap.addEventListener('click', (e) => { if (e.target === wrap) close(null); });
    builder(sh, close);
    app.appendChild(wrap);
    requestAnimationFrame(() => wrap.classList.add('in'));
  });
}

export function createCommentsTab(container, { animeId, seasonId, expanded = false, onExpanded } = {}) {
  const ctl = new Comments(num(animeId), num(seasonId));
  let isExpanded = !!expanded;
  let replyTo = null;
  let sending = false;
  let reportDone = false;
  let reportTimer = 0;
  let lastToggle = 0;
  let disposed = false;
  const changeSubs = new Set();

  container.classList.add('cm');
  container.innerHTML = `
    <div class="cm-sortbar">
      <div class="cm-exp">${icon('keyboard_arrow_up', { size: 22, color: 'rgba(255,255,255,0.55)' })}</div>
      <div class="grow"></div>
      ${SORTS.map((s) => `<div class="cm-chip" data-s="${s.code}">${s.label}</div>`).join('')}
    </div>
    <div class="cm-banner-host"></div>
    <div class="cm-list"><div class="cm-pull"></div><div class="cm-items"></div></div>
    <div class="cm-comp">
      <div class="cm-replying"></div>
      <div class="cm-row">
        <div class="cm-field">${emojiButtonHtml()}<textarea rows="1" maxlength="1000" placeholder="Izoh yozing..."></textarea></div>
        <div class="cm-send">${icon('send', { size: 19, color: 'rgba(255,255,255,0.38)' })}</div>
      </div>
      <div class="cm-panel"></div>
    </div>`;

  const expEl = container.querySelector('.cm-exp');
  const chips = [...container.querySelectorAll('.cm-chip')];
  const bannerHost = container.querySelector('.cm-banner-host');
  const listEl = container.querySelector('.cm-list');
  const pullEl = container.querySelector('.cm-pull');
  const itemsEl = container.querySelector('.cm-items');
  const replyingEl = container.querySelector('.cm-replying');
  const ta = container.querySelector('textarea');
  const sendEl = container.querySelector('.cm-send');
  const emoji = bindEmojiInput({ button: container.querySelector('.tge-btn'), panelHost: container.querySelector('.cm-panel'), input: ta, height: 280, onPickMedia: (p) => sendPackComment(p) });

  function paintExp() {
    expEl.querySelector('.ic').textContent = isExpanded ? 'keyboard_arrow_down' : 'keyboard_arrow_up';
  }
  expEl.addEventListener('click', () => { lastToggle = Date.now(); onExpanded?.(!isExpanded); });
  chips.forEach((ch) => ch.addEventListener('click', () => ctl.setSort(ch.dataset.s)));

  function setExpandedReq(v) {
    if (isExpanded === v) return;
    const now = Date.now();
    if (now - lastToggle < 400) return;
    lastToggle = now;
    onExpanded?.(v);
  }

  listEl.addEventListener('scroll', () => {
    const left = listEl.scrollHeight - listEl.clientHeight - listEl.scrollTop;
    if (left < 400) ctl.loadMore();
    if (listEl.scrollTop > 24) setExpandedReq(true);
    else if (listEl.scrollTop <= 2) setExpandedReq(false);
  }, { passive: true });

  // Tepadan tortib yangilash (`RefreshIndicator`).
  let py = 0; let pull = 0; let refreshing = false;
  listEl.addEventListener('touchstart', (e) => { py = e.touches[0].clientY; pull = 0; }, { passive: true });
  listEl.addEventListener('touchmove', (e) => {
    const y = e.touches[0].clientY;
    if (listEl.scrollTop <= 0 && y > py && !refreshing) {
      pull = Math.min(100, y - py);
      pullEl.style.height = `${pull * 0.6}px`;
    }
  }, { passive: true });
  listEl.addEventListener('touchend', async () => {
    if (pull > 70 && !refreshing) {
      refreshing = true;
      pullEl.style.height = '44px';
      pullEl.innerHTML = spinner(22, 2, C.accent);
      await ctl.load({ force: true });
      refreshing = false;
      pullEl.innerHTML = '';
    }
    pull = 0;
    if (!refreshing) pullEl.style.height = '0px';
    else pullEl.style.height = '0px';
  });

  // ── Ro'yxat ──
  function rowHtml(c, small) {
    const size = small ? 26 : 34;
    const mine = currentUser()?.id === c.userId;
    const media = !c.deleted && (c.mediaType === 'sticker' || c.mediaType === 'gif');
    return `<div class="cm-c${small ? ' small' : ''}" data-id="${esc(c.id)}">
      <div class="cm-prof">${avatar(c.photoUrl, nameOf(c), size)}</div>
      <div class="cm-main">
        <div class="cm-hd"><span class="cm-n cm-prof">${esc(nameOf(c))}</span>
          <span class="cm-t">${esc(`${commentAgo(c.createdAt)} · ${commentClock(c.createdAt)}`)}</span></div>
        ${media ? `<div class="cm-media" data-f="${esc(c.mediaFile)}" data-t="${c.mediaType}"></div>`
    : `<div class="cm-body${c.deleted ? ' del' : ''}">${c.deleted ? "Izoh o'chirilgan" : richText(c.body)}</div>`}
        ${!c.deleted ? `<div class="cm-acts">
          <div class="cm-like${c.liked ? ' on' : ''}">${icon('thumb_up', { fill: c.liked, size: 20 })}${c.likes > 0 ? `<span>${c.likes}</span>` : ''}</div>
          <div class="cm-tb reply">Javob berish</div>
          ${mine ? `<div class="cm-tb del" style="color:${RED300}">O'chirish</div>` : ''}
        </div>` : ''}
      </div>
      ${!mine && !c.deleted ? `<div class="cm-more">${icon('more_vert', { size: 19, color: 'rgba(255,255,255,0.45)' })}</div>` : ''}
    </div>`;
  }

  function render() {
    if (disposed) return;
    chips.forEach((ch) => ch.classList.toggle('on', ch.dataset.s === ctl.sort));
    const items = ctl.items;
    if (ctl.loading && !items.length) {
      itemsEl.innerHTML = `<div class="cm-center">${spinner(36, 2, C.accent)}</div>`;
    } else if (!items.length) {
      itemsEl.innerHTML = `<div class="cm-empty">${icon('mode_comment', { fill: false, size: 46, color: 'rgba(255,255,255,0.2)' })}
        <div class="t">${esc(ctl.error ?? "Hali izoh yo'q — birinchi bo'ling")}</div></div>`;
    } else {
      itemsEl.innerHTML = items.map((c) => {
        const open = ctl.replies.has(c.id);
        const reps = ctl.replies.get(c.id) || [];
        return `<div class="cm-block">
          ${rowHtml(c, false)}
          ${c.replyCount > 0 ? `<div class="cm-rt" data-id="${esc(c.id)}">
            ${ctl.loadingReplies.has(c.id) ? spinner(13, 2, 'rgba(255,255,255,0.38)') : icon(open ? 'keyboard_arrow_up' : 'keyboard_arrow_down', { size: 18, color: C.accent })}
            <span>${open ? 'Javoblarni yashirish' : `${c.replyCount} ta javob`}</span></div>` : ''}
          ${open ? `<div class="cm-replies">${reps.map((r) => `<div class="cm-rw">${rowHtml(r, true)}</div>`).join('')}</div>` : ''}
        </div>`;
      }).join('') + (ctl.hasMore ? `<div class="cm-more-load">${spinner(20, 2, 'rgba(255,255,255,0.38)')}</div>` : '');
      itemsEl.querySelectorAll('.cm-media[data-f]').forEach((n) => renderPackMedia(n, n.dataset.f, n.dataset.t));
      itemsEl.querySelectorAll('img').forEach((img) => { if (img.complete && !img.naturalWidth) img.remove(); });
    }
    changeSubs.forEach((f) => { try { f(); } catch (_) { /* */ } });
  }

  itemsEl.addEventListener('click', async (e) => {
    const rt = e.target.closest('.cm-rt');
    if (rt) { ctl.toggleReplies(rt.dataset.id); return; }
    const row = e.target.closest('.cm-c');
    if (!row) return;
    const c = ctl.find(row.dataset.id);
    if (!c) return;
    if (e.target.closest('.cm-prof')) { if (c.userId > 0) hooks.openUser(c.userId); return; }
    if (e.target.closest('.cm-like')) { const err = await ctl.toggleLike(c.id); if (err) toast(err); return; }
    if (e.target.closest('.cm-tb.reply')) { startReply(c); return; }
    if (e.target.closest('.cm-tb.del')) { confirmDelete(c); return; }
    if (e.target.closest('.cm-more')) openMenu(c);
  });

  // ── Yozish ──
  const blank = () => !ta.value.trim();
  function paintSend() {
    const on = !blank() && !sending;
    sendEl.classList.toggle('on', on);
    sendEl.innerHTML = sending ? spinner(18, 2, '#fff') : icon('send', { size: 19, color: on ? '#fff' : 'rgba(255,255,255,0.38)' });
  }
  function autosize() { ta.style.height = 'auto'; ta.style.height = `${Math.min(ta.scrollHeight, 4 * 19 + 16)}px`; }
  ta.addEventListener('input', () => { autosize(); paintSend(); });
  function paintReplying() {
    ta.placeholder = replyTo ? 'Javob yozing...' : 'Izoh yozing...';
    if (!replyTo) { replyingEl.innerHTML = ''; return; }
    replyingEl.innerHTML = `<div class="cm-rp">${icon('reply', { size: 15, color: 'rgba(255,255,255,0.5)' })}
      <span class="t">${esc(nameOf(replyTo))}ga javob</span>
      <span class="x">${icon('close', { size: 16, color: 'rgba(255,255,255,0.54)' })}</span></div>`;
    replyingEl.querySelector('.x').addEventListener('click', () => { replyTo = null; paintReplying(); });
  }
  function startReply(c) { replyTo = c; paintReplying(); ta.focus(); }

  async function sendPackComment(p) {
    if (sending) return;
    if (!currentUser()) { toast('Izoh yozish uchun hisobingizga kiring'); return; }
    sending = true; paintSend();
    const err = await ctl.add('', { parentId: replyTo?.id ?? '', mediaFile: `pk_${p.pack}_${p.item}`, mediaType: p.kind });
    sending = false;
    if (disposed) return;
    if (err) { paintSend(); toast(err); return; }
    paintSend(); replyTo = null; paintReplying(); emoji.close();
  }

  sendEl.addEventListener('click', async () => {
    if (sending || blank()) return;
    if (!currentUser()) { toast('Izoh yozish uchun hisobingizga kiring'); return; }
    sending = true; paintSend();
    const err = await ctl.add(ta.value, { parentId: replyTo?.id ?? '' });
    sending = false;
    if (disposed) return;
    if (err) { paintSend(); toast(err); return; }
    ta.value = ''; autosize(); paintSend();
    replyTo = null; paintReplying();
    ta.blur(); emoji.close();
  });

  // ── O'chirish ──
  function confirmDelete(c) {
    const app = document.getElementById('app');
    const wrap = document.createElement('div');
    wrap.className = 'dlg-wrap';
    wrap.innerHTML = `<div class="dlg cm-dlg"><div class="cm-dlg-t">Izoh o'chirilsinmi?</div>
      <div class="cm-dlg-b"><button class="btn cm-no">Yo'q</button><button class="btn cm-yes">Ha</button></div></div>`;
    const close = () => { wrap.classList.remove('in'); setTimeout(() => wrap.remove(), 180); };
    wrap.addEventListener('click', (e) => { if (e.target === wrap) close(); });
    wrap.querySelector('.cm-no').addEventListener('click', close);
    wrap.querySelector('.cm-yes').addEventListener('click', async () => {
      close();
      const err = await ctl.remove(c.id);
      if (err) toast(err);
    });
    app.appendChild(wrap);
    requestAnimationFrame(() => wrap.classList.add('in'));
  }

  // ── Shikoyat ──
  async function openMenu(c) {
    const want = await bottomPanel((sh, close) => {
      sh.innerHTML = `<div class="cm-menu-it">${icon('flag', { fill: false, size: 22, color: RED300 })}<span>Shikoyat qilish</span></div>`;
      sh.querySelector('.cm-menu-it').addEventListener('click', () => close(true));
    }, 'menu');
    if (want !== true || disposed) return;
    if (!currentUser()) { toast('Shikoyat yuborish uchun hisobingizga kiring'); return; }
    const sent = await bottomPanel((sh, close) => {
      sh.innerHTML = `<div class="cm-handle"></div>
        <div class="cm-rh">${icon('flag', { fill: false, size: 20, color: RED300 })}<span>Shikoyat qilish</span></div>
        <div class="cm-rd">Iltimos, shikoyat sababi haqida batafsil ma'lumot bering. Shikoyatni iloji boricha tezroq ko'rib chiqishga harakat qilamiz.</div>
        <div class="cm-rq"><div class="n">${esc(nameOf(c))}</div><div class="b">${richText(preview(c))}</div></div>
        <div class="cm-rta"><textarea rows="6" maxlength="${REPORT_MAX}" placeholder="Shikoyat sababini yozing..."></textarea><div class="cnt">0/${REPORT_MAX}</div></div>
        <div class="cm-rerr"></div>
        <div class="cm-rb"><button class="btn cm-cancel">Bekor qilish</button><button class="btn cm-go" disabled>Yuborish</button></div>`;
      const t = sh.querySelector('textarea');
      const go = sh.querySelector('.cm-go');
      const errEl = sh.querySelector('.cm-rerr');
      const cnt = sh.querySelector('.cnt');
      let busy = false;
      t.addEventListener('input', () => {
        cnt.textContent = `${t.value.length}/${REPORT_MAX}`;
        errEl.textContent = '';
        go.disabled = busy || t.value.trim().length < REPORT_MIN;
      });
      sh.querySelector('.cm-cancel').addEventListener('click', () => { if (!busy) close(false); });
      go.addEventListener('click', async () => {
        if (busy) return;
        busy = true; go.disabled = true; go.innerHTML = spinner(18, 2, '#fff');
        const err = await sendCommentReport(c.id, t.value);
        busy = false;
        if (err) { go.textContent = 'Yuborish'; go.disabled = false; errEl.textContent = err; return; }
        close(true);
      });
      setTimeout(() => t.focus(), 250);
    }, 'report');
    if (sent === true && !disposed) showReportDone();
  }

  function showReportDone() {
    clearTimeout(reportTimer);
    reportDone = true;
    bannerHost.innerHTML = `<div class="cm-banner">${icon('check_circle', { size: 20, color: C.success })}
      <div>Shikoyatingiz qabul qilindi.<br>Tez orada shikoyatingizni tekshirib chiqamiz.</div></div>`;
    reportTimer = setTimeout(() => { reportDone = false; if (!disposed) bannerHost.innerHTML = ''; }, 10000);
  }

  ctl.subs.add(render);
  paintExp();
  paintSend();
  ctl.loadFromDisk();
  render();
  ctl.load({ force: true });
  ctl.startWatching();

  return {
    get total() { return ctl.total; },
    get reportShown() { return reportDone; },
    onChange(fn) { changeSubs.add(fn); return () => changeSubs.delete(fn); },
    setExpanded(v) { isExpanded = !!v; paintExp(); },
    reload() { return ctl.load({ force: true }); },
    dispose() {
      disposed = true;
      clearTimeout(reportTimer);
      ctl.stopWatching();
      ctl.subs.clear();
      changeSubs.clear();
      emoji.close();
    },
  };
}
