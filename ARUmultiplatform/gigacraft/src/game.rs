//! O'yin mantig'i (GL siz): teginish boshqaruvi, o'yinchi, blok qo'yish/buzish, pauza va sozlamalar, saqlash.
use crate::audio::Sfx;
use crate::blocks::*;
use crate::entity::{self, Mob, Particle};
use crate::items::{self, Inventory, Item, Stack};
use crate::math::*;
use crate::mesh::Vertex;
use crate::planet::*;
use crate::player::{Input, Player, FLY_MULT};
use crate::ray::{raycast, Hit};
use crate::sky::{self, DAY_SECS};
use crate::ui::*;
use crate::vitals::Vitals;
use crate::world::World;
use std::sync::Arc;

/// Blok qo'yish/buzish masofasi (blok): 28 blok = 7 m.
pub const REACH: f64 = 28.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Role {
    Joy { c: (f32, f32), k: (f32, f32) },
    Look,
    Btn(Btn),
}

#[derive(Clone, Copy, Debug)]
pub struct Touch {
    pub id: i32,
    pub role: Role,
    pub last: (f32, f32),
}

pub struct Game {
    pub world: Arc<World>,
    pub player: Player,
    pub hotbar: [u8; 9],
    pub sel: usize,
    pub mode: Mode,
    pub time: f32,
    pub quality: usize,
    pub sens: f32,
    pub w: f32,
    pub h: f32,
    pub touches: Vec<Touch>,
    pub target: Option<Hit>,
    /// Qayta qurish kerak bo'lgan ustunlar (yuz, cu, cv).
    pub dirty: Vec<(u8, i32, i32)>,
    pub toast: Option<(String, f32)>,
    pub fps: f32,
    pub loading: f32,
    act_t: f32,
    /// Omon qolish rejimi (hayot, ochlik, buzish vaqti, uchish yo'q).
    pub survival: bool,
    pub vitals: Vitals,
    /// Qayta tug'ilish joyi.
    pub spawn: D3,
    airborne: bool,
    peak_r: f64,
    break_t: f32,
    break_cell: Option<Cell>,
    was_ground: bool,
    prev_pos: D3,
    /// Omon qolish inventari (0..9 — tez panel).
    pub inv: Inventory,
    pub cursor: Stack,
    pub grid: [Stack; 9],
    pub station: Station,
    fuel_left: u32,
    pub mobs: Vec<Mob>,
    pub parts: Vec<Particle>,
    crops: Vec<(Cell, f32)>,
    rng: u32,
    pub rain: f32,
    rain_to: f32,
    weather_t: f32,
    /// Chalinishi kerak bo'lgan tovushlar (ilova bo'shatadi).
    pub sfx: Vec<Sfx>,
    step_d: f32,
    step_pos: D3,
    hit_cd: f32,
    spawn_t: f32,
    cursor_pos: (f32, f32),
    recipes: Vec<items::Recipe>,
}

/// O'yin sessiyalari orasida saqlanadigan holat.
pub struct Saved {
    pub pos: D3,
    pub heading: D3,
    pub pitch: f32,
    pub flying: bool,
    pub hotbar: [u8; 9],
    pub sel: usize,
    pub time: f32,
    pub quality: usize,
    pub sens: f32,
    pub survival: bool,
    pub health: f32,
    pub hunger: f32,
    pub inv: Inventory,
}

/// Boshlanish joyi: Toshkent (41.3° sh.k., 69.28° sh.u.) yoki eng yaqin quruqlik.
pub fn find_spawn(world: &World) -> D3 {
    let g = &world.gen;
    for k in 0..200 {
        let (dla, dlo) = if k == 0 { (0.0, 0.0) } else { ((k as f64 * 0.37).sin() * k as f64 * 0.05, (k as f64 * 0.37).cos() * k as f64 * 0.05) };
        let d = crate::earth::latlon_dir(41.3 + dla, 69.28 + dlo);
        let (f, a, b) = dir_to_face(d);
        let (u, v) = (a as i64, b as i64);
        if u < 300 || v < 300 || u > N - 300 || v > N - 300 {
            continue;
        }
        let (s, kind) = g.surf_at(f, u, v);
        if s > SEA_R + 2 && kind != SNOW {
            let mut p = d_mul(dir(f, u as f64 + 0.5, v as f64 + 0.5), s as f64 + 1.0 + 0.01);
            let pl = Player::new(p);
            let mut tries = 0;
            while pl.collides(world, p) && tries < 120 {
                p = d_add(p, d_mul(d_norm(p), 1.0));
                tries += 1;
            }
            return p;
        }
    }
    d_mul([0.0, 1.0, 0.0], SEA_R as f64 + 80.0)
}

impl Game {
    pub fn new(world: Arc<World>, saved: Option<Saved>) -> Game {
        let spawn = find_spawn(&world);
        let mut g = Game {
            world,
            player: Player::new(spawn),
            hotbar: [GRASS, DIRT, STONE, SAND, LOG, LEAF, PLANK, WHITE, CRYSTAL],
            sel: 0,
            mode: Mode::Play,
            time: sky::PHASE_DAY,
            quality: 1,
            sens: 1.0,
            w: 1920.0,
            h: 1080.0,
            touches: Vec::new(),
            target: None,
            dirty: Vec::new(),
            toast: None,
            fps: 0.0,
            loading: 0.0,
            act_t: 0.0,
            survival: false,
            vitals: Vitals::new(),
            spawn,
            airborne: false,
            peak_r: 0.0,
            break_t: 0.0,
            break_cell: None,
            was_ground: false,
            prev_pos: spawn,
            inv: Inventory::default(),
            cursor: Stack::EMPTY,
            grid: [Stack::EMPTY; 9],
            station: Station::Hand,
            fuel_left: 0,
            mobs: Vec::new(),
            parts: Vec::new(),
            crops: Vec::new(),
            rng: 0x2545_F491,
            rain: 0.0,
            rain_to: 0.0,
            weather_t: 120.0,
            sfx: Vec::new(),
            step_d: 0.0,
            step_pos: spawn,
            hit_cd: 0.0,
            spawn_t: 1.0,
            cursor_pos: (0.0, 0.0),
            recipes: items::recipes(),
        };
        if let Some(s) = saved {
            g.player.pos = s.pos;
            g.player.heading = s.heading;
            g.player.pitch = s.pitch;
            g.player.flying = s.flying;
            g.hotbar = s.hotbar;
            g.sel = s.sel.min(8);
            g.time = s.time;
            g.quality = s.quality.min(2);
            g.sens = s.sens.clamp(0.4, 2.5);
            g.survival = s.survival;
            g.vitals.health = s.health.clamp(0.0, crate::vitals::MAX_HP);
            g.vitals.hunger = s.hunger.clamp(0.0, crate::vitals::MAX_FOOD);
            g.prev_pos = g.player.pos;
            g.step_pos = g.player.pos;
            g.inv = s.inv;
            if g.survival {
                g.player.flying = false;
            }
            if g.vitals.health <= 0.0 {
                g.vitals.health = crate::vitals::MAX_HP;
            }
        }
        g
    }

    pub fn resize(&mut self, w: f32, h: f32) {
        self.w = w;
        self.h = h;
    }

    fn layout(&self) -> Layout {
        Layout::new(self.w, self.h)
    }

    pub fn say(&mut self, s: &str) {
        self.toast = Some((s.to_string(), 2.2));
    }

    fn has(&self, f: impl Fn(&Role) -> bool) -> bool {
        self.touches.iter().any(|t| f(&t.role))
    }

    fn held(&self, b: Btn) -> bool {
        self.has(|r| *r == Role::Btn(b))
    }

    pub fn touch_down(&mut self, id: i32, x: f32, y: f32) {
        let l = self.layout();
        match self.mode {
            Mode::Pause => {
                if let Some(a) = l.pause_at(x, y) {
                    self.pause_action(a);
                }
            }
            Mode::Inv => {
                let c = l.inv_close();
                let (px, py, pw, ph) = l.inv_panel();
                if c.hit(x, y) || x < px || x > px + pw || y < py || y > py + ph {
                    self.mode = Mode::Play;
                } else if let Some(i) = l.inv_at(x, y) {
                    self.hotbar[self.sel] = PLACEABLE[i];
                    self.mode = Mode::Play;
                }
            }
            Mode::Craft => {
                self.cursor_pos = (x, y);
                let (px, py, pw, ph) = l.craft_panel();
                match l.craft_at(x, y, self.station) {
                    Some(CraftHit::Close) => self.close_craft(),
                    Some(CraftHit::Inv(i)) => items::click_slot(&mut self.inv.s[i], &mut self.cursor),
                    Some(CraftHit::Grid(i)) => items::click_slot(&mut self.grid[i], &mut self.cursor),
                    Some(CraftHit::Result) => self.take_result(),
                    None => {
                        if self.cursor.is_empty() && (x < px || x > px + pw || y < py || y > py + ph) {
                            self.close_craft();
                        }
                    }
                }
            }
            Mode::Dead => {
                let (bx, by, bw, bh) = l.respawn_btn();
                if x >= bx && x <= bx + bw && y >= by && y <= by + bh {
                    self.respawn();
                }
            }
            Mode::Play => {
                if let Some(b) = l.button_at(x, y, self.player.flying, self.survival) {
                    match b {
                        Btn::Pause => self.mode = Mode::Pause,
                        Btn::Inv => self.open_inv(),
                        Btn::Fly => {
                            self.player.flying = !self.player.flying;
                            self.player.vel = [0.0; 3];
                            let t = if self.player.flying { "UCHISH YOQILDI" } else { "UCHISH O'CHIRILDI" };
                            self.say(t);
                        }
                        Btn::Speed => {
                            self.player.fly_level = (self.player.fly_level + 1) % FLY_MULT.len();
                        }
                        Btn::Break | Btn::Place => {
                            self.touches.push(Touch { id, role: Role::Btn(b), last: (x, y) });
                            if b == Btn::Break {
                                if !self.survival {
                                    self.break_block();
                                }
                            } else {
                                self.place_block();
                            }
                            self.act_t = 0.32;
                        }
                        _ => self.touches.push(Touch { id, role: Role::Btn(b), last: (x, y) }),
                    }
                } else if let Some(i) = l.slot_at(x, y) {
                    if i == self.sel {
                        self.open_inv();
                    }
                    self.sel = i;
                } else if x < self.w * 0.45 && y > self.h * 0.15 && !self.has(|r| matches!(r, Role::Joy { .. })) {
                    self.touches.push(Touch { id, role: Role::Joy { c: (x, y), k: (x, y) }, last: (x, y) });
                } else if !self.has(|r| *r == Role::Look) {
                    self.touches.push(Touch { id, role: Role::Look, last: (x, y) });
                }
            }
        }
    }

    pub fn touch_move(&mut self, id: i32, x: f32, y: f32) {
        self.cursor_pos = (x, y);
        let r = self.layout().joy_r;
        let sens = 3.2 / self.h * self.sens;
        let mut look = (0.0, 0.0);
        for t in self.touches.iter_mut().filter(|t| t.id == id) {
            match &mut t.role {
                Role::Joy { c, k } => {
                    let (dx, dy) = (x - c.0, y - c.1);
                    let d = (dx * dx + dy * dy).sqrt();
                    let f = if d > r { r / d } else { 1.0 };
                    *k = (c.0 + dx * f, c.1 + dy * f);
                }
                Role::Look => look = (x - t.last.0, y - t.last.1),
                Role::Btn(_) => {}
            }
            t.last = (x, y);
        }
        if look != (0.0, 0.0) && self.mode == Mode::Play {
            self.player.look(look.0, look.1, sens);
        }
    }

    pub fn touch_up(&mut self, id: i32) {
        self.touches.retain(|t| t.id != id);
    }

    pub fn touch_cancel_all(&mut self) {
        self.touches.clear();
    }

    pub fn pause_action(&mut self, a: PauseAct) {
        match a {
            PauseAct::Resume => self.mode = Mode::Play,
            PauseAct::Mode => {
                self.survival = !self.survival;
                if self.survival {
                    self.player.flying = false;
                    self.vitals.respawn();
                }
                let t = if self.survival { "OMON QOLISH REJIMI" } else { "IJODIY REJIM" };
                self.say(t);
            }
            PauseAct::Quality => self.quality = (self.quality + 1) % 3,
            PauseAct::Day => self.time = sky::PHASE_DAY,
            PauseAct::Sunset => self.time = sky::PHASE_SUNSET,
            PauseAct::Night => self.time = sky::PHASE_NIGHT,
            PauseAct::SensDown => self.sens = ((self.sens - 0.2) * 10.0).round() / 10.0,
            PauseAct::SensUp => self.sens = ((self.sens + 0.2) * 10.0).round() / 10.0,
        }
        self.sens = self.sens.clamp(0.4, 2.5);
    }

    pub fn eye_dir(&self) -> D3 {
        self.player.view_dir()
    }

    fn aim(&mut self) {
        self.target = raycast(&self.world, self.player.eye(), self.eye_dir(), REACH);
    }

    /// Ijodiy rejimda blokni (yoki jonzotni) darhol buzadi.
    pub fn break_block(&mut self) {
        self.aim();
        if let Some(i) = self.mob_hit() {
            self.hit_mob(i);
            return;
        }
        if let Some(h) = self.target {
            let b = self.world.block(&h.cell);
            entity::burst(&mut self.parts, center(&h.cell), color(b, 1), 8, &mut self.rng);
            self.sfx.push(Sfx::Break);
            let d = self.world.set_block(&h.cell, AIR);
            self.dirty.extend(d);
            self.aim();
        }
    }

    /// Qo'ldagi katak (omon qolishda inventardan, ijodiyda tez paneldan).
    pub fn held_stack(&self) -> Stack {
        if self.survival {
            self.inv.s[self.sel]
        } else {
            Stack::new(self.hotbar[self.sel] as Item, 1)
        }
    }

    pub fn open_inv(&mut self) {
        if self.survival {
            self.open_craft(Station::Hand);
        } else {
            self.mode = Mode::Inv;
        }
    }

    fn open_craft(&mut self, st: Station) {
        self.station = st;
        self.grid = [Stack::EMPTY; 9];
        self.cursor = Stack::EMPTY;
        self.touches.clear();
        self.mode = Mode::Craft;
    }

    /// Katakchadagi va kursordagi buyumlar inventarga qaytadi.
    pub fn close_craft(&mut self) {
        let mut back: Vec<Stack> = self.grid.to_vec();
        back.push(self.cursor);
        for st in back {
            self.give_stack(st);
        }
        self.grid = [Stack::EMPTY; 9];
        self.cursor = Stack::EMPTY;
        self.mode = Mode::Play;
        self.touches.clear();
    }

    fn give_stack(&mut self, st: Stack) {
        if st.is_empty() {
            return;
        }
        if items::is_tool(st.item) {
            if let Some(slot) = self.inv.s.iter_mut().find(|s| s.is_empty()) {
                *slot = st;
            }
        } else {
            self.inv.add(st.item, st.n as u32);
        }
    }

    fn has_fuel(&self) -> bool {
        self.inv.s.iter().any(|s| !s.is_empty() && items::fuel(s.item) > 0)
    }

    /// Hozirgi katakchalar bo'yicha natija.
    pub fn craft_result(&self) -> Stack {
        match self.station {
            Station::Furnace => {
                let i = self.grid[0];
                if i.is_empty() {
                    return Stack::EMPTY;
                }
                match items::smelt(i.item) {
                    Some(out) if self.fuel_left > 0 || self.has_fuel() => Stack::new(out, 1),
                    _ => Stack::EMPTY,
                }
            }
            st => {
                let g = st.grid();
                let arr: Vec<Item> = (0..g * g).map(|k| if self.grid[k].is_empty() { items::NONE } else { self.grid[k].item }).collect();
                match items::craft(&arr, g, g, &self.recipes) {
                    Some((o, n)) => Stack::new(o, n),
                    None => Stack::EMPTY,
                }
            }
        }
    }

    fn take_result(&mut self) {
        let r = self.craft_result();
        if r.is_empty() {
            return;
        }
        if !self.cursor.is_empty() && (self.cursor.item != r.item || self.cursor.n as u32 + r.n as u32 > self.cursor.max() as u32) {
            return;
        }
        if self.station == Station::Furnace {
            if self.fuel_left == 0 {
                if let Some(i) = self.inv.s.iter().position(|s| !s.is_empty() && items::fuel(s.item) > 0) {
                    self.fuel_left += items::fuel(self.inv.s[i].item);
                    self.inv.use_one(i);
                }
            }
            self.fuel_left = self.fuel_left.saturating_sub(1);
            self.grid[0].n -= 1;
            if self.grid[0].n == 0 {
                self.grid[0] = Stack::EMPTY;
            }
        } else {
            for k in 0..9 {
                if !self.grid[k].is_empty() {
                    self.grid[k].n -= 1;
                    if self.grid[k].n == 0 {
                        self.grid[k] = Stack::EMPTY;
                    }
                }
            }
        }
        if self.cursor.is_empty() {
            self.cursor = r;
        } else {
            self.cursor.n += r.n;
        }
        self.sfx.push(Sfx::Craft);
    }

    /// Sandiq: ichidagi o'lja inventarga tushadi, sandiq yo'qoladi.
    fn open_chest(&mut self, cell: Cell) {
        let h = crate::noise::hash3(77, cell.u, cell.r, cell.v);
        let mut loot: Vec<(Item, u32)> = vec![
            (items::COAL, 3 + h % 6),
            (items::IRON_INGOT, 1 + (h >> 4) % 4),
            (items::BREAD, 1 + (h >> 8) % 3),
            (items::SEEDS, 2 + (h >> 10) % 5),
        ];
        if (h >> 13) % 4 == 0 {
            loot.push((items::GOLD_INGOT, 1 + (h >> 15) % 3));
        }
        if (h >> 17) % 8 == 0 {
            loot.push((items::DIAMOND, 1 + (h >> 20) % 2));
        }
        if (h >> 22) % 5 == 0 {
            loot.push((items::tool(0, 0), 1));
        }
        for (it, n) in loot {
            if items::is_tool(it) {
                self.give_stack(Stack::new(it, 1));
            } else {
                self.inv.add(it, n);
            }
        }
        let d = self.world.set_block(&cell, AIR);
        self.dirty.extend(d);
        self.sfx.push(Sfx::Pickup);
        self.say("SANDIQ OCHILDI");
    }

    /// Nishondagi jonzot (blokdan yaqin bo'lsa).
    fn mob_hit(&self) -> Option<usize> {
        let o = self.player.eye();
        let d = self.eye_dir();
        let limit = self.target.map_or(REACH, |t| (t.dist as f64).min(REACH));
        let mut best: Option<(usize, f64)> = None;
        for (i, m) in self.mobs.iter().enumerate() {
            if let Some(t) = entity::ray_hit(m, o, d, limit) {
                if best.map_or(true, |b| t < b.1) {
                    best = Some((i, t));
                }
            }
        }
        best.map(|b| b.0)
    }

    fn hit_mob(&mut self, i: usize) {
        let held = self.held_stack();
        let dmg = if items::tool_kind(held.item) == Tool::Sword {
            3.0 + items::tool_tier(held.item) as f32
        } else if items::is_tool(held.item) {
            2.0
        } else {
            1.0
        };
        if self.survival && items::is_tool(held.item) && self.inv.wear(self.sel) {
            self.say("ASBOB SINDI");
        }
        let ppos = self.player.pos;
        let Some(m) = self.mobs.get_mut(i) else { return };
        m.hp -= dmg;
        m.hurt = 0.4;
        let up = d_norm(m.pos);
        let away = d_sub(m.pos, ppos);
        let flat = d_sub(away, d_mul(up, d_dot(away, up)));
        if d_len(flat) > 1e-6 {
            m.pos = d_add(m.pos, d_mul(d_norm(flat), 2.5));
        }
        let (kind, dead, at) = (m.kind, m.hp <= 0.0, m.center());
        entity::burst(&mut self.parts, at, [0.85, 0.15, 0.15], 8, &mut self.rng);
        self.sfx.push(Sfx::Hit);
        if dead {
            self.mobs.remove(i);
            if self.survival {
                let r = entity::rnd(&mut self.rng);
                let drop: Option<(Item, u32)> = match kind {
                    entity::COW => Some((items::RAW_MEAT, 1 + (r * 2.0) as u32)),
                    entity::SHEEP | entity::CHICKEN => Some((items::RAW_MEAT, 1)),
                    _ if r < 0.5 => Some((items::COAL, 1)),
                    _ if r < 0.7 => Some((items::IRON_INGOT, 1)),
                    _ => None,
                };
                if let Some((it, n)) = drop {
                    self.inv.add(it, n);
                    self.sfx.push(Sfx::Pickup);
                }
                if kind == entity::CHICKEN {
                    self.inv.add(items::SEEDS, 1);
                }
            }
            self.sfx.push(if kind == entity::ZOMBIE { Sfx::Groan } else { Sfx::Moo });
        }
    }

    pub fn place_block(&mut self) {
        let held = self.held_stack();
        if held.is_empty() {
            return;
        }
        if self.survival && items::is_food(held.item) {
            if self.vitals.hunger < crate::vitals::MAX_FOOD {
                self.vitals.eat(items::food_value(held.item));
                self.inv.use_one(self.sel);
                self.sfx.push(Sfx::Eat);
            }
            return;
        }
        self.aim();
        let Some(h) = self.target else { return };
        if self.survival {
            match self.world.block(&h.cell) {
                CRAFTING => return self.open_craft(Station::Table),
                FURNACE => return self.open_craft(Station::Furnace),
                CHEST => return self.open_chest(h.cell),
                _ => {}
            }
        }
        if held.item == items::SEEDS {
            let soil = matches!(self.world.block(&h.cell), GRASS | DIRT | DRYGRASS);
            let above = Cell { r: h.cell.r + 1, ..h.cell };
            if soil && h.place == above && self.world.block(&above) == AIR {
                let d = self.world.set_block(&above, CROP0);
                self.dirty.extend(d);
                self.crops.push((above, 0.0));
                self.inv.use_one(self.sel);
                self.sfx.push(Sfx::Place);
                self.aim();
            }
            return;
        }
        if !items::is_block(held.item) {
            return;
        }
        let b = held.item as u8;
        // O'yinchi ichiga blok qo'ymaymiz
        let (right, up, fwd) = self.player.camera_basis_flat();
        let rel = d_sub(center(&h.place), self.player.pos);
        let (x, y, z) = (d_dot(rel, right), d_dot(rel, up), d_dot(rel, fwd));
        let overlap = x.abs() < crate::player::HALF_W + 0.5 && z.abs() < crate::player::HALF_W + 0.5 && y > -0.5 && y < crate::player::HEIGHT + 0.5;
        if overlap && !self.player.ghost() && solid(b) {
            return;
        }
        let d = self.world.set_block(&h.place, b);
        self.dirty.extend(d);
        if self.survival {
            self.inv.use_one(self.sel);
        }
        self.sfx.push(Sfx::Place);
        self.aim();
    }

    pub fn respawn(&mut self) {
        self.player = Player::new(self.spawn);
        self.vitals.respawn();
        self.touches.clear();
        self.airborne = false;
        self.prev_pos = self.spawn;
        self.step_pos = self.spawn;
        self.mobs.retain(|m| !entity::hostile(m.kind));
        self.mode = Mode::Play;
    }

    pub fn update(&mut self, dt: f32, ready: bool) {
        self.toast = match self.toast.take() {
            Some((t, left)) if left - dt > 0.0 => Some((t, left - dt)),
            _ => None,
        };
        if self.mode != Mode::Play || !ready {
            return;
        }
        self.time = (self.time + dt / DAY_SECS).fract();
        let r = self.layout().joy_r;
        let mut mv = [0.0f32, 0.0];
        for t in &self.touches {
            if let Role::Joy { c, k } = t.role {
                let (dx, dy) = ((k.0 - c.0) / r, (k.1 - c.1) / r);
                let d = (dx * dx + dy * dy).sqrt();
                if d > 0.12 {
                    let f = ((d - 0.12) / 0.88).min(1.0) / d;
                    mv = [dx * f, -dy * f];
                }
            }
        }
        let inp = Input {
            mv,
            jump: self.held(Btn::Jump),
            down: self.held(Btn::Down),
        };
        let w = self.world.clone();
        if self.survival {
            self.player.flying = false;
        }
        let was_ground = self.player.on_ground;
        self.player.update(&w, &inp, dt);
        if self.survival {
            self.survival_tick(dt, &inp, was_ground);
        }
        self.aim();
        self.survival_break(dt);
        self.footsteps();
        self.life(dt);
        self.act_t -= dt;
        if self.act_t <= 0.0 {
            if self.held(Btn::Break) && !self.survival {
                self.break_block();
                self.act_t = 0.18;
            } else if self.held(Btn::Place) {
                self.place_block();
                self.act_t = 0.25;
            }
        }
    }

    /// Omon qolish: charchash, tushish zarari, nafas, o'lim.
    fn survival_tick(&mut self, dt: f32, inp: &Input, was_ground: bool) {
        let pl = &self.player;
        let meters = (d_len(d_sub(pl.pos, self.prev_pos)) / 4.0) as f32;
        self.prev_pos = pl.pos;
        if pl.on_ground || pl.in_water {
            self.vitals.exhaust(meters * if pl.running { 0.1 } else if pl.in_water { 0.015 } else { 0.01 });
        }
        if inp.jump && was_ground {
            self.vitals.exhaust(0.05);
        }
        // Tushish: yerdan ajralgan eng baland nuqtadan qo'nish joyigacha
        let r = d_len(pl.pos);
        if pl.on_ground || pl.in_water || pl.flying {
            if self.airborne && pl.on_ground && !pl.in_water {
                let dist = ((self.peak_r - r) / 4.0) as f32;
                self.vitals.fall(dist);
            }
            self.airborne = false;
            self.peak_r = r;
        } else if !self.airborne {
            self.airborne = true;
            self.peak_r = r;
        } else if r > self.peak_r {
            self.peak_r = r;
        }
        let eye_water = self.world.block(&cell_at(self.player.eye())) == WATER;
        self.vitals.tick(dt, eye_water);
        if self.vitals.dead {
            self.mode = Mode::Dead;
            self.touches.clear();
        }
    }

    /// Blokni buzish uchun kerak bo'ladigan vaqt (soniya): asbob tezlatadi, mos asbobsiz sekinroq.
    fn break_need(&self, b: u8) -> f32 {
        let held = self.inv.s[self.sel].item;
        let mut need = hardness(b) / items::speed(held, b);
        if !items::can_harvest(held, b) {
            need *= 3.3;
        }
        need
    }

    /// Omon qolish rejimida blokni ushlab turib buzish (qattiqligiga va asbobga qarab vaqt ketadi); jonzotga zarba.
    fn survival_break(&mut self, dt: f32) {
        self.hit_cd = (self.hit_cd - dt).max(0.0);
        if !self.survival || !self.held(Btn::Break) {
            self.break_t = 0.0;
            self.break_cell = None;
            return;
        }
        if let Some(i) = self.mob_hit() {
            if self.hit_cd <= 0.0 {
                self.hit_mob(i);
                self.hit_cd = 0.5;
                self.vitals.exhaust(0.1);
            }
            self.break_t = 0.0;
            self.break_cell = None;
            return;
        }
        let Some(t) = self.target else {
            self.break_t = 0.0;
            self.break_cell = None;
            return;
        };
        if self.break_cell != Some(t.cell) {
            self.break_cell = Some(t.cell);
            self.break_t = 0.0;
        }
        self.break_t += dt;
        let b = self.world.block(&t.cell);
        if self.break_t >= self.break_need(b) {
            self.finish_break(t.cell, b);
            self.vitals.exhaust(0.025);
            self.break_t = 0.0;
            self.break_cell = None;
        }
    }

    /// Omon qolishda buzilgan blok: tushgan buyum inventarga, asbob eskiradi, zarrachalar va tovush.
    fn finish_break(&mut self, cell: Cell, b: u8) {
        let held = self.inv.s[self.sel];
        let roll = entity::rnd(&mut self.rng);
        if let Some((it, n)) = items::drop(held.item, b, roll) {
            if self.inv.add(it, n as u32) > 0 {
                self.say("INVENTAR TO'LA");
            } else {
                self.sfx.push(Sfx::Pickup);
            }
        }
        if items::is_tool(held.item) && self.inv.wear(self.sel) {
            self.say("ASBOB SINDI");
        }
        entity::burst(&mut self.parts, center(&cell), color(b, 1), 10, &mut self.rng);
        self.sfx.push(Sfx::Break);
        self.crops.retain(|(c, _)| *c != cell);
        let d = self.world.set_block(&cell, AIR);
        self.dirty.extend(d);
        self.aim();
    }

    /// Qadam tovushi.
    fn footsteps(&mut self) {
        let moved = d_len(d_sub(self.player.pos, self.step_pos));
        self.step_pos = self.player.pos;
        if moved < 20.0 && self.player.on_ground && !self.player.flying {
            self.step_d += moved as f32;
            if self.step_d > 9.0 {
                self.step_d = 0.0;
                self.sfx.push(Sfx::Step);
            }
        }
    }

    /// Jonzotlar, zarrachalar, ekinlar va ob-havo.
    fn life(&mut self, dt: f32) {
        let ppos = self.player.pos;
        self.spawn_t -= dt;
        if self.spawn_t <= 0.0 {
            self.spawn_t = 2.0;
            self.try_spawn();
        }
        let w = self.world.clone();
        let target = if self.survival { Some(ppos) } else { None };
        let mut seed = self.rng;
        let mut hits = 0;
        for m in self.mobs.iter_mut() {
            if d_len(d_sub(m.pos, ppos)) > 160.0 {
                continue;
            }
            if entity::step(m, &w, dt, target, &mut seed) {
                hits += 1;
            }
        }
        let night = sky::state(self.time, d_f32(self.player.up())).night;
        self.mobs.retain(|m| {
            let far = d_len(d_sub(m.pos, ppos)) > 260.0;
            let burn = entity::hostile(m.kind) && night < 0.25 && entity::rnd(&mut seed) < dt * 0.15;
            !far && !burn
        });
        self.rng = seed;
        for _ in 0..hits {
            self.vitals.damage(3.0);
            self.sfx.push(Sfx::Hurt);
            if self.vitals.dead {
                self.mode = Mode::Dead;
                self.touches.clear();
            }
        }
        entity::step_particles(&mut self.parts, dt);
        // Ekinlar o'sadi: har 25 soniyada bir bosqich
        let mut upd: Vec<(Cell, u8)> = Vec::new();
        for (c, age) in self.crops.iter_mut() {
            *age += dt;
            let stage = ((*age / 25.0) as u8).min(3);
            upd.push((*c, CROP0 + stage));
        }
        for (c, b) in upd {
            let cur = self.world.block(&c);
            if is_crop(cur) && cur != b {
                let d = self.world.set_block(&c, b);
                self.dirty.extend(d);
            }
        }
        // Ob-havo
        self.weather_t -= dt;
        if self.weather_t <= 0.0 {
            self.weather_t = 90.0 + entity::rnd(&mut self.rng) * 200.0;
            self.rain_to = if entity::rnd(&mut self.rng) < 0.3 { 1.0 } else { 0.0 };
        }
        self.rain += (self.rain_to - self.rain) * (dt * 0.15).min(1.0);
    }

    fn try_spawn(&mut self) {
        let ppos = self.player.pos;
        let night = sky::state(self.time, d_f32(self.player.up())).night;
        let (passive, hostile) = (self.mobs.iter().filter(|m| !entity::hostile(m.kind)).count(), self.mobs.iter().filter(|m| entity::hostile(m.kind)).count());
        let want_hostile = self.survival && night > 0.6 && hostile < 4 && entity::rnd(&mut self.rng) < 0.6;
        if !want_hostile && passive >= 7 {
            return;
        }
        let kind = if want_hostile { entity::ZOMBIE } else { (entity::rnd(&mut self.rng) * 2.99) as u8 };
        let a = entity::rnd(&mut self.rng) * std::f32::consts::TAU;
        let dist = if want_hostile { 40.0 + entity::rnd(&mut self.rng) as f64 * 40.0 } else { 45.0 + entity::rnd(&mut self.rng) as f64 * 65.0 };
        let (right, _, fwd) = self.player.camera_basis_flat();
        let dirv = d_add(d_mul(right, a.cos() as f64), d_mul(fwd, a.sin() as f64));
        let p = d_mul(d_norm(d_add(ppos, d_mul(dirv, dist))), d_len(ppos));
        let Some(g) = entity::ground(&self.world, p, true) else { return };
        if !want_hostile {
            let below = cell_at(d_sub(g, d_mul(d_norm(g), 0.5)));
            if !matches!(self.world.block(&below), GRASS | DRYGRASS) {
                return;
            }
        }
        self.mobs.push(Mob::new(kind, g, dirv));
    }

    /// Jonzotlar va zarrachalar mesh'i (o'yinchi atrofida).
    pub fn dynamic(&self) -> Option<(D3, Vec<Vertex>)> {
        if self.mobs.is_empty() && self.parts.is_empty() {
            return None;
        }
        let anchor = self.player.pos;
        let mut v = Vec::new();
        for m in &self.mobs {
            if d_len(d_sub(m.pos, anchor)) < 300.0 {
                entity::mob_vertices(m, anchor, &mut v);
            }
        }
        entity::particle_vertices(&self.parts, anchor, &mut v);
        v.truncate(65532);
        let n = v.len() / 4 * 4;
        v.truncate(n);
        if v.is_empty() {
            None
        } else {
            Some((anchor, v))
        }
    }

    fn ui_slots(&self) -> [Stack; 9] {
        let mut o = [Stack::EMPTY; 9];
        for (i, st) in o.iter_mut().enumerate() {
            *st = if self.survival { self.inv.s[i] } else { Stack::new(self.hotbar[i] as Item, 1) };
        }
        o
    }

    pub fn ui_state(&self) -> UiState {
        let l = self.layout();
        let _ = &l;
        let joy = self.touches.iter().find_map(|t| match t.role {
            Role::Joy { c, k } => Some((c, k)),
            _ => None,
        });
        let down: Vec<Btn> = self
            .touches
            .iter()
            .filter_map(|t| match t.role {
                Role::Btn(b) => Some(b),
                _ => None,
            })
            .collect();
        let up = self.player.up();
        let (lat, lon0) = crate::earth::lat_lon(up);
        let lon = if lon0 > 180.0 { lon0 - 360.0 } else { lon0 };
        let alt_m = self.player.altitude() / 4.0;
        let underwater = self.world.block(&cell_at(self.player.eye())) == WATER;
        UiState {
            mode: self.mode,
            slots: self.ui_slots(),
            held: self.held_stack().item,
            craft: if self.mode == Mode::Craft {
                Some(CraftView { station: self.station, inv: self.inv.s, grid: self.grid, result: self.craft_result(), cursor: self.cursor, fuel: self.fuel_left })
            } else {
                None
            },
            cursor_pos: self.cursor_pos,
            rain: self.rain,
            sel: self.sel,
            flying: self.player.flying,
            fly_level: self.player.fly_level,
            joy,
            down,
            info: if alt_m.abs() < 10_000.0 {
                format!("LAT {:.2}  LON {:.2}  ALT {:.0} M  |  {:.0} FPS", lat, lon, alt_m, self.fps)
            } else {
                format!("LAT {:.2}  LON {:.2}  ALT {:.0} KM  |  {:.0} FPS", lat, lon, alt_m / 1000.0, self.fps)
            },
            fps: self.fps,
            quality: self.quality,
            sens: self.sens,
            loading: self.loading,
            toast: self.toast.as_ref().map(|t| t.0.clone()),
            underwater,
            survival: self.survival,
            health: self.vitals.health,
            hunger: self.vitals.hunger,
            air: self.vitals.air,
            flash: self.vitals.flash,
            break_frac: match self.break_cell {
                Some(c) if self.survival => (self.break_t / self.break_need(self.world.block(&c))).clamp(0.0, 1.0),
                _ => 0.0,
            },
        }
    }

    pub fn saved(&self) -> Saved {
        Saved {
            pos: self.player.pos,
            heading: self.player.heading,
            pitch: self.player.pitch,
            flying: self.player.flying,
            hotbar: self.hotbar,
            sel: self.sel,
            time: self.time,
            quality: self.quality,
            sens: self.sens,
            survival: self.survival,
            health: self.vitals.health,
            hunger: self.vitals.hunger,
            inv: self.inv.clone(),
        }
    }
}

/// Saqlash fayli: sarlavha + o'yinchi holati + tahrirlar.
pub fn encode_save(s: &Saved, edits: &[u8]) -> Vec<u8> {
    let mut o = b"GCS5".to_vec();
    for v in s.pos.iter().chain(s.heading.iter()) {
        o.extend_from_slice(&v.to_le_bytes());
    }
    o.extend_from_slice(&s.pitch.to_le_bytes());
    o.push(s.flying as u8);
    o.extend_from_slice(&s.hotbar);
    o.push(s.sel as u8);
    o.extend_from_slice(&s.time.to_le_bytes());
    o.push(s.quality as u8);
    o.extend_from_slice(&s.sens.to_le_bytes());
    o.push(s.survival as u8);
    o.extend_from_slice(&s.health.to_le_bytes());
    o.extend_from_slice(&s.hunger.to_le_bytes());
    for st in &s.inv.s {
        o.extend_from_slice(&st.item.to_le_bytes());
        o.push(st.n);
        o.extend_from_slice(&st.used.to_le_bytes());
    }
    o.extend_from_slice(&(edits.len() as u32).to_le_bytes());
    o.extend_from_slice(edits);
    o
}

pub fn decode_save(d: &[u8]) -> Option<(Saved, &[u8])> {
    if d.len() < 4 + 48 + 4 + 1 + 9 + 1 + 4 + 1 + 4 + 1 + 8 + 4 || (&d[0..4] != b"GCS5" && &d[0..4] != b"GCS4") {
        return None;
    }
    let v5 = &d[0..4] == b"GCS5";
    let f64_at = |o: usize| f64::from_le_bytes(d[o..o + 8].try_into().unwrap());
    let f32_at = |o: usize| f32::from_le_bytes(d[o..o + 4].try_into().unwrap());
    let mut o = 4;
    let pos = [f64_at(o), f64_at(o + 8), f64_at(o + 16)];
    let heading = [f64_at(o + 24), f64_at(o + 32), f64_at(o + 40)];
    o += 48;
    let pitch = f32_at(o);
    o += 4;
    let flying = d[o] != 0;
    o += 1;
    let mut hotbar = [0u8; 9];
    hotbar.copy_from_slice(&d[o..o + 9]);
    o += 9;
    let sel = d[o] as usize;
    o += 1;
    let time = f32_at(o);
    o += 4;
    let quality = d[o] as usize;
    o += 1;
    let sens = f32_at(o);
    o += 4;
    let survival = d[o] != 0;
    o += 1;
    let (health, hunger) = (f32_at(o), f32_at(o + 4));
    o += 8;
    let mut inv = Inventory::default();
    if v5 {
        for st in inv.s.iter_mut() {
            let b = d.get(o..o + 5)?;
            let item = u16::from_le_bytes([b[0], b[1]]);
            let (n, used) = (b[2], u16::from_le_bytes([b[3], b[4]]));
            o += 5;
            if item != 0 && n > 0 && (item as usize) < 256 && n <= items::MAX_STACK {
                *st = Stack { item, n, used };
            }
        }
    }
    if d.len() < o + 4 {
        return None;
    }
    let n = u32::from_le_bytes(d[o..o + 4].try_into().unwrap()) as usize;
    o += 4;
    let edits = d.get(o..o + n)?;
    if pos.iter().chain(heading.iter()).any(|v| !v.is_finite()) || !pitch.is_finite() || !time.is_finite() || !sens.is_finite() || !health.is_finite() || !hunger.is_finite() {
        return None;
    }
    if d_len(pos) < 1000.0 || hotbar.iter().any(|&b| b >= COUNT) {
        return None;
    }
    Some((Saved { pos, heading, pitch, flying, hotbar, sel, time, quality, sens, survival, health, hunger, inv }, edits))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HIGH: i64 = SEA_R + 9000;

    /// Havodagi qalin tosh maydoncha ustida o'yin: tabiiy relyefga bog'liq emas.
    fn game_on_floor() -> Game {
        let w = Arc::new(World::new(1));
        let (f, u0, v0) = (2u8, 30_000_000i64, 30_000_000i64);
        for du in -60..60 {
            for dv in -60..60 {
                for dr in 1..=10 {
                    w.set_block(&Cell { f, u: u0 + du, v: v0 + dv, r: HIGH - dr }, STONE);
                }
            }
        }
        let mut g = Game::new(w, None);
        g.resize(2400.0, 1080.0);
        g.player = Player::new(d_mul(dir(f, u0 as f64 + 0.5, v0 as f64 + 0.5), HIGH as f64));
        g.player.pitch = -0.9; // pastga, polga qarab
        g.dirty.clear();
        g
    }

    #[test]
    fn qo_yish_va_buzish_tugmalari() {
        let mut g = game_on_floor();
        g.update(0.016, true);
        let t = g.target.expect("pol ko'rinmayapti");
        let l = Layout::new(2400.0, 1080.0);
        g.hotbar[0] = CRYSTAL;
        g.sel = 0;
        g.touch_down(1, l.place.cx, l.place.cy);
        assert_eq!(g.world.block(&t.place), CRYSTAL);
        assert!(!g.dirty.is_empty());
        g.touch_up(1);
        g.update(0.016, true);
        g.touch_down(2, l.brk.cx, l.brk.cy);
        assert_eq!(g.world.block(&t.place), AIR);
        g.touch_up(2);
    }

    #[test]
    fn ushlab_turib_takroriy_buzish() {
        let mut g = game_on_floor();
        g.player.pitch = -1.2;
        let l = Layout::new(2400.0, 1080.0);
        g.update(0.016, true);
        g.touch_down(1, l.brk.cx, l.brk.cy);
        let mut n = 0;
        for _ in 0..120 {
            g.update(0.016, true);
            n = g.dirty.len();
        }
        assert!(n >= 6, "takroriy buzish ishlamadi: {n}");
    }

    #[test]
    fn joystik_harakatlantiradi() {
        let mut g = game_on_floor();
        g.player.pitch = 0.0;
        g.update(0.016, true);
        let p0 = g.player.pos;
        g.touch_down(5, 300.0, 800.0);
        g.touch_move(5, 300.0, 800.0 - 150.0);
        for _ in 0..60 {
            g.update(0.016, true);
        }
        assert!(d_len(d_sub(g.player.pos, p0)) > 8.0, "oldinga yurmadi");
        g.touch_up(5);
        assert!(g.touches.is_empty());
    }

    #[test]
    fn qarash_surish_yonalishni_ozgartiradi() {
        let mut g = game_on_floor();
        let h0 = g.player.heading;
        g.touch_down(7, 1700.0, 400.0);
        g.touch_move(7, 1500.0, 400.0);
        let h1 = g.player.heading;
        assert!(d_dot(h0, h1) < 0.99, "yo'nalish o'zgarmadi");
        // chapga surilsa chapga buriladi: (tepa x eski yo'nalish) bo'yicha musbat
        let left = d_cross(g.player.up(), h0);
        assert!(d_dot(h1, left) > 0.0);
        g.touch_up(7);
    }

    #[test]
    fn ikki_barmoq_bir_vaqtda() {
        let mut g = game_on_floor();
        g.touch_down(1, 300.0, 800.0);
        g.touch_down(2, 1700.0, 400.0);
        assert_eq!(g.touches.len(), 2);
        assert!(matches!(g.touches[0].role, Role::Joy { .. }));
        assert_eq!(g.touches[1].role, Role::Look);
        g.touch_down(3, 200.0, 700.0);
        assert_eq!(g.touches.len(), 2);
    }

    #[test]
    fn pauza_va_blok_tanlash() {
        let mut g = game_on_floor();
        let l = Layout::new(2400.0, 1080.0);
        g.touch_down(1, l.pause.cx, l.pause.cy);
        assert_eq!(g.mode, Mode::Pause);
        g.touch_up(1);
        let q0 = g.quality;
        let (x, y, w, h) = l.pause_btn(PauseAct::Quality);
        g.touch_down(1, x + w / 2.0, y + h / 2.0);
        assert_eq!(g.quality, (q0 + 1) % 3);
        let (x, y, w, h) = l.pause_btn(PauseAct::Night);
        g.touch_down(1, x + w / 2.0, y + h / 2.0);
        assert_eq!(g.time, sky::PHASE_NIGHT);
        let (x, y, w, h) = l.pause_btn(PauseAct::Resume);
        g.touch_down(1, x + w / 2.0, y + h / 2.0);
        assert_eq!(g.mode, Mode::Play);
        g.touch_down(1, l.inv.cx, l.inv.cy);
        assert_eq!(g.mode, Mode::Inv);
        let (x, y, w, h) = l.inv_card(3);
        g.touch_down(1, x + w / 2.0, y + h / 2.0);
        assert_eq!(g.mode, Mode::Play);
        assert_eq!(g.hotbar[g.sel], PLACEABLE[3]);
    }

    #[test]
    fn hotbar_tanlash_va_uchish_tugmasi() {
        let mut g = game_on_floor();
        let l = Layout::new(2400.0, 1080.0);
        let (x, y, w, _) = l.slot_rect(4);
        g.touch_down(1, x + w / 2.0, y + w / 2.0);
        assert_eq!(g.sel, 4);
        g.touch_down(1, l.fly.cx, l.fly.cy);
        assert!(g.player.flying);
        g.touch_down(1, l.speed.cx, l.speed.cy);
        assert_eq!(g.player.fly_level, 1);
        for _ in 0..7 {
            g.touch_down(1, l.speed.cx, l.speed.cy);
        }
        assert_eq!(g.player.fly_level, 0, "tezlik darajalari aylanishi kerak");
        g.touch_down(1, l.fly.cx, l.fly.cy);
        assert!(!g.player.flying);
    }

    #[test]
    fn o_yinchi_ichiga_blok_qoyilmaydi() {
        let mut g = game_on_floor();
        g.player.pitch = -1.5; // to'g'ri pastga: o'z oyog'i ostiga
        g.update(0.016, true);
        g.hotbar[0] = STONE;
        g.sel = 0;
        let h = g.target.expect("pol");
        g.place_block();
        assert_eq!(g.world.block(&h.place), AIR, "o'yinchi ichiga blok qo'yildi");
    }

    #[test]
    fn saqlash_aylanadi() {
        let g = game_on_floor();
        let mut s = g.saved();
        s.pos = d_mul(dir(1, 1.0e7, 2.0e7), SEA_R as f64 + 12.5);
        s.hotbar = [3, 4, 5, 6, 7, 8, 10, 11, 12];
        let bytes = encode_save(&s, b"abc");
        let (s2, e) = decode_save(&bytes).unwrap();
        assert_eq!(s2.pos, s.pos);
        assert_eq!(s2.heading, s.heading);
        assert_eq!(s2.survival, s.survival);
        assert_eq!(s2.hotbar, s.hotbar);
        assert_eq!(e, b"abc");
        assert!(decode_save(&bytes[..10]).is_none());
        assert!(decode_save(&[0u8; 120]).is_none());
    }

    #[test]
    fn boshlanish_joyi_yer_ustida() {
        let w = Arc::new(World::new(42));
        let g = Game::new(w.clone(), None);
        assert!(!g.player.collides(&w, g.player.pos), "devor ichida tug'ildi");
        let alt = g.player.altitude();
        assert!(alt > 500.0 && alt < 12000.0, "Toshkent balandligi {alt} blok");
    }

    #[test]
    fn omon_qolishda_uchish_yoq_va_tushish_zarar_beradi() {
        let mut g = game_on_floor();
        let l = Layout::new(2400.0, 1080.0);
        g.mode = Mode::Pause;
        let (x, y, w, h) = l.pause_btn(PauseAct::Mode);
        g.touch_down(1, x + w / 2.0, y + h / 2.0);
        assert!(g.survival);
        g.mode = Mode::Play;
        // uchish tugmasi ishlamaydi
        g.touch_down(1, l.fly.cx, l.fly.cy);
        assert!(!g.player.flying);
        // 10 m (40 blok) balanddan tushish: 7 birlik zarar
        let up = g.player.up();
        g.player.pos = d_add(g.player.pos, d_mul(up, 40.0));
        for _ in 0..200 {
            g.update(0.016, true);
        }
        assert!(g.player.on_ground);
        let lost = crate::vitals::MAX_HP - g.vitals.health;
        assert!(lost >= 5.0 && lost <= 8.0, "tushish zarari {lost}");
    }

    #[test]
    fn ijodiy_rejimda_zarar_yoq() {
        let mut g = game_on_floor();
        let up = g.player.up();
        g.player.pos = d_add(g.player.pos, d_mul(up, 60.0));
        for _ in 0..200 {
            g.update(0.016, true);
        }
        assert_eq!(g.vitals.health, crate::vitals::MAX_HP);
    }

    #[test]
    fn omon_qolishda_buzish_vaqt_oladi() {
        let mut g = game_on_floor();
        g.survival = true;
        g.update(0.016, true);
        let t = g.target.expect("pol");
        let l = Layout::new(2400.0, 1080.0);
        g.touch_down(2, l.brk.cx, l.brk.cy);
        assert_ne!(g.world.block(&t.cell), AIR, "darhol buzilmasligi kerak");
        for _ in 0..300 {
            g.update(0.016, true);
        }
        assert_ne!(g.world.block(&t.cell), AIR, "qo'l bilan 4.8 s da tosh buzilmasligi kerak");
        assert_eq!(g.inv.count(COBBLE as Item), 0, "qo'l bilan tosh buzilsa buyum tushmaydi");
        for _ in 0..600 {
            g.update(0.016, true);
        }
        assert_eq!(g.world.block(&t.cell), AIR, "10 s da buzilishi kerak");
    }

    #[test]
    fn asbob_buzishni_tezlatadi_va_buyum_beradi() {
        let mut g = game_on_floor();
        g.survival = true;
        g.inv.s[0] = Stack::new(items::tool(0, 1), 1); // tosh kurak
        g.sel = 0;
        g.update(0.016, true);
        let t = g.target.expect("pol");
        let l = Layout::new(2400.0, 1080.0);
        g.touch_down(2, l.brk.cx, l.brk.cy);
        for _ in 0..120 {
            g.update(0.016, true);
        }
        assert_eq!(g.world.block(&t.cell), AIR, "tosh kurak bilan 1.9 s da buzilishi kerak");
        let n = g.inv.count(COBBLE as Item);
        assert!(n >= 1);
        assert_eq!(g.inv.s[0].used as u32, n, "asbob har blokda eskirishi kerak");
        assert!(g.sfx.contains(&Sfx::Break));
    }

    #[test]
    fn yasash_ustaxona_va_pech() {
        let mut g = game_on_floor();
        g.survival = true;
        g.inv.add(LOG as Item, 3);
        g.open_inv();
        assert_eq!(g.mode, Mode::Craft);
        // tanani katakka qo'yamiz
        let l = Layout::new(2400.0, 1080.0);
        let (x, y) = l.craft_inv_slot(0);
        g.touch_down(1, x + 10.0, y + 10.0);
        assert_eq!(g.cursor.item, LOG as Item);
        let (gx, gy) = l.craft_grid_slot(Station::Hand, 0);
        g.touch_down(1, gx + 10.0, gy + 10.0);
        assert!(g.cursor.is_empty());
        assert_eq!(g.craft_result(), Stack::new(PLANK as Item, 4));
        let (rx, ry) = l.craft_result_slot();
        g.touch_down(1, rx + 10.0, ry + 10.0);
        assert_eq!(g.cursor, Stack::new(PLANK as Item, 4));
        assert_eq!(g.grid[0].n, 2, "bitta tana sarflanishi kerak");
        // 4 taxtani 2x2 ga: ustaxona
        g.touch_down(1, x + 10.0, y + 10.0); // kursordagini bo'sh katakka (inventar 0 da hozir 0 ta tana yo'q -> almashadi)
        g.close_craft();
        assert_eq!(g.mode, Mode::Play);
        assert!(g.inv.count(PLANK as Item) >= 4);
        assert_eq!(g.inv.count(LOG as Item) + g.inv.count(PLANK as Item) / 4, 3);
        // ustaxona retsepti
        g.inv = Inventory::default();
        g.inv.add(PLANK as Item, 4);
        g.open_inv();
        for k in 0..4 {
            g.grid[k] = Stack::new(PLANK as Item, 1);
        }
        g.inv.s[0].n = 0;
        g.inv.s[0] = Stack::EMPTY;
        assert_eq!(g.craft_result(), Stack::new(CRAFTING as Item, 1));
        g.close_craft();
        // pech: temir rudasini pishirish uchun ko'mir kerak
        g.inv = Inventory::default();
        g.open_craft(Station::Furnace);
        g.grid[0] = Stack::new(IRON_ORE as Item, 2);
        assert_eq!(g.craft_result(), Stack::EMPTY, "yoqilg'isiz pishmaydi");
        g.inv.add(items::COAL, 1);
        assert_eq!(g.craft_result(), Stack::new(items::IRON_INGOT, 1));
        g.take_result();
        assert_eq!(g.cursor, Stack::new(items::IRON_INGOT, 1));
        assert_eq!(g.grid[0].n, 1);
        assert_eq!(g.inv.count(items::COAL), 0, "ko'mir yoqildi");
        assert!(g.fuel_left >= 6, "ko'mir 8 ta pishirishga yetadi");
        g.cursor = Stack::EMPTY;
        g.take_result();
        assert_eq!(g.grid[0].n, 0);
    }

    #[test]
    fn ekin_osadi_va_hosil_beradi() {
        let mut g = game_on_floor();
        g.survival = true;
        g.inv.s[0] = Stack::new(items::SEEDS, 3);
        g.sel = 0;
        g.update(0.016, true);
        let t = g.target.expect("pol");
        // pol toshdan; ekin tuproqqa ekiladi
        g.world.set_block(&t.cell, DIRT);
        g.update(0.016, true);
        g.place_block();
        assert_eq!(g.inv.s[0].n, 2);
        assert_eq!(g.crops.len(), 1);
        let c = g.crops[0].0;
        assert_eq!(g.world.block(&c), CROP0);
        for _ in 0..(80.0 / 0.05) as usize {
            g.update(0.05, true);
        }
        assert_eq!(g.world.block(&c), CROP3, "80 s da pishishi kerak");
        assert!(!solid(CROP3));
    }

    #[test]
    fn ovqat_va_sandiq() {
        let mut g = game_on_floor();
        g.survival = true;
        g.vitals.hunger = 5.0;
        g.inv.s[0] = Stack::new(items::BREAD, 2);
        g.sel = 0;
        g.place_block();
        assert_eq!(g.inv.s[0].n, 1);
        assert!(g.vitals.hunger >= 9.9);
        // sandiq
        g.update(0.016, true);
        let t = g.target.expect("pol");
        g.world.set_block(&t.cell, CHEST);
        g.update(0.016, true);
        g.inv.s[0] = Stack::new(STONE as Item, 5);
        g.place_block();
        assert_eq!(g.world.block(&t.cell), AIR, "sandiq ochilgach yo'qoladi");
        assert!(g.inv.count(items::COAL) >= 3 && g.inv.count(items::IRON_INGOT) >= 1);
        assert_eq!(g.inv.s[0].n, 5, "sandiq ochish blok sarflamaydi");
    }

    #[test]
    fn jonzotga_zarba_va_tushgan_narsa() {
        let mut g = game_on_floor();
        g.survival = true;
        g.update(0.016, true);
        let fwd = g.player.camera_basis_flat().2;
        let pos = d_add(g.player.pos, d_mul(fwd, 8.0));
        let mut m = Mob::new(entity::COW, pos, fwd);
        m.t = 100.0;
        g.mobs.push(m);
        // pastga qaraymiz
        let mut ok = false;
        for k in 0..40 {
            g.player.pitch = -0.02 * k as f32;
            g.aim();
            if g.mob_hit().is_some() {
                ok = true;
                break;
            }
        }
        assert!(ok, "pastga qaraganda ham jonzot topilmadi");
        g.inv.s[0] = Stack::new(items::tool(3, 3), 1); // olmos qilich
        g.sel = 0;
        let l = Layout::new(2400.0, 1080.0);
        g.touch_down(2, l.brk.cx, l.brk.cy);
        for _ in 0..40 {
            g.update(0.016, true);
            if g.mobs.is_empty() {
                break;
            }
        }
        assert!(g.mobs.iter().all(|m| m.kind != entity::COW || m.hp < 10.0), "zarba tegmadi");
    }

    #[test]
    fn zombi_zarar_yetkazadi() {
        let mut g = game_on_floor();
        g.survival = true;
        g.update(0.016, true);
        let pos = d_add(g.player.pos, d_mul(g.player.camera_basis_flat().2, 2.0));
        g.mobs.push(Mob::new(entity::ZOMBIE, pos, [1.0, 0.0, 0.0]));
        let hp0 = g.vitals.health;
        for _ in 0..40 {
            g.update(0.05, true);
        }
        assert!(g.vitals.health < hp0, "zombi urmadi");
    }

    #[test]
    fn saqlashda_inventar_saqlanadi() {
        let mut g = game_on_floor();
        g.survival = true;
        g.inv.add(items::DIAMOND, 3);
        g.inv.s[4] = Stack { item: items::tool(0, 2), n: 1, used: 17 };
        let bytes = encode_save(&g.saved(), &[]);
        let (s2, _) = decode_save(&bytes).expect("o'qish");
        assert_eq!(s2.inv, g.inv);
        assert_eq!(s2.inv.s[4].used, 17);
        // eski GCS4 formati ham o'qiladi
        let mut old = bytes.clone();
        old[3] = b'4';
        let n = 4 + 48 + 4 + 1 + 9 + 1 + 4 + 1 + 4 + 1 + 8;
        let tail = old.split_off(n + items::SLOTS * 5);
        old.truncate(n);
        old.extend_from_slice(&tail);
        let (s3, _) = decode_save(&old).expect("eski format");
        assert!(s3.inv.s.iter().all(|s| s.is_empty()));
    }

    #[test]
    fn olim_va_qayta_tugilish() {
        let mut g = game_on_floor();
        g.survival = true;
        let spawn = g.spawn;
        g.vitals.damage(100.0);
        g.update(0.016, true);
        assert_eq!(g.mode, Mode::Dead);
        let l = Layout::new(2400.0, 1080.0);
        let (x, y, w, h) = l.respawn_btn();
        g.touch_down(1, x + w / 2.0, y + h / 2.0);
        assert_eq!(g.mode, Mode::Play);
        assert_eq!(g.vitals.health, crate::vitals::MAX_HP);
        assert!(d_len(d_sub(g.player.pos, spawn)) < 1e-6, "qayta tug'ilish joyi");
    }
}
