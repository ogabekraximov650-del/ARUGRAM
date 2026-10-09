// Pastki navigatsiya — `lib/screens/root_screen.dart` -> `_BottomNav`.
//
// Sahifa DARHOL almashadi, suzuvchi tugma esa sekin suzib boradi
// (280 ms / oraliq, ekran eniga qarab 0.90..1.20 ko'paytma, 320..1400 ms,
// `easeInOutSine`, bitta kadrga ko'pi bilan 32 ms qo'shiladi).

export const NAV_ITEMS = [
  { icon: 'home', label: 'Bosh sahifa' },
  { icon: 'search', label: 'Qidiruv' },
  { icon: 'grid_view', label: 'Katalog' },
  { icon: 'folder', label: 'Kutubxona' },
  { icon: 'person', label: 'Profil' },
];

const MS_PER_STEP = 280;
const MAX_FRAME_MS = 32;

const easeInOutSine = (t) => -(Math.cos(Math.PI * t) - 1) / 2;

function speedFactor(width) {
  if (width <= 0) return 1;
  const f = 600 / width;
  return Math.min(1.2, Math.max(0.9, f));
}

/** `Color.lerp(white 40%, white, t)` */
function itemColor(t) {
  const a = 0.4 + 0.6 * t;
  return `rgba(255,255,255,${a.toFixed(3)})`;
}

export function createBottomNav(root, onTap) {
  const wrap = document.createElement('div');
  wrap.className = 'bottom-nav-wrap';
  wrap.innerHTML = `
    <div class="bottom-nav">
      <div class="bottom-nav-inner">
        <div class="nav-pill"></div>
        ${NAV_ITEMS.map((it, i) => `
          <div class="nav-item" data-i="${i}">
            <span class="nav-icon"><span class="ic fill">${it.icon}</span><span class="nav-dot"></span></span>
            <div class="nav-label-clip"><div class="nav-label">${it.label}</div></div>
          </div>`).join('')}
      </div>
    </div>`;
  root.appendChild(wrap);

  const inner = wrap.querySelector('.bottom-nav-inner');
  const pill = wrap.querySelector('.nav-pill');
  const items = [...wrap.querySelectorAll('.nav-item')];
  const parts = items.map((el) => ({
    icon: el.querySelector('.nav-icon'),
    glyph: el.querySelector('.ic'),
    clip: el.querySelector('.nav-label-clip'),
    label: el.querySelector('.nav-label'),
  }));

  let pos = 0;
  let from = 0;
  let to = 0;
  let t = 1;
  let ms = 300;
  let last = null;
  let raf = 0;
  let current = 0;
  let labelH = 0;

  function paint() {
    const w = inner.clientWidth;
    const itemW = w / NAV_ITEMS.length;
    pill.style.left = '0px';
    pill.style.width = `${itemW - 8}px`;
    pill.style.transform = `translateX(${pos * itemW + 4}px)`;
    if (!labelH) labelH = parts[0].label.scrollHeight || 14;
    parts.forEach((p, i) => {
      const k = 1 - Math.min(1, Math.max(0, Math.abs(pos - i)));
      const c = itemColor(k);
      p.icon.style.transform = `scale(${1 + 0.22 * k})`;
      p.glyph.style.color = c;
      p.clip.style.height = `${labelH * k}px`;
      p.label.style.opacity = `${k}`;
      p.label.style.color = c;
    });
  }

  function tick(now) {
    if (last === null) {
      last = now;
      raf = requestAnimationFrame(tick);
      return;
    }
    let dt = now - last;
    last = now;
    if (dt > MAX_FRAME_MS) dt = MAX_FRAME_MS;
    t += dt / ms;
    if (t >= 1) t = 1;
    pos = from + (to - from) * easeInOutSine(t);
    paint();
    if (t < 1) raf = requestAnimationFrame(tick);
    else { raf = 0; last = null; }
  }

  function go(i) {
    if (i === current) return;
    current = i;
    onTap(i);
    from = pos;
    to = i;
    t = 0;
    last = null;
    const steps = Math.abs(to - from);
    const d = steps * MS_PER_STEP * speedFactor(window.innerWidth);
    ms = Math.min(1400, Math.max(320, d));
    if (!raf) raf = requestAnimationFrame(tick);
  }

  items.forEach((el, i) => el.addEventListener('click', () => go(i)));
  window.addEventListener('resize', paint);
  // Shriftlar yuklangach yozuv balandligi aniq o'lchanadi.
  document.fonts?.ready.then(() => { labelH = 0; paint(); });
  paint();

  return { go, el: wrap };
}
