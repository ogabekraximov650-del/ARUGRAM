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
pub const COUNT: u8 = 14;

/// Qo'yish mumkin bo'lgan bloklar (havo va suvdan tashqari), tanlash oynasida shu tartibda.
pub const PLACEABLE: [u8; 11] = [GRASS, DIRT, STONE, SAND, SNOW, LOG, LEAF, PLANK, WHITE, PINK, CRYSTAL];

/// To'siq (o'yinchi o'tolmaydi va ko'rinishni to'sadi).
pub fn solid(b: u8) -> bool {
    b != AIR && b != WATER
}

/// Yorug'lik chiqaradi (soyalanmaydi).
pub fn emissive(b: u8) -> bool {
    b == CRYSTAL
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
        STONE => 3.0,
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
        _ => [1.0, 0.0, 1.0],
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
            assert!(solid(b));
        }
    }
}
