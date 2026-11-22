//! ARUMEDIATV ranglari (Kotlin ilovadagi `AruColors` bilan bir xil).

use uzcoders::prelude::*;

pub const BG: Color = Color::hex(0x0A0A0C);
pub const SURFACE: Color = Color::hex(0x101012);
pub const ACCENT: Color = Color::hex(0xC2410C);
pub const ACCENT2: Color = Color::hex(0xE2620F);
pub const BORDER: Color = Color::argb(0x0FFFFFFF);
pub const BORDER_BRIGHT: Color = Color::argb(0x1AFFFFFF);
// UZ: Kotlin `AruColors` bilan BIR XIL (Text F2F2F3, TextDim 9A9AA0, TextFaint 6B6B70). EN: identical to Kotlin `AruColors`.
pub const TEXT: Color = Color::hex(0xF2F2F3);
pub const TEXT_DIM: Color = Color::hex(0x9A9AA0);
pub const TEXT_FAINT: Color = Color::hex(0x6B6B70);
pub const CARD_ALT: Color = Color::hex(0x1B1B1E);
pub const ACCENT_TINT: Color = Color::hex(0x281914);
pub const ERROR: Color = Color::hex(0xF87171);
pub const CARD: Color = Color::hex(0x151517);
pub const GOLD: Color = Color::hex(0xFFC93C);

/// Barcha lupali shisha oynalarning kattalashtirish darajasi (orqadagi narsalar shuncha marta kattalashadi).
pub const GLASS_ZOOM: f32 = 1.3;

/// UZ: `false` — "shisha" (lupa/xiralik) VAQTINCHA o'chirilgan: hamma panel Kotlin ilovasidagidek oddiy kartaga (`CARD` + 1dp chegara) aylanadi.
/// Qaytarish uchun faqat shuni `true` qiling. EN: `false` = glass temporarily off, panels render as flat Kotlin-style cards.
pub const GLASS_ON: bool = false;

/// UZ: shisha o'rniga Kotlin uslubidagi tekis ko'rinish. Urg'uli (apelsin) tint — to'liq apelsin fon; yorqin oq (slayder izi) — shaffof oq; qolgani — karta.
/// EN: flat replacement for glass. Accent tint -> solid accent, strong white (slider track) -> translucent white, the rest -> card + border.
pub trait Flat: Sized {
    fn flat_lens(self, tint: Color, zoom: f32) -> Self;
    fn flat_glass(self, tint: Color) -> Self;
}

fn flat_fill(e: Element, tint: Color) -> Element {
    let accent = tint.r == ACCENT.r && tint.g == ACCENT.g && tint.b == ACCENT.b;
    if accent {
        // Kotlin: faol tugma — to'liq apelsin (o'chiq holatda yarim shaffof).
        e.bg(if tint.a >= 120 { ACCENT } else { ACCENT.with_opacity(0.5) })
    } else if tint.a >= 30 {
        e.bg(tint)
    } else {
        e.bg(CARD).border(1.0, BORDER)
    }
}

impl Flat for Element {
    fn flat_lens(self, tint: Color, zoom: f32) -> Self {
        if GLASS_ON { self.lens(tint, zoom) } else { flat_fill(self, tint) }
    }
    fn flat_glass(self, tint: Color) -> Self {
        if GLASS_ON { self.glass(tint) } else { flat_fill(self, tint) }
    }
}

/// Barcha ekranlar uchun rangli orqa fon: shisha shu shakllarni sindirib va kattalashtirib ko'rsatadi.
pub fn glass_backdrop() -> Element {
    if !GLASS_ON {
        return stack(vec![column(vec![]).bg(BG)]);
    }
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

