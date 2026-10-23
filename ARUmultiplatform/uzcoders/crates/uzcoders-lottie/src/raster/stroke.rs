//! UZ: Chiziq (stroke) ni ko'pburchaklar to'plamiga aylantiradi: har kesma uchun to'rtburchak, burchaklarda xanjar, uchlarda qopqoq.
//! Barcha bo'laklar bir-birini kesmaydigan (burchak xanjarlari tashqi tomonda) qilib tuziladi, shuning uchun ularning qamrovi yig'ilganda qirralar buzilmaydi.
//! EN: Turns a stroke into a set of polygons: a quad per segment, a wedge at each join, a cap at each end.
//! The pieces are built so they do not overlap (join wedges lie on the outer side), so summing their coverage keeps edges clean.
//! RU: Превращает обводку в набор многоугольников: четырёхугольник на каждый отрезок, клин на каждом соединении, окончание на концах.
//! Части строятся без перекрытий (клинья лежат с внешней стороны), поэтому суммирование покрытия не портит края.

use super::path::Contour;
use super::{LineCap, LineJoin, Point};

use std::f32::consts::PI;

type Poly = Vec<Point>;

fn add(a: Point, b: Point) -> Point {
    Point::from_xy(a.x + b.x, a.y + b.y)
}

fn scale(a: Point, k: f32) -> Point {
    Point::from_xy(a.x * k, a.y * k)
}

/// UZ: Yoy bo'yicha burchak qadami (radian), xatolik `tol` dan oshmasligi uchun. EN: Arc step in radians keeping the error below `tol`. RU: Шаг дуги в радианах с погрешностью не больше `tol`.
fn arc_step(h: f32, tol: f32) -> f32 {
    let k = (1.0 - (tol / h.max(1e-6)).min(1.0)).clamp(-1.0, 1.0);
    (2.0 * k.acos()).clamp(0.05, PI / 4.0)
}

/// UZ: Markazi `c`, radiusi `h` yoy: `a0` dan `sweep` gacha, nuqtalar ro'yxatiga qo'shadi. EN: Appends an arc around `c` of radius `h` from `a0` over `sweep`. RU: Добавляет дугу вокруг `c` радиуса `h` от `a0` на `sweep`.
fn arc_points(out: &mut Poly, c: Point, h: f32, a0: f32, sweep: f32, step: f32) {
    let n = ((sweep.abs() / step).ceil() as usize).max(1);
    for i in 0..=n {
        let a = a0 + sweep * (i as f32 / n as f32);
        out.push(Point::from_xy(c.x + h * a.cos(), c.y + h * a.sin()));
    }
}

fn dedup(pts: &[Point], closed: bool) -> Vec<Point> {
    let mut v: Vec<Point> = Vec::with_capacity(pts.len());
    for p in pts {
        if v.last().is_none_or(|q| (q.x - p.x).hypot(q.y - p.y) > 1e-5) {
            v.push(*p);
        }
    }
    if closed && v.len() >= 2 {
        let (a, b) = (v[0], v[v.len() - 1]);
        if (a.x - b.x).hypot(a.y - b.y) <= 1e-5 {
            v.pop();
        }
    }
    v
}

/// UZ: Kontur chiziqlarini ko'pburchaklarga aylantiradi. `tol` — yoy aniqligi (yo'l birliklarida). EN: Converts contour strokes to polygons; `tol` is the arc accuracy in path units. RU: Преобразует обводку контуров в многоугольники; `tol` — точность дуг в единицах контура.
pub(crate) fn stroke_polygons(contours: &[Contour], width: f32, cap: LineCap, join: LineJoin, miter_limit: f32, tol: f32) -> Vec<Poly> {
    let h = width * 0.5;
    let mut polys: Vec<Poly> = Vec::new();
    if !(h > 0.0) || !h.is_finite() {
        return polys;
    }
    let step = arc_step(h, tol);
    for c in contours {
        let pts = dedup(&c.pts, c.closed);
        if pts.is_empty() {
            continue;
        }
        if pts.len() == 1 {
            // UZ: Nol uzunlikli kontur: faqat yumaloq/kvadrat qopqoq nuqta qoldiradi. EN: A zero-length contour leaves a dot only with round/square caps. RU: Контур нулевой длины оставляет точку только при круглом/квадратном окончании.
            let p = pts[0];
            match cap {
                LineCap::Round => {
                    let mut poly = Poly::new();
                    arc_points(&mut poly, p, h, 0.0, 2.0 * PI, step);
                    polys.push(poly);
                }
                LineCap::Square => polys.push(vec![Point::from_xy(p.x - h, p.y - h), Point::from_xy(p.x + h, p.y - h), Point::from_xy(p.x + h, p.y + h), Point::from_xy(p.x - h, p.y + h)]),
                LineCap::Butt => {}
            }
            continue;
        }
        let closed = c.closed && pts.len() >= 3;
        let n = pts.len();
        let seg_count = if closed { n } else { n - 1 };
        // UZ: Kesma yo'nalishi va chap normali. EN: Segment direction and left normal. RU: Направление отрезка и левая нормаль.
        let mut dirs: Vec<(Point, Point)> = Vec::with_capacity(seg_count);
        for i in 0..seg_count {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            let l = (b.x - a.x).hypot(b.y - a.y).max(1e-12);
            let d = Point::from_xy((b.x - a.x) / l, (b.y - a.y) / l);
            dirs.push((d, Point::from_xy(-d.y, d.x)));
        }
        for i in 0..seg_count {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            let nh = scale(dirs[i].1, h);
            polys.push(vec![add(a, nh), add(b, nh), add(b, scale(nh, -1.0)), add(a, scale(nh, -1.0))]);
        }
        // UZ: Burchaklar. EN: Joins. RU: Соединения.
        let join_range = if closed { 0..n } else { 1..n - 1 };
        for v in join_range {
            let prev = if v == 0 { seg_count - 1 } else { v - 1 };
            let next = v % seg_count;
            join_poly(&mut polys, pts[v], dirs[prev], dirs[next], h, join, miter_limit, step);
        }
        if !closed {
            let (d0, n0) = dirs[0];
            let (d1, n1) = dirs[seg_count - 1];
            cap_poly(&mut polys, pts[0], scale(d0, -1.0), n0, h, cap, step, true);
            cap_poly(&mut polys, pts[n - 1], d1, n1, h, cap, step, false);
        }
    }
    polys
}

#[allow(clippy::too_many_arguments)]
fn join_poly(polys: &mut Vec<Poly>, v: Point, a: (Point, Point), b: (Point, Point), h: f32, join: LineJoin, limit: f32, step: f32) {
    let (d1, n1) = a;
    let (d2, n2) = b;
    let cross = d1.x * d2.y - d1.y * d2.x;
    let dot = d1.x * d2.x + d1.y * d2.y;
    if cross.abs() < 1e-6 && dot > 0.0 {
        return;
    }
    // UZ: Tashqi tomon: chapga burilsa — o'ng. EN: Outer side: turning left means right. RU: Внешняя сторона: поворот влево — справа.
    let s = if cross > 0.0 { -1.0 } else { 1.0 };
    let (o1, o2) = (add(v, scale(n1, s * h)), add(v, scale(n2, s * h)));
    match join {
        LineJoin::Round => {
            let (a1, a2) = ((s * n1.y).atan2(s * n1.x), (s * n2.y).atan2(s * n2.x));
            let mut da = a2 - a1;
            while da > PI {
                da -= 2.0 * PI;
            }
            while da < -PI {
                da += 2.0 * PI;
            }
            let mut poly = vec![v];
            arc_points(&mut poly, v, h, a1, da, step);
            polys.push(poly);
        }
        LineJoin::Miter | LineJoin::MiterClip | LineJoin::Bevel => {
            if join != LineJoin::Bevel && dot > -0.999 {
                let ratio = 1.0 / ((1.0 + dot) * 0.5).sqrt();
                if ratio <= limit.max(1.0) {
                    let k = s * h / (1.0 + dot);
                    let tip = add(v, scale(add(n1, n2), k));
                    polys.push(vec![v, o1, tip, o2]);
                    return;
                }
            }
            polys.push(vec![v, o1, o2]);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn cap_poly(polys: &mut Vec<Poly>, p: Point, out_dir: Point, left: Point, h: f32, cap: LineCap, step: f32, is_start: bool) {
    match cap {
        LineCap::Butt => {}
        LineCap::Square => {
            let nh = scale(left, h);
            let ext = scale(out_dir, h);
            polys.push(vec![add(p, nh), add(add(p, nh), ext), add(add(p, scale(nh, -1.0)), ext), add(p, scale(nh, -1.0))]);
        }
        LineCap::Round => {
            // UZ: Yarim doira: oldingi chiziq yo'nalishiga tashqariga. EN: A half disc facing outward. RU: Полукруг наружу.
            let a0 = if is_start { left.y.atan2(left.x) } else { (-left.y).atan2(-left.x) };
            let mut poly = Poly::new();
            arc_points(&mut poly, p, h, a0, PI, step);
            polys.push(poly);
        }
    }
}
