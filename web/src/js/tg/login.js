// Telegram hisobiga kirish oynasi — `lib/screens/phone_login_screen.dart`.
//
// Telegram Android'ning tungi ko'rinishi: to'q ko'k fon, chapga
// tekislangan sarlavha, davlat qatori, kod + raqam maydonlari, kod uchun
// kataklar, pastki o'ngda ko'k dumaloq tugma, bosqichlar orasida yon
// tomonga siljish. Kirish mtcute orqali (`client.js`).
//
// Mini App'da ilova hisobi (account) allaqachon ochiq (Telegram initData) —
// shu sabab bu oyna faqat Telegram'ning O'ZIGA (videolar uchun) kiradi
// (ilovadagi `connectOnly` rejimi).

import qrcode from 'qrcode-generator';
import { push } from '../router.js';
import { esc } from '../format.js';
import { tgUser } from '../api.js';
import { icon } from '../ui.js';
import { getClient, tgError, markAuthorized, isAuthorized, tgLogout } from './client.js';
import { countries, countryByIso, countryByCode } from './countries.js';

const TG = {
  bg: '#1C242F', accent: '#50A8EB', accentDark: '#3A8FD6', hint: '#7D8B99',
  faint: '#4F5D6B', line: '#34414E', red: '#E5575F',
};

const clock = (s) => {
  const h = Math.floor(s / 3600); const m = Math.floor((s % 3600) / 60); const sec = s % 60;
  const two = (v) => String(v).padStart(2, '0');
  return h > 0 ? `${h}:${two(m)}:${two(sec)}` : `${m}:${two(sec)}`;
};

let open = null;

/** Kirish oynasi. Kirilsa `true`, yopilsa `false`. */
export function openTelegramLogin() {
  if (open) return open;
  open = new Promise((resolve) => {
    let done = false;
    push((el, route) => {
      const st = {
        step: 'phone', busy: false, error: '', hint: '', wait: 0, resendLeft: 0, sent: null,
        country: countryByIso('UZ'), cc: '998', num: '', code: '', password: '', showPassword: false,
        phoneCodeHash: '', qrUrl: '', shake: 0, qrAbort: null,
      };
      const timers = [];
      el.classList.add('tgl');
      el.innerHTML = `
        <div class="tgl-bar"><button class="icon-btn tgl-back">${icon('arrow_back', { fill: false, size: 24 })}</button></div>
        <div class="tgl-body"></div>
        <button class="tgl-fab" aria-label="Davom etish"></button>`;
      const body = el.querySelector('.tgl-body');
      const fab = el.querySelector('.tgl-fab');
      const backBtn = el.querySelector('.tgl-back');

      const phone = () => `+${st.cc}${st.num.replace(/\D/g, '')}`;
      const prettyPhone = () => `+${st.cc} ${st.country ? st.country.format(st.num.replace(/\D/g, '')) : st.num}`.trim();
      const codeLen = () => { const n = st.sent?.length || 0; return n >= 4 && n <= 8 ? n : 5; };

      function whereSent() {
        const p = phone();
        switch (st.sent?.type) {
          case 'sms': case 'sms_word': case 'sms_phrase':
            return `Telegram ${p} raqamiga SMS orqali kod yubordi.`;
          case 'call':
            return `Telegram ${p} raqamiga qo'ng'iroq qiladi — kodni aytib beradi.`;
          case 'flash_call': case 'missed_call':
            return `Telegram ${p} raqamiga qo'ng'iroq qiladi. Kod — qo'ng'iroq qilgan raqamning oxirgi ${codeLen()} ta raqami.`;
          case 'email':
            return 'Kod emailingizga yuborildi.';
          case 'fragment':
            return 'Kod Fragment orqali yuborildi.';
          default:
            return `Telegram ${p} raqamiga kirish kodini yubordi.\nKod Telegram ilovasidagi "Telegram" chatida (boshqa qurilmadagi Telegram'da ham ko'rinadi).`;
        }
      }
      const nextLabel = () => ({ sms: 'SMS orqali yuborish', call: "Qo'ng'iroq orqali yuborish", flash_call: "Qo'ng'iroq orqali yuborish", missed_call: "Qo'ng'iroq orqali yuborish" }[st.sent?.nextType] || 'Kodni qayta yuborish');

      function header(ic, title, sub) {
        return `<div class="tgl-head">
          <div class="tgl-circle">${icon(ic, { fill: false, size: 52, color: '#fff' })}</div>
          <div class="tgl-h1 c">${esc(title)}</div>
          <div class="tgl-sub c">${esc(sub).replace(/\n/g, '<br>')}</div>
        </div>`;
      }

      function stepHtml() {
        switch (st.step) {
          case 'phone': {
            const c = st.country;
            return `<div class="tgl-h1" style="margin-top:16px">Telefon raqamingiz</div>
              <div class="tgl-sub">Davlatni tanlang va Telegram hisobingiz ulangan telefon raqamini kiriting.</div>
              <div class="tgl-country">
                ${c ? `<span class="flag">${c.flag}</span>` : ''}
                <span class="nm${c ? '' : ' dim'}">${c ? esc(c.name) : (st.cc ? "Noto'g'ri kod" : 'Davlatni tanlang')}</span>
                ${icon('chevron_right', { fill: false, size: 24, color: TG.hint })}
              </div>
              <div class="tgl-row">
                <label class="tgl-cc"><span>+</span><input class="tgl-in cc" inputmode="tel" maxlength="4" value="${esc(st.cc)}"></label>
                <input class="tgl-in num" inputmode="tel" placeholder="${c && c.pattern ? c.pattern.replace(/X/g, '0') : 'Telefon raqami'}" value="${esc(st.num)}">
              </div>
              <div class="tgl-center"><button class="tgl-link qr">${icon('qr_code_2', { size: 24, color: TG.accent })}<span>QR kod orqali kirish</span></button></div>`;
          }
          case 'code': {
            const n = codeLen();
            const cells = Array.from({ length: n }, (_, i) => {
              const on = i === st.code.length || (i === n - 1 && st.code.length === n);
              return `<div class="tgl-cell${st.error ? ' err' : on ? ' on' : ''}">${esc(st.code[i] || '')}</div>`;
            }).join('');
            return `${header('sms', prettyPhone(), whereSent())}
              <div class="tgl-cells-wrap"><div class="tgl-cells shake-${st.shake}">${cells}</div>
                <input class="tgl-hidden code" inputmode="numeric" autocomplete="one-time-code" maxlength="${n}" value="${esc(st.code)}"></div>
              <div class="tgl-links">
                <button class="tgl-link resend" ${st.busy || st.resendLeft > 0 ? 'disabled' : ''}>${esc(st.resendLeft > 0 ? `${nextLabel()} (${clock(st.resendLeft)})` : nextLabel())}</button>
                <button class="tgl-link change" ${st.busy ? 'disabled' : ''}>Raqamni o'zgartirish</button>
              </div>`;
          }
          case 'password':
            return `${header('lock', 'Parolingiz', st.hint
              ? `Hisobingizda ikki bosqichli tekshiruv yoqilgan. Parolingizni kiriting.\nEslatma: ${st.hint}`
              : 'Hisobingizda ikki bosqichli tekshiruv yoqilgan. Parolingizni kiriting.')}
              <div class="tgl-pass shake-${st.shake}">
                <input class="tgl-in pw" type="${st.showPassword ? 'text' : 'password'}" placeholder="Parol" value="${esc(st.password)}">
                <button class="icon-btn eye">${icon(st.showPassword ? 'visibility_off' : 'visibility', { fill: false, size: 24, color: TG.hint })}</button>
              </div>
              <div class="tgl-links"><button class="tgl-link change" ${st.busy ? 'disabled' : ''}>Raqamni o'zgartirish</button></div>`;
          case 'qr': {
            const steps = ["Telefoningizda Telegram'ni oching", 'Sozlamalar → Qurilmalar → Qurilmani ulash', 'Kirishni tasdiqlash uchun telefonni shu QR kodga qarating'];
            return `<div class="tgl-qr-box">${st.qrUrl ? qrSvg(st.qrUrl) : `<div class="spinner" style="width:36px;height:36px;border-top-color:${TG.accent};border-right-color:${TG.accent}"></div>`}</div>
              <div class="tgl-h1 c" style="font-size:21px;margin-top:28px">QR kod orqali Telegram'ga kirish</div>
              <div class="tgl-steps">${steps.map((t, i) => `<div class="tgl-step"><span class="n">${i + 1}</span><span class="t">${esc(t)}</span></div>`).join('')}</div>
              <div class="tgl-links"><button class="tgl-link tophone">Raqam orqali kirish</button></div>`;
          }
          case 'finishing':
            return `${header('verified_user', 'Kirilmoqda', 'Hisobingiz tasdiqlanmoqda...')}
              ${st.error ? '' : `<div class="tgl-center"><div class="spinner" style="width:36px;height:36px;border-top-color:${TG.accent};border-right-color:${TG.accent}"></div></div>`}`;
          default: return '';
        }
      }

      function render(anim = false) {
        const showFab = st.step !== 'qr' && !(st.step === 'finishing' && !st.error);
        fab.classList.toggle('hide', !showFab);
        fab.innerHTML = st.busy
          ? '<div class="spinner" style="width:24px;height:24px;border-width:2.4px;border-top-color:#fff;border-right-color:#fff"></div>'
          : icon('arrow_forward', { fill: false, size: 24, color: '#fff' });
        const canBack = !(st.step === 'finishing' && !st.error);
        backBtn.style.visibility = canBack ? 'visible' : 'hidden';
        body.innerHTML = `<div class="tgl-step-body${anim ? ' enter' : ''}">${stepHtml()}
          ${st.wait > 0 ? `<div class="tgl-wait">Qayta urinish mumkin: ${clock(st.wait)} dan keyin</div>` : ''}
          ${st.error ? `<div class="tgl-err">${esc(st.error)}</div>` : ''}</div>`;
        bind();
      }

      function focus(sel) { setTimeout(() => body.querySelector(sel)?.focus(), 60); }

      /** Kod kataklari — maydonga tegmasdan, joyida yangilanadi. */
      function paintCells() {
        const n = codeLen();
        body.querySelectorAll('.tgl-cell').forEach((c, i) => {
          const on = i === st.code.length || (i === n - 1 && st.code.length === n);
          c.className = `tgl-cell${st.error ? ' err' : on ? ' on' : ''}`;
          const ch = st.code[i] || '';
          if (c.textContent !== ch) c.textContent = ch;
        });
      }

      function bind() {
        body.querySelector('.tgl-country')?.addEventListener('click', () => { if (!st.busy) pickCountry(); });
        const cc = body.querySelector('.cc');
        cc?.addEventListener('input', () => {
          cc.value = cc.value.replace(/\D/g, '').slice(0, 4);
          st.cc = cc.value;
          st.country = countryByCode(st.cc, st.country);
          const d = st.num.replace(/\D/g, '');
          st.num = st.country ? st.country.format(d) : d;
          const keep = document.activeElement === cc;
          render();
          if (keep) { const n = body.querySelector('.cc'); n.focus(); n.setSelectionRange(n.value.length, n.value.length); }
        });
        const num = body.querySelector('.num');
        if (num) {
          num.addEventListener('input', () => {
            let d = num.value.replace(/\D/g, '');
            const max = st.country && st.country.length ? st.country.length : 15;
            d = d.slice(0, max);
            st.num = st.country ? st.country.format(d) : d;
            num.value = st.num;
          });
          num.addEventListener('keydown', (e) => { if (e.key === 'Enter') submit(); });
          if (st.step === 'phone') focus('.num');
        }
        body.querySelector('.qr')?.addEventListener('click', () => { if (!st.busy) startQr(); });
        const code = body.querySelector('.code');
        if (code) {
          // Maydon QAYTA YARATILMAYDI: har raqamda butun oynani qayta chizish
          // telefonda klaviaturani uzar, kursor boshiga tushib raqamlar
          // teskari yozilar yoki o'chib ketardi. Faqat kataklar yangilanadi.
          const toEnd = () => { try { const l = code.value.length; code.setSelectionRange(l, l); } catch (_) { /* */ } };
          code.addEventListener('input', () => {
            const clean = code.value.replace(/\D/g, '').slice(0, codeLen());
            if (clean !== code.value) code.value = clean;
            toEnd();
            st.code = clean;
            if (st.error) { st.error = ''; body.querySelector('.tgl-err')?.remove(); }
            paintCells();
            if (st.code.length === codeLen()) submit();
          });
          code.addEventListener('focus', toEnd);
          code.addEventListener('click', toEnd);
          body.querySelector('.tgl-cells-wrap').addEventListener('click', () => { code.focus(); toEnd(); });
          focus('.code');
        }
        body.querySelector('.resend')?.addEventListener('click', resend);
        body.querySelectorAll('.change').forEach((b) => b.addEventListener('click', backToPhone));
        const pw = body.querySelector('.pw');
        if (pw) {
          pw.addEventListener('input', () => { st.password = pw.value; });
          pw.addEventListener('keydown', (e) => { if (e.key === 'Enter') submit(); });
          focus('.pw');
        }
        body.querySelector('.eye')?.addEventListener('click', () => { st.showPassword = !st.showPassword; render(); });
        body.querySelector('.tophone')?.addEventListener('click', () => { stopQr(); go('phone'); });
      }

      function go(step) { st.step = step; st.error = ''; render(true); }

      function setWait(secs) {
        st.wait = secs;
        if (secs > 0) {
          const t = setInterval(() => {
            st.wait -= 1;
            if (st.wait <= 0) { clearInterval(t); st.error = ''; render(); return; }
            const w = body.querySelector('.tgl-wait');
            if (w) w.textContent = `Qayta urinish mumkin: ${clock(st.wait)} dan keyin`; else render();
          }, 1000);
          timers.push(t);
        }
      }

      function setSent(sent) {
        st.sent = sent;
        st.resendLeft = Math.min(600, Math.max(0, sent?.timeout > 0 ? sent.timeout : 30));
        const t = setInterval(() => {
          st.resendLeft -= 1;
          if (st.resendLeft <= 0) clearInterval(t);
          if (st.step === 'code') {
            const b = body.querySelector('.resend');
            if (b) { b.disabled = st.busy || st.resendLeft > 0; b.textContent = st.resendLeft > 0 ? `${nextLabel()} (${clock(st.resendLeft)})` : nextLabel(); }
          }
        }, 1000);
        timers.push(t);
      }

      function fail(e, { wait = 0 } = {}) {
        if (wait > 0) setWait(wait);
        if (st.step === 'code' || st.step === 'password') {
          try { window.Telegram?.WebApp?.HapticFeedback?.notificationOccurred('error'); } catch (_) { /* */ }
          st.shake += 1;
          if (st.step === 'code') st.code = '';
          if (st.step === 'password') st.password = '';
        }
        st.busy = false;
        st.error = e || '';
        render();
      }

      async function submit() {
        if (st.busy) return;
        const cl = await getClient().catch((e) => { fail(tgError(e).text); return null; });
        if (!cl) return;
        if (st.step === 'phone') {
          if (st.wait > 0) return;
          if (phone().length < 8) { fail("Raqamni to'liq kiriting"); return; }
          st.busy = true; st.error = ''; render();
          try {
            const r = await cl.sendCode({ phone: phone() });
            if (r && r.type === undefined && r.id) { await finish(); return; }
            st.phoneCodeHash = r.phoneCodeHash;
            st.busy = false;
            setSent({ type: r.type, nextType: r.nextType, timeout: r.timeout, length: r.length });
            st.code = '';
            go('code');
          } catch (e) { const er = tgError(e); fail(er.text, { wait: er.wait }); }
        } else if (st.step === 'code') {
          if (!st.code) return;
          st.busy = true; st.error = ''; render();
          try {
            await cl.signIn({ phone: phone(), phoneCodeHash: st.phoneCodeHash, phoneCode: st.code });
            await finish();
          } catch (e) {
            if (`${e?.text || e?.message}`.includes('SESSION_PASSWORD_NEEDED')) {
              st.hint = (await cl.getPasswordHint().catch(() => '')) || '';
              st.busy = false;
              go('password');
            } else { const er = tgError(e); fail(er.text, { wait: er.wait }); }
          }
        } else if (st.step === 'password') {
          if (!st.password) return;
          st.busy = true; st.error = ''; render();
          try {
            await cl.checkPassword(st.password);
            await finish();
          } catch (e) {
            const er = tgError(e);
            fail(er.text === e?.message ? "Parol noto'g'ri" : er.text, { wait: er.wait });
          }
        } else if (st.step === 'finishing') {
          await finish();
        }
      }

      async function resend() {
        if (st.busy || st.resendLeft > 0 || st.wait > 0) return;
        st.busy = true; st.error = ''; render();
        try {
          const cl = await getClient();
          const r = await cl.resendCode({ phone: phone(), phoneCodeHash: st.phoneCodeHash });
          st.phoneCodeHash = r.phoneCodeHash;
          st.busy = false; st.code = '';
          setSent({ type: r.type, nextType: r.nextType, timeout: r.timeout, length: r.length });
          render();
        } catch (e) { const er = tgError(e); fail(er.text, { wait: er.wait }); }
      }

      function backToPhone() { stopQr(); st.code = ''; st.password = ''; st.phoneCodeHash = ''; go('phone'); }

      async function startQr() {
        go('qr');
        st.qrUrl = '';
        render();
        const cl = await getClient().catch(() => null);
        if (!cl) return;
        const ac = new AbortController();
        st.qrAbort = ac;
        try {
          await cl.signInQr({
            onUrlUpdated: (url) => { st.qrUrl = url; if (st.step === 'qr') render(); },
            password: async () => {
              st.hint = (await cl.getPasswordHint().catch(() => '')) || '';
              // Parol so'raladi — QR oynasidan parol bosqichiga o'tamiz.
              stopQr();
              go('password');
              throw new Error('__password_step');
            },
            abortSignal: ac.signal,
          });
          await finish();
        } catch (e) {
          if (`${e?.message}` === '__password_step' || ac.signal.aborted) return;
          if (st.step === 'qr') { st.error = tgError(e).text; render(); }
        }
      }
      function stopQr() { try { st.qrAbort?.abort(); } catch (_) { /* */ } st.qrAbort = null; }

      async function finish() {
        st.busy = true; st.error = ''; st.step = 'finishing'; render(true);
        const ok = await isAuthorized({ fresh: true });
        if (ok) {
          const opener = tgUser()?.id;
          const me = await getClient().then((c) => c.getMe()).catch(() => null);
          if (me && opener && `${me.id}` !== `${opener}`) {
            await tgLogout();
            fail("Bu Telegram hisobi Mini App ochilgan hisobga mos emas — shu hisobning o'z raqami bilan kiring");
            return;
          }
          markAuthorized(true);
          done = true;
          resolve(true);
          route.close();
        } else {
          fail("Kirish tasdiqlanmadi — qayta urinib ko'ring");
        }
      }

      function pickCountry() {
        push((cel) => {
          cel.classList.add('tgl', 'tgl-pick');
          cel.innerHTML = `<div class="tgl-pick-bar"><button class="icon-btn back">${icon('arrow_back', { fill: false, size: 24 })}</button>
            <input class="tgl-search" placeholder="Qidirish" autofocus></div><div class="scroll tgl-list"></div>`;
          const list = cel.querySelector('.tgl-list');
          const q = cel.querySelector('.tgl-search');
          const draw = () => {
            const s = q.value.trim().toLowerCase().replace('+', '');
            const items = countries.filter((c) => !s || c.name.toLowerCase().includes(s) || c.code.startsWith(s));
            list.innerHTML = items.map((c, i) => `<div class="tgl-ci" data-i="${i}"><span class="flag">${c.flag}</span><span class="nm">${esc(c.name)}</span><span class="cd">+${c.code}</span></div>`).join('');
            list.querySelectorAll('.tgl-ci').forEach((row) => row.addEventListener('click', () => {
              const c = items[Number(row.dataset.i)];
              st.country = c; st.cc = c.code;
              st.num = c.format(st.num.replace(/\D/g, ''));
              history.back();
              render();
              focus('.num');
            }));
          };
          q.addEventListener('input', draw);
          cel.querySelector('.back').addEventListener('click', () => history.back());
          draw();
          setTimeout(() => q.focus(), 80);
          return {};
        });
      }

      fab.addEventListener('click', () => { if (!st.busy) submit(); });
      backBtn.addEventListener('click', () => {
        if (st.step === 'qr') { stopQr(); go('phone'); } else if (st.step === 'code' || st.step === 'password') backToPhone();
        else if (st.step === 'finishing') go('phone');
        else route.close();
      });

      render();
      // Allaqachon kirilgan bo'lsa — darhol yopiladi.
      isAuthorized().then((ok) => { if (ok && !done) { done = true; resolve(true); route.close(); } });

      return {
        onBack() {
          if (st.step !== 'phone' && st.step !== 'finishing') { backBtn.click(); return false; }
          return true;
        },
        dispose() {
          timers.forEach(clearInterval);
          stopQr();
          if (!done) resolve(false);
          open = null;
        },
      };
    }, { transition: 'slide' });
  });
  return open;
}

/** QR kod — Telegram uslubida dumaloq nuqtalar (ilovadagi `QrImageView`). */
function qrSvg(text) {
  const qr = qrcode(0, 'M');
  qr.addData(text);
  qr.make();
  const n = qr.getModuleCount();
  const s = 220 / n;
  const r = s * 0.45;
  const color = '#1C2733';
  const isEye = (x, y) => (x < 7 && y < 7) || (x >= n - 7 && y < 7) || (x < 7 && y >= n - 7);
  let dots = '';
  for (let y = 0; y < n; y++) {
    for (let x = 0; x < n; x++) {
      if (!qr.isDark(y, x) || isEye(x, y)) continue;
      dots += `<circle cx="${(x + 0.5) * s}" cy="${(y + 0.5) * s}" r="${r}"/>`;
    }
  }
  const eye = (ex, ey) => {
    const cx = (ex + 3.5) * s; const cy = (ey + 3.5) * s;
    return `<circle cx="${cx}" cy="${cy}" r="${3 * s}" fill="none" stroke="${color}" stroke-width="${s}"/>
      <circle cx="${cx}" cy="${cy}" r="${1.5 * s}" fill="${color}"/>`;
  };
  return `<svg width="220" height="220" viewBox="0 0 220 220"><g fill="${color}">${dots}</g>
    ${eye(0, 0)}${eye(n - 7, 0)}${eye(0, n - 7)}</svg>`;
}
