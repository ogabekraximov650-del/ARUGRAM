// Admin bilan yozishma — `lib/services/support_service.dart` (onlayn).
//
//   unreadBadge — o'qilmagan xabarlar belgisi (profil tugmasi, pastki
//     panel nuqtasi). FAQAT sonni so'raydi (`/api/chat/unread`):
//       unreadBadge.refresh(), .count, .has, .hasForUser, .hasForAdmin,
//       .isAdmin, .listen(fn) -> unsubscribe, .markRead(), .clear()
//   ChatController — bitta suhbat (foydalanuvchining o'zi; admin
//     ko'rinishi bu yerda kerak emas, lekin `userId` qo'llanadi):
//       loadFromDisk(), load({force}), startPolling(), stopPolling(),
//       send(body, {mediaFile, mediaType, mediaMs}), listen(fn).
//   chatTime(ms), fullTime(ms)
//
// "Diskdagi nusxa" ilovadagidek localStorage'da (darhol ko'rinadi).
// Uzoq kutish (`/api/chat/wait`) — Telegram Bot API usuli: server javobni
// yangi xabar paydo bo'lguncha ushlab turadi.

import { api, currentUser, onUser } from '../api.js';

function lsRead(k) {
  try { return JSON.parse(localStorage.getItem(k) || 'null'); } catch (_) { return null; }
}
function lsWrite(k, v) {
  try { localStorage.setItem(k, JSON.stringify(v)); } catch (_) { /* */ }
}
const num = (v) => { const n = Number(v); return Number.isFinite(n) ? Math.trunc(n) : 0; };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** Server javobini ilovadagi `ChatMessage` ko'rinishiga keltiradi. */
export function chatMessage(j) {
  return {
    id: `${j?.id ?? ''}`,
    fromAdmin: j?.from_admin === true,
    body: `${j?.body ?? ''}`,
    createdAt: num(j?.created_at),
    mediaUrl: `${j?.media_url ?? ''}`,
    mediaType: `${j?.media_type ?? ''}`,
    mediaMs: num(j?.media_ms),
    pending: false,
    seen: j?.seen === true,
  };
}
const toJson = (m) => ({
  id: m.id, from_admin: m.fromAdmin, body: m.body, created_at: m.createdAt,
  media_url: m.mediaUrl, media_type: m.mediaType, media_ms: m.mediaMs, seen: m.seen,
});

export const hasMedia = (m) => !!(m.mediaUrl && m.mediaType);
export const isVoice = (m) => m.mediaType === 'voice';
export const isInline = (m) => m.mediaType === 'sticker' || m.mediaType === 'gif' || m.mediaType === 'round';
export const isViewable = (m) => hasMedia(m) && !isVoice(m) && !isInline(m);

/** `.../api/media/<nom>?...` -> `<nom>` (`TelegramService.fileNameOf`). */
export function fileNameOf(url) {
  const last = `${url ?? ''}`.split('?')[0].split('/').pop() || '';
  return /^[A-Za-z0-9._-]+$/.test(last) && last !== '.' && last !== '..' ? last : '';
}

// ══════════════════════════════════════════════════════════════
//  O'QILMAGAN XABARLAR BELGISI
// ══════════════════════════════════════════════════════════════

const badgeSubs = new Set();

export const unreadBadge = {
  count: 0,
  _admin: false,
  _busy: false,
  _srvCount: -1,
  _mk: -1,
  _at: 0,

  get has() { return this.count > 0; },
  /** Sanoq admin uchunmi (BARCHA suhbatlar yig'indisi). */
  get isAdmin() { return this._admin; },
  /** Oddiy foydalanuvchining o'qilmagan xabari bormi (profil nuqtasi). */
  get hasForUser() { return this.count > 0 && !this._admin; },
  get hasForAdmin() { return this.count > 0 && this._admin; },

  listen(fn) { badgeSubs.add(fn); return () => badgeSubs.delete(fn); },
  _emit() { badgeSubs.forEach((f) => { try { f(); } catch (e) { console.error(e); } }); },

  clear() {
    this._srvCount = -1; this._mk = -1; this._at = 0;
    if (this.count === 0) return;
    this.count = 0;
    this._emit();
  },

  /** Xabarlar o'qildi — nuqta darhol o'chsin. */
  markRead() {
    if (this.count === 0) return;
    this.count = 0;
    this._emit();
  },

  async refresh() {
    if (this._busy) return;
    if (!currentUser()) { this.clear(); return; }
    this._busy = true;
    try {
      const j = await api(`/api/chat/unread?u=${this._srvCount}&mk=${this._mk}&at=${this._at}`);
      const n = num(j?.unread);
      this._srvCount = n;
      this._mk = j?.mk == null ? -1 : num(j.mk);
      this._at = num(j?.at);
      const adm = j?.admin === true;
      if (n !== this.count || adm !== this._admin) {
        this.count = n;
        this._admin = adm;
        this._emit();
      }
    } catch (_) { /* jim: nuqta eski holicha */ }
    this._busy = false;
  },
};

let badgeUid = currentUser()?.id ?? null;
onUser((u) => {
  const uid = u?.id ?? null;
  if (uid !== badgeUid) { badgeUid = uid; unreadBadge.clear(); }
});

// ══════════════════════════════════════════════════════════════
//  BITTA SUHBAT
// ══════════════════════════════════════════════════════════════

export class ChatController {
  constructor({ userId = null } = {}) {
    this.userId = userId;
    this.items = [];
    this._loading = false;
    this.hasData = false;
    this.error = null;
    this._ver = 0;
    this._mk = -1;
    this._at = 0;
    this._watching = false;
    this._subs = new Set();
    this._abort = null;
  }

  get isAdminView() { return this.userId != null; }
  get isLoading() { return this._loading && this.items.length === 0; }
  get _url() { return this.userId == null ? '/api/chat' : `/api/chat/thread/${this.userId}`; }
  get _diskKey() { return `aru_chat_${currentUser()?.id ?? 0}_${this.userId ?? 0}`; }
  get _verKey() { return `aru_chatver_${currentUser()?.id ?? 0}_${this.userId ?? 0}`; }

  listen(fn) { this._subs.add(fn); return () => this._subs.delete(fn); }
  _emit() { this._subs.forEach((f) => { try { f(); } catch (e) { console.error(e); } }); }

  get _lastAt() { return this.items.length ? this.items[this.items.length - 1].createdAt : 0; }
  get _seenCount() { return this.items.filter((m) => m.seen).length; }
  get _liveCount() { return this.items.filter((m) => !m.pending).length; }
  get _oldestAt() {
    let oldest = 0;
    for (const m of this.items) {
      if (m.pending) continue;
      if (oldest === 0 || m.createdAt < oldest) oldest = m.createdAt;
    }
    return oldest;
  }

  /** Diskdagi nusxani DARHOL ko'rsatadi. */
  loadFromDisk() {
    if (this.items.length) return;
    const rows = lsRead(this._diskKey);
    if (!Array.isArray(rows)) return;
    this.items = rows.map(chatMessage);
    this._ver = num(lsRead(this._verKey)?.v);
    this.hasData = true;
    this._emit();
  }

  _saveDisk() {
    lsWrite(this._diskKey, this.items.filter((m) => !m.pending).map(toJson));
  }

  startPolling() {
    if (this._watching) return;
    this._watching = true;
    this._watchLoop();
  }

  stopPolling() {
    this._watching = false;
    try { this._abort?.abort(); } catch (_) { /* */ }
  }

  async _watchLoop() {
    while (this._watching) {
      if (!currentUser()) { await sleep(5000); continue; }
      // Mini App fonda — worker'ga so'rov yuborilmaydi.
      if (document.visibilityState === 'hidden') { await sleep(2000); continue; }
      try {
        const q = `ver=${this._ver}&mk=${this._mk}&at=${this._at}&since=${this._lastAt}`
          + `&seen=${this._seenCount}&count=${this._liveCount}&oldest=${this._oldestAt}`
          + `${this.userId != null ? `&user_id=${this.userId}` : ''}`;
        const ctl = typeof AbortController !== 'undefined' ? new AbortController() : null;
        this._abort = ctl;
        const timer = setTimeout(() => { try { ctl?.abort(); } catch (_) { /* */ } }, 35_000);
        let j;
        try {
          j = await api(`/api/chat/wait?${q}`, ctl ? { signal: ctl.signal } : {});
        } finally {
          clearTimeout(timer);
        }
        if (!this._watching) return;
        const ver = j?.ver == null ? null : num(j.ver);
        this._mk = j?.mk == null ? -1 : num(j.mk);
        this._at = num(j?.at);
        if (j?.new === true) await this.load({ force: true });
        if (ver != null && ver !== this._ver) {
          this._ver = ver;
          lsWrite(this._verKey, { v: ver });
        }
        continue;
      } catch (e) {
        if (!this._watching) return;
        await sleep(e && e.status > 0 ? 3000 : 2000);
      }
    }
  }

  async load({ force = false } = {}) {
    if (this._loading) return;
    if (this.hasData && !force) return;
    if (!currentUser()) {
      this.error = 'Avval hisobingizga kiring';
      this._emit();
      return;
    }
    this._loading = true;
    if (!this.hasData) this._emit();
    const since = this.hasData ? this._lastAt : 0;
    const wasLoaded = this.hasData;
    const oldError = this.error;
    let changed = false;
    try {
      const j = await api(since > 0 ? `${this._url}?since=${since}` : this._url);
      const raw = Array.isArray(j?.items) ? j.items : [];
      const rows = raw.map(chatMessage);

      // Eski xabarlarning "o'qildi" belgisi.
      const seenIds = new Set((Array.isArray(j?.seen_ids) ? j.seen_ids : []).map((e) => `${e}`));
      if (seenIds.size) {
        this.items = this.items.map((m) => {
          if (!m.seen && seenIds.has(m.id)) { changed = true; return { ...m, seen: true }; }
          return m;
        });
      }

      // Admin o'chirgan xabarlar.
      let removed = false;
      if (Array.isArray(j?.all_ids)) {
        const live = new Set(j.all_ids.map((e) => `${e}`));
        const before = this.items.length;
        this.items = this.items.filter((m) => m.pending || live.has(m.id));
        removed = this.items.length !== before;
        if (removed) changed = true;
      }

      if (since > 0) {
        const have = new Set(this.items.map((m) => m.id));
        const fresh = rows.filter((m) => !have.has(m.id));
        if (fresh.length) {
          this.items = this.items.filter((m) => !m.pending).concat(fresh);
          changed = true;
        }
        if (fresh.length || removed) this._saveDisk();
      } else if (removed || rows.length !== this.items.length
          || (rows.length && this.items.length && rows[rows.length - 1].id !== this.items[this.items.length - 1].id)) {
        this.items = rows;
        changed = true;
        lsWrite(this._diskKey, raw);
      }
      this.hasData = true;
      this.error = null;
      unreadBadge.markRead();
    } catch (e) {
      if (e && e.status > 0) this.error = `Yuklanmadi (${e.status})`;
      else if (!this.hasData) this.error = "Internet yo'q";
    }
    this._loading = false;
    if (changed || !wasLoaded || this.error !== oldError) this._emit();
  }

  /** Xabar yuboradi. Xato bo'lsa matn qaytadi. */
  async send(body, { mediaFile = '', mediaType = '', mediaMs = 0 } = {}) {
    const text = `${body ?? ''}`.trim();
    if (!text && !mediaFile) return null;
    const tempId = `tmp${Date.now()}${Math.floor(Math.random() * 1000)}`;
    if (!mediaFile) {
      this.items = this.items.concat([{
        id: tempId, fromAdmin: this.isAdminView, body: text, createdAt: Date.now(),
        mediaUrl: '', mediaType: '', mediaMs: 0, pending: true, seen: false,
      }]);
      this.hasData = true;
      this._emit();
    }
    const drop = () => { this.items = this.items.filter((m) => m.id !== tempId); };
    try {
      const j = await api('/api/chat', {
        method: 'POST',
        body: {
          body: text,
          ...(this.userId != null ? { user_id: this.userId } : {}),
          ...(mediaFile ? { media_file: mediaFile } : {}),
          ...(mediaType ? { media_type: mediaType } : {}),
          ...(mediaMs > 0 ? { media_ms: mediaMs } : {}),
        },
      });
      drop();
      const saved = chatMessage(j);
      if (!this.items.some((m) => m.id === saved.id)) this.items = this.items.concat([saved]);
      this.hasData = true;
      this._saveDisk();
      this._emit();
      return null;
    } catch (e) {
      drop();
      this._emit();
      if (e && e.status > 0) return `${e.body?.error ?? 'Yuborilmadi'}`;
      return "Internet yo'q — qaytadan urinib ko'ring";
    }
  }

  dispose() { this.stopPolling(); this._subs.clear(); }
}

const two = (n) => String(n).padStart(2, '0');

/** `14:32 · 12.09.2026` */
export function fullTime(ms) {
  if (ms <= 0) return '';
  const d = new Date(ms);
  return `${two(d.getHours())}:${two(d.getMinutes())} · ${two(d.getDate())}.${two(d.getMonth() + 1)}.${d.getFullYear()}`;
}

/** Telegram'dagidek qisqa vaqt: bugun — soat, aks holda sana. */
export function chatTime(ms) {
  if (ms <= 0) return '';
  const d = new Date(ms);
  const now = new Date();
  if (d.getFullYear() === now.getFullYear() && d.getMonth() === now.getMonth() && d.getDate() === now.getDate()) {
    return `${two(d.getHours())}:${two(d.getMinutes())}`;
  }
  if (d.getFullYear() === now.getFullYear()) return `${two(d.getDate())}.${two(d.getMonth() + 1)}`;
  return `${two(d.getDate())}.${two(d.getMonth() + 1)}.${d.getFullYear()}`;
}
