//! Sensorli boshqaruv: chap yarim ekran — virtual joy (yurish), o'ng yarim ekran — qarash.

use crate::camera::Camera;
use glam::Vec2;

/// Joy to'liq og'ishining piksel radiusi (zichlikka moslanadi: `density` bilan ko'paytiriladi).
const STICK_RADIUS_DP: f32 = 56.0;
/// Qarash sezgirligi: radian / piksel.
const LOOK_SENS: f32 = 0.0045;
const MOVE_SPEED: f32 = 6.0;
/// Xarita yarim kengligi: shundan tashqariga chiqib bo'lmaydi.
pub const WORLD_HALF: f32 = 46.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TouchAction {
    Down,
    Move,
    Up,
}

#[derive(Clone, Copy)]
struct Stick {
    id: i32,
    origin: Vec2,
    pos: Vec2,
}

#[derive(Clone, Copy)]
struct Look {
    id: i32,
    last: Vec2,
}

pub struct Input {
    width: f32,
    density: f32,
    stick: Option<Stick>,
    look: Option<Look>,
    /// Tugma ushlab turgan barmoq id'lari.
    fire_id: Option<i32>,
    jump_id: Option<i32>,
    jump_edge: bool,
    height: f32,
    /// Shu kadrda to'plangan qarash o'zgarishi (piksel).
    look_delta: Vec2,
}

impl Input {
    pub fn new(width: u32, height: u32, density: f32) -> Self {
        Self {
            width: width as f32,
            height: height as f32,
            density: density.max(0.5),
            stick: None,
            look: None,
            fire_id: None,
            jump_id: None,
            jump_edge: false,
            look_delta: Vec2::ZERO,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width as f32;
        self.height = height as f32;
    }

    /// Otish tugmasi: (markaz, radius) piksellarda.
    pub fn fire_button(&self) -> (Vec2, f32) {
        let d = self.density;
        (Vec2::new(self.width - 100.0 * d, self.height - 100.0 * d), 44.0 * d)
    }

    /// Sakrash tugmasi: (markaz, radius) piksellarda.
    pub fn jump_button(&self) -> (Vec2, f32) {
        let d = self.density;
        (Vec2::new(self.width - 190.0 * d, self.height - 60.0 * d), 32.0 * d)
    }

    pub fn fire_held(&self) -> bool {
        self.fire_id.is_some()
    }

    /// Sakrash bosilgan bo'lsa bir marta `true` qaytaradi.
    pub fn take_jump(&mut self) -> bool {
        std::mem::take(&mut self.jump_edge)
    }

    pub fn touch(&mut self, action: TouchAction, id: i32, x: f32, y: f32) {
        let p = Vec2::new(x, y);
        match action {
            TouchAction::Down => {
                if p.x < self.width * 0.5 {
                    if self.stick.is_none() {
                        self.stick = Some(Stick { id, origin: p, pos: p });
                    }
                } else {
                    let (fc, fr) = self.fire_button();
                    let (jc, jr) = self.jump_button();
                    if p.distance(fc) <= fr * 1.3 {
                        self.fire_id.get_or_insert(id);
                    } else if p.distance(jc) <= jr * 1.4 {
                        if self.jump_id.is_none() {
                            self.jump_id = Some(id);
                            self.jump_edge = true;
                        }
                    } else if self.look.is_none() {
                        self.look = Some(Look { id, last: p });
                    }
                }
            }
            TouchAction::Move => {
                if let Some(s) = self.stick.as_mut() {
                    if s.id == id {
                        s.pos = p;
                    }
                }
                if let Some(l) = self.look.as_mut() {
                    if l.id == id {
                        self.look_delta += p - l.last;
                        l.last = p;
                    }
                }
            }
            TouchAction::Up => {
                if self.stick.map_or(false, |s| s.id == id) {
                    self.stick = None;
                }
                if self.look.map_or(false, |l| l.id == id) {
                    self.look = None;
                }
                if self.fire_id == Some(id) {
                    self.fire_id = None;
                }
                if self.jump_id == Some(id) {
                    self.jump_id = None;
                }
            }
        }
    }

    /// Joy vektori: uzunligi 0..1; x — o'ngga, y — oldinga (ekranda yuqoriga surilsa musbat).
    pub fn move_vector(&self) -> Vec2 {
        let Some(s) = self.stick else { return Vec2::ZERO };
        let r = STICK_RADIUS_DP * self.density;
        let d = (s.pos - s.origin) / r;
        let d = if d.length() > 1.0 { d.normalize() } else { d };
        // Ekran y o'qi pastga qaragan — yuqoriga surish "oldinga".
        Vec2::new(d.x, -d.y)
    }

    /// Chizish uchun: (joy markazi, joy tutqichi, radius) piksellarda.
    pub fn stick_visual(&self) -> Option<(Vec2, Vec2, f32)> {
        let s = self.stick?;
        let r = STICK_RADIUS_DP * self.density;
        let off = s.pos - s.origin;
        let off = if off.length() > r { off.normalize() * r } else { off };
        Some((s.origin, s.origin + off, r))
    }

    pub fn update(&mut self, dt: f32, cam: &mut Camera) {
        let d = std::mem::take(&mut self.look_delta);
        // O'ngga surish — o'ngga burilish (yaw kamayadi), yuqoriga surish — tepaga qarash.
        cam.look(-d.x * LOOK_SENS, -d.y * LOOK_SENS);
        let m = self.move_vector();
        if m != Vec2::ZERO {
            let dir = cam.flat_forward() * m.y + cam.right() * m.x;
            cam.pos += dir * MOVE_SPEED * dt.clamp(0.0, 0.1);
            cam.pos.x = cam.pos.x.clamp(-WORLD_HALF, WORLD_HALF);
            cam.pos.z = cam.pos.z.clamp(-WORLD_HALF, WORLD_HALF);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn joy_oldinga_yuradi() {
        let mut inp = Input::new(1000, 500, 1.0);
        let mut cam = Camera::new(Vec3::ZERO);
        inp.touch(TouchAction::Down, 1, 200.0, 500.0);
        inp.touch(TouchAction::Move, 1, 200.0, 400.0); // yuqoriga surildi
        inp.update(0.05, &mut cam);
        assert!(cam.pos.z < 0.0, "oldinga = -Z");
    }

    #[test]
    fn ong_tomon_qaraydi() {
        let mut inp = Input::new(1000, 500, 1.0);
        let mut cam = Camera::new(Vec3::ZERO);
        inp.touch(TouchAction::Down, 2, 800.0, 300.0);
        inp.touch(TouchAction::Move, 2, 900.0, 300.0); // o'ngga
        inp.update(0.016, &mut cam);
        assert!(cam.yaw < 0.0, "o'ngga surish yaw ni kamaytiradi");
    }

    #[test]
    fn chiqib_ketmaydi() {
        let mut inp = Input::new(1000, 500, 1.0);
        let mut cam = Camera::new(Vec3::new(WORLD_HALF, 0.0, 0.0));
        cam.yaw = -std::f32::consts::FRAC_PI_2; // +X ga qaraydi
        inp.touch(TouchAction::Down, 1, 100.0, 500.0);
        inp.touch(TouchAction::Move, 1, 100.0, 300.0);
        for _ in 0..100 {
            inp.update(0.05, &mut cam);
        }
        assert!(cam.pos.x <= WORLD_HALF + 1e-4);
    }

    #[test]
    fn barmoq_ko_tarilsa_joy_tozalanadi() {
        let mut inp = Input::new(1000, 500, 1.0);
        inp.touch(TouchAction::Down, 1, 100.0, 500.0);
        inp.touch(TouchAction::Up, 1, 100.0, 500.0);
        assert_eq!(inp.move_vector(), Vec2::ZERO);
        assert!(inp.stick_visual().is_none());
    }
}
