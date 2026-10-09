// `AuthService.updateSettings` (ilova): statistikani yashirish va kanallarga
// obuna ruxsati. Hisobdagi qiymat darhol (shu sahifada) o'zgaradi, serverga
// esa yagona yozuv yo'li — `sync.js` (`putSettings`) orqali ketadi.
import { currentUser } from '../api.js';
import { putSettings } from '../sync.js';

const subs = new Set();
export function onSettings(fn) { subs.add(fn); return () => subs.delete(fn); }

export function hiddenStats() { return (currentUser()?.hidden_stats || []).map((e) => `${e}`); }
export function chanConsent() { return currentUser()?.chan_consent === true; }

export function updateSettings({ hiddenStats: hs, chanConsent: cc } = {}) {
  const u = currentUser();
  if (!u) return;
  if (hs !== undefined) u.hidden_stats = [...hs];
  if (cc !== undefined) u.chan_consent = !!cc;
  putSettings(hiddenStats(), chanConsent());
  subs.forEach((f) => { try { f(u); } catch (_) { /* */ } });
}
