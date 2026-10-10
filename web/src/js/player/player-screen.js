// Pleyer ekrani — `lib/screens/video_player_screen.dart` ning onlayn nusxasi.
//
// Tepada pleyer (inline / to'liq ekran), tagida "hozir ko'rilmoqda" qatori,
// tablar (Ma'lumot | Qismlar | Bo'limlar | Izohlar) va qism navigatsiyasi.
// Yuklab olish / o'chirish / kesh YO'Q (faqat onlayn). Video fMP4 bo'lib
// Telegram orqali keladi (`source.js` + `engine.js`).

import { api, imageUrl } from '../api.js';
import { push, back as routerBack } from '../router.js';
import { hooks } from '../hooks.js';
import { icon, toast, promptDialog, bindTap } from '../ui.js';
import { esc, formatCount } from '../format.js';
import { seasonsRepo } from '../seasons.js';
import { billing } from '../services/billing.js';
import { channelGate, setVideoBusy } from '../services/channel-gate.js';
import { watchHistory, watchProgress } from '../services/history.js';
import { createCommentsTab } from '../comments.js';
import { subRequired, channelConsent, ratingSheet, qualityDialog } from './gates.js';
import { loadSeasonInfo, seasonFromDisk, rateSeason, setSeasonFavorite, SeasonInfo } from './season-info.js';
import {
  QUALITIES, fileNameOf, mp4Qualities, fmp4Qualities, requestFmp4, waitForFmp4,
  startPlayback, releasePlayback, explainError,
} from './source.js';
import {
  formatHours, formatMoment, formatRating, fileSizeLabel, fmtDur, introRangesOf,
  seasonIsFree, playerSettings,
} from './util.js';

const HIDE_MS = 5000;
const SPEEDS = [0.25, 0.5, 0.75, 1, 1.25, 1.5, 1.75, 2];
const toI = (v) => { const n = parseInt(`${v ?? 0}`, 10); return Number.isFinite(n) ? n : 0; };
const epNum = (e) => toI(e?.epizod_number);
const epId = (e) => toI(e?.epizod_id);
const epKey = (e) => `${e?.epizod_id ?? e?.epizod_number ?? ''}`;
const playable = (e) => fmp4Qualities(e).length > 0 || mp4Qualities(e).length > 0;
const qualitiesOf = (e) => {
  const set = new Set([...fmp4Qualities(e), ...mp4Qualities(e)]);
  return QUALITIES.filter((q) => set.has(q));
};

export function openSeason(season, opts = {}) {
  if (!season) return;
  push((el, route) => buildScreen(el, route, season, opts), { transition: 'fade' });
}

/** Faqat ID bilan (tarix, statistika, kutubxona). */
export function openSeasonIds(animeId, seasonId, epizodId, extra = {}) {
  const a = toI(animeId); const s = toI(seasonId);
  const season = extra.season || seasonsRepo.find(a, s) || { anime_id: a, season_id: s };
  openSeason({ ...season, anime_id: a, season_id: s }, {
    startEpizodId: epizodId ? toI(epizodId) : null,
    startAtMs: extra.startAtMs ?? null,
  });
}

function buildScreen(el, route, season, opts) {
  el.classList.add('pl');
  let mode = '';
  let gateOff = null;
  let player = null;

  const canWatch = () => billing.active || (seasonIsFree(season) && !channelGate.needsConsent);
  const needsConsent = () => !billing.active && seasonIsFree(season) && channelGate.needsConsent;

  function decide() {
    if (player && canWatch()) return;
    const next = needsConsent() ? 'consent' : canWatch() ? 'player' : 'sub';
    if (next === mode) return;
    mode = next;
    gateOff?.(); gateOff = null;
    if (next === 'player') { el.innerHTML = ''; player = buildPlayer(el, route, season, opts); }
    else if (next === 'consent') gateOff = channelConsent(el);
    else subRequired(el);
  }
  billing.load({ force: true }).catch(() => {});
  const offB = billing.listen(decide);
  const offC = channelGate.listen(decide);
  decide();

  return {
    onBack: () => (player ? player.onBack() : true),
    dispose() { offB(); offC(); gateOff?.(); player?.dispose(); },
  };
}

// ═════════════════════════════════════════════════════════════════════
function buildPlayer(el, route, season, opts) {
  const A = toI(season.anime_id);
  const S = toI(season.season_id);
  let info = seasonFromDisk(A, S);
  let eps = [];
  let loadingEps = true;
  let cur = null;
  let curName = '';
  let selQ = null;
  let userChose = false;
  let eng = null;
  let token = 0;
  let intended = true; // foydalanuvchi niyati: ijro yoki pauza
  let busy = false;
  let waiting = '';
  let errShown = false;
  let showCtl = true;
  let hideT = 0;
  let locked = false;
  let fs = false;
  let tab = 0;
  let ranges = [];
  let introIdx = -1;
  let introTries = {};
  let lastSkip = 0;
  let sleepLeft = 0;
  let sleepMin = 0;
  let sleepT = 0;
  let abort = null;
  let lastSave = 0;
  let lastTick = 0;
  let rate = 1;
  let drag = null;
  let disposed = false;
  let commentsCtl = null;
  let seasonsList = null;
  let noticeT = 0;

  const sStr = (k) => `${info?.season?.[k] ?? season[k] ?? ''}`;
  const sNum = (k) => { const v = info?.season?.[k] ?? season[k]; return toI(v); };
  const bolim = () => (sNum('bolim_id') > 0 ? sNum('bolim_id') : sNum('season_id') > 0 ? sNum('season_id') : 1);

  el.innerHTML = `
    <div class="pl-main">
      <div class="pl-top-part">
        <div class="pl-head">
          <div class="gbtn pl-back">${icon('arrow_back', { size: 24, color: '#fff' })}</div>
          <div class="nm"></div>
        </div>
        <div class="pl-boxwrap">${boxHtml()}</div>
        <div class="pl-now"></div>
        <div class="pl-tabs">
          ${['Ma\'lumot', 'Qismlar', 'Bo\'limlar', 'Izohlar'].map((t, i) => `<div class="pl-tab" data-i="${i}">${t}</div>`).join('')}
        </div>
        <div class="pl-epnav">
          <button class="prev">${icon('skip_previous', { size: 24 })}</button>
          <div class="lb"></div>
          <button class="next">${icon('skip_next', { size: 24 })}</button>
        </div>
      </div>
      <div class="pl-pages">
        <div class="pl-pane" data-p="0"></div>
        <div class="pl-pane" data-p="1"></div>
        <div class="pl-pane" data-p="2"></div>
        <div class="pl-pane cm-pane" data-p="3"></div>
      </div>
    </div>`;

  const $ = (s) => el.querySelector(s);
  const box = $('.pl-box');
  const video = $('video');
  const panes = [...el.querySelectorAll('.pl-pane')];
  const tabEls = [...el.querySelectorAll('.pl-tab')];

  function boxHtml() {
    return `<div class="pl-box show">
      <video playsinline webkit-playsinline preload="auto"></video>
      <div class="pl-empty"><span class="ic" style="font-size:52px;width:52px;height:52px;color:rgba(255,255,255,0.24)">play_circle</span><span>Ko'rmoqchi bo'lgan qismni tanlang</span></div>
      <div class="pl-err"><span class="ic" style="font-size:40px;width:40px;height:40px;color:rgba(255,255,255,0.38)">error</span><div class="et"></div><div class="rt">Qayta urinish</div></div>
      <div class="pl-gest"></div>
      <div class="pl-ctrl">
        <div class="pl-top pl-fs-only">
          <button class="fsback">${icon('arrow_back', { size: 24 })}</button>
          <div class="tt"><div class="t1"></div><div class="t2"></div></div>
          <button class="lock">${icon('lock_open', { size: 22 })}</button>
          <button class="gear">${icon('settings', { size: 22 })}</button>
          <button class="fsexit">${icon('fullscreen_exit', { size: 22 })}</button>
        </div>
        <button class="pl-more pl-nfs-only">${icon('more_vert', { size: 22 })}</button>
        <div class="pl-bottom">
          <div class="pl-brow pl-chips pl-fs-only">
            <div class="pl-chip eplist">${icon('playlist_play', { size: 16 })}</div>
            <div class="pl-chip speed">${icon('speed', { size: 16 })}<span class="sl">1x</span></div>
            <div class="pl-chip hqc">HQ</div>
          </div>
          <div class="pl-brow pl-fsbtns pl-fs-only">
            <button class="pl-fsb prev2">${icon('skip_previous', { size: 24 })}</button>
            <button class="pl-fsb main pp2">${icon('play_arrow', { size: 30 })}</button>
            <button class="pl-fsb next2">${icon('skip_next', { size: 24 })}</button>
          </div>
          <div class="pl-brow">
            <div class="pl-track"><div class="tr"></div><div class="bf"></div><div class="pd"></div><div class="th"></div></div>
            <div class="pl-time">0:00/0:00</div>
            <button class="pl-hq pl-nfs-only hq">HQ</button>
            <button class="pl-fsx pl-nfs-only fsbtn"><svg width="22" height="22" viewBox="0 0 24 24" fill="#fff"><path d="M6 14c-.55 0-1 .45-1 1v3c0 .55.45 1 1 1h3c.55 0 1-.45 1-1s-.45-1-1-1H7v-2c0-.55-.45-1-1-1zm0-4c.55 0 1-.45 1-1V7h2c.55 0 1-.45 1-1s-.45-1-1-1H6c-.55 0-1 .45-1 1v3c0 .55.45 1 1 1zm11 7h-2c-.55 0-1 .45-1 1s.45 1 1 1h3c.55 0 1-.45 1-1v-3c0-.55-.45-1-1-1s-1 .45-1 1v2zM14 7c0 .55.45 1 1 1h2v2c0 .55.45 1 1 1s1-.45 1-1V6c0-.55-.45-1-1-1h-3c-.55 0-1 .45-1 1z"/></svg></button>
          </div>
        </div>
      </div>
      <div class="pl-center"><div class="ring"><div class="pl-ring"></div></div><div class="pp">${icon('play_arrow', { size: 40, color: '#fff' })}</div></div>
      <div class="pl-wait"></div>
      <div class="pl-thin"><div class="bf"></div><div class="pd"></div></div>
      <div class="pl-badge l"></div><div class="pl-badge r"></div>
      <div class="pl-notice"><span></span></div>
      <div class="pl-intro">${icon('fast_forward', { size: 15, color: '#fff' })}<span>O'tkazish</span></div>
      <div class="pl-unlock">${icon('lock', { size: 20, color: '#E2620F' })}</div>
      <div class="pl-scrim"></div>
      <div class="pl-pan pl-menu"></div>
      <div class="pl-pan pl-speed"></div>
      <div class="pl-pan pl-eplist"></div>
      <div class="pl-pan pl-menu pl-set" style="top:50px;right:8px"></div>
      <div class="pl-sleep"><div class="pn"></div></div>
    </div>`;
  }

  // ── Umumiy yordamchilar ─────────────────────────────────────────
  const ordered = () => eps.filter(playable).sort((a, b) => epNum(b) - epNum(a));
  const curIndex = () => { const k = cur ? epKey(cur) : null; return k == null ? -1 : ordered().findIndex((e) => epKey(e) === k); };

  function notice(text, ms = 2600) {
    const n = $('.pl-notice');
    n.querySelector('span').textContent = text;
    n.classList.add('on');
    clearTimeout(noticeT);
    noticeT = setTimeout(() => n.classList.remove('on'), ms);
  }

  function paintBusy() {
    const b = busy || !!waiting;
    box.classList.toggle('busy', b);
    const w = $('.pl-wait');
    w.textContent = waiting;
    w.classList.toggle('on', !!waiting);
  }

  function paintPP() {
    const playing = intended;
    const ic = playing ? 'pause' : 'play_arrow';
    el.querySelectorAll('.pl-center .pp .ic, .pp2 .ic').forEach((n) => { n.textContent = ic; });
    $('.lock .ic').textContent = locked ? 'lock' : 'lock_open';
    $('.lock .ic').style.color = locked ? '#E2620F' : '#fff';
  }

  function paintShow() {
    box.classList.toggle('show', showCtl);
    box.classList.toggle('locked', locked);
  }

  function scheduleHide() {
    clearTimeout(hideT);
    hideT = setTimeout(() => { showCtl = false; paintShow(); }, HIDE_MS);
  }
  function keepShown() { showCtl = true; paintShow(); scheduleHide(); }
  function holdShown() { showCtl = true; paintShow(); clearTimeout(hideT); }

  // ── Sarlavhalar, ro'yxatlar ────────────────────────────────────
  function paintTitle() {
    const name = sStr('nomi');
    $('.pl-head .nm').textContent = name;
    $('.t1').textContent = name || (cur?.epizod_name ?? '');
    if (cur) {
      const en = `${cur.epizod_name ?? ''}`;
      $('.t2').textContent = `${bolim()}-bo'lim · ${epNum(cur)}-qism${en ? ` · ${en}` : ''}`;
    } else $('.t2').textContent = '';
  }

  function paintNow() {
    const host = $('.pl-now');
    if (!cur) { host.innerHTML = ''; return; }
    const views = toI(cur.views_total); const wm = toI(cur.watch_ms_total); const added = toI(cur.created_at);
    const stat = (ic, t) => `<span class="s">${icon(ic, { size: 11.5, fill: false, color: 'rgba(255,255,255,0.45)' })}${esc(t)}</span>`;
    host.innerHTML = `<div class="in"><span class="a">${bolim()}-bo'lim · ${epNum(cur)}-qism</span>
      ${stat('visibility', formatCount(views))}${stat('schedule', formatHours(wm))}${added > 0 ? stat('event_available', formatMoment(added)) : ''}</div>`;
  }

  function paintNav() {
    const list = ordered(); const i = curIndex();
    const hasNext = i > 0; const hasPrev = i >= 0 && i < list.length - 1;
    const lb = $('.pl-epnav .lb');
    lb.textContent = i >= 0 ? `${epNum(list[i])}-qism` : (list.length ? 'Qismni tanlang' : '—');
    lb.classList.toggle('dim', i < 0);
    $('.pl-epnav .prev').disabled = !hasPrev;
    $('.pl-epnav .next').disabled = !hasNext;
    $('.prev2').disabled = !hasPrev;
    $('.next2').disabled = !hasNext;
  }

  function paintEps() {
    const pane = panes[1];
    if (loadingEps) { pane.innerHTML = `<div class="mid"><div class="spinner" style="width:36px;height:36px"></div></div>`; return; }
    const list = ordered();
    if (!list.length) { pane.innerHTML = `<div class="mid">Qismlar topilmadi</div>`; return; }
    pane.innerHTML = `<div class="pl-epgrid">${list.map((e) => {
      const isCur = cur && epKey(e) === epKey(cur);
      return `<div class="pl-eg${isCur ? ' cur' : ''}" data-k="${esc(epKey(e))}">${epNum(e)}-qism</div>`;
    }).join('')}</div><div style="height:8px"></div>`;
    pane.querySelectorAll('.pl-eg').forEach((n) => n.addEventListener('click', () => {
      const e = list.find((x) => epKey(x) === n.dataset.k);
      if (!e) return;
      if (cur && epKey(e) === epKey(cur)) togglePlay();
      else playEpisode(e, { resumeMs: savedPosMs(e) });
    }));
  }

  function centerOnCur() {
    requestAnimationFrame(() => {
      const n = panes[1].querySelector('.pl-eg.cur');
      if (n && panes[1].clientHeight) panes[1].scrollTop = n.offsetTop - (panes[1].clientHeight - n.offsetHeight) / 2;
    });
  }

  async function loadSeasons() {
    const key = `aru_seasons_${A}`;
    try { const c = JSON.parse(localStorage.getItem(key) || 'null'); if (Array.isArray(c)) { seasonsList = c; paintSeasons(); } } catch (_) { /* */ }
    try {
      const fresh = await api(`/api/seasons/anime/${A}`);
      if (Array.isArray(fresh)) { seasonsList = fresh; try { localStorage.setItem(key, JSON.stringify(fresh)); } catch (_) { /* */ } }
    } catch (_) { /* */ }
    if (seasonsList == null) seasonsList = [];
    if (!disposed) paintSeasons();
  }

  function paintSeasons() {
    const pane = panes[2];
    if (seasonsList == null) { pane.innerHTML = `<div class="mid"><div class="spinner" style="width:36px;height:36px"></div></div>`; return; }
    if (!seasonsList.length) { pane.innerHTML = `<div class="mid">Bo'limlar topilmadi</div>`; return; }
    pane.innerHTML = seasonsList.map((s, i) => {
      const isCur = `${s.season_id}` === `${S}`;
      const b = s.bolim_id ?? s.season_id ?? '';
      const nm = `${s.nomi ?? ''}`;
      const u = imageUrl(s.photo_url);
      return `<div class="pl-sn${isCur ? ' cur' : ''}" data-i="${i}">
        <div class="ph">${u ? `<img src="${esc(u)}" alt="" loading="lazy" onerror="this.remove()">` : icon('movie', { fill: false, size: 24, color: 'rgba(255,255,255,0.38)' })}</div>
        <div class="tx"><div class="n1">${esc(nm || `${b}-bo'lim`)}</div><div class="n2">${esc(b)}-bo'lim</div></div>
        ${s.free === false ? `<span class="paid-badge compact">${icon('workspace_premium', { size: 13, color: '#FFC93C' })}</span>` : ''}
        ${isCur ? icon('play_arrow', { size: 24, color: '#C2410C' }) : ''}</div>`;
    }).join('') + '<div style="height:8px"></div>';
    pane.querySelectorAll('.pl-sn').forEach((n) => n.addEventListener('click', () => {
      const s = seasonsList[+n.dataset.i];
      if (!s || `${s.season_id}` === `${S}`) return;
      route.close();
      setTimeout(() => hooks.openSeason(s), 60);
    }));
  }

  function paintInfo() {
    const pane = panes[0];
    const mine = info?.myStars || 0;
    const fav = !!info?.isFav;
    const rc = sNum('rating_count');
    const line = (ic, l, v) => `<div class="pl-stat">${icon(ic, { fill: false, size: 16, color: 'rgba(255,255,255,0.45)' })}<span class="l">${l}:</span><span class="v">${esc(v)}</span></div>`;
    const inf = (l, v) => (v ? `<div class="pl-inf"><div class="l">${l}</div><div class="v">${esc(v)}</div></div>` : '');
    const tav = sStr('tavsif');
    const created = sNum('created_at');
    pane.innerHTML = `
      <div class="pl-acts">
        <div class="pl-act rate">${icon(mine > 0 ? 'star' : 'star', { fill: mine > 0, size: 19, color: '#FFC93C' })}<span class="lb">${mine > 0 ? `Bahoyingiz: ${mine}` : 'Baholash'}</span></div>
        <div class="pl-act fav">${icon('favorite', { fill: fav, size: 19, color: '#C2410C' })}<span class="lb">${fav ? 'Sevimlilarda' : "Sevimlilarga qo'shish"}</span></div>
      </div>
      <div class="pl-card">
        ${line('visibility', "Ko'rishlar", formatCount(sNum('views_total')))}
        ${line('schedule', 'Tomosha vaqti', `${formatHours(sNum('watch_ms_total'))} soat`)}
        ${line('favorite', "Sevimlilarga qo'shilgan", formatCount(sNum('fav_count')))}
        ${line('star', 'Reyting', rc > 0 ? `${formatRating(info?.rating || 0)}  (${formatCount(rc)} ta baho)` : 'hali baholanmagan')}
        ${line('movie_creation', "Bo'lim", `${bolim()}-bo'lim · ${formatCount(sNum('epizod_count'))} qism`)}
        ${created > 0 ? line('event_available', "Qo'shilgan sana", formatMoment(created)) : ''}
      </div>
      <div class="pl-card big">
        ${inf('Turi', sStr('turi'))}${inf('Yili', sStr('yili'))}${inf('Janri', sStr('janri'))}
        ${inf('Studio', sStr('studio'))}${inf('Tarjimon', sStr('tarjimon'))}${inf('Holati', sStr('holati'))}
        ${tav ? `<div class="pl-desc-t">Tavsif</div><div class="pl-desc">${esc(tav)}</div>` : ''}
      </div>`;
    pane.querySelector('.rate').addEventListener('click', async () => {
      const v = await ratingSheet(info?.myStars || 0);
      if (v == null) return;
      info = rateSeason(A, S, v, baseInfo());
      paintInfo();
    });
    pane.querySelector('.fav').addEventListener('click', () => {
      info = setSeasonFavorite(A, S, !(info?.isFav), baseInfo());
      paintInfo();
    });
  }
  const baseInfo = () => info || new SeasonInfo({ season: { ...season }, rating: 0, myStars: 0, isFav: false });

  function setTab(i) {
    tab = i;
    panes.forEach((p, k) => p.classList.toggle('on', k === i));
    tabEls.forEach((t, k) => t.classList.toggle('on', k === i));
    if (i === 1) centerOnCur();
    if (i === 3 && !commentsCtl) {
      commentsCtl = createCommentsTab(panes[3], {
        animeId: A, seasonId: S, expanded: false,
        onExpanded: (v) => { el.classList.toggle('cx', !!v); },
      });
    }
    if (i !== 3) { el.classList.remove('cx'); commentsCtl?.setExpanded(false); }
  }

  // ── Qismlarni yuklash ─────────────────────────────────────────
  const epsKey = `aru_eps_${A}_${S}`;
  async function loadEpisodes() {
    try {
      const c = JSON.parse(localStorage.getItem(epsKey) || 'null');
      if (Array.isArray(c) && c.length) { eps = c; loadingEps = false; afterEps(); }
    } catch (_) { /* */ }
    try {
      const fresh = await api(`/api/epizods/${A}/${S}`);
      if (Array.isArray(fresh)) {
        eps = fresh;
        try { localStorage.setItem(epsKey, JSON.stringify(fresh)); } catch (_) { /* */ }
        if (cur) { // ochiq qism yangi qatori bilan almashtiriladi (intro, sifatlar)
          const f = fresh.find((e) => epId(e) === epId(cur));
          if (f) { Object.assign(cur, f); ranges = introRangesOf(cur); introIdx = -1; introTries = {}; }
        }
      }
    } catch (_) { /* */ }
    loadingEps = false;
    if (!disposed) afterEps();
  }

  function afterEps() { paintEps(); paintNav(); autoOpen(); }

  function savedPosMs(e) {
    const q = qualitiesOf(e)[0] || '';
    const name = fileNameOf(e[`fmp4_url_${q}`] || e[`url_${q}`]);
    const local = name ? watchProgress.positionOf(name) : null;
    if (local != null) return local;
    const h = watchHistory.findEpisode(A, S, epId(e));
    return h && h.positionMs > 0 ? h.positionMs : null;
  }

  function autoOpen() {
    if (cur || disposed) return;
    const list = ordered();
    if (!list.length) return;
    let target = null; let at = null;
    if (opts.startEpizodId) {
      target = list.find((e) => epId(e) === opts.startEpizodId) || null;
      if (target) at = opts.startAtMs ?? null;
    }
    if (!target) {
      const last = watchHistory.lastOfSeason(A, S);
      if (last) { target = list.find((e) => epId(e) === last.epizodId) || null; if (target) at = savedPosMs(target); }
    }
    if (!target) target = list[list.length - 1];
    playEpisode(target, { resumeMs: at, playing: true });
  }

  // ── Ijro ───────────────────────────────────────────────────────
  function pickQuality(e) {
    const av = qualitiesOf(e);
    if (selQ && av.includes(selQ)) return selQ;
    if (!userChose) {
      const sv = watchHistory.qualityOf(A, S, epId(e));
      if (sv && av.includes(sv)) return sv;
    }
    return fmp4Qualities(e)[0] || av[0] || '';
  }

  function destroyEngine() {
    abort?.abort(); abort = null;
    try { eng?.destroy(); } catch (_) { /* */ }
    eng = null;
  }

  function showError(e) {
    const x = explainError(e);
    const er = $('.pl-err');
    er.querySelector('.et').innerHTML = `<b style="color:#fff">${esc(x.title)}</b><br>${esc(x.text)}`;
    er.classList.add('on');
    errShown = true; busy = false; waiting = ''; paintBusy();
  }
  function clearError() { $('.pl-err').classList.remove('on'); errShown = false; }

  async function playEpisode(e, { resumeMs = null, playing = true, recovery = false } = {}) {
    const q = pickQuality(e);
    if (!q) { toast('Bu qism hali tayyor emas'); return; }
    const my = ++token;
    destroyEngine();
    clearError();
    watchHistory.flush();
    cur = e; curName = '';
    intended = playing;
    busy = true; waiting = ''; paintBusy();
    $('.pl-empty').style.display = 'none';
    ranges = introRangesOf(e); introIdx = -1; introTries = {}; lastSkip = 0;
    $('.pl-intro').classList.remove('on');
    paintTitle(); paintNow(); paintNav(); paintEps(); paintPP();
    centerOnCur();
    setVideoBusy(true);
    holdShown();

    const urlOf = (qq) => e[`url_${qq}`] || e[`fmp4_url_${qq}`];
    watchHistory.startEpisode({
      animeId: A, seasonId: S, bolimId: toI(season.bolim_id), epizodId: epId(e), epizodNumber: epNum(e),
      quality: q, seasonName: `${season.nomi ?? ''}`, animeName: `${season.anime_name ?? ''}`,
      seasonPhoto: `${season.photo_url ?? ''}`, videoUrl: urlOf(q) || '', thumbUrl: urlOf(q) || '',
    });

    try {
      let start = resumeMs;
      if (start == null && !recovery) start = savedPosMs(e);
      if (!fileNameOf(e[`fmp4_url_${q}`])) {
        waiting = 'Video tayyorlanyabdi...'; paintBusy();
        const r = await requestFmp4(e).catch(() => null);
        if (my !== token) return;
        if (!(r && (r.ready || []).includes(q))) {
          abort = new AbortController();
          await waitForFmp4(e, q, { signal: abort.signal });
          if (my !== token) return;
        }
        waiting = '';
      }
      curName = fileNameOf(e[`fmp4_url_${q}`]);
      const secs = start && start > 0 ? start / 1000 : 0;
      eng = await startPlayback(video, e, q, {
        startAt: secs,
        onState: ({ state }) => { if (my !== token) return; busy = state === 'buffering' || state === 'loading'; paintBusy(); },
        onError: (er) => { if (my === token) showError(er); },
      });
      if (my !== token) { eng?.destroy(); return; }
      await eng.ready;
      if (my !== token) return;
      busy = false; paintBusy();
      if (intended) { try { await video.play(); } catch (_) { /* brauzer to'sdi — tugma bosiladi */ } }
      video.playbackRate = rate;
      scheduleHide();
    } catch (er) {
      if (my !== token || `${er?.message}` === 'aborted') return;
      showError(er);
    }
  }

  function stepEpisode(delta) {
    const list = ordered(); const i = curIndex();
    const t = i < 0 ? 0 : i - delta;
    if (t < 0 || t >= list.length) return;
    playEpisode(list[t], { resumeMs: savedPosMs(list[t]), playing: intended });
    keepShown();
  }

  function togglePlay() {
    if (!cur) return;
    if (errShown) return;
    if (!video.paused && !video.ended) { intended = false; video.pause(); }
    else {
      intended = true;
      if (video.ended || (video.duration && video.duration - video.currentTime < 3)) video.currentTime = 0;
      video.play().catch(() => {});
    }
    paintPP(); paintEps(); keepShown();
  }

  function seekTo(sec) {
    const d = video.duration || 0;
    video.currentTime = Math.max(0, Math.min(sec, d > 1 ? d - 1 : sec));
  }

  // ── Video hodisalari ──────────────────────────────────────────
  function bufferedEnd() {
    const t = video.currentTime; const r = video.buffered;
    for (let i = 0; i < r.length; i++) if (r.start(i) <= t + 0.3 && r.end(i) >= t) return r.end(i);
    return r.length ? r.end(r.length - 1) : 0;
  }

  function paintProgress() {
    const d = video.duration || 0;
    const t = drag != null ? drag * d : video.currentTime;
    const p = d > 0 ? Math.min(1, Math.max(0, t / d)) : 0;
    const b = d > 0 ? Math.min(1, bufferedEnd() / d) : 0;
    const tr = $('.pl-track');
    tr.querySelector('.pd').style.width = `${p * 100}%`;
    tr.querySelector('.bf').style.width = `${b * 100}%`;
    tr.querySelector('.th').style.left = `${p * 100}%`;
    $('.pl-thin .pd').style.width = `${p * 100}%`;
    $('.pl-thin .bf').style.width = `${b * 100}%`;
    $('.pl-thin').classList.toggle('on', d > 0);
    $('.pl-time').textContent = `${fmtDur(t * 1000)}/${fmtDur(d * 1000)}`;
  }

  function updateIntro(posMs) {
    const idx = ranges.findIndex(([a, b]) => posMs >= a && posMs < b);
    if (idx < 0) {
      if (introIdx !== -1) { introIdx = -1; $('.pl-intro').classList.remove('on'); }
      return;
    }
    const entered = idx !== introIdx;
    introIdx = idx;
    if (playerSettings.autoSkipIntro && (introTries[idx] || 0) < 3) {
      if (entered || Date.now() - lastSkip >= 1200) {
        introTries[idx] = (introTries[idx] || 0) + 1;
        lastSkip = Date.now();
        seekTo((ranges[idx][1] + 400) / 1000);
      }
      $('.pl-intro').classList.remove('on');
      return;
    }
    $('.pl-intro').classList.add('on');
  }

  video.addEventListener('timeupdate', () => {
    paintProgress();
    if (!cur || !curName || !video.duration) return;
    const pos = video.currentTime * 1000; const dur = video.duration * 1000;
    updateIntro(pos);
    const now = Date.now();
    if (now - lastSave >= 1000) {
      lastSave = now;
      watchProgress.save(curName, pos, dur);
      watchHistory.note(pos, dur);
    }
    if (!video.paused && rate === 1 && !video.seeking) {
      if (lastTick) { const dt = now - lastTick; if (dt > 0 && dt < 1500) watchHistory.addWatched(dt); }
      lastTick = now;
    } else lastTick = 0;
  });
  video.addEventListener('progress', paintProgress);
  video.addEventListener('durationchange', paintProgress);
  video.addEventListener('waiting', () => { busy = true; paintBusy(); });
  video.addEventListener('playing', () => { busy = false; paintBusy(); });
  video.addEventListener('seeking', () => { lastTick = 0; });
  video.addEventListener('pause', () => { if (!video.ended && intended === false) paintPP(); });
  video.addEventListener('ended', () => {
    intended = false; paintPP(); paintEps();
    if (playerSettings.autoNextEpisode && curIndex() > 0) { intended = true; stepEpisode(1); }
  });

  // ── Progress chizig'ini surish ─────────────────────────────────
  const track = $('.pl-track');
  let dragWasPlaying = false;
  const isRot = () => box.classList.contains('rot');
  const ratioAt = (e) => {
    const r = track.getBoundingClientRect();
    const v = isRot() ? (e.clientY - r.top) / r.height : (e.clientX - r.left) / r.width;
    return Math.min(1, Math.max(0, v));
  };
  track.addEventListener('pointerdown', (e) => {
    if (!video.duration) return;
    track.setPointerCapture(e.pointerId);
    drag = ratioAt(e); clearTimeout(hideT); paintProgress();
    // Surish davomida video pauzada turadi, qo'yib yuborilgach davom etadi.
    dragWasPlaying = !video.paused && !video.ended;
    if (dragWasPlaying) video.pause();
  });
  track.addEventListener('pointermove', (e) => { if (drag != null) { drag = ratioAt(e); paintProgress(); } });
  const endDrag = () => {
    if (drag == null) return;
    const v = drag; drag = null;
    seekTo(v * (video.duration || 0));
    paintProgress(); scheduleHide();
    // Joy tayyor bo'lgach (seeked) davom etadi — kutish paytida tezlashib ketmaydi.
    if (dragWasPlaying && intended) {
      const go = () => { video.removeEventListener('seeked', go); clearTimeout(t); video.play().catch(() => {}); };
      const t = setTimeout(go, 5000);
      video.addEventListener('seeked', go);
    }
    dragWasPlaying = false;
  };
  track.addEventListener('pointerup', endDrag);
  track.addEventListener('pointercancel', endDrag);

  // ── Gesturalar: bitta bosish — boshqaruv, ikki bosish — ±5 s ────
  let lastTap = 0; let lastSide = ''; let singleT = 0;
  let accL = 0; let accR = 0; const badgeT = {};
  let contUntil = 0; let contSide = ''; let pendSeek = null; let pendT = 0;
  const gest = $('.pl-gest');
  let gdown = null;
  gest.addEventListener('pointerdown', (e) => { gdown = { x: e.clientX, y: e.clientY, t: Date.now() }; });
  gest.addEventListener('pointerup', (e) => {
    const g = gdown; gdown = null;
    if (!g || !cur) return;
    if (Math.abs(e.clientX - g.x) > 14 || Math.abs(e.clientY - g.y) > 14 || Date.now() - g.t > 350) return;
    if (locked) { showCtl = !showCtl; paintShow(); if (showCtl) { clearTimeout(hideT); hideT = setTimeout(() => { showCtl = false; paintShow(); }, 3000); } return; }
    const r = gest.getBoundingClientRect();
    const side = (isRot() ? e.clientY - r.top < r.height / 2 : e.clientX - r.left < r.width / 2) ? 'l' : 'r';
    const now = Date.now();
    // Ikki marta bosib sek boshlangach, tez-tez bosishlar sekni davom ettiradi (boshqaruv chiqmaydi).
    if (now < contUntil && side === contSide) { clearTimeout(singleT); lastTap = now; lastSide = side; doubleSeek(side); return; }
    if (lastSide === side && now - lastTap < 300) {
      clearTimeout(singleT); lastTap = now;
      doubleSeek(side);
    } else {
      lastTap = now; lastSide = side;
      clearTimeout(singleT);
      singleT = setTimeout(() => {
        lastTap = 0;
        if (errShown) return;
        showCtl = !showCtl; paintShow();
        if (showCtl) scheduleHide(); else clearTimeout(hideT);
      }, 300);
    }
  });

  function doubleSeek(side) {
    const d = video.duration || 0;
    const base = video.currentTime;
    const from = pendSeek ?? base;
    contUntil = Date.now() + 700; contSide = side;
    if (side === 'l') accL = Math.min(accL + 5, Math.floor(base)); else accR = Math.min(accR + 5, Math.max(0, Math.floor(d - 1 - base)));
    const b = $(`.pl-badge.${side}`);
    const acc = side === 'l' ? accL : accR;
    const chev = [0, 1, 2].map((k) => `<i style="animation-delay:${(side === 'l' ? 2 - k : k) * 0.135}s">${icon(side === 'l' ? 'arrow_left' : 'arrow_right', { size: 13, color: '#fff' })}</i>`).join('');
    b.innerHTML = `<div class="ch">${chev}</div><span>${acc}s</span>`;
    b.classList.add('on');
    // Sek qisqa tinchlikdan keyin BITTA marta bajariladi (har bosishda dvigatel qayta ishga tushmasin).
    pendSeek = Math.max(0, Math.min(from + (side === 'l' ? -5 : 5), d > 1 ? d - 1 : from + 5));
    clearTimeout(pendT);
    pendT = setTimeout(() => { const t = pendSeek; pendSeek = null; if (t != null) seekTo(t); }, 250);
    try { window.Telegram?.WebApp?.HapticFeedback?.impactOccurred('light'); } catch (_) { /* */ }
    clearTimeout(badgeT[side]);
    badgeT[side] = setTimeout(() => { b.classList.remove('on'); if (side === 'l') accL = 0; else accR = 0; }, 700);
    if (showCtl) scheduleHide();
  }

  // ── Tugmalar ──────────────────────────────────────────────────
  $('.pl-back').addEventListener('click', () => routerBack());
  $('.pl-center .pp').addEventListener('click', togglePlay);
  $('.pp2').addEventListener('click', togglePlay);
  $('.pl-epnav .prev').addEventListener('click', () => stepEpisode(-1));
  $('.pl-epnav .next').addEventListener('click', () => stepEpisode(1));
  $('.prev2').addEventListener('click', () => stepEpisode(-1));
  $('.next2').addEventListener('click', () => stepEpisode(1));
  $('.pl-err .rt').addEventListener('click', () => { if (cur) playEpisode(cur, { resumeMs: video.currentTime * 1000 || savedPosMs(cur) }); });
  tabEls.forEach((t) => t.addEventListener('click', () => setTab(+t.dataset.i)));
  $('.pl-intro').addEventListener('click', () => {
    if (introIdx < 0) return;
    introTries[introIdx] = 3;
    const to = ranges[introIdx][1];
    introIdx = -1; $('.pl-intro').classList.remove('on');
    seekTo((to + 400) / 1000);
  });
  $('.hq').addEventListener('click', openQuality);
  $('.hqc').addEventListener('click', openQuality);
  $('.fsbtn').addEventListener('click', () => setFs(true));
  $('.fsexit').addEventListener('click', () => setFs(false));
  $('.fsback').addEventListener('click', () => setFs(false));
  $('.lock').addEventListener('click', () => {
    locked = !locked;
    if (locked) { closePanels(); showCtl = false; } else keepShown();
    paintPP(); paintShow();
  });
  $('.pl-unlock').addEventListener('click', () => { locked = false; keepShown(); paintPP(); });

  async function openQuality() {
    if (!cur) return;
    holdShown();
    const have = qualitiesOf(cur);
    const sel = pickQuality(cur);
    const q = await qualityDialog(have.map((x) => ({
      q: x, title: x, size: fileSizeLabel(cur[`size_${x}`]), sel: x === sel,
    })));
    scheduleHide();
    if (!q || q === sel) return;
    selQ = q; userChose = true;
    playEpisode(cur, { resumeMs: video.currentTime * 1000, playing: intended, recovery: true });
  }

  // ── Panellar (menyu, tezlik, qismlar, sozlamalar, uxlash) ─────
  const scrim = $('.pl-scrim');
  const pMenu = $('.pl-menu:not(.pl-set)'); const pSet = $('.pl-set');
  const pSpeed = $('.pl-speed'); const pEps = $('.pl-eplist');
  function closePanels() {
    [pMenu, pSet, pSpeed, pEps, scrim].forEach((n) => n.classList.remove('on'));
    if (!locked) scheduleHide();
  }
  scrim.addEventListener('click', closePanels);

  const toggleRow = (id, ic, label, on) => `<div class="pl-row" data-id="${id}">${icon(ic, { size: 20, color: 'rgba(255,255,255,0.7)' })}<span class="lb">${label}</span>${icon(on ? 'toggle_on' : 'toggle_off', { size: 30, color: on ? '#C2410C' : 'rgba(255,255,255,0.3)' })}</div>`;
  function settingsHtml() {
    return `${toggleRow('skip', 'fast_forward', "Avto intro o'tkazish", playerSettings.autoSkipIntro)}<div class="pl-hr"></div>
      ${toggleRow('next', 'skip_next', "Avto qism o'tkazish", playerSettings.autoNextEpisode)}<div class="pl-hr"></div>
      <div class="pl-row${sleepMin > 0 ? ' acc' : ''}" data-id="sleep">${icon('schedule', { size: 20, color: sleepMin > 0 ? '#C2410C' : 'rgba(255,255,255,0.7)' })}<span class="lb">${sleepMin > 0 ? `Uxlash: ${sleepLabel()}` : 'Uxlash vaqti'}</span></div>`;
  }
  function bindSettings(p) {
    p.innerHTML = settingsHtml();
    p.querySelectorAll('.pl-row').forEach((r) => r.addEventListener('click', () => {
      const id = r.dataset.id;
      if (id === 'skip') {
        const on = !playerSettings.autoSkipIntro;
        playerSettings.setAutoSkipIntro(on);
        if (on) { introTries = {}; lastSkip = 0; if (introIdx >= 0) { introTries[introIdx] = 1; lastSkip = Date.now(); seekTo((ranges[introIdx][1] + 400) / 1000); $('.pl-intro').classList.remove('on'); } }
        bindSettings(p);
      } else if (id === 'next') { playerSettings.setAutoNextEpisode(!playerSettings.autoNextEpisode); bindSettings(p); }
      else if (id === 'sleep') { closePanels(); openSleep(); }
    }));
  }
  $('.pl-more').addEventListener('click', () => {
    if (pMenu.classList.contains('on')) { closePanels(); return; }
    bindSettings(pMenu);
    const r = $('.pl-more').getBoundingClientRect();
    pMenu.style.top = `${r.bottom + 6}px`;
    pMenu.style.right = `${Math.max(8, window.innerWidth - r.right)}px`;
    pMenu.classList.add('on'); scrim.classList.add('on'); clearTimeout(hideT);
  });
  $('.gear').addEventListener('click', () => { bindSettings(pSet); pSet.classList.add('on'); scrim.classList.add('on'); clearTimeout(hideT); });
  $('.speed').addEventListener('click', () => {
    pSpeed.innerHTML = SPEEDS.map((s) => `<div class="${s === rate ? 'sel' : ''}" data-s="${s}">${s}×</div>`).join('');
    pSpeed.querySelectorAll('div').forEach((d) => d.addEventListener('click', () => {
      rate = +d.dataset.s; video.playbackRate = rate; $('.sl').textContent = `${rate}x`; closePanels();
    }));
    pSpeed.classList.add('on'); scrim.classList.add('on'); clearTimeout(hideT);
  });
  $('.eplist').addEventListener('click', () => {
    const list = ordered();
    pEps.innerHTML = `<div class="hd"><span class="n">${esc(sStr('nomi'))} · ${bolim()}-bo'lim</span><span class="c">${list.length} qism</span></div>
      <div class="pl-hr"></div>${list.map((e) => {
    const c = cur && epKey(e) === epKey(cur);
    const en = `${e.epizod_name ?? ''}`;
    return `<div class="it${c ? ' cur' : ''}" data-k="${esc(epKey(e))}">${c ? icon('play_arrow', { size: 20, color: '#C2410C' }) : ''}${epNum(e)}-qism${en ? `<span class="en">${esc(en)}</span>` : ''}</div>`;
  }).join('')}`;
    pEps.querySelectorAll('.it').forEach((n) => n.addEventListener('click', () => {
      const e = list.find((x) => epKey(x) === n.dataset.k);
      closePanels();
      if (e && !(cur && epKey(e) === epKey(cur))) playEpisode(e, { resumeMs: savedPosMs(e), playing: intended });
    }));
    pEps.classList.add('on'); scrim.classList.add('on'); clearTimeout(hideT);
    const c = pEps.querySelector('.it.cur'); if (c) pEps.scrollTop = c.offsetTop - 90;
  });

  // Uxlash vaqti
  const sleepEl = $('.pl-sleep');
  function sleepLabel() { const m = Math.floor(sleepLeft / 60); return `${String(m).padStart(2, '0')}:${String(sleepLeft % 60).padStart(2, '0')}`; }
  function setSleep(min) {
    clearInterval(sleepT);
    sleepMin = min; sleepLeft = min * 60;
    if (min > 0) {
      sleepT = setInterval(() => {
        sleepLeft -= 1;
        if (sleepLeft <= 0) { clearInterval(sleepT); sleepMin = 0; sleepLeft = 0; intended = false; video.pause(); paintPP(); paintEps(); }
      }, 1000);
    }
    sleepEl.classList.remove('on');
  }
  function openSleep() {
    const opts2 = [[0, "O'chirish"], [15, '15 daqiqa'], [30, '30 daqiqa'], [60, '60 daqiqa'], [120, '120 daqiqa']];
    sleepEl.querySelector('.pn').innerHTML = `<div class="h">Uxlash vaqti</div>${sleepMin > 0 ? `<div class="left">${sleepLabel()}</div>` : ''}
      <div class="opts">${opts2.map(([m, l]) => `<div class="o${m > 0 && m === sleepMin ? ' sel' : ''}" data-m="${m}">${l}</div>`).join('')}<div class="o" data-m="x">Qo'lda kiritish</div></div>`;
    sleepEl.querySelectorAll('.o').forEach((o) => o.addEventListener('click', async () => {
      if (o.dataset.m === 'x') {
        sleepEl.classList.remove('on');
        const v = await promptDialog('Daqiqa kiriting', { type: 'number', placeholder: '0', ok: 'OK', maxLength: 4 });
        const n = toI(v);
        if (n > 0) setSleep(n);
      } else setSleep(+o.dataset.m);
    }));
    sleepEl.classList.add('on');
    sleepEl.onclick = (e) => { if (e.target === sleepEl) sleepEl.classList.remove('on'); };
  }

  // ── To'liq ekran ──────────────────────────────────────────────
  // To'liq ekran: avval haqiqiy (element) to'liq ekran + gorizontal qulf; bo'lmasa
  // Telegram to'liq ekrani va portretda videoni 90° AYLANTIRIB gorizontal qilamiz.
  function applyRot() {
    const portrait = window.innerHeight > window.innerWidth;
    const rot = fs && portrait;
    box.classList.toggle('rot', rot);
    if (rot) {
      box.style.width = `${window.innerHeight}px`;
      box.style.height = `${window.innerWidth}px`;
      box.style.transform = `translateX(${window.innerWidth}px) rotate(90deg)`;
    } else { box.style.width = ''; box.style.height = ''; box.style.transform = ''; }
  }
  window.addEventListener('resize', applyRot);
  const onFsChange = () => { if (fs && nativeFs && !document.fullscreenElement) setFs(false); };
  document.addEventListener('fullscreenchange', onFsChange);
  let nativeFs = false;

  async function setFs(on) {
    if (fs === on) return;
    fs = on;
    box.classList.toggle('fs', on);
    closePanels();
    const tg = window.Telegram?.WebApp;
    try {
      if (on) {
        nativeFs = false;
        try { await document.documentElement.requestFullscreen?.({ navigationUI: 'hide' }); nativeFs = !!document.fullscreenElement; } catch (_) { /* */ }
        if (nativeFs) await screen.orientation?.lock?.('landscape').catch(() => {});
        else tg?.requestFullscreen?.();
        await new Promise((r) => setTimeout(r, 250));
      } else {
        const was = nativeFs; nativeFs = false;
        try { screen.orientation?.unlock?.(); } catch (_) { /* */ }
        if (was && document.fullscreenElement) await document.exitFullscreen().catch(() => {});
        else tg?.exitFullscreen?.();
      }
    } catch (_) { /* */ }
    applyRot();
    setTimeout(applyRot, 350);
    keepShown();
  }

  // ── Tablar bo'yicha surish ────────────────────────────────────
  let sx = 0; let sy = 0;
  const pages = $('.pl-pages');
  pages.addEventListener('touchstart', (e) => { sx = e.touches[0].clientX; sy = e.touches[0].clientY; }, { passive: true });
  pages.addEventListener('touchend', (e) => {
    const dx = e.changedTouches[0].clientX - sx; const dy = e.changedTouches[0].clientY - sy;
    if (Math.abs(dx) < 70 || Math.abs(dx) < Math.abs(dy) * 1.6) return;
    const n = tab + (dx < 0 ? 1 : -1);
    if (n >= 0 && n < 4) setTab(n);
  }, { passive: true });

  // ── Ilova fonga ketsa ─────────────────────────────────────────
  const onVis = () => {
    if (document.hidden) { if (!video.paused) video.pause(); watchHistory.flush(); watchProgress.flush(); }
    else if (intended && cur && !errShown) video.play().catch(() => {});
  };
  document.addEventListener('visibilitychange', onVis);
  // Izohdagi GIF ovozi yoqilsa pleyer pauza bo'ladi, o'chirilsa davom etadi.
  let pausedByPack = false;
  const onPackSound = (e) => {
    if (e.detail?.on) {
      if (cur && !video.paused && !video.ended) { pausedByPack = true; intended = false; video.pause(); paintPP(); paintEps(); }
    } else if (pausedByPack) {
      pausedByPack = false; intended = true; video.play().catch(() => {}); paintPP(); paintEps();
    }
  };
  window.addEventListener('aru-packsound', onPackSound);

  // ── Boshlash ──────────────────────────────────────────────────
  paintTitle(); paintNav(); paintInfo(); paintEps(); paintSeasons();
  setTab(0); paintPP(); paintShow(); paintBusy();
  loadEpisodes();
  loadSeasons();
  loadSeasonInfo(A, S).then((i) => { if (i && !disposed) { info = i; paintInfo(); paintTitle(); } });
  watchHistory.loadFromDisk?.();
  watchHistory.load?.().catch?.(() => {});

  return {
    onBack() {
      if (sleepEl.classList.contains('on')) { sleepEl.classList.remove('on'); return false; }
      if (scrim.classList.contains('on')) { closePanels(); return false; }
      if (fs) { setFs(false); return false; }
      if (el.classList.contains('cx')) { el.classList.remove('cx'); commentsCtl?.setExpanded(false); return false; }
      return true;
    },
    dispose() {
      disposed = true;
      token += 1;
      clearTimeout(hideT); clearTimeout(noticeT); clearTimeout(singleT); clearInterval(sleepT);
      document.removeEventListener('visibilitychange', onVis);
      window.removeEventListener('aru-packsound', onPackSound);
      window.removeEventListener('resize', applyRot);
      document.removeEventListener('fullscreenchange', onFsChange);
      try { if (document.fullscreenElement) document.exitFullscreen(); } catch (_) { /* */ }
      try { if (curName && video.duration) watchProgress.save(curName, video.currentTime * 1000, video.duration * 1000); } catch (_) { /* */ }
      watchProgress.flush();
      watchHistory.flush();
      destroyEngine();
      commentsCtl?.dispose?.();
      releasePlayback();
      setVideoBusy(false);
      try { screen.orientation?.unlock?.(); if (fs) window.Telegram?.WebApp?.exitFullscreen?.(); } catch (_) { /* */ }
    },
  };
}
