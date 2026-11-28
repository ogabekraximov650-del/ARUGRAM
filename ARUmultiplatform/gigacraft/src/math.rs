//! Kichik matematika: 3 o'lchamli vektor va 4x4 matritsa (ustun bo'yicha, OpenGL tartibida).
use std::ops::{Add, Mul, Neg, Sub};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

pub const fn v3(x: f32, y: f32, z: f32) -> V3 {
    V3 { x, y, z }
}

impl Add for V3 {
    type Output = V3;
    fn add(self, o: V3) -> V3 {
        v3(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl Sub for V3 {
    type Output = V3;
    fn sub(self, o: V3) -> V3 {
        v3(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl Mul<f32> for V3 {
    type Output = V3;
    fn mul(self, k: f32) -> V3 {
        v3(self.x * k, self.y * k, self.z * k)
    }
}
impl Neg for V3 {
    type Output = V3;
    fn neg(self) -> V3 {
        v3(-self.x, -self.y, -self.z)
    }
}

impl V3 {
    pub fn dot(self, o: V3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn cross(self, o: V3) -> V3 {
        v3(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
    pub fn len(self) -> f32 {
        self.dot(self).sqrt()
    }
    pub fn norm(self) -> V3 {
        let l = self.len();
        if l < 1e-12 {
            v3(0.0, 1.0, 0.0)
        } else {
            self * (1.0 / l)
        }
    }
}

pub type D3 = [f64; 3];

pub fn d_add(a: D3, b: D3) -> D3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub fn d_sub(a: D3, b: D3) -> D3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub fn d_mul(a: D3, k: f64) -> D3 {
    [a[0] * k, a[1] * k, a[2] * k]
}
pub fn d_dot(a: D3, b: D3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub fn d_cross(a: D3, b: D3) -> D3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
pub fn d_len(a: D3) -> f64 {
    d_dot(a, a).sqrt()
}
pub fn d_norm(a: D3) -> D3 {
    let l = d_len(a);
    if l < 1e-300 {
        [0.0, 1.0, 0.0]
    } else {
        d_mul(a, 1.0 / l)
    }
}
pub fn d_f32(a: D3) -> V3 {
    v3(a[0] as f32, a[1] as f32, a[2] as f32)
}

pub type M4 = [f32; 16];

/// Perspektiv proyeksiya (ustun bo'yicha).
pub fn perspective(fovy: f32, aspect: f32, near: f32, far: f32) -> M4 {
    let f = 1.0 / (fovy * 0.5).tan();
    let mut m = [0.0; 16];
    m[0] = f / aspect;
    m[5] = f;
    m[10] = (far + near) / (near - far);
    m[11] = -1.0;
    m[14] = 2.0 * far * near / (near - far);
    m
}

/// Qarash yo'nalishi: yaw=0 bo'lsa -Z ga qaraydi; pitch>0 — yuqoriga.
pub fn forward(yaw: f32, pitch: f32) -> V3 {
    v3(
        -yaw.sin() * pitch.cos(),
        pitch.sin(),
        -yaw.cos() * pitch.cos(),
    )
}

/// O'ng tomon yo'nalishi (gorizontal).
pub fn right(yaw: f32) -> V3 {
    v3(yaw.cos(), 0.0, -yaw.sin())
}

/// Faqat aylanishdan iborat ko'rinish matritsasi (kamera boshlang'ich nuqtada: dunyo kameraga nisbatan beriladi).
pub fn view_rot(yaw: f32, pitch: f32) -> M4 {
    let f = forward(yaw, pitch);
    let s = f.cross(v3(0.0, 1.0, 0.0)).norm();
    let u = s.cross(f);
    let mut m = [0.0; 16];
    m[0] = s.x;
    m[4] = s.y;
    m[8] = s.z;
    m[1] = u.x;
    m[5] = u.y;
    m[9] = u.z;
    m[2] = -f.x;
    m[6] = -f.y;
    m[10] = -f.z;
    m[15] = 1.0;
    m
}

/// Ixtiyoriy yo'nalish uchun ko'rinish matritsasi: o'ng, tepa va oldinga birlik vektorlar.
pub fn view_basis(r: V3, u: V3, f: V3) -> M4 {
    let mut m = [0.0; 16];
    m[0] = r.x;
    m[4] = r.y;
    m[8] = r.z;
    m[1] = u.x;
    m[5] = u.y;
    m[9] = u.z;
    m[2] = -f.x;
    m[6] = -f.y;
    m[10] = -f.z;
    m[15] = 1.0;
    m
}

/// Soya xaritasi uchun ortogonal matritsa (kamera markazida): `ldir` — yorug'lik tomon yo'nalish, `half` — yarim kenglik,
/// `depth` — chuqurlik yarim oralig'i. Yorug'likka yaqin nuqta kichikroq chuqurlik oladi.
pub fn light_matrix(ldir: V3, up_hint: V3, half: f32, depth: f32) -> M4 {
    let z = ldir.norm();
    let mut x = up_hint.cross(z);
    if x.len() < 1e-4 {
        x = v3(1.0, 0.0, 0.0).cross(z);
    }
    let x = x.norm();
    let y = z.cross(x);
    let mut m = [0.0; 16];
    m[0] = x.x / half;
    m[4] = x.y / half;
    m[8] = x.z / half;
    m[1] = y.x / half;
    m[5] = y.y / half;
    m[9] = y.z / half;
    m[2] = -z.x / depth;
    m[6] = -z.y / depth;
    m[10] = -z.z / depth;
    m[15] = 1.0;
    m
}

/// a * b (ustun bo'yicha matritsalar).
pub fn mul(a: &M4, b: &M4) -> M4 {
    let mut r = [0.0; 16];
    for c in 0..4 {
        for rr in 0..4 {
            let mut s = 0.0;
            for k in 0..4 {
                s += a[k * 4 + rr] * b[c * 4 + k];
            }
            r[c * 4 + rr] = s;
        }
    }
    r
}

/// Ko'rinish piramidasi tekisliklari (Gribb–Hartmann); har biri (a,b,c,d), normal ichkariga qaragan.
pub fn frustum(vp: &M4) -> [[f32; 4]; 6] {
    let row = |i: usize| [vp[i], vp[4 + i], vp[8 + i], vp[12 + i]];
    let (r0, r1, r2, r3) = (row(0), row(1), row(2), row(3));
    let mut p = [[0.0f32; 4]; 6];
    for i in 0..4 {
        p[0][i] = r3[i] + r0[i];
        p[1][i] = r3[i] - r0[i];
        p[2][i] = r3[i] + r1[i];
        p[3][i] = r3[i] - r1[i];
        p[4][i] = r3[i] + r2[i];
        p[5][i] = r3[i] - r2[i];
    }
    for pl in p.iter_mut() {
        let l = (pl[0] * pl[0] + pl[1] * pl[1] + pl[2] * pl[2]).sqrt().max(1e-12);
        for v in pl.iter_mut() {
            *v /= l;
        }
    }
    p
}

/// Kameraga nisbatan markazi `c` va radiusi `r` bo'lgan shar piramida ichidami (qisman ham bo'lsa — ha).
pub fn sphere_visible(fr: &[[f32; 4]; 6], c: V3, r: f32) -> bool {
    for p in fr {
        if p[0] * c.x + p[1] * c.y + p[2] * c.z + p[3] < -r {
            return false;
        }
    }
    true
}

pub fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [lerp(a[0], b[0], t), lerp(a[1], b[1], t), lerp(a[2], b[2], t)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oldinga_qarash() {
        let f = forward(0.0, 0.0);
        assert!((f.z + 1.0).abs() < 1e-6 && f.x.abs() < 1e-6);
        let r = right(0.0);
        assert!((r.x - 1.0).abs() < 1e-6);
    }

    #[test]
    fn kamera_oldidagi_nuqta_piramidada() {
        let p = perspective(1.2, 2.0, 0.3, 1000.0);
        let v = view_rot(0.0, 0.0);
        let vp = mul(&p, &v);
        let fr = frustum(&vp);
        assert!(sphere_visible(&fr, v3(0.0, 0.0, -10.0), 0.1));
        assert!(!sphere_visible(&fr, v3(0.0, 0.0, 10.0), 0.1));
        assert!(!sphere_visible(&fr, v3(0.0, 0.0, -2000.0), 0.1));
    }

    #[test]
    fn matritsa_ko_paytirish_birlik() {
        let mut id = [0.0; 16];
        for i in 0..4 {
            id[i * 5] = 1.0;
        }
        let p = perspective(1.0, 1.5, 0.1, 100.0);
        let r = mul(&id, &p);
        for i in 0..16 {
            assert!((r[i] - p[i]).abs() < 1e-6);
        }
    }

    #[test]
    fn asos_matritsasi_view_rot_bilan_mos() {
        let (yaw, pitch) = (0.7f32, 0.3f32);
        let f = forward(yaw, pitch);
        let r = right(yaw);
        let u = r.cross(f);
        let a = view_basis(r, u, f);
        let b = view_rot(yaw, pitch);
        for i in 0..16 {
            assert!((a[i] - b[i]).abs() < 1e-5, "{i}: {} {}", a[i], b[i]);
        }
    }
}
