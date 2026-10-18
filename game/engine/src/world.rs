//! Tajriba dunyosi: zamin, binolar, qutilar (deterministik).

use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Instance {
    pub pos: [f32; 4],
    pub scale: [f32; 4],
    /// rgb + a (a > 0.5 — to'r chiziladi)
    pub color: [f32; 4],
}

struct Lcg(u32);
impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.0 >> 8) as f32 / (1u32 << 24) as f32
    }
}

fn boxi(cx: f32, cz: f32, sx: f32, sy: f32, sz: f32, rgb: [f32; 3]) -> Instance {
    Instance {
        pos: [cx, sy * 0.5, cz, 0.0],
        scale: [sx, sy, sz, 0.0],
        color: [rgb[0], rgb[1], rgb[2], 0.0],
    }
}

pub fn build() -> Vec<Instance> {
    let mut v = Vec::new();
    v.push(Instance {
        pos: [0.0, -0.5, 0.0, 0.0],
        scale: [120.0, 1.0, 120.0, 0.0],
        color: [0.42, 0.62, 0.36, 1.0],
    });
    let mut r = Lcg(7);
    let palette = [
        [0.85, 0.45, 0.35],
        [0.35, 0.55, 0.85],
        [0.90, 0.78, 0.35],
        [0.60, 0.45, 0.80],
        [0.75, 0.75, 0.78],
        [0.35, 0.72, 0.62],
    ];
    for _ in 0..70 {
        let x = (r.next() - 0.5) * 88.0;
        let z = (r.next() - 0.5) * 88.0;
        if x.abs() < 5.0 && z.abs() < 5.0 {
            continue;
        }
        let big = r.next() > 0.7;
        let (sx, sz) = if big { (4.0 + r.next() * 5.0, 4.0 + r.next() * 5.0) } else { (1.2 + r.next() * 1.6, 1.2 + r.next() * 1.6) };
        let sy = if big { 3.0 + r.next() * 7.0 } else { 1.0 + r.next() * 1.8 };
        let c = palette[(r.next() * palette.len() as f32) as usize % palette.len()];
        v.push(boxi(x, z, sx, sy, sz, c));
    }
    // daraxtlar: tanasi + 2 qavat yaproq
    for _ in 0..45 {
        let x = (r.next() - 0.5) * 90.0;
        let z = (r.next() - 0.5) * 90.0;
        if x.abs() < 6.0 && z.abs() < 10.0 {
            continue;
        }
        let h = 1.6 + r.next() * 1.4;
        v.push(boxi(x, z, 0.35, h, 0.35, [0.45, 0.30, 0.18]));
        let g = 0.45 + r.next() * 0.25;
        v.push(Instance { pos: [x, h + 0.7, z, 0.0], scale: [2.0, 1.4, 2.0, 0.0], color: [0.2, g, 0.22, 0.0] });
        v.push(Instance { pos: [x, h + 1.7, z, 0.0], scale: [1.2, 1.2, 1.2, 0.0], color: [0.22, g + 0.08, 0.25, 0.0] });
    }
    // markaziy belgi: baland ustun
    v.push(boxi(0.0, -10.0, 1.5, 6.0, 1.5, [0.95, 0.35, 0.30]));
    v
}
