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

pub const MAX_HUD_VERTICES: usize = 8192;

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

fn line(out: &mut Vec<HudVertex>, a: Vec2, b: Vec2, w: f32, col: [f32; 4]) {
    let d = (b - a).normalize_or_zero();
    let n = Vec2::new(-d.y, d.x) * w * 0.5;
    push_tri(out, a + n, b + n, b - n, col);
    push_tri(out, a + n, b - n, a - n, col);
}

fn ring_rect(out: &mut Vec<HudVertex>, tl: Vec2, size: f32, w: f32, col: [f32; 4]) {
    rect_tl(out, tl.x - w, tl.y - w, size + 2.0 * w, w, col);
    rect_tl(out, tl.x - w, tl.y + size, size + 2.0 * w, w, col);
    rect_tl(out, tl.x - w, tl.y, w, size, col);
    rect_tl(out, tl.x + size, tl.y, w, size, col);
}

fn rect_tl(out: &mut Vec<HudVertex>, x: f32, y: f32, w: f32, h: f32, col: [f32; 4]) {
    rect(out, Vec2::new(x + w * 0.5, y + h * 0.5), Vec2::new(w * 0.5, h * 0.5), col);
}

fn gear_icon(v: &mut Vec<HudVertex>, c: Vec2, r: f32, col: [f32; 4]) {
    for k in 0..8 {
        let a = k as f32 / 8.0 * std::f32::consts::TAU;
        disc(v, c + Vec2::new(a.cos(), a.sin()) * r * 0.82, r * 0.2, col);
    }
    ring(v, c, r * 0.72, r * 0.26, col);
    disc(v, c, r * 0.2, col);
}

fn action_buttons(v: &mut Vec<HudVertex>, g: &Game, highlight: bool) {
    let d = g.density;
    let input = &g.input;
    let hl = [1.0, 0.85, 0.2, 0.95];
    let (fc, fr) = input.fire_button();
    let fa = if input.fire_held() { 0.75 } else { 0.45 };
    disc(v, fc, fr, [0.95, 0.25, 0.2, fa * 0.6]);
    ring(v, fc, fr, 2.5 * d, [1.0, 1.0, 1.0, fa]);
    disc(v, fc, fr * 0.28, [1.0, 1.0, 1.0, fa]);
    let (jc, jr) = input.jump_button();
    disc(v, jc, jr, [1.0, 1.0, 1.0, 0.18]);
    ring(v, jc, jr, 2.0 * d, [1.0, 1.0, 1.0, 0.5]);
    push_tri(
        v,
        jc + Vec2::new(0.0, -jr * 0.45),
        jc + Vec2::new(jr * 0.45, jr * 0.3),
        jc + Vec2::new(-jr * 0.45, jr * 0.3),
        [1.0, 1.0, 1.0, 0.8],
    );
    let (sc, sr) = input.switch_button();
    disc(v, sc, sr, [1.0, 1.0, 1.0, 0.18]);
    ring(v, sc, sr, 2.0 * d, [1.0, 1.0, 1.0, 0.5]);
    let cc = g.slots[g.cur].kind.color();
    disc(v, sc, sr * 0.4, [cc[0], cc[1], cc[2], 0.95]);
    if highlight {
        for (c, r) in [(fc, fr), (jc, jr), (sc, sr)] {
            ring(v, c, r + 6.0 * d, 3.0 * d, hl);
        }
    }
}


fn minimap(v: &mut Vec<HudVertex>, g: &Game) {
    let d = g.density;
    // kichik xarita (yuqori o'ng): ko'rinadigan maydon ±72 m
    let (tl, msz) = g.input.map_rect();
    let (mx, my) = (tl.x, tl.y);
    let k = msz / 300.0;
    let mc = Vec2::new(mx + msz * 0.5, my + msz * 0.5);
    rect_tl(v, mx, my, msz, msz, [0.0, 0.0, 0.0, 0.4]);
    let wh = crate::input::WORLD_HALF * k;
    rect(v, mc, Vec2::splat(wh), [0.35, 0.6, 0.3, 0.45]);
    let to_map = |x: f32, z: f32| mc + Vec2::new(x, z) * k;
    let z = &g.zone;
    ring(v, to_map(z.center.x, z.center.y), (z.radius * k).max(2.0 * d), 1.6 * d, [0.3, 0.5, 1.0, 0.95]);
    if !z.finished {
        ring(v, to_map(z.next_center.x, z.next_center.y), (z.next_radius * k).max(2.0 * d), 1.2 * d, [1.0, 1.0, 1.0, 0.95]);
    }
    let pm = to_map(g.camera.pos.x, g.camera.pos.z);
    let f = g.camera.forward();
    let fd = Vec2::new(f.x, f.z).normalize_or_zero();
    let side = Vec2::new(-fd.y, fd.x);
    push_tri(v, pm + fd * 6.0 * d, pm - fd * 4.0 * d + side * 4.0 * d, pm - fd * 4.0 * d - side * 4.0 * d, [1.0, 0.9, 0.2, 1.0]);
    // zona taymeri: kutish — oq, qisqarish — qizil
    let (zx, zy) = (mx, my + msz + 6.0 * d);
    rect_tl(v, zx, zy, msz, 6.0 * d, [0.0, 0.0, 0.0, 0.45]);
    let zc = if z.shrinking { [1.0, 0.3, 0.25, 0.95] } else { [1.0, 1.0, 1.0, 0.95] };
    rect_tl(v, zx, zy, msz * z.progress(), 6.0 * d, zc);

}

/// Sozlamalar rejimi: tugmalarni sudrab joylashtirish.
fn settings_overlay(v: &mut Vec<HudVertex>, g: &Game) {
    let (w, h, d) = (g.width as f32, g.height as f32, g.density);
    rect_tl(v, 0.0, 0.0, w, h, [0.0, 0.0, 0.0, 0.62]);
    action_buttons(v, g, true);
    minimap(v, g);
    let (tl, msz) = g.input.map_rect();
    ring_rect(v, tl, msz, 3.0 * d, [1.0, 0.85, 0.2, 0.95]);
    for (act, c, r) in g.input.settings_controls() {
        disc(v, c, r, [1.0, 1.0, 1.0, 0.18]);
        ring(v, c, r, 2.0 * d, [1.0, 1.0, 1.0, 0.7]);
        let ic = [1.0, 1.0, 1.0, 0.95];
        match act {
            crate::input::SettingsAction::Reset => {
                ring(v, c, r * 0.5, r * 0.14, ic);
                push_tri(v, c + Vec2::new(r * 0.5, -r * 0.5), c + Vec2::new(r * 0.78, -r * 0.1), c + Vec2::new(r * 0.22, -r * 0.1), ic);
            }
            crate::input::SettingsAction::Smaller => rect(v, c, Vec2::new(r * 0.45, r * 0.08), ic),
            crate::input::SettingsAction::Bigger => {
                rect(v, c, Vec2::new(r * 0.45, r * 0.08), ic);
                rect(v, c, Vec2::new(r * 0.08, r * 0.45), ic);
            }
            crate::input::SettingsAction::Done => {
                let gc = [0.4, 1.0, 0.5, 1.0];
                line(v, c + Vec2::new(-r * 0.42, 0.0), c + Vec2::new(-r * 0.1, r * 0.32), r * 0.16, gc);
                line(v, c + Vec2::new(-r * 0.1, r * 0.32), c + Vec2::new(r * 0.44, -r * 0.34), r * 0.16, gc);
            }
        }
    }
    // joystik (sudrab joylashtiriladi)
    let (sc, sr) = g.input.stick_home();
    disc(v, sc, sr, [1.0, 1.0, 1.0, 0.10]);
    ring(v, sc, sr, 2.0 * d, [1.0, 1.0, 1.0, 0.45]);
    disc(v, sc, sr * 0.42, [1.0, 1.0, 1.0, 0.4]);
    ring(v, sc, sr + 6.0 * d, 3.0 * d, [1.0, 0.85, 0.2, 0.95]);
}

/// HUD uchlarini yig'adi (piksel koordinatalarida).
pub fn build(g: &Game) -> Vec<HudVertex> {
    let (w, h, d) = (g.width as f32, g.height as f32, g.density);
    let input = &g.input;
    let mut v = Vec::with_capacity(2048);
    if input.settings_open() {
        settings_overlay(&mut v, g);
        v.truncate(MAX_HUD_VERTICES);
        return v;
    }
    let white = [1.0, 1.0, 1.0, 0.85];

    // qizil zarba va o'lim parda
    if g.hurt > 0.0 {
        rect_tl(&mut v, 0.0, 0.0, w, h, [0.9, 0.0, 0.0, 0.35 * (g.hurt / 0.35).min(1.0)]);
    }

    // joystik
    let (home, hr) = input.stick_home();
    let (origin, knob, radius) = input.stick_visual().unwrap_or((home, home, hr));
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

    // otish: tracer va yorug'lik chaqnashi
    if g.flash > 0.0 {
        let k = g.flash / 0.07;
        line(&mut v, Vec2::new(w * 0.70, h * 0.88), c, 3.0 * d, [1.0, 0.9, 0.5, 0.8 * k]);
        rect_tl(&mut v, 0.0, 0.0, w, h, [1.0, 0.95, 0.8, 0.07 * k]);
    }

    action_buttons(&mut v, g, false);
    let (gc, gr) = input.gear_button();
    disc(&mut v, gc, gr, [0.0, 0.0, 0.0, 0.35]);
    gear_icon(&mut v, gc, gr * 0.85, [1.0, 1.0, 1.0, 0.85]);

    // jon va o'q-dori
    let (bx, by, bw, bh) = (20.0 * d, 20.0 * d, 200.0 * d, 12.0 * d);
    rect_tl(&mut v, bx, by, bw, bh, [0.0, 0.0, 0.0, 0.45]);
    let f = (g.hp / crate::game::MAX_HP).clamp(0.0, 1.0);
    rect_tl(&mut v, bx, by, bw * f, bh, [1.0 - f, 0.2 + 0.7 * f, 0.2, 0.95]);
    let (ay, ah) = (by + bh + 6.0 * d, 6.0 * d);
    let st = g.stats();
    rect_tl(&mut v, bx, ay, bw * 0.7, ah, [0.0, 0.0, 0.0, 0.45]);
    let af = if g.reload > 0.0 {
        1.0 - (g.reload / st.reload).clamp(0.0, 1.0)
    } else {
        g.mag() as f32 / st.mag as f32
    };
    let ac = if g.reload > 0.0 { [0.4, 0.7, 1.0, 0.95] } else { [1.0, 0.85, 0.3, 0.95] };
    rect_tl(&mut v, bx, ay, bw * 0.7 * af, ah, ac);
    // zaxira: har bir kvadrat = bitta magazin
    let pips = ((g.reserve() + st.mag - 1) / st.mag).min(10);
    for k in 0..pips {
        rect_tl(&mut v, bx + k as f32 * 10.0 * d, ay + ah + 4.0 * d, 7.0 * d, 7.0 * d, [0.85, 0.85, 0.9, 0.85]);
    }
    // qurol uyalari
    for (k, s) in g.slots.iter().enumerate() {
        let c = s.kind.color();
        let sz = if k == g.cur { 22.0 * d } else { 14.0 * d };
        let x = bx + k as f32 * 30.0 * d;
        let y = ay + ah + 20.0 * d + (22.0 * d - sz);
        rect_tl(&mut v, x, y, sz, sz, [c[0], c[1], c[2], if k == g.cur { 1.0 } else { 0.6 }]);
    }
    // zonadan tashqarida — ko'k parda
    if !g.zone.contains(g.camera.pos.x, g.camera.pos.z) {
        rect_tl(&mut v, 0.0, 0.0, w, h, [0.1, 0.3, 1.0, 0.22]);
    }

    minimap(&mut v, g);

    if g.dead_t > 0.0 {
        rect_tl(&mut v, 0.0, 0.0, w, h, [0.0, 0.0, 0.0, 0.6]);
    }
    v.truncate(MAX_HUD_VERTICES);
    v
}
