// Foydalanuvchining O'Z Telegram hisobi (MTProto, mtcute) — Mini App'da.
//
// Ilovada bu ishni Rust yadrosi (`rust/src/telegram.rs`, grammers) qiladi:
// videolar va yozishma fayllari foydalanuvchining bot bilan chatiga bot
// nusxalaydi (`/api/tg/deliver`), ilova esa faylni o'sha chatdan o'zi
// oladi. Worker orqali bayt o'tmaydi.
//
// Brauzerda TCP yo'q — mtcute Telegram'ning WebSocket manzillari orqali
// ulanadi. Sessiya brauzerning IndexedDB'sida (`aru-tg`) saqlanadi.
// `api_id`/`api_hash` worker'dan (`/api/tg/config`).

import { TelegramClient, WebCryptoProvider } from '@mtcute/web';
import { api } from '../api.js';
import { withTimeout } from './startup.js';

let cfg = null;
let client = null;
let starting = null;
let authorized = null;
const subs = new Set();


export function onTgChange(fn) { subs.add(fn); return () => subs.delete(fn); }
function emit() {
  try { if (authorized === true) localStorage.setItem('aru_tg_on', '1'); else if (authorized === false) localStorage.removeItem('aru_tg_on'); } catch (_) { /* */ }
  subs.forEach((f) => { try { f(authorized); } catch (_) { /* */ } });
}

/** Worker sozlamasi: `{enabled, video, channel, api_id, api_hash, bot}`. */
export async function tgConfig() {
  if (cfg) return cfg;
  cfg = await api('/api/tg/config');
  return cfg;
}

/** mtcute klienti (bitta). Ulanadi, lekin kirishni o'zi so'ramaydi. */
export async function getClient() {
  if (client) return client;
  if (starting) return starting;
  starting = (async () => {
    const c = await tgConfig();
    if (!c?.enabled) throw new Error('tg_disabled');
    const cl = new TelegramClient({
      apiId: Number(c.api_id),
      apiHash: `${c.api_hash}`,
      storage: 'aru-tg',
      crypto: new WebCryptoProvider({ wasmInput: new URL('assets/mtcute.wasm', location.href) }),
      initConnectionOptions: {
        deviceModel: 'ARUmediaTV Mini App',
        systemVersion: navigator.platform || 'Web',
        appVersion: '1.0',
        langCode: 'uz',
        systemLangCode: 'uz',
      },
      // Fayl yuklash alohida ulanishlarda ("download" turi): avval hamma `getFile` asosiy
      // ulanishdan o'tardi (ilovadagi Rust yadrosida topilgan xuddi shu sekinlik sababi:
      // bitta ulanish — bitta so'rovlar navbati). Telegram: katta fayllar media-DC da
      // alohida ulanishlarda. Rasmiy ilovalar kabi 4 ta (premium 8).
      network: {
        connectionCount: (kind, dcId, isPremium) => {
          if (kind === 'main') return 0;
          if (kind === 'download') return isPremium ? 8 : 4;
          if (kind === 'downloadSmall') return 2;
          return isPremium || (dcId !== 2 && dcId !== 4) ? 8 : 4; // upload
        },
      },
      disableUpdates: false,
      logLevel: 1,
    });
    try { await withTimeout(cl.connect(), 20000, 'tg_connect_timeout'); } catch (e) { try { cl.close?.(); } catch (_) { /* */ } throw e; }
    client = cl;
    return cl;
  })();
  try { return await starting; } finally { starting = null; }
}

/** Telegram hisobiga kirilganmi (tarmoq bilan tekshiradi, natija keshlanadi). */
export async function isAuthorized({ fresh = false } = {}) {
  if (authorized !== null && !fresh) return authorized;
  try {
    const cl = await getClient();
    const me = await withTimeout(cl.getMe(), 20000, 'tg_getme_timeout');
    authorized = !!me;
    // Yangilanishlar (updates) Mini App'ga kerak emas — asosiy oqimni band qilmasin.
    if (authorized) { try { cl.stopUpdatesLoop?.(); } catch (_) { /* */ } }
  } catch (e) {
    // Faqat Telegram "kirilmagan" desa — chiqqan deb hisoblanadi; tarmoq
    // xatosida holat o'zgarmaydi (keyingi safar qayta tekshiriladi).
    const t = `${e?.text || e?.message || ''}`;
    if (/AUTH_KEY|SESSION_|UNAUTHORIZED|USER_DEACTIVATED|not authorized/i.test(t)) authorized = false;
    else return false;
  }
  emit();
  return authorized;
}

export function authorizedCached() { return authorized === true; }

export function markAuthorized(v) { authorized = v; emit(); }

/** Telegram xatosini odam tushunadigan matnga. `{text, wait}` */
export function tgError(e) {
  const t = `${e?.text || e?.message || e || ''}`;
  const m = t.match(/(?:FLOOD_WAIT|FLOOD_PREMIUM_WAIT|SLOWMODE_WAIT|PASSWORD_TOO_FRESH)_(\d+)/);
  const wait = m ? Number(m[1]) : Number(e?.seconds || 0);
  if (wait > 0) return { text: "Telegram juda ko'p urinishni chekladi.", wait };
  const map = {
    PHONE_NUMBER_INVALID: "Telefon raqami noto'g'ri",
    PHONE_NUMBER_BANNED: 'Bu raqam Telegram tomonidan bloklangan',
    PHONE_NUMBER_FLOOD: "Bu raqamga juda ko'p kod so'raldi — keyinroq urinib ko'ring",
    PHONE_CODE_INVALID: "Kod noto'g'ri",
    PHONE_CODE_EXPIRED: "Kodning muddati tugadi — qayta so'rang",
    PHONE_CODE_EMPTY: 'Kodni kiriting',
    PASSWORD_HASH_INVALID: "Parol noto'g'ri",
    PHONE_NUMBER_UNOCCUPIED: "Bu raqamda Telegram hisobi yo'q",
    AUTH_RESTART: "Qayta urinib ko'ring",
    SEND_CODE_UNAVAILABLE: "Kodni yuborishning boshqa usuli qolmadi — keyinroq urinib ko'ring",
  };
  for (const k of Object.keys(map)) if (t.includes(k)) return { text: map[k], wait: 0 };
  if (/network|websocket|timeout|fetch/i.test(t)) return { text: "Internet bilan muammo — qayta urinib ko'ring", wait: 0 };
  return { text: t || "Noma'lum xato", wait: 0 };
}

/** Telegram'dan chiqish (shu qurilma sessiyasi). */
export async function tgLogout() {
  try { const cl = await getClient(); await cl.logOut(); } catch (_) { /* */ }
  authorized = false;
  emit();
}
