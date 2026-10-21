//! Video pleyer: Android'da NDK (Media3 ExoPlayer, `exo`), boshqa muhitda (testlar) — bo'sh o'rnak.

#[cfg(target_os = "android")]
mod exo;
#[cfg(target_os = "android")]
pub use exo::*;

#[cfg(not(target_os = "android"))]
mod stub;
#[cfg(not(target_os = "android"))]
pub use stub::*;
