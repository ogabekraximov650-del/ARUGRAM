// Qism videosi manbasi — Mini App pleyeri uchun (fMP4, Telegram orqali).
//
// Qism qatori (`GET /api/epizods/:a/:s` elementi) ichida ilova uchun
// `url_<q>` (oddiy MP4) va Mini App uchun `fmp4_url_<q>` (fMP4 nusxa,
// `worker/src/fmp4.rs`) bor. Pleyer FAQAT fMP4 ni o'ynaydi:
//
//   * fMP4 bor -> `startPlayback` (Telegram'ga kirish, bot chatiga yetkazish,
//     bo'laklab o'ynatish — `engine.js`);
//   * fMP4 yo'q, MP4 bor -> worker navbatga qo'yadi (`POST /api/fmp4/request`),
//     Actions o'zi fMP4 yasaydi; pleyer "tayyorlanmoqda" deydi va
//     `waitForFmp4` bilan kutadi.
//
// Eksport:
//   QUALITIES                       — ['1080p','720p','480p','360p'] (kattadan)
//   fileNameOf(url)                 — manzildan fayl nomi
//   mp4Qualities(ep)                — ilovada bor sifatlar (url_<q>)
//   fmp4Qualities(ep)               — Mini App'da tayyor sifatlar (fmp4_url_<q>)
//   requestFmp4(ep)                 — {ready:[q], queued:[q]} (navbatga qo'yadi)
//   waitForFmp4(ep, q, {signal, onTick}) — tayyor bo'lgach yangilangan nom
//   startPlayback(video, ep, q, {startAt, ahead, onState, onError}) — engine
//   devicePlatform()                — 'android' | 'ios' | 'desktop' | 'other' (Telegram.WebApp.platform)
//   chooseSource(ep, q)             — {kind:'mp4'|'fmp4', name} yoki null (qurilma qo'llagan format)
//   explainError(e)                 — foydalanuvchiga matn {title, text, code}

import { apiPost, api, ApiError } from '../api.js';
import { createEngine, engineSupported } from './engine.js';
import { createMp4Stream, mp4Streaming } from './mp4-stream.js';
import { ensureTelegram } from '../tg/media.js';
import { startup } from '../tg/startup.js';

export const QUALITIES = ['1080p', '720p', '480p', '360p'];

export function fileNameOf(url) {
  const u = `${url ?? ''}`.trim();
  if (!u) return '';
  return u.split('?')[0].split('/').pop();
}

export const mp4Qualities = (ep) => QUALITIES.filter((q) => fileNameOf(ep?.[`url_${q}`]));
export const fmp4Qualities = (ep) => QUALITIES.filter((q) => fileNameOf(ep?.[`fmp4_url_${q}`]));

export async function requestFmp4(ep) {
  return apiPost('/api/fmp4/request', {
    anime_id: ep.anime_id, season_id: ep.season_id, epizod_id: ep.epizod_id,
  });
}

/** fMP4 tayyor bo'lguncha kutadi (har 15 soniyada so'raydi). */
export async function waitForFmp4(ep, q, { signal, onTick } = {}) {
  for (let i = 0; i < 160; i++) {
    if (signal?.aborted) throw new Error('aborted');
    const list = await api(`/api/epizods/${ep.anime_id}/${ep.season_id}`).catch(() => null);
    const fresh = Array.isArray(list) ? list.find((x) => `${x.epizod_id}` === `${ep.epizod_id}`) : null;
    if (fresh) {
      Object.assign(ep, fresh);
      const name = fileNameOf(fresh[`fmp4_url_${q}`]);
      if (name) return name;
      if (!fmp4Qualities(fresh).length && i % 4 === 3) await requestFmp4(fresh).catch(() => {});
      const other = fmp4Qualities(fresh)[0];
      if (other && onTick) onTick({ other });
    }
    onTick?.({ i });
    await new Promise((r, j) => {
      const t = setTimeout(r, 15000);
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

/**
 * Qurilma qo'llaydigan formatni tanlaydi (foydalanuvchi: "Android bo'lsa MP4, iOS bo'lsa fMP4 —
 * qurilma qo'llab-quvvatlaydigan formatda"):
 *   * iOS (WebKit): fMP4 + MSE (`ManagedMediaSource`); MP4 oqimi (Service Worker) ishonchsiz;
 *   * boshqalar: oddiy MP4 — brauzerning o'z pleyeri (Service Worker bo'lsa), bo'lmasa fMP4 + MSE.
 * Tanlangan format faylda yo'q bo'lsa, ikkinchisi olinadi; ikkalasi ham yaroqsiz bo'lsa — `null`
 * (fMP4 tayyorlash so'raladi, `requestFmp4`).
 */
export async function chooseSource(ep, q) {
  const mp4 = fileNameOf(ep?.[`url_${q}`]);
  const fmp4 = fileNameOf(ep?.[`fmp4_url_${q}`]);
  const mse = engineSupported();
  const sw = devicePlatform() === 'ios' ? false : await mp4Streaming();
  const order = devicePlatform() === 'ios' ? ['fmp4', 'mp4'] : ['mp4', 'fmp4'];
  for (const kind of order) {
    if (kind === 'mp4' && mp4 && sw) return { kind, name: mp4 };
    if (kind === 'fmp4' && fmp4 && mse) return { kind, name: fmp4 };
  }
  return null;
}

/** Qismni o'ynatadi. Telegram'ga kirilmagan bo'lsa — kirish oynasi. */
export async function startPlayback(video, ep, q, { startAt = 0, ahead = 20, onState, onError } = {}) {
  await startup();
  if (!(await ensureTelegram())) throw new Error('tg_login_cancelled');
  const pick = await chooseSource(ep, q);
  if (!pick) {
    if (!engineSupported() && fileNameOf(ep[`fmp4_url_${q}`])) throw new Error('mse_unsupported');
    throw new Error('no_fmp4');
  }
  if (pick.kind === 'mp4') {
    const eng = createMp4Stream(video, { name: pick.name, startAt, onState, onError });
    try {
      await eng.ready;
      return eng;
    } catch (e) {
      // Oddiy MP4 ochilmadi (SW/kodek) — fMP4 ga o'tamiz (bo'lsa).
      eng.destroy();
      const alt = fileNameOf(ep[`fmp4_url_${q}`]);
      if (!alt || !engineSupported()) throw e;
    }
  }
  return createEngine(video, { name: pick.kind === 'mp4' ? fileNameOf(ep[`fmp4_url_${q}`]) : pick.name, ahead, startAt, onState, onError });
}

/** Pleyer yopilganda bot chatiga TEGILMAYDI (eski nusxalar faqat ilova ochilganda tozalanadi). */
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
    case 'tg_login_cancelled':
      return { code, title: "Telegram ulanmagan", text: "Videolar Telegram hisobingiz orqali ko'rsatiladi — Telegram'ni ulang." };
    case 'not_on_telegram':
    case 'not_in_chat':
      return { code, title: 'Video topilmadi', text: "Video hali Telegram'ga joylanmagan." };
    case 'fmp4_timeout':
      return { code, title: 'Video tayyor emas', text: "Video hali tayyorlanmoqda — birozdan keyin qayta oching." };
    case 'tg_disabled':
      return { code, title: 'Video ochilmadi', text: "Telegram orqali ko'rish hozircha o'chiq." };
    default:
      return { code, title: 'Video ochilmadi', text: `Videoni ochib bo'lmadi — internetni tekshirib, qayta urinib ko'ring. (${`${code}`.slice(0, 80)})` };
  }
}
