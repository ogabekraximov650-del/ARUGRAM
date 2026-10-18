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
pub const MAX_HP: f32 = 100.0;
const BOT_COUNT: usize = 6;
const PICKUP_RANGE: f32 = 1.6;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WeaponKind {
    Pistol,
    Rifle,
    Shotgun,
    Sniper,
}

pub struct Stats {
    pub body: f32,
    pub head: f32,
    pub cd: f32,
    pub mag: u32,
    pub reload: f32,
    pub pellets: u32,
    pub spread: f32,
    pub recoil: f32,
}

impl WeaponKind {
    pub fn stats(self) -> Stats {
        match self {
            WeaponKind::Pistol => Stats { body: 24.0, head: 48.0, cd: 0.30, mag: 12, reload: 1.2, pellets: 1, spread: 0.004, recoil: 0.010 },
            WeaponKind::Rifle => Stats { body: 30.0, head: 60.0, cd: 0.10, mag: 30, reload: 1.8, pellets: 1, spread: 0.012, recoil: 0.008 },
            WeaponKind::Shotgun => Stats { body: 12.0, head: 20.0, cd: 0.90, mag: 6, reload: 2.2, pellets: 8, spread: 0.06, recoil: 0.030 },
            WeaponKind::Sniper => Stats { body: 90.0, head: 150.0, cd: 1.40, mag: 5, reload: 2.5, pellets: 1, spread: 0.0, recoil: 0.040 },
        }
    }
    pub fn color(self) -> [f32; 3] {
        match self {
            WeaponKind::Pistol => [0.55, 0.65, 0.85],
            WeaponKind::Rifle => [0.95, 0.55, 0.15],
            WeaponKind::Shotgun => [0.85, 0.25, 0.25],
            WeaponKind::Sniper => [0.2, 0.7, 0.3],
        }
    }
}

#[derive(Clone, Copy)]
pub struct Slot {
    pub kind: WeaponKind,
    pub mag: u32,
    pub reserve: u32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PickupKind {
    Weapon(WeaponKind),
    Ammo,
    Medkit,
}

pub struct Pickup {
    pub pos: Vec3,
    pub kind: PickupKind,
    pub alive: bool,
}

/// (kutish s, qisqarish s, yangi radius, soniyasiga zarar)
const PHASES: [(f32, f32, f32, f32); 4] = [(30.0, 30.0, 45.0, 1.0), (25.0, 25.0, 28.0, 2.0), (20.0, 25.0, 14.0, 4.0), (15.0, 20.0, 5.0, 7.0)];
const FINAL_DPS: f32 = 10.0;

pub struct Zone {
    pub center: Vec2,
    pub radius: f32,
    pub next_center: Vec2,
    pub next_radius: f32,
    pub phase: usize,
    pub t: f32,
    pub shrinking: bool,
    pub finished: bool,
    from_center: Vec2,
    from_radius: f32,
}

impl Zone {
    fn new(rng: &mut Lcg) -> Self {
        let mut z = Self {
            center: Vec2::ZERO,
            radius: 70.0,
            next_center: Vec2::ZERO,
            next_radius: PHASES[0].2,
            phase: 0,
            t: 0.0,
            shrinking: false,
            finished: false,
            from_center: Vec2::ZERO,
            from_radius: 70.0,
        };
        z.pick_next(rng);
        z
    }

    fn pick_next(&mut self, rng: &mut Lcg) {
        self.next_radius = PHASES[self.phase].2;
        let room = (self.radius - self.next_radius).max(0.0);
        let a = rng.next() * std::f32::consts::TAU;
        self.next_center = self.center + Vec2::new(a.cos(), a.sin()) * room * rng.next().sqrt();
    }

    pub fn dps(&self) -> f32 {
        if self.finished {
            FINAL_DPS
        } else {
            PHASES[self.phase].3
        }
    }

    pub fn contains(&self, x: f32, z: f32) -> bool {
        Vec2::new(x, z).distance(self.center) <= self.radius
    }

    /// Joriy bosqichning to'liq davomiyligi va o'tgan ulushi (0..1) — HUD uchun.
    pub fn progress(&self) -> f32 {
        if self.finished {
            return 1.0;
        }
        let (wait, shrink, _, _) = PHASES[self.phase];
        if self.shrinking {
            (self.t / shrink).clamp(0.0, 1.0)
        } else {
            (self.t / wait).clamp(0.0, 1.0)
        }
    }

    fn update(&mut self, dt: f32, rng: &mut Lcg) {
        if self.finished {
            return;
        }
        let (wait, shrink, _, _) = PHASES[self.phase];
        self.t += dt;
        if !self.shrinking {
            if self.t >= wait {
                self.shrinking = true;
                self.t = 0.0;
                self.from_center = self.center;
                self.from_radius = self.radius;
            }
        } else {
            let k = (self.t / shrink).clamp(0.0, 1.0);
            self.center = self.from_center.lerp(self.next_center, k);
            self.radius = self.from_radius + (self.next_radius - self.from_radius) * k;
            if k >= 1.0 {
                self.phase += 1;
                self.shrinking = false;
                self.t = 0.0;
                if self.phase >= PHASES.len() {
                    self.finished = true;
                } else {
                    self.pick_next(rng);
                }
            }
        }
    }
}

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
    pub slots: Vec<Slot>,
    pub cur: usize,
    pub reload: f32,
    fire_cd: f32,
    pub pickups: Vec<Pickup>,
    pub zone: Zone,
    pub time: f32,
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
            slots: vec![Slot { kind: WeaponKind::Pistol, mag: 12, reserve: 24 }],
            cur: 0,
            reload: 0.0,
            fire_cd: 0.0,
            pickups: Vec::new(),
            zone: Zone::new(&mut Lcg(99)),
            time: 0.0,
            kills: 0,
            bots: Vec::new(),
            rng: Lcg(12345),
            hit_marker: 0.0,
            hurt: 0.0,
            dead_t: 0.0,
        };
        g.zone = Zone::new(&mut g.rng);
        for _ in 0..BOT_COUNT {
            let b = g.new_bot();
            g.bots.push(b);
        }
        g.spawn_pickups();
        g
    }

    pub fn stats(&self) -> Stats {
        self.slots[self.cur].kind.stats()
    }
    pub fn mag(&self) -> u32 {
        self.slots[self.cur].mag
    }
    pub fn reserve(&self) -> u32 {
        self.slots[self.cur].reserve
    }

    fn spawn_pickups(&mut self) {
        // o'yinchi yonida kafolatli to'plam
        let fixed = [
            (Vec3::new(3.0, 0.0, 8.0), PickupKind::Weapon(WeaponKind::Rifle)),
            (Vec3::new(-3.0, 0.0, 8.0), PickupKind::Ammo),
            (Vec3::new(0.0, 0.0, 4.5), PickupKind::Medkit),
        ];
        for (pos, kind) in fixed {
            self.pickups.push(Pickup { pos, kind, alive: true });
        }
        for i in 0..24 {
            let mut pos = Vec3::new((self.rng.next() - 0.5) * 84.0, 0.0, (self.rng.next() - 0.5) * 84.0);
            push_out(&mut pos, HEIGHT, &self.colliders);
            let kind = match i % 6 {
                0 => PickupKind::Weapon(WeaponKind::Rifle),
                1 => PickupKind::Weapon(WeaponKind::Shotgun),
                2 => PickupKind::Weapon(WeaponKind::Sniper),
                3 => PickupKind::Ammo,
                4 => PickupKind::Medkit,
                _ => PickupKind::Ammo,
            };
            self.pickups.push(Pickup { pos, kind, alive: true });
        }
    }

    fn collect_pickups(&mut self) {
        let feet = Vec3::new(self.camera.pos.x, self.camera.pos.y - EYE, self.camera.pos.z);
        for i in 0..self.pickups.len() {
            let p = &self.pickups[i];
            if !p.alive || Vec2::new(p.pos.x - feet.x, p.pos.z - feet.z).length() > PICKUP_RANGE || (p.pos.y - feet.y).abs() > 2.5 {
                continue;
            }
            let kind = p.kind;
            match kind {
                PickupKind::Medkit => {
                    if self.hp >= MAX_HP {
                        continue;
                    }
                    self.hp = (self.hp + 40.0).min(MAX_HP);
                }
                PickupKind::Ammo => {
                    let mag = self.stats().mag;
                    self.slots[self.cur].reserve += mag * 2;
                }
                PickupKind::Weapon(w) => {
                    if let Some(idx) = self.slots.iter().position(|s| s.kind == w) {
                        self.slots[idx].reserve += w.stats().mag * 2;
                    } else {
                        let s = Slot { kind: w, mag: w.stats().mag, reserve: w.stats().mag * 2 };
                        if self.slots.len() < 3 {
                            self.slots.push(s);
                            self.cur = self.slots.len() - 1;
                        } else {
                            self.slots[self.cur] = s;
                        }
                        self.reload = 0.0;
                        self.fire_cd = 0.3;
                    }
                }
            }
            self.pickups[i].alive = false;
        }
    }

    fn new_bot(&mut self) -> Bot {
        let player = Vec3::new(self.camera.pos.x, 0.0, self.camera.pos.z);
        let mut pos = Vec3::ZERO;
        for _ in 0..20 {
            pos = Vec3::new((self.rng.next() - 0.5) * 80.0, 0.0, (self.rng.next() - 0.5) * 80.0);
            if pos.distance(player) > 20.0 && self.zone.contains(pos.x, pos.z) {
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
        let st = self.stats();
        let o = self.camera.pos;
        let base = self.camera.forward();
        let right = self.camera.right();
        let up = right.cross(base);
        for _ in 0..st.pellets {
            let sx = (self.rng.next() - 0.5) * 2.0 * st.spread;
            let sy = (self.rng.next() - 0.5) * 2.0 * st.spread;
            let d = (base + right * sx + up * sy).normalize();
            let tw = self.world_hit(o, d);
            let mut best: Option<(f32, usize, f32)> = None;
            for (i, b) in self.bots.iter().enumerate() {
                if !b.alive {
                    continue;
                }
                if let Some(t) = b.head().ray(o, d) {
                    if t < tw && best.map_or(true, |x| t < x.0) {
                        best = Some((t, i, st.head));
                    }
                }
                if let Some(t) = b.body().ray(o, d) {
                    if t < tw && best.map_or(true, |x| t < x.0) {
                        best = Some((t, i, st.body));
                    }
                }
            }
            if let Some((_, i, dmg)) = best {
                let b = &mut self.bots[i];
                b.hp -= dmg;
                b.hit_t = 0.12;
                self.hit_marker = 0.15;
                if b.hp <= 0.0 && b.alive {
                    b.alive = false;
                    b.respawn = 5.0;
                    self.kills += 1;
                }
            }
        }
        self.camera.look(0.0, st.recoil);
    }

    pub fn respawn_player(&mut self) {
        let mut p = Vec2::new(0.0, 8.0);
        if !self.zone.contains(p.x, p.y) {
            p = self.zone.center;
        }
        let mut feet = Vec3::new(p.x, 0.0, p.y);
        push_out(&mut feet, HEIGHT, &self.colliders);
        self.camera.pos = feet + Vec3::Y * EYE;
        self.vy = 0.0;
        self.hp = MAX_HP;
        self.slots = vec![Slot { kind: WeaponKind::Pistol, mag: 12, reserve: 24 }];
        self.cur = 0;
        self.reload = 0.0;
    }

    pub fn update(&mut self, dt: f32) {
        if self.input.settings_open() {
            return;
        }
        let dt = dt.clamp(0.0, 0.05);
        self.time += dt;
        self.zone.update(dt, &mut self.rng);
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

        self.collect_pickups();

        // zonadan tashqarida zarar
        if !self.zone.contains(feet.x, feet.z) {
            self.hp -= self.zone.dps() * dt;
            self.hurt = self.hurt.max(0.15);
            if self.hp <= 0.0 {
                self.hp = 0.0;
                self.dead_t = 2.0;
            }
        }

        if self.input.take_switch() && self.slots.len() > 1 {
            self.cur = (self.cur + 1) % self.slots.len();
            self.reload = 0.0;
            self.fire_cd = 0.25;
        }
        let st = self.stats();
        if self.reload > 0.0 {
            self.reload -= dt;
            if self.reload <= 0.0 {
                let s = &mut self.slots[self.cur];
                let take = (st.mag - s.mag).min(s.reserve);
                s.mag += take;
                s.reserve -= take;
            }
        } else if self.slots[self.cur].mag == 0 && self.slots[self.cur].reserve > 0 {
            self.reload = st.reload;
        }
        self.fire_cd -= dt;
        if self.input.fire_held() && self.reload <= 0.0 && self.slots[self.cur].mag > 0 && self.fire_cd <= 0.0 {
            self.shoot();
            self.slots[self.cur].mag -= 1;
            self.fire_cd = st.cd;
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
            let outside = !self.zone.contains(b.pos.x, b.pos.z);
            if outside {
                b.hp -= self.zone.dps() * dt;
                if b.hp <= 0.0 {
                    b.alive = false;
                    b.respawn = 5.0;
                    self.bots[i] = b;
                    continue;
                }
                let back = self.zone.center - Vec2::new(b.pos.x, b.pos.z);
                mv = back / back.length().max(0.01) * 3.5;
            } else if sees {
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
        for p in self.pickups.iter().filter(|p| p.alive) {
            let bob = 0.65 + 0.15 * (self.time * 3.0 + p.pos.x).sin();
            let (rgb, scale) = match p.kind {
                PickupKind::Weapon(w) => (w.color(), [0.8, 0.2, 0.25, 0.0]),
                PickupKind::Ammo => ([1.0, 0.85, 0.2], [0.4, 0.4, 0.4, 0.0]),
                PickupKind::Medkit => ([0.95, 0.95, 0.95], [0.5, 0.4, 0.5, 0.0]),
            };
            v.push(Instance { pos: [p.pos.x, bob, p.pos.z, 0.0], scale, color: [rgb[0], rgb[1], rgb[2], 0.0] });
            let beam = if p.kind == PickupKind::Medkit { [0.3, 0.9, 0.4] } else { rgb };
            v.push(Instance { pos: [p.pos.x, 3.0, p.pos.z, 0.0], scale: [0.12, 6.0, 0.12, 0.0], color: [beam[0], beam[1], beam[2], 0.0] });
        }
        let ring = |c: Vec2, r: f32, n: usize, w: f32, col: [f32; 3], v: &mut Vec<Instance>| {
            for k in 0..n {
                let a = k as f32 / n as f32 * std::f32::consts::TAU;
                v.push(Instance {
                    pos: [c.x + a.cos() * r, 15.0, c.y + a.sin() * r, 0.0],
                    scale: [w, 30.0, w, 0.0],
                    color: [col[0], col[1], col[2], 0.0],
                });
            }
        };
        ring(self.zone.center, self.zone.radius, 96, 1.0, [0.2, 0.45, 1.0], &mut v);
        if !self.zone.finished {
            ring(self.zone.next_center, self.zone.next_radius, 48, 0.5, [0.95, 0.95, 0.95], &mut v);
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
    fn zona_qisqaradi_va_ichida_qoladi() {
        let mut g = Game::new(1280, 576, 2.0);
        let r0 = g.zone.radius;
        for _ in 0..(60 * 70) {
            g.zone.update(1.0 / 60.0, &mut g.rng);
            assert!(g.zone.center.length() + g.zone.radius <= r0 + 1e-3);
        }
        assert!(g.zone.radius < 46.0, "qisqardi");
    }

    #[test]
    fn zona_tashqarisida_jon_kamayadi() {
        let mut g = Game::new(1280, 576, 2.0);
        g.zone.center = Vec2::new(30.0, 30.0);
        g.zone.radius = 3.0;
        for b in g.bots.iter_mut() {
            b.alive = false;
            b.respawn = 100.0;
        }
        step(&mut g, 60);
        assert!(g.hp < MAX_HP);
    }

    #[test]
    fn qurol_yigish_va_almashtirish() {
        let mut g = Game::new(1280, 576, 2.0);
        g.pickups.clear();
        let feet = Vec3::new(g.camera.pos.x, 0.0, g.camera.pos.z);
        g.pickups.push(Pickup { pos: feet, kind: PickupKind::Weapon(WeaponKind::Shotgun), alive: true });
        step(&mut g, 2);
        assert_eq!(g.slots.len(), 2);
        assert_eq!(g.slots[g.cur].kind, WeaponKind::Shotgun);
        let (c, _) = g.input.switch_button();
        g.touch(TouchAction::Down, 9, c.x, c.y);
        step(&mut g, 2);
        assert_eq!(g.slots[g.cur].kind, WeaponKind::Pistol);
    }

    #[test]
    fn oq_tugasa_qayta_oqlanadi() {
        let mut g = Game::new(1280, 576, 2.0);
        g.pickups.clear();
        g.slots[0].mag = 1;
        let (c, _) = g.input.fire_button();
        g.touch(TouchAction::Down, 8, c.x, c.y);
        step(&mut g, 3);
        assert_eq!(g.mag(), 0);
        g.touch(TouchAction::Up, 8, 0.0, 0.0);
        step(&mut g, 100);
        assert_eq!(g.mag(), 12);
        assert_eq!(g.reserve(), 12);
    }

    #[test]
    fn nur_quti_kesishishi() {
        let b = Aabb { min: Vec3::new(-1.0, 0.0, -6.0), max: Vec3::new(1.0, 2.0, -4.0) };
        let t = b.ray(Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, -1.0)).unwrap();
        assert!((t - 4.0).abs() < 1e-4);
        assert!(b.ray(Vec3::new(5.0, 1.0, 0.0), Vec3::new(0.0, 0.0, -1.0)).is_none());
    }
}
