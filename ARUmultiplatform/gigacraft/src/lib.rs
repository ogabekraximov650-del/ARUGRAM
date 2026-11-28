//! GigaCraft — noldan yozilgan voxel o'yin (Rust + OpenGL ES). Tashqi kutubxona yo'q.
//!
//! Tuzilish: `gen` (dunyo yaratish) -> `world` (bo'laklar va tahrirlar) -> `mesh`/`lod` (chizish uchun geometriya) ->
//! `stream` (fonda yuklash) -> `render` (OpenGL) ; `player`, `ray`, `game`, `ui` — o'yin mantig'i; `app` hammasini birlashtiradi;
//! `android` — NativeActivity qobig'i.
pub mod app;
pub mod blocks;
pub mod earth;
pub mod egl;
pub mod font;
pub mod game;
pub mod gen;
pub mod gl;
pub mod hud;
pub mod lod;
pub mod math;
pub mod mesh;
pub mod noise;
pub mod planet;
pub mod player;
pub mod png;
pub mod ray;
pub mod rivers;
pub mod render;
pub mod sky;
pub mod stream;
pub mod text;
pub mod tilecache;
pub mod ui;
pub mod vitals;
pub mod world;

#[cfg(target_os = "android")]
mod android;
