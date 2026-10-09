// Sahifalar ustiga ochiladigan ekranlar (Flutter `Navigator.push`).
//
//   push((el, route) => { ...; return { dispose() {} } }, { transition })
//
// `el` — butun ekranni egallaydigan bo'sh div (fon `--bg`). Builder
// qaytargan obyektdagi `dispose` ekran yopilganda chaqiriladi,
// `onBack` (bo'lsa) — Orqaga bosilganda (false qaytarsa yopilmaydi).
// Telegram'ning "Orqaga" tugmasi stek bo'sh bo'lmaganda ko'rinadi.

const stack = [];
let host = null;

const tg = () => window.Telegram?.WebApp;

function syncBackButton() {
  const bb = tg()?.BackButton;
  if (!bb) return;
  try { if (stack.length) bb.show(); else bb.hide(); } catch (_) { /* */ }
}

function onTgBack() { back(); }

export function initRouter(root) {
  host = root;
  try { tg()?.BackButton?.onClick(onTgBack); } catch (_) { /* */ }
  // Brauzer (Telegram Desktop/Web) "orqaga" tugmasi.
  window.addEventListener('popstate', () => {
    if (stack.length) back(true);
  });
}

/**
 * Yangi ekran. `transition`: 'fade' (pleyer — `openSeasonFromAnywhere`),
 * 'slide' (oddiy `MaterialPageRoute`), 'none'.
 */
export function push(builder, { transition = 'slide' } = {}) {
  const el = document.createElement('div');
  el.className = `route route-${transition}`;
  host.appendChild(el);
  const route = { el, closed: false, api: null, close: () => remove(route) };
  stack.push(route);
  try { history.pushState({ aru: stack.length }, ''); } catch (_) { /* */ }
  route.api = builder(el, route) || {};
  requestAnimationFrame(() => requestAnimationFrame(() => el.classList.add('in')));
  syncBackButton();
  return route;
}

function remove(route, fromHistory = false) {
  const i = stack.indexOf(route);
  if (i < 0 || route.closed) return;
  route.closed = true;
  stack.splice(i, 1);
  try { route.api?.dispose?.(); } catch (_) { /* */ }
  route.el.classList.remove('in');
  route.el.classList.add('out');
  setTimeout(() => route.el.remove(), 260);
  if (!fromHistory) { try { history.back(); } catch (_) { /* */ } }
  syncBackButton();
}

/** Eng ustdagi ekranni yopadi (uning `onBack` i rozi bo'lsa). */
export function back(fromHistory = false) {
  const top = stack[stack.length - 1];
  if (!top) return false;
  if (top.api?.onBack && top.api.onBack() === false) {
    if (fromHistory) { try { history.pushState({ aru: stack.length }, ''); } catch (_) { /* */ } }
    return true;
  }
  remove(top, fromHistory);
  return true;
}

export function pop() { return back(); }
export function depth() { return stack.length; }
export function top() { return stack[stack.length - 1] || null; }
