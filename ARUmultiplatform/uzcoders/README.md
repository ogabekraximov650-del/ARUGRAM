# uzcoders

**O'zbekcha (UZ)** | **English (EN)** | **Русский (RU)**

---

## UZ — O'zbekcha

**uzcoders** — Rust tilida mobil ilovalar uchun UI va tizim kutubxonasi (O'zbekistonlik dasturchilardan). Hammasi noldan yozilgan, tashqi bog'liqliklarsiz.

> Holat: **0.1 (ishlanmoqda)**. API barqaror emas.

### Paketlar

| Paket | Vazifasi |
|---|---|
| `uzcoders-core` | geometriya, rang, reaktiv holat, animatsiya, kiritish hodisalari, fon vazifalari |
| `uzcoders-layout` | flex joylashuv dvigateli |
| `uzcoders-text` | TrueType shrift o'qish, harf rasterizatsiyasi, matn qatorlarga bo'linishi |
| `uzcoders-render` | chizish buyruqlari, dasturiy (CPU) rasterizator, PNG / JPEG dekoderlari |
| `uzcoders-lottie` | Lottie / Telegram `.tgs` animatsiyalarini RGBA kadrlarga chizuvchi sof Rust kutubxona |
| `uzcoders-gpu` | GPU bilan chizish: paketlash (batcher) va OpenGL ES 2.0 bajaruvchisi |
| `uzcoders-ui` | deklarativ UI daraxti, vidjetlar, scroll, matn kiritish, navigatsiya |
| `uzcoders-android` | Android `NativeActivity` qobig'i va Java yelimi (`UzActivity`) |
| `uzcoders` | hammasini birlashtiruvchi asosiy paket |

### Maqsad
Rust'da, Kotlin Compose kabi qulay, mobil (Android, keyin iOS) uchun to'liq ilova yozish imkonini beradigan kutubxona.

### Litsenziya
`MIT OR Apache-2.0` — ikkalasidan birini tanlang. Hissa qo'shganingizda `Signed-off-by` (DCO) qo'shing, qarang: [CONTRIBUTING.md](CONTRIBUTING.md).
`assets/fonts/DejaVuSans.ttf` — DejaVu/Bitstream Vera litsenziyasi ostida (erkin tarqatiladi).

---

## EN — English

**uzcoders** is a UI and system library for mobile apps in Rust (made by developers from Uzbekistan). Everything is written from scratch, with no external dependencies.

> Status: **0.1 (work in progress)**. The API is not stable.

### Crates

| Crate | Purpose |
|---|---|
| `uzcoders-core` | geometry, color, reactive state, animation, input events, background tasks |
| `uzcoders-layout` | flex layout engine |
| `uzcoders-text` | TrueType font parsing, glyph rasterization, text wrapping |
| `uzcoders-render` | draw commands, software (CPU) rasterizer, PNG / JPEG decoders |
| `uzcoders-lottie` | a pure Rust renderer from Lottie / Telegram `.tgs` animations to RGBA frames |
| `uzcoders-gpu` | GPU drawing: batching and an OpenGL ES 2.0 executor |
| `uzcoders-ui` | declarative UI tree, widgets, scrolling, text input, navigation |
| `uzcoders-android` | the Android `NativeActivity` shell and the Java glue (`UzActivity`) |
| `uzcoders` | the umbrella crate that re-exports everything |

### Goal
A library that lets you write complete mobile apps (Android first, iOS later) in Rust, as comfortably as with Kotlin Compose.

### License
`MIT OR Apache-2.0` — choose either one. When contributing, add a `Signed-off-by` line (DCO); see [CONTRIBUTING.md](CONTRIBUTING.md).
`assets/fonts/DejaVuSans.ttf` is under the DejaVu/Bitstream Vera license (freely redistributable).

---

## RU — Русский

**uzcoders** — UI- и системная библиотека для мобильных приложений на Rust (от разработчиков из Узбекистана). Всё написано с нуля, без внешних зависимостей.

> Статус: **0.1 (в разработке)**. API нестабилен.

### Пакеты

| Пакет | Назначение |
|---|---|
| `uzcoders-core` | геометрия, цвет, реактивное состояние, анимация, события ввода, фоновые задачи |
| `uzcoders-layout` | flex-движок раскладки |
| `uzcoders-text` | чтение шрифтов TrueType, растеризация глифов, перенос текста по строкам |
| `uzcoders-render` | команды рисования, программный (CPU) растеризатор, декодеры PNG / JPEG |
| `uzcoders-lottie` | библиотека на чистом Rust, рисующая Lottie / Telegram `.tgs` в RGBA-кадры |
| `uzcoders-gpu` | рисование на GPU: пакетирование (batcher) и исполнитель OpenGL ES 2.0 |
| `uzcoders-ui` | декларативное дерево UI, виджеты, прокрутка, ввод текста, навигация |
| `uzcoders-android` | оболочка Android `NativeActivity` и Java-прослойка (`UzActivity`) |
| `uzcoders` | общий пакет, объединяющий всё |

### Цель
Библиотека, позволяющая писать полноценные мобильные приложения (сначала Android, затем iOS) на Rust так же удобно, как на Kotlin Compose.

### Лицензия
`MIT OR Apache-2.0` — выберите любую. При вкладе добавляйте строку `Signed-off-by` (DCO), см. [CONTRIBUTING.md](CONTRIBUTING.md).
`assets/fonts/DejaVuSans.ttf` — под лицензией DejaVu/Bitstream Vera (свободное распространение).
