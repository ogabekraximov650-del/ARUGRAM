//! # uzcoders-gpu
//! GPU bilan chizish: `DrawCmd` ro'yxati bitta uchlar oqimiga paketlanadi (`Batcher`), OpenGL ES 2.0 shu oqimni
//! bir necha chaqiruvda chizadi (`GlRenderer`, faqat Android). Harflar va ikonkalar atlasda saqlanadi.

pub mod atlas;
pub mod batch;
#[cfg(target_os = "android")]
pub mod gl;

pub use batch::{Batcher, Frame, Op, Vertex, ATLAS_SIZE};
