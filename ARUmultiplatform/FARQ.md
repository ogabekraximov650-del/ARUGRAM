# Flutter ↔ Kotlin: nima bor, nima yo'q (yangilangan: 2026-10-16)

Kotlin ilovasi (`ARUmultiplatform/android`) Flutter (`lib/`) bilan ekranma-ekran solishtirilib, Kotlin uslubida qayta yozildi.

## Kotlin'da BOR (Flutter'dagidek)
- Kirish darvozasi (`AuthGate`): ilova hisobi + Telegram hisobi bo'lmaguncha butun ekran kirish oynasi; davlat tanlash, raqam shabloni, QR, bot orqali kirish.
- 5 ta sahifa (Bosh sahifa, Qidiruv, Katalog, Kutubxona, Profil) — hammasida ARUMEDIATV sarlavhasi; suzuvchi menyu.
- Kartalar: reyting, ko'rishlar, yosh (rangli), "Pullik" belgisi; qidiruv kartasi (janr bilan).
- Bo'lim sahifasi va pleyer (Media3): yumaloq pleyer, ⋮ menyu, HQ, to'liq ekran boshqaruvi, intro o'tkazish, sifat/tezlik, uyqu taymeri, avto-keyingi; obuna/kanal ruxsati to'liq ekranlari.
- Izohlar (javob, layk, shikoyat, o'chirish), Telegram uslubidagi emoji/GIF/stiker paneli (qidiruv, saralangan, teri rangi), yoyish tugmasi.
- Kutubxona: Tarix (anime kartalari, qismlar, to'xtagan joydagi kadr), Yuklanmalar (bo'lim kartalari, sifat kataklari), Sevimlilar.
- Profil (Flutter tartibi), Sozlamalar, Qurilmalar (versiya, kirgan/oxirgi faollik), Profilni tahrirlash, Xotira (halqa diagramma, toifalar, tozalash).
- Obuna va balans (3 sahifa: To'ldirish, Obuna, Tarix; chegirma; avto tekshirish).
- Yordam chati (Telegram uslubi): javob/iqtibos, nusxa, qidiruv, tanlab o'chirish (admin), rasm/video/fayl, ovoz (bosib turib, surib bekor/qulf), kun ajratgichlari.
- Emoji/GIF/stiker to'plamlari: yaratish, qo'shish (video kesish bilan), obuna.
- Admin paneli (9 bo'lim).
- Oxirgi halokat oynasi, vaqtinchalik fayllarni tozalash, hisobni 5 daqiqada yangilash.

## Kotlin'da YO'Q (ataylab yoki hali)
- Bildirishnomalar (Flutter'da ham "tez orada").
- Dumaloq video xabar yozish (kamera kerak) va chatdagi galereya/fayl brauzeri ("tg_attach_sheet" o'rniga tizim tanlagichi).
- Tasvirni yaxshilash (GPU filtr) pleyerda.
- Hisoblar orasida almashish, `offline_library` (oflaynda faqat yuklanganlarni ko'rsatish).
- Kodlash ekranida Telegram qadalgan holat va GitHub bilan to'g'ridan-to'g'ri so'rov.
- Bosh sahifadagi umumiy statistika banneri (Flutter'da ham kompilyatsiya bayrog'i bilan o'chiq).
