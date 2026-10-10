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
import { C, icon, appBar, bindAppBar, toast, confirmDialog } from '../ui.js';
import { updateSettings, hiddenStats, chanConsent } from '../services/user-settings.js';
import { STAT_KINDS, formatBytes } from './stats.js';
import { cacheStats, cacheClear, cacheLimitSetting, setCacheLimitSetting } from '../tg/chunk-cache.js';

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
        <div class="ss-sec" style="margin-top:22px">KESH (VIDEO)</div>
        <div class="ss-note" style="margin-top:6px">Ko'rilgan video bo'laklari shu qurilmada saqlanadi: qayta ochish va orqaga surish internetsiz, darhol. Joy tugagandagina eng eskisi o'chadi.</div>
        <div style="height:12px"></div>
        <div class="glass ss-row">
          <div class="ss-ic">${icon('storage', { size: 18, color: C.accent })}</div>
          <div class="ss-main"><div class="ss-t">Kesh hajmi</div><div class="ss-h ss-cache-used">Hisoblanmoqda...</div></div>
          <select class="ss-cache-limit field" style="width:auto;max-width:48%"></select>
        </div>
        <div class="glass ss-row ss-cache-clear" style="cursor:pointer">
          <div class="ss-ic">${icon('delete', { fill: false, size: 18, color: '#E5484D' })}</div>
          <div class="ss-main"><div class="ss-t" style="color:#E5484D">Keshni tozalash</div><div class="ss-h">Saqlangan video bo'laklarini o'chiradi</div></div>
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
    // ── Kesh (`tg/chunk-cache.js`) ──
    const usedEl = el.querySelector('.ss-cache-used');
    const sel = el.querySelector('.ss-cache-limit');
    const GB = 1024 * 1024 * 1024;
    const opts = [['auto', "Avtomatik (qurilma sig'ganicha)"], [`${500 * 1024 * 1024}`, '500 MB'], [`${GB}`, '1 GB'], [`${2 * GB}`, '2 GB'], [`${5 * GB}`, '5 GB']];
    sel.innerHTML = opts.map(([v, l]) => `<option value="${v}">${l}</option>`).join('');
    sel.value = opts.some(([v]) => v === cacheLimitSetting()) ? cacheLimitSetting() : 'auto';
    const refresh = async () => {
      try {
        const st = await cacheStats();
        usedEl.textContent = `Saqlangan: ${formatBytes(st.used)}${st.count ? ` (${st.count} ta bo'lak)` : ''}${st.quota ? ` · brauzer ruxsati ${formatBytes(st.quota)}` : ''}`;
      } catch (_) { usedEl.textContent = "Kesh mavjud emas"; }
    };
    refresh();
    sel.addEventListener('change', async () => { await setCacheLimitSetting(sel.value); toast('Saqlandi'); refresh(); });
    el.querySelector('.ss-cache-clear').addEventListener('click', async () => {
      if (!(await confirmDialog('Keshni tozalash', "Saqlangan video bo'laklari o'chiriladi. Videolar keyin qayta yuklanadi.", { ok: "O'chirish", danger: true }))) return;
      await cacheClear(); toast('Kesh tozalandi'); refresh();
    });
    return {};
  });
}
