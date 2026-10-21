//! UZ: Oyna muhiti: tizim panellari (status bar, navigatsiya paneli) egallagan xavfsiz chekinishlar, zichlik va oyna o'lchami.
//! EN: Window environment: safe-area insets taken by system bars (status bar, navigation bar), screen density and window size.
//! RU: Окружение окна: безопасные отступы под системные панели (статус-бар, навигационная панель), плотность экрана и размер окна.

use crate::geometry::EdgeInsets;
use std::cell::Cell;

thread_local! {
    static SAFE: Cell<EdgeInsets> = const { Cell::new(EdgeInsets::ZERO) };
    static DENSITY: Cell<f32> = const { Cell::new(1.0) };
    static WINDOW_DP: Cell<(f32, f32)> = const { Cell::new((0.0, 0.0)) };
}

/// UZ: Tizim panellari ostida qolib ketmaslik uchun kerakli chekinishlar (dp). Qobiq (platforma qatlami) o'rnatadi, UI qurilganda o'qiladi.
/// EN: Insets (dp) needed to avoid drawing under system bars. Set by the shell (platform layer) and read while the UI is built.
/// RU: Отступы (dp), чтобы содержимое не оказывалось под системными панелями. Задаются оболочкой (платформенным слоем) и читаются при построении UI.
pub fn safe_insets() -> EdgeInsets {
    SAFE.with(|s| s.get())
}

pub fn set_safe_insets(e: EdgeInsets) {
    SAFE.with(|s| s.set(e));
}

/// UZ: Ekran zichligi (dp -> piksel koeffitsiyenti). UI qurilayotganda o'qiladi (masalan, virtual ro'yxat uchun).
/// EN: Screen density (dp -> pixel factor). Read while building the UI (for example, by virtualized lists).
/// RU: Плотность экрана (коэффициент dp -> пиксели). Читается при построении UI (например, виртуальными списками).
pub fn density() -> f32 {
    DENSITY.with(|d| d.get())
}

pub fn set_density(d: f32) {
    DENSITY.with(|c| c.set(d));
}

/// UZ: Oyna o'lchami (dp). UI qurilayotganda o'qiladi (masalan, kartalar kengligini hisoblash uchun).
/// EN: Window size (dp). Read while building the UI (for example, to compute card widths).
/// RU: Размер окна (dp). Читается при построении UI (например, для расчёта ширины карточек).
pub fn window_dp() -> (f32, f32) {
    WINDOW_DP.with(|w| w.get())
}

pub fn set_window_dp(w: f32, h: f32) {
    WINDOW_DP.with(|c| c.set((w, h)));
}
