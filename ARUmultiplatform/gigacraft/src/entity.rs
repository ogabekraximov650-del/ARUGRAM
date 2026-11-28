//! Jonzotlar (hayvonlar va dushmanlar) va zarrachalar: harakat, sun'iy intellekt, ko'rinish (quti-modellar).
use crate::blocks::*;
use crate::math::*;
use crate::mesh::{rel, Vertex};
use crate::planet::*;
use crate::world::World;

pub const COW: u8 = 0;
pub const SHEEP: u8 = 1;
pub const CHICKEN: u8 = 2;
pub const ZOMBIE: u8 = 3;

pub fn hostile(kind: u8) -> bool {
    kind == ZOMBIE
}

pub fn max_hp(kind: u8) -> f32 {
    match kind {
        COW => 10.0,
        SHEEP => 8.0,
        CHICKEN => 4.0,
        _ => 20.0,
    }
}

/// Yurish tezligi (blok/soniya).
pub fn speed(kind: u8) -> f64 {
    match kind {
        COW => 5.0,
        SHEEP => 5.5,
        CHICKEN => 6.0,
        _ => 10.5,
    }
}

/// Zarba doirasi radiusi (blok) va balandligi.
pub fn size(kind: u8) -> (f64, f64) {
    match kind {
        COW => (2.2, 5.0),
        SHEEP => (2.0, 4.6),
        CHICKEN => (1.0, 3.0),
        _ => (1.2, 7.4),
    }
}

pub fn name(kind: u8) -> &'static str {
    match kind {
        COW => "SIGIR",
        SHEEP => "QO'Y",
        CHICKEN => "TOVUQ",
        _ => "ZOMBI",
    }
}

#[derive(Clone, Debug)]
pub struct Mob {
    pub kind: u8,
    /// Oyoq ostidagi nuqta (dunyo koordinatalari).
    pub pos: D3,
    /// Qarash yo'nalishi (tangens, birlik).
    pub fwd: D3,
    pub hp: f32,
    /// Keyingi qaror vaqti.
    pub t: f32,
    pub walking: bool,
    pub hurt: f32,
    pub cd: f32,
    pub phase: f32,
    /// Juftlashishga tayyor (soniya qoldi).
    pub love: f32,
    /// Bola: katta bo'lishigacha soniya (0 — katta).
    pub baby: f32,
}

impl Mob {
    pub fn new(kind: u8, pos: D3, fwd: D3) -> Mob {
        let mut m = Mob { kind, pos, fwd, hp: max_hp(kind), t: 1.0, walking: false, hurt: 0.0, cd: 0.0, phase: 0.0, love: 0.0, baby: 0.0 };
        m.fix_fwd();
        m
    }

    pub fn up(&self) -> D3 {
        d_norm(self.pos)
    }

    /// Bola kichik.
    pub fn scale(&self) -> f64 {
        if self.baby > 0.0 {
            0.55
        } else {
            1.0
        }
    }

    fn fix_fwd(&mut self) {
        let up = self.up();
        let f = d_sub(self.fwd, d_mul(up, d_dot(self.fwd, up)));
        self.fwd = if d_len(f) < 1e-9 { d_norm(d_cross(up, [0.0, 0.0, 1.0])) } else { d_norm(f) };
    }

    /// Markaz (zarba/qarash uchun).
    pub fn center(&self) -> D3 {
        d_add(self.pos, d_mul(self.up(), size(self.kind).1 * 0.5 * self.scale()))
    }
}

/// Sodda tasodifiy son [0,1).
pub fn rnd(s: &mut u32) -> f32 {
    *s ^= *s << 13;
    *s ^= *s >> 17;
    *s ^= *s << 5;
    (*s >> 8) as f32 / (1u32 << 24) as f32
}

/// Berilgan nuqta ustunidagi tayanch sirt (tepasi): qattiq blok ustidagi havo. Suv yoki g'or bo'lsa `None`.
pub fn ground(w: &World, p: D3, land_only: bool) -> Option<D3> {
    let c = cell_at(d_add(p, d_mul(d_norm(p), 0.6)));
    for k in (-4..=4i64).rev() {
        let cell = Cell { f: c.f, u: c.u, v: c.v, r: c.r + k };
        let b = w.block(&cell);
        if solid(b) {
            let above = w.block(&Cell { r: cell.r + 1, ..cell });
            let above2 = w.block(&Cell { r: cell.r + 2, ..cell });
            if solid(above) || solid(above2) {
                return None;
            }
            if land_only && (above == WATER) {
                return None;
            }
            let d = d_norm(center(&cell));
            return Some(d_mul(d, cell.r as f64 + 1.0));
        }
        if b == WATER && land_only {
            return None;
        }
    }
    None
}

/// Bir qadam: o'yinchiga qarab yoki tasodifiy yuradi. `target` — hujum qiladigan (dushman) uchun o'yinchi nuqtasi.
/// Dushman o'yinchiga yetsa `true` qaytaradi (zarba berish kerak).
pub fn step(m: &mut Mob, w: &World, dt: f32, target: Option<D3>, seed: &mut u32) -> bool {
    m.hurt = (m.hurt - dt).max(0.0);
    m.cd = (m.cd - dt).max(0.0);
    m.love = (m.love - dt).max(0.0);
    m.baby = (m.baby - dt).max(0.0);
    let up = m.up();
    let mut hit = false;
    if let (true, Some(t)) = (hostile(m.kind), target) {
        let to = d_sub(t, m.pos);
        let flat = d_sub(to, d_mul(up, d_dot(to, up)));
        let dist = d_len(flat);
        if dist < 90.0 {
            m.fwd = d_norm(flat);
            m.walking = dist > 3.2;
            if dist <= 3.6 && m.cd <= 0.0 && d_dot(to, up).abs() < 8.0 {
                m.cd = 1.0;
                hit = true;
            }
        } else {
            wander(m, dt, seed);
        }
    } else {
        wander(m, dt, seed);
    }
    if m.walking {
        let np = d_add(m.pos, d_mul(m.fwd, speed(m.kind) * dt as f64));
        let np = d_mul(d_norm(np), d_len(m.pos));
        match ground(w, np, true) {
            Some(g) if (d_len(g) - d_len(m.pos)).abs() <= 2.6 => {
                m.pos = d_mul(d_norm(np), d_len(g));
                m.phase += dt * 8.0;
            }
            _ => {
                // To'siq: burilib olamiz
                let a = (rnd(seed) - 0.5) * 3.0;
                let side = d_cross(up, m.fwd);
                m.fwd = d_norm(d_add(d_mul(m.fwd, a.cos() as f64), d_mul(side, a.sin() as f64)));
                m.t = 0.3;
            }
        }
        m.fix_fwd();
    } else if let Some(g) = ground(w, m.pos, true) {
        m.pos = d_mul(d_norm(m.pos), d_len(g));
    }
    hit
}

fn wander(m: &mut Mob, dt: f32, seed: &mut u32) {
    m.t -= dt;
    if m.t <= 0.0 {
        m.t = 1.5 + rnd(seed) * 4.0;
        m.walking = rnd(seed) < 0.6;
        let a = rnd(seed) * std::f32::consts::TAU;
        let up = m.up();
        let side = d_cross(up, m.fwd);
        m.fwd = d_norm(d_add(d_mul(m.fwd, a.cos() as f64), d_mul(side, a.sin() as f64)));
    }
}

/// Nur (nuqta `o`, yo'nalish `d` birlik) jonzotga urildimi: masofa.
pub fn ray_hit(m: &Mob, o: D3, d: D3, reach: f64) -> Option<f64> {
    let (rad, h) = size(m.kind);
    let (rad, h) = (rad * m.scale(), h * m.scale());
    let up = m.up();
    // Tik silindr: eng yaqin nuqtani topamiz
    let mut best: Option<f64> = None;
    let mut t = 0.0;
    while t <= reach {
        let p = d_add(o, d_mul(d, t));
        let rel = d_sub(p, m.pos);
        let y = d_dot(rel, up);
        let hor = d_sub(rel, d_mul(up, y));
        if y >= -0.2 && y <= h + 0.2 && d_len(hor) <= rad {
            best = Some(t);
            break;
        }
        t += 0.4;
    }
    best
}

fn push_box(out: &mut Vec<Vertex>, anchor: D3, c: D3, ax: (D3, D3, D3), half: [f64; 3], col: [f32; 3]) {
    let (r, u, f) = ax;
    let corner = |sx: f64, sy: f64, sz: f64| -> D3 {
        d_add(c, d_add(d_mul(r, sx * half[0]), d_add(d_mul(u, sy * half[1]), d_mul(f, sz * half[2]))))
    };
    // (normal ignishi, 4 burchak)
    let faces: [([f64; 3], [[f64; 3]; 4], f32); 6] = [
        ([0.0, 1.0, 0.0], [[-1.0, 1.0, -1.0], [1.0, 1.0, -1.0], [1.0, 1.0, 1.0], [-1.0, 1.0, 1.0]], 1.0),
        ([0.0, -1.0, 0.0], [[-1.0, -1.0, -1.0], [1.0, -1.0, -1.0], [1.0, -1.0, 1.0], [-1.0, -1.0, 1.0]], 0.62),
        ([1.0, 0.0, 0.0], [[1.0, -1.0, -1.0], [1.0, 1.0, -1.0], [1.0, 1.0, 1.0], [1.0, -1.0, 1.0]], 0.86),
        ([-1.0, 0.0, 0.0], [[-1.0, -1.0, -1.0], [-1.0, 1.0, -1.0], [-1.0, 1.0, 1.0], [-1.0, -1.0, 1.0]], 0.86),
        ([0.0, 0.0, 1.0], [[-1.0, -1.0, 1.0], [1.0, -1.0, 1.0], [1.0, 1.0, 1.0], [-1.0, 1.0, 1.0]], 0.94),
        ([0.0, 0.0, -1.0], [[-1.0, -1.0, -1.0], [1.0, -1.0, -1.0], [1.0, 1.0, -1.0], [-1.0, 1.0, -1.0]], 0.74),
    ];
    for (n, cs, shade) in faces {
        let nd = d_add(d_mul(r, n[0]), d_add(d_mul(u, n[1]), d_mul(f, n[2])));
        let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        for k in cs {
            let p = corner(k[0], k[1], k[2]);
            out.push(Vertex {
                p: rel(p, anchor),
                c: [q(col[0] * shade), q(col[1] * shade), q(col[2] * shade), 255],
                n: [(nd[0] * 127.0) as i8, (nd[1] * 127.0) as i8, (nd[2] * 127.0) as i8, 0],
            });
        }
    }
}

type Part = ([f32; 3], [f32; 3], [f32; 3], bool);

/// Jonzot qismlari: (siljish [o'ng, tepa, old], yarim o'lchamlar, rang, oyoqmi).
fn parts(kind: u8) -> Vec<Part> {
    match kind {
        COW => {
            let b = [0.92, 0.92, 0.92];
            let d = [0.30, 0.22, 0.18];
            vec![
                ([0.0, 3.5, 0.0], [1.1, 1.2, 2.2], b, false),
                ([0.0, 3.9, -0.4], [1.12, 0.5, 0.9], d, false),
                ([0.0, 4.5, 2.9], [0.8, 0.8, 0.8], d, false),
                ([-1.0, 1.1, 1.6], [0.4, 1.1, 0.4], d, true),
                ([1.0, 1.1, 1.6], [0.4, 1.1, 0.4], d, true),
                ([-1.0, 1.1, -1.6], [0.4, 1.1, 0.4], d, true),
                ([1.0, 1.1, -1.6], [0.4, 1.1, 0.4], d, true),
            ]
        }
        SHEEP => {
            let w = [0.96, 0.95, 0.92];
            let d = [0.35, 0.30, 0.28];
            vec![
                ([0.0, 3.2, 0.0], [1.3, 1.3, 1.9], w, false),
                ([0.0, 4.0, 2.4], [0.7, 0.7, 0.8], d, false),
                ([-0.8, 1.0, 1.3], [0.35, 1.0, 0.35], d, true),
                ([0.8, 1.0, 1.3], [0.35, 1.0, 0.35], d, true),
                ([-0.8, 1.0, -1.3], [0.35, 1.0, 0.35], d, true),
                ([0.8, 1.0, -1.3], [0.35, 1.0, 0.35], d, true),
            ]
        }
        CHICKEN => vec![
            ([0.0, 1.6, 0.0], [0.7, 0.7, 0.9], [0.98, 0.98, 0.95], false),
            ([0.0, 2.6, 0.9], [0.4, 0.45, 0.4], [0.98, 0.98, 0.95], false),
            ([0.0, 3.1, 0.9], [0.12, 0.2, 0.25], [0.9, 0.15, 0.15], false),
            ([-0.3, 0.45, 0.0], [0.1, 0.45, 0.1], [0.95, 0.75, 0.2], true),
            ([0.3, 0.45, 0.0], [0.1, 0.45, 0.1], [0.95, 0.75, 0.2], true),
        ],
        _ => {
            let skin = [0.42, 0.66, 0.40];
            vec![
                ([0.0, 4.7, 0.0], [1.0, 1.5, 0.5], [0.22, 0.50, 0.60], false),
                ([0.0, 7.0, 0.0], [0.8, 0.8, 0.8], skin, false),
                ([-1.45, 5.2, 1.0], [0.4, 0.4, 1.4], skin, false),
                ([1.45, 5.2, 1.0], [0.4, 0.4, 1.4], skin, false),
                ([-0.5, 1.6, 0.0], [0.45, 1.6, 0.45], [0.20, 0.22, 0.55], true),
                ([0.5, 1.6, 0.0], [0.45, 1.6, 0.45], [0.20, 0.22, 0.55], true),
            ]
        }
    }
}

pub fn mob_vertices(m: &Mob, anchor: D3, out: &mut Vec<Vertex>) {
    let up = m.up();
    let right = d_norm(d_cross(up, m.fwd));
    let ax = (right, up, m.fwd);
    let flash = if m.hurt > 0.0 { 0.55 } else { 0.0 };
    let sc = m.scale() as f32;
    for (i, (off, half, col, leg)) in parts(m.kind).into_iter().enumerate() {
        let off = [off[0] * sc, off[1] * sc, off[2] * sc];
        let half = [half[0] * sc, half[1] * sc, half[2] * sc];
        let swing = if leg && m.walking { (m.phase + i as f32 * 1.7).sin() * 0.7 } else { 0.0 };
        let c = d_add(m.pos, d_add(d_mul(right, off[0] as f64), d_add(d_mul(up, off[1] as f64), d_mul(m.fwd, (off[2] + swing) as f64))));
        let lv = if m.love > 0.0 { 0.35 } else { 0.0 };
        let col = [col[0] + (1.0 - col[0]) * (flash * 0.7 + lv), col[1] * (1.0 - flash) * (1.0 - lv * 0.4), col[2] * (1.0 - flash) * (1.0 - lv * 0.2)];
        push_box(out, anchor, c, ax, [half[0] as f64, half[1] as f64, half[2] as f64], col);
    }
}

#[derive(Clone, Debug)]
pub struct Particle {
    pub pos: D3,
    pub vel: D3,
    pub life: f32,
    pub col: [f32; 3],
    pub size: f32,
}

pub fn burst(out: &mut Vec<Particle>, at: D3, col: [f32; 3], n: usize, seed: &mut u32) {
    let up = d_norm(at);
    let tu = d_norm(d_cross(up, [0.0, 0.0, 1.0]));
    let tv = d_cross(up, tu);
    for _ in 0..n {
        let (a, b, c) = (rnd(seed) - 0.5, rnd(seed), rnd(seed) - 0.5);
        let v = d_add(d_mul(tu, a as f64 * 16.0), d_add(d_mul(up, 8.0 + b as f64 * 14.0), d_mul(tv, c as f64 * 16.0)));
        let k = 0.8 + rnd(seed) * 0.4;
        out.push(Particle { pos: at, vel: v, life: 0.5 + rnd(seed) * 0.5, col: [col[0] * k, col[1] * k, col[2] * k], size: 0.35 });
    }
    if out.len() > 400 {
        let cut = out.len() - 400;
        out.drain(0..cut);
    }
}

pub fn step_particles(ps: &mut Vec<Particle>, dt: f32) {
    for p in ps.iter_mut() {
        let up = d_norm(p.pos);
        p.vel = d_sub(p.vel, d_mul(up, 39.2 * dt as f64));
        p.pos = d_add(p.pos, d_mul(p.vel, dt as f64));
        p.life -= dt;
    }
    ps.retain(|p| p.life > 0.0);
}

pub fn particle_vertices(ps: &[Particle], anchor: D3, out: &mut Vec<Vertex>) {
    for p in ps {
        let up = d_norm(p.pos);
        let r = d_norm(d_cross(up, [0.3, 0.1, 0.9]));
        let f = d_cross(r, up);
        let s = p.size as f64 * (0.4 + 0.6 * (p.life / 0.5).min(1.0) as f64);
        push_box(out, anchor, p.pos, (r, up, f), [s, s, s], p.col);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::find_spawn;

    #[test]
    fn rnd_oraliqda() {
        let mut s = 12345u32;
        for _ in 0..1000 {
            let v = rnd(&mut s);
            assert!((0.0..1.0).contains(&v));
        }
    }

    #[test]
    fn jonzot_yerda_yuradi_va_ushlanadi() {
        let w = World::new(1);
        let sp = find_spawn(&w);
        let g = ground(&w, sp, true).expect("o'yinchi ostida tayanch bor");
        let mut m = Mob::new(COW, g, d_cross(d_norm(g), [0.0, 0.0, 1.0]));
        let mut seed = 7u32;
        let start = m.pos;
        let mut moved = 0.0f64;
        for _ in 0..600 {
            m.walking = true;
            m.t = 5.0;
            step(&mut m, &w, 0.05, None, &mut seed);
            moved = moved.max(d_len(d_sub(m.pos, start)));
            // yer ustida qoladi
            let gr = ground(&w, m.pos, true);
            assert!(gr.is_some(), "jonzot havoda qoldi");
            assert!((d_len(gr.unwrap()) - d_len(m.pos)).abs() < 0.01);
        }
        assert!(moved > 8.0, "jonzot yurmadi: {moved}");
        // nur urilishi
        let o = d_add(m.pos, d_add(d_mul(m.up(), 3.0), d_mul(m.fwd, -20.0)));
        assert!(ray_hit(&m, o, m.fwd, 40.0).is_some());
        assert!(ray_hit(&m, o, d_mul(m.fwd, -1.0), 40.0).is_none());
    }

    #[test]
    fn dushman_oyinchiga_yetsa_uradi() {
        let w = World::new(1);
        let sp = find_spawn(&w);
        let g = ground(&w, sp, true).unwrap();
        let fwd = d_norm(d_cross(d_norm(g), [0.0, 0.0, 1.0]));
        let mz = d_add(g, d_mul(fwd, 30.0));
        let mut m = Mob::new(ZOMBIE, ground(&w, d_mul(d_norm(mz), d_len(g)), false).unwrap_or(g), fwd);
        let mut seed = 3u32;
        let mut hit = false;
        for _ in 0..400 {
            hit |= step(&mut m, &w, 0.05, Some(g), &mut seed);
        }
        assert!(hit, "zombi o'yinchiga yetib kelmadi");
    }

    #[test]
    fn quti_va_zarrachalar_cho_qqilari() {
        let w = World::new(1);
        let g = ground(&w, find_spawn(&w), true).unwrap();
        for kind in [COW, SHEEP, CHICKEN, ZOMBIE] {
            let m = Mob::new(kind, g, d_cross(d_norm(g), [0.0, 0.0, 1.0]));
            let mut v = Vec::new();
            mob_vertices(&m, g, &mut v);
            assert!(!v.is_empty() && v.len() % 4 == 0 && v.len() / 4 < 16000);
            for q in &v {
                assert!(q.p.iter().all(|x| x.is_finite() && x.abs() < 30.0), "{kind}: {:?}", q.p);
            }
        }
        let mut ps = Vec::new();
        let mut seed = 1u32;
        burst(&mut ps, g, [0.5, 0.5, 0.5], 10, &mut seed);
        assert_eq!(ps.len(), 10);
        for _ in 0..40 {
            step_particles(&mut ps, 0.05);
        }
        assert!(ps.is_empty());
    }
}
