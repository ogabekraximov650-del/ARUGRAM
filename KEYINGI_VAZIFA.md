# Loyihaning hozirgi holati va keyingi ish uchun eslatmalar

> Bu fayl **hozirgi** arxitekturani tasvirlaydi. Avvalgi versiyasi
> allaqachon bekor qilingan rejani (`fvp`/`mdk` past-darajali API'siga
> o'tish) tasvirlar edi — u yo'ldan **voz kechilgan**, pleyer rasmiy
> `video_player` (Android'da ExoPlayer/Media3) ustida ishlaydi.

## Repo va muhit

- Repo: `ogabekraximov650-del/fulutter` — Flutter ilova + Rust yadrosi
  (`rust/`) + Cloudflare Worker (`worker/`) + B2 (fayl ombori) +
  Turso (baza).
- Foydalanuvchi bilan **faqat o'zbek tilida** gaplashing; texnik
  tushunchalarni oddiy tilda tushuntiring, taxmin qilmang — kodni
  o'qib tasdiqlang.
- **ISH DAVOMIDA XABAR YOZMANG.** Foydalanuvchi talabi (aynan
  shunday): "vazifani to'liq tugatmagunincha menga umuman xabar
  yozma, shunchaki vazifani bajar va oxirida vazifani to'liq
  tugatgach xabar ber — sababi sen har safar xabar yuborganingda
  limit kamayadi".

  Ya'ni: "hozir buni qilyapman", "endi buni boshladim", oraliq
  hisobot — **YO'Q**. Hamma vazifa bajarilib, tekshirilib,
  push qilingandan keyin BITTA to'liq xabar.
- Ishni **alohida branch**da qiling, `main`ga to'g'ridan-to'g'ri push
  qilmang. **Istisno:** foydalanuvchi aniq so'rasa (masalan worker
  o'zgarishi darhol deploy bo'lishi kerak bo'lsa) — `main`ga.
- APK build: `.github/workflows/build-flutter-apk.yml` — `main` va
  `claude/**` branchlariga push qilinganda **avtomatik** ishga tushadi.
  Qo'lda `workflow_dispatch` qilmang.
- Worker deploy: `.github/workflows/deploy-worker.yml` — **faqat
  `main`ga** `worker/**` o'zgarishi push qilinganda. Ya'ni feature
  branchdagi worker o'zgarishlari **deploy bo'lmaydi**.
- Commit qilishdan oldin: `rm -rf rust/target rust/Cargo.lock
  worker/target worker/Cargo.lock`.

## TELEGRAM ORQALI VIDEO (ARUGRAM, 2026-09)

TALAB (foydalanuvchi): xarajatni kamaytirish uchun videolar Telegram
serveri orqali uzatilsin. Ilova nomi **ARUmedia** (TV yo'q).

**Qanday ishlaydi**

1. Admin videoni YOPIQ kanalga yuklaydi (Telegram ilovasidan, 2 GB
   gacha, shifrlanmagan). Kanalda faqat admin va bot (bot — ADMIN).
   Izoh (caption) = B2 dagi fayl nomi (izoh bo'lmasa faylning o'z
   nomi). Bot postni ko'rib `tg_files` ga yozadi va adminga
   "✅ nom → #post" deb xabar beradi.
2. Foydalanuvchi ilovada Profil → "Telegram orqali ko'rish" dan o'z
   Telegram hisobini ulaydi (raqam, kod, parol).
3. Qism ochilganda ilova `/api/tg/deliver` ni chaqiradi: worker
   obunani SERVERDA tekshiradi, bot videoni kanaldan foydalanuvchining
   bot chatiga `copyMessage` qiladi (`protect_content`). Foydalanuvchi
   kanalga a'zo emas.
4. Rust yadrosi (`rust/src/telegram.rs`, kutubxona `grammers`) faylni
   o'sha chatdan oladi va telefonda `127.0.0.1/tg/<xabar>/<fayl>`
   manbasini ochadi. Onlayn ko'rishda baytlar faqat XOTIRADA; yuklab
   olishda hozirgidek 1 MB lab AES-128-GCM bilan shifrlanib yoziladi.
   Worker keshini "isitish" bu holda o'chiq — B2'ga so'rov ketmaydi.
5. Telegram ulanmagan / qism kanalda yo'q / xato — hammasi avtomatik
   odatdagi worker (B2) yo'liga qaytadi.

**Ilovaga kirish ham Telegram raqami bilan** (Profil → raqam → kod →
kerak bo'lsa 2 bosqichli parol, `phone_login_screen.dart`). Telegram
ulangach ilova foydalanuvchi nomidan botga `/start <token>` yuboradi
(`rust_tg_start_bot`) — worker'dagi bot orqali kirish o'zgarmagan,
shaxsni Telegram'ning o'zi tasdiqlaydi. `/api/tg/config` shu sabab
SESSIYASIZ (faqat ilova imzosi bilan). Eski bot oynasi "Bot orqali
kirish" tugmasi ortida qoldi.

**Push:** foydalanuvchi talabi — bundan keyin FAQAT `main` ga.

**Nom va server (2026-09):** ilova nomi **ARUGRAM**, paket
`uz.arugram.soft`, worker nomi `arugram` (eski `arumediatv` ga
TEGILMAYDI). Ilovadagi server manzili `lib/services/api_base.dart`
da — CI uni Cloudflare subdomenidan o'zi hisoblab `--dart-define`
bilan beradi. Bot nomi kodda yo'q: `getMe` dan olinadi.

**Kamroq so'rov + bot chatini tozalash (2026-09):** bot nusxalarini
ILOVA o'zi foydalanuvchi hisobi bilan o'chiradi
(`rust_tg_clear_bot_chat`, `messages.deleteHistory`) — pleyerdan
chiqqanda, yuklab olish tugaganda, internet qaytganda (uzilganda
navbatga qo'yiladi) va har ishga tushishda. Worker'da `tg_sent`,
`/api/tg/release` va cron YO'Q. `/api/tg/deliver` — bazaga bitta
o'qish, yozish yo'q. Video bot chatidan fayl NOMI bo'yicha topiladi
(shaxsiy chat xabar raqamlari har hisobda boshqa). Sozlama
(`/api/tg/config`) telefonda 12 soat keshlanadi.

**Worker orqali FAYL O'TMAYDI (2026-09, foydalanuvchi talabi):**
yuklash ham, ko'rsatish ham ilovaning o'zi orqali (MTProto).
- Yuklash: `TelegramService.uploadFile` — admin to'g'ridan-to'g'ri
  kanalga (+ `/api/tg/admin/file`), boshqalar o'z BOT CHATIGA; bot
  webhook'da (`tg_user_media`) uni kanalga `copyMessage` qiladi va
  ilova `/api/tg/claim` bilan kutadi. Oddiy foydalanuvchi faqat
  `avatar_<id>_...` va `chat_<id>_...` nomlarini yoza oladi.
- Fayllar HUJJAT emas, ODDIY ko'rinishda: rasm — surat, video — oqimli
  video (`media_for`).
- Ko'rsatish: rasm keshi (`_TelegramFileService`) ekrandagi rasmlarni
  BITTA `/api/tg/deliver {files:[..]}` (`copyMessages`) bilan bot
  chatiga oladi, `127.0.0.1/tg/0/<nom>` dan o'qiydi, so'ng chat
  tozalanadi. Worker'dagi `tg_store`/`tg_serve` va Telegram avatar
  proksisi (`/api/avatar`) O'CHIRILGAN.

**Admin videoni ilovadan kanalga yuklaydi** (qism qo'shish ekrani,
admin Telegram hisobini ulagan bo'lsa): `rust_tg_upload_start` —
MTProto, 4 GB gacha, izoh = fayl nomi; so'ng
`POST /api/tg/admin/file`. Qism/sifat o'chirilsa kanal posti ham
o'chadi (`tg_forget_file`). Kanalda "Restrict saving content"
YOQILMASIN — aks holda bot `copyMessage` qila olmaydi.

**Sirlar** (GitHub → `secret` environment, deploy ularni worker'ga
qo'yadi): `TG_API_ID`, `TG_API_HASH` (my.telegram.org),
`TG_CHANNEL_ID` (-100... ko'rinishida). Uchalasi qo'yilmaguncha
Telegram yo'li o'chiq.

**Muhim:** `glass_pumpkin = "=2.0.0-rc0"` Cargo.toml da QOTIRILGAN —
rc1 grammers-crypto'ni buzadi. Cargo.lock repoda yo'q, shu sabab
olib tashlamang.

## SHIFRLAB YUKLASH (AES-128-CTR) VA BAZA TOZALASH (2026-09-25)

**Shifrlash.** Ilova Telegram'ga yuklaydigan HAR BIR fayl (qism,
poster, avatar, yozishma fayli) yuklanish paytida AES-128-CTR bilan
shifrlanadi (`rust/src/telegram.rs` -> `CtrReader`). Har faylga
alohida tasodifiy 16 baytlik kalit, IV nol. Telegram'da fayl HUJJAT
(`application/octet-stream`) bo'lib turadi — Telegram uni ochib
ko'rsata olmaydi. O'qishda `fetch_part` kerakli qismni o'z joyidan
ochadi (`ctr_apply`), ya'ni faqat kerakli baytlar olinadi.
- Kalit serverda: `tg_files.file_key` (hamma fayl) va qismlar uchun
  `epizod_db.key_*`. Admin — `/api/tg/admin/file {key}`, boshqalar —
  `/api/tg/claim?key=` (faqat o'z `avatar_<id>_`/`chat_<id>_` fayliga).
- Kalit RO'YXATDA berilmaydi (`hide_keys`), faqat `/api/tg/deliver`
  javobidagi `keys` da — ruxsat tekshirilgach. Telefonda `keys.bin`
  (shifrlangan).
- Kalitsiz fayl (admin Telegram ilovasidan o'zi qo'ygan post) —
  ochiq, avvalgidek o'qiladi.

**`epizod_db`:** `url_<q>` (fayl nomi), `size_<q>` (BAYT, son),
`key_<q>`. Fayl nomi o'zgarmaydi (ilova yuklashda yasaydi) — bot
chatidagi nusxa ham, kesh ham, kalit ham shu nom bo'yicha topiladi;
Bot API `file_id` foydalanuvchi hisobida ishlamaydi.

**Internet uzilsa pleyer kutadi.** `player_source.rs` tarmoq xatosida
`RETRY` (-2) qaytaradi, `AruDataSource` 1 s kutib qayta so'raydi —
pleyer oxirgi kadrda "buferlanmoqda" bo'lib turadi va internet
qaytgach o'zi davom etadi. Internet uzilganda bot chati "band"lari
bekor QILINMAYDI (`TelegramService._watchConnectivity`).

**Progress chizig'i:** diskdagi bo'laklar oq rangda
(`rust_video_cache_ranges` -> `_DiskRanges`).

**Yozishma:** videolar ham `aru://` (diskka keshlanadi); ovozli xabar
Telegram'dan (`VoicePlayer`); `chat_` fayllari faqat suhbat
ishtirokchilariga (`/api/tg/deliver`), xabarga faqat o'z faylini
biriktirish mumkin (`chat_send`).

**Yuklab olish Telegram'dan TO'G'RIDAN-TO'G'RI (2026-09-25).**
`fetch_span_tg` bo'laklarni `telegram::fetch_range` bilan oladi —
mahalliy `127.0.0.1/tg` HTTP orqali EMAS. Sabab: har bayt telefon
ichida ikki marta aylanardi va telefon tezlik o'lchagichi ilovadagi
raqamdan ~2 barobar ko'p ko'rsatardi; uzilgan HTTP oqimlar oldindan
so'ralgan qismlarni ham behuda tashlardi.

**Yuklash (upload) qismlab, qayta urinish bilan.** `upload_parts` —
512 KiB lik qismlar, 4 ta parallel, har qism 8 martagacha qayta
uriniladi (tarmoq xatosi, FLOOD_WAIT, 60 s javobsiz). Foiz Telegram
QABUL QILGAN baytlar bo'yicha. Post (`SendMedia`) ham xuddi o'sha
`random_id` bilan qayta yuboriladi (RANDOM_ID_DUPLICATE — post
chatdan topiladi). Izohda `key:<hex>` qatori bor — bot postni ko'rib
kalitni o'zi yozadi, ya'ni fayl yuklangach qolgan ishni bot va worker
qiladi; ilova keyingi so'rovni yubora olmasa ham fayl qayta
yuklanmaydi. Foydalanuvchilarga nusxa IZOHSIZ (`remove_caption`).

**Telegram sessiyasi uzilsa (2026-09-25).** Foydalanuvchi Telegram'da
"Qurilmalar"dan ilovani chiqarsa: Rust `check_dead` sessiya o'lganini
Telegram javob bergan HAR BIR joyda sezadi (fayl o'qish, bot chatini
tozalash, yuklash, `with_client`), ilova esa ochilganda, qaytganda va
video oldidan (30 s da bir marta) `rust_tg_check_session`
(`updates.getState`) bilan tekshiradi. Sessiya tashlanadi, `AuthGate`
ochiq sahifalarni yopib raqam oynasini ko'rsatadi. Qayta kirilgach bot
chatida qolgan nusxalar tozalanadi (`_afterLogin`).

**Kirish kodi (Cherrygram `LoginActivity.java` asosida, 2026-09-25).**
Kod qayerga ketgani (`auth.sentCode.type`: ilova/SMS/qo'ng'iroq/email)
ekranda aytiladi, `auth.resendCode` bilan qayta yuboriladi. Har
kirishda (va chiqishda — `auth.loggedOut`) Telegram bergan
`future_auth_token` `tokens.bin` ga saqlanadi va keyingi
`auth.sendCode` da `logout_tokens` ga qo'yiladi — tanilgan qurilma
kodsiz kiradi (`sentCodeSuccess`) yoki darhol parol so'raladi. Parol
SRP'si o'zimizda (`check_password_srp`), chunki grammers'niki tokenni
tashlab yuboradi.

**Kirish kodi kelmasa (2026-09-25).** `arsLan4k1390/Cherrygram`
tekshirildi: `LoginActivity` bizniki bilan bir xil, farqi — rasmiy
(uzoq yillik) `api_id` va haqiqiy qurilma ma'lumoti. Endi:
* `initConnection` — telefon modeli (`aru/signature` -> `device`),
  Android va ilova versiyasi, til `uz` (`rust_tg_set_device`);
  ilgari grammers standarti "Android 32-bit / 0.10.0 / en" edi;
* bir raqamga 2 daqiqada bir martadan ko'p `auth.sendCode` YO'Q —
  ko'p so'rov Telegram cheklovini yoqadi (SEND_CODE_UNAVAILABLE);
* QR orqali kirish (`rust_tg_qr_token`, `auth.exportLoginToken`,
  `updateLoginToken`, `importLoginToken`) — boshqa qurilmadagi
  Telegram: Sozlamalar → Qurilmalar → "Qurilmani ulash".

**Bot chati TOZALANMAYDI (2026-09-25, foydalanuvchi talabi).**
Fayllar shifrlangan, shu sabab nusxalar chatda qoladi. Kerakli fayl
avval bot chatidan NOMI bo'yicha izlanadi (`rust_tg_find`: oxirgi
200 xabar, so'ng `messages.search`); topilsa kanaldan qayta nusxa
olinmaydi. Kaliti telefonda yo'q bo'lsa — `/api/tg/deliver
{"keys_only": true}` (nusxasiz, bazaga yozuvsiz). Bot yangi nusxa
yuborgach chat foydalanuvchi hisobi bilan O'QILGAN deb belgilanadi
(`rust_tg_mark_read`, `messages.readHistory`) — Telegram'da "N ta
o'qilmagan" chiqmaydi. Worker'da cron ham, nusxalar jadvali ham YO'Q.

**Baza tozalash:** `ci/WIPE_DB_ONCE` + `ci/wipe_db.py` —
`deploy-worker.yml` yangi worker'dan KEYIN bazani tozalaydi va
worker'ni qayta deploy qiladi. Belgi bazaga yoziladi
(`app_config.wipe_done`) — bir belgi bilan faqat BIR MARTA. Yana
tozalash kerak bo'lsa — faylning ichidagi belgini o'zgartiring.
Sxemada `ALTER` yamoqlari va eski bir martalik tozalashlar YO'Q;
ishlatilmaydigan ustunlar olib tashlangan (`ratings_db.created_at`,
`subs_db.updated_at`, `comment_likes.created_at`,
`payments_db.paid_at`, `orphan_files.noted_at`,
`comments_db.edited_at`, `chat_messages.media_thumb`,
`tg_files.file_id`, `users_db.banned_at`, `epizod_db.yosh`).

## BAZA TOZALANDI VA SXEMA IXCHAMLASHTIRILDI (2026-09)

B2 ombori va Turso bazasi **ikkinchi marta butunlay bo'shatildi**
(bir martalik GitHub Action bilan; fayl ishlatilgach repodan
o'chirildi). Shu sabab `init_db` jadvallarni yakuniy ko'rinishida
yaratadi va `ALTER TABLE` yamoqlari YO'Q.

**`migrate_db` va `reset_stats_once` OLIB TASHLANDI.** Ular eski
sxemani yamoqlash uchun edi; baza toza bo'lgach ikkalasi ham
faqat ortiqcha kod va har yangi izolyatda ortiqcha so'rov edi.

**QOIDA (o'zgarmadi):** yana tozalash kerak bo'lsa — avval YANGI
workerni deploy qiling, KEYIN tozalang. Aks holda tozalash
paytida kelgan bitta so'rov eski sxemani qaytarib yozib qo'yadi.

### ORTIQCHA USTUNLAR OLIB TASHLANDI

Sabab: baza bekorga shishmasin. Qaysi ustun nega ketdi:

| Jadval | Olib tashlandi | Nega |
|---|---|---|
| `users_db` | `language_code`, `is_premium` | yozilardi, hech qayerda o'qilmasdi |
| `sessions_db` | `telegram_id`, `username`, `first_name`, `api_base` | `users_db` dagining nusxasi; ism o'zgarsa eskirib qolardi |
| `login_tokens` | `api_base` | ishlatilmasdi |
| `anime_db` | `created_at` | anime qachon qo'shilgani hech qayerda ko'rsatilmaydi |
| `watch_history_db` | `created_at` | yozilardi, o'qilmasdi |

**`watch_history_db.video_url` endi YALANG FAYL NOMI.** Bu jadval
eng tez o'sadi (qatorlar = foydalanuvchilar × ko'rilgan qismlar),
shu sabab har qatorda to'liq manzil (~90 belgi) o'rniga faqat
fayl nomi (~35 belgi) saqlanadi. Manzil ilovaga berishdan oldin
`resolve_list` bilan yig'iladi — `epizod_db.url_*` bilan bir xil
qoida. Yon foyda: domen o'zgarsa eski yozuvlar ishlayveradi.

### KALIT `epizod_id`, RAQAM EMAS (2026-09, TOPILGAN XATO)

TALAB (foydalanuvchi): "watch history jurnaliga epizod raqami
emas idsi yozilsin — sababi qism raqamini o'zgartirganda tomosha
tarixidagi kadrlar qotib qoldi, ya'ni ishlamadi".

Sabab aniq edi: `watch_history_db` ning birlamchi kaliti
`(user_id, anime_id, season_id, epizod_number)` edi. Admin
"5-qism"ni "6-qism" qilib qo'ysa, tarixdagi yozuv HECH QAYSI
qismga tegmay qolardi — kadr ham, "davom ettirish" ham ishlamasdi.

Endi kalit `epizod_id` (qism qo'shilganda bir marta beriladi va
hech qachon o'zgarmaydi).

**`epizod_number` USTUNI YO'Q** (foydalanuvchi talabi:
"jurnaldan epizod number'ni olib tashla, epizod id yetadi").
Raqam baribir `epizod_db` dan `LEFT JOIN` bilan olinardi — bu
ustun faqat o'qilmagan nusxa edi. Jadval eng tez o'sadigani,
har qatordan bitta ustun tejash arziydi.

Diskdagi NUSXADA raqam saqlanaveradi: oflaynda "N-qism" deb
yozish uchun boshqa manba yo'q.

ESLATMA: `CREATE TABLE IF NOT EXISTS` mavjud jadvalni
o'zgartirmaydi, ya'ni bazada ustunning O'ZI keyingi tozalashgacha
qolib turadi (unga endi hech narsa yozilmaydi va o'qilmaydi).
`ALTER TABLE` yamog'i ATAYLAB qo'shilmadi — bu yerdagi qoida.

Qo'shilgan ustun: **`last_quality`** — foydalanuvchi shu qismni
oxirgi marta qaysi sifatda ko'rgani ("720p"). Pleyer qismni
ochishda shu sifatni tiklaydi (`_restoreQuality`), ya'ni keyingi
safar internet yoqilganda video AYNAN o'sha sifatdan davom etadi.
Foydalanuvchi sifatni qo'lda tanlasa — uning tanlovi ustun
(`_qualityChosenByUser`).

Ilovadagi eski (diskda qolgan) yozuvlarda `epizod_id` yo'q. Ular
o'qishda TASHLAB YUBORILADI (`WatchHistory._fromRows`) — aks
holda hammasi bitta kalitga (0) tushib, bir-birining ustiga
yozilardi.

### JANRLAR: BITTA BO'LIM — BITTA QATOR (2026-09)

TALAB (foydalanuvchi): "hozir tursoda bitta bo'lim uchun 4 yoki
5 ta qator yozilyabdi, bu esa harajatni oshiradi".

To'g'ri edi: `season_janr` BOG'LOVCHI jadval edi va har bir janr
uchun alohida qator + alohida `INSERT` ketardi.

Endi u `PRIMARY KEY (anime_id, season_id)` va **`janr_1 ...
janr_10`** ustunlaridan iborat: bitta bo'lim = bitta qator,
bitta `INSERT ... ON CONFLICT` so'rovi. Bo'sh ustun = janr yo'q.
Janr tanlanganda birinchi bo'sh ustunga tushadi (`save_janrs`
qatorni qaytadan yozadi, ya'ni bu o'z-o'zidan hal bo'ladi).

Nega 10 ta: ro'yxatda jami 37 janr bor, bitta bo'limga odatda
3-6 tasi qo'yiladi; bo'sh TEXT ustun SQLite'da bir baytdan
oshmaydi.

**`idx_janr` indeksi OLIB TASHLANDI** — janr 10 ta ustunning
istalganida bo'lishi mumkin va bitta indeks ularni qamrab
ololmaydi. Janr bo'yicha filtr bo'limlar ro'yxatini to'liq ko'rib
chiqadi, lekin bo'limlar soni KICHIK (kontent, foydalanuvchi
emas) — ya'ni bu arzon, yutuq esa har bo'limga bitta yozuv.

### `epizod_db`: `intro_1 ... intro_10`

Openingni o'tkazib yuborish oraliqlari — 5 ta JUFTLIK
(`intro_1`/`intro_2` — 1-oraliq boshi va oxiri, ... `intro_9`/
`intro_10` — 5-oraliq). Qiymat SONIYADA (matn emas): pleyer har
kadrda solishtiradi. 0 — oraliq belgilanmagan.

## VAQT MINTAQASI — UTC+5 (TOSHKENT)

Hamma vaqt Unix millisekundda (UTC) saqlanadi. Statistika
"chelaklari" esa Toshkent vaqti bo'yicha belgilanadi:
`day_key(ms)` -> `2026-09-11`, `hour_key(ms)` -> `2026-09-11T14`.
**Bu funksiyalarni o'zgartirmang** — eski qatorlar boshqa
mintaqada yozilgan bo'lsa, hisob siljib ketadi.

## INDEKSLAR: KAM, LEKIN ANIQ

Har bir indeks YOZISHNI sekinlashtiradi. Qoida: **birlamchi
kalitning BOSHIDAGI ustunlar bo'yicha qidiruvga qo'shimcha indeks
kerak emas.**

| Indeks | Qaysi so'rov uchun |
|---|---|
| `season_janr(janr)` | janr bo'yicha filtr |
| `users_db(LOWER(username))` unique | username takrorlanmasin |
| `users_db(created_at)` | "shu davrda nechta hisob ochilgan" |
| `login_tokens(expires_at)` | eskirgan tokenlarni tozalash |
| `sessions_db(user_id, last_seen_at)` | qurilmalar ro'yxati, 4 ta chegara |
| `sessions_db(last_seen_at)` | **kunlik faol foydalanuvchi** |
| `watch_history_db(user_id, deleted_at, updated_at DESC)` | tarix ro'yxati |

`anime_db`, `epizod_db`, `ratings_db`, `favorites_db`,
`stats_*` — faqat birlamchi kalit. Qo'shimcha indeks qo'shishdan
oldin uni AYNAN qaysi so'rov ishlatishini yozib qo'ying.

`session_user` ham tejaldi: ikkita so'rov o'rniga bitta "quvur",
va `last_seen_at` faqat **60 soniyada bir marta** yoziladi.

## SHAFFOF STATISTIKA

`GET /api/stats` — hammasi bitta so'rovda, chekkada **5 daqiqa**
keshlanadi (`cache_seconds`), ilovada yana 10 daqiqa
(`StatsService`). Raqamlar diskka ham yoziladi — oflaynda oxirgi
ma'lum holat ko'rinadi.

| Ko'rsatkich | Kunlik | Hafta / oy | Umumiy |
|---|---|---|---|
| Foydalanuvchilar | oxirgi 24 soatda onlayn (`sessions_db.last_seen_at`) | `users_db.created_at` | hamma hisob |
| Ko'rishlar | oxirgi 24 soat | kunlik chelaklar | jami |
| Trafik | oxirgi 24 soat | kunlik chelaklar | jami |
| Tomosha vaqti | oxirgi 24 soat | kunlik chelaklar | jami |

Trafik chelaklarini **ILOVA** to'ldiradi (sinxronlash paketi
ichida). Cloudflare Analytics manbasi sinab ko'rilgan va OLIB
TASHLANGAN — sabab "YAGONA YOZUV YO'LI" bo'limida.

**YILLIK ko'rsatkich ATAYLAB YO'Q** (foydalanuvchi talabi):
kunlik, haftalik, oylik va umumiy yetarli.

**Hodisalar ro'yxati saqlanmaydi** (u millionlab qator bo'lardi) —
faqat yig'indilar: `stats_hourly` (oxirgi 24 soat uchun, 3 kundan
eskisi o'chiriladi) va `stats_daily` (hafta/oy/jami).

Chelaklarga yozish **paketga bir marta** bo'ladi (ilgari har bir
qism uchun 4 ta qator) — "YAGONA YOZUV YO'LI" bo'limiga qarang.

**TRAFIKNI ILOVA SANAYDI (worker EMAS).**

Ikki marta tuzatildi, ikkinchisi yakuniy:

1. *Birinchi urinish (endi yo'q).* Javobning E'LON QILINGAN
   uzunligi (`Content-Length`) sanalardi — 166 MB lik video
   "1,14 GB" bo'lib ko'rinardi, chunki pleyer `Range: bytes=0-`
   deb butun faylni so'rab, bir necha megabaytdan keyin
   ulanishni uzadi.
2. *Ikkinchi urinish (endi yo'q).* Javob tanasi sanovchi
   `TransformStream` quvuridan o'tkazilardi. Hisob to'g'rilandi,
   lekin IJRO BUZILDI — pleyerda "yuklanmadi" xatosi chiqa
   boshladi.

**Hozirgi qoida: worker javobga UMUMAN tegmaydi.** `b2_play` va
`b2_proxy` javoblari qanday bo'lsa shundayligicha uzatiladi.
Hech qanday `X-U` sarlavhasi ham yo'q (yadrodan ham, pleyerdan
ham olib tashlangan) — manzil ham, kesh kalitlari ham toza.

### UCHINCHI URINISH — YADRO HISOBLAGICHI HAM OLIB TASHLANDI

Bir muddat hisob Android yadrosidan olindi
(`TrafficStats.getUidRxBytes`). U ham NOTO'G'RI chiqdi.

TOPILGAN XATO (foydalanuvchi: "ilova trafikni xato hisoblayapti —
ilova ICHIDA aylanayotgan trafikni ham hisoblayapti").

`getUidRxBytes` ilovaning UID'i ostidagi HAMMA soketni sanaydi,
shu jumladan MAHALLIY (`127.0.0.1`) uzatmani ham. Pleyer esa
videoni tarmoqdan emas, ilovaning O'Z kesh-serveridan oladi.
Natija:

* bitta video IKKI MARTA sanalardi — bir marta Rust yadrosi uni
  workerdan tortib olganda, ikkinchi marta o'sha baytlar pleyerga
  mahalliy uzatilganda;
* ALLAQACHON yuklab olingan videoni oflayn qayta ko'rganda ham
  trafik o'sardi — hech qanday bayt tarmoqdan kelmagan bo'lsa ham.

`aru/net` kanali OLIB TASHLANDI (`MainActivity.kt` da endi uning
o'rnida `aru/storage` — telefon xotirasi uchun).

### HOZIRGI QOIDA: FAQAT TARMOQQA CHIQADIGAN IKKI JOY

TALAB (foydalanuvchi): "ilova faqatgina internet yoniq vaqtda
worker orqali kelgan baytlarni hisoblashi kerak, ilova
ichidagilarni emas".

| Manba | Nima sanaladi |
|---|---|
| `rust_video_cache_net_bytes` | Rust yadrosi workerdan HAQIQATAN tortib olgan video baytlari |
| `NetMeter` (`lib/services/net_meter.dart`) | http klienti qabul qilgan bayt: API javoblari, posterlar, avatarlar |

Diskdan o'qish, `127.0.0.1`, keshdan olingan rasm — UMUMAN
sanalmaydi, chunki ular bu ikki joydan o'tmaydi.

**HAMMA SO'ROV QANDAY QILIB SANOVCHI KLIENTDAN O'TADI.** Ilovada
14 ta faylda `http.get(...)` bor. Ularni birma-bir o'zgartirish
qarz bo'lardi (yangi so'rov yozilganda hisobga qo'shishni unutish
oson), shu sabab `main()` butun ilovani `http.runWithClient`
zonasida ishga tushiradi — o'shanda `http.get`, `http.post` va
umuman `Client()` chaqiruvlarining HAMMASI `CountingClient` ni
oladi. `cached_network_image` ham oddiy `http.Client()` yaratadi,
ya'ni posterlar ham shu hisobga tushadi.

**MUHIM:** `CountingClient` ning ICHKI klienti `Zone.root.run`
bilan yaratiladi. Aks holda u o'zini o'zi chaqirib, cheksiz
rekursiyaga tushadi.

Qolgani o'zgarmadi:

* `lib/services/traffic_service.dart` — har 30 soniyada (va
  ilova fon'ga o'tganda) o'lchov oladi, FARQNI yig'indiga
  qo'shadi va diskka yozadi (`list_traffic.rustbin`, shifrlangan).
  Yozuvda hisob raqami ham bor: hisob almashsa eski yig'indi
  tashlab yuboriladi.
* Yig'indi **sinxronlash paketining ichida** ketadi
  (`traffic_bytes`) — alohida so'rov YO'Q. Worker uni HAM umumiy
  chelaklarga, HAM `users_db.traffic_bytes` ga qo'shadi
  (`note_traffic`). Javob 200 bo'lsa
  ilova yuborilgan miqdorni ayiradi va qaytadan sanay boshlaydi.
* Ilova qayta ishga tushganda ikkala hisoblagich ham nolga
  tushadi — bu aniqlanadi (yangi qiymat eskisidan kichik) va
  o'sha qiymatning o'zi farq sifatida olinadi.

**Profil sahifasidagi "Trafik" = Turso'dagi raqam + ilovada
hozircha yuborilmagan yig'indi.** Talab: hisobot sutkada bir
marta ketadi, lekin ko'rsatkich kutib turmasligi kerak. Shu
sabab ekranda ikkovining yig'indisi ko'rinadi:

* raqam TIRIK — video ko'rilgan sayin o'sadi;
* hisobot o'tgan zahoti yig'indi bazaga ko'chadi va son
  SAKRAMAYDI (bazadagisi o'sadi, mahalliysi shuncha kamayadi);
* internet bo'lmasa ham diskdagi oxirgi raqam + mahalliy
  yig'indi ko'rinadi.

**Ko'rish — ODAM BOSHIGA BITTA.** Foydalanuvchi talabi: "bitta
odam bitta videoni 50 marta ko'rsa ham ko'rishlar soni 1 tadan
oshmasin". Shu sabab umumiy hisob faqat shu odam shu qismni
BIRINCHI marta ko'rganda oshadi (`view_count = 0` bo'lganda);
shaxsiy `view_count` esa o'sib boraveradi.

## ILOVA HAJMI: VAQTINCHALIK NUSXALAR

TOPILGAN XATO (foydalanuvchi: "ilova hajmi juda tez ko'tarilib
ketyapti, xuddi keraksiz fayllarni yuklab olayotgandek").

Sabab yuklab olingan videolar emas, **fayl TANLASH** edi:
`image_picker` galereyadan tanlangan faylni ilovaning
vaqtinchalik papkasiga (`getTemporaryDirectory`, Android'da
`cacheDir`) NUSXALAYDI. Admin panelidan 300 MB lik qism
yuklansa, telefonda yana 300 MB paydo bo'lardi — va hech qachon
o'chmasdi. Uch sifat bilan bitta qism ~1 GB joy egallardi.

Yechim `lib/services/storage_janitor.dart` da, ikki qatlam:

1. `dropPicked(path)` — yuklash tugashi bilan nusxa o'chiriladi.
   Video uchun qanday tugashidan qat'i nazar (`finally`), rasm
   uchun esa FAQAT muvaffaqiyatda — xato bo'lsa foydalanuvchi
   qayta urinib ko'ra olsin.
2. `sweep()` — ilova ochilganda 30 daqiqadan eski qoldiqlar
   tozalanadi (tizim ilovani yuklash o'rtasida yopib qo'ygan
   bo'lsa). `libCachedImageData` (posterlar keshi) tegilmaydi.

Faqat ilovaning O'Z papkasidagi nusxa o'chiriladi —
galereyadagi asl faylga hech qachon tegilmaydi.

## HAR BIR HISOBGA — O'Z PAPKASI

TALAB (foydalanuvchi): "chiqish yoki hisobni o'chirishda endi hech
narsa tozalanmasin; boshqa accountga o'tganda ilova ichida
`accountid_1` deb oxiriga user id qo'yib papka ochilsin, yangi
accountga o'tsa `accountid_5` — ya'ni account ma'lumotlari
chalkashib ketmasligi uchun. Lekin rasm va video fayllar bitta
joydan olinishi kerak ikkala accountda ham."

Hisobga TEGISHLI hamma narsa `<hujjatlar>/accountid_<id>` ichida:

* tomosha tarixi va tarix kadrlari (`list_*`, `thumb_*`);
* qayerda to'xtagani (`WatchProgress`);
* sevimlilar, shaxsiy statistika, trafik hisobi.

Kirilmagan bo'lsa — `accountid_0` (mehmon).

**UMUMIY bo'lib qoladigan narsalar** (ikkala hisob ham bitta
joydan oladi, bir xil fayl ikki marta yuklab olinmaydi):

* yuklab olingan videolar — `video_byte_cache` (Rust yadrosi);
* posterlar — vaqtinchalik papkadagi `libCachedImageData`;
* anime ro'yxati — `anime_cache.rustbin`.

**HECH NARSA O'CHIRILMAYDI.** `AccountData.switchTo(id)` faqat
uch ish qiladi: eski papkaga yozilmagan narsalarni yozadi,
papkani almashtiradi, xotiradagi ro'yxatlarni yangi papkadan
qaytadan o'qiydi. Eski hisobga qaytilsa hammasi o'z holicha
ochiladi.

**ESKI VERSIYADAN KO'CHISH:** ilgari fayllar hujjatlar papkasining
o'zida yotardi. Hisob birinchi marta o'z papkasini olganda ular
avtomatik ko'chiriladi (`_adoptLegacyFiles`) — tarix yo'qolmaydi.

## PLEYER OYNALARINI SURISH — SILLIQLIK

Uchta oyna (`Ma'lumot | Qismlar | Bo'limlar`) `PageView` bilan
qo'lda suriladi. Kuchsiz telefonda qotishning sabablari va
yechimlari:

* har bir oyna `_KeepAlivePage` ichida — bir marta qurilgach
  tirik qoladi (`AutomaticKeepAliveClientMixin`);
* oyna ichi `RepaintBoundary` da — surish paytida mazmun
  qaytadan CHIZILMAYDI, tayyor qatlam ko'chiriladi;
* pleyerning o'zi ham, uning ostidagi "hozir nima ko'rilyapti"
  qatori ham alohida `RepaintBoundary` da — pleyerning o'z
  yangilanishi pastdagi ro'yxatni sudrab ketmaydi;
* barmoq ekranda turganda davriy ishlar to'xtaydi
  (`_gestureBusy`): yuklab olish holati so'ralmaydi, oyna
  tekshiruvi va sog'liq kuzatuvchisi o'tkazib yuboriladi.
  Qulf 5 soniyadan keyin o'zi ochiladi — "surish tugadi" xabari
  kelmay qolsa ham tizim to'xtab qolmaydi;
* `allowImplicitScrolling: true` — qo'shni oyna oldindan
  quriladi.

## TOMOSHA VAQTI

TALAB: "1x tezlikda ko'rganda hisoblansin" va "epizod vaqtidan
oshmasin".

- ilova videoning O'Z nuqtasi bo'yicha o'lchaydi (pauza, buferlash
  va sek qo'shilmaydi), faqat ijro ketayotganda va tezlik 1x
  bo'lganda (`video_player_screen` -> `_watchTickPos`);
- yig'indi qism uzunligidan oshmaydi (`WatchHistory.addWatched`
  va serverda yana bir marta cheklanadi);
- serverga JAMI vaqt yuboriladi, server esa faqat **farqni**
  qo'shadi — takroriy yuborish raqamni shishirmaydi;
- ko'rinishi: `1:59` (faqat soat:daqiqa), kattasi `1.284:05`.

## BAHO (IMDb USULI) VA SEVIMLILAR

Ikkovi ham **BO'LIM** darajasida (`ratings_db`, `favorites_db`).

Reyting — **oddiy o'rtacha** (`sum / count`, ikki kasr xona).
Foydalanuvchi talabi: "birinchi odam 10 baho bersa reyting ham
10 bo'lsin, iloji boricha ANIQ bo'lsin". IMDb uslubidagi vaznli
(bayes) o'rtacha sinab ko'rilgan edi — u bitta baho bo'lganda
10 ni 8.5 ga tushirardi va shu sabab OLIB TASHLANDI.

Baho qo'yish oynasida yulduz bosilganda faqat TANLANADI;
saqlash uchun "Baholash" tugmasi bosiladi ("Bekor qilish" ham
bor).

**Baho va sevimli serverga DARHOL bormaydi.** Yangi holat
telefonda hisoblanadi (server bilan BIR XIL qoida bo'yicha),
diskka yoziladi va `SyncQueue` navbatiga tushadi — oflaynda ham
ishlaydi va "saqlanmadi" degan xato chiqmaydi. Batafsil —
"YAGONA YOZUV YO'LI" bo'limi.

Tezlik uchun `season_db` da hisoblangan ustunlar turadi:
`views_total`, `watch_ms_total`, `fav_count`, `rating_sum`,
`rating_count`, `epizod_count`. Ya'ni Ma'lumot oynasi uchun
BITTA qator o'qiladi (`GET /api/season/:a/:s`), `COUNT(*)`
hech qachon ishlatilmaydi.

## BO'LIM QO'SHISH VA JANRLAR

**TOPILGAN XATO (500):** `season_id` birlamchi kalitning bir
qismi edi va QO'LDA kiritilardi — band raqam kiritilsa SQLite
"UNIQUE constraint failed" berardi va so'rov 500 bo'lib
yiqilardi.

Endi `season_id` ni **server beradi** (`MAX+1`), formada faqat
"N-bo'lim" raqami so'raladi, band raqam esa tushunarli xabar
bilan qaytariladi ("2-bo'lim allaqachon mavjud").

Janrlar `lib/data/janrlar.dart` da (37 ta, alifbo tartibida) va
bo'lim qo'shish oynasida **tugma** ko'rinishida. Bazada
`season_janr` jadvalining `janr_1 ... janr_10` ustunlarida
saqlanadi (bitta bo'lim — bitta qator, yuqoridagi "JANRLAR"
bo'limiga qarang); `season_db.janri` esa faqat ko'rsatish uchun
matn nusxasi.

## OPENINGNI O'TKAZIB YUBORISH (2026-09)

TALAB (foydalanuvchi): qism qo'shishda `5:14  6:44` deb yozilsa,
video 5:14 ga kelganda "O'tkazib yuborish" tugmasi chiqsin;
bosilsa video 6:44 ga sakrasin.

| Qayerda | Nima |
|---|---|
| Baza | `epizod_db.intro_1 ... intro_10` — 5 juftlik, MATN (`"5:14"`) |
| Qoida | `lib/services/intro_times.dart` — matn <-> millisekund |
| Admin | `add_epizod_screen.dart` — video yuklash oynalari tagida 2 ustun × 5 qator |
| Pleyer | `video_player_screen.dart` -> `_updateIntro` / `_skipIntro` |

### VAQT MATN BO'LIB SAQLANADI (soniya EMAS)

TALAB (foydalanuvchi): "intro vaqtini 5:14 va 6:44 qilib
yoziladigan qil, soniya bilan emas".

Ilgari bazada SONIYA turardi va admin oynasi uni ikki marta
o'girardi (yozishda matn -> soniya, ochishda soniya -> matn).
Bitta ortiqcha qatlam, bitta ortiqcha xato manbai.

Endi bazada ham, ekranda ham AYNAN bir xil matn. Pleyer uni qism
ochilganda BIR MARTA millisekundga o'giradi (`introRangesOf`) va
keyin tayyor songa qaraydi — ya'ni tezlikka ta'sir qilmaydi.

O'girish qoidasi IKKI joyda kerak (admin oynasi va pleyer), shu
sabab u alohida faylda: `lib/services/intro_times.dart`
(`test/intro_times_test.dart` bilan qo'riqlanadi). Nusxa
ko'chirmang — ikkovi bir kun ajralib qoladi.

Tushuniladigan yozuvlar: `5:14`, `05:14`, `1:02:03`, `314`
(yalang soniya — eski yozuvlar). Xato yozuv oraliqni shunchaki
"belgilanmagan" qiladi, PLEYERNI YIQITMAYDI.

### TOPILGAN XATO: INTRO CHIQMASDI

Foydalanuvchi: "pleyerda intro chiqmayapti".

Sabab intro kodida emas edi. Pleyer qismlar ro'yxatini AVVAL
diskdagi keshdan o'qiydi va darhol qism ochadi; keyin serverdan
yangi ro'yxat keladi va `_episodes` almashtiriladi — LEKIN
`_currentEp` ESKI (kesh) obyekt bo'lib qolardi.

Ya'ni admin qismga endi qo'shgan intro vaqtlari ochiq qismda
ko'rinmasdi. Xuddi shu narsa yangi qo'shilgan sifat yoki
o'zgargan nom uchun ham amal qilardi.

Yechim: `_adoptFreshEpisode()` — yangi ro'yxat kelganda ochiq
qism AYNAN o'sha qismning yangi qatori bilan almashtiriladi
(`epizod_id` bo'yicha) va intro oraliqlari qaytadan o'qiladi.

### VAQTNI YOZIB BO'LMASDI (TOPILGAN XATO)

Foydalanuvchi: "intro vaqtini yozib bo'lmayapti, boshqacha
keyboard chiqishi kerak edi".

Sabab: maydonda `TextInputType.phone` turardi. Telefon
klaviaturasida `-`, `+`, `*#`, `.` bor, LEKIN **ikki nuqta
(`:`) YO'Q** — ya'ni `5:14` deb yozishning imkoni yo'q edi.

Endi `TextInputType.datetime` (aynan vaqt uchun: raqamlar bilan
birga `:` chiqadi). Ustiga ikki qavat himoya:

* `FilteringTextInputFormatter` faqat raqam va `:` ni o'tkazadi;
* saqlashda `normalizeIntroInput` ishga tushadi — `514` ham
  `5:14` bo'lib saqlanadi (oxirgi ikki raqam soniya), `44` ->
  `0:44`, `7` -> `0:07`.

### TUGMANING KO'RINISHI, NOMI VA JOYI

* **nomi** — "O'tkazish" (foydalanuvchi aniq shunday so'ragan);
* **joyi** — videoning CHAP YUQORI burchagi. Fullscreen'da
  kontrollar ochiq bo'lsa yuqori qatorda "orqaga" tugmasi
  turadi, shu sabab intro tugmasi o'sha qatorning TAGIGA
  tushadi (`top: 54`) — aks holda ular ustma-ust kelardi;
* **ko'rinishi** pastki paneldagi `HQ` tugmasidan AYNAN
  ko'chirilgan: fon oq 15%, chekkasi `white30`, burchagi 7.
  **Ikkovini birga o'zgartiring** — aks holda ular ajralib
  qoladi.

Ko'rinish qoidasi:

* tugma intro oralig'i **TUGAGUNCHA** turadi (foydalanuvchi
  talabi). Ilgari 5 soniyalik taymer bor edi va tugma o'zi
  yashirinardi — o'sha payt ekranga qaralmasa o'tkazib yuborish
  imkoni yo'qolardi. Endi u faqat oraliq tugaganda yoki bosilgach
  yo'qoladi.

Tugma Stack'ning ENG USTIDA, sek gesture qatlamidan KEYIN
turadi — aks holda unga bosilgan tap sek qatlamiga tushib,
video oldinga sakrab ketardi.

### AVTOMATIK O'TKAZISH (uch nuqta menyusi)

TALAB (foydalanuvchi): "o'ng yuqori qismiga 3ta nuqta qo'y,
ustiga bossa `introni avtomatik o'tkazish` degan yoqib
o'chiradigan tugma bo'lsin: yoqib qo'ysa intro avtomatik
o'tkazib yuboriladi, agar o'chiq bo'lsa qo'lda o'tkazishi kerak".

* uch nuqta — videoning O'NG YUQORI burchagida, faqat kontrollar
  ochiq bo'lganda (yoki menyu ochiq turganda) ko'rinadi.
  Fullscreen sarlavhasi uning tagiga kirmasin deb yuqori qatorda
  44 px joy qoldirilgan;
* bosilganda uch nuqta ostida KICHIK OYNA ochiladi: "Avto
  o'tkazish" yozuvi va yoqib-o'chiradigan tugma (`toggle_on` /
  `toggle_off`). Ko'rinishi `HQ` tugmasidan olingan — oq 15% fon,
  `white30` chekka, burchak 7 (foydalanuvchi talabi);
* **`PopupMenuButton` ISHLATILMAYDI.** Foydalanuvchi talabi:
  "avto o'tkazishni bosganda oyna yopilib ketmasin, faqat oyna
  tashqarisiga yoki 3ta nuqtaga bossa yo'qolsin" — `PopupMenuButton`
  esa tanlangan zahoti o'zini yopadi va buni o'zgartirib bo'lmaydi.
  O'rniga Stack'da uchta qatlam: PARDA (butun ekran, bosilsa
  yopadi), OYNA, va ularning USTIDA uch nuqta (unga bosilsa
  parda tutib qolmasdan menyu yopiladi);
* menyu ochiq turganda kontrollar YASHIRINMAYDI (`_hideTimer`
  bekor qilinadi) — aks holda uch nuqta daraxtdan olib tashlanib,
  oyna "muallaq" qolardi;
* yoqilgan bo'lsa `_updateIntro` tugma ko'rsatish o'rniga
  darhol `_skipIntro()` chaqiradi. Tugma yoqilgan damda video
  intro ichida bo'lsa — o'sha zahoti o'tkaziladi.

Holat `lib/services/app_settings.dart` da: diskda, SHIFRLANGAN
va HISOB PAPKASIDA (`list_settings.rustbin`). Ya'ni bitta
telefonda ikki kishi kirsa har birining o'z sozlamasi bo'ladi.
`shared_preferences` ATAYLAB qo'shilmadi — bittagina bayroq
uchun yangi bog'liqlik va shifrlanmagan fayl ortiqcha.

## PLEYER: PARDA, OXIR VA HALQA

### BILDIRISHNOMA PARDASI PAUZA QILMAYDI

TOPILGAN XATO (foydalanuvchi: "telefonning yuqoridagi internet va
boshqa narsalarni yoqib o'chiradigan oynasini tushirsa video
pauza bo'lyapti; agar ko'tarsa yana play bo'lib ketsin").

Sabab: `didChangeAppLifecycleState` da `inactive` ham `paused`
bilan bir qatorda turardi. Android pardani tushirganda `inactive`
yuboradi — ilova esa FONGA KETMAYDI, video ko'rinib turaveradi.
Xuddi shu holat qo'ng'iroq oynasi va tizim dialoglarida ham.

Endi:

| Holat | Nima bo'ladi |
|---|---|
| `inactive` | tegilmaydi (parda, dialog) |
| `paused` / `hidden` / `detached` | pauza + ESLAB QOLINADI |
| `resumed` | biz pauza qilgan bo'lsak qaytadi |

`_pausedByLifecycle` bayrog'i SHART: usiz foydalanuvchi ataylab
pauza qilib qo'ygan video ham fon'dan qaytganda o'z-o'zidan ijro
bo'lib ketardi.

### VIDEO TUGAGACH QAYTA BOSHLANMAYDI

TALAB (foydalanuvchi): "video tugagach qayta boshlanmasin,
shunchaki pauza bo'lsin".

`_onCompleted` da ilgari `seekTo(0)` + `play()` turardi. Endi
pleyer oxirida pauza bo'lib turadi. "Play" bosilsa
`_togglePlayPause` uni BOSHIDAN boshlaydi (aks holda `play()`
oxirda turgan videoda hech narsa qilmasdi).

### HALQA: FAQAT AYLANMA YOY

TALAB (foydalanuvchi, aniqlashtirilgan): "play/pause atrofida
aylanadigan chiziq qolsin va avvalgidek aylansin, faqat orqasida
kichkina qizil chiziq bor — shuni olib tashla".

Ya'ni play/pause tugmasi atrofida:

* KUTISH paytida (buferlash, sek, tayyorlash) — avvalgidek
  aylanma yoy. Unga TEGILMAGAN;
* qolgan HAMMA vaqtda — hech nima. Videoning qayeridaligini
  ko'rsatadigan qizil yoy ham, uning orqasidagi xira halqa ham
  OLIB TASHLANDI (vaqt pastdagi chiziqda ko'rinib turibdi).

Kod soddalashdi: `_PlayerRing` va `_PlayerRingPainter` dan
`busy`, `progress`, `trackColor` maydonlari butunlay olindi —
halqa endi FAQAT kutish holatida yaratiladi (`_centerButton`
ichida `if (busy)`), shu sabab u har doim aylanib turadi.

### PASTKI (QO'LDA SURILADIGAN) PROGRESS — FAQAT QIZIL NUQTA

TALAB (foydalanuvchi): "progress chizig'ida faqat qizil nuqta
qolsin deganda PASTDAGI videoni boshqa vaqtga o'tkazadigan,
ya'ni qo'lda suriladigan progressni aytgandim".

`_VideoProgressBar` endi hech qanday chiziq chizmaydi:

* orqa (yuklanmagan) qism — yo'q;
* diskka yuklab olingan oq qism — yo'q (shu sabab
  `downloadedRatio` va `_readyRatio()` ham olib tashlandi);
* o'tilgan qizil qism — yo'q.

Qoladigan yagona ko'rinadigan narsa — hozirgi joydagi QIZIL
NUQTA. Stack ichidagi shaffof `SizedBox(width: w)` faqat kenglik
beradi; bosish zonasi (`thumb + 20` balandlik) avvalgidek keng
qoldi, ya'ni surish qulayligi kamaymadi.

## PLEYERDAGI VAQT — FAQAT DAQIQA VA SONIYA

TALAB (foydalanuvchi): "pleyerdagi vaqt faqat daqiqa va
soniyalarda ko'rsatilsin; agar video 2 soat bo'lsa pleyer
`120:00` qilib ko'rsatishi kerak".

Soat AJRATILMAYDI — daqiqa 60 dan oshib ketaveradi. Ikki joyda
bir xil qoida: `video_player_screen.dart` -> `_fmt` va
`history_screen.dart` -> `_clock` (tarix oynasidagi vaqt ham
shunday — foydalanuvchi so'ragan).

## AYLANMA HALQA — BITTA, BITTA JOYDA

TOPILGAN XATO (foydalanuvchi: "pleyerda sek qilganda aylanadigan
progress chizig'idan IKKITA chiqib qolyapti").

Sabab: halqa IKKI joyda chizilardi — kontrollar ichidagi tugma
atrofida (`_centerButton`) va kontrollar yashiringandagi alohida
qatlamda (`_busyRingOverlay`). Ular `_showControls` bo'yicha
almashardi, LEKIN kontrollar `AnimatedOpacity` bilan 200 ms
so'nadi: bayroq o'zgargan zahoti ikkinchi halqa chiqar,
birinchisi esa hali so'nib ulgurmagan bo'lardi. Ustiga ikkovi
har xil joyda turardi (biri kontrollar ustunining o'rtasida,
ikkinchisi ekran markazida) — shu sabab ustma-ust ham tushmasdi.

Endi play/pause tugmasi ham, halqa ham Stack'dagi BITTA
qatlamda (ekran markazida), `_busyRingOverlay` va `_spinnerOnly`
OLIB TASHLANGAN. Kontrollar ichida tugma yo'q — u yerda faqat
tugma egallaydigan bo'sh joy (`SizedBox`) qoldi.

| kutish | ikonka | ekranda |
|---|---|---|
| ha | ha | aylanma halqa + ikonka |
| ha | yo'q | faqat aylanma halqa |
| yo'q | ha | faqat ikonka (halqa yo'q) |
| yo'q | yo'q | hech nima |

**Bu tuzilishni buzmang:** halqani yana ikkinchi joyda chizsangiz
muammo o'sha zahoti qaytadi.

## OFLAYNDA — FAQAT YUKLAB OLINGANLARI

TALAB (foydalanuvchi):

* bosh sahifada faqat yuklab olingan qismi bor anime kartochkasi;
* tomosha tarixi va sevimlilardan ham faqat yuklab olingan
  epizodi borlari ko'rinsin, **qolganlari yashirilsin LEKIN
  XOTIRADA TURSIN**.

Ya'ni hech narsa O'CHIRILMAYDI — ro'yxat faqat filtrlanadi va
internet yoqilishi bilan hammasi qaytadi.

`lib/services/offline_library.dart` — bitta indeks, uchta ekran:

1. diskdagi bo'limlar ro'yxatidan har bir bo'limning qismlari
   o'qiladi (`eps_<anime>_<season>`);
2. hamma sifat manzillari BITTA ro'yxatga yig'iladi;
3. `videoStats` bitta chaqiruvda hammasining holatini beradi —
   u faqat XOTIRADAGI hisobni o'qiydi, diskka chiqmaydi.

**NEGA HAR QATORDA `videoIsComplete` CHAQIRILMAYDI:** u DISKKA
chiqadi (bir marta skanerlaydi) va ro'yxat chizilayotganda buni
qilish kadrlarni tashlab yuborardi.

**NEGA `videoStats` IKKI MARTA SO'RALADI:** Rust tomoni diskni
fon oqimida skanerlaydi, ya'ni ilova endi ochilganda birinchi
javob bo'sh bo'lishi mumkin. Bo'sh javobni "hech narsa
yuklanmagan" deb qabul qilsak, oflayn bosh sahifa bir zumga
bo'm-bo'sh ko'rinardi.

Internet bor-yo'qligi ham SHU YERDA (`isOffline`) — uchta ekran
bir xil haqiqatga qaraydi, uchta alohida obuna ochilmaydi.

**Indeks hali yig'ilmagan bo'lsa (`ready == false`) hech narsa
yashirilmaydi** — aks holda oflaynda ochilgan ilova bir lahzaga
bo'm-bo'sh ko'rinardi.

## OFLAYNDA PLEYER

Ikki xato tuzatildi:

1. **"Ma'lumot" oynasi bo'sh turardi.** `SeasonService.load` da
   disk keshi yo'q edi. Endi javob diskka yoziladi
   (`season_<a>_<s>`) va oflaynda o'sha ko'rsatiladi; pleyer uni
   tarmoq kutmasdan, DARHOL o'qiydi (`SeasonService.fromDisk`).
   Ustiga ekrandagi maydonlar avval `_info` dan olinadi
   (`_seasonStr` / `_seasonNum`): tarixdan ochilganda
   `widget.season` da atigi bir necha maydon bo'ladi.
2. **Tarixdagi kadr bosilganda qism ochilmasdi.**
   `_autoOpenEpisode` ichida to'g'ridan-to'g'ri
   `if (_offline) return;` turardi. Endi oflaynda ham ochiladi;
   ochib bo'lmaydigan qism (hech bir sifati to'liq emas)
   `_getUrl` bo'sh qaytargani uchun o'zi chetlab o'tiladi.

Tarixdan ochish endi qism RAQAMI bilan emas, `startEpizodId`
bilan bo'ladi.

## TARIX KADRLARI: DARHOL VA QOTISHSIZ

Ikki xato ketma-ket tuzatildi va yechim AYNAN hozirgisi.

**1-xato.** "Anime bo'yicha oynasidan Qism bo'yicha oynasiga surib
o'tkazganda birozga qotib turib keyin o'tyabdi."

Sabab: qo'shni oyna surish BOSHLANGAN zahoti quriladi
(`allowImplicitScrolling`) va o'sha kadrda ro'yxatdagi har bir
qator kadr so'rardi. Kadr esa diskdan SINXRON o'qilib shifri
ochilardi (`secureLoad` — FFI), ya'ni bu ish UI oqimida, aynan
surish boshlangan kadrda bajarilardi.

**2-xato (birinchi yechim keltirib chiqargan).** Birinchi yechim
surish davom etayotganda kadr so'rovlarini KUTDIRARDI
(`holdThumbs` / `releaseThumbs`). Qotish yo'qoldi, lekin rasmlar
kechikib chiqadigan bo'ldi — foydalanuvchi: "juda sekin
yangilanyapti, tez va real-time'da yangilanishi kerak". Chunki
kutish barmoq ko'tarilgunicha (fling bilan bir-ikki soniya)
davom etardi.

**HOZIRGI YECHIM: KUTISH YO'Q, OLDINDAN TAYYOR.**

Ro'yxat o'qilishi bilan diskdagi kadrlar FON'DA xotiraga
ko'chiriladi (`WatchHistory._warmThumbs`) — har bir fayldan oldin
kadrga yo'l beriladi, ya'ni UI qotmaydi, va har bir kadr tayyor
bo'lishi bilan ro'yxat yangilanadi. Ro'yxat qurilganda esa
qatorlar kadrni XOTIRADAN oladi (`peekThumb`): na disk, na
kutish.

Shu sabab surish paytidagi qulf endi KERAK EMAS va OLIB
TASHLANDI — qulfsiz ham surish silliq, chunki surish paytida
bajariladigan ish umuman qolmadi.

### YOZISHMADAGI VIDEO KADRI — KO'RUVCHI O'ZI YASAYDI

**TALAB (foydalanuvchi):** «serverga thumbnail yuklanmasin»,
«tomosha tarixidagidek, faqat support chatga mos qilib,
xatolarsiz va tez ishlaydigan qilib video boshidan kadr olinsin»,
«kadr 2-chi soniyadan emas — 100 ms dagi kadr».

**Hozirgi usul (`lib/services/chat_video_thumb.dart`):**

1. Rust yadrosining `"/thumb?u=<manzil>&ms=100"` yo'li faylning
   faqat kerakli baytlarini (`moov` + birinchi kalit kadr) olib,
   BITTA KADRLIK haqiqiy MP4 yasaydi. Yozishmadagi video 2-5 MB
   bo'lgani uchun bu odatda ~100-300 KB.
2. `MainActivity.grabFrame` shundan JPEG chiqaradi.
3. JPEG **shifrlanib diskka** yoziladi
   (`chatthumb_<fayl>.rustbin`) — shu video uchun tarmoqqa boshqa
   hech qachon chiqilmaydi.

Serverga HECH NARSA yuklanmaydi: `media_thumb` maydoni ham,
B2'dagi `thumb_*.jpg` fayllari ham endi ishlatilmaydi.

**Kadr qaysi lahzadan:** `ms=100`. 2 soniya ko'p edi (videoning
boshi ko'rinmasdi), 0 ms esa ko'pincha qora chiqadi. 100 ms
baribir birinchi kalit kadrning ichida bo'ladi, ya'ni 0 ms bilan
bir xil baytlar olinadi — qo'shimcha narxi YO'Q.

### BU XUSUSIYAT BIR MARTA QAYTARIB OLINGAN — SHARTLARNI BUZMANG

Avvalgi urinish (`chat_thumbs.dart`, commit `6671626` da olib
tashlangan) **ishlamadi va video ijrosini sekinlashtirdi**.
`ChatThumbs` ning to'xtatuvchisi yo'q edi: ekran yopilsa ham ish
davom etardi — 4 urinish x 35 s + tanaffuslar ≈ **2,5 daqiqa**,
ikkitasi parallel. Foydalanuvchi chatdan chiqib video ochganda
bu so'rovlar hamon tarmoqni yeb turardi va sekin ulanishda ijro
qotardi.

O'shanda yozilgan TO'RTTA SHART hozir bajarilgan — **ularni
buzmang:**

| shart | qayerda bajarilgan |
|---|---|
| 1. Ekran yopilganda ish to'xtasin | `ChatVideoThumb` **singleton EMAS**: uni ekran yaratadi va `dispose()` qiladi. `_disposed` har `await` dan keyin tekshiriladi. |
| 2. Video ijro etilayotganda umuman ishlamasin | `VideoGate.busy` — har urinishdan oldin. Pleyer va media oynasi `initState`/`dispose` da `enter()`/`leave()` chaqiradi (`lib/services/video_gate.dart`). |
| 3. Urinishlar soni VA vaqti chegaralangan | Lahzalar `_atMsList` = 100 ms, 1 s, 5 s, 15 s (bosh qismi buzilgan video uchun); har lahza bir urinish (15 s), faqat vaqt tugasa bir marta qayta (30 s). Ekran uchun jami **40 ta tarmoq urinishi** (`_sessionBudget`). Navbatda kutish urinish hisoblanmaydi (ilgari 8 s dan keyin sinalmay yiqilardi). |
| 4. Sekin tarmoqda sinalsin | Shu sabab takror urinish yo'q darajada kam — sekin tarmoqda takror faqat zarar. |

**Shuni ham buzmang:** kadr ro'yxat qurilayotgan kadrda
so'ralmaydi — `_VideoThumb` uni `addPostFrameCallback` ichida
so'raydi va puffak faqat XOTIRADAN o'qiydi (`peek`). Diskdan
sinxron o'qish (`secureLoad`, FFI + shifr) ham birinchi `await`
dan KEYIN bajariladi.

**Yadrodagi kadr xotirasi (`THUMB_MEMO`) BIR NECHTA yozuv saqlaydi.**
Ilgari u bitta edi va izohda "bitta yetarli, qatorlar birin-ketin
so'raydi" deb yozilgandi. Kadr ajratuvchi manzilni HAR DOIM ikki
marta ochadi (avval metadata, keyin kadr) — parallel so'rovlarda
ikkinchisi birinchisining bo'lagini o'chirib yuborardi va kadr
qaytadan yasalardi. Regressiya testi:
`kadr_xotirasi_bir_nechta_yozuvni_saqlaydi`.

### KADR NEGA JUDA SEKIN EDI — ZAXIRA BO'LAK (2026-09)

**TOPILGAN XATO (foydalanuvchi):** «thumbnail qo'yish juda juda
sekin ishlayapti... tomosha tarixidagi thumbnail qo'yish ham
nimagadir sekin».

**Sabab:** `ThumbReader::read` ning har bir chaqiruvi ALOHIDA
HTTP so'rovi edi, `find_moov` esa MP4 sarlavhalarini **atigi 16
baytdan** o'qiydi. Odatdagi fayl tartibi `ftyp` → `mdat` → `moov`,
ya'ni bitta kadr uchun:

| # | o'qish | nima |
|---|---|---|
| 1 | `read(0, 16)` | `ftyp` sarlavhasi |
| 2 | `read(32, 16)` | `mdat` sarlavhasi |
| 3 | `read(<oxiri>, 16)` | `moov` sarlavhasi |
| 4 | `read(moov, ~0,5 MB)` | `moov` tanasi |
| 5 | kalit kadr | kadr baytlari |

**Beshta ketma-ket so'rov**, har biri to'liq borib-kelish vaqti
(mobil tarmoqda 0,3-0,8 s) → bitta kadr 2-5 soniya. Bu tomosha
tarixiga ham, yozishmaga ham BIR XIL tegadi — shu sabab
foydalanuvchi ikkalasining sekinligini aytgan.

**Yechim:** `ThumbReader` da **zaxira bo'lak**. Tarmoqqa
chiqilganda kerakligidan ko'proq (`READAHEAD` = 256 KB) olinadi
va xotirada saqlanadi; kichik sarlavha o'qishlari o'sha
bo'lakdan chiqadi. So'rovlar 5 tadan **2 taga** tushadi
(ko'pincha `moov` va kalit kadr bitta bo'lakka tushib, bittaga
ham).

Katta o'qish (kalit kadr oralig'i 8 MB gacha) saqlanmaydi —
`BUF_MAX` = 1 MB, aks holda arzon telefonda xotira video
ijrosidan tortib olinardi.

**Regressiya testi:** `kadr_sarlavhalari_bitta_sorovda_keladi` —
uchta kichik o'qish bitta so'rovga tushishini o'lchaydi. Zaxira
olib tashlansa test yiqiladi.

Shundan keyin yozishmadagi `_maxParallel` 1 dan **2** ga
ko'tarildi (tomosha tarixidagidek) va `_sessionBudget` 8 dan
**16** ga: har bir kadr arzonlashgach, bitta sekin video
orqasidagilarni ushlab turmaydi.

### PLEYER SEKIN OCHILISHI VA TARIX KADRLARI (2026-09-24)

**Foydalanuvchi:** «pleyer va support chatdagi video judayam sekin
ochilyapti, ba'zida ochilmay qolyapti», «tomosha tarixidagi kadrlar
sekin yangilanyapti va ba'zida (rasmdagidek) xato bo'lib qolyapti».

| Sabab | Tuzatish |
|---|---|
| Har kadr uchun `moov` (0,3-2 MB) qaytadan tarmoqdan olinardi | `MOOV_MEMO` (Rust, 4 ta video / 16 MB / 30 daqiqa). Test: `moov_xotirada_saqlanadi` |
| Ko'rish davomida har 45 s da ANIQ kadr yasalardi — kalit kadrdan to'xtagan joygacha 8 MB gacha, ijro bilan bitta kanalda | Ko'rish davomida faqat KALIT KADR (`/thumb?...&exact=0`). Aniq kadr pleyer yopilgach yasaladi (`_roughKeys`) |
| Tarix kadrlari `VideoGate` ga bo'ysunmasdi — qism almashganda / tarixdan ochilganda pleyer bilan kanalni bo'lishardi | Aniq kadr `VideoGate.busy` paytida KUTADI |
| Vaqtinchalik (eski nuqtadagi) rasm turgan qator haqiqiy kadrni boshqa so'ramasdi | `_Frame` `hasThumb`/`ensureThumb` bilan qayta so'raydi (yiqilsa 20 s tanaffus) |
| Rasmdagi qotgan doira — `RefreshIndicator` + `BouncingScrollPhysics` | Tarix ro'yxatlari `Clamping` harakatida; yangilash 25 s dan oshmaydi |
| `initialize()` 25 s da uzilar, keyin oyna MAJBURAN qayta isitilardi (150 s gacha) | 40 s; vaqt tugasa isitmasdan bir marta qayta ochiladi |
| Worker har so'rovda `app_min_version` ni Turso'dan o'qirdi | Izolyat xotirasida 60 s (`min_version_cached`) |

### YOZISHMADA `moov` OXIRIDA BO'LGAN VIDEOLAR KADRSIZ EDI (2026-09-24)

**Foydalanuvchi:** «support chatdagi videolarning hammasida thumbnail
ko'rsatilmayapti, faqat men encode qilgan va moov atomi oldinga
o'tkazilgan videolarda ko'rsatilyapti».

Rust'dagi kadr yasash `moov` oxirida bo'lgan faylda ham TO'G'RI
ishlaydi — ffmpeg bilan yasalgan ikkala fayldan (faststart va
oddiy) baytma-bayt BIR XIL bo'lak chiqdi (`haqiqiy_fayldan_kadr`,
qo'lda ishlatiladigan test). Muammo tarmoqda edi:

| Sabab | Tuzatish |
|---|---|
| Worker `/api/media` keshda yo'q faylga javob berishdan OLDIN butun faylni keshga ko'chiradi — katta (telefon/Telegram) videoda o'nlab soniya; yadro 15 s da ulanishni uzardi va isitish ham to'xtab, kadr HECH QACHON chiqmasdi | **Foydalanuvchi talabi: FAQAT keshdan, B2'ga to'g'ridan-to'g'ri murojaat YO'Q** (keshga tushmasa 503). Yadro kadr so'rovlarini 180 s kutadi (`THUMB_NET_TIMEOUT`), tayyor bo'lak `THUMB_MEMO` da 10 daqiqa turadi (8 ta / 6 MB) |
| Hajm uchun HEAD (worker'da doim 404) + `bytes=0-0` — ikkita ortiqcha so'rov | Hajm birinchi o'qishning `Content-Range` idan (`probe_head`) |
| `ThumbReader` da bitta zaxira bo'lak: `moov` (oxiri) o'qilgach faylning boshi o'chib, kalit kadr uchun yana so'rov ketardi | Ikkita zaxira bo'lak, faylning boshi saqlanadi. `moov` oxirida: 2 so'rov (ilgari 5), boshida: 1. Test: `moov_oxirida_bolsa_ham_kadr_yasaladi` |

### KADR AJRATUVCHI TASLIM BO'LMAYDI (`MainActivity.grabFrame`)

Telegramdan kelgan ba'zi MP4 fayllarda `getFrameAtTime` har doim
`null` qaytaradi — bu ilovaning xatosi EMAS: telefonning O'Z fayl
menejeri ham o'sha fayllarda kadr ko'rsata olmaydi. Shu sabab
ketma-ket bir necha yo'l sinaladi va birinchi natija beradigani
olinadi:

1. bo'lakning oxirgi kadri (`OPTION_CLOSEST`);
2. bo'lakning boshi (`OPTION_CLOSEST_SYNC`, keyin `OPTION_CLOSEST`);
3. `getFrameAtTime()` — tizim o'zi tanlagan "vakil kadr";
4. `getFrameAtIndex(0)` (Android 9+) — izlashsiz, eng ishonchlisi.

Hammasi MAHALLIY ish: tarmoq kerak emas, har biri bir necha o'n
millisekund.

**Kesh muddati (foydalanuvchi talabi):** Cloudflare kesh yozuvlari
`CHUNK_CACHE_SECONDS` = **1000 kun** (ilgari 400).

## YUKLAB OLISH: NAVBAT, 3 TA JOY VA OXIRIGACHA TEZLIK (2026-09-24)

**Foydalanuvchi:** «qaysi sifat boshida bosilsa avval o'sha yuklansin,
yangilari navbatda tursin», «bir vaqtda eng ko'pi 3 ta sifat,
bittasi tugashi bilan keyingisi avtomatik», «fayl 70% ga borganda
tezlik pasayib ketyapti» (fayllar 71 MB gacha).

| Nima | Qanday |
|---|---|
| Navbat tartibi | `DownloadState::seq` (bosilish tartibi). Ilgari `HashMap` dan birinchi uchragani olinardi — tartib tasodifiy edi. Diskdagi navbat ham shu tartibda yoziladi |
| 3 ta joy | `DOWNLOAD_WORKERS = 3`, `choose_task`: boshlangan vazifa (`started`) tugaguncha joyini ushlaydi — xato bilan kutayotgan bo'lsa ham. Test: `navbat_tartibi_va_uchta_joy` |
| Navbatdagini isitmaslik | `start_prepare` endi `start_download` da emas, `run_download` boshida |
| UI | `queued` maydoni → "navbatda" yozuvi (pleyer va Kutubxona) |
| 70% dagi sekinlashish | Sabab: ulush (4 MiB) oxirigacha egasida qolardi; ish tugagan oqimlar chiqib ketar, oxirini sekin ulanishlar YOLG'IZ tortardi. 71 bo'lak / 16 oqim — qolgan ish oqimlardan kamaygan payt ≈ 77%. Yechim: **ish o'g'irlash** (`steal_locked`: eng kech tugaydigan ulushdan tezlikka mutanosib qism) + **yakuniy takrorlash** (`duplicate_locked`: odatdagidan 2 barobar uzoq ketayotgan oxirgi bo'lak parallel olinadi, yutqazgan oqim `stop` bayrog'i bilan darhol to'xtaydi; bitta bo'lak eng ko'pi 2 marta) |
| O'lchov | `yuklab_olish_umumiy_kanalda_oxirigacha_tez` (umumiy kanal + ulanish chegarasi): eski — 70% dan keyin tezlik ~45% ga tushardi; yangi — tushmaydi, jami 4,6 s → 3,6 s. Ortiqcha trafik < 1% (`tekshir_trafik`) |
| 480 MiB dan katta fayl | Keyingi oyna endi oyna BOSHIDA isitiladi (ilgari oxirida — chegarada hamma oqim kutib qolardi) |

### 98% DA TO'XTAB QOLISH (2026-09-24, skrinshot: 70,1/71,1 MB, telefon tarmog'i 0 KB/s)

Yuklash SEKIN emas, TO'XTAB turardi. Sabab: bitta javob keshdan
emas kelsa (`X-Cache: MISS`, `note_cold_window`), oyna qayta
isitilar va HAR BIR oqim HAR BIR so'rovdan oldin `wait_for_warm` da
90 s gacha kutardi. Qo'shimcha: bosqichda birorta bo'lak olinmasa
keyingi urinishgacha 60 s gacha kutilardi.

Tuzatish: qayta isitish uchun BITTA umumiy muddat — `REWARM_WAIT_MAX`
= 3 s (isitish fon'da tugaydi); xatodan keyingi kutish eng ko'pi
10 s. Test `keshdan_bitta_miss_yuklashni_toxtatib_qoymaydi`:
31 s → 4 s.

**Telegram / Cherrygram bilan solishtirish** (`FileLoadOperation`):
Telegram 128 KB (tajribaviy/Cherrygram "o'rta": 512 KB, 8 so'rov;
Cherrygram "ekstremal": 1 MB, 12 so'rov) qat'iy bo'laklar bilan,
doimiy ulanishlarda, bir vaqtda N ta so'rovni havoda ushlab turadi —
biri tugashi bilan keyingisi. Bizda: 1 MiB bo'lak, 16 oqim,
moslashuvchan ulush + ish o'g'irlash + yakuniy takrorlash — xuddi
shu tamoyil (havodagi so'rovlar soni oxirigacha kamaymaydi).

### YUKLASH 5,5 MB/s DAN OSHMASDI — DASTURIY AES (2026-09-24)

Foydalanuvchi interneti 8-10 MB/s, yuklash esa 5,5 MB/s. Sabab:
`aes` 0.8 ARM64 da apparat AES'ni FAQAT `--cfg aes_armv8` bilan
ishlatadi; CI'da bu bayroq yo'q edi, ya'ni har bir bo'lak DASTURIY
AES-128-CBC bilan shifrlanardi. O'lchov (`crypto::tests::shifr_tezligi`,
server protsessori): apparat ~1050 MB/s, dasturiy ~47 MB/s — telefonning
kichik yadrolarida bundan bir necha barobar sekin.

Tuzatish (keyinroq soddalashtirildi): `aes`/`cbc`/`aes-gcm` kutubxonalari
BUTUNLAY olib tashlandi — hamma shifrlash `ring` orqali (u 64-bit da
apparat AES'ni o'zi aniqlaydi, bayroq kerak emas). Pastdagi
"AES-CBC → AES-128-GCM" bo'limiga qarang.

### VIDEO BO'LAKLARI: AES-CBC → AES-128-GCM (2026-09-24)

Foydalanuvchi: «CTR CBC dan yaxshi bo'lsa CBC ni butunlay olib tashla».
CBC (FFmpeg uchun edi, FFmpeg endi ishlatilmaydi) va `cbc`/`aes`
kutubxonalari OLIB TASHLANDI. Bo'laklar endi `ring` ning AES-128-GCM
i bilan (CTR + 16 bayt teg): `[12 bayt tasodifiy nonce][shifr][teg]`.

* NEGA GCM, toza CTR emas: `video_cache` shifrlangan va (kalit
  kechikkanda yozilgan) OCHIQ bo'lakni HAJMIDAN ajratadi — toza CTR da
  hajm o'zgarmaydi. Teg esa buzilgan bo'lakni aniqlaydi.
* `ring` HTTPS uchun allaqachon bor: 64-bit da apparat AES, 32-bit da
  NEON. O'lchov (server): CBC 1050 MB/s → GCM 5700 MB/s.
* Fayl nomi `.bin` → `.c2`; eski `.bin` bo'laklar `scan_and_clean` da
  o'chiriladi (eski yuklanmalar QAYTA yuklanadi).
* Tezroq shifrlash vaqtlarni o'zgartirdi: ish o'g'irlash va yakuniy
  takrorlash endi faqat haqiqatan sekin holatda (`STEAL_MIN_MS` 1,5 s,
  takror — bo'lak kamida 1 s yuklanayotgan va 1 s yutuq bo'lsa).
* Testlar `[profile.test] opt-level = 2` bilan (debug da sinov
  serverlari sekin edi).

**Worker keshi:** B2 dagi hamma fayl 1000 kun (`CHUNK_CACHE_SECONDS`);
telefon sarlavhasi (`CLIENT_CACHE`) ham 1000 kun; 12 MiB dan katta
faylni oraliqsiz so'raganda ham endi kesh oynasi orqali. Telegram
avatarlari (B2 emas, `/api/avatar/`) 1 kun — rasm almashsa yangilansin.

### DISKDA HAMMA NARSA SHIFRLANGAN (2026-09-24)

Foydalanuvchi: «diskda saqlanadigan hamma narsa shifrlansin, faqat
video emas».

| Nima | Qanday |
|---|---|
| Video bo'laklari | AES-128-GCM (`crypto::encrypt_chunk`) |
| Ro'yxat keshlari, tarix/chat kadrlari, navbat, meta | AES-256-GCM (`seal_blob`, `secureSave`) |
| **Rasm keshi** (posterlar, avatarlar, izoh rasmlari) | Ilgari OCHIQ edi. Endi `image_cache.dart`: `_SealingLocalFs` — undan olingan HAR QANDAY fayl muhrlangan (`_SealedFile`: `openWrite`/`writeAsBytes` muhrlaydi, `readAsBytes` ochadi). Papka `files/aru_images/v2/` |
| URL'lar ro'yxati | Ilgari ochiq sqlite. Endi `JsonCacheInfoRepository.withFile` + muhrlangan fayl `aru_images/v2.index` (kesh `.tmp` qo'shni fayl orqali yozadi — u ham muhrlangan, yorliq `.tmp` siz) |
| Hisob egasi fayli | `secureSave` (kalit yo'q bo'lsagina ochiq — himoya o'chmasin) |
| Sozlamalar/tokenlar | `EncryptedSharedPreferences` (avvaldan) |
| Vaqtinchalik: yozilayotgan ovozli xabar, tanlangan rasm | Tizim plagini yozadi (shifrlab bo'lmaydi); yuborilgach O'CHIRILADI |

Rust: `rust_seal_bytes`/`rust_open_bytes`/`rust_free_bytes` (ikkilik,
base64 siz); `RustCore.sealBytes/openBytes`, `cryptoReady`. Kalit
tayyor bo'lmasa rasm DISKKA yozilmaydi va mavjud fayllar o'chirilmaydi.
Eski ochiq kesh (`aru_images/*`, `databases/aru_images.db*`)
`AppImageCache.dropLegacy` da o'chadi.

**Test (haqiqiy yadro bilan):**
`cd rust && cargo build --release`, keyin
`LD_PRELOAD=$PWD/rust/target/release/librust_core.so flutter test test/image_cache_encryption_test.dart`.
Oddiy `flutter test` da bu testlar o'tkazib yuboriladi.

### BIR NECHTA SIFAT BIRGA: JAMI 16 TA ULANISH (2026-09-24)

Foydalanuvchi: «yangi tizimda battar sekin, 2 MB/s dan o'tmayapti».
3 ta sifat x 16 oqim = 48 ta parallel ulanish ochilardi. Endi havodagi
so'rovlar BUTUN ilova uchun `DL_TOTAL_CONNS` = 16 (`DlPermit`, ruxsat
`fetch_span` ichida — isitishni kutish tugagach — olinadi). Sifatlar
ularni bo'lishadi; biri tugasa qolganlari darhol oladi. Ruxsat
kutilgan vaqt oqimning "sekinligi"ga kirmaydi (`inflight_restart`).
Test: `uch_sifat_birga_ulanishlar_chegarasidan_oshmaydi`.

**Chat kadrlari:** buzilgan bosh qismli videolar uchun bir nechta lahza
(`_atMsList`); navbatdagi 8 soniyalik chegara olib tashlandi (u
sekin videolar orqasidagilarni sinamasdan "yiqildi" deb qo'yardi).

## PROFIL: XOTIRA VA TRAFIK

TALAB (foydalanuvchi): "profildagi Xotira va Trafik
statistikalarining o'rnini almashtir: xotirada faqat xotira
ko'rsatilsin, trafikda esa nimaga qancha trafik ketgani aniq
qilib ko'rsatilsin".

| Qayerda | Nima |
|---|---|
| To'rtlikning 4-katagi | **Xotira** — bitta umumiy raqam |
| Pastdagi keng oyna | **Trafik** — toifalar ro'yxati |

### TRAFIK TOIFALARI

Jami raqam = serverdagi son (`users_db.traffic_bytes`) + ilovada
hozircha yuborilmagan yig'indi (eski qoida: hisobot sutkada bir
marta ketadi, ko'rsatkich esa kutmasligi kerak).

Toifalar esa FAQAT telefonda ma'lum — serverda bitta umumiy son
turadi, u nimaga ketganini bilmaydi:

| Toifa | Manba |
|---|---|
| Videolar | `rust_video_cache_net_bytes` |
| Rasmlar | `/api/image/...`, `/api/avatar/...` (http klient) |
| Ma'lumotlar | qolgan hamma API so'rovi |

`TrafficService._totals` — bu UMR BO'YI hisob: sutkalik hisobot
yuborilgach ham NOLLANMAYDI (nollanadigani `_pending`).
Diskka `list_traffic.rustbin` ga yoziladi.

Toifalar yig'indisi jamidan kam bo'lsa (ilova qayta o'rnatilgan,
boshqa qurilmada ko'rilgan) — farq **"Oldingi hisob"** qatoriga
tushadi, ya'ni foizlar har doim 100% ni beradi.

### XOTIRA TOIFALARI

Xotira o'lchovi (`storage_usage.dart`) o'z holicha qoldi — u endi
faqat JAMI raqam sifatida ko'rsatiladi. Toifalarga bo'lish kodi
saqlanib turibdi: kerak bo'lsa oyna qaytariladi.

## YAGONA YOZUV YO'LI: `SyncQueue` + `POST /api/sync`

**Bu loyihaning eng muhim arxitektura qarori.** Buzmang.

### NEGA (pul)

Turso har bir **yozilgan qator** uchun to'lov oladi. Eski
tartibda bitta qism ko'rilganda **7 ta** qator yozilardi:

| Nima | Qator |
|---|---|
| `watch_history_db` upsert | 1 |
| `epizod_db` — views_total, watch_ms_total | 1 |
| `season_db` — views_total, watch_ms_total | 1 |
| `stats_hourly` + `stats_daily` (views) | 2 |
| `stats_hourly` + `stats_daily` (watch_ms) | 2 |

Ustiga `sessions_db.last_seen_at` **har 60 soniyada**. Bitta faol
odam kuniga ~149 qator, shundan ~85% i `last_seen_at`.

| Foydalanuvchi | Eski tartib | Yangi tartib |
|---|---|---|
| 10k | 45M/oy | 4.5M/oy |
| 100k | ~420M/oy (~$281) | ~45M/oy (**$24.92**) |
| 300k | ~1.3 mlrd/oy | ~135M/oy (~$53) |

Foydalanuvchi qo'ygan shart: **kunlik yozish so'rovlari 50 tadan
oshmasin**. Hozirgi tartibda o'rtacha **2-4 ta**.

### UCHTA QOIDA (`lib/services/sync_queue.dart`)

**1. Hamma yozuv avval telefonda.** Ekranda o'zgarish DARHOL
ko'rinadi, serverga keyin xabar beriladi.

**2. Navbat siqiladi.** Har yozuvning `key` si bor
(`h:anime:season:epizod`, `r:anime:season`, `f:anime:season`);
o'sha kalit navbatda bo'lsa eskisi ALMASHTIRILADI. Bir qismni 50
marta ko'rgan odam ham navbatda **bitta** qator qoldiradi.

Ikki xil maydon HAR XIL siqiladi:

| Tur | Qoida |
|---|---|
| holat (pozitsiya, sifat, baho, sevimli) | oxirgisi o'rnini bosadi |
| "birinchi ko'rish" belgisi | **yo'qolmaydi** (`||`) |

`watched_ms` JAMI qiymat sifatida yuboriladi (`WatchHistory` uni
eski yozuvdan davom ettiradi), farqni server hisoblaydi.

**3. Yuborish shartlari + qat'iy kunlik chegara.**

| Shart | Qiymat |
|---|---|
| Navbat to'ldi | 20 qator |
| Fonga ketdi va oxirgi yuborishdan | 30 daqiqa |
| Ochildi va oxirgi yuborishdan | 6 soat |
| Har holda | 24 soatda 1 marta |
| Chiqish / hisobni o'chirish | majburiy |
| **Oddiy yuborish, kuniga** | **12** |
| **Qat'iy chegara, kuniga** | **50** |

Kunlik hisoblagich telefonda (`list_sync_state.rustbin`), mahalliy
yarim tunda nolga tushadi.

### SERVER: `POST /api/sync` (`sync_route`)

ATIGI IKKI marta bazaga boradi:

1. **bitta o'qish quvuri** — eski holat (tarix, baho, sevimlilar,
   mavjud bo'limlar) va oxirgi paket raqami;
2. **bitta yozuv quvuri** — hamma o'zgarish birdan.

Jamlanadi:

* statistika chelaklari — **paketga bir marta** (ilgari har bir
  qism uchun 4 ta);
* `season_db` — **bo'limga bitta** UPDATE: ko'rish, tomosha vaqti,
  reyting va sevimlilar o'zgarishi birga ketadi;
* qiymatlar **NISBIY** (`+?`) yoziladi — boshqa qurilmadan kelgan
  o'zgarish ustidan yozib yuborilmaydi.

### IKKI MARTA SANALMASLIK

Har paketda bir martalik `batch_id`. Oxirgisi `sync_batches`
jadvalida saqlanadi; takrori kelsa worker **hech narsa
yozmaydi** va `duplicate: true` qaytaradi.

Bu MUHIM: ko'rishlar soni va tomosha vaqti **qo'shiladigan**
raqamlar — takror yozilsa hisob shishib ketardi.

### SOXTA RAQAMLARDAN HIMOYA

Endi ko'rishlar sonini va tomosha vaqtini TELEFON aytadi, ya'ni
o'zgartirilgan ilova statistikani shishira olardi. Har paketda:

| Chegara | Qiymat |
|---|---|
| tarix yozuvlari | 100 |
| baho / sevimli | 50 |
| tomosha vaqti jami | 24 soat |
| yangi ko'rishlar | 50 |
| trafik | 256 GiB |

Telefon soati ham tekshiriladi: `updated_at` kelajakda bo'lsa
server vaqtiga tenglashtiriladi.

### `last_seen_at`: 60 SONIYA → 12 SOAT

`SEEN_EVERY_MS`. Bu bazadagi eng ko'p takrorlanadigan yozuv edi.
Kunlik faol foydalanuvchi 24 soatlik oyna bilan sanaladi, shu
sabab 12 soat aniqlikni buzmaydi. Sinxronlash paketi ham shu
vaqtni yangilaydi — u yerda bepul, o'sha quvurning ichida.

### RO'YXATLAR: SERVER JAVOBI USTIGA NAVBAT QO'YILADI

**Yo'l qo'yilishi mumkin bo'lgan xato:** tarix va sevimlilar
ro'yxati serverdan keladi va xotiradagini butunlay almashtiradi.
Navbatdagi yozuv serverda hali yo'q — ya'ni foydalanuvchi
hozirgina qo'shgan narsasi ekrandan YO'QOLIB qolardi.

Shu sabab `WatchHistory._mergeLocal` va
`FavoritesService._mergeLocal` server javobining ustiga
`SyncQueue.pendingHistory()` / `pendingFavorites()` ni qo'yadi.
**Bu ikkisini olib tashlamang.**

### NIMA HALI HAM DARHOL KETADI

| Nima | Nega |
|---|---|
| Kirish / ro'yxatdan o'tish | sessiyani server yaratadi |
| Admin paneli (anime/bo'lim/qism) | kontentning o'zi, faqat admin |
| Hisobni o'chirish | orqaga qaytmaydigan amal |

### UMUMIY TRAFIK — ILOVADAN (Cloudflare Analytics OLIB TASHLANDI)

Bir muddat bosh sahifadagi trafik Cloudflare'ning
`workersInvocationsAdaptive.sum.responseBodySize` maydonidan
olindi. Texnik jihatdan **ishladi**, lekin raqam telefon qabul
qilganidan **~10 barobar katta** chiqdi: pleyer
`Range: bytes=0-` bilan so'rab, bir necha megabaytdan keyin
ulanishni uzadi — Cloudflare esa yo'lga chiqqan baytni sanaydi.

Foydalanuvchi bunga "bu soxta" dedi, shu sabab **butun Cloudflare
manbasi olib tashlandi** (`cf_traffic_sync`, `traffic_src`,
`CF_*` sirlar). Endi yagona manba — ilova.

**Eski qatorlar BIR MARTA o'chirildi.** `init_db` ichida
`traffic_reset_v2` belgisi bilan: belgi yo'q bo'lsa
`stats_hourly` va `stats_daily` dagi `metric='traffic'` qatorlari
o'chiriladi va belgi qo'yiladi. Aks holda yangi (to'g'ri) raqam
eskisining ustiga qo'shilib, hech qachon haqiqatga kelmasdi.
Kelajakda yana tozalash kerak bo'lsa — belgi nomini
`traffic_reset_v3` ga o'zgartiring.

**Agar server xarajatini bilish kerak bo'lsa** — uni Cloudflare
dashboardidan qarash kerak, ilovaga qo'shish emas
(foydalanuvchi talabi: admin paneliga ham qo'shilmasin).

## TOPILGAN XATOLAR: KIRISH, CHIQISH VA HISOBNI O'CHIRISH

Uchtasi ham 2026-09 da foydalanuvchi topgan va tuzatilgan.

### 1. CHIQQANDAN KEYIN ILOVA O'ZINI O'ZI QAYTA KIRGIZARDI

**Belgi:** hisobdan chiqib, qaytadan kirmoqchi bo'lsa
"Telegramni ochish" tugmasi umuman chiqmasdan hisobga qaytib
kirib ketardi.

**Sabab:** kutilayotgan kirish tokeni HISOB PAPKASIDA saqlanadi
(`_pendingPath` -> `dataDirPath`). `check()` kirish
muvaffaqiyatli bo'lganda avval `_save()` ni chaqirardi, u esa
papkani `accountid_0` (mehmon) dan `accountid_<id>` ga
almashtirardi. Keyingi `clearPending()` YANGI papkadagi mavjud
bo'lmagan faylni o'chirardi — asl token mehmon papkasida
qolaverardi. Chiqilgandan keyin ilova yana mehmon papkasiga
tushar, o'sha tokenni topar va o'zini o'zi kirgizib yuborardi.

**Tuzatish:** `clearPending()` endi `_save()` dan OLDIN
chaqiriladi; ustiga `_clear()` (chiqish) papka almashishidan
oldin ham, keyin ham tozalaydi. **Tartibni buzmang.**

### 2. HISOBNI UMUMAN O'CHIRIB BO'LMASDI

**Sabab (ikkitasi):**

1. Profil rasmi B2'dan o'chmasa BUTUN amal to'xtardi va 502
   qaytardi. B2 bir zumga javob bermasa foydalanuvchi hisobidan
   abadiy qutula olmasdi;
2. hamma o'chirish BITTA Turso quvurida edi — bitta buyruq
   yiqilsa hisobning o'zi ham o'chmay qolardi.

**Tuzatish:**

* rasm o'chmasa ham hisob o'chadi, fayl nomi `orphan_files`
  jadvaliga yoziladi (fayl "yo'qolib" ketmaydi, keyin topib
  o'chirsa bo'ladi);
* ikki bosqich: 1) hisoblagichlarni tuzatish va shaxsiy
  yozuvlar — xatosi yutiladi; 2) `sessions_db`, `login_tokens`,
  `users_db` — MAJBURIY, yiqilsa aniq xabar qaytadi;
* ilova endi serverning xabarini KO'RSATADI ("O'chirib bo'lmadi"
  degan quruq matn o'rniga).

### 3. HISOB PAPKASI BOSHQA ODAMGA O'TIB KETISHI MUMKIN EDI

**Xavf:** `users_db.id` QAYTA ISHLATILADI — server yangi hisobga
"band bo'lmagan eng kichik raqam" ni beradi (`next_user_slot`,
`user_1` / `User 1` nomlari uchun). Papka nomi esa aynan shu
raqamdan yasaladi (`accountid_1`). Ya'ni 1-raqamli hisob
o'chirilib, keyingi odam ham 1 ni olsa, eski egasining
telefonidagi papka yangi odamga ochilib qolardi.

**Tuzatish:** papkaga `owner.json` yoziladi — ichida egasining
TELEGRAM raqami (u hech qachon qayta ishlatilmaydi). Kirganda
raqam mos kelmasa papka tozalanadi
(`AccountData.guardOwner`, `switchTo` va `restore` da
chaqiriladi).

### 4. `ensure_db` YIQILSA HAM "TAYYOR" DEB BELGILARDI

`init_db` xatosi yutilardi, lekin `DB_READY` baribir
qo'yilardi — izolyat umrining oxirigacha jadvallar yaratilmagan
holda ishlayverardi va har bir so'rov "no such table" bilan
yiqilardi. Endi belgi FAQAT uchala quvur ham muvaffaqiyatli
bo'lganda qo'yiladi.

## BALANS, OBUNA VA TO'LOVLAR (tezchek.uz)

TALAB (foydalanuvchi): profil sahifasida "Obuna olish va Balans
to'ldirish" tugmasi; ichida uchta surib o'tkaziladigan oyna —
**Obuna**, **To'ldirish**, **Tarix**. Balansda pul bo'lsa Obuna
oynasi, bo'lmasa To'ldirish oynasi ochiladi.

### PUL BILAN BOG'LIQ HAR BIR QAROR SERVERDA

Ilovada birorta narx YO'Q. Tariflar `worker/src/lib.rs` dagi
`PLANS` da:

| Kun | Narx |
|---|---|
| 1 | 1 000 so'm |
| 5 | 4 000 so'm |
| 10 | 7 000 so'm |
| 20 | 12 000 so'm |
| 30 | 15 000 so'm |

Ilova faqat **kunni** yuboradi (`{"days": 5}`), narxni server
o'zi topadi. Aks holda o'zgartirilgan ilova 30 kunlik obunani
1 so'mga olardi. **Narxni ilovaga ko'chirmang.**

### JADVALLAR

| Jadval | Nima |
|---|---|
| `payments_db` | har bir to'lov havolasi (`order_id` PK, `status`, `expires_at`) |
| `subs_db` | odamga bitta qator: obuna qachon tugaydi |
| `billing_log` | Tarix oynasi: har bir to'ldirish va obuna |

### YO'LLAR

| Yo'l | Nima qiladi |
|---|---|
| `GET /api/billing` | balans, obuna muddati, tariflar, faol havolalar, tarix |
| `POST /api/billing/create` | tezchek'da to'lov yaratadi, havola qaytaradi |
| `POST /api/billing/check` | to'lov bo'ldimi; bo'lsa balansni oshiradi |
| `POST /api/billing/subscribe` | balansdan yechib obunani uzaytiradi |

### TEZCHEK API

Hujjat: `https://tezchek.uz/public-api-system`
(OpenAPI: `?action=get_openapi`). Atigi ikkita yo'l kerak:

```
POST https://tezchek.uz/api/create_invoice
     {api_key, amount}            -> {ok, order_id, pay_url}

POST https://tezchek.uz/api/status_invoice
     {api_key, order_id}          -> {ok, payment:{status:"paid"|...}}
```

Kalit — worker siri **`TEZCHEK_API_KEY`** (GitHub secret'dan
`deploy-worker.yml` qo'yadi). U ilovaga **hech qachon
chiqmaydi**.

### PUL IKKI MARTA QO'SHILMASLIGI

"Tekshirish" tugmasini necha marta bossa ham balans BIR MARTA
oshadi:

```sql
UPDATE payments_db SET status='paid', paid_at=?
 WHERE order_id=? AND status='pending' RETURNING order_id
```

Qator qaytmasa — demak boshqa so'rov ulgurgan va balans
allaqachon oshirilgan. **Bu shartni olib tashlamang.**

### HAVOLA 1 SOAT YASHAYDI

`PAY_LINK_TTL_MS`. Muddati o'tgan havolalar ro'yxatda
ko'rsatilmaydi, bir kundan keyin esa jadvaldan o'chiriladi
(`/api/billing` ichidagi tozalash).

## YUKLANMALAR OYNASI

TALAB (foydalanuvchi): foiz **real vaqtda** yangilansin, qanaqa
sifatda / qancha MB / qancha foiz yozilsin, tozalash tugmasi
kattaroq va aniq ishlaydigan bo'lsin, kadr yonida yuklab olish
ikoni bo'lsin.

* `DownloadsIndex.refreshStats()` — ARZON yangilanish: faqat
  `downloaded`/`total` raqamlari (bitta `videoStats` chaqiruvi,
  diskka chiqmaydi). Ro'yxatda har soniyada chaqiriladi, to'liq
  `refresh()` esa 10 soniyada bir marta;
* ro'yxat `WatchHistory` ni ham eshitadi — kadr tarix
  oynasidagidek darhol yangilanadi (ilgari faqat `DownloadsIndex`
  eshitilardi va kadr eskirib turardi);
* `qualityOf(url)` — fayl nomidan sifatni ajratadi
  (`ep_1_1_720p_...mp4` -> `720p`);
* tugmalar `_RowIconButton` — 40x40 bosish maydoni (ilgari ikonka
  17 nuqta edi va barmoq tegmasdan qolardi);
* yuklab olish ikoni `_QualitySheet` ni ochadi: har bir sifat,
  hajmi va "Yuklab olish" tugmasi. Hajm yuklab olingan sifat
  uchun diskdan, qolganlari uchun bir baytlik `Range` so'rovidan
  (`DownloadsIndex.sizeOf`).

## TUZATILGAN XATOLAR (2026-09, ikkinchi to'plam)

### AVTO O'TKAZISH ISHLAMASDI — CHEKSIZ SEK HALQASI

**Belgi:** avto o'tkazish yoqilganda intro vaqti kelganda
o'rtadagi halqa aylanaverardi, video esa o'tmasdi.

**Sabab:** sek AYNAN oraliqning oxirgi millisekundiga qilinardi.
Pleyer eng yaqin KALIT KADRGA tushadi va u ko'pincha oraliqning
ICHIDA qolardi — keyingi pozitsiya yangilanishida `_introAt`
yana o'sha oraliqni topib, `_skipIntro` qaytadan chaqirilardi.

**Tuzatish:** sek oraliq oxiridan `_introSkipPad` (400 ms) keyinga
qilinadi va o'tkazilgan oraliq `_introDone` ga yoziladi — ikkinchi
marta AVTOMATIK o'tkazilmaydi (qo'lda tugma bosish mumkin).

### TRAFIK FOIZI 100% DAN OSHARDI

Tepadagi jami raqam SERVERDAN olinardi, pastdagi taqsimot esa
TELEFONDAGI umrbod hisobdan — ikki xil manba. Telefondagisi
kattaroq bo'lib qolsa "70,9 MB — 100.00%" bo'lib, jami esa
70,3 MB bo'lib turardi. Endi jami — **ikkovining kattasi**.

### PASTKI PANEL TIZIM TUGMALARI USTIGA CHIQARDI

Faqat `viewPadding` ga tayanilardi va u ba'zi holatda nol
kelardi. Endi `viewPadding` va `padding` ning KATTASI olinadi,
eng kam chekinish 24 nuqta.

### PLEYER OYNALARI KO'RINMASDI

Yarim shaffof oq to'rtburchak ochiq kadr ustida yo'qolib ketardi.
Endi `_PlayerPanel`: `BackdropFilter` (orqa xiralashadi) + quyuq
QORA fon + aniq chegara va soya.

## YUKLASH TEZLIGI O'LCHAGICHI

**Nima aniqlangan:** server (isitilgan keshdan) **35-87 MB/s**
beradi va bo'lak o'lchami deyarli ahamiyatsiz — 1x32 MiB ham,
16x2 MiB ham bir xil. Ya'ni 5-6 MB/s chegara **telefondagi
kodda**.

Qaysi qismida ekani TAXMIN bilan emas, o'lchov bilan aniqlanadi.
`DlTiming` har bir oqimning vaqtini beshga bo'lib yig'adi va
yuklash tugagach jurnalga yozadi:

```
O'LCHOV <kalit>: 166.0 MB / 31.2s = 5.32 MB/s |
  kutish 2.1s (7%) · ttfb 2.9s (9%) · o'qish 18.1s (58%) ·
  yozish 9.4s (30%) · qulf 0.8s (3%)
```

Bitta yuklashdan keyin shu qator sababni ANIQ ko'rsatadi.

## TELEGRAM O'ZI OCHILMAYDI

TALAB (foydalanuvchi): "Telegram orqali kirish tugmasini
bosganda Telegram avtomatik ochilmasin — shunchaki 'Telegramni
ochish' degan tugma chiqib tursin".

Ilgari kirish ekrani ochilishi bilan ilova o'zi Telegramga
sakrab ketardi. Endi ekran ochilganda faqat BITTA holat so'rovi
yuboriladi (token diskda saqlangani uchun foydalanuvchi START'ni
allaqachon bosgan bo'lishi mumkin), Telegram esa FAQAT
"Telegramni ochish" tugmasi bosilganda ochiladi.

## KUTUBXONA: YUKLANMALAR

TALAB (foydalanuvchi): "Kutubxona sahifasidagi yuklanmalar
oynasini olib tashlab, tarix oynasiga qism bo'yicha oynasining
o'ng tarafiga qo'sh".

Kutubxonada endi ikkita oyna (Tarix, Sevimlilar); "Yuklanmalar"
esa tomosha tarixining UCHINCHI sahifasi — barmoq bilan surib
o'tiladi.

### RO'YXAT QANDAY YIG'ILADI

Rust yadrosida "qaysi videolar keshda bor" degan ro'yxat YO'Q —
u faqat berilgan manzillar bo'yicha holat qaytaradi. Shu sabab
`downloads_index.dart` nomzodlarni `OfflineLibrary` dagi kabi
yig'adi (anime keshi + tarix + sevimlilar -> `eps_*` -> hamma
sifat manzillari) va BITTA `videoStats` chaqiruvi bilan
holatni oladi. `downloaded > 0` bo'lganlari qoladi.

**Tartib — oxirgi bo'lak qachon yozilgani.** Rust har video
uchun alohida papka ochadi
(`<support>/video_byte_cache/<kalit>`), papkaning
o'zgartirilgan vaqti aynan shuni bildiradi. Kalit qoidasi
`cacheKeyOf` da va u `rust/src/video_cache.rs` -> `cache_key`
bilan BIR XIL bo'lishi SHART — biri o'zgarsa ikkinchisi ham
o'zgarishi kerak.

**Bitta qism = bitta qator** (eng ko'p yuklangan sifat), lekin
o'chirishda qismning HAMMA sifati o'chiriladi.

**To'liq yuklanmagan qism** bosilganda ochilmaydi — avval
"to'liq yuklab olinsinmi?" deb so'raladi (foydalanuvchi talabi).

## BREND: ARU / ARUmedia

Ilova nomi — **ARUmedia**, logotipi — **ARU**. (`fulutter` faqat
repo/papka nomi va Android paket nomi: `uz.fulutter.fulutter`.)

| Qayerda | Nima |
|---|---|
| Telefondagi belgi | `branding/android-res/mipmap-*/ic_launcher.png` |
| Yumaloq belgi | `ic_launcher_round.png` — MIUI/One UI aynan shuni oladi |
| Moslashuvchan belgi | `mipmap-anydpi-v26/ic_launcher.xml` (qora fon + oq harflar) |
| Belgi ostidagi yozuv | CI manifestga `android:label="ARUmedia"` yozadi |
| Ochilish ekrani | qora fon + ARU logotipi (`drawable*/launch_background.xml`) |
| Ilova ichida | `lib/widgets/aru_logo.dart` -> `assets/aru-mark.png` |

### "Telefonda hali ham Flutter belgisi turibdi"

Build 162 APK'si **ochib tekshirildi**: `android:icon` ->
`mipmap/ic_launcher`, ichidagi 11 ta rasmning hammasi ARU
(ko'k piksel 0%), `application-label:'ARU'`. Ya'ni APK to'g'ri
edi — telefon ESKI belgini keshdan ko'rsatayotgan edi (yorliq
yangilangan, rasm esa yo'q — bu aynan kesh belgisi).

Shu sabab `ic_launcher_round` qo'shildi: bu resurs ilgari umuman
mavjud bo'lmagan, ya'ni uning eski keshlangan nusxasi ham yo'q.
Kesh baribir qolsa — ilovani **butunlay o'chirib**, qaytadan
o'rnatish kifoya.

**TOPILGAN XATO (tuzatildi).** `build-flutter-apk.yml` ning
`paths:` filtrida `branding/**` yo'q edi — ya'ni logotipni
o'zgartirgan commit'lar **umuman APK yig'masdi** va telefonda eski,
belgisiz APK turaverardi.

Endi qo'shimcha himoya ham bor: tayyor APK ochilib, ichidagi belgi
rasmi **ochib ko'riladi**. Ko'k piksel topilsa (Flutter'ning
standart belgisi) build ataylab yiqiladi. Bayt solishtirilmaydi,
chunki `aapt2` PNG'larni qayta siqadi.

## PASTKI PANEL TIZIM TUGMALARI ORTIDA QOLMASIN

Pleyer to'liq ekranda `immersiveSticky` rejimini yoqadi va o'shanda
`MediaQuery.padding.bottom` **nolga tushadi**. Pleyerdan
chiqilganda MIUI yangilangan `padding` ni kechikib yuboradi —
`SafeArea` esa aynan `padding` ga tayanadi, shu sabab panel tizim
tugmalari ortida qolib ketardi.

Yechim: `MediaQuery.viewPaddingOf(context).bottom`. `viewPadding`
tizim paneli yashiringan bo'lsa ham **jismoniy** chekinishni
ko'rsatib turadi, ya'ni nolga tushmaydi.

## PASTKI TUGMA SUZISHI — SAHIFA DARHOL, TUGMA SEKIN

Foydalanuvchi talabi (aynan shunday): "bosishim bilan o'sha
sahifaga o'tishi kerak, lekin tugmani kattalashtiradigan narsa
birozgina sekinroq va silliq, ketma-ket tugmalarni
kattalashtirib o'tishi kerak".

Ya'ni ikkovi BIR-BIRIDAN MUSTAQIL:

| Nima | Qanday |
|---|---|
| Sahifa | bosilgan **zahoti** (`IndexedStack` indeksi) |
| Pushti tugma | o'z yo'lini **sekin** bosib o'tadi (~280 ms/oraliq) |

### Ikkita sabab bor edi

**1. Sahifa aynan suzish paytida qurilardi.** `PageView` +
`jumpToPage` sahifani BIRINCHI marta o'tilgan damda quradi
(`initState`, ro'yxatlar, rasm vidjetlari) va kuchsiz telefonda
bu bir necha kadr qotish beradi.

Endi **`IndexedStack`**: barcha sahifalar ilova ochilganda bir
marta quriladi, keyin faqat qaysi biri ko'rinishi almashadi —
tugma bosilganda quriladigan ish umuman qolmaydi. Bosh
sahifadan boshqa to'rttasi yengil (hech biri `initState` da
tarmoqqa chiqmaydi), shu sabab ilova ochilishiga sezilarli
ta'sir qilmaydi. `sizing: StackFit.expand` SHART — aks holda
sahifalar butun ekranni egallamaydi.

**2. Animatsiya vaqtni haqiqiy soat bo'yicha o'lchardi.**
`AnimationController` shunday ishlaydi: bitta kadr 300 ms
chizilsa, keyingi kadrda u darhol o'sha 300 ms ga **sakraydi** —
qisqa suzish esa butunlay yeb ketiladi. Aynan shu sabab kuchsiz
telefonda suzish umuman ko'rinmasdi.

Endi `AnimationController` YO'Q. Uning o'rniga oddiy `Ticker`
(`_onNavTick`) va vaqt QO'LDA qo'shiladi:

```dart
var dt = (elapsed - last).inMicroseconds / 1000.0;
if (dt > _maxFrameMs) dt = _maxFrameMs;   // 32 ms
_navT += dt / _navMs;
```

Qotish bo'lsa suzish shunchaki cho'ziladi, lekin **hech qachon
sakramaydi**: tugma har doim oradagi hamma belgini birin-ketin
kattalashtirib o'tadi. **Bu tartibni buzmang** — `Ticker` ni
qaytadan `AnimationController` ga almashtirsangiz muammo o'sha
zahoti qaytadi.

### Tezlik

Bitta tugma oralig'iga **280 ms**, ustiga ekran kengligi
bo'yicha ko'paytma (`_speedFactor`): ~360 dp da 1.20, 600 dp da
1.00, 900 dp va undan katta ekranda 0.90 — kichik ekranda tugma
bosib o'tadigan masofa qisqa, shu sabab bir xil vaqt u yerda
"shosha-pisha" ko'rinadi. Umumiy davomiylik 320–1400 ms
oralig'ida qisiladi. Bosh sahifadan profilga (4 oraliq) ~1,1
soniya.

Egri chiziq — `Curves.easeInOutSine`. `easeInOutCubic` sinab
ko'rilgan edi: u o'rtasida o'rtacha tezlikdan IKKI BAROBAR tez
ketardi va aynan shu "uchib o'tdi" hissini bergan.

## HAMMA FAYL SHIFRLANADI

Talab: ilovaga tegishli **barcha** fayllar shifrlangan bo'lsin.
Rasm keshi (`cached_network_image`) bundan mustasno — foydalanuvchi
uni shart emas dedi.

| Fayl | Usul |
|---|---|
| Video bo'laklari | AES-128-CBC (bo'lak darajasida) |
| Ro'yxat keshi, kirish tokeni | AES-256-GCM |
| `meta.json` | AES-256-GCM (`read_meta` / `write_meta`) |
| `download_queue.json` | AES-256-GCM (`read_sealed` / `write_sealed`) |
| `w<N>.warm` belgilari | AES-256-GCM |
| Tarix kadrlari (JPEG) | AES-256-GCM (`secureSave`, base64) |

Yangi kichik fayl qo'shsangiz — `read_sealed` / `write_sealed` dan
foydalaning, `fs::write` ni to'g'ridan-to'g'ri ishlatmang.

**Yorliq (label) qat'iy belgilanadi**, fayl yo'lidan olinmaydi:
papka yo'li ilova yangilanganda o'zgarishi mumkin va o'shanda kalit
ham o'zgarib, eski fayllar o'qilmay qolardi.

**Migratsiya**: shifrlashdan oldin yozilgan ochiq fayllar ham
o'qilaveradi (avval shifr ochishga urinamiz, bo'lmasa oddiy
ma'lumot deb qaraymiz). Keyingi yozishda ular o'zi shifrlangan
holatga o'tadi.

## TOMOSHA TARIXI

### Qayerda saqlanadi

**Asosiy manba — Turso, lekin YOZUV NAVBAT ORQALI.** Mahalliy
nusxa ham oflayn uchun, ham yuborilmagan o'zgarishlarni ushlab
turish uchun kerak: server ro'yxati kelganda uning ustiga
`SyncQueue.pendingHistory()` qo'yiladi (`_mergeLocal`), aks holda
hozirgina ko'rilgan qism ekrandan yo'qolib qolardi.

Jadval `watch_history_db`: `user_id + anime_id + season_id +
epizod_id` birlamchi kalit (RAQAM emas — yuqoridagi "KALIT
`epizod_id`" bo'limiga qarang), ya'ni bitta qism uchun HAR DOIM
bitta qator. `(user_id, deleted_at, updated_at DESC)` indeksi —
ro'yxat aynan shu tartibda so'raladi.

### KIRMAGAN FOYDALANUVCHI ANIMENI OCHOLMAYDI

Bosh sahifadagi kartalar HAMMAGA ko'rinadi, lekin ustiga bosilganda
`AuthService.isLoggedIn` tekshiriladi (`HomeScreen._openSeason`).
Kirilmagan bo'lsa pleyer OCHILMAYDI — o'rniga oyna chiqadi:
"Iltimos anime ko'rish uchun avval profil sahifasiga o'tib
accountingizga kiring yoki yangi accaunt oching".

### SEVIMLILAR VA SHAXSIY STATISTIKA

Pleyerdagi yurakcha bosilgan bo'limlar Kutubxonadagi
**Sevimlilar** oynasida ko'rinadi (`GET /api/favorites` — javobda
bo'lim qatorlarining O'ZI keladi, ya'ni kartochka darhol
chiziladi va pleyer qo'shimcha so'rovsiz ochiladi). Ro'yxat
Kutubxona tugmasi bosilganda yangilanadi va diskka yoziladi.

Sevimlilar ro'yxati ham server javobining ustiga navbatni
qo'yadi (`FavoritesService._mergeLocal`) — yurakcha bosilgan
zahoti Kutubxonada ko'rinadi, sinxronlash kutilmaydi.

Profil sahifasida rasm/balans tagida **2x2 shaxsiy statistika**:
nechta ANIME (bo'lim emas — `anime_id` bo'yicha noyob), nechta
qism, necha soat va qancha trafik. Bitta so'rov:
`GET /api/me/stats`. Paket muvaffaqiyatli ketgach u majburiy
yangilanadi — aks holda raqamlar bir necha soat orqada qolardi.

## ADMIN PANELIDAN "OTILIB CHIQISH"

Rasm/video tanlashda Android galereyani oldinga chiqaradi va
xotirasi kam telefonda ILOVANI BUTUNLAY YOPADI — foydalanuvchi
qaytganda ilova noldan ochilardi.

Buni `Navigator` bilan hal qilib bo'lmaydi (jarayonning o'zi
o'ladi). Shu sabab `lib/services/ui_state.dart`: fayl tanlashdan
OLDIN diskka belgi qo'yiladi, tanlash tugashi bilan olib
tashlanadi. Ilova ochilganda `RootScreen` o'sha belgini ko'rsa —
admin panelini qaytadan ochadi. Foydalanuvchi orqaga qaytsa yoki
ilovani o'zi yopsa, belgi allaqachon tozalangan bo'ladi.

## BOSH SAHIFADAGI KARTA

Karta = bitta BO'LIM (`season_db` qatori). Nomning ustida
`N-bo'lim` yozuvi turadi (`bolim_id`), nomga ajratiladigan joy
esa kartaning O'Z kengligidan hisoblanadi (`LayoutBuilder`):
harf kattaligi va qatorlar soni ekranga qarab o'zgaradi, ya'ni
uzun nom kichik telefonda rasmni bosib ketmaydi, kattasida esa
to'liq ko'rinadi.

### So'rovlar soni — buzmang

| Qachon | Nechta so'rov |
|---|---|
| Pleyerdan chiqilganda / qism almashganda / ilova fonga ketganda | **1 ta** `POST /api/history` |
| Kutubxona tugmasi bosilganda | **1 ta** `GET /api/history` |

To'xtagan joy har soniya eslab qolinadi, lekin u FAQAT telefon
xotirasiga yoziladi (`WatchProgress`) — serverga emas.

### QANCHA KO'RILSA TARIXGA TUSHADI (chegara QAT'IY EMAS)

**TOPILGAN XATO.** Chegara qat'iy 15 soniya edi: `flush()` da ham,
`WatchProgress.save()` da ham. 17 soniyalik qismda esa boshidagi 15
va oxiridagi 30 soniya butun qismni qoplab olardi — ya'ni qisqa
qism necha marta ko'rilsa ham **tarixga umuman tushmasdi** va har
safar **boshidan** ochilardi. Foydalanuvchi aynan shuni ko'rgan.

Endi chegara qism uzunligiga bog'langan va ikkala joyda BITTA
qoida (`WatchProgress.minPositionFor` / `endMarginFor`):
uzunlikning **10%** i, lekin ko'pi bilan 15 (boshida) va 30
(oxirida) soniya. Uzun qismlarda hech narsa o'zgarmadi.

### RO'YXAT DARHOL YANGILANADI (server javobi kutilmaydi)

`flush()` yozuvni serverga yuborishdan OLDIN uchta ish qiladi:

1. `_applyLocal` — yozuvni xotiradagi ro'yxatga qo'yadi (bor bo'lsa
   ustiga yozadi), sanani yangilaydi, ro'yxatni qayta saralaydi va
   shifrlangan nusxaga yozadi. Shu sabab hozirgina ko'rilgan qism
   Kutubxonada **eng tepada** turadi — internet bo'lmasa ham;
2. `_prepareThumb` — to'xtagan joydagi kadrni SHU ZAHOTI yasab
   diskka yozadi (pastga qarang);
3. va faqat keyin — bitta `POST`.

Nom, bo'lim nomi va rasmlar `startEpisode()` orqali pleyerdan
keladi (`widget.season`), ya'ni mahalliy yozuv ham to'liq bo'ladi.

### KADRLAR REAL VAQTDA YANGILANADI

Kadr tayyor bo'lishi bilan `WatchHistory` xabar beradi
(`notifyListeners`), har bir qator esa `peekThumb` orqali uni
darhol oladi. Ya'ni "Anime bo'yicha" va "Qism bo'yicha"
oynalarining IKKALASI ham bir vaqtda yangilanadi — boshqa oynaga
kirib chiqishni kutish shart emas.

### YOZUVNI O'CHIRISH

Qism kadri ustida **uzoq bosilsa** "Rostdan ham bu tarixni
o'chirib tashlaysizmi?" so'raladi. "Ha" bo'lsa: ro'yxatdan darhol
yo'qoladi, kadr fayli o'chiriladi, serverga `DELETE /api/history`
ketadi.

**Yozuv bazadan O'CHMAYDI** (foydalanuvchi talabi): faqat
`deleted_at` belgilanadi. Qism keyin qayta ko'rilsa yozuv yana
paydo bo'ladi, statistika esa umuman buzilmaydi. Yuborib bo'lmasa — navbatga tushadi (`_op: delete`) va
keyin yuboriladi, ya'ni yozuv qaytib kelmaydi.

`GET` javobi ro'yxat uchun kerak bo'lgan hamma narsani bir yo'la
beradi (anime nomi, posteri, bo'lim raqami), ya'ni qo'shimcha
so'rov yo'q. Ro'yxat **60 soniya** xotirada "yangi" hisoblanadi.

**Yuklash `HistoryTab` ning `initState` ida EMAS**: Kutubxona
sahifasi ilova ochilganda birga quriladi (`IndexedStack`), shu
sabab u yerda yuklasak foydalanuvchi kutubxonani ochmasa ham
so'rov ketardi. Yuklash `RootScreen._onTabTap` da — tugma
bosilganda.

**Oflayn**: yuborib bo'lmagan yozuv shifrlangan navbatga
(`watch_history_outbox_<user>`) tushadi va keyingi yuklashda
yuboriladi. O'qib bo'lmasa — oxirgi olingan ro'yxat ko'rsatiladi.
Kalitlar HAR BIR HISOB UCHUN ALOHIDA: bitta telefondan ikki kishi
kirsa, biri ikkinchisining tarixini ko'rmaydi.

### To'xtagan joydagi kadr — MILLISEKUNDGACHA ANIQ

**TOPILGAN XATO.** Ilgari ikkita joyda aniqlik yo'qolardi:
kalit (`thumbKey`) vaqtni **10 soniyaga** yaxlitlardi va Rust
faqat **kalit kadr**ni berardi (Android esa uni
`OPTION_CLOSEST_SYNC` bilan o'qirdi). Natijada rasm to'xtagan
joydan bir necha soniya narida bo'lardi.

Endi:

* `thumbKey` — aniq millisekund (`<video>_<ms>`);
* Rust `/thumb` kalit kadrdan **so'ralgan kadrgacha** bo'lgan
  namunalarni BITTA oraliq bilan o'qib, ulardan kichik MP4 yasaydi
  (`mp4::build_clip_mp4`). Dekodlash tartibida har bir kadrning
  tayanchlari undan oldin turadi, shu sabab bo'lakni istalgan
  joyda kesish xavfsiz. `ctts` ATAYLAB ko'chirilmaydi — usiz
  "eng oxirgi kadr" AYNAN so'ralgan kadr bo'lib qoladi;
* `MainActivity.kt` bo'lakning davomiyligini o'qib, **oxiridan
  1 ms beri**ga `OPTION_CLOSEST` bilan boradi.

Chegaralar: bitta kadr uchun eng ko'pi **16 MB** va **900**
namuna o'qiladi; oshsa eski yo'lga (bitta kalit kadr) qaytadi —
rasm eskiroq bo'ladi, lekin trafik cheklangan qoladi.

### Kadr QACHON yasaladi

Ko'rish tugagan zahoti (`flush()` ichida), ro'yxat ochilishini
kutmasdan. Ikkita sabab:

* tarix oynasi ochilganda og'ir ish qolmaydi — "Anime bo'yicha"
  dan "Qism bo'yicha" ga o'tishdagi qotish aynan shundan edi;
* **oflaynda ham rasm ko'rinadi**: yuklab olinmagan qism uchun
  kadr tarmoqdan olinadi, tarmoq esa aynan ko'rish paytida bor
  edi.

`rust/src/mp4.rs` — MP4 konteyneridan kadr ajratib oladi;
`video_cache.rs` dagi `/thumb?u=<url>&ms=<vaqt>` yo'li uni xizmat
qiladi:

1. yuqori darajadagi atomlar kezilib `moov` topiladi — faqat
   16 baytlik sarlavhalar o'qiladi, ya'ni `mdat` (butun video)
   ustidan sakrab o'tiladi;
2. `stts`/`stss`/`stsc`/`stsz`/`stco` jadvallaridan kerakli
   soniyaning KALIT KADRI topiladi;
3. faqat o'sha kadr olinadi (diskdan — bepul, yoki tarmoqdan —
   50-300 KB);
4. `build_clip_mp4` kalit kadrdan so'ralgan kadrgacha bo'lgan
   to'la haqiqiy MP4 yasaydi (`stsd` asl fayldan AYNAN
   ko'chiriladi — busiz dekoder kadrni ocholmaydi);
5. natija xotirada 60 soniya turadi va o'zi o'chadi — diskka
   YOZILMAYDI.

Dart tomoni (`WatchHistory.thumbnail`) bu manzilni ilovaning O'Z
kadr ajratuvchisiga (`MainActivity.kt`) beradi, u telefonning
APPARAT dekoderi bilan JPEG chiqaradi. JPEG shifrlangan holda saqlanadi va qism
oldinga surilsa eskisi o'chiriladi.

**Kadrni Rust dekodlamaydi va dekodlamasin**: H.264/H.265
dekoderi sof Rust'da yo'q, C kutubxonasi esa APK'ni bir necha MB
kattalashtiradi va loyihaning "faqat sof Rust" qoidasini buzadi.

**`/thumb` va `/v` ni bir joyga qo'shmang**: `/v` (ijro) tarmoqqa
UMUMAN chiqmaydi — bu loyihaning asosiy qoidasi. `/thumb` esa
chiqishi mumkin, lekin faqat bir necha yuz kilobayt oladi va
bo'laklarni diskka yozmaydi.

Har bir qadamda xato bo'lsa 404 qaytadi va ilova posterni
ko'rsatadi — hech qachon yiqilmaydi.

### Kutubxona sahifasi

Uchta oyna: **Tarix** (ishlaydi), **Sevimlilar** va
**Yuklanmalar** (hozircha bo'sh — keyingi vazifa).

Tarix ichida "Anime bo'yicha" va "Qism bo'yicha". Ikkovi BITTA
joyda yashaydi (`PageView`): tugma bosilsa ham, barmoq bilan
surilsa ham sahifa suzib almashadi.

**Anime bo'yicha** — har bir anime bitta karta: oxirgi ko'rilgan
qismning KADRI, tagida **bo'lim nomi** (anime nomi EMAS), tagida
"Oxirgi marta N-bo'lim M-qismni ko'rdingiz" va
"Sana: 12:46/01/01/2026" (soat/kun/oy/yil).

**Qism bo'yicha** — 16:9 kadr, pastida progress chizig'i,
chiziqning USTIDA yozuvlar: chapda bo'lim nomi / `N-bo'lim
M-qism` / `sana: ...`, o'ngda esa `43,21% | 12:34/56:12`.

Kadrga bosilsa — o'sha qism AYNAN o'sha joydan ochiladi; uzoq
bosilsa — o'chirish so'raladi.

**Kadr ustiga qorayish (scrim) TUSHMAYDI** — foydalanuvchi
rasm tiniq ko'rinishini so'ragan; o'qilishi yozuvning O'Z qora
soyasi bilan ta'minlanadi.

### Pleyer oynalari

Tartib: **Ma'lumot | Qismlar | Bo'limlar**, ochilganda Ma'lumot
turadi. Oynalar `PageView` bilan **qo'lda suriladi**; har biri
`_KeepAlivePage` ichida — bir marta qurilgandan keyin tirik
qoladi va surish paytida QAYTA QURILMAYDI (kuchsiz telefondagi
qotish aynan shundan edi). Tarix oynalarida ham xuddi shunday
(`AutomaticKeepAliveClientMixin` + qatorlarda `RepaintBoundary`).

Ma'lumot oynasida: yuqorida **Baholash** (10 ta yulduz) va
**Sevimlilarga qo'shish**; tagida bo'limning raqamlari
(ko'rishlar, tomosha vaqti, sevimlilar, reyting, `N-bo'lim ·
M qism`, qo'shilgan sana); undan keyin to'liq ma'lumot.

Pleyer bilan qism o'tkazish tugmalari **orasida** — hozir qaysi
bo'lim va qism ko'rilayotgani, u necha marta ko'rilgani, qancha
vaqt tomosha qilingani va qachon qo'shilgani.

### Pleyer QAYSI qismni ochadi

`_autoOpenEpisode` / `_resumeTarget` (`video_player_screen.dart`):

1. tarixdan kelingan bo'lsa — AYNAN o'sha qism, o'sha vaqtdan
   (`startEpizodId` / `startAt`);
2. shu bo'limning tarixda yozuvi bo'lsa — o'sha qism, to'xtagan
   joyidan;
3. aks holda — eng birinchi qism.

Sifat ham tiklanadi: tarixdagi `last_quality` shu qismda mavjud
bo'lsa, video aynan o'sha sifatdan ochiladi (`_restoreQuality`).

**OFLAYNDA HAM OCHILADI (2026-09 da o'zgardi).** Ilgari bu yerda
`if (_offline) return;` turardi va tarixdagi kadr bosilganda
oflaynda hech nima ochilmasdi. Endi qism to'liq yuklab olingan
bo'lsa ochiladi; aks holda `_getUrl` bo'sh qaytaradi va pleyer
o'rnida "Ko'rmoqchi bo'lgan qismni tanlang" yozuvi qoladi.
Avtomatik ochish internet holati ANIQLANGUNCHA baribir kutadi
(`_connectivityKnown`) — qaysi sifat ochilishi shunga bog'liq.

Qism qo'lda tanlanganda ham nuqta `_savedPositionOf` orqali
topiladi: avval `WatchProgress` (aniqroq), bo'lmasa tarixdagi
nuqta — ya'ni ilova qayta o'rnatilgan bo'lsa ham qism kelgan
joyidan ochiladi.

## Tekshiruv (har bir o'zgarishdan keyin)

```
flutter analyze          # 0 muammo bo'lishi kerak
flutter test             # test/ — ism va username qoidalari
cd rust && cargo test --lib     # 15/15 o'tishi kerak
cd worker && cargo check --target wasm32-unknown-unknown
```

## PLEYER VA YUKLAB OLISH — IKKI MUSTAQIL TIZIM

Bu loyihadagi **eng muhim qoida**. Buzilsa, foydalanuvchi darhol
sezadi: video o'zi yuklab olina boshlaydi.

| Holat | Manba | Diskka yozadimi |
|---|---|---|
| Fayl **100%** diskda | Mahalliy server (`127.0.0.1`) | Yo'q (faqat o'qiydi) |
| Fayl to'liq emas | **Faqat** worker `/api/play/...` | **Yo'q** |
| Internet yo'q + fayl to'liq emas | Ijro etib bo'lmaydi | — |

**Mahalliy server (`serve()`) TARMOQQA UMUMAN CHIQMAYDI.** U:

- bo'laklarni faqat **diskdan** o'qiydi (`read_cached_chunk`);
- faylning hajmini ham faqat **diskdagi `meta.json`** dan oladi;
- bo'lak yoki `meta.json` topilmasa — **404** qaytaradi va ilova
  workerga o'tadi.

Ilgari `serve()` yetishmayotgan bo'lakni tarmoqdan olib diskka
yozardi. Natijada "videoni ko'rish" amalda "yuklab olish"ga
aylanardi: foydalanuvchi tugmani bosmagan bo'lsa ham video diskka
yozilardi; videoni o'chirgandan keyin esa u butunlay qaytadan
yuklanardi.

Qoidani **`keshdan_bir_javobda_va_sek_bosimiga_bardosh`** testi
qo'riqlaydi: mahalliy serverga yuklab olinmagan video so'ralganda
manba jurnali **bo'sh** qolishi shart.

**To'liq yuklanganlikni faqat DISK hal qiladi**
(`RustCore.videoIsComplete` — diskni skanerlaydi). Ekrandagi hisob
(`DownloadManager.statOf`) bu qarorda **ishlatilmaydi**: u bir necha
soniya eskirgan bo'ladi va aynan shu o'chirilgandan keyingi qayta
yuklanishga olib kelgan edi.

## Manba ALMASHISHI (mahalliy <-> worker)

Manba video ochilganda bir marta tanlanadi va keyin **kuzatib
boriladi** (`_checkSourceSwitch`, har 2 soniyada):

- onlayn ko'rilayotganda fayl **to'liq yuklab olinsa** — aynan
  o'sha joydan mahalliy serverga o'tadi;
- mahalliy ko'rilayotganda fayl **o'chirilsa** — aynan o'sha joydan
  workerga o'tadi.

Tekshiruv arzon: avval xotiradagi hisob (ishora) ko'riladi, faqat
u **o'zgarganda** disk skanerlanadi.

## VIDEO TEZ OCHILISHI: "keshda ko'rilgan" belgisi

Ilova ilgari HAR SAFAR `/api/warm` ga so'rov yuborib javobini
kutardi — hatto kesh tayyor bo'lganda ham. O'lchandi: bunday
"bo'sh" so'rov data-markazdan **0.26-0.63 s**, telefonda mobil
tarmoqda **1-3 s**; `/api/play` ning o'zi esa atigi **0.3 s**.

Endi isitish muvaffaqiyatli tugaganda diskka belgi yoziladi
(`w<N>.warm`, 6 soat yashaydi). Belgi yangi bo'lsa ilova
**kutmaydi**: pleyerni darhol ochadi, isitishni fon'da ishga
tushiradi. Kesh kutilmaganda o'chgan bo'lsa — pleyer xatoga chiqadi
va odatdagi tiklanish yo'li oynani isitib, o'sha joydan qayta
ochadi.

FFI: `rust_video_cache_window_seen(url, widx)`.

## Internet uzilishi

- Onlayn ijro paytida internet uzilsa pleyer **o'ldirilmaydi**;
  ekranda xabar chiqadi va joriy nuqta eslab qolinadi.
- Internet qaytishi bilan video **o'sha nuqtadan avtomatik** davom
  etadi (`_onNetworkBack`), "takroriy xato" hisoblagichi esa nolga
  tushadi — internetning yo'qligi pleyerning nosozligi emas.

## KIRISH TOKENI DISKDA (AES-256-GCM)

**TOPILGAN MUAMMO.** Kirish tokeni faqat XOTIRADA turardi.
Foydalanuvchi Telegramga o'tganda xotirasi kam telefonlarda Android
ilovani BUTUNLAY yopib qo'yishi mumkin — token yo'qolardi va qaytib
kelgan odam kira olmasdi. U qaytadan urinardi, ilova esa HAR SAFAR
serverdan YANGI token so'rardi, har bir START esa serverda YANGI
SESSIYA ochardi. Natija: foydalanuvchi bir marta ham kira olmagani
holda "Qurilmalar" ro'yxatida **4 ta sessiya**.

**YECHIM — ikki tomondan.**

*Ilova tomonda.* Token diskka AES-256-GCM bilan MUHRLANGAN faylga
yoziladi (`pending_login.bin`). Kalit asosiy kalitdan (Android
Keystore) HKDF orqali olinadi, ya'ni fayl boshqa qurilmada ham,
ilovadan tashqarida ham ochilmaydi.

- `rust_secure_save` / `_load` / `_clear` — `rust/src/crypto.rs`;
- `AuthService.start()` muddati tugamagan tokenni **qayta
  ishlatadi** — ortiqcha sessiya umuman ochilmaydi;
- `AuthService.restore()` va ilovaga qaytishda
  (`RootScreen.didChangeAppLifecycleState`) `resumePendingLogin()`
  chaqiriladi: START bosilgan bo'lsa hisob **o'zi** ochiladi;
- kirilgach yoki 5 daqiqa o'tgach fayl o'chiriladi.

**QOIDA:** shifrlash o'chiq bo'lsa (asosiy kalit hali o'rnatilmagan)
fayl **umuman yozilmaydi**. Sessiya tokenini ochiq matnda diskka
yozgandan ko'ra, kutilayotgan kirishni yo'qotgan yaxshi.

Test: `cargo test --lib maxfiy_fayl` — diskdagi faylda token ochiq
matnda YO'Qligi ham tekshiriladi.

*Worker tomonda.* `create_session` endi ayni shu qurilma (nomi +
tizimi bir xil) uchun eski yozuvni oldindan o'chiradi — bitta
telefon ro'yxatda HAR DOIM bitta qator egallaydi. Busiz takroriy
urinishlar 4 ta chegarani to'ldirib, foydalanuvchining BOSHQA
haqiqiy qurilmalarini chiqarib yuborardi.

## ISM VA USERNAME — AVTOMATIK BERILADI, HECH NARSA SO'RALMAYDI

Telegramdan faqat `telegram_id` (hisobni tanish uchun), til va
premium belgisi olinadi. **Ism va username Telegramdan
OLINMAYDI.**

- Yangi hisobga nomni **server o'zi qo'yadi**: bazada band
  bo'lmagan **eng kichik** raqamdan `User 7` (ism) va `user_7`
  (username), `profile_done = 1`. Ya'ni foydalanuvchi START
  bosgan zahoti ilovaga kiradi — hech qanday oyna chiqmaydi;
- bo'sh raqamni `next_user_slot` bitta SQL so'rovida topadi
  (`worker/src/lib.rs`). Hisob ID'sining o'zi ishlatilmaydi:
  hisob o'chirilsa ID bo'shaydi va o'chirilgan odamning nomi
  yangi odamga tushib qolardi;
- nomsiz qolgan **eski** hisoblarga `init_db` bir marta
  `UPDATE OR IGNORE ... username='user_'||id` bilan nom beradi;
- foydalanuvchi ikkovini ham profil kartasining **o'ng yuqori
  burchagidagi tahrirlash tugmasi** orqali o'zgartiradi
  (`lib/screens/profile_edit_screen.dart`);
- keyingi kirishlarda `upsert_user` ism/username ustiga
  **yozmaydi** — aks holda tanlangan nom har safar qaytib
  qolardi.

### Ism qoidasi

- eng ko'pi **20 ta belgi**; emoji va istalgan belgi mumkin;
- 20 tani **ilova** sanaydi (`AuthService.nameProblem`, `characters`
  paketi — bitta emoji bitta belgi). Serverdagi chegara faqat
  suiiste'molga qarshi (160 ta Unicode kodi): Rustning `chars()`
  emojini bir necha kod deb sanaydi, ya'ni u yerda 20 deb
  qo'ysak, 4 ta emojili ism ham rad etilardi.

### Username qoidalari (IKKI JOYDA bir xil)

`worker/src/lib.rs` → `username_problem` va
`lib/services/auth_service.dart` → `AuthService.usernameProblem`.
**Birini o'zgartirsangiz ikkinchisini ham o'zgartiring.**

- 3–15 belgi (yuqori chegara — foydalanuvchi talabi);
- faqat `A-Z a-z 0-9 _`. Klaviaturada ham boshqa belgi
  yozilmaydi (`FilteringTextInputFormatter`), ya'ni emoji va
  bo'shliq umuman kirmaydi;
- takrorlanmaydi. Qiyoslash registrga bog'liq emas
  (`LOWER(username)`), bo'sh nomlar indeksga kirmaydi:
  `CREATE UNIQUE INDEX ... WHERE username <> ''`.

Real vaqtda tekshirish: `GET /api/auth/username-check?u=...`.
Ilova har bir belgida emas, yozish to'xtaganidan **350 ms** keyin
so'raydi va kechikib kelgan javobni (nom o'zgargan bo'lsa)
e'tiborsiz qoldiradi — aks holda 15 harf 15 ta so'rov bo'lardi va
javoblar tartibsiz kelib natijani chalkashtirardi.

`profile_done` endi hamma hisobda 1 — maydon eski ilova
versiyalari bilan moslik uchun qoldirilgan, unga qarab hech
qanday oyna ochilmaydi.

## HISOBNI O'CHIRISH VA HISOBDAN CHIQISH

Ikkalasi ham **progress chizig'i bilan** ko'rsatiladi
(`_TaskDialog`, `profile_screen.dart`) — foydalanuvchi talabi.

### CHIQISH (`logout`)

1. `syncBeforeLogout()` — navbat MAJBURIY yuboriladi;
2. chiziq 100% ga yetgach: **"Hammasi saqlandi — ma'lumotlaringiz
   sinxronlandi. Sizni ilovamizda kutib qolamiz!"**;
3. keyin `logout()`.

**Internet yo'q bo'lsa chiqish BLOKLANMAYDI.** Oyna "ma'lumotlar
telefonda saqlanadi va keyingi kirishingizda yuboriladi" deydi va
**Baribir chiqish / Qayta urinish / Bekor qilish** tugmalarini
beradi. Navbat hisobning `accountid_<id>` papkasida qoladi.

**Chiqishda telefonda HECH NARSA o'chmaydi** (eski qoida o'z
kuchida): o'sha hisobga qaytilsa hammasi joyida turadi.

### HISOBNI O'CHIRISH (`POST /api/auth/delete-account`)

Ilovada **ikki marta** so'raladi; ikkinchi oyna oqibatlarni
ro'yxat qilib ko'rsatadi.

**TARTIB QAT'IY:** 1) B2'dagi profil rasmi → 2) hisoblagichlarni
tuzatish → 3) qolgan hamma yozuv → 4) telefondagi nusxalar.

Nega rasm birinchi: bazadagi yozuv B2'dagi faylga **yagona
havola**. Avval hisob o'chsa va keyin fayl o'chmay qolsa, uni endi
hech kim topa olmaydi — fayl omborda abadiy yotib, pul yeb turadi.
Shu sabab rasm o'chishi **tekshiriladi** (`b2_delete_checked`):
o'chmasa hisobga umuman tegilmaydi va 502 qaytadi.

#### Nima o'chadi, nima qoladi

| O'CHADI (odamning o'zi) | QOLADI (tarixiy jamlanma) |
|---|---|
| `users_db`, `sessions_db`, `login_tokens` | `stats_hourly` / `stats_daily` |
| `watch_history_db` | `epizod_db.views_total`, `watch_ms_total` |
| `favorites_db` | `season_db.views_total`, `watch_ms_total` |
| `ratings_db` | |
| `sync_batches` | |
| B2'dagi avatar | |

**TUZATILADI** — hozirgi holatni sanaydigan raqamlar:
`season_db.fav_count`, `rating_sum`, `rating_count`
(`MAX(... - ?, 0)` bilan, bo'limga bitta UPDATE).

**Nega baho ham o'chadi** (foydalanuvchi topgan xato): bir odam
hisobini 3-4 marta o'chirib, har safar yangi hisobdan baho bersa
reyting soxtalashadi. Odamlarning 90% i hisobni o'chirmaydi —
ilovani o'chiradi yoki shunchaki chiqib ketadi, ya'ni bu yo'l
ataylab suiiste'mol uchun ochiq qolardi.

**Navbat yuborilmaydi, tashlab yuboriladi** (`SyncQueue.wipe`):
bir soniyadan keyin baribir o'chadigan yozuvni yozishning ma'nosi
yo'q.

#### Telefonda nima tozalanadi (`AccountData.wipeDevice`)

Hisob papkasi (tomosha tarixi, sevimlilar, kadrlar, trafik
hisobi, sozlamalar, navbat) → yuklab olingan videolar
(`videoCacheWipe`) → posterlar keshi (`libCachedImageData`).

## QAYSI TELEGRAM BILAN KIRISH

Telefonda bir nechta Telegram bo'lishi mumkin (Telegram, Telegram X,
Plus Messenger...). Ilgari havola `url_launcher` orqali tizimning
STANDART ilovasiga ketardi — foydalanuvchi tanlay olmasdi.

Endi `lib/services/telegram_apps.dart`:

- `kKnownTelegramApps` — ma'lum paketlar ro'yxati;
- `installed()` — qaysilari o'rnatilganini bilib oladi;
- `openWith()` — havolani ANIQ paketga yuboradi.

Bitta bo'lsa to'g'ridan-to'g'ri ochiladi, bir nechta bo'lsa ro'yxat
chiqadi. Tanlov shu seans uchun eslab qolinadi.

**MUHIM:** Android 11+ da ilova boshqa ilovaning borligini faqat
manifestdagi `<queries>` ro'yxatidagilar uchun bila oladi. Paketlar
CI'da (`build-flutter-apk.yml`) `<package>` sifatida qo'shiladi —
ro'yxatga yangi ilova qo'shsangiz, **o'sha yerga ham qo'shing**.

## PROFIL RASMI (foydalanuvchi o'zi tanlaydi)

Profildagi rasm ustiga bosilsa galereya ochiladi. Yo'l anime
rasmlari bilan **bir xil**: ilova faylni B2'ga to'g'ridan-to'g'ri
yuklaydi (`/api/upload-token`), keyin workerga faqat **fayl nomini**
aytadi (`POST /api/auth/avatar`). Rasm baytlari worker orqali
o'tmaydi.

- Fayl nomi qolipi **`avatar_<foydalanuvchi id>_<vaqt>.jpg`** —
  worker uni `valid_avatar_file` bilan tekshiradi. Busiz kimdir
  o'z profiliga masalan `anime_17.jpg` ni bog'lab, keyingi
  almashtirishda worker o'sha anime rasmini B2'dan **o'chirib**
  yuborardi.
- Eski rasm B2'dan **butunlay** o'chiriladi — lekin faqat yangisi
  bazaga saqlangandan **keyin** (saqlash yiqilsa foydalanuvchi
  rasmsiz qolmasin).
- Nom har safar yangi (ichida vaqt belgisi bor), shu sabab eski
  rasm keshda qolib ketmaydi.
- `users_db.avatar_file` bo'sh bo'lsa — Telegram avatari
  (`/api/avatar/:id`) ko'rsatiladi.

`users_db` ga ikkita ustun qo'shildi: **`balance`** (profildagi
"Balans:" qatori) va **`avatar_file`**. Ular `init_db` da
`ALTER TABLE` bilan, **har biri alohida** yuboriladi: Turso
to'plamdagi birinchi xatodan keyin qolganini bajarmaydi, ya'ni
ikkovi bitta to'plamda bo'lsa ikkinchisi hech qachon yaratilmasdi.

## Qayerda to'xtaganini eslab qolish

`lib/services/watch_progress.dart` — barcha nuqtalar bitta JSON
ro'yxatda (`watch_positions`). Boshidagi va oxiridagi chegaralar
qism uzunligining 10% i (ko'pi bilan 15 / 30 soniya — yuqoridagi
"QANCHA KO'RILSA" bo'limiga qarang); yozish **har soniyada,
darhol diskka**
(faqat telefon xotirasiga — serverga umuman yuborilmaydi).

## TANBAL (LAZY) OYNA KESHLASH — eng muhim qoida

Fayl **480 MiB**lik "oynalarga" bo'linadi (`WARM_WINDOW`; Rust va
worker'da **aynan bir xil** bo'lishi shart). Oynalar **ketma-ket
emas, faqat kerak bo'lganda** keshlanadi:

| Qachon | Nima bo'ladi |
|---|---|
| Video ochilganda | Faqat **#0** oyna keshlanadi. Foydalanuvchi shuni kutadi. |
| Ijro oyna chegarasiga 64 MiB qolganda | Keyingi oyna **fon'da** keshlanadi (kutish sezilmaydi). |
| Hali keshlanmagan joyga sek qilinganda | Avval o'sha oyna keshlanadi, **keyin** sek bajariladi. |
| Foydalanuvchi oxiriga bormasa | Oxirgi oyna B2'dan **hech qachon** o'qilmaydi. |

**B2'ga so'rov faqat isitish (`/api/warm`) paytida, oynasiga bir
marta ketadi.** Boshqa hech qaysi yo'l B2'ga chiqmaydi. Yangi kod
yozganda bu qoidani buzmang.

Tegishli FFI (`rust/src/video_cache.rs` → `lib/services/rust_bridge.dart`):

- `rust_video_cache_prepare` / `_prepare_status` — #0 oynani tayyorlash;
- `rust_video_cache_warm_window(url, widx)` — oynani fon'da keshlash;
- `rust_video_cache_window_status(url, widx)` — 0 ketyapti / 1 tayyor /
  2 yiqildi / 3 boshlanmagan;
- `rust_video_cache_total(url)`, `rust_video_cache_window_size()` —
  ilova oyna chegarasini shular bilan hisoblaydi.

Test: `cargo test --lib tanbal` →
`tanbal_keshlash_faqat_kerakli_oynani_oladi`.

## NEGA BUFER TOZALANMASLIGI MUHIM

`/api/play` keshda yo'q joy so'ralganda **503** qaytaradi. ExoPlayer
uchun bu **qaytarib bo'lmaydigan** xato: pleyerni butunlay qaytadan
ochishga to'g'ri keladi va **yig'ilgan butun bufer yo'qoladi**
(foydalanuvchi buni "sek qilsam video qaytadan sekin ochiladi" deb
ko'radi).

Shu sabab `lib/screens/video_player_screen.dart` da:

- ilova pleyerdan **oldinda yuradi** (`_startWindowPrefetch`,
  `_ensureWindowFor`) — 503 umuman yuz bermaydi;
- qotish belgisida avval **yengil turtki** (`_nudgePlayer`:
  `seekTo` + `play`) sinaladi — bufer saqlanadi;
- pleyerni qaytadan ochish (`_recoverPlayer`) — **oxirgi chora**, va
  undan oldin `_handleFatalError` kerakli oynani keshga oldiradi.

Yangi "tuzatish" qo'shayotganda pleyerni qaytadan ochish yo'lini
kengaytirmang — avval oldini olishga harakat qiling.

## 480 MiB'dan katta fayllar

`worker/src/lib.rs` → `stitched_response`: javob bir nechta oynadan
**oqim bilan** ulanadi (uzunlik oldindan to'g'ri e'lon qilinadi).
Busiz ExoPlayer javobning tugashini "fayl tugadi" deb tushunadi va
video 480 MiB'da to'xtab qolardi. Keyingi oyna hali keshda bo'lmasa
javob **kutadi** (B2'ga chiqmaydi).

## Yuklab olish

- Navbat diskda saqlanadi (`download_queue.json`) — ilova o'ldirilsa
  ham yuklash o'zi davom etadi;
- internet qaytganda kutish darhol bekor qilinadi
  (`DownloadManager._resumeActive`);
- diskdagi bo'lak **1 MiB** (foiz va "to'xtagan joydan davom" shunga
  tayanadi), bitta so'rovdagi eng katta oraliq esa **64 MiB**
  (`DL_REQUEST_CHUNKS`, worker'dagi `RANGE_MAX` bilan bir xil);
- **oqimlar soni 16** (`DOWNLOAD_THREADS`). O'lchov: bitta ulanish
  mobil tarmoqda atigi ~0.4-0.5 MB/s beradi (yo'l kechikishi sabab),
  shu sabab umumiy tezlik deyarli TO'G'RIDAN-TO'G'RI oqimlar soniga
  proporsional. Qurilmadagi o'lchov: 6 ta oqim = 3 MB/s (oxiriga
  borib 0.5), 12 ta = 5 -> 3.5-4 MB/s, 16 ta = ~6.5 MB/s kutiladi.
  Bundan ko'proq qilish ma'nosiz — kanal (8 MB/s) to'lgach oqim
  qo'shish faqat xotira va batareya sarflaydi.

### Ish qanday taqsimlanadi (`Work` + `claim_next`)

Oynadagi yetishmayotgan bo'laklar **bitta umumiy kursorda** turadi;
oqimlar ishni **kerak bo'lganda, kichik ulushlar bilan** oladi:

| Qoida | Nima qiladi |
|---|---|
| **Adil ulush** | Hech bir oqim qolgan ishning `1/oqimlar` ulushidan ko'pini olmaydi — oxirida bitta oqimda katta ish qolib ketmaydi. |
| **Vaqtga moslashish** | Ulush oqimning O'Z tezligiga qarab ~`CLAIM_TARGET_SECS` (5 s) lik ish qilib olinadi. Sekin ulanish kichik ulush oladi. |
| **Pastki chegara** | Ulush `CLAIM_MIN` (4 MiB) dan kichik bo'lmaydi — mayda so'rovlar uchun yo'l vaqti bekorga sarflanmasin. Kichik faylda chegara o'zi kichrayadi. |
| **Qaytarish** | Javob yarmida uzilsa, ulushning olinmagani navbatga qaytariladi (`return_work`) va uni birinchi bo'sh oqim oladi. |

**NEGA O'ZGARDI (foydalanuvchi: "3 -> 2 -> 1 -> 0.5 MB/s").** Ilgari
ish `DOWNLOAD_THREADS` ta teng **yo'lakka** (`Lane`) bo'linardi va
yo'lagi tugagan oqim boshqasining ishini o'g'irlashi mumkin edi —
lekin faqat HAVODA BO'LMAGAN qismini. 166 MB fayl = 166 bo'lak,
6 ta yo'lak = ~28 bo'lak, bitta so'rov esa 64 bo'lakkacha: ya'ni
**har bir oqim o'z yo'lagini bitta so'rovda olib qo'yardi** va
o'g'irlash uchun hech narsa qolmasdi. Ishi tugagan oqim butunlay
chiqib ketardi, faol oqimlar soni 6 -> 5 -> ... -> 1 ga tushardi va
tezlik ham aynan shunga proporsional pasayardi. Yo'lak tuzatishi
faqat ~400 MB'dan katta fayllarda ishlardi.

Testlar: `yuklab_olish_bitta_sekin_ulanishdan_sudralmaydi` (bitta
sekin ulanish butun yuklashni sudramaydi + ekrandagi tezlik
o'lchovi), `yuklab_olish_yolaklar_bilan_takrorsiz_ketadi` (qoplama:
takror ham, bo'shliq ham yo'q), `yuklab_olish_oxirigacha_parallel_ketadi`.

### Kesh chetiga chiqib ketgan oyna

Worker javobida `X-Cache: MISS` yoki `HIT-RANGE` bo'lsa — javob
isitilgan oynadan EMAS, ya'ni Cloudflare oyna yozuvini o'chirgan va
qolgan yuklash sekin yo'ldan ketadi. Endi ilova buni sezib oynani
**qayta isitadi** (`note_cold_window`), lekin B2 puli uchun QATTIQ
cheklangan: bitta oyna uchun `REWARM_COOLDOWN` (5 daqiqa) ichida
eng ko'pi bir marta. Test: `oyna_keshdan_tushsa_qayta_isitiladi`.

### Oyna chegarasi (480 MiB) endi to'xtatmaydi

Oynadagi **oxirgi ulush olingan zahoti** keyingi oyna fon'da
isitila boshlaydi (`warm_window_bg`). Ilgari chegarada hamma oqim
to'xtab, 480 MiB B2'dan keshga ko'chguncha kutib turardi. Tanbal
keshlash qoidasi buzilmaydi: isitish baribir faqat o'sha oynaga
o'tish oldidan boshlanadi.

### Ekranda tezlik

`rust_video_cache_stats` javobiga `speed` (bayt/soniya) va
`streams` (faol oqimlar soni) qo'shildi; qism qatorida
"1080p / 47% / 78 / 166MB · 5.8 MB/s" ko'rinadi. Sabab: ilgari
ekranda faqat foiz bor edi va "sekinlashdi" degan gapni tekshirib
bo'lmasdi.

- worker keshdan 64 MiB beradi, B2'dan esa 8 MiB (`B2_RANGE_MAX`) —
  xotira uchun. Xotiradan beriladigan javoblar `X-Cache: MISS` yoki
  `HIT-RANGE` bilan keladi va ilova ulardan chegara "o'rganmaydi";
- server chegarasi (`SERVER_SPAN_MAX`) 5 daqiqada unutiladi — bitta
  noxush javob ilovani abadiy sekinlashtirmaydi.

## Javob uzunligi E'LON QILINISHI SHART (`fixed_length_stream`)

`Response::from_stream` javobni `Transfer-Encoding: chunked` bilan
yuboradi va qo'lda yozilgan `Content-Length`ni runtime tashlab
yuboradi. Jonli o'lchovda bu og'ir nuqsonga olib kelgani aniqlandi:
bitta 16 MiB so'rovning javobi **10 urinishdan 5-6 tasida
0.6-1.5 MB da jimgina uzilib** qolardi — na xato, na belgi. Ilova
esa buni "shuncha ekan" deb qabul qilib, qolganini qayta-qayta
so'rardi.

Shu sabab **keshdan kesib beriladigan HAR BIR javob**
`fixed_length_stream` quvuridan o'tkaziladi (`play_from_warm_cache`,
`b2_proxy_range`ning ikkala kesh yo'li, `b2_proxy_full`). Shunda
runtime `Content-Length`ni o'zi qo'yadi va javob erta uzilsa mijoz
buni DARHOL xato deb ko'radi. Xotiraga hech narsa yig'ilmaydi.

Ilova tomoni baribir bardoshli: uzilgan javobdan olingani diskda
qoladi va keyingi urinish aynan o'sha joydan davom etadi — buni
`javob_yarmida_uzilsa_ham_yuklash_tugaydi` testi qo'riqlaydi.

## Miqyos (yuz minglab foydalanuvchi)

- **YOZUV** — hammasi `POST /api/sync` orqali, paket bo'lib
  ("YAGONA YOZUV YO'LI" bo'limiga qarang). 100 ming faol
  foydalanuvchida Turso xarajati ~$25/oy;
- `ensure_db` — jadval yaratish buyruqlari izolyat umrida **bir
  marta** (avval har bir so'rovda 10 ta DDL Turso'ga ketardi);
- ro'yxat so'rovlari (`/api/anime`, `/api/seasons/...`,
  `/api/epizods/...`) chekkada **30 soniya** keshlanadi, yozishdan
  keyin darhol tozalanadi (`purge_list_cache`).

**Hali hal qilinmagan:** Cloudflare keshi har bir data-markazda
alohida. Ya'ni bitta epizodni ko'p mamlakatdan ko'rishsa, oyna har
bir data-markaz uchun B2'dan alohida o'qiladi. Buni butunlay yo'q
qilish uchun Cache Reserve yoki R2 kerak bo'ladi — bu arxitektura
o'zgarishi, foydalanuvchi bilan kelishilmagan.

## TELEGRAM ORQALI KIRISH

Foydalanuvchi hisobi **faqat Telegram bot orqali** ochiladi —
parol, SMS, email yo'q.

### Oqim

```
Ilova                          Worker                      Telegram
  │ POST /api/auth/telegram/start
  ├────────────────────────────>│ 16 xonali token yaratadi
  │<─── token + deep_link ──────│ login_tokens (pending, 5 daq)
  │ t.me/arumedialoginbot   (bot manzili)?start=<token>
  ├─────────────────────────────────────────────────────────>│
  │                             │<── POST /api/telegram/webhook
  │                             │  users_db: topadi yoki yaratadi
  │                             │  sessions_db + login_tokens:
  │                             │    BITTA to'plam so'rovida
  │ GET /api/auth/telegram/status?token=...  (har 0,8 sek + resumed)
  ├────────────────────────────>│
  │<── session + user ──────────│
```

### BOT TEZLIGI — BAZAGA NECHA MARTA BORILADI

Botning "sekinligi" Telegramda emas, **bazaga ketma-ket
borishlarda** edi. Cloudflare chekkasidan Turso'ga har borish
~100 ms, START bosilgandan keyin esa ular ketma-ket ketardi:

| Ilgari | Hozir |
|---|---|
| `SELECT` login_tokens | o'sha-o'sha (1) |
| `SELECT` users_db + `UPDATE` users_db | bitta `UPDATE ... RETURNING *` (1) |
| `SELECT MAX(id)` sessions_db | yo'q — ID `INSERT` ichida hisoblanadi |
| `DELETE` eski qurilma | hammasi bitta `turso_batch` (1) |
| `INSERT` sessiya | ⤷ o'sha to'plamda |
| `DELETE` chegaradan ortig'i | ⤷ o'sha to'plamda |
| `UPDATE` login_tokens approved | ⤷ o'sha to'plamda |
| **~8 borish** | **3 borish** |

Ikkita qoida:

1. `turso_batch` endi **har bir buyruq natijasini tekshiradi** va
   xatoda `Err` qaytaradi. Ilgari javob umuman o'qilmasdi —
   sessiya yozilmagan bo'lsa ham kirish tokeni "tasdiqlangan"
   bo'lib qolishi mumkin edi. Turso to'plamdagi **birinchi
   xatodan keyin qolganini bajarmaydi**, ya'ni tartib muhim:
   sessiya `INSERT` i tasdiqlashdan OLDIN turadi.
2. Ilova natijani **0,8 soniyada** bir so'raydi (ilgari 2 sek).

### BOTDAGI YOZUVLAR

Foydalanuvchi botdan **texnik atama ko'rmasligi kerak**. Har bir
xabar ikki narsani aytadi: NIMA bo'ldi va ENDI NIMA QILISH kerak.
Matnlar bitta joyda — `MSG_HELP`, `MSG_BAD_LINK`, `MSG_EXPIRED`,
`MSG_ALREADY`, `MSG_TRY_LATER` (`worker/src/lib.rs`). Ilgari
xatolik matni to'g'ridan-to'g'ri chiqarilardi
(`❌ Xatolik: Telegram xatosi (sendMessage): ...`) — bu
foydalanuvchiga hech narsa tushuntirmaydi, faqat qo'rqitadi.

Kirish muvaffaqiyatli bo'lganda xabar **ID va username** ni ham
ko'rsatadi va ularni qayerdan o'zgartirishni aytadi.

### Jadvallar (`init_db` ichida, alohida `turso_batch`)

| Jadval | Vazifasi |
|---|---|
| `users_db` | `id` = **oxirgi id + 1** (anime/epizod bilan bir xil tartib), `telegram_id` UNIQUE |
| `login_tokens` | bir martalik 16 xonali token, 5 daqiqa yashaydi |
| `sessions_db` | sessiya jurnali: hisob + **qaysi qurilma** (`device`, `platform`, `app_version`). `session_token` ustunida tokenning **SHA-256 xeshi** turadi, tokenning o'zi EMAS |
| `app_config` | webhook siri va manzili |

`init_db` ning yuqori qismida `ALTER TABLE ... ADD COLUMN` bor va u
ustun mavjud bo'lganda xato beradi — shu sabab kirish jadvallari
**alohida** `turso_batch` chaqiruvida yuboriladi.

### SESSIYA TOKENI BAZADA OCHIQ SAQLANMAYDI

**TOPILGAN XAVF:** `sessions_db.session_token` da tokenning O'ZI
turardi. Baza oqib ketsa (yoki `TURSO_TOKEN` qo'lga tushsa)
hujumchi barcha faol sessiyalarni o'sha zahoti egallardi.

Endi bazada faqat **SHA-256 xeshi** saqlanadi (`token_hash`).
Ilova xom tokenni yuboradi, worker xeshlab solishtiradi.

* Tuz (salt) YO'Q va kerak emas: token 64 bayt tasodifiy
  ma'lumot, lug'at hujumi unga ta'sir qilmaydi.
* **Eski sessiyalar uzilmaydi:** `session_user` avval xesh bilan
  qaraydi, topilmasa ochiq token bilan qaraydi va qatorni o'sha
  zahoti xeshga o'tkazadi. Migratsiya o'z-o'zidan bo'ladi.
* `login_tokens.session_token` — **ataylab xom** qoladi: u ilovaga
  tokenni bir marta yetkazish kanali. Qator 5 daqiqa yashaydi
  (`LOGIN_CLAIM_TTL_MS`), ya'ni ochiq ko'rinish oynasi shu bilan
  cheklangan.

**Bu yerni buzmang:** yangi joyda `session_token` bo'yicha qidirsangiz
qiymatni `token_hash()` dan o'tkazing, aks holda so'rov jim ishlamay
qoladi.

### YOZISHMANI KUZATISH: VERSIYA BO'YICHA (200 QATOR EMAS, 1 QATOR)

`chat_threads.chat_ver` — suhbatda **biror narsa** o'zgarganda
bittaga oshadigan son. Uzoq kutish (`/api/chat/wait?ver=N`) aynan
shu bitta sonni asosiy kalit bo'yicha o'qiydi.

**Nega:** ilgari har tekshiruvda oxirgi **200 xabar** o'qilib
(`CHAT_LIMIT`), ulardan 4 ta son hisoblanardi. Bitta kutish
so'rovi = 16 tekshiruv = ~3 200 qator. Chat ochiq bitta odam
sekundiga **~168 qator** o'qirdi — hech narsa bo'lmasa ham. Bu
Turso kvotasining asosiy yeyuvchisi edi va bekor turgan
foydalanuvchilar soniga proportsional o'sardi.

Versiya **oshiriladigan** joylar (hammasi shu ro'yxatda bo'lishi shart):

| Joy | Nima bo'ladi |
|---|---|
| xabar yuborish (2 ta upsert) | `INSERT` da `chat_ver=1`, `DO UPDATE` da `+1` |
| "o'qildi" belgisi | `+1`, lekin **faqat** `unread_* <> 0` bo'lsa |
| `refresh_thread` (o'chirishdan keyin) | `+1` |

**ENG MUHIM TUZOQ:** "o'qildi" dagi `<> 0` shartini olib tashlamang.
Usiz har ochilish versiyani oshiradi → ilova o'zgarish deb biladi →
qayta yuklaydi → yana "o'qildi" → **cheksiz aylanish**.

**Eski APK'lar uzilmaydi:** `ver` yubormagan mijozga eski (200
qatorli) yo'l o'z holicha ishlaydi.

Ustun mavjud bazaga **alohida** `ALTER TABLE` bilan qo'shiladi va
natijasi ataylab e'tiborsiz qoldiriladi — u umumiy `turso_batch`
ichida bo'lsa, "ustun bor" xatosi `DB_READY` ni o'rnatmay qo'yardi
va butun DDL har so'rovda qaytadan ketardi.

### EMOJI VA YARIM SHAFFOF MATN

Foydalanuvchi yozgan matn ko'p joyda `Colors.white.withValues(alpha: X)`
bilan chiziladi. `TextStyle.color` dan bo'yoq (Paint) yasaladi va
**rangli** emoji glifi ham o'sha alpha bilan chiziladi — natijada
emoji qora fon ichidan ko'rinib, **qoramtir** bo'lib qoladi
(alpha 0.55 da ayniqsa yaqqol).

Yechim — `lib/widgets/emoji_text.dart`: matn harf/emoji bo'laklariga
ajratiladi, emojidan alpha olib tashlanadi, harflar esa o'z xiraligini
saqlaydi. Bo'linish `characters` paketi bilan (grapheme cluster
bo'yicha) — aks holda `👨‍👩‍👧` yoki `👍🏽` o'rtasidan kesilardi.

**Qoida:** foydalanuvchi yozgan matnni alpha bilan chizsangiz
`Text` emas, `EmojiText` ishlating.

### 4 TA QURILMA CHEGARASI

Bitta hisobga eng ko'pi **4 ta** qurilma kira oladi. 5-chisi
kirganda `create_session` eng **oxirgi onlayn bo'lgan 4 tasini**
qoldiradi, qolgani (ya'ni eng oldin onlayn bo'lgani) o'chiriladi.
Tartib `last_seen_at` bo'yicha, u esa har bir `/api/auth/me`
so'rovida yangilanadi.

Chiqarilgan qurilma keyingi `/api/auth/me` da **401** oladi va
ilova o'zini avtomatik hisobdan chiqaradi (`AuthService.refresh`).
**FAQAT 401** hisobni o'chiradi — tarmoq xatosi yoki 500 emas,
aks holda internet uzilganda foydalanuvchi hisobidan chiqib
ketardi.

### Xavfsizlik qoidalari — BUZILMASIN

- **Bot tokeni ilovaga hech qachon tushmasligi kerak.** U faqat
  Cloudflare secret (`TELEGRAM_BOT_TOKEN`). APK ichidagi satrlarni
  har kim o'qiy oladi.
- **Telegram fayl manzili** (`api.telegram.org/file/bot<TOKEN>/...`)
  ichida bot tokeni bor. Shu sabab avatar `/api/avatar/:id` orqali
  **worker ichidan** uzatiladi va tashqariga faqat rasm baytlari
  chiqadi. Bu manzilni hech qachon javobga qo'shmang.
- Webhook `X-Telegram-Bot-Api-Secret-Token` sarlavhasi bo'yicha
  tekshiriladi. Sirni worker **o'zi** yaratadi (`app_config`) —
  qo'lda qo'shiladigan qo'shimcha secret yo'q.
- Sessiya tokeni javoblarda **qaytarilmaydi**:
  `/api/auth/sessions` uni `current: true/false` belgisiga
  aylantirib, ustunning o'zini o'chirib tashlaydi.

### Sozlash tekshiruvi

Kirish ishlamay qolsa BIRINCHI shu manzil ochiladi:

```
https://arumediatv.uzcom.workers.dev/api/auth/telegram/health
```

`"ok": true` — hammasi joyida. `bot_token_configured: false` bo'lsa
Cloudflare secret yo'q; `bot_reachable: false` bo'lsa token noto'g'ri
(BotFather'da tiklangan bo'lishi mumkin). Javobda hech qanday sir
ma'lumot yo'q.

### Webhook o'zini o'zi ro'yxatdan o'tkazadi

`ensure_webhook` birinchi `/api/auth/telegram/start` so'rovida
ishlaydi: worker o'z domenini so'rovdan biladi, shu sabab domen
o'zgarsa ham o'zi qayta ro'yxatdan o'tadi. Qo'lda `setWebhook`
qilish shart emas.

### Kesh bilan aloqasi

`main()` ichida kirish yo'llari (`/api/auth/`, `/api/telegram/`)
**yozish keshini tozalash**dan ATAYLAB ajratilgan. Aks holda har
bir kirish `/api/anime` va `/api/seasons` keshini kuydirib
yuborardi. Kirish javoblari `Cache-Control: no-store` bilan
keladi.

### Ilova tomoni

| Fayl | Vazifasi |
|---|---|
| `lib/services/auth_service.dart` | sessiya, hisob, qurilma ma'lumoti (`ChangeNotifier`) |
| `lib/widgets/telegram_logo.dart` | logotip — `CustomPainter`, hech qanday `assets/` yo'q |
| `lib/screens/telegram_login_screen.dart` | kutish ekrani (2 sek so'rov + `resumed`da darhol) |
| `lib/screens/sessions_screen.dart` | kirgan qurilmalar ro'yxati |
| `lib/screens/profile_screen.dart` | kirilmagan: **faqat** logotip + tugma; kirilgan: to'liq profil |

Sessiya tokeni `flutter_secure_storage` da (Android Keystore).
Hisob ma'lumoti ham keshlanadi — shu sabab **internetsiz** ham
profil ochiq turadi.

### Keyingi qadam (hozir QILINMAGAN — foydalanuvchi so'ragan)

Ko'rish tarixini (`lib/services/watch_progress.dart`) hisobga
bog'lash. Hozir progress faqat telefonda saqlanadi; serverga
bog'langanda telefon almashtirilganda ham tarix qolardi. Buning
uchun `users_db.id` bo'yicha yangi jadval (masalan
`watch_progress_db`) va `/api/progress` endpointlari kerak
bo'ladi. **Admin panel himoyasi ataylab qo'shilmagan** — ilova
hali sinovda, tayyor bo'lganda panel butunlay olib tashlanadi.

## APK VERSIYASI VA "ILOVA O'RNATILMADI" (2026-09)

### Ikkita raqam — ikkita boshqa vazifa

| raqam | qayerdan | kim ishlatadi |
|---|---|---|
| `versionName` (`0.0.1`) | `pubspec.yaml` | faqat **odam** ko'radi |
| `versionCode` (butun son) | `pubspec.yaml` dagi `+` dan keyin | **Android** — mavjud ilova ustiga yangilash qarorini shu bo'yicha qabul qiladi |

Hozir ikkalasi ham `pubspec.yaml` dan olinadi (foydalanuvchi
talabi: `version: 0.0.1+1`, ya'ni versionName `0.0.1`,
versionCode `1`). **Yangi APK chiqarganda `+` dan keyingi raqamni
oshirishni unutmang** — aks holda telefondagi ilova ustiga
yangisi tushmaydi.

Tarix: 265-build'gacha `versionCode = github.run_number` edi
(telefonlarga 265 gacha raqamlar o'rnatilgan), keyin u pubspec'ga
o'tkazilib 1 ga tushib ketdi. Ya'ni **telefonda eski ilova
tursa**, 1 kabi kichik raqam downgrade bo'ladi va rad etiladi.
Toza o'rnatishda raqamning kattaligi ahamiyatsiz.

### O'RNATILMASLIK SABABI: TELEFON XOTIRASI TO'LGAN EDI

**HAL QILINDI.** Foydalanuvchi: «telefon xotirasi to'lib ketibdi,
shunga shunaqa bo'lyapti ekan».

Ya'ni sabab APK'da ham, `versionCode` da ham, imzo kalitida ham
EMAS edi. Android o'rnatish uchun joy topa olmaganda
"Ilova o'rnatilmadi" deb, boshqa hech narsa tushuntirmasdan rad
etadi — aynan shu bo'lgan.

Quyidagi tekshiruv baribir foydali bo'ldi va saqlanadi.

**APK'ning o'zida nuqson yo'q** — build-297, 298 va 300 yuklab
olinib tekshirilgan:

* imzo sertifikati uchchalasida bir xil (`CN=AniRaxUz`),
  v1 + v2 + v3 imzo sxemalari joyida, APK Signing Block butun;
* `AndroidManifest.xml` 297 va 298 da **bayt-bayt bir xil**
  (SHA-256 mos), 300 da faqat versiya raqami farq qiladi;
* `minSdkVersion=24`, `targetSdkVersion=36`, paket
  `uz.arumediatv.soft`;
* `lib/` da faqat `armeabi-v7a`, `.so` fayllar **siqilmagan**
  (`method=0`) va **4096 ga tekislangan** — ya'ni
  `extractNativeLibs=false` talabi bajarilgan;
* ZIP butun: 92 ta yozuv, `MANIFEST.MF` da 89 ta `Name:`.

### IMZO KALITI

265-build'gacha har build **tasodifiy** debug kaliti bilan
imzolanardi (`$HOME/.android/debug.keystore` toza runner'da hech
qachon topilmasdi). Endi kalit repoda: `ci/release.keystore`
(`CN=AniRaxUz`, SHA-256 `8F:47:32:E1:B6:01:D6:34:...`), Secrets
qo'yilgan bo'lsa undan olinadi.

Boshqa kalitli APK eskisining ustiga tushmaydi
(`INSTALL_FAILED_UPDATE_INCOMPATIBLE`) — lekin **toza
o'rnatishda bu ham sabab bo'la olmaydi**.

### SERVER TOMONDAGI TUZOQ

`X-App-Version` ga `pubspec.yaml` dagi to'liq yozuv (`0.0.1+1`)
boradi va worker uni admin paneldagi **"ENG PAST VERSIYA"**
(`app_min_version`) bilan solishtiradi
(`worker/src/lib.rs` → `version_rank`). Versiyani
**pasaytirganda** panelda turgan chegarani ham pasaytirish kerak,
aks holda ilova o'rnatiladi-yu, har so'rov `426` bilan rad
etiladi. Diqqat: `version_rank` da build raqami `clamp(0, 999)` —
999 dan katta build raqami taqqoslashda farq qilmaydi.

## ILOVA BELGISI (ARU logotipi)

Qora plita, undan **ARU** harflari o'yib olingan. Barcha fayllar va
ularni qayta chiqarish tartibi: **`branding/README.md`**.

Muhim: `android/` papkasi har build'da `flutter create` bilan qaytadan
yaratiladi va u bilan birga Flutter'ning standart ko'k belgisi keladi.
Shu sabab `build-flutter-apk.yml` dagi «Ilova belgisini o'rnatish»
qadami har safar `branding/android-res/` ni ustiga ko'chiradi. Bu
qadamni olib tashlamang — belgi darhol standartiga qaytadi.

Belgi CI'da QAYTA CHIZILMAYDI: PNG'lar repoda tayyor yotadi, CI faqat
ko'chiradi. Logotip o'zgarsa `node branding/build-icons.js` ni
mahalliy ishga tushirib, natijani commit qilish kerak.

## Tegilmaydigan joylar

- `rust/src/video_cache.rs` ning bo'lak-keshlash va shifrlash qismi
  (sinovdan o'tgan, 15 ta test);
- `rust/Cargo.toml` dagi `panic = "abort"` — Rust tomonida panic
  bo'lsa butun ilova o'ladi, shu sabab Rust kodi juda ehtiyotkorlik
  bilan yozilgan.

## Pleyer mahalliy serversiz (Android)

- `packages/video_player_android` — video_player_android 2.12.2 nusxasi
  (`pubspec.yaml` → `dependency_overrides`). Qo'shilganlari:
  `AruDataSource` (JNI → `rust/src/player_source.rs`), `AruVideoAsset`
  (`aru://file/<nom>`), `AruLoadControl` (bufer: 20–40 s oldinga, 30 s orqaga).
- Pleyer diskdagi shifrlangan 1 MiB bo'laklarni o'zi o'qiydi; yo'q bo'lak
  Telegram'dan olinib diskka yoziladi, oldinga faqat 2 bo'lak tayyorlanadi.
- iOS'da hozircha eski yo'l (mahalliy HTTP manba).
- Mahalliy HTTP server hali yuklab olish, rasmlar va eskizlar uchun ishlatiladi.

## Botsiz kirish (Telegram Login) va Telegram ko'rinishidagi kirish oynasi

- **Botsiz kirish.** Telegram hisobi ulangach ilova foydalanuvchi nomidan
  `messages.requestUrlAuth` → `messages.acceptUrlAuth` qiladi
  (`rust_tg_url_auth`, manzil `$kApiBase/login`). Telegram imzolagan manzil
  `POST /api/auth/telegram/widget` ga yuboriladi. Worker
  (`verify_tg_login`) HMAC-SHA256 ni SHA256(bot_token) kaliti bilan
  tekshiradi, `auth_date` 5 daqiqadan eski bo'lmasin. Imzo bir martalik
  (`login_tokens` da `w:<hash>`). Keyin `create_session` chaqiriladi
  (4 qurilma chegarasi saqlanadi).
  - Talab: BotFather → `/setdomain` → `arugram.uzcom.workers.dev`
    (`https://` siz).
  - Bu yo'l ishlamasa eski bot orqali kirishga qaytiladi.
- **Sessiyalar bog'langan.** Telegram sessiyasi uzilsa
  (`_onSessionLost`), ilova hisobidan ham chiqiladi. Ilovadan chiqilganda
  Telegram sessiyasi ham uziladi (ilgaridan shunday edi).
- **Kirish oynasi Telegram'dagidek:**
  - to'q ko'k fon;
  - davlat tanlash sahifasi: barcha davlatlar
    (`lib/widgets/tg_countries.dart`), qidiruv bilan;
  - kod va raqam alohida maydonlarda, raqam shablon bo'yicha
    bo'laklanadi;
  - kod uchun raqam kataklari, xatoda chayqalish;
  - QR sahifasi 1-2-3 qadamlar bilan;
  - bosqichlar orasida yon tomonga siljish.
- **Davlat IP bo'yicha.** Davlat Telegram'ning `help.getNearestDc`
  javobidan olinadi (`rust_tg_nearest_country`), masalan `+998`.
  Foydalanuvchi uni istalgan payt o'zgartira oladi.

## Bot chatiga takroriy nusxalar va qayta yuklangan fayl

- **Sabablar:**
  1. Pleyer, yuklab olish va qayta urinishlar `prepare` ni bir vaqtda
     chaqirardi.
  2. 20 soniyalik kutish tugaganda eski chaqiruv to'xtamasdi, yangisi
     esa yana nusxa so'rardi.
  3. O'qish xatosidan keyin fayl 2 daqiqa chetlatilardi
     (`FAIL_COOLDOWN`). Fayl chatda topilsa ham u "yo'q" deb
     hisoblanardi.
  4. Qidiruv faqat hujjatlar ichidan qidirardi, oddiy videolar esa
     "video" turkumida.
  5. Chatni tekshirish tarmoq xatosi bilan tugasa ham nusxa
     so'ralardi.
- **Tuzatildi:**
  - Bitta nom uchun bitta tayyorlash (`_preparing`).
  - Yaqinda (10 daqiqa) yuborilgan fayl qayta so'ralmaydi — chat bir
    necha marta qaraladi (`_deliveredAt`).
  - Chat tekshirilmasa nusxa so'ralmaydi.
  - `remember` chetlatishni olib tashlaydi.
  - Videolar video filtri bilan ham qidiriladi.
- **Kalit:** chatda topilgan fayl kaliti sessiyada bir marta serverdan
  yangilanadi (`keys_only`, `_keysFresh`). Fayl qayta yuklangan bo'lsa
  telefondagi eski kalit bilan ochilmaydi.
- **Hajm:** pleyer Telegram'dagi haqiqiy hajmni ishlatadi
  (`cached_doc_size`). Hajm o'zgargan bo'lsa diskdagi eski bo'laklar
  tashlanadi.

## Tuzatish: video ochilmadi, bot nusxa yubormadi

- **Sabab.** Bot chatidagi eski nusxalar `messages.search` bilan
  qidiriladi, Telegram esa bu qidiruvni tez-tez cheklaydi. Qidiruv
  xatosi bot chatini tekshirishni butunlay yiqitardi. Oldingi
  tuzatishdan keyin esa tekshiruv yiqilsa nusxa umuman so'ralmasdi.
  Natijada video ochilmadi.
- **Tuzatish.**
  - Qidiruv xatosi endi e'tiborsiz qoldiriladi va faqat logga
    yoziladi.
  - Tekshiruv baribir yiqilsa, bot so'raladi. Takroriy nusxadan
    `_preparing` va `_deliveredAt` himoya qiladi.
- **Kirish.** Hisob yana faqat bot orqali tasdiqlanadi (foydalanuvchi
  talabi): `/start <token>`. `acceptUrlAuth` yo'li
  (`rust_tg_url_auth`, `/api/auth/telegram/widget`) olib tashlandi.
  Yuqoridagi "Botsiz kirish" bo'limi endi amal qilmaydi.

## Bot chati: qidiruvsiz, so'ralishi bilan nusxa, keyin tozalash

Foydalanuvchi talabi: "chatdan izlash juda sekin — olib tashla; video
so'ralishi bilan copy message ishga tushsin; pleyerdan chiqilganda yoki
internet qaytganda ilova o'z hisobi bilan chat tarixini tozalasin".

- Nusxa so'rashdan oldingi chat tekshiruvi olib tashlandi: `rust_tg_find`,
  `messages.search`, `keys_only` va `rust_tg_mark_read` endi yo'q.
  `/api/tg/deliver` darhol chaqiriladi.
- Rust faylni faqat chatning oxirgi 100 ta xabari ichidan topadi
  (`find_in_chat` — bitta `GetHistory`).
- Chatni ilova tozalaydi (`rust_tg_clear_bot_chat`,
  `messages.deleteHistory`). Tozalash quyidagi paytlarda ishlaydi:
  - pleyerdan chiqilganda (`unhold`, 2 soniyadan keyin);
  - internet qaytganda;
  - ilova ochilganda.
- Nusxa ishlatilayotgan yoki hozir so'ralayotgan bo'lsa (`_holders`,
  `_delivering`), tozalash kutadi. Foydalanuvchi yuklagan fayl bot uni
  kanalga ko'chirmaguncha chatdan o'chirilmaydi.
- Takroriy nusxadan himoya: bitta fayl uchun bitta tayyorlash
  (`_preparing`).

## Yuklash hammasi bot orqali, bir nechta ulanish, Telegram'dagidek halqa

- **Hamma fayl bot chatiga yuklanadi.** Bu admin'ga ham tegishli. Kanalga
  faylni bot ko'chiradi (`tg_user_media`), ilova esa `/api/tg/claim`
  bilan kutadi. Ilgari admin fayli to'g'ridan-to'g'ri kanalga ketardi va
  admin'ning Telegram hisobi kanalda bo'lmasa "Kanal topilmadi" xatosi
  chiqardi.
  - `/api/tg/admin/file` olib tashlandi.
  - `claim` shu nomdagi eski yozuvda (qayta yuklash paytida) kalit
    mos kelmaguncha "tayyor emas" deydi. Aks holda chat erta
    tozalanib, yangi fayl kanalga yetib bormasdi.
- **Tezlik.** `grammers` har bir DC ga bitta TCP ulanish ochadi —
  hamma qismlar bitta ulanishdan o'tardi. Endi fayl qismlari asosiy
  ulanish va yana 4 ta qo'shimcha ulanishga navbat bilan
  taqsimlanadi (`DL_CONNS`, `part_client`).
  - Hammasi bitta sessiya (bitta auth kalit) bilan ishlaydi.
  - Qo'shimcha ulanishlar DC ga faqat asosiy ulanish u yerda
    muvaffaqiyatli ishlagach ulanadi (`dl_dcs`). Aks holda har biri o'z
    kalitini yasab, ruxsatsiz qolardi.
  - `MAX_INFLIGHT` 24 ga, yuklashdagi `UP_WORKERS` 8 ga oshirildi.
- **Nusxa tezroq.** Video ochishdan oldin chat tozalanishi
  boshlanmaydi — faqat hozir ketayotgan tozalash tugashi kutiladi.
- **Halqa** (`SpinRing`):
  - yoy doim aylanib turadi va aylanish davomida uzayadi;
  - foiz o'lchangan tezlik bilan bir tekis o'sadi, haqiqiy qiymatdan
    o'zib ketmaydi.

## Tuzatish: boshqa hisobda yuklash — PEER_ID_INVALID

- **Sabab.** Botning `access_hash` i har bir Telegram hisobi uchun
  boshqacha, ilova esa uni xotirada saqlab qolardi (`bot_peer`). Hisob
  almashgach `messages.sendMedia` eski qiymat bilan ketardi.
- **Tuzatish.**
  - `forget_peers` bot manzilini va qo'shimcha ulanishlar uchun
    "tayyor DC" belgilarini (`dl_dcs`) tozalaydi. U kirilganda
    (`after_login`, `auth_dcs` bilan birga), sessiya o'lganda
    (`check_dead`) va chiqilganda chaqiriladi.
  - `sendMedia` baribir `PEER_ID_INVALID` bersa, bot qaytadan topiladi
    va o'sha yuklangan fayl bir marta qayta yuboriladi. Fayl qayta
    yuklanmaydi.

## Telegram'dagidek yozish paneli: emoji, GIF, stikerlar, ovoz, dumaloq video

- **Panel** (`lib/widgets/tg_composer.dart`) izohlarda va support chatda
  ishlaydi:
  - maydondagi 🙂 klaviatura o'rniga panel ochadi (balandligi —
    klaviaturaniki);
  - pastda "Emoji | GIF | Stikerlar" tugmalari, sahifalar yon tomonga
    suriladi;
  - Emoji sahifasi:
    - Unicode 13.1 gacha emoji (`tg_emoji_data.dart`,
      `emoji-test.txt` dan yasalgan);
    - yaqinda ishlatilganlar;
    - maxsus emoji to'plamlari — Premium bo'lmasa 🔒 bilan ko'rinadi,
      yuborilmaydi;
    - ⌫ tugmasi.
  - GIF sahifasi: qidiruv (`@gif`), saqlanganlar va mashhurlar.
  - Stikerlar sahifasi: yaqinda ishlatilganlar, sevimlilar, to'plamlar.
- **Ma'lumot** (`lib/services/tg_media.dart`) foydalanuvchining o'z
  Telegram hisobidan olinadi. Rust funksiyalari:
  - `rust_tg_premium`, `rust_tg_sticker_sets`, `rust_tg_sticker_set`,
    `rust_tg_custom_emoji`;
  - `rust_tg_saved_gifs`, `rust_tg_gif_search`;
  - `rust_tg_media_file` (fayllar diskda keshlanadi);
  - `rust_tg_send_gif`.
- **Xabar formati:**
  - stiker: `media_type=sticker`,
    `media_file=stk_<to'plam>_<hash>_<hujjat>` (u64 hex). Ko'ruvchi
    stikerni `getStickerSet` bilan o'zi oladi;
  - GIF: tayyor hujjat bot chatiga yuboriladi (qayta yuklanmaydi,
    shifrlanmaydi), bot uni kanalga ko'chiradi. Nomi `cmt_<id>_...mp4`
    yoki `chat_<id>_...mp4`, `media_type=gif`;
  - maxsus emoji: matnda `[ce:<hujjat id>:<emoji>]`. Yozish maydonida u
    rasm bo'lib ko'rinadi (`TgTextController`), `EmojiText` ham shunday
    chizadi.
- **Izohlar:** faqat matn, emoji, stiker va GIF — fayl va ovoz yo'q.
  `comments_db` ga `media_file`, `media_type` ustunlari qo'shildi.
  Eski bazaga `ALTER` bir marta ishlaydi, belgisi
  `app_config.mig_comment_media`.
- **Support chat** (Telegram'dagidek):
  - qator: `[🙂 Xabar 📎] (🎤/⏺/➤)`;
  - `tg_record_button.dart`:
    - qisqa bosish mikrofon ↔ kamera orasida almashtiradi;
    - bosib turish yozishni boshlaydi (150 ms), qo'yib yuborilsa
      yuboriladi;
    - chapga surilsa bekor qilinadi (`min(0.35·kenglik, 140) × 0.3`);
    - tepaga 57 dp surilsa qulflanadi.
    - Qiymatlar Telegram Android'ning `ChatActivityEnterView` idan
      olingan.
  - Dumaloq video (`camera` paketi, old kamera, ko'pi bilan 60 s):
    - `media_type=round`;
    - chatda doira bo'lib ovozsiz, takrorlanib o'ynaydi;
    - bosilsa boshidan ovoz bilan, atrofida progress halqasi;
    - fayl shifrlangan keshdan o'qiladi (`aru://`).
  - Stiker, GIF va dumaloq video pufaksiz chiziladi.
  - APK'ga `CAMERA` ruxsati qo'shildi (workflow).

## Stikerlar Telegram/Cherrygram kabi: rlottie + libvpx, panel qotmaydi

- **Qotish sabablari:**
  - `.tgs` Dart'dagi `lottie` paketi bilan UI oqimida o'qilib, o'nlab
    animatsiya bir vaqtda chizilardi;
  - har bir stiker fayli uchun `Isolate.run` bilan yangi isolate
    ochilardi;
  - to'plam butunligicha (`Wrap`) qurilardi.
- **Cherrygram (Telegram fork'i) qanday qiladi:**
  - `RLottieDrawable`: rlottie, 4 ta fon oqimi, kadrlar keshi;
  - `AnimatedFileDrawable`: FFmpeg + libvpx, `.webm` shaffoflik bilan
    (`gifvideo.cpp`: YUVA420P -> ARGB).
- **Endi bizda ham shunday:**
  - `rust/third_party/rlottie` (MIT) va `rust/third_party/libvpx` (BSD,
    faqat VP9 dekoder, sof C; sarlavhalar
    `configure --target=generic-gnu` bilan yasalgan) `build.rs` da `cc`
    bilan yig'iladi. Android'da C++ statik bog'lanadi
    (`c++_static`). armv7'da pixman assembleri NDK clang'ida
    yig'ilmaydi, shu sabab u yerda sof C yo'li (`-U__ARM_NEON__`).
  - `rust/src/sticker_anim.rs`: `rust_anim_open`, `_frames`, `_fps`,
    `_render`, `_close`. `.webm` uchun kichik EBML o'quvchi bor: rang
    `Block`/`SimpleBlock` da, shaffoflik `BlockAdditional` da.
    `native/vp9_shim.c` ikkala VP9 oqimini ochib, premultiplied RGBA
    beradi.
  - Testlar: Lottie kadri; `vpxenc` bilan yasalgan shaffof `.webm`
    (`src/testdata/alpha_sticker.webm`).
  - arm64 `.so` 3.9 MB dan 4.8 MB ga oshdi.
- **Dart tomoni:**
  - `lib/services/native_pool.dart` — doimiy ishchi isolate'lar:
    `io` (3 ta, tarmoq) va `render` (2 ta, kadrlar). `tgCall` endi
    shu hovuz orqali ishlaydi.
  - `TgAnimView`:
    - kadr fon isolate'ida chiziladi, bir animatsiyaga bir so'rov,
      ulgurmasa kadr tashlab o'tiladi;
    - 3 MB gacha kadrlar xotirada saqlanadi;
    - birinchi kadrlar umumiy keshda turadi;
    - panelda o'lcham ko'pi bilan 160 px va bir vaqtda 18 ta
      animatsiya.
  - Panel: faqat ko'ringan kataklar quriladi (`SliverGrid` + `_SetCell`),
    ko'rinmayotgan sahifada `TickerMode` o'chiq. Ustunlar Telegram'dagidek
    `kenglik / 45 dp` (emoji) va `/ 72 dp` (stiker).
  - `lottie` paketi olib tashlandi.

## Telegram'dagidek biriktirish oynasi (`tg_attach_sheet.dart`)

- Cherrygram `ChatAttachAlert` kabi yasalgan:
  - pastdan tortib kattalashtiriladigan oyna;
  - ilova ichidagi galereya to'ri (3 ustun), birinchi katakda jonli
    kamera — bosilsa suratga oladi;
  - videoda uzunligi ko'rsatiladi;
  - ko'pi bilan 10 ta narsa raqamli doira bilan tanlanadi;
  - tepada albom tanlash;
  - pastda "Galereya | Fayl" tugmalari; biror narsa tanlanganda ular
    o'rnida izoh maydoni va ➤ (soni bilan).
- "Fayl" bo'limi (`file_picker`): ichki xotira, siqilmagan rasm/video,
  musiqa.
- Paketlar: `photo_manager`, `file_picker`, `open_filex`. Workflow
  `READ_MEDIA_IMAGES/VIDEO/VISUAL_USER_SELECTED` ruxsatlarini
  qo'shadi.
- Yangi xabar turi `file`. `media_file` — `chat_<id>_...<ext>`, matnda
  asl nomi turadi. Puffakda belgi, nom va kengaytma ko'rinadi, bosilsa
  fayl mos ilova bilan ochiladi. Izoh birinchi rasm/videoga qo'shiladi.
- Kamera va fayl tanlagichi keshga nusxalagan fayllar yuborilgach
  o'chiriladi. Galereyadagi asl faylga tegilmaydi.

## Dumaloq video Telegram'dagidek (`tg_round_recorder.dart`)

- Cherrygram'ning `InstantCameraView` qiymatlari asosida:
  - chat ustida xiralashgan va qoraygan fon;
  - doira ekran qisqa tomonining ~92% i;
  - ochilishda doira 0.1 → 1 kattalashib, pastdan ko'tariladi;
  - atrofida oq, 3 dp qalinlikdagi progress yoyi (60 s);
  - doira ostida kamerani almashtirish tugmasi (`setDescription`,
    yozish to'xtamaydi);
  - yuborilganda doira kichrayib, chap pastga "uchadi".
- Doira barmoq bosilishi bilan chiqadi, kamera uning ichida ochiladi.
  Kamera ochilguncha barmoq qo'yib yuborilsa, yozuv osilib qolmaydi
  (`TgRecordButton._pressed`).
- Chatdagi dumaloq video ekran qisqa tomonining 60% i
  (`roundMessageSize`).

## Stiker/GIF oynasida ilova yopilishi — rlottie o'rniga tlottie

* Sabab: rlottie (LOTTIE_THREAD_SUPPORT'siz) bitta global rasterizatordan
  foydalanadi; ikki render izolyati bir vaqtda chizganda xotira buzilib,
  ilova native darajada yopilardi. Premium emoji ham shu sababli chizilmay,
  oddiy emoji ko'rinardi.
* Yechim: Telegram'ning o'z Lottie renderi — tlottie (github.com/dkaraush/tlottie,
  MIT, sof Rust) `rust/third_party/tlottie` ga nusxalandi (`SOURCE.md`).
  Har bir stiker o'z `CPURenderer` iga ega, global holat yo'q.
* rlottie va C++ (libc++_static) bog'lanishi olib tashlandi; libvpx (webm) qoldi.

## Stiker/emoji/GIF diskda keshlanadi + "Xotiradan foydalanish" oynasi

* Rust (`telegram.rs`): Telegram `MediaDataController` kabi — to'plamlar
  ro'yxati, to'plam ichi, saqlangan GIF'lar, maxsus emoji hujjatlari va
  premium holati `tg/media/meta/*.json` da (javob + hash + fayl havolalari).
  Yangi yozuv tarmoqsiz qaytadi, eskisi `hash` bilan tekshiriladi
  (`NotModified` — qayta yuklanmaydi), internet bo'lmasa keshdagisi.
  Fayllar (`tg/media/<id>`) diskda bo'lsa ulanishsiz darhol qaytadi.
* Profil → Xotira bosilsa `StorageScreen`: halqa diagramma, belgilanadigan
  toifalar (Videolar, Posterlar, Stikerlar va emojilar, GIF va chat
  fayllari, Vaqtinchalik), "Keshni tozalash <hajm>". Hisob ma'lumotlari
  tozalanmaydi.

## Stiker/GIF yuklanmasligi + Telegram'dagidek panel

* Sabab (topilgan): Telegram chaqiruvlari 3 ishchiga navbat bilan
  (band-bo'shiga qaramay) bo'linardi; sekin fayl yuklash ortida
  to'plamlar ro'yxati kutib qolardi. Endi fayllar alohida `NativePool.files`
  da, chaqiruv eng bo'sh ishchiga tushadi; Rust so'rovlarida 20 s chegarasi.
* Xato bo'lsa panelda matni ko'rinadi ("Xato: ..."); Rust panic sababi
  `last_crash.txt` ga yoziladi va keyingi ochilishda ekranda chiqadi.
* Panel Telegram `EmojiView` kabi: siljiydigan tanlov, "Qidiruv" qatori
  (emoji — kalit so'zlar `getEmojiKeywords`; stiker/GIF — ❤️👍👎🎉… tugmalari,
  `getStickers`/`@gif`), GIF'lar teng balandlikdagi qatorlarda, bosilganda
  kichrayadigan tugmalar, ⌫ animatsiyasi.
* Yozish tugmasi: 🎤 ↔ dumaloq video belgisi burilib-kattalashib almashadi,
  yozishda katta doira va halqa. Dumaloq video ochilmasa — sababi va qayta urinish.

## "Xotiradan foydalanish" oynasi Cherrygram'dagidek + stikerlar tozalanmasdi

* **Topilgan xato:** "Stikerlar va emojilar" hajmi butun `tg/media`
  dan sanalardi, "Keshni tozalash" esa `tg/media/meta` ni (to'plamlar
  ro'yxati, emoji kalit so'zlari, maxsus emoji hujjatlari — hajmning
  asosiy qismi) tashlab ketardi, shu sabab raqam kamaymasdi. Endi
  `tg/media` butunlay o'chadi; Rust ro'yxatlarni Telegram'dan qayta
  oladi. Ilova qayta ochilganda xotirada yo'q maxsus emoji hujjati
  `rust_tg_media_file` da ID bo'yicha qayta so'raladi
  (`GetCustomEmojiDocuments`). Posterlar: `aru_images/v2.index`
  (bo'sh keshda ~30 B) endi kesh hisobiga kirmaydi, yetim muhrlangan
  rasmlar ham o'chadi. O'chirish `_wipe` (alohida funksiya — `Isolate.run`
  yopilmasi ekrandagi `onProgress` ni ushlab qolmasin).
* **Oyna** (`storage_screen.dart`) — Cherrygram
  `CacheControlActivity` + `CacheChart` o'lchamlari bilan: halqa
  (200/172/38, 2° oraliq, zarrachalar, bosilgan bo'lak 9 ga
  kattalashadi va qatori yoritiladi), belgisi olingan toifa halqadan
  chiqadi, sarlavha + qurilma xotirasi chizig'i (`aru/storage` ->
  `stats`, `MainActivity.kt`), dumaloq belgilashli qatorlar, "Keshni
  tozalash / Tanlanganini tozalash" tugmasi, tasdiqlash, "Kesh
  tozalanmoqda" pardasi, bo'sh keshda yashil halqa "Xotira tozalandi".
  "Keshni avtomatik o'chirish" va "eng katta hajm" bo'limlari YO'Q —
  ularga mos ish ilovada hali yo'q.

## Emojilar — Telegram'niki (telefonniki EMAS)

* Oddiy emoji Telegram serverida yo'q — rasmlar ilovaning o'zida
  (rasmiy Telegram `emoji.pack`, Cherrygram `assets/emoji`). Ular
  `fonts/TgEmoji.ttf` (rangli CBDT, 3606 ta, ~7,4 MB) ga yig'ilgan —
  qanday: `tool/tg_emoji/README.md`.
* Butun ilovada zaxira shrift (`main.dart` -> `fontFamilyFallback`);
  emoji bo'laklarida asosiy shrift (`tgEmojiStyle`, `EmojiText`,
  `TgTextController.buildTextSpan` — yozish maydonida ham), panel
  kataklari ham shu shriftda.
* Panel ro'yxati Telegram tartibida (`EmojiData.dataColored`,
  `fixEmoji` bilan) — `tg_emoji_data.dart` avtomatik yasalgan.

## Chat Telegram'dagidek (2-bosqich)

* Dumaloq video: ExoPlayer'da `setEnableDecoderFallback(true)`
  (apparat dekoder ochilmasa dasturiysi), pufakcha xatoda bir marta
  o'zi qayta urinadi.
* Biriktirish oynasi (`tg_attach_sheet.dart`) — `ChatAttachAlert`:
  shisha tab (Lottie `assets/tg_anim/tab_*.json`), Galereya/Fayl/Musiqa,
  tanlashda 0.787 kichrayish, raqamli doira, izoh + ➤ nishoncha.
* Yozish tugmasi (`tg_record_button.dart`) — `RecordCircle`,
  `BlobDrawable`: 🎤↔📹 Lottie (`voice_and_video.json`), ovoz
  balandligiga qarab to'lqin (record `onAmplitudeChanged`), qulf,
  "0:03,45" taymer, yaltiroq "Bekor qilish uchun suring".
* Pufakchalar (`tg_bubble.dart`) — `MessageDrawable` dumi, guruhlash,
  vaqt matn oxirida, kun ajratgichi.

## Emoji/GIF/stiker paneli: silliqlik, xotira, xatolar (3-bosqich)

* Stikerlar harakatlanmasdi: panelning 18 ta "o'rni" yashirin sahifa
  (emoji to'plamlari, tepadagi belgilar) bilan band bo'lib qolardi.
  Endi `TgAnimView` TickerMode'ga qaraydi: yashirin — Rust tutqichi va
  kadrlar keshi bo'shaydi, ko'rinsa qayta ochiladi; kadrlar keshi
  umumiy 48 MB chegara bilan; panelda 30 kadr/s; tepadagi to'plam
  belgilari `frozen` (bitta kadr). `NativePool.render` — 3 ishchi.
* Faqat ekrandagilar: `cacheExtent` bir katak, tez surilganda yuklash
  kechiktiriladi (`_Deferred`, `recommendDeferredLoadingForContext`).
* Premium emoji/stiker/GIF yuklanguncha zaxira emoji EMAS — bo'sh joy.
* GIF paneli: kichik rasm, so'ng ko'rinayotgan GIF'ning o'zi o'ynaydi
  (bir vaqtda 4 ta, `_gifSlots`).
* Crash: Rust `panic = "unwind"` + `with_client`/`rust_anim_*` da
  `catch_unwind` — ichki xato ilovani yopmaydi, xato bo'lib qaytadi.
* "Boshqa stiker ketyapti": qayta ishlatilgan katak eski stikerni
  ko'rsatardi — `TgStickerRefView`/`TgStickerView`/`TgAnimView` endi
  kalit (key) bilan; `TgGifMessage` fayl almashganini sezadi.
* Izohdagi GIF ochilmasdi: bot kanalga ko'chirguncha kelgan so'rov
  faylni seans oxirigacha "yo'q" deb belgilardi — endi 20 s
  (`_ExpiringSet`), `TgGifMessage` 6 marta qayta urinadi.
* Hisob almashganda: `TgMedia.resetAccount` (xotiradagi ro'yxatlar,
  "yaqinda"lar fayli) + Rust `forget_media_lists` (`tg/media/meta`);
  panel sahifalari `accountChanged` bilan qayta quriladi.
* Panel ochilishi: `AnimatedSize` o'rniga o'lchami o'zgarmaydigan
  panel + `ClipRect/Align` (250 ms); klaviatura ochiq bo'lsa darhol
  almashadi; yopilganda `Offstage` (holati saqlanadi); 🙂↔⌨ —
  Telegram Lottie (`smile_to_keyboard.json`).

## Tuzatish: matnlar g'alati (raqamlar yo'q, bo'shliqlar keng)

Sabab — `TgEmoji` shrifti: telefonda "Roboto" topilmagach Flutter
hamma matnni ro'yxatdagi birinchi shrift (`TgEmoji`) bilan chizdi, unda
esa raqam va bo'shliq glifi bo'sh/keng edi. Tuzatish: shrift `cmap`
idan ASCII va boshqa oddiy belgilar olib tashlandi
(`tool/tg_emoji/README.md`), `fontFamilyFallback` =
`['sans-serif', 'TgEmoji']`. Galereyada video kichik rasmi chiqmasa
zaxira yo'llar (`thumbnailData`, birinchi kadr).

## Telegram bilan taqqoslash (4-bosqich)

* Emoji paneli (`EmojiView`, `EmojiTabsStrip`): bo'lim belgilari —
  Telegram Lottie (`msg_emoji_*.json`, tanlanganda o'ynaydi), tugma
  30 dp / oraliq 3 dp / burchak 8; sarlavhalar 15 qalin; teri rangi —
  bosib turilsa 6 variantli oyna (`tgEmojiColored`, `addColorToCode`).
* Dumaloq video (`InstantCameraView`): 180 ms decelerate, yarim
  balandlikdan ko'tariladi; tugmalar pastki chapda shisha panelda
  (kamera almashtirish aylanadi, chiroq: orqa — fonar, old — ekran oq);
  oyna faqat xabarlar ustida, yozish paneli ko'rinadi.
* Ovozli xabar (`SeekBarWaveform`): 44 dp tugma, to'lqin (3 dp qadam,
  2 dp chiziq, ±7 dp), yozishda yig'iladi va matnga `[wf:...]` bo'lib
  qo'shiladi (`tg_waveform.dart`); worker ro'yxatda "Ovozli xabar".
* Dumaloq video pufagi ovoz bilan o'ynaganda kattalashadi.
* Sarlavha (`ChatAvatarContainer`): 42 dp rasm, nom 18, holat qatori.
* Javob (`tg_reply.dart`): chapga surish yoki menyu → javob qatori;
  xabarda iqtibos (bosilsa asl xabarga o'tadi va yoritiladi); matnda
  `[re:<id>]` (worker ro'yxatda olib tashlaydi). Bosib turish menyusi:
  Javob berish, Nusxa olish, (admin) Tanlash, O'chirish. Pastga tushish
  tugmasi.

## 5-bosqich: chat bo'sh, qulflash, bot tarixi, qizish

* Support chat bo'sh ko'rinardi: dumaloq video oynasi yopiq holatda
  Stack'da 0 o'lchamli oddiy bola bo'lib qolar va ro'yxatni 0 kenglikka
  siqardi. Endi doim `Positioned.fill`, Stack esa `StackFit.expand`.
* Qulflash (`tg_record_button.dart`, Telegram `ChatActivityEnterView`):
  qulflangan bosishda barmoq ko'tarilsa endi YUBORILMAYDI (ilgari
  darhol ketib qolardi); bekor — `distCanMove` ning to'liq masofasi yoki
  qo'yib yuborishda < 0.45; chapga 30% dan ko'p surilganda qulflanmaydi;
  doira darhol ochiladi (kamera/mikrofon ochilishini kutmaydi), shu
  paytdagi surish/qulflash ham ishlaydi. Qisqa bosishda "bosib turing"
  maslahati; belgi animatsiyasi tizimdagi "animatsiyalarni o'chirish"ga
  qaramaydi.
* Bot tarixi: band yoki xato bo'lganda tozalash tashlab ketilmaydi —
  20/30 s dan keyin qayta uriniladi; Rust'da `revoke` rad etilsa o'z
  tomonidan o'chiriladi, uzilgan so'rov qayta yuboriladi.
* Emoji/GIF/stiker (`tg_media_view.dart`): umumiy soat (~30 Hz),
  hamma animatsiya ≤ 30 kadr/s, bir vaqtda 1..3 kadr chiziladi
  (yadroga qarab), panel emojisi 100 px / stiker 160 px / xabarda
  320 px, faqat ko'rsatiladigan kadrlar keshlanadi (56 MB), surish
  paytida yangi kadr chizilmaydi (`tgAnimScrolled`), kadr `setState`
  siz almashadi. GIF: kam yadroli telefonda 2 ta, surish to'xtagach
  ochiladi. Yuklash: 5 tagacha parallel, uzilgan bo'lak qayta so'raladi
  ("request error: dropped").
* Tozalash oynasi: Telegram `utyan_cache.json` (supurayotgan jo'ja),
  darhol chiqadi, kamida 1.6 s turadi.
* Yozish paytida butun chat har 100 ms qayta qurilmaydi (faqat doira);
  yuklanayotgan rasm pufak o'lchamida ochiladi.

## Tozalash oynalari Telegram'dagidek

* Tasdiqlash — Telegram `AlertDialog` (eni ≤ 356, sarlavha 20 qalin,
  matn 16, tugmalar bir qatorda o'ngda: "Bekor qilish" ko'k,
  "Keshni tozalash" qizil 12% fon bilan).
* "Kesh tozalanmoqda" pardasi to'liq ekran enida, balandligi 350
  (`ClearingCacheView`); tugagach pastda "... kesh tozalandi" xabari.

## GIF yuklanmasligi, stiker "2x", panel stikerlari, GIF har 10 s

* GIF (chat va izohlar): buzuq/yarim yuklangan fayl keshda qolib GIF
  hech qachon ochilmasdi — endi o'chiriladi va qayta yuklanadi; yuklash
  90 s dan osilsa qayta; 10 martagacha urinish, keyin ↻ (bosilsa qayta).
  Bir vaqtda ko'pi 6 ta pleyer (dekoderlar cheklangan). Bot chatini
  tozalash 40 s dan oshmaydi va fayl so'rovlari uni ko'pi 8 s kutadi
  (ilgari osilib qolsa hamma GIF/rasm "aylanib" qolardi).
* GIF'lar ko'ringach 1 marta o'ynaydi, keyin har 10 soniyada bir
  (`_PlayEvery`) — panelda ham, chatda ham.
* Stiker "qotib / 2x": kadr ulgurmaganda vaqt bo'yicha sakrash o'rniga
  ketma-ket keyingi kadr chiziladi (soat moslanadi); bir vaqtda 1..4
  kadr chiziladi.
* Stikerlar panelida stikerlar animatsiyalanmaydi (faqat birinchi kadr).

## Telegram'ga yaqinlashtirish: GIF to'g'ridan-to'g'ri, kadrlar diskda, kuchsiz telefon

* GIF: yuborishda fayl nomiga Telegram kaliti qo'shiladi
  (`..._g<id>_<access_hash>_<dc>_<file_reference>.mp4`, hex;
  `rust_tg_gif_token`). Ko'ruvchi GIF'ni o'z Telegram hisobi bilan
  to'g'ridan-to'g'ri Telegram serveridan oladi (`rust_tg_gif_direct`) —
  bot chati va worker ishtirokisiz. Havola eskirsa (yoki eski nom
  bo'lsa) avvalgi yo'l (bot chati) ishlaydi. Worker o'zgarmadi: nom
  `chat_{me}_` / `cmt_{me}_` bilan boshlanadi va ≤ 200 belgi.
* Stiker kadrlari diskda (`sticker_anim.rs`, Telegram `BitmapsCache`
  kabi): chizilgan kadr `deflate` bilan siqilib, tutqich yopilganda
  stiker yonidagi `<fayl>.<w>x<h>.afc` ga yoziladi; keyingi safar kadr
  chizilmaydi — diskdan ochiladi. Papkada jami 200 MB dan oshsa eng
  eskilari o'chadi; "Keshni tozalash" bilan birga o'chadi.
* Telefon kuchi (`device_perf.dart`, Telegram
  `getDevicePerformanceClass` kabi): yadrolar, eng yuqori chastota,
  xotira. Kuchsizda: panel emojilari harakatsiz, 1 ta kadr bir vaqtda,
  kichikroq o'lcham, 24 MB kesh, panelda 1 GIF, GIF faqat 1 marta.

## O'chirish tezligi, bot chati ekrandan chiqqach, worker so'rovlari

* O'chirish (izoh va support chat): Telegram'dagidek darhol ekrandan
  yo'qoladi, server javobi fonda; xato bo'lsa joyiga qaytadi.
  Worker: `chat_del_many` — bitta `DELETE ... RETURNING` (ilgari 3
  so'rov); B2/Telegram fayllari, suhbat qatori va izoh javoblari/
  layklari `ctx.wait_until` bilan FON'da — javob darhol qaytadi.
* Bot chati: pleyer va support chat yopilganda `screenClosed()` —
  2 soniyadan keyin tozalanadi (band bo'lsa bo'shashi bilan). GIF
  yuborilgach bot chati ko'pi 20 soniya band (ilgari 3 daqiqagacha).
* Worker'ga kamroq so'rov: "o'qilmagan" nuqtasi 12 → 45 s, ilova fonda
  yoki suhbat ochiq bo'lsa yuborilmaydi; ilovaga qaytishda `/auth/me`
  ko'pi 5 daqiqada, nuqta 20 soniyada bir; suhbatning uzoq kutishi
  ilova fonda to'xtaydi; GIF uchun `/api/tg/claim` so'ralmaydi.

## Stiker/GIF ko'rish oynasi (Telegram `ContentPreviewViewer`)

* Panelda stikerlar va GIF'lar harakatsiz (GIF — faqat kichik rasm,
  `TgGifThumb(play: false)`).
* Bir marta bosilganda — ko'rish oynasi (`tg_media_preview.dart`): fon
  xiralashadi, markazda katta stiker (tepasida emojisi) yoki GIF — shu
  yerda harakatlanadi (GIF to'xtovsiz); ostida "Stiker yuborish" /
  "GIF yuborish"; bo'sh joyga bosilsa yopiladi.
* Chat va izohlarga yuborilganlari avvalgidek harakatlanadi.
* Panel emojilari — ekrandagi hammasi harakatlanadi (kuchsiz telefonda
  ham).

## GIF, oldindan tayyorlash, premium emoji (admin), ovoz/dumaloq video, chat foni

* Yuborilgan GIF (chat, izohlar) to'xtovsiz takrorlanadi; 10 soniyalik
  tanaffus yo'q. Buzuq (MP4 emas) fayl keshda qolmaydi — o'chirilib
  qayta yuklanadi ("qorayib yotibdi").
* Oldindan tayyorlash: havola -> stiker hujjati va maxsus emoji hujjati
  diskda eslab qolinadi (`tg_known_docs.json`, `TgMedia.warmup`), fayl
  diskda bo'lsa navbatsiz darhol (`fileSync`) — stiker/emoji birinchi
  kadrdanoq chiziladi. Chat va izohlar yuklanganda oxirgi 40 xabardagi
  stiker, GIF (12 ta) va maxsus emojilar fonda tayyorlanadi
  (`tgPrefetch`).
* Admin (server tasdiqlagan) maxsus emojilarni Telegram Premium'siz
  yuboradi.
* Ovozli xabar / dumaloq video: oldingi fayl yuklanayotganda yozilgani
  endi tashlab yuborilmaydi — navbat bilan yuboriladi; uzunlik haqiqiy
  vaqtdan; o'zi yuborgani telefondagi asl fayldan darhol o'ynaydi
  (`VoicePlayer.localFiles`); dumaloq video kanalga hali ko'chmagan bo'lsa
  o'zi 6 martagacha qayta urinadi; yuklash progressi har bo'lakda butun
  ekranni qayta qurmaydi.
* Support chat: Telegram fon naqshi (`assets/tg_pattern.png`,
  `default_pattern.svg` dan) + aksentga mos gradient
  (`tg_chat_background.dart`), sarlavha va yozish paneli to'q kulrang
  (soya bilan), pufaklar: kiruvchi `#232120`, chiquvchi `#8C3A12`.

## Support chat — Telegram 12 ko'rinishi

* Sarlavha: fon ustida suzib turgan tabletkalar — chapda ←, o'rtada
  rasm (44) + nom (19, qalin) + holat (bosilsa profil), o'ngda ⋮.
  ⋮ menyusi: Qidiruv (pastda "3 / 7" va ↑↓, topilgan xabarga o'tadi),
  Profilni ko'rish (admin), Tanlash va Tarixni tozalash (admin).
  Tanlash rejimi ham tabletkalarda. Xabarlar sarlavha ostidan o'tadi.
* Yozish maydoni fon ustida suzib turgan tabletka (shaffof panel).
* Pufaklar: kiruvchi `#2A2420`, chiquvchi `#7A4A2A`; chiquvchida vaqt,
  ✓/✓✓ va to'lqin `#E2BE9C`; ✓✓ — Telegram'dagidek ingichka chiziqli
  (`_ChecksPainter`). Ovozli xabar tugmasi 48 dp, ochroq tusda.
* Foydalanuvchi ko'rinishida (1:1 chat) xabarlar yonida rasm yo'q
  (Telegram shaxsiy chati kabi); admin ko'rinishida qoladi.

## GIF — ilova ichidagi ffmpeg (Telegram `AnimatedFileDrawable` kabi)

* GIF (H.264 MP4) endi telefon video pleyeri bilan EMAS, ilova ichidagi
  ffmpeg H.264 dekoderi bilan ochiladi (`rust/third_party/ffmpeg`,
  ffmpeg 7.1.1, LGPL 2.1+; faqat libavcodec H.264 + libavutil, sof C,
  asm'siz; `SOURCES.txt` — yig'iladigan fayllar, `config.h` Android
  uchun tuzatilgan). MP4 konteyneri Rust'da o'qiladi
  (`sticker_anim.rs`: `parse_mp4`, `Kind::Mp4`), dekoder — C shim
  (`native/h264_shim.c`: YUV -> RGBA, "cover" kesish).
* GIF'lar stikerlar bilan bir dvigatelda: umumiy soat, ≤ 30 kadr/s,
  kadrlar diskda (`.afc`), dekoderlar soni cheklovi va qorayish yo'q.
  `TgGifThumb` (panel, ko'rish oynasi), `TgGifMessage` (chat, izohlar)
  — `TgAnimView(gif: true, height: ..)`; nisbat `rust_anim_probe` dan.
* Panelda stikerlar va GIF'lar yana harakatlanadi (Telegram kabi).
* Premium emoji tanlanganda yuborish tugmasi yonadi (maydonga dastur
  orqali yozilganda ham qayta chiziladi).
* LGPL: ffmpeg manbasi va litsenziyasi `third_party/ffmpeg` da; statik
  bog'langan — tarqatishda LGPL shartlariga e'tibor bering (o'zgartirilgan
  ffmpeg manbasi ochiq, foydalanuvchi qayta bog'lay olishi kerak).

## Tizim tugmalari ortida qora panel yo'q

* `MainActivity.onCreate`: navigatsiya/holat paneli shaffof,
  `isNavigationBarContrastEnforced = false` (Android 10+ 3 tugmali
  navigatsiya ortidagi qoramtir parda o'chadi), API 30+ da
  `setDecorFitsSystemWindows(false)`; Dart'da ham
  `systemNavigationBarContrastEnforced: false`.

## Android 15/16 planshet (CCTAB): ilova ishlamasligi — 16 KB sahifa

* Sabab (ehtimoliy, alomatlar mos): yangi 64 bitli Android 15/16
  qurilmalarda 16 KB xotira sahifasi; `librust_core.so` 4 KB ga
  tekislangan edi -> yuklanmaydi -> Telegram xizmati ishga tushmaydi,
  kirish oynasi "o'chiq" (maydon va tugmalar qoramtir).
* Tuzatish: `rust/.cargo/config.toml` — `-z max-page-size=16384`
  (va `common-page-size`); CI'da `llvm-readelf` bilan tekshiruv
  (arm64); APK `zipalign -P 16` (16 KB) bilan tekislanadi.
* Kirish oynasi: sozlama olinmasa 4 s dan keyin o'zi qayta so'raydi;
  sababi yoziladi (yadro ochilmadi / serverga ulanib bo'lmadi —
  internet va sana/vaqtni tekshiring / kirish yoqilmagan).

## Animatsiyalar to'g'ridan-to'g'ri ekranga (Texture) — silliq

* Sabab: har kadr Rust -> Dart isolate -> UI oqimi -> `ui.Image`
  yo'lidan o'tardi; ekrandagi o'nlab emoji/stiker/GIF UI oqimini band
  qilib, animatsiyalar sekin va uzuq-uzuq bo'lardi.
* Endi (Android): har animatsiyaga Flutter `Texture`
  (`SurfaceProducer`, `packages/video_player_android/.../AruAnimTextures.java`,
  kanal `aru/anim`) va Rust o'z oqimlarida (`rust/src/anim_player.rs`:
  1 rejalashtiruvchi + yadrolar/2 chizuvchi) kadrni `ANativeWindow` ga
  to'g'ridan-to'g'ri chizadi. Dart faqat o'ynat/to'xtat (`AnimPlayers`,
  `native_pool.dart`). ≤ 30 kadr/s, kechiksa sakramaydi, kadrlar
  diskda (`.afc`). Ilova fonga o'tsa yuza ajratiladi, qaytganda qayta
  ulanadi. Texture yo'li ishlamasa — avtomatik eski (Dart) yo'l.
* `TgAnimView`: `_openTex` / `_texPlay` / `_closeTex`; birinchi kadr
  yuzaga chiqqach ko'rsatiladi (qora miltillamasin).

## Kuchsiz (2 GB) telefon uchun 3 bosqich

1. Impeller o'chirildi (Skia) — CI manifestga
   `io.flutter.embedding.android.EnableImpeller=false` qo'shadi
   (flutter/flutter #148472, #153186, #183510: Android'da ko'p
   rasm/animatsiyada o'rta va arzon telefonlarda kadr tashlaydi).
2. Telegram `DrawingInBackgroundThreadDrawable` usuli: `TgAnimBatch`
   (tg_media_view.dart) — ichidagi animatsiyalar o'z yuzasini ochmaydi,
   o'rnini aytadi; guruh hammasini Rust'da bitta yuzaga chizdiradi
   (`rust_player_open_multi`, anim_player.rs `Item`). Emoji/stiker
   paneli qatorlari (`_gridRow`), GIF qatorlari, chat xabari va izoh
   matni — har biri bitta Texture (ilgari har animatsiya alohida).
   Faqat o'zgargan kadr qayta chiziladi.
3. Kuchsiz telefon (`DevicePerf.low`): 20 kadr/s
   (`rust_player_fps_cap`), guruh yuzasi 1.5x o'lchamda (aks holda 2x).

## Operativ xotira (RAM) tejash — 2 GB telefonlar uchun (2026-09)

TALAB: "Ilova kodini scannerlab chiq va RAM'ni yeydigan narsalarni
kamaytir yoki tuzat — pleyer bufer, rasmlar va boshqalar".

Topilgan va tuzatilganlar:

1. **Video pleyer buferi** (`AruLoadControl.java`): bayt chegarasi
   yo'q edi — yuqori bitreytli videoda 40 s oldinga + 30 s orqaga
   yuzlab MB bo'lardi. Endi oldinga 15..30 s, orqaga 10 s va
   **24 MB umumiy chegara** (`setTargetBufferBytes`,
   `prioritizeTimeOverSizeThresholds = false`).
2. **GIF/stiker kadrlari keshi** (`sticker_anim.rs`, `DiskFrames`):
   yangi chizilgan kadrlar tutqich yopilguncha XOTIRADA turardi (uzun
   GIF — o'nlab MB), yopishda `.afc` fayl yana ikki marta xotirada
   yig'ilardi. Endi kadr darhol vaqtinchalik `.spill` faylga yoziladi
   (xotirada faqat joyi), `.afc` esa oqim bilan yoziladi. Qolib ketgan
   `.spill`/`.part` fayllar (1 soatdan eski) tozalanadi.
3. **Flutter rasm keshi** (`main.dart`): odatiy 100 MB / 1000 rasm
   o'rniga kuchsizda 40 MB / 200, qolganida 80 MB / 500.
4. **To'liq ekran rasm** (`media_view_screen.dart`): rasm asl
   o'lchamida dekodlanardi (12 MP ≈ 48 MB). Endi ekran eni x2 gacha
   (720..2160 px).
5. **Fon isolate'lari** (`native_pool.dart`): kuchsiz telefonda
   11 o'rniga 6 ta (io 2, files 2, render 2).
6. **Animatsiya keshlari** (`tg_media_view.dart`): kuchsizda kadrlar
   keshi 24 → 16 MB, birinchi kadrlar 150 → 60 ta.

Tekshirildi (o'zgartirish kerak emas): video kesh yuklovchisi
(16 x 1 MB bufer), video/tarix eskizlari xotira keshi (chegaralangan),
chat va ro'yxat rasmlari (`memCacheWidth` bor).

## Stiker/GIF: Telegram kabi bosib turish, bir marta o'ynash, sifat, surish, APK nomi (2026-09)

TALABLAR (foydalanuvchi):
1. Panelda (to'plamlar) faqat GIF va emojilar harakatlansin; emoji,
   GIF va stiker ustiga BOSIB TURILGANDA ekran tepasida katta bo'lib
   chiqib animatsiyalansin.
2. Yuborilgan stiker va GIF ekranda to'liq ko'ringanda bir marta
   o'ynasin; ekrandan chiqib qayta ko'rinsa — yana bir marta.
   (Matndagi maxsus emojilar aylanib turaveradi.)
3. GIF va stikerlar sifati pasayib ketgan.
4. Izoh va support chatni surganda qotyapti.
5. APK nomida build raqami bo'lsin.

Qilinganlar:
- `tg_media_preview.dart` qayta yozildi: `TgHoldTarget` (bir bosish —
  darhol yuboradi, bosib turish — `TgHoldPreview`) va ekran tepasidagi
  katta ko'rinish; barmoq boshqa katakka surilsa almashadi, qo'yib
  yuborilsa yopiladi. Orqa fon blur'siz (kuchsiz telefon uchun).
- Stikerlar panelida stikerlar `frozen` (faqat birinchi kadr, Rust
  tutqichi darhol yopiladi). GIF va emojilar harakatlanadi.
- Rust `anim_player.rs`: `once` rejimi (oxirgi kadrda to'xtaydi) va
  `rust_player_replay`. Dart'da `_OnceWatch` — to'liq ko'ringanda
  qayta o'ynatadi; ustiga bosilsa ham (`_TapReplay`, ota-ona bosishini
  o'g'irlamaydi). `TgStickerRefView` va `TgGifMessage` — `once: true`.
- Sifat: xabardagi stiker 512 px gacha (ilgari 320), GIF 512 px
  (ilgari 360), panel kataklari kattaroq; guruh yuzasi 3x zichlikda
  (ilgari 2x). `h264_shim.c` — eng yaqin nuqta o'rniga bilinear,
  kuchli kichraytirishda 2x2 o'rtacha.
- Surish: guruh yuzasi endi butun pufakcha emas, faqat animatsiyalar
  egallagan to'rtburchak; tez surishda yuza ochish kechiktiriladi
  (`Scrollable.recommendDeferredLoadingForContext`); support chat
  so'rovi o'zgarish bo'lmasa `notifyListeners` chaqirmaydi (ilgari
  har bir necha soniyada butun chat qayta qurilardi); izohlar
  ro'yxatida `items` nusxasi har qator uchun olinmaydi; chat foni
  `isComplex` (rasterlab keshlanadi).
- CI: APK nomi `arugram-<admin-32|user-64>-build-<raqam>.apk`.

## Sek kutishi olib tashlandi (2026-09)

TALAB: "Sek qilgandagi kutish vaqtini olib tashla — endi video
xotiradan ko'rsatiladi, bu kerak emas".

`video_player_screen.dart`: sek buyrug'i 200 ms (ikki marta bosishda
320 ms) tinchlikni kutardi (`_seekIdle`, `_seekIdleTap`,
`_seekIdleTimer`). Endi olib tashlandi — `_requestSeek` darhol
`_commitPendingSeek` ni chaqiradi. Ketma-ket buyruqlarni `_runSeek`
yig'adi (bir vaqtda bitta `seekTo`, navbatda faqat eng oxirgisi).
+5/-5 ko'rsatkichi 700 ms turadi (`_seekBadgeHold`). Keshda yo'q
joyga sek qilinganda bo'lakni yuklab olishni kutish
(`_ensureWindowFor`) o'z joyida qoldi — bu tarmoqdan olish uchun.

## Avto-kodlash: asl video → H.265 sifatlar → Telegram → jurnal (2026-09)

TALAB (foydalanuvchi): epizod yuklash oynasiga "Original video yuklash"
tugmasi; video yuklangach GitHub Actions uni sifatlarga bo'lib
kodlasin (`anime` repodagi H.265 workflow), Telegram'ga yuklasin va
`epizod_db` ga yozsin. Asl video KANALGA emas, BOTGA yuklansin
(kelajakda boshqalar ham yuklaydi). Xavfsizlik: yuklangan vaqti
bo'yicha KETMA-KET, bittasi tugamaguncha keyingisi boshlanmasin, bir
sifat qayta-qayta kodlanmasin.

Qilinganlar:
- Ilova (`add_epizod_screen.dart`): "Asl video (avto-kodlash)" kartasi.
  Video bot chatiga yuklanadi (`orig_<foydalanuvchi id>_<anime>_<bo'lim>_<vaqt>.mp4`),
  "Saqlash"dan keyin `POST /api/encode/queue`. Tahrirlashda holat
  ko'rinadi (navbatda / kodlanmoqda, tayyor: ... / tayyor / xato +
  "Qayta urinish").
- Worker:
  * `epizod_db.origin_video` ustuni (`mig_origin_video`), `encode_jobs`
    jadvali (navbat: `queued_at`, `state`, `done`, `runner`,
    `lease_until`, `attempts`, `error`);
  * bot `orig_<uid>_` nomli faylni oddiy foydalanuvchidan ham kanalga
    ko'chiradi (`tg_user_media`);
  * `/api/encode/queue`, `/api/encode/status` (admin, ilova imzosi bilan);
    `/api/encode/peek|claim|heartbeat|quality|finish` (Actions,
    `X-Encode-Token` = `ENCODE_TOKEN`, ilova imzosi talab qilinmaydi);
  * navbatga qo'yilganda workflow'ni ishga tushiradi (`GH_TOKEN`).
- Xavfsizlik: bir vaqtda bitta "running" ish (boshqasi `busy`);
  tartib `queued_at`; tayyor sifat (`done`) qayta kodlanmaydi, yuklanib
  jurnalga yozilmay qolgani (`tg_files` dagi `ep_<a>_<s>_<e>_<q>_<queued_at>.mp4`)
  faqat jurnalga yoziladi; 30 daqiqalik ijara + heartbeat (run o'lsa
  keyingisi qolganidan davom etadi); eski run yozuvlari 409 bilan rad
  etiladi; 3 urinishdan keyin "error" (navbatni to'smaydi), adminga
  Telegram xabari; bir xil asl video qayta navbatga qo'yilmaydi.
- `.github/workflows/encode.yml` + `tool/encode/run.py`: navbat bo'sh
  bo'lsa ~10 s da tugaydi; ffmpeg libx265 (CRF 30/29/28/27, medium,
  hvc1, +faststart, AAC), upscale yo'q, davomiylik tekshiruvi,
  AES-128-CTR (ilova bilan bir xil) shifrlab kanalga yuklash.

KERAKLI SECRETS (GitHub → ARUGRAM → Settings → Secrets):
  PYRO_SESSION_B64_1/2/3 (anime repodagi — kanal egasi), TG_API_ID,
  TG_API_HASH, ENCODE_TOKEN (32+ tasodifiy belgi), ENCODE_GH_TOKEN
  (fine-grained token: faqat ARUGRAM, "Actions: Read and write"),
  ixtiyoriy API_BASE. Keyin worker'ni qayta deploy qilish kerak
  (`deploy-worker.yml` ENCODE_TOKEN va GH_TOKEN ni qo'yadi).

## Animatsiya faqat katta ko'rinishda; Actions faqat qo'lda (2026-09)

TALAB (foydalanuvchi): "emoji, GIF va stikerlar oddiy holatda umuman
animatsiyalanmasin — faqat ustiga bosib turganda va chatda bir bosganda
tepada ko'rsatilganda. Chat va izohlarda ham ko'ringanda
animatsiyalanmasin. Hozir panelni ochishim bilan qotib, ilova yopilib
ketyapti". "Actions'ni qo'lda ishga tushiraman, avto ishga tushmasin".

- `TgAnimView.live` (odatda `false`): hamma joyda faqat birinchi kadr —
  Rust tutqichi darhol yopiladi, `Texture`/guruh yuzasi, soat, kadrlar
  keshi umuman ishlatilmaydi (panel ochilganda o'nlab yuza ochilib,
  xotira to'lib ilova yopilardi). Harakat faqat `TgHoldPreview` da
  (`live: true`).
- Panelda GIF — faqat kichik rasm (to'liq fayl yuklanmaydi).
- Chat/izohlardagi stiker va GIF: bir bosilsa `TgHoldPreview.showTap`
  (alohida sahifa, istalgan joyga yoki "orqaga" bosilsa yopiladi).
- `encode.yml`: faqat `workflow_dispatch`; worker workflow'ni
  chaqirmaydi (`GH_TOKEN`/`ENCODE_GH_TOKEN` kerak emas).

## Ilova nomi ARUmediaTV, paket uz.arumediatv.com (2026-09)

TALAB: "Ilova nomini ARUmediaTV ga o'zgartir, papka manzilini
uz.arumediatv.com ga o'zgartir".

- `build-flutter-apk.yml`: manifest `android:label="ARUmediaTV"` (va
  tekshiruvi), Release nomi; `flutter create --org uz.arumediatv`,
  so'ng `applicationId` va `namespace` aniq `uz.arumediatv.com`,
  `MainActivity.kt` shu paketga (`kotlin/uz/arumediatv/com/`) ko'chiriladi.
- `main.dart`: `MaterialApp.title` — ARUmediaTV.
- DIQQAT: paket nomi almashdi — Android buni yangi ilova deb biladi,
  eski ARUGRAM ustiga yangilanmaydi (eskisini o'chirib o'rnatish kerak).
  APK fayl nomlari (`arugram-...-build-N.apk`) o'zgarmadi.

## Kirish oynasi: raqam yozib bo'lmasdi (telefon va planshet) (2026-09)

Belgi: "Telegram orqali kirish hozircha yoqilmagan" + maydonlar xira,
raqam yozilmaydi. Bu matn — server `/api/tg/config` javobida
`enabled: false` (worker'da `TG_API_ID`/`TG_API_HASH` yo'q yoki raqam
emas) yoki yadro sozlanmagani. Ilgari sababi farqlanmasdi, xato esa
jimgina tashlanardi.

- Raqam va davlat kodi maydonlari endi har doim yoziladi (faqat
  "davom" tugmasi sozlamani kutadi).
- Sabab aniq ko'rsatiladi: server sozlanmagan / yadro xatosi
  (`TelegramService.initError`).
- Worker (`tg_api_creds`): secret'ga ortiqcha matn bilan yozilgan
  qiymat ham o'qiladi (raqamlar va 32 belgili hex).

## GIF: maxfiy kanalsiz va B2'siz — faqat Telegram serveridan (2026-09)

TALAB (foydalanuvchi): "maxfiy kanalga GIF yuborish va undan yuklab
olish deb videolar ochilmayapti. GIF kanalga yuborilmasin, Telegram
serveridan olinsin — hech qanday B2 va maxfiy kanallarsiz".

- Yuborish (`support_chat_screen.dart`, `comments_tab.dart`): bot
  chatiga/kanalga hech narsa yuborilmaydi — xabarga faqat nom yoziladi,
  nomda GIF'ning Telegram kaliti (`gifNameTag`: id, access_hash, dc,
  file_reference). `TelegramService.sendGif` olib tashlandi.
- Olish (`_fetchChatFile`): faqat `_directGif` — ko'ruvchining o'z
  Telegram hisobi bilan to'g'ridan-to'g'ri. Bot chati orqali olish
  (`fetchBytes`) olib tashlandi: u bot chatini band qilib, tozalashda
  video nusxalariga xalaqit berardi.
- Cheklov: Telegram `file_reference` ni vaqt o'tib eskirtirishi mumkin —
  juda eski GIF xabarlari ochilmasligi mumkin (qayta urinish tugmasi
  bor). Kalitsiz eski GIF'lar (kanal orqali yuborilganlar) endi
  ochilmaydi.
- `TG_API_ID`/`TG_API_HASH` eskirgan edi — foydalanuvchi GitHub
  secret'larini yangilaydi, worker keyingi deploy'da oladi.

## Premium emoji — ilova obunasi bilan (2026-09)

TALAB: "premium emoji, GIF va stikerlarni ilovamizdan obuna sotib olgan
odam yubora olsin — Telegram Premium bo'lishi shart emas".

- `tg_composer.dart` (`_EmojiPage._load`): ruxsat endi
  `BillingService.active` (ilova obunasi) yoki admin; Telegram Premium
  tekshirilmaydi. Qulf/xabar matni yangilandi.
- GIF va stikerlar oldindan hech qanday premium tekshiruvisiz ishlaydi
  (yuborish Telegram'ga emas, bizning chat/izohlarga).
- (2026-09, davomi) GIF va stikerlar ham faqat ilova obunasi (yoki
  admin) bilan yuboriladi: `_TgMediaPanelState._allowed` — panelni hamma
  ko'radi, yuborishda obuna tekshiriladi (obuna yo'q bo'lsa "Profil →
  Obuna" xabari). Yuborishning boshqa yo'li yo'q (faqat panel).

## Emoji/stiker sekinligi, ekran qotishi, chatda "qora" (2026-09)

TALAB: "animatsiya o'chirilgan bo'lsa ham bosib turganda sekin, chatga
yuborilgani ishlamayapti, ekran qotib yotibdi, emoji va stikerlar juda
sekin yuklanyapti, chatdagilar qop-qora — Telegram'da hammasi ishlaydi".

TOPILGAN SABAB: harakatsiz holatda ham HAR bir emoji/stiker uchun to'liq
fayl yuklanib, birinchi kadr Rust'da (tlottie/VP9) chizilardi. Navbat
(`_AnimClock`) kuchsiz telefonda bir vaqtda BITTA kadr chizadi va yangi
kelgan katakni navbat boshiga qo'yadi — panel ochilganda yuzlab emoji
navbatga tushib, chatdagi stikerlar oxirida qolardi; birinchi kadr
tayyor bo'lguncha esa hech narsa chizilmasdi (qora/bo'sh).

- Panel, matndagi emojilar va yozish maydonidagi emojilar (`still`):
  Telegram'ning tayyor kichik rasmi (thumbnail, bir necha KB) —
  Rust'da chizish yo'q, to'liq fayl yuklanmaydi.
- Chatdagi stiker va katta ko'rinish: birinchi kadr / animatsiya
  tayyor bo'lguncha kichik rasm ko'rinib turadi (`_ThumbImage`,
  `TgAnimView` endi yuklanayotganda ham `fallback` ni ko'rsatadi).
- Chatdagi GIF olinmasa — sababi ekranda (`_gifErrors`).

## Asl videoni o'chirish tugmasi; sessiya secret'i tekshiruvi (2026-09)

- Ilova: "Asl video (avto-kodlash)" kartasida qolgan sifatlardagi kabi
  o'chirish tugmasi (tasdiqlash bilan). Worker `POST /api/encode/delete`
  (admin): `epizod_db.origin_video` tozalanadi, navbatdan olinadi
  (ishlayotgan run 409 olib to'xtaydi), kanal posti o'chadi
  (`tg_forget_file`). Tayyor sifatlar qoladi.
- `encode.yml`: log'da har bir `PYRO_SESSION_B64_n` uzunligi, boshi va
  oxiri chiqadi (yaroqli qiymatda har biri 2276 belgi) — ortiqcha matn
  qaysi secret'ga tushgani ko'rinadi. Birinchi urinishda jami 6974 belgi
  chiqdi (kutilgan 6828) — secret'larga 146 ta ortiqcha belgi tushgan.
- (2026-09, davomi) Sessiya secret'lariga nusxalashda ortiqcha matn
  tushib qolaverdi (jami 6974 belgi, kerak 6828). Foydalanuvchi talabi
  bilan sessiya `ENCODE_TOKEN` bilan shifrlanib repo'ga qo'yildi:
  `tool/encode/session.enc` (openssl AES-256-CBC, PBKDF2 200 000).
  `encode.yml` avval shu faylni ochadi, bo'lmasa `PYRO_SESSION_B64_*`.
  ENCODE_TOKEN almashtirilsa — faylni yangi kalit bilan qayta shifrlash
  kerak.

## Asl video kaliti bazada, kodlashdan keyin o'chirish, stiker xatolari (2026-09)

- `epizod_db.origin_key`, `encode_jobs.origin_key` (`mig_origin_key`):
  ilova navbatga qo'yishda asl videoning ochish kalitini ham yuboradi;
  `claim` avval kanal postidagi kalitni, bo'lmasa bazadagisini beradi.
  `origin_key` ro'yxat javoblarida yashiriladi (`hide_keys`).
- `finish` (hamma sifat tayyor): asl video kanaldan (`tg_forget_file`)
  va bazadan (`origin_video`, `origin_key`) o'chiriladi.
- `STICKERSET_INVALID` (panel va chatdagi stikerlar ochilmadi): eskirgan
  `access_hash` — Rust `load_set_hashed` o'rnatilgan to'plamlar
  ro'yxatidan yangisini olib bir marta qayta so'raydi (`fresh_set_hash`).
- Qotish: xato bo'lgan to'plam/fayl 30 s qayta so'ralmaydi (ilgari har
  katak qayta qurilganda yangi so'rov ketardi); fayl navbati endi
  oxirgi so'ralgandan (ekrandagidan) boshlaydi; kichik rasm olinmasa —
  to'liq faylning birinchi kadri.

## Kalit kanal postida yo'q; Turso o'qishlari kamaytirildi; ma'lumot doimiy (2026-09)

- XAVFSIZLIK: ilova (`upload_to_channel`) va Actions (`run.py`) kanal
  postiga endi faqat fayl nomini yozadi — ochish kaliti YO'Q. Kalit:
  ilova -> `/api/tg/claim`, Actions -> `/api/encode/quality` (`msg_id`
  bilan `tg_files` ga yoziladi).
- Turso (5 kunda 500 000 o'qish): sessiya izolyat xotirasida 60 s
  (`SESSION_MEMO`, chiqishda o'chadi); chat kutishi 16 x 1.2 s o'rniga
  4 x 5 s — o'qishlar ~4 barobar kam.
- "Keshni tozalash"da `tg/media/meta` (Telegram'dan kelgan ma'lumot)
  o'chmaydi — faqat fayllar.

- (2026-09) Katalog ro'yxatlari chekka keshda 1 soat (`EDGE_CACHE_SECONDS`), yozishdan keyin darhol tozalanadi va yangisi qayta keshlanadi; ilovaga 30 s. Bot tezligi — o'zgartirilmadi.

## Telegram premium emoji, GIF va stikerlar olib tashlandi (2026-09)

- Foydalanuvchi talabi: ilova o'zining premium emoji, GIF va stiker
  tizimini yasaydi. Faqat oddiy (Unicode) emojilar qoldi.
- Ilova: `tg_media.dart`, `tg_media_view.dart`, `tg_media_preview.dart`
  o'chirildi; panelda Emoji (oddiy + yaqinda ishlatilganlar) va bo'sh
  "GIF" / "Stikerlar" oynalari ("Tez orada") qoldi.
- Eski stiker/GIF xabarlari `MediaPlaceholder` bilan, eski `[ce:..]`
  belgilari oddiy emoji bo'lib ko'rinadi (`plainEmojiText`).
- Rust: `sticker_anim.rs`, `anim_player.rs`, Telegram stiker/GIF
  funksiyalari, libvpx, ffmpeg, tlottie o'chirildi; Java `AruAnimTextures`
  o'chirildi.
- Worker: izohga va chatga yangi stiker/GIF qabul qilinmaydi.
- Eski `tg/media` fayllari "Vaqtinchalik fayllar" bilan tozalanadi.

## KODLASH BOTI, ASL VIDEO KO'RSATISH VA CRON (2026-09-28)

**Talab:** ikkinchi Telegram bot — faqat admin, faqat bo'limi bor
animelarga avto-kodlash uchun qism qo'shadi; qism raqamini so'raydi;
bor qism almashtiriladi (qayta kodlanadi); sifatlar tayyor bo'lguncha
asl video ko'rsatiladi; cron har 10 daqiqada navbat va Actions'ni
tekshiradi.

**Bot** (`worker/src/lib.rs` → `encbot_*`, webhook
`/api/telegram/encode-bot`, secret `ENCODE_BOT_TOKEN`):
- Webhook o'zi o'rnatiladi (cron'da yoki birinchi `/api/auth|telegram`
  so'rovida), sir `app_config.encbot_secret`.
- Faqat `ADMIN_TELEGRAM_ID`, faqat shaxsiy chat.
- `/start` → anime (bo'limi borlar, 10 tadan sahifa) → bo'lim — tugmalar PASTDAN (reply keyboard, `is_persistent` yo'q — yashirish belgisi chiqadi). Anime tugmasi nomi bo'yicha topiladi (takror nomda `(#id)`), tanlangan anime `app_config.encbot_anime`; bo'lim tugmasi `N-bo'lim` raqami bo'yicha. Bo'lim
  `app_config.encbot_target` ga yoziladi; keyin yuborilgan yoki FORWARD
  qilingan har bir video navbatdagi qism (MAX+1) bo'ladi — raqam
  so'ralmaydi, reply shart emas (foydalanuvchi talabi; raqamni ilovadan
  o'zgartiradi). Webhook `max_connections: 1` — ketma-ket, raqamlar
  to'qnashmaydi. `/holat` — navbat. `GET /api/telegram/encode-bot` —
  token/webhook tekshiruvi.
- Video kanalga `copyMessage` (izoh = `orig_bot_<a>_<s>_<n>_<ms>.<ext>`),
  `tg_files` ga kalitsiz yoziladi. Bot kanalda ADMIN bo'lishi shart.
  Asosiy bot `orig_` postlari haqida adminga xabar yubormaydi.
- Qism bor bo'lsa: sifatlari tozalanib fayllari o'chadi, eski asl video
  o'chadi, navbat yangilanadi (eski run 409 bilan to'xtaydi).

**Asl video ko'rsatish** (`with_origin`): `epizod_db.origin_size`,
`origin_height` (migratsiya `mig_origin_meta`). Qism ro'yxatida asl video
balandligiga mos BO'SH sifat o'rnida (`origin_slot`, noma'lum — 720p)
beriladi; ilova o'zgarmagan. PUT bu qiymatni saqlamaydi (`origin_video`
bilan solishtiriladi). `orig_` fayllar ham obuna talab qiladi
(`/api/tg/deliver`). `finish` da `origin_video` tozalanadi va kesh
yangilanadi (`encbot_purge`). Qism o'chirilsa asl video va navbat ham.

**Cron** (`wrangler.toml` `*/10 * * * *`, `scheduled` → `encode_kick`):
navbatda ish (queued yoki ijarasi o'tgan running) bo'lsa, faol ijara
bo'lmasa va GitHub'da `encode.yml` run'i kutmayotgan/ishlamayotgan
bo'lsa — `workflow_dispatch` (`GH_ACTIONS_TOKEN`, repo `GH_REPO` yoki
`ogabekraximov650-del/ARUGRAM`). Ishga tushirilganda adminga xabar,
xato bo'lsa soatiga bir marta. Navbatga qo'yilganda ham darhol chaqiriladi.

**Kodlash tugagach tozalash** (`/api/encode/finish`, foydalanuvchi talabi):
asl video ustunlari (`origin_video/key/size/height`) tozalanadi, `encode_jobs`
qatori O'CHIRILADI (ilovadagi "Tayyor" belgisi endi chiqmaydi), `tg_files`
qatori va kanal posti o'chadi (`tg_forget_file`; asosiy bot o'chira olmasa —
kodlash boti).

**Tuzatish: bot yuklagan asl video ochilmasdi.** `/api/tg/deliver` nusxalarni
izohsiz (`remove_caption`) yuborardi, bot orqali kelgan videoning fayl nomi
esa Telegram'niki (`video.mp4`) — ilova (`doc_matches`) uni nom bo'yicha
topolmasdi. Endi `orig_bot_` fayllar alohida `copyMessages` bilan IZOHI
bilan yuboriladi (izohda faqat nom, kalit yo'q).

**Botda qismlar ro'yxati va almashtirish.** Bo'lim tanlanganda pastda:
tepada "➕ Yangi qism qo'shish", ostida qismlar (eng yangisi tepada, 3
tadan). Qism tugmasi — keyingi BITTA video o'sha qismni almashtiradi.
Holat: `encbot_season` (`a/s`), `encbot_target` (`a/s` qo'shish, `a/s/n`
almashtirish, bo'sh — tanlanmagan).

**Asl video izoh bo'yicha, qolganlari fayl nomi bo'yicha.** Worker hamma
`orig_` fayllarni bot chatiga izohi bilan yuboradi; ilova (`doc_matches`)
`orig_` ni faqat izoh, boshqa hujjatlarni faqat fayl nomi bo'yicha topadi.

**Ilovada "Original".** `video_player_screen.dart` → `_isOriginQuality`:
`url_<q>` `origin_video` ga teng bo'lsa sifat nomi o'rniga "Original"
ko'rinadi (eski ilovada hanuz sifat nomi).

**Kunlik chegara — 10 ta qism (qat'iy, tugmasiz; foydalanuvchi talabi,
GitHub Actions'dan me'yorida foydalanish uchun).** `ENCODE_DAILY_LIMIT`.
Hisob `app_config.encode_day` = `<Toshkent kuni>:<soni>`, `claim` da
birinchi urinish (`attempts == 1`) bo'lsa +1. Chegara to'lganda `claim` va
`peek` faqat yarim qolgan (ijarasi o'tgan `running`) ishni beradi,
`encode_kick` Actions'ni ishga tushirmaydi — qolganlar ertaga 00:00 dan
keyin cron bilan o'zi boshlanadi. Botdagi "Holat": "Bugun: N/10".

**Tuzatish: kodlangan sifat/yangi qism ilovada ko'rinmasdi, asl video
yo'qolmasdi.** Chekka kesh faqat YOZUV bo'lgan data-markazda tozalanadi;
sifatni GitHub Actions (AQSh), yangi qismni kodlash boti (Telegram serveri)
yozadi — foydalanuvchi yaqinidagi kesh 1 soat eski turardi. Endi
`/api/epizods/` ro'yxati chekkada 5 daqiqa, kodlanayotgan qism bo'lsa
(`"origin_video":"orig_` javobda) — 1 daqiqa. Kesh kaliti `-v2` (eski
yozuvlar bekor).

## ONLAYN KO'RISH: KO'PI BILAN 1 DAQIQA OLDINGA, QAYTA OCHILISHSIZ (2026-09-29)

**Talab:** onlayn ko'rishda ijro joyidan ko'pi bilan 1 daqiqa oldinga
yuklansin; video diskdan ko'rsatilsin; to'liq yuklab olinganda pleyer
qayta ochilmasin. Cloudflare kesh kerak emas.

- `rust/src/player_source.rs`: `rust_player_position(name, pos_ms, dur_ms)`
  — ilova ijro joyini beradi (sog'liq taymeri har 0.8 s + har `seekTo`
  oldidan; `dispose` da 0). `net_allowed`: diskda yo'q bo'lak Telegram'dan
  faqat `pos + 60 s` (o'rtacha bitreyt bo'yicha) gacha olinadi; undan
  uzoqdagisi `WAIT` → JNI `RETRY` → `AruDataSource` 1 s kutib qayta so'raydi.
  Istisno: fayl boshi (4 MiB) va oxiri (8 MiB, `moov`), joy noma'lum yoki
  2 daqiqadan eski. Test: `oldinga_bir_daqiqadan_ortiq_olinmaydi`.
- `video_player_screen.dart` → `_checkSourceSwitch`: manba `aru://` bo'lsa
  fayl to'liq yuklanganda pleyer QAYTA OCHILMAYDI (faqat `_playViaLocal`).
- Worker: ro'yxatlarning Cloudflare chekka keshi butunlay olib tashlandi
  (`list_cache_url`, `purge_list_cache`, `encbot_purge` yo'q) — har so'rov
  bazadan.

**Tuzatish: 1 daqiqa chegarasi pleyerni qotirib qo'ydi.** Pleyer buferi
tugaganda ijro joyi surilmaydi, chegara ham — kerakli bo'lak chegaradan
tashqarida bo'lsa pleyer abadiy kutardi. Endi ilova `buffering` ni ham
beradi (`v.isBuffering`; sek oldidan — true) va BUFERLANAYOTGANDA cheklov
yo'q. Admin ekranlarida asl video sifat nomi bilan emas: ro'yxatda
"Original — tayyorlanmoqda", tahrirlashda faqat "Asl video" bo'limida.

**Tizim tugmalari ortidagi qora panel (2026-09-29).** Oyna panellar ostiga
faqat Android 11+ da (`setDecorFitsSystemWindows`) cho'zilardi; Android 10
va eski MIUI'da ilova tugmalar ustida tugab, pastda tizim qora foni qolardi.
`android-template/MainActivity.kt` → `applyEdgeToEdge()`: eski Android'da
`SYSTEM_UI_FLAG_LAYOUT_*`, `FLAG_TRANSLUCENT_NAVIGATION` olib tashlanadi,
`FLAG_DRAWS_SYSTEM_BAR_BACKGROUNDS`; `onCreate`, `onResume` va fokus
qaytganda qayta qo'llanadi (MIUI tiklab yuboradi).

**Oynalar tizim tugmalari ortida qolmasin (zaxira chekinish).** MIUI'da
(ayniqsa to'liq ekrandan keyin) Flutter'ga pastki chekinish 0 kelardi —
baholash, yuklab olish va boshqa pastki oynalarning tugmalari telefon
tugmalari ortida qolardi. `lib/services/nav_inset.dart` + Kotlin
`aru/insets` → `navBottom` (barqaror balandlik, yashirilganda ham
o'zgarmaydi). `main.dart` → `MaterialApp.builder`: `MediaQuery` ning
`padding`/`viewPadding` pastki qiymati KAMIDA shuncha (klaviatura ochiq,
landshaft va pleyer to'liq ekranda — `NavInset.immersive` — qo'llanmaydi).
Qayta o'qish: birinchi kadr, ilovaga qaytish, to'liq ekrandan chiqish.

**YANGILANISH: pleyer so'rovlari to'xtatilmaydi.** Pleyerning o'z
o'qishlarini `WAIT` bilan to'xtatish tez-tez pauza va aylanma berdi
(anime bitreyti sahnaga qarab bir necha barobar o'zgaradi). Endi pleyer
so'ragan bo'lak doim beriladi; `net_allowed` faqat `prefetch` ga.
Oldinda turadigan jami: pleyer buferi (`AruLoadControl`, ≤30 s) + ≤2 MiB.
Diskdagi foiz esa BUTUN ko'rilgan joylarni (oldingi seanslar, sekdan
oldingi joy) ham o'z ichiga oladi — u "oldinga yuklangan" degani emas.

**Sakrab uzoqqa o'qish cheklandi.** Pleyer chizig'ida ijro joyidan ~3 daqiqa
oldinda yakka bo'lak paydo bo'lardi. Endi `read`: oxirgi o'qilgan bo'lakdan
ketma-ket (≤ `AHEAD`+1) o'qish — doim; sakrab, `pos + 60 s` dan uzoqdagi va
diskda yo'q bo'lak — `WAIT` (Java 1 s kutadi). Pleyer buferlanayotganda
(`buffering`) va sek oldidan (ilova joyni bildiradi) cheklov yo'q.

**Sirpanuvchi oyna: diskda doim `pos .. pos + 1 daqiqa` (foydalanuvchi
talabi: "1:00 da 2:00 gacha, 1:01 da 2:01 gacha").** `player_source.rs` →
`fill_window`: har `rust_player_position` da (~0.8 s) fonda oynadagi diskda
yo'q bo'laklar ketma-ket olinadi (bir fayl — bitta oqim, `filling`).
Chegaradan keyingisi olinmaydi. Qadam — 1 MiB bo'lak (bitreytga qarab bir
necha soniya). Test: `oyna_joy_bilan_birga_suriladi`.

## YUKLANMALAR ANIME BO'YICHA, ASL VIDEO YASHIRILDI (2026-09-29)

- Kutubxona: Tarix · Yuklanmalar · Sevimlilar. Tarix faqat anime bo'yicha.
  Yuklanmalar ham anime bo'yicha (`_DownloadAnimeCard` → `DownloadsAnimeScreen`,
  qismlar raqam bo'yicha); qism kartasida har sifat uchun `_QualityBox`
  (eni sifatlar soniga qarab, yuklangan ulushga qarab apelsin to'ladi).
- Tarixga tushish chegarasi 1 s (kadr ham), ko'rishda kadr har 10 s.
- Asl video ilovada KO'RSATILMAYDI (`with_origin` olib tashlandi): u
  shifrlanmagan va foydalanuvchi bot chatida Telegram'da ochiq ko'rinardi.
  `/api/tg/deliver` `orig_` ni faqat adminga beradi.
- Sifat hajmi yorlig'i avval diskdan (`rust_video_cache_total`), bo'lmasa
  `size_*` (ustunlar zaxira sifatida qoladi — foydalanuvchi tanlovi).

**YANGILANISH (2026-09-29): shifrlangan asl video ko'rsatiladi, ketma-ketlik
buzilmaydi.** `GET /api/epizods/a/s`: ilova orqali yuklangan (shifrlangan,
`origin_key` bor) asl video kodlanguncha bo'sh sifat o'rnida beriladi
(`with_encrypted_origin`, "Original"); kodlash botidan kelgan shifrsiz asl
video berilmaydi va `/api/tg/deliver` uni faqat adminga beradi. Ro'yxat
qism raqami bo'yicha BIRINCHI TAYYOR BO'LMAGAN qismda to'xtaydi
(`episode_ready`). Admin ekrani (`epizod_management_screen`) `?all=1` +
sessiya bilan hammasini oladi.

**Kutubxona: surib o'tish, yuklanmalarda faqat bayti bor sifatlar, navbatdagi
sifatda suzuvchi apelsin (2026-09-29).** `library_screen.dart`: `IndexedStack`
o'rniga `PageView` (tugma ham, o'ngga-chapga surish ham; `_KeepAlive`).
`history_screen.dart`: qism kartasida faqat `downloaded > 0`, yuklanayotgan
yoki navbatdagi sifatlar; navbatdagi (yoki hajmi noma'lum yuklanayotgan)
`_QualityBox` ustida `LinearProgressIndicator` (sifatlar oynasidagi
cheksiz chiziq bilan bir xil).

**Tuzatish: "1 daqiqa" 5 daqiqagacha cho'zilardi (VBR).** Oyna baytlari
o'rtacha bitreyt bilan hisoblanardi; tinch (arzon) sahnada 1 daqiqalik
o'rtacha bayt 4-5 daqiqani qamrardi. Endi `player_source.rs` `moov` dan
namuna jadvalini o'qiydi (`track_from`, fon oqimida bir marta) va oynani
AYNAN `sample_at_ms → locate().offset` bilan hisoblaydi (`window_of`);
jadval yo'q bo'lsa o'rtacha bitreyt zaxira (`avg_window`). Testlar:
`moov_dan_soniya_bayt_jadvali` (haqiqiy MP4), `oyna_joy_bilan_birga_suriladi`.

**Pleyer chizig'i VAQT bo'yicha (2026-09-29).** `rust_video_cache_ranges`
baytni `bayt/hajm` deb qaytarardi — bitreyt o'zgarganda 1 daqiqa chiziqda
4-5 daqiqa bo'lib ko'rinardi. Endi `moov` o'qilgach (`player_source::time_map`,
`mp4::VideoTrack::chunk_start_ms`) aniq soniyalar. Oyna: ketma-ket o'qish
uchun alohida ruxsat olib tashlandi, har tarmoq bo'lagi jurnalga yoziladi
(`Pleyer: <nom> #<bo'lak> tarmoqdan (<sabab>), joy <s>`; sabab: pleyer/oyna/
oldindan/moov). Yuklanmalar kartasi: sifatlar 2 tadan, tugmalar kattaroq,
o'chirish qizil, hamma sifat yig'indi tezligi.

**Yuklanmalar kartasi = BO'LIM (2026-09-29).** Guruh kaliti `animeId/seasonId`;
karta: bo'lim nomi, "N-bo'lim", biror sifati TO'LIQ yuklangan qismlar soni,
"Hajmi" — faqat DISKDA mavjud bayt (jami hajm emas). Karta ichi
(`DownloadsAnimeScreen`) shu bo'limning qismlari, raqam bo'yicha.

**Tuzatish: pleyer qotib qolardi (ovoz uzoqda).** Oyna faqat VIDEO yo'lakcha
bo'yicha edi; ovoz va video qo'pol interleave qilingan fayllarda ExoPlayer
ovoz baytlari uchun faylning boshqa joyiga sakraydi va o'sha bo'lak
"oynadan tashqari" deb kutdirilardi. Endi oyna VIDEO + OVOZ (`mp4::
parse_moov_audio`) yo'lakchalari uchun `pos .. pos + 1 daqiqa` baytlari
(`windows_of`), fon to'ldirish ham ikkalasini oladi. Zaxira: pleyer bir
bo'lakni 3 s dan ko'p kutsa, majburan beriladi (`Reader::waiting`).

## Avto-kodlash yangi akkauntda (2026-09)

- Kodlash `ogabek008/avtoencode` repoda (private, Actions daqiqalari
  cheklangan). Repo `.github/workflows/setup-autoencode.yml` bilan
  yaratiladi (`tool/encode/setup_repo.py`): fayllar yuklanadi,
  `TG_API_ID`, `TG_API_HASH`, `ENCODE_TOKEN` shifrlab o'rnatiladi.
- Ishga tushirish: worker cron (har 10 daqiqa, `encode_kick`) `GH_REPO`
  (`tool/encode/gh_repo.txt`) va `GH_ACTIONS_TOKEN` bilan. Token
  `tool/encode/gh_token.enc` da (ENCODE_TOKEN bilan shifrlangan);
  `deploy-worker.yml` uni ochib worker secret'lariga qo'yadi. Token
  almashsa faqat shu fayl yangilanadi (bir haftalik token tugaydi).
- Run 4 soatdan keyin yangi ish olmaydi (`START_BUDGET_MIN`), 5 soatda
  majburan to'xtaydi (`timeout-minutes: 300`); worker yangisini boshlaydi.
- Eski repodagi `encode.yml` faqat qo'lda; worker `GH_REPO`siz Actions'ni
  ishga tushirmaydi.
- Public qilinganda GitHub reponi bloklagan edi (`Repository has been
  locked`) — hozircha private.

## Kodlashning kunlik chegarasi olib tashlandi (2026-09)

- Foydalanuvchi talabi: "Chegarini olib tashla, endi keragi yo'q".
  `ENCODE_DAILY_LIMIT` (10 ta qism/kun), `encode_today`, `tashkent_day`
  va `app_config.encode_day` hisoblagichi o'chirildi.
- `/api/encode/peek`, `/api/encode/claim`, `encode_kick` endi navbatdagi
  hamma ishni chegarasiz beradi; bot "Holat" xabarida "Bugun ... N/10"
  qatori va "Bugungi chegara to'ldi" xabari yo'q.
- Bazadagi eski `encode_day` yozuvi zararsiz (endi o'qilmaydi).
- Eslatma: yuqoridagi "Kunlik chegara — 10 ta qism" bo'limi eskirgan.

## Botda kodlash jarayoni: qism, sifat, foiz (2026-09)

- `run.py` ffmpeg'ni `-progress pipe:1` bilan ishga tushiradi va foizni
  (`out_time / davomiylik`) `heartbeat` orqali yuboradi (har 2 daqiqada,
  bosqich almashganda darhol): `download`, `enc|1080p|37|1|4`,
  `upload|720p|2|4`.
- Worker `encode_jobs.progress` ustunida saqlaydi (`mig_encode_progress`),
  botdagi "Holat" xabari hozir ishlayotgan qismni ko'rsatadi:
  anime nomi, bo'lim/qism raqami, sifat (i/n) va foiz.
- Actions log'idagi vaqt (`16:41:23`) — UTC; Toshkent = +5 soat.
- Yangi repoga (`avtoencode`) `run.py` ni `setup-autoencode.yml` yetkazadi.

## Kodlash: ikki marta ishga tushish tuzatildi, log har soniyada (2026-09)

- SABAB (foydalanuvchi: "ishlab turgan bo'lsa ham worker yangisini
  uyg'otibdi"): (1) cron, bot tugmasi va qism qo'shish bir vaqtda
  `encode_kick` ni chaqirsa, GitHub ro'yxatida hali ko'rinmagan run'ni
  ko'rmay ikkovi ham yangi run ochardi; (2) `setup_repo.py` oxirida
  `encode.yml` ni o'zi ishga tushirardi. `concurrency` sababli ortiqcha run
  navbatda turib, keyingisi kelganda bekor bo'lardi.
- TUZATISH: ishga tushirish huquqi bazada atomik olinadi
  (`app_config.encode_kicked_at`, `INSERT .. ON CONFLICT .. WHERE .. RETURNING`,
  6 daqiqa); `setup_repo.py` endi ishga tushirmaydi.
- `run.py` kodlash paytida har soniyada bitta to'liq qator yozadi: foiz,
  video vaqti, tezlik (x), kadr/s, hajm, o'tgan vaqt, qolgan vaqt; boshida
  yadrolar soni.
- SEKINLIK SABABI: private repoda runner 2 yadroli (public'da 4). 24 daqiqalik
  1080p ~46 daqiqa (0.5x). Bepul private limit oyiga 2000 daqiqa (~20 ta
  qism); public repo cheksiz, lekin GitHub avval reponi bloklagan edi.

## Kodlash: bekor qilingan run'dan keyin tinmay run ochilishi tuzatildi (2026-09)

- SABAB: run qo'lda bekor qilinsa, ish bazada `running` va ijarasi 30 daqiqa
  qolardi. Yangi run "boshqa run ishlayapti" deb muvaffaqiyatli tugardi,
  "Davom ettirish" qadami esa navbat bo'sh emasligi uchun yana yangisini
  ochardi — tinmay qisqa run'lar.
- TUZATISH: (1) ijara 6 daqiqa (heartbeat 2 daqiqada); (2) `run.py`
  SIGINT/SIGTERM'da ishni `finish {cancelled:true}` bilan darhol navbatga
  qaytaradi (urinish sanalmaydi); (3) "Davom ettirish" faqat `run.py` haqiqatan
  ish bajarganda (`worked=1` output) yangi run ochadi.
- Log qatoriga bitreyt qo'shildi. Preset `medium` o'zgarmadi.
- Log: har soniyalik qatorda bitreyt va hozirgi/taxminiy yakuniy hajm; manba
  qatorida hajm va bitreyt; sifat tugaganda o'rtacha bitreyt va hajm.
- Botdagi "Holat": hozir ishlayotgan qism uchun Actions log'idagi eng yangi
  statistika (foiz, tezlik, kadr/s, bitreyt, hajm va taxminiy yakuniy hajm,
  o'tgan/qolgan vaqt) va "N soniya oldin yangilangan". `run.py` har 30
  soniyada heartbeat bilan `enc|sifat|foiz|i|n|tezlik|fps|bitreyt|MB|taxmin|o'tdi|qoldi`
  yuboradi (Turso: soatiga ~120 yozuv).

## Actions log'i yopiq kanalga; worker'ga 10 daqiqada; qayta boshlash (2026-09)

- `run.py` (`ChannelLog`) log'ni kanalga (`LOG_CHANNEL_ID`, avtoencode
  workflow'ida `-1004360822958`) har `LOG_INTERVAL_SEC` (5) soniyada YANGI
  xabar bilan yuboradi: shu orada to'plangan qatorlar bitta xabar. Tahrir
  yaxshi ishlamadi (foydalanuvchi qarori). Kanalga log run boshlanishi bilan
  yoqiladi, har qism o'z sarlavhasi bilan; yuklab olish/yuklash ham har
  soniyada qator beradi. FloodWait bo'lsa qatorlar to'planib, keyin bittada
  ketadi. Xato bo'lsa kodlash to'xtamaydi, 5 xatodan keyin kanalga yozish
  o'chadi. Sessiya hisobi kanalga a'zo bo'lishi kerak (soatiga ~720 xabar; yuborilgan qatorlar qayta yuborilmaydi).
- Worker'ga heartbeat 10 daqiqada (ijara 25 daqiqa); bot "Holat" shu oxirgi
  holatni ko'rsatadi, jonli log — kanalda.
- Qo'lda qayta boshlash: `restart-autoencode.yml` (`restart_run.py`) —
  avtoencode'dagi run'larni bekor qiladi, `/api/encode/release` bilan
  ishlarni navbatga qaytaradi va yangisini ishga tushiradi.
- Kanalga log run'ning o'zi boshlanishi bilan yoqiladi (`Run ... boshlandi`),
  qism kutilmaydi; yuklab olish/yuklash ham har soniyada qator beradi
  (`transfer_progress`). ffmpeg/pip o'rnatilishi (Python'gacha ~1.5 daqiqa)
  kanalga tushmaydi — ular `run.py` ishga tushmasdan oldin.

## Eskirgan "ishlayapti" avtomatik bo'shatiladi; tezlik (2026-09)

- Run qo'lda bekor qilinganda ish bazada "ishlayapti" bo'lib qolar va bot
  "ishlayapti" der, yangi run esa ochilmasdi. `encode_kick`: oxirgi
  heartbeat 4 daqiqadan eski va GitHub'da ishlayotgan/kutayotgan run yo'q
  bo'lsa — ish bo'shatiladi va yangi run ishga tushadi.
- Tezlik: private repoda runner 2 yadroli, public'da 4 (`ubuntu-latest`).
  Log boshida CPU modeli chiqadi; ixtiyoriy `H265_X265_EXTRA` (preset
  o'zgarmaydi).
- Yangi va eski (`anime` repo, `scripts/encode_h265.sh`) kodlash buyruqlari
  qatorma-qator solishtirildi: libx265, preset medium, CRF 30/-1/-2/-3,
  `-x265-params log-level=error`, lanczos, yuv420p/hvc1, AAC — BIR XIL.
  Tezlik farqi skriptdan emas, runner uskunasidan (private: 2 mantiqiy
  yadro ≈ 1 jismoniy, public: 4). Log boshida endi mantiqiy/jismoniy
  yadrolar soni chiqadi.

## EMOJI, GIF VA STIKER TO'PLAMLARI — ILOVANING O'Z TIZIMI (2026-09)

> Eslatma: yuqoridagi "Telegramdagi emoji/GIF/stikerlar" (rlottie, libvpx,
> tlottie, `tg_media.dart`) bo'limlari ESKIRGAN — o'sha kod repodan olib
> tashlangan (kuchsiz telefonlar ko'tara olmadi). Quyidagisi uning o'rnida.

**Nega.** Telegram'ning `.tgs` (Lottie) va `.webm` stikerlari har kadrda
CPU'da chizilardi. Endi foydalanuvchilar o'zi yasagan rasmlarni yuklaydi,
ular yengil WebP ga aylantiriladi va ilova ularni Telegram'siz o'zi ko'rsatadi.

**Tuzilma.** Bitta TO'PLAM = kanaldagi bitta shifrlangan fayl
(`pk_<id>_<versiya>.arp`). Emoji, GIF va stiker uchun ALOHIDA to'plamlar
(tur to'plam yaratilganda tanlanadi). Fayl <= 1 GB, element <= 5 MB.
- Format (`tool/encode/arupack.py`): `[16 bayt: "ARUP", versiya, sarlavha
  uzunligi][sarlavha JSON][hamma kichik rasm (thumb)][elementlar]` — MP4
  dagi `moov`/faststart kabi: ilova avval faqat sarlavhani, keyin kerakli
  bo'lakni (`Range`) o'qiydi. Kichik rasmlar ketma-ket va boshida — to'plam
  oynasi bitta oraliq so'rovi bilan to'ladi.
- Shifrlash: butun fayl AES-128-CTR (IV nol, `rust/src/telegram.rs` bilan
  bir xil). **Har yangi versiyaga YANGI kalit** — bir xil kalit+IV bilan
  o'zgargan faylni qayta shifrlash CTR'ni buzadi. Element raqamlari
  (`next`) qayta ishlatilmaydi.
- Kalit va xabar raqami `tg_files` da (boshqa fayllardagidek); ko'rish —
  `/api/tg/deliver` (`pk_...` hammaga ochiq), keyin mahalliy
  `127.0.0.1/tg/...` dan `Range` bilan (`PackService._range`).

**Oqim.**
1. Ilova rasmni (PNG/JPG/GIF/WebP, <= 5 MB, tur BAYTLARDAN aniqlanadi)
   shifrlab bot chatiga yuklaydi: `pki_<hisob>_...` (`tg_user_media`,
   `/api/tg/claim` shu prefiksni taniydi), keyin `SyncQueue` orqali `add`.
2. Worker `pack_ops` ga `pending` yozadi. ADMIN (`admin_packs_screen.dart`,
   `/api/packs/admin/review`) ko'rib chiqadi: tasdiqlansa `approved`,
   rad etilsa `rejected` + sabab (egasiga ko'rinadi, fayl kanaldan o'chadi).
3. Tasdiqlangan amali bor to'plamni Actions oladi (`tool/packs/run.py`,
   ALOHIDA `packs.yml` workflow'i, yangi akkauntdagi repoda; kodlash
   `encode.yml` ga TEGILMAYDI). Worker uni faqat `approved` amal bo'lganda
   ishga tushiradi (`packs::kick`, cron har 10 daqiqa + admin tasdiqlaganda).
   `pending` (admin ko'rmagan) va `rejected` amallar navbatda TURADI,
   lekin Actions ularni ishlamaydi. **Kodlash va to'plamlar BIR VAQTDA
   ishlaydi** (3000+ qism kodlanayotganda to'plamlar kutib qolmasin —
   foydalanuvchi talabi): ALOHIDA `concurrency` guruhi (`arugram-packs`) va
bir xil Telegram sessiyasi (`tool/encode/session.enc`; u allaqachon bir
   necha joyda ishlaydi, foydalanuvchi qarori). Run: eski faylni yuklab ochadi, elementlarni
   `arunorm.py` bilan yengil WebP ga aylantiradi (stiker 512 px, emoji
   128 px, GIF 480 px; <= 20 kadr/s; uzun/og'ir animatsiya RAD etiladi;
   har elementga 96 px statik thumb), yangi kalit bilan yuklaydi va
   `finish` yuboradi. Bitta element xatosi qolganlariga tegmaydi. To'plam
   1 GB dan oshsa qolganlari "to'plam to'ldi" bilan rad.
4. Ko'rish: to'plam oynasida faqat statik thumb; xabarda animatsiya faqat
   `AnimSlots` bo'sh joyi bo'lsa (kuchsiz telefonda 2, o'rtachada 5,
   kuchlida 9), qolganlari statik. Hamma narsa `aru_packs/` da shifrlab
   keshlanadi; disk hajmi CHEKLANMAGAN (foydalanuvchi talabi).

**Turso (kam yozuv).** Elementlar bazada EMAS, faylning sarlavhasida.
Jadvallar: `pack_db` (to'plam + Actions ijarasi), `pack_ops` (kutayotgan/
tasdiqlangan/rad etilgan amallar; bajarilgani O'CHADI), `pack_subs`.
Hamma foydalanuvchi yozuvi `POST /api/sync` dagi `packs` massivi orqali
(`packs::sync_stmts`: har amal o'zini tekshiradi, yaroqsizi tashlanadi;
chegaralar: 30 to'plam, 100 kutayotgan rasm, 200 obuna, bir paketda 50 amal).
O'qish: `/api/packs/library` (mening + obunalar + amallar, BITTA so'rov),
`/public`, `/info`.

**Xabarlarda.** Stiker/GIF: `media_type=sticker|gif`, `media_file=
pk_<to'plam>_<element>` (worker to'plam borligini va turi mosligini
tekshiradi — `packs::valid_ref`; support chat va izohlarda). Matn ichidagi
emoji: `[pe:<to'plam>:<element>:<emoji>]` (eski Telegram `[ce:...]` bilan
adashmasin). Yozish maydonida u BITTA belgi (U+E000...) va rasm bo'lib
chiziladi (`TgTextController`), yuborishda belgiga aylanadi.

**Hisob o'chirilsa** to'plamlar QOLADI (foydalanuvchi talabi) — faqat
o'sha odamning obunalari (`pack_subs`) o'chadi.

**Actions.** `tool/encode/setup_repo.py` (workflow: "Avto-kodlash repo'sini
yaratish") yangi repoga `packs.yml`, `tool/packs/*` fayllarini yuklaydi —
YANGI FAYLLAR YETIB BORISHI UCHUN uni bir marta qo'lda ishga tushiring.
Navbat: `/api/packs/job/peek|claim|heartbeat|finish` (`ENCODE_TOKEN`).

**Ma'lum cheklovlar.**
- Ko'rish uchun Telegram hisobi ulangan bo'lishi kerak (butun ilova shunday).
- Actions `finish` yubormay o'lsa, kanalda bitta ortiqcha (bazada yo'q) post
  qoladi — zararsiz.
- `.tgs` va video (MP4/WebM) qabul qilinmaydi: faqat PNG/JPG/GIF/WebP.
- Animatsiyali WebP kirishda ham qabul qilinadi, lekin GIF eng ishonchli.

**Fayllar.** `worker/src/packs.rs`, `worker/src/lib.rs` (sync, deliver,
claim, chat/izoh, peek/kick, hisob o'chirish), `tool/packs/{arupack,
arunorm,run,test_arupack}.py`, `tool/packs/packs.workflow.yml`, `lib/services/{pack_service,
sync_queue}.dart`, `lib/widgets/{pack_views,tg_composer,emoji_text}.dart`,
`lib/screens/{my_packs,pack_detail,admin_packs}_screen.dart`, testlar:
`test/pack_test.dart`, `test/pack_widget_test.dart`.

### To'plam qo'shish oynasi, ilova emojisi va video (2026-09)

- **Qo'shish oynasi** (`pack_add_screen.dart`): tanlangan rasm/video KO'RINIB
  turadi; har biri uchun mos emoji, videoni kesish, olib tashlash; hajm
  (<= 5 MB) va uzunlik yuklashdan OLDIN tekshiriladi, sabab kartada chiqadi.
- **Mos emoji** telefonning klaviaturasidan emas, ilovaning o'z Telegram
  emoji oynasidan (`pack_emoji_picker.dart`, `TgEmoji` shrifti).
- **Video** (MP4/WebM, faylning o'zi <= 5 MB): kesish oynasi
  (`pack_video_trim_screen.dart`) — bo'lak fayl nomiga yoziladi
  (`pki_..._t<boshi_ms>-<oxiri_ms>.bin`), kesish va yengil WebP ga aylantirish
  Actions'da (`arunorm.py`, `ffmpeg`; 15 kadr/s, ovoz tashlanadi, emoji uchun
  markazdan kvadrat). Uzunlik: emoji 5 s, stiker 8 s, GIF 15 s. Admin videoni
  o'ynatib ko'radi (`admin_packs_screen.dart`). `packs.yml` ffmpeg o'rnatadi.
- **Rasm ko'rinmasligi:** birinchi o'qishda 256 KB olinadi va undagi kichik
  rasm/elementlar darhol keshga tushadi; kichik rasm olinmasa elementning
  o'zi sinaladi; oxirgi xato to'plam oynasida yozib qo'yiladi
  (`PackService.lastError`).

### Bosib turganda katta ko'rinish va menyu; saralanganlar (2026-09)

Telegram Android (`ContentPreviewViewer.java`, GitHub'dan o'qildi) dagidek:
- Katakni bosib turilsa (`pack_preview.dart`): orqa fon 0x71000000 (~120 ms)
  + yengil xiralik, element markazda KATTA (eng kichik tomon - 40 dp),
  tepasida unga mos emoji, pastida yumaloq menyu (320 ms, easeOutQuint,
  tepadan 12 dp siljib chiqadi). Animatsiyali element shu yerda o'ynaydi.
- Stiker/GIF menyusi: yuborish, saralanganlarga qo'shish/o'chirish, (egasi
  uchun) to'plamdan o'chirish. Emoji: "Emoji yuborish", "Emojidan nusxa olish".
- ⭐ Saralanganlar (`PackService.favorites`, telefonda `pack_fav_<tur>`) —
  panelning birinchi bo'limi. "Tovushsiz yuborish" va "Rejalashtirish"
  qilinmadi: bizda bunday xabar turlari yo'q.

### Animatsiya qanday ishlaydi va tekshiruv tuzatishlari (2026-09)

- Animatsiya — yengil animatsion WebP (<= 15-20 kadr/s, kichik o'lcham).
  Panelda hech qachon animatsiya YO'Q (faqat statik thumb). Xabarda va katta
  ko'rinishda o'ynaydi, lekin bir vaqtda ko'pi bilan `AnimSlots.max` ta
  (kuchsiz 2, o'rtacha 5, kuchli 9); joy bo'lmasa statik thumb turadi va joy
  bo'shashi bilan o'zi boshlanadi. Ekrandan chiqqan vidjet joyini qaytaradi.
- Tuzatildi: xabar internet yo'qligidan bo'sh qolib ketmasin (3 marta qayta
  uriniladi); to'plam amallari kunlik yuborish chegarasini yemasin (3 s
  yig'iladi, kun chegarasiga 10 qolganda majburlanmaydi); bitta xabarga <= 20
  maxsus emoji (server uzunlik chegarasi belgini kesib qo'ymasin); yuklash
  paytida orqaga chiqib fayllarni o'chirib bo'lmaydi; kichik rasm olinmasa
  faqat kichik (<= 300 KB) element o'rniga yuklanadi; tanlangan stiker/GIF
  xabar chiqmasdan oldin oldindan yuklanadi; eski rad etilgan yozuvlar
  (30 kun) bazadan tozalanadi.

### "Hech narsa ko'rinmayapti" sababi topildi (2026-09)

`PackService.header()` va `_thumb()` da `future.whenComplete(() => map.remove(k))`
yozilgan edi. `Map.remove` o'sha Future'ning O'ZINI qaytaradi, `whenComplete`
esa qaytarilgan Future'ni kutadi — ya'ni o'zini o'zi abadiy kutib qotardi:
sarlavha va kichik rasmlar HECH QACHON tugamasdi (panel va to'plam oynasida
bo'sh kataklar). Endi `whenComplete(() { map.remove(k); })` (figurali qavs bilan).
Qayta yuz bermasligi uchun `test/pack_pipeline_test.dart`: Python yasagan
HAQIQIY to'plam fayli (`test/fixtures/sample_pack.arp`) Telegram'siz o'qiladi
(`PackService.rangeOverride`). Qoida: `whenComplete` ichida `=>` bilan
Future qaytaradigan narsa yozmang.

Qulaylik (Telegram `EmojiView.java` asosida): ⚙ tugma (GIF/Stikerlar
sahifasida to'plamlarni boshqarish), to'plam nomiga bosilsa to'plam ochiladi,
xabardagi stikerga bosilsa to'plami ochiladi va uni qo'shish mumkin; video
davomiyligi o'qilmasa ham yuborish mumkin (server tekshiradi).

### Panel tuzilishi Telegram (`EmojiView.java`) bilan bir xil (2026-09)

Telegram kodidan o'qib olingan (DrKLO/Telegram, `EmojiView.java`):
- Tepadagi bo'limlar qatori (`EmojiTabsStrip`, 36 dp) ro'yxat USTIDA suzadi:
  pastga aylantirilsa tepaga chiqib yashirinadi, tepaga aylantirilsa qaytadi
  (`checkTabsY`). Ro'yxatning yuqori bo'sh joyi 36 dp, pastki 44 dp.
- Qidiruv qatori (50 dp) — ro'yxatning BIRINCHI elementi: u ham ro'yxat bilan
  aylanib ketadi (qotib turmaydi).
- Emoji katagi kenglik/45 dp, stiker kenglik/72 dp, GIF qatori ~100 dp
  (bizda ~118 dp, har biri o'z nisbatida, qator kenglikka to'liq sig'adi).
- Emoji sahifasi tartibi: yaqinda, Unicode bo'limlari, KEYIN maxsus emoji
  to'plamlari. GIF sahifasida tepadagi qator YO'Q (faqat qidiruv + devor).
- Stikerlar sahifasida ⚙ (sozlamalar) tugmasi, to'plam nomiga bosilsa
  to'plam ochiladi, bosib turilsa katta ko'rinish va menyu.
Bizda: `_Sections` (`leading` — qidiruv, `tabs` — suzuvchi qator, `ValueNotifier`
bilan faqat qator qayta chiziladi). Panel foni issiq to'q rang (skrinshotdagi).

### To'plamga yuklash 100% da qotib qolmasin (2026-09)

`TelegramService.uploadFile` yuklash tugagach botning kanalga ko'chirishini
(`_awaitClaim`, ~26 s va undan ko'p) KUTARDI — foydalanuvchi 100% da qotib
qolgandek ko'rardi. Endi to'plam qo'shishda kutilmaydi (`waitForClaim: false`:
fayl allaqachon Telegram'da, bot va worker o'zi tugatadi), bosqich matni
ko'rsatiladi ("Telegram qabul qilmoqda...") va Telegram 120 s javob bermasa
yuklash bekor qilinib xato matni chiqadi.

## Video tahrirlash oynasi (to'plamga video qo'shishda)

Nima: video tanlanganda CapCut uslubidagi oyna avtomatik ochiladi — kadrlardan
iborat vaqt chizig'i, chetlarini/oynani sudrab bo'lak tanlash, tanlangan bo'lak
aylanib ko'rinadi. Eng uzun bo'lak to'plam turiga qarab: emoji 5 s, stiker 8 s,
GIF 15 s (`packMaxSeconds`, `tool/packs/arunorm.py` bilan bir xil). Kesishni
baribir Actions bajaradi (fayl nomida `_t<start>-<end>`).
Kadrlar: `android-template/MainActivity.kt` → `aru/thumb` kanalining `frames`
usuli (MediaMetadataRetriever, oqimda, xato bo'lsa null). Kanal bo'lmasa
vaqt chizig'i kulrang bo'ladi, kesish baribir ishlaydi.
Fayllar: `lib/screens/pack_video_trim_screen.dart`, `pack_add_screen.dart`,
`test/pack_trim_test.dart`.

## Telegram uslubidagi fayl tanlash

Nima: attach oynasining "Fayl" bo'limi Telegram'dagidek — "Ichki xotira"
(jildlar bo'ylab yurish, `..`, qidiruv, saralash, ko'p tanlash), "Galereya",
"Oxirgi fayllar" (Download va boshqa odatiy jildlar), tizim tanlagichi
"Boshqa ilovalardan" zaxira sifatida. To'plamga qo'shishda (`packMode`) faqat
rasm/video ko'rinadi. Barcha jildlar uchun `MANAGE_EXTERNAL_STORAGE` (workflow
manifestga qo'shadi) va `aru/files` kanali (`hasAll`, `requestAll`,
MainActivity). Ruxsat berilmasa — tugma va zaxira tanlagich.
Fayllar: `lib/widgets/tg_file_browser.dart`, `tg_attach_sheet.dart`,
`android-template/MainActivity.kt`, `.github/workflows/build-flutter-apk.yml`,
`test/tg_file_browser_test.dart`.

## APK'ni Telegram "Saqlangan xabarlar"ga yuborish

`build-flutter-apk.yml` oxirida (Release'dan keyin) har bir tayyor APK Telegram
hisobining "Saqlangan xabarlar"iga (`me`) yuboriladi: `tool/notify/send_apk.py`.
Sessiya `tool/encode/session.enc` dan (`ENCODE_TOKEN` bilan) nusxaga tiklanadi,
`TG_API_ID`/`TG_API_HASH` secret'lari ishlatiladi. Qadam `continue-on-error`:
xato bo'lsa build buzilmaydi. 4 ta APK — 4 ta xabar.

## To'plam cheklovlari yengillashtirildi

`tool/packs/arunorm.py`: kadrlar soni, animatsiya/video uzunligi, manba o'lchami
va emoji kvadrat sharti bo'yicha rad etish OLIB TASHLANDI (foydalanuvchi talabi).
Qoldi: 5 MB hajm, chiqish o'lchami (stiker 512 / emoji 128 / GIF 480 px), sekundiga
20 kadr. Kadrlar o'qilishi bilan kichraytiriladi (xotira to'lmaydi). GIF
to'plamida rasm ham, animatsiya ham mumkin. Ilovadagi tahrirlash oynasining
tur bo'yicha eng uzun bo'lagi (5/8/15 s) saqlandi.

## O'lchamlar (emoji < stiker < GIF) va GIFda OVOZ

`tool/packs/arunorm.py`: emoji 128 px (eng kichik), stiker 384 px, GIF 640 px
(asl nisbatda, xilma-xil). GIF to'plamiga ovozli video yuklansa, ovoz
saqlanadi: H.264+AAC MP4 (<= 5 MB, sifat pog'onalari bilan), sarlavhada `a=2`
(`PackItem.video`). Ovozsiz video, emoji va stikerda oldingidek yengil WebP.
Ilova: `PackImage` video elementni vaqtinchalik fayldan takrorlab o'ynatadi
(chatda ovozsiz, bosilsa ovoz yoqiladi; uzoq bosib ko'rish va element oynasida
ovozli), o'yin joylari `AnimSlots` bilan cheklangan. Ro'yxat/devorda faqat
kichik statik rasm.
Fayllar: `arupack.py`, `arunorm.py`, `run.py`, `test_arupack.py`,
`lib/services/pack_service.dart`, `lib/widgets/pack_views.dart`, `pack_preview.dart`.

## Ovozni tanlash va moslashtirish

Video tahrirlash oynasidagi karnay tugmasi GIF uchun ovozni yoqadi/o'chiradi
(natija `(boshi, oxiri, ovoz)`); ovozsiz bo'lsa fayl nomi `..._m.bin` bilan
tugaydi (`run.py: mute_of`). Emoji/stikerda ovoz doim olib tashlanadi.
Actions mos kelmagan faylni RAD ETMAY moslashtiradi (`arunorm.normalize`):
noma'lum rasm formati (BMP, TIFF...) — Pillow, noma'lum video — ffmpeg;
emoji rasmi ham markazdan kvadratga kesiladi; 5 MB ga sig'masa sifat va o'lcham
pasayadi, keyin kadrlar siyraklashtiriladi; ovozli MP4 sig'masa ovozsiz WebP.
Rad etish faqat fayl umuman o'qilmasa yoki eng past sifatda ham 5 MB dan katta bo'lsa.

## Ovozli GIF — H.265 (HEVC)

`arunorm._video_keep_audio` endi H.265 (libx265, `hvc1`) + AAC bilan kodlaydi:
bir xil sifatda H.264 dan ~40% kichik, ya'ni 5 MB ga sifat pasaytirmasdan
sig'adi. x265 yiqilsa — H.264 zaxirasi. Pleyer (ExoPlayer) HEVC ni qurilma
dekoderi bilan o'ynatadi (kodlash workflow'idagi H.265 videolar kabi).

## Muhim: to'plam kodi ikkinchi repoda (sinxronlash)

Kadrlar chegarasi olib tashlanganiga qaramay "kadrlar juda ko'p (200 ta)" bilan
rad etilishi sababi: `packs.yml` BOSHQA akkauntdagi `avtoencode` repoda
ishlaydi va uning kodi (`run.py`, `arunorm.py`, `arupack.py`) o'sha yerga NUSXA
edi — bu repodagi o'zgarish yetib bormagan. Endi `.github/workflows/sync-packs.yml`
`tool/packs/**` o'zgarganda `tool/packs/sync_repo.py` bilan o'zgargan fayllarni
`gh_token.enc` tokeni orqali o'sha repoga yuklaydi (qo'lda ishga tushirish shart emas).

Admin tekshiruvi: video OVOZ bilan (karnay tugmasi), "Butun to'plam" va
"Egasining profili" tugmalari. Galereya bo'sh chiqsa — sabab va yo'llar
(ruxsat sozlamalari, cheklangan ruxsatni kengaytirish, tizim galereyasi);
fayl brauzeri xatosi ekranda ko'rsatiladi, Android 10 uchun
`requestLegacyExternalStorage`. Ovozli MP4 o'lchamlari 16 ga karrali.

## Rad etilganlarni qayta yuborish va tozalash

To'plam oynasidagi "Yuborilgan rasmlar"da rad etilgan qatorda "Qayta yuborish"
(asl fayl telefonda saqlangan bo'lsa; muvaffaqiyatli yuborilsa eskisi tozalanadi)
va "O'chirish" tugmalari, sarlavhada "Rad etilganlarni tozalash". Server:
`packs.rs` sync amali `clear` (faqat egasining `rejected` yozuvlari). Rad etilganlar
30 kun ko'rinadi (`REJECTED_SHOW_MS`), tozalanmasa 30 kundan keyin avtomatik
o'chadi: `packs::cleanup_rejected` (cron'dan, kuniga bir marta 03:00 UTC) va har
job tugaganda. Telefondagi nusxalar (`aru_packs/sent/`) server yozuvi yo'qolgach
o'chadi. Eski (yangilanishdan oldingi) rad etilganlarda asl fayl yo'q — faqat "O'chirish".

## Animatsiya hamma joyda (kam bosim bilan) va GIF ovozi

`pack_views.dart`: `AnimSlots` uch hovuzli — KICHIK (<= 200 KB, emoji: past/o'rta/kuchli
telefonda 6/14/24), KATTA (stiker/GIF: 2/5/9), VIDEO (ovozli MP4: 1/2). Panelda
(stiker, emoji, GIF devori) va to'plam oynasida ham `animate: true`: faqat ekrandagi
katakchalar (lazy ro'yxat) va faqat joy bor bo'lganicha animatsiya qiladi, qolgani
kichik statik rasm; joy bo'shasa boshlanadi. To'liq element olinmasa qayta uriniladi.
`PackSoundHub`: izohda GIF ovozi yoqilsa asosiy pleyer pauza bo'ladi; pleyerda play
bosilsa GIF ovozi o'chadi, animatsiya davom etadi (`video_player_screen.dart`).

## Faqat to'liq ko'ringan elementlar animatsiya qiladi; Telegram kodidan xulosalar

`PackImage`: aylanuvchi ro'yxatda element ekranda TO'LIQ ko'ringandagina animatsiya
boshlanadi (`_fullyVisible`, aylantirish to'xtagach 140 ms dan keyin `_recheck`);
ko'rinmay qolsa joyini bo'shatib statik rasmga qaytadi. Shu sababli hovuz kattaroq:
kichik 12/30/60, katta 4/8/14.
Telegram (DrKLO) `MediaController.loadGalleryPhotosAlbums`: rasm va video ALOHIDA
MediaStore so'rovlari (`Images` / `Video`), ruxsat `READ_MEDIA_*` (13+) yoki
`READ_EXTERNAL_STORAGE`. `ChatAttachAlertDocumentLayout`: "barcha fayllar" ruxsati
(`isExternalStorageManager`) yo'q bo'lsa tizim fayl tanlagichiga o'tadi. Bizda ham:
galereya birlashtirilgan so'rovda bo'sh chiqsa rasm/video alohida so'raladi; "Ichki
xotira" ruxsatsiz tizim tanlagichini ochadi.

## Kechiktirilgan yuklash (ekran + 2 qator)

`PackImage` element ekran va undan tepa/pastga 2 qator (element balandligi x 2) ichida
bo'lgandagina yuklanadi (`_withinWindow`); undan uzoqdagilar kutadi va aylantirish
davomida oynaga kirishi bilan (60 ms) tez yuklanadi. Server so'rovlari navbati
(`_Gate`) endi yangisi-birinchi: hozir ko'rinayotgan katakcha eski, ekrandan ketganlardan
oldin olinadi.

## Doimiy diskda saqlash va tez ochilish

Emoji/GIF/stiker kichik rasmlari va elementlari diskda (`aru_packs/`, shifrlangan)
DOIMIY saqlanadi: hech qanday hajm chegarasi yo'q, "Keshni tozalash" (video kesh,
`video_byte_cache/`) ularga tegmaydi, faqat `PackService.clearCache` o'chiradi (hech
qayerdan chaqirilmaydi). Endi to'plam ma'lumoti (fayl nomi, versiya) ham diskda
(`pack_known_v1`, 300 tagacha): ilova qayta ochilganda boshqalarning to'plamlari
ham tarmoqsiz diskdan darhol chiqadi; ma'lumot 6 soatdan eski bo'lsa orqa fonda
yangilanadi. GIF devorida nisbat 0.4..3.5 oralig'ida (uzun/keng elementlar o'z
nisbatida, kichraytirilgan holda).

## Xotira oynasida "Emoji, GIF va stikerlar"

`storage_usage.dart`/`storage_screen.dart`: `aru_packs/` keshi endi "Boshqa"ga emas, alohida
toifaga tushadi: "Emoji, GIF va stikerlar" (ochiladigan), ichida Emoji / GIF / Stiker
(to'plam turi `PackService.packKinds()` dan). Har bir bo'lim alohida belgilanadi va
tozalanadi (`PackService.clearKinds`), butun toifa — `clearCache`. Tozalangan rasmlar
bulutdan (kanaldan) qayta yuklanadi.

## Tuzatishlar: galereya, to'plam o'qish, admin video

* Galereya: MIUI'da `getAssetListPaged` "near LIMIT: syntax error" bilan yiqilardi —
  endi tartib aniq beriladi (`FilterOptionGroup(orders)`), sahifalash yiqilsa `getAssetListRange`,
  u ham yiqilsa rasm/video alohida so'raladi.
* To'plam o'qish (`PackService._range`): mahalliy Telegram serveri uzun o'qishni o'rtasida
  uzardi ("Connection closed while receiving data") — o'qish 1 MB bo'laklarga bo'linadi,
  har bo'lak 3 marta uriniladi, xatodan keyingi kutish 20 s emas 5 s. Shu sabab uzoq bosishda
  va panelda to'liq element (GIF animatsiyasi) yuklanmay, faqat statik rasm qolardi.
* `PackImage`: aylanmaydigan tarkibda (ko'rish oynasi) "to'liq ko'rinish" tekshiruvi o'tkazib yuboriladi.
* Admin: video ochilmasa sabab va "Tashqi ilovada ochish" tugmasi ko'rinadi.

## Ekrandagi elementlar tez va to'liq yuklanadi

`PackService._data`: yonma-yon turgan kichik elementlar (<= 200 KB, jami <= 768 KB) BITTA
so'rovda olinadi va hammasi xotira/diskka tushadi (`_dataGroup`) — ekrandagi katakchalar
ketma-ket bo'lgani uchun Telegram'ga so'rov soni keskin kamayadi. So'rovlar kanali 4 dan
6 ga, aylantirish to'xtagach animatsiyani boshlash 80 ms (avval 140).

## Ko'ringan hamma emoji/GIF/stiker animatsiya qiladi (hamma joyda)

`pack_views.dart`:
* `_fullyVisible` endi geometrik: element ekran ichida va o'rab turgan HAMMA aylanuvchi
  ro'yxatlar (ichma-ich, gorizontal ham) ichida to'liq bo'lsa. Yopiq/orqadagi sahifa
  (`TickerMode`: panelning boshqa varag'i, ustiga ochilgan oyna) va ilova fonda — yo'q.
* `_VisWatch`: bitta umumiy taymer (300 ms) hamma `PackImage`ni qayta tekshiradi — panel
  ochilishi/yopilishi, klaviatura, varaq almashishi kabi aylantirishsiz o'zgarishlar ham sezilsin.
* `AnimSlots` chegarasi bir ekranga sig'adigandan ko'p (kichik 48/72/120, katta 16/24/36,
  video 4/6/8): ko'ringan hammasi o'ynaydi, chegara faqat favqulodda himoya. Kuchsiz
  telefonda bosim ko'rinmaydiganlar darhol to'xtashi va `cacheWidth` bilan kamayadi.
* Bosib turilgandagi katta ko'rinish va to'plam elementi oynasi `priority: true`: joy
  bo'lmasa eng eski oddiy egasining joyini oladi (`slotEvicted`), yopilgach u qaytadi.
* Ovozli GIF pleyerlari `mixWithOthers: true` — ovoz fokusini so'ramaydi, shuning uchun
  bir nechtasi bir-birini (va asosiy pleyer ularni) pauza qilmaydi; chatdagi hamma GIF
  ovozsiz o'ynaydi, bosilgani ovozli (`PackSoundHub`), ekrandan chiqsa ovozi o'chadi
  (`onStopped`). Vaqtinchalik fayl nomi vidjetga xos (bir xil GIF ikki marta bo'lsa
  biri ikkinchisining faylini o'chirmaydi).
* Panel belgilari (to'plam ikonkalari) va "Mening to'plamlarim" ro'yxatida ham `animate: true`.

## O'chirilgan anime/bo'lim/qism hamma foydalanuvchining tomosha tarixidan ketadi

`worker/src/lib.rs`:
* Admin anime, bo'lim yoki qismni o'chirsa — `watch_history_db` dagi mos qatorlar ham
  (hamma foydalanuvchida) o'sha zahoti o'chadi.
* `/api/history` va statistika "Qismlar" ro'yxati (`episodes`) faqat MAVJUD qismni
  ko'rsatadi (`e.epizod_id IS NOT NULL`) — eski qoldiqlar darhol ko'rinmaydi.
* `cleanup_orphan_history` (cron, kuniga bir marta 03:20 UTC): `epizod_db` da yo'q
  qismlarning tarixini o'chiradi (o'zgarishdan oldin o'chirilganlar uchun).
  `NOT IN (SELECT ...)` — o'qish = tarix + qismlar soni; `epizod_db` bo'sh bo'lsa tegmaydi.
* `/api/sync`: mavjud bo'lmagan bo'lim uchun tarix yozuvi qabul qilinmaydi (telefondagi
  eski navbat o'chirilgan animeni tarixga qaytarmasin; qo'shimcha o'qish yo'q —
  `real_season` allaqachon olinadi).

## To'plamga yuborish: "Yuborilmoqda..." qotib qolmaydi, bittada bitta fayl

Sabab: fayl Telegram'ga yuklangach, qo'shish amali telefondagi `SyncQueue`da serverga
yuborilishini kutadi. Navbat oddiy yuborishni kuniga 24 ta bilan cheklaydi — faol kunda
chegara to'lsa (yoki amal boshqa paket ketayotganda qo'shilib, `flush` `_sending` sabab
o'tkazib yuborsa) amal ertasi kungacha "Yuborilmoqda..." bo'lib turardi.
* `sync_queue.dart`: navbatda to'plam amali bo'lsa qat'iy chegaragacha (50) majburiy
  yuboriladi; `removeKey` — bitta yozuvni olib tashlash.
* `pack_service.dart`: `_flushSoon` boshqa paket ketayotgan bo'lsa kutib qayta uriniladi;
  `retryQueued`, `cancelQueued`, `hasQueuedAdd`.
* `pack_detail_screen.dart`: "Yuborilmoqda..." qatorida "Qayta urinish" va "Bekor qilish";
  oldingi fayl serverga yetmaguncha yangisini qo'shib bo'lmaydi.
* Bittada BITTA fayl: `tg_attach_sheet.dart` (`packMode` — galereyada bitta tanlash,
  yangisi eskisining o'rnini oladi; fayl tanlagichda `allowMultiple: false`),
  `pack_add_screen.dart` (faqat birinchi fayl; "Boshqa fayl tanlash" almashtiradi).
Bekor qilingan faylning Telegram'ga yuklangan nusxasi kanalda qoladi (bazaga yozilmagan).

## Xotira halqasi: foizlar tashqarida, bo'laklar sekinroq

`storage_screen.dart`: foiz yozuvlari halqaning TASHQARISIDA (bo'lak markazi yo'nalishida,
bo'lak rangida, yozuv o'lchamiga qarab halqadan uzoqlashadi), 1% dan boshlab ko'rinadi;
chizish maydoni 200 -> 270. Tanlash o'zgarganda bo'laklarning kattalashib-kichrayishi
1200 ms -> 2200 ms (`_move`, `easeInOutCubic`).

## Kesh tozalash animatsiyasi: jo'ja axlatni qutiga tashlaydi

`lib/widgets/trash_chick.dart` (`TrashChickAnimation`): Telegram `utyan_cache` (supurayotgan
jo'ja, Lottie) o'rniga kod bilan chizilgan sahna — jo'ja chap tomondagi eshikni ochib chiqadi,
qo'lidagi axlat qopini qutiga tashlaydi (qopqoq ochiladi, qop yoy bo'ylab uchadi, qopqoq
yopilib chang ko'tariladi), xursand sakraydi va ortiga qaytib kiradi; 4.2 s da takrorlanadi.
`storage_screen.dart` `_ClearingView` shuni ishlatadi (260 x 170). Tashqi fayl yo'q.

## Kesh tozalash animatsiyasi v2: asl `utyan` jo'jasi, haqiqiy burilish

Oldingi (qo'lda chizilgan) jo'ja o'rniga — Telegram `utyan_cache.json` dagi ASL jo'ja.
`lib/widgets/utyan_parts.dart` — Lottie'dan (lottie-web orqali, kadr 106 va 200) olingan
vektor shakllar (tana, bosh, tumshuq, og'iz, ko'zlar, qo'llar, yaltirashlar; asl rang va
qalinlik), avtomatik yasalgan. `trash_chick.dart`:
* Burilish — ikki holat nuqtama-nuqta aralashtiriladi (Lottie'ning o'zidagi bosh burilishi):
  yuz bosh sirti bo'ylab suriladi, uzoq ko'z chetga kirib torayadi, tana yassilanmaydi.
  Chapga — ko'zgudagi nishonga siljish (ko'zlar o'rin almashadi), qo'llar tana ichida kesiladi.
* Lapanglab sakrab yurish, otishdan oldin cho'kish, otishda cho'zilish, "^^" xursandlik
  sakrashi; eshik 3D perspektivada ochiladi (ichkaridan yorug'lik), qopqoq sakrab yopiladi,
  chang (bitta qatlamda) va uchqunlar. 5.4 s, takrorlanadi.

## Galereya bo'sh chiqishi va videoning o'ng chetidagi yashil chiziq

* Galereya ("rasm yoki video topilmadi (ruxsat: authorized, albomlar: 10)"): albomlar
  topiladi, lekin `photo_manager` elementlarni o'qiy olmaydi — u har qatorni
  `File(path).exists()` bilan tekshiradi va hamma ustunlarni majburiy o'qiydi; ba'zi
  telefonlarda hammasi jimgina tushib qoladi. Endi `photo_manager` birinchi sahifada bo'sh
  qaytarsa `tg_attach_sheet.dart` zaxira yo'lga (`_NativeGallery`) o'tadi:
  `MainActivity.kt` "aru/gallery" — `list` (MediaStore.Files, faqat kerakli ustunlar,
  LIMIT'siz, kursor surib; albom = `bucket_id`), `thumb` (`loadThumbnail` / eski
  `Thumbnails`), `copy` (`content://` dan `cacheDir/aru_picked/` ga nusxa;
  `StorageJanitor.dropPicked` o'chiradi). `photo_manager` fayl bermasa ham nusxa shu yo'ldan.
* Yashil chiziq: Android 10+ da Flutter video yuzasi `ImageReader` (`handlesCropAndRotation()
  == false`) — dekoder kadr enini 32/64 ga yaxlitlaganda (360 -> 384) kesish e'tiborsiz
  qoladi, o'ngda yashil bo'shliq, kadr siqiladi. `MainActivity.onCreate` da
  `FlutterRenderer.debugForceSurfaceProducerGlTextures = true` — `SurfaceTexture` yo'li
  kesish/burishni matritsa bilan qo'llaydi. Ilova Skia'da (Impeller o'chiq), shuning uchun
  xavfsiz. Barcha videolarga (pleyer, chat, GIF, kesish oynasi) taalluqli.

## Kesh tozalash animatsiyasi v3: hajmga qarab 4 sahna, uy, panjara, qanotsimon qo'llar

`lib/widgets/trash_chick.dart` (`TrashChickAnimation(bytes: ...)`), bir marta o'ynaydi;
uzunligi `TrashChickAnimation.durationFor(bytes)` — `storage_screen.dart` foiz chizig'i shu
vaqtga bog'langan (halqa ham sekinroq: 3200 ms). Sahna: chapda uy burchagi (tom, devor,
eshik), o'ng chetgacha yog'och panjara, og'zi doim ochiq quti; hammasi kichik, yo'l uzun.
* < 500 MB — eshikdan chiqmaydi: o'zining chap qo'li bilan ramkaning o'ziga nisbatan chap
  tomonini (bizga o'ng ustun) ushlab, o'ng-chapga mo'ralaydi, o'ng qo'li bilan otadi.
* 500 MB – 2 GB — qanot uchida osilgan qopni qutigacha olib borib tashlaydi.
* 2 – 5 GB — qop qutidan katta: to'liq yotgan holda, uchidagi tugunidan ikki qo'llab,
  orqasi bilan yurib qiynalib sudraydi, qutining yoniga qo'yadi, terini artadi.
* 5 GB+ — qop eshikka tiqiladi, chiranadi, otilib chiqadi, jo'ja orqaga uchib o'tirib
  qoladi, ikki qo'li bilan ko'zini ishqalab yig'laydi, yig'lab sudrab boradi va qaytadi.
Qo'llar — har biri alohida, kalta, qanotsimon (asl jo'ja qo'llari kabi). Qop o'lchami
hajmga qarab silliq o'sadi (`_bagSize`). Prototip brauzerda yasalib, Dart'ga ko'chirilgan.

## Kesh tozalash animatsiyasi v4: real fizika (eshik, qopning chiqishi, yiqilish)

`lib/widgets/trash_chick.dart`. Foydalanuvchi izohlari bo'yicha:
* Eshik TASHQARIGA ochiladi (devor ustiga yotadi, orqa yuzi ko'rinadi); jo'ja eshik
  o'yig'idan chuqurlik bilan chiqadi (`_S.depth < 0` — uy ichida: kichikroq, tepada,
  devor orqasida chiziladi), ya'ni eshik orqasidan emas, eshikdan chiqadi.
* Beton poydevor faqat devor ostida; eshik o'yig'ida yer bilan tekis ostona plitasi.
* Katta qop (2 GB+) eshikdan old tomoni (yig'ilgan og'zi) bilan chiqadi (`_End`,
  `_bagEnd`); bo'g'zi (`_neck`) qopdan qo'llargacha toraygan, burmali. Tortilgan tomonga
  burilganda tik o'q atrofida aylanish proyeksiyasi: yon tanasi `sin`, og'iz tomoni `cos`
  bilan — so'ng yonboshlab yotib sudraladi.
* 5 GB+: jo'ja tovonlariga tayanib (`pivotR`) chiranadi; qop bo'shaganda havoga uchmaydi —
  tovoni atrofida orqasiga ag'dariladi (teskari mayatnik, burchak ~u²), dumaloq orqasida
  so'nuvchi chayqalib yotadi, ho'ngrab yig'laydi (keskin nafas, sekin chiqarish), zo'rg'a
  o'tirib, yuzini bizga buradi, navbatma-navbat ko'zini ishqalaydi. Qop tortilgan tomonga
  otilib chiqib, ishqalanish bilan to'xtaydi. Ko'z yoshi tomchilari erkin tushadi va
  yerga tegib sachraydi (`_tearDrops`).
* Orqa ko'rinish (`yaw < -1`): yuz qismlari bosh sirti bo'ylab chetga o'tib, tana
  siluetiga qirqiladi; ikkala qo'l tana orqasida.
* Otilgan qop — haqiqiy parabola, ozgina aylanadi; qo'ldagi qop qanot uchida mayatnikdek
  tebranadi.
Namuna videolar Dart kodining o'zidan (`flutter test` ichida `RepaintBoundary.toImage`)
yozib olingan.

## Tizim animatsiyalari o'chiq telefonda: emoji/stiker to'xtab qolishi va jo'ja tez o'tishi

Telefonda "Animatsiya ko'lami: o'chiq" (dasturchi sozlamasi, ba'zi quvvat tejash
rejimlari) bo'lsa Flutter `disableAnimations = true` qiladi:
* `Image` animatsiyali WebP/GIF ni birinchi kadrda to'xtatadi (`widgets/image.dart`,
  `MediaQuery.maybeDisableAnimationsOf`) — emoji va stikerlar qimirlamaydi. To'plam
  GIF'lari ovozli MP4 (`VideoPlayer`) bo'lgani uchun ishlayveradi.
* `AnimationController` (`AnimationBehavior.normal`) davomiylikni 0.05 ga ko'paytiradi —
  13.5 s lik jo'ja ~0.7 s da o'tib ketadi, halqa ham.
Yechim: `main.dart` `builder` da `MediaQuery.disableAnimations` har doim `false`;
`trash_chick.dart` va `storage_screen.dart` (`_move`) kontrollerlari
`AnimationBehavior.preserve`.

## To'plamga video yuklash: uzunlik chegaralari

`packMaxSeconds` (`pack_video_trim_screen.dart`): GIF — 0 (uzunlik cheklanmaydi, faqat
fayl 5 MB dan oshmasin), stiker — 12 s, emoji — 8 s. 0 bo'lsa kesish oynasi butun
videoni tanlashga ruxsat beradi va "eng ko'pi" yozuvi chiqmaydi. Serverda
(`tool/packs/arunorm.py`) uzunlik tekshirilmaydi — faqat 5 MB.

## BEPUL BO'LIMLAR — YARMI (2026-10)

TALAB (foydalanuvchi): qismi bor bo'limlarning yarmi obunasiz ko'rilsin.
Qismi OLDINROQ joylangan bo'limlar bepul, eng yangi qism joylanganlari
pullik. Bepul bo'limning HAMMA qismi bepul.

- Hisob (`worker/src/lib.rs` -> `free_seasons`): hamma bo'lim
  `MAX(epizod_db.created_at)` o'sish bo'yicha (teng bo'lsa `anime_id`,
  `season_id`) tartiblanadi, birinchi `n / 2` tasi (butun bo'lib
  pastga: 10 -> 5, 3 -> 1, 11 -> 5) bepul. Qismi yo'q bo'lim sanalmaydi.
  Anime chegarasi yo'q — tartib butun ilova bo'yicha.
- Nega `MAX` (oxirgi qism): hozir efirda bo'lgan, yangi qism chiqayotgan
  bo'lim pullik bo'lib qoladi; eski, tugagan bo'limlar bepul.
  (Agar "birinchi qism" bo'yicha kerak bo'lsa — `MAX` ni `MIN` ga almashtiring.)
- Baza yozuvi YO'Q, ustun qo'shilmadi: natija har so'rovda hisoblanadi va
  izolyatda 60 soniya keshlanadi (o'qishni kamaytirish uchun).
- Server to'sig'i: `/api/tg/deliver` — `ep_`/`orig_` fayli bepul bo'limniki
  bo'lsa obunasiz ham beriladi (fayl nomidan bo'lim `season_of_file` bilan
  olinadi), aks holda 402.
- Ilova: `/api/seasons`, `/api/seasons/anime/:id`, `/api/season/:a/:s`
  javobida bo'limda `free` (bool) bor. Pleyer (`video_player_screen.dart`)
  `_canWatch = obuna || seasonIsFree(season)` bo'yicha ochiladi
  (`seasons_repo.dart` -> `seasonIsFree`).
- Eslatma: eski B2 yo'li (`/api/play`) hech qachon obunani tekshirmagan —
  o'zgarmadi.

## MAJBURIY OBUNA KANALLARI (2026-10)

TALAB (foydalanuvchi): bepul bo'limni ochganda ilova (pullikdagi
"obuna oling" oynasi kabi) kanallarga obuna bo'lish uchun RUXSAT so'raydi.
Ruxsat berilishi bilan pleyer ochiladi, ilova orqa fonda (har 5 daqiqada)
foydalanuvchining O'Z Telegram hisobi bilan kanallarga qo'shiladi / yopiq
kanalga so'rov yuboradi. Ruxsatni Sozlamalar -> "Bepul ko'rish" dan
o'chirish mumkin. Asos — `aniraxuzbot15` (`bot/src/handlers/channels.ts`).

- Server: `worker/src/channels.rs`. Jadvallar `channels_db` (public/private,
  `need` limit, `joined` hisob) va `chan_requests` — FAQAT yopiq
  kanalga so'rov yuborganlar (bir marta sanaladi; kanal o'chsa ular ham
  o'chadi). Ochiq kanalga qo'shilganlar jurnalga yozilmaydi — faqat `joined`+1.
- Hisob Telegram hodisalari bilan (ilovaga ishonilmaydi): `chat_join_request`
  (so'rov — `getChatMember` uni ko'rmaydi), `chat_member` (ochiq kanalga
  qo'shildi). Asosiy bot kanalda ADMIN bo'lishi shart ("Foydalanuvchi
  qo'shish", yopiq uchun "Havola orqali taklif qilish"). Webhook `|v3`.
- Yo'llar: `GET /api/channels` (ilova; limiti to'lmaganlar),
  `GET/POST /api/admin/channels` (admin: add/limit/del).
  Ilova bazaga hech narsa YOZMAYDI. Kanallar soni cheklanmagan, tashqi
  havolalar yo'q (foydalanuvchi talabi bilan olib tashlandi).
- Admin: ilovadagi "Majburiy obunalar" (`admin_channels_screen.dart`) va
  ASOSIY bot (admin shaxsiy chatida `/start`, `/kanallar` yoki
  "🔐 Majburiy obunalar"; holat `app_config.chan_wait`). Kodlash botidan
  olib tashlandi. Botda kanal qo'shishda pastda aniraxuzbot15 dagi
  `request_chat` tugmalari: "🤖 Botni kanalga admin qilish" va
  "🆔 Kanal IDsi botga yuborish" (`chat_shared`), forward yoki @username/ID ham
  ishlaydi. Webhook `|v4` (+`callback_query`).
- Ilovadagi admin ekrani Telegram botsiz ishlaydi: @username yoki ID
  berilsa ilova adminning O'Z Telegram hisobi bilan kanalni topib botni
  admin qiladi (`rust_tg_make_bot_admin`, `channels.editAdmin`), keyin
  serverga qo'shadi.
- Ilova: `lib/services/channel_gate.dart` (ruxsat `chan_consent`, bajarilgan
  `chan_done`, ro'yxat 15 daqiqa kesh), Rust `rust_tg_join_channel`
  (`channels.joinChannel` / `messages.importChatInvite`, INVITE_REQUEST_SENT
  = so'rov yuborildi). Pleyer: `_ChannelConsentScreen`.
- Server bepul bo'limni kanalga qo'shilganlik bo'yicha TO'SMAYDI (pleyer
  ruxsatdan keyin darhol ochilishi kerak edi) — to'siq ilovada.

## Sozlamalar: tugma = "yashirish", SyncQueue orqali, kanal ruxsati bazada (2026-10)

**Nima:** Sozlamalardagi statistika tugmalari endi "yashirish" ma'nosida
(yangi hisobda hammasi o'chiq = hammasi ochiq; yoqilgani boshqalarga
ko'rinmaydi). Kanallarga avtomatik obuna ruxsati telefondan bazaga
ko'chdi (`users_db.chan_consent`, `mig_chan_consent`).

**Nega:** tugmalar ishlamasdi — har bosishda `POST /api/me/privacy` +
`/api/auth/me` ketardi, tugmalar javobgacha qulflanardi, worker esa
foydalanuvchi qatorini 60 soniya eslab qolgani (`SESSION_MEMO`) uchun
eski ro'yxatni qaytarib, tugmani orqaga surib qo'yardi.

**Qanday:** `AuthService.updateSettings` holatni darhol telefondagi
hisobga yozadi va `SyncQueue.putSettings` ga bitta qator qo'yadi
(kalit `settings`, oxirgisi qoladi). `/api/sync` paketidagi `settings`
`{hidden_stats, chan_consent}` bitta `UPDATE users_db` bo'ladi va
`SESSION_MEMO` tozalanadi. Serverdan kelgan hisobga yuborilmagan /
5 daqiqa ichidagi mahalliy sozlama ustun (`_keepLocalSettings`).
`/api/me/privacy` eski ilovalar uchun qoldi. Eski telefondagi
`chan_consent` bir marta bazaga ko'chiriladi (`ChannelGate._sync`).

Fayllar: `lib/screens/settings_screen.dart`, `lib/services/auth_service.dart`,
`lib/services/sync_queue.dart`, `lib/services/channel_gate.dart`,
`worker/src/lib.rs`.

## Bepul ko'rish sekinligi (2026-10)

Foydalanuvchi: "bepul ko'rishda ilova juda sekin, pleyer va qismlar sekin
yuklanyapti".

- `free_seasons` (worker) butun `epizod_db` ni GROUP BY bilan o'qiydi va
  natija faqat bitta izolyatda 60 s turardi — bo'limlar ro'yxati, bo'lim
  oynasi va obunasiz odamning HAR BIR `/api/tg/deliver` so'rovi ko'pincha
  shu og'ir so'rovni kutardi. Endi Cloudflare Cache API'da 10 daqiqa
  (`FREE_SEASONS_EDGE_URL`) + izolyatda 2 daqiqa. Yangi qism qo'shilganda
  bepul ro'yxat 10 daqiqagacha kechikib yangilanadi.
- `seasonIsFree` (ilova) obunasiz odamda pleyerning har chizishida butun
  bo'limlar ro'yxatini aylanardi — endi indeks (`_freeIndex`).
- `ChannelGate`: ruxsat berilgan zahoti (pleyer endi ochilayotganda)
  kanallarga qo'shilish boshlanib, video bilan Telegram ulanishini
  talashardi — birinchi aylanish 45 s keyin (`_firstDelay`).

## Admin: kodlash navbati va jonli log — bazaga yozuvsiz (2026-10)

Foydalanuvchi: admin panelida hozir kodlanayotgan qism (bo'lim nomi, bo'lim,
qism, encode log statistikasi) va pastida navbat (surat, nom, bo'lim, qism);
"yangilash tugmasini bosganda worker oxirgi logni so'rab olsin, panel bot ham
shunday — faqat so'ralganda; shunda har 10 daqiqada bazaga log yozish shart
bo'lmasdi".

- **Runner (`tool/encode/run.py`)**: davriy `heartbeat` olib tashlandi.
  `claim` da `no_heartbeat: true` -> ijara butun run'ga
  (`ENCODE_RUN_LEASE_MS`, 5 soat 20 daqiqa). Jonli holat log kanalidagi
  bitta QADALGAN xabarda (`#arustatus`, `StatusPin`): ~15 s da tahrirlanadi,
  birinchi qatorlar `kalit: qiymat` (run, job `a/s/e`, num, progress,
  updated), `---` dan keyin oxirgi 14 log qatori (ffmpeg qatori
  almashtiriladi). Runner `claim` javobidagi `status_bots` ni (kodlash va
  asosiy bot) log kanaliga o'zi admin qiladi va `log_chat` ni worker'ga
  aytadi (`app_config.encode_log_chat`, faqat o'zgarganda yoziladi).
- **Worker**: `GET /api/encode/admin` (navbat: running/queue/errors, bitta
  JOIN so'rov), `GET /api/encode/live` (`encode_live`: `getChat` ->
  `pinned_message` + GitHub run va qadamlari). Ikkalasi faqat admin.
  Kodlash botining "Holat" xabari ham jonli holatni so'raganda o'qiydi
  (`encode_live_text`). Uzun ijarali run o'lgan-o'lmaganini `encode_kick`
  har safar GitHub'dan tekshiradi; `quality` ijarani qisqartirmaydi
  (`MAX(lease_until, ?)`). Eski runner'lar uchun `heartbeat` yo'li qoldi.
  Log kanali sirdan ham berilishi mumkin: `ENCODE_LOG_CHANNEL`.
- **Ilova**: `lib/screens/admin_encode_screen.dart` (admin paneli ->
  "Kodlash navbati"), yuqorida "Yangilash" tugmasi.
- `run.py` avtoencode repo'ga `sync-packs.yml` orqali avtomatik ko'chadi
  (`tool/packs/sync_repo.py` -> `FILES`).

## Kodlash statistikasi — real vaqtda va batafsil (2026-10)

Foydalanuvchi: "encode statistikani to'liq mayda detallarigacha aniq va real
timeda ko'rsat; videoni qancha daqiqa kodlangani va jami qancha daqiqaligi
ham ko'rsatilsin".

- **Runner**: qadalgan `#arustatus` xabari endi ~3 s da tahrirlanadi
  (`STATUS_INTERVAL_SEC`, FloodWait bo'lsa oraliq o'zi 1 s ga uzayadi,
  15 s gacha). Sarlavhaga `data: {JSON}` qatori qo'shildi: `cur` (ffmpeg:
  foiz, `out_s`/`dur_s` — kodlangan va jami soniya, kadr, kadr/s, tezlik,
  bitreyt, hajm, taxminiy hajm, o'tdi/qoldi, drop/dup, q), `ladder`
  (har sifat: kutmoqda/kodlanmoqda/yuklanmoqda/tayyor, CRF, hajm, kb/s,
  kodlash vaqti), `src` (o'lcham, davomiylik, hajm, bitreyt, kadr/s, jami
  kadr, kodek), `xfer` (yuklab olish/Telegram'ga yuklash: foiz, MB, MB/s,
  qoldi), `sys` (CPU %, yadro, yuklama, RAM, bo'sh disk), `run_s`,
  `job_started`, `attempt`. 4000 belgidan oshsa eski log qatorlari tashlanadi.
- **Rust**: `rust_tg_read_pinned(kanal_id)` — adminning o'z hisobi bilan
  qadalgan xabarni o'qiydi (`channels.getMessages`; kanal va xabar raqami
  eslab qolinadi, raqam 30 s da `getFullChannel` bilan yangilanadi).
- **Ilova** (`admin_encode_screen.dart`): ekran ochiq turganda har 2 s da
  to'g'ridan-to'g'ri Telegram'dan o'qiydi ("Jonli" yashil nuqta). Bo'lmasa
  (admin kanalda emas / Telegram ulanmagan) — worker orqali 10 s da
  ("Worker orqali", sariq). GitHub qadamlari 30 s da, navbat — qism
  almashganda. Worker `/api/encode/live` endi `log_chat` va `data` ham
  beradi.

## Pullik belgisi hamma kartochkada; bepul/pullik — faqat ko'rinadigan qismlar bo'yicha (2026-10)

- `lib/widgets/paid_badge.dart`: `PaidBadge` (oltin toj + "Pullik", `compact` —
  faqat toj) va `PaidMark` (bo'lim pullik bo'lsa belgi). Ishlatiladi:
  `SeasonCard` (bosh sahifa, katalog, sevimlilar, statistika — belgilar
  o'chiq kartochkada ham), qidiruv, pleyerdagi bo'limlar ro'yxati, tarix
  (`AnimeRow`, `EpisodeRow`), yuklanmalar (bo'lim kartasi va qism qatori),
  izohlar statistikasidagi poster. `seasonIsPaid` ma'lumot yo'q bo'lsa
  `false` (ilova bilgan bo'limlar: `/api/seasons` dagi 200 ta + `free`
  maydoni kelgan obyektlar).
- `free_seasons` (worker): faqat ILOVADA KO'RINADIGAN qismlar sanaladi —
  kodlangan sifati (`url_*`) bor yoki kalitli asl video (`origin_key` 32
  belgi). Kalitsiz, hali kodlanmagan asl video (kodlash boti) bo'limni pullik
  qilmaydi. Qism ko'rinadigan bo'lganda (`/api/epizods` qo'shish/tahrir,
  `encode/quality`, kalitli `encode/queue`) `free_seasons_forget` keshni
  tozalaydi (izolyat + shu data markaz; boshqalarida 10 daqiqagacha).

## Kanal qo'shish oynasi, aniq limit, grafiklar, statistika bloklari, kodlash holati tuzatildi (2026-10)

- **Kodlash holati ko'rinmasdi**: Pyrogram matnni Markdown deb o'qib, `---`
  ajratgich va `--:--` ni "tagiga chizish" qilib yo'qotardi. Endi qadalgan
  xabar `ParseMode.DISABLED` bilan yuboriladi/tahrirlanadi; o'quvchilar
  (worker `parse_status_pin`, ilova `parseStatusPin`) faqat tanish
  kalitlarni sarlavha deb oladi, qolgani log.
- **Worker keshi**: `/api/encode/live` natijasi Cloudflare keshida 30 s
  (`ENCODE_LIVE_CACHE_URL`), log kanali izolyatda eslab qolinadi — Turso
  o'qilmaydi. Ilovaning asosiy yo'li baribir worker'siz (qadalgan xabar).
- **GitHub o'qish tokeni**: `GH_READ_TOKEN` (fine-grained, faqat kodlash
  repo'si, "Actions: Read-only"; GitHub secret -> `deploy-worker.yml`).
  Bo'lsa `/api/encode/live` uni faqat adminga `gh` bilan beradi va worker
  GitHub'ga o'zi bormaydi; ilova run qadamlarini GitHub'dan 30 s da so'raydi.
  Yozish huquqli `GH_ACTIONS_TOKEN` ilovaga hech qachon chiqmaydi.
- **Kanal qo'shish** (`admin_channel_add_screen.dart`): qidiruv maydoni ->
  `rust_tg_channel_info` (admin hisobi: `resolveUsername` / suhbatlardan ID /
  `checkChatInvite`; `getFullChannel` — obunachilar, tavsif; kichik surat
  base64, diskka yozilmaydi) -> ma'lumot kartasi -> tur, limit (aniq son yoki
  cheksiz) -> qo'shish (bot admin qilinadi, server tekshiradi).
- **Limit aniq**: `op: set` (`channels::set_limit`), ilovada son + "Cheksiz";
  botda "Limitni belgilash" / "Cheksiz qilish" (oshirish/kamaytirish olib
  tashlandi, eski tugmalar ishlab turadi).
- **Grafiklar** (`lib/widgets/trend_chart.dart`, `GET /api/stats/series`,
  5 daqiqa kesh): 24 soat / 7 / 30 kun / hammasi, bosib-surib aniq qiymat,
  o'zgarish foizi. Ko'rsatkichlar: `users` (yangi hisoblar), `anime_views`,
  `season_views`, `views`, `watch_ms`, `traffic`, `chan` / `chan:<id>`
  (faqat admin). Kanal chelaklari `channels.rs` hodisalarida yoziladi
  (yangi qo'shilish/so'rovga +2 qator).
- **Statistika bloklari**: foydalanuvchilar, anime / bo'lim / qism
  ko'rishlar, ko'rish vaqti, trafik — kunlik/haftalik/oylik/umumiy + grafik.
  `anime_views` / `season_views` `sync_route` da odam boshiga bitta
  sanaladi; o'tmishi bir marta tomosha tarixidan `0000-00-00` chelagiga
  yoziladi (`mig_view_seed`, faqat "umumiy" ga kiradi).

## Kodlash holati — har daqiqada worker xotirasiga (`EncodeLive`, Durable Object) (2026-10)

Foydalanuvchi: "har daqiqada Cloudflare keshga oxirgi to'liq log yozilsin va
to'g'ridan-to'g'ri kesh orqali ko'rsatilsin" (Turso'siz).

- Oddiy Cloudflare keshi (Cache API) har data markazda alohida: runner (AQSh)
  yozgani O'zbekistondagi admin so'roviga ko'rinmasdi. Shu sabab
  `EncodeLive` Durable Object (SQLite sinf, bepul rejada ishlaydi, migratsiya
  `v3`, bog'lanish `ENCODE_LIVE` — `wrangler.toml` da ham asosiy, ham
  `env.production` da). Bitta nusxa (`get_by_name("encode")`): oxirgi matn
  xotirada + o'z omborida (`s` kaliti).
- Runner (`run.py` -> `StatusPin.send_worker`): `#arustatus` matnini
  `POST /api/encode/push` (ENCODE_TOKEN, imzo tekshiruvidan ozod) ga har
  `WORKER_PUSH_SEC` (60) soniyada, qism boshida va oxirida yuboradi.
- `encode_live_fresh`: avval `EncodeLive` (5 daqiqadan yangi bo'lsa,
  `source: "cache"`), bo'lmasa Telegram qadalgan xabari (zaxira).
- Ilova: `/api/encode/live` ni ~16 s da so'raydi; `source != cache` bo'lsa
  zaxira — qadalgan xabarni o'zi o'qiydi.

## Tarix kadrlari Telegram'dan, yuklash tezligi, izohlar statistikasi (2026-10)

- **Tarixda kadr o'rniga poster / sekin kadrlar.** Videolar endi
  Telegram'da, worker fayl baytlarini bermaydi. Tarix kadrlari endi
  yozishmadagidek mahalliy Telegram manzili (`/tg/0/<nom>`) orqali
  olinadi: `WatchHistory` da BITTA navbat (`_runQueue`), tarixdagi
  tartib bo'yicha yuqoridan pastga, bittadan; navbatdagi 4 ta qism bitta
  `/api/tg/deliver` bilan (`TelegramService.deliverMany`). Baytlar
  xotiraga o'qiladi, diskka yozilmaydi; kadr uchun ochilgan papka
  (`meta.json`) ish tugagach o'chiriladi (`ThumbDirGuard`) — yuklanmalarda
  "qisman yuklangan" ko'rinmaydi. Kadr eng baland sifatli fayldan
  (`HistoryItem.thumbUrl`, faqat telefonda), 1280 px, JPEG 90.
- **Tezlik.** Pleyerning oldindan olishi `AHEAD` 2 -> 4 bo'lak (1 daqiqa
  qoidasi o'z kuchida), `DL_CONNS` 4 -> 6, `MAX_INFLIGHT` 24 -> 32.
  FLOOD_WAIT ko'paysa birinchi navbatda shularni qaytaring.
- **Majburiy kanallar** (`channel_gate.dart`): bittadan, 5-10 s oraliq,
  pleyer ochiq (`VideoGate.busy`) paytda to'xtaydi, Telegram limitida
  30 daqiqa kutish (`chan_pause`).
- **Izohlar statistikasi**: worker `media_file` ham beradi, ekranda
  GIF/stikerning o'zi (`PackMediaView`) va "Izoh/Javob · GIF yubordi ·
  yozdi" yozuvi.
- **Bosh sahifa kartasi**: "N-bo'lim · M ta qism" (`epizod_count`), oq rangda.
- **Xotira halqasi**: bo'laklar orasida bo'shliq yo'q, ulushlar
  siljiydi va burchaklar har kadrda ketma-ket yig'iladi; tozalashda
  halqa parda foizi bilan bir sur'atda bo'shaydi; foizlar `000.00`.

## Chat so'rovlari kesh belgisi bilan, Turso o'lchovi (2026-10)

- **Chat belgisi.** `chat_messages`/`chat_threads` ga har qanday yozuv
  `turso_after` da avtomatik seziladi va Cloudflare keshiga "o'zgarish
  vaqti" (`CHAT_MARK_URL`) qo'yiladi. `chat_wait` endi har 2 s da shu
  belgini tekshiradi, Turso'ni faqat belgi o'zgarganda yoki 45 s da bir
  marta (`CHAT_FALLBACK_MS`) o'qiydi; ilova `mk`/`at` ni qaytarib yuboradi.
  `chat_unread` belgi o'zgarmagan bo'lsa 3 daqiqagacha (`UNREAD_FALLBACK_MS`)
  bazaga bormaydi. Kesh har ma'lumot markazida alohida — zaxira
  tekshiruvlar shu uchun; belgi yo'qolsa "o'zgardi" deb hisoblanadi.
- **O'lchov.** Har Turso buyrug'idan keyin `aru_tq r=.. w=.. <SQL>` jurnal
  qatori (faqat `wrangler tail` ko'radi, saqlanmaydi).
  `.github/workflows/worker-stats.yml` (+ `tool/stats/tail_report.py`)
  N daqiqa tinglab, yo'l va SQL bo'yicha o'qilgan/yozilgan qatorlar
  jadvalini "Summary" ga chiqaradi. Keyingi tejash shu jadvalga qarab.
- **Webhook tejashlari** (worker hisobotidan keyin): sxema belgisi
  Cloudflare keshida (`schema_mark_url` — worker kodining xeshi; kod
  o'zgarsa jadvallar bir marta qayta tekshiriladi); deyarli o'zgarmaydigan
  `app_config` kalitlari izolyat xotirasida 10 daqiqa (`CONFIG_MEMO_KEYS`,
  admin suhbat holatlari ATAYLAB yo'q); yopiq kanal so'rovi bitta
  `INSERT ... SELECT ... RETURNING`, yangi bo'lsa "+1" bilan
  (`channels.rs` -> `record_request`, `COUNT(*)` sanashlar olib tashlandi).
- **Zaxira tekshiruv 10 daqiqa** (foydalanuvchi talabi): `CHAT_FALLBACK_MS`,
  `UNREAD_FALLBACK_MS`, `COMMENTS_FALLBACK_MS` = 10 daqiqa. Bir ma'lumot
  markazida o'zgarish 2 s da seziladi; boshqa markazdan yozilgani 10
  daqiqagacha kechikishi mumkin.
- **Izohlar ham kesh belgisida** (`COMMENTS_MARK_URL`, `comments_db` /
  `comment_likes` ga yozuv avtomatik seziladi). `GET /api/comments/:a/:s`
  ilova `mk`/`at` yuborsa va o'zgarish bo'lmasa `{"same":true}` (Turso'siz,
  diskdagi ro'yxat qoladi). Oyna ochiq turganda `GET /api/comments/wait?mk=`
  (~20 s, Turso'siz) — yangi izoh/layk bo'lsa ro'yxat yangilanadi.
  Belgi umumiy (hamma bo'limlar uchun bitta).
- **Tejash hisobi.** Turso'ga borilmagan har holatda worker `aru_sv <tur> <n>`
  jurnal qatorini yozadi (`saved()`); `tool/stats/tail_report.py` ularni
  "Tejamkor tizim" jadvaliga jamlaydi va `tool/stats/baseline.json`
  (tejashdan OLDINGI o'lchov) bilan daqiqasiga solishtiradi. `sxema`
  uchun 7 — taxminiy (1 paket + 6 migratsiya belgisi).
- **Kadr ishi diskka hech narsa yozmaydi.** Baytlar xotirada edi, lekin
  kesh papkasi va `meta.json` (hajm) yozilib, keyin o'chirilardi
  (`ThumbDirGuard`) — bir paytda boshlangan yuklab olish yo'q papkaga
  yoza olmay qotardi. Endi `serve_thumb` papka ochmaydi va `meta.json`
  yozmaydi (hajm xotirada), diskdan faqat o'qiydi; `ThumbDirGuard` olib
  tashlandi. `write_full_chunk` papka yo'q bo'lsa qayta yaratadi.
- **Pleyer `AHEAD` yana 2** (4 bo'lganda parallel bo'laklar orasida
  teshik qolardi). `DL_CONNS`/`MAX_INFLIGHT` oshirilgani yuklab olish uchun qoladi.
- **Yuklab olish (2026-10-06).** Asl sabab: kadr navbati `/tg/0/<nom>`
  orqali o'qiganda fayl hali bot chatida ko'rinmasa `note_failure` uni 2
  daqiqaga Telegram'dan chetlatardi (`route_url` -> `None`) va shu payt
  boshlangan yuklash worker yo'liga o'tib qotardi (hisobotda
  `GET/HEAD /api/image/ep_..mp4`). Endi kadr `/tg/t/<nom>` bilan o'qiydi
  (`soft` — xatosi chetlatmaydi), worker'ga umuman bormaydi;
  `deliverMany` `_missing` ga yozmaydi. `download_manager.dart` va
  `DL_CONNS=4`/`MAX_INFLIGHT=24` avvalgi (b786d4b) holatiga qaytarildi.
- **Kartadagi "N ta qism"** — faqat ko'rinadigan qismlar
  (`visible_episode_counts`, `/api/seasons` da `epizod_count` almashtiriladi;
  isolyat + Cloudflare keshi 10 daqiqa, `free_seasons_forget` tozalaydi).

## "O'zgarish bormi?" tizimi — Durable Object `MarkHub` (2026-10-06)

- **Belgilar.** Turso'ga har bir yozuv (`turso_after`, `w>0`) o'zgargan
  jadval belgisini (`t:<jadval>` -> ms) yangilaydi: `MarkHub` (butun dunyo
  uchun bitta DO, `wrangler.toml` migratsiya `v4`, binding `MARK_HUB`) +
  zaxira Cloudflare keshi (`MARK_CACHE_BASE`). Oylik chegara YO'Q
  (foydalanuvchi talabi). DO javob bermasa — 5 daqiqa zaxira rejimi;
  yetib bormagan belgilar `MARK_PENDING` da turib keyingi murojaatda ketadi.
- **ETag.** `main()` da `etag_tables(path)` dagi GET yo'llari (bo'limlar,
  bo'lim, tarix, sevimlilar, `me/stats`, `user/:id/stats/:kind`): versiya =
  jadvallar belgisi + token xeshi (DO'siz rejimda + 10 daqiqalik bo'lak).
  `If-None-Match` mos kelsa 304 — Turso'ga borilmaydi. Ilova:
  `lib/services/etag_http.dart` (javob + versiya diskda, 304 da diskdagisi).
  YANGI GET yo'l qo'shilsa, uning jadvallarini `etag_tables` ga yozing.
- **Chat/izohlar/nuqta** endi `marks_get` (DO) dan: DO ishlasa 10 daqiqalik
  zaxira Turso tekshiruvi yo'q; kutish ichidagi 2 s lik tekshiruv — kesh.
- **Faqat o'zgargan qatorlar (delta), tomosha tarixi.** `watch_history_db.srv_at`
  (SERVER vaqti, `mig_hist_srv_at`; `updated_at` ilova soatidan, ishonchsiz).
  `GET /api/history?since=` — `MarkHub` bo'yicha: tarix o'zgarmagan bo'lsa
  Turso'siz bo'sh delta; faqat tarix o'zgargan bo'lsa `srv_at > since`
  (indeks `idx_history_srv`); nomlar/qismlar o'zgargan, qator o'chirilgan
  (`del_<jadval>` belgisi — har `DELETE` da) yoki DO ishlamasa — to'liq.
  Ilova: diskdagi ro'yxatga qo'shadi, 30 s ustma-ust, sutkada bir to'liq.
- **Indeks `idx_history_upd (user_id, updated_at DESC)`** — statistikadagi
  qismlar sahifasi endi faqat o'z 40 qatorini o'qiydi (ilgari butun tarixni
  saralardi: hisobotda 2 so'rov = 630 qator).

## Avto-kodlash: ishlayotgan run "o'lgan" deb hisoblanardi (2026-10)

Belgi: botga har 10 daqiqada "Cron: Kodlash boshlandi (GitHub Actions ishga
tushirildi)" kelardi, navbat 3 soatda 31 dan 30 ga zo'rg'a tushdi.

Sabab (`worker/src/lib.rs` -> `encode_kick`): ishlayotgan ish (ijarasi uzun,
`no_heartbeat`) har cron'da GitHub'dagi `runs?status=in_progress` ro'yxati
bilan tekshirilardi. GitHub bu filtrni qidiruv indeksidan beradi va
ishlayotgan run unda ko'rinmay qolardi -> worker ishni navbatga qaytarar
(`attempts-1` bilan — hech qachon "xato" bo'lmasdi), yangi run ochardi; eski
run esa sifatni yozolmay (409 `job_lost`) qismni boshidan kodlardi.

Tuzatish:
- `encode_run_alive`: ishni olgan run (`runner` = `<run_id>-<attempt>`)
  to'g'ridan-to'g'ri `GET /actions/runs/<id>` bilan tekshiriladi; faqat
  `completed` yoki 404 bo'lsa ish bo'shatiladi (aniqlab bo'lmasa tegilmaydi).
- `encode_gh_busy`: ishga tushirishdan oldingi tekshiruv ham `?status=`
  filtrisiz — oxirgi 10 run olinib, `status != completed` qidiriladi.
- Cron bo'shatganda urinish QAYTARILMAYDI: run haqiqatan o'lsa (xotira va
  h.k.) qism 3 martadan keyin "xato" bo'lib, navbatni to'smaydi. Qo'lda
  `release` va bekor qilish (`cancelled`) avvalgidek urinish sanamaydi.

## Kodlash botida "Post kodlash" bo'limi (2026-10)

YANGILANISH (foydalanuvchi talabi): post endi H.265 bilan kodlanadi —
`anime` repodagi "Encode (H265)" sozlamalari: libx265, sof CRF — har qanday
sifatda 30 (foydalanuvchi talabi; `H265_CRF`, `H265_PRESET` — `post.workflow.yml`),
`hvc1` tegi (Telegram/iOS'da ochilishi uchun), audio AAC 128k. Pastdagi
"H.264" so'zlari shu yangilanishgacha bo'lgan holat.

TALAB (foydalanuvchi): `anime` repodagi Encode tizimini kodlash botiga
ulash. Botda ikki bo'lim: "📱 Ilova uchun" (eski oqim) va "🎬 Post kodlash".
Rasm va video yuboriladi, izohida post nomi; Actions videoni H.264 bilan
kodlab (boshida 3 soniya rasm, burchakda logotip), logotip fayli nomidagi
ID'ga Telegram'da ochiladigan VIDEO qilib, tagida aynan shu nom bilan
yuboradi. Navbatdagi postlar tugma; bosilganda rasm + video, tagida inline
tahrirlash/o'chirish. Jarayon botda ilovadagi kodlash holati kabi jonli
ko'rinadi. Kodlash yangi akkauntdagi `avtoencode` repoda, avto-kodlashdan
ALOHIDA workflow'da ("ikkalasi alohida narsalar"). Hajm chegarasi yo'q.

Qanday ishlaydi:
- `worker/src/postbot.rs` — bot qismi, `kick` va
  `/api/post/{claim,check,progress,finish}`. `/start` — bosh menyu. Post
  rejimi `app_config.encbot_mode='post'`, yig'ilayotgan post `post_draft`,
  tahrirlash `post_edit` (`<id>:n|p|v`), takror himoyasi `post_last_src`
  (Telegram webhook'ni qayta yuborsa bitta post ikki marta tushmasin).
  Rasm va video admin chatidan yopiq kanalga `copyMessage` (fayl worker'dan
  o'tmaydi). Navbat — jadval `post_jobs`.
- `kick` (post qo'shilganda, "qayta urinish"da va cron'da) — `GH_REPO`
  dagi `post.yml` ni ishga tushiradi. Avto-kodlash navbatiga (`encode_kick`,
  `peek`) postlar QO'SHILMAYDI.
- `tool/post/post.py` — run navbat bo'shaguncha postlarni ketma-ket ishlaydi.
  Claim'da worker botga holat xabarini yuboradi va raqamini beradi; runner
  uni har ~10 s `/api/post/progress` bilan tahrirlaydi (bosqich, foiz-chiziq,
  tezlik, kadr/s, bitreyt, hajm, o'tgan/qolgan vaqt) — bazaga yozuvsiz.
  Tugaganda (yoki xato/bekor bo'lsa) shu xabar yakuniy holatga o'tadi.
- `tool/post/encode.sh` — `anime/scripts/encode.sh` (H.264) bilan bir xil
  ffmpeg buyrug'i, faqat kirish fayl yo'llari bilan; audiosiz videoga
  jimlik qo'yiladi. `tool/post/6076003760_logo.png` — `anime/anipng` dan;
  fayl nomidagi ID = video yuboriladigan odam.
- `tool/post/post.workflow.yml` -> `avtoencode/.github/workflows/post.yml`,
  `tool/post/*.py|*.sh|*.png` bilan birga `sync-packs.yml`
  (`tool/packs/sync_repo.py`) orqali avtomatik ko'chadi; `setup_repo.py`
  ham yuklaydi.
- SESSIYA: `post.yml` ham `tool/encode/session.enc` ni ishlatadi
  (foydalanuvchi: hamma sessiya bitta akkauntniki). Post va avto-kodlash bir
  vaqtda Telegram'ga ulanadi — foydalanuvchi 6 oy davomida muammo
  kuzatmagan. Sessiya bekor qilinsa (AUTH_KEY_DUPLICATED) — post uchun
  alohida sessiya yaratib, `post.yml` ga berish kerak bo'ladi.
- Post 3 marta xato bersa `error` — bot navbatida ❌, "Qayta urinish" /
  "O'chirish". Kodlanayotgan postni faqat o'chirish mumkin: `post.py`
  yuborishdan oldin `/api/post/check` qiladi (409 — yuborilmaydi).

## Kodlash botida "Anibla yuklash" bo'limi (2026-10)

TALAB (foydalanuvchi): bot anibla.uz ga login/parol bilan kiradi, "Izlash"
tugmasi; nom yozilganda topilgan videoning mavjud sifatlari tugma bo'lib
chiqadi; bosilganda video GitHub Actions (IKKINCHI akkaunt, `GH_REPO`) orqali
yuklab olinib, BOT CHATIGA yuboriladi. Login/parol AES bilan shifrlangan faylda.

Qanday ishlaydi:
- `worker/src/anibla.rs` — kodlash botining uchinchi bo'limi
  ("🎞 Anibla yuklash", bosh menyuda). Rejim `encbot_mode='anibla'`: yozilgan
  matn — qidiruv. Natijalar inline tugma (📺 serial / 🎬 film) -> muqova va
  tavsif -> fasl -> qismlar (30 tadan sahifa) -> sifatlar (1080p/720p/...,
  taxminiy hajmi bilan). Tugmalarda faqat raqamlar (`z?:<nav>:...`,
  `callback_data` 64 bayt); oxirgi qidiruv `app_config.anibla_nav` da (har
  qidiruvda bitta yozuv), eski qidiruv tugmasi bosilsa "qaytadan izlang".
- Sayt API (sayt JS kodidan aniqlangan, `anibla.rs` boshidagi izoh):
  `api/backend/api/v1` — qidiruv, fasl, qism ro'yxati loginsiz; qism/film
  videosi (`episodes/<slug>/<fasl>/<qism>`, `movies/<slug>`) LOGIN bilan
  (cookie `access_token`). Token 14 kun, `app_config.anibla_token` + izolyat
  xotirasi; 401 bo'lsa qayta kiradi. `video` -> `?format=api` -> HLS master
  m3u8 -> sifatlar. Playlist va bo'laklar loginsiz ochiladi.
- NAVBAT (foydalanuvchi talabi: "alohida workflow'da; videolar ko'p bo'lsa
  bazada navbatda tursin va ketma-ket yuklansin"): sifat bosilganda jadval
  `anibla_jobs` ga bitta yozuv (variant m3u8, izoh, fayl nomi, chat, holat
  xabari), takror bosish himoyasi (shu url navbatda bo'lsa qo'shilmaydi).
  `anibla::kick` (qo'shilganda, "qayta"da va cron'da) — `GH_REPO` dagi
  ALOHIDA `anibla.yml` ni ishga tushiradi (`anibla_kicked_at` atomik belgi,
  concurrency `arugram-anibla`). Run navbat bo'shaguncha `/api/anibla/claim`
  bilan videolarni KETMA-KET oladi (ijara 130 daq, 3 urinish, keyin `error`).
  "📋 Yuklash navbati" — ro'yxat, olib tashlash (`zx`), qayta urinish (`zr`).
  Video baytlari worker'dan O'TMAYDI.
- `tool/anibla/download.py`: ffmpeg `-c copy` (qayta kodlamasdan) mp4 ga
  yig'adi, `tool/encode/session.enc` sessiyasi bilan yopiq kanalga yuklaydi,
  `/api/anibla/done` — kodlash boti uni bot chatiga `copyMessage` qiladi,
  kanal postini va navbat yozuvini o'chiradi. Jonli holat — `/api/anibla/progress` (bazaga
  yozuvsiz). 2 GB dan katta fayl — xato ("pastroq sifatni tanlang").
- `tool/anibla/anibla.workflow.yml` va `download.py` -> avtoencode repo
  (`sync-packs.yml`, `tool/packs/sync_repo.py`). `creds.enc` KO'CHIRILMAYDI.
- LOGIN/PAROL: `tool/anibla/creds.enc` — JSON `{site, login, password}`,
  AES-256-CBC + PBKDF2 (200 000), `gh_token.enc` bilan bir xil format va
  KALIT ham bir xil — `ENCODE_TOKEN` (foydalanuvchi: "qolgan fayllar shu kalit
  bilan"; alohida `ANIBLA_KEY` secret qo'yilsa, o'sha ustun). `deploy-worker.yml` uni
  ochib worker secret `ANIBLA_CREDS` qiladi (`creds.enc` o'zgarsa deploy
  avtomatik). Yangilash: `ANIBLA_KEY=<ENCODE_TOKEN> bash tool/anibla/set_creds.sh`.
- JONLI HOLAT (foydalanuvchi talabi): holat xabari har 5 soniyada tahrirlanadi
  (`LIVE_SEC`, alohida oqimda — yuklash to'xtamaydi) va ichida Actions
  log'ining oxirgi 8 qatori ("📜 Log", `LOG_LINES`): bosqichlar, har 10 s
  foiz/tezlik, ffmpeg ogohlantirishlari. Telegram "retry after N" bersa,
  `/api/anibla/progress` N ni qaytaradi va runner shuncha kutadi.
- BO'LIMLAR (foydalanuvchi talabi: "saytdagidek kategoriyali tugmalar, nom
  yozib qidirish ham tursin"): "📂 Bo'limlar" — inline tugmalar: "🆕 Oxirgi
  yuklanganlar" (filtrsiz `media/mobile`, sayt yangilarini boshida beradi) va
  saytning `GET categories` ro'yxati (Ongoing, Hamma animelar, Yakunlangan,
  Anime filmlar, ...; kodga yozilmagan, saytdan olinadi). `zc:<id>` ->
  `media/mobile?categories=<id>` — qidiruv bilan bir xil ro'yxat/sahifalash
  (`listing`, `anibla_nav.c/lt`).
- Navbat: har bir kutayotgan videoni (`zx`) yoki hammasini birdan (`zxa`)
  o'chirish; yuklanayotganiga tegilmaydi.
- PASTKI PANEL (foydalanuvchi talabi: "ro'yxatlar inline emas, pastki
  paneldan; anime nomlari qatorga 1 ta, qismlar 3 ta, sifat 1 ta"): bo'limlar,
  anime/film ro'yxati, fasllar, qismlar (30 tadan sahifa, "◀️ Oldingi" /
  "Keyingi ▶️"), sifatlar — hammasi reply keyboard. Tugma MATNI keladi:
  anime — `Item::label()` bilan, fasl — "🗂 ", qism — "▶️ N-qism", sifat —
  "⬇️ 720p ..." prefiksi; qaysi ro'yxat ochiqligi `anibla_nav.v`
  (list|seasons|eps|q) va `i/j/ep/k` da (har qadamda bitta yozuv).
  "⬅️ Orqaga" — bir qadam orqaga. Inline faqat navbat tugmalarida qoldi;
  eski inline ro'yxat tugmalari "pastki panelda" deb javob beradi.
- JONLI HOLAT KO'RINMASDI (foydalanuvchi: "5 soniyalik log ko'rsatilmayapti"):
  holat xabari "navbatga qo'shildi" xabari bo'lib yuqorida qolib ketardi.
  Endi `claim` chat OXIRIGA yangi holat xabarini yuboradi (`status_msg`
  yangilanadi), "📋 Yuklash navbati" ham yuklanayotgan video holatini ro'yxat
  ostiga ko'chiradi. `/api/anibla/progress` joriy `status_msg` ni bazadan
  O'QIYDI (runner o'zgarmadi). Navbat ro'yxatida izoh qatori ("❌ xato")
  olib tashlandi — har qatorda holat so'z bilan (navbatda/yuklanmoqda/xato).
- TEZ YUKLASH (foydalanuvchi: "sayt 10+ MB/s bera oladi"): `download.py`
  variant playlist bo'laklarini `PARALLEL` (12) tadan bir vaqtda yuklaydi,
  tartib bilan bitta .ts ga qo'shadi, ffmpeg `-c copy` bilan mp4 qiladi.
  Sinov (720p, 278 bo'lak, 278 MB): ~11.8 MB/s (avval bitta oqim ~2 MB/s).
  Playlist shifrlangan / fMP4 / bayt oralig'i bo'lsa — eski ffmpeg usuli
  (`download_ffmpeg`). Telegram'ga yuklash hali bitta ulanishda.
- HOLAT KO'RINISHI (foydalanuvchi: "Post kodlashdagidek chiqsin"): worker
  `live_text` — qalin sarlavha "⬇️ Yuklash #4: Nomi", ostida fasl/qism/sifat;
  runner faqat tanani yuboradi: tugagan bosqichlar ("✅ Yuklab olindi: MB ·
  WxH · davomiylik"), joriy bosqich (foiz-chiziq, tezlik, bo'lak, hajm va
  taxminiy yakuniy hajm, o'tdi/qoldi), oxirgi 4 log qatori, "⏱ jami".
- OVOZ YO'LI (foydalanuvchi: "ovozi o'zbekcha emas, ruscha"): ba'zi qismlar
  (`/external-hls/...`, masalan Muzli devor 14, Moviy quticha 2) master
  playlistida o'zbekcha ovoz ALOHIDA: `#EXT-X-MEDIA:TYPE=AUDIO,LANGUAGE="uz"`,
  video bo'laklari ichida esa boshqa (ruscha) ovoz bor. Eski kod faqat video
  variantini olardi. Endi worker `variants` ovoz guruhidan `uz` (keyin
  DEFAULT=YES) ni tanlaydi, navbatga `video\naudio` yoziladi; runner ikkalasini
  parallel yuklab `-map 0:v:0 -map 1:a:0` bilan qo'shadi (ichki ovoz
  tashlanadi). Sinov: natijadagi ovoz o'zbekcha playlist bilan bayt-bayt bir
  xil (MD5). Sifat tanlashda "🔊 Ovoz: Ўзбек" ko'rinadi. Oddiy (`/content/...`)
  qismlarda bitta ovoz — o'zgarishsiz.
- O'ZBEKCHA OVOZLI QISMLAR TEZLIGI: `external-hls` bo'laklari har biri ~2.3 s
  kutadi (sayt ularni boshqa serverdan olib beradi), tezlik oqimlar soniga
  proporsional: 12 — 2.5 MB/s, 32 — 6.5, 48 — 8.1, 64 — 8.0 MB/s. Shu sabab
  bunday qismlar `PARALLEL_EXT=48` oqimda (oddiylari `PARALLEL=12`). Sinov
  (Muzli devor 14, 480p, video+ovoz 304 bo'lak): 66 s -> 23 s.
- SIFAT BOSILGANDA JAVOB (foydalanuvchi: 3 ta sifat bosildi, bot hech narsa
  demadi): `pick_quality` endi BIRINCHI ish sifatida oddiy matnli (HTML'siz)
  "⏳ 1080p qabul qilindi — navbatga qo'shilmoqda..." xabarini yuboradi;
  keyingi har qanday natija shu xabarni tahrirlaydi: "✅ Navbatga qo'shildi
  (#N, oldinda K ta)" yoki "❌ Navbatga qo'shilmadi: <sabab>". Avval bir necha
  joyda (anime/fasl/qism topilmasa) jim `return` bo'lardi.
- TELEGRAM'GA TEZ YUKLASH (foydalanuvchi talabi): Pyrogram 2.0.106 katta
  faylni BITTA media-ulanishda 4 so'rov bilan yuboradi va xato bo'lgan
  bo'lakni jimgina tashlab ketadi. `download.py` -> `fast_save_file`:
  `app.save_file` vaqtincha almashtiriladi, 512 KB bo'laklar `UP_CONN`=4
  ulanish x `UP_WORKERS`=4 so'rov bilan parallel; har bo'lak xato/FloodWait da
  qayta yuboriladi. Kichik fayllar (muqova) va `FilePartMissing` qayta
  yuborish — Pyrogram'ning o'z usuli. Tez usul xato bersa — oddiy usulda
  qayta. Soxta ulanish bilan sinaldi (bo'laklar to'g'ri, xatolarda qayta
  yuboradi); haqiqiy Telegram'da sinalmagan (sessiyani bu yerdan ishlatish
  AUTH_KEY_DUPLICATED xavfi). Sessiya kodlash bilan umumiy — Telegram
  cheklasa, `UP_CONN` ni kamaytirish kerak.
- PANEL TARTIBI (foydalanuvchi talabi): doimiy 6 tugma (Orqaga, Bosh menyu,
  Izlash, Yuklash navbati / Bo'limlar) va "◀️ Oldingi / Keyingi ▶️" panelning
  TEPASIDA. Qismlar bitta sahifada (`EP_PAGE`=300, faqat juda uzun seriallar
  bo'linadi) va TESKARI tartibda (eng yangisi birinchi).
