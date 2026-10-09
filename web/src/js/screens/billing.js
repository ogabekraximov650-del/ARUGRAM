// Obuna, to'ldirish va tarix — `lib/screens/billing_screen.dart`.
//
// Tepada balans va obuna holati, ostida uchta tugma (To'ldirish, Obuna,
// Tarix) va qo'lda surib o'tkaziladigan uchta oyna (PageView). Bu
// faylda birorta narx yo'q — hammasi serverdan (`services/billing.js`).
//
//   openBilling()            — profildagidek: balansda pul bo'lsa Obuna,
//                              bo'lmasa To'ldirish oynasidan ochiladi;
//   openBilling({startPage}) — 0 To'ldirish, 1 Obuna, 2 Tarix.

import { push } from '../router.js';
import { appBar, bindAppBar, icon, spinner, toast, dialog, openLink, C } from '../ui.js';
import { esc } from '../format.js';
import { billing, planLabel, formatSum, formatLeft, linkLeft } from '../services/billing.js';

const LABELS = ["To'ldirish", 'Obuna', 'Tarix'];

// Havola kuzatuvi (`_LinkTileState._pollPlan`): boshida tez-tez, keyin siyrak.
const POLL_PLAN = [
  [2 * 60_000, 4_000],
  [10 * 60_000, 15_000],
  [2 * 3600_000, 60_000],
];

const two = (n) => String(n).padStart(2, '0');

function whenText(ms) {
  if (ms <= 0) return '';
  const d = new Date(ms);
  return `${two(d.getDate())}.${two(d.getMonth() + 1)}.${d.getFullYear()}  ${two(d.getHours())}:${two(d.getMinutes())}`;
}

/** Tarifning eng qisqasiga nisbatan tejash foizi (`_discountOf`). */
function discountOf(plan, all) {
  if (!all.length || plan.days <= 0) return 0;
  let base = all[0];
  for (const p of all) if (p.days > 0 && p.days < base.days) base = p;
  if (base.days <= 0 || base.price <= 0 || plan.days <= base.days) return 0;
  const saved = Math.round((1 - (plan.price / plan.days) / (base.price / base.days)) * 100);
  return saved < 0 ? 0 : saved;
}

function placeholder(loading, text) {
  return `<div class="bill-ph">
    ${loading ? spinner(24, 2.2, 'rgba(255,255,255,0.54)')
    : icon('receipt_long', { size: 40, color: 'rgba(255,255,255,0.25)' })}
    <div class="t">${esc(text)}</div></div>`;
}

/** `SubCountdown`: har soniyada yangilanadigan yozuv. */
function countdown(expired = "Yo'q") {
  return `<span class="bill-cd" data-exp="${esc(expired)}"></span>`;
}

function paintCountdowns(root) {
  const left = billing.left;
  root.querySelectorAll('.bill-cd').forEach((el) => {
    const t = left <= 0 ? el.dataset.exp : formatLeft(left);
    if (el.textContent !== t) el.textContent = t;
  });
}

export function openBilling({ startPage } = {}) {
  const start = startPage ?? (billing.balance > 0 ? 1 : 0);
  push((el) => buildBilling(el, start), { transition: 'fade' });
}

function buildBilling(el, start) {
  el.classList.add('bill');
  el.innerHTML = `
    ${appBar({ title: 'Obuna va balans' })}
    <div class="bill-bar-wrap"><div class="glass bill-bar">
      <div class="col"><div class="k">Balans</div><div class="bal"></div></div>
      <div class="grow"></div>
      <div class="col end"><div class="k">Obuna</div><div class="sub">${countdown()}</div></div>
    </div></div>
    <div class="bill-switch">${LABELS.map((l, i) => `<div class="bill-sw" data-i="${i}">${l}</div>`).join('')}</div>
    <div class="bill-pages">
      <div class="bill-page" data-p="0"></div>
      <div class="bill-page" data-p="1"></div>
      <div class="bill-page" data-p="2"></div>
    </div>`;
  bindAppBar(el);

  const pagesEl = el.querySelector('.bill-pages');
  const [topEl, subEl, histEl] = [...el.querySelectorAll('.bill-page')];
  const sw = [...el.querySelectorAll('.bill-sw')];
  let page = start;

  function setPage(i) {
    page = i;
    sw.forEach((b, k) => b.classList.toggle('on', k === i));
  }
  function goTo(i, smooth = true) {
    if (i !== page) setPage(i);
    pagesEl.scrollTo({ left: i * pagesEl.clientWidth, behavior: smooth ? 'smooth' : 'auto' });
  }
  sw.forEach((b, i) => b.addEventListener('click', () => goTo(i)));
  pagesEl.addEventListener('scroll', () => {
    const w = pagesEl.clientWidth || 1;
    const i = Math.round(pagesEl.scrollLeft / w);
    if (i !== page && Math.abs(pagesEl.scrollLeft - i * w) < w * 0.5) setPage(i);
  }, { passive: true });
  setPage(start);
  requestAnimationFrame(() => goTo(start, false));

  // ── Balans qatori ──
  function paintBar() {
    el.querySelector('.bill-bar .bal').textContent = formatSum(billing.balance);
    el.querySelector('.bill-bar .sub').style.color = billing.active ? C.success : 'rgba(255,255,255,0.7)';
  }

  // ── 1) TO'LDIRISH ──
  topEl.innerHTML = `
    <div class="glass bill-card bill-amount">
      <div class="lbl">Summani yozing</div>
      <label class="bill-input"><input type="text" inputmode="numeric" placeholder="10000" maxlength="12"><span class="suf">so'm</span></label>
      <button class="btn btn-filled bill-go"><span class="t">To'lashga o'tish</span></button>
    </div>
    <div class="bill-links"></div>`;
  const amountIn = topEl.querySelector('input');
  const goBtn = topEl.querySelector('.bill-go');
  const linksEl = topEl.querySelector('.bill-links');
  amountIn.addEventListener('input', () => {
    const v = amountIn.value.replace(/[^0-9]/g, '');
    if (v !== amountIn.value) amountIn.value = v;
  });
  let creating = false;
  goBtn.addEventListener('click', async () => {
    if (creating) return;
    const n = parseInt(amountIn.value.replace(/[^0-9]/g, ''), 10);
    if (!n || n <= 0) { toast('Summani yozing'); return; }
    creating = true;
    goBtn.disabled = true;
    goBtn.innerHTML = spinner(18, 2, '#fff');
    const r = await billing.createLink(n);
    creating = false;
    goBtn.disabled = false;
    goBtn.innerHTML = `<span class="t">To'lashga o'tish</span>`;
    if (r.error) { toast(r.error); return; }
    amountIn.value = '';
    amountIn.blur();
    if (!r.url) { toast('Havola tayyor — pastdagi "To\'lash" tugmasini bosing'); return; }
    try { openLink(r.url); } catch (_) { toast('Brauzer ochilmadi — pastdagi "To\'lash" tugmasini bosing'); }
  });

  // Har bir havola: o'z kuzatuvchisi (pul o'zi tushadi).
  const watchers = new Map(); // orderId -> {el, timer, from, checking, done, busy}

  function gapOf(w) {
    const since = Date.now() - w.from;
    for (const [until, gap] of POLL_PLAN) if (since < until) return gap;
    return POLL_PLAN[POLL_PLAN.length - 1][1];
  }
  function arm(w) {
    clearTimeout(w.timer);
    if (w.done || closed) return;
    w.timer = setTimeout(async () => { await silentCheck(w); arm(w); }, gapOf(w));
  }
  function onPaid() { goTo(2); }
  async function silentCheck(w) {
    if (w.checking || w.done || closed) return;
    w.checking = true;
    try {
      const r = await billing.check(w.orderId);
      if (!closed && r.paid) { w.done = true; clearTimeout(w.timer); onPaid(); }
    } catch (_) { /* keyingi urinishda */ }
    w.checking = false;
  }

  function linkTile(l) {
    const d = document.createElement('div');
    d.className = 'glass bill-card bill-link';
    d.innerHTML = `
      <div class="row">
        <div class="amt">${esc(formatSum(l.amount))}</div><div class="grow"></div>
        ${icon('timer', { fill: false, size: 14, color: 'rgba(255,255,255,0.5)' })}
        <div class="left"></div>
      </div>
      <div class="btns">
        <button class="btn btn-filled pay">To'lov qilish</button>
        <button class="btn bill-outline chk">Tekshirish</button>
      </div>`;
    d.querySelector('.pay').addEventListener('click', () => {
      try { openLink(l.url); } catch (_) { toast('Brauzer ochilmadi'); }
    });
    const chk = d.querySelector('.chk');
    chk.addEventListener('click', async () => {
      const w = watchers.get(l.orderId);
      if (!w || w.busy) return;
      w.busy = true;
      chk.disabled = true;
      chk.innerHTML = spinner(16, 2, 'rgba(255,255,255,0.7)');
      const r = await billing.check(l.orderId);
      w.busy = false;
      chk.disabled = false;
      chk.textContent = 'Tekshirish';
      if (closed) return;
      if (r.error) { toast(r.error); return; }
      if (!r.paid) { toast("Hozircha to'lov ko'rinmadi"); return; }
      w.done = true;
      clearTimeout(w.timer);
      toast("To'lov qabul qilindi — balans yangilandi");
      onPaid();
    });
    return d;
  }

  function paintLeft() {
    for (const w of watchers.values()) {
      const s = Math.floor(linkLeft(w.link) / 1000);
      const t = `Qolgan vaqt ${two(Math.floor(s / 60))}:${two(s % 60)} daqiqa`;
      const le = w.el.querySelector('.left');
      if (le.textContent !== t) le.textContent = t;
    }
  }

  let shownKey = '';
  function paintLinks() {
    const links = billing.links.filter((l) => linkLeft(l) > 0);
    const key = links.map((l) => l.orderId).join(',');
    if (key === shownKey) { paintLeft(); return; }
    shownKey = key;
    // Ro'yxatdan chiqqan havolalarning kuzatuvi to'xtaydi.
    for (const [id, w] of watchers) {
      if (!links.some((l) => l.orderId === id)) { clearTimeout(w.timer); watchers.delete(id); }
    }
    linksEl.innerHTML = '';
    if (!links.length) {
      linksEl.innerHTML = `<div class="bill-note">Summani yozing — to'lov sahifasi o'zi ochiladi.<br>Pul hisobingizga avtomatik tushadi, havola esa<br>1 soat faol turadi.</div>`;
      return;
    }
    for (const l of links) {
      let w = watchers.get(l.orderId);
      const tile = linkTile(l);
      if (!w) {
        w = { orderId: l.orderId, link: l, el: tile, timer: 0, from: Date.now(), checking: false, done: false, busy: false };
        watchers.set(l.orderId, w);
        arm(w);
      } else {
        w.el = tile;
        w.link = l;
      }
      linksEl.appendChild(tile);
    }
    paintLeft();
  }

  // Odam bank ilovasidan/brauzerdan QAYTDI — darhol tekshiriladi.
  function onVisible() {
    if (document.visibilityState !== 'visible') return;
    for (const w of watchers.values()) {
      if (w.done) continue;
      w.from = Date.now();
      silentCheck(w);
      arm(w);
    }
  }
  document.addEventListener('visibilitychange', onVisible);

  // ── 2) OBUNA ──
  function paintSubscribe() {
    const plans = billing.plans;
    if (!plans.length) {
      subEl.innerHTML = placeholder(billing.isLoading, billing.error ?? 'Tariflar yuklanmoqda...');
      return;
    }
    const locked = billing.active;
    subEl.innerHTML = `
      ${locked ? `<div class="glass bill-card bill-active">
        ${icon('verified', { size: 20, color: C.accent })}
        <div class="col">
          <div class="t">Obunangiz faol</div>
          <div class="r"><span class="y">Yana </span>${countdown('tugadi')}</div>
          <div class="s">Tugagach yangi tarif tanlay olasiz.</div>
        </div></div>` : ''}
      ${plans.map((p, i) => {
        const disc = discountOf(p, plans);
        return `<div class="glass bill-card bill-plan" data-i="${i}">
          <div class="col">
            <div class="r"><span class="n">+ ${esc(planLabel(p.days))}</span>
              ${disc >= 5 ? `<span class="disc">${disc}% chegirma</span>` : ''}</div>
            <div class="p">${esc(formatSum(p.price))}</div>
          </div>
          <button class="btn btn-filled buy"${locked ? ' disabled' : ''}>Sotib olish</button>
        </div>`;
      }).join('')}
      <div class="bill-foot">${locked ? 'Obunangiz tugagach yangi tarif tanlay olasiz.' : 'Obuna balansdan yechiladi.'}</div>`;
    subEl.querySelectorAll('.bill-plan').forEach((tile) => {
      const p = plans[Number(tile.dataset.i)];
      const btn = tile.querySelector('.buy');
      btn.addEventListener('click', () => buy(p, btn, locked));
    });
    paintCountdowns(subEl);
  }

  let buying = false;
  async function buy(plan, btn, locked) {
    if (buying) return;
    if (locked) { toast('Sizda faol obuna bor — u tugagach yangisini olasiz'); return; }
    if (billing.balance < plan.price) { toast("Balansda mablag' yetarli emas — avval to'ldiring"); return; }
    const ok = await dialog({
      title: 'Obunani tasdiqlang',
      cls: 'bill-dlg',
      content: `<div class="dlg-text">${esc(planLabel(plan.days))} obuna — ${esc(formatSum(plan.price))}.<br>Summa balansingizdan yechiladi.</div>`,
      actions: [{ text: 'Bekor qilish', value: false }, { text: 'Sotib olish', value: true }],
    });
    if (ok !== true || closed) return;
    buying = true;
    btn.disabled = true;
    btn.innerHTML = spinner(16, 2, '#fff');
    const err = await billing.subscribe(plan.days);
    buying = false;
    if (closed) return;
    // Muvaffaqiyatda ro'yxat qayta chiziladi (`billing.listen`).
    if (btn.isConnected) { btn.disabled = false; btn.textContent = 'Sotib olish'; }
    toast(err ?? 'Obuna faollashtirildi');
  }

  // ── 3) TARIX ──
  function paintHistory() {
    const h = billing.history;
    if (!h.length) {
      histEl.innerHTML = placeholder(billing.isLoading, billing.error ?? "Hozircha yozuv yo'q");
      return;
    }
    histEl.innerHTML = h.map((e) => {
      const up = e.kind === 'topup';
      const col = up ? C.success : C.accent;
      return `<div class="bill-hrow"><div class="glass bill-hist">
        <div class="ib" style="background:${up ? 'rgba(74,222,128,0.16)' : 'rgba(194,65,12,0.16)'}">
          ${icon(up ? 'add_card' : 'workspace_premium', { size: 18, color: col })}</div>
        <div class="col">
          <div class="t">${up ? "Balans to'ldirildi" : `${esc(planLabel(e.days))} obuna`}</div>
          <div class="w">${whenText(e.createdAt)}</div>
        </div>
        <div class="a" style="color:${up ? C.success : 'rgba(255,255,255,0.8)'}">${up ? '+' : '−'}${esc(formatSum(e.amount))}</div>
      </div></div>`;
    }).join('');
  }

  function paintAll() {
    paintBar();
    paintLinks();
    paintSubscribe();
    paintHistory();
    paintCountdowns(el);
  }

  let closed = false;
  const unsub = billing.listen(paintAll);
  const tick = setInterval(() => {
    paintCountdowns(el);
    paintLinks();
  }, 1000);
  paintAll();
  // Ekran ochilganda eng yangi holat olinadi.
  billing.load({ force: true });

  return {
    dispose() {
      closed = true;
      unsub();
      clearInterval(tick);
      document.removeEventListener('visibilitychange', onVisible);
      for (const w of watchers.values()) clearTimeout(w.timer);
      watchers.clear();
    },
  };
}
