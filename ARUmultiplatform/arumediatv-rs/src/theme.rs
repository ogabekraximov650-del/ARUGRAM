//! ARUMEDIATV ranglari (Kotlin ilovadagi `AruColors` bilan bir xil).

use uzcoders::prelude::*;

pub const BG: Color = Color::hex(0x0A0A0C);
pub const SURFACE: Color = Color::hex(0x101012);
pub const ACCENT: Color = Color::hex(0xC2410C);
pub const ACCENT2: Color = Color::hex(0xE2620F);
pub const BORDER_BRIGHT: Color = Color::argb(0x1AFFFFFF);
pub const TEXT: Color = Color::hex(0xFFFFFF);
pub const TEXT_DIM: Color = Color::hex(0xF2F2F5);
pub const TEXT_FAINT: Color = Color::hex(0xE2E2E8);
pub const ERROR: Color = Color::hex(0xF87171);
pub const CARD: Color = Color::hex(0x151517);
pub const GOLD: Color = Color::hex(0xFFC93C);

/// Barcha lupali shisha oynalarning kattalashtirish darajasi (orqadagi narsalar shuncha marta kattalashadi).
pub const GLASS_ZOOM: f32 = 1.3;

/// Barcha ekranlar uchun rangli orqa fon: shisha shu shakllarni sindirib va kattalashtirib ko'rsatadi.
pub fn glass_backdrop() -> Element {
    stack(vec![
        column(vec![]).bg(BG).glows(vec![
            (0.15, 0.10, 280.0, Color::rgba(226, 98, 15, 170)),
            (0.95, 0.30, 300.0, Color::rgba(110, 80, 230, 150)),
            (0.05, 0.62, 280.0, Color::rgba(40, 170, 150, 120)),
            (0.90, 0.88, 300.0, Color::rgba(226, 98, 15, 130)),
        ]),
        column(vec![
            gap_box(0.0, 120.0),
            row(vec![gap_box(36.0, 0.0), column(vec![]).width(120.0).height(120.0).radius(60.0).gradient(ACCENT2, Color::hex(0x7C3AED))]),
            gap_box(0.0, 190.0),
            row(vec![spacer(), column(vec![]).width(150.0).height(70.0).radius(35.0).gradient(Color::hex(0x14B8A6), Color::hex(0x2563EB)), gap_box(30.0, 0.0)]),
            gap_box(0.0, 220.0),
            row(vec![gap_box(60.0, 0.0), column(vec![]).width(100.0).height(100.0).radius(28.0).gradient(Color::hex(0xF59E0B), ACCENT)]),
        ]),
    ])
}

