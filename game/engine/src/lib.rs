//! ARU o'yin dvigateli (tajriba bosqichi): 3D dunyo, kamera, sensorli boshqaruv.

pub mod audio;
pub mod camera;
pub mod game;
pub mod hud;
pub mod mesh;
pub mod models;
pub mod input;
pub mod renderer;
pub mod scene;
pub mod terrain;
pub mod weapons;
pub mod world;

#[cfg(target_os = "android")]
mod android;

pub use game::Game;
