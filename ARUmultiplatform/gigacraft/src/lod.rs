//! Uzoq relyef: sayyora yuzlari to'rt shoxli daraxt (quadtree) bilan bo'linadi. Yaqinda bloklar (16 blokli ustunlar),
//! uzoqlashgan sari 2 baravar yirik plitalar. Hammasi birgalikda yoriqsiz qoplaydi; tutashgan joylarda "etak" yoriqlarni yopadi.
use crate::blocks::*;
use crate::gen::{Gen, CRYS_CELL, TREE_CELL};
use crate::math::*;
use crate::mesh::{push_quad, quad_normal, rel, vtx, Vertex};
use crate::noise::{hash2, unit as unit01};
use crate::planet::*;

/// Plita tomoni (blok): 16 << level.
pub fn tile_size(level: u32) -> i64 {
    16i64 << level
}

/// Plitadagi kataklar soni (bir tomonda).
pub fn cells(level: u32) -> usize {
    if level <= 2 || level >= 12 {
        16
    } else {
        8
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Leaf {
    pub f: u8,
    pub l: u32,
    pub tu: i64,
    pub tv: i64,
}

/// Sifat bo'yicha daraxt bo'linish koeffitsientlari: (0-daraja ustunlar uchun, plitalar uchun).
pub const K_LOD: [(f64, f64); 3] = [(4.0, 2.4), (6.0, 3.2), (8.0, 4.0)];

/// Kamera (dunyo koordinatalari) uchun kerakli barg tugunlar va ularning masofasi.
pub fn leaves(cam: D3, k1: f64, k2: f64) -> Vec<(Leaf, f64)> {
    let cam_r = d_len(cam);
    let cam_dir = d_mul(cam, 1.0 / cam_r);
    let sea = SEA_R as f64;
    let horizon = if cam_r > sea { (sea / cam_r).acos() } else { 0.0 } + 40000.0 / sea;
    let mut out = Vec::new();
    let mut stack: Vec<Leaf> = (0..6u8).map(|f| Leaf { f, l: LMAX, tu: 0, tv: 0 }).collect();
    while let Some(n) = stack.pop() {
        let s = tile_size(n.l) as f64;
        let cd = dir(n.f, (n.tu as f64 + 0.5) * s, (n.tv as f64 + 0.5) * s);
        let theta = d_dot(cam_dir, cd).clamp(-1.0, 1.0).acos();
        let rho = s * 0.75 / sea;
        if theta - rho > horizon {
            continue;
        }
        let dist = d_len(d_sub(d_mul(cd, sea), cam));
        let k = if n.l == 1 { k1 } else { k2 };
        if n.l > 0 && dist < s * k {
            for (du, dv) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                stack.push(Leaf { f: n.f, l: n.l - 1, tu: n.tu * 2 + du, tv: n.tv * 2 + dv });
            }
        } else {
            out.push((n, dist));
        }
    }
    out
}

const WATER_SHALLOW: [f32; 3] = [0.25, 0.62, 0.90];
const WATER_DEEP: [f32; 3] = [0.07, 0.20, 0.50];

/// Uzoqdan ko'rinadigan sirt rangi (h — dengiz sathidan butun balandlik, kind — sirt bloki, bio — iqlim turi).
#[allow(clippy::too_many_arguments)]
pub fn terrain_color(g: &Gen, d: D3, h: i64, wl: i64, kind: u8, bio: u8, a: i64, b: i64, cell: i64) -> [f32; 3] {
    let var = 0.97 + 0.06 * unit01(hash2(5, a.div_euclid(cell), b.div_euclid(cell)));
    if h < wl {
        let depth = (wl - h) as f32;
        if depth < 5.0 {
            return lerp3(color(SAND, 0), WATER_SHALLOW, depth / 5.0);
        }
        return lerp3(WATER_SHALLOW, WATER_DEEP, smoothstep(0.0, 300.0, depth));
    }
    let mut c = color(kind, 0);
    if kind == GRASS || kind == DRYGRASS {
        let t = (g.tree_density(d, kind, bio) / 0.55).min(1.0) * 0.6;
        c = lerp3(c, color(LEAF, 0), t);
    }
    [c[0] * var, c[1] * var, c[2] * var]
}

/// Qutini chizadi (pastki yuzsiz): uzoqdagi daraxt va kristallar uchun. a,b — yuz indekslari, r — radius.
#[allow(clippy::too_many_arguments)]
fn push_box(out: &mut Vec<Vec<Vertex>>, f: u8, anchor: D3, ab: [f64; 4], r: [f64; 2], col: [f32; 3], emis: bool) {
    let [a0, a1, b0, b1] = ab;
    let [r0, r1] = r;
    let pt = |a: f64, b: f64, rr: f64| d_mul(dir(f, a, b), rr);
    let inside = pt((a0 + a1) * 0.5, (b0 + b1) * 0.5, (r0 + r1) * 0.5);
    let faces: [[D3; 4]; 5] = [
        [pt(a0, b0, r1), pt(a1, b0, r1), pt(a1, b1, r1), pt(a0, b1, r1)],
        [pt(a0, b0, r0), pt(a1, b0, r0), pt(a1, b0, r1), pt(a0, b0, r1)],
        [pt(a0, b1, r0), pt(a1, b1, r0), pt(a1, b1, r1), pt(a0, b1, r1)],
        [pt(a0, b0, r0), pt(a0, b1, r0), pt(a0, b1, r1), pt(a0, b0, r1)],
        [pt(a1, b0, r0), pt(a1, b1, r0), pt(a1, b1, r1), pt(a1, b0, r1)],
    ];
    for q in faces {
        let n = quad_normal(q, inside);
        push_quad(out, [vtx(rel(q[0], anchor), col, 1.0, n, emis), vtx(rel(q[1], anchor), col, 1.0, n, emis), vtx(rel(q[2], anchor), col, 1.0, n, emis), vtx(rel(q[3], anchor), col, 1.0, n, emis)]);
    }
}

/// Uzoq plitaga daraxt va kristallar qo'shadi (daraja oshgani sari kattalashtirilgan, siyrakroq).
fn add_features(g: &Gen, lf: Leaf, anchor: D3, out: &mut Vec<Vec<Vertex>>) {
    let size = tile_size(lf.l);
    let (iu0, iv0) = (lf.tu * size, lf.tv * size);
    let level = lf.l;
    let k = 1.0 + (level as f32 - 1.0) * 0.45;
    let keep_mask = (1u32 << (level - 1).min(3)) - 1;
    let kf = k as f64;
    for cz in iv0.div_euclid(TREE_CELL)..=(iv0 + size).div_euclid(TREE_CELL) {
        for cx in iu0.div_euclid(TREE_CELL)..=(iu0 + size).div_euclid(TREE_CELL) {
            let Some(t) = g.tree_in_cell(lf.f, cx, cz) else { continue };
            if t.x < iu0 || t.x >= iu0 + size || t.z < iv0 || t.z >= iv0 + size || (t.hash >> 6) & keep_mask != 0 {
                continue;
            }
            let (x, z) = (t.x as f64, t.z as f64);
            let y = (t.base + 1) as f64;
            let th = t.th as f64 * kf;
            let (rx, ry) = (t.rx as f64 * kf, t.ry as f64 * kf);
            let tw = kf.max(1.0);
            let var = 0.9 + 0.2 * unit01(t.hash >> 3);
            let lc = color(LEAF, 0);
            let lcol = [lc[0] * var, lc[1] * var, lc[2] * var];
            push_box(out, lf.f, anchor, [x - tw, x + tw + 1.0, z - tw, z + tw + 1.0], [y, y + th], color(LOG, 1), false);
            push_box(out, lf.f, anchor, [x - rx, x + rx + 1.0, z - rx, z + rx + 1.0], [y + th - ry, y + th + ry], lcol, false);
            let (r2, h2) = (rx * 0.6, ry * 0.7);
            push_box(out, lf.f, anchor, [x - r2, x + r2 + 1.0, z - r2, z + r2 + 1.0], [y + th + ry - 1.0, y + th + ry + h2], lcol, false);
        }
    }
    if level <= 4 {
        for cz in (iv0 - 12).div_euclid(CRYS_CELL)..=(iv0 + size + 12).div_euclid(CRYS_CELL) {
            for cx in (iu0 - 12).div_euclid(CRYS_CELL)..=(iu0 + size + 12).div_euclid(CRYS_CELL) {
                for s in g.spires_in_cell(lf.f, cx, cz) {
                    if s.x < iu0 || s.x >= iu0 + size || s.z < iv0 || s.z >= iv0 + size {
                        continue;
                    }
                    let (x, z) = (s.x as f64, s.z as f64);
                    let y = s.base as f64;
                    let h = s.h as f64;
                    let w = s.rb as f64 * 0.55 * kf;
                    let c = color(CRYSTAL, 0);
                    push_box(out, lf.f, anchor, [x - w, x + w + 1.0, z - w, z + w + 1.0], [y, y + h * 0.65], c, true);
                    push_box(out, lf.f, anchor, [x - w * 0.5, x + w * 0.5 + 1.0, z - w * 0.5, z + w * 0.5 + 1.0], [y + h * 0.65, y + h], c, true);
                }
            }
        }
    }
}

/// Plita meshi (to'rtburchaklar ro'yxati). Cho'qqilar `tile_anchor` ga nisbatan.
pub fn build_tile(g: &Gen, lf: Leaf) -> Vec<Vertex> {
    let n = cells(lf.l);
    let size = tile_size(lf.l);
    let cell = size / n as i64;
    let (iu0, iv0) = (lf.tu * size, lf.tv * size);
    let anchor = tile_anchor(lf.f, lf.l, lf.tu, lf.tv);
    let m = n + 2;
    // Har bir katak — bitta "ustun" (yaqindagi bloklar kabi pog'onali). Atrofida bir qator qo'shimcha katak.
    let mut top = vec![0f64; m * m];
    let mut hint = vec![0i64; m * m];
    let mut wls = vec![0i64; m * m];
    let mut kinds = vec![0u8; m * m];
    let mut bios = vec![0u8; m * m];
    for j in 0..m {
        for i in 0..m {
            let a = (iu0 + (i as i64 - 1) * cell) as f64 + cell as f64 * 0.5;
            let b = (iv0 + (j as i64 - 1) * cell) as f64 + cell as f64 * 0.5;
            let t = g.terrain(dir(lf.f, a, b));
            hint[j * m + i] = t.h;
            wls[j * m + i] = t.wl;
            kinds[j * m + i] = t.kind;
            bios[j * m + i] = t.bio;
            // Yaqindagi bloklar bilan bir xil: quruqlik tepasi h+1, suv yuzasi +0.9
            top[j * m + i] = SEA_R as f64 + if t.h < t.wl { t.wl as f64 + 0.9 } else { t.h as f64 + 1.0 };
        }
    }
    let skirt = cell as f64 * 3.0 + 6.0;
    let mut out: Vec<Vec<Vertex>> = Vec::new();
    let pt = |a: i64, b: i64, r: f64| d_mul(dir(lf.f, a as f64, b as f64), r);
    for cj in 1..=n {
        for ci in 1..=n {
            let idx = cj * m + ci;
            let (t, h, wl) = (top[idx], hint[idx], wls[idx]);
            let (a0, b0) = (iu0 + (ci as i64 - 1) * cell, iv0 + (cj as i64 - 1) * cell);
            let (a1, b1) = (a0 + cell, b0 + cell);
            let (ca, cb) = (a0 as f64 + cell as f64 * 0.5, b0 as f64 + cell as f64 * 0.5);
            let dc = dir(lf.f, ca, cb);
            let (kd, bo) = (kinds[idx], bios[idx]);
            let tcol = terrain_color(g, dc, h, wl, kd, bo, a0, b0, cell);
            let scol = if h < wl {
                tcol
            } else {
                let c = match kd {
                    GRASS | DRYGRASS => color(GRASS, 1),
                    SAND => color(SAND, 0),
                    SNOW => color(SNOW, 0),
                    _ => color(STONE, 0),
                };
                [c[0] * 0.95, c[1] * 0.95, c[2] * 0.95]
            };
            let q = [pt(a0, b0, t), pt(a1, b0, t), pt(a1, b1, t), pt(a0, b1, t)];
            let up = [dc[0] as f32, dc[1] as f32, dc[2] as f32];
            push_quad(&mut out, [vtx(rel(q[0], anchor), tcol, 1.0, up, false), vtx(rel(q[1], anchor), tcol, 1.0, up, false), vtx(rel(q[2], anchor), tcol, 1.0, up, false), vtx(rel(q[3], anchor), tcol, 1.0, up, false)]);
            let inside = d_mul(dc, t - 1.0);
            // (qo'shni siljish, chekka nuqtalari)
            let dirs: [(i32, i32, (i64, i64), (i64, i64)); 4] = [(-1, 0, (a0, b0), (a0, b1)), (1, 0, (a1, b0), (a1, b1)), (0, -1, (a0, b0), (a1, b0)), (0, 1, (a0, b1), (a1, b1))];
            for (di, dj, e0, e1) in dirs {
                let nt = top[(cj as i32 + dj) as usize * m + (ci as i32 + di) as usize];
                let at_edge = (di == -1 && ci == 1) || (di == 1 && ci == n) || (dj == -1 && cj == 1) || (dj == 1 && cj == n);
                let mut low = if t > nt + 0.01 {
                    nt
                } else if at_edge {
                    t - skirt
                } else {
                    continue;
                };
                if at_edge {
                    low = low.min(t - skirt);
                }
                let w = [pt(e0.0, e0.1, t), pt(e1.0, e1.1, t), pt(e1.0, e1.1, low), pt(e0.0, e0.1, low)];
                let nrm = quad_normal(w, inside);
                push_quad(&mut out, [vtx(rel(w[0], anchor), scol, 1.0, nrm, false), vtx(rel(w[1], anchor), scol, 1.0, nrm, false), vtx(rel(w[2], anchor), scol, 1.0, nrm, false), vtx(rel(w[3], anchor), scol, 1.0, nrm, false)]);
            }
        }
    }
    if lf.l <= 6 {
        add_features(g, lf, anchor, &mut out);
    }
    out.into_iter().flatten().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cam_at(f: u8, u: f64, v: f64, alt: f64) -> D3 {
        d_mul(dir(f, u, v), SEA_R as f64 + alt)
    }

    /// Kameradan yaqin nuqtalarning har biri aynan bitta bargda yotadi (teshik ham, ustma-ust tushish ham yo'q).
    #[test]
    fn daraxt_aniq_bir_marta_qoplaydi() {
        let (u0, v0) = (20_000_123.0, 31_000_456.0);
        let cam = cam_at(2, u0, v0, 8.0);
        let lv = leaves(cam, 6.0, 3.2);
        let mut seen = 0;
        for du in (-3000..3000).step_by(137) {
            for dv in (-3000..3000).step_by(149) {
                let (iu, iv) = ((u0 as i64) + du, (v0 as i64) + dv);
                let hits = lv.iter().filter(|(l, _)| l.f == 2 && iu >= l.tu * tile_size(l.l) && iu < (l.tu + 1) * tile_size(l.l) && iv >= l.tv * tile_size(l.l) && iv < (l.tv + 1) * tile_size(l.l)).count();
                assert_eq!(hits, 1, "nuqta ({iu},{iv}) {hits} marta");
                seen += 1;
            }
        }
        assert!(seen > 500);
    }

    #[test]
    fn eng_yaqin_barglar_ustun_va_yirik_uzoqda() {
        let cam = cam_at(0, 30_000_000.0, 30_000_000.0, 8.0);
        let lv = leaves(cam, 6.0, 3.2);
        assert!(lv.iter().any(|(l, _)| l.l == 0), "ustunlar yo'q");
        let (mut near, mut far) = (u32::MAX, 0);
        for (l, d) in &lv {
            if *d < 100.0 {
                near = near.min(l.l);
            }
            if *d > 4000.0 {
                far = far.max(l.l);
            }
        }
        assert_eq!(near, 0);
        assert!(far >= 5, "uzoqda yirik plita yo'q: {far}");
        assert!(lv.len() > 300 && lv.len() < 4000, "bargalar soni {}", lv.len());
        let cols = lv.iter().filter(|(l, _)| l.l == 0).count();
        assert!(cols > 200 && cols < 1500, "ustunlar {cols}");
    }

    #[test]
    fn kosmosdan_butun_sayyora() {
        let cam = cam_at(4, 33_000_000.0, 33_000_000.0, 30_000_000.0);
        let lv = leaves(cam, 6.0, 3.2);
        assert!(lv.len() > 20 && lv.len() < 3000, "{}", lv.len());
        // kamera tomonidagi yarim sharda yuzlar bor
        assert!(lv.iter().any(|(l, _)| l.f == 4));
        // orqa tomon (qarama-qarshi yuz) ko'rinmaydi
        assert!(!lv.iter().any(|(l, _)| l.f == 5), "orqa yuz chizilmasligi kerak");
    }

    #[test]
    fn plita_cheklangan_hajmda() {
        let g = Gen::new(2);
        for l in [1u32, 2, 3, 6, 14] {
            let lf = Leaf { f: 1, l, tu: 3, tv: 2 };
            let v = build_tile(&g, lf);
            let n = cells(l);
            assert!(v.len() >= n * n * 4);
            assert!(v.len() <= 4 * crate::mesh::MAX_VERTS);
            for p in &v {
                assert!(p.p.iter().all(|c| c.is_finite()));
            }
        }
    }

    #[test]
    fn tepalar_tekis_devorlar_tik() {
        let g = Gen::new(8);
        let lf = Leaf { f: 2, l: 2, tu: 300_000, tv: 400_000 };
        let v = build_tile(&g, lf);
        let n = cells(2);
        // Katak tepalari: radial normal bilan, to'rt cho'qqi anchor dan bir xil uzoqlikda emas, lekin bir xil radiusda
        let anchor = tile_anchor(lf.f, lf.l, lf.tu, lf.tv);
        let mut tops = 0;
        let mut walls = 0;
        for q in v.chunks_exact(4) {
            let r: Vec<f64> = q.iter().map(|c| d_len(d_add(anchor, [c.p[0] as f64, c.p[1] as f64, c.p[2] as f64]))).collect();
            let flat = r.iter().all(|x| (x - r[0]).abs() < 0.05);
            let up = anchor;
            let nd = [q[0].n[0] as f64, q[0].n[1] as f64, q[0].n[2] as f64];
            let radial = d_dot(d_norm(nd), d_norm(up)) > 0.9;
            if flat && radial {
                tops += 1;
            } else if !radial {
                walls += 1;
            }
        }
        assert!(tops >= n * n, "tepalar kam: {tops}");
        assert!(walls > 0, "devor yo'q (pog'ona va etak bo'lishi kerak)");
    }

    #[test]
    fn okean_ko_k() {
        let g = Gen::new(11);
        for i in 0..6000i64 {
            let f = (i % 6) as u8;
            let (u, v) = (3000 + i * 9_000_001 % (N - 6000), 3000 + i * 6_000_007 % (N - 6000));
            let d = dir(f, u as f64 + 0.5, v as f64 + 0.5);
            let t = g.terrain(d);
            if t.h < -80 {
                let c = terrain_color(&g, d, t.h, t.wl, t.kind, t.bio, u, v, 4);
                assert!(c[2] > c[0], "okean ko'k bo'lishi kerak: {:?}", c);
                return;
            }
        }
    }
}
