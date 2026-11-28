//! Matn: TrueType shriftdan harf atlasi (A8) tuzish va matn o'lchash. Tashqi kutubxona yo'q.
use crate::font::{raster, ttf::Font};

#[derive(Clone, Copy, Default, Debug)]
pub struct Glyph {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
    /// Qalam nuqtasidan chap va yuqori chetgacha (piksel; top — bazaviy chiziqdan tepaga).
    pub left: i16,
    pub top: i16,
    pub adv: f32,
}

pub struct Atlas {
    pub w: usize,
    pub h: usize,
    pub px: Vec<u8>,
    glyphs: Vec<Glyph>,
    /// Atlasga rasterlangan o'lcham (em, piksel).
    pub size: f32,
    pub ascent: f32,
    pub line: f32,
    /// To'liq oq 4x4 maydon (shakllar uchun) markazi, UV da.
    pub white_uv: (f32, f32),
}

const FIRST: u32 = 32;
const LAST: u32 = 126;

impl Atlas {
    pub fn build(font: &[u8], size: f32) -> Option<Atlas> {
        let f = Font::parse(font)?;
        let (w, h) = (512usize, 512usize);
        let mut px = vec![0u8; w * h];
        for y in 0..4 {
            for x in 0..4 {
                px[y * w + x] = 255;
            }
        }
        let mut glyphs = vec![Glyph::default(); (LAST - FIRST + 1) as usize];
        let (mut cx, mut cy, mut row_h) = (6usize, 1usize, 0usize);
        let scale = size / f.units_per_em as f32;
        for code in FIRST..=LAST {
            let ch = char::from_u32(code)?;
            let gid = f.glyph_id(ch);
            let adv = f.advance_units(gid) as f32 * scale;
            let contours = f.outline(gid);
            let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            for c in &contours {
                for p in c {
                    x0 = x0.min(p.x);
                    x1 = x1.max(p.x);
                    y0 = y0.min(p.y);
                    y1 = y1.max(p.y);
                }
            }
            let g = &mut glyphs[(code - FIRST) as usize];
            g.adv = adv;
            if x0 > x1 {
                continue;
            }
            let left = (x0 * scale).floor() as i32 - 1;
            let top = (y1 * scale).ceil() as i32 + 1;
            let right = (x1 * scale).ceil() as i32 + 1;
            let bottom = (y0 * scale).floor() as i32 - 1;
            let (gw, gh) = ((right - left) as usize, (top - bottom) as usize);
            if cx + gw + 1 > w {
                cx = 1;
                cy += row_h + 1;
                row_h = 0;
            }
            if cy + gh + 1 > h {
                return None;
            }
            let segs = raster::flatten(&contours, scale, left as f32, top as f32);
            let cov = raster::fill(&segs, gw, gh);
            for yy in 0..gh {
                let dst = (cy + yy) * w + cx;
                px[dst..dst + gw].copy_from_slice(&cov[yy * gw..yy * gw + gw]);
            }
            *g = Glyph {
                x: cx as u16,
                y: cy as u16,
                w: gw as u16,
                h: gh as u16,
                left: left as i16,
                top: top as i16,
                adv,
            };
            cx += gw + 1;
            row_h = row_h.max(gh);
        }
        Some(Atlas {
            w,
            h,
            px,
            glyphs,
            size,
            ascent: f.ascent as f32 * scale,
            line: (f.ascent as f32 - f.descent as f32 + f.line_gap as f32) * scale,
            white_uv: (2.0 / w as f32, 2.0 / h as f32),
        })
    }

    pub fn glyph(&self, c: char) -> Option<&Glyph> {
        let c = match c {
            '\u{02BB}' | '\u{2018}' | '\u{2019}' | '\u{02BC}' => '\'',
            c => c,
        };
        let code = c as u32;
        if (FIRST..=LAST).contains(&code) {
            Some(&self.glyphs[(code - FIRST) as usize])
        } else {
            None
        }
    }

    /// Matn kengligi, `px` — em balandligi (ekran pikseli).
    pub fn width(&self, s: &str, px: f32) -> f32 {
        let k = px / self.size;
        s.chars().map(|c| self.glyph(c).map_or(0.0, |g| g.adv) * k).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FONT: &[u8] = include_bytes!("../assets/font.ttf");

    #[test]
    fn atlas_quriladi() {
        let a = Atlas::build(FONT, 48.0).expect("atlas");
        let g = a.glyph('A').unwrap();
        assert!(g.w > 5 && g.h > 10);
        assert!(g.adv > 5.0);
        let sum: u64 = a.px.iter().map(|&v| v as u64).sum();
        assert!(sum > 200_000, "atlas bo'sh");
        // bo'sh joy (probel) kengligi bor, lekin rasmi yo'q
        let sp = a.glyph(' ').unwrap();
        assert!(sp.adv > 2.0 && sp.w == 0);
        assert!(a.width("ARU", 48.0) > a.width("A", 48.0));
        assert!((a.width("ARU", 96.0) - 2.0 * a.width("ARU", 48.0)).abs() < 1e-3);
    }

    #[test]
    fn hamma_ascii_harf_atlasda_va_chegarada() {
        let a = Atlas::build(FONT, 48.0).unwrap();
        for code in 33u32..=126 {
            let c = char::from_u32(code).unwrap();
            let g = a.glyph(c).unwrap();
            assert!((g.x as usize + g.w as usize) <= a.w && (g.y as usize + g.h as usize) <= a.h, "{c}");
        }
        assert!(a.glyph('é').is_none());
        assert_eq!(a.glyph('\u{02BB}').unwrap().adv, a.glyph('\'').unwrap().adv);
    }
}
