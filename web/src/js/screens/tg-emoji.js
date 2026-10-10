// Emoji: matnda to'liq rangda ko'rsatish (`lib/widgets/emoji_text.dart`)
// va Telegram'dagidek emoji paneli (`lib/widgets/tg_composer.dart` ->
// `TgInputArea`, `_EmojiPage`). Support chat va Izohlar ikkalasi shuni
// ishlatadi.
//
// Panelning "GIF" va "Stikerlar" oynalari hamda maxsus (to'plam) emojilar
// bu yerda YO'Q: ular ilovaning o'z to'plamlaridan Telegram orqali
// bo'lak-bo'lak o'qiladi (`pack_service.dart`) — Mini App'da hali
// ko'chirilmagan. Eski `[ce:..]` / `[pe:..]` belgilari oddiy emoji bo'lib
// chiqadi (ilovadagi `plainEmojiText` kabi).
//
//   richText(text)                 — HTML: emoji to'liq rangda, \n -> <br>
//   plainEmojiText(text)           — belgilarsiz oddiy matn
//   emojiButton() / createEmojiPanel({onPick, onBackspace})
//   bindEmojiInput({button, panelHost, input, onChange})
//   insertAtCursor(input, s), backspaceAt(input)

import { icon } from '../ui.js';
import { esc } from '../format.js';
import { openMyPacks } from './packs.js';
import { createPackPage } from './pack-panel.js';

const CE = /\[ce:(-?\d{1,20}):([^\]]{1,16})\]/g;
const PE = /\[pe:(\d{1,16}):(\d{1,9}):([^\]]{0,16})\]/g;

/** Eski maxsus emoji va to'plam emojilari — oddiy emoji. */
export function plainEmojiText(text) {
  let t = `${text ?? ''}`;
  if (t.includes('[ce:')) t = t.replace(CE, (_, __, e) => e);
  if (t.includes('[pe:')) t = t.replace(PE, (_, __, ___, e) => e || '');
  return t;
}

const seg = typeof Intl !== 'undefined' && Intl.Segmenter ? new Intl.Segmenter(undefined, { granularity: 'grapheme' }) : null;

/** Grafema bo'laklari (emoji bitta belgi emas). */
export function graphemes(s) {
  if (!s) return [];
  if (seg) return [...seg.segment(s)].map((x) => x.segment);
  return Array.from(s);
}

/** `_isEmojiCluster` */
export function isEmojiCluster(cluster) {
  const cps = Array.from(cluster).map((c) => c.codePointAt(0));
  if (!cps.length) return false;
  if (cps.includes(0xFE0F) || cps.includes(0x20E3)) return true;
  const c = cps[0];
  if (c >= 0x1F1E6 && c <= 0x1F1FF) return true;
  if (c >= 0x1F300 && c <= 0x1FAFF) return true;
  if (c >= 0x1F000 && c <= 0x1F2FF) return true;
  if (c >= 0x2600 && c <= 0x27BF) return true;
  if (c >= 0x2B00 && c <= 0x2BFF) return true;
  return false;
}

/**
 * Matnni HTML qiladi: harflar berilgan rangda (alpha bilan), EMOJI esa
 * to'liq rangda (`.emj`) — "qoramtir emoji" bo'lmasin.
 */
export function richText(text) {
  const t = plainEmojiText(text);
  if (!t) return '';
  let out = '';
  let buf = '';
  let bufEmoji = null;
  const flush = () => {
    if (!buf) return;
    const h = esc(buf).replace(/\n/g, '<br>');
    out += bufEmoji ? `<span class="emj">${h}</span>` : h;
    buf = '';
  };
  for (const ch of graphemes(t)) {
    const e = isEmojiCluster(ch);
    if (bufEmoji !== null && bufEmoji !== e) flush();
    bufEmoji = e;
    buf += ch;
  }
  flush();
  return out;
}

// ── Matn maydoniga yozish ──────────────────────────────────────

export function insertAtCursor(input, s) {
  const a = input.selectionStart ?? input.value.length;
  const b = input.selectionEnd ?? input.value.length;
  const max = input.maxLength > 0 ? input.maxLength : Infinity;
  const next = input.value.slice(0, a) + s + input.value.slice(b);
  if (next.length > max) return;
  input.value = next;
  const p = a + s.length;
  try { input.setSelectionRange(p, p); } catch (_) { /* */ }
  input.dispatchEvent(new Event('input', { bubbles: true }));
}

/** Kursor oldidagi bitta grafemani (emoji butunligicha) o'chiradi. */
export function backspaceAt(input) {
  const a = input.selectionStart ?? input.value.length;
  const b = input.selectionEnd ?? input.value.length;
  let v = input.value;
  let p = a;
  if (a !== b) {
    v = v.slice(0, a) + v.slice(b);
  } else {
    if (a === 0) return;
    const parts = graphemes(v.slice(0, a));
    const last = parts.pop() || '';
    p = a - last.length;
    v = v.slice(0, p) + v.slice(a);
  }
  input.value = v;
  try { input.setSelectionRange(p, p); } catch (_) { /* */ }
  input.dispatchEvent(new Event('input', { bubbles: true }));
}

// ── Yaqinda ishlatilganlar ─────────────────────────────────────

const RECENT_KEY = 'aru_recent_emoji';
function recentLoad() {
  try { const l = JSON.parse(localStorage.getItem(RECENT_KEY) || '[]'); return Array.isArray(l) ? l.filter((x) => typeof x === 'string') : []; } catch (_) { return []; }
}
function recentNote(e) {
  const l = recentLoad().filter((x) => x !== e);
  l.unshift(e);
  if (l.length > 35) l.length = 35;
  try { localStorage.setItem(RECENT_KEY, JSON.stringify(l)); } catch (_) { /* */ }
}

// Telegram `EmojiTabsStrip` belgilari (Lottie o'rnida — Material ikonkalar).
const GROUP_ICONS = {
  smileys: 'sentiment_satisfied', animals: 'pets', food: 'lunch_dining', activity: 'sports_soccer',
  travel: 'directions_car', objects: 'lightbulb', symbols: 'emoji_symbols', flags: 'flag',
};

const STRIP_ITEM = 33;
const STRIP_H = 40;
const HEADER_H = 30;

/** 🙂 / ⌨ tugmasi (44x44, belgi 26, oq 55%). */
export function emojiButtonHtml() {
  return `<div class="tge-btn">${icon('sentiment_satisfied', { fill: false, size: 26, color: 'rgba(255,255,255,0.55)', cls: 'tge-smile' })}${icon('keyboard', { fill: false, size: 26, color: 'rgba(255,255,255,0.55)', cls: 'tge-kb' })}</div>`;
}

/**
 * Emoji paneli (`TgMediaPanel` -> Emoji sahifasi). Qaytaradi: element.
 * `onPick(emoji)`, `onBackspace()`.
 */
export function createEmojiPanel({ onPick, onBackspace, onPickMedia }) {
  const el = document.createElement('div');
  el.className = 'tge-panel';
  el.innerHTML = `<div class="tge-scroll"></div><div class="tge-strip"><div class="tge-strip-in"><div class="tge-sel"></div></div></div>
    <div class="tge-pkv"></div>
    <div class="tge-bs">${icon('backspace', { fill: false, size: 20, color: 'rgba(255,255,255,0.7)' })}</div>
    <div class="tge-gear">${icon('settings', { fill: false, size: 22, color: 'rgba(255,255,255,0.7)' })}</div>
    ${onPickMedia ? `<div class="tge-pill">${['Emoji', 'GIF', 'Stiker'].map((t, i) => `<div class="tge-pt" data-t="${i}">${t}</div>`).join('')}</div>` : ''}`;
  const scroll = el.querySelector('.tge-scroll');
  const strip = el.querySelector('.tge-strip');
  const stripIn = el.querySelector('.tge-strip-in');
  const sel = el.querySelector('.tge-sel');
  let recent = recentLoad();
  const sections = () => [
    { title: 'Yaqinda ishlatilgan', emoji: recent, ic: 'schedule' },
    ...TG_EMOJI_GROUPS.map((g) => ({ title: g.title, emoji: g.emoji, ic: GROUP_ICONS[g.id] || 'circle' })),
  ];
  let secs = sections();
  let current = 0;
  let built = false;
  let cols = 8;
  let cell = 40;

  stripIn.style.width = `${secs.length * STRIP_ITEM}px`;
  stripIn.insertAdjacentHTML('beforeend', secs.map((s, i) => `<div class="tge-tab" data-i="${i}">${icon(s.ic, { fill: false, size: 22 })}</div>`).join(''));
  const tabs = [...stripIn.querySelectorAll('.tge-tab')];

  function paintStrip() {
    sel.style.transform = `translateX(${current * STRIP_ITEM + 1.5}px)`;
    tabs.forEach((t, i) => { t.querySelector('.ic').style.color = i === current ? '#fff' : '#9A9AA0'; });
    const x = 6 + current * STRIP_ITEM;
    const view = strip.clientWidth;
    if (view && (x < strip.scrollLeft || x + STRIP_ITEM > strip.scrollLeft + view)) {
      strip.scrollTo({ left: Math.max(0, x - view / 2 + STRIP_ITEM / 2), behavior: 'smooth' });
    }
  }

  function build() {
    const w = (el.clientWidth || window.innerWidth) - 10;
    cols = Math.max(7, Math.floor(w / 45));
    cell = w / cols;
    secs = sections();
    scroll.innerHTML = `<div style="height:${STRIP_H}px"></div>${secs.map((s, si) => (s.emoji.length ? `
      <div class="tge-sec" data-s="${si}">
        <div class="tge-head">${esc(s.title)}</div>
        <div class="tge-grid" style="grid-template-columns:repeat(${cols},${cell}px);grid-auto-rows:${cell}px">
          ${s.emoji.map((e) => `<div class="tge-cell" style="font-size:${(cell * 0.6).toFixed(1)}px">${e}</div>`).join('')}
        </div></div>` : '')).join('')}<div style="height:64px"></div>`;
    built = true;
  }

  function offsetOf(si) {
    const s = scroll.querySelector(`.tge-sec[data-s="${si}"]`);
    return s ? s.offsetTop - STRIP_H : 0;
  }

  let stripDy = 0;
  let lastOff = 0;
  let jumping = false;
  scroll.addEventListener('scroll', () => {
    const off = scroll.scrollTop;
    if (!jumping) {
      stripDy = off <= 0 ? 0 : Math.min(0, Math.max(-STRIP_H, stripDy - (off - lastOff)));
      strip.style.transform = `translateY(${stripDy}px)`;
    }
    lastOff = off;
    if (jumping) return;
    const secEls = [...scroll.querySelectorAll('.tge-sec')];
    let at = 0;
    for (const s of secEls) {
      if (s.offsetTop - STRIP_H <= off + 4) at = Number(s.dataset.s);
    }
    if (at !== current) { current = at; paintStrip(); }
  }, { passive: true });

  tabs.forEach((t, i) => t.addEventListener('click', () => {
    if (!secs[i].emoji.length) return;
    current = i;
    paintStrip();
    jumping = true;
    stripDy = 0;
    strip.style.transform = 'translateY(0px)';
    scroll.scrollTo({ top: offsetOf(i), behavior: 'smooth' });
    setTimeout(() => { jumping = false; lastOff = scroll.scrollTop; }, 350);
  }));

  // Katak bosilganda 0.8 gacha kichrayadi (`_Press`).
  scroll.addEventListener('pointerdown', (e) => {
    const c = e.target.closest('.tge-cell');
    if (c) c.classList.add('down');
  });
  const up = () => scroll.querySelectorAll('.tge-cell.down').forEach((c) => c.classList.remove('down'));
  scroll.addEventListener('pointerup', up);
  scroll.addEventListener('pointercancel', up);
  scroll.addEventListener('pointerleave', up);
  scroll.addEventListener('click', (e) => {
    const c = e.target.closest('.tge-cell');
    if (!c) return;
    const em = c.textContent;
    recentNote(em);
    onPick?.(em);
  });

  // ⌫ — bosib turilsa ketma-ket o'chiradi.
  const bs = el.querySelector('.tge-bs');
  let rep = 0;
  let hold = 0;
  bs.addEventListener('pointerdown', (e) => {
    e.preventDefault();
    bs.classList.add('down');
    let n = 0;
    hold = setTimeout(() => {
      rep = setInterval(() => { n++; onBackspace?.(); if (n > 12) onBackspace?.(); }, 60);
    }, 450);
  });
  const stop = (fire) => {
    bs.classList.remove('down');
    const repeating = !!rep;
    clearTimeout(hold); clearInterval(rep); rep = 0; hold = 0;
    if (fire && !repeating) onBackspace?.();
  };
  bs.addEventListener('pointerup', () => stop(true));
  bs.addEventListener('pointercancel', () => stop(false));
  bs.addEventListener('pointerleave', () => { if (hold || rep) stop(false); });

  // ── GIF va Stikerlar sahifalari (ilovaning o'z to'plamlari; `pack-panel.js`) ──
  const pkv = el.querySelector('.tge-pkv');
  const gear = el.querySelector('.tge-gear');
  let tab = 0;
  const pills = [...el.querySelectorAll('.tge-pt')];
  const pages = {};
  const pageOf = (i) => {
    const kind = i === 1 ? 'gif' : 'sticker';
    if (!pages[kind]) {
      pages[kind] = createPackPage(kind, { onPick: (p) => onPickMedia?.(p) });
      pages[kind].el.style.display = 'none';
      pkv.appendChild(pages[kind].el);
    }
    return pages[kind];
  };
  function setTab(i) {
    tab = i;
    pills.forEach((x, k) => x.classList.toggle('on', k === i));
    const emo = i === 0;
    scroll.style.display = emo ? '' : 'none';
    strip.style.display = emo ? '' : 'none';
    pkv.style.display = emo ? 'none' : 'block';
    bs.style.display = emo ? '' : 'none';
    gear.style.display = emo ? 'none' : 'flex';
    Object.values(pages).forEach((pg) => { pg.el.style.display = 'none'; });
    if (!emo) { const pg = pageOf(i); pg.el.style.display = 'block'; pg.show(); }
  }
  pills.forEach((x, k) => x.addEventListener('click', () => setTab(k)));
  gear.addEventListener('click', () => openMyPacks(tab === 1 ? 'gif' : 'sticker'));
  pkv.style.display = 'none';
  gear.style.display = 'none';
  pills[0]?.classList.add('on');

  return {
    el,
    /** Panel ochilganda (yaqinda ishlatilganlar yangilanadi). */
    show() {
      const r = recentLoad();
      if (!built || r.join('') !== recent.join('')) { recent = r; build(); }
      paintStrip();
    },
  };
}

/**
 * Yozish qatori + panel (`TgInputArea`). `button` — 🙂 tugmasi elementi,
 * `panelHost` — panel qo'yiladigan joy (qator ostida), `input` — textarea.
 * Qaytaradi: `{ close(), isOpen() }`.
 */
export function bindEmojiInput({ button, panelHost, input, height = 300, onPickMedia }) {
  let open = false;
  let panel = null;
  panelHost.classList.add('tge-host');
  function setOpen(v, instant = false) {
    if (v === open) return;
    open = v;
    button.classList.toggle('open', v);
    if (v && !panel) {
      panel = createEmojiPanel({
        onPick: (e) => insertAtCursor(input, e),
        onBackspace: () => backspaceAt(input),
        onPickMedia,
      });
      panel.el.style.height = `${height}px`;
      panelHost.appendChild(panel.el);
    }
    panelHost.style.transition = instant ? 'none' : '';
    panelHost.style.height = v ? `${height}px` : '0px';
    if (v) requestAnimationFrame(() => panel.show());
  }
  button.addEventListener('click', () => {
    if (open) {
      setOpen(false, true);
      input.focus();
    } else {
      const kbUp = document.activeElement === input;
      input.blur();
      setOpen(true, kbUp);
    }
  });
  // Maydonga bosildi — klaviatura chiqadi, panel darhol yopiladi.
  input.addEventListener('focus', () => { if (open) setOpen(false, true); });
  return { close: () => setOpen(false), isOpen: () => open };
}

// AVTOMATIK YASALGAN — `lib/widgets/tg_emoji_data.dart` dan (Telegram `EmojiData`).
export const TG_EMOJI_GROUPS = [{"id":"smileys","title":"Emoji va odamlar","emoji":["😀","😃","😄","😁","😆","🥹","😅","😂","🤣","🥲","☺️","😊","😇","🙂","🙃","😉","😌","😍","🥰","😘","😗","😙","😚","😋","😛","😝","😜","🤪","🤨","🧐","🤓","😎","🥸","🤩","🥳","🙂‍↕️","😏","😒","🙂‍↔️","😞","😔","😟","😕","🙁","☹️","😣","😖","😫","😩","🥺","😢","😭","😤","😠","😡","🤬","🤯","😳","🥵","🥶","😶‍🌫️","😱","😨","😰","😥","😓","🤗","🤔","🫣","🤭","🫢","🫡","🤫","🫠","🤥","😶","🫥","😐","🫤","😑","🫨","😬","🙄","😯","😦","😧","😮","😲","🥱","🫩","🫪","😴","🤤","😪","😮‍💨","😵","😵‍💫","🤐","🥴","🤢","🤮","🤧","😷","🤒","🤕","🤑","🤠","😈","👿","👹","👺","🤡","💩","👻","💀","☠","👽","👾","🤖","🎃","😺","😸","😹","😻","😼","😽","🙀","😿","😾","🫶","🤲","👐","🙌","👏","🤝","👍","👎","👊","✊","🤛","🤜","🫷","🫸","🤞","✌️","🫰","🤟","🤘","👌","🤌","🤏","🫳","🫴","👈","👉","👆","👇","☝️","✋","🤚","🖐","🖖","👋","🤙","🫲","🫱","💪","🦾","🖕","✍","🙏","🫵","🦶","🦵","🦿","💄","💋","👄","🫦","🦷","👅","👂","🦻","👃","🫆","👣","👁","👀","🫀","🫁","🧠","🗣","👤","👥","🫂","👶","👧","🧒","👦","👩","🧑","👨","👩‍🦱","🧑‍🦱","👨‍🦱","👩‍🦰","🧑‍🦰","👨‍🦰","👱‍♀","👱","👱‍♂","👩‍🦳","🧑‍🦳","👨‍🦳","👩‍🦲","🧑‍🦲","👨‍🦲","🧔‍♀","🧔","🧔‍♂","👵","🧓","👴","👲","👳‍♀","👳","👳‍♂","🧕","👮‍♀","👮","👮‍♂","👷‍♀","👷","👷‍♂","💂‍♀","💂","💂‍♂","🕵‍♀","🕵","🕵‍♂","👩‍⚕","🧑‍⚕","👨‍⚕","👩‍🌾","🧑‍🌾","👨‍🌾","👩‍🍳","🧑‍🍳","👨‍🍳","👩‍🎓","🧑‍🎓","👨‍🎓","👩‍🎤","🧑‍🎤","👨‍🎤","👩‍🏫","🧑‍🏫","👨‍🏫","👩‍🏭","🧑‍🏭","👨‍🏭","👩‍💻","🧑‍💻","👨‍💻","👩‍💼","🧑‍💼","👨‍💼","👩‍🔧","🧑‍🔧","👨‍🔧","👩‍🔬","🧑‍🔬","👨‍🔬","👩‍🎨","🧑‍🎨","👨‍🎨","👩‍🚒","🧑‍🚒","👨‍🚒","👩‍✈️","🧑‍✈️","👨‍✈️","👩‍🚀","🧑‍🚀","👨‍🚀","👩‍⚖","🧑‍⚖","👨‍⚖","👰‍♀","👰","👰‍♂","🤵‍♀","🤵","🤵‍♂","👸","🫅","🤴","🥷","🦸‍♀","🦸","🦸‍♂","🦹‍♀","🦹","🦹‍♂","🤶","🧑‍🎄","🎅","🧙‍♀","🧙","🧙‍♂","🧝‍♀","🧝","🧝‍♂","🧌","🧛‍♀","🧛","🧛‍♂","🧟‍♀","🧟","🧟‍♂","🧞‍♀","🧞","🧞‍♂","🧜‍♀","🧜","🧜‍♂","🧚‍♀","🧚","🧚‍♂","👼","🤰","🫄","🫃","🤱","👩‍🍼","🧑‍🍼","👨‍🍼","🙇‍♀","🙇","🙇‍♂","💁‍♀","💁","💁‍♂","🙅‍♀","🙅","🙅‍♂","🙆‍♀","🙆","🙆‍♂","🙋‍♀","🙋","🙋‍♂","🧏‍♀","🧏","🧏‍♂","🤦‍♀","🤦","🤦‍♂","🤷‍♀","🤷","🤷‍♂","🙎‍♀","🙎","🙎‍♂","🙍‍♀","🙍","🙍‍♂","💇‍♀","💇","💇‍♂","💆‍♀","💆","💆‍♂","🧖‍♀","🧖","🧖‍♂","💅","🤳","💃","🕺","🧑‍🩰","👯‍♀","👯","👯‍♂","🕴","👩‍🦽","🧑‍🦽","👨‍🦽","👩‍🦽‍➡️","🧑‍🦽‍➡️","👨‍🦽‍➡️","👩‍🦼","🧑‍🦼","👨‍🦼","👩‍🦼‍➡️","🧑‍🦼‍➡️","👨‍🦼‍➡️","🚶‍♀","🚶","🚶‍♂","🚶‍♀‍➡️","🚶‍➡️","🚶‍♂‍➡️","👩‍🦯","🧑‍🦯","👨‍🦯","👩‍🦯‍➡️","🧑‍🦯‍➡️","👨‍🦯‍➡️","🧎‍♀","🧎","🧎‍♂","🏃‍♀","🏃","🏃‍♂","🏃‍♀‍➡️","🏃‍➡️","🏃‍♂‍➡️","🧎‍♀‍➡️","🧎‍➡️","🧎‍♂‍➡️","🧍‍♀","🧍","🧍‍♂","👫","👭","👬","👩‍❤️‍👨","👩‍❤️‍👩","💑","👨‍❤️‍👨","👩‍❤️‍💋‍👨","👩‍❤️‍💋‍👩","💏","👨‍❤️‍💋‍👨","🪢","🧶","🧵","🪡","🧥","🥼","🦺","👚","👕","👖","🩲","🩳","👔","👗","👙","🩱","👘","🥻","🩴","🥿","👠","👡","👢","👞","👟","🥾","🧦","🧤","🧣","🎩","🧢","👒","🎓","⛑","🪖","👑","💍","👝","👛","👜","💼","🎒","🧳","👓","🕶","🥽","🌂"]},{"id":"animals","title":"Hayvonlar va tabiat","emoji":["🐶","🐱","🐭","🐹","🐰","🦊","🐻","🐼","🐻‍❄️","🐨","🐯","🦁","🐮","🐷","🐽","🐸","🐵","🙈","🙉","🙊","🐒","🐔","🐧","🐦","🐤","🐣","🐥","🪿","🦆","🐦‍⬛️","🦅","🦉","🦇","🐺","🐗","🐴","🦄","🫎","🐝","🪱","🐛","🦋","🐌","🐞","🐜","🪰","🪲","🪳","🦟","🦗","🕷","🕸","🦂","🐢","🐍","🦎","🦖","🦕","🐙","🦑","🪼","🦐","🦞","🦀","🐡","🐠","🐟","🐬","🐳","🐋","🫍","🦈","🦭","🐊","🐅","🐆","🦓","🦍","🦧","🫈","🦣","🐘","🦛","🦏","🐪","🐫","🦒","🦘","🦬","🐃","🐂","🐄","🫏","🐎","🐖","🐏","🐑","🦙","🐐","🦌","🐕","🐩","🦮","🐕‍🦺","🐈","🐈‍⬛️","🪶","🪽","🐓","🦃","🦤","🦚","🦜","🦢","🦩","🕊","🐇","🦝","🦨","🦡","🦫","🦦","🦥","🐁","🐀","🐿","🦔","🐾","🐉","🐲","🐦‍🔥","🌵","🎄","🌲","🌳","🌴","🪾","🪵","🌱","🌿","☘","🍀","🎍","🪴","🎋","🍃","🍂","🍁","🪺","🪹","🍄","🍄‍🟫","🐚","🪸","🪨","🛘","🌾","💐","🌷","🌹","🥀","🪻","🪷","🌺","🌸","🌼","🌻","🌞","🌝","🌛","🌜","🌚","🌕","🌖","🌗","🌘","🌑","🌒","🌓","🌔","🌙","🌎","🌍","🌏","🪐","💫","⭐️","🌟","✨","⚡️","☄","💥","🔥","🫯","🌪","🌈","☀️","🌤","⛅️","🌥","☁️","🌦","🌧","⛈","🌩","🌨","❄️","☃️","⛄️","🌬","💨","💧","💦","🫧","☔️","☂","🌊","🌫️"]},{"id":"food","title":"Ovqat va ichimliklar","emoji":["🍏","🍎","🍐","🍊","🍋","🍋‍🟩","🍌","🍉","🍇","🍓","🫐","🍈","🍒","🍑","🥭","🍍","🥥","🥝","🍅","🍆","🥑","🫛","🥦","🥬","🥒","🌶","🫑","🌽","🥕","🫒","🧄","🧅","🥔","🫜","🍠","🫚","🥐","🥯","🍞","🥖","🥨","🧀","🥚","🍳","🧈","🥞","🧇","🥓","🥩","🍗","🍖","🦴","🌭","🍔","🍟","🍕","🫓","🥪","🥙","🧆","🌮","🌯","🫔","🥗","🥘","🫕","🥫","🫙","🍝","🍜","🍲","🍛","🍣","🍱","🥟","🦪","🍤","🍙","🍚","🍘","🍥","🥠","🥮","🍢","🍡","🍧","🍨","🍦","🥧","🧁","🍰","🎂","🍮","🍭","🍬","🍫","🍿","🍩","🍪","🌰","🥜","🫘","🍯","🥛","🫗","🍼","🫖","☕️","🍵","🧃","🥤","🧋","🍶","🍺","🍻","🥂","🍷","🥃","🍸","🍹","🧉","🍾","🧊","🥄","🍴","🍽","🥣","🥡","🥢","🧂"]},{"id":"activity","title":"Faoliyat","emoji":["⚽️","🏀","🏈","⚾️","🥎","🎾","🏐","🏉","🥏","🎱","🪀","🏓","🏸","🏒","🏑","🥍","🏏","🪃","🥅","⛳️","🪁","🛝","🏹","🎣","🤿","🥊","🥋","🎽","🛹","🛼","🛷","⛸","🥌","🎿","⛷","🏂","🪂","🏋️‍♀","🏋️","🏋️‍♂","🤼‍♀","🤼","🤼‍♂","🤸‍♀","🤸","🤸‍♂","⛹‍♀","⛹","⛹‍♂","🤺","🤾‍♀","🤾","🤾‍♂","🏌️‍♀","🏌️","🏌️‍♂","🏇","🧘‍♀","🧘","🧘‍♂","🏄‍♀","🏄","🏄‍♂","🏊‍♀","🏊","🏊‍♂","🤽‍♀","🤽","🤽‍♂","🚣‍♀","🚣","🚣‍♂","🧗‍♀","🧗","🧗‍♂","🚵‍♀","🚵","🚵‍♂","🚴‍♀","🚴","🚴‍♂","🏆","🥇","🥈","🥉","🏅","🎖","🏵","🎗","🎫","🎟","🎪","🤹‍♀","🤹","🤹‍♂","🎭","🩰","🎨","🫟","🎬","🎤","🎧","🎼","🎹","🪇","🥁","🪘","🎷","🎺","🪊","🪗","🎸","🪕","🪉","🎻","🪈","🎲","♟","🎯","🎳","🎮","🎰","🧩"]},{"id":"travel","title":"Sayohat va joylar","emoji":["🚗","🚕","🚙","🚌","🚎","🏎","🚓","🚑","🚒","🚐","🛻","🚚","🚛","🚜","🦯","🦽","🦼","🩼","🛴","🚲","🛵","🏍","🛺","🛞","🚨","🚔","🚍","🚘","🚖","🚡","🚠","🚟","🚃","🚋","🚞","🚝","🚄","🚅","🚈","🚂","🚆","🚇","🚊","🚉","✈️","🛫","🛬","🛩","💺","🛰","🚀","🛸","🚁","🛶","⛵️","🚤","🛥","🛳","⛴","🚢","🛟","⚓️","🪝","⛽️","🚧","🚦","🚥","🚏","🗺","🗿","🗽","🗼","🏰","🏯","🏟","🎡","🎢","🎠","⛲️","⛱","🏖","🏝","🏜","🌋","⛰","🏔","🗻","🏕","⛺️","🛖","🏠","🏡","🏘","🏚","🏗","🏭","🏢","🏬","🏣","🏤","🏥","🏦","🏨","🏪","🏫","🏩","💒","🏛","⛪️","🕌","🕍","🛕","🕋","⛩","🛤","🛣","🗾","🎑","🏞","🌅","🌄","🌠","🎇","🎆","🌇","🌆","🏙","🌃","🌌","🌉","🌁"]},{"id":"objects","title":"Buyumlar","emoji":["⌚️","📱","📲","💻","⌨","🖥","🖨","🖱","🖲","🕹","🗜","💽","💾","💿","📀","📼","📷","📸","📹","🎥","📽","🎞","📞","☎️","📟","📠","📺","📻","🎙","🎚","🎛","🧭","⏱","⏲","⏰","🕰","⌛️","⏳","📡","🔋","🪫","🔌","💡","🔦","🕯","🪔","🧯","🛢","💸","💵","💴","💶","💷","🪙","💰","💳","🪪","💎","🪎","⚖","🪜","🧰","🪛","🔧","🔨","⚒","🛠","⛏","🪏","🪚","🔩","⚙","🪤","🧱","⛓","⛓‍💥","🧲","🔫","💣","🧨","🪓","🔪","🗡","⚔","🛡","🚬","⚰","🪦","⚱","🏺","🔮","📿","🧿","🪬","💈","⚗","🔭","🔬","🕳","🩻","🩹","🩺","💊","💉","🩸","🧬","🦠","🧫","🧪","🌡","🧹","🪠","🧺","🧻","🚽","🚰","🚿","🛁","🛀","🧼","🪥","🪒","🪮","🧽","🪣","🧴","🛎","🔑","🗝","🚪","🪑","🛋","🛏","🛌","🧸","🪆","🖼","🪞","🪟","🛍","🛒","🎁","🎈","🎏","🎀","🪄","🪅","🎊","🎉","🎎","🪭","🏮","🎐","🪩","🧧","✉️","📩","📨","📧","💌","📥","📤","📦","🏷","🪧","📪","📫","📬","📭","📮","📯","📜","📃","📄","📑","🧾","📊","📈","📉","🗒","🗓","📆","📅","🗑","📇","🗃","🗳","🗄","📋","📁","📂","🗂","🗞","📰","📓","📔","📒","📕","📗","📘","📙","📚","📖","🔖","🧷","🔗","📎","🖇","📐","📏","🧮","📌","📍","✂️","🖊","🖋","✒️","🖌","🖍","📝","✏️","🔍","🔎","🔏","🔐","🔒","🔓"]},{"id":"symbols","title":"Belgilar","emoji":["🩷","❤️","🧡","💛","💚","🩵","💙","💜","🖤","🩶","🤍","🤎","💔","❤️‍🔥","❤️‍🩹","❣","💕","💞","💓","💗","💖","💘","💝","💟","☮","✝","☪","🕉","☸","🪯","✡","🔯","🕎","☯","☦","🛐","⛎","♈️","♉️","♊️","♋️","♌️","♍️","♎️","♏️","♐️","♑️","♒️","♓️","🆔","⚛","🉑","☢","☣","📴","📳","🈶","🈚️","🈸","🈺","🈷","✴️","🆚","💮","🉐","㊙️","㊗️","🈴","🈵","🈹","🈲","🅰","🅱","🆎","🆑","🅾","🆘","❌","⭕️","🛑","⛔️","📛","🚫","💯","💢","♨️","🚷","🚯","🚳","🚱","🔞","📵","🚭","❗️","❕","❓","❔","‼️","⁉️","🔅","🔆","〽️","⚠️","🚸","🔱","⚜","🔰","♻️","✅","🈯️","💹","❇️","✳️","❎","🌐","💠","Ⓜ️","🌀","💤","🏧","🚾","♿️","🅿️","🛗","🈳","🈂","🛂","🛃","🛄","🛅","🛜","🚹","🚺","🚼","👨‍👩‍👦","👨‍👩‍👧‍👦","👩‍👦","👩‍👧‍👦","⚧","🚻","🚮","🎦","📶","🈁","🔣","ℹ️","🔤","🔡","🔠","🆖","🆗","🆙","🆒","🆕","🆓","0⃣","1⃣","2⃣","3⃣","4⃣","5⃣","6⃣","7⃣","8⃣","9⃣","🔟","🔢","#⃣","*⃣","⏏️","▶️","⏸","⏯","⏹","⏺","⏭","⏮","⏩","⏪","⏫","⏬","◀️","🔼","🔽","➡️","⬅️","⬆️","⬇️","↗️","↘️","↙️","↖️","↕️","↔️","↪️","↩️","⤴️","⤵️","🔀","🔁","🔂","🔄","🔃","🎵","🎶","➕","➖","➗","✖️","🟰","♾","💲","💱","™️","©","®","👁‍🗨","🔚","🔙","🔛","🔝","🔜","〰","➰","➿","✔️","☑️","🔘","🔴","🟠","🟡","🟢","🔵","🟣","⚫️","⚪️","🟤","🔺","🔻","🔸","🔹","🔶","🔷","🔳","🔲","▪️","▫️","◾️","◽️","◼️","◻️","🟥","🟧","🟨","🟩","🟦","🟪","⬛️","⬜️","🟫","🔈","🔇","🔉","🔊","🔔","🔕","📣","📢","💬","💭","🗯","♠️","♣️","♥️","♦️","🃏","🎴","🀄️","🕐","🕑","🕒","🕓","🕔","🕕","🕖","🕗","🕘","🕙","🕚","🕛","🕜","🕝","🕞","🕟","🕠","🕡","🕢","🕣","🕤","🕥","🕦","🕧"]},{"id":"flags","title":"Bayroqlar","emoji":["🏳️","🏴","🏴‍☠","🏁","🚩","🏳️‍🌈","🏳️‍⚧","🇺🇳","🇦🇫","🇦🇽","🇦🇱","🇩🇿","🇦🇸","🇦🇩","🇦🇴","🇦🇮","🇦🇶","🇦🇬","🇦🇷","🇦🇲","🇦🇼","🇦🇺","🇦🇹","🇦🇿","🇧🇸","🇧🇭","🇧🇩","🇧🇧","🇧🇾","🇧🇪","🇧🇿","🇧🇯","🇧🇲","🇧🇹","🇧🇴","🇧🇦","🇧🇼","🇧🇷","🇻🇬","🇧🇳","🇧🇬","🇧🇫","🇧🇮","🇰🇭","🇨🇲","🇨🇦","🇮🇨","🇨🇻","🇧🇶","🇰🇾","🇨🇫","🇹🇩","🇮🇴","🇨🇱","🇨🇳","🇨🇽","🇨🇨","🇨🇴","🇰🇲","🇨🇬","🇨🇩","🇨🇰","🇨🇷","🇨🇮","🇭🇷","🇨🇺","🇨🇼","🇨🇾","🇨🇿","🇩🇰","🇩🇯","🇩🇲","🇩🇴","🇪🇨","🇪🇬","🇸🇻","🇬🇶","🇪🇷","🇪🇪","🇸🇿","🇪🇹","🇪🇺","🇫🇰","🇫🇴","🇫🇯","🇫🇮","🇫🇷","🇬🇫","🇵🇫","🇹🇫","🇬🇦","🇬🇲","🇬🇪","🇩🇪","🇬🇭","🇬🇮","🇬🇷","🇬🇱","🇬🇩","🇬🇵","🇬🇺","🇬🇹","🇬🇬","🇬🇳","🇬🇼","🇬🇾","🇭🇹","🇭🇳","🇭🇰","🇭🇺","🇮🇸","🇮🇳","🇮🇩","🇮🇷","🇮🇶","🇮🇪","🇮🇲","🇮🇱","🇮🇹","🇯🇲","🇯🇵","🎌","🇯🇪","🇯🇴","🇰🇿","🇰🇪","🇰🇮","🇽🇰","🇰🇼","🇰🇬","🇱🇦","🇱🇻","🇱🇧","🇱🇸","🇱🇷","🇱🇾","🇱🇮","🇱🇹","🇱🇺","🇲🇴","🇲🇬","🇲🇼","🇲🇾","🇲🇻","🇲🇱","🇲🇹","🇲🇭","🇲🇶","🇲🇷","🇲🇺","🇾🇹","🇲🇽","🇫🇲","🇲🇩","🇲🇨","🇲🇳","🇲🇪","🇲🇸","🇲🇦","🇲🇿","🇲🇲","🇳🇦","🇳🇷","🇳🇵","🇳🇱","🇳🇨","🇳🇿","🇳🇮","🇳🇪","🇳🇬","🇳🇺","🇳🇫","🇰🇵","🇲🇰","🇲🇵","🇳🇴","🇴🇲","🇵🇰","🇵🇼","🇵🇸","🇵🇦","🇵🇬","🇵🇾","🇵🇪","🇵🇭","🇵🇳","🇵🇱","🇵🇹","🇵🇷","🇶🇦","🇷🇪","🇷🇴","🇷🇺","🇷🇼","🇼🇸","🇸🇲","🇸🇹","🇨🇶","🇸🇦","🇸🇳","🇷🇸","🇸🇨","🇸🇱","🇸🇬","🇸🇽","🇸🇰","🇸🇮","🇬🇸","🇸🇧","🇸🇴","🇿🇦","🇰🇷","🇸🇸","🇪🇸","🇱🇰","🇧🇱","🇸🇭","🇰🇳","🇱🇨","🇵🇲","🇻🇨","🇸🇩","🇸🇷","🇸🇪","🇨🇭","🇸🇾","🇹🇼","🇹🇯","🇹🇿","🇹🇭","🇹🇱","🇹🇬","🇹🇰","🇹🇴","🇹🇹","🇹🇳","🇹🇷","🇹🇲","🇹🇨","🇹🇻","🇺🇬","🇺🇦","🇦🇪","🇬🇧","🏴󠁧󠁢󠁥󠁮󠁧󠁿","🏴󠁧󠁢󠁳󠁣󠁴󠁿","🏴󠁧󠁢󠁷󠁬󠁳󠁿","🇺🇸","🇺🇾","🇻🇮","🇺🇿","🇻🇺","🇻🇦","🇻🇪","🇻🇳","🇼🇫","🇪🇭","🇾🇪","🇿🇲","🇿🇼"]}];
export const TG_EMOJI_COLORED = new Set(["🫶","🤲","👐","🙌","👏","👍","👎","👊","✊","🤛","🤜","🫷","🫸","🤞","✌","🫰","🤟","🤘","👌","🤌","🤏","🫳","🫴","👈","👉","👆","👇","☝","✋","🤚","🖐","🖖","👋","🤙","🫲","🫱","💪","🖕","✍","🙏","🫵","🦶","🦵","👂","🦻","👃","👶","👧","🧒","👦","👩","🧑","👨","👩‍🦱","🧑‍🦱","👨‍🦱","👩‍🦰","🧑‍🦰","👨‍🦰","👱‍♀","👱","👱‍♂","👩‍🦳","🧑‍🦳","👨‍🦳","👩‍🦲","🧑‍🦲","👨‍🦲","🧔‍♀","🧔","🧔‍♂","👵","🧓","👴","👲","👳‍♀","👳","👳‍♂","🧕","👮‍♀","👮","👮‍♂","👷‍♀","👷","👷‍♂","💂‍♀","💂","💂‍♂","🕵‍♀","🕵","🕵‍♂","👩‍⚕","🧑‍⚕","👨‍⚕","👩‍🌾","🧑‍🌾","👨‍🌾","👩‍🍳","🧑‍🍳","👨‍🍳","👩‍🎓","🧑‍🎓","👨‍🎓","👩‍🎤","🧑‍🎤","👨‍🎤","👩‍🏫","🧑‍🏫","👨‍🏫","👩‍🏭","🧑‍🏭","👨‍🏭","👩‍💻","🧑‍💻","👨‍💻","👩‍💼","🧑‍💼","👨‍💼","👩‍🔧","🧑‍🔧","👨‍🔧","👩‍🔬","🧑‍🔬","👨‍🔬","👩‍🎨","🧑‍🎨","👨‍🎨","👩‍🚒","🧑‍🚒","👨‍🚒","👩‍✈","🧑‍✈","👨‍✈","👩‍🚀","🧑‍🚀","👨‍🚀","👩‍⚖","🧑‍⚖","👨‍⚖","👰‍♀","👰","👰‍♂","🤵‍♀","🤵","🤵‍♂","👸","🤴","🥷","🦸‍♀","🦸","🦸‍♂","🦹‍♀","🦹","🦹‍♂","🤶","🧑‍🎄","🎅","🧙‍♀","🧙","🧙‍♂","🧝‍♀","🧝","🧝‍♂","🧛‍♀","🧛","🧛‍♂","🧜‍♀","🧜","🧜‍♂","🧚‍♀","🧚","🧚‍♂","👼","🤰","🫄","🫃","🤱","👩‍🍼","🧑‍🍼","👨‍🍼","🙇‍♀","🙇","🙇‍♂","💁‍♀","💁","💁‍♂","🙅‍♀","🙅","🙅‍♂","🙆‍♀","🙆","🙆‍♂","🙋‍♀","🙋","🙋‍♂","🧏‍♀","🧏","🧏‍♂","🤦‍♀","🤦","🤦‍♂","🤷‍♀","🤷","🤷‍♂","🙎‍♀","🙎","🙎‍♂","🙍‍♀","🙍","🙍‍♂","💇‍♀","💇","💇‍♂","💆‍♀","💆","💆‍♂","🧖‍♀","🧖","🧖‍♂","💅","🤳","💃","🕺","🧑‍🩰","🕴","👩‍🦽","🧑‍🦽","👨‍🦽","👩‍🦽‍➡","🧑‍🦽‍➡","👨‍🦽‍➡","👩‍🦼","🧑‍🦼","👨‍🦼","👩‍🦼‍➡","🧑‍🦼‍➡","👨‍🦼‍➡","🚶‍♀","🚶","🚶‍♂","🚶‍♀‍➡","🚶‍➡","🚶‍♂‍➡","👩‍🦯","🧑‍🦯","👨‍🦯","👩‍🦯‍➡","🧑‍🦯‍➡","👨‍🦯‍➡","🧎‍♀","🧎","🧎‍♂","🏃‍♀","🏃","🏃‍♂","🏃‍♀‍➡","🏃‍➡","🏃‍♂‍➡","🧎‍♀‍➡","🧎‍➡","🧎‍♂‍➡","🧍‍♀","🧍","🧍‍♂","🏋‍♀","🏋","🏋‍♂","🤸‍♀","🤸","🤸‍♂","⛹‍♀","⛹","⛹‍♂","🤾‍♀","🤾","🤾‍♂","🏌‍♀","🏌","🏌‍♂","🏇","🧘‍♀","🧘","🧘‍♂","🏄‍♀","🏄","🏄‍♂","🏊‍♀","🏊","🏊‍♂","🤽‍♀","🤽","🤽‍♂","🚣‍♀","🚣","🚣‍♂","🧗‍♀","🧗","🧗‍♂","🚵‍♀","🚵","🚵‍♂","🚴‍♀","🚴","🚴‍♂","🤹‍♀","🤹","🤹‍♂","🛀"]);
