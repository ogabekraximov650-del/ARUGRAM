//! Rang: RGBA (har bir kanal 0..=255), to'g'ri (premultiplied emas) alfa.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const TRANSPARENT: Color = Color::rgba(0, 0, 0, 0);
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    pub const WHITE: Color = Color::rgb(255, 255, 255);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
    /// `0xRRGGBB` (alfa = 255).
    pub const fn hex(v: u32) -> Self {
        Self::rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }
    /// `0xAARRGGBB` (Android ARGB tartibi).
    pub const fn argb(v: u32) -> Self {
        Self::rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, (v >> 24) as u8)
    }
    /// `#RRGGBB` yoki `#RRGGBBAA` matnidan.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.strip_prefix('#').unwrap_or(s);
        let byte = |i: usize| u8::from_str_radix(s.get(i..i + 2)?, 16).ok();
        match s.len() {
            6 => Some(Self::rgb(byte(0)?, byte(2)?, byte(4)?)),
            8 => Some(Self::rgba(byte(0)?, byte(2)?, byte(4)?, byte(6)?)),
            _ => None,
        }
    }
    /// Alfani `0.0..=1.0` ga ko'paytiradi.
    pub fn with_opacity(self, o: f32) -> Self {
        Self {
            a: (self.a as f32 * o.clamp(0.0, 1.0)).round() as u8,
            ..self
        }
    }
    /// Ikki rang orasida chiziqli oraliq (`t` = 0 → `self`, 1 → `o`).
    pub fn lerp(self, o: Color, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Color::rgba(
            m(self.r, o.r),
            m(self.g, o.g),
            m(self.b, o.b),
            m(self.a, o.a),
        )
    }
    /// Qoraytirish (`f` = 0..1, 0 — o'zgarishsiz).
    pub fn darken(self, f: f32) -> Color {
        self.lerp(Color::rgba(0, 0, 0, self.a), f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_hex() {
        assert_eq!(Color::parse("#3390EC"), Some(Color::hex(0x3390EC)));
        assert_eq!(
            Color::parse("3390EC80"),
            Some(Color::rgba(0x33, 0x90, 0xEC, 0x80))
        );
        assert_eq!(Color::parse("#12"), None);
        assert_eq!(Color::argb(0x80FF0000), Color::rgba(255, 0, 0, 128));
    }

    #[test]
    fn lerp_and_opacity() {
        assert_eq!(
            Color::BLACK.lerp(Color::WHITE, 0.5),
            Color::rgb(128, 128, 128)
        );
        assert_eq!(Color::WHITE.with_opacity(0.5).a, 128);
    }
}
