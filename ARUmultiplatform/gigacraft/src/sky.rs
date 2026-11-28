//! Osmon: kun/tun aylanishi, quyosh va oy yo'nalishi, ranglar (kechqurun — binafsha-to'q sariq).
use crate::math::{lerp3, smoothstep, v3, V3};

/// Kun aylanishi bir marta aylanishining haqiqiy vaqti (soniya).
pub const DAY_SECS: f32 = 720.0;

#[derive(Clone, Copy, Debug)]
pub struct SkyState {
    /// Asosiy yorug'lik (quyosh yoki oy) tomon yo'nalish.
    pub light_dir: V3,
    pub light_col: [f32; 3],
    /// Quyoshning o'zi yo'nalishi (osmonda disk chizish uchun).
    pub sun: V3,
    pub amb: [f32; 3],
    pub top: [f32; 3],
    pub hor: [f32; 3],
    pub fog: [f32; 3],
    /// 0 — kun, 1 — tun (yulduzlar yorqinligi).
    pub night: f32,
}

/// Bulutli/yomg'irli osmon: ranglar kulrangga, to'g'ridan-to'g'ri quyosh nuri susayadi. `k` — yomg'ir kuchi [0,1].
pub fn overcast(mut s: SkyState, k: f32) -> SkyState {
    let k = k.clamp(0.0, 1.0) * 0.65;
    if k <= 0.0 {
        return s;
    }
    let gray = |c: [f32; 3]| {
        let l = (c[0] + c[1] + c[2]) / 3.0 * 0.82;
        [l, l, l * 1.04]
    };
    s.top = lerp3(s.top, gray(s.top), k);
    s.hor = lerp3(s.hor, gray(s.hor), k);
    s.fog = lerp3(s.fog, gray(s.fog), k);
    s.amb = lerp3(s.amb, gray(s.amb), k * 0.8);
    for c in s.light_col.iter_mut() {
        *c *= 1.0 - 0.55 * k;
    }
    s
}

/// `t` — kun fazasi [0,1); `up` — o'yinchi turgan joyning "tepa" yo'nalishi (sayyora markazidan tashqariga).
/// t=0.25 va up=(0,1,0) bo'lsa — tush.
pub fn state(t: f32, up: V3) -> SkyState {
    let a = t * std::f32::consts::TAU;
    let sun = v3(a.cos() * 0.85, a.sin(), 0.35).norm();
    let e = sun.dot(up);
    let day = smoothstep(-0.08, 0.30, e);
    let set = (1.0 - (e * 3.2).abs()).clamp(0.0, 1.0);
    let top_day = [0.22, 0.52, 0.95];
    let hor_day = [0.70, 0.84, 1.00];
    let top_night = [0.015, 0.020, 0.070];
    let hor_night = [0.060, 0.060, 0.160];
    let top_set = [0.30, 0.17, 0.52];
    let hor_set = [1.00, 0.52, 0.30];
    let mut top = lerp3(top_night, top_day, day);
    let mut hor = lerp3(hor_night, hor_day, day);
    top = lerp3(top, top_set, set * 0.85);
    hor = lerp3(hor, hor_set, set * 0.9);
    let sun_i = smoothstep(-0.03, 0.20, e);
    let warm = lerp3([1.0, 0.55, 0.30], [1.0, 0.96, 0.88], smoothstep(0.05, 0.45, e));
    let moon = [0.22, 0.26, 0.46];
    let (light_dir, light_col) = if e > -0.02 {
        (sun, [warm[0] * sun_i * 1.05, warm[1] * sun_i * 1.05, warm[2] * sun_i * 1.05])
    } else {
        (-sun, moon)
    };
    let amb_night = [0.24, 0.26, 0.44];
    let amb_day = [0.50, 0.58, 0.78];
    let mut amb = lerp3(amb_night, amb_day, day);
    amb = lerp3(amb, [0.62, 0.45, 0.60], set * 0.5);
    let fog = [hor[0] * 0.95, hor[1] * 0.95, hor[2] * 0.97];
    SkyState {
        light_dir,
        light_col,
        sun,
        amb,
        top,
        hor,
        fog,
        night: 1.0 - smoothstep(-0.20, 0.05, e),
    }
}

/// Vaqt tugmalari uchun: kun, kech, tun fazalari.
pub const PHASE_DAY: f32 = 0.18;
pub const PHASE_SUNSET: f32 = 0.455;
pub const PHASE_NIGHT: f32 = 0.78;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tush_yorqin_tun_qorongi() {
        let up = v3(0.0, 1.0, 0.0);
        let d = state(0.25, up);
        let n = state(0.75, up);
        let lum = |c: [f32; 3]| c[0] + c[1] + c[2];
        assert!(lum(d.amb) > lum(n.amb) * 1.5);
        assert!(lum(d.top) > lum(n.top) * 5.0);
        assert!(d.night < 0.01 && n.night > 0.99);
        assert!(d.light_dir.y > 0.9);
        // tunda yorug'lik — oydan (yuqoridan)
        assert!(n.light_dir.y > 0.5, "oy yuqorida bo'lishi kerak: {:?}", n.light_dir);
    }

    #[test]
    fn kechqurun_gorizont_issiq() {
        let s = state(PHASE_SUNSET + 0.03, v3(0.0, 1.0, 0.0));
        assert!(s.hor[0] > s.hor[2], "kechqurun gorizont qizg'ish: {:?}", s.hor);
    }

    #[test]
    fn rangler_oraliqda() {
        for i in 0..200 {
            let s = state(i as f32 / 200.0, v3(0.0, 1.0, 0.0));
            for c in s.top.iter().chain(s.hor.iter()).chain(s.amb.iter()).chain(s.light_col.iter()).chain(s.fog.iter()) {
                assert!((0.0..=1.2).contains(c), "t={} c={}", i, c);
            }
            assert!((s.light_dir.len() - 1.0).abs() < 1e-4);
        }
    }

    #[test]
    fn yomgirda_osmon_xira() {
        let d = state(PHASE_DAY, v3(0.0, 1.0, 0.0));
        let r = overcast(d, 1.0);
        assert!(r.light_col[0] < d.light_col[0] * 0.8);
        assert!((r.top[0] - r.top[2]).abs() < (d.top[0] - d.top[2]).abs(), "kulrangga o'tishi kerak");
        let same = overcast(d, 0.0);
        assert_eq!(same.top, d.top);
    }
}
