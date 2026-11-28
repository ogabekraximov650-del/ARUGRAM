//! O'yinchi: harakat, sakrash, uchish, suzish va bloklar bilan to'qnashuv sayyora ustida (tortishish markazga qarab).
//! 1 blok = 25 sm, o'yinchi bo'yi 7 blok (1.75 m).
use crate::blocks::{solid, WATER};
use crate::math::*;
use crate::planet::*;
use crate::world::World;

/// Yarim kenglik (blok): 2 blok = 50 sm.
pub const HALF_W: f64 = 1.0;
/// Bo'y (blok): 7 blok = 1.75 m.
pub const HEIGHT: f64 = 7.0;
/// Ko'z balandligi oyoqdan.
pub const EYE: f64 = 6.4;
pub const WALK: f64 = 17.2; // 4.3 m/s
/// Haqiqiy og'irlik kuchi tezlanishi: 9.8 m/s^2 = 39.2 blok/s^2.
pub const GRAVITY: f64 = 39.2;
/// Odamning sakrashi ~ 60 sm (2.5 blok).
pub const JUMP_V: f64 = 14.0;
/// Yugurish tezligi koeffitsienti (joystik oxirigacha surilganda): 6.9 m/s.
pub const RUN: f64 = 1.6;
pub const STEP_UP: f64 = 2.0; // 50 sm gacha pog'onaga o'zi chiqadi
/// Uchish tezligi ko'paytirgichlari. 0-darajadan boshqasida devorlardan o'tadi.
pub const FLY_MULT: [f64; 8] = [1.0, 3.0, 10.0, 30.0, 100.0, 300.0, 1000.0, 3000.0];
pub const FLY_LABELS: [&str; 8] = ["1X", "3X", "10X", "30X", "100X", "300X", "1000X", "3000X"];
pub const FLY_BASE: f64 = 44.0;

#[derive(Clone, Copy, Default)]
pub struct Input {
    /// (yon, oldinga) — joystik, -1..1
    pub mv: [f32; 2],
    pub jump: bool,
    pub down: bool,
}

#[derive(Clone)]
pub struct Player {
    /// Sayyora markaziga nisbatan dunyo koordinatalari (oyoq nuqtasi).
    pub pos: D3,
    pub vel: D3,
    /// Gorizontal (tegma tekislikdagi) oldinga yo'nalish, birlik vektor.
    pub heading: D3,
    pub pitch: f32,
    pub on_ground: bool,
    pub flying: bool,
    pub fly_level: usize,
    pub in_water: bool,
    /// Yugurmoqda.
    pub running: bool,
    /// Kamera tebranishi fazasi va kuchi, tushishdagi egilish (faqat ko'rinish uchun).
    pub bob_phase: f64,
    pub bob_amp: f64,
    pub land_dip: f64,
    /// Yer (yoki suv) yuzasidan balandlik (blok): uchish tezligi va to'qnashuvsiz rejim shunga bog'liq.
    pub agl: f64,
}

/// Joyi `pos` bo'lgan o'yinchi uchun tegma asos: (o'ng, tepa, oldinga).
fn tangent_basis(pos: D3, heading: D3) -> (D3, D3, D3) {
    let up = d_norm(pos);
    let h = d_norm(d_sub(heading, d_mul(up, d_dot(heading, up))));
    let right = d_norm(d_cross(h, up));
    (right, up, h)
}

impl Player {
    pub fn new(pos: D3) -> Player {
        let up = d_norm(pos);
        let r = if up[1].abs() > 0.95 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
        let heading = d_norm(d_sub(r, d_mul(up, d_dot(r, up))));
        Player { pos, vel: [0.0; 3], heading, pitch: 0.0, on_ground: false, flying: false, fly_level: 0, in_water: false, running: false, bob_phase: 0.0, bob_amp: 0.0, land_dip: 0.0, agl: 0.0 }
    }

    pub fn up(&self) -> D3 {
        d_norm(self.pos)
    }

    pub fn eye(&self) -> D3 {
        d_add(self.pos, d_mul(self.up(), EYE))
    }

    /// Chizish uchun ko'z nuqtasi: yurganda tebranadi, qattiq tushganda pastga egiladi.
    pub fn eye_render(&self) -> D3 {
        let (right, up, _) = tangent_basis(self.pos, self.heading);
        let by = self.bob_phase.sin() * 0.16 * self.bob_amp - self.land_dip;
        let bx = (self.bob_phase * 0.5).cos() * 0.10 * self.bob_amp;
        d_add(self.eye(), d_add(d_mul(up, by), d_mul(right, bx)))
    }

    /// Kamera asosi: (o'ng, tepa, oldinga) — qarash burchagi bilan.
    pub fn camera_basis(&self) -> (D3, D3, D3) {
        let (right, up, h) = tangent_basis(self.pos, self.heading);
        let p = self.pitch as f64;
        let fwd = d_add(d_mul(h, p.cos()), d_mul(up, p.sin()));
        let cam_up = d_cross(right, fwd);
        (right, d_norm(cam_up), fwd)
    }

    /// Tegma tekislikdagi asos (qarash burchagisiz): (o'ng, tepa, oldinga).
    pub fn camera_basis_flat(&self) -> (D3, D3, D3) {
        tangent_basis(self.pos, self.heading)
    }

    pub fn view_dir(&self) -> D3 {
        self.camera_basis().2
    }

    /// Gorizontal burilish (radian, musbat — chapga).
    pub fn turn(&mut self, angle: f64) {
        let (_, up, h) = tangent_basis(self.pos, self.heading);
        let side = d_cross(up, h);
        self.heading = d_norm(d_add(d_mul(h, angle.cos()), d_mul(side, angle.sin())));
    }

    pub fn look(&mut self, dx: f32, dy: f32, sens: f32) {
        self.turn(-(dx * sens) as f64);
        self.pitch = (self.pitch - dy * sens).clamp(-1.55, 1.55);
    }

    /// Bloklar bilan to'qnashuvsiz uchish (tez yoki yerdan baland): relyefdan pastga tushmaydi.
    pub fn ghost(&self) -> bool {
        self.flying && (self.fly_level > 0 || self.agl > 200.0)
    }

    /// Uchish tezligi koeffitsienti: yerdan balandlikka mutanosib (ekranda sezilarli tezlik bir xil bo'lsin).
    pub fn fly_scale(&self) -> f64 {
        (self.agl / 40.0).max(1.0)
    }

    /// Joy ostidagi relyef sathi (radius): quruqlik, ko'l yoki daryo suvi yuzasi.
    fn ground_radius(w: &World, p: D3) -> f64 {
        let t = w.gen.terrain(d_norm(p));
        SEA_R as f64 + t.h.max(t.wl) as f64
    }

    pub fn altitude(&self) -> f64 {
        altitude(self.pos)
    }

    /// O'yinchi tanasi (namunalar to'ri) `p` joyda to'siqqa tegadimi.
    pub fn collides_at(&self, w: &World, p: D3, basis: (D3, D3, D3)) -> bool {
        let (right, up, fwd) = basis;
        const HS: [f64; 8] = [0.05, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 6.95];
        const OFF: [f64; 3] = [-0.95, 0.0, 0.95];
        let mut last = Cell { f: 255, u: -1, v: -1, r: -1 };
        for &hy in &HS {
            for &ox in &OFF {
                for &oz in &OFF {
                    let q = d_add(p, d_add(d_mul(up, hy), d_add(d_mul(right, ox), d_mul(fwd, oz))));
                    let c = cell_at(q);
                    if c == last {
                        continue;
                    }
                    last = c;
                    if solid(w.block(&c)) {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn collides(&self, w: &World, p: D3) -> bool {
        self.collides_at(w, p, tangent_basis(p, self.heading))
    }

    /// Bitta yo'nalish (birlik vektor) bo'yicha siljish: kichik qadamlar bilan. `true` — to'siqqa tegdi.
    fn move_axis(&mut self, w: &World, axis: D3, basis: (D3, D3, D3), delta: f64, can_step: bool) -> bool {
        if delta == 0.0 {
            return false;
        }
        let up = basis.1;
        let steps = (delta.abs() / 0.5).ceil().max(1.0) as usize;
        let d = delta / steps as f64;
        for _ in 0..steps {
            let np = d_add(self.pos, d_mul(axis, d));
            if !self.collides_at(w, np, basis) {
                self.pos = np;
                continue;
            }
            if can_step {
                let mut lift = 1.0;
                while lift <= STEP_UP + 1e-9 {
                    let t = d_add(np, d_mul(up, lift));
                    if !self.collides_at(w, t, basis) {
                        self.pos = t;
                        break;
                    }
                    lift += 1.0;
                }
                if lift <= STEP_UP + 1e-9 {
                    continue;
                }
            }
            // To'qnashuv nuqtasigacha aniq yaqinlashamiz
            let (mut lo, mut hi) = (0.0f64, d);
            for _ in 0..10 {
                let mid = (lo + hi) * 0.5;
                let t = d_add(self.pos, d_mul(axis, mid));
                if self.collides_at(w, t, basis) {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            self.pos = d_add(self.pos, d_mul(axis, lo));
            return true;
        }
        false
    }

    pub fn update(&mut self, w: &World, inp: &Input, dt: f32) {
        let dt = (dt.min(0.05)) as f64;
        // Parallel ko'chirish: oldinga yo'nalish doim tegma tekislikda
        let basis = tangent_basis(self.pos, self.heading);
        self.heading = basis.2;
        let (right, up, fwd) = basis;
        let (sx, sy) = (inp.mv[0] as f64, inp.mv[1] as f64);
        self.agl = d_len(self.pos) - Self::ground_radius(w, self.pos);
        let head = d_add(self.pos, up);
        self.in_water = w.block(&cell_at(head)) == WATER;

        if self.flying {
            let sp = FLY_BASE * FLY_MULT[self.fly_level.min(FLY_MULT.len() - 1)] * self.fly_scale();
            let mut v = d_add(d_mul(fwd, sy * sp), d_mul(right, sx * sp));
            if inp.jump {
                v = d_add(v, d_mul(up, sp * 0.8));
            } else if inp.down {
                v = d_sub(v, d_mul(up, sp * 0.8));
            }
            self.vel = v;
            if self.ghost() {
                // Sayyora egriligi bo'ylab: balandlik faqat tepa/past tugmalari bilan o'zgaradi
                let r_target = d_len(self.pos) + d_dot(v, up) * dt;
                let moved = d_add(self.pos, d_mul(v, dt));
                self.pos = d_mul(d_norm(moved), r_target);
                // Yerga kirib bo'lmaydi: relyef ustida qoladi (tog'larni bosib o'tadi)
                let floor = Self::ground_radius(w, self.pos) + 0.5;
                if d_len(self.pos) < floor {
                    self.pos = d_mul(d_norm(self.pos), floor);
                }
            } else {
                self.move_axis(w, right, basis, d_dot(v, right) * dt, false);
                self.move_axis(w, fwd, basis, d_dot(v, fwd) * dt, false);
                self.move_axis(w, up, basis, d_dot(v, up) * dt, false);
            }
            self.on_ground = false;
            self.running = false;
            self.bob_amp *= 0.9;
            self.land_dip *= 0.9;
            return;
        }

        let mag = (sx * sx + sy * sy).sqrt();
        self.running = mag > 0.92 && self.on_ground && !self.in_water;
        let speed = if self.in_water {
            WALK * 0.55
        } else if self.running {
            WALK * RUN
        } else {
            WALK
        };
        let (mut vr, mut vf, mut vu) = (d_dot(self.vel, right), d_dot(self.vel, fwd), d_dot(self.vel, up));
        // Qor ustida sirpanchiq; daryoda oqim olib ketadi
        let under = w.block(&cell_at(d_sub(self.pos, d_mul(up, 0.2))));
        let k = if self.in_water {
            14.0
        } else if self.on_ground && under == crate::blocks::SNOW {
            2.5
        } else if self.on_ground {
            14.0
        } else {
            3.5
        };
        let a = (k * dt).min(1.0);
        let flow = if self.in_water { w.gen.river_flow(up).map_or([0.0; 3], |f| d_mul(f, 9.0)) } else { [0.0; 3] };
        vr += (sx * speed + d_dot(flow, right) - vr) * a;
        vf += (sy * speed + d_dot(flow, fwd) - vf) * a;
        let vu_before = vu;
        if self.in_water {
            vu -= GRAVITY * 0.8 * dt;
            vu *= 1.0 - (2.5 * dt).min(0.5);
            if inp.jump {
                vu = 22.0;
            } else if inp.down {
                vu = -22.0;
            }
        } else {
            vu = (vu - GRAVITY * dt).max(-220.0);
            if inp.jump && self.on_ground {
                vu = JUMP_V;
            }
        }
        let was_ground = self.on_ground;
        self.on_ground = false;
        if self.move_axis(w, up, basis, vu * dt, false) {
            if vu < 0.0 {
                self.on_ground = true;
                if !was_ground && vu_before < -9.0 {
                    self.land_dip = (self.land_dip + (-vu_before) * 0.012).min(1.2);
                }
            }
            vu = 0.0;
        }
        let can_step = self.on_ground || self.in_water;
        if self.move_axis(w, right, basis, vr * dt, can_step) {
            vr = 0.0;
        }
        if self.move_axis(w, fwd, basis, vf * dt, can_step) {
            vf = 0.0;
        }
        self.vel = d_add(d_add(d_mul(right, vr), d_mul(fwd, vf)), d_mul(up, vu));
        if !self.on_ground && vu <= 0.0 {
            let t = d_sub(self.pos, d_mul(up, 0.05));
            if self.collides_at(w, t, basis) {
                self.on_ground = true;
            }
        }
        // Kamera tebranishi: yurish tezligiga mutanosib; havoda va suvda so'nadi
        let vh = (vr * vr + vf * vf).sqrt();
        let target = if self.on_ground { (vh / WALK).min(1.6) } else { 0.0 };
        self.bob_amp += (target - self.bob_amp) * (8.0 * dt).min(1.0);
        self.bob_phase += vh * dt * 0.17 * std::f64::consts::TAU * if self.on_ground { 1.0 } else { 0.0 };
        self.land_dip *= (1.0 - 6.0 * dt).max(0.0);
        // Ko'p yurgandan keyin asos yangilanadi (egrilik)
        self.heading = tangent_basis(self.pos, self.heading).2;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::STONE;
    use std::collections::HashSet;

    const HIGH: i64 = SEA_R + 9000;

    /// Havodagi tosh maydoncha (platforma): tabiiy relyefga bog'liq emas. Qaytaradi: (dunyo, tik turgan joy).
    fn world_with_floor() -> (World, D3) {
        let w = World::new(1);
        let (f, u0, v0) = (2u8, 30_000_000i64, 30_000_000i64);
        for du in -60..60 {
            for dv in -60..60 {
                w.set_block(&Cell { f, u: u0 + du, v: v0 + dv, r: HIGH - 1 }, STONE);
            }
        }
        (w, d_mul(dir(f, u0 as f64 + 0.5, v0 as f64 + 0.5), HIGH as f64))
    }

    fn run(p: &mut Player, w: &World, inp: &Input, secs: f32) {
        for _ in 0..(secs / 0.016) as usize {
            p.update(w, inp, 0.016);
        }
    }

    fn fill_line(w: &World, p: &Player, ahead: f64, thick: f64, height: i64) {
        let (right, up, fwd) = tangent_basis(p.pos, p.heading);
        let mut set = HashSet::new();
        let mut a = 0.0;
        while a < thick {
            for k in -30..=30 {
                for hy in 0..height {
                    let q = d_add(p.pos, d_add(d_mul(fwd, ahead + a), d_add(d_mul(right, k as f64 * 0.5), d_mul(up, 0.5 + hy as f64))));
                    set.insert(cell_at(q));
                }
            }
            a += 0.5;
        }
        for c in set {
            w.set_block(&c, STONE);
        }
    }

    #[test]
    fn tushadi_va_polga_qo_nadi() {
        let (w, floor) = world_with_floor();
        let start = d_add(floor, d_mul(d_norm(floor), 20.0));
        let mut p = Player::new(start);
        run(&mut p, &w, &Input::default(), 1.5);
        assert!(p.on_ground);
        assert!((p.altitude() + SEA_R as f64 - HIGH as f64).abs() < 0.1, "balandlik {}", p.pos[0]);
    }

    #[test]
    fn sakrash_balandligi_besh_blok() {
        let (w, floor) = world_with_floor();
        let mut p = Player::new(floor);
        run(&mut p, &w, &Input::default(), 0.3);
        assert!(p.on_ground);
        let base = d_len(p.pos);
        let inp = Input { jump: true, ..Default::default() };
        let mut maxr = base;
        for _ in 0..80 {
            p.update(&w, &inp, 0.016);
            maxr = maxr.max(d_len(p.pos));
        }
        let h = maxr - base;
        assert!(h > 2.0 && h < 3.4, "sakrash {h} blok");
    }

    #[test]
    fn yurish_tezligi_va_devor() {
        let (w, floor) = world_with_floor();
        let mut p = Player::new(floor);
        run(&mut p, &w, &Input::default(), 0.3);
        let start = p.pos;
        run(&mut p, &w, &Input { mv: [0.0, 0.91], ..Default::default() }, 1.0);
        let d = d_len(d_sub(p.pos, start));
        assert!(d > 13.0 && d < 19.0, "yurdi {d}");
        // 5 blok balandlikdagi devor (o'tib bo'lmaydi), 14 blok oldinda
        let before = p.pos;
        fill_line(&w, &p, 14.0, 1.0, 5);
        run(&mut p, &w, &Input { mv: [0.0, 0.91], ..Default::default() }, 2.0);
        let walked = d_len(d_sub(p.pos, before));
        assert!(walked > 10.0 && walked < 14.6, "devordan o'tib ketdi yoki yurmadi: {walked}");
    }

    #[test]
    fn ikki_blokli_pogonaga_chiqadi() {
        let (w, floor) = world_with_floor();
        let mut p = Player::new(floor);
        run(&mut p, &w, &Input::default(), 0.3);
        let r0 = d_len(p.pos);
        fill_line(&w, &p, 6.0, 20.0, 2);
        run(&mut p, &w, &Input { mv: [0.0, 0.91], ..Default::default() }, 1.0);
        assert!(d_len(p.pos) > r0 + 1.5, "pog'onaga chiqmadi: {} -> {}", r0, d_len(p.pos));
    }

    #[test]
    fn uchish_va_ghost() {
        let (w, floor) = world_with_floor();
        let mut p = Player::new(d_add(floor, d_mul(d_norm(floor), 10.0)));
        p.flying = true;
        let r0 = d_len(p.pos);
        run(&mut p, &w, &Input { jump: true, ..Default::default() }, 1.0);
        assert!(d_len(p.pos) > r0 + 30.0);
        p.fly_level = 2;
        assert!(p.ghost());
        let s = p.pos;
        run(&mut p, &w, &Input { mv: [0.0, 0.91], ..Default::default() }, 1.0);
        assert!(d_len(d_sub(p.pos, s)) > 300.0, "tez uchish {}", d_len(d_sub(p.pos, s)));
    }

    #[test]
    fn suvda_botmaydi_va_suzadi() {
        let w = World::new(1);
        let (f, u0, v0) = (3u8, 25_000_000i64, 25_000_000i64);
        let mut set = HashSet::new();
        for du in -10..10 {
            for dv in -10..10 {
                for dr in 0..30 {
                    set.insert(Cell { f, u: u0 + du, v: v0 + dv, r: HIGH + dr });
                }
                w.set_block(&Cell { f, u: u0 + du, v: v0 + dv, r: HIGH - 1 }, STONE);
            }
        }
        for c in set {
            w.set_block(&c, WATER);
        }
        let top = d_mul(dir(f, u0 as f64 + 0.5, v0 as f64 + 0.5), (HIGH + 20) as f64);
        let mut p = Player::new(top);
        run(&mut p, &w, &Input::default(), 3.0);
        assert!(p.altitude() + (SEA_R as f64) < HIGH as f64 + 2.0, "tubiga cho'kmadi");
        run(&mut p, &w, &Input { jump: true, ..Default::default() }, 1.5);
        assert!(d_len(p.pos) > HIGH as f64 + 10.0, "suzib chiqmadi {}", d_len(p.pos) - HIGH as f64);
    }

    #[test]
    fn qarash_va_asos_ortogonal() {
        let mut p = Player::new(d_mul(dir(0, 1.0e7, 2.0e7), SEA_R as f64 + 30.0));
        p.look(0.0, -100000.0, 0.01);
        assert!(p.pitch <= 1.55);
        p.look(0.0, 100000.0, 0.01);
        assert!(p.pitch >= -1.55);
        p.look(120.0, 0.0, 0.03);
        let (r, u, f) = p.camera_basis();
        assert!(d_dot(r, u).abs() < 1e-9 && d_dot(r, f).abs() < 1e-9 && d_dot(u, f).abs() < 1e-9);
        assert!((d_len(r) - 1.0).abs() < 1e-9 && (d_len(u) - 1.0).abs() < 1e-9 && (d_len(f) - 1.0).abs() < 1e-9);
        // chapga burilganda yo'nalish o'zgaradi
        let h0 = p.heading;
        p.turn(0.5);
        assert!(d_dot(h0, p.heading) < 0.9);
        assert!(d_dot(p.heading, p.up()).abs() < 1e-9);
    }

    #[test]
    fn sayyorani_aylanib_chiqish_yo_nalishni_saqlaydi() {
        // Ghost rejimida uzoq masofa bosib o'tilgach tepa vektor markazdan chiqadi va asos ortogonal qoladi
        let w = World::new(1);
        let mut p = Player::new(d_mul(dir(0, 3.0e7, 3.0e7), SEA_R as f64 + 60_000.0));
        p.flying = true;
        p.fly_level = 7;
        let start = p.pos;
        for _ in 0..4000 {
            p.update(&w, &Input { mv: [0.0, 0.91], ..Default::default() }, 0.016);
        }
        assert!(d_len(d_sub(p.pos, start)) > 1.0e6, "uzoqqa bormadi");
        assert!((p.altitude() - 60_000.0).abs() < 3000.0, "balandlik yo'qoldi: {}", p.altitude());
        let (r, u, f) = p.camera_basis();
        assert!(d_dot(r, u).abs() < 1e-6 && d_dot(u, f).abs() < 1e-6);
    }

    #[test]
    fn tez_uchishda_yerga_kirmaydi() {
        // Toshkent atrofida pastdan, ghost rejimida pastga uchamiz: relyefdan past tushmasligi kerak
        let w = World::new(1);
        let d = crate::earth::latlon_dir(41.3, 69.28);
        let ground = SEA_R as f64 + w.gen.terrain(d).h as f64;
        let mut p = Player::new(d_mul(d, ground + 30.0));
        p.flying = true;
        p.fly_level = 3;
        for _ in 0..120 {
            p.update(&w, &Input { down: true, mv: [0.0, 0.9], ..Default::default() }, 0.016);
            let g = SEA_R as f64 + w.gen.terrain(d_norm(p.pos)).h.max(w.gen.terrain(d_norm(p.pos)).wl) as f64;
            assert!(d_len(p.pos) >= g + 0.4, "yerga kirdi: {} < {}", d_len(p.pos), g);
        }
    }
}
