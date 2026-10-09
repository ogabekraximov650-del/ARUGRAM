// SEVIMLI BO'LIMLAR — `lib/services/season_info.dart` -> `FavoritesService`.
//
//   favorites.items                       — bo'lim qatorlari (yangisi tepada)
//   favorites.isLoading
//   favorites.loadFromDisk()              — localStorage nusxasi (tarmoqsiz)
//   favorites.load({force})               — GET /api/favorites (60 s yangi)
//   favorites.isFav(animeId, seasonId)
//   favorites.set(animeId, seasonId, on, season?)
//       — navbatga (`putFavorite`) + ro'yxat DARHOL o'zgaradi
//       (ilovadagi `SeasonInfo.setFavorite` ning ro'yxatga tegishli qismi)
//   favorites.toggle(season) -> yangi holat (bool)
//   favorites.applyLocal(animeId, seasonId, on, season)  — faqat ro'yxat
//   favorites.markChanged()
//   favorites.listen(fn) -> unsubscribe
//   favorites.clear()

import { api, currentUser, onUser, sessionToken } from '../api.js';
import { putFavorite, pendingFavorites } from '../sync.js';

const toI = (v) => { const n = parseInt(`${v ?? 0}`, 10); return Number.isFinite(n) ? n : 0; };
const FRESH_FOR = 60_000;
const keyOf = (e) => `${toI(e.anime_id)}:${toI(e.season_id)}`;

class FavoritesService {
  constructor() {
    this._items = [];
    this._loadedAt = 0;
    this._loading = false;
    this._subs = new Set();
  }

  get _cacheKey() { return `aru_favorites_${currentUser()?.id ?? 0}`; }

  get items() { return this._items.slice(); }
  get isLoading() { return this._loading; }

  listen(fn) { this._subs.add(fn); return () => this._subs.delete(fn); }
  _emit() { this._subs.forEach((f) => { try { f(); } catch (e) { console.error(e); } }); }

  _save() {
    try { localStorage.setItem(this._cacheKey, JSON.stringify(this._items)); } catch (_) { /* */ }
  }

  loadFromDisk() {
    if (this._items.length) return;
    try {
      const rows = JSON.parse(localStorage.getItem(this._cacheKey) || 'null');
      if (!Array.isArray(rows) || !rows.length) return;
      this._items = rows.filter((r) => r && typeof r === 'object');
      this._emit();
    } catch (_) { /* */ }
  }

  async load({ force = false } = {}) {
    if (this._loading) return;
    if (!force && this._loadedAt && Date.now() - this._loadedAt < FRESH_FOR) return;
    if (!sessionToken()) {
      if (this._items.length) { this._items = []; this._emit(); }
      return;
    }
    this._loading = true;
    this._emit();
    try {
      const data = await api('/api/favorites');
      if (data && typeof data === 'object') {
        const fresh = (Array.isArray(data.items) ? data.items : []).filter((r) => r && typeof r === 'object');
        this._items = this._mergeLocal(fresh);
        this._loadedAt = Date.now();
        this._save();
      }
    } catch (_) {
      // Internet yo'q — diskdagi nusxa qoladi.
    }
    this._loading = false;
    this._emit();
  }

  markChanged() { this._loadedAt = 0; }

  isFav(animeId, seasonId) {
    const k = `${toI(animeId)}:${toI(seasonId)}`;
    const p = pendingFavorites().get(k);
    if (p !== undefined) return p;
    return this._items.some((e) => keyOf(e) === k);
  }

  applyLocal(animeId, seasonId, on, season = {}) {
    const a = toI(animeId); const s = toI(seasonId);
    const next = this._items.filter((e) => !(toI(e.anime_id) === a && toI(e.season_id) === s));
    if (on) next.unshift({ ...season, anime_id: a, season_id: s });
    this._items = next;
    this._loadedAt = 0;
    this._save();
    this._emit();
  }

  /** Sevimlilarga qo'shish / olib tashlash (navbat + ro'yxat). */
  set(animeId, seasonId, on, season = {}) {
    putFavorite(animeId, seasonId, on);
    this.applyLocal(animeId, seasonId, on, season);
  }

  toggle(season) {
    const on = !this.isFav(season.anime_id, season.season_id);
    this.set(season.anime_id, season.season_id, on, season);
    return on;
  }

  _mergeLocal(server) {
    const pend = pendingFavorites();
    if (!pend.size) return server;
    const out = server.filter((e) => pend.get(keyOf(e)) !== false);
    pend.forEach((on, k) => {
      if (!on) return;
      if (out.some((e) => keyOf(e) === k)) return;
      const local = this._items.find((e) => keyOf(e) === k);
      if (local) out.unshift(local);
    });
    return out;
  }

  clear() {
    this._items = [];
    this._loadedAt = 0;
    this._emit();
  }
}

export const favorites = new FavoritesService();

onUser((u) => { if (u) favorites.loadFromDisk(); });
if (currentUser()) favorites.loadFromDisk();
