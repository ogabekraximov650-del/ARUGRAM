// Qobiq keshi (Service Worker) OLIB TASHLANDI (foydalanuvchi: "web app keshda saqlanmasin").
// Bu fayl faqat avval o'rnatilgan eski Service Worker'ni o'chirish uchun qoldirilgan: brauzer
// ro'yxatdan o'tgan Service Worker'ni o'zi yangilaydi -> bu kod ishga tushadi, hamma keshni
// tozalaydi va o'zini o'chiradi. Yangi o'rnatish YO'Q (`index.html` ro'yxatdan o'tkazmaydi).
// `__V__` — `build.mjs` har yig'ishda almashtiradi (fayl har safar o'zgarib, yangilanish ishga tushsin).
const V = '__V__'; // eslint-disable-line no-unused-vars

self.addEventListener('install', () => { self.skipWaiting(); });

self.addEventListener('activate', (e) => {
  e.waitUntil((async () => {
    try { for (const k of await caches.keys()) await caches.delete(k); } catch (_) { /* */ }
    try { await self.registration.unregister(); } catch (_) { /* */ }
  })());
});
