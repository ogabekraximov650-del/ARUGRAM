# Telegram (DrKLO/Telegram 13.2.2) -> ARUMEDIATV worker: qaysi funksiyalar ishlaydi, qaysilari yo'q

Sana: 2026-11-19. Manba: `DrKLO/Telegram` (rasmiy Android ilovasi, GPLv2, oxirgi yangilanish 2026-11-12, versiya 13.2.2, protokol qatlami 233).
Usul: kod statik tahlil qilindi (ilova ishga tushirilmadi, hech narsa yig'ilmadi). Hisob-kitob `deserializeResponse` bor TL klasslar (RPC so'rovlar) bo'yicha.

## 1. Umumiy raqamlar
- RPC so'rov turlari: **~914** (TLRPC.java: 553, tl/ papkasi: 361). Ilova kodida ulardan **~850 tasi** haqiqatan ishlatiladi.
- Ekran/faoliyat sinflari (`ui/*.java`): 216 ta; `MessagesController` 25 700 qator, `TLRPC.java` 72 300 qator.
- Barcha so'rovlar bitta eshikdan o'tadi: `ConnectionsManager.sendRequest(...)` -> C++ `tgnet` (MTProto, TCP). Shu eshikni HTTP "tarjimon" bilan almashtirish mumkin (asosiy g'oya).
- Bizning worker'dagi suhbat: faqat **foydalanuvchi <-> admin** (`chat_messages`: user_id, from_admin). Foydalanuvchilar o'rtasida yozishma, guruh, kanal, kontaktlar YO'Q.

## 2. Guruhlar bo'yicha (so'rovlar soni taxminiy)

### A. Tarjimon qatlamida oson/arzon (stub yoki oddiy javob) — ~230 ta
| Nomlar fazosi | Soni | Izoh |
|---|---|---|
| (root): ping, initConnection, invokeWithLayer, getFutureSalts, ... | ~50 | MTProto xizmat so'rovlari; tarjimon o'zi javob beradi |
| help.* (getConfig, getAppConfig, getNearestDc, getCountriesList, terms...) | 25 | statik/sozlama javoblar |
| langpack.* | 5 | til paketlarini statik xizmat qilish |
| account.* (sozlamalar: maxfiylik, bildirishnoma, mavzu, fon, sessiyalar, 2FA...) | ~126 | ko'pi saqlanmaydigan "bo'sh" javob; haqiqiysi: profil, username, sessiyalar (bizda `/api/session`, `/api/auth`) |
| photos.*, users.* | ~8 | profil rasmi/ma'lumot: bizda `users_db`, `/api/avatar` |

### B. Worker'ga YANGI endpoint va jadvallar yozilsa ishlaydi (o'rta qiyinlik) — ~150 ta
| Funksiya | Kerak bo'ladigan ish |
|---|---|
| Kirish (auth.*, 23) | Telefon raqam + SMS kodi **ishlamaydi** (Telegram/Firebase SMS). Bizning kirish (bot orqali / Google) bilan almashtiriladi |
| Suhbatlar ro'yxati, tarix, xabar yuborish/tahrirlash/o'chirish, o'qildi belgisi (messages.* ning ~60 tasi) | foydalanuvchilar orasidagi xabarlar uchun yangi jadval va API (hozir faqat admin chat) |
| Yangilanishlar (updates.getState/getDifference/getChannelDifference, 3) | har foydalanuvchi uchun `pts`/`seq` hisoblagichli yangilanishlar jurnali + jonli yetkazish (uzoq so'rov bor: `/api/live/wait`) |
| Fayl yuklash/olish (upload.*, 8) | bo'lak-bo'lak yuklash, kesh; bizning `vd` (R2 -> Telegram kanal) bilan ulash mumkin |
| Kontaktlar (contacts.*, 25) | username bo'yicha qidirish mumkin; telefon kitobi bilan **moslashtirish yo'q** |
| Stikerlar/emoji/GIF (stickers.*, 11 + messages.getStickers...) | bizning `/api/packs` bilan; Telegram'ning rasmiy to'plamlari **yo'q** |
| Qidiruv (messages.search*) | faqat bizning bazadagi matn qidiruvi; "global qidiruv" yo'q |
| Qoralama, pin, mute, arxiv, javob, forward, reaksiya, so'rovnoma (poll) | har biri ustun va endpoint |

### C. Katta yangi tizim kerak (qiyin, keyinga) — ~120 ta
| Funksiya | Soni | Sabab |
|---|---|---|
| Guruhlar/superguruhlar/kanallar (channels.*) | 58 | a'zolar, huquqlar, tarix, kanal yangilanishlari modeli |
| communities.*, forum.* (mavzular) | 16 | guruh modeli ustiga |
| chatlists.* (papka ulashish), folders.* | ~24 | papkalar modeli |
| Maxfiy suhbat (messages.requestEncryption va h.k.) | ~12 | kalit almashish va shifrlangan navbat (server faqat uzatadi) |
| Bot bilan ishlash (bots.*, inline, callback tugmalar) | ~30 | foydalanuvchi ilovasidagi bot mexanikasi; faqat o'zimizning botlar uchun alohida yozish mumkin |

### D. Ishlamaydi / umuman mumkin emas (tashqi Telegram infratuzilmasi) — ~210 ta
| Funksiya | Soni | Nega |
|---|---|---|
| Qo'ng'iroqlar (phone.*, `tgcalls` WebRTC) va guruh qo'ng'iroqlari | 44 | Telegram'ning media-serverlari/reley; protokol boshqa; alohida WebRTC SFU kerak |
| Storilar (stories.*) | 60 | Telegram tarmog'idagi storilar infratuzilmasi va qoidalari |
| Stars, sovg'alar, reklama daromadi (stars.*) | 34 | Telegram to'lov tizimi |
| To'lovlar (payments.*), Premium (premium.*) | ~39 | Google Play/Stripe + Telegram hisobi |
| TON hamyon (wallet.*, toncenter.*, `gram-wallet`) | 35 | Blokcheyn va Telegram integratsiyasi |
| Telegram Passport (SecureID) | — | KYC hujjat tizimi |
| AI yozuvchi (aicompose.*), tarjima, ovozdan matnga | ~10 | Telegram bulutli xizmatlari |
| Kanal statistikasi (stats.*), Fragment, o'zgarmas sahifalar (iv.*) | ~9 | Telegram xizmatlari |
| Mini ilovalar (WebApp) — Telegram'ning bot veb-ilovalari | — | haqiqiy Telegram bot serverlariga bog'liq |

## 3. RPC dan tashqari tizimlar
| Tizim | Holat |
|---|---|
| Push-bildirishnoma (`GcmPushListenerService`, Firebase) | O'z Firebase loyihasi + worker'dan FCM yuborish kerak; hozir bizda yo'q (bizda polling) |
| SMS kod (`SmsReceiver`, Firebase telefon tekshiruvi) | Ishlamaydi (bizda SMS yo'q) |
| Google Maps (`GoogleMapsProvider`) | O'z Maps API kaliti kerak; jonli joylashuv uchun server qismi yo'q |
| Mahalliy baza (`MessagesStorage`, SQLite) va fayl keshi | Ilova ichida qoladi, serverga bog'liq emas — saqlanadi |
| Parol/passcode, mavzular, animatsiyalar, ovozli xabar yozish | Mahalliy — o'zgarishsiz ishlaydi |
| Sertifikat/RSA kalitlar (`Handshake.cpp`) | MTProto'ni butunlay chetlab o'tsak kerak emas |

## 4. Xulosa
- **Birinchi ishlaydigan natija (2-3 bosqich):** kirish (bizning akkaunt) + suhbatlar ro'yxati + xabar yozish/olish (admin suhbati yoki yangi foydalanuvchilararo jadval) + oddiy rasm/video.
- **Ancha ish:** guruhlar/kanallar, jonli yangilanishlar to'liq, kontaktlar, push.
- **Tashlab ketiladi:** qo'ng'iroqlar, storilar, Stars, to'lovlar, Premium, hamyon, Passport, AI.
- **Qo'shimcha cheklovlar:** GPLv2 (kodni ochiq saqlash), o'z `api_id`, "Telegram" nomi va logotipi ishlatilmasin, yig'ish NDK 27 bilan, Actions'da ~30 daqiqa.
