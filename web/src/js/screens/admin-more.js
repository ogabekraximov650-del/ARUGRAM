// Admin paneli qolgan bo'limlari — `ui/admin/AdminOtherScreens.kt`, `AdminReportsScreen.kt`,
// `AdminEncodeScreen.kt`, `AdminDbScreen.kt`. Serverga so'rovlar ilovadagi bilan bir xil.
// Ataylab YO'Q: "Emoji, GIF va stikerlar" (alohida bosqich) va "Brauzer" (faqat ilovadagi WebView).

import { api, imageUrl } from '../api.js';
import { esc, formatCount, toInt } from '../format.js';
import { push } from '../router.js';
import { appBar, bindAppBar, bindTap, toast, spinner, mdDialog } from '../ui.js';
import { formatBytes, formatHours, formatMoment } from './stats.js';
import { openPublicProfile } from './public-profile.js';

const errOf = (e, d) => e?.body?.error || e?.message || d;
const load = () => `<div class="am-center">${spinner(34, 2, 'rgba(255,255,255,0.54)')}</div>`;
const empty = (t) => `<div class="am-center"><span class="am-dim">${esc(t)}</span></div>`;
const pill = (t, cls = '') => `<button class="am-pill ${cls}">${esc(t)}</button>`;

function shell(title, build, { actions = '' } = {}) {
  push((el) => {
    el.innerHTML = `${appBar({ title, actions })}<div class="scroll"><div class="am"></div></div>`;
    bindAppBar(el);
    const host = el.querySelector('.am');
    const state = { dead: false };
    const r = build(host, state, el) || {};
    return { dispose() { state.dead = true; r.dispose?.(); } };
  });
}

// ══════════ Majburiy kanallar ══════════
export function openAdminChannels() {
  shell('Majburiy kanallar', (host, st, el) => {
    let items = null; let error = null;
    const paint = () => {
      if (items == null) { host.innerHTML = error ? empty(error) : load(); return; }
      if (!items.length) { host.innerHTML = empty("Kanal yo'q"); return; }
      host.innerHTML = items.map((c, i) => {
        const need = toInt(c.need);
        return `<div class="am-card" data-i="${i}">
          <div class="am-t">${esc(c.title)}</div>
          <div class="am-s">${c.kind === 'private' ? 'Yopiq' : 'Ochiq'} · ${toInt(c.joined)} / ${need > 0 ? need : 'cheksiz'} · ${c.active ? 'faol' : "to'ldi"}</div>
          <div class="am-f">${esc(c.url)}</div>
          <div class="am-row">${pill('Limit', 'lim')}${pill("O'chirish", 'danger del')}</div></div>`;
      }).join('');
      host.querySelectorAll('.am-card').forEach((card) => {
        const c = items[+card.dataset.i];
        card.querySelector('.lim').addEventListener('click', () => limit(c));
        card.querySelector('.del').addEventListener('click', () => del(c));
      });
    };
    async function refresh() {
      try { const r = await api('/api/admin/channels'); if (st.dead) return; items = Array.isArray(r?.items) ? r.items : []; error = null; } catch (e) { error = errOf(e, 'Yuklanmadi'); }
      paint();
    }
    async function post(body, ok) {
      try { await api('/api/admin/channels', { method: 'POST', body }); toast(ok); await refresh(); } catch (e) { toast(errOf(e, 'Xato')); }
    }
    async function limit(c) {
      const v = prompt('Limit (0 — cheksiz)', toInt(c.need) > 0 ? String(toInt(c.need)) : '');
      if (v == null) return;
      post({ op: 'set', id: c.id, need: toInt(v.replace(/\D/g, '')) }, 'Saqlandi');
    }
    async function del(c) {
      const ok = await mdDialog({ title: "Kanalni o'chirish", html: `«${esc(c.title)}» ro'yxatdan olib tashlanadi.`, confirm: { label: "O'chirish", color: '#F87171' }, dismiss: 'Bekor' });
      if (ok) post({ op: 'del', id: c.id }, "O'chirildi");
    }
    el.insertAdjacentHTML('beforeend', '<button class="am-fab">+ Kanal qo\'shish</button>');
    el.querySelector('.am-fab').addEventListener('click', () => openChannelAdd(refresh));
    refresh();
    paint();
  });
}

function openChannelAdd(done) {
  shell("Kanal qo'shish", (host) => {
    host.innerHTML = `<input class="field am-in" placeholder="@username, -100... ID yoki t.me/+havola">
      <div class="am-row" style="margin-top:12px"><button class="am-pill on" data-k="public">Ochiq</button><button class="am-pill" data-k="private">Yopiq</button></div>
      <div class="am-row" style="margin-top:12px;align-items:center"><span class="am-t" style="flex:1">Cheksiz</span><label class="switch"><input type="checkbox" class="am-un" checked><span class="tr"></span><span class="th"></span></label></div>
      <input class="field am-need" inputmode="numeric" placeholder="Limit (masalan 1000)" style="margin-top:10px;display:none">
      <div class="am-err"></div>
      <button class="am-pill on am-go" style="margin-top:16px;padding:12px 20px">Qo'shish</button>
      <div class="am-s" style="margin-top:12px">Eslatma: botni kanalga admin qilib qo'shish ilovada (Telegram hisobi bilan) bajariladi; saytda kanalni faqat ro'yxatga qo'shasiz.</div>`;
    let kind = 'public';
    host.querySelectorAll('[data-k]').forEach((b) => b.addEventListener('click', () => {
      kind = b.dataset.k; host.querySelectorAll('[data-k]').forEach((x) => x.classList.toggle('on', x === b));
    }));
    const un = host.querySelector('.am-un'); const need = host.querySelector('.am-need');
    un.addEventListener('change', () => { need.style.display = un.checked ? 'none' : 'block'; });
    const err = host.querySelector('.am-err');
    host.querySelector('.am-go').addEventListener('click', async () => {
      const input = host.querySelector('.am-in').value.trim();
      const n = un.checked ? 0 : toInt(need.value);
      if (!input) { err.textContent = 'Kanalni yozing'; return; }
      if (!un.checked && n <= 0) { err.textContent = "Limitni yozing yoki «Cheksiz»ni yoqing"; return; }
      try {
        await api('/api/admin/channels', { method: 'POST', body: { op: 'add', kind, input, need: n } });
        toast("Kanal qo'shildi"); done?.(); history.back();
      } catch (e) { err.textContent = errOf(e, 'Xato'); }
    });
  });
}

// ══════════ Umumiy statistika ══════════
const RANGES = [['24h', '24 soat'], ['7d', '7 kun'], ['30d', '30 kun'], ['all', 'Hammasi']];

export function openAdminStats() {
  shell('Umumiy statistika', (host, st) => {
    host.innerHTML = load();
    (async () => {
      let s;
      try { s = await api('/api/stats'); } catch (e) { if (!st.dead) host.innerHTML = empty(errOf(e, 'Yuklanmadi')); return; }
      if (st.dead) return;
      const c = s.content || {};
      const cnt = (v) => `${formatCount(toInt(v))} ta`;
      const cards = [
        ['Foydalanuvchilar', s.users, 'users', cnt], ["Anime ko'rishlar", s.anime_views, 'anime_views', cnt],
        ["Bo'lim ko'rishlar", s.season_views, 'season_views', cnt], ["Qism ko'rishlar", s.views, 'views', cnt],
        ["Ko'rish vaqti", s.watch, 'watch_ms', (v) => `${formatHours(toInt(v))} soat`], ['Trafik sarfi', s.traffic, 'traffic', (v) => formatBytes(toInt(v))],
      ];
      host.innerHTML = `<div class="am-card"><div class="am-t">Kontent</div>
        <div class="am-s" style="font-size:13px">Anime: ${toInt(c.anime)} · Bo'lim: ${toInt(c.seasons)} · Qism: ${toInt(c.episodes)}</div>
        <div class="am-f" style="margin-top:4px">Yangi qismlar: bugun ${toInt(c.episodes_daily)} · hafta ${toInt(c.episodes_weekly)} · oy ${toInt(c.episodes_monthly)}</div></div>
        ${cards.map(([t, b, m, f], i) => `<div class="am-card am-stat" data-i="${i}"><div class="am-t">${t}</div>
          <div class="am-big">${f(b?.total)}</div>
          <div class="am-cols">${[['Bugun', b?.daily], ['Hafta', b?.weekly], ['Oy', b?.monthly]].map(([l, v]) => `<div><div class="am-v">${f(v)}</div><div class="am-f">${l}</div></div>`).join('')}</div>
          <div class="am-chart"></div></div>`).join('')}
        <div class="am-f" style="margin:6px 0 28px">Vaqt mintaqasi: UTC+5 (Toshkent)</div>`;
      host.querySelectorAll('.am-stat').forEach((card) => {
        const [, , m, f] = cards[+card.dataset.i];
        let open = false;
        card.addEventListener('click', (e) => {
          if (e.target.closest('.am-chart')) return;
          open = !open;
          const ch = card.querySelector('.am-chart');
          ch.style.display = open ? 'block' : 'none';
          if (open && !ch.dataset.ready) { ch.dataset.ready = '1'; trend(ch, m, f); }
        });
      });
    })();
  });
}

function trend(box, metric, fmt) {
  let range = '7d'; let touch = -1;
  const draw = async () => {
    box.innerHTML = `<div class="am-row" style="margin-top:12px">${RANGES.map(([k, l]) => `<button class="am-pill sm${k === range ? ' on' : ''}" data-r="${k}">${l}</button>`).join('')}</div><div class="am-plot">${load()}</div>`;
    box.querySelectorAll('[data-r]').forEach((b) => b.addEventListener('click', () => { range = b.dataset.r; touch = -1; draw(); }));
    let pts = [];
    try { pts = (await api(`/api/stats/series?metric=${encodeURIComponent(metric)}&range=${range}`)).points || []; } catch (_) { pts = []; }
    const plot = box.querySelector('.am-plot');
    if (!pts.length) { plot.innerHTML = '<div class="am-center" style="height:120px"><span class="am-dim">Ma\'lumot yo\'q</span></div>'; return; }
    const max = Math.max(1, ...pts.map((p) => toInt(p.v)));
    plot.innerHTML = `<div class="am-bars">${pts.map((p, i) => `<i data-i="${i}" style="height:${Math.max(1, (toInt(p.v) / max) * 100)}%"></i>`).join('')}</div><div class="am-f am-pt" style="margin-top:6px">Eng yuqori: ${fmt(max)} · bosing</div>`;
    const bars = plot.querySelectorAll('.am-bars i');
    bars.forEach((b) => b.addEventListener('click', () => {
      touch = +b.dataset.i; bars.forEach((x, i) => x.classList.toggle('sel', i === touch));
      plot.querySelector('.am-pt').textContent = `${String(pts[touch].t).replace('T', ' ')} → ${fmt(pts[touch].v)}`;
    }));
  };
  draw();
}

// ══════════ Ilova va xavfsizlik ══════════
export function openAdminApp() {
  shell('Ilova va xavfsizlik', (host, st) => {
    host.innerHTML = load();
    let cfg = null;
    async function refresh() {
      try { cfg = await api('/api/admin/app'); } catch (e) { host.innerHTML = empty(errOf(e, 'Yuklanmadi')); return; }
      if (st.dead) return;
      host.innerHTML = `<div class="am-card"><div class="am-t">Eng kichik versiya</div>
        <div class="am-s" style="margin:6px 0 10px">Bundan eski ilovalar yangilanmaguncha ishlamaydi. Masalan 0.1.0 yoki 0.1.0+9. Bo'sh — cheklov yo'q.</div>
        <input class="field am-ver" maxlength="20" value="${esc(cfg.min_version || '')}"><div class="am-row" style="margin-top:10px">${pill('Saqlash', 'on sv')}</div></div>
        <div class="am-card"><div class="am-t">Ilova imzosi</div>
        <div class="am-s" style="margin:6px 0">${cfg.gate_on ? 'Himoya YOQILGAN: faqat tanilgan imzoli ilovalar kiradi.' : "Himoya o'chiq."}</div>
        <div class="am-f">Ishonchli imzolar: ${toInt(cfg.sig_count)} · bu qurilma: ${cfg.my_sig_trusted ? 'ishonchli' : 'ishonchsiz'}</div>
        <div class="am-row" style="margin-top:10px">${pill('Shu imzoga ishonish', 'tm')}${pill('Faqat shu imzo', 'om')}${pill('Tozalash', 'danger cl')}</div></div>`;
      const post = async (body, ok) => { try { await api('/api/admin/app', { method: 'POST', body }); toast(ok); } catch (e) { toast(errOf(e, 'Saqlanmadi')); } refresh(); };
      host.querySelector('.sv').addEventListener('click', () => { const v = host.querySelector('.am-ver').value.trim(); post({ min_version: v }, v ? 'Saqlandi' : "O'chirildi"); });
      host.querySelector('.tm').addEventListener('click', () => post({ trust_me: true }, "Qo'shildi"));
      host.querySelector('.om').addEventListener('click', () => post({ only_me: true }, 'Saqlandi'));
      host.querySelector('.cl').addEventListener('click', () => post({ clear: true }, 'Tozalandi'));
    }
    refresh();
  });
}

// ══════════ Shikoyatlar ══════════
export function openAdminReports() {
  shell('Shikoyatlar', (host, st) => {
    let items = []; let total = 0; let page = 0; let more = false; let ready = false; let err = null; let busy = false;
    const person = (p) => `${p?.first_name || ''}`.trim() || (p?.username ? `@${p.username}` : 'Foydalanuvchi');
    const paint = () => {
      if (!ready) { host.innerHTML = err ? empty(err) : load(); return; }
      if (!items.length) { host.innerHTML = empty(err || "Shikoyat yo'q"); return; }
      host.innerHTML = `<div class="am-f" style="margin-bottom:10px">Jami: ${total}</div>${items.map((r, i) => `<div class="am-card" data-i="${i}">
        <div class="am-row" style="align-items:center"><span class="am-src">${r.kind === 'comment' ? 'Izohdan' : r.kind === 'dm' ? 'Shaxsiy yozishmadan' : esc(r.kind)}</span><span class="am-f">${esc(formatMoment(r.created_at))}</span></div>
        <div class="am-t" style="margin-top:6px;font-weight:400">Sabab: ${esc(r.reason)}</div>
        <div class="am-quote">${r.target_alive ? esc(r.target_body || '(matnsiz)') : "(o'chirilgan)"}</div>
        <div class="am-row" style="justify-content:space-between;margin-top:8px"><span class="am-s au" style="cursor:pointer">Muallif: ${esc(person(r.target_user))}</span><span class="am-s rp" style="cursor:pointer">Shikoyatchi: ${esc(person(r.reporter))}</span></div>
        <div class="am-row" style="margin-top:10px">${r.kind === 'comment' && r.target_alive ? pill("Izohni o'chirish", 'danger dc') : ''}${pill('Bloklash', 'danger bn')}${pill('Tozalash', 'cl')}</div></div>`).join('')}
        ${more ? `<div style="text-align:center;margin:8px 0 24px">${pill("Yana yuklash", 'mr')}</div>` : ''}`;
      host.querySelectorAll('.am-card').forEach((card) => {
        const r = items[+card.dataset.i];
        card.querySelector('.au').addEventListener('click', () => openPublicProfile(r.target_user?.id));
        card.querySelector('.rp').addEventListener('click', () => openPublicProfile(r.reporter?.id));
        card.querySelector('.dc')?.addEventListener('click', async () => {
          try { await api(`/api/comments/${r.target_id}`, { method: 'DELETE' }); toast("O'chirildi"); reload(); } catch (e) { toast(errOf(e, "O'chirilmadi")); }
        });
        card.querySelector('.cl').addEventListener('click', async () => {
          try { await api(`/api/admin/report/${r.id}`, { method: 'DELETE' }); items = items.filter((x) => x.id !== r.id); total = Math.max(0, total - 1); paint(); } catch (e) { toast(errOf(e, 'Tozalanmadi')); }
        });
        card.querySelector('.bn').addEventListener('click', async () => {
          const days = prompt(`${person(r.target_user)} necha kunga bloklansin? (0 — butunlay)`, '7');
          if (days == null) return;
          const reason = prompt('Sabab (ixtiyoriy)', r.reason || '') || '';
          try { await api(`/api/admin/user/${r.target_user?.id}`, { method: 'POST', body: { action: 'ban', days: toInt(days), reason } }); toast('Bloklandi'); } catch (e) { toast(errOf(e, 'Bloklanmadi')); }
        });
      });
      host.querySelector('.mr')?.addEventListener('click', loadMore);
    };
    async function reload() {
      try { const j = await api('/api/admin/reports?page=0'); if (st.dead) return; items = j.items || []; total = toInt(j.total ?? items.length); more = !!j.has_more; page = 0; err = null; } catch (e) { err = errOf(e, 'Yuklanmadi'); }
      ready = true; paint();
    }
    async function loadMore() {
      if (busy) return; busy = true;
      try { const j = await api(`/api/admin/reports?page=${page + 1}`); const have = new Set(items.map((x) => x.id)); items = items.concat((j.items || []).filter((x) => !have.has(x.id))); more = !!j.has_more; page += 1; } catch (_) { /* */ }
      busy = false; paint();
    }
    reload(); paint();
  });
}

// ══════════ Kodlash navbati ══════════
const clock = (s) => {
  if (s == null || Number.isNaN(+s) || +s < 0) return '—';
  s = Math.floor(+s); const two = (n) => String(n).padStart(2, '0');
  return s >= 3600 ? `${Math.floor(s / 3600)}:${two(Math.floor(s / 60) % 60)}:${two(s % 60)}` : `${two(Math.floor(s / 60))}:${two(s % 60)}`;
};
const num = (v, d = 1, suf = '') => (v == null || Number.isNaN(+v) ? '—' : `${(+v).toFixed(d)}${suf}`);
const stat = (k, v) => `<div class="am-stile"><div class="am-f">${k}</div><div class="am-v">${esc(String(v))}</div></div>`;
const ecard = (title, body, trail = '') => `<div class="am-card"><div class="am-row" style="align-items:center"><span class="am-t" style="flex:1;font-size:14px">${title}</span><span class="am-f">${trail}</span></div><div style="margin-top:10px">${body}</div></div>`;
const bar = (p, label) => `<div class="am-bar"><div class="am-bar-t"><div style="width:${Math.max(0, Math.min(100, +p || 0))}%"></div></div><span>${label}</span></div>`;

function queueRow(j, error = false) {
  return `<div class="am-qr"><div class="am-qp">${j.photo_url ? `<img alt="" loading="lazy" src="${esc(imageUrl(j.photo_url))}" onerror="this.remove()">` : ''}</div>
    <div style="min-width:0;flex:1"><div class="am-t" style="font-size:13px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis">${esc(j.nomi || j.anime_name || '')}</div>
    <div class="am-s">${toInt(j.bolim_id)}-bo'lim · ${toInt(j.epizod_number)}-qism</div>
    ${error && j.text ? `<div class="am-f" style="color:#F87171">${esc(j.text)}</div>` : ''}</div></div>`;
}

export function openAdminEncode() {
  shell('Kodlash navbati', (host, st) => {
    host.innerHTML = load();
    let queue = null; let ticks = 0; let lastJob = ''; let timer = 0;
    async function tick() {
      let live = null;
      try { live = await api('/api/encode/live'); } catch (_) { /* */ }
      if (st.dead) return;
      const s = live?.status;
      const job = s ? `${s.anime_id}:${s.season_id}:${s.epizod_id}` : '';
      if (queue == null || job !== lastJob || ticks % 15 === 0) {
        try { queue = await api('/api/encode/admin'); } catch (_) { /* */ }
        lastJob = job;
      }
      ticks++;
      if (st.dead) return;
      paint(live, queue);
      timer = setTimeout(tick, 4000);
    }
    function paint(live, q) {
      const s = live?.status; const data = s?.data || {};
      const cur = data.cur; const xfer = data.xfer; const src = data.src; const sys = data.sys; const ladder = data.ladder || [];
      if (!q && !s) { host.innerHTML = empty('Yuklanmadi'); return; }
      const updated = toInt(s?.updated_at); const age = updated > 0 ? Math.floor((Date.now() - updated) / 1000) : -1;
      const out = [`<div class="am-f" style="color:${age >= 0 && age < 120 ? '#4ADE80' : 'var(--gold)'};font-size:12px">${age >= 0 && age < 120 ? `● Jonli · ${age}s` : age >= 0 ? `● Eski holat · ${age}s` : "● Holat yo'q"}</div>`];
      if (cur) {
        out.push(ecard(`Kodlanmoqda: ${esc(cur.q)}`, `${bar(cur.pct, `${num(cur.pct)}%`)}
          <div class="am-t" style="font-size:13px;margin:8px 0">Kodlandi: ${clock(cur.out_s)} / ${clock(cur.dur_s)}</div>
          <div class="am-tiles">${stat('Kadr', `${toInt(cur.frame)}${toInt(src?.frames) > 0 ? ` / ${src.frames}` : ''}`)}${stat('Kadr/s', num(cur.fps))}${stat('Tezlik', num(cur.speed, 2, 'x'))}${stat('Bitreyt', num(cur.bitrate_kbps, 0, ' kb/s'))}
          ${stat('Hajm', num(cur.size_mb, 1, ' MB'))}${stat('Taxminiy hajm', num(cur.est_mb, 0, ' MB'))}${stat("O'tdi", clock(cur.elapsed))}${stat('Qoldi', `~${clock(cur.eta)}`)}${stat('Kvantizator (q)', num(cur.qp))}${stat('Tashlangan / takror', `${toInt(cur.drop)} / ${toInt(cur.dup)}`)}</div>`));
      } else if (xfer) {
        out.push(ecard(esc(xfer.kind || 'Yuklash'), `${bar(xfer.pct, `${num(xfer.pct)}%`)}<div class="am-tiles" style="margin-top:8px">${stat('Hajm', `${num(xfer.cur_mb)} / ${num(xfer.total_mb, 1, ' MB')}`)}${stat('Tezlik', num(xfer.mbps, 2, ' MB/s'))}${stat("O'tdi", clock(xfer.elapsed))}${stat('Qoldi', `~${clock(xfer.eta)}`)}</div>`));
      } else out.push(ecard('Joriy bosqich', `<span class="am-s" style="font-size:13px">${esc(s?.progress || "Hozir kodlanayotgan qism yo'q")}</span>`));
      if (ladder.length) {
        out.push(ecard('Sifatlar', ladder.map((r) => {
          const stt = r.state; const ic = stt === 'done' ? '✓' : (stt === 'enc' || stt === 'upload') ? '⟳' : '○';
          const lbl = { enc: 'kodlanmoqda', upload: 'yuklanmoqda', done: 'tayyor' }[stt] || 'kutmoqda';
          return `<div class="am-lad"><span style="width:22px;color:${stt === 'done' ? '#4ADE80' : 'var(--accent2)'}">${ic}</span><b style="width:52px">${esc(r.q)}</b><span class="am-s">${[lbl, r.crf != null ? `CRF ${r.crf}` : '', r.size_mb != null ? `${r.size_mb} MB` : ''].filter(Boolean).join(' · ')}</span></div>`;
        }).join(''), `${ladder.filter((r) => r.state === 'done').length}/${ladder.length} tayyor`));
      }
      if (src) out.push(ecard('Asl video', `<div class="am-tiles">${stat("O'lcham", `${toInt(src.w) > 0 ? `${src.w}×` : ''}${src.h}p`)}${stat('Davomiyligi', clock(src.dur_s))}${stat('Hajm', num(src.mb, 1, ' MB'))}${stat('Bitreyt', num(src.kbps, 0, ' kb/s'))}${src.fps != null ? stat('Kadr/s', num(src.fps, 2)) : ''}${src.codec ? stat('Kodek', src.codec) : ''}</div>`));
      if (sys) out.push(ecard('Kompyuter (GitHub runner)', `<div class="am-tiles">${stat('CPU', num(sys.cpu, 1, '%'))}${stat('Yadro', sys.cores ?? '-')}${stat('Yuklama', num(sys.load, 2))}${stat('RAM', toInt(sys.ram_total_mb) > 0 ? `${toInt(sys.ram_used_mb)} / ${toInt(sys.ram_total_mb)} MB` : '-')}${stat("Bo'sh disk", num(sys.disk_free_gb, 1, ' GB'))}</div>`));
      const lines = (s?.lines || []).filter((x) => `${x}`.trim());
      out.push(ecard('Encode log', lines.length ? `<pre class="am-log">${esc(lines.slice(-60).join('\n'))}</pre>` : `<span class="am-f">${esc(live?.status_error || "Log yo'q")}</span>`, updated > 0 ? esc(formatMoment(updated)) : ''));
      const gh = live?.gh || live?.github;
      if (gh) {
        const url = gh.url || gh.html_url || '';
        out.push(ecard('GitHub run', `${url ? `<a class="am-link" href="${esc(url)}" target="_blank" rel="noopener">${esc(url)}</a>` : ''}${(gh.steps || []).map((x) => `<div class="am-f">${esc(x.name)} — ${esc(x.status)}</div>`).join('')}`, esc(gh.status || '')));
      }
      const running = q?.running || []; const queued = q?.queue || []; const errors = q?.errors || [];
      out.push(ecard(`Hozir (${running.length})`, running.length ? running.map((j) => queueRow(j)).join('') : '<span class="am-f">Yo\'q</span>'));
      out.push(ecard(`Navbatda (${queued.length})`, queued.length ? queued.map((j) => queueRow(j)).join('') : '<span class="am-f">Navbat bo\'sh</span>'));
      if (errors.length) out.push(ecard(`Xatolar (${errors.length})`, errors.map((j) => queueRow(j, true)).join('')));
      const sc = host.parentElement.scrollTop;
      host.innerHTML = out.join('<div style="height:12px"></div>');
      host.parentElement.scrollTop = sc;
    }
    tick();
    return { dispose() { clearTimeout(timer); } };
  });
}

// ══════════ Ma'lumotlar bazasi (faqat o'qish) ══════════
export function openAdminDb() {
  shell("Ma'lumotlar bazasi", (host, st) => {
    host.innerHTML = `<input class="field am-in" placeholder="Jurnal nomini qidiring"><div class="am-f" style="margin:8px 4px"></div><div class="am-dbl"></div>`;
    const list = host.querySelector('.am-dbl'); const cnt = host.querySelector('.am-f');
    let tables = null;
    const paint = () => {
      if (!tables) return;
      const q = host.querySelector('.am-in').value.trim().toLowerCase();
      const shown = tables.filter((t) => !q || t.name.toLowerCase().includes(q));
      cnt.textContent = `${shown.length} ta jurnal · faqat o'qish`;
      list.innerHTML = shown.map((t) => `<div class="am-card am-tb" data-n="${esc(t.name)}" style="display:flex;padding:12px 14px;cursor:pointer"><span class="am-mono" style="flex:1">${esc(t.name)}</span>${t.kind === 'view' ? '<span style="color:var(--accent);font-size:11px">ko\'rinish</span>' : ''}</div>`).join('') || empty('Jurnal topilmadi');
      list.querySelectorAll('.am-tb').forEach((r) => r.addEventListener('click', () => openDbRows(r.dataset.n)));
    };
    host.querySelector('.am-in').addEventListener('input', paint);
    list.innerHTML = load();
    api('/api/admin/db/tables').then((r) => { if (st.dead) return; tables = r.items || []; paint(); }).catch((e) => { list.innerHTML = empty(errOf(e, 'Jurnallar olinmadi')); });
  });
}

function openDbRows(table) {
  shell(table, (host, st) => {
    let cols = []; let rows = []; let more = true; let order = ''; let desc = true; let col = ''; let q = ''; let busy = false;
    host.innerHTML = `<div class="am-row" style="margin-bottom:10px"><select class="field am-sel"></select><input class="field am-in" placeholder="Qidirish" style="flex:1"></div><div class="am-tw"></div><div class="am-more"></div>`;
    const wrap = host.querySelector('.am-tw'); const sel = host.querySelector('.am-sel');
    const cell = (v) => (v == null ? '<span class="am-null">NULL</span>' : esc(String(v).length > 200 ? `${String(v).slice(0, 200)}…` : String(v)));
    const paint = () => {
      wrap.innerHTML = `<table class="am-table"><thead><tr>${cols.map((c) => `<th data-c="${esc(c.name)}">${esc(c.name)}${order === c.name ? (desc ? ' ▼' : ' ▲') : ''}</th>`).join('')}</tr></thead>
        <tbody>${rows.map((r, i) => `<tr data-i="${i}">${r.map((v) => `<td>${cell(v)}</td>`).join('')}</tr>`).join('')}</tbody></table>`;
      host.querySelector('.am-more').innerHTML = more && rows.length ? `<div style="text-align:center;margin:10px 0 24px">${pill('Yana yuklash', 'mr')}</div>` : '';
      host.querySelector('.mr')?.addEventListener('click', () => fetchRows(false));
      wrap.querySelectorAll('th').forEach((th) => th.addEventListener('click', () => { if (order === th.dataset.c) desc = !desc; else { order = th.dataset.c; desc = true; } fetchRows(true); }));
      wrap.querySelectorAll('tbody tr').forEach((tr) => tr.addEventListener('click', () => {
        const r = rows[+tr.dataset.i];
        mdDialog({ title: table, html: `<div class="am-det">${cols.map((c, i) => `<div class="am-f">${esc(c.name)}</div><div class="am-detv">${r[i] == null ? '<span class="am-null">NULL</span>' : esc(String(r[i]))}</div>`).join('')}</div>`, dismiss: 'Yopish' });
      }));
    };
    async function fetchRows(reset) {
      if (busy) return; busy = true;
      if (reset) { wrap.innerHTML = load(); rows = []; }
      try {
        const j = await api(`/api/admin/db/rows?table=${encodeURIComponent(table)}&offset=${rows.length}&limit=100&order=${encodeURIComponent(order)}&dir=${desc ? 'desc' : 'asc'}&col=${encodeURIComponent(q ? col : '')}&q=${encodeURIComponent(q)}`);
        if (st.dead) return;
        if (reset || !cols.length) {
          cols = j.columns || [];
          if (!col) { col = cols[0]?.name || ''; sel.innerHTML = cols.map((c) => `<option>${esc(c.name)}</option>`).join(''); }
        }
        rows = rows.concat(j.rows || []); more = !!j.has_more; paint();
      } catch (e) { wrap.innerHTML = empty(errOf(e, 'Yozuvlar olinmadi')); }
      busy = false;
    }
    sel.addEventListener('change', () => { col = sel.value; if (q) fetchRows(true); });
    let tm = 0;
    host.querySelector('.am-in').addEventListener('input', (e) => { clearTimeout(tm); tm = setTimeout(() => { q = e.target.value.trim(); fetchRows(true); }, 450); });
    fetchRows(true);
  });
}
