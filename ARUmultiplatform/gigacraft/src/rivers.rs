//! Haqiqiy daryolar (Natural Earth 1:50m): segmentlar ro'yxati va 1 daraja katakli indeks.
//! `assets/rivers.bin`: "RIV1", soni (u32), har bir segment: lat0, lon0, lat1, lon1 (f32), daraja (u8).
use crate::math::D3;
use std::collections::HashMap;
use std::sync::OnceLock;

const DATA: &[u8] = include_bytes!("../assets/rivers.bin");
/// 1 daraja = 111.2 km = 444 780 blok.
pub const BLOCKS_PER_DEG: f64 = 444_780.0;

#[derive(Clone, Copy)]
struct Seg {
    la0: f32,
    lo0: f32,
    la1: f32,
    lo1: f32,
    rank: u8,
}

pub struct Rivers {
    segs: Vec<Seg>,
    grid: HashMap<(i32, i32), Vec<u32>>,
}

#[derive(Clone, Copy, Debug)]
pub struct Near {
    /// Daryo o'qigacha masofa (blok).
    pub dist: f64,
    /// Yarim kenglik (blok).
    pub half: f64,
    /// Oqim yo'nalishi (sharq, shimol) birlik vektori.
    pub flow: (f64, f64),
}

pub fn get() -> &'static Rivers {
    static R: OnceLock<Rivers> = OnceLock::new();
    R.get_or_init(|| {
        assert!(&DATA[0..4] == b"RIV1");
        let n = u32::from_le_bytes([DATA[4], DATA[5], DATA[6], DATA[7]]) as usize;
        let mut segs = Vec::with_capacity(n);
        let f = |o: usize| f32::from_le_bytes([DATA[o], DATA[o + 1], DATA[o + 2], DATA[o + 3]]);
        for i in 0..n {
            let o = 8 + i * 17;
            segs.push(Seg { la0: f(o), lo0: f(o + 4), la1: f(o + 8), lo1: f(o + 12), rank: DATA[o + 16] });
        }
        let mut grid: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        for (i, s) in segs.iter().enumerate() {
            let (a, b) = (s.la0.min(s.la1).floor() as i32, s.la0.max(s.la1).floor() as i32);
            let (c, d) = (s.lo0.min(s.lo1).floor() as i32, s.lo0.max(s.lo1).floor() as i32);
            for la in a..=b {
                for lo in c..=d {
                    grid.entry((la, lo)).or_default().push(i as u32);
                }
            }
        }
        Rivers { segs, grid }
    })
}

fn half_width(rank: u8) -> f64 {
    match rank {
        0..=2 => 700.0,
        3..=4 => 400.0,
        5..=6 => 200.0,
        _ => 120.0,
    }
}

impl Rivers {
    pub fn count(&self) -> usize {
        self.segs.len()
    }

    /// Nuqtaga eng yaqin daryo (agar yarim kenglikning 2.4 baravari ichida bo'lsa).
    pub fn near(&self, lat: f64, lon: f64) -> Option<Near> {
        let (la0, lo0) = (lat.floor() as i32, lon.floor() as i32);
        let coslat = lat.to_radians().cos().max(0.05);
        let mut best: Option<Near> = None;
        for dla in -1..=1 {
            for dlo in -1..=1 {
                let key = (la0 + dla, (lo0 + dlo).rem_euclid(360));
                let Some(list) = self.grid.get(&key) else { continue };
                for &i in list {
                    let s = &self.segs[i as usize];
                    // tekis yaqinlashish: x = sharq (daraja * cos lat), y = shimol
                    let (ax, ay) = ((s.lo0 as f64 - lon) * coslat, s.la0 as f64 - lat);
                    let (bx, by) = ((s.lo1 as f64 - lon) * coslat, s.la1 as f64 - lat);
                    let (dx, dy) = (bx - ax, by - ay);
                    let len2 = dx * dx + dy * dy;
                    let t = if len2 < 1e-18 { 0.0 } else { ((-ax * dx - ay * dy) / len2).clamp(0.0, 1.0) };
                    let (px, py) = (ax + dx * t, ay + dy * t);
                    let dist = (px * px + py * py).sqrt() * BLOCKS_PER_DEG;
                    let half = half_width(s.rank);
                    if dist < half * 2.4 && best.map_or(true, |b| dist / half < b.dist / b.half) {
                        let l = len2.sqrt().max(1e-12);
                        best = Some(Near { dist, half, flow: (dx / l, dy / l) });
                    }
                }
            }
        }
        best
    }
}

/// Sharq va shimol birlik vektorlari (dunyo koordinatalarida) berilgan yo'nalishda.
pub fn east_north(d: D3) -> (D3, D3) {
    let lat = d[1].clamp(-1.0, 1.0).asin();
    let lon = (-d[2]).atan2(d[0]);
    ([-lon.sin(), 0.0, -lon.cos()], [-lat.sin() * lon.cos(), lat.cos(), lat.sin() * lon.sin()])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segmentlar_yuklanadi() {
        assert!(get().count() > 10_000);
    }

    #[test]
    fn amudaryo_yaqinida_daryo_bor() {
        // Amudaryo ~ 37.1°N, 67.8°E atrofida (Afg'oniston chegarasi)
        let mut found = false;
        for i in 0..400 {
            let (la, lo) = (36.0 + (i % 20) as f64 * 0.15, 66.0 + (i / 20) as f64 * 0.15);
            if get().near(la, lo).is_some() {
                found = true;
            }
        }
        assert!(found, "Amudaryo topilmadi");
    }

    #[test]
    fn nilni_topadi_va_masofa_hisoblanadi() {
        // Nil ~ 26°N atrofida 32°E
        let mut best = f64::MAX;
        for i in 0..600 {
            let la = 25.0 + (i % 30) as f64 * 0.1;
            let lo = 31.0 + (i / 30) as f64 * 0.1;
            if let Some(n) = get().near(la, lo) {
                best = best.min(n.dist);
            }
        }
        assert!(best < 3000.0, "Nil yaqinida daryo topilmadi: {best}");
    }

    #[test]
    fn sharq_shimol_ortogonal() {
        let d = crate::earth::latlon_dir(41.3, 69.28);
        let (e, n) = east_north(d);
        let dot = |a: D3, b: D3| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        assert!(dot(e, n).abs() < 1e-9 && dot(e, d).abs() < 1e-9 && dot(n, d).abs() < 1e-9);
        // shimol qutbga qarab tepaga (+Y) o'sadi
        assert!(n[1] > 0.5);
    }
}
