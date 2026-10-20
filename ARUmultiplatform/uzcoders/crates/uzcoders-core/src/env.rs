//! Oyna muhiti: tizim panellari (status bar, navigatsiya paneli) egallagan xavfsiz chekinishlar.

use crate::geometry::EdgeInsets;
use std::cell::Cell;

thread_local! {
    static SAFE: Cell<EdgeInsets> = const { Cell::new(EdgeInsets::ZERO) };
}

/// Tizim panellari ostida qolmaslik uchun kerakli chekinishlar (dp). Qobiq o'rnatadi, UI qurilganda o'qiladi.
pub fn safe_insets() -> EdgeInsets {
    SAFE.with(|s| s.get())
}

pub fn set_safe_insets(e: EdgeInsets) {
    SAFE.with(|s| s.set(e));
}
