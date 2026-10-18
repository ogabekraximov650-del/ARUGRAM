//! Sahna ma'lumotlari: mesh turlari, instanslar (joylashuv + burilish + material), to'qnashuv qutilari.

use bytemuck::{Pod, Zeroable};
use glam::{Quat, Vec3};

/// Chiziladigan oddiy shakllar (hammasi birlik o'lchamda, instans o'lchami bilan kattalashtiriladi).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(usize)]
pub enum Mesh {
    /// Quti, tomoni 1, markazda.
    Cube = 0,
    /// Shar, diametri 1.
    Sphere = 1,
    /// Silindr: diametri 1, balandligi 1 (o'qi Y).
    Cylinder = 2,
    /// Konus: asosi (diametr 1) pastda, uchi tepada.
    Cone = 3,
    /// Uchburchak prizma (tom): kesim XY da, Z bo'ylab cho'zilgan.
    Prism = 4,
    /// Kesik konus: pastki diametri 1, yuqorisi 0.7.
    Frustum = 5,
}
pub const MESH_COUNT: usize = 6;

/// Sirt materiallari (shaderda protsedur tekstura va yaltirash shunga qarab).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum Mat {
    Flat = 0,
    Grass = 1,
    Brick = 2,
    Plaster = 3,
    Roof = 4,
    Glass = 5,
    Asphalt = 6,
    Wood = 7,
    Metal = 8,
    Foliage = 9,
    Emissive = 10,
    Cloth = 11,
    Concrete = 12,
    Skin = 13,
    Hair = 14,
    Rock = 15,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Instance {
    pub pos: [f32; 4],
    /// xyz — o'lcham, w — material raqami.
    pub scale: [f32; 4],
    pub color: [f32; 4],
    /// Burilish kvaterniyoni (x, y, z, w).
    pub rot: [f32; 4],
}

impl Instance {
    pub fn new(center: Vec3, size: Vec3, rgb: [f32; 3], mat: Mat, rot: Quat) -> Self {
        Self {
            pos: [center.x, center.y, center.z, 0.0],
            scale: [size.x, size.y, size.z, mat as u32 as f32],
            color: [rgb[0], rgb[1], rgb[2], 1.0],
            rot: rot.to_array(),
        }
    }
}

/// Mesh turi bo'yicha guruhlangan instanslar.
#[derive(Default, Clone)]
pub struct Lists(pub [Vec<Instance>; MESH_COUNT]);

impl Lists {
    pub fn push(&mut self, mesh: Mesh, inst: Instance) {
        self.0[mesh as usize].push(inst);
    }
    pub fn total(&self) -> usize {
        self.0.iter().map(|v| v.len()).sum()
    }
    pub fn counts(&self) -> [usize; MESH_COUNT] {
        let mut c = [0; MESH_COUNT];
        for (i, v) in self.0.iter().enumerate() {
            c[i] = v.len();
        }
        c
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    /// Nur kesishishi (slab usuli): masofa yoki `None`.
    pub fn ray(&self, o: Vec3, d: Vec3) -> Option<f32> {
        let mut t0 = 0.0f32;
        let mut t1 = f32::MAX;
        for a in 0..3 {
            if d[a].abs() < 1e-8 {
                if o[a] < self.min[a] || o[a] > self.max[a] {
                    return None;
                }
            } else {
                let inv = 1.0 / d[a];
                let (mut a0, mut a1) = ((self.min[a] - o[a]) * inv, (self.max[a] - o[a]) * inv);
                if a0 > a1 {
                    std::mem::swap(&mut a0, &mut a1);
                }
                t0 = t0.max(a0);
                t1 = t1.min(a1);
                if t0 > t1 {
                    return None;
                }
            }
        }
        Some(t0)
    }
}
