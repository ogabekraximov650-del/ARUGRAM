// Balans, obuna va to'lovlar — `lib/services/billing_service.dart` (onlayn).
//
// Bu yerda hech qanday narx yoki hisob-kitob YO'Q: tariflar, balans va
// obuna muddati serverdan keladi (`/api/billing`), to'lovni tezchek.uz
// tasdiqlaydi. Pul so'rovlari (`create`, `check`, `subscribe`) ilovadagidek
// DARHOL ketadi — `SyncQueue` ga tushmaydi (ular kamdan-kam bo'ladi).
//
// Ilovadagi `restore()` (oxirgi ma'lum muddat diskda) bu yerda
// localStorage'da: pleyer obunani tarmoq kelguncha ham biladi.
//
//   billing.active / balance / until / daysLeft / left (ms)
//   billing.plans / links / history / isLoading / hasData / error
//   billing.load({force}), billing.listen(fn) -> unsubscribe
//   billing.createLink(amount) -> {error, url}
//   billing.check(orderId) -> {paid, error}
//   billing.subscribe(days) -> error | null
//   planLabel(days), formatLeft(ms), formatLeftShort(ms), formatSum(n)

import { api, currentUser, onUser } from '../api.js';

const CACHE_KEY = 'aru_billing_v1';
const subs = new Set();

function emit() { subs.forEach((f) => { try { f(); } catch (e) { console.error(e); } }); }

function num(v) { const n = Number(v); return Number.isFinite(n) ? Math.trunc(n) : 0; }

function readCache() {
  try {
    const j = JSON.parse(localStorage.getItem(CACHE_KEY) || 'null');
    const uid = currentUser()?.id ?? null;
    if (j && (uid == null || j.uid === uid)) return j;
  } catch (_) { /* */ }
  return null;
}

function saveCache(balance, until) {
  try {
    localStorage.setItem(CACHE_KEY, JSON.stringify({ balance, until, uid: currentUser()?.id ?? null }));
  } catch (_) { /* */ }
}

function errOf(e, fallback) {
  if (e && e.status > 0) return `${e.body?.error ?? fallback}`;
  return null;
}

/** Tarif nomi: "1 oylik", "3 oylik", ... (kutilmagan qiymatda — "N kunlik"). */
export function planLabel(days) {
  if (days >= 365) return '12 oylik';
  if (days > 0 && days % 30 === 0) return `${days / 30} oylik`;
  return `${days} kunlik`;
}

const two = (n) => String(n).padStart(2, '0');

/** Qolgan vaqt: `2 kun 05:12:33` yoki `05:12:33`. */
export function formatLeft(ms) {
  if (ms <= 0) return 'tugadi';
  const s = Math.floor(ms / 1000);
  const days = Math.floor(s / 86400);
  const clock = `${two(Math.floor(s / 3600) % 24)}:${two(Math.floor(s / 60) % 60)}:${two(s % 60)}`;
  return days > 0 ? `${days} kun ${clock}` : clock;
}

/** Qisqa: `2 kun` yoki `05:12:33`. */
export function formatLeftShort(ms) {
  if (ms <= 0) return 'tugadi';
  const s = Math.floor(ms / 1000);
  if (s >= 86400) return `${Math.floor(s / 86400)} kun`;
  return `${two(Math.floor(s / 3600))}:${two(Math.floor(s / 60) % 60)}:${two(s % 60)}`;
}

/** `15 000 so'm` */
export function formatSum(amount) {
  const n = String(Math.abs(num(amount)));
  let out = '';
  for (let i = 0; i < n.length; i++) {
    if (i > 0 && (n.length - i) % 3 === 0) out += ' ';
    out += n[i];
  }
  return `${out} so'm`;
}

const cached = readCache();

export const billing = {
  balance: cached ? num(cached.balance) : 0,
  until: cached ? num(cached.until) : 0,
  plans: [],
  links: [],
  history: [],
  isLoading: false,
  hasData: false,
  error: null,

  /** Obuna faolmi (muddat SERVER bergan sana; o'tgani hech qachon faol emas). */
  get active() { return this.until > Date.now(); },

  /** Obunaga necha kun qolgan (faol bo'lmasa 0). */
  get daysLeft() {
    const ms = this.until - Date.now();
    return ms <= 0 ? 0 : Math.ceil(ms / 86400000);
  },

  /** Qolgan vaqt (ms). */
  get left() { return Math.max(0, this.until - Date.now()); },

  listen(fn) { subs.add(fn); return () => subs.delete(fn); },

  /** Hisob almashganda — xotira bo'shatiladi. */
  clear() {
    this.balance = 0; this.until = 0; this.links = []; this.history = [];
    this.hasData = false; this.error = null;
    emit();
  },

  async load({ force = false } = {}) {
    if (this.isLoading) return;
    if (this.hasData && !force) return;
    this.isLoading = true;
    this.error = null;
    emit();
    try {
      const j = await api('/api/billing');
      this._apply(j || {});
      this.hasData = true;
    } catch (e) {
      this.error = e && e.status > 0 ? `Ma'lumot olinmadi (${e.status})` : "Internet yo'q";
    }
    this.isLoading = false;
    emit();
  },

  _apply(j) {
    this.balance = num(j.balance);
    this.until = num(j.subscription_until);
    this.plans = (Array.isArray(j.plans) ? j.plans : [])
      .map((e) => ({ days: num(e?.days), price: num(e?.price) }))
      .filter((p) => p.days > 0);
    this.links = (Array.isArray(j.links) ? j.links : [])
      .map((e) => ({
        orderId: `${e?.order_id ?? ''}`,
        amount: num(e?.amount),
        url: `${e?.pay_url ?? ''}`,
        expiresAt: num(e?.expires_at),
      }))
      .filter((l) => l.url);
    this.history = (Array.isArray(j.history) ? j.history : []).map((e) => ({
      kind: `${e?.kind ?? ''}`,
      amount: num(e?.amount),
      days: num(e?.days),
      note: `${e?.note ?? ''}`,
      createdAt: num(e?.created_at),
    }));
    saveCache(this.balance, this.until);
  },

  /** To'lov havolasi yaratadi: `{error, url}`. */
  async createLink(amount) {
    try {
      const j = await api('/api/billing/create', { method: 'POST', body: { amount } });
      const url = `${j?.pay_url ?? ''}`;
      await this.load({ force: true });
      return { error: null, url: url || null };
    } catch (e) {
      return { error: errOf(e, 'Havola yaratilmadi') ?? "Internet yo'q — qaytadan urinib ko'ring", url: null };
    }
  },

  /** To'lov bo'ldimi: `{paid, error}`. */
  async check(orderId) {
    try {
      const j = await api('/api/billing/check', { method: 'POST', body: { order_id: orderId } });
      const paid = j?.status === 'paid';
      if (paid) await this.load({ force: true });
      return { paid, error: null };
    } catch (e) {
      return { paid: false, error: errOf(e, "Tekshirib bo'lmadi") ?? "Internet yo'q" };
    }
  },

  /** Obuna sotib oladi. Xato bo'lsa matn, aks holda `null`. */
  async subscribe(days) {
    try {
      await api('/api/billing/subscribe', { method: 'POST', body: { days } });
      await this.load({ force: true });
      return null;
    } catch (e) {
      return errOf(e, 'Obuna olinmadi') ?? "Internet yo'q — qaytadan urinib ko'ring";
    }
  },
};

/** Faol havolada qancha vaqt qoldi (ms). */
export const linkLeft = (l) => Math.max(0, l.expiresAt - Date.now());

// Hisob almashsa — eski hisobning balansi ko'rinmasin.
let lastUid = currentUser()?.id ?? null;
onUser((u) => {
  const uid = u?.id ?? null;
  if (uid === lastUid) return;
  lastUid = uid;
  const c = readCache();
  billing.clear();
  if (c) { billing.balance = num(c.balance); billing.until = num(c.until); emit(); }
});
