// Bo'limlar ro'yxati — `lib/services/seasons_repo.dart` (onlayn).
//
// Bosh sahifa va Katalog BITTA ro'yxatni ishlatadi. Telefondagi
// nusxa (localStorage) darhol ko'rsatiladi, keyin serverdan yangisi.

import { api } from './api.js';

const CACHE_KEY = 'aru_seasons_v1';
const subs = new Set();

function readCache() {
  try {
    const v = JSON.parse(localStorage.getItem(CACHE_KEY) || 'null');
    return Array.isArray(v) ? v : null;
  } catch (_) { return null; }
}

export const seasonsRepo = {
  items: readCache() || [],
  loading: false,
  failed: false,
  loaded: false,

  listen(fn) { subs.add(fn); return () => subs.delete(fn); },
  emit() { subs.forEach((f) => { try { f(); } catch (e) { console.error(e); } }); },

  async load() {
    if (this.loaded) return;
    await this.fetch();
  },

  async fetch() {
    if (this.loading) return;
    this.loading = true;
    this.emit();
    try {
      const list = await api('/api/seasons');
      if (Array.isArray(list)) {
        this.items = list;
        this.failed = false;
        this.loaded = true;
        try { localStorage.setItem(CACHE_KEY, JSON.stringify(list)); } catch (_) { /* */ }
      }
    } catch (_) {
      this.failed = true;
    }
    this.loading = false;
    this.emit();
  },

  /** Janrlar (vergul bilan ajratilgan `janri` dan, alifbo tartibida). */
  get genres() {
    const set = new Set();
    for (const s of this.items) {
      for (const p of `${s.janri ?? ''}`.split(',')) { const t = p.trim(); if (t) set.add(t); }
    }
    return [...set].sort();
  },

  /** Yillar (kamayish tartibida). */
  get years() {
    const set = new Set();
    for (const s of this.items) { const t = `${s.yili ?? ''}`.trim(); if (t) set.add(t); }
    return [...set].sort((a, b) => (parseInt(b, 10) || 0) - (parseInt(a, 10) || 0));
  },

  find(animeId, seasonId) {
    return this.items.find((s) => `${s.anime_id}` === `${animeId}` && `${s.season_id}` === `${seasonId}`) || null;
  },
};
