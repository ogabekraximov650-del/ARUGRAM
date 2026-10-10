import { cacheWipe } from './tg/chunk-cache.js';
// Worker bilan aloqa.
//
// Ikki xil kirish (ikkalasi ham avtomatik, Telegram `initData` orqali —
// `worker/src/tma.rs`):
//   * `X-Tma` — 12 soatlik token: worker'ning "faqat ilovadan" eshigidan
//     (`app_gate`) o'tish uchun. Rasmlarda manzilga qo'shiladi (`?tma=`).
//   * `Authorization: Bearer <sessiya>` — bazadagi ACCOUNT (ilovadagi
//     Telegram orqali kirish bilan bir xil hisob). Ilova qaysi so'rovlarga
//     sessiya qo'shsa, sayt ham xuddi shularga qo'shadi.
//
// Sessiya localStorage'da saqlanadi va faqat yaroqsiz bo'lib qolganda
// (401) qayta so'raladi — har ochilishda Turso'ga yozuv ketmasin.

export const API_BASE = 'https://arugram.uzcom.workers.dev';

const TMA_KEY = 'aru_tma_token';
const SES_KEY = 'aru_session';

let token = '';
let tokenExp = 0;
let session = '';
let authing = null;
let me = null;
const listeners = new Set();

function lsGet(k) { try { return localStorage.getItem(k); } catch (_) { return null; } }
function lsSet(k, v) { try { if (v == null) localStorage.removeItem(k); else localStorage.setItem(k, v); } catch (_) { /* */ } }

(function loadSaved() {
  try {
    const o = JSON.parse(lsGet(TMA_KEY) || 'null');
    if (o && o.token && o.exp > Date.now() + 60_000 && o.uid === tgUserId()) {
      token = o.token;
      tokenExp = o.exp;
    }
  } catch (_) { /* */ }
  const s = JSON.parse(lsGet(SES_KEY) || 'null');
  if (s && s.token && s.uid === tgUserId()) session = s.token;
})();

export function initData() {
  return window.Telegram?.WebApp?.initData || '';
}

/** Telegram foydalanuvchisi (Telegram bergan, imzolangan). */
export function tgUser() {
  return window.Telegram?.WebApp?.initDataUnsafe?.user || null;
}

function tgUserId() {
  return tgUser()?.id || 0;
}

/** Bazadagi account (`/api/auth/me` -> `user`). */
export function currentUser() { return me; }
export function onUser(fn) { listeners.add(fn); return () => listeners.delete(fn); }
// Kesh egasi: boshqa hisob kirsa (yoki chiqilsa) avvalgi hisob keshi o'chadi.
function cacheOwner(u) {
  try {
    const cur = u?.id ? `${u.id}` : '';
    const was = localStorage.getItem('aru_cache_owner') || '';
    if (cur && was && was !== cur) cacheWipe();
    if (cur) localStorage.setItem('aru_cache_owner', cur);
  } catch (_) { /* */ }
}
function setMe(u) { me = u; cacheOwner(u); listeners.forEach((f) => { try { f(u); } catch (_) { /* */ } }); }

export class ApiError extends Error {
  constructor(status, body) {
    super(body?.error || `http_${status}`);
    this.status = status;
    this.body = body || {};
  }
}

/** Token (va kerak bo'lsa sessiya) oladi. */
export async function auth({ force = false, withSession = false } = {}) {
  const need = force || !token || tokenExp <= Date.now() + 60_000 || (withSession && !session);
  if (!need) return token;
  if (authing) return authing;
  authing = (async () => {
    const init = initData();
    if (!init) throw new ApiError(0, { error: 'no_telegram' });
    const r = await fetch(`${API_BASE}/api/tma/auth`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ init_data: init, session: withSession || !session }),
    });
    const j = await r.json().catch(() => ({}));
    if (!r.ok) throw new ApiError(r.status, j);
    token = j.token;
    tokenExp = Date.now() + (Number(j.expires_in) || 3600) * 1000;
    lsSet(TMA_KEY, JSON.stringify({ token, exp: tokenExp, uid: tgUserId() }));
    if (j.session) {
      session = j.session;
      lsSet(SES_KEY, JSON.stringify({ token: session, uid: tgUserId() }));
    }
    return token;
  })();
  try { return await authing; } finally { authing = null; }
}

/** Saytni ishga tushirish: token + account. Bloklangan bo'lsa — xato. */
export async function bootstrap() {
  await auth();
  try {
    const j = await api('/api/auth/me');
    setMe(j.user || null);
  } catch (e) {
    if (e.status === 401) {
      await auth({ force: true, withSession: true });
      const j = await api('/api/auth/me');
      setMe(j.user || null);
    } else {
      throw e;
    }
  }
  return me;
}

/** Account ma'lumotini serverdan qayta o'qiydi. */
export async function refreshMe() {
  const j = await api('/api/auth/me');
  setMe(j.user || null);
  return me;
}

export function sessionToken() { return session; }

/**
 * So'rov. `body` obyekt bo'lsa JSON qilib yuboriladi.
 * 403 (token eskirgan) -> token yangilanadi; 401 (sessiya o'chgan) ->
 * sessiya yangilanadi; ikkalasi ham bir martadan.
 */
export async function api(path, opts = {}) {
  let retried403 = false;
  let retried401 = false;
  for (;;) {
    const t = await auth();
    const headers = { ...(opts.headers || {}), 'X-Tma': t };
    if (session) headers.Authorization = `Bearer ${session}`;
    let body = opts.body;
    if (body && typeof body === 'object' && !(body instanceof FormData) && !(body instanceof Blob)
        && !(body instanceof ArrayBuffer) && !(body instanceof Uint8Array)) {
      body = JSON.stringify(body);
      headers['Content-Type'] = 'application/json';
    }
    const r = await fetch(`${API_BASE}${path}`, { ...opts, body, headers });
    if (r.status === 403 && !retried403) {
      retried403 = true;
      await auth({ force: true });
      continue;
    }
    if (r.status === 401 && !retried401 && !opts.noSessionRetry) {
      retried401 = true;
      await auth({ force: true, withSession: true });
      continue;
    }
    if (r.status === 204 || r.status === 304) return null;
    const text = await r.text();
    let j = null;
    try { j = text ? JSON.parse(text) : null; } catch (_) { j = { raw: text }; }
    if (!r.ok) throw new ApiError(r.status, j);
    return j;
  }
}

export const apiGet = (p) => api(p);
export const apiPost = (p, body) => api(p, { method: 'POST', body });

const PIX = 'data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7';
const tgOn = () => { try { return localStorage.getItem('aru_tg_on') === '1'; } catch (_) { return false; } };

/**
 * Worker rasmi (`/api/image/...`). Telegram ulangan bo'lsa fayl worker'dan
 * EMAS (ilovadagidek: worker orqali fayl o'tmaydi), Telegram'dan olinadi —
 * shu sabab bu yerda 1x1 rasm + fayl nomi qaytadi, `tg/images.js` uni
 * almashtiradi. Telegram'da bo'lmasa — oddiy manzil (zaxira).
 */
export function imageUrl(url) {
  const u = `${url ?? ''}`.trim();
  if (!u) return '';
  if (!u.includes('/api/') || !token) return u;
  const full = u + (u.includes('?') ? '&' : '?') + 'tma=' + encodeURIComponent(token);
  const m = /\/api\/(?:image|media)\/([^?#]+)/.exec(u);
  if (m && tgOn()) return `${PIX}#aru=${encodeURIComponent(decodeURIComponent(m[1]))}&o=${encodeURIComponent(full)}`;
  return full;
}

/** Saytdan chiqish: sessiya serverda o'chiriladi. */
export async function logout() {
  try { await api('/api/auth/logout', { method: 'POST', noSessionRetry: true }); } catch (_) { /* */ }
  session = '';
  lsSet(SES_KEY, null);
  try { localStorage.removeItem('aru_cache_owner'); } catch (_) { /* */ }
  await cacheWipe();
  setMe(null);
}
