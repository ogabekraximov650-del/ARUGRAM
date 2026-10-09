// Worker bilan aloqa.
//
// Kirish: Telegram bergan `initData` -> `POST /api/tma/auth` -> qisqa
// token (`worker/src/tma.rs`). Keyingi so'rovlar tokenni `X-Tma`
// sarlavhasida, rasmlar esa manzilda (`?tma=`) yuboradi.

export const API_BASE = 'https://arugram.uzcom.workers.dev';

const KEY = 'aru_tma_token';
let token = '';
let tokenExp = 0;
let authing = null;

function loadSaved() {
  try {
    const raw = sessionStorage.getItem(KEY);
    if (!raw) return;
    const o = JSON.parse(raw);
    if (o && o.token && o.exp > Date.now() + 60_000) {
      token = o.token;
      tokenExp = o.exp;
    }
  } catch (_) { /* xotira yo'q — har safar so'raladi */ }
}

function save() {
  try { sessionStorage.setItem(KEY, JSON.stringify({ token, exp: tokenExp })); } catch (_) { /* */ }
}

export function initData() {
  return window.Telegram?.WebApp?.initData || '';
}

/** Token oladi (bor bo'lsa — o'sha). Telegram'dan tashqarida — xato. */
export async function auth(force = false) {
  if (!force && token && tokenExp > Date.now() + 60_000) return token;
  if (authing) return authing;
  authing = (async () => {
    const init = initData();
    if (!init) throw new Error('no_telegram');
    const r = await fetch(`${API_BASE}/api/tma/auth`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ init_data: init }),
    });
    if (!r.ok) throw new Error(`auth_${r.status}`);
    const j = await r.json();
    token = j.token;
    tokenExp = Date.now() + (Number(j.expires_in) || 3600) * 1000;
    save();
    return token;
  })();
  try {
    return await authing;
  } finally {
    authing = null;
  }
}

/** GET/POST — JSON. 403 bo'lsa token yangilanib bir marta qayta urinadi. */
export async function api(path, opts = {}) {
  for (let attempt = 0; attempt < 2; attempt++) {
    const t = await auth(attempt > 0);
    const r = await fetch(`${API_BASE}${path}`, {
      ...opts,
      headers: { ...(opts.headers || {}), 'X-Tma': t },
    });
    if (r.status === 403 && attempt === 0) continue;
    if (!r.ok) throw new Error(`http_${r.status}`);
    return r.json();
  }
  throw new Error('forbidden');
}

/** Worker rasmi (`/api/image/...`) — tokenni manzilga qo'shadi. */
export function imageUrl(url) {
  const u = `${url ?? ''}`.trim();
  if (!u) return '';
  if (!u.includes('/api/') || !token) return u;
  return u + (u.includes('?') ? '&' : '?') + 'tma=' + encodeURIComponent(token);
}

loadSaved();
