//! Oyna muhiti: tizim panellari (status bar, navigatsiya paneli) egallagan xavfsiz chekinishlar.

use crate::geometry::EdgeInsets;
use std::cell::Cell;

thread_local! {
    static SAFE: Cell<EdgeInsets> = const { Cell::new(EdgeInsets::ZERO) };
    static DENSITY: Cell<f32> = const { Cell::new(1.0) };
}

/// Tizim panellari ostida qolmaslik uchun kerakli chekinishlar (dp). Qobiq o'rnatadi, UI qurilganda o'qiladi.
pub fn safe_insets() -> EdgeInsets {
    SAFE.with(|s| s.get())
}

pub fn set_safe_insets(e: EdgeInsets) {
    SAFE.with(|s| s.set(e));
}

/// Ekran zichligi (dp -> piksel). UI qurilayotganda o'qiladi (masalan virtual ro'yxat uchun).
pub fn density() -> f32 {
    DENSITY.with(|d| d.get())
}

pub fn set_density(d: f32) {
    DENSITY.with(|c| c.set(d));
}
