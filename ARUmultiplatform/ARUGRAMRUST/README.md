# ARUGRAMRUST

UZ: ARUGRAM'ning Rust (uzcoders) versiyasi. Hozir faqat **kirish qismi namunasi** (animatsiya namunasi, `demo/`) bor; ilova kodi keyin.
EN: The Rust (uzcoders) version of ARUGRAM. For now only a **login animation sample** (`demo/`) exists; the app code comes later.
RU: Rust-версия (uzcoders) ARUGRAM. Пока есть только **образец анимаций входа** (`demo/`); код приложения — позже.

## demo/

- `login_demo.html` — UZ: kirish oqimi (telefon → kod → parol → "Chatlar tez orada qo'shiladi") animatsiyalari, o'z dizaynimiz. Barcha animatsiyalar bitta virtual soatda, shuning uchun video har kadrda aniq.
  EN: the login flow animations (phone → code → password → "Chats coming soon"), our own design. Everything runs on one virtual clock, so the video is frame-exact.
  RU: анимации потока входа (телефон → код → пароль → «Чаты скоро появятся»), наш собственный дизайн. Всё идёт по одним виртуальным часам, видео покадрово точно.
- `render.mjs` — UZ: `node render.mjs video out.mp4` (60 fps). EN/RU: same.

## Litsenziya / License / Лицензия

UZ: Animatsiyalar **toza usulda** (clean-room) o'zimiz yozilgan: GPL manba kodi (DrKLO Telegram), uning Lottie/rasm fayllari yoki kodi ko'chirilmagan. Faqat umumiy xatti-harakat g'oyalari (sahifa surilishi, prujinali silkinish va h.k.) ishlatilgan; barcha son qiymatlari bizniki.
EN: The animations were written **clean-room**: no GPL source (DrKLO Telegram), Lottie/image files or code was copied. Only general behaviour ideas (page slide, spring shake, etc.) are used; all numeric values are ours.
RU: Анимации написаны **по методу clean-room**: код GPL (DrKLO Telegram), его Lottie/изображения не копировались. Использованы лишь общие идеи поведения (сдвиг страницы, пружинное дрожание и т. п.); все числовые значения наши.
