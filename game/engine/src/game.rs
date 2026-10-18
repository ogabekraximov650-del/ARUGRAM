//! O'yin mantig'i (GPU'dan mustaqil): fizika, to'qnashuv, otish, zona, narsalar va yonimdagi qiz.

use crate::audio;
use crate::camera::Camera;
use crate::input::{Input, TouchAction};
use crate::models::{self, GirlState, VmState};
use crate::renderer::SUN_DIR;
use crate::scene::{Aabb, Instance, Lists, Mat, Mesh, MESH_COUNT};
use crate::terrain::{self, WORLD_HALF};
use crate::world;
use glam::{Quat, Vec2, Vec3};

pub use crate::weapons::{Stats, WeaponKind};

pub const EYE: f32 = 1.7;
const RADIUS: f32 = 0.4;
const HEIGHT: f32 = 1.8;
const GRAVITY: f32 = 22.0;
const JUMP_SPEED: f32 = 7.5;
/// Shu balandlikkacha bo'lgan to'siq ustiga to'g'ridan-to'g'ri chiqib olinadi (zina, ostona).
const STEP: f32 = 0.32;
pub const MAX_HP: f32 = 100.0;
const PICKUP_RANGE: f32 = 1.6;
const ZONE_START: f32 = 140.0;
const SPAWN: Vec2 = Vec2::new(0.0, 12.0);

/// Qiz: o'yinchi 5 m dan uzoqlashmaguncha turadi; uzoqlashganda o'yinchi yurgan tomonga yuradi, o'yinchi to'xtasa to'xtaydi.
const COMPANION_RANGE: f32 = 5.0;
const COMPANION_ARRIVE: f32 = 3.5;

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
const PHASES: [(f32, f32, f32, f32); 4] = [(40.0, 40.0, 90.0, 1.0), (30.0, 30.0, 55.0, 2.0), (25.0, 30.0, 30.0, 4.0), (20.0, 25.0, 12.0, 7.0)];
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
            radius: ZONE_START,
            next_center: Vec2::ZERO,
            next_radius: PHASES[0].2,
            phase: 0,
            t: 0.0,
            shrinking: false,
            finished: false,
            from_center: Vec2::ZERO,
            from_radius: ZONE_START,
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


struct Lcg(u32);
impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.0 >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// Yonimdagi qiz holati.
pub struct Companion {
    pub pos: Vec3,
    pub yaw: f32,
    pub phase: f32,
    /// Hozirgi tezlik (m/s).
    pub speed: f32,
    pub look_yaw: f32,
    walking: bool,
}

impl Companion {
    fn new(pos: Vec3) -> Self {
        Self { pos, yaw: 0.0, phase: 0.0, speed: 0.0, look_yaw: 0.0, walking: false }
    }
}

/// Har kadr chiziladigan o'zgaruvchan sahna.
pub struct FrameData {
    pub dynamic: Lists,
    pub dynamic_shadow: [usize; MESH_COUNT],
    pub viewmodel: Lists,
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
    pub companion: Companion,
    pub flash: f32,
    pub hurt: f32,
    pub dead_t: f32,
    recoil: f32,
    raise: f32,
    walk_phase: f32,
    impacts: Vec<(Vec3, f32)>,
    events: Vec<u8>,
    step_dist: f32,
    zone_tick: f32,
    rng: Lcg,
    player_vel: Vec2,
    last_dir: Vec2,
}

fn smooth_angle(cur: f32, want: f32, rate: f32, dt: f32) -> f32 {
    let mut diff = (want - cur) % std::f32::consts::TAU;
    if diff > std::f32::consts::PI {
        diff -= std::f32::consts::TAU;
    } else if diff < -std::f32::consts::PI {
        diff += std::f32::consts::TAU;
    }
    cur + diff * (1.0 - (-rate * dt).exp())
}

fn wrap_angle(a: f32) -> f32 {
    let mut d = a % std::f32::consts::TAU;
    if d > std::f32::consts::PI {
        d -= std::f32::consts::TAU;
    } else if d < -std::f32::consts::PI {
        d += std::f32::consts::TAU;
    }
    d
}

/// Doirani (RADIUS) qutilar ichidan gorizontal itarib chiqaradi. `p.y` — oyoq balandligi.
fn push_out(p: &mut Vec3, height: f32, colliders: &[Aabb]) {
    for _ in 0..2 {
        for b in colliders {
            if b.max.y - p.y <= STEP || p.y + height <= b.min.y {
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

/// Oyoq ostidagi tayanch balandligi: relyef yoki chiqib olsa bo'ladigan quti usti.
fn support(p: Vec3, colliders: &[Aabb]) -> f32 {
    let mut g = terrain::height(p.x, p.z);
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
        let w = world::build();
        let mut g = Self {
            camera: Camera::new(Vec3::new(SPAWN.x, EYE, SPAWN.y)),
            input: Input::new(width, height, density),
            width,
            height,
            density,
            colliders: w.colliders,
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
            companion: Companion::new(Vec3::new(SPAWN.x + 2.0, 0.0, SPAWN.y - 1.0)),
            flash: 0.0,
            hurt: 0.0,
            dead_t: 0.0,
            recoil: 0.0,
            raise: 1.0,
            walk_phase: 0.0,
            impacts: Vec::new(),
            events: Vec::new(),
            step_dist: 0.0,
            zone_tick: 0.0,
            rng: Lcg(12345),
            player_vel: Vec2::ZERO,
            last_dir: Vec2::new(0.0, -1.0),
        };
        g.zone = Zone::new(&mut g.rng);
        g.spawn_pickups(&w.loot);
        g.companion.yaw = 0.0;
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

    fn spawn_pickups(&mut self, loot: &[Vec3]) {
        // o'yinchi yonida kafolatli to'plam
        let fixed = [
            (Vec3::new(3.0, 0.0, 14.0), PickupKind::Weapon(WeaponKind::Rifle)),
            (Vec3::new(-3.0, 0.0, 14.0), PickupKind::Ammo),
            (Vec3::new(0.0, 0.0, 9.0), PickupKind::Medkit),
        ];
        for (pos, kind) in fixed {
            self.pickups.push(Pickup { pos, kind, alive: true });
        }
        for (i, p) in loot.iter().enumerate() {
            let kind = match i % 8 {
                0 | 4 => PickupKind::Weapon(WeaponKind::Rifle),
                1 => PickupKind::Weapon(WeaponKind::Shotgun),
                2 => PickupKind::Weapon(WeaponKind::Sniper),
                3 | 5 => PickupKind::Ammo,
                6 => PickupKind::Medkit,
                _ => PickupKind::Weapon(WeaponKind::Pistol),
            };
            self.pickups.push(Pickup { pos: *p, kind, alive: true });
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
                        self.raise = 0.0;
                    }
                }
            }
            self.pickups[i].alive = false;
        }
    }

    /// Shu kadrda to'plangan ovoz hodisalari (`audio::EV_*`); ro'yxat bo'shatiladi.
    pub fn take_events(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.events)
    }

    fn emit(&mut self, ev: u8) {
        if self.events.len() < 32 {
            self.events.push(ev);
        }
    }

    pub fn set_density(&mut self, d: f32) {
        self.density = d.max(0.3);
        self.input.set_density(d);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.input.resize(width, height);
    }

    pub fn touch(&mut self, action: TouchAction, id: i32, x: f32, y: f32) {
        self.input.touch(action, id, x, y);
    }

    /// Nur relyefga tegadigan masofa (yo'q bo'lsa `f32::MAX`).
    fn terrain_hit(&self, o: Vec3, d: Vec3) -> f32 {
        let (mut t, step, max) = (0.0f32, 1.0f32, 200.0f32);
        let above = |t: f32| {
            let p = o + d * t;
            p.y - terrain::height(p.x, p.z)
        };
        if above(0.0) < 0.0 {
            return 0.0;
        }
        while t < max {
            let nt = t + step;
            if above(nt) < 0.0 {
                let (mut lo, mut hi) = (t, nt);
                for _ in 0..8 {
                    let mid = (lo + hi) * 0.5;
                    if above(mid) < 0.0 {
                        hi = mid;
                    } else {
                        lo = mid;
                    }
                }
                return hi;
            }
            t = nt;
        }
        f32::MAX
    }

    fn world_hit(&self, o: Vec3, d: Vec3) -> f32 {
        let mut t = self.terrain_hit(o, d);
        for b in &self.colliders {
            if let Some(h) = b.ray(o, d) {
                t = t.min(h);
            }
        }
        t
    }

    fn shoot(&mut self) {
        let st = self.stats();
        self.flash = 0.07;
        self.recoil = 1.0;
        self.emit(match self.slots[self.cur].kind {
            WeaponKind::Pistol => audio::EV_SHOT_PISTOL,
            WeaponKind::Rifle => audio::EV_SHOT_RIFLE,
            WeaponKind::Shotgun => audio::EV_SHOT_SHOTGUN,
            WeaponKind::Sniper => audio::EV_SHOT_SNIPER,
        });
        let o = self.camera.pos;
        let base = self.camera.forward();
        let right = self.camera.right();
        let up = right.cross(base);
        for _ in 0..st.pellets {
            let sx = (self.rng.next() - 0.5) * 2.0 * st.spread;
            let sy = (self.rng.next() - 0.5) * 2.0 * st.spread;
            let d = (base + right * sx + up * sy).normalize();
            let tw = self.world_hit(o, d);
            if tw < 150.0 && self.impacts.len() < 24 {
                self.impacts.push((o + d * tw, 0.0));
            }
        }
        self.camera.look(0.0, st.recoil);
    }

    pub fn respawn_player(&mut self) {
        let mut p = SPAWN;
        if !self.zone.contains(p.x, p.y) {
            p = self.zone.center;
        }
        let mut feet = Vec3::new(p.x, 0.0, p.y);
        push_out(&mut feet, HEIGHT, &self.colliders);
        feet.y = support(feet, &self.colliders);
        self.camera.pos = feet + Vec3::Y * EYE;
        self.vy = 0.0;
        self.hp = MAX_HP;
        self.slots = vec![Slot { kind: WeaponKind::Pistol, mag: 12, reserve: 24 }];
        self.cur = 0;
        self.reload = 0.0;
        self.raise = 0.0;
    }

    pub fn update(&mut self, dt: f32) {
        if self.input.settings_open() {
            return;
        }
        let dt = dt.clamp(0.0, 0.05);
        self.time += dt;
        self.zone.update(dt, &mut self.rng);
        self.flash = (self.flash - dt).max(0.0);
        self.recoil *= (-14.0 * dt).exp();
        self.raise = (self.raise + dt / 0.35).min(1.0);
        for im in self.impacts.iter_mut() {
            im.1 += dt;
        }
        self.impacts.retain(|im| im.1 < 0.5);
        self.hurt = (self.hurt - dt).max(0.0);

        let before = Vec2::new(self.camera.pos.x, self.camera.pos.z);
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
        // o'yinchi tezligi (silliqlangan) — qiz shunga qarab harakatlanadi
        if dt > 0.0 {
            let now = Vec2::new(self.camera.pos.x, self.camera.pos.z);
            let raw = (now - before) / dt;
            let k = 1.0 - (-dt / 0.10).exp();
            self.player_vel += (raw - self.player_vel) * k;
        }
        self.update_companion(dt);
    }

    fn update_player(&mut self, dt: f32) {
        let before = self.camera.pos;
        self.input.update(dt, &mut self.camera);
        let mut feet = Vec3::new(self.camera.pos.x, before.y - EYE, self.camera.pos.z);
        push_out(&mut feet, HEIGHT, &self.colliders);

        let jump = self.input.take_jump();
        let ground = support(feet, &self.colliders);
        let y0 = feet.y;
        if jump && self.grounded {
            self.vy = JUMP_SPEED;
            self.grounded = false;
        }
        let was_grounded = self.grounded;
        self.vy -= GRAVITY * dt;
        feet.y += self.vy * dt;
        if feet.y <= ground && self.vy <= 0.0 {
            feet.y = ground;
            self.vy = 0.0;
            self.grounded = true;
        } else if was_grounded && self.vy <= 0.0 && y0 - ground <= 0.4 && y0 >= ground {
            // nishab pastga: yerga yopishib yuradi
            feet.y = ground;
            self.vy = 0.0;
            self.grounded = true;
        } else {
            self.grounded = false;
        }
        self.camera.pos = Vec3::new(feet.x, feet.y + EYE, feet.z);

        let moved = Vec2::new(feet.x - before.x, feet.z - before.z).length();
        self.step_dist += moved;
        self.walk_phase += moved * 2.4;
        if self.grounded && self.step_dist > 2.0 {
            self.step_dist = 0.0;
            self.emit(audio::EV_STEP);
        }
        let n_before = self.pickups.iter().filter(|p| p.alive).count();
        self.collect_pickups();
        if self.pickups.iter().filter(|p| p.alive).count() < n_before {
            self.emit(audio::EV_PICKUP);
        }

        // zonadan tashqarida zarar
        if !self.zone.contains(feet.x, feet.z) {
            self.hp -= self.zone.dps() * dt;
            self.hurt = self.hurt.max(0.15);
            self.zone_tick -= dt;
            if self.zone_tick <= 0.0 {
                self.zone_tick = 1.0;
                self.emit(audio::EV_HURT);
            }
            if self.hp <= 0.0 {
                self.hp = 0.0;
                self.dead_t = 2.0;
            }
        }

        if self.input.take_switch() && self.slots.len() > 1 {
            self.cur = (self.cur + 1) % self.slots.len();
            self.reload = 0.0;
            self.fire_cd = 0.25;
            self.raise = 0.0;
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
            self.emit(audio::EV_RELOAD);
        }
        self.fire_cd -= dt;
        if self.input.fire_held() && self.reload <= 0.0 && self.slots[self.cur].mag > 0 && self.fire_cd <= 0.0 && self.raise >= 0.7 {
            self.shoot();
            self.slots[self.cur].mag -= 1;
            self.fire_cd = st.cd;
        }
    }

    fn update_companion(&mut self, dt: f32) {
        let feet = Vec2::new(self.camera.pos.x, self.camera.pos.z);
        let pspeed = self.player_vel.length();
        if pspeed > 0.3 {
            self.last_dir = self.player_vel / pspeed;
        }
        let pdir = self.last_dir;
        let side = Vec2::new(-pdir.y, pdir.x);
        let cpos = Vec2::new(self.companion.pos.x, self.companion.pos.z);
        let to_player = feet - cpos;
        let dist = to_player.length();
        let c = &mut self.companion;

        if dist > 40.0 {
            // juda uzoqda qoldi — o'yinchi yurgan tomonga o'tkazamiz
            let p = feet + pdir * 3.0 + side * 1.6;
            c.pos = Vec3::new(p.x, c.pos.y, p.y);
        }
        let dist = (feet - Vec2::new(c.pos.x, c.pos.z)).length();

        // holat: o'yinchi 5 m dan uzoqlashsa va yurayotgan bo'lsa — yuradi; to'xtasa yoki yetib kelsa — to'xtaydi
        if c.walking {
            if pspeed < 0.3 || dist < COMPANION_ARRIVE {
                c.walking = false;
            }
        } else if dist > COMPANION_RANGE && pspeed >= 0.3 {
            c.walking = true;
        }

        let mut want_speed = 0.0;
        let mut move_dir = Vec2::ZERO;
        if c.walking {
            // o'yinchi yurgan tomonda: oldida-yonida, orqasida emas
            let target = feet + pdir * 2.5 + side * 1.6;
            let to = target - Vec2::new(c.pos.x, c.pos.z);
            let d = to.length();
            if d > 0.05 {
                move_dir = to / d;
                want_speed = (pspeed * 1.1 + (d - 1.0).max(0.0)).clamp(1.5, 8.0);
            }
        }
        // tezlanish/sekinlashish (to'xtaganda tez, lekin sakramasdan)
        let accel = if want_speed > c.speed { 18.0 } else { 40.0 };
        let dv = (want_speed - c.speed).clamp(-accel * dt, accel * dt);
        c.speed = (c.speed + dv).max(0.0);
        if c.speed > 0.01 && move_dir != Vec2::ZERO {
            c.pos.x += move_dir.x * c.speed * dt;
            c.pos.z += move_dir.y * c.speed * dt;
        }
        let mut p = c.pos;
        push_out(&mut p, HEIGHT, &self.colliders);
        c.pos = Vec3::new(p.x, support(Vec3::new(p.x, c.pos.y, p.z), &self.colliders), p.z);
        c.phase += c.speed * dt * 1.7;

        // burilish: yurganda harakat tomoniga; turganda o'yinchiga qaraydi
        let to_pl = feet - Vec2::new(c.pos.x, c.pos.z);
        let face_player = (-to_pl.x).atan2(-to_pl.y);
        if c.speed > 0.8 && move_dir != Vec2::ZERO {
            let want = (-move_dir.x).atan2(-move_dir.y);
            c.yaw = smooth_angle(c.yaw, want, 9.0, dt);
            c.look_yaw += (0.0 - c.look_yaw) * (1.0 - (-6.0 * dt).exp());
        } else {
            let rel = wrap_angle(face_player - c.yaw);
            if rel.abs() > 1.1 {
                c.yaw = smooth_angle(c.yaw, face_player, 3.0, dt);
            }
            let rel = wrap_angle(face_player - c.yaw).clamp(-1.0, 1.0);
            c.look_yaw += (rel - c.look_yaw) * (1.0 - (-5.0 * dt).exp());
        }
    }

    /// Chiziladigan sahna: qiz, chang, narsalar, zona devori va birinchi shaxs qurol.
    pub fn frame(&self) -> FrameData {
        let mut d = Lists::default();
        let c = &self.companion;
        models::girl(
            &mut d,
            &GirlState {
                pos: c.pos,
                yaw: c.yaw,
                phase: c.phase,
                moving: (c.speed / 4.0).clamp(0.0, 1.0),
                time: self.time,
                look_yaw: c.look_yaw,
            },
        );
        for (p, t) in &self.impacts {
            let k = 1.0 - t / 0.5;
            d.push(
                Mesh::Sphere,
                Instance::new(*p, Vec3::splat(0.10 + 0.30 * k), [0.78, 0.74, 0.66], Mat::Flat, Quat::IDENTITY),
            );
        }
        let dynamic_shadow = d.counts();

        for p in self.pickups.iter().filter(|p| p.alive) {
            let bob = 0.55 + 0.12 * (self.time * 3.0 + p.pos.x).sin();
            let spin = Quat::from_rotation_y(self.time * 1.2 + p.pos.z);
            let at = Vec3::new(p.pos.x, p.pos.y + bob, p.pos.z);
            let (rgb, beam) = match p.kind {
                PickupKind::Weapon(w) => {
                    let col = w.color();
                    let len = match w {
                        WeaponKind::Pistol => 0.35,
                        WeaponKind::Shotgun => 0.9,
                        WeaponKind::Sniper => 1.0,
                        _ => 0.8,
                    };
                    d.push(Mesh::Cube, Instance::new(at, Vec3::new(0.08, 0.12, len), [0.12, 0.12, 0.14], Mat::Metal, spin));
                    d.push(Mesh::Cube, Instance::new(at + spin * Vec3::new(0.0, -0.1, len * 0.3), Vec3::new(0.06, 0.15, 0.08), [0.1, 0.1, 0.12], Mat::Cloth, spin));
                    d.push(Mesh::Cylinder, Instance::new(at + spin * Vec3::new(0.0, 0.02, -len * 0.5), Vec3::new(0.05, 0.3, 0.05), [0.3, 0.3, 0.33], Mat::Metal, spin * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)));
                    (col, col)
                }
                PickupKind::Ammo => {
                    d.push(Mesh::Cube, Instance::new(at, Vec3::new(0.45, 0.28, 0.3), [0.28, 0.34, 0.2], Mat::Metal, spin));
                    d.push(Mesh::Cube, Instance::new(at + Vec3::new(0.0, 0.16, 0.0), Vec3::new(0.2, 0.03, 0.05), [0.8, 0.7, 0.2], Mat::Flat, spin));
                    ([1.0, 0.85, 0.2], [1.0, 0.85, 0.2])
                }
                PickupKind::Medkit => {
                    d.push(Mesh::Cube, Instance::new(at, Vec3::new(0.42, 0.3, 0.3), [0.95, 0.95, 0.95], Mat::Plaster, spin));
                    d.push(Mesh::Cube, Instance::new(at + spin * Vec3::new(0.0, 0.0, -0.152), Vec3::new(0.24, 0.07, 0.01), [0.85, 0.1, 0.12], Mat::Flat, spin));
                    d.push(Mesh::Cube, Instance::new(at + spin * Vec3::new(0.0, 0.0, -0.152), Vec3::new(0.07, 0.24, 0.01), [0.85, 0.1, 0.12], Mat::Flat, spin));
                    ([0.95, 0.95, 0.95], [0.3, 0.9, 0.4])
                }
            };
            let _ = rgb;
            d.push(Mesh::Cylinder, Instance::new(Vec3::new(p.pos.x, p.pos.y + 3.0, p.pos.z), Vec3::new(0.10, 6.0, 0.10), beam, Mat::Emissive, Quat::IDENTITY));
        }

        // zona devori: yorug' ustunlar (joriy — ko'k, keyingi — oq)
        let ring = |d: &mut Lists, c: Vec2, r: f32, n: usize, w: f32, col: [f32; 3]| {
            for k in 0..n {
                let a = k as f32 / n as f32 * std::f32::consts::TAU;
                let (x, z) = (c.x + a.cos() * r, c.y + a.sin() * r);
                let y = terrain::height(x, z);
                d.push(Mesh::Cylinder, Instance::new(Vec3::new(x, y + 20.0, z), Vec3::new(w, 40.0, w), col, Mat::Emissive, Quat::IDENTITY));
            }
        };
        let zr = self.zone.radius;
        let n_cur = ((std::f32::consts::TAU * zr / 3.0).ceil() as usize).clamp(24, 320);
        ring(&mut d, self.zone.center, zr, n_cur, 3.3, [0.15, 0.35, 1.0]);
        if !self.zone.finished {
            ring(&mut d, self.zone.next_center, self.zone.next_radius, 40, 0.5, [0.9, 0.9, 0.95]);
        }

        let mut vm = Lists::default();
        if self.dead_t <= 0.0 {
            let st = self.stats();
            models::viewmodel(
                &mut vm,
                &VmState {
                    kind: self.slots[self.cur].kind,
                    recoil: self.recoil,
                    walk_phase: self.walk_phase,
                    moving: (self.player_vel.length() / 5.0).clamp(0.0, 1.0),
                    reload: if self.reload > 0.0 { 1.0 - (self.reload / st.reload).clamp(0.0, 1.0) } else { 0.0 },
                    raise: self.raise,
                    flash: (self.flash / 0.07).clamp(0.0, 1.0),
                    time: self.time,
                },
            );
        }
        let _ = SUN_DIR;
        FrameData { dynamic: d, dynamic_shadow, viewmodel: vm }
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

    /// Joystikni oldinga bosib ushlaydi (kamera qaragan tomonga yuradi).
    fn walk_forward(g: &mut Game) {
        let (home, _) = g.input.stick_home();
        g.touch(TouchAction::Down, 1, home.x, home.y);
        g.touch(TouchAction::Move, 1, home.x, home.y - 300.0);
    }

    fn stop(g: &mut Game) {
        g.touch(TouchAction::Up, 1, 0.0, 0.0);
    }

    #[test]
    fn zaminda_turadi() {
        let mut g = Game::new(1280, 576, 2.0);
        step(&mut g, 60);
        assert!((g.camera.pos.y - EYE).abs() < 1e-3, "y = {}", g.camera.pos.y);
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
    fn qutilar_ichiga_kirib_bolmaydi() {
        let g = Game::new(1280, 576, 2.0);
        for b in g.colliders.iter().take(200) {
            let c = (b.min + b.max) * 0.5;
            let mut feet = Vec3::new(c.x, terrain::height(c.x, c.z), c.z);
            push_out(&mut feet, HEIGHT, &g.colliders);
            for bb in &g.colliders {
                if bb.max.y - feet.y <= STEP {
                    continue;
                }
                let inside = feet.x > bb.min.x && feet.x < bb.max.x && feet.z > bb.min.z && feet.z < bb.max.z && feet.y + HEIGHT > bb.min.y;
                assert!(!inside, "quti ichida qoldi: {feet:?} / {bb:?}");
            }
        }
    }

    #[test]
    fn relyefda_yuradi_va_yerda_qoladi() {
        let mut g = Game::new(1280, 576, 2.0);
        g.camera.pos = Vec3::new(60.0, EYE + terrain::height(60.0, 0.0), 0.0);
        g.camera.yaw = std::f32::consts::FRAC_PI_2; // -X ga emas, +... qaysi tomon bo'lsa ham
        walk_forward(&mut g);
        for _ in 0..240 {
            g.update(1.0 / 60.0);
            let feet = g.camera.pos.y - EYE;
            let h = support(Vec3::new(g.camera.pos.x, feet, g.camera.pos.z), &g.colliders);
            assert!(feet >= h - 0.01 && feet - h < 0.5, "feet {feet} ground {h}");
        }
    }

    #[test]
    fn zona_qisqaradi_va_ichida_qoladi() {
        let mut g = Game::new(1280, 576, 2.0);
        let r0 = g.zone.radius;
        for _ in 0..(60 * 300) {
            g.zone.update(1.0 / 60.0, &mut g.rng);
            assert!(g.zone.center.length() + g.zone.radius <= r0 + 1e-3);
        }
        assert!(g.zone.radius < 20.0, "qisqardi");
    }

    #[test]
    fn zona_tashqarisida_jon_kamayadi() {
        let mut g = Game::new(1280, 576, 2.0);
        g.zone.center = Vec2::new(60.0, 60.0);
        g.zone.radius = 3.0;
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
        step(&mut g, 30); // qurol ko'tarilsin
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
    fn hodisalar_otishda_chiqadi() {
        let mut g = Game::new(1280, 576, 2.0);
        g.pickups.clear();
        step(&mut g, 30);
        let (c, _) = g.input.fire_button();
        g.touch(TouchAction::Down, 8, c.x, c.y);
        step(&mut g, 2);
        let ev = g.take_events();
        assert!(ev.contains(&audio::EV_SHOT_PISTOL));
        assert!(g.take_events().is_empty());
    }

    #[test]
    fn qiz_5_metr_ichida_turadi() {
        let mut g = Game::new(1280, 576, 2.0);
        g.companion.pos = Vec3::new(g.camera.pos.x + 2.0, 0.0, g.camera.pos.z);
        let start = g.companion.pos;
        // o'yinchi 2 soniya yuradi (qiz hali 5 m dan uzoqlashmagan)
        walk_forward(&mut g);
        for _ in 0..40 {
            g.update(1.0 / 60.0);
            let d = Vec2::new(g.camera.pos.x - g.companion.pos.x, g.camera.pos.z - g.companion.pos.z).length();
            if d > COMPANION_RANGE {
                break;
            }
            assert!((g.companion.pos - start).length() < 1e-3, "5 m ichida qimirladi");
        }
    }

    #[test]
    fn qiz_oyinchi_yurgan_tomonga_yuradi_va_toxtasa_toxtaydi() {
        let mut g = Game::new(1280, 576, 2.0);
        g.camera.pos = Vec3::new(0.0, EYE, 20.0);
        g.camera.yaw = 0.0; // -Z ga qaraydi
        g.companion.pos = Vec3::new(2.0, 0.0, 20.0);
        g.update(1.0 / 60.0);
        walk_forward(&mut g);
        let z0 = g.companion.pos.z;
        step(&mut g, 60 * 5);
        // qiz o'yinchi bilan birga -Z tomonga siljigan va orqada qolmagan
        assert!(g.companion.pos.z < z0 - 5.0, "qiz yurmadi: {} -> {}", z0, g.companion.pos.z);
        let d = Vec2::new(g.camera.pos.x - g.companion.pos.x, g.camera.pos.z - g.companion.pos.z).length();
        assert!(d < 10.0, "juda uzoqda: {d}");
        // o'yinchi to'xtadi — qiz tez to'xtaydi
        stop(&mut g);
        step(&mut g, 30);
        let p = g.companion.pos;
        step(&mut g, 60);
        assert!((g.companion.pos - p).length() < 0.05, "to'xtamadi");
        assert!(g.companion.speed < 0.05);
    }

    #[test]
    fn nur_relyefga_uradi() {
        let g = Game::new(1280, 576, 2.0);
        let t = g.terrain_hit(Vec3::new(0.0, 1.7, 12.0), Vec3::new(0.0, -1.0, 0.0));
        assert!((t - 1.7).abs() < 0.05, "{t}");
        assert_eq!(g.terrain_hit(Vec3::new(0.0, 5.0, 12.0), Vec3::new(0.0, 1.0, 0.0)), f32::MAX);
    }
}
