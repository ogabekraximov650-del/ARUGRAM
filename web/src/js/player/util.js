// Pleyer ekrani yordamchilari — `lib/services/format.dart`,
// `lib/services/intro_times.dart`, `seasons_repo.dart -> seasonIsFree`.

import { formatCount } from '../format.js';
import { seasonsRepo } from '../seasons.js';

const two = (v) => String(v).padStart(2, '0');

/** Tomosha vaqti: `1:59` (soat:daqiqa), soat uch xonadan ajratiladi. */
export function formatHours(ms) {
  const total = Math.floor(Math.round(Number(ms) || 0) / 1000);
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  return `${formatCount(hours)}:${two(minutes)}`;
}

/** `12:34/01/01/2026` — soat/kun/oy/yil (telefon mintaqasida). */
export function formatMoment(ms) {
  const n = Number(ms) || 0;
  if (n <= 0) return '';
  const d = new Date(n);
  return `${two(d.getHours())}:${two(d.getMinutes())}/${two(d.getDate())}/${two(d.getMonth() + 1)}/${d.getFullYear()}`;
}

/** Reyting: `07.62`. */
export function formatRating(value) {
  const v = Math.min(10, Math.max(0, Number(value) || 0));
  const t = v.toFixed(2);
  return t.length < 5 ? `0${t}` : t;
}

export function formatBytes(bytes) {
  const b = Number(bytes) || 0;
  if (b < 1024) return `${Math.round(b)} B`;
  const units = ['KB', 'MB', 'GB', 'TB', 'PB'];
  let v = b / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; }
  const t = v >= 100 ? v.toFixed(0) : (v >= 10 ? v.toFixed(1) : v.toFixed(2));
  return `${t.replace('.', ',')} ${units[i]}`;
}

/** Bazadagi `size_*` (bayt yoki eski matn). */
export function fileSizeLabel(v) {
  if (typeof v === 'number') return v > 0 ? formatBytes(v) : '';
  const t = `${v ?? ''}`.trim();
  if (/^-?\d+$/.test(t)) { const n = parseInt(t, 10); return n > 0 ? formatBytes(n) : ''; }
  return t;
}

/** Pleyer vaqti: `m:ss` (ilovadagi `_fmt`). */
export function fmtDur(ms) {
  const s = Math.max(0, Math.floor((Number(ms) || 0) / 1000));
  return `${Math.floor(s / 60)}:${two(s % 60)}`;
}

// ── Intro vaqtlari (`intro_times.dart`) ─────────────────────────────

const INTRO_SLOTS = 10;

export function introMs(raw) {
  const t = `${raw ?? ''}`.trim();
  if (!t) return 0;
  if (!t.includes(':')) {
    const sec = parseInt(t, 10) || 0;
    return sec > 0 ? sec * 1000 : 0;
  }
  const parts = t.split(':');
  if (parts.length > 3) return 0;
  let total = 0;
  for (const p of parts) {
    if (!/^\s*\d+\s*$/.test(p)) return 0;
    total = total * 60 + parseInt(p, 10);
  }
  return total > 0 ? total * 1000 : 0;
}

/** `[[fromMs, toMs], ...]` */
export function introRangesOf(ep) {
  if (!ep) return [];
  const out = [];
  for (let i = 1; i < INTRO_SLOTS; i += 2) {
    const from = introMs(ep[`intro_${i}`]);
    const to = introMs(ep[`intro_${i + 1}`]);
    if (from <= 0 || to <= from) continue;
    out.push([from, to]);
  }
  return out;
}

// ── Bepul / pullik (`seasons_repo.dart`) ────────────────────────────

const toI = (v) => { const n = parseInt(`${v ?? 0}`, 10); return Number.isFinite(n) ? n : 0; };

export function seasonIsFree(s) {
  if (typeof s?.free === 'boolean') return s.free;
  const r = seasonsRepo.find(toI(s?.anime_id), toI(s?.season_id));
  return r?.free === true;
}

export function seasonIsPaid(s) {
  if (typeof s?.free === 'boolean') return !s.free;
  const r = seasonsRepo.find(toI(s?.anime_id), toI(s?.season_id));
  return typeof r?.free === 'boolean' ? !r.free : false;
}

// ── Pleyer sozlamalari (`app_settings.dart`: intro / keyingi qism) ──

const SET_KEY = 'aru_player_settings';

function readSettings() {
  try { return JSON.parse(localStorage.getItem(SET_KEY) || 'null') || {}; } catch (_) { return {}; }
}

export const playerSettings = {
  get autoSkipIntro() { return readSettings().auto_skip_intro === true; },
  get autoNextEpisode() { return readSettings().auto_next_episode === true; },
  set(k, v) {
    const o = readSettings();
    o[k] = !!v;
    try { localStorage.setItem(SET_KEY, JSON.stringify(o)); } catch (_) { /* */ }
  },
  setAutoSkipIntro(v) { this.set('auto_skip_intro', v); },
  setAutoNextEpisode(v) { this.set('auto_next_episode', v); },
};
