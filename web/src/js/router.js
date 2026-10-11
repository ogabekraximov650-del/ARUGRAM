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

// Ekran to'liq ochilgach pastdagi sahifalar/navigatsiya yashiriladi (brauzer ularni chizmaydi) —
// shunda ekranlar silliq ishlaydi. Yopilishdan oldin darhol qaytariladi.
let coverT = 0;
function cover(on) {
  clearTimeout(coverT);
  const apply = () => {
    host.querySelectorAll(':scope > .pages, :scope > .bottom-nav-wrap').forEach((n) => { n.style.visibility = on ? 'hidden' : ''; });
    // Eng ustdagi ekran HAR DOIM ko'rinadi (u yopilgach pastdagisi ochiladi —
    // aks holda u `hidden` qolib, orqada profil sahifasi ko'rinib qolardi).
    stack.forEach((r, i) => { r.el.style.visibility = on && i < stack.length - 1 ? 'hidden' : ''; });
  };
  if (on) coverT = setTimeout(apply, 360); else apply();
}

function onTgBack() { back(); }

export function initRouter(root) {
  host = root;
  try { tg()?.BackButton?.onClick(onTgBack); } catch (_) { /* */ }
  // Brauzer (Telegram Desktop/Web) "orqaga" tugmasi.
  window.addEventListener('popstate', (ev) => {
    // Tarix yozuvi `{ aru: <stek chuqurligi> }` bilan belgilanadi. Brauzer shu
    // chuqurlikka qaytgan bo'lsa-yu, stek allaqachon shuncha bo'lsa (ekranni
    // o'zimiz yopib `history.back()` chaqirgan edik) — hech narsa qilinmaydi,
    // aks holda keyingi ekran ham yopilib, boshiga qaytardi. Vaqtga bog'liq emas.
    const target = Number(ev.state?.aru) || 0;
    while (stack.length > target) {
      const before = stack.length;
      back(true);
      if (stack.length >= before) break; // ekran yopilishga rozi emas (onBack)
    }
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
  cover(false);
  requestAnimationFrame(() => requestAnimationFrame(() => el.classList.add('in')));
  cover(true);
  syncBackButton();
  return route;
}

function remove(route, fromHistory = false) {
  const i = stack.indexOf(route);
  if (i < 0 || route.closed) return;
  route.closed = true;
  stack.splice(i, 1);
  cover(false);
  if (stack.length) cover(true);
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
