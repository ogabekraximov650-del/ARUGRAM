//! UZ: Piksellar xaritasi (premultiplied RGBA8) va chizish: yo'l to'ldirish/chiziq (qamrov yig'ish usuli), gradientlar, maska, aralashtirish.
//! Qamrov: har bir chiziq kesmasi o'tgan piksellarga imzolangan yuza qo'shadi; qator bo'ylab yig'indi — piksel qamrovi (aniq yuza, qirralar yumshoq).
//! EN: A pixel map (premultiplied RGBA8) and drawing: path fill/stroke (signed-area coverage accumulation), gradients, masks, blending.
//! Coverage: every edge adds signed area to the pixels it crosses; the running sum along a row is the pixel coverage (exact area, smooth edges).
//! RU: Карта пикселей (RGBA8 с предумножением) и рисование: заливка/обводка (накопление знаковой площади), градиенты, маски, смешивание.
//! Покрытие: каждое ребро добавляет знаковую площадь пикселям; накопленная сумма по строке — покрытие пикселя (точная площадь, гладкие края).

use super::path::Path;
use super::stroke::stroke_polygons;
use super::{BlendMode, Color, FillRule, IntRect, Paint, PixmapPaint, Shader, Stroke, Transform};

const MAX_SIDE: u32 = 16_384;
const MAX_BYTES: usize = 512 * 1024 * 1024;

/// UZ: Piksellar xaritasi: har piksel 4 bayt (R, G, B, A), ranglar alfaga ko'paytirilgan.
/// EN: A pixmap: 4 bytes per pixel (R, G, B, A), colors premultiplied by alpha.
/// RU: Карта пикселей: 4 байта на пиксель (R, G, B, A), цвета предумножены на альфу.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pixmap {
    w: u32,
    h: u32,
    data: Vec<u8>,
}

/// UZ: Maska turi. EN: A mask type. RU: Тип маски.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskType {
    /// UZ: Alfa kanali. EN: The alpha channel. RU: Альфа-канал.
    Alpha,
    /// UZ: Yorqinlik (alfaga ko'paytirilgan). EN: Luminance (times alpha). RU: Яркость (умноженная на альфу).
    Luminance,
}

/// UZ: 8 bitli maska (piksel boshiga bayt). EN: An 8-bit mask (one byte per pixel). RU: 8-битная маска (байт на пиксель).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mask {
    w: u32,
    h: u32,
    data: Vec<u8>,
}

impl Mask {
    /// UZ: Pixmapdan maska yasaydi. EN: Builds a mask from a pixmap. RU: Строит маску из pixmap.
    #[must_use]
    pub fn from_pixmap(p: &Pixmap, kind: MaskType) -> Self {
        let data = p
            .data
            .chunks_exact(4)
            .map(|px| match kind {
                MaskType::Alpha => px[3],
                // UZ: Rang alfaga ko'paytirilgan, shuning uchun yorqinlik*alfa to'g'ridan-to'g'ri hisoblanadi. EN: Color is premultiplied, so luminance*alpha is direct. RU: Цвет предумножен, поэтому яркость*альфа считается напрямую.
                MaskType::Luminance => {
                    (0.2126 * f32::from(px[0]) + 0.7152 * f32::from(px[1]) + 0.0722 * f32::from(px[2])).round().clamp(0.0, 255.0) as u8
                }
            })
            .collect();
        Self { w: p.w, h: p.h, data }
    }

    /// UZ: Maskani teskarisiga aylantiradi. EN: Inverts the mask. RU: Инвертирует маску.
    pub fn invert(&mut self) {
        for v in &mut self.data {
            *v = 255 - *v;
        }
    }
}

impl Pixmap {
    /// UZ: Bo'sh (shaffof) xarita; o'lcham 0 yoki juda katta bo'lsa `None`. EN: A transparent pixmap; `None` for zero or huge sizes. RU: Прозрачная карта; `None` при нулевом или огромном размере.
    #[must_use]
    pub fn new(w: u32, h: u32) -> Option<Self> {
        if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE {
            return None;
        }
        let len = (w as usize).checked_mul(h as usize)?.checked_mul(4)?;
        if len > MAX_BYTES {
            return None;
        }
        Some(Self { w, h, data: vec![0; len] })
    }

    /// UZ: Kenglik. EN: Width. RU: Ширина.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.w
    }

    /// UZ: Balandlik. EN: Height. RU: Высота.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.h
    }

    /// UZ: Piksel baytlari (premultiplied RGBA). EN: Pixel bytes (premultiplied RGBA). RU: Байты пикселей (RGBA с предумножением).
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// UZ: O'zgaruvchan piksel baytlari. EN: Mutable pixel bytes. RU: Изменяемые байты пикселей.
    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// UZ: O'zini qaytaradi (mosligi uchun). EN: Returns itself (for API symmetry). RU: Возвращает себя (для единообразия API).
    #[must_use]
    pub const fn as_ref(&self) -> &Self {
        self
    }

    /// UZ: Hammasini bir rang bilan to'ldiradi. EN: Fills everything with one color. RU: Заливает всё одним цветом.
    pub fn fill(&mut self, c: Color) {
        let px = premul_u8(c);
        for p in self.data.chunks_exact_mut(4) {
            p.copy_from_slice(&px);
        }
    }

    /// UZ: To'rtburchak qismini nusxalaydi; chegaradan tashqarida yoki bo'sh bo'lsa `None`. EN: Copies a sub-rectangle; `None` if outside or empty. RU: Копирует прямоугольник; `None`, если вне границ или пусто.
    #[must_use]
    pub fn clone_rect(&self, r: IntRect) -> Option<Self> {
        let (x0, y0) = (i64::from(r.left()), i64::from(r.top()));
        let (x1, y1) = (x0 + i64::from(r.width()), y0 + i64::from(r.height()));
        if x0 < 0 || y0 < 0 || x1 > i64::from(self.w) || y1 > i64::from(self.h) {
            return None;
        }
        let mut out = Self::new(r.width(), r.height())?;
        let rw = r.width() as usize * 4;
        for row in 0..r.height() as usize {
            let s = ((y0 as usize + row) * self.w as usize + x0 as usize) * 4;
            out.data[row * rw..(row + 1) * rw].copy_from_slice(&self.data[s..s + rw]);
        }
        Some(out)
    }

    /// UZ: Maskani qo'llaydi (har piksel maska qiymatiga ko'paytiriladi). EN: Applies a mask (each pixel is multiplied by the mask). RU: Применяет маску (каждый пиксель умножается на маску).
    pub fn apply_mask(&mut self, m: &Mask) {
        if m.w != self.w || m.h != self.h {
            return;
        }
        for (p, k) in self.data.chunks_exact_mut(4).zip(&m.data) {
            let k = u32::from(*k);
            for ch in p.iter_mut() {
                *ch = ((u32::from(*ch) * k + 127) / 255) as u8;
            }
        }
    }

    /// UZ: Yo'lni to'ldiradi. EN: Fills a path. RU: Заливает контур.
    pub fn fill_path(&mut self, path: &Path, paint: &Paint, rule: FillRule, t: Transform, mask: Option<&Mask>) {
        let tol = 0.1 / t.mean_scale();
        let mut polys: Vec<Vec<(f32, f32)>> = Vec::new();
        for c in path.flatten(tol) {
            if c.pts.len() >= 2 || c.closed {
                polys.push(c.pts.iter().map(|p| t.map(p.x, p.y)).collect());
            }
        }
        if let Some(cov) = Coverage::build(&polys, rule, self.w, self.h) {
            self.paint_coverage(&cov, paint, t, mask, 1.0);
        }
    }

    /// UZ: Yo'l chizig'ini chizadi. EN: Strokes a path. RU: Обводит контур.
    pub fn stroke_path(&mut self, path: &Path, paint: &Paint, stroke: &Stroke, t: Transform, mask: Option<&Mask>) {
        let tol = 0.1 / t.mean_scale();
        let contours = match &stroke.dash {
            Some(d) => match path.dash_impl(d, tol) {
                Some(p) => p.flatten(tol),
                None => return,
            },
            None => path.flatten(tol),
        };
        // UZ: Ingichka chiziq (qurilmada <= 1 piksel) 1 piksel qilib chiziladi va qamrov qalinlikka ko'paytiriladi (Skia "hairline" usuli). EN: A thin stroke (<= 1 device pixel) is drawn 1 pixel wide with coverage scaled by the width (the Skia "hairline" approach). RU: Тонкая обводка (<= 1 пикселя) рисуется шириной в 1 пиксель с покрытием, умноженным на толщину (подход «hairline» Skia).
        let (l0, l1) = ((t.sx * stroke.width).hypot(t.ky * stroke.width), (t.kx * stroke.width).hypot(t.sy * stroke.width));
        let wd = (l0 + l1) * 0.5;
        let (width, cov_scale) = if wd > 0.0 && wd < 1.0 { (stroke.width / wd, wd) } else { (stroke.width, 1.0) };
        let polys = stroke_polygons(&contours, width, stroke.line_cap, stroke.line_join, stroke.miter_limit, tol);
        let mut dev: Vec<Vec<(f32, f32)>> = Vec::with_capacity(polys.len());
        for p in &polys {
            let mut v: Vec<(f32, f32)> = p.iter().map(|q| t.map(q.x, q.y)).collect();
            // UZ: Barcha bo'laklar bir xil yo'nalishda bo'lsin (yig'indi birlashma bo'ladi). EN: Make all pieces share one orientation (the sum becomes a union). RU: Все части одной ориентации (сумма даёт объединение).
            if signed_area(&v) < 0.0 {
                v.reverse();
            }
            dev.push(v);
        }
        if let Some(cov) = Coverage::build(&dev, FillRule::Winding, self.w, self.h) {
            self.paint_coverage(&cov, paint, t, mask, cov_scale);
        }
    }

    /// UZ: Boshqa xaritani `(x, y)` ga chizadi (faqat siljish; almashtirish e'tiborga olinmaydi). EN: Draws another pixmap at `(x, y)` (offset only; the transform is ignored). RU: Рисует другую карту в `(x, y)` (только сдвиг; преобразование игнорируется).
    pub fn draw_pixmap(&mut self, x: i32, y: i32, src: &Self, paint: &PixmapPaint, _t: Transform, mask: Option<&Mask>) {
        let op = paint.opacity.clamp(0.0, 1.0);
        for sy in 0..src.h as i64 {
            let dy = i64::from(y) + sy;
            if dy < 0 || dy >= i64::from(self.h) {
                continue;
            }
            for sx in 0..src.w as i64 {
                let dx = i64::from(x) + sx;
                if dx < 0 || dx >= i64::from(self.w) {
                    continue;
                }
                let si = ((sy as usize) * src.w as usize + sx as usize) * 4;
                let s = [
                    f32::from(src.data[si]) / 255.0 * op,
                    f32::from(src.data[si + 1]) / 255.0 * op,
                    f32::from(src.data[si + 2]) / 255.0 * op,
                    f32::from(src.data[si + 3]) / 255.0 * op,
                ];
                let di = ((dy as usize) * self.w as usize + dx as usize) * 4;
                let mut c = 1.0;
                if let Some(m) = mask {
                    c = f32::from(m.data[(dy as usize) * self.w as usize + dx as usize]) / 255.0;
                }
                blend_px(&mut self.data[di..di + 4], s, paint.blend_mode, c);
            }
        }
    }

    fn paint_coverage(&mut self, cov: &Coverage, paint: &Paint, t: Transform, mask: Option<&Mask>, cov_scale: f32) {
        let inv = match &paint.shader {
            Shader::SolidColor(_) => None,
            _ => match t.invert() {
                Some(i) => Some(i),
                None => return,
            },
        };
        let solid = match &paint.shader {
            Shader::SolidColor(c) => Some(premul_f32(*c)),
            _ => None,
        };
        for row in 0..cov.h {
            let y = cov.y0 + row as i32;
            for col in 0..cov.w {
                let c = cov.data[row * cov.w + col];
                if c <= 0.0 {
                    continue;
                }
                let x = cov.x0 + col as i32;
                let mut c = c * cov_scale;
                if let Some(m) = mask {
                    c *= f32::from(m.data[y as usize * self.w as usize + x as usize]) / 255.0;
                    if c <= 0.0 {
                        continue;
                    }
                }
                let s = match solid {
                    Some(s) => s,
                    None => {
                        let (ux, uy) = inv.map_or((0.0, 0.0), |i| i.map(x as f32 + 0.5, y as f32 + 0.5));
                        premul_f32(shade(&paint.shader, ux, uy))
                    }
                };
                let di = (y as usize * self.w as usize + x as usize) * 4;
                blend_px(&mut self.data[di..di + 4], s, paint.blend_mode, c);
            }
        }
    }
}

fn premul_f32(c: Color) -> [f32; 4] {
    let a = c.a.clamp(0.0, 1.0);
    [c.r.clamp(0.0, 1.0) * a, c.g.clamp(0.0, 1.0) * a, c.b.clamp(0.0, 1.0) * a, a]
}

fn premul_u8(c: Color) -> [u8; 4] {
    let p = premul_f32(c);
    [(p[0] * 255.0).round() as u8, (p[1] * 255.0).round() as u8, (p[2] * 255.0).round() as u8, (p[3] * 255.0).round() as u8]
}

/// UZ: Bitta pikselni aralashtiradi: `s` — manba (premultiplied, 0..1), `c` — qamrov. EN: Blends one pixel: `s` is the source (premultiplied, 0..1), `c` the coverage. RU: Смешивает пиксель: `s` — источник (предумножен, 0..1), `c` — покрытие.
fn blend_px(d: &mut [u8], s: [f32; 4], mode: BlendMode, c: f32) {
    let dst = [f32::from(d[0]) / 255.0, f32::from(d[1]) / 255.0, f32::from(d[2]) / 255.0, f32::from(d[3]) / 255.0];
    let mut out = [0.0f32; 4];
    for i in 0..4 {
        let b = match mode {
            BlendMode::SourceOver => s[i] + dst[i] * (1.0 - s[3]),
            BlendMode::SourceIn => s[i] * dst[3],
            BlendMode::DestinationOut => dst[i] * (1.0 - s[3]),
        };
        out[i] = dst[i] + (b - dst[i]) * c;
    }
    // UZ: Premultiplied bo'lgani uchun rang alfadan oshmasligi kerak. EN: Premultiplied color must not exceed alpha. RU: Предумноженный цвет не должен превышать альфу.
    let a = out[3].clamp(0.0, 1.0);
    for i in 0..3 {
        d[i] = (out[i].clamp(0.0, a) * 255.0 + 0.5) as u8;
    }
    d[3] = (a * 255.0 + 0.5) as u8;
}

fn stop_color(stops: &[super::GradientStop], t: f32) -> Color {
    let first = &stops[0];
    if t <= first.pos {
        return first.color;
    }
    for w in stops.windows(2) {
        if t <= w[1].pos {
            let span = (w[1].pos - w[0].pos).max(1e-9);
            let k = ((t - w[0].pos) / span).clamp(0.0, 1.0);
            let (a, b) = (w[0].color, w[1].color);
            return Color { r: a.r + (b.r - a.r) * k, g: a.g + (b.g - a.g) * k, b: a.b + (b.b - a.b) * k, a: a.a + (b.a - a.a) * k };
        }
    }
    stops[stops.len() - 1].color
}

/// UZ: Gradient rangi (foydalanuvchi fazosidagi nuqtada). EN: The gradient color at a point in user space. RU: Цвет градиента в точке пользовательского пространства.
fn shade(shader: &Shader, x: f32, y: f32) -> Color {
    match shader {
        Shader::SolidColor(c) => *c,
        Shader::LinearGradient(g) => {
            let (dx, dy) = (g.end.x - g.start.x, g.end.y - g.start.y);
            let l2 = dx * dx + dy * dy;
            let t = if l2 <= 1e-12 { 0.0 } else { (((x - g.start.x) * dx + (y - g.start.y) * dy) / l2).clamp(0.0, 1.0) };
            stop_color(&g.stops, t)
        }
        Shader::RadialGradient(g) => {
            // UZ: Fokusdan boshlanadigan nol radiusli doiradan (markaz, radius) doiraga o'tuvchi oila: |p - (f + t(c-f))| = t*r. EN: A family from a zero-radius circle at the focal point to the (center, radius) circle. RU: Семейство от окружности нулевого радиуса в фокусе до окружности (центр, радиус).
            let (cdx, cdy) = (g.center.x - g.focal.x, g.center.y - g.focal.y);
            let (px, py) = (x - g.focal.x, y - g.focal.y);
            let a = cdx * cdx + cdy * cdy - g.radius * g.radius;
            let b = px * cdx + py * cdy;
            let c0 = px * px + py * py;
            let t = if a.abs() < 1e-9 {
                if b > 1e-9 { c0 / (2.0 * b) } else { 0.0 }
            } else {
                let disc = (b * b - a * c0).max(0.0);
                let sq = disc.sqrt();
                let (t1, t2) = ((b - sq) / a, (b + sq) / a);
                let best = [t1, t2].into_iter().filter(|t| t.is_finite() && *t >= 0.0).fold(f32::NAN, f32::max);
                if best.is_nan() { 0.0 } else { best }
            };
            stop_color(&g.stops, t.clamp(0.0, 1.0))
        }
    }
}

fn signed_area(p: &[(f32, f32)]) -> f32 {
    let mut s = 0.0;
    for i in 0..p.len() {
        let (a, b) = (p[i], p[(i + 1) % p.len()]);
        s += a.0 * b.1 - b.0 * a.1;
    }
    s * 0.5
}

/// UZ: Qamrov: `(x0, y0)` dan boshlanuvchi `w`×`h` to'rtburchakda har piksel uchun 0..1. EN: Coverage: 0..1 per pixel in a `w`×`h` rectangle at `(x0, y0)`. RU: Покрытие: 0..1 на пиксель в прямоугольнике `w`×`h` с началом `(x0, y0)`.
struct Coverage {
    x0: i32,
    y0: i32,
    w: usize,
    h: usize,
    data: Vec<f32>,
}

impl Coverage {
    fn build(polys: &[Vec<(f32, f32)>], rule: FillRule, cw: u32, ch: u32) -> Option<Self> {
        let (mut minx, mut miny, mut maxx, mut maxy) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for p in polys {
            for &(x, y) in p {
                if !x.is_finite() || !y.is_finite() {
                    return None;
                }
                minx = minx.min(x);
                miny = miny.min(y);
                maxx = maxx.max(x);
                maxy = maxy.max(y);
            }
        }
        if minx > maxx {
            return None;
        }
        let x0 = (minx.floor() as i64).clamp(0, i64::from(cw));
        let y0 = (miny.floor() as i64).clamp(0, i64::from(ch));
        let x1 = (maxx.ceil() as i64).clamp(0, i64::from(cw));
        let y1 = (maxy.ceil() as i64).clamp(0, i64::from(ch));
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        let (w, h) = ((x1 - x0) as usize, (y1 - y0) as usize);
        let stride = w + 2;
        let mut acc = vec![0.0f32; stride * h];
        let (ox, oy) = (x0 as f32, y0 as f32);
        for p in polys {
            let n = p.len();
            if n < 2 {
                continue;
            }
            for i in 0..n {
                let (a, b) = (p[i], p[(i + 1) % n]);
                edge(&mut acc, stride, w as f32, h as f32, (a.0 - ox, a.1 - oy), (b.0 - ox, b.1 - oy));
            }
        }
        let mut data = vec![0.0f32; w * h];
        for row in 0..h {
            let mut sum = 0.0f32;
            for col in 0..w {
                sum += acc[row * stride + col];
                let t = sum.abs();
                data[row * w + col] = match rule {
                    FillRule::Winding => t.min(1.0),
                    FillRule::EvenOdd => {
                        let m = t % 2.0;
                        if m > 1.0 { 2.0 - m } else { m }
                    }
                };
            }
        }
        Some(Self { x0: x0 as i32, y0: y0 as i32, w, h, data })
    }
}

/// UZ: Bitta qirrani qamrov buferiga qo'shadi: avval y bo'yicha kesadi, keyin x=0 va x=w chegaralarida bo'ladi (tashqaridagi qism vertikal chiziqqa aylanadi).
/// EN: Adds one edge to the coverage buffer: clipped in y, then split at x=0 and x=w (parts outside become vertical lines).
/// RU: Добавляет ребро в буфер покрытия: отсекает по y, затем делит по x=0 и x=w (внешние части становятся вертикальными).
fn edge(acc: &mut [f32], stride: usize, w: f32, h: f32, p0: (f32, f32), p1: (f32, f32)) {
    if p0.1 == p1.1 {
        return;
    }
    let (dir, a, b) = if p0.1 < p1.1 { (1.0f32, p0, p1) } else { (-1.0f32, p1, p0) };
    if b.1 <= 0.0 || a.1 >= h {
        return;
    }
    let dxdy = (b.0 - a.0) / (b.1 - a.1);
    let (mut ax, mut ay, mut bx, mut by) = (a.0, a.1, b.0, b.1);
    if ay < 0.0 {
        ax -= ay * dxdy;
        ay = 0.0;
    }
    if by > h {
        bx -= (by - h) * dxdy;
        by = h;
    }
    // UZ: x bo'yicha bo'lish nuqtalari (parametr). EN: Split parameters along x. RU: Параметры деления по x.
    let mut ts: Vec<f32> = vec![0.0, 1.0];
    if (ax - bx).abs() > 1e-9 {
        for bound in [0.0, w] {
            let t = (bound - ax) / (bx - ax);
            if t > 0.0 && t < 1.0 {
                ts.push(t);
            }
        }
    }
    ts.sort_by(f32::total_cmp);
    for k in 0..ts.len() - 1 {
        let (t0, t1) = (ts[k], ts[k + 1]);
        let (mut x0, y0) = (ax + (bx - ax) * t0, ay + (by - ay) * t0);
        let (mut x1, y1) = (ax + (bx - ax) * t1, ay + (by - ay) * t1);
        let mid = (x0 + x1) * 0.5;
        if mid <= 0.0 {
            x0 = 0.0;
            x1 = 0.0;
        } else if mid >= w {
            x0 = w;
            x1 = w;
        } else {
            x0 = x0.clamp(0.0, w);
            x1 = x1.clamp(0.0, w);
        }
        accumulate(acc, stride, h, dir, (x0, y0), (x1, y1));
    }
}

/// UZ: Imzolangan yuza yig'ish (y0 < y1, x chegaraga sig'adi). EN: Signed-area accumulation (y0 < y1, x within bounds). RU: Накопление знаковой площади (y0 < y1, x в границах).
fn accumulate(a: &mut [f32], stride: usize, h: f32, dir: f32, p0: (f32, f32), p1: (f32, f32)) {
    if p1.1 <= p0.1 {
        return;
    }
    let dxdy = (p1.0 - p0.0) / (p1.1 - p0.1);
    let mut x = p0.0;
    let y_start = p0.1.floor().max(0.0) as usize;
    let y_end = (p1.1.ceil() as usize).min(h as usize);
    for y in y_start..y_end {
        let line = y * stride;
        let dy = ((y + 1) as f32).min(p1.1) - (y as f32).max(p0.1);
        if dy <= 0.0 {
            continue;
        }
        let xnext = x + dxdy * dy;
        let d = dy * dir;
        let (x0, x1) = if x < xnext { (x, xnext) } else { (xnext, x) };
        let x0floor = x0.floor();
        let x0i = x0floor as usize;
        let x1ceil = x1.ceil();
        let x1i = x1ceil as usize;
        if x1i <= x0i + 1 {
            let xmf = 0.5 * (x + xnext) - x0floor;
            a[line + x0i] += d - d * xmf;
            a[line + x0i + 1] += d * xmf;
        } else {
            let s = 1.0 / (x1 - x0);
            let x0f = x0 - x0floor;
            let a0 = 0.5 * s * (1.0 - x0f) * (1.0 - x0f);
            let x1f = x1 - x1ceil + 1.0;
            let am = 0.5 * s * x1f * x1f;
            a[line + x0i] += d * a0;
            if x1i == x0i + 2 {
                a[line + x0i + 1] += d * (1.0 - a0 - am);
            } else {
                let a1 = s * (1.5 - x0f);
                a[line + x0i + 1] += d * (a1 - a0);
                for xi in x0i + 2..x1i - 1 {
                    a[line + xi] += d * s;
                }
                let a2 = a1 + (x1i - x0i - 3) as f32 * s;
                a[line + x1i - 1] += d * (1.0 - a2 - am);
            }
            a[line + x1i] += d * am;
        }
        x = xnext;
    }
}
