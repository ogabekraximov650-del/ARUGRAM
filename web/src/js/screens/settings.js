// Sozlamalar — `lib/screens/settings_screen.dart`.
//
// Tugma YOQILGAN = statistika YASHIRILGAN (yangi hisobda hammasi
// o'chiq, ya'ni ochiq). Holat `services/user-settings.js` orqali
// (ilovadagi `AuthService.updateSettings`) darhol hisobga yoziladi va
// `sync.js` (`putSettings`) bilan serverga ketadi — yangi yozuv yo'li yo'q.
//
// "Kanallarga avtomatik obuna" ruxsati ham shu paketda
// (`chan_consent`). Ilovadagi `ChannelGate` ning orqa fondagi
// qo'shilishi (Telegram hisobi bilan) bu yerda yo'q — faqat sozlama.

import { currentUser } from '../api.js';
import { esc } from '../format.js';
import { push } from '../router.js';
import { C, icon, appBar, bindAppBar, toast } from '../ui.js';
import { updateSettings, hiddenStats, chanConsent } from '../services/user-settings.js';
import { STAT_KINDS } from './stats.js';

const ROWS = [
  { kind: 'anime', ic: 'movie_filter', hint: "Nechta anime ko'rganingiz" },
  { kind: 'episodes', ic: 'play_circle', fill: false, hint: "Nechta qism ko'rganingiz va ularning ro'yxati" },
  { kind: 'seasons', ic: 'grid_view', hint: "Ko'rgan bo'limlaringiz ro'yxati" },
  { kind: 'favorites', ic: 'bookmark', hint: 'Sevimlilarga saqlaganlaringiz' },
  { kind: 'rated', ic: 'star', hint: "Baho bergan bo'limlaringiz va bergan bahoyingiz" },
  { kind: 'comments', ic: 'mode_comment', hint: 'Yozgan izohlaringiz, javoblari va layklari' },
  { kind: 'watch', ic: 'schedule', hint: 'Jami necha soat tomosha qilganingiz' },
];

function rowHtml(id, ic, title, hint, value, fill = true) {
  return `<div class="glass ss-row">
    <div class="ss-ic">${icon(ic, { fill, size: 18, color: C.accent })}</div>
    <div class="ss-main"><div class="ss-t">${title}</div><div class="ss-h">${hint}</div></div>
    <label class="switch"><input type="checkbox" data-id="${id}"${value ? ' checked' : ''}${currentUser() ? '' : ' disabled'}><span class="tr"></span><span class="th"></span></label>
  </div>`;
}

export function openSettings() {
  push((el) => {
    const u = currentUser();
    const hidden = hiddenStats();
    el.innerHTML = `${appBar({ title: 'Sozlamalar' })}
      <div class="scroll"><div class="ss">
        <div class="ss-sec">MAXFIYLIK</div>
        <div class="ss-note" style="margin-top:6px">Statistikangiz odatda OCHIQ turadi. Yashirmoqchi bo'lganingizning tugmasini yoqing — yoqilgani boshqalarga umuman ko'rinmaydi.</div>
        <div style="height:12px"></div>
        ${ROWS.map((r) => rowHtml(r.kind, r.ic, esc(STAT_KINDS[r.kind].label), r.hint, hidden.includes(r.kind), r.fill !== false)).join('')}
        <div class="ss-note" style="margin-top:2px">Profil rasmi, ismingiz, username va ID raqamingiz har doim ko'rinadi.<br>Balansingiz va sarflagan trafigingiz esa HECH QACHON boshqalarga ko'rinmaydi.</div>
        <div class="ss-sec" style="margin-top:22px">BEPUL KO'RISH</div>
        <div style="height:10px"></div>
        ${rowHtml('__chan', 'campaign', 'Kanallarga avtomatik obuna',
    "Bepul bo'limlar uchun ilova Telegram hisobingiz bilan majburiy kanallarga qo'shiladi (yopiq kanalga so'rov yuboradi). O'chirsangiz, bepul bo'lim ochilganda ruxsat yana so'raladi.",
    chanConsent())}
        <div class="ss-sec" style="margin-top:22px">PLEYER</div>
        <div class="ss-note" style="margin-top:6px">Video formati. fMP4 — sinalgan standart yo'l. MP4 — brauzerning o'z pleyeri (tajriba: ba'zi telefonlarda surish tezroq bo'lishi mumkin, ishlamasa o'zi fMP4 ga qaytadi). O'zgartirgach videoni qayta oching.</div>
        <div style="height:12px"></div>
        <div class="glass ss-row">
          <div class="ss-ic">${icon('play_circle', { fill: false, size: 18, color: C.accent })}</div>
          <div class="ss-main"><div class="ss-t">Video formati</div></div>
          <select class="ss-fmt field" style="width:auto;max-width:55%"><option value="0">fMP4 (standart)</option><option value="1">MP4 (tajriba)</option></select>
        </div>
      </div></div>`;
    bindAppBar(el);

    el.querySelectorAll('.switch input').forEach((inp) => {
      inp.addEventListener('change', () => {
        if (!currentUser()) return;
        const id = inp.dataset.id;
        const on = inp.checked;
        if (id === '__chan') {
          updateSettings({ chanConsent: on });
        } else {
          let list = hiddenStats();
          if (!on) list = list.filter((k) => k !== id);
          else if (!list.includes(id)) list.push(id);
          updateSettings({ hiddenStats: list });
        }
        if (id === '__chan') toast(on ? 'Ruxsat berildi' : "Ruxsat o'chirildi");
        else {
          const label = STAT_KINDS[id].label;
          toast(on ? `${label} yashirildi` : `${label} endi boshqalarga ko'rinadi`);
        }
      });
    });
    const fmt = el.querySelector('.ss-fmt');
    try { fmt.value = localStorage.getItem('aru_mp4') === '1' ? '1' : '0'; } catch (_) { /* */ }
    fmt.addEventListener('change', () => {
      try { localStorage.setItem('aru_mp4', fmt.value); } catch (_) { /* */ }
      toast(fmt.value === '1' ? 'MP4 yoqildi — videoni qayta oching' : 'fMP4 yoqildi — videoni qayta oching');
    });
    return {};
  });
}
