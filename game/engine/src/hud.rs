//! Ekran ustidagi 2D elementlar: joystik va nishon (piksel koordinatalarida).

use crate::input::Input;
use bytemuck::{Pod, Zeroable};
use glam::Vec2;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct HudVertex {
    pub pos: [f32; 2],
    pub color: [f32; 4],
}

pub const MAX_HUD_VERTICES: usize = 4096;

fn push_tri(out: &mut Vec<HudVertex>, a: Vec2, b: Vec2, c: Vec2, col: [f32; 4]) {
    for p in [a, b, c] {
        out.push(HudVertex { pos: p.to_array(), color: col });
    }
}

fn disc(out: &mut Vec<HudVertex>, center: Vec2, r: f32, col: [f32; 4]) {
    const N: usize = 40;
    for k in 0..N {
        let a0 = k as f32 / N as f32 * std::f32::consts::TAU;
        let a1 = (k + 1) as f32 / N as f32 * std::f32::consts::TAU;
        push_tri(
            out,
            center,
            center + Vec2::new(a0.cos(), a0.sin()) * r,
            center + Vec2::new(a1.cos(), a1.sin()) * r,
            col,
        );
    }
}

fn ring(out: &mut Vec<HudVertex>, center: Vec2, r: f32, w: f32, col: [f32; 4]) {
    const N: usize = 48;
    for k in 0..N {
        let a0 = k as f32 / N as f32 * std::f32::consts::TAU;
        let a1 = (k + 1) as f32 / N as f32 * std::f32::consts::TAU;
        let d0 = Vec2::new(a0.cos(), a0.sin());
        let d1 = Vec2::new(a1.cos(), a1.sin());
        let (o0, o1) = (center + d0 * r, center + d1 * r);
        let (i0, i1) = (center + d0 * (r - w), center + d1 * (r - w));
        push_tri(out, o0, o1, i0, col);
        push_tri(out, o1, i1, i0, col);
    }
}

fn rect(out: &mut Vec<HudVertex>, c: Vec2, half: Vec2, col: [f32; 4]) {
    let (a, b) = (c - half, c + half);
    let (p, q) = (Vec2::new(b.x, a.y), Vec2::new(a.x, b.y));
    push_tri(out, a, p, b, col);
    push_tri(out, a, b, q, col);
}

/// HUD uchlarini yig'adi. `w`,`h` — ekran o'lchami (piksel).
pub fn build(input: &Input, w: f32, h: f32, density: f32) -> Vec<HudVertex> {
    let mut v = Vec::with_capacity(1024);
    let white = [1.0, 1.0, 1.0, 0.85];
    let (origin, knob, radius) = input.stick_visual().unwrap_or((
        Vec2::new(w * 0.17, h * 0.72),
        Vec2::new(w * 0.17, h * 0.72),
        56.0 * density,
    ));
    let active = input.stick_visual().is_some();
    let a = if active { 1.0 } else { 0.45 };
    disc(&mut v, origin, radius, [1.0, 1.0, 1.0, 0.10 * a]);
    ring(&mut v, origin, radius, 2.0 * density, [1.0, 1.0, 1.0, 0.45 * a]);
    disc(&mut v, knob, radius * 0.42, [1.0, 1.0, 1.0, 0.40 * a]);

    let c = Vec2::new(w * 0.5, h * 0.5);
    let (gap, len, t) = (4.0 * density, 9.0 * density, 1.2 * density);
    rect(&mut v, c + Vec2::new(gap + len * 0.5, 0.0), Vec2::new(len * 0.5, t), white);
    rect(&mut v, c - Vec2::new(gap + len * 0.5, 0.0), Vec2::new(len * 0.5, t), white);
    rect(&mut v, c + Vec2::new(0.0, gap + len * 0.5), Vec2::new(t, len * 0.5), white);
    rect(&mut v, c - Vec2::new(0.0, gap + len * 0.5), Vec2::new(t, len * 0.5), white);
    v.truncate(MAX_HUD_VERTICES);
    v
}
