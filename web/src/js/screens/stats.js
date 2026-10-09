// Statistika — ilovadagi quyidagilarning onlayn nusxasi:
//   * `lib/services/stats_service.dart`  -> `appStats` (GET /api/stats), `myStats` (GET /api/me/stats)
//   * `lib/services/user_stats.dart`     -> `STAT_KINDS`, `UserStatsList` (GET /api/user/:id/stats/:key)
//   * `lib/widgets/trend_chart.dart`     -> `trendChart()` (GET /api/stats/series)
//   * `lib/widgets/stats_banner.dart`    -> `createStatsBanner()` (bosh sahifa uchun)
//   * `lib/screens/stats_screen.dart`    -> `openStats()`
//   * `lib/screens/stat_detail_screen.dart` -> `openStatDetail()`
//
// Ilovada raqamlar diskda (shifrlangan kesh) saqlanadi — bu yerda
// localStorage'da (faqat o'qish keshi, Turso'ga yozuv yo'q).
//
// Ataylab YO'Q: tarix qatorini uzoq bosib o'chirish (EpisodeRow ->
// `_confirmRemove`) — u `sync.js` dagi tarix yozuvlariga tegishli,
// Kutubxona/Tarix ekrani bilan birga keladi; kadr (`_Frame`) — kadrlar
// faqat telefonda yasaladi, sayt doim posterni ko'rsatadi; GIF/stiker
// izohlarida media o'rniga `MediaPlaceholder` (to'plamlar moduli
// Telegram orqali yuklaydi).

import { api, ApiError, currentUser, imageUrl } from '../api.js';
import { esc, formatCount, toInt } from '../format.js';
import { hooks } from '../hooks.js';
import { push, back } from '../router.js';
import { seasonsRepo } from '../seasons.js';
import {
  C, icon, spinner, bindTap, appBar, bindAppBar, seasonCardHtml, bindImages, cardWidth,
  isPaid, paidBadge,
} from '../ui.js';

// ══════════════════════════════════════════════════════════════
//  FORMAT (`lib/services/format.dart`)
// ══════════════════════════════════════════════════════════════

/** `1288490188` -> `1,20 GB` */
export function formatBytes(bytes) {
  const b = Number(bytes) || 0;
  if (b < 1024) return `${Math.round(b)} B`;
  const units = ['KB', 'MB', 'GB', 'TB', 'PB'];
  let v = b / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; }
  const text = v >= 100 ? v.toFixed(0) : (v >= 10 ? v.toFixed(1) : v.toFixed(2));
  return `${text.replace('.', ',')} ${units[i]}`;
}

/** Tomosha vaqti: `1.284:05` (soat:daqiqa). */
export function formatHours(ms) {
  const total = Math.floor(Math.round(Number(ms) || 0) / 1000);
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  return `${formatCount(hours)}:${String(minutes).padStart(2, '0')}`;
}

const two = (n) => String(n).padStart(2, '0');

/** `12:34/01/01/2026` — soat/kun/oy/yil. */
export function formatMoment(ms) {
  const v = toInt(ms);
  if (v <= 0) return '';
  const d = new Date(v);
  return `${two(d.getHours())}:${two(d.getMinutes())}/${two(d.getDate())}/${two(d.getMonth() + 1)}/${d.getFullYear()}`;
}

/** `12.03.2026` */
function shortDate(ms) {
  const v = toInt(ms);
  if (v <= 0) return '';
  const d = new Date(v);
  return `${two(d.getDate())}.${two(d.getMonth() + 1)}.${d.getFullYear()}`;
}

function lsGet(k) { try { return JSON.parse(localStorage.getItem(k) || 'null'); } catch (_) { return null; } }
function lsSet(k, v) { try { localStorage.setItem(k, JSON.stringify(v)); } catch (_) { /* */ } }

function emitter() {
  const subs = new Set();
  return {
    listen(fn) { subs.add(fn); return () => subs.delete(fn); },
    emit() { subs.forEach((f) => { try { f(); } catch (e) { console.error(e); } }); },
  };
}

// ══════════════════════════════════════════════════════════════
//  STATISTIKA TURLARI (`StatKind`)
// ══════════════════════════════════════════════════════════════

export const STAT_KINDS = {
  anime: { key: 'anime', label: 'Anime', openable: false },
  episodes: { key: 'episodes', label: 'Qismlar', openable: true },
  seasons: { key: 'seasons', label: "Bo'limlar", openable: true },
  favorites: { key: 'favorites', label: 'Sevimlilar', openable: true },
  rated: { key: 'rated', label: 'Baholangan', openable: true },
  comments: { key: 'comments', label: 'Izohlar', openable: true },
  watch: { key: 'watch', label: 'Tomosha vaqti', openable: false },
};

// ══════════════════════════════════════════════════════════════
//  UMUMIY STATISTIKA (`StatsService`) — GET /api/stats
// ══════════════════════════════════════════════════════════════

const APP_KEY = 'aru_app_stats_v1';

function block(j) {
  const v = (k) => toInt((j || {})[k]);
  return { daily: v('daily'), weekly: v('weekly'), monthly: v('monthly'), total: v('total') };
}

function parseAppStats(j) {
  const c = (j && j.content) || {};
  return {
    content: {
      anime: toInt(c.anime),
      seasons: toInt(c.seasons),
      episodes: toInt(c.episodes),
      newEpisodes: {
        daily: toInt(c.episodes_daily), weekly: toInt(c.episodes_weekly),
        monthly: toInt(c.episodes_monthly), total: toInt(c.episodes),
      },
    },
    users: block(j?.users),
    views: block(j?.views),
    animeViews: block(j?.anime_views),
    seasonViews: block(j?.season_views),
    traffic: block(j?.traffic),
    watch: block(j?.watch),
  };
}

export const appStats = {
  ...emitter(),
  raw: lsGet(APP_KEY),
  stats: parseAppStats(lsGet(APP_KEY) || {}),
  loading: false,
  loadedAt: 0,
  /** 10 daqiqa ichida qayta so'ralmaydi. */
  async load(force = false) {
    if (this.loading) return;
    if (!force && this.loadedAt && Date.now() - this.loadedAt < 10 * 60_000) return;
    this.loading = true;
    this.emit();
    try {
      const j = await api('/api/stats');
      if (j && typeof j === 'object') {
        this.stats = parseAppStats(j);
        this.loadedAt = Date.now();
        lsSet(APP_KEY, j);
      }
    } catch (_) { /* oxirgi ma'lum raqamlar qoladi */ }
    this.loading = false;
    this.emit();
  },
};

// ══════════════════════════════════════════════════════════════
//  SHAXSIY STATISTIKA (`MyStatsService`) — GET /api/me/stats
// ══════════════════════════════════════════════════════════════

function parseMyStats(j) {
  const v = (k) => toInt((j || {})[k]);
  return {
    animes: v('animes'), episodes: v('episodes'), watchMs: v('watch_ms'), traffic: v('traffic'),
    seasons: v('seasons'), favorites: v('favorites'), rated: v('rated'), comments: v('comments'),
  };
}

const myKey = () => `aru_my_stats_${currentUser()?.id || 0}`;

export const myStats = {
  ...emitter(),
  stats: parseMyStats(lsGet(myKey()) || {}),
  loading: false,
  loadedAt: 0,
  loadFromDisk() {
    const j = lsGet(myKey());
    if (j) { this.stats = parseMyStats(j); this.emit(); }
  },
  /** Profil tez-tez ochiladi — 2 daqiqa yetarli. */
  async load(force = false) {
    if (this.loading) return;
    if (!force && this.loadedAt && Date.now() - this.loadedAt < 2 * 60_000) return;
    this.loading = true;
    this.emit();
    try {
      const j = await api('/api/me/stats');
      if (j && typeof j === 'object') {
        this.stats = parseMyStats(j);
        this.loadedAt = Date.now();
        lsSet(myKey(), j);
      }
    } catch (_) { /* oxirgi ma'lum raqamlar qoladi */ }
    this.loading = false;
    this.emit();
  },
};

// ══════════════════════════════════════════════════════════════
//  GRAFIK (`trend_chart.dart`)
// ══════════════════════════════════════════════════════════════

const RANGES = [['24h', '24 soat'], ['7d', '7 kun'], ['30d', '30 kun'], ['all', 'Hammasi']];
const MONTHS = ['yan', 'fev', 'mar', 'apr', 'may', 'iyun', 'iyul', 'avg', 'sen', 'okt', 'noy', 'dek'];
const memo = new Map();

/** "2026-10-04T13" -> "4-okt 13:00", "2026-10-04" -> "4-okt 2026". */
function seriesLabel(t) {
  const d = `${t}`.split('T');
  const p = d[0].split('-');
  if (p.length < 3) return `${t}`;
  const m = parseInt(p[1], 10) || 1;
  const day = parseInt(p[2], 10) || 1;
  const base = `${day}-${MONTHS[Math.min(11, Math.max(0, m - 1))]}`;
  return d.length > 1 ? `${base} ${d[1]}:00` : `${base} ${p[0]}`;
}

function hexA(hex, a) {
  const n = parseInt(hex.slice(1), 16);
  return `rgba(${(n >> 16) & 255},${(n >> 8) & 255},${n & 255},${a})`;
}

/**
 * `metric`: views | anime_views | season_views | watch_ms | traffic | users.
 * Qaytaradi: `{ el, dispose() }`.
 */
export function trendChart(metric, format, { color = C.accent2, height = 150 } = {}) {
  const el = document.createElement('div');
  el.className = 'tc';
  el.innerHTML = `
    <div class="tc-head">
      <div class="tc-hl"><div class="tc-v"></div><div class="tc-l"></div></div>
      <div class="tc-chg"></div>
    </div>
    <div class="tc-box" style="height:${height}px"></div>
    <div class="tc-axis"><span></span><span></span></div>
    <div class="tc-ranges">${RANGES.map(([k, l]) => `<div class="tc-r" data-k="${k}">${l}</div>`).join('')}</div>`;
  const vEl = el.querySelector('.tc-v');
  const lEl = el.querySelector('.tc-l');
  const chg = el.querySelector('.tc-chg');
  const box = el.querySelector('.tc-box');
  const axis = el.querySelector('.tc-axis');
  const ranges = [...el.querySelectorAll('.tc-r')];

  let range = '7d';
  let points = null;
  let loading = false;
  let error = null;
  let touch = null;
  let dead = false;
  let canvas = null;

  function paintRanges() {
    ranges.forEach((r) => {
      const on = r.dataset.k === range;
      r.classList.toggle('on', on);
      r.style.background = on ? hexA(color, 0.18) : 'rgba(255,255,255,0.05)';
      r.style.borderColor = on ? hexA(color, 0.6) : 'transparent';
    });
  }

  function draw() {
    if (!canvas) return;
    const w = box.clientWidth;
    const h = height;
    if (w <= 0) return;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = Math.round(w * dpr);
    canvas.height = Math.round(h * dpr);
    canvas.style.width = `${w}px`;
    canvas.style.height = `${h}px`;
    const g = canvas.getContext('2d');
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    g.clearRect(0, 0, w, h);
    g.strokeStyle = 'rgba(255,255,255,0.06)';
    g.lineWidth = 1;
    for (let i = 1; i <= 3; i++) {
      const y = (h * i) / 4;
      g.beginPath(); g.moveTo(0, y); g.lineTo(w, y); g.stroke();
    }
    const values = (points || []).map((p) => p.v);
    if (values.length < 2) return;
    const maxV = Math.max(...values);
    const top = maxV <= 0 ? 1 : maxV * 1.12;
    const pt = (i) => [(w * i) / (values.length - 1), h - (values[i] / top) * (h - 6) - 3];
    const line = new Path2D();
    const p0 = pt(0);
    line.moveTo(p0[0], p0[1]);
    for (let i = 1; i < values.length; i++) {
      const a = pt(i - 1); const b = pt(i);
      line.quadraticCurveTo(a[0], a[1], (a[0] + b[0]) / 2, (a[1] + b[1]) / 2);
      if (i === values.length - 1) line.lineTo(b[0], b[1]);
    }
    const fill = new Path2D(line);
    fill.lineTo(w, h); fill.lineTo(0, h); fill.closePath();
    const grad = g.createLinearGradient(0, 0, 0, h);
    grad.addColorStop(0, hexA(color, 0.35));
    grad.addColorStop(1, hexA(color, 0));
    g.fillStyle = grad;
    g.fill(fill);
    g.strokeStyle = color;
    g.lineWidth = 2.2;
    g.lineCap = 'round';
    g.lineJoin = 'round';
    g.stroke(line);
    const last = pt(values.length - 1);
    g.fillStyle = color;
    g.beginPath(); g.arc(last[0], last[1], 3.5, 0, Math.PI * 2); g.fill();
    if (touch != null && touch >= 0 && touch < values.length) {
      const p = pt(touch);
      g.strokeStyle = 'rgba(255,255,255,0.35)';
      g.lineWidth = 1;
      g.beginPath(); g.moveTo(p[0], 0); g.lineTo(p[0], h); g.stroke();
      g.fillStyle = '#fff';
      g.beginPath(); g.arc(p[0], p[1], 5, 0, Math.PI * 2); g.fill();
      g.fillStyle = color;
      g.beginPath(); g.arc(p[0], p[1], 3.5, 0, Math.PI * 2); g.fill();
    }
  }

  function renderHead() {
    const pts = points || [];
    const total = pts.reduce((a, p) => a + p.v, 0);
    const half = Math.floor(pts.length / 2);
    const first = pts.slice(0, half).reduce((a, p) => a + p.v, 0);
    const second = pts.slice(pts.length - half).reduce((a, p) => a + p.v, 0);
    const change = first > 0 ? ((second - first) * 100) / first : null;
    const up = (change ?? 0) >= 0;
    const touched = touch != null && touch < pts.length ? pts[touch] : null;
    vEl.textContent = touched ? format(touched.v) : format(total);
    lEl.textContent = touched ? seriesLabel(touched.t) : "Tanlangan davr bo'yicha jami";
    if (change != null && !touched) {
      chg.style.display = '';
      chg.style.background = hexA(up ? C.success : C.danger, 0.15);
      chg.style.color = up ? C.success : C.danger;
      chg.textContent = `${up ? '▲' : '▼'} ${Math.abs(change).toFixed(1)}%`;
    } else {
      chg.style.display = 'none';
    }
    if (pts.length >= 2) {
      axis.style.display = '';
      axis.children[0].textContent = seriesLabel(pts[0].t);
      axis.children[1].textContent = seriesLabel(pts[pts.length - 1].t);
    } else {
      axis.style.display = 'none';
    }
  }

  function renderBody() {
    if (loading && points == null) {
      canvas = null;
      box.innerHTML = `<div class="tc-center">${spinner(20, 2, 'rgba(255,255,255,0.38)')}</div>`;
    } else if (error && points == null) {
      canvas = null;
      box.innerHTML = `<div class="tc-center"><span class="tc-err">${esc(error)}</span></div>`;
    } else {
      if (!canvas) {
        box.innerHTML = '';
        canvas = document.createElement('canvas');
        box.appendChild(canvas);
        bindTouch();
      }
      draw();
    }
    renderHead();
  }

  function at(x) {
    const n = (points || []).length;
    if (n < 2) return null;
    const w = box.clientWidth || 1;
    return Math.min(n - 1, Math.max(0, Math.round((x / w) * (n - 1))));
  }

  function bindTouch() {
    let down = false;
    const set = (e) => {
      const r = box.getBoundingClientRect();
      touch = at(e.clientX - r.left);
      draw(); renderHead();
    };
    canvas.addEventListener('pointerdown', (e) => { down = true; set(e); });
    canvas.addEventListener('pointermove', (e) => { if (down) set(e); });
    const up = () => { if (!down) return; down = false; touch = null; draw(); renderHead(); };
    canvas.addEventListener('pointerup', up);
    canvas.addEventListener('pointercancel', up);
    canvas.addEventListener('pointerleave', up);
  }

  async function load() {
    const key = `${metric}|${range}`;
    const hit = memo.get(key);
    if (hit && Date.now() - hit.at < 5 * 60_000) {
      points = hit.pts; error = null; touch = null;
      renderBody();
      return;
    }
    loading = true; error = null;
    renderBody();
    const want = range;
    try {
      const j = await api(`/api/stats/series?metric=${encodeURIComponent(metric)}&range=${range}`);
      if (dead || want !== range) return;
      const pts = (Array.isArray(j?.points) ? j.points : [])
        .filter((p) => p && typeof p === 'object')
        .map((p) => ({ t: `${p.t}`, v: toInt(p.v) }));
      memo.set(key, { at: Date.now(), pts });
      points = pts; loading = false; touch = null;
    } catch (e) {
      if (dead || want !== range) return;
      loading = false;
      error = e instanceof ApiError && e.status ? `Yuklanmadi (${e.status})` : "Internet yo'q";
    }
    renderBody();
  }

  ranges.forEach((r) => r.addEventListener('click', () => {
    if (range === r.dataset.k) return;
    range = r.dataset.k;
    paintRanges();
    load();
  }));

  const ro = typeof ResizeObserver !== 'undefined' ? new ResizeObserver(() => draw()) : null;
  ro?.observe(box);
  paintRanges();
  load();
  return { el, dispose() { dead = true; ro?.disconnect(); } };
}

// ══════════════════════════════════════════════════════════════
//  UMUMIY STATISTIKA EKRANI (`stats_screen.dart`)
// ══════════════════════════════════════════════════════════════

const CARDS = [
  {
    title: 'Foydalanuvchilar', ic: 'people_alt', key: 'users', metric: 'users',
    fmt: (v) => `${formatCount(v)} ta`,
    note: 'Kunlik — oxirgi 24 soatda kirganlar. Grafik — yangi ochilgan hisoblar',
  },
  {
    title: "Anime ko'rishlar", ic: 'movie_filter', key: 'animeViews', metric: 'anime_views',
    fmt: (v) => `${formatCount(v)} ta`,
    note: "Bitta odam bitta animeni ko'rgani bir marta sanaladi",
  },
  {
    title: "Bo'lim ko'rishlar", ic: 'video_library', key: 'seasonViews', metric: 'season_views',
    fmt: (v) => `${formatCount(v)} ta`,
    note: "Bitta odam bitta bo'limni ko'rgani bir marta sanaladi",
  },
  {
    title: "Qism ko'rishlar", ic: 'play_circle', key: 'views', metric: 'views',
    fmt: (v) => `${formatCount(v)} ta`,
    note: "Qism ochilib ko'rilgani hisoblanadi",
  },
  {
    title: "Ko'rish vaqti", ic: 'schedule', key: 'watch', metric: 'watch_ms',
    fmt: (v) => `${formatHours(v)} soat`,
    note: '1x tezlikdagi haqiqiy vaqt',
  },
  {
    title: 'Trafik sarfi', ic: 'cloud_download', key: 'traffic', metric: 'traffic',
    fmt: formatBytes,
    note: 'Barcha foydalanuvchilar qabul qilgan hajm',
  },
];

function statRow(label, value, strong = false) {
  return `<div class="st-row${strong ? ' strong' : ''}"><span class="st-rl">${label}:</span><span class="st-rv">${esc(value)}</span></div>`;
}

function contentCardHtml(c) {
  const big = (label, v, ic) => `<div class="st-big">${icon(ic, { size: 20, color: C.accent })}
    <div class="st-bv">${formatCount(v)}</div><div class="st-bl">${label}</div></div>`;
  const n = c.newEpisodes;
  return `<div class="glass st-card st-content">
    <div class="st-ch">${icon('video_library', { size: 20, color: C.accent })}<span class="st-ct">Ilovadagi kontent</span></div>
    <div class="st-bigs">${big('Anime', c.anime, 'movie_filter')}${big("Bo'lim", c.seasons, 'video_library')}${big('Qism', c.episodes, 'play_circle')}</div>
    <div class="st-new">Yangi qo'shilgan qismlar — kunlik: ${formatCount(n.daily)}, haftalik: ${formatCount(n.weekly)}, oylik: ${formatCount(n.monthly)}</div>
  </div>`;
}

/** Umumiy statistika ekrani (bosh sahifadagi banner ochadi). */
export function openStats() {
  push((el) => {
    el.innerHTML = `
      <div class="st-top">
        <div class="glass st-back">${icon('arrow_back', { size: 24, color: '#fff' })}</div>
        <div class="st-title">Umumiy statistika</div>
        <div class="st-load"></div>
      </div>
      <div class="scroll"><div class="st-list">
        <div class="st-content-slot"></div>
        ${CARDS.map((c, i) => `<div class="glass st-card" data-i="${i}">
          <div class="st-ch">${icon(c.ic, { size: 20, color: C.accent })}<span class="st-ct">${c.title}</span></div>
          <div class="st-rows"></div>
          <div class="st-chart"></div>
          <div class="st-note">${c.note}</div>
        </div>`).join('')}
        <div class="st-tz">Vaqt mintaqasi: UTC+5 (Toshkent)</div>
      </div></div>`;
    bindTap(el.querySelector('.st-back'), () => back(), { scale: false });
    const charts = [];
    el.querySelectorAll('.st-card[data-i]').forEach((card) => {
      const c = CARDS[+card.dataset.i];
      const ch = trendChart(c.metric, c.fmt);
      card.querySelector('.st-chart').appendChild(ch.el);
      charts.push(ch);
    });
    function render() {
      const s = appStats.stats;
      el.querySelector('.st-load').innerHTML = appStats.loading ? spinner(18, 2, 'rgba(255,255,255,0.54)') : '';
      el.querySelector('.st-content-slot').innerHTML = contentCardHtml(s.content);
      el.querySelectorAll('.st-card[data-i]').forEach((card) => {
        const c = CARDS[+card.dataset.i];
        const b = s[c.key];
        card.querySelector('.st-rows').innerHTML = statRow('Kunlik', c.fmt(b.daily)) + statRow('Haftalik', c.fmt(b.weekly))
          + statRow('Oylik', c.fmt(b.monthly)) + statRow('Umumiy', c.fmt(b.total), true);
      });
    }
    const off = appStats.listen(render);
    render();
    appStats.load();
    return { dispose() { off(); charts.forEach((c) => c.dispose()); } };
  }, { transition: 'fade' });
}

// ══════════════════════════════════════════════════════════════
//  BOSH SAHIFADAGI BANNER (`stats_banner.dart`)
// ══════════════════════════════════════════════════════════════

/**
 * Bosh sahifa uchun: `const b = createStatsBanner(); host.appendChild(b.el);`
 * (ilovada sarlavha va "Ommabop anime" orasida, `padding: 0 16 4`).
 */
export function createStatsBanner() {
  const el = document.createElement('div');
  el.className = 'glass sb';
  el.innerHTML = `
    <div class="sb-h"><span class="sb-t">Umumiy statistikani ko'rish</span>${icon('chevron_right', { size: 22, color: 'rgba(255,255,255,0.6)' })}</div>
    <div class="sb-u"></div>
    <div class="sb-clip"></div>`;
  const uEl = el.querySelector('.sb-u');
  const clip = el.querySelector('.sb-clip');
  let index = 0;
  const items = (s) => [
    ['play_circle', false, `Kunlik ko'rishlar: ${formatCount(s.views.daily)} ta`],
    ['cloud_download', false, `Kunlik trafik: ${formatBytes(s.traffic.daily)}`],
    ['schedule', true, `Kunlik tomosha: ${formatHours(s.watch.daily)} soat`],
    ['group', false, `Jami foydalanuvchi: ${formatCount(s.users.total)} ta`],
    ['visibility', false, `Jami ko'rishlar: ${formatCount(s.views.total)} ta`],
  ];
  const itemHtml = (it) => `<div class="sb-item">${icon(it[0], { fill: it[1], size: 15, color: 'rgba(255,255,255,0.55)' })}<span>${esc(it[2])}</span></div>`;
  function render(slide = false) {
    const s = appStats.stats;
    uEl.textContent = `Kunlik foydalanuvchilar: ${formatCount(s.users.daily)} ta`;
    const list = items(s);
    const html = itemHtml(list[index % list.length]);
    const old = clip.querySelector('.sb-item:not(.out)');
    if (!slide || !old) { clip.innerHTML = html; return; }
    const tmp = document.createElement('div');
    tmp.innerHTML = html;
    const nu = tmp.firstElementChild;
    nu.classList.add('in-start');
    clip.appendChild(nu);
    old.classList.add('out');
    requestAnimationFrame(() => requestAnimationFrame(() => nu.classList.remove('in-start')));
    setTimeout(() => old.remove(), 450);
  }
  bindTap(el, () => openStats(), { scale: false });
  const off = appStats.listen(() => render(false));
  const timer = setInterval(() => { index++; render(true); }, 4000);
  render(false);
  appStats.load();
  return { el, dispose() { clearInterval(timer); off(); } };
}

// ══════════════════════════════════════════════════════════════
//  BITTA STATISTIKA OYNASI (`stat_detail_screen.dart`)
// ══════════════════════════════════════════════════════════════

class UserStatsList {
  constructor(userId, kind) {
    this.userId = userId; this.kind = kind;
    this.items = []; this.page = 0; this.loading = false; this.hasMore = true; this.error = null;
    Object.assign(this, emitter());
  }

  async refresh() {
    this.page = 0; this.hasMore = true; this.items = []; this.error = null;
    this.emit();
    await this.loadMore();
  }

  async loadMore() {
    if (this.loading || !this.hasMore) return;
    this.loading = true;
    this.emit();
    try {
      const j = await api(`/api/user/${this.userId}/stats/${this.kind.key}?page=${this.page}`);
      const rows = Array.isArray(j?.items) ? j.items : [];
      this.items = this.items.concat(rows);
      this.hasMore = j?.has_more === true;
      this.page++;
      this.error = null;
    } catch (e) {
      if (e instanceof ApiError && e.status === 403) {
        this.error = 'Bu statistika yashirilgan';
        this.hasMore = false;
      } else if (e instanceof ApiError && e.status) {
        this.error = "Ro'yxat kelmadi";
        this.hasMore = false;
      } else {
        this.error = this.items.length === 0 ? "Internet yo'q" : null;
        this.hasMore = false;
      }
    }
    this.loading = false;
    this.emit();
  }
}

function paidOf(animeId, seasonId) {
  const s = seasonsRepo.find(animeId, seasonId);
  return isPaid(s);
}

const pct = (v) => `${(Number.isNaN(v) ? 0 : Math.min(100, Math.max(0, v))).toFixed(2).replace('.', ',')}%`;
const clock = (ms) => {
  const t = Math.floor(toInt(ms) / 1000);
  return `${two(Math.floor(t / 60))}:${two(t % 60)}`;
};

/** Tarix qatori (`history_screen.dart` -> `EpisodeRow`), poster bilan. */
function episodeRowHtml(r) {
  const animeId = toInt(r.anime_id);
  const seasonId = toInt(r.season_id);
  const bolimId = toInt(r.bolim_id);
  const pos = toInt(r.position_ms);
  const dur = toInt(r.duration_ms);
  let progress = dur > 0 ? pos / dur : 0;
  if (!(progress > 0)) progress = 0;
  if (progress > 1) progress = 1;
  const sName = `${r.season_name ?? ''}`.trim();
  const aName = `${r.anime_name ?? ''}`.trim();
  const title = sName || aName || 'Anime';
  const poster = imageUrl(`${r.season_photo ?? ''}` || `${r.anime_photo ?? ''}`);
  const cleared = toInt(r.deleted_at) !== 0;
  const date = formatMoment(r.updated_at);
  return `<div class="sd-ep">
    ${poster ? `<img class="sd-ep-img" alt="" loading="lazy" src="${esc(poster)}" onerror="this.remove()">` : ''}
    ${paidOf(animeId, seasonId) ? `<div class="sd-ep-paid">${paidBadge()}</div>` : ''}
    <div class="sd-ep-txt">
      <div class="sd-ep-l">
        <div class="sh" style="font-size:11.5px;font-weight:700">${esc(title)}</div>
        <div class="sh" style="font-size:10.5px">${bolimId > 0 ? bolimId : seasonId}-bo'lim ${toInt(r.epizod_number)}-qism</div>
        <div class="sh" style="font-size:10px;color:rgba(255,255,255,0.85)">sana: ${date}</div>
      </div>
      <div class="sh sd-ep-r">${pct(progress * 100)} | ${clock(pos)}/${clock(dur)}</div>
    </div>
    <div class="sd-ep-bar"><div style="width:${(progress * 100).toFixed(3)}%"></div></div>
    ${cleared ? '<div class="sd-tag">Tarixdan tozalangan</div>' : ''}
  </div>`;
}

/** Maxsus emoji `[pe:to'plam:element:emoji]` -> oddiy emoji (`EmojiText` o'rnida). */
function emojiText(s) {
  return esc(`${s ?? ''}`.replace(/\[pe:\d{1,16}:\d{1,9}:([^\]]{0,16})\]/g, '$1')).replace(/\n/g, '<br>');
}

function mediaPlaceholder(type) {
  const gif = type === 'gif';
  return `<div class="sd-media">${icon(gif ? 'gif_box' : 'emoji_emotions', { fill: false, size: 18, color: 'rgba(255,255,255,0.7)' })}<span>${gif ? 'GIF' : 'Stiker'}</span></div>`;
}

function commentRowHtml(r) {
  const animeId = toInt(r.anime_id);
  const seasonId = toInt(r.season_id);
  const photo = imageUrl(`${r.photo_url ?? ''}`);
  const season = `${r.season_name ?? ''}`.trim();
  const anime = `${r.anime_name ?? ''}`.trim();
  const body = `${r.body ?? ''}`;
  const mediaType = `${r.media_type ?? ''}`;
  const mediaFile = `${r.media_file ?? ''}`;
  const hasMedia = mediaFile !== '' && (mediaType === 'sticker' || mediaType === 'gif');
  const isReply = `${r.parent_id ?? ''}` !== '';
  const what = mediaType === 'sticker' ? 'stiker' : mediaType === 'gif' ? 'GIF' : '';
  const caption = [isReply ? 'Javob' : 'Izoh', what ? `${what} yubordi` : '', body.trim() ? 'yozdi' : '']
    .filter(Boolean).join(' · ');
  const replies = toInt(r.reply_count);
  return `<div class="glass sd-cm">
    <div class="sd-cm-row">
      <div class="sd-cm-poster">${photo ? `<img alt="" loading="lazy" src="${esc(photo)}" onerror="this.remove()">` : ''}
        ${paidOf(animeId, seasonId) ? `<div class="sd-cm-paid">${paidBadge(true)}</div>` : ''}</div>
      <div class="sd-cm-main">
        <div class="sd-cm-t">${esc(season || anime || "Bo'lim")}</div>
        <div class="sd-cm-c">${esc(caption)}</div>
        ${hasMedia ? `<div style="margin-top:6px">${mediaPlaceholder(mediaType)}</div>` : ''}
        ${body.trim() ? `<div class="sd-cm-b">${emojiText(body)}</div>` : (!hasMedia && what ? `<div class="sd-cm-b">${what}</div>` : '')}
        <div class="sd-cm-f">
          ${icon('favorite', { size: 14, color: C.accent })}<span class="sd-cm-likes">${toInt(r.likes)}</span>
          ${replies > 0
    ? `<span class="sd-cm-rep">${icon('keyboard_arrow_down', { size: 18, color: C.accent3 })}<span>${replies} ta javob</span></span>`
    : '<span class="sd-cm-none">Javob yo\'q</span>'}
          <span class="grow"></span><span class="sd-cm-d">${shortDate(r.created_at)}</span>
        </div>
      </div>
    </div>
    <div class="sd-cm-replies" hidden></div>
  </div>`;
}

function replyHtml(c) {
  const fn = `${c.first_name ?? ''}`.trim();
  const un = `${c.username ?? ''}`.trim();
  const name = fn || (un ? `@${un}` : 'Foydalanuvchi');
  const photo = imageUrl(`${c.photo_url ?? ''}`);
  const mt = `${c.media_type ?? ''}`;
  const media = c.deleted !== true && (mt === 'sticker' || mt === 'gif') && `${c.media_file ?? ''}`;
  return `<div class="sd-rp">
    <div class="sd-rp-av">${photo ? `<img alt="" src="${esc(photo)}" onerror="this.remove()">` : ''}</div>
    <div class="sd-rp-main">
      <div class="sd-rp-h"><span class="sd-rp-n">${esc(name)}</span>${icon('favorite', { size: 12, color: C.accent })}<span class="sd-rp-l">${toInt(c.likes)}</span></div>
      ${media ? `<div style="padding:2px 0">${mediaPlaceholder(mt)}</div>` : ''}
      ${`${c.body ?? ''}`.trim() ? `<div class="sd-rp-b">${emojiText(c.body)}</div>` : ''}
    </div>
  </div>`;
}

function bindCommentRow(el, r) {
  const animeId = toInt(r.anime_id);
  const seasonId = toInt(r.season_id);
  const id = `${r.id ?? ''}`;
  const box = el.querySelector('.sd-cm-replies');
  const rep = el.querySelector('.sd-cm-rep');
  let open = false; let loading = false; let replies = null; let error = null;
  const poster = el.querySelector('.sd-cm-poster');
  if (animeId > 0) {
    poster.style.cursor = 'pointer';
    poster.addEventListener('click', () => hooks.openSeason(seasonsRepo.find(animeId, seasonId)
      || { anime_id: animeId, season_id: seasonId, nomi: r.season_name, photo_url: r.photo_url }));
  }
  function paint() {
    box.hidden = !open;
    if (rep) rep.querySelector('.ic').textContent = open ? 'keyboard_arrow_up' : 'keyboard_arrow_down';
    if (!open) return;
    let inner;
    if (loading) inner = `<div class="sd-spin">${spinner(22, 2)}</div>`;
    else if (error) inner = `<div class="sd-rp-err">${esc(error)}</div>`;
    else inner = (replies || []).map(replyHtml).join('');
    box.innerHTML = `<div class="sd-div"></div>${inner}`;
  }
  rep?.addEventListener('click', async () => {
    if (open) { open = false; paint(); return; }
    open = true; paint();
    if (replies || loading) return;
    if (animeId <= 0 || !id) { replies = []; paint(); return; }
    loading = true; error = null; paint();
    try {
      const j = await api(`/api/comments/${animeId}/${seasonId}/${encodeURIComponent(id)}`);
      replies = (Array.isArray(j?.items) ? j.items : []).slice().reverse();
    } catch (_) {
      error = 'Javoblar kelmadi';
    }
    loading = false; paint();
  });
}

function starBadge(stars) {
  return `<span class="sd-star">${icon('star', { size: 13, color: C.gold })}<span>${stars}</span></span>`;
}

/**
 * Bitta statistikaning ro'yxati.
 * `kind` — `STAT_KINDS` dagi kalit ('episodes', 'seasons', ...).
 */
export function openStatDetail({ userId, kind, owner = '', isMe = false }) {
  const k = typeof kind === 'string' ? STAT_KINDS[kind] : kind;
  if (!k || !k.openable) return;
  push((el) => {
    el.innerHTML = `${appBar({ title: esc(k.label), sub: owner.trim() ? esc(owner.trim()) : '' })}<div class="scroll"></div>`;
    bindAppBar(el);
    const scroll = el.querySelector('.scroll');
    const list = new UserStatsList(userId, k);
    let shown = 0;
    let wrap = null;

    function reset() { scroll.innerHTML = ''; wrap = null; shown = 0; }

    function render() {
      const rows = list.items;
      if (rows.length === 0) {
        reset();
        scroll.innerHTML = list.loading
          ? `<div class="sd-spin">${spinner(22, 2)}</div>`
          : `<div class="sd-empty">${esc(list.error ?? "Hozircha bo'sh")}</div>`;
        return;
      }
      if (!wrap) {
        scroll.innerHTML = '';
        wrap = document.createElement('div');
        wrap.className = k.key === 'episodes' || k.key === 'comments' ? 'sd-list' : 'grid sd-grid';
        scroll.appendChild(wrap);
        shown = 0;
      }
      // Faqat yangi qatorlar qo'shiladi (ochilgan javoblar yo'qolmasin).
      const fresh = rows.slice(shown);
      const w = cardWidth(scroll.clientWidth);
      const tmp = document.createElement('div');
      fresh.forEach((r) => {
        if (k.key === 'episodes') tmp.innerHTML = episodeRowHtml(r);
        else if (k.key === 'comments') tmp.innerHTML = commentRowHtml(r);
        else {
          const stars = toInt(r.my_stars);
          tmp.innerHTML = seasonCardHtml(r, w, { badges: false, corner: k.key === 'rated' && stars > 0 ? starBadge(stars) : '' });
        }
        const node = tmp.firstElementChild;
        wrap.appendChild(node);
        if (k.key === 'episodes') {
          bindTap(node, () => hooks.openSeasonIds(toInt(r.anime_id), toInt(r.season_id), toInt(r.epizod_id)), { scale: false });
        } else if (k.key === 'comments') {
          bindCommentRow(node, r);
        } else {
          bindTap(node, () => hooks.openSeason(r));
          bindImages(node);
        }
      });
      shown = rows.length;
      let sp = scroll.querySelector(':scope > .sd-spin');
      if (list.loading && !sp) {
        sp = document.createElement('div');
        sp.className = 'sd-spin';
        sp.innerHTML = spinner(22, 2);
        scroll.appendChild(sp);
      } else if (!list.loading && sp) sp.remove();
    }

    const onScroll = () => {
      const left = scroll.scrollHeight - scroll.clientHeight - scroll.scrollTop;
      if (left < 400) list.loadMore();
    };
    scroll.addEventListener('scroll', onScroll, { passive: true });
    const off = list.listen(() => { render(); if (!list.loading) requestAnimationFrame(onScroll); });
    list.refresh();
    return { dispose() { off(); } };
  });
}
