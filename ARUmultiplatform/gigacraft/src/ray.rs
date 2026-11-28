//! Nur otish: qaysi blokka qaralayotganini topadi (egri sayyora uchun kichik qadamlar bilan yurib).
use crate::blocks::targetable;
use crate::math::*;
use crate::planet::*;
use crate::world::World;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// Tegilgan blok.
    pub cell: Cell,
    /// Undan oldingi bo'sh blok (yangi blok shu yerga qo'yiladi).
    pub place: Cell,
    pub dist: f32,
}

/// `o` nuqtadan `d` (birlik) yo'nalishda `max` blokkacha. Boshlang'ich blok qattiq bo'lsa `None`.
pub fn raycast(w: &World, o: D3, d: D3, max: f64) -> Option<Hit> {
    let mut prev = cell_at(o);
    if targetable(w.block(&prev)) {
        return None;
    }
    let step = 0.1;
    let mut t = step;
    while t <= max {
        let c = cell_at(d_add(o, d_mul(d, t)));
        if c != prev {
            if targetable(w.block(&c)) {
                return Some(Hit { cell: c, place: prev, dist: t as f32 });
            }
            prev = c;
        }
        t += step;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::*;

    const HIGH: i64 = SEA_R + 9000;

    fn at(u: i64, v: i64, dr: i64) -> Cell {
        Cell { f: 0, u: 30_000_000 + u, v: 30_000_000 + v, r: HIGH + dr }
    }

    fn up(c: &Cell) -> D3 {
        d_norm(center(c))
    }

    #[test]
    fn pastga_nur_polga_uradi() {
        let w = World::new(1);
        let target = at(3, 3, 0);
        w.set_block(&target, STONE);
        let o = d_add(center(&target), d_mul(up(&target), 5.0));
        let h = raycast(&w, o, d_mul(up(&target), -1.0), 20.0).unwrap();
        assert_eq!(h.cell, target);
        assert_eq!(h.place, Cell { r: target.r + 1, ..target });
        assert!((h.dist - 4.5).abs() < 0.3, "{}", h.dist);
    }

    #[test]
    fn havoda_hech_narsa_yoq() {
        let w = World::new(1);
        let c = at(0, 0, 500);
        assert!(raycast(&w, center(&c), d_norm(d_cross(up(&c), [0.0, 1.0, 0.3])), 50.0).is_none());
    }

    #[test]
    fn yon_tomonlama_va_masofa_chegarasi() {
        let w = World::new(1);
        let start = at(0, 0, 10);
        let wall = at(5, 0, 10);
        w.set_block(&wall, STONE);
        let dirv = d_norm(d_sub(d_mul(dir(0, 30_000_005.5, 30_000_000.5), HIGH as f64 + 10.5), center(&start)));
        let h = raycast(&w, center(&start), dirv, 20.0).unwrap();
        assert_eq!(h.cell, wall);
        assert_eq!(h.place, at(4, 0, 10));
        assert!(raycast(&w, center(&start), dirv, 3.0).is_none());
    }

    #[test]
    fn ichkarida_none() {
        let w = World::new(1);
        let c = at(0, 0, 10);
        w.set_block(&c, STONE);
        assert!(raycast(&w, center(&c), up(&c), 10.0).is_none());
    }
}
