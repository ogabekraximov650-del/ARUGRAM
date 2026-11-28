//! Blok turlari va ranglari (o'zimizning uslub: yumshoq ranglar, yonuvchi kristall).

pub const AIR: u8 = 0;
pub const GRASS: u8 = 1;
pub const DIRT: u8 = 2;
pub const STONE: u8 = 3;
pub const SAND: u8 = 4;
pub const SNOW: u8 = 5;
pub const LOG: u8 = 6;
pub const LEAF: u8 = 7;
pub const CRYSTAL: u8 = 8;
pub const WATER: u8 = 9;
pub const WHITE: u8 = 10;
pub const PINK: u8 = 11;
pub const PLANK: u8 = 12;
pub const DRYGRASS: u8 = 13;
pub const COBBLE: u8 = 14;
pub const COAL_ORE: u8 = 15;
pub const IRON_ORE: u8 = 16;
pub const GOLD_ORE: u8 = 17;
pub const DIAMOND_ORE: u8 = 18;
pub const GLASS: u8 = 19;
pub const BRICK: u8 = 20;
pub const CRAFTING: u8 = 21;
pub const FURNACE: u8 = 22;
pub const CHEST: u8 = 23;
pub const LAMP: u8 = 24;
/// Bug'doy o'sish bosqichlari (0..3); 3 — pishgan.
pub const CROP0: u8 = 25;
pub const CROP3: u8 = 28;
pub const ENCHANT: u8 = 29;
pub const REDSTONE_ORE: u8 = 30;
pub const WIRE_OFF: u8 = 31;
pub const WIRE_ON: u8 = 32;
pub const LEVER_OFF: u8 = 33;
pub const LEVER_ON: u8 = 34;
pub const RLAMP_OFF: u8 = 35;
pub const RLAMP_ON: u8 = 36;
/// Hali ochilmagan o'ljali sandiq (uylar va qal'alarda): birinchi ochilganda oddiy sandiqqa aylanadi.
pub const LOOTCHEST: u8 = 37;
pub const COUNT: u8 = 38;

/// Qo'yish mumkin bo'lgan bloklar (havo va suvdan tashqari), tanlash oynasida shu tartibda.
pub const PLACEABLE: [u8; 22] = [
    GRASS, DIRT, STONE, SAND, SNOW, LOG, LEAF, PLANK, WHITE, PINK, CRYSTAL, COBBLE, GLASS, BRICK, CRAFTING, FURNACE, CHEST, LAMP, ENCHANT, WIRE_OFF,
    LEVER_OFF, RLAMP_OFF,
];

pub fn is_crop(b: u8) -> bool {
    (CROP0..=CROP3).contains(&b)
}

/// Past (to'liq kub bo'lmagan) bloklar: ekin, sim, tutqich. Ular to'siq emas.
pub fn is_thin(b: u8) -> bool {
    is_crop(b) || block_height(b) < 1.0
}

/// Past blokning balandligi (blok ulushi); oddiylari uchun 1.
pub fn block_height(b: u8) -> f32 {
    match b {
        CROP0..=CROP3 => 0.25 + 0.2 * (b - CROP0) as f32,
        WIRE_OFF | WIRE_ON => 0.1,
        LEVER_OFF | LEVER_ON => 0.6,
        _ => 1.0,
    }
}

/// To'siq (o'yinchi o'tolmaydi va ko'rinishni to'sadi). Past bloklar to'siq emas.
pub fn solid(b: u8) -> bool {
    b != AIR && b != WATER && !is_thin(b)
}

/// Nishonga olish va buzish mumkin (suv va havodan boshqa hammasi).
pub fn targetable(b: u8) -> bool {
    b != AIR && b != WATER
}

/// Orqasi ko'rinadigan blok (qo'shni yuzlar chiziladi).
pub fn see_through(b: u8) -> bool {
    b == AIR || b == WATER || b == GLASS || is_thin(b)
}

/// Yorug'lik chiqaradi (soyalanmaydi).
pub fn emissive(b: u8) -> bool {
    b == CRYSTAL || b == LAMP || b == RLAMP_ON || b == WIRE_ON
}

/// Atrofni yoritadigan blok.
pub fn light_source(b: u8) -> bool {
    b == LAMP || b == RLAMP_ON
}

pub fn name(b: u8) -> &'static str {
    match b {
        GRASS => "O'T",
        DIRT => "TUPROQ",
        STONE => "TOSH",
        SAND => "QUM",
        SNOW => "QOR",
        LOG => "TANA",
        LEAF => "BARG",
        CRYSTAL => "KRISTALL",
        WATER => "SUV",
        WHITE => "OQ BLOK",
        PINK => "PUSHTI",
        PLANK => "TAXTA",
        DRYGRASS => "QURUQ O'T",
        COBBLE => "BRUSCHATKA",
        COAL_ORE => "KO'MIR RUDASI",
        IRON_ORE => "TEMIR RUDASI",
        GOLD_ORE => "OLTIN RUDASI",
        DIAMOND_ORE => "ALMOS RUDASI",
        GLASS => "OYNA",
        BRICK => "G'ISHT",
        CRAFTING => "USTAXONA",
        FURNACE => "PECH",
        CHEST => "SANDIQ",
        LAMP => "CHIROQ",
        CROP0..=CROP3 => "BUG'DOY",
        ENCHANT => "SEHR STOLI",
        REDSTONE_ORE => "REDSTONE RUDASI",
        WIRE_OFF | WIRE_ON => "SIM",
        LEVER_OFF | LEVER_ON => "TUTQICH",
        RLAMP_OFF | RLAMP_ON => "REDSTONE CHIROQ",
        LOOTCHEST => "SANDIQ",
        _ => "HAVO",
    }
}

/// Qo'l bilan buzish vaqti (soniya), omon qolish rejimida. Asboblar (keyingi qadamda) uni qisqartiradi.
pub fn hardness(b: u8) -> f32 {
    match b {
        GRASS | DRYGRASS => 0.9,
        DIRT | SAND => 0.75,
        SNOW | LEAF => 0.3,
        LOG => 3.0,
        PLANK => 2.0,
        STONE | COBBLE => 3.0,
        COAL_ORE => 3.5,
        IRON_ORE | GOLD_ORE => 4.5,
        DIAMOND_ORE => 5.5,
        BRICK => 3.5,
        GLASS => 0.4,
        CRAFTING | CHEST => 2.2,
        FURNACE => 3.5,
        LAMP | RLAMP_OFF | RLAMP_ON => 0.5,
        ENCHANT => 5.0,
        REDSTONE_ORE => 4.5,
        WIRE_OFF | WIRE_ON | LEVER_OFF | LEVER_ON => 0.1,
        LOOTCHEST => 2.2,
        CROP0..=CROP3 => 0.05,
        CRYSTAL => 2.0,
        WHITE | PINK => 1.5,
        _ => 1.0,
    }
}

/// Blok rangi: face 0 — tepa, 1 — yon, 2 — past. Qiymatlar [0,1].
pub fn color(b: u8, face: u8) -> [f32; 3] {
    match b {
        GRASS => match face {
            0 => [0.47, 0.87, 0.58],
            1 => [0.52, 0.43, 0.47],
            _ => [0.50, 0.40, 0.46],
        },
        DIRT => [0.52, 0.40, 0.46],
        STONE => [0.50, 0.46, 0.60],
        SAND => [0.95, 0.83, 0.62],
        SNOW => [0.96, 0.96, 1.00],
        LOG => {
            if face == 1 {
                [0.47, 0.33, 0.40]
            } else {
                [0.72, 0.54, 0.50]
            }
        }
        LEAF => [0.93, 0.50, 0.69],
        CRYSTAL => [1.00, 0.60, 0.20],
        WATER => [0.20, 0.52, 0.88],
        WHITE => [0.94, 0.93, 0.98],
        PINK => [0.96, 0.30, 0.60],
        PLANK => [0.85, 0.62, 0.45],
        DRYGRASS => match face {
            0 => [0.80, 0.78, 0.45],
            1 => [0.52, 0.43, 0.47],
            _ => [0.50, 0.40, 0.46],
        },
        COBBLE => [0.42, 0.40, 0.52],
        COAL_ORE => [0.26, 0.24, 0.32],
        IRON_ORE => [0.80, 0.62, 0.52],
        GOLD_ORE => [0.98, 0.84, 0.30],
        DIAMOND_ORE => [0.35, 0.90, 0.95],
        GLASS => [0.72, 0.90, 0.98],
        BRICK => [0.74, 0.32, 0.30],
        CRAFTING => match face {
            0 => [0.80, 0.58, 0.40],
            _ => [0.62, 0.44, 0.30],
        },
        FURNACE => match face {
            0 => [0.55, 0.52, 0.62],
            _ => [0.38, 0.36, 0.46],
        },
        CHEST => match face {
            0 => [0.86, 0.62, 0.28],
            _ => [0.70, 0.46, 0.20],
        },
        LAMP => [1.00, 0.92, 0.55],
        ENCHANT => match face {
            0 => [0.55, 0.15, 0.30],
            _ => [0.22, 0.10, 0.36],
        },
        REDSTONE_ORE => [0.78, 0.22, 0.22],
        WIRE_OFF => [0.42, 0.08, 0.08],
        WIRE_ON => [1.00, 0.16, 0.10],
        LEVER_OFF => [0.55, 0.50, 0.45],
        LEVER_ON => [0.95, 0.72, 0.30],
        RLAMP_OFF => [0.50, 0.36, 0.28],
        RLAMP_ON => [1.00, 0.86, 0.52],
        LOOTCHEST => match face {
            0 => [0.86, 0.62, 0.28],
            _ => [0.70, 0.46, 0.20],
        },
        CROP0 => [0.45, 0.78, 0.35],
        b if is_crop(b) => lerp_col([0.45, 0.78, 0.35], [0.92, 0.80, 0.30], (b - CROP0) as f32 / 3.0),
        _ => [1.0, 0.0, 1.0],
    }
}

fn lerp_col(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// Asbob turi: buzishni tezlashtiradi.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Hand,
    Pick,
    Axe,
    Shovel,
    Sword,
}

/// Blokni qaysi asbob tez buzadi.
pub fn best_tool(b: u8) -> Tool {
    match b {
        STONE | COBBLE | COAL_ORE | IRON_ORE | GOLD_ORE | DIAMOND_ORE | REDSTONE_ORE | ENCHANT | BRICK | FURNACE | CRYSTAL => Tool::Pick,
        LOG | PLANK | CRAFTING | CHEST | LOOTCHEST => Tool::Axe,
        GRASS | DIRT | SAND | SNOW | DRYGRASS => Tool::Shovel,
        _ => Tool::Hand,
    }
}

/// Eng kamida qanday asbob darajasi kerak (0 — qo'l; 1 — yog'och; 2 — tosh; 3 — temir; 4 — olmos).
pub fn min_tier(b: u8) -> u8 {
    match b {
        STONE | COBBLE | COAL_ORE | BRICK | FURNACE | CRYSTAL => 1,
        IRON_ORE => 2,
        GOLD_ORE | DIAMOND_ORE | REDSTONE_ORE => 3,
        ENCHANT => 2,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hamma_blok_rangi_oraliqda() {
        for b in 1..COUNT {
            for f in 0..3 {
                for c in color(b, f) {
                    assert!((0.0..=1.0).contains(&c));
                }
            }
            assert!(!name(b).is_empty());
        }
    }

    #[test]
    fn qattiqlik() {
        assert!(!solid(AIR) && !solid(WATER) && solid(STONE) && solid(LEAF));
        assert!(emissive(CRYSTAL) && !emissive(STONE));
        for b in PLACEABLE {
            assert!(solid(b) || is_thin(b) || b == GLASS);
        }
        assert!(!solid(CROP0) && targetable(CROP3) && see_through(GLASS) && emissive(LAMP));
        assert!(block_height(CROP3) < 1.0 && block_height(CROP0) > 0.0 && is_thin(WIRE_ON) && is_thin(LEVER_OFF) && !is_thin(STONE));
        assert!(light_source(RLAMP_ON) && !light_source(RLAMP_OFF) && emissive(WIRE_ON));
        assert_eq!(best_tool(STONE), Tool::Pick);
        assert!(min_tier(DIAMOND_ORE) > min_tier(IRON_ORE));
    }
}
