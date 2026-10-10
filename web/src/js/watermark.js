// Suv belgisi: foydalanuvchining @nomi va ID'si ekran bo'ylab xira, qiyshaytirilgan holda takrorlanadi.
// Skrinshot/ekran yozuvi sizib chiqsa, kimdan chiqqani ma'lum bo'ladi. Bosishlarga xalaqit bermaydi.

import { currentUser } from './api.js';

function labelOf() {
  const u = currentUser();
  if (!u?.id) return '';
  const nm = u.username ? `@${u.username}` : `${u.first_name ?? ''}`.trim();
  return `${nm ? `${nm} · ` : ''}ID ${u.id}`;
}

function tile(label) {
  const t = label.replace(/&/g, '&amp;').replace(/</g, '&lt;');
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="260" height="170"><text x="130" y="85" text-anchor="middle" transform="rotate(-24 130 85)" font-family="sans-serif" font-size="14" font-weight="700" fill="#fff" fill-opacity="0.11" stroke="#000" stroke-opacity="0.07" stroke-width="0.6">${t}</text></svg>`;
  return `url("data:image/svg+xml;utf8,${encodeURIComponent(svg)}")`;
}

/** `host` ichiga suv belgisi qatlamini qo'yadi (`fixed` — butun ekran). Qaytaradi: element yoki null. */
export function installWatermark(host, { fixed = false, cls = '' } = {}) {
  const label = labelOf();
  if (!host || !label) return null;
  const el = document.createElement('div');
  el.className = `aru-wm ${cls}`.trim();
  el.setAttribute('aria-hidden', 'true');
  el.style.cssText = `position:${fixed ? 'fixed' : 'absolute'};inset:0;pointer-events:none;z-index:${fixed ? 9998 : 6};background-image:${tile(label)};background-size:260px 170px;`;
  host.appendChild(el);
  // Surib turiladi: bir joyga qarab kesib tashlash qiyin bo'lsin.
  const move = () => { el.style.backgroundPosition = `${Math.floor(Math.random() * 260)}px ${Math.floor(Math.random() * 170)}px`; };
  move();
  setInterval(() => { if (el.isConnected) move(); }, 17000);
  return el;
}
