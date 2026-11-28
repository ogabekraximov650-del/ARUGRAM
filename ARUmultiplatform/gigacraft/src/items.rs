//! Buyumlar, asboblar, inventar, yasash va pishirish retseptlari (GL siz, sinaladigan mantiq).
use crate::blocks::*;

/// Buyum raqami: 0..64 — bloklar (blok raqami bilan bir xil), 64+ — boshqa buyumlar.
pub type Item = u16;

pub const NONE: Item = 0;
pub const STICK: Item = 64;
pub const COAL: Item = 65;
pub const IRON_INGOT: Item = 66;
pub const GOLD_INGOT: Item = 67;
pub const DIAMOND: Item = 68;
pub const WHEAT: Item = 69;
pub const BREAD: Item = 70;
pub const SEEDS: Item = 71;
pub const RAW_MEAT: Item = 72;
pub const COOKED_MEAT: Item = 73;
pub const APPLE: Item = 74;
/// Asboblar: TOOL0 + tur*4 + daraja (tur: 0 kurak, 1 bolta, 2 belkurak, 3 qilich; daraja: 0 yog'och .. 3 olmos).
pub const TOOL0: Item = 80;
pub const TOOL_END: Item = 96;

pub const SLOTS: usize = 36;
pub const MAX_STACK: u8 = 64;

pub fn tool(kind: u16, tier: u16) -> Item {
    TOOL0 + kind * 4 + tier
}

pub fn is_tool(i: Item) -> bool {
    (TOOL0..TOOL_END).contains(&i)
}

pub fn tool_kind(i: Item) -> Tool {
    if !is_tool(i) {
        return Tool::Hand;
    }
    match (i - TOOL0) / 4 {
        0 => Tool::Pick,
        1 => Tool::Axe,
        2 => Tool::Shovel,
        _ => Tool::Sword,
    }
}

/// Asbob darajasi: 1 yog'och, 2 tosh, 3 temir, 4 olmos. Asbob bo'lmasa 0.
pub fn tool_tier(i: Item) -> u8 {
    if is_tool(i) {
        ((i - TOOL0) % 4) as u8 + 1
    } else {
        0
    }
}

pub fn max_durability(i: Item) -> u16 {
    match tool_tier(i) {
        1 => 60,
        2 => 130,
        3 => 250,
        4 => 1500,
        _ => 0,
    }
}

pub fn is_block(i: Item) -> bool {
    i > 0 && i < COUNT as Item
}

pub fn is_food(i: Item) -> bool {
    matches!(i, BREAD | COOKED_MEAT | APPLE | RAW_MEAT)
}

pub fn food_value(i: Item) -> f32 {
    match i {
        BREAD => 5.0,
        COOKED_MEAT => 8.0,
        APPLE => 4.0,
        RAW_MEAT => 2.0,
        _ => 0.0,
    }
}

pub fn name(i: Item) -> &'static str {
    if is_block(i) {
        return crate::blocks::name(i as u8);
    }
    match i {
        STICK => "TAYOQCHA",
        COAL => "KO'MIR",
        IRON_INGOT => "TEMIR QUYMA",
        GOLD_INGOT => "OLTIN QUYMA",
        DIAMOND => "ALMOS",
        WHEAT => "BUG'DOY",
        BREAD => "NON",
        SEEDS => "URUG'",
        RAW_MEAT => "XOM GO'SHT",
        COOKED_MEAT => "PISHGAN GO'SHT",
        APPLE => "OLMA",
        _ if is_tool(i) => {
            const N: [[&str; 4]; 4] = [
                ["YOG'OCH KURAK", "TOSH KURAK", "TEMIR KURAK", "ALMOS KURAK"],
                ["YOG'OCH BOLTA", "TOSH BOLTA", "TEMIR BOLTA", "ALMOS BOLTA"],
                ["YOG'OCH BELKURAK", "TOSH BELKURAK", "TEMIR BELKURAK", "ALMOS BELKURAK"],
                ["YOG'OCH QILICH", "TOSH QILICH", "TEMIR QILICH", "ALMOS QILICH"],
            ];
            N[((i - TOOL0) / 4) as usize][((i - TOOL0) % 4) as usize]
        }
        _ => "",
    }
}

/// Buyum rangi (blok bo'lmaganlar uchun belgi rangi).
pub fn color(i: Item) -> [f32; 3] {
    if is_block(i) {
        return crate::blocks::color(i as u8, 0);
    }
    match i {
        STICK => [0.60, 0.42, 0.28],
        COAL => [0.14, 0.14, 0.18],
        IRON_INGOT => [0.85, 0.85, 0.90],
        GOLD_INGOT => [0.98, 0.84, 0.30],
        DIAMOND => [0.35, 0.90, 0.95],
        WHEAT => [0.92, 0.80, 0.30],
        BREAD => [0.80, 0.55, 0.28],
        SEEDS => [0.50, 0.70, 0.30],
        RAW_MEAT => [0.90, 0.40, 0.45],
        COOKED_MEAT => [0.62, 0.34, 0.20],
        APPLE => [0.90, 0.15, 0.20],
        _ if is_tool(i) => match (i - TOOL0) % 4 {
            0 => [0.72, 0.52, 0.34],
            1 => [0.60, 0.60, 0.66],
            2 => [0.92, 0.92, 0.96],
            _ => [0.35, 0.92, 0.96],
        },
        _ => [1.0, 0.0, 1.0],
    }
}

/// Asbob bilan blokni buzish tezligi ko'paytiruvchisi.
pub fn speed(held: Item, b: u8) -> f32 {
    let bt = best_tool(b);
    let tk = tool_kind(held);
    if tk == bt && tk != Tool::Hand {
        match tool_tier(held) {
            1 => 2.0,
            2 => 4.0,
            3 => 6.0,
            _ => 8.0,
        }
    } else if tk == Tool::Sword && b == LEAF {
        4.0
    } else {
        1.0
    }
}

/// Blok shu asbob bilan buzilganda buyum beradimi.
pub fn can_harvest(held: Item, b: u8) -> bool {
    let need = min_tier(b);
    need == 0 || (tool_kind(held) == Tool::Pick && tool_tier(held) >= need)
}

/// Buzilgan blokdan tushadigan buyum (raqam, soni). `roll` — tasodifiy [0,1).
pub fn drop(held: Item, b: u8, roll: f32) -> Option<(Item, u8)> {
    if !can_harvest(held, b) {
        return None;
    }
    Some(match b {
        GRASS | DRYGRASS => (DIRT as Item, 1),
        STONE => (COBBLE as Item, 1),
        COAL_ORE => (COAL, 1),
        DIAMOND_ORE => (DIAMOND, 1),
        LEAF => {
            if roll < 0.08 {
                (APPLE, 1)
            } else if roll < 0.2 {
                (STICK, 1)
            } else {
                return None;
            }
        }
        GLASS => return None,
        b if b == CROP3 => (WHEAT, 1 + (roll * 2.0) as u8),
        b if is_crop(b) => (SEEDS, 1),
        WATER | AIR => return None,
        _ => (b as Item, 1),
    })
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stack {
    pub item: Item,
    pub n: u8,
    /// Asbobning ishlatilgan marta soni.
    pub used: u16,
}

impl Stack {
    pub const EMPTY: Stack = Stack { item: NONE, n: 0, used: 0 };

    pub fn new(item: Item, n: u8) -> Stack {
        Stack { item, n, used: 0 }
    }

    pub fn is_empty(&self) -> bool {
        self.n == 0 || self.item == NONE
    }

    pub fn max(&self) -> u8 {
        if is_tool(self.item) {
            1
        } else {
            MAX_STACK
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inventory {
    /// 0..9 — tez panel, 9..36 — qolgan.
    pub s: [Stack; SLOTS],
}

impl Default for Inventory {
    fn default() -> Self {
        Inventory { s: [Stack::EMPTY; SLOTS] }
    }
}

impl Inventory {
    /// Buyumlarni qo'shadi; sig'maganlar sonini qaytaradi.
    pub fn add(&mut self, item: Item, mut n: u32) -> u32 {
        if item == NONE {
            return 0;
        }
        let max = Stack::new(item, 1).max() as u32;
        for st in self.s.iter_mut() {
            if n == 0 {
                break;
            }
            if st.item == item && !st.is_empty() && (st.n as u32) < max {
                let k = (max - st.n as u32).min(n);
                st.n += k as u8;
                n -= k;
            }
        }
        for st in self.s.iter_mut() {
            if n == 0 {
                break;
            }
            if st.is_empty() {
                let k = max.min(n);
                *st = Stack::new(item, k as u8);
                n -= k;
            }
        }
        n
    }

    pub fn count(&self, item: Item) -> u32 {
        self.s.iter().filter(|s| s.item == item && !s.is_empty()).map(|s| s.n as u32).sum()
    }

    /// Bittasini olib tashlaydi.
    pub fn take_one(&mut self, item: Item) -> bool {
        for st in self.s.iter_mut() {
            if st.item == item && !st.is_empty() {
                st.n -= 1;
                if st.n == 0 {
                    *st = Stack::EMPTY;
                }
                return true;
            }
        }
        false
    }

    /// Tez paneldagi katak bittasini kamaytiradi.
    pub fn use_one(&mut self, i: usize) {
        if self.s[i].n > 0 {
            self.s[i].n -= 1;
            if self.s[i].n == 0 {
                self.s[i] = Stack::EMPTY;
            }
        }
    }

    /// Asbobni ishlatadi; sinsa `true`.
    pub fn wear(&mut self, i: usize) -> bool {
        let st = &mut self.s[i];
        if !is_tool(st.item) {
            return false;
        }
        st.used += 1;
        if st.used >= max_durability(st.item) {
            *st = Stack::EMPTY;
            return true;
        }
        false
    }
}

/// Kursor bilan katakni almashtirish (teginish): bir xil bo'lsa birlashtiradi, bo'lmasa almashtiradi.
pub fn click_slot(slot: &mut Stack, cursor: &mut Stack) {
    if !cursor.is_empty() && !slot.is_empty() && slot.item == cursor.item && slot.max() > 1 {
        let k = (slot.max() - slot.n).min(cursor.n);
        slot.n += k;
        cursor.n -= k;
        if cursor.n == 0 {
            *cursor = Stack::EMPTY;
        }
    } else {
        std::mem::swap(slot, cursor);
    }
}

// ---------- Retseptlar ----------

pub struct Recipe {
    /// Kenglik, balandlik va katakchalar (qatordan qatorga); NONE — bo'sh.
    pub w: usize,
    pub h: usize,
    pub cells: Vec<Item>,
    pub out: Item,
    pub n: u8,
}

const P: Item = PLANK as Item;
const S: Item = STICK;
const C: Item = COBBLE as Item;
const I: Item = IRON_INGOT;
const D: Item = DIAMOND;
const L: Item = LOG as Item;
const G: Item = GLASS as Item;
const __: Item = NONE;

macro_rules! r {
    ($w:expr, $h:expr, [$($c:expr),*], $o:expr, $n:expr) => {
        Recipe { w: $w, h: $h, cells: vec![$($c),*], out: $o, n: $n }
    };
}

macro_rules! tools {
    ($m:expr, $t:expr) => {
        [
            r!(3, 3, [$m, $m, $m, __, S, __, __, S, __], tool(0, $t), 1),
            r!(2, 3, [$m, $m, $m, S, __, S], tool(1, $t), 1),
            r!(1, 3, [$m, S, S], tool(2, $t), 1),
            r!(1, 3, [$m, $m, S], tool(3, $t), 1),
        ]
    };
}

pub fn recipes() -> Vec<Recipe> {
    let mut v = vec![
        r!(1, 1, [L], P, 4),
        r!(1, 2, [P, P], S, 4),
        r!(2, 2, [P, P, P, P], CRAFTING as Item, 1),
        r!(3, 3, [C, C, C, C, __, C, C, C, C], FURNACE as Item, 1),
        r!(3, 3, [P, P, P, P, __, P, P, P, P], CHEST as Item, 1),
        r!(1, 2, [COAL, S], LAMP as Item, 2),
        r!(2, 2, [G, G, G, G], G, 4),
        r!(3, 1, [WHEAT, WHEAT, WHEAT], BREAD, 1),
        r!(2, 2, [C, C, C, C], BRICK as Item, 4),
        r!(2, 2, [SAND as Item, SAND as Item, SAND as Item, SAND as Item], G, 4),
    ];
    // Eslatma: bu yerda bloklar `as Item`; asboblar:
    for (m, t) in [(P, 0u16), (C, 1), (I, 2), (D, 3)] {
        v.extend(tools!(m, t));
    }
    v
}

/// To'r (w x h, qatorma-qator, 3x3 gacha) bo'yicha retsept natijasini topadi.
pub fn craft(grid: &[Item], gw: usize, gh: usize, rs: &[Recipe]) -> Option<(Item, u8)> {
    // Bo'sh chetlarni qirqamiz
    let (mut x0, mut x1, mut y0, mut y1) = (gw, 0, gh, 0);
    for y in 0..gh {
        for x in 0..gw {
            if grid[x + y * gw] != NONE {
                x0 = x0.min(x);
                x1 = x1.max(x);
                y0 = y0.min(y);
                y1 = y1.max(y);
            }
        }
    }
    if x1 < x0 {
        return None;
    }
    let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
    for r in rs {
        if r.w != w || r.h != h {
            continue;
        }
        for mirror in [false, true] {
            let ok = (0..h).all(|y| {
                (0..w).all(|x| {
                    let rx = if mirror { w - 1 - x } else { x };
                    grid[(x0 + x) + (y0 + y) * gw] == r.cells[rx + y * w]
                })
            });
            if ok {
                return Some((r.out, r.n));
            }
        }
    }
    None
}

/// Pechda pishirish: kirish -> chiqish.
pub fn smelt(i: Item) -> Option<Item> {
    match i {
        x if x == IRON_ORE as Item => Some(IRON_INGOT),
        x if x == GOLD_ORE as Item => Some(GOLD_INGOT),
        x if x == SAND as Item => Some(GLASS as Item),
        x if x == COBBLE as Item => Some(STONE as Item),
        x if x == LOG as Item => Some(COAL),
        RAW_MEAT => Some(COOKED_MEAT),
        _ => None,
    }
}

/// Yoqilg'i: bitta buyum nechta pishirishga yetadi.
pub fn fuel(i: Item) -> u32 {
    match i {
        COAL => 8,
        x if x == LOG as Item => 3,
        x if x == PLANK as Item => 1,
        STICK => 1,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kopaytirish_va_qoshish() {
        let mut inv = Inventory::default();
        assert_eq!(inv.add(DIRT as Item, 100), 0);
        assert_eq!(inv.count(DIRT as Item), 100);
        assert_eq!(inv.s[0].n, 64);
        assert_eq!(inv.s[1].n, 36);
        assert!(inv.take_one(DIRT as Item));
        assert_eq!(inv.count(DIRT as Item), 99);
        // to'lib ketish
        let mut full = Inventory::default();
        let left = full.add(STONE as Item, 64 * 36 + 5);
        assert_eq!(left, 5);
    }

    #[test]
    fn asbob_xossalari() {
        let wp = tool(0, 0);
        let dp = tool(0, 3);
        assert_eq!(tool_kind(wp), Tool::Pick);
        assert_eq!(tool_tier(wp), 1);
        assert_eq!(tool_tier(dp), 4);
        assert!(speed(dp, STONE) > speed(wp, STONE) && speed(wp, STONE) > speed(NONE, STONE));
        assert!(speed(tool(1, 0), STONE) == 1.0, "bolta toshni tezlashtirmaydi");
        // temir rudasi uchun tosh kurak yetmaydi, temir yetadi
        assert!(!can_harvest(tool(0, 0), IRON_ORE));
        assert!(can_harvest(tool(0, 1), IRON_ORE));
        assert!(!can_harvest(tool(0, 1), DIAMOND_ORE) && can_harvest(tool(0, 2), GOLD_ORE));
        assert!(can_harvest(tool(0, 3), DIAMOND_ORE));
        assert_eq!(drop(tool(0, 0), STONE, 0.5), Some((COBBLE as Item, 1)));
        assert_eq!(drop(NONE, STONE, 0.5), None);
        assert_eq!(drop(NONE, COAL_ORE, 0.5), None);
        assert_eq!(drop(tool(0, 0), COAL_ORE, 0.5), Some((COAL, 1)));
        assert_eq!(drop(NONE, LOG, 0.5), Some((LOG as Item, 1)));
        assert_eq!(drop(NONE, CROP3, 0.1).unwrap().0, WHEAT);
        assert_eq!(drop(NONE, CROP0, 0.1).unwrap().0, SEEDS);
    }

    #[test]
    fn asbob_eskiradi() {
        let mut inv = Inventory::default();
        inv.s[0] = Stack::new(tool(3, 0), 1);
        for _ in 0..59 {
            assert!(!inv.wear(0));
        }
        assert!(inv.wear(0));
        assert!(inv.s[0].is_empty());
    }

    #[test]
    fn yasash_retseptlari() {
        let rs = recipes();
        // 1 tana -> 4 taxta
        assert_eq!(craft(&[L, 0, 0, 0], 2, 2, &rs), Some((P, 4)));
        // 2x2 taxta -> ustaxona
        assert_eq!(craft(&[P, P, P, P], 2, 2, &rs), Some((CRAFTING as Item, 1)));
        // taxta ustma-ust -> tayoqcha (to'rning istalgan joyida)
        assert_eq!(craft(&[0, 0, 0, 0, P, 0, 0, P, 0], 3, 3, &rs), Some((S, 4)));
        // tosh kurak, 3x3
        assert_eq!(craft(&[C, C, C, 0, S, 0, 0, S, 0], 3, 3, &rs), Some((tool(0, 1), 1)));
        // ... ko'zgudagi shakli va siljitilgani ham ishlaydi
        assert_eq!(craft(&[0, S, 0, 0, S, 0, C, C, C], 3, 3, &rs), None);
        assert_eq!(craft(&[S, 0, S, C, C, C, 0, 0, 0].map(|x| x), 3, 3, &rs), None);
        // temir bolta (ko'zgu)
        assert_eq!(craft(&[I, I, 0, I, S, 0, 0, S, 0], 3, 3, &rs), Some((tool(1, 2), 1)));
        assert_eq!(craft(&[0, I, I, 0, S, I, 0, S, 0], 3, 3, &rs), Some((tool(1, 2), 1)));
        // olmos qilich
        assert_eq!(craft(&[0, D, 0, 0, D, 0, 0, S, 0], 3, 3, &rs), Some((tool(3, 3), 1)));
        // pech 3x3 da, 2x2 da chiqmaydi
        assert_eq!(craft(&[C, C, C, C, 0, C, C, C, C], 3, 3, &rs), Some((FURNACE as Item, 1)));
        assert_eq!(craft(&[C, C, C, C], 2, 2, &rs).map(|r| r.0), Some(BRICK as Item));
        assert_eq!(craft(&[0; 4], 2, 2, &rs), None);
        assert_eq!(craft(&[STICK, 0, 0, 0], 2, 2, &rs), None);
    }

    #[test]
    fn retsept_chiqishi_togri() {
        for r in recipes() {
            assert_eq!(r.cells.len(), r.w * r.h);
            assert!(r.out != NONE && r.n > 0);
            assert!(!name(r.out).is_empty(), "{}", r.out);
        }
    }

    #[test]
    fn pishirish_va_yoqilgi() {
        assert_eq!(smelt(IRON_ORE as Item), Some(IRON_INGOT));
        assert_eq!(smelt(SAND as Item), Some(GLASS as Item));
        assert_eq!(smelt(RAW_MEAT), Some(COOKED_MEAT));
        assert_eq!(smelt(STICK), None);
        assert_eq!(smelt(DIRT as Item), None);
        assert!(fuel(COAL) > fuel(LOG as Item) && fuel(DIRT as Item) == 0);
    }

    #[test]
    fn kursor_almashish() {
        let mut a = Stack::new(DIRT as Item, 10);
        let mut c = Stack::new(DIRT as Item, 60);
        click_slot(&mut a, &mut c);
        assert_eq!((a.n, c.n), (64, 6));
        let mut b = Stack::new(STONE as Item, 3);
        click_slot(&mut b, &mut c);
        assert_eq!(b.item, DIRT as Item);
        assert_eq!(c.item, STONE as Item);
    }

    #[test]
    fn nomlar_va_ranglar() {
        for i in 1..TOOL_END {
            if (COUNT as Item..STICK).contains(&i) || (APPLE + 1..TOOL0).contains(&i) {
                continue;
            }
            assert!(!name(i).is_empty(), "{i}");
            for c in color(i) {
                assert!((0.0..=1.0).contains(&c));
            }
        }
    }
}
