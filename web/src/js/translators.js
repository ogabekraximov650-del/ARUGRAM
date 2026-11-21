// Tarjimonlar logotiplari — Android `TranslatorLogos` (`data/Translators.kt`): `tr_id` -> logotip manzili, bir marta yuklanadi.
// Bo'lim kartochkasida tarjimon logotipi (yumaloq) ko'rsatiladi.

import { api, imageUrl } from './api.js';

const subs = new Set();
let started = false;

export const translatorLogos = {
  map: new Map(),
  listen(fn) { subs.add(fn); return () => subs.delete(fn); },
  /** `id` bo'yicha logotip manzili ('' — yo'q). */
  get(id) { return this.map.get(Number(id)) || ''; },
  async ensure() {
    if (started) return;
    started = true;
    try {
      const list = await api('/api/translators');
      if (!Array.isArray(list)) throw new Error('royxat');
      for (const t of list) {
        const id = Number(t.id);
        if (id > 0 && t.photo_url) this.map.set(id, imageUrl(t.photo_url));
      }
      subs.forEach((f) => { try { f(); } catch (e) { console.error(e); } });
    } catch (_) {
      started = false;
    }
  },
};
