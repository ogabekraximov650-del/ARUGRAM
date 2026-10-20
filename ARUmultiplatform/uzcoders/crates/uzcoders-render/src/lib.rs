//! uzcoders-render — chizish buyruqlari ro'yxati va dasturiy (CPU) canvas.

pub mod png;

use std::rc::Rc;
use uzcoders_core::{Color, Rect};
use uzcoders_text::{Font, GlyphCache};

/// Chizish buyrug'i. UI daraxti shu ro'yxatga aylanadi, backend (CPU yoki GPU) uni bajaradi.
#[derive(Clone, Debug)]
pub enum DrawCmd {
    /// Yumaloq burchakli to'rtburchak.
    RoundRect {
        rect: Rect,
        radius: f32,
        color: Color,
    },
    /// Matn: `x` — chap chet, `baseline` — qator asos chizig'i.
    Text {
        x: f32,
        baseline: f32,
        text: String,
        size: f32,
        color: Color,
    },
    PushClip(Rect),
    PopClip,
}

pub struct Canvas {
    pub width: usize,
    pub height: usize,
    /// RGBA8, qatorma-qator.
    pub pixels: Vec<u8>,
    glyphs: GlyphCache,
    clips: Vec<(i32, i32, i32, i32)>,
}

impl Canvas {
    pub fn new(width: usize, height: usize, font: Rc<Font>) -> Self {
        Canvas {
            width,
            height,
            pixels: vec![0; width * height * 4],
            glyphs: GlyphCache::new(font),
            clips: Vec::new(),
        }
    }

    /// O'lchamni o'zgartiradi (harf keshi saqlanadi); piksellar tozalanadi.
    pub fn resize(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
        self.pixels = vec![0; width * height * 4];
        self.clips.clear();
    }

    pub fn font(&self) -> &Font {
        self.glyphs.font()
    }

    pub fn clear(&mut self, c: Color) {
        for p in self.pixels.chunks_exact_mut(4) {
            p.copy_from_slice(&[c.r, c.g, c.b, c.a]);
        }
    }

    fn bounds(&self) -> (i32, i32, i32, i32) {
        let mut b = (0, 0, self.width as i32, self.height as i32);
        if let Some(&(x0, y0, x1, y1)) = self.clips.last() {
            b = (b.0.max(x0), b.1.max(y0), b.2.min(x1), b.3.min(y1));
        }
        b
    }

    fn blend(&mut self, x: i32, y: i32, c: Color, cov: f32) {
        let a = (c.a as f32 / 255.0) * cov;
        if a <= 0.0 {
            return;
        }
        let i = (y as usize * self.width + x as usize) * 4;
        let d = &mut self.pixels[i..i + 4];
        let da = d[3] as f32 / 255.0;
        let oa = a + da * (1.0 - a);
        if oa <= 0.0 {
            return;
        }
        let mix = |s: u8, dv: u8| ((s as f32 * a + dv as f32 * da * (1.0 - a)) / oa + 0.5) as u8;
        d[0] = mix(c.r, d[0]);
        d[1] = mix(c.g, d[1]);
        d[2] = mix(c.b, d[2]);
        d[3] = (oa * 255.0 + 0.5) as u8;
    }

    pub fn fill_round_rect(&mut self, r: Rect, radius: f32, c: Color) {
        let (bx0, by0, bx1, by1) = self.bounds();
        let x0 = (r.x.floor() as i32).max(bx0);
        let y0 = (r.y.floor() as i32).max(by0);
        let x1 = (r.right().ceil() as i32).min(bx1);
        let y1 = (r.bottom().ceil() as i32).min(by1);
        let (hw, hh) = (r.w * 0.5, r.h * 0.5);
        let rad = radius.min(hw).min(hh).max(0.0);
        let (cx, cy) = (r.x + hw, r.y + hh);
        for y in y0..y1 {
            for x in x0..x1 {
                let (px, py) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                let qx = px.abs() - (hw - rad);
                let qy = py.abs() - (hh - rad);
                let sd =
                    (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - rad;
                self.blend(x, y, c, (0.5 - sd).clamp(0.0, 1.0));
            }
        }
    }

    pub fn draw_text(&mut self, x: f32, baseline: f32, text: &str, size: f32, c: Color) {
        let (bx0, by0, bx1, by1) = self.bounds();
        let mut pen = x;
        for ch in text.chars() {
            let g = self.glyphs.get(ch, size);
            let gx = pen.round() as i32 + g.left;
            let gy = baseline.round() as i32 - g.top;
            for row in 0..g.height {
                let y = gy + row as i32;
                if y < by0 || y >= by1 {
                    continue;
                }
                for col in 0..g.width {
                    let x = gx + col as i32;
                    if x < bx0 || x >= bx1 {
                        continue;
                    }
                    let v = g.data[row * g.width + col];
                    if v != 0 {
                        self.blend(x, y, c, v as f32 / 255.0);
                    }
                }
            }
            pen += g.advance;
        }
    }

    pub fn execute(&mut self, cmds: &[DrawCmd]) {
        for cmd in cmds {
            match cmd {
                DrawCmd::RoundRect {
                    rect,
                    radius,
                    color,
                } => self.fill_round_rect(*rect, *radius, *color),
                DrawCmd::Text {
                    x,
                    baseline,
                    text,
                    size,
                    color,
                } => self.draw_text(*x, *baseline, text, *size, *color),
                DrawCmd::PushClip(r) => {
                    let (bx0, by0, bx1, by1) = self.bounds();
                    self.clips.push((
                        (r.x.floor() as i32).max(bx0),
                        (r.y.floor() as i32).max(by0),
                        (r.right().ceil() as i32).min(bx1),
                        (r.bottom().ceil() as i32).min(by1),
                    ));
                }
                DrawCmd::PopClip => {
                    self.clips.pop();
                }
            }
        }
    }

    pub fn pixel(&self, x: usize, y: usize) -> Color {
        let i = (y * self.width + x) * 4;
        Color::rgba(
            self.pixels[i],
            self.pixels[i + 1],
            self.pixels[i + 2],
            self.pixels[i + 3],
        )
    }

    pub fn to_png(&self) -> Vec<u8> {
        png::encode_rgba(self.width as u32, self.height as u32, &self.pixels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canvas(w: usize, h: usize) -> Canvas {
        let f = Font::parse(include_bytes!("../../../assets/fonts/DejaVuSans.ttf")).unwrap();
        Canvas::new(w, h, Rc::new(f))
    }

    #[test]
    fn tortburchak_va_burchak() {
        let mut c = canvas(40, 40);
        c.clear(Color::BLACK);
        c.fill_round_rect(Rect::new(5.5, 5.0, 30.0, 30.0), 10.0, Color::WHITE);
        assert_eq!(c.pixel(20, 20), Color::WHITE);
        assert_eq!(c.pixel(5, 5), Color::BLACK); // yumaloq burchak ichida emas
        assert_eq!(c.pixel(2, 20), Color::BLACK);
        let edge = c.pixel(5, 20);
        assert!(
            edge.r > 100 && edge.r < 255,
            "chekka silliqlanishi: {edge:?}"
        );
    }

    #[test]
    fn matn_va_klip() {
        let mut c = canvas(80, 30);
        c.clear(Color::WHITE);
        c.execute(&[
            DrawCmd::PushClip(Rect::new(0.0, 0.0, 20.0, 30.0)),
            DrawCmd::Text {
                x: 2.0,
                baseline: 22.0,
                text: "Hello".into(),
                size: 20.0,
                color: Color::BLACK,
            },
            DrawCmd::PopClip,
        ]);
        let dark_left = (0..20).any(|x| (0..30).any(|y| c.pixel(x, y).r < 100));
        let dark_right = (20..80).any(|x| (0..30).any(|y| c.pixel(x, y).r < 250));
        assert!(dark_left);
        assert!(!dark_right);
    }

    #[test]
    fn shaffof_aralash() {
        let mut c = canvas(4, 4);
        c.clear(Color::BLACK);
        c.fill_round_rect(
            Rect::new(0.0, 0.0, 4.0, 4.0),
            0.0,
            Color::rgba(255, 255, 255, 128),
        );
        let p = c.pixel(2, 2);
        assert!(p.r > 120 && p.r < 136);
    }

    #[test]
    fn png_hajmi() {
        let c = canvas(3, 2);
        assert!(c.to_png().len() > 50);
    }
}
