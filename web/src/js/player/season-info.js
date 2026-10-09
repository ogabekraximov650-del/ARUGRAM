// Bo'lim ma'lumoti — `lib/services/season_info.dart` (`SeasonInfo`,
// `SeasonService`) ning Mini App nusxasi.
//
// BITTA so'rov (`GET /api/season/:a/:s`): ko'rishlar, tomosha vaqti,
// sevimlilar soni, reyting va shu odamning O'Z bahosi / sevimlisi.
// Diskdagi (localStorage) nusxa darhol ko'rsatiladi, keyin yangisi.
// Yozuvlar (baho, sevimli) — faqat navbat orqali (`sync.js`), ilovadagidek.
//
//   seasonFromDisk(a, s) -> SeasonInfo | null
//   loadSeasonInfo(a, s) -> Promise<SeasonInfo | null>
//   rateSeason(a, s, stars, current) -> SeasonInfo
//   setSeasonFavorite(a, s, on, current) -> SeasonInfo

import { api } from '../api.js';
import { putRating, pendingFavorites, pendingRatings } from '../sync.js';
import { favorites } from '../services/favorites.js';

const num = (v) => { const n = Number(v); return Number.isFinite(n) ? Math.trunc(n) : 0; };

export class SeasonInfo {
  constructor({ season = {}, rating = 0, myStars = 0, isFav = false } = {}) {
    this.season = season;
    this.rating = rating;
    this.myStars = myStars;
    this.isFav = isFav;
  }

  _int(k) { return num(this.season[k]); }
  get views() { return this._int('views_total'); }
  get watchMs() { return this._int('watch_ms_total'); }
  get favCount() { return this._int('fav_count'); }
  get ratingCount() { return this._int('rating_count'); }
  get epizodCount() { return this._int('epizod_count'); }
  get bolimId() { return this._int('bolim_id'); }
  get createdAt() { return this._int('created_at'); }

  static fromJson(j) {
    return new SeasonInfo({
      season: j && typeof j.season === 'object' && j.season ? { ...j.season } : {},
      rating: Number(j?.rating) || 0,
      myStars: num(j?.my_stars),
      isFav: j?.is_fav === true,
    });
  }

  toJson() {
    return { season: this.season, rating: this.rating, my_stars: this.myStars, is_fav: this.isFav };
  }
}

const cacheKey = (a, s) => `aru_season_${a}_${s}`;

function applyStars(info, stars) {
  const oldStars = info.myStars;
  const oldCount = info.ratingCount;
  const oldSum = 'rating_sum' in info.season ? num(info.season.rating_sum) : Math.round(info.rating * oldCount);
  const count = oldCount + (oldStars > 0 ? 0 : 1);
  const sum = oldSum + stars - oldStars;
  const rating = count <= 0 ? 0 : Math.round((sum / count) * 100) / 100;
  const season = { ...info.season, rating_count: count, rating_sum: sum };
  return new SeasonInfo({ season, rating, myStars: stars, isFav: info.isFav });
}

/** Navbatda turgan (hali yuborilmagan) baho va sevimli ustidan qo'yiladi. */
function withPending(a, s, info) {
  const key = `${a}:${s}`;
  let out = info;
  try {
    const fav = pendingFavorites().get(key);
    if (fav !== undefined && fav !== out.isFav) {
      const season = { ...out.season, fav_count: Math.max(0, out.favCount + (fav ? 1 : -1)) };
      out = new SeasonInfo({ season, rating: out.rating, myStars: out.myStars, isFav: fav });
    }
    const stars = pendingRatings().get(key);
    if (stars !== undefined && stars !== out.myStars) out = applyStars(out, stars);
  } catch (_) { /* */ }
  return out;
}

function saveDisk(a, s, info) {
  try { localStorage.setItem(cacheKey(a, s), JSON.stringify(info.toJson())); } catch (_) { /* */ }
}

export function seasonFromDisk(a, s) {
  try {
    const j = JSON.parse(localStorage.getItem(cacheKey(a, s)) || 'null');
    if (!j || typeof j !== 'object') return null;
    return withPending(a, s, SeasonInfo.fromJson(j));
  } catch (_) { return null; }
}

export async function loadSeasonInfo(a, s) {
  try {
    const j = await api(`/api/season/${a}/${s}`);
    if (!j || typeof j !== 'object') return seasonFromDisk(a, s);
    try { localStorage.setItem(cacheKey(a, s), JSON.stringify(j)); } catch (_) { /* */ }
    return withPending(a, s, SeasonInfo.fromJson(j));
  } catch (_) {
    return seasonFromDisk(a, s);
  }
}

/** Baho: ekranda DARHOL, serverga navbat orqali. */
export function rateSeason(a, s, stars, current) {
  const next = applyStars(current, stars);
  saveDisk(a, s, next);
  putRating(a, s, stars);
  return next;
}

/** Sevimlilar: ekranda DARHOL, serverga navbat orqali (`favorites.set`). */
export function setSeasonFavorite(a, s, on, current) {
  const count = current.isFav === on ? current.favCount : Math.max(0, current.favCount + (on ? 1 : -1));
  const season = { ...current.season, fav_count: count };
  const next = new SeasonInfo({ season, rating: current.rating, myStars: current.myStars, isFav: on });
  saveDisk(a, s, next);
  favorites.set(a, s, on, season);
  return next;
}
