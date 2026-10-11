// Admin: foydalanuvchilar — `lib/screens/admin_users_screen.dart`.
// Uch varaq (ilovadagi tartibda): Yangi, Onlayn, Suhbatlar (uchinchi).
// Server (ilovadagi bilan bir xil): GET /api/admin/users?sort=new|online&q=&page=,
// POST /api/admin/user/:id {action: balance|sub|sub_clear|ban|unban}, GET /api/chat/threads,
// DELETE /api/chat/thread/:id. Suhbat ichi — `support.js` (admin ko'rinishi).

import { api } from '../api.js';
import { esc, formatCount } from '../format.js';
import { push } from '../router.js';
import { icon, appBar, bindAppBar, bindTap, toast, dialog, sheet, confirmDialog, spinner, avatarHtml, C } from '../ui.js';
import { formatSum } from '../services/billing.js';
import { formatMoment } from '../player/util.js';
import { openSupport } from './support.js';
import { openPublicProfile } from './public-profile.js';

const TABS = [
  { id: 'new', label: 'Yangi' },
  { id: 'online', label: 'Onlayn' },
  { id: 'chats', label: 'Suhbatlar' },
];

const nameOf = (u) => {
  const n = `${u.first_name ?? ''} ${u.last_name ?? ''}`.trim();
  if (n) return n;
  if (u.username) return `@${u.username}`;
  return `Foydalanuvchi ${u.id}`;
};
const threadName = (t) => {
  const n = `${t.first_name ?? ''}`.trim();
  if (n) return n;
  return t.username ? `@${t.username}` : `Foydalanuvchi ${t.user_id}`;
};
const subDaysLeft = (u) => {
  const ms = Number(u.sub_until || 0) - Date.now();
  return ms <= 0 ? 0 : Math.ceil(ms / 86400000);
};
function banLeftText(u) {
  const ms = Number(u.ban_until || 0) - Date.now();
  if (ms <= 0) return '0 daqiqa';
  const d = Math.floor(ms / 86400000);
  const h = Math.floor((ms % 86400000) / 3600000);
  if (d > 0) return h > 0 ? `${d} kun ${h} soat` : `${d} kun`;
  const hh = Math.floor(ms / 3600000);
  const m = Math.floor((ms % 3600000) / 60000);
  if (hh > 0) return m > 0 ? `${hh} soat ${m} daqiqa` : `${hh} soat`;
  return `${Math.min(59, Math.max(1, Math.floor(ms / 60000)))} daqiqa`;
}
const errText = (e) => `${e?.body?.error || e?.message || e}`;

export function openAdminUsers() {
  push((el) => {
    let tab = 'new';
    let closed = false;
    let picked = false;
    let query = '';
    let debounce = 0;
    let pollT = 0;
    // varaq holati: users[sort] = {items, page, total, more, loading, error}
    const users = { new: null, online: null };
    let threads = { items: [], loading: true, error: null };

    el.innerHTML = `${appBar({ title: 'Foydalanuvchilar' })}
      <div class="ad-ut">
        <div class="ad-search"><span>${icon('search', { size: 19, color: 'rgba(255,255,255,0.5)' })}</span>
          <input class="ad-q" placeholder="ID raqam yoki username" autocomplete="off"><button class="ad-qx" hidden aria-label="Tozalash">${icon('close', { size: 17, color: 'rgba(255,255,255,0.54)' })}</button></div>
        <div class="ad-tabs">${TABS.map((t) => `<button class="ad-tab" data-t="${t.id}">${t.label}<i class="ad-badge" hidden></i></button>`).join('')}</div>
      </div>
      <div class="scroll ad-pg"><div class="ss ad-body"></div></div>`;
    bindAppBar(el);
    const $ = (s) => el.querySelector(s);
    const body = $('.ad-body');
    const scroller = $('.ad-pg');

    // ── Foydalanuvchilar varag'i ──
    function userRow(u, sort) {
      const when = sort === 'new' ? u.created_at : u.last_login_at;
      return `<div class="glass ad-ur" data-id="${u.id}">
        ${avatarHtml(u.photo_url, nameOf(u), 46)}
        <div class="ss-main"><div class="ad-u1"><span class="ss-t" style="${u.banned ? 'text-decoration:line-through;opacity:.6' : ''}">${esc(nameOf(u))}</span><span class="ad-id">ID ${u.id}</span></div>
          <div class="ad-u2"><span class="ss-h">${esc(formatMoment(Number(when) || 0))}</span>${Number(u.balance) > 0 ? `<b class="ad-bal">${esc(formatSum(u.balance))}</b>` : ''}</div></div>
      </div>`;
    }

    function paintUsers() {
      const st = users[tab];
      if (!st || (st.loading && !st.items.length)) {
        body.innerHTML = `<div style="display:flex;justify-content:center;margin-top:48px">${spinner(30, 2, 'rgba(255,255,255,0.54)')}</div>`;
        return;
      }
      if (!st.items.length) {
        body.innerHTML = `<div class="ss-note" style="text-align:center;margin-top:48px">${esc(st.error || 'Hech kim topilmadi')}</div>`;
        return;
      }
      body.innerHTML = st.items.map((u) => userRow(u, tab)).join('')
        + `<div class="ss-note" style="text-align:center;margin:14px 0">${st.more ? spinner(20, 2, 'rgba(255,255,255,0.4)')
          : esc(query ? `${st.items.length} ta topildi` : `Jami ${formatCount(st.total)} ta foydalanuvchi`)}</div>`;
      body.querySelectorAll('.ad-ur').forEach((r) => {
        const u = st.items.find((x) => `${x.id}` === r.dataset.id);
        bindTap(r, () => openActions(u), { scale: false });
      });
    }

    async function loadUsers(sort, { more = false } = {}) {
      let st = users[sort];
      if (!st || !more) { st = users[sort] = { items: st?.items || [], page: 0, total: st?.total || 0, more: false, loading: true, error: null }; if (sort === tab) paintUsers(); }
      else if (st.loading || !st.more) return;
      else st.loading = true;
      const page = more ? st.page + 1 : 0;
      try {
        const j = await api(`/api/admin/users?sort=${sort}&page=${page}&q=${encodeURIComponent(query)}`);
        if (closed) return;
        const items = Array.isArray(j?.items) ? j.items : [];
        if (more) {
          const have = new Set(st.items.map((u) => u.id));
          st.items = st.items.concat(items.filter((u) => !have.has(u.id)));
          st.page = page;
        } else st.items = items;
        st.total = Number(j?.total) || 0;
        st.more = j?.has_more === true;
        st.error = null;
      } catch (e) {
        st.error = e?.status === 403 ? "Bu bo'lim faqat admin uchun" : (e?.status ? `Yuklanmadi (${e.status})` : "Internet yo'q");
        st.more = false;
        if (!more) st.items = [];
      }
      st.loading = false;
      if (sort === tab) paintUsers();
    }

    // ── Suhbatlar varag'i ──
    function paintBadge() {
      const n = threads.items.reduce((s, t) => s + (Number(t.unread) || 0), 0);
      const b = $('.ad-tab[data-t="chats"] .ad-badge');
      b.hidden = n <= 0;
      b.textContent = `${n}`;
    }

    function paintThreads() {
      paintBadge();
      if (threads.loading && !threads.items.length) {
        body.innerHTML = `<div style="display:flex;justify-content:center;margin-top:48px">${spinner(30, 2, 'rgba(255,255,255,0.54)')}</div>`;
        return;
      }
      if (!threads.items.length) {
        body.innerHTML = `<div class="ss-note" style="text-align:center;margin-top:60px">${icon('forum', { fill: false, size: 48, color: 'rgba(255,255,255,0.2)' })}<br>${esc(threads.error || 'Hali hech kim yozmagan')}</div>`;
        return;
      }
      body.innerHTML = threads.items.map((t) => `<div class="glass ad-ur ad-th" data-id="${t.user_id}">
        <span class="ad-av" data-p="1">${avatarHtml(t.photo_url, threadName(t), 46)}</span>
        <div class="ss-main"><div class="ad-u1"><span class="ss-t">${esc(threadName(t))}</span><span class="ad-id">${esc(formatMoment(Number(t.last_at) || 0))}</span></div>
          <div class="ad-u2"><span class="ss-h ad-lb" style="${Number(t.unread) > 0 ? 'color:rgba(255,255,255,.88);font-weight:600' : ''}">${t.last_from_admin ? 'Siz: ' : ''}${esc(`${t.last_body ?? ''}`.slice(0, 120))}</span>${Number(t.unread) > 0 ? `<i class="ad-badge on">${t.unread}</i>` : ''}</div></div>
      </div>`).join('') + '<div class="ss-note" style="text-align:center;margin:14px 0">Suhbatni o\'chirish uchun uni bosib turing</div>';
      body.querySelectorAll('.ad-th').forEach((r) => {
        const t = threads.items.find((x) => `${x.user_id}` === r.dataset.id);
        let lp = 0; let long = false; let sx = 0; let sy = 0;
        r.addEventListener('pointerdown', (e) => {
          long = false; sx = e.clientX; sy = e.clientY;
          clearTimeout(lp);
          lp = setTimeout(() => { long = true; removeThread(t); }, 550);
        });
        const cancel = () => clearTimeout(lp);
        r.addEventListener('pointermove', (e) => { if (Math.abs(e.clientX - sx) > 10 || Math.abs(e.clientY - sy) > 10) cancel(); });
        r.addEventListener('pointerup', cancel);
        r.addEventListener('pointercancel', cancel);
        r.addEventListener('pointerleave', cancel);
        r.addEventListener('contextmenu', (e) => e.preventDefault());
        r.addEventListener('click', (e) => {
          if (long) { long = false; return; }
          if (e.target.closest('.ad-av')) { openPublicProfile(t.user_id); return; }
          openChat(t.user_id, threadName(t), t.photo_url);
        });
      });
    }

    async function loadThreads({ quiet = false } = {}) {
      if (!quiet && !threads.items.length) { threads.loading = true; if (tab === 'chats') paintThreads(); }
      try {
        const j = await api('/api/chat/threads');
        if (closed) return;
        threads = { items: Array.isArray(j?.items) ? j.items : [], loading: false, error: null };
      } catch (e) {
        if (closed) return;
        threads.loading = false;
        threads.error = e?.status === 403 ? "Bu bo'lim faqat admin uchun" : "Yuklanmadi";
      }
      if (tab === 'chats') paintThreads(); else paintBadge();
    }

    function openChat(userId, title, photoUrl) {
      openSupport({ userId, title, photoUrl });
      // Orqaga qaytilganda ro'yxat yangilanadi (o'qilmaganlar soni).
      const t = setInterval(() => { if (!document.querySelector('.sup')) { clearInterval(t); if (!closed) loadThreads({ quiet: true }); } }, 600);
    }

    async function removeThread(t) {
      const ok = await confirmDialog("Suhbatni o'chirish?", `${esc(threadName(t))} bilan butun yozishma o'chirilsinmi? Bu amalni ortga qaytarib bo'lmaydi.`, { ok: "O'chirish", cancel: "Yo'q", danger: true });
      if (!ok) return;
      try {
        await api(`/api/chat/thread/${t.user_id}`, { method: 'DELETE' });
        threads.items = threads.items.filter((x) => x.user_id !== t.user_id);
        paintThreads();
      } catch (e) { toast(errText(e)); }
    }

    // ── Foydalanuvchi ustida amallar ──
    async function act(u, action, extra = {}) {
      try {
        const j = await api(`/api/admin/user/${u.id}`, { method: 'POST', body: { action, days: 0, ...extra } });
        u.balance = j?.balance ?? u.balance;
        if (j && 'banned' in j) { u.banned = j.banned === true; u.ban_until = j.ban_until ?? 0; u.ban_reason = j.ban_reason ?? ''; }
        if (j && 'subscription_until' in j) u.sub_until = j.subscription_until ?? 0;
        if (tab !== 'chats') paintUsers();
        return null;
      } catch (e) { return errText(e); }
    }

    function askNumber({ title, sub, hint, addLabel, subLabel, clearLabel }) {
      let inp;
      return dialog({
        title: esc(title),
        content: (b) => {
          b.innerHTML = `<div class="ss-note" style="margin:0 0 10px">${esc(sub)}</div><input class="field" type="number" inputmode="numeric" min="1" placeholder="${esc(hint)}">`;
          inp = b.querySelector('input');
          setTimeout(() => inp.focus(), 50);
        },
        actions: [{ text: 'Bekor', value: null }, ...(clearLabel ? [{ text: clearLabel, value: 'clear', danger: true }] : []), { text: subLabel, value: 'sub' }, { text: addLabel, value: 'add', primary: true }],
      }).then((v) => {
        if (v === 'clear') return { clear: true };
        const n = parseInt(inp?.value, 10);
        if (!v || v === 'Bekor' || !(n > 0)) return null;
        return { n, add: v === 'add' };
      });
    }

    async function askBalance(u) {
      const r = await askNumber({ title: "Balansni o'zgartirish", sub: `${nameOf(u)} · hozir ${formatSum(u.balance)}`, hint: 'Masalan 10000', addLabel: "Qo'shish", subLabel: 'Ayirish' });
      if (!r) return;
      const err = await act(u, 'balance', { amount: r.add ? r.n : -r.n });
      toast(err ?? (r.add ? `${formatSum(r.n)} qo'shildi` : `${formatSum(r.n)} yechib olindi`));
    }

    async function askSub(u) {
      const left = subDaysLeft(u);
      const r = await askNumber({
        title: 'Obuna muddati', sub: left > 0 ? `${nameOf(u)} · hozir ${left} kun qolgan` : `${nameOf(u)} · obunasi yo'q`,
        hint: 'Necha kun', addLabel: "Kun qo'shish", subLabel: 'Kun ayirish',
        clearLabel: left > 0 ? 'Obunani bekor qilish' : '',
      });
      if (r?.clear) { clearSub(u); return; }
      if (r) { const err = await act(u, 'sub', { days: r.add ? r.n : -r.n }); toast(err ?? (r.add ? `${r.n} kun qo'shildi` : `${r.n} kun olib tashlandi`)); }
    }

    async function clearSub(u) {
      const err = await act(u, 'sub_clear');
      toast(err ?? 'Obuna bekor qilindi');
    }

    async function askBan(u) {
      if (u.banned) {
        const ok = await confirmDialog('Blokdan chiqarish', `${esc(nameOf(u))} blokdan chiqarilsinmi?${u.ban_reason ? `<br><span class="ss-h">Sabab: ${esc(u.ban_reason)}</span>` : ''}`, { ok: 'Ochish', cancel: "Yo'q" });
        if (!ok) return;
        const err = await act(u, 'unban');
        toast(err ?? 'Blokdan chiqarildi');
        return;
      }
      const PRESETS = [[1, '1 kun'], [3, '3 kun'], [7, '7 kun'], [30, '30 kun'], [0, 'Muddatsiz']];
      let days = 7; let custom; let reason; let hintEl;
      const eff = () => { const c = parseInt(custom.value, 10); return c > 0 ? c : days; };
      const paintHint = () => { const d = eff(); hintEl.textContent = d > 0 ? `${d} kunga bloklanadi` : 'MUDDATSIZ bloklanadi'; };
      const v = await dialog({
        title: `${esc(nameOf(u))} bloklansinmi?`,
        content: (b) => {
          b.innerHTML = `<div class="ss-note" style="margin:0 0 10px">Barcha qurilmalaridan darhol chiqariladi. Hisobiga kirmoqchi bo'lganda bot muddat va sababni ko'rsatadi.</div>
            <div class="ad-lbl" style="margin-top:6px">MUDDAT</div>
            <div class="ad-seg">${PRESETS.map(([d, l]) => `<button type="button" class="ad-chip${d === days ? ' on' : ''}" data-d="${d}">${l}</button>`).join('')}</div>
            <input class="field ad-cd" type="number" inputmode="numeric" placeholder="Yoki kunni qo'lda yozing" style="margin-top:10px">
            <div class="ad-lbl">SABAB</div>
            <textarea class="field ad-rs" rows="2" placeholder="Nima uchun bloklanyapti?"></textarea>
            <div class="ss-note ad-bh" style="margin-top:8px;color:#ff9b6b"></div>`;
          custom = b.querySelector('.ad-cd'); reason = b.querySelector('.ad-rs'); hintEl = b.querySelector('.ad-bh');
          b.querySelector('.ad-seg').addEventListener('click', (e) => {
            const c = e.target.closest('.ad-chip'); if (!c) return;
            days = parseInt(c.dataset.d, 10); custom.value = '';
            b.querySelectorAll('.ad-chip').forEach((x) => x.classList.toggle('on', x === c));
            paintHint();
          });
          custom.addEventListener('input', () => {
            if (custom.value) b.querySelectorAll('.ad-chip').forEach((x) => x.classList.remove('on'));
            paintHint();
          });
          paintHint();
        },
        actions: [{ text: 'Bekor qilish', value: null }, { text: 'Bloklash', value: 'ban', danger: true }],
      });
      if (v !== 'ban') return;
      const d = eff();
      const err = await act(u, 'ban', { days: d, reason: reason.value.trim() });
      toast(err ?? (d > 0 ? `Bloklandi — ${d} kun` : 'Muddatsiz bloklandi'));
    }

    function openActions(u) {
      sheet((b, close) => {
        const left = subDaysLeft(u);
        const tile = (id, ic, label, danger) => `<div class="ad-act${danger ? ' danger' : ''}" data-a="${id}">${icon(ic, { size: 22, color: danger ? '#ff6b6b' : 'rgba(255,255,255,0.85)' })}<span>${label}</span></div>`;
        b.innerHTML = `<div class="ad-sh">
          <div class="ad-shh">${avatarHtml(u.photo_url, nameOf(u), 52)}<div class="ss-main"><div class="ss-t">${esc(nameOf(u))}</div>
            <div class="ss-h">${esc(`ID ${u.id} · ${formatSum(u.balance)}${left > 0 ? ` · obuna ${left} kun` : ''}${u.banned ? ' · bloklangan' : ''}`)}</div>
            ${u.banned ? `<div class="ss-h" style="color:#ff9b6b">${u.ban_until > 0 ? `Blok tugashiga ${esc(banLeftText(u))} qoldi` : 'Muddatsiz bloklangan'}${u.ban_reason ? `<br>Sabab: ${esc(u.ban_reason)}` : ''}</div>` : ''}</div></div>
          ${tile('balance', 'account_balance_wallet', "Balansni o'zgartirish")}
          ${tile('sub', 'workspace_premium', 'Obuna muddati')}
          ${tile('chat', 'forum', 'Yozishmani ochish')}
          ${tile('profile', 'person', "Profilini ko'rish")}
          ${tile('ban', u.banned ? 'lock_open' : 'block', u.banned ? 'Blokdan chiqarish' : 'Bloklash', !u.banned)}
        </div>`;
        b.querySelectorAll('.ad-act').forEach((a) => a.addEventListener('click', () => close(a.dataset.a)));
      }).then((a) => {
        if (closed || !a) return;
        if (a === 'balance') askBalance(u);
        else if (a === 'sub') askSub(u);
        else if (a === 'chat') openChat(u.id, nameOf(u), u.photo_url);
        else if (a === 'profile') openPublicProfile(u.id);
        else if (a === 'ban') askBan(u);
      });
    }

    // ── Varaqlar ──
    function setTab(t, { fromUser = false } = {}) {
      if (fromUser) picked = true;
      tab = t;
      el.querySelectorAll('.ad-tab').forEach((b) => b.classList.toggle('on', b.dataset.t === t));
      $('.ad-search').style.display = t === 'chats' ? 'none' : '';
      scroller.scrollTop = 0;
      if (t === 'chats') { paintThreads(); loadThreads({ quiet: true }); }
      else if (users[t] && !query) paintUsers(); else loadUsers(t);
      if (t !== 'chats' && users[t]?.items.length && !query) loadUsers(t); // fonda yangilash
    }
    el.querySelectorAll('.ad-tab').forEach((b) => b.addEventListener('click', () => setTab(b.dataset.t, { fromUser: true })));

    // izlash
    const qi = $('.ad-q'); const qx = $('.ad-qx');
    qi.addEventListener('input', () => {
      qx.hidden = !qi.value;
      clearTimeout(debounce);
      debounce = setTimeout(() => { query = qi.value.trim(); users.new = users.online = null; loadUsers(tab); }, 400);
    });
    qx.addEventListener('click', () => { qi.value = ''; qx.hidden = true; query = ''; users.new = users.online = null; loadUsers(tab); });

    // cheksiz ro'yxat
    scroller.addEventListener('scroll', () => {
      if (tab === 'chats') return;
      if (scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 400) loadUsers(tab, { more: true });
    }, { passive: true });

    // chapga/o'ngga surish
    let sx = 0; let sy = 0;
    scroller.addEventListener('touchstart', (e) => { sx = e.touches[0].clientX; sy = e.touches[0].clientY; }, { passive: true });
    scroller.addEventListener('touchend', (e) => {
      const dx = e.changedTouches[0].clientX - sx; const dy = e.changedTouches[0].clientY - sy;
      if (Math.abs(dx) < 70 || Math.abs(dx) < Math.abs(dy) * 1.6) return;
      const i = TABS.findIndex((t) => t.id === tab) + (dx < 0 ? 1 : -1);
      if (i >= 0 && i < TABS.length) setTab(TABS[i].id, { fromUser: true });
    }, { passive: true });

    // Suhbatlarni vaqti-vaqti bilan yangilab turish (ekran ochiq va ko'rinib turganda).
    pollT = setInterval(() => { if (!document.hidden && !document.querySelector('.sup')) loadThreads({ quiet: true }); }, 8000);

    // Boshlanish: o'qilmagan xabar bo'lsa — Suhbatlar, aks holda Yangi.
    setTab('new');
    loadThreads({ quiet: true }).then(() => {
      if (closed || picked || tab === 'chats') return;
      if (threads.items.some((t) => Number(t.unread) > 0)) setTab('chats');
    });

    return { dispose() { closed = true; clearInterval(pollT); clearTimeout(debounce); } };
  });
}
