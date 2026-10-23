# uzcoders-lottie

**UZ:** Lottie va Telegram `.tgs` animatsiyalarini RGBA kadrlarga chizuvchi sof Rust kutubxona. Chizish o'zimizning dasturiy rasterizatorimizda (`raster`): qirralari yumshoq yo'l to'ldirish va chiziq, punktir, gradient, maska, matte. Tashqi grafik kutubxona yo'q (faqat `serde`/`serde_json`, `thiserror`, `rustc-hash`).
**EN:** A pure Rust renderer from Lottie and Telegram `.tgs` animations to RGBA frames. Drawing uses our own software rasterizer (`raster`): anti-aliased fill and stroke, dashes, gradients, masks, mattes. No external graphics crate (only `serde`/`serde_json`, `thiserror`, `rustc-hash`).
**RU:** Библиотека на чистом Rust, рисующая Lottie и Telegram `.tgs` в RGBA-кадры. Рисование — собственный программный растеризатор (`raster`): заливка и обводка со сглаживанием, штрихи, градиенты, маски, маты. Внешней графической библиотеки нет (только `serde`/`serde_json`, `thiserror`, `rustc-hash`).

```rust
use uzcoders_lottie::{Animation, RenderConfig, Renderer, Rgba8, tgs};

let json = tgs::to_json(&std::fs::read("sticker.tgs")?).unwrap();   // .tgs (gzip) yoki oddiy JSON / or plain JSON
let anim = Animation::from_json_str(&json)?;
let prepared = Renderer::default().prepare(&anim)?;                 // bir marta / once
let cfg = RenderConfig::new(Rgba8::TRANSPARENT, 160.0 / anim.width as f32);
let frame = prepared.render_frame(anim.in_point + 10.0, cfg)?;      // premultiplied RGBA8
```

**UZ:** Mantiq [`rasterlottie`](https://github.com/neodyland/rasterlottie) (MIT OR Apache-2.0, neodyland) dan olingan va moslashtirilgan; uning chizish qismi (`tiny-skia`) o'rniga o'zimizning `raster` modulimiz qo'yilgan. Litsenziyalar: `LICENSE-MIT`, `LICENSE-APACHE`.
**EN:** The logic is derived from and adapted from [`rasterlottie`](https://github.com/neodyland/rasterlottie) (MIT OR Apache-2.0, neodyland); its drawing layer (`tiny-skia`) is replaced by our own `raster` module. Licenses: `LICENSE-MIT`, `LICENSE-APACHE`.
**RU:** Логика взята и адаптирована из [`rasterlottie`](https://github.com/neodyland/rasterlottie) (MIT OR Apache-2.0, neodyland); слой рисования (`tiny-skia`) заменён собственным модулем `raster`. Лицензии: `LICENSE-MIT`, `LICENSE-APACHE`.

**UZ:** Cheklovlar: matn qatlamlari, rasm assetlari, ifodalar (expressions, kichik qismidan tashqari), qatlam effektlari (kichik qismidan tashqari), `mm`/`rd` kabi ba'zi shakl turlari qo'llab-quvvatlanmaydi — `Renderer::prepare` aniq sabab bilan xato beradi.
**EN:** Limits: text layers, image assets, expressions (except a small subset), layer effects (except a small subset) and some shape items such as `mm`/`rd` are unsupported; `Renderer::prepare` returns an error with the exact reason.
**RU:** Ограничения: текстовые слои, растровые ассеты, выражения (кроме малой части), эффекты слоёв (кроме малой части) и некоторые фигуры вроде `mm`/`rd` не поддерживаются; `Renderer::prepare` возвращает ошибку с точной причиной.
