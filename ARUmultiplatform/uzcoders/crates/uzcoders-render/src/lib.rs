//! uzcoders-render — chizish buyruqlari ro'yxati va dasturiy (CPU) canvas.

pub mod image;
pub mod jpeg;
pub mod path;
pub mod png;

pub use image::{decode_image, decode_png, Image};
pub use jpeg::decode_jpeg;
pub use path::Path;

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
    /// Yumaloq burchakli kontur (ichkariga qarab `width` qalinlikda).
    RoundRectStroke {
        rect: Rect,
        radius: f32,
        width: f32,
        color: Color,
    },
    /// "Shisha" panel: shaffof tint, chetida yorug'lik chizig'i (yuqori-chapdan kuchliroq), ichki yorug'lik va yuqori yaltirash.
    Glass {
        rect: Rect,
        radius: f32,
        tint: Color,
    },
    /// Vertikal gradient to'ldirish (yumaloq burchakli): tepa va past ranglar orasida.
    Gradient {
        rect: Rect,
        radius: f32,
        top: Color,
        bottom: Color,
    },
    /// Haqiqiy shisha: ostidagi (shu paytgacha chizilgan) tasvir `blur` piksel radiusda xiralashadi, ustiga `tint` va yorug' chegara.
    Backdrop {
        rect: Rect,
        radius: f32,
        blur: f32,
        /// Linza kattalashtirishi (1 — yo'q): ostidagi tasvir panel markazidan shuncha marta kattalashadi ("lupa" effekti).
        zoom: f32,
        tint: Color,
    },
    /// Aylana yoyi (yuklanish ko'rsatkichi): `start` va `sweep` radianda, `width` — chiziq qalinligi.
    Arc {
        cx: f32,
        cy: f32,
        radius: f32,
        width: f32,
        start: f32,
        sweep: f32,
        color: Color,
    },
    /// Yumshoq radial yorug'lik dog'i (markazdan chetga so'nadi): fon uchun.
    Glow {
        cx: f32,
        cy: f32,
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
        /// Shrift qalinligi (400 oddiy, 700 qalin, 900 qora); eng yaqin yuz tanlanadi.
        weight: u16,
        /// Har belgidan keyin qo'shiladigan oraliq (piksel).
        spacing: f32,
    },
    /// Rasm `rect` ga cho'ziladi; `opacity` 0..1.
    Image {
        image: Rc<Image>,
        rect: Rect,
        opacity: f32,
    },
    /// Vektor yo'l (ikonka): `view` — yo'l koordinatalari qutisi (masalan 24), `rect` ga cho'ziladi.
    Path {
        path: Rc<Path>,
        view: f32,
        rect: Rect,
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
    /// Qo'shimcha shrift yuzlari (qalinlik, kesh).
    faces: Vec<(u16, GlyphCache)>,
    clips: Vec<(i32, i32, i32, i32)>,
}

impl Canvas {
    pub fn new(width: usize, height: usize, font: Rc<Font>) -> Self {
        Canvas {
            width,
            height,
            pixels: vec![0; width * height * 4],
            glyphs: GlyphCache::new(font),
            faces: Vec::new(),
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

    /// Qo'shimcha shrift yuzi (masalan qalin: 700). Oddiy (400) yuz `new` da beriladi.
    pub fn add_face(&mut self, weight: u16, font: Rc<Font>) {
        self.faces.push((weight, GlyphCache::new(font)));
    }

    fn cache_for(&mut self, weight: u16) -> &mut GlyphCache {
        let mut best: Option<usize> = None;
        let mut best_d = (400i32 - weight as i32).abs();
        for (i, (w, _)) in self.faces.iter().enumerate() {
            let d = (*w as i32 - weight as i32).abs();
            if d < best_d {
                best_d = d;
                best = Some(i);
            }
        }
        match best {
            Some(i) => &mut self.faces[i].1,
            None => &mut self.glyphs,
        }
    }

    pub fn font(&self) -> &Font {
        self.glyphs.font()
    }

    #[cfg(test)]
    fn font_rc(&self) -> Rc<Font> {
        self.glyphs.font_rc()
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

    pub fn stroke_round_rect(&mut self, r: Rect, radius: f32, width: f32, c: Color) {
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
                let outer = (0.5 - sd).clamp(0.0, 1.0);
                let inner = (0.5 - (sd + width)).clamp(0.0, 1.0);
                self.blend(x, y, c, outer - inner);
            }
        }
    }

    pub fn draw_text(
        &mut self,
        x: f32,
        baseline: f32,
        text: &str,
        size: f32,
        c: Color,
        weight: u16,
        spacing: f32,
    ) {
        let (bx0, by0, bx1, by1) = self.bounds();
        let mut pen = x;
        for ch in text.chars() {
            let g = self.cache_for(weight).get(ch, size);
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
            pen += g.advance + spacing;
        }
    }

    pub fn fill_gradient(&mut self, r: Rect, radius: f32, top: Color, bottom: Color) {
        let (bx0, by0, bx1, by1) = self.bounds();
        let x0 = (r.x.floor() as i32).max(bx0);
        let y0 = (r.y.floor() as i32).max(by0);
        let x1 = (r.right().ceil() as i32).min(bx1);
        let y1 = (r.bottom().ceil() as i32).min(by1);
        let (hw, hh) = (r.w * 0.5, r.h * 0.5);
        let rad = radius.min(hw).min(hh).max(0.0);
        let (cx, cy) = (r.x + hw, r.y + hh);
        for y in y0..y1 {
            let t = ((y as f32 + 0.5 - r.y) / r.h).clamp(0.0, 1.0);
            let c = top.lerp(bottom, t);
            for x in x0..x1 {
                let (px, py) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                let (qx, qy) = (px.abs() - (hw - rad), py.abs() - (hh - rad));
                let sd =
                    (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - rad;
                self.blend(x, y, c, (0.5 - sd).clamp(0.0, 1.0));
            }
        }
    }

    /// Backdrop: hududni xiralashtirib, yumaloq maska bilan qayta chizadi (CPU zaxirasi).
    pub fn draw_backdrop(&mut self, r: Rect, radius: f32, blur: f32, zoom: f32, tint: Color) {
        let (bx0, by0, bx1, by1) = self.bounds();
        let x0 = (r.x.floor() as i32).max(bx0);
        let y0 = (r.y.floor() as i32).max(by0);
        let x1 = (r.right().ceil() as i32).min(bx1);
        let y1 = (r.bottom().ceil() as i32).min(by1);
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let (w, h) = ((x1 - x0) as usize, (y1 - y0) as usize);
        let mut buf: Vec<[f32; 3]> = Vec::with_capacity(w * h);
        for y in y0..y1 {
            for x in x0..x1 {
                let i = (y as usize * self.width + x as usize) * 4;
                buf.push([
                    self.pixels[i] as f32,
                    self.pixels[i + 1] as f32,
                    self.pixels[i + 2] as f32,
                ]);
            }
        }
        // Linza: markazdan `zoom` marta kattalashtirib qayta tanlash.
        if zoom > 1.001 {
            let src = buf.clone();
            let (mx, my) = (w as f32 * 0.5, h as f32 * 0.5);
            for y in 0..h {
                for x in 0..w {
                    let sx =
                        (mx + (x as f32 + 0.5 - mx) / zoom).clamp(0.0, w as f32 - 1.0) as usize;
                    let sy =
                        (my + (y as f32 + 0.5 - my) / zoom).clamp(0.0, h as f32 - 1.0) as usize;
                    buf[y * w + x] = src[sy * w + sx];
                }
            }
        }
        // Ikki o'tishli quti xiralashtirish (gorizontal + vertikal), ikki marta ~ Gauss.
        let rad = ((blur * 0.5) as usize).max(1);
        for _ in 0..(if blur > 0.5 { 2 } else { 0 }) {
            for horizontal in [true, false] {
                let src = buf.clone();
                let (len, lines) = if horizontal { (w, h) } else { (h, w) };
                for l in 0..lines {
                    for i in 0..len {
                        let (a, b) = (i.saturating_sub(rad), (i + rad).min(len - 1));
                        let mut acc = [0f32; 3];
                        for k in a..=b {
                            let idx = if horizontal { l * w + k } else { k * w + l };
                            for c in 0..3 {
                                acc[c] += src[idx][c];
                            }
                        }
                        let n = (b - a + 1) as f32;
                        let idx = if horizontal { l * w + i } else { i * w + l };
                        buf[idx] = [acc[0] / n, acc[1] / n, acc[2] / n];
                    }
                }
            }
        }
        let (hw, hh) = (r.w * 0.5, r.h * 0.5);
        let rad_c = radius.min(hw).min(hh).max(0.0);
        let (cx, cy) = (r.x + hw, r.y + hh);
        for y in y0..y1 {
            for x in x0..x1 {
                let (px, py) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                let (qx, qy) = (px.abs() - (hw - rad_c), py.abs() - (hh - rad_c));
                let sd = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0)
                    - rad_c;
                let cov = (0.5 - sd).clamp(0.0, 1.0);
                if cov > 0.0 {
                    let b = buf[(y - y0) as usize * w + (x - x0) as usize];
                    self.blend(
                        x,
                        y,
                        Color::rgba(b[0] as u8, b[1] as u8, b[2] as u8, 255),
                        cov,
                    );
                }
            }
        }
        self.fill_round_rect(r, radius, tint);
        self.stroke_round_rect(r, radius, 1.5, Color::rgba(255, 255, 255, 90));
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_arc(&mut self, cx: f32, cy: f32, r: f32, w: f32, start: f32, sweep: f32, c: Color) {
        let (bx0, by0, bx1, by1) = self.bounds();
        let x0 = ((cx - r - 1.0).floor() as i32).max(bx0);
        let y0 = ((cy - r - 1.0).floor() as i32).max(by0);
        let x1 = ((cx + r + 1.0).ceil() as i32).min(bx1);
        let y1 = ((cy + r + 1.0).ceil() as i32).min(by1);
        let tau = std::f32::consts::TAU;
        for y in y0..y1 {
            for x in x0..x1 {
                let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                let d = (dx * dx + dy * dy).sqrt();
                let ring = (0.5 - ((d - (r - w * 0.5)).abs() - w * 0.5)).clamp(0.0, 1.0);
                if ring <= 0.0 {
                    continue;
                }
                let rel = (dy.atan2(dx) - start).rem_euclid(tau);
                let ca = (((rel * d).min((sweep - rel) * d)) + 0.5).clamp(0.0, 1.0);
                self.blend(x, y, c, ring * ca);
            }
        }
    }

    pub fn draw_glow(&mut self, cx: f32, cy: f32, radius: f32, c: Color) {
        let (bx0, by0, bx1, by1) = self.bounds();
        let x0 = ((cx - radius).floor() as i32).max(bx0);
        let y0 = ((cy - radius).floor() as i32).max(by0);
        let x1 = ((cx + radius).ceil() as i32).min(bx1);
        let y1 = ((cy + radius).ceil() as i32).min(by1);
        for y in y0..y1 {
            for x in x0..x1 {
                let d =
                    ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt() / radius;
                let k = (1.0 - d).clamp(0.0, 1.0);
                self.blend(x, y, c, k * k);
            }
        }
    }

    /// Rasmni `rect` ga (bilinear) cho'zib chizadi.
    pub fn draw_image(&mut self, img: &Image, rect: Rect, opacity: f32) {
        if rect.is_empty() || img.w == 0 || img.h == 0 {
            return;
        }
        let (bx0, by0, bx1, by1) = self.bounds();
        let x0 = (rect.x.floor() as i32).max(bx0);
        let y0 = (rect.y.floor() as i32).max(by0);
        let x1 = (rect.right().ceil() as i32).min(bx1);
        let y1 = (rect.bottom().ceil() as i32).min(by1);
        let (sx, sy) = (img.w as f32 / rect.w, img.h as f32 / rect.h);
        let at = |x: i32, y: i32| {
            let (x, y) = (
                x.clamp(0, img.w as i32 - 1) as usize,
                y.clamp(0, img.h as i32 - 1) as usize,
            );
            let i = (y * img.w as usize + x) * 4;
            [
                img.rgba[i] as f32,
                img.rgba[i + 1] as f32,
                img.rgba[i + 2] as f32,
                img.rgba[i + 3] as f32,
            ]
        };
        for y in y0..y1 {
            for x in x0..x1 {
                let u = (x as f32 + 0.5 - rect.x) * sx - 0.5;
                let v = (y as f32 + 0.5 - rect.y) * sy - 0.5;
                let (iu, iv) = (u.floor(), v.floor());
                let (fu, fv) = (u - iu, v - iv);
                let (p00, p10, p01, p11) = (
                    at(iu as i32, iv as i32),
                    at(iu as i32 + 1, iv as i32),
                    at(iu as i32, iv as i32 + 1),
                    at(iu as i32 + 1, iv as i32 + 1),
                );
                let mut c = [0f32; 4];
                for k in 0..4 {
                    c[k] = (p00[k] * (1.0 - fu) + p10[k] * fu) * (1.0 - fv)
                        + (p01[k] * (1.0 - fu) + p11[k] * fu) * fv;
                }
                self.blend(
                    x,
                    y,
                    Color::rgba(c[0] as u8, c[1] as u8, c[2] as u8, 255),
                    c[3] / 255.0 * opacity,
                );
            }
        }
    }

    /// Vektor yo'lni `rect` ga cho'zib to'ldiradi (`view` — yo'l koordinatalari qutisi).
    pub fn fill_path(&mut self, path: &Path, view: f32, rect: Rect, c: Color) {
        if view <= 0.0 || rect.is_empty() {
            return;
        }
        let (bx0, by0, bx1, by1) = self.bounds();
        let (ox, oy) = (rect.x.floor(), rect.y.floor());
        let (w, h) = (rect.w.ceil() as usize + 2, rect.h.ceil() as usize + 2);
        let (sx, sy) = (rect.w / view, rect.h / view);
        let segs: uzcoders_text::raster::Segs = path
            .segs
            .iter()
            .map(|&((x0, y0), (x1, y1))| {
                (
                    (x0 * sx + rect.x - ox, y0 * sy + rect.y - oy),
                    (x1 * sx + rect.x - ox, y1 * sy + rect.y - oy),
                )
            })
            .collect();
        let cov = uzcoders_text::raster::fill(&segs, w, h);
        for row in 0..h {
            let y = oy as i32 + row as i32;
            if y < by0 || y >= by1 {
                continue;
            }
            for col in 0..w {
                let x = ox as i32 + col as i32;
                if x < bx0 || x >= bx1 {
                    continue;
                }
                let v = cov[row * w + col];
                if v != 0 {
                    self.blend(x, y, c, v as f32 / 255.0);
                }
            }
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
                DrawCmd::RoundRectStroke {
                    rect,
                    radius,
                    width,
                    color,
                } => self.stroke_round_rect(*rect, *radius, *width, *color),
                DrawCmd::Text {
                    x,
                    baseline,
                    text,
                    size,
                    color,
                    weight,
                    spacing,
                } => self.draw_text(*x, *baseline, text, *size, *color, *weight, *spacing),
                DrawCmd::Path {
                    path,
                    view,
                    rect,
                    color,
                } => self.fill_path(path, *view, *rect, *color),
                DrawCmd::Image {
                    image,
                    rect,
                    opacity,
                } => self.draw_image(image, *rect, *opacity),
                DrawCmd::Gradient {
                    rect,
                    radius,
                    top,
                    bottom,
                } => self.fill_gradient(*rect, *radius, *top, *bottom),
                DrawCmd::Backdrop {
                    rect,
                    radius,
                    blur,
                    zoom,
                    tint,
                } => self.draw_backdrop(*rect, *radius, *blur, *zoom, *tint),
                DrawCmd::Glass { rect, radius, tint } => {
                    self.fill_round_rect(*rect, *radius, *tint);
                    self.stroke_round_rect(*rect, *radius, 1.5, Color::rgba(255, 255, 255, 90));
                }
                DrawCmd::Glow {
                    cx,
                    cy,
                    radius,
                    color,
                } => self.draw_glow(*cx, *cy, *radius, *color),
                DrawCmd::Arc {
                    cx,
                    cy,
                    radius,
                    width,
                    start,
                    sweep,
                    color,
                } => self.draw_arc(*cx, *cy, *radius, *width, *start, *sweep, *color),
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
                weight: 400,
                spacing: 0.0,
            },
            DrawCmd::PopClip,
        ]);
        let dark_left = (0..20).any(|x| (0..30).any(|y| c.pixel(x, y).r < 100));
        let dark_right = (20..80).any(|x| (0..30).any(|y| c.pixel(x, y).r < 250));
        assert!(dark_left);
        assert!(!dark_right);
    }

    #[test]
    fn backdrop_xiralashtiradi() {
        let mut c = canvas(40, 20);
        c.clear(Color::BLACK);
        // Chap yarmi oq, o'ng yarmi qora; panel o'rtada chegara ustida: chegara xiralashishi kerak.
        c.fill_round_rect(Rect::new(0.0, 0.0, 20.0, 20.0), 0.0, Color::WHITE);
        c.draw_backdrop(
            Rect::new(8.0, 2.0, 24.0, 16.0),
            4.0,
            8.0,
            1.0,
            Color::rgba(0, 0, 0, 0),
        );
        let mid = c.pixel(20, 10).r;
        assert!(mid > 40 && mid < 215, "o'rta piksel xiralashmadi: {mid}");
    }

    #[test]
    fn yol_chiziladi() {
        let mut c = canvas(24, 24);
        c.clear(Color::BLACK);
        let p = Path::parse("M4 4H20V20H4z").unwrap();
        c.fill_path(&p, 24.0, Rect::new(0.0, 0.0, 24.0, 24.0), Color::WHITE);
        assert_eq!(c.pixel(12, 12), Color::WHITE);
        assert_eq!(c.pixel(1, 1), Color::BLACK);
        // 2 baravar katta qutiga cho'ziladi.
        let mut c = canvas(48, 48);
        c.clear(Color::BLACK);
        c.fill_path(&p, 24.0, Rect::new(0.0, 0.0, 48.0, 48.0), Color::WHITE);
        assert_eq!(c.pixel(30, 30), Color::WHITE);
        assert_eq!(c.pixel(44, 44), Color::BLACK);
        assert_eq!(c.pixel(2, 2), Color::BLACK);
    }

    #[test]
    fn qalin_yuz() {
        let mut c = canvas(40, 30);
        c.add_face(700, c.font_rc());
        assert!(c.cache_for(700).len() == 0);
    }

    #[test]
    fn kontur_ichi_bosh() {
        let mut c = canvas(40, 40);
        c.clear(Color::BLACK);
        c.stroke_round_rect(Rect::new(4.0, 4.0, 32.0, 32.0), 6.0, 2.0, Color::WHITE);
        assert_eq!(c.pixel(20, 20), Color::BLACK);
        assert!(c.pixel(5, 20).r > 200);
        assert_eq!(c.pixel(1, 20), Color::BLACK);
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
