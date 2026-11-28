//! Shovqin: butun sonli xesh va silliq qiymat shovqini (f64 koordinatalar — juda uzoq masofada ham aniq).

/// (seed, x, z) -> 32 bitli xesh.
pub fn hash2(seed: u32, x: i64, z: i64) -> u32 {
    let mut h = (x as u64).wrapping_mul(0x9E37_79B1_85EB_CA87)
        ^ (z as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ (seed as u64).wrapping_mul(0x1656_67B1_9E37_79F9);
    h ^= h >> 32;
    h = h.wrapping_mul(0xD6E8_FEB8_6659_FD93);
    h ^= h >> 32;
    h as u32
}

/// (seed, x, y, z) -> 32 bitli xesh.
pub fn hash3(seed: u32, x: i64, y: i64, z: i64) -> u32 {
    hash2(hash2(seed, x, y), z, y ^ 0x5bd1_e995)
}

/// Xeshdan [0,1) oralig'idagi son.
pub fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / 16_777_216.0
}

fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// Silliq 2D qiymat shovqini, natija [0,1].
pub fn vnoise2(seed: u32, x: f64, z: f64) -> f32 {
    let (xf, zf) = (x.floor(), z.floor());
    let (fx, fz) = (fade((x - xf) as f32), fade((z - zf) as f32));
    let (ix, iz) = (xf as i64, zf as i64);
    let a = unit(hash2(seed, ix, iz));
    let b = unit(hash2(seed, ix + 1, iz));
    let c = unit(hash2(seed, ix, iz + 1));
    let d = unit(hash2(seed, ix + 1, iz + 1));
    let top = a + (b - a) * fx;
    let bot = c + (d - c) * fx;
    top + (bot - top) * fz
}

/// Fraktal shovqin (bir necha oktava), natija taxminan [0,1].
pub fn fbm2(seed: u32, x: f64, z: f64, oct: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut norm = 0.0;
    let mut f = 1.0;
    for o in 0..oct {
        sum += amp * vnoise2(seed.wrapping_add(o * 7919), x * f, z * f);
        norm += amp;
        amp *= 0.5;
        f *= 2.0;
    }
    sum / norm
}

/// Silliq 3D qiymat shovqini, natija [0,1].
pub fn vnoise3(seed: u32, x: f64, y: f64, z: f64) -> f32 {
    let (xf, yf, zf) = (x.floor(), y.floor(), z.floor());
    let (fx, fy, fz) = (fade((x - xf) as f32), fade((y - yf) as f32), fade((z - zf) as f32));
    let (ix, iy, iz) = (xf as i64, yf as i64, zf as i64);
    let h = |dx: i64, dy: i64, dz: i64| unit(hash3(seed, ix + dx, iy + dy, iz + dz));
    let l = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let x00 = l(h(0, 0, 0), h(1, 0, 0), fx);
    let x10 = l(h(0, 1, 0), h(1, 1, 0), fx);
    let x01 = l(h(0, 0, 1), h(1, 0, 1), fx);
    let x11 = l(h(0, 1, 1), h(1, 1, 1), fx);
    l(l(x00, x10, fy), l(x01, x11, fy), fz)
}

/// Fraktal 3D shovqin, natija taxminan [0,1]. Koordinatalar "to'lqin uzunligi" birligida beriladi.
pub fn fbm3(seed: u32, x: f64, y: f64, z: f64, oct: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut norm = 0.0;
    let mut f = 1.0;
    for o in 0..oct {
        sum += amp * vnoise3(seed.wrapping_add(o * 7919), x * f, y * f, z * f);
        norm += amp;
        amp *= 0.5;
        f *= 2.0;
    }
    sum / norm
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xesh_barqaror() {
        assert_eq!(hash2(1, 5, 7), hash2(1, 5, 7));
        assert_ne!(hash2(1, 5, 7), hash2(1, 7, 5));
        assert_ne!(hash2(1, 5, 7), hash2(2, 5, 7));
    }

    #[test]
    fn shovqin_oraliqda_va_silliq() {
        let mut prev = vnoise2(3, 0.0, 0.0);
        for i in 1..2000 {
            let v = vnoise2(3, i as f64 * 0.01, 0.37);
            assert!((0.0..=1.0).contains(&v));
            assert!((v - prev).abs() < 0.05, "sakrash: {} -> {}", prev, v);
            prev = v;
        }
    }

    #[test]
    fn uzoq_koordinatada_ham_ishlaydi() {
        let a = fbm2(9, 8.0e9, -7.0e9, 4);
        let b = fbm2(9, 8.0e9 + 0.001, -7.0e9, 4);
        assert!((0.0..=1.0).contains(&a));
        assert!((a - b).abs() < 0.01);
    }

    #[test]
    fn shovqin_3d_oraliqda_va_silliq() {
        let mut prev = vnoise3(3, 0.0, 0.3, 0.7);
        for i in 1..2000 {
            let v = vnoise3(3, i as f64 * 0.01, 0.3, 0.7);
            assert!((0.0..=1.0).contains(&v));
            assert!((v - prev).abs() < 0.06, "{prev} -> {v}");
            prev = v;
        }
        let a = fbm3(9, 4.0e7, -3.0e7, 1.0e7, 4);
        assert!((0.0..=1.0).contains(&a));
    }
}
