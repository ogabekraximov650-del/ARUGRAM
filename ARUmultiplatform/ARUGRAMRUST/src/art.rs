//! ARUGRAM tanishtiruv animatsiyalari: 6 sahifa, vektor chizish (`uzcoders-lottie::raster`), o'zimiz chizgan, ko'k uslub.
//! Har sahifa `LOOP_MS` ichida takrorlanadigan tsikl; ilova uni kadrlar varag'iga (sprite) aylantirib o'ynatadi.

use std::f32::consts::PI;
use uzcoders::prelude::Image;
use uzcoders_lottie::raster::*;

pub const FPS: u32 = 24;
/// Sahifalar tsikl uzunligi (ms).
pub const LOOPS: [f32; 6] = [2400.0, 2400.0, 2400.0, 3600.0, 2400.0, 2400.0];

type Rgba = [u8; 4];
const WHITE: Rgba = [255, 255, 255, 255];
const BLUE: Rgba = [26, 119, 212, 255];

fn solid(c: Rgba) -> Shader {
    Shader::SolidColor(Color::from_rgba8(c[0], c[1], c[2], c[3]))
}
fn paint(s: Shader) -> Paint {
    Paint { shader: s, blend_mode: BlendMode::SourceOver, anti_alias: true }
}
fn alpha(c: Rgba, k: f32) -> Rgba {
    [c[0], c[1], c[2], (f32::from(c[3]) * k.clamp(0.0, 1.0)) as u8]
}
fn ease_out(x: f32) -> f32 {
    1.0 - (1.0 - x.clamp(0.0, 1.0)).powi(3)
}
fn ease_in_out(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    if x < 0.5 { 4.0 * x * x * x } else { 1.0 - (-2.0 * x + 2.0).powi(3) / 2.0 }
}

fn circle(cx: f32, cy: f32, r: f32) -> Path {
    PathBuilder::from_oval(Rect::from_xywh(cx - r, cy - r, 2.0 * r, 2.0 * r).unwrap_or_else(|| Rect::from_xywh(0.0, 0.0, 1.0, 1.0).unwrap())).unwrap_or_default()
}
fn rrect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Path {
    let r = r.min(w / 2.0).min(h / 2.0);
    let k = 0.5523 * r;
    let mut b = PathBuilder::new();
    b.move_to(x + r, y);
    b.line_to(x + w - r, y);
    b.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    b.line_to(x + w, y + h - r);
    b.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    b.line_to(x + r, y + h);
    b.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    b.line_to(x, y + r);
    b.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    b.close();
    b.finish().unwrap_or_default()
}
fn poly(pts: &[(f32, f32)], close: bool) -> Path {
    let mut b = PathBuilder::new();
    for (i, p) in pts.iter().enumerate() {
        if i == 0 { b.move_to(p.0, p.1) } else { b.line_to(p.0, p.1) }
    }
    if close {
        b.close();
    }
    b.finish().unwrap_or_default()
}
fn star() -> Path {
    poly(&[(0.0, -6.0), (1.6, -1.6), (6.0, 0.0), (1.6, 1.6), (0.0, 6.0), (-1.6, 1.6), (-6.0, 0.0), (-1.6, -1.6)], true)
}
fn polyline_len(pts: &[(f32, f32)]) -> f32 {
    pts.windows(2).map(|w| ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt()).sum()
}

/// Chizish maydoni: mantiqiy koordinatalar -110..110, `px` x `px` piksel.
struct Cv {
    pm: Pixmap,
    base: Transform,
}
impl Cv {
    fn new(px: u32) -> Self {
        let s = px as f32 / 220.0;
        Cv { pm: Pixmap::new(px, px).expect("pixmap"), base: Transform::identity().pre_scale(s, s).pre_translate(110.0, 110.0) }
    }
    fn tf(&self, x: f32, y: f32, sc: f32, rot: f32) -> Transform {
        self.base.pre_translate(x, y).pre_rotate(rot).pre_scale(sc, sc)
    }
    fn fill(&mut self, p: &Path, c: Rgba, t: Transform) {
        self.pm.fill_path(p, &paint(solid(c)), FillRule::Winding, t, None);
    }
    fn fill_shader(&mut self, p: &Path, s: Shader, t: Transform) {
        self.pm.fill_path(p, &paint(s), FillRule::Winding, t, None);
    }
    fn stroke(&mut self, p: &Path, c: Rgba, w: f32, dash: Option<(f32, f32, f32)>, t: Transform) {
        let st = Stroke { width: w, miter_limit: 4.0, line_cap: LineCap::Round, line_join: LineJoin::Round, dash: dash.and_then(|(a, b, o)| StrokeDash::new(vec![a, b], o)) };
        self.pm.stroke_path(p, &paint(solid(c)), &st, t, None);
    }
    /// 72 radiusli ko'k doira.
    fn disc(&mut self) {
        let g = LinearGradient::new(Point::from_xy(-72.0, -72.0), Point::from_xy(72.0, 72.0),
            vec![GradientStop::new(0.0, Color::from_rgba8(60, 188, 247, 255)), GradientStop::new(1.0, Color::from_rgba8(22, 115, 208, 255))], SpreadMode::Pad, Transform::identity());
        if let Some(g) = g {
            let t = self.base;
            self.fill_shader(&circle(0.0, 0.0, 72.0), g, t);
        }
    }
    /// Doira ichiga qirqilgan qatlam.
    fn clipped(&mut self, f: impl FnOnce(&mut Cv)) {
        let mut l = Cv { pm: Pixmap::new(self.pm.width(), self.pm.height()).expect("pixmap"), base: self.base };
        f(&mut l);
        let mut m = Pixmap::new(self.pm.width(), self.pm.height()).expect("pixmap");
        let (pa, t) = (paint(solid(WHITE)), self.base);
        m.fill_path(&circle(0.0, 0.0, 72.0), &pa, FillRule::Winding, t, None);
        l.pm.apply_mask(&Mask::from_pixmap(&m, MaskType::Alpha));
        self.pm.draw_pixmap(0, 0, &l.pm, &PixmapPaint::default(), Transform::identity(), None);
    }
    fn image(self) -> Image {
        let (w, h) = (self.pm.width(), self.pm.height());
        let mut rgba = self.pm.data().to_vec();
        for p in rgba.chunks_exact_mut(4) {
            let a = u32::from(p[3]);
            if a > 0 && a < 255 {
                for c in &mut p[..3] {
                    *c = ((u32::from(*c) * 255 + a / 2) / a).min(255) as u8;
                }
            }
        }
        Image { w, h, rgba }
    }
}

fn sparkles(cv: &mut Cv, t: f32, loop_ms: f32, list: &[(f32, f32, f32, f32)]) {
    for &(x, y, s, ph) in list {
        let k = 0.5 + 0.5 * (2.0 * PI * t / loop_ms + ph).sin();
        let tr = cv.tf(x, y, s * (0.4 + 0.8 * k), t / loop_ms * 90.0 + ph * 30.0);
        cv.fill(&star(), alpha([159, 214, 255, 255], 0.35 + 0.65 * k), tr);
    }
}
fn pulse_ring(cv: &mut Cv, t: f32, period: f32, r0: f32, r1: f32, ph: f32, w: f32) {
    let u = ((t + ph) % period) / period;
    let t0 = cv.base;
    cv.stroke(&circle(0.0, 0.0, r0 + (r1 - r0) * ease_out(u)), alpha(WHITE, 0.55 * (1.0 - u)), w * (1.0 - 0.6 * u), None, t0);
}

fn page1(cv: &mut Cv, t: f32) {
    let b = cv.base;
    pulse_ring(cv, t, 2400.0, 72.0, 100.0, 0.0, 3.0);
    pulse_ring(cv, t, 2400.0, 72.0, 100.0, 1200.0, 3.0);
    cv.disc();
    for i in 0..3 {
        let a = (t / 2400.0 * 120.0 + i as f32 * 120.0).to_radians();
        let tr = cv.tf(a.cos() * 92.0, a.sin() * 92.0, 1.0, 0.0);
        cv.fill(&circle(0.0, 0.0, 3.2), alpha(WHITE, 0.7), tr);
    }
    let k = 0.19 * (1.0 + 0.025 * (2.0 * PI * t / 2400.0).sin());
    let tr = b.pre_scale(k, k).pre_translate(-261.0, -152.0);
    for part in aru_logo() {
        cv.pm.fill_path(&part, &paint(solid(WHITE)), FillRule::Winding, tr, None);
    }
}

/// UZ: "ARU" logotipi (branding/aru-logo-square.svg dagi harflar), 522x305 birlikda. EN: the "ARU" logo letters (from branding/aru-logo-square.svg), 522x305 units. RU: буквы логотипа «ARU» (из branding/aru-logo-square.svg), 522x305 единиц.
fn aru_logo() -> Vec<Path> {
    const K: f32 = 0.552_284_75;
    let mut parts = Vec::new();
    let mut b = PathBuilder::new();
    // A
    b.move_to(0.0, 300.0);
    for (x, y) in [(99.0, 0.0), (125.0, 0.0), (224.0, 300.0), (173.454, 300.0), (151.674, 234.0), (72.326, 234.0), (50.546, 300.0)] {
        b.line_to(x, y);
    }
    b.close();
    b.move_to(85.526, 194.0);
    for (x, y) in [(112.0, 113.776), (138.474, 194.0)] {
        b.line_to(x, y);
    }
    b.close();
    parts.push(std::mem::replace(&mut b, PathBuilder::new()).finish().unwrap_or_default());
    // R
    b.move_to(198.0, 0.0);
    b.line_to(264.0, 0.0);
    b.cubic_to(264.0 + 86.0 * K, 0.0, 350.0, 86.0 - 86.0 * K, 350.0, 86.0);
    b.cubic_to(350.0, 86.0 + 86.0 * K, 264.0 + 86.0 * K, 172.0, 264.0, 172.0);
    for (x, y) in [(198.0, 172.0), (198.0, 300.0), (246.0, 300.0), (246.0, 0.0)] {
        b.line_to(x, y);
    }
    b.close();
    b.move_to(246.0, 40.0);
    b.line_to(264.0, 40.0);
    b.cubic_to(264.0 + 38.0 * K, 40.0, 302.0, 86.0 - 46.0 * K, 302.0, 86.0);
    b.cubic_to(302.0, 86.0 + 46.0 * K, 264.0 + 38.0 * K, 132.0, 264.0, 132.0);
    b.line_to(246.0, 132.0);
    b.close();
    parts.push(std::mem::replace(&mut b, PathBuilder::new()).finish().unwrap_or_default());
    b.move_to(276.0, 132.0);
    for (x, y) in [(324.0, 132.0), (394.0, 300.0), (346.0, 300.0)] {
        b.line_to(x, y);
    }
    b.close();
    parts.push(std::mem::replace(&mut b, PathBuilder::new()).finish().unwrap_or_default());
    // U
    b.move_to(350.0, 0.0);
    b.line_to(350.0, 219.0);
    b.cubic_to(350.0, 219.0 + 86.0 * K, 436.0 - 86.0 * K, 305.0, 436.0, 305.0);
    b.cubic_to(436.0 + 86.0 * K, 305.0, 522.0, 219.0 + 86.0 * K, 522.0, 219.0);
    b.line_to(522.0, 0.0);
    b.line_to(474.0, 0.0);
    b.line_to(474.0, 219.0);
    b.cubic_to(474.0, 219.0 + 46.0 * K, 436.0 + 38.0 * K, 265.0, 436.0, 265.0);
    b.cubic_to(436.0 - 38.0 * K, 265.0, 398.0, 219.0 + 46.0 * K, 398.0, 219.0);
    b.line_to(398.0, 0.0);
    b.close();
    parts.push(b.finish().unwrap_or_default());
    parts
}

fn page2(cv: &mut Cv, t: f32) {
    cv.disc();
    let u = (t % 2400.0) / 2400.0;
    let (x, fast) = if u < 0.23 {
        let p = u / 0.23;
        (-130.0 + 130.0 * ease_out(p), 1.0 - p)
    } else if u < 0.78 {
        (0.0, 0.0)
    } else {
        let p = (u - 0.78) / 0.22;
        (140.0 * ease_in_out(p), p)
    };
    let chk = ((u - 0.34) / 0.2).clamp(0.0, 1.0);
    cv.clipped(|l| {
        for (i, y) in [-22.0f32, 0.0, 22.0].iter().enumerate() {
            let len = 8.0 + fast * 52.0;
            let x0 = x - 82.0 - i as f32 * 6.0 - fast * 22.0;
            let tr = l.base;
            l.fill(&rrect(x0, y - 2.0, len, 4.0, 2.0), alpha(WHITE, 0.15 + 0.6 * fast), tr);
        }
        let tr = l.tf(x, 0.0, 1.0, 0.0);
        l.fill(&rrect(-32.0, -21.0, 64.0, 40.0, 13.0), WHITE, tr);
        l.fill(&poly(&[(-22.0, 18.0), (-30.0, 31.0), (-8.0, 19.0)], true), WHITE, tr);
        for (i, w) in [24.0f32, 18.0, 12.0].iter().enumerate() {
            l.fill(&rrect(-18.0 + i as f32 * 0.0, -13.0 + i as f32 * 8.0, *w + 14.0 - i as f32 * 4.0, 5.0, 2.5), alpha([42, 143, 224, 255], 0.55), tr);
        }
        if chk > 0.0 {
            let pts = [(-10.0f32, 8.0f32), (-3.0, 15.0), (11.0, 1.0)];
            let pts2 = [(2.0f32, 8.0f32), (9.0, 15.0), (23.0, 1.0)];
            let tot = 2.0 * polyline_len(&pts);
            let on = tot * chk;
            let first = polyline_len(&pts);
            let p1 = poly(&pts, false);
            cv_dash(l, &p1, on.min(first), tr);
            if on > first {
                cv_dash(l, &poly(&pts2, false), on - first, tr);
            }
        }
    });
    sparkles(cv, t, 2400.0, &[(80.0, -78.0, 0.9, 0.0), (92.0, 60.0, 0.7, 2.0), (-86.0, 66.0, 0.8, 4.0)]);
}
fn cv_dash(l: &mut Cv, p: &Path, on: f32, tr: Transform) {
    l.stroke(p, BLUE, 4.5, Some((on.max(0.01), 500.0, 0.0)), tr);
}

fn page3(cv: &mut Cv, t: f32) {
    cv.disc();
    let fl = (2.0 * PI * t / 2400.0).sin() * 2.4;
    let tr = cv.tf(0.0, -2.0 + fl, 1.0, 0.0);
    for (x, y, r) in [(-22.0, 2.0, 21.0), (2.0, -12.0, 28.0), (25.0, 0.0, 21.0)] {
        cv.fill(&circle(x, y, r), WHITE, tr);
    }
    cv.fill(&rrect(-42.0, 0.0, 88.0, 24.0, 12.0), WHITE, tr);
    let pts: Vec<(f32, f32)> = (0..=80).map(|i| {
        let a = i as f32 / 80.0 * 2.0 * PI;
        let k = 1.0 + a.sin().powi(2);
        (30.0 * a.cos() / k, 30.0 * a.sin() * a.cos() / k)
    }).collect();
    let l = polyline_len(&pts);
    let p = poly(&pts, true);
    let it = cv.tf(0.0, -1.0 + fl, 1.0, 0.0);
    cv.stroke(&p, [212, 233, 251, 255], 6.0, None, it);
    cv.stroke(&p, BLUE, 6.0, Some((0.34 * l, 0.66 * l, (t / 2400.0) * l)), it);
    sparkles(cv, t, 2400.0, &[(76.0, -70.0, 1.0, 0.0), (-88.0, -40.0, 0.8, 1.7), (86.0, 54.0, 0.7, 3.1), (-70.0, 74.0, 0.9, 4.2)]);
}

fn chip(cv: &mut Cv, k: usize, x: f32, y: f32, sc: f32, a: f32) {
    let tr = cv.tf(x, y, sc, (x / 40.0).cos() * 8.0);
    cv.fill(&rrect(-17.0, -17.0, 34.0, 34.0, 9.0), alpha(WHITE, a), tr);
    let g = alpha([42, 143, 224, 255], a);
    match k {
        0 => {
            cv.fill(&poly(&[(-10.0, 9.0), (-3.0, 0.0), (2.0, 6.0), (6.0, 2.0), (14.0, 9.0)], true), g, tr);
            cv.fill(&circle(6.0, -7.0, 3.0), g, tr);
        }
        1 => cv.fill(&poly(&[(-5.0, -9.0), (9.0, 0.0), (-5.0, 9.0)], true), g, tr),
        _ => {
            for (i, y) in [-9.0f32, -2.0, 5.0].iter().enumerate() {
                cv.fill(&rrect(-10.0, y - 1.5, if i == 2 { 12.0 } else { 20.0 }, 3.4, 1.7), g, tr);
            }
        }
    }
}
fn page4(cv: &mut Cv, t: f32) {
    let u = (t % 1800.0) / 1800.0;
    let b = cv.base;
    cv.stroke(&circle(0.0, 0.0, 72.0 + 26.0 * ease_out(u)), alpha(WHITE, 0.55 * (1.0 - u)), 4.0 * (1.0 - 0.6 * u), None, b);
    cv.disc();
    let mut items: Vec<(usize, f32, f32, f32)> = (0..3).map(|k| {
        let a = 2.0 * PI * t / 3600.0 + k as f32 * 2.0 * PI / 3.0;
        (k, a.cos() * 80.0, a.sin() * 30.0 - a.cos() * 12.0, a.sin())
    }).collect();
    items.sort_by(|p, q| p.3.total_cmp(&q.3));
    let draw = |cv: &mut Cv, it: &(usize, f32, f32, f32)| {
        let d = 0.5 + 0.5 * it.3;
        chip(cv, it.0, it.1, it.2, 0.68 + 0.3 * d, 0.6 + 0.4 * d);
    };
    for it in items.iter().filter(|i| i.3 < 0.0) {
        draw(cv, it);
    }
    let s = 1.0 + 0.07 * (2.0 * PI * t / 1800.0).sin();
    let tr = cv.tf(0.0, 0.0, s, 0.0);
    cv.fill(&poly(&[(8.0, -40.0), (-20.0, 6.0), (-3.0, 6.0), (-9.0, 40.0), (22.0, -8.0), (4.0, -8.0)], true), WHITE, tr);
    cv.stroke(&poly(&[(8.0, -40.0), (-20.0, 6.0), (-3.0, 6.0), (-9.0, 40.0), (22.0, -8.0), (4.0, -8.0)], true), WHITE, 3.0, None, tr);
    for it in items.iter().filter(|i| i.3 >= 0.0) {
        draw(cv, it);
    }
}

fn page5(cv: &mut Cv, t: f32) {
    pulse_ring(cv, t, 2400.0, 72.0, 98.0, 0.0, 3.0);
    cv.disc();
    let ang: [f32; 6] = [210.0, 255.0, 300.0, 330.0, 15.0, 165.0];
    // qalqon "urilganda" biroz silkinadi
    let mut hit = 0.0f32;
    for i in 0..6 {
        let u = ((t + i as f32 * 400.0) % 2400.0) / 2400.0;
        if u >= 0.55 {
            let dt = (u - 0.55) * 2400.0;
            hit += (1.0 - dt / 240.0).max(0.0);
        }
    }
    let sc = 1.0 + 0.03 * hit.min(1.5);
    let tr = cv.tf(0.0, 0.0, sc, 0.0);
    let mut sh = PathBuilder::new();
    sh.move_to(0.0, -54.0);
    sh.line_to(42.0, -39.0);
    sh.line_to(42.0, -4.0);
    sh.cubic_to(42.0, 25.0, 23.0, 44.0, 0.0, 55.0);
    sh.cubic_to(-23.0, 44.0, -42.0, 25.0, -42.0, -4.0);
    sh.line_to(-42.0, -39.0);
    sh.close();
    cv.fill(&sh.finish().unwrap_or_default(), WHITE, tr);
    let lt = cv.tf(0.0, 4.0, sc, 0.0);
    let mut sk = PathBuilder::new();
    sk.move_to(-9.0, -2.0);
    sk.line_to(-9.0, -10.0);
    sk.cubic_to(-9.0, -20.5, 9.0, -20.5, 9.0, -10.0);
    sk.line_to(9.0, -2.0);
    cv.stroke(&sk.finish().unwrap_or_default(), BLUE, 5.5, None, lt);
    cv.fill(&rrect(-15.0, -3.0, 30.0, 25.0, 7.0), BLUE, lt);
    cv.fill(&circle(0.0, 7.0, 3.4), WHITE, lt);
    cv.fill(&rrect(-1.6, 8.0, 3.2, 8.0, 1.6), WHITE, lt);
    for i in 0..6 {
        let u = ((t + i as f32 * 400.0) % 2400.0) / 2400.0;
        let (r, op, rad) = if u < 0.55 { (130.0 - 72.0 * (u / 0.55), 1.0, 4.6) } else { let p = (u - 0.55) / 0.45; (58.0 + 42.0 * ease_out(p), 1.0 - p, 4.6 - 1.8 * p) };
        let a = ang[i].to_radians() + if u >= 0.55 { 0.35 * (u - 0.55) * if i % 2 == 1 { 1.0 } else { -1.0 } } else { 0.0 };
        let tr = cv.tf(a.cos() * r, a.sin() * r, 1.0, 0.0);
        cv.fill(&circle(0.0, 0.0, rad), alpha([255, 96, 112, 255], op), tr);
        if (0.55..0.8).contains(&u) {
            let p = (u - 0.55) / 0.25;
            let a0 = ang[i].to_radians();
            let tr = cv.tf(a0.cos() * 56.0, a0.sin() * 56.0, 1.0, 0.0);
            cv.stroke(&circle(0.0, 0.0, 4.0 + 12.0 * ease_out(p)), alpha([255, 224, 228, 255], 0.9 * (1.0 - p)), 3.0, None, tr);
        }
    }
}

fn page6(cv: &mut Cv, t: f32) {
    cv.disc();
    let u = (t % 2400.0) / 2400.0;
    let b = cv.base;
    let off = u * 16.0;
    for s in [-1.0f32, 1.0] {
        let mut p = PathBuilder::new();
        p.move_to(52.0 * s, 34.0);
        p.quad_to(50.0 * s, -6.0, 10.0 * s, -14.0);
        cv.stroke(&p.finish().unwrap_or_default(), alpha(WHITE, 0.7), 2.4, Some((2.0, 6.0, off)), b);
    }
    let flash = |c: f32, w: f32| (1.0 - (u - c).abs() / w).clamp(0.0, 1.0);
    let cp = 1.0 + 0.08 * flash(0.42, 0.08).max(flash(0.5, 0.08));
    let ct = cv.tf(0.0, -20.0 + (2.0 * PI * t / 2400.0).sin() * 1.8, cp, 0.0);
    for (x, y, r) in [(-14.0, 0.0, 17.0), (4.0, -12.0, 23.0), (20.0, 2.0, 19.0)] {
        cv.fill(&circle(x, y, r), WHITE, ct);
    }
    cv.fill(&rrect(-28.0, 0.0, 58.0, 20.0, 10.0), WHITE, ct);
    let ph = cv.tf(-54.0, 40.0, 1.0, 0.0);
    cv.fill(&rrect(-10.0, -17.0, 20.0, 34.0, 5.0), WHITE, ph);
    cv.fill(&rrect(-7.0, -13.0, 14.0, 24.0, 2.5), alpha([42, 143, 224, 255], 0.35 + 0.65 * flash(0.02, 0.1)), ph);
    let lp = cv.tf(54.0, 38.0, 1.0, 0.0);
    cv.fill(&rrect(-22.0, -16.0, 44.0, 28.0, 4.0), WHITE, lp);
    cv.fill(&rrect(-28.0, 13.0, 56.0, 5.0, 2.5), WHITE, lp);
    cv.fill(&rrect(-18.0, -12.0, 36.0, 20.0, 2.0), alpha([42, 143, 224, 255], 0.35 + 0.65 * flash(0.92, 0.1)), lp);
    let q = |x0: f32, y0: f32, cx: f32, cy: f32, x1: f32, y1: f32, p: f32| {
        let (a, b2, c) = ((1.0 - p).powi(2), 2.0 * (1.0 - p) * p, p * p);
        (a * x0 + b2 * cx + c * x1, a * y0 + b2 * cy + c * y1)
    };
    let mut pk = |pos: (f32, f32)| {
        let tr = cv.tf(pos.0, pos.1, 1.0, 0.0);
        cv.fill(&circle(0.0, 0.0, 6.5), BLUE, tr);
        cv.fill(&circle(0.0, 0.0, 4.5), WHITE, tr);
    };
    if u < 0.42 {
        pk(q(-52.0, 34.0, -50.0, -6.0, -10.0, -14.0, ease_in_out((u / 0.4).clamp(0.0, 1.0))));
    }
    if (0.5..0.92).contains(&u) {
        pk(q(10.0, -14.0, 50.0, -6.0, 52.0, 34.0, ease_in_out(((u - 0.5) / 0.4).clamp(0.0, 1.0))));
    }
    sparkles(cv, t, 2400.0, &[(78.0, -56.0, 0.9, 0.0), (-84.0, -52.0, 0.8, 2.2)]);
}

/// Sahifaning `t` ms dagi kadri (`px` x `px`, oddiy alfa).
pub fn render(page: usize, t: f32, px: u32) -> Image {
    let mut cv = Cv::new(px);
    match page {
        0 => page1(&mut cv, t),
        1 => page2(&mut cv, t),
        2 => page3(&mut cv, t),
        3 => page4(&mut cv, t),
        4 => page5(&mut cv, t),
        _ => page6(&mut cv, t),
    }
    cv.image()
}

/// Kadrlar varag'i: (rasm, ustunlar, kadrlar soni).
pub fn sheet(page: usize, px: u32) -> (Image, u32, u32) {
    let n = (LOOPS[page] * FPS as f32 / 1000.0).round() as u32;
    let cols = (4096 / px).min(n).max(1);
    let rows = n.div_ceil(cols);
    let (w, h) = (cols * px, rows * px);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for i in 0..n {
        let f = render(page, i as f32 * 1000.0 / FPS as f32, px);
        let (ox, oy) = ((i % cols) * px, (i / cols) * px);
        for y in 0..px {
            let s = (y * px * 4) as usize;
            let d = (((oy + y) * w + ox) * 4) as usize;
            rgba[d..d + (px * 4) as usize].copy_from_slice(&f.rgba[s..s + (px * 4) as usize]);
        }
    }
    (Image { w, h, rgba }, cols, n)
}

/// Quyosh (`moon = false`) yoki oy belgisi, 34x34 mantiqiy, `px` piksel.
pub fn theme_icon(moon: bool, px: u32) -> Image {
    let s = px as f32 / 34.0;
    let b = Transform::identity().pre_scale(s, s).pre_translate(17.0, 17.0);
    let mut pm = Pixmap::new(px, px).expect("pixmap");
    let c = [42u8, 157, 232, 255];
    if moon {
        pm.fill_path(&circle(0.0, 0.0, 9.5), &paint(solid(c)), FillRule::Winding, b, None);
        let mut m = Pixmap::new(px, px).expect("pixmap");
        m.fill_path(&circle(5.5, -4.5, 8.0), &paint(solid(WHITE)), FillRule::Winding, b, None);
        let mut mask = Mask::from_pixmap(&m, MaskType::Alpha);
        mask.invert();
        pm.apply_mask(&mask);
    } else {
        pm.fill_path(&circle(0.0, 0.0, 6.0), &paint(solid(c)), FillRule::Winding, b, None);
        let st = Stroke { width: 2.4, miter_limit: 4.0, line_cap: LineCap::Round, line_join: LineJoin::Round, dash: None };
        for i in 0..8 {
            let a = i as f32 * PI / 4.0;
            let p = poly(&[(a.cos() * 9.5, a.sin() * 9.5), (a.cos() * 13.5, a.sin() * 13.5)], false);
            pm.stroke_path(&p, &paint(solid(c)), &st, b, None);
        }
    }
    let mut cv = Cv { pm, base: b };
    let img = std::mem::replace(&mut cv, Cv { pm: Pixmap::new(1, 1).expect("pixmap"), base: b }).image();
    img
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn har_sahifa_chiziladi_va_tsikl_yopiq() {
        for p in 0..6 {
            let a = render(p, 600.0, 96);
            let nz = a.rgba.chunks_exact(4).filter(|x| x[3] > 0).count();
            assert!(nz > 96 * 96 / 6, "sahifa {p}: kam piksel {nz}");
            // Tsikl boshi va oxiri deyarli bir xil (uzluksiz takrorlanish).
            let (s, e) = (render(p, 0.0, 64), render(p, LOOPS[p], 64));
            let diff: u64 = s.rgba.iter().zip(&e.rgba).map(|(a, b)| u64::from(a.abs_diff(*b))).sum();
            assert!(diff < 64 * 64 * 4 * 4, "sahifa {p}: tsikl yopilmaydi ({diff})");
        }
    }

    #[test]
    fn varaq_o_lchami_togri() {
        let (img, cols, n) = sheet(0, 64);
        assert_eq!(n, 58);
        assert_eq!(img.w, cols * 64);
        assert!(img.h >= (n / cols) * 64);
        let (_, _, n4) = sheet(3, 32);
        assert_eq!(n4, 86);
    }

    #[test]
    fn mavzu_belgilari() {
        for moon in [false, true] {
            let i = theme_icon(moon, 34);
            assert!(i.rgba.chunks_exact(4).any(|p| p[3] > 200));
        }
    }
}
