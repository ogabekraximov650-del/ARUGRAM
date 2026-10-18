//! Sensorli boshqaruv: chap yarim ekran — virtual joy (yurish), o'ng yarim ekran — qarash.

use crate::camera::Camera;
use glam::Vec2;

/// Joy to'liq og'ishining piksel radiusi (zichlikka moslanadi: `density` bilan ko'paytiriladi).
const STICK_RADIUS_DP: f32 = 56.0;
/// Qarash sezgirligi: radian / piksel.
const LOOK_SENS: f32 = 0.0045;
const MOVE_SPEED: f32 = 6.0;
/// Xarita yarim kengligi: shundan tashqariga chiqib bo'lmaydi.
pub use crate::terrain::WORLD_HALF;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TouchAction {
    Down,
    Move,
    Up,
}

/// Tugmalar joylashuvi: o'ng-pastki burchakdan dp masofa (zichlikka bog'liq emas) + umumiy o'lcham.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub fire: Vec2,
    pub jump: Vec2,
    pub switch: Vec2,
    pub scale: f32,
    /// Joystik markazi: chap-pastki burchakdan dp.
    pub stick: Vec2,
    /// Kichik xarita markazi: o'ng-yuqori burchakdan dp.
    pub map: Vec2,
}

impl Layout {
    pub const DEFAULT: Layout = Layout {
        fire: Vec2::new(100.0, 100.0),
        jump: Vec2::new(190.0, 60.0),
        switch: Vec2::new(100.0, 190.0),
        scale: 1.0,
        stick: Vec2::new(120.0, 110.0),
        map: Vec2::new(76.0, 76.0),
    };

    pub fn to_array(self) -> [f32; 11] {
        [
            self.fire.x, self.fire.y, self.jump.x, self.jump.y, self.switch.x, self.switch.y, self.scale,
            self.stick.x, self.stick.y, self.map.x, self.map.y,
        ]
    }

    /// Saqlangan qiymatlardan tiklaydi. 7 ta son (eski format) — joystik/xarita standart;
    /// noto'g'ri (NaN, chegaradan tashqari) bo'lsa — `None`.
    pub fn from_slice(a: &[f32]) -> Option<Layout> {
        if a.len() < 7 || a.iter().any(|x| !x.is_finite()) {
            return None;
        }
        if a[..6].iter().any(|x| *x < 0.0 || *x > 2000.0) || !(0.5..=2.0).contains(&a[6]) {
            return None;
        }
        let mut l = Layout {
            fire: Vec2::new(a[0], a[1]),
            jump: Vec2::new(a[2], a[3]),
            switch: Vec2::new(a[4], a[5]),
            scale: a[6],
            ..Layout::DEFAULT
        };
        if a.len() >= 11 {
            if a[7..11].iter().any(|x| *x < 0.0 || *x > 2000.0) {
                return None;
            }
            l.stick = Vec2::new(a[7], a[8]);
            l.map = Vec2::new(a[9], a[10]);
        }
        Some(l)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SettingsAction {
    Reset,
    Smaller,
    Bigger,
    Done,
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
    switch_id: Option<i32>,
    switch_edge: bool,
    layout: Layout,
    layout_dirty: bool,
    settings: bool,
    drag: Option<(i32, usize)>,
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
            switch_id: None,
            switch_edge: false,
            layout: Layout::DEFAULT,
            layout_dirty: false,
            settings: false,
            drag: None,
            look_delta: Vec2::ZERO,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width as f32;
        self.height = height as f32;
    }

    fn corner(&self, off: Vec2) -> Vec2 {
        Vec2::new(self.width - off.x * self.density, self.height - off.y * self.density)
    }

    /// Otish tugmasi: (markaz, radius) piksellarda.
    pub fn fire_button(&self) -> (Vec2, f32) {
        (self.corner(self.layout.fire), 44.0 * self.density * self.layout.scale)
    }

    /// Sakrash tugmasi: (markaz, radius) piksellarda.
    pub fn jump_button(&self) -> (Vec2, f32) {
        (self.corner(self.layout.jump), 32.0 * self.density * self.layout.scale)
    }

    /// Qurol almashtirish tugmasi: (markaz, radius) piksellarda.
    pub fn switch_button(&self) -> (Vec2, f32) {
        (self.corner(self.layout.switch), 30.0 * self.density * self.layout.scale)
    }

    /// Joystik: (markaz, radius) piksellarda.
    pub fn stick_home(&self) -> (Vec2, f32) {
        let d = self.density;
        (
            Vec2::new(self.layout.stick.x * d, self.height - self.layout.stick.y * d),
            STICK_RADIUS_DP * d,
        )
    }

    /// Kichik xarita: (chap-yuqori burchak, tomon) piksellarda.
    pub fn map_rect(&self) -> (Vec2, f32) {
        let d = self.density;
        let size = 120.0 * d;
        let c = Vec2::new(self.width - self.layout.map.x * d, self.layout.map.y * d);
        (c - Vec2::splat(size * 0.5), size)
    }

    /// Sozlamalar (tishli g'ildirak) tugmasi — kichik xaritaning chap tomonida.
    pub fn gear_button(&self) -> (Vec2, f32) {
        let d = self.density;
        let (tl, _) = self.map_rect();
        (Vec2::new(tl.x - 34.0 * d, tl.y + 20.0 * d), 20.0 * d)
    }

    /// Sozlamalar rejimidagi boshqaruv tugmalari (tepa o'rtada).
    pub fn settings_controls(&self) -> [(SettingsAction, Vec2, f32); 4] {
        let d = self.density;
        let (cx, y, r) = (self.width * 0.5, 44.0 * d, 26.0 * d);
        [
            (SettingsAction::Reset, Vec2::new(cx - 120.0 * d, y), r),
            (SettingsAction::Smaller, Vec2::new(cx - 40.0 * d, y), r),
            (SettingsAction::Bigger, Vec2::new(cx + 40.0 * d, y), r),
            (SettingsAction::Done, Vec2::new(cx + 120.0 * d, y), r),
        ]
    }

    pub fn settings_open(&self) -> bool {
        self.settings
    }

    pub fn layout(&self) -> Layout {
        self.layout
    }

    pub fn set_layout(&mut self, l: Layout) {
        self.layout = l;
    }

    /// O'zgargan bo'lsa saqlash uchun joylashuvni qaytaradi (bir marta).
    pub fn take_layout(&mut self) -> Option<[f32; 11]> {
        if std::mem::take(&mut self.layout_dirty) {
            Some(self.layout.to_array())
        } else {
            None
        }
    }

    fn open_settings(&mut self) {
        self.settings = true;
        self.stick = None;
        self.look = None;
        self.fire_id = None;
        self.jump_id = None;
        self.switch_id = None;
        self.jump_edge = false;
        self.switch_edge = false;
        self.look_delta = Vec2::ZERO;
    }

    fn settings_event(&mut self, action: TouchAction, id: i32, p: Vec2) {
        let d = self.density;
        match action {
            TouchAction::Down => {
                for (act, c, r) in self.settings_controls() {
                    if p.distance(c) <= r * 1.2 {
                        match act {
                            SettingsAction::Reset => self.layout = Layout::DEFAULT,
                            SettingsAction::Smaller => self.layout.scale = (self.layout.scale - 0.1).max(0.7),
                            SettingsAction::Bigger => self.layout.scale = (self.layout.scale + 0.1).min(1.6),
                            SettingsAction::Done => self.settings = false,
                        }
                        self.layout_dirty = true;
                        return;
                    }
                }
                let btns = [self.fire_button(), self.jump_button(), self.switch_button(), self.stick_home()];
                for (i, (c, r)) in btns.iter().enumerate() {
                    if p.distance(*c) <= *r * 1.3 {
                        self.drag = Some((id, i));
                        return;
                    }
                }
                let (tl, size) = self.map_rect();
                if p.x >= tl.x && p.x <= tl.x + size && p.y >= tl.y && p.y <= tl.y + size {
                    self.drag = Some((id, 4));
                }
            }
            TouchAction::Move => {
                if let Some((did, i)) = self.drag {
                    if did == id {
                        let off = Vec2::new(
                            ((self.width - p.x) / d).clamp(30.0, (self.width / d - 30.0).max(30.0)),
                            ((self.height - p.y) / d).clamp(30.0, (self.height / d - 30.0).max(30.0)),
                        );
                        let maxx = (self.width / d - 30.0).max(30.0);
                        let maxy = (self.height / d - 30.0).max(30.0);
                        match i {
                            0 => self.layout.fire = off,
                            1 => self.layout.jump = off,
                            2 => self.layout.switch = off,
                            3 => {
                                // joystik: chap-pastki burchakdan
                                self.layout.stick = Vec2::new(
                                    (p.x / d).clamp(60.0, maxx),
                                    ((self.height - p.y) / d).clamp(60.0, maxy),
                                )
                            }
                            _ => {
                                self.layout.map = Vec2::new(
                                    ((self.width - p.x) / d).clamp(60.0, maxx),
                                    (p.y / d).clamp(60.0, maxy),
                                )
                            }
                        }
                        self.layout_dirty = true;
                    }
                }
            }
            TouchAction::Up => {
                if self.drag.map_or(false, |(did, _)| did == id) {
                    self.drag = None;
                }
            }
        }
    }

    /// Sakrash bosilgan bo'lsa bir marta `true` qaytaradi.
    pub fn take_jump(&mut self) -> bool {
        std::mem::take(&mut self.jump_edge)
    }

    /// Almashtirish bosilgan bo'lsa bir marta `true` qaytaradi.
    pub fn take_switch(&mut self) -> bool {
        std::mem::take(&mut self.switch_edge)
    }

    pub fn fire_held(&self) -> bool {
        self.fire_id.is_some()
    }

    pub fn touch(&mut self, action: TouchAction, id: i32, x: f32, y: f32) {
        let p = Vec2::new(x, y);
        if self.settings {
            self.settings_event(action, id, p);
            return;
        }
        match action {
            TouchAction::Down => {
                let (gc, gr) = self.gear_button();
                if p.distance(gc) <= gr * 1.4 {
                    self.open_settings();
                    return;
                }
                let (home, hr) = self.stick_home();
                if p.x < self.width * 0.5 || p.distance(home) <= hr * 2.0 {
                    if self.stick.is_none() {
                        // joystik doirasi ichida bosilsa — joystik joyida (qo'zg'almas), tashqarida — bosilgan joyda
                        let origin = if p.distance(home) <= hr * 2.0 { home } else { p };
                        self.stick = Some(Stick { id, origin, pos: p });
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
                    } else if p.distance(self.switch_button().0) <= self.switch_button().1 * 1.4 {
                        if self.switch_id.is_none() {
                            self.switch_id = Some(id);
                            self.switch_edge = true;
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
                if self.switch_id == Some(id) {
                    self.switch_id = None;
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
        if self.settings {
            return;
        }
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
    fn sozlamalarda_tugma_suriladi_va_saqlanadi() {
        let mut inp = Input::new(1280, 576, 2.0);
        let (g, _) = inp.gear_button();
        inp.touch(TouchAction::Down, 1, g.x, g.y);
        inp.touch(TouchAction::Up, 1, g.x, g.y);
        assert!(inp.settings_open());
        let (f, _) = inp.fire_button();
        inp.touch(TouchAction::Down, 2, f.x, f.y);
        inp.touch(TouchAction::Move, 2, 600.0, 300.0);
        inp.touch(TouchAction::Up, 2, 600.0, 300.0);
        let (nf, _) = inp.fire_button();
        assert!((nf - Vec2::new(600.0, 300.0)).length() < 1.0);
        assert!(inp.take_layout().is_some());
        assert!(inp.take_layout().is_none());
        // tugma bosilmaydi, sozlamalarda
        assert!(!inp.fire_held());
        let (c, _) = inp.settings_controls().iter().find(|x| x.0 == SettingsAction::Done).map(|x| (x.1, x.2)).unwrap();
        inp.touch(TouchAction::Down, 3, c.x, c.y);
        assert!(!inp.settings_open());
    }

    #[test]
    fn layout_noto_g_ri_qiymatni_rad_etadi() {
        assert!(Layout::from_slice(&[f32::NAN; 7]).is_none());
        assert!(Layout::from_slice(&[1.0; 3]).is_none());
        assert_eq!(Layout::from_slice(&Layout::DEFAULT.to_array()), Some(Layout::DEFAULT));
    }

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
