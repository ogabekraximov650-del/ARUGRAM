// Ism va username'ni tahrirlash — `lib/screens/profile_edit_screen.dart`.
//
// Ilovadagidek: ism (eng ko'pi 20 ta ko'zga ko'ringan belgi) va
// username (3-15, faqat A-Z a-z 0-9 _; yozish to'xtagach 350 ms dan
// keyin `GET /api/auth/username-check`). Saqlash — `POST /api/auth/profile`
// (ilova ham shu yo'lni chaqiradi). Bio maydoni ilovada yo'q; profil
// rasmi esa profil sahifasida (va u Telegram orqali yuklanadi —
// saytda yo'q, `profile.js` izohiga qarang).

import { api, ApiError, currentUser } from '../api.js';
import { push, back } from '../router.js';
import { C, icon, spinner, bindTap } from '../ui.js';

export const NAME_MAX = 20;

function graphemes(s) {
  try {
    if (typeof Intl !== 'undefined' && Intl.Segmenter) {
      return [...new Intl.Segmenter(undefined, { granularity: 'grapheme' }).segment(s)].map((x) => x.segment);
    }
  } catch (_) { /* */ }
  return [...s];
}

/** `AuthService.nameProblem` */
export function nameProblem(s) {
  const n = `${s}`.trim();
  if (!n) return "Ism bo'sh bo'lmasin";
  if (graphemes(n).length > NAME_MAX) return `Ism eng ko'pi ${NAME_MAX} ta belgi bo'lishi mumkin`;
  return null;
}

/** `AuthService.usernameProblem` (worker'dagi `username_problem` bilan bir xil). */
export function usernameProblem(u) {
  if (u.length < 3) return "Username kamida 3 ta belgidan iborat bo'lsin";
  if (u.length > 15) return "Username eng ko'pi 15 ta belgi bo'lishi mumkin";
  if (!/^[A-Za-z0-9_]+$/.test(u)) return 'Faqat harf, raqam va pastki chiziq (_) ishlatiladi';
  return null;
}

/** `null` — javob olinmadi. */
async function usernameAvailable(u) {
  try {
    const d = await api(`/api/auth/username-check?u=${encodeURIComponent(u)}`);
    if (!d || d.valid !== true) return d ? false : null;
    return d.available === true;
  } catch (_) {
    return null;
  }
}

/**
 * `onSaved()` — saqlangandan keyin (profil sahifasi qayta chiziladi).
 */
export function openProfileEdit({ onSaved } = {}) {
  push((el) => {
    const u = currentUser() || {};
    const startName = `${u.first_name ?? ''}`;
    const startUser = `${u.username ?? ''}`;
    let state = 'boshlangich'; // yozilmoqda | tekshirilmoqda | bosh | band | xato
    let problem = '';
    let saving = false;
    let saveError = null;
    let debounce = 0;
    let run = 0;

    el.innerHTML = `<div class="scroll"><div class="pe">
      <div class="pe-top">
        <div class="glass pe-back">${icon('arrow_back', { size: 24, color: '#fff' })}</div>
        <div class="pe-title">Profilni tahrirlash</div>
      </div>
      <div class="pe-label" style="margin-top:26px">Ism</div>
      <div class="glass pe-field">${icon('person', { fill: false, size: 20, color: 'rgba(255,255,255,0.38)' })}
        <input class="pe-in pe-name" type="text" placeholder="Ismingiz" autocomplete="off"></div>
      <div class="pe-help pe-name-help"></div>
      <div class="pe-label" style="margin-top:22px">Username</div>
      <div class="glass pe-field">${icon('alternate_email', { size: 20, color: 'rgba(255,255,255,0.38)' })}
        <input class="pe-in pe-user" type="text" placeholder="username" maxlength="15" autocomplete="off"
          autocapitalize="off" autocorrect="off" spellcheck="false"><span class="pe-suffix"></span></div>
      <div class="pe-help pe-user-help"></div>
      <div class="pe-save"></div>
      <div class="pe-err"></div>
    </div></div>`;

    const nameIn = el.querySelector('.pe-name');
    const userIn = el.querySelector('.pe-user');
    const nameHelp = el.querySelector('.pe-name-help');
    const userHelp = el.querySelector('.pe-user-help');
    const suffix = el.querySelector('.pe-suffix');
    const saveBox = el.querySelector('.pe-save');
    const errBox = el.querySelector('.pe-err');
    nameIn.value = startName;
    userIn.value = startUser;

    const userUnchanged = () => userIn.value.trim().toLowerCase() === startUser.toLowerCase();

    function canSave() {
      if (saving) return false;
      if (nameProblem(nameIn.value) != null) return false;
      if (!userUnchanged() && state !== 'bosh') return false;
      return nameIn.value.trim() !== startName.trim() || !userUnchanged();
    }

    function paint() {
      const np = nameProblem(nameIn.value);
      nameHelp.textContent = np ?? `Eng ko'pi ${NAME_MAX} ta belgi. Emoji va istalgan belgi ishlatsangiz bo'ladi.`;
      nameHelp.style.color = np == null ? 'rgba(255,255,255,0.38)' : C.accent;

      let text; let color; let suf = '';
      if (userUnchanged()) {
        text = "Bu — hozirgi username'ingiz.";
        color = 'rgba(255,255,255,0.38)';
      } else {
        switch (state) {
          case 'yozilmoqda':
          case 'tekshirilmoqda':
            text = 'Tekshirilmoqda...'; color = 'rgba(255,255,255,0.45)';
            suf = spinner(16, 2, 'rgba(255,255,255,0.38)');
            break;
          case 'bosh':
            text = "Bu username bo'sh — olsangiz bo'ladi"; color = C.success;
            suf = icon('check_circle', { size: 20, color: C.success });
            break;
          case 'band':
          case 'xato':
            text = problem; color = C.accent;
            suf = icon('error', { fill: false, size: 20, color: C.accent });
            break;
          default:
            text = '3-15 ta belgi. Faqat harf, raqam va _ ishlatiladi.'; color = 'rgba(255,255,255,0.38)';
        }
      }
      userHelp.textContent = text;
      userHelp.style.color = color;
      suffix.innerHTML = suf;

      const on = canSave();
      saveBox.style.opacity = on ? '1' : '0.45';
      saveBox.innerHTML = saving ? spinner(20, 2.2, '#fff') : '<span>Saqlash</span>';
      errBox.textContent = saveError ?? '';
      errBox.style.display = saveError ? '' : 'none';
      nameIn.disabled = saving;
      userIn.disabled = saving;
    }

    async function ask(name) {
      const my = ++run;
      state = 'tekshirilmoqda';
      paint();
      const free = await usernameAvailable(name);
      if (my !== run || userIn.value.trim() !== name) return;
      if (free == null) { state = 'xato'; problem = "Tekshirib bo'lmadi — internetni tekshiring"; }
      else if (free) { state = 'bosh'; problem = ''; }
      else { state = 'band'; problem = 'Bu username band'; }
      paint();
    }

    function onUser() {
      clearTimeout(debounce);
      // Taqiqlangan belgilar umuman yozilmaydi (`FilteringTextInputFormatter`).
      const clean = userIn.value.replace(/[^A-Za-z0-9_]/g, '').slice(0, 15);
      if (clean !== userIn.value) userIn.value = clean;
      const v = userIn.value.trim();
      if (userUnchanged()) { state = 'boshlangich'; problem = ''; paint(); return; }
      if (!v) { state = 'xato'; problem = "Username bo'sh bo'lmasin"; paint(); return; }
      const p = usernameProblem(v);
      if (p) { state = 'xato'; problem = p; paint(); return; }
      state = 'yozilmoqda'; problem = '';
      paint();
      debounce = setTimeout(() => ask(v), 350);
    }

    function onName() {
      // `maxLength` ko'zga ko'ringan belgilar bo'yicha kesadi.
      const g = graphemes(nameIn.value);
      if (g.length > NAME_MAX) nameIn.value = g.slice(0, NAME_MAX).join('');
      paint();
    }

    async function save() {
      if (!canSave()) return;
      saving = true; saveError = null;
      paint();
      try {
        const j = await api('/api/auth/profile', {
          method: 'POST',
          body: { first_name: nameIn.value.trim(), username: userIn.value.trim() },
        });
        const me = currentUser();
        if (me && j?.user) Object.assign(me, j.user);
        try { onSaved?.(); } catch (_) { /* */ }
        back();
        return;
      } catch (e) {
        if (e instanceof ApiError && e.status) {
          const m = `${e.body?.error ?? ''}`;
          saveError = m || "Saqlab bo'lmadi";
        } else {
          saveError = "Tarmoq xatosi — qaytadan urinib ko'ring";
        }
      }
      saving = false;
      paint();
    }

    nameIn.addEventListener('input', onName);
    userIn.addEventListener('input', onUser);
    bindTap(el.querySelector('.pe-back'), () => { if (!saving) back(); });
    bindTap(saveBox, () => save());
    paint();
    return {
      dispose() { clearTimeout(debounce); run++; },
      onBack() { return !saving; },
    };
  }, { transition: 'fade' });
}

