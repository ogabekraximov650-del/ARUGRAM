// Qism videosi manbasi — Mini App pleyeri uchun (yangi tizim `vd`, Telegram hisobisiz).
//
// Qism qatori (`GET /api/epizods/:a/:s` elementi) ichida `url_<q>` (oddiy MP4, faststart) bor. Fayl 18 MiB shifrlangan bo'laklarda
// (Telegram -> Cloudflare kesh), kalit `/api/vd/info` bilan (kirgan foydalanuvchiga). Brauzer baytlarni ochadi va MP4 -> fMP4 ni
// o'zi qiladi (`vd/remux.js`, `engine.js`) — serverda fMP4 nusxa YO'Q.
//   * fayl `vd` da tayyor (yoki kamida birinchi bo'lagi saqlangan) -> `startPlayback`;
//   * aks holda pleyer "tayyorlanmoqda" deydi va `waitForFmp4` bilan kutadi (har 5 s so'raydi).
//
// Eksport:
//   QUALITIES                       — ['1080p','720p','480p','360p'] (kattadan)
//   fileNameOf(url)                 — manzildan fayl nomi
//   mp4Qualities(ep)                — bor sifatlar (url_<q>); fmp4Qualities — xuddi shu (eski nom)
//   requestFmp4(ep)                 — endi hech narsa qilmaydi (eski nom)
//   waitForFmp4(ep, q, {signal, onTick}) — fayl yangi tizimda tayyor bo'lguncha kutadi
//   startPlayback(video, ep, q, {startAt, ahead, onState, onError}) — engine
//   devicePlatform()                — 'android' | 'ios' | 'desktop' | 'other' (Telegram.WebApp.platform)
//   chooseSource(ep, q)             — {kind:'vd', name, info} yoki null (hali tayyor emas yoki MSE qo'llanmaydi)
//   explainError(e)                 — foydalanuvchiga matn {title, text, code}

import { ApiError } from '../api.js';
import { createEngine, engineSupported } from './engine.js';
import { vdInfo } from '../vd/store.js';

export const QUALITIES = ['1080p', '720p', '480p', '360p'];

export function fileNameOf(url) {
  const u = `${url ?? ''}`.trim();
  if (!u) return '';
  return u.split('?')[0].split('/').pop();
}

export const mp4Qualities = (ep) => QUALITIES.filter((q) => fileNameOf(ep?.[`url_${q}`]));
/** Eski nom (pleyer ekrani ishlatadi): endi hamma sifat — `url_<q>` — yangi tizimda. */
export const fmp4Qualities = mp4Qualities;

/** Eski fMP4 so'rovi endi kerak emas: hamma narsa yangi tizimda. */
export async function requestFmp4() {
  return { ready: [], queued: [] };
}

/** Fayl yangi tizimda tayyor bo'lguncha kutadi (har 5 soniyada so'raydi, ~15 daqiqagacha). */
export async function waitForFmp4(ep, q, { signal, onTick } = {}) {
  const name = fileNameOf(ep?.[`url_${q}`]);
  for (let i = 0; i < 180; i++) {
    if (signal?.aborted) throw new Error('aborted');
    if (name && (await vdInfo(name, { fresh: true }))) return name;
    onTick?.({ i });
    await new Promise((r, j) => {
      const t = setTimeout(r, 5000);
      signal?.addEventListener('abort', () => { clearTimeout(t); j(new Error('aborted')); }, { once: true });
    });
  }
  throw new Error('fmp4_timeout');
}

/** Qurilma turi: Telegram o'zi aytadi (`Telegram.WebApp.platform`), bo'lmasa brauzer qatori. */
export function devicePlatform() {
  const p = `${window.Telegram?.WebApp?.platform || ''}`.toLowerCase();
  if (p === 'android' || p === 'android_x') return 'android';
  if (p === 'ios') return 'ios';
  if (p === 'macos' || p === 'tdesktop' || p === 'weba' || p === 'webk' || p === 'web' || p === 'unigram') return 'desktop';
  const ua = navigator.userAgent || '';
  if (/iPhone|iPad|iPod/i.test(ua) || (/Mac/i.test(ua) && navigator.maxTouchPoints > 1)) return 'ios';
  if (/Android/i.test(ua)) return 'android';
  return p ? 'other' : 'desktop';
}

/** MSE bor va fayl yangi tizimda tayyor bo'lsa — `{kind:'vd', name, info}`; aks holda `null` (kutiladi, `waitForFmp4`). */
export async function chooseSource(ep, q) {
  const name = fileNameOf(ep?.[`url_${q}`]);
  if (!name || !engineSupported()) return null;
  const info = await vdInfo(name);
  return info ? { kind: 'vd', name, info } : null;
}

/** Qismni o'ynatadi (Telegram hisobi kerak emas). */
export async function startPlayback(video, ep, q, { startAt = 0, ahead = 60, onState, onError, onStall, shouldPlay } = {}) {
  const pick = await chooseSource(ep, q);
  if (!pick) {
    if (!engineSupported()) throw new Error('mse_unsupported');
    throw new Error('not_ready');
  }
  return createEngine(video, { info: pick.info, ahead, startAt, onState, onError, onStall, shouldPlay });
}

/** Pleyer yopilganda qiladigan ish yo'q (bot chatiga TEGILMAYDI). */
export function releasePlayback() {}

export function explainError(e) {
  const code = e instanceof ApiError ? (e.body?.error || `http_${e.status}`) : `${e?.message || e}`;
  switch (code) {
    case 'subscription':
      return { code, title: 'Obuna kerak', text: "Bu qismni ko'rish uchun obuna bo'ling." };
    case 'bot_blocked':
      return { code, title: 'Bot to\'xtatilgan', text: "Videoni yuborish uchun botni qayta ishga tushiring (botga /start yuboring)." };
    case 'codec_unsupported':
      return { code, title: 'Video ochilmadi', text: "Qurilmangiz bu video formatini (H.265) Telegram ichida qo'llamaydi." };
    case 'mse_unsupported':
      return { code, title: 'Video ochilmadi', text: "Telegram'ingiz eskirgan — video pleyer ishlamaydi. Telegram'ni yangilang (iPhone: iOS 17.1+)." };
    case 'not_on_telegram':
    case 'not_in_chat':
      return { code, title: 'Video topilmadi', text: "Video hali Telegram'ga joylanmagan." };
    case 'fmp4_timeout':
    case 'not_ready':
      return { code, title: 'Video tayyor emas', text: "Video hali tayyorlanmoqda — birozdan keyin qayta oching." };
    case 'bad_mp4':
      return { code, title: 'Video ochilmadi', text: "Video fayli buzuq yoki qo'llab-quvvatlanmaydi." };
    case 'tg_disabled':
      return { code, title: 'Video ochilmadi', text: "Telegram orqali ko'rish hozircha o'chiq." };
    default:
      return { code, title: 'Video ochilmadi', text: `Videoni ochib bo'lmadi — internetni tekshirib, qayta urinib ko'ring. (${`${code}`.slice(0, 80)})` };
  }
}
