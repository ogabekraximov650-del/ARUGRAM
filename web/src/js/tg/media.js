// Telegram orqali fayllar — YENGIL kirish nuqtasi.
//
// mtcute (Telegram kutubxonasi, ~1 MB) faqat kerak bo'lganda yuklanadi
// (`media-impl.js`), shu sabab Mini App tez ochiladi. Hamma funksiyalar
// `media-impl.js` dagidek (izohlari o'sha yerda).

let impl = null;
const load = () => (impl ||= import('./media-impl.js'));
let authorized = false;

export async function ensureTelegram() { const ok = await (await load()).ensureTelegram(); authorized = ok; return ok; }
export function isTelegramAuthorized() { return authorized; }
// Telegram'ga hech kirilmagan bo'lsa mtcute umuman yuklanmaydi (belgi —
// `client.js` -> `markAuthorized`).
const flag = () => { try { return localStorage.getItem('aru_tg_on') === '1'; } catch (_) { return false; } };
export async function checkTelegram() {
  if (!flag()) { authorized = false; return false; }
  const m = await load(); authorized = await m.isAuthorizedFresh(); return authorized;
}
export async function openFile(name, opts) { return (await load()).openFile(name, opts); }
export async function fetchFile(name, opts) { return (await load()).fetchFile(name, opts); }
export async function mediaUrl(name, opts) { return (await load()).mediaUrl(name, opts); }
export async function uploadFile(file, name) { return (await load()).uploadFile(file, name); }
export async function clearBotChat() { if (impl) return (await load()).clearBotChat(); return undefined; }
export async function ctrApply(hex, offset, data) { return (await load()).ctrApply(hex, offset, data); }
export async function telegramLogout() { const m = await load(); await m.logout(); authorized = false; }
export async function telegramMe() { return (await load()).me(); }
export async function joinChannel(kind, url) { return (await load()).joinChannel(kind, url); }
