//! Omon qolish ko'rsatkichlari: hayot, ochlik, havo (suv ostida), zarar va o'lim. Faqat hisob-kitob (GL siz), testlanadi.

/// Eng ko'p hayot: 20 birlik = 10 ta yurak (1 yurak = 2 birlik).
pub const MAX_HP: f32 = 20.0;
pub const MAX_FOOD: f32 = 20.0;
/// Suv ostida nafas olmay turish vaqti (soniya).
pub const MAX_AIR: f32 = 10.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Vitals {
    pub health: f32,
    pub hunger: f32,
    pub air: f32,
    exhaustion: f32,
    regen_t: f32,
    starve_t: f32,
    drown_t: f32,
    /// Zarbadan keyingi qizil chaqnash [0,1].
    pub flash: f32,
    pub dead: bool,
}

impl Default for Vitals {
    fn default() -> Self {
        Vitals::new()
    }
}

impl Vitals {
    pub fn new() -> Vitals {
        Vitals { health: MAX_HP, hunger: MAX_FOOD, air: MAX_AIR, exhaustion: 0.0, regen_t: 0.0, starve_t: 0.0, drown_t: 0.0, flash: 0.0, dead: false }
    }

    pub fn respawn(&mut self) {
        *self = Vitals::new();
    }

    pub fn damage(&mut self, hp: f32) {
        if self.dead || hp <= 0.0 {
            return;
        }
        self.health -= hp;
        self.flash = 1.0;
        if self.health <= 0.0 {
            self.health = 0.0;
            self.dead = true;
        }
    }

    /// Balanddan tushish: 3 metrdan keyin har bir metr uchun yarim yurak (1 birlik). Qaytaradi: yetkazilgan zarar.
    pub fn fall(&mut self, meters: f32) -> f32 {
        let d = (meters - 3.0).floor().max(0.0);
        if d > 0.0 {
            self.damage(d);
        }
        d
    }

    /// Charchash (Minecraft birligida: 4 birlik = 1 ochlik nuqtasi).
    pub fn exhaust(&mut self, amount: f32) {
        self.exhaustion += amount.max(0.0);
    }

    pub fn eat(&mut self, food: f32) {
        self.hunger = (self.hunger + food).min(MAX_FOOD);
    }

    pub fn tick(&mut self, dt: f32, head_in_water: bool) {
        if self.dead {
            return;
        }
        self.flash = (self.flash - dt * 2.0).max(0.0);
        if head_in_water {
            self.air = (self.air - dt).max(0.0);
            if self.air <= 0.0 {
                self.drown_t += dt;
                while self.drown_t >= 1.0 && !self.dead {
                    self.drown_t -= 1.0;
                    self.damage(2.0);
                }
            }
        } else {
            self.air = (self.air + dt * 4.0).min(MAX_AIR);
            self.drown_t = 0.0;
        }
        while self.exhaustion >= 4.0 {
            self.exhaustion -= 4.0;
            self.hunger = (self.hunger - 1.0).max(0.0);
        }
        if self.hunger >= 18.0 && self.health < MAX_HP {
            self.regen_t += dt;
            if self.regen_t >= 4.0 {
                self.regen_t = 0.0;
                self.health = (self.health + 1.0).min(MAX_HP);
                self.exhaust(0.6);
            }
        } else if self.hunger <= 0.0 {
            self.starve_t += dt;
            if self.starve_t >= 4.0 {
                self.starve_t = 0.0;
                // Ochlikdan faqat yarim yurakkacha tushadi (o'lmaydi)
                if self.health > 1.0 {
                    self.damage(1.0);
                }
            }
        } else {
            self.regen_t = 0.0;
            self.starve_t = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tushish_zarari() {
        let mut v = Vitals::new();
        assert_eq!(v.fall(3.0), 0.0);
        assert_eq!(v.health, MAX_HP);
        assert_eq!(v.fall(5.5), 2.0);
        assert_eq!(v.health, MAX_HP - 2.0);
        assert!(v.flash > 0.9);
        v.fall(40.0);
        assert!(v.dead && v.health == 0.0);
    }

    #[test]
    fn suvda_cho_kish() {
        let mut v = Vitals::new();
        for _ in 0..(MAX_AIR * 10.0) as usize {
            v.tick(0.1, true);
        }
        assert!(v.air < 0.2 && v.health == MAX_HP, "hali zarar yo'q: {} {}", v.air, v.health);
        for _ in 0..25 {
            v.tick(0.1, true);
        }
        assert!(v.health < MAX_HP, "cho'kish zarari yo'q");
        // havoga chiqsa nafas tiklanadi
        let mut w = Vitals::new();
        for _ in 0..50 {
            w.tick(0.1, true);
        }
        for _ in 0..50 {
            w.tick(0.1, false);
        }
        assert!(w.air > 5.0);
    }

    #[test]
    fn ochlik_va_tiklanish() {
        let mut v = Vitals::new();
        v.exhaust(8.0);
        v.tick(0.016, false);
        assert_eq!(v.hunger, MAX_FOOD - 2.0);
        // to'q bo'lsa jarohat tiklanadi
        let mut h = Vitals::new();
        h.damage(6.0);
        for _ in 0..(5.0 / 0.1) as usize {
            h.tick(0.1, false);
        }
        assert!(h.health > MAX_HP - 6.0, "tiklanmadi {}", h.health);
        // och qolsa tiklanmaydi
        let mut s = Vitals::new();
        s.hunger = 5.0;
        s.damage(4.0);
        for _ in 0..200 {
            s.tick(0.1, false);
        }
        assert_eq!(s.health, MAX_HP - 4.0);
    }

    #[test]
    fn ochlikdan_olmaydi_va_ovqat() {
        let mut v = Vitals::new();
        v.hunger = 0.0;
        for _ in 0..2000 {
            v.tick(0.1, false);
        }
        assert!(!v.dead && v.health >= 1.0 && v.health <= 2.0, "{}", v.health);
        v.eat(6.0);
        assert_eq!(v.hunger, 6.0);
        v.eat(100.0);
        assert_eq!(v.hunger, MAX_FOOD);
    }

    #[test]
    fn olim_va_qayta_tugilish() {
        let mut v = Vitals::new();
        v.damage(100.0);
        assert!(v.dead);
        v.damage(5.0);
        v.tick(1.0, true);
        assert_eq!(v.health, 0.0);
        v.respawn();
        assert!(!v.dead && v.health == MAX_HP && v.hunger == MAX_FOOD && v.air == MAX_AIR);
    }

    #[test]
    fn chaqnash_so_nadi() {
        let mut v = Vitals::new();
        v.damage(2.0);
        assert!(v.flash > 0.99);
        for _ in 0..40 {
            v.tick(0.016, false);
        }
        assert!(v.flash < 0.4);
    }
}
