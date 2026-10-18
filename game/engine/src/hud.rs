//! Ekran ustidagi 2D elementlar: joystik va nishon (piksel koordinatalarida).

use crate::game::Game;
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

fn rect_tl(out: &mut Vec<HudVertex>, x: f32, y: f32, w: f32, h: f32, col: [f32; 4]) {
    rect(out, Vec2::new(x + w * 0.5, y + h * 0.5), Vec2::new(w * 0.5, h * 0.5), col);
}

/// HUD uchlarini yig'adi (piksel koordinatalarida).
pub fn build(g: &Game) -> Vec<HudVertex> {
    let (w, h, d) = (g.width as f32, g.height as f32, g.density);
    let input = &g.input;
    let mut v = Vec::with_capacity(2048);
    let white = [1.0, 1.0, 1.0, 0.85];

    // qizil zarba va o'lim parda
    if g.hurt > 0.0 {
        rect_tl(&mut v, 0.0, 0.0, w, h, [0.9, 0.0, 0.0, 0.35 * (g.hurt / 0.35).min(1.0)]);
    }

    // joystik
    let (origin, knob, radius) = input.stick_visual().unwrap_or((
        Vec2::new(w * 0.17, h * 0.72),
        Vec2::new(w * 0.17, h * 0.72),
        56.0 * d,
    ));
    let a = if input.stick_visual().is_some() { 1.0 } else { 0.45 };
    disc(&mut v, origin, radius, [1.0, 1.0, 1.0, 0.10 * a]);
    ring(&mut v, origin, radius, 2.0 * d, [1.0, 1.0, 1.0, 0.45 * a]);
    disc(&mut v, knob, radius * 0.42, [1.0, 1.0, 1.0, 0.40 * a]);

    // nishon
    let c = Vec2::new(w * 0.5, h * 0.5);
    let (gap, len, t) = (4.0 * d, 9.0 * d, 1.2 * d);
    rect(&mut v, c + Vec2::new(gap + len * 0.5, 0.0), Vec2::new(len * 0.5, t), white);
    rect(&mut v, c - Vec2::new(gap + len * 0.5, 0.0), Vec2::new(len * 0.5, t), white);
    rect(&mut v, c + Vec2::new(0.0, gap + len * 0.5), Vec2::new(t, len * 0.5), white);
    rect(&mut v, c - Vec2::new(0.0, gap + len * 0.5), Vec2::new(t, len * 0.5), white);

    // tegish belgisi
    if g.hit_marker > 0.0 {
        let red = [1.0, 0.25, 0.2, 0.95];
        for (sx, sy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            rect(&mut v, c + Vec2::new(sx, sy) * 14.0 * d, Vec2::splat(2.2 * d), red);
        }
    }

    // tugmalar
    let (fc, fr) = input.fire_button();
    let fa = if input.fire_held() { 0.75 } else { 0.45 };
    disc(&mut v, fc, fr, [0.95, 0.25, 0.2, fa * 0.6]);
    ring(&mut v, fc, fr, 2.5 * d, [1.0, 1.0, 1.0, fa]);
    disc(&mut v, fc, fr * 0.28, [1.0, 1.0, 1.0, fa]);
    let (jc, jr) = input.jump_button();
    disc(&mut v, jc, jr, [1.0, 1.0, 1.0, 0.18]);
    ring(&mut v, jc, jr, 2.0 * d, [1.0, 1.0, 1.0, 0.5]);
    push_tri(
        &mut v,
        jc + Vec2::new(0.0, -jr * 0.45),
        jc + Vec2::new(jr * 0.45, jr * 0.3),
        jc + Vec2::new(-jr * 0.45, jr * 0.3),
        [1.0, 1.0, 1.0, 0.8],
    );

    // jon va o'q-dori
    let (bx, by, bw, bh) = (20.0 * d, 20.0 * d, 200.0 * d, 12.0 * d);
    rect_tl(&mut v, bx, by, bw, bh, [0.0, 0.0, 0.0, 0.45]);
    let f = (g.hp / crate::game::MAX_HP).clamp(0.0, 1.0);
    rect_tl(&mut v, bx, by, bw * f, bh, [1.0 - f, 0.2 + 0.7 * f, 0.2, 0.95]);
    let (ay, ah) = (by + bh + 6.0 * d, 6.0 * d);
    rect_tl(&mut v, bx, ay, bw * 0.7, ah, [0.0, 0.0, 0.0, 0.45]);
    let af = if g.reload > 0.0 {
        1.0 - (g.reload / 1.6).clamp(0.0, 1.0)
    } else {
        g.ammo as f32 / crate::game::MAG_SIZE as f32
    };
    let ac = if g.reload > 0.0 { [0.4, 0.7, 1.0, 0.95] } else { [1.0, 0.85, 0.3, 0.95] };
    rect_tl(&mut v, bx, ay, bw * 0.7 * af, ah, ac);

    // o'ldirilgan botlar: sariq kvadratchalar (tepa o'rtada)
    let n = g.kills.min(20) as f32;
    let sq = 10.0 * d;
    let x0 = w * 0.5 - n * (sq + 3.0 * d) * 0.5;
    for k in 0..g.kills.min(20) {
        rect_tl(&mut v, x0 + k as f32 * (sq + 3.0 * d), 18.0 * d, sq, sq, [1.0, 0.85, 0.2, 0.95]);
    }

    if g.dead_t > 0.0 {
        rect_tl(&mut v, 0.0, 0.0, w, h, [0.0, 0.0, 0.0, 0.6]);
    }
    v.truncate(MAX_HUD_VERTICES);
    v
}
