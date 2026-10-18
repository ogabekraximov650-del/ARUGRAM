//! O'yin mantig'i (GPU'dan mustaqil): fizika, to'qnashuv, otish, botlar.

use crate::camera::Camera;
use crate::input::{Input, TouchAction, WORLD_HALF};
use crate::world::{self, Instance};
use glam::{Vec2, Vec3};

pub const EYE: f32 = 1.7;
const RADIUS: f32 = 0.4;
const HEIGHT: f32 = 1.8;
const GRAVITY: f32 = 22.0;
const JUMP_SPEED: f32 = 7.5;
pub const MAG_SIZE: u32 = 30;
pub const MAX_HP: f32 = 100.0;
const BOT_COUNT: usize = 6;

#[derive(Clone, Copy, Debug)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    fn from_instance(i: &Instance) -> Self {
        let c = Vec3::new(i.pos[0], i.pos[1], i.pos[2]);
        let h = Vec3::new(i.scale[0], i.scale[1], i.scale[2]) * 0.5;
        Self { min: c - h, max: c + h }
    }

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

struct Lcg(u32);
impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.0 >> 8) as f32 / (1u32 << 24) as f32
    }
}

pub struct Bot {
    /// Oyoq nuqtasi.
    pub pos: Vec3,
    pub hp: f32,
    pub alive: bool,
    wander: Vec2,
    timer: f32,
    cooldown: f32,
    pub hit_t: f32,
    respawn: f32,
    strafe: f32,
}

impl Bot {
    fn body(&self) -> Aabb {
        Aabb {
            min: self.pos + Vec3::new(-0.35, 0.0, -0.25),
            max: self.pos + Vec3::new(0.35, 1.35, 0.25),
        }
    }
    fn head(&self) -> Aabb {
        Aabb {
            min: self.pos + Vec3::new(-0.22, 1.35, -0.22),
            max: self.pos + Vec3::new(0.22, 1.79, 0.22),
        }
    }
    fn eye(&self) -> Vec3 {
        self.pos + Vec3::new(0.0, 1.5, 0.0)
    }
}

pub struct Game {
    pub camera: Camera,
    pub input: Input,
    pub width: u32,
    pub height: u32,
    pub density: f32,
    colliders: Vec<Aabb>,
    vy: f32,
    pub grounded: bool,
    pub hp: f32,
    pub ammo: u32,
    pub reload: f32,
    fire_cd: f32,
    pub kills: u32,
    pub bots: Vec<Bot>,
    rng: Lcg,
    pub hit_marker: f32,
    pub hurt: f32,
    pub dead_t: f32,
}

/// Doira (r) ni quti ichidan xy tekisligida itarib chiqaradi.
fn push_out(p: &mut Vec3, height: f32, colliders: &[Aabb]) {
    for _ in 0..2 {
        for b in colliders {
            if p.y >= b.max.y - 0.05 || p.y + height <= b.min.y {
                continue;
            }
            let cx = p.x.clamp(b.min.x, b.max.x);
            let cz = p.z.clamp(b.min.z, b.max.z);
            let (dx, dz) = (p.x - cx, p.z - cz);
            let d2 = dx * dx + dz * dz;
            if d2 >= RADIUS * RADIUS {
                continue;
            }
            if d2 > 1e-8 {
                let d = d2.sqrt();
                let k = (RADIUS - d) / d;
                p.x += dx * k;
                p.z += dz * k;
            } else {
                // markaz quti ichida: eng yaqin yon tomonga chiqaramiz
                let opts = [
                    (p.x - b.min.x, -1.0, 0.0),
                    (b.max.x - p.x, 1.0, 0.0),
                    (p.z - b.min.z, 0.0, -1.0),
                    (b.max.z - p.z, 0.0, 1.0),
                ];
                let (pen, sx, sz) = opts.iter().copied().fold(opts[0], |a, o| if o.0 < a.0 { o } else { a });
                p.x += sx * (pen + RADIUS);
                p.z += sz * (pen + RADIUS);
            }
        }
    }
    p.x = p.x.clamp(-WORLD_HALF, WORLD_HALF);
    p.z = p.z.clamp(-WORLD_HALF, WORLD_HALF);
}

fn support(p: Vec3, colliders: &[Aabb]) -> f32 {
    let mut g = 0.0f32;
    for b in colliders {
        let m = RADIUS * 0.6;
        if p.x > b.min.x - m && p.x < b.max.x + m && p.z > b.min.z - m && p.z < b.max.z + m && b.max.y <= p.y + 0.35 {
            g = g.max(b.max.y);
        }
    }
    g
}

impl Game {
    pub fn new(width: u32, height: u32, density: f32) -> Self {
        let colliders: Vec<Aabb> = world::build()
            .iter()
            .filter(|i| i.color[3] < 0.5)
            .map(Aabb::from_instance)
            .collect();
        let mut g = Self {
            camera: Camera::new(Vec3::new(0.0, EYE, 8.0)),
            input: Input::new(width, height, density),
            width,
            height,
            density,
            colliders,
            vy: 0.0,
            grounded: true,
            hp: MAX_HP,
            ammo: MAG_SIZE,
            reload: 0.0,
            fire_cd: 0.0,
            kills: 0,
            bots: Vec::new(),
            rng: Lcg(12345),
            hit_marker: 0.0,
            hurt: 0.0,
            dead_t: 0.0,
        };
        for _ in 0..BOT_COUNT {
            let b = g.new_bot();
            g.bots.push(b);
        }
        g
    }

    fn new_bot(&mut self) -> Bot {
        let player = Vec3::new(self.camera.pos.x, 0.0, self.camera.pos.z);
        let mut pos = Vec3::ZERO;
        for _ in 0..20 {
            pos = Vec3::new((self.rng.next() - 0.5) * 80.0, 0.0, (self.rng.next() - 0.5) * 80.0);
            if pos.distance(player) > 20.0 {
                break;
            }
        }
        push_out(&mut pos, HEIGHT, &self.colliders);
        Bot {
            pos,
            hp: MAX_HP,
            alive: true,
            wander: Vec2::ZERO,
            timer: 0.0,
            cooldown: 1.0 + self.rng.next(),
            hit_t: 0.0,
            respawn: 0.0,
            strafe: 1.0,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.input.resize(width, height);
    }

    pub fn touch(&mut self, action: TouchAction, id: i32, x: f32, y: f32) {
        self.input.touch(action, id, x, y);
    }

    fn world_hit(&self, o: Vec3, d: Vec3) -> f32 {
        let mut t = f32::MAX;
        if d.y < -1e-6 {
            t = -o.y / d.y;
        }
        for b in &self.colliders {
            if let Some(h) = b.ray(o, d) {
                t = t.min(h);
            }
        }
        t
    }

    fn line_of_sight(&self, a: Vec3, b: Vec3) -> bool {
        let d = b - a;
        let len = d.length();
        if len < 1e-4 {
            return true;
        }
        self.world_hit(a, d / len) >= len
    }

    fn shoot(&mut self) {
        let o = self.camera.pos;
        let d = self.camera.forward();
        let tw = self.world_hit(o, d);
        let mut best: Option<(f32, usize, f32)> = None;
        for (i, b) in self.bots.iter().enumerate() {
            if !b.alive {
                continue;
            }
            if let Some(t) = b.head().ray(o, d) {
                if t < tw && best.map_or(true, |x| t < x.0) {
                    best = Some((t, i, 70.0));
                }
            }
            if let Some(t) = b.body().ray(o, d) {
                if t < tw && best.map_or(true, |x| t < x.0) {
                    best = Some((t, i, 34.0));
                }
            }
        }
        if let Some((_, i, dmg)) = best {
            let b = &mut self.bots[i];
            b.hp -= dmg;
            b.hit_t = 0.12;
            self.hit_marker = 0.15;
            if b.hp <= 0.0 {
                b.alive = false;
                b.respawn = 5.0;
                self.kills += 1;
            }
        }
        self.camera.look(0.0, 0.012);
    }

    pub fn respawn_player(&mut self) {
        self.camera.pos = Vec3::new(0.0, EYE, 8.0);
        self.vy = 0.0;
        self.hp = MAX_HP;
        self.ammo = MAG_SIZE;
        self.reload = 0.0;
    }

    pub fn update(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.05);
        self.hit_marker = (self.hit_marker - dt).max(0.0);
        self.hurt = (self.hurt - dt).max(0.0);

        if self.dead_t > 0.0 {
            let keep = self.camera.pos;
            self.input.update(dt, &mut self.camera);
            self.input.take_jump();
            self.camera.pos = keep;
            self.dead_t -= dt;
            if self.dead_t <= 0.0 {
                self.respawn_player();
            }
        } else {
            self.update_player(dt);
        }
        self.update_bots(dt);
    }

    fn update_player(&mut self, dt: f32) {
        self.input.update(dt, &mut self.camera);
        let mut feet = Vec3::new(self.camera.pos.x, self.camera.pos.y - EYE, self.camera.pos.z);
        push_out(&mut feet, HEIGHT, &self.colliders);

        if self.input.take_jump() && self.grounded {
            self.vy = JUMP_SPEED;
            self.grounded = false;
        }
        let ground = support(feet, &self.colliders);
        self.vy -= GRAVITY * dt;
        feet.y += self.vy * dt;
        if feet.y <= ground && self.vy <= 0.0 {
            feet.y = ground;
            self.vy = 0.0;
            self.grounded = true;
        } else {
            self.grounded = false;
        }
        self.camera.pos = Vec3::new(feet.x, feet.y + EYE, feet.z);

        if self.reload > 0.0 {
            self.reload -= dt;
            if self.reload <= 0.0 {
                self.ammo = MAG_SIZE;
            }
        }
        self.fire_cd -= dt;
        if self.input.fire_held() && self.ammo > 0 && self.reload <= 0.0 && self.fire_cd <= 0.0 {
            self.shoot();
            self.ammo -= 1;
            self.fire_cd = 0.14;
            if self.ammo == 0 {
                self.reload = 1.6;
            }
        }
    }

    fn update_bots(&mut self, dt: f32) {
        let player_eye = self.camera.pos;
        let player_alive = self.dead_t <= 0.0;
        for i in 0..self.bots.len() {
            if !self.bots[i].alive {
                self.bots[i].respawn -= dt;
                if self.bots[i].respawn <= 0.0 {
                    let nb = self.new_bot();
                    self.bots[i] = nb;
                }
                continue;
            }
            let mut b = std::mem::replace(
                &mut self.bots[i],
                Bot { pos: Vec3::ZERO, hp: 0.0, alive: false, wander: Vec2::ZERO, timer: 0.0, cooldown: 0.0, hit_t: 0.0, respawn: 0.0, strafe: 1.0 },
            );
            b.hit_t = (b.hit_t - dt).max(0.0);
            let to = Vec2::new(player_eye.x - b.pos.x, player_eye.z - b.pos.z);
            let dist = to.length();
            let sees = player_alive && dist < 40.0 && self.line_of_sight(b.eye(), player_eye - Vec3::new(0.0, 0.3, 0.0));
            let mut mv = Vec2::ZERO;
            if sees {
                let dir = to / dist.max(0.01);
                b.timer -= dt;
                if b.timer <= 0.0 {
                    b.strafe = if self.rng.next() > 0.5 { 1.0 } else { -1.0 };
                    b.timer = 1.0 + self.rng.next() * 1.5;
                }
                if dist > 9.0 {
                    mv += dir * 3.0;
                }
                mv += Vec2::new(-dir.y, dir.x) * b.strafe * 1.6;
                b.cooldown -= dt;
                if b.cooldown <= 0.0 && dist < 35.0 {
                    b.cooldown = 0.9 + self.rng.next() * 0.8;
                    if self.rng.next() < 0.4 {
                        self.hp -= 8.0;
                        self.hurt = 0.35;
                        if self.hp <= 0.0 {
                            self.hp = 0.0;
                            self.dead_t = 2.0;
                        }
                    }
                }
            } else {
                b.timer -= dt;
                if b.timer <= 0.0 {
                    let a = self.rng.next() * std::f32::consts::TAU;
                    b.wander = Vec2::new(a.cos(), a.sin()) * 2.0;
                    b.timer = 2.0 + self.rng.next() * 2.0;
                }
                mv = b.wander;
            }
            b.pos.x += mv.x * dt;
            b.pos.z += mv.y * dt;
            push_out(&mut b.pos, HEIGHT, &self.colliders);
            self.bots[i] = b;
        }
    }

    /// Har kadr o'zgaradigan jismlar (botlar) — GPU'ga yoziladi.
    pub fn dynamic_instances(&self) -> Vec<Instance> {
        let mut v = Vec::new();
        for b in self.bots.iter().filter(|b| b.alive) {
            let flash = if b.hit_t > 0.0 { 1.0 } else { 0.0 };
            let body = [0.80 + 0.2 * flash, 0.25 + 0.75 * flash, 0.20 + 0.8 * flash, 0.0];
            let head = [0.95, 0.80 + 0.2 * flash, 0.65 + 0.35 * flash, 0.0];
            v.push(Instance { pos: [b.pos.x, b.pos.y + 0.675, b.pos.z, 0.0], scale: [0.7, 1.35, 0.5, 0.0], color: body });
            v.push(Instance { pos: [b.pos.x, b.pos.y + 1.57, b.pos.z, 0.0], scale: [0.44, 0.44, 0.44, 0.0], color: head });
        }
        v
    }

    pub fn hud(&self) -> Vec<crate::hud::HudVertex> {
        crate::hud::build(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(g: &mut Game, n: usize) {
        for _ in 0..n {
            g.update(1.0 / 60.0);
        }
    }

    #[test]
    fn zaminda_turadi() {
        let mut g = Game::new(1280, 576, 2.0);
        step(&mut g, 60);
        assert!((g.camera.pos.y - EYE).abs() < 1e-3);
        assert!(g.grounded);
    }

    #[test]
    fn sakraydi_va_qaytadi() {
        let mut g = Game::new(1280, 576, 2.0);
        step(&mut g, 5);
        let (c, _) = g.input.jump_button();
        g.touch(TouchAction::Down, 5, c.x, c.y);
        step(&mut g, 10);
        assert!(g.camera.pos.y > EYE + 0.3, "sakradi");
        step(&mut g, 120);
        assert!((g.camera.pos.y - EYE).abs() < 1e-3, "qo'ndi");
    }

    #[test]
    fn quti_ichiga_kirib_bolmaydi() {
        let mut g = Game::new(1280, 576, 2.0);
        let b = g.colliders[0];
        let c = (b.min + b.max) * 0.5;
        let mut feet = Vec3::new(c.x, 0.0, c.z);
        push_out(&mut feet, HEIGHT, &g.colliders);
        g.camera.pos = feet + Vec3::Y * EYE;
        for bb in &g.colliders {
            let inside = feet.x > bb.min.x && feet.x < bb.max.x && feet.z > bb.min.z && feet.z < bb.max.z && feet.y < bb.max.y - 0.05;
            assert!(!inside);
        }
    }

    #[test]
    fn otish_botni_uradi() {
        let mut g = Game::new(1280, 576, 2.0);
        g.colliders.clear();
        g.bots.truncate(1);
        g.camera.pos = Vec3::new(0.0, EYE, 0.0);
        g.camera.yaw = 0.0;
        g.camera.pitch = 0.0;
        g.bots[0].pos = Vec3::new(0.0, 0.0, -10.0);
        g.shoot();
        assert!(g.bots[0].hp < MAX_HP, "tanaga tegdi");
    }

    #[test]
    fn devor_orqasidagi_botga_tegmaydi() {
        let mut g = Game::new(1280, 576, 2.0);
        g.colliders = vec![Aabb { min: Vec3::new(-3.0, 0.0, -6.0), max: Vec3::new(3.0, 5.0, -5.0) }];
        g.bots.truncate(1);
        g.camera.pos = Vec3::new(0.0, EYE, 0.0);
        g.camera.yaw = 0.0;
        g.camera.pitch = 0.0;
        g.bots[0].pos = Vec3::new(0.0, 0.0, -10.0);
        g.shoot();
        assert_eq!(g.bots[0].hp, MAX_HP);
    }

    #[test]
    fn nur_quti_kesishishi() {
        let b = Aabb { min: Vec3::new(-1.0, 0.0, -6.0), max: Vec3::new(1.0, 2.0, -4.0) };
        let t = b.ray(Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, -1.0)).unwrap();
        assert!((t - 4.0).abs() < 1e-4);
        assert!(b.ray(Vec3::new(5.0, 1.0, 0.0), Vec3::new(0.0, 0.0, -1.0)).is_none());
    }
}
