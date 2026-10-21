//! UZ: Konturni piksel qoplamasiga (0..255) aylantiruvchi rasterizator.
//! Usul: imzoli maydon yig'indisi — har bir kesma o'zi kesib o'tgan piksellarga maydon hissasini qo'shadi,
//! keyin qatorlar bo'ylab yig'ib chiqiladi. Bu "nolga teng bo'lmagan" (non-zero) to'ldirish qoidasiga mos keladi.
//! EN: A rasterizer that turns an outline into pixel coverage (0..255).
//! Method: signed-area accumulation — each segment adds its area contribution to the pixels it crosses,
//! then the values are accumulated along each row. This matches the non-zero fill rule.
//! RU: Растеризатор, превращающий контур в пиксельное покрытие (0..255).
//! Метод: накопление знаковой площади — каждый отрезок добавляет вклад площади в пересекаемые пиксели,
//! затем значения накапливаются вдоль строк. Это соответствует правилу заливки non-zero.

use crate::ttf::Contour;

/// UZ: Tekis kesmalar ro'yxati (piksel koordinatalarida, y o'qi pastga qaragan).
/// EN: A list of straight segments (in pixel coordinates, y axis pointing down).
/// RU: Список прямых отрезков (в пиксельных координатах, ось y направлена вниз).
pub type Segs = Vec<((f32, f32), (f32, f32))>;

fn quad(p0: (f32, f32), p1: (f32, f32), p2: (f32, f32), out: &mut Segs) {
    let dev = ((p0.0 - 2.0 * p1.0 + p2.0).powi(2) + (p0.1 - 2.0 * p1.1 + p2.1).powi(2)).sqrt();
    let n = ((dev * 1.5).sqrt().ceil() as usize).clamp(1, 32);
    let mut prev = p0;
    for i in 1..=n {
        let t = i as f32 / n as f32;
        let u = 1.0 - t;
        let q = (
            u * u * p0.0 + 2.0 * u * t * p1.0 + t * t * p2.0,
            u * u * p0.1 + 2.0 * u * t * p1.1 + t * t * p2.1,
        );
        out.push((prev, q));
        prev = q;
    }
}

/// UZ: Konturlarni (shrift birliklarida) piksel kesmalariga aylantiradi: `(x*scale - ox, oy - y*scale)`.
/// EN: Converts outlines (in font units) into pixel segments: `(x*scale - ox, oy - y*scale)`.
/// RU: Преобразует контуры (в единицах шрифта) в пиксельные отрезки: `(x*scale - ox, oy - y*scale)`.
pub fn flatten(contours: &[Contour], scale: f32, ox: f32, oy: f32) -> Segs {
    let mut segs = Vec::new();
    let tf = |x: f32, y: f32| (x * scale - ox, oy - y * scale);
    for c in contours {
        let n = c.len();
        if n < 2 {
            continue;
        }
        let pt = |i: usize| tf(c[i].x, c[i].y);
        let mid = |a: (f32, f32), b: (f32, f32)| ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5);
        let (start, first) = if c[0].on {
            (pt(0), 1)
        } else if c[n - 1].on {
            (pt(n - 1), 0)
        } else {
            (mid(pt(0), pt(n - 1)), 0)
        };
        let last = if c[0].on || !c[n - 1].on { n } else { n - 1 };
        let mut cur = start;
        let mut ctrl: Option<(f32, f32)> = None;
        for i in first..last {
            let p = pt(i);
            if c[i].on {
                match ctrl.take() {
                    Some(k) => quad(cur, k, p, &mut segs),
                    None => segs.push((cur, p)),
                }
                cur = p;
            } else {
                if let Some(k) = ctrl {
                    let m = mid(k, p);
                    quad(cur, k, m, &mut segs);
                    cur = m;
                }
                ctrl = Some(p);
            }
        }
        match ctrl {
            Some(k) => quad(cur, k, start, &mut segs),
            None => segs.push((cur, start)),
        }
    }
    segs
}

struct Acc {
    w: usize,
    h: usize,
    a: Vec<f32>,
}

impl Acc {
    fn add(&mut self, row: usize, x: i32, v: f32) {
        if x >= 0 && (x as usize) < self.w {
            self.a[row * self.w + x as usize] += v;
        }
    }

    fn line(&mut self, p0: (f32, f32), p1: (f32, f32)) {
        if p0.1 == p1.1 {
            return;
        }
        let (dir, p0, p1) = if p0.1 < p1.1 {
            (1.0, p0, p1)
        } else {
            (-1.0, p1, p0)
        };
        let dxdy = (p1.0 - p0.0) / (p1.1 - p0.1);
        let mut x = p0.0;
        let mut y0 = 0usize;
        if p0.1 > 0.0 {
            y0 = p0.1 as usize;
        } else {
            x -= p0.1 * dxdy;
        }
        let y1 = (p1.1.ceil().max(0.0) as usize).min(self.h);
        for y in y0..y1 {
            let dy = ((y + 1) as f32).min(p1.1) - (y as f32).max(p0.1);
            let xnext = x + dxdy * dy;
            let d = dy * dir;
            let (xa, xb) = if x < xnext { (x, xnext) } else { (xnext, x) };
            let xaf = xa.floor();
            let xai = xaf as i32;
            let xbc = xb.ceil();
            let xbi = xbc as i32;
            if xbi <= xai + 1 {
                let xmf = 0.5 * (x + xnext) - xaf;
                self.add(y, xai, d - d * xmf);
                self.add(y, xai + 1, d * xmf);
            } else {
                let s = 1.0 / (xb - xa);
                let xa_f = xa - xaf;
                let a0 = 0.5 * s * (1.0 - xa_f) * (1.0 - xa_f);
                let xb_f = xb - xbc + 1.0;
                let am = 0.5 * s * xb_f * xb_f;
                self.add(y, xai, d * a0);
                if xbi == xai + 2 {
                    self.add(y, xai + 1, d * (1.0 - a0 - am));
                } else {
                    let a1 = s * (1.5 - xa_f);
                    self.add(y, xai + 1, d * (a1 - a0));
                    for xi in xai + 2..xbi - 1 {
                        self.add(y, xi, d * s);
                    }
                    let a2 = a1 + (xbi - xai - 3) as f32 * s;
                    self.add(y, xbi - 1, d * (1.0 - a2 - am));
                }
                self.add(y, xbi, d * am);
            }
            x = xnext;
        }
    }
}

/// UZ: Kesmalarni `w x h` o'lchamli qoplama xaritasiga (0..255) aylantiradi.
/// EN: Turns segments into a `w x h` coverage map (0..255).
/// RU: Превращает отрезки в карту покрытия размером `w x h` (0..255).
pub fn fill(segs: &Segs, w: usize, h: usize) -> Vec<u8> {
    let mut acc = Acc {
        w,
        h,
        a: vec![0.0; w * h],
    };
    for &(p0, p1) in segs {
        acc.line(p0, p1);
    }
    let mut out = vec![0u8; w * h];
    for y in 0..h {
        let mut sum = 0.0f32;
        for x in 0..w {
            sum += acc.a[y * w + x];
            out[y * w + x] = (sum.abs().min(1.0) * 255.0 + 0.5) as u8;
        }
    }
    out
}
