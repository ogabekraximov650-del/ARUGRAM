//! UZ: Chizish moduli: qatlamlar, shakllar, maskalar, matte va natija kadrlari.
//! EN: The rendering module: layers, shapes, masks, mattes and output frames.
//! RU: Модуль отрисовки: слои, фигуры, маски, маты и выходные кадры.

mod assets;
mod composition;
mod drawing;
mod drawing_paths;
mod layer_effects;
mod renderer;
mod renderer_cache;
mod renderer_types;
mod sample_cache;
mod scene;
mod text;
mod values;

pub use renderer::{ImageAssetResolver, PreparedAnimation, RasterFrame, RenderConfig, Renderer, Rgba8};
