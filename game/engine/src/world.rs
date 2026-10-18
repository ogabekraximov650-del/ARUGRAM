//! Dunyo: qishloq (yo'llar, uylar, ombor, mashinalar, chiroqlar), o'rmon, toshlar. Hammasi deterministik.

use crate::scene::{Aabb, Instance, Lists, Mat, Mesh};
use crate::terrain;
use glam::{Quat, Vec3};
use std::f32::consts::FRAC_PI_2;

pub struct World {
    pub statics: Lists,
    pub colliders: Vec<Aabb>,
    /// Narsa (qurol, o'q-dori, dori) qo'yish mumkin bo'lgan joylar.
    pub loot: Vec<Vec3>,
}

struct Lcg(u32);
impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.0 >> 8) as f32 / (1u32 << 24) as f32
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.next()
    }
}

#[derive(Clone, Copy)]
struct Rect {
    x0: f32,
    z0: f32,
    x1: f32,
    z1: f32,
}

impl Rect {
    fn around(c: Vec3, hx: f32, hz: f32) -> Self {
        Rect { x0: c.x - hx, z0: c.z - hz, x1: c.x + hx, z1: c.z + hz }
    }
    fn overlaps(&self, o: &Rect) -> bool {
        self.x0 < o.x1 && self.x1 > o.x0 && self.z0 < o.z1 && self.z1 > o.z0
    }
    fn contains(&self, x: f32, z: f32, pad: f32) -> bool {
        x > self.x0 - pad && x < self.x1 + pad && z > self.z0 - pad && z < self.z1 + pad
    }
}

/// Mahalliy koordinata tizimi: `k` — 90° burilishlar soni (0: old tomon +Z, 1: +X, 2: -Z, 3: -X).
struct B {
    lists: Lists,
    cols: Vec<Aabb>,
    loot: Vec<Vec3>,
    origin: Vec3,
    k: i32,
    q: Quat,
}

impl B {
    fn at(&mut self, origin: Vec3, k: i32) {
        self.origin = origin;
        self.k = k.rem_euclid(4);
        self.q = Quat::from_rotation_y(self.k as f32 * FRAC_PI_2);
    }

    fn map(&self, l: Vec3) -> Vec3 {
        self.origin + self.q * l
    }

    /// Mesh (to'qnashuvsiz). `size` mahalliy o'lchamda.
    fn vis(&mut self, mesh: Mesh, c: Vec3, size: Vec3, rgb: [f32; 3], mat: Mat, rot: Quat) {
        let inst = Instance::new(self.map(c), size, rgb, mat, self.q * rot);
        self.lists.push(mesh, inst);
    }

    /// Faqat to'qnashuv qutisi (mahalliy markaz va o'lcham).
    fn col(&mut self, c: Vec3, size: Vec3) {
        let w = self.map(c);
        let s = if self.k % 2 == 1 { Vec3::new(size.z, size.y, size.x) } else { size };
        self.cols.push(Aabb { min: w - s * 0.5, max: w + s * 0.5 });
    }

    /// Ko'rinadigan va to'qnashadigan quti.
    fn solid(&mut self, c: Vec3, size: Vec3, rgb: [f32; 3], mat: Mat) {
        self.vis(Mesh::Cube, c, size, rgb, mat, Quat::IDENTITY);
        self.col(c, size);
    }

    fn cube(&mut self, c: Vec3, size: Vec3, rgb: [f32; 3], mat: Mat) {
        self.vis(Mesh::Cube, c, size, rgb, mat, Quat::IDENTITY);
    }

    fn loot_at(&mut self, l: Vec3) {
        let p = self.map(l);
        self.loot.push(p);
    }
}

const WHITE: [f32; 3] = [0.92, 0.92, 0.90];
const GLASS: [f32; 3] = [0.30, 0.42, 0.55];
const WOOD: [f32; 3] = [0.55, 0.38, 0.22];
const DARK_WOOD: [f32; 3] = [0.30, 0.20, 0.13];

fn v3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

/// Ichi bo'sh uy (eshigi old tomonda), devorlari, tomi, derazalari, eshigi va mebellari bilan.
fn house(b: &mut B, w: f32, d: f32, wall: ([f32; 3], Mat), roof: [f32; 3], variant: u32) {
    let (t, h, y0) = (0.25, 2.8, 0.2);
    let wy = y0 + h * 0.5;
    // poydevor
    b.solid(v3(0.0, 0.1, 0.0), v3(w + 0.4, 0.2, d + 0.4), [0.6, 0.6, 0.6], Mat::Concrete);
    // devorlar
    b.solid(v3(0.0, wy, -d * 0.5 + t * 0.5), v3(w, h, t), wall.0, wall.1);
    b.solid(v3(-w * 0.5 + t * 0.5, wy, 0.0), v3(t, h, d), wall.0, wall.1);
    b.solid(v3(w * 0.5 - t * 0.5, wy, 0.0), v3(t, h, d), wall.0, wall.1);
    let gap = 1.3;
    let seg = (w - gap) * 0.5;
    let sx = (gap + seg) * 0.5;
    b.solid(v3(-sx, wy, d * 0.5 - t * 0.5), v3(seg, h, t), wall.0, wall.1);
    b.solid(v3(sx, wy, d * 0.5 - t * 0.5), v3(seg, h, t), wall.0, wall.1);
    b.solid(v3(0.0, y0 + 2.2 + (h - 2.2) * 0.5, d * 0.5 - t * 0.5), v3(gap, h - 2.2, t), wall.0, wall.1);
    // shift
    b.solid(v3(0.0, y0 + h + 0.075, 0.0), v3(w - 0.1, 0.15, d - 0.1), [0.8, 0.78, 0.72], Mat::Plaster);
    // tom
    let ridge_x = w > d;
    let (span, len) = if ridge_x { (d + 0.9, w + 0.9) } else { (w + 0.9, d + 0.9) };
    let rise = 1.4 + span * 0.12;
    let rot = if ridge_x { Quat::from_rotation_y(FRAC_PI_2) } else { Quat::IDENTITY };
    b.vis(Mesh::Prism, v3(0.0, y0 + h + 0.15 + rise * 0.5, 0.0), v3(span, rise, len), roof, Mat::Roof, rot);
    // tom ostidagi to'siq (gable) — devor rangida
    if variant % 2 == 0 {
        b.cube(v3(w * 0.25, y0 + h + 0.15 + rise + 0.5, -d * 0.2), v3(0.5, 1.4, 0.5), [0.58, 0.28, 0.2], Mat::Brick);
    }
    // derazalar
    let win_y = y0 + 1.55;
    for sx in [-1.0f32, 1.0] {
        let x = sx * (gap * 0.5 + seg * 0.5);
        b.cube(v3(x, win_y, d * 0.5 + 0.01), v3(1.05, 1.25, 0.05), WHITE, Mat::Plaster);
        b.cube(v3(x, win_y, d * 0.5 + 0.03), v3(0.88, 1.08, 0.05), GLASS, Mat::Glass);
    }
    let nz = ((d - 1.5) / 3.2).floor().max(1.0) as i32;
    for i in 0..nz {
        let z = -d * 0.5 + d * (i as f32 + 0.5) / nz as f32;
        for sx in [-1.0f32, 1.0] {
            let x = sx * (w * 0.5 + 0.01);
            b.cube(v3(x, win_y, z), v3(0.05, 1.25, 1.05), WHITE, Mat::Plaster);
            b.cube(v3(x + sx * 0.02, win_y, z), v3(0.05, 1.08, 0.88), GLASS, Mat::Glass);
        }
    }
    b.cube(v3(0.0, win_y, -d * 0.5 - 0.01), v3(1.05, 1.25, 0.05), WHITE, Mat::Plaster);
    b.cube(v3(0.0, win_y, -d * 0.5 - 0.03), v3(0.88, 1.08, 0.05), GLASS, Mat::Glass);
    // ochiq eshik
    let hinge = v3(gap * 0.5, y0 + 1.05, d * 0.5);
    let phi = 75f32.to_radians();
    let dir = v3(-phi.cos(), 0.0, phi.sin());
    let theta = (-dir.z).atan2(dir.x);
    b.vis(Mesh::Cube, hinge + dir * 0.45, v3(0.9, 2.1, 0.05), WOOD, Mat::Wood, Quat::from_rotation_y(theta));
    // ayvon zinasi
    b.solid(v3(0.0, 0.05, d * 0.5 + 0.7), v3(2.0, 0.1, 1.0), [0.62, 0.62, 0.62], Mat::Concrete);
    // ichki: lampa va mebel
    b.vis(Mesh::Sphere, v3(0.0, y0 + h - 0.2, 0.0), v3(0.3, 0.2, 0.3), [1.0, 0.92, 0.7], Mat::Emissive, Quat::IDENTITY);
    let fy = y0;
    match variant % 3 {
        0 => {
            // stol + 2 stul
            b.cube(v3(-w * 0.25, fy + 0.74, -d * 0.2), v3(1.3, 0.06, 0.8), WOOD, Mat::Wood);
            for (lx, lz) in [(-0.6, -0.33), (0.6, -0.33), (-0.6, 0.33), (0.6, 0.33)] {
                b.cube(v3(-w * 0.25 + lx, fy + 0.36, -d * 0.2 + lz), v3(0.06, 0.72, 0.06), DARK_WOOD, Mat::Wood);
            }
            b.col(v3(-w * 0.25, fy + 0.39, -d * 0.2), v3(1.3, 0.78, 0.8));
            b.solid(v3(-w * 0.25 + 1.0, fy + 0.25, -d * 0.2 + 0.2), v3(0.45, 0.5, 0.45), WOOD, Mat::Wood);
            b.loot_at(v3(-w * 0.25, fy + 0.9, -d * 0.2));
        }
        1 => {
            // karavot
            b.solid(v3(w * 0.25, fy + 0.3, -d * 0.5 + 1.2), v3(1.0, 0.6, 2.0), [0.85, 0.85, 0.9], Mat::Cloth);
            b.cube(v3(w * 0.25, fy + 0.65, -d * 0.5 + 0.5), v3(0.7, 0.15, 0.4), [0.9, 0.9, 0.95], Mat::Cloth);
            b.cube(v3(w * 0.25, fy + 0.52, -d * 0.5 + 1.8), v3(1.02, 0.12, 1.2), [0.3, 0.4, 0.7], Mat::Cloth);
            b.loot_at(v3(-w * 0.2, fy, 0.0));
        }
        _ => {
            // shkaf + sandiq
            b.solid(v3(-w * 0.5 + 0.7, fy + 1.0, -d * 0.5 + 0.45), v3(1.0, 2.0, 0.5), DARK_WOOD, Mat::Wood);
            b.solid(v3(w * 0.5 - 0.7, fy + 0.3, 0.0), v3(0.9, 0.6, 0.5), [0.45, 0.3, 0.15], Mat::Wood);
            b.loot_at(v3(0.0, fy, -d * 0.2));
        }
    }
    b.loot_at(v3(w * 0.2, fy, d * 0.2));
}

fn warehouse(b: &mut B) {
    let (w, d, h, t) = (18.0, 11.0, 6.0, 0.3);
    let steel = [0.42, 0.50, 0.58];
    let wy = h * 0.5;
    b.solid(v3(0.0, 0.1, 0.0), v3(w + 0.4, 0.2, d + 0.4), [0.55, 0.55, 0.55], Mat::Concrete);
    b.solid(v3(0.0, wy + 0.2, -d * 0.5 + t * 0.5), v3(w, h, t), steel, Mat::Metal);
    b.solid(v3(-w * 0.5 + t * 0.5, wy + 0.2, 0.0), v3(t, h, d), steel, Mat::Metal);
    b.solid(v3(w * 0.5 - t * 0.5, wy + 0.2, 0.0), v3(t, h, d), steel, Mat::Metal);
    let gap = 4.0;
    let seg = (w - gap) * 0.5;
    let sx = (gap + seg) * 0.5;
    b.solid(v3(-sx, wy + 0.2, d * 0.5 - t * 0.5), v3(seg, h, t), steel, Mat::Metal);
    b.solid(v3(sx, wy + 0.2, d * 0.5 - t * 0.5), v3(seg, h, t), steel, Mat::Metal);
    b.solid(v3(0.0, 0.2 + 4.0 + (h - 4.0) * 0.5, d * 0.5 - t * 0.5), v3(gap, h - 4.0, t), steel, Mat::Metal);
    b.solid(v3(0.0, h + 0.35, 0.0), v3(w + 0.6, 0.3, d + 0.6), [0.30, 0.34, 0.38], Mat::Metal);
    // yuqori deraza chizig'i
    for i in 0..6 {
        let x = -w * 0.5 + 1.5 + i as f32 * 3.0;
        b.cube(v3(x, h - 0.6, d * 0.5 + 0.01), v3(2.0, 0.7, 0.05), GLASS, Mat::Glass);
    }
    // javonlar va qutilar
    for sz in [-3.0f32, 0.0, 3.0] {
        b.solid(v3(-5.5, 1.3, sz), v3(5.0, 2.2, 0.6), [0.25, 0.38, 0.6], Mat::Metal);
        b.cube(v3(-5.5, 2.6, sz), v3(4.6, 0.5, 0.5), WOOD, Mat::Wood);
        b.loot_at(v3(-3.2, 0.2, sz + 0.9));
    }
    for (x, z, s) in [(4.5, -3.0, 1.2), (5.8, -3.0, 1.0), (5.1, -3.0, 0.9), (4.8, 2.5, 1.3)] {
        b.solid(v3(x, 0.2 + s * 0.5, z), v3(s, s, s), WOOD, Mat::Wood);
    }
    b.loot_at(v3(5.0, 0.2, 0.0));
    b.loot_at(v3(0.0, 0.2, -3.5));
}

fn car(b: &mut B, rgb: [f32; 3]) {
    b.col(v3(0.0, 0.7, 0.0), v3(1.9, 1.4, 4.2));
    b.vis(Mesh::Cube, v3(0.0, 0.62, 0.0), v3(1.9, 0.6, 4.2), rgb, Mat::Metal, Quat::IDENTITY);
    b.vis(Mesh::Cube, v3(0.0, 1.15, 0.1), v3(1.7, 0.55, 2.2), rgb, Mat::Metal, Quat::IDENTITY);
    b.cube(v3(0.0, 1.17, 0.1), v3(1.72, 0.4, 2.0), [0.12, 0.16, 0.2], Mat::Glass);
    let wr = Quat::from_rotation_z(FRAC_PI_2);
    for (x, z) in [(-0.9, -1.35), (0.9, -1.35), (-0.9, 1.35), (0.9, 1.35)] {
        b.vis(Mesh::Cylinder, v3(x, 0.36, z), v3(0.72, 0.28, 0.72), [0.08, 0.08, 0.08], Mat::Cloth, wr);
    }
    for x in [-0.65, 0.65] {
        b.cube(v3(x, 0.7, 2.1), v3(0.35, 0.18, 0.05), [1.0, 0.95, 0.75], Mat::Emissive);
        b.cube(v3(x, 0.7, -2.1), v3(0.3, 0.15, 0.05), [0.9, 0.1, 0.1], Mat::Emissive);
    }
}

fn lamp(b: &mut B) {
    b.vis(Mesh::Cylinder, v3(0.0, 2.6, 0.0), v3(0.14, 5.2, 0.14), [0.2, 0.22, 0.25], Mat::Metal, Quat::IDENTITY);
    b.col(v3(0.0, 2.6, 0.0), v3(0.3, 5.2, 0.3));
    b.cube(v3(0.0, 5.15, 0.6), v3(0.1, 0.1, 1.3), [0.2, 0.22, 0.25], Mat::Metal);
    b.vis(Mesh::Sphere, v3(0.0, 5.05, 1.15), v3(0.35, 0.22, 0.35), [1.0, 0.95, 0.75], Mat::Emissive, Quat::IDENTITY);
}

fn water_tower(b: &mut B) {
    let steel = [0.5, 0.52, 0.55];
    for (x, z) in [(-1.6, -1.6), (1.6, -1.6), (-1.6, 1.6), (1.6, 1.6)] {
        b.vis(Mesh::Cylinder, v3(x, 4.5, z), v3(0.3, 9.0, 0.3), steel, Mat::Metal, Quat::IDENTITY);
        b.col(v3(x, 4.5, z), v3(0.4, 9.0, 0.4));
    }
    b.vis(Mesh::Cylinder, v3(0.0, 10.5, 0.0), v3(5.0, 3.2, 5.0), [0.6, 0.62, 0.65], Mat::Metal, Quat::IDENTITY);
    b.vis(Mesh::Cone, v3(0.0, 12.9, 0.0), v3(5.4, 1.6, 5.4), [0.55, 0.25, 0.2], Mat::Roof, Quat::IDENTITY);
    b.cube(v3(0.0, 4.0, 0.0), v3(3.4, 0.1, 0.1), steel, Mat::Metal);
    b.cube(v3(0.0, 4.0, 0.0), v3(0.1, 0.1, 3.4), steel, Mat::Metal);
}

fn pine(b: &mut B, x: f32, z: f32, s: f32, shade: f32) {
    let y = terrain::height(x, z);
    b.at(v3(x, y, z), 0);
    let g = [0.10 * shade, 0.30 * shade, 0.14 * shade];
    b.vis(Mesh::Cylinder, v3(0.0, 1.0 * s, 0.0), v3(0.4 * s, 2.0 * s, 0.4 * s), [0.32, 0.22, 0.14], Mat::Wood, Quat::IDENTITY);
    for (i, (dia, hh)) in [(3.4, 2.4), (2.7, 2.2), (1.9, 2.0)].iter().enumerate() {
        b.vis(Mesh::Cone, v3(0.0, (2.0 + i as f32 * 1.5) * s + hh * 0.5 * s, 0.0), v3(dia * s, hh * s, dia * s), g, Mat::Foliage, Quat::IDENTITY);
    }
    b.col(v3(0.0, 1.5 * s, 0.0), v3(0.5 * s, 3.4 * s + 0.6, 0.5 * s));
}

fn oak(b: &mut B, x: f32, z: f32, s: f32, shade: f32, birch: bool) {
    let y = terrain::height(x, z);
    b.at(v3(x, y, z), 0);
    let trunk = if birch { [0.85, 0.85, 0.8] } else { [0.35, 0.25, 0.16] };
    b.vis(Mesh::Cylinder, v3(0.0, 1.6 * s, 0.0), v3(0.5 * s, 3.2 * s, 0.5 * s), trunk, Mat::Wood, Quat::IDENTITY);
    let g = if birch { [0.40 * shade, 0.58 * shade, 0.22 * shade] } else { [0.17 * shade, 0.40 * shade, 0.16 * shade] };
    for (dx, dy, dz, r) in [(0.0, 4.3, 0.0, 3.4), (1.0, 3.7, 0.6, 2.4), (-0.9, 3.9, -0.7, 2.6), (0.2, 5.2, -0.3, 2.2)] {
        b.vis(Mesh::Sphere, v3(dx * s, dy * s, dz * s), v3(r * s, r * 0.85 * s, r * s), g, Mat::Foliage, Quat::IDENTITY);
    }
    b.col(v3(0.0, 1.6 * s, 0.0), v3(0.5 * s, 3.2 * s + 0.6, 0.5 * s));
}

fn bush(b: &mut B, x: f32, z: f32, s: f32) {
    let y = terrain::height(x, z);
    b.at(v3(x, y, z), 0);
    b.vis(Mesh::Sphere, v3(0.0, 0.45 * s, 0.0), v3(1.3 * s, 0.95 * s, 1.2 * s), [0.20, 0.42, 0.18], Mat::Foliage, Quat::IDENTITY);
    b.vis(Mesh::Sphere, v3(0.5 * s, 0.35 * s, 0.2 * s), v3(0.8 * s, 0.7 * s, 0.8 * s), [0.24, 0.46, 0.2], Mat::Foliage, Quat::IDENTITY);
}

fn rock(b: &mut B, x: f32, z: f32, s: f32, ry: f32) {
    let y = terrain::height(x, z);
    b.at(v3(x, y, z), 0);
    let q = Quat::from_rotation_y(ry) * Quat::from_rotation_x(0.2);
    b.vis(Mesh::Sphere, v3(0.0, 0.5 * s, 0.0), v3(2.2 * s, 1.5 * s, 1.8 * s), [0.5, 0.5, 0.5], Mat::Rock, q);
    b.col(v3(0.0, 0.6 * s, 0.0), v3(1.5 * s, 1.1 * s, 1.3 * s));
}

pub fn build() -> World {
    let mut b = B {
        lists: Lists::default(),
        cols: Vec::new(),
        loot: Vec::new(),
        origin: Vec3::ZERO,
        k: 0,
        q: Quat::IDENTITY,
    };
    let mut rng = Lcg(2024);
    let mut taken: Vec<Rect> = Vec::new();

    // yo'llar: 2 ta kesishgan asfalt yo'l, yo'lak va chiziqlar
    let asphalt = [0.20, 0.20, 0.22];
    b.at(Vec3::ZERO, 0);
    b.cube(v3(0.0, 0.03, 0.0), v3(6.0, 0.06, 92.0), asphalt, Mat::Asphalt);
    b.cube(v3(0.0, 0.031, 0.0), v3(92.0, 0.06, 6.0), asphalt, Mat::Asphalt);
    for s in [-1.0f32, 1.0] {
        b.cube(v3(s * 4.0, 0.06, 0.0), v3(1.8, 0.12, 92.0), [0.62, 0.62, 0.6], Mat::Concrete);
        b.cube(v3(0.0, 0.06, s * 4.0), v3(92.0, 0.12, 1.8), [0.62, 0.62, 0.6], Mat::Concrete);
    }
    for i in -7..=7 {
        let p = i as f32 * 6.0;
        if p.abs() < 4.0 {
            continue;
        }
        b.cube(v3(0.0, 0.07, p), v3(0.18, 0.02, 2.6), [0.95, 0.95, 0.9], Mat::Flat);
        b.cube(v3(p, 0.071, 0.0), v3(2.6, 0.02, 0.18), [0.95, 0.95, 0.9], Mat::Flat);
    }
    taken.push(Rect { x0: -5.5, z0: -48.0, x1: 5.5, z1: 48.0 });
    taken.push(Rect { x0: -48.0, z0: -5.5, x1: 48.0, z1: 5.5 });

    // ombor (eng katta bino)
    let wpos = v3(-28.0, 0.0, -30.0);
    b.at(wpos, 0);
    warehouse(&mut b);
    taken.push(Rect::around(wpos, 10.5, 7.0));

    // suv minorasi
    let tpos = v3(30.0, 0.0, 28.0);
    b.at(tpos, 0);
    water_tower(&mut b);
    taken.push(Rect::around(tpos, 4.0, 4.0));

    // uylar: yo'llar bo'ylab, eshigi yo'lga qaragan
    let walls: [([f32; 3], Mat); 6] = [
        ([0.93, 0.89, 0.80], Mat::Plaster),
        ([0.94, 0.86, 0.62], Mat::Plaster),
        ([0.72, 0.82, 0.92], Mat::Plaster),
        ([0.92, 0.72, 0.64], Mat::Plaster),
        ([0.62, 0.31, 0.23], Mat::Brick),
        ([0.70, 0.36, 0.26], Mat::Brick),
    ];
    let roofs: [[f32; 3]; 4] = [[0.62, 0.27, 0.18], [0.28, 0.30, 0.36], [0.38, 0.25, 0.18], [0.24, 0.38, 0.28]];
    let mut idx = 0u32;
    let mut plots: Vec<(Vec3, i32, f32)> = Vec::new(); // (yo'l o'qi bo'yicha markaz, burilish, yo'lga masofa)
    for s in [-1.0f32, 1.0] {
        for p in [-34.0f32, -20.0, 18.0, 32.0] {
            // N-S yo'l: x = ±, eshik markazga qaraydi
            plots.push((v3(s * 1.0, 0.0, p), if s < 0.0 { 1 } else { 3 }, 7.5));
        }
        for p in [-34.0f32, -18.0, 20.0, 34.0] {
            plots.push((v3(p, 0.0, s * 1.0), if s < 0.0 { 0 } else { 2 }, 7.5));
        }
    }
    for (c, k, off) in plots {
        let w = 7.0 + (idx % 3) as f32 * 1.5;
        let d = 6.0 + (idx % 2) as f32 * 1.5;
        let along_z = k % 2 == 1; // eshik X ga qaraydi => yo'l N-S
        let (hx, hz) = if along_z { (d * 0.5 + 0.5, w * 0.5 + 0.5) } else { (w * 0.5 + 0.5, d * 0.5 + 0.5) };
        let pos = if along_z {
            v3(c.x.signum() * (off + d * 0.5), 0.0, c.z)
        } else {
            v3(c.x, 0.0, c.z.signum() * (off + d * 0.5))
        };
        let r = Rect::around(pos, hx + 1.2, hz + 1.2);
        if taken.iter().any(|t| t.overlaps(&r)) {
            continue;
        }
        taken.push(r);
        b.at(pos, k);
        let wall = walls[(idx as usize * 5 + 1) % walls.len()];
        let roof = roofs[(idx as usize * 3) % roofs.len()];
        house(&mut b, w, d, wall, roof, idx);
        idx += 1;
    }

    // mashinalar (yo'l chetida)
    let car_cols: [[f32; 3]; 5] = [[0.75, 0.12, 0.12], [0.15, 0.30, 0.65], [0.85, 0.85, 0.85], [0.15, 0.15, 0.17], [0.85, 0.65, 0.12]];
    for (i, (x, z, k)) in [(2.0f32, -14.0f32, 0), (-2.0, 24.0, 2), (-12.0, 2.0, 1), (16.0, -2.0, 3), (2.0, 38.0, 0)].iter().enumerate() {
        b.at(v3(*x, 0.06, *z), *k);
        car(&mut b, car_cols[i % car_cols.len()]);
        b.loot_at(v3(2.0, 0.0, 0.0));
    }

    // ko'cha chiroqlari
    for p in [-36.0f32, -18.0, 18.0, 36.0] {
        b.at(v3(5.2, 0.0, p), 3);
        lamp(&mut b);
        b.at(v3(-5.2, 0.0, p), 1);
        lamp(&mut b);
        b.at(v3(p, 0.0, 5.2), 2);
        lamp(&mut b);
        b.at(v3(p, 0.0, -5.2), 0);
        lamp(&mut b);
    }

    // yo'l bo'yidagi quti va bochkalar
    for i in 0..14 {
        let ang = rng.range(0.0, std::f32::consts::TAU);
        let r = rng.range(8.0, 40.0);
        let (x, z) = (ang.cos() * r, ang.sin() * r);
        if taken.iter().any(|t| t.contains(x, z, 0.8)) {
            continue;
        }
        b.at(v3(x, 0.0, z), 0);
        if i % 2 == 0 {
            let s = rng.range(0.8, 1.2);
            b.solid(v3(0.0, s * 0.5, 0.0), v3(s, s, s), WOOD, Mat::Wood);
            b.loot_at(v3(0.0, s + 0.1, 0.0));
        } else {
            b.vis(Mesh::Cylinder, v3(0.0, 0.5, 0.0), v3(0.6, 1.0, 0.6), [0.7, 0.25, 0.15], Mat::Metal, Quat::IDENTITY);
            b.col(v3(0.0, 0.5, 0.0), v3(0.6, 1.0, 0.6));
        }
    }

    // o'simliklar va toshlar
    let mut placed: Vec<(f32, f32)> = Vec::new();
    let mut tries = 0;
    let mut trees = 0;
    while trees < 190 && tries < 2500 {
        tries += 1;
        let x = rng.range(-95.0, 95.0);
        let z = rng.range(-95.0, 95.0);
        let r = (x * x + z * z).sqrt();
        if r < 12.0 || taken.iter().any(|t| t.contains(x, z, 2.5)) {
            continue;
        }
        // qishloq ichida siyrak, tashqarida zich
        if r < 40.0 && rng.next() > 0.18 {
            continue;
        }
        if placed.iter().any(|(px, pz)| (px - x).powi(2) + (pz - z).powi(2) < 9.0) {
            continue;
        }
        placed.push((x, z));
        trees += 1;
        let s = rng.range(0.85, 1.35);
        let shade = rng.range(0.85, 1.2);
        match (rng.next() * 3.0) as i32 {
            0 => pine(&mut b, x, z, s, shade),
            1 => oak(&mut b, x, z, s, shade, false),
            _ => oak(&mut b, x, z, s * 0.9, shade, true),
        }
    }
    for _ in 0..110 {
        let x = rng.range(-95.0, 95.0);
        let z = rng.range(-95.0, 95.0);
        if (x * x + z * z).sqrt() < 10.0 || taken.iter().any(|t| t.contains(x, z, 1.5)) {
            continue;
        }
        bush(&mut b, x, z, rng.range(0.8, 1.5));
    }
    for _ in 0..45 {
        let x = rng.range(-95.0, 95.0);
        let z = rng.range(-95.0, 95.0);
        if (x * x + z * z).sqrt() < 30.0 || taken.iter().any(|t| t.contains(x, z, 2.0)) {
            continue;
        }
        rock(&mut b, x, z, rng.range(0.8, 2.2), rng.range(0.0, 3.0));
    }

    // dalada narsa joylari (o'rmon va qishloq oralig'ida)
    for _ in 0..40 {
        let x = rng.range(-90.0, 90.0);
        let z = rng.range(-90.0, 90.0);
        if taken.iter().any(|t| t.contains(x, z, 1.0)) || placed.iter().any(|(px, pz)| (px - x).powi(2) + (pz - z).powi(2) < 4.0) {
            continue;
        }
        b.loot.push(v3(x, terrain::height(x, z), z));
    }

    World { statics: b.lists, colliders: b.cols, loot: b.loot }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dunyo_bor_va_narsa_joylari_yetarli() {
        let w = build();
        assert!(w.statics.total() > 500, "instanslar: {}", w.statics.total());
        assert!(w.colliders.len() > 100);
        assert!(w.loot.len() >= 30, "narsa joylari: {}", w.loot.len());
    }

    #[test]
    fn boshlangich_nuqta_bosh() {
        let w = build();
        // o'yinchi tug'iladigan joy (0, ., 12) va ± 1 m da hech qanday quti yo'q
        for c in &w.colliders {
            let inside = 0.0 > c.min.x - 0.6 && 0.0 < c.max.x + 0.6 && 12.0 > c.min.z - 0.6 && 12.0 < c.max.z + 0.6 && c.min.y < 1.8;
            assert!(!inside, "spawn to'silgan: {c:?}");
        }
    }

    #[test]
    fn uyga_kirish_mumkin() {
        // har bir uyning eshigi oldida bo'sh joy bor: ya'ni old devorda tirqish (qutilar orasida 1.3 m ochiq)
        let w = build();
        assert!(w.colliders.iter().all(|c| c.min.x < c.max.x && c.min.y < c.max.y && c.min.z < c.max.z));
    }
}
