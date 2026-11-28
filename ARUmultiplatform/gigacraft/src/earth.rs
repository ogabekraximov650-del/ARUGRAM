//! Haqiqiy Yer xaritasi: balandlik (ETOPO5, ~28 km kataklar) va o'simlik/iqlim turlari (Natural Earth I rangidan).
//! `assets/earth.bin`: "EARTH2", kenglik/balandlik (u16 x2), balandlik (i16, metr), tur (u8), ko'l qoplami (u8) va ko'l sathi (u8, 20 m birlikda).
use crate::math::D3;
use std::sync::OnceLock;

pub const WATER: u8 = 0;
pub const FOREST: u8 = 1;
pub const GRASS: u8 = 2;
pub const SAVANNA: u8 = 3;
pub const DESERT: u8 = 4;
pub const TUNDRA: u8 = 5;
pub const ICE: u8 = 6;
pub const LAKE: u8 = 7;

const DATA: &[u8] = include_bytes!("../assets/earth.bin");

pub struct Earth {
    w: usize,
    h: usize,
    elev: &'static [u8],
    bio: &'static [u8],
    cov: &'static [u8],
    lvl: &'static [u8],
}

/// To'liq namuna: balandlik (m), tur, ko'l qoplami [0,1] va ko'l sathi (m).
#[derive(Clone, Copy, Debug)]
pub struct Sample {
    pub el: f32,
    pub bio: u8,
    pub lake: f32,
    pub lake_level: f32,
}

pub fn get() -> &'static Earth {
    static E: OnceLock<Earth> = OnceLock::new();
    E.get_or_init(|| {
        assert!(&DATA[0..6] == b"EARTH2", "earth.bin noto'g'ri");
        let w = u16::from_le_bytes([DATA[6], DATA[7]]) as usize;
        let h = u16::from_le_bytes([DATA[8], DATA[9]]) as usize;
        let eo = 10;
        let bo = eo + w * h * 2;
        let co = bo + w * h;
        let lo = co + w * h;
        Earth { w, h, elev: &DATA[eo..bo], bio: &DATA[bo..co], cov: &DATA[co..lo], lvl: &DATA[lo..lo + w * h] }
    })
}

/// Kenglik/uzunlik (daraja) -> birlik yo'nalish. Shimoliy qutb = +Y, 0° uzunlik = +X, sharqqa = -Z.
pub fn latlon_dir(lat: f64, lon: f64) -> D3 {
    let (la, lo) = (lat.to_radians(), lon.to_radians());
    [la.cos() * lo.cos(), la.sin(), -la.cos() * lo.sin()]
}

/// Yo'nalish -> (kenglik, uzunlik) daraja, uzunlik [0, 360).
pub fn lat_lon(d: D3) -> (f64, f64) {
    let lat = d[1].clamp(-1.0, 1.0).asin().to_degrees();
    let lon = (-d[2]).atan2(d[0]).to_degrees();
    (lat, lon.rem_euclid(360.0))
}

impl Earth {
    fn e(&self, x: usize, y: usize) -> f32 {
        let i = (y * self.w + x) * 2;
        i16::from_le_bytes([self.elev[i], self.elev[i + 1]]) as f32
    }

    /// (balandlik metrda — ikki chiziqli interpolyatsiya, tur — eng yaqin katak).
    pub fn sample(&self, d: D3) -> (f32, u8) {
        let s = self.sample_full(d);
        (s.el, s.bio)
    }

    pub fn sample_full(&self, d: D3) -> Sample {
        let (lat, lon) = lat_lon(d);
        let fx = lon / 360.0 * self.w as f64 - 0.5;
        let fy = ((90.0 - lat) / 180.0 * self.h as f64 - 0.5).clamp(0.0, self.h as f64 - 1.0);
        let x0 = fx.floor();
        let (tx, ty) = ((fx - x0) as f32, (fy - fy.floor()) as f32);
        let xi = |x: f64| (x.rem_euclid(self.w as f64)) as usize;
        let (xa, xb) = (xi(x0), xi(x0 + 1.0));
        let (ya, yb) = (fy.floor() as usize, (fy.floor() as usize + 1).min(self.h - 1));
        let bil = |f: &dyn Fn(usize, usize) -> f32| {
            let top = f(xa, ya) * (1.0 - tx) + f(xb, ya) * tx;
            let bot = f(xa, yb) * (1.0 - tx) + f(xb, yb) * tx;
            top * (1.0 - ty) + bot * ty
        };
        let el = bil(&|x, y| self.e(x, y));
        let lake = bil(&|x, y| self.cov[y * self.w + x] as f32 / 255.0);
        let lake_level = [(xa, ya), (xb, ya), (xa, yb), (xb, yb)].iter().map(|&(x, y)| self.lvl[y * self.w + x] as f32 * 20.0).fold(0.0f32, f32::max);
        let (nx, ny) = (xi(fx.round()), (fy.round() as usize).min(self.h - 1));
        Sample { el, bio: self.bio[ny * self.w + nx], lake, lake_level }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(lat: f64, lon: f64) -> (f32, u8) {
        get().sample(latlon_dir(lat, lon))
    }

    #[test]
    fn tashkent_haqiqiy_joyda() {
        let (el, b) = at(41.3, 69.28);
        assert!(el > 350.0 && el < 900.0, "Toshkent balandligi {el} m");
        assert_eq!(b, SAVANNA);
    }

    #[test]
    fn mashhur_joylar() {
        assert_eq!(at(23.0, 10.0).1, DESERT, "Sahara");
        assert_eq!(at(-3.0, -60.0 + 360.0).1, FOREST, "Amazonka");
        assert_eq!(at(62.0, 100.0).1, FOREST, "Sibir tayga");
        assert_eq!(at(72.0, -40.0 + 360.0).1, ICE, "Grenlandiya");
        assert_eq!(at(0.0, 180.0).1, WATER, "Tinch okean");
        assert!(at(28.0, 87.0).0 > 3000.0, "Himolay");
        assert!(at(0.0, 180.0).0 < -3000.0, "okean tubi");
        assert_eq!(at(-85.0, 0.0).1, ICE, "Antarktida");
    }

    #[test]
    fn lat_lon_aylanadi() {
        for &(la, lo) in &[(41.3, 69.28), (-33.9, 151.2), (10.0, 350.0), (89.0, 12.0)] {
            let (a, b) = lat_lon(latlon_dir(la, lo));
            assert!((a - la).abs() < 1e-9 && (b - lo).abs() < 1e-9, "{la},{lo} -> {a},{b}");
        }
    }

    #[test]
    fn shar_yuzida_uzluksiz() {
        // 0 va 360 daraja uzunlik tutashuvida sakrash yo'q
        let (a, _) = at(10.0, 359.99);
        let (b, _) = at(10.0, 0.01);
        assert!((a - b).abs() < 60.0, "{a} {b}");
    }

    #[test]
    fn koller_haqiqiy_joyda() {
        let s = get().sample_full(latlon_dir(53.5, 108.0)); // Baykal
        assert!(s.lake > 0.4 && s.lake_level > 200.0 && s.lake_level < 900.0, "{s:?}");
        let h = get().sample_full(latlon_dir(45.0, 280.0)); // Gyuron ko'li
        assert!(h.lake > 0.3, "{h:?}");
        let t = get().sample_full(latlon_dir(41.3, 69.28));
        assert!(t.lake < 0.05);
    }
}
