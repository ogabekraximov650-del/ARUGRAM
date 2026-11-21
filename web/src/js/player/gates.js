// Pleyer ekranining "eshiklari" va oynalari:
//   * `_SubRequiredScreen`     -> subRequired(el)
//   * `_ChannelConsentScreen`  -> channelConsent(el)
//   * `_RatingSheet`           -> ratingSheet(current) -> Promise<stars|null>
//   * `_showQualityDialog`     -> qualityDialog(items) -> Promise<q|null>
// (`lib/screens/video_player_screen.dart` oxiridagi vidjetlar.)

import { icon, spinner, bindTap, C } from '../ui.js';
import { back as routerBack } from '../router.js';
import { esc } from '../format.js';
import { billing } from '../services/billing.js';
import { channelGate } from '../services/channel-gate.js';
import { openBilling } from '../screens/billing.js';

function gateBar() {
  return `<div class="pl-gate-bar"><button class="icon-btn pl-gate-back" aria-label="Orqaga">${icon('arrow_back', { size: 24 })}</button></div>`;
}

/** Obuna kerak (`_SubRequiredScreen`). */
export function subRequired(el) {
  el.innerHTML = `${gateBar()}
    <div class="pl-gate-scroll"><div class="pl-gate">
      <div class="pl-sub-badge">${icon('workspace_premium', { size: 44, color: C.accentTint })}</div>
      <div class="pl-gate-t" style="margin-top:22px">Obuna kerak</div>
      <div class="pl-gate-s">Anime ko'rish uchun obuna bo'lishi kerak. Tariflar 1 kundan 30 kungacha — profil sahifasidan yoki quyidagi tugmadan tanlang.</div>
      <button class="pl-gate-btn" style="margin-top:24px">Obuna olish</button>
      <button class="pl-gate-link">Obunani yangilash</button>
    </div></div>`;
  el.querySelector('.pl-gate-back').addEventListener('click', () => routerBack());
  el.querySelector('.pl-gate-btn').addEventListener('click', () => openBilling({ startPage: 1 }));
  el.querySelector('.pl-gate-link').addEventListener('click', () => billing.load({ force: true }));
}

/** Bepul ko'rish: kanallarga ruxsat (`_ChannelConsentScreen`). */
export function channelConsent(el) {
  const paint = () => {
    const list = channelGate.channels;
    const chans = list || [];
    el.innerHTML = `${gateBar()}
      <div class="pl-gate-scroll"><div class="pl-gate">
        ${icon('campaign', { size: 64, color: C.accent })}
        <div class="pl-gate-t" style="margin-top:18px">Bepul ko'rish</div>
        <div class="pl-gate-s">Bu bo'limni bepul ko'rish uchun quyidagi kanallarga obuna bo'lishingiz kerak. Ruxsat bersangiz, ilova Telegram hisobingiz bilan ularga o'zi qo'shiladi (yopiq kanalga so'rov yuboradi). Kanaldan istalgan vaqtda Telegram'da chiqib ketishingiz mumkin.</div>
        <div style="height:16px"></div>
        ${list == null ? `<div style="padding:12px">${spinner(36, 4)}</div>` : ''}
        ${chans.map((c) => `<div class="pl-chan">
          ${icon(c.kind === 'public' ? 'campaign' : 'lock', { fill: false, size: 18, color: 'rgba(255,255,255,0.62)' })}
          <div class="t">${esc(c.title || c.url)}</div></div>`).join('')}
        <button class="pl-gate-btn" style="margin-top:20px">Ruxsat berish va ko'rish</button>
        <button class="pl-gate-link">Kanalsiz — obuna olish</button>
      </div></div>`;
    el.querySelector('.pl-gate-back').addEventListener('click', () => routerBack());
    el.querySelector('.pl-gate-btn').addEventListener('click', () => channelGate.grant());
    el.querySelector('.pl-gate-link').addEventListener('click', () => openBilling({ startPage: 1 }));
  };
  paint();
  const off = channelGate.listen(paint);
  channelGate.refresh();
  return off;
}

// ── Oynalar ─────────────────────────────────────────────────────────

/** Ekran ustidagi qatlam (orqa fon bosilsa yopiladi). */
function overlay(cls, build, hostEl) {
  return new Promise((resolve) => {
    const app = hostEl || document.getElementById('app');
    const wrap = document.createElement('div');
    wrap.className = `pl-ov ${cls}`;
    const close = (v) => {
      wrap.classList.remove('in');
      setTimeout(() => wrap.remove(), 220);
      resolve(v ?? null);
    };
    wrap.addEventListener('click', (e) => { if (e.target === wrap) close(null); });
    build(wrap, close);
    app.appendChild(wrap);
    requestAnimationFrame(() => wrap.classList.add('in'));
  });
}

/** Baholash (Android `AlertDialog`: "Baholang (1–10)", 1..10 doiralar; raqam bosilsa darhol baholanadi). */
export function ratingSheet(current) {
  return overlay('pl-rate-ov', (wrap, close) => {
    wrap.innerHTML = `<div class="pl-rate">
      <div class="pl-rate-t">Baholang (1\u201310)</div>
      <div class="pl-rate-nums">${Array.from({ length: 10 }, (_, i) => `<div class="n${i + 1 === current ? ' on' : ''}" data-i="${i + 1}">${i + 1}</div>`).join('')}</div>
      <div class="pl-rate-btns"><div class="pl-rate-cancel">Bekor qilish</div></div></div>`;
    wrap.querySelectorAll('.n').forEach((n) => n.addEventListener('click', () => close(+n.dataset.i)));
    wrap.querySelector('.pl-rate-cancel').addEventListener('click', () => close(null));
  });
}

/** Sifat tanlash (`_showQualityDialog`). `items`: [{q, title, size, sel}]. */
export function qualityDialog(items, hostEl) {
  return overlay('pl-qd-ov', (wrap, close) => {
    wrap.innerHTML = `<div class="pl-qd">
      <div class="pl-qd-t">Sifatni tanlang</div>
      ${items.map((it) => `<div class="pl-qd-item${it.sel ? ' sel' : ''}" data-q="${esc(it.q)}">
        <div class="col"><div class="q">${esc(it.title.toUpperCase())}</div>${it.size ? `<div class="sz">${esc(it.size)}</div>` : ''}</div>
        ${it.sel ? icon('check', { size: 24, color: '#000' }) : ''}
      </div>`).join('')}
    </div>`;
    wrap.querySelectorAll('.pl-qd-item').forEach((n) => bindTap(n, () => close(n.dataset.q), { scale: false }));
  }, hostEl);
}
