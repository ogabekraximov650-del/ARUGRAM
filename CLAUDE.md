# CLAUDE.md — ARUGRAM

Bu fayl har sessiya boshida o'qiladi. Batafsil tarix va har bir
qarorning sababi: `KEYINGI_VAZIFA.md` (katta fayl, kerakli bo'limni
`grep -n '^## '` bilan toping va faqat o'shani o'qing).

## Ishlash tartibi (foydalanuvchi talabi, QAT'IY)

- **Ish davomida HECH QANDAY xabar yozma.** "Endi buni qilyapman",
  "Rust tayyor, endi Dart tomoni", "Now build.rs and Cargo.toml" kabi
  oraliq gaplar — YO'Q. Tool chaqiruvlari orasida matn yozilmaydi.
  Sabab: har bir xabar obuna limitini kamaytiradi.
- Vazifa to'liq bajarilib, tekshirilib, push qilingandan keyin
  **BITTA** yakuniy xabar: nima qilindi, qaysi fayllar o'zgardi,
  nimani qo'lda tekshirish kerak, nima qilinmay qoldi (bo'lsa).
- Faqat o'rtada to'xtash mumkin bo'lgan holatlar: qaytarib bo'lmaydigan
  amal (o'chirish, baza tozalash) yoki vazifani ikki xil tushunish
  mumkin bo'lsa — shunda bitta qisqa savol.
- Foydalanuvchi bilan **faqat o'zbek tilida**. Texnik narsalarni oddiy
  tilda tushuntiring. Taxmin qilmang — kodni o'qib tasdiqlang.
- Commit xabarlari ham o'zbek tilida (mavjud tarixdagidek).

## Git

- **Faqat `main` branchga push qiling** (foydalanuvchi talabi).
  Boshqa branch yaratib push qilmang, PR ochmang.
- Push'dan oldin `git fetch origin main` va ustiga qo'ying — lokal
  `main` eskirgan bo'lishi mumkin.
- Commit'dan oldin: `rm -rf rust/target rust/Cargo.lock worker/target worker/Cargo.lock`.
- `main`ga push avtomatik ishga tushiradi:
  - `lib/**`, `rust/**`, `assets/**`, `branding/**`, `android-template/**`,
    `pubspec.yaml` o'zgarsa — APK build (`build-flutter-apk.yml`);
  - `worker/**` o'zgarsa — worker deploy (`deploy-worker.yml`).
  Workflow'larni qo'lda `workflow_dispatch` qilmang.
  `encode.yml` — faqat foydalanuvchi o'zi qo'lda ishga tushiradi.

## Loyiha tuzilishi

ARUGRAM — anime ko'rish ilovasi. Videolar va fayllar yopiq Telegram
kanali orqali uzatiladi (xarajatni kamaytirish uchun).

| Joy | Nima |
|---|---|
| `lib/` | Flutter ilova (Dart). `screens/` — ekranlar, `services/` — mantiq, `widgets/` — Telegram uslubidagi chat komponentlari (`tg_*`) |
| `lib/services/rust_bridge.dart` | Dart ↔ Rust (`librust_core.so`) FFI ko'prigi |
| `lib/services/api_base.dart` | Server manzili (CI `--dart-define=API_BASE` bilan beradi) |
| `lib/services/sync_queue.dart` | **Yagona yozuv yo'li** → `POST /api/sync` |
| `lib/services/telegram_service.dart` | Telegram hisobi, yuklash, bot chatini tozalash |
| `rust/src/` | Rust yadrosi: `telegram.rs` (grammers/MTProto), `video_cache.rs` (shifrlangan bo'lak-kesh), `player_source.rs` (pleyer uchun JNI manba), `crypto.rs` |
| `packages/video_player_android` | `video_player_android` nusxasi: `AruDataSource` pleyerni mahalliy serversiz, diskdagi shifrlangan keshdan o'qitadi |
| `worker/src/lib.rs` | Cloudflare Worker (Rust → WASM), worker nomi `arugram`. Baza — Turso |
| `tool/encode/` | H.265 avto-kodlash (Python, `encode.yml`) |
| `ci/` | Imzo kaliti (`release.keystore`), baza tozalash skripti |
| `android-template/` | CI `flutter create` dan keyin qo'yadigan `MainActivity.kt` (`/android/` repoda yo'q) |

Paket: `uz.arugram.soft`. Eski `arumediatv` worker'iga **tegilmaydi**.

## Buzilmasligi kerak bo'lgan qoidalar

- **Turso yozuvlari pul turadi.** Hamma yozuv faqat `SyncQueue` +
  `POST /api/sync` orqali. Yangi to'g'ridan-to'g'ri yozuv yo'li qo'shmang;
  o'qishlarni ham kamaytirish (keshlash) ustun.
- **Worker orqali fayl baytlari o'tmaydi.** Yuklash va ko'rsatish
  ilovaning o'zi orqali (MTProto, bot chati).
- **Hamma fayl shifrlanadi** (Telegram'ga AES-128-CTR, diskda
  AES-128-GCM bo'laklar). Kalitlar ro'yxatlarda berilmaydi (`hide_keys`).
- `rust/src/video_cache.rs` bo'lak-kesh/shifrlash qismi — sinalgan,
  tegmang. `rust/Cargo.toml` dagi `panic = "abort"` — Rust'da panic
  butun ilovani o'ldiradi, `unwrap()` dan saqlaning.
- `glass_pumpkin = "=2.0.0-rc0"` qotirilgan — o'zgartirmang.
- Kanalda "Restrict saving content" yoqilmasin (bot `copyMessage` qila olmaydi).
- Vaqt mintaqasi — UTC+5 (Toshkent).
- Maxfiy kalitlar (`TG_API_ID`, `TG_API_HASH`, `TG_CHANNEL_ID`,
  `ENCODE_TOKEN` va h.k.) GitHub Secrets'da — kodga yozmang.

## Tekshiruv (har bir o'zgarishdan keyin)

```
flutter analyze                                   # 0 muammo
flutter test                                      # test/
cd rust && cargo test --lib
cd worker && cargo check --target wasm32-unknown-unknown
```

Muhitda Flutter bo'lmasa — faqat o'zgargan qismning Rust tekshiruvlarini
ishlating va yakuniy xabarda Dart tekshirilmaganini ayting.

## Hujjatlash

Katta o'zgarish yoki yangi arxitektura qarori bo'lsa — `KEYINGI_VAZIFA.md`
ga yangi `## ` bo'lim qo'shing (nima, nega, qaysi fayllar).
