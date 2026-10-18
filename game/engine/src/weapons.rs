//! Qurol turlari va ularning xarakteristikalari.

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

    /// HUD belgisi uchun rang.
    pub fn color(self) -> [f32; 3] {
        match self {
            WeaponKind::Pistol => [0.55, 0.65, 0.85],
            WeaponKind::Rifle => [0.95, 0.55, 0.15],
            WeaponKind::Shotgun => [0.85, 0.25, 0.25],
            WeaponKind::Sniper => [0.2, 0.7, 0.3],
        }
    }
}
