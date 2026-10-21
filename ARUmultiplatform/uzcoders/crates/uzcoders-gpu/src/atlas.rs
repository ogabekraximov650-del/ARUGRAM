//! UZ: Harflar va ikonkalar atlasi uchun "javon" (shelf) usulidagi joylashtirgich: elementlar qatorlarga ketma-ket terib chiqiladi.
//! EN: A "shelf" packer for the glyph/icon atlas: items are placed one after another in rows.
//! RU: Упаковщик «полками» (shelf) для атласа глифов и иконок: элементы укладываются подряд по строкам.

pub struct Atlas {
    pub size: u32,
    shelves: Vec<Shelf>,
    next_y: u32,
}

struct Shelf {
    y: u32,
    h: u32,
    x: u32,
}

impl Atlas {
    pub fn new(size: u32) -> Atlas {
        Atlas {
            size,
            shelves: Vec::new(),
            next_y: 0,
        }
    }

    /// UZ: `w x h` o'lchamli joy ajratadi (atrofida 1 piksel bo'sh joy qoldiriladi). `None` — joy qolmagan.
    /// EN: Reserves a `w x h` slot (with 1 pixel of padding around it). `None` means there is no room left.
    /// RU: Выделяет место размером `w x h` (вокруг остаётся 1 пиксель отступа). `None` — места не осталось.
    pub fn alloc(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        let (pw, ph) = (w + 2, h + 2);
        if pw > self.size || ph > self.size {
            return None;
        }
        for s in &mut self.shelves {
            if s.h >= ph && s.h <= ph * 2 && s.x + pw <= self.size {
                let pos = (s.x + 1, s.y + 1);
                s.x += pw;
                return Some(pos);
            }
        }
        if self.next_y + ph > self.size {
            return None;
        }
        let y = self.next_y;
        self.next_y += ph;
        self.shelves.push(Shelf { y, h: ph, x: pw });
        Some((1, y + 1))
    }

    pub fn reset(&mut self) {
        self.shelves.clear();
        self.next_y = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joylashtirish_va_toshish() {
        let mut a = Atlas::new(64);
        let p1 = a.alloc(10, 10).unwrap();
        let p2 = a.alloc(10, 10).unwrap();
        // UZ: bitta javonda (qatorda) joylashadi
        // EN: placed on the same shelf (row).
        // RU: размещаются на одной полке (в одной строке).
        assert_eq!(p1.1, p2.1);
        assert!(p2.0 > p1.0);
        // UZ: Test: atlas to'lguncha ajratamiz.
        // EN: Test: keep allocating until the atlas is full.
        // RU: Тест: выделяем места, пока атлас не заполнится.
        let mut n = 0;
        while a.alloc(10, 10).is_some() {
            n += 1;
            assert!(n < 1000);
        }
        assert!(n >= 20);
        a.reset();
        assert!(a.alloc(10, 10).is_some());
        assert!(a.alloc(100, 10).is_none());
    }

    #[test]
    fn kesishmaydi() {
        let mut a = Atlas::new(128);
        let mut rects = Vec::new();
        for i in 0..30u32 {
            let (w, h) = (8 + i % 7, 9 + i % 5);
            if let Some((x, y)) = a.alloc(w, h) {
                rects.push((x, y, w, h));
            }
        }
        for (i, a1) in rects.iter().enumerate() {
            for b in &rects[i + 1..] {
                let overlap =
                    a1.0 < b.0 + b.2 && b.0 < a1.0 + a1.2 && a1.1 < b.1 + b.3 && b.1 < a1.1 + a1.3;
                assert!(!overlap, "{a1:?} {b:?}");
            }
        }
    }
}
