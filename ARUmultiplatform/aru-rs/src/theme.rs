//! Ranglar va matn yordamchilari. Uslub: qora plita, oq (teskari) faol belgi, bitta signal rangi.

use uzcoders::prelude::*;

pub const BG: Color = Color::hex(0x000000);
pub const S1: Color = Color::hex(0x121212);
pub const S2: Color = Color::hex(0x171717);
pub const S3: Color = Color::hex(0x242424);
pub const LINE: Color = Color::hex(0x2A2A2A);
pub const MUTE: Color = Color::hex(0x9B9B9B);
pub const DIM: Color = Color::hex(0xC9C9C9);
pub const SIG: Color = Color::hex(0xE2620F);
pub const WHITE: Color = Color::WHITE;

/// Oddiy matn.
pub fn t(s: impl Into<String>, size: f32, c: Color) -> Element {
    text(s).size(size).color(c).line_height(size * 1.3)
}

/// Qalin matn.
pub fn b(s: impl Into<String>, size: f32, c: Color) -> Element {
    t(s, size, c).bold()
}

/// Sarlavha: tor va juda qalin, bosh harflarda, harflar bir-biriga yopishgan.
pub fn disp(s: &str, size: f32, c: Color) -> Element {
    text(s.to_uppercase()).size(size).color(c).black().letter_spacing(-size * 0.02).line_height(size * 0.95)
}

/// Uzun matnni `n` belgigacha qisqartiradi.
pub fn short(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    let mut o: String = s.chars().take(n.saturating_sub(1)).collect();
    o.push('…');
    o
}
