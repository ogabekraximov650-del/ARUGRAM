# ARUGRAM — Telegram mijozi (Kotlin + Rust, noldan)

DrKLO/Telegram (`../Telegram`) kodi faqat **o'rganish uchun** (joylashuv, o'lchamlar, animatsiyalar); bu yerda hech narsa undan ko'chirilmagan,
hammasi o'zimiz yozgan (GPL majburiyati yo'q). Telegram bilan ishlash — `rust/` (grammers, MTProto), ekranlar — `app/` (Kotlin + Compose).

## Hozirgi holat (1-bosqich)
- **Kirish** Telegram bilan bir xil: telefon (davlat + raqam shabloni) -> davlat tanlash -> kod (katakchalar, qayta yuborish) -> ikki bosqichli parol.
- Kirgach — "Chatlar tez orada qo'shiladi" bosh oynasi (keyingi bosqichlar ro'yxati) va "Hisobdan chiqish".

## Tuzilma
| Joy | Nima |
|---|---|
| `rust/` | Rust yadrosi (`librust_core.so`): Telegram kirishi, sessiya (shifrlangan), keyin chatlar |
| `app/.../core/` | JNA ko'prigi (`RustLib`, `RustCore`), `SecureStore` (Keystore kaliti) |
| `app/.../auth/` | `AuthModel` (kirish holati), `Countries`, `PhoneFormat` (sof funksiyalar, testlangan) |
| `app/.../ui/` | `theme` (Telegram ranglari), `components` (`TgOutlineBox`, `TgFab`, `TgCodeBoxes`...), `login` (sahifalar), `main` |
| `app/src/main/assets/countries.txt` | Davlatlar (libphonenumber ma'lumotlaridan yasalgan; nomlar o'zbekcha) |

## Yig'ish
FAQAT GitHub Actions: `.github/workflows/arugram-android.yml` (Rust -> testlar -> imzolangan APK -> Telegram "Saqlangan xabarlar").
API kalitlari (`TG_API_ID`, `TG_API_HASH`) GitHub Secrets'dan `BuildConfig` ga beriladi, kodga yozilmaydi.

## Telegram'dan olingan o'lchamlar (DrKLO)
Sahifa: chetlari 18 dp, tepa 30 dp. Sarlavha 18 sp (qalin), izoh 14 sp. Maydon: 58 dp, burchak 8 dp, ramka 0,5 -> 1,67 dp, sarlavha 16 sp -> ×0,75.
Tugma: 56 dp, o'ngdan 20, pastdan 14. Kod katagi: 34×42 dp, oraliq 7, ramka 1,5, burchak 4, raqam 20 sp. Sahifa almashuvi: 300 ms, tezlanib-sekinlanuvchi.

## Tanishtiruv oynasi va mavzu (2026-10)
`ui/intro/IntroScreen.kt`: 6 sahifali pager (har birida Canvas'da chizilgan harakatlanuvchi rasm), nuqtalar, yorug'lik o'tuvchi
"Suhbatni boshlash" tugmasi, "Continue in English" (matnlar `IntroTexts.kt`), tepa o'ngda quyosh/oy tugmasi. Mavzu tanlovi
`SharedPreferences("ui")` da saqlanadi, ranglar `lerpTg` bilan silliq almashadi (`TgTheme.kt`).
