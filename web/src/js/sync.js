// YAGONA YOZUV NAVBATI — `lib/services/sync_queue.dart` ning web nusxasi.
//
// Qoidalar ilovadagidek (batafsil sabablar — Dart faylida):
//   1. Hamma yozuv avval telefonda (localStorage), ekran darhol o'zgaradi.
//   2. Navbat siqiladi: har yozuvning `key` i bor (`h:a:s:e`, `r:a:s`,
//      `f:a:s`, `settings`, `p:...`) — o'sha kalit navbatda bo'lsa eskisi
//      almashtiriladi. "Birinchi ko'rish" (`new_view`) belgisi YO'QOLMAYDI.
//   3. Yuborish shartlari: yangi yozuvdan 1 daqiqa keyin, sahifa
//      yashirilganda (fon) darhol; ikki paket orasi kamida 2 daqiqa;
//      kuniga oddiy 24 ta, qat'iy 50 ta paket.
// Har paketda bir martalik `batch_id` — worker takrorni tanib oladi.
//
// ── ILOVADAN FARQI ────────────────────────────────────────────────
//   * Trafik hisobi (`TrafficService`) yo'q: saytda MTProto trafigi
//     sanalmaydi. `traffic_bytes` har doim 0 bo'lib ketadi (worker
//     maydonni ixtiyoriy deb o'qiydi), "faqat trafik — 6 soatda bir"
//     sharti ham kerak emas.
//   * Paketdagi qatorlar worker chegarasiga moslangan: tarix 100, baho,
//     sevimli va to'plam amallari 50 tadan (`MAX_SYNC_HISTORY`,
//     `MAX_SYNC_SMALL`). Oshsa worker butun paketni 413 bilan rad etardi.
//   * Sahifa yashirilganda / yopilganda so'rov `keepalive` bilan ketadi.
//   * Navbat localStorage'da, hisob (account id) bo'yicha alohida.

import { api, currentUser, sessionToken } from './api.js';

export const SyncKind = {
  history: 'history',
  rating: 'rating',
  favorite: 'favorite',
  pack: 'pack',
  settings: 'settings',
};

/** `flush()` natijasi (ilovadagi `SyncResult`). */
export const SyncResult = { done: 'done', offline: 'offline', noAccount: 'noAccount' };

const NORMAL_PER_DAY = 24;
const HARD_PER_DAY = 50;
const MAX_ROWS_PER_BATCH = 150;
const MAX_HISTORY_PER_BATCH = 100; // worker: MAX_SYNC_HISTORY
const MAX_SMALL_PER_BATCH = 50; // worker: MAX_SYNC_SMALL
const MAX_QUEUE_ROWS = 1000;
const WRITE_DELAY = 60_000;
const MIN_GAP = 120_000;
const TIMEOUT = 25_000;
const KEEPALIVE_MAX = 60_000; // brauzer keepalive tanasi ~64 KB gacha

let rows = [];
let sentToday = 0;
let day = '';
let lastSentAt = 0;
let sending = false;
let loadedFor = null;
let delayed = 0;
let started = false;
const subs = new Set();
const doneSubs = new Set();
const beforeBg = new Set();

function uid() { return currentUser()?.id ?? 0; }
const queueKey = () => `aru_sync_queue_${uid()}`;
const stateKey = () => `aru_sync_state_${uid()}`;

function lsRead(k) {
  try { return JSON.parse(localStorage.getItem(k) || 'null'); } catch (_) { return null; }
}
function lsWrite(k, v) {
  try { localStorage.setItem(k, JSON.stringify(v)); } catch (_) { /* to'la / taqiqlangan */ }
}

function emit() { subs.forEach((f) => { try { f(); } catch (e) { console.error(e); } }); }

/** Navbatni (shu hisobniki) o'qiydi. Hisob almashsa qaytadan. */
function load() {
  const u = uid();
  if (loadedFor === u) return;
  loadedFor = u;
  rows = [];
  sentToday = 0;
  day = '';
  lastSentAt = 0;
  const q = lsRead(queueKey());
  if (Array.isArray(q)) {
    for (const e of q) if (e && typeof e.key === 'string' && typeof e.kind === 'string') rows.push(e);
  }
  const st = lsRead(stateKey());
  if (st && typeof st === 'object') {
    day = typeof st.day === 'string' ? st.day : '';
    sentToday = Number(st.sent) || 0;
    lastSentAt = Number(st.last_at) || 0;
  }
}

function saveQueue() { lsWrite(queueKey(), rows); }
function saveState() { lsWrite(stateKey(), { day, sent: sentToday, last_at: lastSentAt }); }

/** Mahalliy yarim tunda hisoblagich nolga tushadi. */
function rollDay() {
  const d = new Date();
  const today = `${d.getFullYear()}-${d.getMonth() + 1}-${d.getDate()}`;
  if (day !== today) {
    day = today;
    sentToday = 0;
    saveState();
  }
}

function later(ms) {
  clearTimeout(delayed);
  delayed = setTimeout(() => { maybeFlush('kechikkan'); }, ms);
}

const toI = (v) => { const n = parseInt(`${v ?? 0}`, 10); return Number.isFinite(n) ? n : 0; };

function put(kind, key, data, op = 'set') {
  load();
  const at = Date.now();
  rows = rows.filter((e) => e.key !== key);
  rows.push({ kind, key, op, at, data });
  while (rows.length > MAX_QUEUE_ROWS) rows.shift();
  saveQueue();
  emit();
  maybeFlush('yozuv');
}

function find(key) { return rows.find((e) => e.key === key) || null; }

// ── NAVBATGA QO'SHISH ───────────────────────────────────────────

/**
 * Tomosha tarixi yozuvi. `row` — `watchHistory` tayyorlagan qator
 * (snake_case: anime_id, season_id, epizod_id, video_url, last_quality,
 * position_ms, duration_ms, watched_ms, new_view).
 */
export function putHistory(row) {
  const a = toI(row?.anime_id);
  const s = toI(row?.season_id);
  const e = toI(row?.epizod_id);
  if (a <= 0 || e <= 0) return;
  const key = `h:${a}:${s}:${e}`;
  const prev = find(key);
  const prevNew = !!prev && prev.op === 'set' && prev.data?.new_view === true;
  put(SyncKind.history, key, {
    anime_id: a,
    season_id: s,
    epizod_id: e,
    video_url: `${row.video_url ?? ''}`,
    last_quality: `${row.last_quality ?? ''}`,
    position_ms: toI(row.position_ms),
    duration_ms: toI(row.duration_ms),
    watched_ms: toI(row.watched_ms),
    new_view: row.new_view === true || prevNew,
    updated_at: Date.now(),
  });
}

/** Tarixdan yashirish (serverda o'chirilmaydi, belgilanadi). */
export function hideHistory(animeId, seasonId, epizodId) {
  const a = toI(animeId); const s = toI(seasonId); const e = toI(epizodId);
  if (a <= 0 || e <= 0) return;
  put(SyncKind.history, `h:${a}:${s}:${e}`, {
    anime_id: a, season_id: s, epizod_id: e, deleted: true, updated_at: Date.now(),
  }, 'delete');
}

/** Baho (1..10). */
export function putRating(animeId, seasonId, stars) {
  const a = toI(animeId); const s = toI(seasonId); const st = toI(stars);
  if (a <= 0 || st < 1 || st > 10) return;
  put(SyncKind.rating, `r:${a}:${s}`, { anime_id: a, season_id: s, stars: st, updated_at: Date.now() });
}

/** Sevimlilarga qo'shish / olib tashlash. */
export function putFavorite(animeId, seasonId, on) {
  const a = toI(animeId); const s = toI(seasonId);
  if (a <= 0) return;
  put(SyncKind.favorite, `f:${a}:${s}`, { anime_id: a, season_id: s, on: !!on, updated_at: Date.now() });
}

/** Emoji/GIF/stiker to'plami amali (`p:new:<id>`, `p:add:<fayl>` ...). */
export function putPack(key, data) { put(SyncKind.pack, key, data); }

/** Sozlamalarning TO'LIQ holati (kalit bitta — `settings`). */
export function putSettings(hiddenStats, chanConsent) {
  put(SyncKind.settings, 'settings', {
    hidden_stats: Array.isArray(hiddenStats) ? hiddenStats : [],
    chan_consent: !!chanConsent,
  });
}

/** Navbatdan bitta yozuvni (kalit bo'yicha) olib tashlaydi. */
export function removeKey(key) {
  load();
  const n = rows.length;
  rows = rows.filter((e) => e.key !== key);
  if (rows.length === n) return;
  saveQueue();
  emit();
}

// ── YUBORILMAGAN YOZUVLAR (ro'yxatlar server javobiga qo'yadi) ──

export function hasPendingSettings() { load(); return rows.some((e) => e.kind === SyncKind.settings); }

export function pendingPacks() {
  load();
  return rows.filter((e) => e.kind === SyncKind.pack && e.data && typeof e.data === 'object')
    .map((e) => ({ ...e.data }));
}

/** `'anime:season:epizod'` -> yashirilganmi (`true` — o'chirilgan). */
export function pendingHistory() {
  load();
  const out = new Map();
  for (const e of rows) {
    if (e.kind !== SyncKind.history || !e.data) continue;
    const d = e.data;
    out.set(`${d.anime_id}:${d.season_id}:${d.epizod_id}`, e.op === 'delete');
  }
  return out;
}

/** `'anime:season'` -> yoqilganmi. */
export function pendingFavorites() {
  load();
  const out = new Map();
  for (const e of rows) {
    if (e.kind !== SyncKind.favorite || !e.data) continue;
    out.set(`${e.data.anime_id}:${e.data.season_id}`, e.data.on === true);
  }
  return out;
}

/** `'anime:season'` -> yulduzlar soni. */
export function pendingRatings() {
  load();
  const out = new Map();
  for (const e of rows) {
    if (e.kind !== SyncKind.rating || !e.data) continue;
    const st = toI(e.data.stars);
    if (st > 0) out.set(`${e.data.anime_id}:${e.data.season_id}`, st);
  }
  return out;
}

export function pendingCount() { load(); return rows.length; }
export function isSending() { return sending; }
export function sentTodayCount() { load(); return sentToday; }

/** Navbat o'zgarganda. Qaytaradi: obunani bekor qilish funksiyasi. */
export function listenSync(fn) { subs.add(fn); return () => subs.delete(fn); }

/** Paket muvaffaqiyatli ketgach (ilovada — `MyStatsService.load(force)`). */
export function onSyncDone(fn) { doneSubs.add(fn); return () => doneSubs.delete(fn); }

/**
 * Sahifa fonga ketishidan OLDIN chaqiriladi (masalan pleyerdagi joriy
 * tarix yozuvini navbatga qo'yish uchun). Qaytaradi: bekor qilish.
 */
export function beforeBackground(fn) { beforeBg.add(fn); return () => beforeBg.delete(fn); }

// ── YUBORISH SHARTLARI ──────────────────────────────────────────

const hasPackRows = () => rows.some((e) => e.kind === SyncKind.pack);

/**
 * Shartlar bajarilgan bo'lsa yuboradi. `reason`: 'yozuv' | 'fon' |
 * 'ochilish' | 'kechikkan'.
 */
export async function maybeFlush(reason, { keepalive = false } = {}) {
  load();
  rollDay();
  if (sending) return;
  if (rows.length === 0) return;
  const packs = hasPackRows();
  if (sentToday >= (packs ? HARD_PER_DAY : NORMAL_PER_DAY)) return;
  const since = Date.now() - lastSentAt;
  if (reason === 'yozuv') { later(WRITE_DELAY); return; }
  const wait = MIN_GAP - since;
  if (wait > 0) { later(wait); return; }
  clearTimeout(delayed);
  await flush({ force: packs, keepalive });
}

/**
 * Navbatni serverga yuboradi (kunlik qat'iy chegara buzilmaydi).
 * `onStep(step, progress)` — ixtiyoriy progress.
 * Qaytaradi: `SyncResult` qiymati.
 */
export async function flush({ force = false, onStep = null, keepalive = false } = {}) {
  load();
  rollDay();
  if (sending) return SyncResult.offline;
  if (!sessionToken() || uid() <= 0) return SyncResult.noAccount;
  if (sentToday >= HARD_PER_DAY) return SyncResult.offline;
  if (!force && sentToday >= NORMAL_PER_DAY) return SyncResult.offline;

  sending = true;
  emit();
  try {
    if (rows.length === 0) {
      onStep?.('Hammasi saqlangan', 1);
      return SyncResult.done;
    }
    let ok = true;
    let guard = 0;
    while (rows.length && guard < 5 && sentToday < HARD_PER_DAY) {
      guard++;
      const share = 0.15 + 0.75 * (guard === 1 ? 0.6 : 1);
      onStep?.("Ma'lumotlar yuborilmoqda", Math.min(0.95, share));
      // eslint-disable-next-line no-await-in-loop
      ok = await sendOnce(keepalive);
      if (!ok) break;
      // keepalive: sahifa yopilyapti — bitta paket yetadi.
      if (keepalive) break;
    }
    if (ok) {
      onStep?.('Hammasi saqlandi', 1);
      return SyncResult.done;
    }
    onStep?.("Internet yo'q", 1);
    return SyncResult.offline;
  } finally {
    sending = false;
    emit();
  }
}

async function sendOnce(keepalive) {
  const batch = [];
  let nHist = 0; let nRate = 0; let nFav = 0; let nPack = 0;
  for (const e of rows) {
    if (batch.length >= MAX_ROWS_PER_BATCH) break;
    if (e.kind === SyncKind.history) { if (nHist >= MAX_HISTORY_PER_BATCH) continue; nHist++; }
    else if (e.kind === SyncKind.rating) { if (nRate >= MAX_SMALL_PER_BATCH) continue; nRate++; }
    else if (e.kind === SyncKind.favorite) { if (nFav >= MAX_SMALL_PER_BATCH) continue; nFav++; }
    else if (e.kind === SyncKind.pack) { if (nPack >= MAX_SMALL_PER_BATCH) continue; nPack++; }
    batch.push(e);
  }
  const history = []; const ratings = []; const favorites = []; const packs = [];
  let settings = null;
  for (const e of batch) {
    const data = { ...(e.data || {}) };
    switch (e.kind) {
      case SyncKind.history: history.push(data); break;
      case SyncKind.rating: ratings.push(data); break;
      case SyncKind.favorite: favorites.push(data); break;
      case SyncKind.pack: packs.push(data); break;
      case SyncKind.settings: settings = data; break;
      default: break;
    }
  }
  const body = {
    batch_id: newBatchId(),
    history,
    ratings,
    favorites,
    packs,
    traffic_bytes: 0,
  };
  if (settings) body.settings = settings;
  const text = JSON.stringify(body);

  const ctl = typeof AbortController !== 'undefined' ? new AbortController() : null;
  const timer = ctl ? setTimeout(() => ctl.abort(), TIMEOUT) : 0;
  let status = 0;
  try {
    await api('/api/sync', {
      method: 'POST',
      body: text,
      headers: { 'Content-Type': 'application/json' },
      keepalive: keepalive && text.length < KEEPALIVE_MAX,
      signal: ctl?.signal,
    });
    status = 200;
  } catch (e) {
    status = Number(e?.status) || 0; // 0 — tarmoq yo'q / vaqt tugadi
  } finally {
    clearTimeout(timer);
  }
  if (status === 0) return false; // navbat joyida qoladi

  // Har qanday javob — rad etilgan bo'lsa ham — so'rov sifatida sanaladi.
  sentToday++;
  lastSentAt = Date.now();
  saveState();

  if (status >= 200 && status < 300) {
    dropSent(batch);
    saveQueue();
    doneSubs.forEach((f) => { try { f(); } catch (_) { /* */ } });
    emit();
    return true;
  }
  if (status === 401) return false;
  if (status >= 400 && status < 500) {
    // Server rad etdi — abadiy qayta yubormaymiz.
    dropSent(batch);
    saveQueue();
    emit();
    return false;
  }
  return false;
}

/** Yuborilganlarni olib tashlaydi — kalit VA vaqt (`at`) bo'yicha. */
function dropSent(batch) {
  const sent = new Map();
  for (const e of batch) sent.set(`${e.key}`, Number(e.at) || 0);
  rows = rows.filter((e) => {
    const at = sent.get(`${e.key}`);
    if (at === undefined) return true;
    return (Number(e.at) || 0) > at;
  });
}

function newBatchId() {
  return `${Date.now()}-${Math.floor(Math.random() * 0x7fffffff)}`;
}

// ── HODISALAR ───────────────────────────────────────────────────

function onBackground() {
  beforeBg.forEach((f) => { try { f(); } catch (e) { console.error(e); } });
  maybeFlush('fon', { keepalive: true });
}

/**
 * Hozir yuborishga urinadi — ilova fonga ketgandagi kabi (shartlar:
 * 2 daqiqalik oraliq va kunlik chegara saqlanadi). Pleyerdan chiqishda
 * chaqirsa bo'ladi.
 */
export function flushNow() {
  return maybeFlush('fon', { keepalive: document.visibilityState === 'hidden' });
}

/** Hisobdan chiqilganda / o'chirilganda — navbat tashlanadi. */
export function wipe() {
  load();
  rows = [];
  sentToday = 0;
  lastSentAt = 0;
  saveQueue();
  saveState();
  emit();
}

/** `app.js` bir marta chaqiradi (bootstrap'dan keyin). */
export function startSync() {
  if (started) return;
  started = true;
  load();
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'hidden') onBackground();
    else maybeFlush('ochilish');
  });
  window.addEventListener('pagehide', onBackground);
  maybeFlush('ochilish');
}
