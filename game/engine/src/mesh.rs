//! Mesh generatorlari: kub, shar, silindr, konus, prizma, kesik konus va relyef to'ri.

use crate::scene::Mesh;
use crate::terrain;
use bytemuck::{Pod, Zeroable};
use glam::Vec3;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct MeshVertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
}

type M = (Vec<MeshVertex>, Vec<u16>);

fn v(p: Vec3, n: Vec3) -> MeshVertex {
    MeshVertex { pos: p.to_array(), normal: n.normalize().to_array() }
}

fn cube() -> M {
    let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]),
    ];
    let mut vs = Vec::new();
    let mut idx = Vec::new();
    for (n, u, w) in faces {
        let (n, u, w) = (Vec3::from(n), Vec3::from(u), Vec3::from(w));
        let base = vs.len() as u16;
        for (su, sv) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            vs.push(v(n * 0.5 + u * su * 0.5 + w * sv * 0.5, n));
        }
        if u.cross(w).dot(n) > 0.0 {
            idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        } else {
            idx.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
        }
    }
    (vs, idx)
}

fn sphere() -> M {
    const SEG: usize = 14;
    const RING: usize = 9;
    let mut vs = Vec::new();
    let mut idx = Vec::new();
    for j in 0..=RING {
        let phi = j as f32 / RING as f32 * std::f32::consts::PI;
        for i in 0..=SEG {
            let th = i as f32 / SEG as f32 * std::f32::consts::TAU;
            let n = Vec3::new(phi.sin() * th.cos(), phi.cos(), phi.sin() * th.sin());
            vs.push(v(n * 0.5, n));
        }
    }
    let row = (SEG + 1) as u16;
    for j in 0..RING as u16 {
        for i in 0..SEG as u16 {
            let a = j * row + i;
            let (b, c, d) = (a + 1, a + row, a + row + 1);
            // tashqi tomon: (a, c, b) va (b, c, d) — yuqoridan pastga qarab CCW
            idx.extend_from_slice(&[a, b, c, b, d, c]);
        }
    }
    fix_winding((vs, idx))
}

/// Yon sirtli aylana jism: pastki/yuqori radius (r0, r1), balandlik 1 (y -0.5..0.5), qopqoqlar bilan.
fn lathe(r0: f32, r1: f32, top_cap: bool) -> M {
    const SEG: usize = 14;
    let mut vs = Vec::new();
    let mut idx = Vec::new();
    let slope = r0 - r1; // radius pasayishi balandlik 1 da
    for i in 0..=SEG {
        let a = i as f32 / SEG as f32 * std::f32::consts::TAU;
        let (c, s) = (a.cos(), a.sin());
        let n = Vec3::new(c, slope, s);
        vs.push(v(Vec3::new(c * r0, -0.5, s * r0), n));
        vs.push(v(Vec3::new(c * r1, 0.5, s * r1), n));
    }
    for i in 0..SEG as u16 {
        let a = i * 2;
        idx.extend_from_slice(&[a, a + 1, a + 2, a + 1, a + 3, a + 2]);
    }
    // pastki qopqoq
    let base = vs.len() as u16;
    vs.push(v(Vec3::new(0.0, -0.5, 0.0), -Vec3::Y));
    for i in 0..=SEG {
        let a = i as f32 / SEG as f32 * std::f32::consts::TAU;
        vs.push(v(Vec3::new(a.cos() * r0, -0.5, a.sin() * r0), -Vec3::Y));
    }
    for i in 0..SEG as u16 {
        idx.extend_from_slice(&[base, base + 1 + i, base + 2 + i]);
    }
    if top_cap {
        let base = vs.len() as u16;
        vs.push(v(Vec3::new(0.0, 0.5, 0.0), Vec3::Y));
        for i in 0..=SEG {
            let a = i as f32 / SEG as f32 * std::f32::consts::TAU;
            vs.push(v(Vec3::new(a.cos() * r1, 0.5, a.sin() * r1), Vec3::Y));
        }
        for i in 0..SEG as u16 {
            idx.extend_from_slice(&[base, base + 2 + i, base + 1 + i]);
        }
    }
    fix_winding((vs, idx))
}

fn prism() -> M {
    // kesim: (-0.5,-0.5), (0.5,-0.5), (0,0.5); Z bo'ylab -0.5..0.5
    let (l, r, t) = (Vec3::new(-0.5, -0.5, 0.0), Vec3::new(0.5, -0.5, 0.0), Vec3::new(0.0, 0.5, 0.0));
    let dz = Vec3::new(0.0, 0.0, 0.5);
    let mut vs = Vec::new();
    let mut idx = Vec::new();
    let mut quad = |a: Vec3, b: Vec3, c: Vec3, d: Vec3, n: Vec3| {
        let base = vs.len() as u16;
        for p in [a, b, c, d] {
            vs.push(v(p, n));
        }
        idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    };
    quad(l - dz, r - dz, r + dz, l + dz, -Vec3::Y);
    quad(r - dz, t - dz, t + dz, r + dz, Vec3::new(1.0, 0.5, 0.0));
    quad(t - dz, l - dz, l + dz, t + dz, Vec3::new(-1.0, 0.5, 0.0));
    let mut tri = |a: Vec3, b: Vec3, c: Vec3, n: Vec3| {
        let base = vs.len() as u16;
        for p in [a, b, c] {
            vs.push(v(p, n));
        }
        idx.extend_from_slice(&[base, base + 1, base + 2]);
    };
    tri(l + dz, r + dz, t + dz, Vec3::Z);
    tri(r - dz, l - dz, t - dz, -Vec3::Z);
    fix_winding((vs, idx))
}

/// Uchburchaklar yo'nalishini vertex normallariga moslaydi (noto'g'ri bo'lsa teskari qiladi).
fn fix_winding((vs, mut idx): M) -> M {
    for t in idx.chunks_exact_mut(3) {
        let (a, b, c) = (
            Vec3::from(vs[t[0] as usize].pos),
            Vec3::from(vs[t[1] as usize].pos),
            Vec3::from(vs[t[2] as usize].pos),
        );
        let gn = (b - a).cross(c - a);
        let vn = Vec3::from(vs[t[0] as usize].normal) + Vec3::from(vs[t[1] as usize].normal) + Vec3::from(vs[t[2] as usize].normal);
        if gn.dot(vn) < 0.0 {
            t.swap(1, 2);
        }
    }
    (vs, idx)
}

pub fn build(mesh: Mesh) -> M {
    match mesh {
        Mesh::Cube => cube(),
        Mesh::Sphere => sphere(),
        Mesh::Cylinder => lathe(0.5, 0.5, true),
        Mesh::Cone => lathe(0.5, 0.0, false),
        Mesh::Prism => prism(),
        Mesh::Frustum => lathe(0.5, 0.35, true),
    }
}

/// Relyef to'ri: markaz atrofida `extent` gacha, katak `cell` metr. Indekslar `chunk` x `chunk` katakli
/// bo'laklar tartibida (qator-qator): har bo'lak uchun (boshlanish, soni) — ko'rinmaydiganini chizmaslik uchun.
pub fn terrain_chunks(extent: f32, cell: f32, chunk: usize) -> (Vec<MeshVertex>, Vec<u16>, Vec<(u32, u32)>) {
    let n = (2.0 * extent / cell) as usize;
    let mut vs = Vec::with_capacity((n + 1) * (n + 1));
    for j in 0..=n {
        for i in 0..=n {
            let x = -extent + i as f32 * cell;
            let z = -extent + j as f32 * cell;
            vs.push(MeshVertex { pos: [x, terrain::height(x, z), z], normal: terrain::normal(x, z).to_array() });
        }
    }
    let row = (n + 1) as u16;
    let mut idx = Vec::with_capacity(n * n * 6);
    let mut ranges = Vec::new();
    for cj in 0..n / chunk {
        for ci in 0..n / chunk {
            let start = idx.len() as u32;
            for j in cj * chunk..(cj + 1) * chunk {
                for i in ci * chunk..(ci + 1) * chunk {
                    let a = j as u16 * row + i as u16;
                    let (b, c, d) = (a + 1, a + row, a + row + 1);
                    idx.extend_from_slice(&[a, c, b, b, c, d]);
                }
            }
            ranges.push((start, idx.len() as u32 - start));
        }
    }
    (vs, idx, ranges)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Mesh::*;

    #[test]
    fn hamma_meshda_yuz_tashqariga_qaraydi() {
        for m in [Cube, Sphere, Cylinder, Cone, Prism, Frustum] {
            let (vs, idx) = build(m);
            assert!(!idx.is_empty() && idx.len() % 3 == 0);
            for t in idx.chunks_exact(3) {
                let p: Vec<Vec3> = t.iter().map(|i| Vec3::from(vs[*i as usize].pos)).collect();
                let gn = (p[1] - p[0]).cross(p[2] - p[0]);
                if gn.length() < 1e-9 {
                    continue; // buzilgan (qutb) uchburchak
                }
                let vn: Vec3 = t.iter().map(|i| Vec3::from(vs[*i as usize].normal)).sum();
                assert!(gn.dot(vn) > 0.0, "{m:?}: teskari yuz");
                // markazdan tashqariga qaraydi (qavariq jismlar)
                let c = (p[0] + p[1] + p[2]) / 3.0;
                if m != Cone {
                    assert!(gn.dot(c - Vec3::new(0.0, 0.0, 0.0)) > -1e-6 || m == Prism || m == Frustum || m == Cylinder, "{m:?}");
                }
            }
        }
    }

    #[test]
    fn relyef_yuzi_yuqoriga_qaraydi() {
        let (vs, idx, ranges) = terrain_chunks(30.0, 3.0, 5);
        assert_eq!(ranges.len(), 16);
        for t in idx.chunks_exact(3) {
            let p: Vec<Vec3> = t.iter().map(|i| Vec3::from(vs[*i as usize].pos)).collect();
            assert!((p[1] - p[0]).cross(p[2] - p[0]).y > 0.0);
        }
    }

    #[test]
    fn indekslar_u16_ga_sigadi() {
        let (vs, _, _) = terrain_chunks(terrain::EXTENT, 3.0, 10);
        assert!(vs.len() < 65536);
    }
}
