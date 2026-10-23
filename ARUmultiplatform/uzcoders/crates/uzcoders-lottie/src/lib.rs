//! UZ: uzcoders-lottie — Lottie va Telegram `.tgs` animatsiyalarini RGBA kadrlarga chizuvchi sof Rust kutubxona.
//! Chizish o'zimizning dasturiy rasterizatorimizda (`raster`), tashqi grafik kutubxonasiz. Mantiq `rasterlottie`
//! (MIT OR Apache-2.0, https://github.com/neodyland/rasterlottie) loyihasidan olingan va moslashtirilgan; litsenziyalar `LICENSE-*`.
//! EN: uzcoders-lottie — a pure Rust renderer from Lottie and Telegram `.tgs` animations to RGBA frames.
//! Drawing uses our own software rasterizer (`raster`), with no external graphics crate. The logic is derived from
//! `rasterlottie` (MIT OR Apache-2.0, https://github.com/neodyland/rasterlottie) and adapted; licenses are in `LICENSE-*`.
//! RU: uzcoders-lottie — библиотека на чистом Rust, рисующая Lottie и Telegram `.tgs` в RGBA-кадры.
//! Рисование — наш программный растеризатор (`raster`), без внешней графической библиотеки. Логика взята из
//! `rasterlottie` (MIT OR Apache-2.0, https://github.com/neodyland/rasterlottie) и адаптирована; лицензии — `LICENSE-*`.

#[macro_use]
mod trace_macros;

mod effects;
mod error;
mod expression;
mod model;
mod model_parse;
mod model_types;
pub mod raster;
mod render;
mod support;
pub mod tgs;
#[cfg(test)]
mod tests;
mod timeline;

pub use error::LottieError;
pub use model::{
    AnimatedValue, Animation, Asset, BezierPath, BezierVertex, Font, FontCharacter, Layer,
    LayerType, MaskMode, PositionValue, ShapeItem, ShapePathValue, SplitPosition, TextData,
    TextDocument, TrackMatteMode, Transform,
};
pub use raster::Pixmap;
pub use render::{ImageAssetResolver, PreparedAnimation, RasterFrame, RenderConfig, Renderer, Rgba8};
pub use support::{SupportProfile, SupportReport, UnsupportedFeature, UnsupportedKind, analyze_animation, analyze_animation_with_profile};
