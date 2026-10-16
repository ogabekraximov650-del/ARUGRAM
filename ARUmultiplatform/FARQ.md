# Flutter ↔ Kotlin: nima bor, nima yo'q (2026-10)

Flutter (`lib/`, ~68 ming qator) bilan Kotlin ilovasi (`ARUmultiplatform/android`) solishtirildi. Tartib — muhimlik bo'yicha.

## Kotlin'da BOR
| Bo'lim | Izoh |
|---|---|
| Bosh sahifa, Qidiruv, Katalog | 2 ustunli kartalar, 7 bo'lim, janr/yil filtri, oflayn kesh |
| Kutubxona | Tarix, Sevimlilar (faqat o'qish), Yuklanmalar (bo'sh) |
| Telegram orqali kirish | telefon → kod → parol (MTProto, Rust) |
| Rasmlar | Telegram orqali, shifrlangan disk keshi |
| Profil | hisob kartasi, ID nusxalash, shaxsiy statistika (`/api/me/stats`), Telegram holati, qurilmalar, kesh, ilova haqida |
| Profilni tahrirlash, Qurilmalar | ism/username, boshqa qurilmani o'chirish |
| Sozlamalar | statistikani yashirish, kanallarga obuna ruxsati (`SyncQueue` orqali) |
| Accountni o'chirish / chiqish | ikki bosqichli tasdiq, chiqishda navbat avval serverga saqlanadi |
| `SyncQueue` | yagona yozuv yo'li (`POST /api/sync`): sozlamalar, sevimli, baho, tarix (pleyer uchun tayyor) |

## Kotlin'da YO'Q (Flutter'da bor)
1. **Anime sahifasi** (`anime_detail_screen`): bo'limlar, qismlar ro'yxati, baho berish, sevimliga qo'shish, tavsif, janrlar.
2. **Video pleyer** (`video_player_screen`, 8.7 ming qator): ExoPlayer + Rust keshidan o'qish (`player_source.rs` tayyor), sifat tanlash, davom ettirish, intro o'tkazish, tomosha tarixini yozish.
3. **Izohlar** (`comments_tab`): yozish, javob, layk.
4. **Yuklab olish va oflayn kutubxona** (`download_manager`, `downloads_index`, `offline_library`, `storage_screen`): Yuklanmalar bo'limi, xotira boshqaruvi.
5. **Admin bilan yozishma** (`support_chat_screen`, `support_service`): chat, o'qilmagan nuqtasi (profil va menyuda).
6. **Obuna va balans** (`billing_screen`, `billing_service`): tarif, balans to'ldirish.
7. **Emoji, GIF va stikerlar** (`my_packs`, `pack_*`, `pack_service`).
8. **Boshqa foydalanuvchi profili** (`public_profile_screen`, `stat_detail_screen`): statistika ro'yxatlari.
9. **Profil rasmini yuklash** (`auth/avatar`): Telegram'ga yuklash (`rust_tg` upload) kerak.
10. **Bot orqali kirish** (`telegram_login_screen`) va kanallarga obuna (`channel_gate`).
11. **Trafik hisobi** (`traffic_service`, `net_meter`) — profildagi "Trafik" hozir serverdagi qiymatni ko'rsatadi, yangisini yubormaydi.
12. **Bildirishnomalar** (Flutter'da ham "tez orada").
13. **Admin paneli** (faqat admin build): foydalanuvchilar, kodlash, hisobotlar, kanallar, ilova sozlamalari, anime/bo'lim/qism qo'shish va boshqarish (`admin_*`, `add_*`, `*_management`).

## Tavsiya etilgan tartib
Anime sahifasi → Pleyer (+ tarix yozish) → Sevimli/Baho/Izoh → Yuklab olish → Support chat → Billing → Paketlar → Admin.
Har bosqich alohida push va alohida sinov bilan.
