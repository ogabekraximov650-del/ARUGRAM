//! Relyef: markazda (qishloq) tekis, chetlarida tepaliklar. Balandlik formulasi o'yinchi/jismlar uchun ham shu.

/// Relyef to'rini yoyilishi (har tomonga, metr).
pub const EXTENT: f32 = 180.0;
/// O'yinchi yura oladigan chegara.
pub const WORLD_HALF: f32 = 98.0;
const FLAT_R: f32 = 40.0;
const HILL_R: f32 = 75.0;

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn height(x: f32, z: f32) -> f32 {
    let r = (x * x + z * z).sqrt();
    let k = smoothstep(FLAT_R, HILL_R, r);
    if k <= 0.0 {
        return 0.0;
    }
    let h = (x * 0.045 + 1.3).sin() * (z * 0.037 + 0.4).cos()
        + 0.5 * (x * 0.09 + z * 0.07 + 2.0).sin()
        + 0.25 * (x * 0.17 - z * 0.13).sin();
    (h + 0.6) * 2.4 * k
}

pub fn normal(x: f32, z: f32) -> glam::Vec3 {
    let e = 0.5;
    let dx = height(x + e, z) - height(x - e, z);
    let dz = height(x, z + e) - height(x, z - e);
    glam::Vec3::new(-dx, 2.0 * e, -dz).normalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markaz_tekis_chetlari_baland() {
        assert_eq!(height(0.0, 0.0), 0.0);
        assert_eq!(height(30.0, 20.0), 0.0);
        let far = (0..40).map(|i| height(120.0 + i as f32 * 2.0, 10.0)).fold(0.0f32, |a, b| a.max(b.abs()));
        assert!(far > 0.5 && far < 8.0);
    }

    #[test]
    fn nishab_yumshoq() {
        // qo'shni nuqtalar orasida sakrash yo'q
        let mut prev = height(40.0, 0.0);
        for i in 1..400 {
            let h = height(40.0 + i as f32 * 0.25, 0.0);
            assert!((h - prev).abs() < 0.5);
            prev = h;
        }
    }
}
