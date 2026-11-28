//! Redstone: tutqich -> sim tarmog'i -> chiroq. Tarmoq bog'langan sim kataklari (6 qo'shni) bo'yicha topiladi.
use crate::blocks::*;
use crate::planet::*;
use crate::world::World;
use std::collections::{HashMap, HashSet};

const DIRS: [(i64, i64, i64); 6] = [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)];

fn near(c: &Cell, d: (i64, i64, i64)) -> Cell {
    let n = wrap(c.f, c.u + d.0, c.v + d.1, c.r + d.2);
    n
}

fn is_wire(b: u8) -> bool {
    b == WIRE_OFF || b == WIRE_ON
}

fn is_lamp(b: u8) -> bool {
    b == RLAMP_OFF || b == RLAMP_ON
}

/// `at` atrofidagi redstone holatini qayta hisoblaydi (tutqich almashganda, sim/chiroq qo'yilganda yoki buzilganda).
/// O'zgargan kataklar va ularning yangi bloklarini qaytaradi; chaqiruvchi ularni dunyoga yozadi.
pub fn settle(w: &World, at: &Cell) -> Vec<(Cell, u8)> {
    let mut seeds = vec![*at];
    for d in DIRS {
        seeds.push(near(at, d));
    }
    // 1. Sim tarmoqlari
    let mut comp_of: HashMap<Cell, usize> = HashMap::new();
    let mut comps: Vec<Vec<Cell>> = Vec::new();
    for s in &seeds {
        if !is_wire(w.block(s)) || comp_of.contains_key(s) {
            continue;
        }
        let id = comps.len();
        let mut stack = vec![*s];
        let mut cells = Vec::new();
        comp_of.insert(*s, id);
        while let Some(c) = stack.pop() {
            cells.push(c);
            if cells.len() > 800 {
                break;
            }
            for d in DIRS {
                let n = near(&c, d);
                if !comp_of.contains_key(&n) && is_wire(w.block(&n)) {
                    comp_of.insert(n, id);
                    stack.push(n);
                }
            }
        }
        comps.push(cells);
    }
    let mut changes: HashMap<Cell, u8> = HashMap::new();
    let mut wire_state: HashMap<Cell, bool> = HashMap::new();
    for cells in &comps {
        let powered = cells.iter().any(|c| DIRS.iter().any(|&d| w.block(&near(c, d)) == LEVER_ON));
        for c in cells {
            wire_state.insert(*c, powered);
            let want = if powered { WIRE_ON } else { WIRE_OFF };
            if w.block(c) != want {
                changes.insert(*c, want);
            }
        }
    }
    // 2. Chiroqlar: tarmoq kataklari va seed'lar qo'shnilari
    let mut lamps: HashSet<Cell> = HashSet::new();
    for c in comps.iter().flatten().chain(seeds.iter()) {
        for d in DIRS {
            let n = near(c, d);
            if is_lamp(w.block(&n)) {
                lamps.insert(n);
            }
        }
        if is_lamp(w.block(c)) {
            lamps.insert(*c);
        }
    }
    for l in lamps {
        let on = DIRS.iter().any(|&d| {
            let n = near(&l, d);
            match wire_state.get(&n) {
                Some(&p) => p,
                None => matches!(w.block(&n), WIRE_ON | LEVER_ON),
            }
        });
        let want = if on { RLAMP_ON } else { RLAMP_OFF };
        if w.block(&l) != want {
            changes.insert(l, want);
        }
    }
    changes.into_iter().collect()
}

/// O'zgarishlarni dunyoga yozadi; qayta chizish kerak bo'lgan ustunlarni qaytaradi.
pub fn apply(w: &World, changes: &[(Cell, u8)]) -> Vec<(u8, i32, i32)> {
    let mut out = Vec::new();
    for (c, v) in changes {
        out.extend(w.set_block(c, *v));
    }
    out
}

/// Tutqichni almashtiradi, tarmoqni yangilaydi; qayta chizish kerak bo'lgan ustunlarni qaytaradi.
pub fn toggle_lever(w: &World, c: &Cell) -> Vec<(u8, i32, i32)> {
    let nb = match w.block(c) {
        LEVER_OFF => LEVER_ON,
        LEVER_ON => LEVER_OFF,
        _ => return Vec::new(),
    };
    let mut dirty = w.set_block(c, nb);
    let ch = settle(w, c);
    dirty.extend(apply(w, &ch));
    dirty
}

/// Redstone bilan bog'liq blokmi.
pub fn related(b: u8) -> bool {
    matches!(b, WIRE_OFF | WIRE_ON | LEVER_OFF | LEVER_ON | RLAMP_OFF | RLAMP_ON)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(u: i64, r: i64) -> Cell {
        Cell { f: 0, u: 30_000_000 + u, v: 30_000_000, r: SEA_R + 9000 + r }
    }

    #[test]
    fn tutqich_sim_orqali_chiroqni_yoqadi() {
        let w = World::new(1);
        // tutqich(0) - sim 1..4 - chiroq(5)
        w.set_block(&at(0, 0), LEVER_OFF);
        for i in 1..=4 {
            w.set_block(&at(i, 0), WIRE_OFF);
        }
        w.set_block(&at(5, 0), RLAMP_OFF);
        assert!(settle(&w, &at(5, 0)).is_empty(), "tutqich o'chiq: o'zgarish yo'q");
        toggle_lever(&w, &at(0, 0));
        assert_eq!(w.block(&at(0, 0)), LEVER_ON);
        for i in 1..=4 {
            assert_eq!(w.block(&at(i, 0)), WIRE_ON, "sim {i}");
        }
        assert_eq!(w.block(&at(5, 0)), RLAMP_ON);
        // o'chirish
        toggle_lever(&w, &at(0, 0));
        assert_eq!(w.block(&at(3, 0)), WIRE_OFF);
        assert_eq!(w.block(&at(5, 0)), RLAMP_OFF);
    }

    #[test]
    fn uzilgan_sim_va_alohida_tarmoq() {
        let w = World::new(1);
        w.set_block(&at(0, 0), LEVER_ON);
        w.set_block(&at(1, 0), WIRE_OFF);
        w.set_block(&at(2, 0), WIRE_OFF);
        // 3-katak bo'sh: uzilish
        w.set_block(&at(4, 0), WIRE_OFF);
        w.set_block(&at(5, 0), RLAMP_OFF);
        let ch = settle(&w, &at(1, 0));
        for (c, v) in ch {
            w.set_block(&c, v);
        }
        assert_eq!(w.block(&at(2, 0)), WIRE_ON);
        let ch = settle(&w, &at(4, 0));
        for (c, v) in ch {
            w.set_block(&c, v);
        }
        assert_eq!(w.block(&at(4, 0)), WIRE_OFF, "uzilgan bo'lak quvvatsiz");
        assert_eq!(w.block(&at(5, 0)), RLAMP_OFF);
        // uzilishni ulaymiz
        w.set_block(&at(3, 0), WIRE_OFF);
        for (c, v) in settle(&w, &at(3, 0)) {
            w.set_block(&c, v);
        }
        assert_eq!(w.block(&at(4, 0)), WIRE_ON);
        assert_eq!(w.block(&at(5, 0)), RLAMP_ON);
        // simni buzamiz: chiroq o'chadi
        w.set_block(&at(3, 0), AIR);
        for (c, v) in settle(&w, &at(3, 0)) {
            w.set_block(&c, v);
        }
        assert_eq!(w.block(&at(4, 0)), WIRE_OFF);
        assert_eq!(w.block(&at(5, 0)), RLAMP_OFF);
    }

    #[test]
    fn tutqich_chiroqqa_toqridan_toqri() {
        let w = World::new(1);
        w.set_block(&at(0, 0), LEVER_OFF);
        w.set_block(&at(1, 0), RLAMP_OFF);
        toggle_lever(&w, &at(0, 0));
        assert_eq!(w.block(&at(1, 0)), RLAMP_ON);
    }
}
