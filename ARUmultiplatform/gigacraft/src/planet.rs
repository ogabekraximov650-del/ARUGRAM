//! Sayyora geometriyasi: kub-shar (6 yuz) tarmog'i. Har bir blok = (yuz, u, v, qatlam). Qatlam raqami = sayyora markazidan radius (blokda).
//! Teng burchakli proyeksiya: yuz markazida blok ~1x1, burchaklarga qarab biroz torayadi.
use crate::math::*;

/// Daraxt chuqurligi: yuz tomoni N = 16 << LMAX blok.
pub const LMAX: u32 = 22;
pub const N: i64 = 16 << LMAX;
/// Dengiz sathi qatlami (radius, blok): yuz markazida blok kengligi ~1 bo'lishi uchun 2N/pi.
pub const SEA_R: i64 = 42_722_830;
const QPI: f64 = std::f64::consts::FRAC_PI_4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Cell {
    pub f: u8,
    pub u: i64,
    pub v: i64,
    pub r: i64,
}

/// Bo'lak (16x16x16 blok) koordinatalari.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CP {
    pub f: u8,
    pub u: i32,
    pub v: i32,
    pub r: i32,
}

/// Yuz o'qlari: (normal, u yo'nalishi, v yo'nalishi).
pub fn axes(f: u8) -> (D3, D3, D3) {
    match f {
        0 => ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        1 => ([-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        2 => ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        3 => ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        4 => ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        _ => ([0.0, 0.0, -1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
    }
}

/// Yuz indekslari (haqiqiy son; blok `i` oralig'i [i, i+1]) -> birlik yo'nalish. Yuzdan tashqari qiymatlar ham ishlaydi (qo'shni yuzga o'tadi).
pub fn dir(f: u8, a: f64, b: f64) -> D3 {
    let (n, u, v) = axes(f);
    let xu = ((a / N as f64 * 2.0 - 1.0) * QPI).tan();
    let xv = ((b / N as f64 * 2.0 - 1.0) * QPI).tan();
    d_norm(d_add(n, d_add(d_mul(u, xu), d_mul(v, xv))))
}

/// Birlik yo'nalish -> (yuz, a, b) haqiqiy indekslar.
pub fn dir_to_face(d: D3) -> (u8, f64, f64) {
    let (ax, ay, az) = (d[0].abs(), d[1].abs(), d[2].abs());
    let f = if ax >= ay && ax >= az {
        if d[0] > 0.0 {
            0
        } else {
            1
        }
    } else if ay >= az {
        if d[1] > 0.0 {
            2
        } else {
            3
        }
    } else if d[2] > 0.0 {
        4
    } else {
        5
    };
    let (n, u, v) = axes(f);
    let dn = d_dot(d, n);
    let (pu, pv) = (d_dot(d, u) / dn, d_dot(d, v) / dn);
    let a = (pu.atan() / QPI + 1.0) * 0.5 * N as f64;
    let b = (pv.atan() / QPI + 1.0) * 0.5 * N as f64;
    (f, a, b)
}

/// Dunyo nuqtasi -> u joylashgan blok.
pub fn cell_at(p: D3) -> Cell {
    let r = d_len(p);
    let (f, a, b) = dir_to_face(d_mul(p, 1.0 / r));
    Cell {
        f,
        u: (a.floor() as i64).clamp(0, N - 1),
        v: (b.floor() as i64).clamp(0, N - 1),
        r: r.floor() as i64,
    }
}

/// Blok burchagi (butun indeks) berilgan radiusda.
pub fn corner(f: u8, a: i64, b: i64, r: f64) -> D3 {
    d_mul(dir(f, a as f64, b as f64), r)
}

/// Blok markazi (dunyo koordinatalarida).
pub fn center(c: &Cell) -> D3 {
    d_mul(dir(c.f, c.u as f64 + 0.5, c.v as f64 + 0.5), c.r as f64 + 0.5)
}

/// Yuz chegarasidan oshgan indekslarni qo'shni yuzga o'tkazadi.
pub fn wrap(f: u8, u: i64, v: i64, r: i64) -> Cell {
    if (0..N).contains(&u) && (0..N).contains(&v) {
        return Cell { f, u, v, r };
    }
    let (nf, a, b) = dir_to_face(dir(f, u as f64 + 0.5, v as f64 + 0.5));
    Cell {
        f: nf,
        u: (a.floor() as i64).clamp(0, N - 1),
        v: (b.floor() as i64).clamp(0, N - 1),
        r,
    }
}

pub fn neighbor(c: &Cell, du: i64, dv: i64, dr: i64) -> Cell {
    wrap(c.f, c.u + du, c.v + dv, c.r + dr)
}

pub fn chunk_of(c: &Cell) -> (CP, usize) {
    let cp = CP {
        f: c.f,
        u: c.u.div_euclid(16) as i32,
        v: c.v.div_euclid(16) as i32,
        r: c.r.div_euclid(16) as i32,
    };
    let i = (c.u.rem_euclid(16) + c.v.rem_euclid(16) * 16 + c.r.rem_euclid(16) * 256) as usize;
    (cp, i)
}

/// Bo'lakning boshlang'ich bloki.
pub fn chunk_origin(cp: CP) -> (i64, i64, i64) {
    (cp.u as i64 * 16, cp.v as i64 * 16, cp.r as i64 * 16)
}

/// Eng yaqin ustun (yuz, cu, cv) ning dengiz sathidagi markazi: mesh boshlanish nuqtasi.
pub fn col_anchor(f: u8, cu: i32, cv: i32) -> D3 {
    d_mul(dir(f, cu as f64 * 16.0 + 8.0, cv as f64 * 16.0 + 8.0), SEA_R as f64)
}

/// Daraja `l` plitasi (tomoni 16<<l blok) markazi, dengiz sathida.
pub fn tile_anchor(f: u8, l: u32, tu: i64, tv: i64) -> D3 {
    let s = (16i64 << l) as f64;
    d_mul(dir(f, (tu as f64 + 0.5) * s, (tv as f64 + 0.5) * s), SEA_R as f64)
}

/// Sayyora yuzasidagi balandlik (dengiz sathidan, blokda).
pub fn altitude(p: D3) -> f64 {
    d_len(p) - SEA_R as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yo_nalish_va_indeks_aylanadi() {
        for f in 0..6u8 {
            for &(a, b) in &[(0.5, 0.5), (1234.25, 99999.75), (N as f64 - 0.5, 7.5), (N as f64 * 0.5, N as f64 * 0.5)] {
                let d = dir(f, a, b);
                assert!((d_len(d) - 1.0).abs() < 1e-12);
                let (f2, a2, b2) = dir_to_face(d);
                assert_eq!(f2, f, "yuz {f} {a} {b}");
                assert!((a2 - a).abs() < 1e-3 && (b2 - b).abs() < 1e-3, "{a} {b} -> {a2} {b2}");
            }
        }
    }

    #[test]
    fn nuqta_blok_aylanadi() {
        let c = Cell { f: 2, u: 12_345_678, v: 33_000_001, r: SEA_R + 17 };
        let p = center(&c);
        assert_eq!(cell_at(p), c);
        let c2 = Cell { f: 5, u: 5, v: N - 6, r: SEA_R - 400 };
        assert_eq!(cell_at(center(&c2)), c2);
    }

    #[test]
    fn blok_olchami_taxminan_bir() {
        // yuz markazida qo'shni burchaklar orasi ~ 1 blok
        let (a, b) = (N / 2, N / 2);
        let p0 = corner(0, a, b, SEA_R as f64);
        let p1 = corner(0, a + 1, b, SEA_R as f64);
        let p2 = corner(0, a, b + 1, SEA_R as f64);
        for d in [d_len(d_sub(p1, p0)), d_len(d_sub(p2, p0))] {
            assert!((d - 1.0).abs() < 0.02, "blok kengligi {d}");
        }
        // burchakda torayadi, lekin 0.5 dan kam emas
        let q0 = corner(0, 1000, 1000, SEA_R as f64);
        let q1 = corner(0, 1001, 1000, SEA_R as f64);
        let w = d_len(d_sub(q1, q0));
        assert!(w > 0.5 && w < 1.0, "burchak kengligi {w}");
    }

    #[test]
    fn qoshni_yuzga_otish() {
        let c = Cell { f: 0, u: N - 1, v: N / 2, r: SEA_R };
        let n = neighbor(&c, 1, 0, 0);
        assert_ne!(n.f, c.f, "qo'shni yuzga o'tishi kerak");
        // masofa ~1 blok
        let d = d_len(d_sub(center(&n), center(&c)));
        assert!(d > 0.4 && d < 2.5, "{d}");
        // orqaga qaytish
        let back = neighbor(&n, -1, 0, 0);
        let back2 = (0..4).any(|_| true) && (back.f == c.f || d_len(d_sub(center(&back), center(&c))) < 2.5);
        assert!(back2);
        // yuz ichida oddiy qo'shni
        let m = neighbor(&Cell { f: 3, u: 100, v: 100, r: SEA_R }, 1, 0, 1);
        assert_eq!((m.f, m.u, m.v, m.r), (3, 101, 100, SEA_R + 1));
    }

    #[test]
    fn chunk_va_indeks() {
        let (cp, i) = chunk_of(&Cell { f: 1, u: 33, v: 17, r: SEA_R });
        assert_eq!((cp.f, cp.u, cp.v), (1, 2, 1));
        assert_eq!(cp.r as i64, SEA_R.div_euclid(16));
        assert_eq!(i, 1 + 16 + (SEA_R.rem_euclid(16) as usize) * 256);
    }

    #[test]
    fn sayyora_hajmi_yer_darajasida() {
        // aylana ~ 4N blok * 0.25 m
        let km = 4.0 * N as f64 * 0.25 / 1000.0;
        assert!(km > 60_000.0 && km < 80_000.0, "{km} km");
        // gorizont masofasi ko'z balandligida ~ 5-6 km
        let h = 6.4;
        let hor = (2.0 * SEA_R as f64 * h).sqrt() * 0.25 / 1000.0;
        assert!(hor > 4.0 && hor < 8.0, "{hor} km");
    }
}
