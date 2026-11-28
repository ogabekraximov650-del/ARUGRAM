//! HUD (ekran ustidagi interfeys) cho'qqilarini yig'ish: to'rtburchak, yumaloq plitalar, doira, matn, blok belgilari.
use crate::blocks;
use crate::text::Atlas;

pub type Col = [u8; 4];

pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Col {
    [r, g, b, a]
}

pub const WHITE: Col = rgba(255, 255, 255, 255);
pub const BLACK: Col = rgba(0, 0, 0, 255);
/// Imzo rangi (ARU: to'q sariq).
pub const SIG: Col = rgba(226, 98, 15, 255);

pub fn with_alpha(c: Col, a: u8) -> Col {
    [c[0], c[1], c[2], a]
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct HV {
    pub x: f32,
    pub y: f32,
    pub u: f32,
    pub v: f32,
    pub c: [u8; 4],
}

pub struct Hud<'a> {
    pub v: Vec<HV>,
    at: &'a Atlas,
}

type P = (f32, f32);

/// Yumaloq to'rtburchak konturi nuqtalari (soat strelkasi bo'yicha).
pub fn rr_points(x: f32, y: f32, w: f32, h: f32, r: f32, seg: usize) -> Vec<P> {
    let r = r.clamp(0.0, w.min(h) * 0.5);
    let mut p = Vec::new();
    let corners = [
        (x + r, y + r, 180.0f32),
        (x + w - r, y + r, 270.0),
        (x + w - r, y + h - r, 0.0),
        (x + r, y + h - r, 90.0),
    ];
    for (cx, cy, a0) in corners {
        for i in 0..=seg {
            let a = (a0 + 90.0 * i as f32 / seg as f32).to_radians();
            p.push((cx + r * a.cos(), cy + r * a.sin()));
        }
    }
    p
}

impl<'a> Hud<'a> {
    pub fn new(at: &'a Atlas) -> Hud<'a> {
        Hud {
            v: Vec::with_capacity(4096),
            at,
        }
    }

    fn push(&mut self, p: P, uv: P, c: Col) {
        self.v.push(HV {
            x: p.0,
            y: p.1,
            u: uv.0,
            v: uv.1,
            c,
        });
    }

    pub fn tri(&mut self, p: [P; 3], c: Col) {
        let uv = self.at.white_uv;
        for q in p {
            self.push(q, uv, c);
        }
    }

    pub fn quad(&mut self, p: [P; 4], c: Col) {
        self.tri([p[0], p[1], p[2]], c);
        self.tri([p[0], p[2], p[3]], c);
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, c: Col) {
        self.quad([(x, y), (x + w, y), (x + w, y + h), (x, y + h)], c);
    }

    pub fn rrect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, c: Col) {
        let pts = rr_points(x, y, w, h, r, 6);
        let m = (x + w * 0.5, y + h * 0.5);
        for i in 0..pts.len() {
            self.tri([m, pts[i], pts[(i + 1) % pts.len()]], c);
        }
    }

    /// Yumaloq to'rtburchak kontur chizig'i (ichkariga qalinligi `th`).
    pub fn rrect_line(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, th: f32, c: Col) {
        let o = rr_points(x, y, w, h, r, 6);
        let i = rr_points(x + th, y + th, w - 2.0 * th, h - 2.0 * th, (r - th).max(0.0), 6);
        for k in 0..o.len() {
            let n = (k + 1) % o.len();
            self.quad([o[k], o[n], i[n], i[k]], c);
        }
    }

    pub fn circle(&mut self, cx: f32, cy: f32, r: f32, c: Col) {
        let n = 36;
        for k in 0..n {
            let (a0, a1) = (k as f32 / n as f32 * std::f32::consts::TAU, (k + 1) as f32 / n as f32 * std::f32::consts::TAU);
            self.tri([(cx, cy), (cx + r * a0.cos(), cy + r * a0.sin()), (cx + r * a1.cos(), cy + r * a1.sin())], c);
        }
    }

    pub fn ring(&mut self, cx: f32, cy: f32, r: f32, th: f32, c: Col) {
        let n = 48;
        for k in 0..n {
            let (a0, a1) = (k as f32 / n as f32 * std::f32::consts::TAU, (k + 1) as f32 / n as f32 * std::f32::consts::TAU);
            let (ri, ro) = (r - th, r);
            self.quad(
                [
                    (cx + ro * a0.cos(), cy + ro * a0.sin()),
                    (cx + ro * a1.cos(), cy + ro * a1.sin()),
                    (cx + ri * a1.cos(), cy + ri * a1.sin()),
                    (cx + ri * a0.cos(), cy + ri * a0.sin()),
                ],
                c,
            );
        }
    }

    /// Matn: `y` — qator tepasi, `px` — em balandligi. Kenglikni qaytaradi.
    pub fn text(&mut self, x: f32, y: f32, px: f32, s: &str, c: Col) -> f32 {
        let k = px / self.at.size;
        let base = y + self.at.ascent * k;
        let mut pen = x;
        for ch in s.chars() {
            let Some(g) = self.at.glyph(ch).copied() else { continue };
            if g.w > 0 {
                let (x0, y0) = (pen + g.left as f32 * k, base - g.top as f32 * k);
                let (x1, y1) = (x0 + g.w as f32 * k, y0 + g.h as f32 * k);
                let (aw, ah) = (self.at.w as f32, self.at.h as f32);
                let (u0, v0) = (g.x as f32 / aw, g.y as f32 / ah);
                let (u1, v1) = ((g.x + g.w) as f32 / aw, (g.y + g.h) as f32 / ah);
                self.push((x0, y0), (u0, v0), c);
                self.push((x1, y0), (u1, v0), c);
                self.push((x1, y1), (u1, v1), c);
                self.push((x0, y0), (u0, v0), c);
                self.push((x1, y1), (u1, v1), c);
                self.push((x0, y1), (u0, v1), c);
            }
            pen += g.adv * k;
        }
        pen - x
    }

    /// Markazlashtirilgan matn (cx — gorizontal markaz, cy — vertikal markaz).
    pub fn text_c(&mut self, cx: f32, cy: f32, px: f32, s: &str, c: Col) {
        let w = self.at.width(s, px);
        self.text(cx - w * 0.5, cy - px * 0.5, px, s, c);
    }

    pub fn text_width(&self, s: &str, px: f32) -> f32 {
        self.at.width(s, px)
    }

    /// Izometrik blok belgisi: markaz (cx,cy), yarim balandlik `s`.
    pub fn cube_icon(&mut self, cx: f32, cy: f32, s: f32, block: u8) {
        let k = 0.866 * s;
        let mk = |face: u8, f: f32| -> Col {
            let c = blocks::color(block, face);
            [(c[0] * f * 255.0) as u8, (c[1] * f * 255.0) as u8, (c[2] * f * 255.0) as u8, 255]
        };
        self.quad([(cx, cy - s), (cx + k, cy - 0.5 * s), (cx, cy), (cx - k, cy - 0.5 * s)], mk(0, 1.0));
        self.quad([(cx - k, cy - 0.5 * s), (cx, cy), (cx, cy + s), (cx - k, cy + 0.5 * s)], mk(1, 0.78));
        self.quad([(cx, cy), (cx + k, cy - 0.5 * s), (cx + k, cy + 0.5 * s), (cx, cy + s)], mk(1, 0.58));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atlas() -> Atlas {
        Atlas::build(include_bytes!("../assets/font.ttf"), 48.0).unwrap()
    }

    #[test]
    fn shakllar_cho_qqi_soni() {
        let a = atlas();
        let mut h = Hud::new(&a);
        h.rect(0.0, 0.0, 10.0, 10.0, WHITE);
        assert_eq!(h.v.len(), 6);
        h.tri([(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)], WHITE);
        assert_eq!(h.v.len(), 9);
    }

    #[test]
    fn yumaloq_plita_chegarada() {
        let a = atlas();
        let mut h = Hud::new(&a);
        h.rrect(100.0, 50.0, 200.0, 80.0, 30.0, BLACK);
        assert!(h.v.len() > 50);
        for v in &h.v {
            assert!(v.x >= 99.9 && v.x <= 300.1 && v.y >= 49.9 && v.y <= 130.1);
        }
        let mut h2 = Hud::new(&a);
        h2.rrect_line(0.0, 0.0, 100.0, 100.0, 20.0, 4.0, WHITE);
        assert!(h2.v.len() > 50);
    }

    #[test]
    fn matn_kengligi_va_belgilar() {
        let a = atlas();
        let mut h = Hud::new(&a);
        let w = h.text(10.0, 10.0, 40.0, "ARU 123", WHITE);
        assert!((w - h.text_width("ARU 123", 40.0)).abs() < 1e-3);
        assert!(h.v.len() >= 6 * 5, "harflar chizilmadi: {}", h.v.len());
        assert!(h.v.len() % 6 == 0);
        let before = h.v.len();
        h.text(0.0, 0.0, 40.0, "   ", WHITE);
        assert_eq!(h.v.len(), before, "probel chizilmaydi");
    }

    #[test]
    fn blok_belgisi_uch_yuz() {
        let a = atlas();
        let mut h = Hud::new(&a);
        h.cube_icon(50.0, 50.0, 20.0, blocks::STONE);
        assert_eq!(h.v.len(), 18);
    }
}
