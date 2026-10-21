//! UZ: Rang: RGBA (har bir kanal 0..=255), alfa kanali to'g'ri (premultiplied emas) saqlanadi.
//! EN: Color: RGBA (each channel 0..=255); alpha is stored straight (not premultiplied).
//! RU: Цвет: RGBA (каждый канал 0..=255); альфа хранится прямая (не premultiplied).

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
    /// UZ: `0xRRGGBB` qiymatidan rang yasaydi (alfa = 255, ya'ni to'liq quyuq).
    /// EN: Builds a color from `0xRRGGBB` (alpha = 255, fully opaque).
    /// RU: Создаёт цвет из `0xRRGGBB` (альфа = 255, полностью непрозрачный).
    pub const fn hex(v: u32) -> Self {
        Self::rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }
    /// UZ: `0xAARRGGBB` qiymatidan rang yasaydi (Android ARGB tartibi).
    /// EN: Builds a color from `0xAARRGGBB` (Android ARGB order).
    /// RU: Создаёт цвет из `0xAARRGGBB` (порядок ARGB, как в Android).
    pub const fn argb(v: u32) -> Self {
        Self::rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, (v >> 24) as u8)
    }
    /// UZ: `#RRGGBB` yoki `#RRGGBBAA` matnidan rangni o'qiydi; noto'g'ri bo'lsa `None`.
    /// EN: Parses a color from `#RRGGBB` or `#RRGGBBAA` text; returns `None` if invalid.
    /// RU: Разбирает цвет из строки `#RRGGBB` или `#RRGGBBAA`; при ошибке возвращает `None`.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.strip_prefix('#').unwrap_or(s);
        let byte = |i: usize| u8::from_str_radix(s.get(i..i + 2)?, 16).ok();
        match s.len() {
            6 => Some(Self::rgb(byte(0)?, byte(2)?, byte(4)?)),
            8 => Some(Self::rgba(byte(0)?, byte(2)?, byte(4)?, byte(6)?)),
            _ => None,
        }
    }
    /// UZ: Alfa kanalini `0.0..=1.0` ga ko'paytiradi (shaffoflikni kamaytirish uchun).
    /// EN: Multiplies the alpha channel by `0.0..=1.0` (to make the color more transparent).
    /// RU: Умножает альфа-канал на `0.0..=1.0` (чтобы сделать цвет прозрачнее).
    pub fn with_opacity(self, o: f32) -> Self {
        Self {
            a: (self.a as f32 * o.clamp(0.0, 1.0)).round() as u8,
            ..self
        }
    }
    /// UZ: Ikki rang orasidagi chiziqli oraliq rang (`t` = 0 → `self`, `t` = 1 → `o`).
    /// EN: Linear interpolation between two colors (`t` = 0 → `self`, `t` = 1 → `o`).
    /// RU: Линейная интерполяция между двумя цветами (`t` = 0 → `self`, `t` = 1 → `o`).
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
    /// UZ: Rangni qoraytiradi (`f` = 0..1; 0 — o'zgarishsiz, 1 — butunlay qora).
    /// EN: Darkens the color (`f` = 0..1; 0 leaves it unchanged, 1 makes it fully black).
    /// RU: Затемняет цвет (`f` = 0..1; 0 — без изменений, 1 — полностью чёрный).
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
