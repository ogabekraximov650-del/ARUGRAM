//! uzcoders-text — shrift o'qish, harflarni chizish va matnni qatorlarga bo'lish (noldan yozilgan).

pub mod raster;
pub mod ttf;

use std::collections::HashMap;
use std::rc::Rc;
pub use ttf::Font;

/// Bitta harfning piksel qoplamasi. `left`/`top` — chizish nuqtasidan (baseline boshi) siljish:
/// bitmap chap-yuqori burchagi `(pen_x + left, baseline - top)` da turadi.
#[derive(Debug, Clone, Default)]
pub struct GlyphBitmap {
    pub width: usize,
    pub height: usize,
    pub left: i32,
    pub top: i32,
    pub advance: f32,
    pub data: Vec<u8>,
}

impl Font {
    /// Harfni `size` (piksel, em balandligi) o'lchamda rasterlaydi.
    pub fn rasterize(&self, ch: char, size: f32) -> GlyphBitmap {
        let gid = self.glyph_id(ch);
        let scale = size / self.units_per_em as f32;
        let advance = self.advance_units(gid) as f32 * scale;
        let contours = self.outline(gid);
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for c in &contours {
            for p in c {
                x0 = x0.min(p.x);
                x1 = x1.max(p.x);
                y0 = y0.min(p.y);
                y1 = y1.max(p.y);
            }
        }
        if x0 > x1 {
            return GlyphBitmap {
                advance,
                ..Default::default()
            };
        }
        let left = (x0 * scale).floor() as i32 - 1;
        let top = (y1 * scale).ceil() as i32 + 1;
        let right = (x1 * scale).ceil() as i32 + 1;
        let bottom = (y0 * scale).floor() as i32 - 1;
        let (w, h) = ((right - left) as usize, (top - bottom) as usize);
        let segs = raster::flatten(&contours, scale, left as f32, top as f32);
        GlyphBitmap {
            width: w,
            height: h,
            left,
            top,
            advance,
            data: raster::fill(&segs, w, h),
        }
    }

    /// Bitta qator balandligi (piksel).
    pub fn line_height(&self, size: f32) -> f32 {
        let s = size / self.units_per_em as f32;
        (self.ascent as f32 - self.descent as f32 + self.line_gap as f32) * s
    }

    pub fn ascent_px(&self, size: f32) -> f32 {
        self.ascent as f32 * size / self.units_per_em as f32
    }

    pub fn char_advance(&self, ch: char, size: f32) -> f32 {
        self.advance_units(self.glyph_id(ch)) as f32 * size / self.units_per_em as f32
    }

    pub fn text_width(&self, text: &str, size: f32) -> f32 {
        text.chars().map(|c| self.char_advance(c, size)).sum()
    }

    /// Matnni `max_width` ga sig'adigan qatorlarga bo'ladi (so'z chegarasida; juda uzun so'z belgi bo'yicha).
    /// `\n` majburiy qator uzilishi. `max_width = None` — bo'lmaydi.
    pub fn wrap(&self, text: &str, size: f32, max_width: Option<f32>) -> Vec<String> {
        let mut lines = Vec::new();
        for para in text.split('\n') {
            let max = match max_width {
                Some(m) => m,
                None => {
                    lines.push(para.to_string());
                    continue;
                }
            };
            let mut line = String::new();
            let mut w = 0.0f32;
            for word in split_words(para) {
                let ww = self.text_width(word, size);
                let is_space = word.starts_with(' ');
                if w + ww <= max || line.is_empty() && is_space {
                    line.push_str(word);
                    w += ww;
                    continue;
                }
                if is_space {
                    continue; // qator oxiridagi probel tashlanadi
                }
                if !line.is_empty() {
                    lines.push(line.trim_end().to_string());
                    line = String::new();
                    w = 0.0;
                }
                if ww <= max {
                    line.push_str(word);
                    w = ww;
                } else {
                    for ch in word.chars() {
                        let cw = self.char_advance(ch, size);
                        if w + cw > max && !line.is_empty() {
                            lines.push(std::mem::take(&mut line));
                            w = 0.0;
                        }
                        line.push(ch);
                        w += cw;
                    }
                }
            }
            lines.push(line.trim_end().to_string());
        }
        lines
    }

    /// Matn o'lchami: (kenglik, balandlik) va qatorlar.
    pub fn measure(
        &self,
        text: &str,
        size: f32,
        max_width: Option<f32>,
    ) -> (f32, f32, Vec<String>) {
        let lines = self.wrap(text, size, max_width);
        let w = lines
            .iter()
            .map(|l| self.text_width(l, size))
            .fold(0.0, f32::max);
        (w, lines.len() as f32 * self.line_height(size), lines)
    }
}

fn split_words(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut prev_space: Option<bool> = None;
    for (i, c) in s.char_indices() {
        let sp = c == ' ';
        if let Some(p) = prev_space {
            if p != sp {
                out.push(&s[start..i]);
                start = i;
            }
        }
        prev_space = Some(sp);
    }
    if start < s.len() {
        out.push(&s[start..]);
    }
    out
}

/// Harf bitmaplari keshi: bir xil (harf, o'lcham) qayta rasterlanmaydi.
pub struct GlyphCache {
    font: Rc<Font>,
    map: HashMap<(char, u32), Rc<GlyphBitmap>>,
}

impl GlyphCache {
    pub fn new(font: Rc<Font>) -> Self {
        GlyphCache {
            font,
            map: HashMap::new(),
        }
    }

    pub fn font(&self) -> &Font {
        &self.font
    }

    pub fn get(&mut self, ch: char, size: f32) -> Rc<GlyphBitmap> {
        let key = (ch, (size * 4.0).round() as u32);
        if let Some(g) = self.map.get(&key) {
            return g.clone();
        }
        let g = Rc::new(self.font.rasterize(ch, key.1 as f32 / 4.0));
        self.map.insert(key, g.clone());
        g
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn font() -> Font {
        Font::parse(include_bytes!("../../../assets/fonts/DejaVuSans.ttf"))
            .expect("shrift o'qilmadi")
    }

    #[test]
    fn parse_va_metrika() {
        let f = font();
        assert_eq!(f.units_per_em, 2048);
        assert!(f.num_glyphs() > 1000);
        assert!(f.ascent > 0 && f.descent < 0);
        assert_ne!(f.glyph_id('A'), 0);
        assert_ne!(f.glyph_id('ё'), 0);
        assert_ne!(f.glyph_id('ў'), 0);
        assert_eq!(f.glyph_id('\u{10FFFF}'), 0);
    }

    #[test]
    fn harf_qoplamasi() {
        let f = font();
        let g = f.rasterize('A', 32.0);
        assert!(g.width > 10 && g.height > 15);
        assert!(g.data.iter().any(|&v| v == 255));
        assert!(g.data.iter().any(|&v| v == 0));
        let ink: u32 = g.data.iter().map(|&v| v as u32).sum();
        assert!(ink > 255 * 80);
        assert!(g.advance > 15.0 && g.advance < 30.0);
    }

    #[test]
    fn maydon_mosligi() {
        // 'I' to'g'ri to'rtburchak: qoplama yig'indisi maydonga yaqin bo'lishi kerak.
        let f = font();
        let g = f.rasterize('I', 64.0);
        let ink: f32 = g.data.iter().map(|&v| v as f32 / 255.0).sum();
        let gid = f.glyph_id('I');
        let c = f.outline(gid);
        let xs: Vec<f32> = c.iter().flatten().map(|p| p.x).collect();
        let ys: Vec<f32> = c.iter().flatten().map(|p| p.y).collect();
        let s = 64.0 / 2048.0;
        let w = (xs.iter().cloned().fold(f32::MIN, f32::max)
            - xs.iter().cloned().fold(f32::MAX, f32::min))
            * s;
        let h = (ys.iter().cloned().fold(f32::MIN, f32::max)
            - ys.iter().cloned().fold(f32::MAX, f32::min))
            * s;
        assert!(
            (ink - w * h).abs() < w * h * 0.15,
            "ink={ink} area={}",
            w * h
        );
    }

    #[test]
    fn teshik_harf() {
        // 'O' markazi bo'sh (teshik), halqasi to'la.
        let f = font();
        let g = f.rasterize('O', 64.0);
        let mid = g.data[(g.height / 2) * g.width + g.width / 2];
        assert_eq!(mid, 0);
        let row = &g.data[(g.height / 2) * g.width..(g.height / 2 + 1) * g.width];
        assert!(row[..g.width / 3].iter().any(|&v| v == 255));
    }

    #[test]
    fn probel_bosh() {
        let g = font().rasterize(' ', 20.0);
        assert!(g.data.is_empty());
        assert!(g.advance > 3.0);
    }

    #[test]
    fn murakkab_glif() {
        // 'é' murakkab (e + urg'u) bo'lishi mumkin: kamida 2 kontur.
        let f = font();
        assert!(f.outline(f.glyph_id('é')).len() >= 2);
    }

    #[test]
    fn qatorga_bolish() {
        let f = font();
        let w = f.text_width("salom dunyo", 16.0);
        let lines = f.wrap("salom dunyo", 16.0, Some(w * 0.7));
        assert_eq!(lines, vec!["salom", "dunyo"]);
        let one = f.wrap("salom dunyo", 16.0, Some(w + 1.0));
        assert_eq!(one.len(), 1);
        assert_eq!(f.wrap("a\nb", 16.0, None), vec!["a", "b"]);
        let long = f.wrap("aaaaaaaaaaaaaaaaaaaa", 16.0, Some(40.0));
        assert!(long.len() > 3);
        assert!(long.iter().all(|l| f.text_width(l, 16.0) <= 40.0 + 12.0));
    }

    #[test]
    fn kesh() {
        let mut c = GlyphCache::new(Rc::new(font()));
        let a = c.get('a', 20.0);
        let b = c.get('a', 20.0);
        assert!(Rc::ptr_eq(&a, &b));
        c.get('a', 24.0);
        assert_eq!(c.len(), 2);
    }
}
