# uzcoders

**uzcoders** — Rust'da mobil ilovalar uchun UI va tizim kutubxonasi (O'zbekistonlik dasturchilardan).
*A from-scratch UI and system library in Rust, mobile-first.*

> Holat: **0.1 (ishlanmoqda)**. API barqaror emas.

## Paketlar

| Paket | Vazifasi |
|---|---|
| `uzcoders-core` | geometriya, rang, reaktiv holat, animatsiya, kiritish hodisalari |
| `uzcoders-layout` | flex joylashuv dvigateli |
| `uzcoders-text` | TrueType shrift, harf rasterizatsiyasi, matn joylashuvi *(yo'lda)* |
| `uzcoders-render` | chizish buyruqlari, dasturiy rasterizator, PNG *(yo'lda)* |
| `uzcoders-ui` | deklarativ UI daraxti va vidjetlar *(yo'lda)* |
| `uzcoders` | hammasini birlashtiruvchi asosiy paket |

## Maqsad
Rust'da, Kotlin Compose kabi qulay, mobil (Android, keyin iOS) uchun to'liq ilova yozish imkonini beradigan kutubxona.
Tashqi bog'liqliklarsiz, hamma narsa noldan yoziladi.

## Litsenziya
`MIT OR Apache-2.0` — ikkalasidan birini tanlang. Hissa qo'shganingizda `Signed-off-by` (DCO) qo'shing, qarang: [CONTRIBUTING.md](CONTRIBUTING.md).
`assets/fonts/DejaVuSans.ttf` — DejaVu/Bitstream Vera litsenziyasi ostida (erkin tarqatiladi).
