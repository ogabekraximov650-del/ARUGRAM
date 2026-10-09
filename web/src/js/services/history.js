// TOMOSHA TARIXI va TO'XTAGAN JOY — `lib/services/watch_history.dart`
// va `lib/services/watch_progress.dart` ning onlayn qismi.
//
// ── PLEYER UCHUN API (Dart'dagi nomlar bilan bir xil) ─────────────
// Vaqtlar HAMMA JOYDA millisekundda (Dart'dagi `Duration` o'rniga).
//
//   watchHistory.loadFromDisk()                  — localStorage nusxasi (tarmoqsiz)
//   watchHistory.load({force})                   — GET /api/history (60 s yangi)
//   watchHistory.items                           — HistoryItem[] (oxirgisi tepada)
//   watchHistory.isLoading
//   watchHistory.listen(fn) -> unsubscribe
//   watchHistory.startEpisode({animeId, seasonId, epizodId, epizodNumber,
//       videoUrl, thumbUrl?, quality?, bolimId?, animeName?, seasonName?,
//       animePhoto?, seasonPhoto?})              — pleyer qism ochganda
//   watchHistory.note(positionMs, durationMs)   — har soniya (faqat xotira)
//   watchHistory.addWatched(deltaMs)            — 1x tezlikda ijro bo'lgan vaqt
//   watchHistory.flush()                         — navbatga (SyncQueue) qo'yadi;
//       pleyerdan chiqishda, qism almashganda, fonga ketganda (fon — avtomatik)
//   watchHistory.prewarmThumb()                  — saytda hech narsa qilmaydi
//   watchHistory.lastOfSeason(animeId, seasonId) -> HistoryItem|null
//   watchHistory.findEpisode(animeId, seasonId, epizodId) -> HistoryItem|null
//   watchHistory.qualityOf(animeId, seasonId, epizodId) -> '720p' | ''
//   watchHistory.remove(item)                    — tarixdan yashirish
//   watchHistory.byAnime / watchHistory.episodesOf(animeId)
//   watchHistory.clear()
//
//   watchProgress.positionOf(url) -> ms | null
//   watchProgress.save(url, positionMs, durationMs)
//   watchProgress.forget(url)
//   watchProgress.flush()
//   watchProgress.minPositionFor(durationMs) / endMarginFor(durationMs)
//
// HistoryItem maydonlari (camelCase, Dart'dagidek): animeId, seasonId,
// bolimId, epizodId, epizodNumber, animeName, seasonName, animePhoto,
// seasonPhoto, videoUrl, thumbUrl, lastQuality, positionMs, durationMs,
// watchedMs, viewCount, updatedAt; getterlar: progress (0..1), percent,
// poster, title, bolimNumber; sameEpisode(a,s,e).
//
// To'xtagan kadrlar (`_prepareThumb`, `thumbnail`, `peekThumb`) — faqat
// ilovada (Rust yadrosi + Android kadr ajratuvchisi). Saytda ro'yxat
// bo'lim posterini ko'rsatadi.

import { api, currentUser, onUser, sessionToken } from '../api.js';
import { putHistory, hideHistory, pendingHistory, maybeFlush, beforeBackground } from '../sync.js';

const toI = (v) => { const n = parseInt(`${v ?? 0}`, 10); return Number.isFinite(n) ? n : 0; };
const str = (v) => `${v ?? ''}`;
const uid = () => currentUser()?.id ?? 0;

function lsRead(k) {
  try { return JSON.parse(localStorage.getItem(k) || 'null'); } catch (_) { return null; }
}
function lsWrite(k, v) {
  try { localStorage.setItem(k, JSON.stringify(v)); } catch (_) { /* */ }
}

export class HistoryItem {
  constructor(o) {
    this.animeId = o.animeId;
    this.seasonId = o.seasonId;
    this.bolimId = o.bolimId || 0;
    this.epizodId = o.epizodId;
    this.epizodNumber = o.epizodNumber || 0;
    this.animeName = o.animeName || '';
    this.seasonName = o.seasonName || '';
    this.animePhoto = o.animePhoto || '';
    this.seasonPhoto = o.seasonPhoto || '';
    this.videoUrl = o.videoUrl || '';
    this.thumbUrl = o.thumbUrl || '';
    this.lastQuality = o.lastQuality || '';
    this.positionMs = o.positionMs || 0;
    this.durationMs = o.durationMs || 0;
    this.watchedMs = o.watchedMs || 0;
    this.viewCount = o.viewCount || 0;
    this.updatedAt = o.updatedAt || 0;
  }

  get progress() {
    if (this.durationMs <= 0) return 0;
    const r = this.positionMs / this.durationMs;
    if (Number.isNaN(r) || r < 0) return 0;
    return r > 1 ? 1 : r;
  }

  get percent() { return this.progress * 100; }
  get poster() { return this.seasonPhoto || this.animePhoto; }
  get title() {
    if (this.seasonName.trim()) return this.seasonName.trim();
    if (this.animeName.trim()) return this.animeName.trim();
    return 'Anime';
  }

  get bolimNumber() { return this.bolimId > 0 ? this.bolimId : this.seasonId; }

  sameEpisode(a, s, id) {
    return this.animeId === toI(a) && this.seasonId === toI(s) && this.epizodId === toI(id);
  }

  toJson() {
    return {
      anime_id: this.animeId,
      season_id: this.seasonId,
      bolim_id: this.bolimId,
      epizod_id: this.epizodId,
      epizod_number: this.epizodNumber,
      anime_name: this.animeName,
      season_name: this.seasonName,
      anime_photo: this.animePhoto,
      season_photo: this.seasonPhoto,
      video_url: this.videoUrl,
      thumb_url: this.thumbUrl,
      last_quality: this.lastQuality,
      position_ms: this.positionMs,
      duration_ms: this.durationMs,
      watched_ms: this.watchedMs,
      view_count: this.viewCount,
      updated_at: this.updatedAt,
    };
  }

  static fromJson(j) {
    return new HistoryItem({
      animeId: toI(j.anime_id),
      seasonId: toI(j.season_id),
      bolimId: toI(j.bolim_id),
      epizodId: toI(j.epizod_id),
      epizodNumber: toI(j.epizod_number),
      animeName: str(j.anime_name),
      seasonName: str(j.season_name),
      animePhoto: str(j.anime_photo),
      seasonPhoto: str(j.season_photo),
      videoUrl: str(j.video_url),
      thumbUrl: str(j.thumb_url),
      lastQuality: str(j.last_quality),
      positionMs: toI(j.position_ms),
      durationMs: toI(j.duration_ms),
      watchedMs: toI(j.watched_ms),
      viewCount: toI(j.view_count),
      updatedAt: toI(j.updated_at),
    });
  }
}

const keyOf = (e) => `${e.animeId}:${e.seasonId}:${e.epizodId}`;
const byTime = (a, b) => b.updatedAt - a.updatedAt;

/** Eski (epizod_id siz) qatorlar tashlanadi. */
function fromRows(list) {
  const out = [];
  for (const r of list || []) {
    if (!r || typeof r !== 'object') continue;
    const it = HistoryItem.fromJson(r);
    if (it.epizodId > 0) out.push(it);
  }
  return out;
}

const FRESH_FOR = 60_000;
const MIN_FLUSH_MS = 1000;

class WatchHistory {
  constructor() {
    this._items = [];
    this._loadedAt = 0;
    this._loading = false;
    this._loadedForUser = 0;
    this._pending = null;
    this._flushedStamp = '';
    this._pendingNewView = false;
    this._subs = new Set();
  }

  get _listKey() { return `aru_watch_history_${uid()}`; }
  get _nowKey() { return `${this._listKey}_now`; }

  get items() { return this._items.slice(); }
  get isLoading() { return this._loading; }

  listen(fn) { this._subs.add(fn); return () => this._subs.delete(fn); }
  _emit() { this._subs.forEach((f) => { try { f(); } catch (e) { console.error(e); } }); }

  // ── HOZIR KO'RILAYOTGAN QISM ──────────────────────────────────

  startEpisode({
    animeId, seasonId, epizodId, epizodNumber = 0, videoUrl = '', thumbUrl = '', quality = '',
    bolimId = 0, animeName = '', seasonName = '', animePhoto = '', seasonPhoto = '',
  } = {}) {
    const a = toI(animeId); const s = toI(seasonId); const e = toI(epizodId);
    if (a <= 0 || e <= 0) return;
    const prev = this._pending;
    if (prev && (prev.anime_id !== a || prev.epizod_id !== e || prev.season_id !== s)) this.flush();
    const before = this.findEpisode(a, s, e);
    const same = !!prev && prev.anime_id === a && prev.season_id === s && prev.epizod_id === e;
    if (!same) this._pendingNewView = true;
    const carried = same ? (prev.watched_ms || 0) : 0;
    const saved = before?.watchedMs || 0;
    if (!same) this._flushedStamp = '';
    this._pending = {
      anime_id: a,
      season_id: s,
      bolim_id: toI(bolimId),
      epizod_id: e,
      epizod_number: toI(epizodNumber),
      last_quality: str(quality),
      watched_ms: carried > saved ? carried : saved,
      anime_name: str(animeName),
      season_name: str(seasonName),
      anime_photo: str(animePhoto),
      season_photo: str(seasonPhoto),
      video_url: str(videoUrl),
      thumb_url: str(thumbUrl),
      position_ms: 0,
      duration_ms: 0,
    };
  }

  /** Joriy nuqta (ms). Faqat xotira. */
  note(positionMs, durationMs) {
    const p = this._pending;
    if (!p || !(durationMs > 0)) return;
    p.position_ms = Math.round(positionMs) || 0;
    p.duration_ms = Math.round(durationMs);
  }

  /** Saytda kadr yasalmaydi (faqat ilovada). */
  prewarmThumb() {}

  /** 1x tezlikda haqiqatan ko'rilgan vaqt (ms); qism uzunligidan oshmaydi. */
  addWatched(deltaMs) {
    const p = this._pending;
    if (!p || !(deltaMs > 0)) return;
    const duration = p.duration_ms || 0;
    let total = (p.watched_ms || 0) + Math.round(deltaMs);
    if (duration > 0 && total > duration) total = duration;
    p.watched_ms = total;
  }

  /** Kutayotgan yozuvni ro'yxatga va navbatga qo'yadi (istalgancha chaqirsa bo'ladi). */
  flush() {
    const p = this._pending;
    if (!p) return;
    const duration = p.duration_ms || 0;
    const position = p.position_ms || 0;
    if (duration <= 0 || position < MIN_FLUSH_MS) return;
    const watched = p.watched_ms || 0;
    const stamp = `${position}/${watched}/${p.last_quality}`;
    if (!this._pendingNewView && stamp === this._flushedStamp) return;
    this._flushedStamp = stamp;
    const row = { ...p, updated_at: Date.now(), new_view: this._pendingNewView };
    this._pendingNewView = false;
    this._applyLocal(row);
    putHistory(row);
  }

  _applyLocal(row) {
    const a = toI(row.anime_id); const s = toI(row.season_id); const e = toI(row.epizod_id);
    if (a <= 0 || e <= 0) return;
    const list = this._items.slice();
    const at = list.findIndex((x) => x.sameEpisode(a, s, e));
    const old = at >= 0 ? list[at] : this._anyOf(a, s);
    const pick = (fresh, saved) => (str(fresh) ? str(fresh) : (saved || ''));
    const item = new HistoryItem({
      animeId: a,
      seasonId: s,
      bolimId: toI(row.bolim_id) > 0 ? toI(row.bolim_id) : (old?.bolimId || 0),
      epizodId: e,
      epizodNumber: toI(row.epizod_number) > 0 ? toI(row.epizod_number) : (old?.epizodNumber || 0),
      animeName: pick(row.anime_name, old?.animeName),
      seasonName: pick(row.season_name, old?.seasonName),
      animePhoto: pick(row.anime_photo, old?.animePhoto),
      seasonPhoto: pick(row.season_photo, old?.seasonPhoto),
      videoUrl: pick(row.video_url, old?.videoUrl),
      thumbUrl: pick(row.thumb_url, old?.thumbUrl),
      lastQuality: pick(row.last_quality, old?.lastQuality),
      positionMs: toI(row.position_ms),
      durationMs: toI(row.duration_ms),
      watchedMs: toI(row.watched_ms),
      viewCount: (old?.viewCount || 0) + (row.new_view === true ? 1 : 0),
      updatedAt: toI(row.updated_at) > 0 ? toI(row.updated_at) : Date.now(),
    });
    if (at >= 0) list[at] = item; else list.push(item);
    list.sort(byTime);
    this._items = list;
    this._loadedForUser = uid() || this._loadedForUser;
    this._saveDisk();
    this._emit();
  }

  _anyOf(a, s) {
    return this._items.find((e) => e.animeId === a && e.seasonId === s)
      || this._items.find((e) => e.animeId === a) || null;
  }

  _saveDisk() { lsWrite(this._listKey, this._items.map((e) => e.toJson())); }

  // ── SERVER ────────────────────────────────────────────────────

  async load({ force = false } = {}) {
    if (this._loading) return;
    if (!force && this._loadedAt && Date.now() - this._loadedAt < FRESH_FOR) return;
    if (!sessionToken()) {
      if (this._items.length) { this._items = []; this._loadedForUser = 0; this._emit(); }
      return;
    }
    const userId = uid();
    if (userId !== this._loadedForUser) {
      this._items = [];
      this._loadedAt = 0;
      this._loadedForUser = userId;
    }
    this._loading = true;
    this._emit();

    try { await maybeFlush('ochilish'); } catch (_) { /* */ }

    let fresh = null;
    try {
      const base = lsRead(this._listKey);
      const mark = lsRead(this._nowKey);
      const srvNow = toI(mark?.t);
      const fullAt = toI(mark?.f);
      const nowMs = Date.now();
      const canDelta = Array.isArray(base) && srvNow > 0 && nowMs - fullAt < 24 * 3600_000;
      const since = srvNow - 30000;
      const data = await api(`/api/history${canDelta ? `?since=${since}` : ''}`);
      const rows = (Array.isArray(data?.items) ? data.items : []).filter((m) => m && typeof m === 'object');
      const isDelta = data?.delta === true && canDelta;
      if (isDelta) {
        const byKey = new Map();
        for (const m of base) byKey.set(`${m.anime_id}:${m.season_id}:${m.epizod_id}`, m);
        for (const m of rows) {
          const k = `${m.anime_id}:${m.season_id}:${m.epizod_id}`;
          const hidden = toI(m.deleted_at) !== 0 || toI(m.gone) === 1;
          if (hidden) byKey.delete(k);
          else {
            const old = byKey.get(k);
            byKey.set(k, { ...m, ...(old && old.thumb_url != null ? { thumb_url: old.thumb_url } : {}) });
          }
        }
        fresh = fromRows([...byKey.values()]);
      } else {
        fresh = rows.map((r) => HistoryItem.fromJson(r));
      }
      const t = toI(data?.now);
      if (t > 0) lsWrite(this._nowKey, { t, f: isDelta ? fullAt : nowMs });
      this._loadedAt = Date.now();
    } catch (_) {
      // Pastda diskdagi nusxaga tushamiz.
    }

    if (!fresh) {
      const cached = lsRead(this._listKey);
      if (Array.isArray(cached)) fresh = fromRows(cached);
    }
    if (fresh) {
      fresh = this._mergeLocal(fresh);
      fresh.sort(byTime);
      this._items = fresh;
      this._loadedForUser = userId;
      this._saveDisk();
    }
    this._loading = false;
    this._emit();
  }

  /** Diskdagi (localStorage) nusxa — TARMOQSIZ. */
  loadFromDisk() {
    const userId = uid();
    if (userId === 0) return;
    if (this._items.length && this._loadedForUser === userId) return;
    const cached = lsRead(this._listKey);
    if (!Array.isArray(cached) || !cached.length) return;
    const rows = fromRows(cached).sort(byTime);
    if (!rows.length) return;
    this._items = rows;
    this._loadedForUser = userId;
    this._emit();
  }

  /** Server ro'yxati ustiga yuborilmagan o'zgarishlar. */
  _mergeLocal(server) {
    const pend = pendingHistory();
    if (!pend.size) return server;
    const localOf = (k) => this._items.find((e) => keyOf(e) === k) || null;
    const out = [];
    for (const e of server) {
      const op = pend.get(keyOf(e));
      if (op === true) continue;
      out.push(op === false ? (localOf(keyOf(e)) || e) : e);
    }
    pend.forEach((hidden, k) => {
      if (hidden) return;
      if (out.some((e) => keyOf(e) === k)) return;
      const local = localOf(k);
      if (local) out.push(local);
    });
    return out;
  }

  qualityOf(animeId, seasonId, epizodId) {
    return this.findEpisode(animeId, seasonId, epizodId)?.lastQuality || '';
  }

  findEpisode(animeId, seasonId, epizodId) {
    return this._items.find((e) => e.sameEpisode(animeId, seasonId, epizodId)) || null;
  }

  lastOfSeason(animeId, seasonId) {
    const a = toI(animeId); const s = toI(seasonId);
    return this._items.find((e) => e.animeId === a && e.seasonId === s) || null;
  }

  /** Yozuvni ro'yxatdan darhol olib tashlaydi; serverda yashiriladi. */
  remove(item) {
    this._items = this._items.filter((e) => !e.sameEpisode(item.animeId, item.seasonId, item.epizodId));
    this._saveDisk();
    this._emit();
    hideHistory(item.animeId, item.seasonId, item.epizodId);
  }

  /** `remove` ning ID bilan varianti. */
  hide(animeId, seasonId, epizodId) {
    const it = this.findEpisode(animeId, seasonId, epizodId);
    if (it) this.remove(it);
    else hideHistory(animeId, seasonId, epizodId);
  }

  clear() {
    this._items = [];
    this._loadedAt = 0;
    this._loadedForUser = 0;
    this._pending = null;
    this._emit();
  }

  /** Har anime uchun eng so'nggi ko'rilgan qism. */
  get byAnime() {
    const seen = new Set();
    const out = [];
    for (const it of this._items) {
      if (!seen.has(it.animeId)) { seen.add(it.animeId); out.push(it); }
    }
    return out;
  }

  episodesOf(animeId) {
    const a = toI(animeId);
    return this._items.filter((e) => e.animeId === a);
  }
}

// ── TO'XTAGAN JOY (WatchProgress) ────────────────────────────────

const MAX_ENTRIES = 300;
const MAX_MIN_POSITION = 15_000;
const MAX_END_MARGIN = 30_000;
const SHORT_SHARE = 0.10;

class WatchProgress {
  constructor() {
    this._positions = new Map();
    this._loadedFor = null;
    this._dirty = false;
  }

  get _key() { return `aru_watch_positions_${uid()}`; }

  minPositionFor(durationMs) { return Math.min(durationMs * SHORT_SHARE, MAX_MIN_POSITION); }
  endMarginFor(durationMs) { return Math.min(durationMs * SHORT_SHARE, MAX_END_MARGIN); }

  ensureLoaded() {
    const u = uid();
    if (this._loadedFor === u) return;
    this._loadedFor = u;
    this._positions = new Map();
    this._dirty = false;
    const rows = lsRead(this._key);
    if (!Array.isArray(rows)) return;
    for (const r of rows) {
      if (r && typeof r.u === 'string' && r.u && typeof r.ms === 'number' && r.ms > 0) {
        this._positions.set(r.u, Math.round(r.ms));
      }
    }
  }

  /** Saqlangan nuqta (ms) yoki `null`. */
  positionOf(url) {
    if (!url) return null;
    this.ensureLoaded();
    const ms = this._positions.get(url);
    return ms && ms > 0 ? ms : null;
  }

  /** Nuqtani eslab qoladi (boshida yoki oxirida bo'lsa — o'chiradi). */
  save(url, positionMs, durationMs) {
    if (!url || !(durationMs > 0)) return;
    this.ensureLoaded();
    const atEnd = durationMs - positionMs <= this.endMarginFor(durationMs);
    if (positionMs < this.minPositionFor(durationMs) || atEnd) {
      if (this._positions.delete(url)) { this._dirty = true; this.flush(); }
      return;
    }
    const ms = Math.round(positionMs);
    const old = this._positions.get(url);
    if (old != null && Math.abs(old - ms) < 900) return;
    this._positions.set(url, ms);
    this._dirty = true;
    this.flush();
  }

  reload() { this._loadedFor = null; this.ensureLoaded(); }

  forget(url) {
    this.ensureLoaded();
    if (this._positions.delete(url)) { this._dirty = true; this.flush(); }
  }

  flush() {
    if (!this._dirty) return;
    this._dirty = false;
    let rows = [...this._positions.entries()].map(([u, ms]) => ({ u, ms }));
    if (rows.length > MAX_ENTRIES) rows = rows.slice(rows.length - MAX_ENTRIES);
    lsWrite(this._key, rows);
  }
}

export const watchHistory = new WatchHistory();
export const watchProgress = new WatchProgress();

// Ilova ochilganda/kirilganda diskdagi nusxa darhol o'qiladi
// (`lastOfSeason` so'rovsiz ishlashi uchun).
onUser((u) => { if (u) watchHistory.loadFromDisk(); });
if (currentUser()) watchHistory.loadFromDisk();

// Fonga ketishdan oldin joriy qism holati navbatga tushsin.
beforeBackground(() => { watchHistory.flush(); watchProgress.flush(); });
