//! Harf/ikonka atlasi uchun "javon" (shelf) joylashtirgich.

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

    /// `w x h` joy ajratadi (atrofida 1 px bo'sh joy bilan). `None` — joy yo'q.
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
        assert_eq!(p1.1, p2.1); // bitta javonda
        assert!(p2.0 > p1.0);
        // To'lguncha ajratamiz.
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
