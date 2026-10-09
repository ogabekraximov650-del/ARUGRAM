# Mini App ekranlarini ko'chirish — umumiy qoidalar (ichki hujjat)

Maqsad: Flutter ilovadagi (`lib/`) ekranlarni `web/src/` dagi Telegram Mini App'ga
"ikki tomchi suvdek" ko'chirish: AYNAN o'sha ranglar, o'lchamlar (Flutter dp = CSS px),
oraliqlar, burchaklar, shrift o'lchamlari/qalinligi, ikonkalar, matnlar (o'zbekcha, harfma-harf),
holatlar (yuklanmoqda / bo'sh / xato), animatsiyalar (iloji boricha).

## Taqiqlar
- Faqat ONLAYN qism. Oflaynga xos narsalar (yuklab olish, yuklanmalar, xotira/kesh tozalash,
  "Offline" belgilari, OfflineLibrary filtrlari) — YO'Q.
- Admin panel ekranlari (`admin_*`, `add_*`, `*_management_*`, `pack_add`, `pack_video_trim`) — YO'Q.
- `git commit` / `git push` QILMANG. Faqat o'zingizga berilgan fayllarni yarating/o'zgartiring.
- Umumiy fayllarni (`api.js`, `ui.js`, `router.js`, `app.js`, `css/app.css`, `nav.js`, `home.js`,
  `seasons.js`, `hooks.js`, `format.js`) O'ZGARTIRMANG. Yetmagan yordamchini o'z faylingizda yozing.
- Turso'ga yangi yozuv yo'li qo'shmang: ilova qaysi API'ni chaqirsa, sayt ham o'shani chaqiradi
  (yozuvlar — `sync.js` orqali, xuddi ilovadagi `SyncQueue` kabi).
- Worker (`worker/`) va Flutter (`lib/`) kodiga TEGMANG.

## Freymvork (`web/src/js/`)
- `api.js`: `api(path, {method, body})` (JSON; X-Tma + Bearer sessiya o'zi qo'shiladi),
  `apiPost(path, body)`, `imageUrl(url)` (worker rasmlari uchun SHART), `currentUser()`, `onUser(fn)`,
  `refreshMe()`, `tgUser()`, `ApiError {status, body}`. Ilovadagi `AuthService.user` = `currentUser()`
  (`/api/auth/me` -> `user`).
- `ui.js`: `icon(name,{fill,size,color})` (Material Symbols Rounded; Flutter `Icons.x_rounded` -> `icon('x')`,
  `Icons.x_outlined`/`_border` -> `{fill:false}`), `spinner`, `bindTap` (GlassTappable), `ripple` (InkWell),
  `toast` (SnackBar), `dialog`, `confirmDialog`, `promptDialog`, `sheet` (bottom sheet), `appBar`+`bindAppBar`,
  `glass`, `seasonCardHtml`+`bindSeasonCards`+`cardWidth` (SeasonCard), `emptyGlass`, `avatarHtml`,
  `openLink`, `haptic`, `C` (AppColors).
- `router.js`: `push((el, route) => ({dispose(){}, onBack(){}}), {transition:'slide'|'fade'})`, `back()`.
  `el` — butun ekran (flex column). Ichida `appBar(...)` + `<div class="scroll">...</div>` qiling.
- `hooks.js`: `hooks.openSeason(season)`, `hooks.openSeasonIds(animeId, seasonId, epizodId?)`,
  `hooks.goTab(i)`, `hooks.openUser(userId)`.
- `seasons.js`: `seasonsRepo.items/genres/years/find(a,s)/listen(fn)/load()/fetch()`.
- `format.js`: `formatCount`, `formatCompact`, `toInt`, `esc` (HTML uchun har doim `esc`!).
- Tab ekrani: `export function createX(pageEl) { ...; return { onShow(){}, onHide(){} } }`.
  Sahifa elementi `.page` — o'zi aylanadi (scroll). Pastda 120px bo'sh joy qoldiring (panel uchun).
- CSS: o'z faylingiz `web/src/css/<nom>.css` (build hammasini qo'shadi). Klass nomlariga prefiks qo'ying.
- Umumiy CSS klasslar: `.glass`, `.glass-lite`, `.btn .btn-filled/.btn-text/.btn-outline`, `.field`,
  `.tile`, `.section-label`, `.switch`, `.icon-btn`, `.appbar`, `.scroll`, `.grid`, `.center-box`, `.spinner`.

## Tekshirish
- `cd web && node build.mjs` (xatosiz bo'lishi shart).
- `node tools/preview.mjs <mocks.mjs> <out-dir>` — Playwright bilan skrinshot (mock'lar fayli
  formati `tools/preview.mjs` boshida). Skrinshotlarni Read bilan ko'rib, Flutter kodi bilan solishtiring.
  Mock va skrinshotlarni scratchpad'ga yozing, repoga emas.

## Yakunda
Qisqa hisobot: qaysi fayllar, qaysi Dart ekranlari to'liq ko'chirildi, nima qolib ketdi va nega,
qaysi API'lar ishlatildi, boshqa modullarga (pleyer va h.k.) kerak bo'lgan eksportlar.
