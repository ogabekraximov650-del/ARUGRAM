//! UZ: # uzcoders-gpu
//! GPU bilan chizish: `DrawCmd` ro'yxati bitta uchlar oqimiga paketlanadi (`Batcher`), OpenGL ES 2.0 shu oqimni
//! bir necha chaqiruvda chizadi (`GlRenderer`, faqat Android). Harflar va ikonkalar atlasda saqlanadi.
//! EN: # uzcoders-gpu
//! GPU drawing: a `DrawCmd` list is packed into one vertex stream (`Batcher`); OpenGL ES 2.0 draws that stream
//! in a few calls (`GlRenderer`, Android only). Glyphs and icons are stored in an atlas.
//! RU: # uzcoders-gpu
//! Рисование на GPU: список `DrawCmd` упаковывается в один поток вершин (`Batcher`); OpenGL ES 2.0 рисует этот поток
//! за несколько вызовов (`GlRenderer`, только Android). Глифы и иконки хранятся в атласе.

pub mod atlas;
pub mod batch;
#[cfg(target_os = "android")]
pub mod gl;

pub use batch::{Batcher, Frame, Op, Vertex, ATLAS_SIZE};

#[cfg(target_os = "android")]
pub use gl::{set_video_hooks, GlRenderer, VideoCreate, VideoUpdate};
