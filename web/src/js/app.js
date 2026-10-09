// ARUmediaTV — Telegram Mini App. Ilovaning onlayn nusxasi.
//
// Tuzilishi `lib/screens/root_screen.dart` dagidek: 5 ta sahifa bir
// vaqtda quriladi (IndexedStack), pastki panel faqat qaysi biri
// ko'rinishini almashtiradi. Boshqa ekranlar (pleyer, sozlamalar ...)
// `router.js` orqali ustiga ochiladi.
//
// Tab ekranlari: `createX(pageEl) -> { onShow?(), onHide?() }`.

import { bootstrap } from './api.js';
import { createBottomNav } from './nav.js';
import { createHome } from './home.js';
import { initRouter } from './router.js';
import { hooks } from './hooks.js';
import { createSearch } from './screens/search.js';
import { createCatalog } from './screens/catalog.js';
import { createLibrary } from './screens/library.js';
import { createProfile } from './screens/profile.js';
import { openSeason, openSeasonIds } from './player/player-screen.js';
import { openPublicProfile } from './screens/public-profile.js';
import { startSync } from './sync.js';
import { channelGate } from './services/channel-gate.js';
import { checkTelegram, ensureTelegram } from './tg/media.js';
import { watchImages, rescanImages } from './tg/images.js';
import { unreadBadge, ChatController } from './services/support.js';

const tg = window.Telegram?.WebApp;
const app = document.getElementById('app');

function setupTelegram() {
  if (!tg) return;
  try {
    tg.ready();
    tg.expand();
    tg.setHeaderColor?.('#0A0A0C');
    tg.setBackgroundColor?.('#0A0A0C');
    tg.setBottomBarColor?.('#0A0A0C');
    // Ro'yxatni surganda Mini App yopilib ketmasin.
    tg.disableVerticalSwipes?.();
    tg.enableClosingConfirmation?.();
  } catch (_) { /* eski Telegram — e'tiborsiz */ }
}

function gate(html) {
  app.innerHTML = `<div class="gate"><img src="assets/aru-mark.png" alt="ARU"><div class="msg">${html}</div></div>`;
}

async function main() {
  setupTelegram();
  if (!tg?.initData) {
    gate("ARUmediaTV'ni Telegram ichida oching:<br>botga /start yuboring va <b>«ARUmediaTV'ni ochish»</b> tugmasini bosing.");
    return;
  }
  try {
    await bootstrap();
  } catch (e) {
    if (e?.body?.error === 'banned') {
      gate(`🚫 ${(e.body.message || 'Hisobingiz bloklangan').replace(/</g, '&lt;')}<br><br>Savollaringiz bo'lsa adminga yozing.`);
    } else {
      gate("Ulanib bo'lmadi. Internetni tekshirib, Mini App'ni qayta oching.");
    }
    return;
  }

  initRouter(app);
  hooks.openSeason = openSeason;
  hooks.openSeasonIds = openSeasonIds;
  hooks.openUser = openPublicProfile;

  const pagesEl = document.createElement('div');
  pagesEl.className = 'pages';
  app.appendChild(pagesEl);
  const pages = [0, 1, 2, 3, 4].map((i) => {
    const p = document.createElement('div');
    p.className = 'page' + (i === 0 ? ' active' : '');
    pagesEl.appendChild(p);
    return p;
  });

  const tabs = [
    createHome(pages[0]),
    createSearch(pages[1]),
    createCatalog(pages[2]),
    createLibrary(pages[3]),
    createProfile(pages[4]),
  ];
  let current = 0;

  const nav = createBottomNav(app, (i) => {
    if (i === current) return;
    tabs[current]?.onHide?.();
    current = i;
    pages.forEach((p, k) => p.classList.toggle('active', k === i));
    tabs[i]?.onShow?.();
  });
  hooks.goTab = (i) => nav.go(i);

  // Profil tugmasidagi nuqta: admin yozishmasida o'qilmagan xabar bor (45 s da bir marta).
  const dot = app.querySelector('.nav-item[data-i="4"] .nav-dot');
  const paintDot = () => { if (dot) dot.style.display = unreadBadge.has ? 'block' : 'none'; };
  unreadBadge.listen(paintDot);
  unreadBadge.refresh();
  setInterval(() => {
    if (document.hidden || (ChatController.watchingCount || 0) > 0) return;
    unreadBadge.refresh();
  }, 45000);
  document.addEventListener('visibilitychange', () => { if (!document.hidden) unreadBadge.refresh(); });

  startSync();
  // Telegram (videolar) holati fonda tekshiriladi; kanallarga obuna —
  // ruxsat berilgan bo'lsa orqa fonda (`channel-gate.js`).
  watchImages(app);
  // Rasmlar va videolar Telegram orqali keladi (ilovadagidek) — kirilmagan
  // bo'lsa kirish oynasi ochiladi; kirilgach rasmlar qayta yuklanadi.
  checkTelegram().catch(() => false).then(async (ok) => {
    if (!ok) ok = await ensureTelegram().catch(() => false);
    if (ok) rescanImages();
  }).finally(() => channelGate.start());
}

main();
