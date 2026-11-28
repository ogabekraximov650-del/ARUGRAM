//! Sayyora generatsiyasi: relyef, biomlar, daraxtlar va kristallar. Hammasi yo'nalish (birlik vektor) bo'yicha
//! 3D shovqin va xeshga asoslangan: ixtiyoriy joyni mustaqil va bir xil natija bilan qayta hosil qilish mumkin.
use crate::blocks::*;
use crate::earth;
use crate::math::{smoothstep, D3};
use crate::noise::*;
use crate::planet::*;
use crate::rivers;

pub const CV: usize = 4096;
const R: f64 = SEA_R as f64;
pub const TREE_CELL: i64 = 14;
pub const CRYS_CELL: i64 = 80;
/// Yuz chetidan shuncha blok ichkarida daraxt/kristall qo'yilmaydi (yuz tutashuvi murakkab).
const EDGE_MARGIN: i64 = 40;

#[derive(Clone, Copy)]
pub struct Gen {
    pub seed: u32,
}

/// Bir ustun (16x16 blok) uchun sirt ma'lumoti.
pub struct ColInfo {
    /// Sirt qatlami (eng yuqori qattiq blokning radiusi).
    pub s: Vec<i64>,
    /// Suv sathi qatlami (okeanda — dengiz sathi; ko'l va daryoda — o'z sathi).
    pub w: Vec<i64>,
    /// Sirt turi (GRASS/SAND/SNOW/STONE).
    pub kind: Vec<u8>,
}

impl ColInfo {
    pub fn span(&self) -> (i64, i64) {
        (*self.s.iter().min().unwrap(), *self.s.iter().max().unwrap())
    }

    pub fn wmax(&self) -> i64 {
        *self.w.iter().max().unwrap()
    }
}

/// Relyef namunasi: balandlik, sirt bloki, tur va suv sathi (hammasi dengiz sathiga nisbatan blokda).
#[derive(Clone, Copy, Debug)]
pub struct Terr {
    pub h: i64,
    pub kind: u8,
    pub bio: u8,
    pub wl: i64,
}

#[derive(Clone, Copy, Debug)]
pub struct Tree {
    pub f: u8,
    pub x: i64,
    pub z: i64,
    pub base: i64,
    pub th: i32,
    pub rx: f32,
    pub ry: f32,
    pub hash: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct Spire {
    pub f: u8,
    pub x: i64,
    pub z: i64,
    pub base: i64,
    pub h: i32,
    pub rb: f32,
    pub lean: f32,
}

impl Gen {
    pub fn new(seed: u32) -> Gen {
        Gen { seed }
    }

    fn fb(&self, d: D3, salt: u32, wl: f64, oct: u32) -> f32 {
        fbm3(self.seed.wrapping_add(salt), d[0] * R / wl, d[1] * R / wl, d[2] * R / wl, oct)
    }

    fn polar(&self, d: D3) -> f32 {
        smoothstep(0.82, 0.98, d[1].abs() as f32)
    }

    /// Haqiqiy Yer xaritasi bo'yicha relyef: okean, ko'llar, daryolar, tog'lar, biomlar.
    pub fn terrain(&self, d: D3) -> Terr {
        let sm = earth::get().sample_full(d);
        let (el, bio) = (sm.el, sm.bio);
        let hills = self.fb(d, 101, 1400.0, 4);
        let detail = self.fb(d, 202, 90.0, 3);
        // Ko'l: qirg'oq chizig'i shovqin bilan o'yilgan
        if el > 0.0 && sm.lake > 0.15 && sm.lake + (hills - 0.5) * 0.5 + (detail - 0.5) * 0.15 > 0.5 {
            let wl = (sm.lake_level * 4.0).round() as i64;
            let h = wl - 40 + ((detail - 0.5) * 8.0) as i64;
            return Terr { h, kind: SAND, bio: earth::LAKE, wl };
        }
        let h: f32 = if el <= 0.0 {
            // Okean: sayoz shelf (75 m gacha), chuqur joylar tekis tub
            let depth = (-el * 4.0).min(300.0);
            -depth + (hills - 0.5) * 40.0 * (1.0 - depth / 300.0) + (detail - 0.5) * 4.0
        } else {
            let base = el * 4.0;
            let rough = 0.25 + (el / 1200.0).min(1.5);
            let rid = 1.0 - (2.0 * self.fb(d, 303, 3500.0, 4) - 1.0).abs();
            let mount = smoothstep(400.0, 2500.0, el) * rid * rid * base * 0.3;
            base + (hills - 0.5) * 120.0 * rough + (detail - 0.5) * 16.0 * (0.5 + 0.5 * rough) + mount
        };
        let mut hi = h.floor() as i64;
        let mut wl = 0i64;
        let mut river = false;
        if el > 0.0 && hi > 0 {
            let (lat, lon) = earth::lat_lon(d);
            if let Some(r) = rivers::get().near(lat, lon) {
                let t = r.dist / r.half;
                let f = smoothstep(0.7, 2.4, t as f32) as f64;
                let bank = hi;
                hi = ((bank - 6) as f64 * (1.0 - f) + bank as f64 * f).round() as i64;
                if t < 1.0 {
                    wl = bank - 1;
                    river = true;
                }
                if t < 1.1 {
                    river = true;
                }
            }
        }
        let polar = self.polar(d);
        let elev_m = hi as f32 / 4.0;
        let rock_n = vnoise3(self.seed.wrapping_add(606), d[0] * R / 60.0, d[1] * R / 60.0, d[2] * R / 60.0);
        let kind = if hi < 0 || river {
            SAND
        } else if bio == earth::ICE || elev_m > 4600.0 - 2200.0 * polar {
            SNOW
        } else if elev_m > 2800.0 + (rock_n - 0.5) * 500.0 {
            STONE
        } else if hi <= 2 || bio == earth::DESERT {
            SAND
        } else if bio == earth::SAVANNA {
            DRYGRASS
        } else {
            GRASS
        };
        Terr { h: hi, kind, bio, wl }
    }

    /// Dengiz sathidan balandlik (blokda), yo'nalish bo'yicha uzluksiz.
    pub fn height_dir(&self, d: D3) -> f32 {
        self.terrain(d).h as f32
    }

    /// Daryo kanalidagi oqim yo'nalishi (dunyo koordinatalarida, birlik), kanalda bo'lmasa `None`.
    pub fn river_flow(&self, d: D3) -> Option<D3> {
        let (lat, lon) = earth::lat_lon(d);
        let r = rivers::get().near(lat, lon)?;
        if r.dist >= r.half {
            return None;
        }
        let (e, n) = rivers::east_north(d);
        Some([e[0] * r.flow.0 + n[0] * r.flow.1, e[1] * r.flow.0 + n[1] * r.flow.1, e[2] * r.flow.0 + n[2] * r.flow.1])
    }

    /// Daraxt zichligi [0, 0.6]: tur va mahalliy shovqin bo'yicha.
    pub fn tree_density(&self, d: D3, kind: u8, bio: u8) -> f32 {
        if kind != GRASS && kind != DRYGRASS {
            return 0.0;
        }
        let base = match bio {
            earth::FOREST => 0.55,
            earth::GRASS => 0.14,
            earth::SAVANNA => 0.05,
            earth::TUNDRA => 0.05,
            _ => 0.0,
        };
        (base * (0.35 + 1.3 * self.fb(d, 505, 700.0, 3))).clamp(0.0, 0.6)
    }

    /// Yuz va ustun (cu, cv) uchun sirt ma'lumoti.
    pub fn column(&self, f: u8, cu: i32, cv: i32) -> ColInfo {
        let mut s = vec![0; 256];
        let mut w = vec![0; 256];
        let mut kind = vec![0; 256];
        for lv in 0..16i64 {
            for lu in 0..16i64 {
                let d = dir(f, (cu as i64 * 16 + lu) as f64 + 0.5, (cv as i64 * 16 + lv) as f64 + 0.5);
                let t = self.terrain(d);
                s[(lu + lv * 16) as usize] = SEA_R + t.h;
                w[(lu + lv * 16) as usize] = SEA_R + t.wl;
                kind[(lu + lv * 16) as usize] = t.kind;
            }
        }
        ColInfo { s, w, kind }
    }

    /// Bitta (yuz, u, v) blok ustuning sirt qatlami va turi.
    pub fn surf_at(&self, f: u8, u: i64, v: i64) -> (i64, u8) {
        let t = self.terrain(dir(f, u as f64 + 0.5, v as f64 + 0.5));
        (SEA_R + t.h, t.kind)
    }

    pub fn tree_in_cell(&self, f: u8, cx: i64, cz: i64) -> Option<Tree> {
        let salt = self.seed.wrapping_add(11 + f as u32 * 977);
        let hh = hash2(salt, cx, cz);
        let x = cx * TREE_CELL + (hh % TREE_CELL as u32) as i64;
        let z = cz * TREE_CELL + ((hh >> 8) % TREE_CELL as u32) as i64;
        if x < EDGE_MARGIN || z < EDGE_MARGIN || x >= N - EDGE_MARGIN || z >= N - EDGE_MARGIN {
            return None;
        }
        let d = dir(f, x as f64 + 0.5, z as f64 + 0.5);
        let t = self.terrain(d);
        if t.wl > t.h || unit(hash2(salt.wrapping_add(1), cx, cz)) >= self.tree_density(d, t.kind, t.bio) {
            return None;
        }
        let base = SEA_R + t.h;
        Some(Tree {
            f,
            x,
            z,
            base,
            th: 16 + ((hh >> 16) % 14) as i32,
            rx: 6.0 + ((hh >> 20) & 7) as f32 * 0.6,
            ry: 5.0 + ((hh >> 24) & 3) as f32 * 0.8,
            hash: hh,
        })
    }

    pub fn spires_in_cell(&self, f: u8, cx: i64, cz: i64) -> Vec<Spire> {
        let salt = self.seed.wrapping_add(21 + f as u32 * 977);
        let hh = hash2(salt, cx, cz);
        if unit(hh) >= 0.09 {
            return Vec::new();
        }
        let ox = cx * CRYS_CELL + 12 + ((hh >> 4) % (CRYS_CELL as u32 - 24)) as i64;
        let oz = cz * CRYS_CELL + 12 + ((hh >> 12) % (CRYS_CELL as u32 - 24)) as i64;
        if ox < EDGE_MARGIN || oz < EDGE_MARGIN || ox >= N - EDGE_MARGIN || oz >= N - EDGE_MARGIN {
            return Vec::new();
        }
        if self.surf_at(f, ox, oz).0 - SEA_R <= 3 {
            return Vec::new();
        }
        let n = 2 + (hh >> 20) % 3;
        let mut out = Vec::new();
        for i in 0..n {
            let h2 = hash2(salt.wrapping_add(1 + i), cx, cz);
            let (dx, dz) = if i == 0 { (0, 0) } else { (((h2 % 13) as i64) - 6, (((h2 >> 8) % 13) as i64) - 6) };
            let (x, z) = (ox + dx, oz + dz);
            let r = unit(h2 >> 3);
            out.push(Spire {
                f,
                x,
                z,
                base: self.surf_at(f, x, z).0 - 2,
                h: if i == 0 { 28 + (r * 40.0) as i32 } else { 12 + (r * 20.0) as i32 },
                rb: if i == 0 { 3.6 } else { 2.4 },
                lean: (unit(h2 >> 11) - 0.5) * 0.5,
            });
        }
        out
    }

    /// Bo'lak bloklarini hosil qiladi; `col` — shu ustun sirt ma'lumoti.
    pub fn fill_chunk(&self, cp: CP, col: &ColInfo) -> Vec<u8> {
        let mut b = vec![AIR; CV];
        let (iu0, iv0, r0) = chunk_origin(cp);
        for lv in 0..16usize {
            for lu in 0..16usize {
                let s = col.s[lu + lv * 16];
                let wlay = col.w[lu + lv * 16];
                let surf = col.kind[lu + lv * 16];
                let sub = match surf {
                    GRASS | DRYGRASS => DIRT,
                    SAND => SAND,
                    SNOW => SNOW,
                    _ => STONE,
                };
                let depth = match surf {
                    GRASS | DRYGRASS | SAND => 5,
                    SNOW => 2,
                    _ => 0,
                };
                for lr in 0..16usize {
                    let layer = r0 + lr as i64;
                    let v = if layer > s {
                        if layer <= wlay {
                            WATER
                        } else {
                            AIR
                        }
                    } else if layer == s {
                        surf
                    } else if layer > s - 1 - depth {
                        sub
                    } else {
                        STONE
                    };
                    b[lu + lv * 16 + lr * 256] = v;
                }
            }
        }
        self.decorate(&mut b, cp.f, iu0, iv0, r0);
        b
    }

    fn decorate(&self, b: &mut [u8], f: u8, iu0: i64, iv0: i64, r0: i64) {
        if iu0 < EDGE_MARGIN - 40 || iv0 < EDGE_MARGIN - 40 {
            // yuz chetiga juda yaqin: dekor yo'q
        }
        let fl = |a: i64, c: i64| a.div_euclid(c);
        for cz in fl(iv0 - 12, TREE_CELL)..=fl(iv0 + 15 + 12, TREE_CELL) {
            for cx in fl(iu0 - 12, TREE_CELL)..=fl(iu0 + 15 + 12, TREE_CELL) {
                let Some(t) = self.tree_in_cell(f, cx, cz) else { continue };
                let top = t.base + t.th as i64 + t.ry.ceil() as i64 + 1;
                if top < r0 || t.base > r0 + 15 {
                    continue;
                }
                put_tree(b, &t, iu0, iv0, r0);
            }
        }
        for cz in fl(iv0 - 12, CRYS_CELL)..=fl(iv0 + 15 + 12, CRYS_CELL) {
            for cx in fl(iu0 - 12, CRYS_CELL)..=fl(iu0 + 15 + 12, CRYS_CELL) {
                for s in self.spires_in_cell(f, cx, cz) {
                    if s.base + s.h as i64 + 2 < r0 || s.base > r0 + 15 {
                        continue;
                    }
                    put_spire(b, &s, iu0, iv0, r0);
                }
            }
        }
    }
}

fn put_tree(b: &mut [u8], t: &Tree, iu0: i64, iv0: i64, r0: i64) {
    let cyc = (t.base + t.th as i64) as f32;
    for lr in 0..16i64 {
        let layer = r0 + lr;
        for lv in 0..16i64 {
            for lu in 0..16i64 {
                let (wx, wz) = (iu0 + lu, iv0 + lv);
                let i = (lu + lv * 16 + lr * 256) as usize;
                if b[i] != AIR {
                    continue;
                }
                if layer > t.base && layer <= t.base + t.th as i64 && (wx == t.x || wx == t.x + 1) && (wz == t.z || wz == t.z + 1) {
                    b[i] = LOG;
                    continue;
                }
                let dx = (wx - t.x) as f32 - 0.5;
                let dz = (wz - t.z) as f32 - 0.5;
                let dy = layer as f32 - cyc;
                let e = (dx * dx + dz * dz) / (t.rx * t.rx) + dy * dy / (t.ry * t.ry);
                if e <= 1.0 && hash3(t.hash, wx, layer, wz) & 7 != 0 {
                    b[i] = LEAF;
                }
            }
        }
    }
}

fn put_spire(b: &mut [u8], s: &Spire, iu0: i64, iv0: i64, r0: i64) {
    for lr in 0..16i64 {
        let t = r0 + lr - s.base;
        if t < 0 || t > s.h as i64 {
            continue;
        }
        let tf = t as f32;
        let r = s.rb * (1.0 - tf / s.h as f32) + 0.45;
        let shift = tf * s.lean;
        for lv in 0..16i64 {
            for lu in 0..16i64 {
                let dx = (iu0 + lu - s.x) as f32 - shift;
                let dz = (iv0 + lv - s.z) as f32;
                if dx.abs().max(dz.abs()) <= r {
                    let i = (lu + lv * 16 + lr * 256) as usize;
                    if b[i] == AIR || b[i] == LEAF {
                        b[i] = CRYSTAL;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn some_land(g: &Gen) -> (u8, i64, i64) {
        for i in 0..4000i64 {
            let f = (i % 6) as u8;
            let (u, v) = (EDGE_MARGIN + 1000 + i * 8_000_003 % (N - 4000), EDGE_MARGIN + 1000 + i * 5_000_011 % (N - 4000));
            let (s, k) = g.surf_at(f, u, v);
            if s > SEA_R + 5 && k == GRASS {
                return (f, u, v);
            }
        }
        panic!("quruqlik topilmadi");
    }

    #[test]
    fn balandlik_barqaror_va_oraliqda() {
        let g = Gen::new(7);
        let d = dir(2, 1.0e7, 2.0e7);
        assert_eq!(g.height_dir(d), g.height_dir(d));
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for i in 0..600 {
            let f = (i % 6) as u8;
            let h = g.height_dir(dir(f, (i * 9_999_991 % N) as f64, (i * 7_777_777 % N) as f64));
            lo = lo.min(h);
            hi = hi.max(h);
        }
        assert!(lo >= -312.0 && hi < 40_000.0, "{lo}..{hi}");
        assert!(hi - lo > 100.0, "tekis sayyora: {lo}..{hi}");
    }

    #[test]
    fn yuz_chegarasida_uzluksiz() {
        // Bir-biriga tutashgan nuqtalar (turli yuzlarda) deyarli bir xil balandlik beradi
        let g = Gen::new(5);
        let c = Cell { f: 0, u: N - 1, v: N / 2, r: SEA_R };
        let n = neighbor(&c, 1, 0, 0);
        assert_ne!(n.f, c.f);
        let h0 = g.height_dir(dir(c.f, c.u as f64 + 0.5, c.v as f64 + 0.5));
        let h1 = g.height_dir(dir(n.f, n.u as f64 + 0.5, n.v as f64 + 0.5));
        assert!((h0 - h1).abs() < 12.0, "{h0} {h1}");
    }

    #[test]
    fn ustun_tuzilishi() {
        let g = Gen::new(3);
        let (f, u, v) = some_land(&g);
        let (cu, cv) = ((u / 16) as i32, (v / 16) as i32);
        let col = g.column(f, cu, cv);
        let (lu, lv) = ((u % 16) as usize, (v % 16) as usize);
        let s = col.s[lu + lv * 16];
        let cr = s.div_euclid(16) as i32;
        let b = g.fill_chunk(CP { f, u: cu, v: cv, r: cr }, &col);
        let lr = s.rem_euclid(16) as usize;
        assert!(solid(b[lu + lv * 16 + lr * 256]), "sirt bloki qattiq bo'lishi kerak");
        if lr > 0 {
            assert!(solid(b[lu + lv * 16 + (lr - 1) * 256]));
        }
        let above = g.fill_chunk(CP { f, u: cu, v: cv, r: cr + 400 }, &col);
        assert!(above.iter().all(|&x| x == AIR), "baland havo bo'sh");
    }

    #[test]
    fn suv_dengiz_sathidan_past() {
        let g = Gen::new(11);
        for i in 0..6000i64 {
            let f = (i % 6) as u8;
            let (u, v) = (EDGE_MARGIN + 100 + i * 9_000_001 % (N - 500), EDGE_MARGIN + 100 + i * 6_000_007 % (N - 500));
            let (s, _) = g.surf_at(f, u, v);
            if s < SEA_R - 6 {
                let (cu, cv) = ((u / 16) as i32, (v / 16) as i32);
                let col = g.column(f, cu, cv);
                let layer = SEA_R - 1;
                let b = g.fill_chunk(CP { f, u: cu, v: cv, r: layer.div_euclid(16) as i32 }, &col);
                let (lu, lv, lr) = ((u % 16) as usize, (v % 16) as usize, layer.rem_euclid(16) as usize);
                if col.s[lu + lv * 16] < SEA_R - 1 {
                    assert_eq!(b[lu + lv * 16 + lr * 256], WATER);
                }
                return;
            }
        }
        panic!("dengiz topilmadi");
    }

    fn cell_near(lat: f64, lon: f64, size: i64) -> (u8, i64, i64) {
        let (f, a, b) = dir_to_face(earth::latlon_dir(lat, lon));
        (f, a as i64 / size, b as i64 / size)
    }

    #[test]
    fn daraxt_va_kristall_uchraydi() {
        let g = Gen::new(5);
        let (f, c0, d0) = cell_near(-3.0, 300.0, TREE_CELL); // Amazonka
        let mut trees = 0;
        for cz in d0 - 60..d0 + 60 {
            for cx in c0 - 60..c0 + 60 {
                if g.tree_in_cell(f, cx, cz).is_some() {
                    trees += 1;
                }
            }
        }
        assert!(trees > 100, "Amazonkada daraxt kam: {trees}");
        // cho'lda (Sahara) daraxt yo'q
        let (f2, c2, d2) = cell_near(23.0, 10.0, TREE_CELL);
        let mut dt = 0;
        for cz in d2 - 40..d2 + 40 {
            for cx in c2 - 40..c2 + 40 {
                if g.tree_in_cell(f2, cx, cz).is_some() {
                    dt += 1;
                }
            }
        }
        assert_eq!(dt, 0, "Saharada daraxt bo'lmasligi kerak");
        let (f3, c3, d3) = cell_near(41.3, 69.28, CRYS_CELL);
        let mut crys = 0;
        for cz in d3 - 60..d3 + 60 {
            for cx in c3 - 60..c3 + 60 {
                crys += g.spires_in_cell(f3, cx, cz).len();
            }
        }
        assert!(crys > 3, "kristall kam: {crys}");
    }

    #[test]
    fn daraxt_tanasi_bolakda_bor() {
        let g = Gen::new(5);
        let (f, c0, d0) = cell_near(-3.0, 300.0, TREE_CELL);
        for cz in d0 - 80..d0 + 80 {
            for cx in c0 - 80..c0 + 80 {
                if let Some(t) = g.tree_in_cell(f, cx, cz) {
                    let (cu, cv) = ((t.x / 16) as i32, (t.z / 16) as i32);
                    let cr = (t.base + 3).div_euclid(16) as i32;
                    let mut found = false;
                    for dv in -1..=1 {
                        for du in -1..=1 {
                            let col = g.column(f, cu + du, cv + dv);
                            let b = g.fill_chunk(CP { f, u: cu + du, v: cv + dv, r: cr }, &col);
                            if b.iter().any(|&x| x == LOG) {
                                found = true;
                            }
                        }
                    }
                    assert!(found, "tana topilmadi {:?}", t);
                    return;
                }
            }
        }
        panic!("daraxt yo'q");
    }

    #[test]
    fn haqiqiy_xarita_relyefga_tasir_qiladi() {
        let g = Gen::new(1);
        // Okean tubi tekis (-300), Tinch okeanida
        let t = g.terrain(earth::latlon_dir(0.0, 180.0));
        assert!(t.h <= -250 && t.kind == SAND && t.bio == earth::WATER, "{t:?}");
        // Himolay baland
        let t2 = g.terrain(earth::latlon_dir(28.0, 87.0));
        assert!(t2.h > 8000, "Himolay balandligi {t2:?}");
        // Toshkent: quruq o't (dasht), dengiz sathidan 1000 blokdan baland
        let t3 = g.terrain(earth::latlon_dir(41.3, 69.28));
        assert!(t3.h > 1000 && (t3.kind == DRYGRASS || t3.kind == SAND), "Toshkent {t3:?}");
        // Grenlandiya muzligi — qor
        let t4 = g.terrain(earth::latlon_dir(72.0, 320.0));
        assert_eq!(t4.kind, SNOW);
    }

    #[test]
    fn koller_va_daryolar_suv_bilan_toladi() {
        let g = Gen::new(1);
        // Baykal ko'li: suv sathi quruqlikdan baland, tubi pastda
        let mut lake = None;
        for i in 0..400 {
            let t = g.terrain(earth::latlon_dir(53.0 + (i % 20) as f64 * 0.1, 107.0 + (i / 20) as f64 * 0.1));
            if t.bio == earth::LAKE {
                lake = Some(t);
                break;
            }
        }
        let t = lake.expect("Baykal topilmadi");
        assert!(t.wl > t.h + 20 && t.wl > 800, "{t:?}");
        // Daryo: biror joyda kanal (wl > h) topiladi
        let mut river = 0;
        for i in 0..6000 {
            let d = earth::latlon_dir(36.0 + (i % 100) as f64 * 0.02, 66.0 + (i / 100) as f64 * 0.02);
            let t = g.terrain(d);
            if t.wl > 0 && t.wl > t.h && t.bio != earth::LAKE {
                river += 1;
            }
        }
        assert!(river > 0, "Amudaryo kanali topilmadi");
    }
}
