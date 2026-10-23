//! UZ: Vektor yo'l: kesma, kvadratik va kubik egri chiziqlar; yo'lni tekislash va punktirga bo'lish.
//! EN: A vector path: lines, quadratic and cubic curves; flattening and dashing.
//! RU: Векторный контур: отрезки, квадратичные и кубические кривые; спрямление и разбиение на штрихи.

use super::{Point, Rect, StrokeDash};

/// UZ: Yo'l bo'lagi. EN: A path segment. RU: Сегмент контура.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathSegment {
    /// UZ: Yangi kontur boshlanishi. EN: Starts a contour. RU: Начало контура.
    MoveTo(Point),
    /// UZ: To'g'ri chiziq. EN: A line. RU: Прямая.
    LineTo(Point),
    /// UZ: Kvadratik egri chiziq (boshqaruv, oxirgi nuqta). EN: A quadratic curve (control, end). RU: Квадратичная кривая (управляющая, конец).
    QuadTo(Point, Point),
    /// UZ: Kubik egri chiziq (ikki boshqaruv, oxirgi nuqta). EN: A cubic curve (two controls, end). RU: Кубическая кривая (две управляющие, конец).
    CubicTo(Point, Point, Point),
    /// UZ: Konturni yopadi. EN: Closes the contour. RU: Замыкает контур.
    Close,
}

/// UZ: Tekislangan kontur: nuqtalar va yopiqligi. EN: A flattened contour: points and whether it is closed. RU: Спрямлённый контур: точки и признак замкнутости.
#[derive(Debug, Clone)]
pub(crate) struct Contour {
    pub(crate) pts: Vec<Point>,
    pub(crate) closed: bool,
}

/// UZ: Vektor yo'l. EN: A vector path. RU: Векторный контур.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Path {
    segs: Vec<PathSegment>,
}

impl Path {
    /// UZ: Yo'lda bo'lak yo'qmi. EN: Whether the path has no segments. RU: Нет ли сегментов.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.segs.is_empty()
    }

    /// UZ: Bo'laklar ketma-ketligi. EN: Iterates the segments. RU: Перебор сегментов.
    pub fn segments(&self) -> impl Iterator<Item = PathSegment> + '_ {
        self.segs.iter().copied()
    }

    /// UZ: Egri chiziqlarni `tol` xatolik bilan to'g'ri kesmalarga ajratadi.
    /// EN: Splits curves into straight segments within the error `tol`.
    /// RU: Разбивает кривые на отрезки с погрешностью `tol`.
    pub(crate) fn flatten(&self, tol: f32) -> Vec<Contour> {
        let tol = tol.max(1e-4);
        let mut out: Vec<Contour> = Vec::new();
        let mut cur: Vec<Point> = Vec::new();
        let mut start = Point::default();
        let finish = |cur: &mut Vec<Point>, closed: bool, out: &mut Vec<Contour>| {
            if cur.len() >= 2 || (closed && !cur.is_empty()) {
                out.push(Contour { pts: std::mem::take(cur), closed });
            } else {
                cur.clear();
            }
        };
        for s in &self.segs {
            match *s {
                PathSegment::MoveTo(p) => {
                    finish(&mut cur, false, &mut out);
                    cur.push(p);
                    start = p;
                }
                PathSegment::LineTo(p) => {
                    if cur.is_empty() {
                        cur.push(start);
                    }
                    cur.push(p);
                }
                PathSegment::QuadTo(c, p) => {
                    if cur.is_empty() {
                        cur.push(start);
                    }
                    let p0 = *cur.last().unwrap_or(&start);
                    let dd = ((p0.x - 2.0 * c.x + p.x).hypot(p0.y - 2.0 * c.y + p.y)).max(0.0);
                    let n = ((dd / (4.0 * tol)).sqrt().ceil() as usize).clamp(1, 256);
                    for i in 1..=n {
                        let t = i as f32 / n as f32;
                        let u = 1.0 - t;
                        cur.push(Point::from_xy(u * u * p0.x + 2.0 * u * t * c.x + t * t * p.x, u * u * p0.y + 2.0 * u * t * c.y + t * t * p.y));
                    }
                }
                PathSegment::CubicTo(c1, c2, p) => {
                    if cur.is_empty() {
                        cur.push(start);
                    }
                    let p0 = *cur.last().unwrap_or(&start);
                    let d1 = (p0.x - 2.0 * c1.x + c2.x).hypot(p0.y - 2.0 * c1.y + c2.y);
                    let d2 = (c1.x - 2.0 * c2.x + p.x).hypot(c1.y - 2.0 * c2.y + p.y);
                    let dd = d1.max(d2);
                    let n = ((0.75 * dd / tol).sqrt().ceil() as usize).clamp(1, 256);
                    for i in 1..=n {
                        let t = i as f32 / n as f32;
                        let u = 1.0 - t;
                        let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                        cur.push(Point::from_xy(a * p0.x + b * c1.x + c * c2.x + d * p.x, a * p0.y + b * c1.y + c * c2.y + d * p.y));
                    }
                }
                PathSegment::Close => {
                    finish(&mut cur, true, &mut out);
                    // UZ: Yopilgandan keyin keyingi chiziq shu kontur boshidan davom etadi. EN: After a close the next segment continues from the contour start. RU: После замыкания следующий сегмент продолжается от начала контура.
                    cur.push(start);
                }
            }
        }
        // UZ: Oxirgi ochiq kontur. EN: The last open contour. RU: Последний открытый контур.
        if cur.len() >= 2 {
            out.push(Contour { pts: cur, closed: false });
        }
        out
    }

    /// UZ: Yo'lni punktir naqshiga bo'ladi (faqat "yoqilgan" bo'laklar qoladi, to'g'ri kesmalarga tekislangan).
    /// Nol uzunlikli "yoqilgan" bo'lak nuqtaga aylanadi (yumaloq uchli chiziqda ko'rinadi).
    /// EN: Dashes the path (only the "on" pieces remain, flattened into lines).
    /// A zero-length "on" piece becomes a point (visible with a round cap).
    /// RU: Разбивает контур штрихами (остаются только «включённые» участки, спрямлённые в отрезки).
    /// «Включённый» участок нулевой длины превращается в точку (видна при круглом окончании).
    #[must_use]
    pub fn dash(&self, dash: &StrokeDash, _res_scale: f32) -> Option<Path> {
        self.dash_impl(dash, 0.05)
    }

    pub(crate) fn dash_impl(&self, dash: &StrokeDash, tol: f32) -> Option<Path> {
        let arr = &dash.array;
        let total_pattern: f32 = arr.iter().sum();
        if total_pattern <= 0.0 || !total_pattern.is_finite() {
            return None;
        }
        let mut out = PathBuilder::new();
        let mut budget: u32 = 400_000;
        for contour in self.flatten(tol) {
            let mut pts = contour.pts.clone();
            if contour.closed && pts.len() >= 2 && pts.first() != pts.last() {
                pts.push(pts[0]);
            }
            // UZ: Yig'ma uzunlik. EN: Cumulative length. RU: Накопленная длина.
            let mut cum: Vec<f32> = Vec::with_capacity(pts.len());
            let mut acc = 0.0f32;
            cum.push(0.0);
            for w in pts.windows(2) {
                acc += (w[1].x - w[0].x).hypot(w[1].y - w[0].y);
                cum.push(acc);
            }
            let length = acc;
            if length <= 0.0 || !length.is_finite() {
                continue;
            }
            // UZ: Naqshning boshlang'ich holati (siljish). EN: The initial pattern state (offset). RU: Начальное состояние узора (смещение).
            let mut phase = dash.offset.rem_euclid(total_pattern);
            let mut idx = 0usize;
            let mut rem = arr[0];
            for (i, gap) in arr.iter().enumerate() {
                if phase > *gap || (phase == *gap && *gap != 0.0) {
                    phase -= *gap;
                } else {
                    idx = i;
                    rem = *gap - phase;
                    break;
                }
            }
            let mut d = 0.0f32;
            while d < length {
                if budget == 0 {
                    return None;
                }
                budget -= 1;
                if idx % 2 == 0 {
                    push_piece(&mut out, &pts, &cum, d, (d + rem).min(length));
                }
                d += rem;
                idx = (idx + 1) % arr.len();
                rem = arr[idx];
            }
        }
        out.finish()
    }
}

/// UZ: Tekislangan kontur bo'ylab `[a, b]` masofadagi bo'lakni yo'lga qo'shadi. EN: Appends the piece at distance `[a, b]` along a flattened contour. RU: Добавляет участок на расстоянии `[a, b]` вдоль спрямлённого контура.
fn push_piece(out: &mut PathBuilder, pts: &[Point], cum: &[f32], a: f32, b: f32) {
    let at = |d: f32| -> Point {
        let i = match cum.binary_search_by(|v| v.total_cmp(&d)) {
            Ok(i) => return pts[i],
            Err(i) => i.clamp(1, cum.len() - 1),
        };
        let (c0, c1) = (cum[i - 1], cum[i]);
        let k = if c1 > c0 { ((d - c0) / (c1 - c0)).clamp(0.0, 1.0) } else { 0.0 };
        Point::from_xy(pts[i - 1].x + (pts[i].x - pts[i - 1].x) * k, pts[i - 1].y + (pts[i].y - pts[i - 1].y) * k)
    };
    let start = at(a);
    out.move_to(start.x, start.y);
    for (i, c) in cum.iter().enumerate() {
        if *c > a && *c < b {
            out.line_to(pts[i].x, pts[i].y);
        }
    }
    let end = at(b);
    out.line_to(end.x, end.y);
}

/// UZ: Yo'l quruvchi. EN: A path builder. RU: Построитель контура.
#[derive(Debug, Clone, Default)]
pub struct PathBuilder {
    segs: Vec<PathSegment>,
}

impl PathBuilder {
    /// UZ: Bo'sh quruvchi. EN: An empty builder. RU: Пустой построитель.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// UZ: Yangi kontur. EN: Starts a contour. RU: Начинает контур.
    pub fn move_to(&mut self, x: f32, y: f32) {
        self.segs.push(PathSegment::MoveTo(Point::from_xy(x, y)));
    }

    /// UZ: To'g'ri chiziq. EN: A line. RU: Прямая.
    pub fn line_to(&mut self, x: f32, y: f32) {
        self.segs.push(PathSegment::LineTo(Point::from_xy(x, y)));
    }

    /// UZ: Kvadratik egri chiziq. EN: A quadratic curve. RU: Квадратичная кривая.
    pub fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.segs.push(PathSegment::QuadTo(Point::from_xy(x1, y1), Point::from_xy(x, y)));
    }

    /// UZ: Kubik egri chiziq. EN: A cubic curve. RU: Кубическая кривая.
    pub fn cubic_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.segs.push(PathSegment::CubicTo(Point::from_xy(x1, y1), Point::from_xy(x2, y2), Point::from_xy(x, y)));
    }

    /// UZ: Konturni yopadi. EN: Closes the contour. RU: Замыкает контур.
    pub fn close(&mut self) {
        if !matches!(self.segs.last(), None | Some(PathSegment::Close)) {
            self.segs.push(PathSegment::Close);
        }
    }

    /// UZ: Boshqa yo'lni qo'shadi. EN: Appends another path. RU: Добавляет другой контур.
    pub fn push_path(&mut self, other: &Path) {
        self.segs.extend_from_slice(&other.segs);
    }

    /// UZ: Tugatadi; chiziq yo'q bo'lsa `None`. EN: Finishes; `None` if nothing was drawn. RU: Завершает; `None`, если ничего не нарисовано.
    #[must_use]
    pub fn finish(self) -> Option<Path> {
        if self.segs.len() <= 1 {
            return None;
        }
        Some(Path { segs: self.segs })
    }

    /// UZ: To'rtburchak yo'li. EN: A rectangle path. RU: Контур прямоугольника.
    #[must_use]
    pub fn from_rect(r: Rect) -> Path {
        Path {
            segs: vec![
                PathSegment::MoveTo(Point::from_xy(r.left(), r.top())),
                PathSegment::LineTo(Point::from_xy(r.right(), r.top())),
                PathSegment::LineTo(Point::from_xy(r.right(), r.bottom())),
                PathSegment::LineTo(Point::from_xy(r.left(), r.bottom())),
                PathSegment::Close,
            ],
        }
    }

    /// UZ: Ellips yo'li: o'ng nuqtadan soat mili bo'yicha (past, chap, tepa). EN: An ellipse path: from the right point clockwise (bottom, left, top). RU: Контур эллипса: от правой точки по часовой (низ, лево, верх).
    #[must_use]
    pub fn from_oval(r: Rect) -> Option<Path> {
        const K: f32 = 0.552_284_75;
        let (cx, cy) = ((r.left() + r.right()) * 0.5, (r.top() + r.bottom()) * 0.5);
        let (rx, ry) = ((r.right() - r.left()) * 0.5, (r.bottom() - r.top()) * 0.5);
        if rx <= 0.0 || ry <= 0.0 {
            return None;
        }
        let mut b = Self::new();
        b.move_to(cx + rx, cy);
        b.cubic_to(cx + rx, cy + K * ry, cx + K * rx, cy + ry, cx, cy + ry);
        b.cubic_to(cx - K * rx, cy + ry, cx - rx, cy + K * ry, cx - rx, cy);
        b.cubic_to(cx - rx, cy - K * ry, cx - K * rx, cy - ry, cx, cy - ry);
        b.cubic_to(cx + K * rx, cy - ry, cx + rx, cy - K * ry, cx + rx, cy);
        b.close();
        b.finish()
    }
}
