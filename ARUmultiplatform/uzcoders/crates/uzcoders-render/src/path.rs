//! UZ: Vektor yo'l (SVG `d` sintaksisi) ikonkalar uchun. Buyruqlar: M, L, H, V, C, S, Q, T, A, Z va ularning nisbiy (kichik harfli) shakllari.
//! EN: Vector paths (SVG `d` syntax) for icons. Commands: M, L, H, V, C, S, Q, T, A, Z and their relative (lowercase) forms.
//! RU: Векторные контуры (синтаксис SVG `d`) для иконок. Команды: M, L, H, V, C, S, Q, T, A, Z и их относительные (строчные) формы.

pub type Seg = ((f32, f32), (f32, f32));

/// UZ: Tekislangan yo'l: to'g'ri kesmalar ro'yxati (ko'rinish-qutisi birliklarida).
/// EN: A flattened path: a list of straight segments (in view-box units).
/// RU: Спрямлённый контур: список прямых отрезков (в единицах view-box).
#[derive(Clone, Debug, Default)]
pub struct Path {
    pub segs: Vec<Seg>,
}

struct Lexer<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Lexer<'a> {
    fn skip(&mut self) {
        while self.i < self.b.len()
            && (self.b[self.i] == b' '
                || self.b[self.i] == b','
                || self.b[self.i] == b'\n'
                || self.b[self.i] == b'\t'
                || self.b[self.i] == b'\r')
        {
            self.i += 1;
        }
    }

    fn cmd(&mut self) -> Option<u8> {
        self.skip();
        match self.b.get(self.i) {
            Some(c) if c.is_ascii_alphabetic() => {
                self.i += 1;
                Some(*c)
            }
            _ => None,
        }
    }

    fn has_num(&mut self) -> bool {
        self.skip();
        matches!(self.b.get(self.i), Some(c) if c.is_ascii_digit() || *c == b'-' || *c == b'+' || *c == b'.')
    }

    fn num(&mut self) -> Option<f32> {
        self.skip();
        let s = self.i;
        if matches!(self.b.get(self.i), Some(b'-') | Some(b'+')) {
            self.i += 1;
        }
        let mut dot = false;
        while let Some(&c) = self.b.get(self.i) {
            if c.is_ascii_digit() {
                self.i += 1;
            } else if c == b'.' && !dot {
                dot = true;
                self.i += 1;
            } else {
                break;
            }
        }
        if matches!(self.b.get(self.i), Some(b'e') | Some(b'E')) {
            self.i += 1;
            if matches!(self.b.get(self.i), Some(b'-') | Some(b'+')) {
                self.i += 1;
            }
            while matches!(self.b.get(self.i), Some(c) if c.is_ascii_digit()) {
                self.i += 1;
            }
        }
        std::str::from_utf8(&self.b[s..self.i]).ok()?.parse().ok()
    }

    /// UZ: Yoy bayrog'i: A buyrug'ida bitta raqam `0`/`1`, ba'zan keyingi raqamga yopishib yoziladi.
    /// EN: Arc flag: in the A command it is a single digit `0`/`1`, sometimes written glued to the next number.
    /// RU: Флаг дуги: в команде A это одна цифра `0`/`1`, иногда записанная слитно со следующим числом.
    fn flag(&mut self) -> Option<bool> {
        self.skip();
        let c = *self.b.get(self.i)?;
        self.i += 1;
        match c {
            b'0' => Some(false),
            b'1' => Some(true),
            _ => None,
        }
    }
}

fn cubic(out: &mut Vec<Seg>, p0: (f32, f32), p1: (f32, f32), p2: (f32, f32), p3: (f32, f32)) {
    let n = 12;
    let mut prev = p0;
    for i in 1..=n {
        let t = i as f32 / n as f32;
        let u = 1.0 - t;
        let q = (
            u * u * u * p0.0 + 3.0 * u * u * t * p1.0 + 3.0 * u * t * t * p2.0 + t * t * t * p3.0,
            u * u * u * p0.1 + 3.0 * u * u * t * p1.1 + 3.0 * u * t * t * p2.1 + t * t * t * p3.1,
        );
        out.push((prev, q));
        prev = q;
    }
}

fn quad(out: &mut Vec<Seg>, p0: (f32, f32), p1: (f32, f32), p2: (f32, f32)) {
    let n = 10;
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

#[allow(clippy::too_many_arguments)]
fn arc(
    out: &mut Vec<Seg>,
    p0: (f32, f32),
    mut rx: f32,
    mut ry: f32,
    rot: f32,
    large: bool,
    sweep: bool,
    p1: (f32, f32),
) {
    if p0 == p1 {
        return;
    }
    rx = rx.abs();
    ry = ry.abs();
    if rx == 0.0 || ry == 0.0 {
        out.push((p0, p1));
        return;
    }
    let phi = rot.to_radians();
    let (cp, sp) = (phi.cos(), phi.sin());
    let dx = (p0.0 - p1.0) * 0.5;
    let dy = (p0.1 - p1.1) * 0.5;
    let x1 = cp * dx + sp * dy;
    let y1 = -sp * dx + cp * dy;
    let lam = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if lam > 1.0 {
        let s = lam.sqrt();
        rx *= s;
        ry *= s;
    }
    let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let mut co = if den == 0.0 {
        0.0
    } else {
        (num / den).max(0.0).sqrt()
    };
    if large == sweep {
        co = -co;
    }
    let cxp = co * rx * y1 / ry;
    let cyp = -co * ry * x1 / rx;
    let cx = cp * cxp - sp * cyp + (p0.0 + p1.0) * 0.5;
    let cy = sp * cxp + cp * cyp + (p0.1 + p1.1) * 0.5;
    let ang = |ux: f32, uy: f32, vx: f32, vy: f32| {
        let a = (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
        a
    };
    let th1 = ang(1.0, 0.0, (x1 - cxp) / rx, (y1 - cyp) / ry);
    let mut dth = ang(
        (x1 - cxp) / rx,
        (y1 - cyp) / ry,
        (-x1 - cxp) / rx,
        (-y1 - cyp) / ry,
    );
    if !sweep && dth > 0.0 {
        dth -= std::f32::consts::TAU;
    } else if sweep && dth < 0.0 {
        dth += std::f32::consts::TAU;
    }
    let n = ((dth.abs() / std::f32::consts::FRAC_PI_2 * 6.0).ceil() as usize).max(2);
    let mut prev = p0;
    for i in 1..=n {
        let t = th1 + dth * i as f32 / n as f32;
        let (ct, st) = (t.cos(), t.sin());
        let q = (
            cp * rx * ct - sp * ry * st + cx,
            sp * rx * ct + cp * ry * st + cy,
        );
        out.push((prev, if i == n { p1 } else { q }));
        prev = q;
    }
}

impl Path {
    /// UZ: SVG `d` matnini o'qiydi. Sintaksis noto'g'ri bo'lsa `None` qaytadi.
    /// EN: Parses SVG `d` text. Returns `None` if the syntax is invalid.
    /// RU: Разбирает текст SVG `d`. При неверном синтаксисе возвращает `None`.
    pub fn parse(d: &str) -> Option<Path> {
        let mut lx = Lexer {
            b: d.as_bytes(),
            i: 0,
        };
        let mut out: Vec<Seg> = Vec::new();
        let (mut cur, mut start) = ((0.0f32, 0.0f32), (0.0f32, 0.0f32));
        // UZ: oldingi egri chiziqning boshqaruv nuqtasi (S/T buyruqlari uchun)
        // EN: the previous curve's control point (for the S/T commands).
        // RU: контрольная точка предыдущей кривой (для команд S/T).
        let mut last_ctrl: Option<(f32, f32)> = None;
        let mut last_cmd = b' ';
        let mut cmd = lx.cmd()?;
        loop {
            let rel = cmd.is_ascii_lowercase();
            let up = cmd.to_ascii_uppercase();
            let (ox, oy) = if rel { cur } else { (0.0, 0.0) };
            let mut ctrl: Option<(f32, f32)> = None;
            match up {
                b'Z' => {
                    if cur != start {
                        out.push((cur, start));
                    }
                    cur = start;
                }
                b'M' => {
                    let p = (ox + lx.num()?, oy + lx.num()?);
                    cur = p;
                    start = p;
                    // UZ: Buyruqdan keyingi qo'shimcha koordinata juftliklari yashirin `L` (to'g'ri chiziq) hisoblanadi.
                    // EN: Extra coordinate pairs after a move command are treated as implicit `L` (line) commands.
                    // RU: Дополнительные пары координат после команды перемещения считаются неявными `L` (линия).
                    cmd = if rel { b'l' } else { b'L' };
                    last_cmd = up;
                    let _ = last_cmd;
                    last_ctrl = None;
                    if !lx.has_num() {
                        match lx.cmd() {
                            Some(c) => cmd = c,
                            None => break,
                        }
                    }
                    continue;
                }
                b'L' => {
                    let p = (ox + lx.num()?, oy + lx.num()?);
                    out.push((cur, p));
                    cur = p;
                }
                b'H' => {
                    let p = (if rel { cur.0 } else { 0.0 } + lx.num()?, cur.1);
                    out.push((cur, p));
                    cur = p;
                }
                b'V' => {
                    let p = (cur.0, if rel { cur.1 } else { 0.0 } + lx.num()?);
                    out.push((cur, p));
                    cur = p;
                }
                b'C' => {
                    let p1 = (ox + lx.num()?, oy + lx.num()?);
                    let p2 = (ox + lx.num()?, oy + lx.num()?);
                    let p3 = (ox + lx.num()?, oy + lx.num()?);
                    cubic(&mut out, cur, p1, p2, p3);
                    ctrl = Some(p2);
                    cur = p3;
                }
                b'S' => {
                    let p1 = match (last_ctrl, matches!(last_cmd, b'C' | b'S')) {
                        (Some(c), true) => (2.0 * cur.0 - c.0, 2.0 * cur.1 - c.1),
                        _ => cur,
                    };
                    let p2 = (ox + lx.num()?, oy + lx.num()?);
                    let p3 = (ox + lx.num()?, oy + lx.num()?);
                    cubic(&mut out, cur, p1, p2, p3);
                    ctrl = Some(p2);
                    cur = p3;
                }
                b'Q' => {
                    let p1 = (ox + lx.num()?, oy + lx.num()?);
                    let p2 = (ox + lx.num()?, oy + lx.num()?);
                    quad(&mut out, cur, p1, p2);
                    ctrl = Some(p1);
                    cur = p2;
                }
                b'T' => {
                    let p1 = match (last_ctrl, matches!(last_cmd, b'Q' | b'T')) {
                        (Some(c), true) => (2.0 * cur.0 - c.0, 2.0 * cur.1 - c.1),
                        _ => cur,
                    };
                    let p2 = (ox + lx.num()?, oy + lx.num()?);
                    quad(&mut out, cur, p1, p2);
                    ctrl = Some(p1);
                    cur = p2;
                }
                b'A' => {
                    let rx = lx.num()?;
                    let ry = lx.num()?;
                    let rot = lx.num()?;
                    let large = lx.flag()?;
                    let sweep = lx.flag()?;
                    let p = (ox + lx.num()?, oy + lx.num()?);
                    arc(&mut out, cur, rx, ry, rot, large, sweep, p);
                    cur = p;
                }
                _ => return None,
            }
            last_ctrl = ctrl;
            last_cmd = up;
            if up == b'Z' || !lx.has_num() {
                match lx.cmd() {
                    Some(c) => cmd = c,
                    None => break,
                }
            }
        }
        // UZ: Yopilmagan konturlar ham to'ldirish uchun avtomatik yopiladi.
        // EN: Unclosed contours are closed automatically so they can be filled.
        // RU: Незамкнутые контуры автоматически замыкаются, чтобы их можно было залить.
        if cur != start {
            out.push((cur, start));
        }
        Some(Path { segs: out })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kvadrat() {
        let p = Path::parse("M2 2H10V10H2z").unwrap();
        assert_eq!(p.segs.len(), 4);
    }

    #[test]
    fn nisbiy_va_yashirin_l() {
        let p = Path::parse("m1,1 4,0 0,4 -4,0z").unwrap();
        assert_eq!(p.segs.len(), 4);
        assert_eq!(p.segs[0], ((1.0, 1.0), (5.0, 1.0)));
    }

    #[test]
    fn material_orqaga_strelka() {
        let p =
            Path::parse("M20,11H7.83l5.59,-5.59L12,4l-8,8 8,8 1.41,-1.41L7.83,13H20v-2z").unwrap();
        assert!(p.segs.len() >= 9);
    }

    #[test]
    fn yoy_va_egri() {
        let p = Path::parse("M12 2a10 10 0 1 0 0.001 0z").unwrap();
        assert!(p.segs.len() > 10);
        let q = Path::parse("M0 0C1 1 2 1 3 0S5 -1 6 0Q7 1 8 0T10 0").unwrap();
        assert!(q.segs.len() > 30);
        assert!(Path::parse("X1 2").is_none());
    }
}
