# ARUGRAMRUST

UZ: ARUGRAM'ning Rust (uzcoders) versiyasi. Hozir faqat **kirish qismi namunasi** (animatsiya namunasi, `demo/`) bor; ilova kodi keyin.
EN: The Rust (uzcoders) version of ARUGRAM. For now only a **login animation sample** (`demo/`) exists; the app code comes later.
RU: Rust-версия (uzcoders) ARUGRAM. Пока есть только **образец анимаций входа** (`demo/`); код приложения — позже.

## demo/

- `login_demo.html` — UZ: kirish oqimi (telefon → kod → parol → "Chatlar tez orada qo'shiladi") animatsiyalari, o'z dizaynimiz. Barcha animatsiyalar bitta virtual soatda, shuning uchun video har kadrda aniq.
  EN: the login flow animations (phone → code → password → "Chats coming soon"), our own design. Everything runs on one virtual clock, so the video is frame-exact.
  RU: анимации потока входа (телефон → код → пароль → «Чаты скоро появятся»), наш собственный дизайн. Всё идёт по одним виртуальным часам, видео покадрово точно.
- `intro_demo.html` — UZ: tanishtiruv (intro) ekrani: 6 sahifa, har birida o'zimiz chizgan animatsiya, kun/tun tugmasi va "Suhbatni boshlash" tugmasi. EN: the intro screen: 6 pages, each with our own animation, the day/night button and the "Start" button. RU: экран знакомства: 6 страниц со своей анимацией, кнопка день/ночь и кнопка «Начать».
- `render.mjs` — UZ: `DEMO=intro_demo.html node render.mjs video out.mp4` (60 fps). Video faqat GitHub Actions'da (`.github/workflows/arugramrust-demo.yml`) yasaladi va Telegram'ga yuboriladi. EN/RU: same.

## Litsenziya / License / Лицензия

UZ: Animatsiyalar **toza usulda** (clean-room) o'zimiz yozilgan: GPL manba kodi (DrKLO Telegram), uning Lottie/rasm fayllari yoki kodi ko'chirilmagan. Faqat umumiy xatti-harakat g'oyalari (sahifa surilishi, prujinali silkinish va h.k.) ishlatilgan; barcha son qiymatlari bizniki.
EN: The animations were written **clean-room**: no GPL source (DrKLO Telegram), Lottie/image files or code was copied. Only general behaviour ideas (page slide, spring shake, etc.) are used; all numeric values are ours.
RU: Анимации написаны **по методу clean-room**: код GPL (DrKLO Telegram), его Lottie/изображения не копировались. Использованы лишь общие идеи поведения (сдвиг страницы, пружинное дрожание и т. п.); все числовые значения наши.

## Ilova / App / Приложение

UZ: `src/lib.rs` — 6 sahifali tanishtiruv (`src/art.rs` — o'zimiz chizgan vektor animatsiyalar, varaq sifatida oldindan chiziladi) va telefon raqami ekrani. APK: `.github/workflows/arugramrust.yml` (Telegram'ga yuboriladi). Bosh sahifa keyin.
EN: `src/lib.rs` — the 6-page intro (`src/art.rs` — our own vector animations, pre-rendered into sprite sheets) and the phone-number screen. APK: `.github/workflows/arugramrust.yml` (sent to Telegram). Home screen comes later.
RU: `src/lib.rs` — знакомство из 6 страниц (`src/art.rs` — собственные векторные анимации, заранее рисуются в листы) и экран номера телефона. APK: `.github/workflows/arugramrust.yml` (отправляется в Telegram). Главный экран — позже.

## Litsenziya (yangilandi) / License (updated) / Лицензия (обновлено)

UZ: Loyiha ochiq bo'lishi rejalashtirilgani uchun Telegram (GPL) kodi va resurslaridan to'g'ridan-to'g'ri foydalanilgan — **ARUGRAMRUST GPL-2.0-or-later**. Tafsilot: `NOTICE.md`, `LICENSE`. `demo/tgintro/` — Telegram'ning `IntroRenderer.c` ini kompyuterda (Mesa EGL) ishga tushirib kadr oladi; `demo/tg_video.html` — Telegram uslubidagi tanishtiruv + kirish videosi.
EN: As the project is planned to be open source, Telegram (GPL) code and resources are used directly — **ARUGRAMRUST is GPL-2.0-or-later**. See `NOTICE.md`, `LICENSE`. `demo/tgintro/` runs Telegram's `IntroRenderer.c` on a PC (Mesa EGL) to capture frames; `demo/tg_video.html` is the Telegram-style intro + sign-in video.
RU: Так как проект планируется открыть, код и ресурсы Telegram (GPL) используются напрямую — **ARUGRAMRUST под GPL-2.0-or-later**. См. `NOTICE.md`, `LICENSE`. `demo/tgintro/` запускает `IntroRenderer.c` Telegram на ПК (Mesa EGL) и снимает кадры; `demo/tg_video.html` — видео интро + входа в стиле Telegram.
