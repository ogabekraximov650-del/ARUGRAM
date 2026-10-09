// Majburiy obuna kanallari — `lib/services/channel_gate.dart` (Mini App).
//
// Bepul bo'limni ko'rishdan oldin foydalanuvchidan ruxsat so'raladi
// (`needsConsent`); ruxsat berilsa pleyer darhol ochiladi, sayt esa orqa
// fonda (har 5 daqiqada, kanallar orasida 5-10 soniya) foydalanuvchining
// O'Z Telegram hisobi bilan kanallarga qo'shiladi yoki so'rov yuboradi.
// Video ko'rilayotganda (`setVideoBusy(true)`) qo'shilmaydi.
// Ruxsat bazada (`users_db.chan_consent`, `sync.js` -> putSettings).

import { api, currentUser } from '../api.js';
import { chanConsent, updateSettings } from './user-settings.js';
import { joinChannel, isTelegramAuthorized } from '../tg/media.js';

const EVERY = 5 * 60 * 1000;
const LIMIT_PAUSE = 30 * 60 * 1000;
const BUSY_POLL = 10 * 1000;
const LIST_TTL = 15 * 60 * 1000;
const subs = new Set();

let list = null;
let listAt = 0;
let timer = 0;
let running = false;
let busy = false;
const skip = new Set();

const ls = {
  get(k) { try { return JSON.parse(localStorage.getItem(`${k}_${currentUser()?.id || 0}`) || 'null'); } catch (_) { return null; } },
  set(k, v) { try { localStorage.setItem(`${k}_${currentUser()?.id || 0}`, JSON.stringify(v)); } catch (_) { /* */ } },
};
const done = () => new Set(ls.get('chan_done') || []);

export const channelGate = {
  listen(fn) { subs.add(fn); return () => subs.delete(fn); },
  emit() { subs.forEach((f) => { try { f(); } catch (_) { /* */ } }); },
  get consented() { return chanConsent(); },
  get channels() { if (!list) list = ls.get('chan_list'); return list; },
  /** Ruxsat so'ralishi kerakmi. */
  get needsConsent() {
    if (this.consented) return false;
    const l = this.channels;
    return l == null || l.length > 0;
  },
  async refresh({ force = false } = {}) {
    const now = Date.now();
    if (!force && list && now - listAt < LIST_TTL) return;
    try {
      const j = await api('/api/channels');
      list = (j?.items || []).map((c) => ({ id: c.id, kind: `${c.kind || ''}`, title: `${c.title || ''}`, url: `${c.url || ''}` }));
      listAt = now;
      ls.set('chan_list', list);
      this.emit();
    } catch (_) { /* */ }
  },
  grant() { updateSettings({ chanConsent: true }); this.emit(); arm(5000); },
  revoke() { updateSettings({ chanConsent: false }); clearTimeout(timer); this.emit(); },
  start() { if (this.consented) arm(5000); },
};

export function setVideoBusy(v) { busy = !!v; }

function arm(ms) {
  clearTimeout(timer);
  timer = setTimeout(tick, ms);
}

async function tick() {
  if (running || !channelGate.consented) return;
  if (busy) { arm(BUSY_POLL); return; }
  const pause = ls.get('chan_pause') || 0;
  if (Date.now() < pause) { arm(pause - Date.now()); return; }
  if (!isTelegramAuthorized()) { arm(EVERY); return; }
  running = true;
  let next = EVERY;
  try {
    await channelGate.refresh();
    if (busy) { next = BUSY_POLL; return; }
    const d = done();
    const todo = (list || []).filter((c) => c.url && !d.has(`${c.kind}|${c.url}`) && !skip.has(`${c.kind}|${c.url}`));
    if (!todo.length) { skip.clear(); return; }
    const c = todo[0];
    const key = `${c.kind}|${c.url}`;
    const r = await joinChannel(c.kind, c.url);
    const err = `${r.error || ''}`;
    if (r.ok) {
      d.add(key); ls.set('chan_done', [...d]);
    } else if (/FLOOD|CHANNELS_TOO_MUCH/.test(err) || r.wait) {
      const wait = Math.max(LIMIT_PAUSE, (r.wait || 0) * 1000);
      ls.set('chan_pause', Date.now() + wait);
      next = wait;
      return;
    } else if (/INVITE_HASH_EXPIRED|INVITE_HASH_INVALID|USERNAME_NOT_OCCUPIED|USERNAME_INVALID/.test(err)) {
      d.add(key); ls.set('chan_done', [...d]);
    } else {
      skip.add(key);
    }
    next = 5000 + Math.floor(Math.random() * 5001);
  } finally {
    running = false;
    if (channelGate.consented) arm(next);
  }
}
