//! Video pleyer: Android'da NDK (`imp`), boshqa muhitda (testlar) — bo'sh o'rnak.

#[cfg(target_os = "android")]
mod imp;
#[cfg(target_os = "android")]
pub use imp::*;

#[cfg(not(target_os = "android"))]
mod stub;
#[cfg(not(target_os = "android"))]
pub use stub::*;
