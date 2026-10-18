//! ARU o'yin dvigateli (tajriba bosqichi): 3D dunyo, kamera, sensorli boshqaruv.

pub mod camera;
pub mod game;
pub mod hud;
pub mod input;
pub mod renderer;
pub mod world;

#[cfg(target_os = "android")]
mod android;

pub use game::Game;
