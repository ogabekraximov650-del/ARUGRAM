//! Bo'lak (16x16x16) meshi: faqat havoga ochiq yuzlar chiziladi (yer ostidagi yopiq yuzlar yo'q), burchaklarda AO.
//! Cho'qqilar sayyora egriligi bo'yicha joylashadi (kub-shar), berilgan `anchor` nuqtasiga nisbatan.
use crate::blocks::*;
use crate::math::*;
use crate::noise::{hash3, unit};
use crate::planet::*;
use crate::world::World;

/// Cho'qqi: joy (f32x3), rang+AO (u8x4), normal+yorug'lik bayrog'i (i8x4). Jami 20 bayt.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vertex {
    pub p: [f32; 3],
    pub c: [u8; 4],
    pub n: [i8; 4],
}

/// Bitta meshdagi cho'qqilar chegarasi (u16 indeks): to'rtburchak soni 16383 dan oshmasin.
pub const MAX_VERTS: usize = 65532;

#[derive(Default)]
pub struct MeshData {
    pub opaque: Vec<Vec<Vertex>>,
    pub water: Vec<Vec<Vertex>>,
}

impl MeshData {
    pub fn is_empty(&self) -> bool {
        self.opaque.is_empty() && self.water.is_empty()
    }
    pub fn quads(&self) -> usize {
        self.opaque.iter().chain(self.water.iter()).map(|v| v.len() / 4).sum()
    }
}

pub fn push_quad(parts: &mut Vec<Vec<Vertex>>, q: [Vertex; 4]) {
    if parts.last().map_or(true, |p| p.len() + 4 > MAX_VERTS) {
        parts.push(Vec::with_capacity(4096));
    }
    parts.last_mut().unwrap().extend_from_slice(&q);
}

pub fn vtx(p: [f32; 3], col: [f32; 3], ao: f32, n: [f32; 3], emis: bool) -> Vertex {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    let s = |v: f32| (v.clamp(-1.0, 1.0) * 127.0).round() as i8;
    Vertex {
        p,
        c: [q(col[0]), q(col[1]), q(col[2]), q(ao)],
        n: [s(n[0]), s(n[1]), s(n[2]), if emis { 127 } else { 0 }],
    }
}

/// Dunyo nuqtasini `anchor` ga nisbatan f32 ga aylantiradi.
pub fn rel(p: D3, anchor: D3) -> [f32; 3] {
    let d = d_sub(p, anchor);
    [d[0] as f32, d[1] as f32, d[2] as f32]
}

/// To'rtburchak normali (geometriyadan), `inside` nuqtadan uzoqqa qaragan.
pub fn quad_normal(q: [D3; 4], inside: D3) -> [f32; 3] {
    let n = d_norm(d_cross(d_sub(q[1], q[0]), d_sub(q[3], q[0])));
    let mid = d_mul(d_add(d_add(q[0], q[1]), d_add(q[2], q[3])), 0.25);
    let n = if d_dot(n, d_sub(mid, inside)) < 0.0 { d_mul(n, -1.0) } else { n };
    [n[0] as f32, n[1] as f32, n[2] as f32]
}

const AO_LEVEL: [f32; 4] = [0.50, 0.68, 0.84, 1.0];

/// Bo'lakni `out` ga mesh qiladi (cho'qqilar `anchor` ga nisbatan). Qo'shni bo'laklar kerak bo'lsa avtomatik hosil qilinadi.
pub fn mesh_chunk_into(world: &World, cp: CP, anchor: D3, out: &mut MeshData) {
    let p = world.padded(cp);
    let mut any = false;
    'scan: for y in 0..16usize {
        for z in 0..16usize {
            for x in 0..16usize {
                if p[(x + 1) + (z + 1) * 18 + (y + 1) * 324] != AIR {
                    any = true;
                    break 'scan;
                }
            }
        }
    }
    if !any {
        return;
    }
    let at = |x: i32, y: i32, z: i32| -> u8 { p[(x + 1) as usize + (z + 1) as usize * 18 + (y + 1) as usize * 324] };
    let (iu0, iv0, r0) = chunk_origin(cp);
    // Panjara yo'nalishlari (u, v bo'yicha 17x17)
    let mut dirs = vec![[0.0f64; 3]; 17 * 17];
    for z in 0..17i64 {
        for x in 0..17i64 {
            dirs[(x + z * 17) as usize] = dir(cp.f, (iu0 + x) as f64, (iv0 + z) as f64);
        }
    }
    let world_pos = |x: i32, y: f64, z: i32| -> [f32; 3] { rel(d_mul(dirs[(x + z * 17) as usize], r0 as f64 + y), anchor) };
    // Yuz normallari: bo'lak markazidagi asos
    let (ca, cb) = ((iu0 + 8) as f64, (iv0 + 8) as f64);
    let up = dir(cp.f, ca, cb);
    let tu = d_norm(d_sub(dir(cp.f, ca + 1.0, cb), dir(cp.f, ca - 1.0, cb)));
    let tv = d_norm(d_sub(dir(cp.f, ca, cb + 1.0), dir(cp.f, ca, cb - 1.0)));
    let basis = [tu, up, tv];
    for y in 0..16i32 {
        for z in 0..16i32 {
            for x in 0..16i32 {
                let b = at(x, y, z);
                if b == AIR {
                    continue;
                }
                let pos = [x, y, z];
                for face in 0..6usize {
                    let axis = face / 2;
                    let sign: i32 = if face % 2 == 0 { 1 } else { -1 };
                    let mut d = [0i32; 3];
                    d[axis] = sign;
                    let nb = at(x + d[0], y + d[1], z + d[2]);
                    let visible = if b == WATER { nb == AIR } else { nb == AIR || nb == WATER };
                    if !visible {
                        continue;
                    }
                    let (ua, va) = ((axis + 1) % 3, (axis + 2) % 3);
                    let kind = if axis == 1 {
                        if sign > 0 {
                            0
                        } else {
                            2
                        }
                    } else {
                        1
                    };
                    let nd = d_mul(basis[axis], sign as f64);
                    let nrm = [nd[0] as f32, nd[1] as f32, nd[2] as f32];
                    let emis = emissive(b);
                    let var = 0.92 + 0.16 * unit(hash3(7, iu0 + x as i64, r0 + y as i64, iv0 + z as i64));
                    let col = {
                        let c = color(b, kind);
                        if emis {
                            [c[0], c[1], c[2]]
                        } else {
                            [c[0] * var, c[1] * var, c[2] * var]
                        }
                    };
                    let a = [x + d[0], y + d[1], z + d[2]];
                    let mut ao = [3usize; 4];
                    if b != WATER && !emis {
                        for (ci, (cu, cv)) in [(0, 0), (1, 0), (1, 1), (0, 1)].iter().enumerate() {
                            let du = *cu * 2 - 1;
                            let dv = *cv * 2 - 1;
                            let mut s1p = a;
                            s1p[ua] += du;
                            let mut s2p = a;
                            s2p[va] += dv;
                            let mut cp2 = a;
                            cp2[ua] += du;
                            cp2[va] += dv;
                            let s1 = solid(at(s1p[0], s1p[1], s1p[2])) as usize;
                            let s2 = solid(at(s2p[0], s2p[1], s2p[2])) as usize;
                            let c = solid(at(cp2[0], cp2[1], cp2[2])) as usize;
                            ao[ci] = if s1 == 1 && s2 == 1 { 0 } else { 3 - (s1 + s2 + c) };
                        }
                    }
                    let mut vs = [Vertex::default(); 4];
                    for (ci, (cu, cv)) in [(0usize, 0usize), (1, 0), (1, 1), (0, 1)].iter().enumerate() {
                        let mut pp = [pos[0], pos[1], pos[2]];
                        pp[axis] += if sign > 0 { 1 } else { 0 };
                        pp[ua] += *cu as i32;
                        pp[va] += *cv as i32;
                        let mut ry = pp[1] as f64;
                        if b == WATER && axis == 1 && sign > 0 {
                            ry -= 0.1;
                        }
                        let br = if emis { 1.0 } else { AO_LEVEL[ao[ci]] };
                        vs[ci] = vtx(world_pos(pp[0], ry, pp[2]), col, br, nrm, emis);
                    }
                    let flip = ao[0] + ao[2] < ao[1] + ao[3];
                    let q = if flip { [vs[1], vs[2], vs[3], vs[0]] } else { vs };
                    if b == WATER {
                        push_quad(&mut out.water, q);
                    } else {
                        push_quad(&mut out.opaque, q);
                    }
                }
            }
        }
    }
}

/// Bo'lakni o'z markaziga nisbatan mesh qiladi.
pub fn mesh_chunk(world: &World, cp: CP) -> MeshData {
    let (iu0, iv0, r0) = chunk_origin(cp);
    let anchor = corner(cp.f, iu0 + 8, iv0 + 8, (r0 + 8) as f64);
    let mut m = MeshData::default();
    mesh_chunk_into(world, cp, anchor, &mut m);
    m
}

/// Butun ustunni (barcha radial bo'laklar) bitta mesh qiladi; cho'qqilar `col_anchor` ga nisbatan.
pub fn mesh_column(world: &World, f: u8, cu: i32, cv: i32) -> MeshData {
    let (c0, c1) = world.section_range(f, cu, cv);
    let anchor = col_anchor(f, cu, cv);
    let mut out = MeshData::default();
    for cr in c0..=c1 {
        mesh_chunk_into(world, CP { f, u: cu, v: cv, r: cr }, anchor, &mut out);
    }
    out
}

/// Meshdagi chegaraviy qutilar (min, max).
pub fn bounds(m: &MeshData) -> ([f32; 3], [f32; 3]) {
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for part in m.opaque.iter().chain(m.water.iter()) {
        for v in part {
            for a in 0..3 {
                lo[a] = lo[a].min(v.p[a]);
                hi[a] = hi[a].max(v.p[a]);
            }
        }
    }
    if lo[0] > hi[0] {
        ([0.0; 3], [0.0; 3])
    } else {
        (lo, hi)
    }
}

/// Bitta blokni o'rab turuvchi (biroz kattalashtirilgan) quti: tanlangan blokni ko'rsatish uchun. (boshlanish nuqtasi, cho'qqilar)
pub fn cell_box(c: &Cell) -> (D3, Vec<Vertex>) {
    let e = 0.012;
    let anchor = center(c);
    let (a0, a1) = (c.u as f64 - e, c.u as f64 + 1.0 + e);
    let (b0, b1) = (c.v as f64 - e, c.v as f64 + 1.0 + e);
    let (r0, r1) = (c.r as f64 - e, c.r as f64 + 1.0 + e);
    let pt = |a: f64, b: f64, r: f64| d_mul(dir(c.f, a, b), r);
    let mut out = Vec::with_capacity(24);
    // 6 yuz: har biri (a,b,r) ning bittasi qat'iy, qolgan ikkitasi o'zgaradi
    let corners = |fixed: usize, val: [f64; 2]| -> [D3; 4] {
        let side = |i: usize, k: usize| -> f64 {
            // i: 0 -> a, 1 -> b, 2 -> r; k: 0 -> past, 1 -> yuqori
            match (i, k) {
                (0, 0) => a0,
                (0, _) => a1,
                (1, 0) => b0,
                (1, _) => b1,
                (_, 0) => r0,
                _ => r1,
            }
        };
        let (i1, i2) = ((fixed + 1) % 3, (fixed + 2) % 3);
        let mut q = [[0.0; 3]; 4];
        for (n, (k1, k2)) in [(0, 0), (1, 0), (1, 1), (0, 1)].iter().enumerate() {
            let mut abr = [0.0f64; 3];
            abr[fixed] = val[0];
            abr[i1] = side(i1, *k1);
            abr[i2] = side(i2, *k2);
            q[n] = pt(abr[0], abr[1], abr[2]);
        }
        q
    };
    for fixed in 0..3usize {
        for hi in 0..2usize {
            let v = match (fixed, hi) {
                (0, 0) => a0,
                (0, _) => a1,
                (1, 0) => b0,
                (1, _) => b1,
                (_, 0) => r0,
                _ => r1,
            };
            let q = corners(fixed, [v, 0.0]);
            let n = quad_normal(q, anchor);
            for p in q {
                out.push(Vertex { p: rel(p, anchor), c: [255, 255, 255, 255], n: [(n[0] * 127.0) as i8, (n[1] * 127.0) as i8, (n[2] * 127.0) as i8, 127] });
            }
        }
    }
    (anchor, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HIGH: i64 = SEA_R + 9000;

    fn cube_chunk(f: u8, u: i64, v: i64) -> (World, CP) {
        let w = World::new(1);
        (w, CP { f, u: (u / 16) as i32, v: (v / 16) as i32, r: (HIGH / 16) as i32 })
    }

    fn cell(f: u8, u: i64, v: i64, dr: i64) -> Cell {
        Cell { f, u, v, r: (HIGH / 16) * 16 + dr }
    }

    fn land_chunk(w: &World) -> CP {
        for i in 0..4000i64 {
            let f = (i % 6) as u8;
            let (u, v) = (3000 + i * 8_000_003 % (N - 6000), 3000 + i * 5_000_011 % (N - 6000));
            let (s, k) = w.gen.surf_at(f, u, v);
            if s > SEA_R + 5 && k == GRASS {
                return CP { f, u: (u / 16) as i32, v: (v / 16) as i32, r: (s / 16) as i32 };
            }
        }
        panic!("quruqlik yo'q");
    }

    #[test]
    fn bosh_havo_bolagi_bosh() {
        let (w, cp) = cube_chunk(0, 30_000_000, 30_000_000);
        assert!(mesh_chunk(&w, cp).is_empty());
    }

    #[test]
    fn toliq_qattiq_bolak_ichida_yuz_yoq() {
        let w = World::new(1);
        let m = mesh_chunk(&w, CP { f: 0, u: 100_000, v: 100_000, r: ((SEA_R - 3000) / 16) as i32 });
        assert!(m.is_empty(), "yopiq tosh bo'lagida yuz bo'lmasligi kerak");
    }

    #[test]
    fn sirt_bolagi_faqat_ochiq_yuzlar() {
        let w = World::new(3);
        let cp = land_chunk(&w);
        let m = mesh_chunk(&w, cp);
        assert!(m.quads() > 100, "sirt kam: {}", m.quads());
        assert!(m.quads() < 6000, "ortiqcha yuz: {}", m.quads());
        for part in m.opaque.iter().chain(m.water.iter()) {
            assert_eq!(part.len() % 4, 0);
            assert!(part.len() <= MAX_VERTS);
            for v in part {
                for c in v.p {
                    assert!(c.abs() < 40.0, "bo'lakdan tashqarida: {:?}", v.p);
                }
            }
        }
    }

    #[test]
    fn yolgiz_blok_olti_yuz() {
        let (w, cp) = cube_chunk(0, 30_000_000, 30_000_000);
        w.set_block(&cell(0, 30_000_005, 30_000_005, 5), STONE);
        let m = mesh_chunk(&w, cp);
        assert_eq!(m.quads(), 6);
        for v in &m.opaque[0] {
            assert_eq!(v.c[3], 255, "yolg'iz blok to'liq yorug'");
        }
    }

    #[test]
    fn ikki_blok_orasidagi_yuz_chizilmaydi() {
        let (w, cp) = cube_chunk(2, 20_000_000, 20_000_000);
        w.set_block(&cell(2, 20_000_005, 20_000_005, 5), STONE);
        w.set_block(&cell(2, 20_000_006, 20_000_005, 5), STONE);
        assert_eq!(mesh_chunk(&w, cp).quads(), 10);
    }

    #[test]
    fn chegaradagi_blok_qoshni_bolakni_hisobga_oladi() {
        let (w, cp) = cube_chunk(1, 10_000_000, 10_000_000);
        w.set_block(&cell(1, 10_000_015, 10_000_005, 5), STONE);
        w.set_block(&cell(1, 10_000_016, 10_000_005, 5), STONE);
        assert_eq!(mesh_chunk(&w, cp).quads(), 5, "umumiy yuz chizilmasligi kerak");
    }

    #[test]
    fn yuz_chegarasidagi_blok_qoshni_yuzni_hisobga_oladi() {
        let w = World::new(1);
        let r = (HIGH / 16) * 16 + 5;
        let c = Cell { f: 0, u: N - 1, v: N / 2 + 3, r };
        let n = neighbor(&c, 1, 0, 0);
        assert_ne!(n.f, c.f);
        w.set_block(&c, STONE);
        w.set_block(&n, STONE);
        let (cp, _) = chunk_of(&c);
        assert_eq!(mesh_chunk(&w, cp).quads(), 5, "yuzlar orasidagi tutashuv yuzi chizilmasligi kerak");
    }

    #[test]
    fn ao_burchakda_qorongi() {
        let (w, cp) = cube_chunk(0, 30_000_000, 30_000_000);
        for du in 4..7 {
            for dv in 4..7 {
                w.set_block(&cell(0, 30_000_000 + du, 30_000_000 + dv, 5), STONE);
            }
        }
        w.set_block(&cell(0, 30_000_005, 30_000_005, 6), STONE);
        let m = mesh_chunk(&w, cp);
        assert!(m.opaque[0].iter().any(|v| v.c[3] < 255), "AO ishlamayapti");
    }

    #[test]
    fn suv_alohida_royxatda() {
        let w = World::new(11);
        for i in 0..6000i64 {
            let f = (i % 6) as u8;
            let (u, v) = (3000 + i * 9_000_001 % (N - 6000), 3000 + i * 6_000_007 % (N - 6000));
            if w.gen.surf_at(f, u, v).0 < SEA_R - 6 {
                let cp = CP { f, u: (u / 16) as i32, v: (v / 16) as i32, r: ((SEA_R - 1) / 16) as i32 };
                let m = mesh_chunk(&w, cp);
                assert!(!m.water.is_empty(), "suv yuzasi yo'q");
                return;
            }
        }
        panic!("dengiz topilmadi");
    }

    #[test]
    fn ustun_meshi_bolaklar_yigindisi() {
        let w = World::new(3);
        let cp = land_chunk(&w);
        let col = mesh_column(&w, cp.f, cp.u, cp.v);
        let (c0, c1) = w.section_range(cp.f, cp.u, cp.v);
        let mut total = 0;
        for cr in c0..=c1 {
            total += mesh_chunk(&w, CP { r: cr, ..cp }).quads();
        }
        assert_eq!(col.quads(), total);
        let (lo, hi) = bounds(&col);
        assert!(hi[1] != lo[1] || hi[0] != lo[0]);
        assert!(lo.iter().chain(hi.iter()).all(|v| v.abs() < 60_000.0), "ustun cho'qqilari anchor atrofida emas: {:?} {:?}", lo, hi);
    }

    #[test]
    fn tanlash_qutisi_olti_yuz() {
        let c = Cell { f: 3, u: 12_345_678, v: 22_222_222, r: SEA_R + 4 };
        let (o, v) = cell_box(&c);
        assert_eq!(v.len(), 24);
        assert!(d_len(d_sub(o, center(&c))) < 1e-9);
        for q in v.chunks(4) {
            for x in q {
                assert!(x.p.iter().all(|c| c.abs() < 2.0));
            }
        }
    }
}
