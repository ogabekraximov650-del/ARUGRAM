//! Namuna rasmlar: tashqi fayl yo'q, shaklar kodda chiziladi (hajm kichik, tarmoq kerak emas).
//! Haqiqiy kontent (Telegram kanalidan) ulangach shu joyga haqiqiy muqovalar keladi.

use std::{cell::RefCell, collections::HashMap, rc::Rc};
use uzcoders::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Art {
    Mountain,
    City,
    Dunes,
    Food,
    Tech,
    Forest,
    Dawn,
}

thread_local! {
    static CACHE: RefCell<HashMap<Art, Rc<Image>>> = RefCell::new(HashMap::new());
}

/// Rasm bir marta chiziladi va eslab qolinadi.
pub fn art(a: Art) -> Rc<Image> {
    CACHE.with(|c| c.borrow_mut().entry(a).or_insert_with(|| Rc::new(paint(a))).clone())
}

fn rgb(c: u32) -> [f32; 3] {
    [((c >> 16) & 255) as f32, ((c >> 8) & 255) as f32, (c & 255) as f32]
}

fn inside(p: &[(f32, f32)], x: f32, y: f32) -> bool {
    let mut c = false;
    let mut j = p.len() - 1;
    for i in 0..p.len() {
        let ((xi, yi), (xj, yj)) = (p[i], p[j]);
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            c = !c;
        }
        j = i;
    }
    c
}

/// Chizish maydoni: koordinatalar "virtual" (400 yoki 390 kenglikda), `k` — piksel koeffitsiyenti.
struct Pix {
    w: usize,
    h: usize,
    k: f32,
    px: Vec<u8>,
}

impl Pix {
    fn new(w: usize, h: usize, vw: f32) -> Pix {
        Pix { w, h, k: w as f32 / vw, px: vec![255; w * h * 4] }
    }

    /// Tepadan pastga ko'p bosqichli gradient: (joy 0..1, rang).
    fn grad(&mut self, stops: &[(f32, u32)]) {
        for y in 0..self.h {
            let t = y as f32 / (self.h - 1) as f32;
            let mut i = 0;
            while i + 2 < stops.len() && t > stops[i + 1].0 {
                i += 1;
            }
            let ((t0, c0), (t1, c1)) = (stops[i], stops[i + 1]);
            let u = ((t - t0) / (t1 - t0)).clamp(0.0, 1.0);
            let (a, b) = (rgb(c0), rgb(c1));
            for x in 0..self.w {
                let o = (y * self.w + x) * 4;
                for ch in 0..3 {
                    self.px[o + ch] = (a[ch] + (b[ch] - a[ch]) * u) as u8;
                }
            }
        }
    }

    fn put(&mut self, x: usize, y: usize, c: [f32; 3], a: f32) {
        let o = (y * self.w + x) * 4;
        for ch in 0..3 {
            self.px[o + ch] = (self.px[o + ch] as f32 * (1.0 - a) + c[ch] * a) as u8;
        }
    }

    fn bounds(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> (usize, usize, usize, usize) {
        (
            x0.max(0.0) as usize,
            (x1.ceil().max(0.0) as usize).min(self.w),
            y0.max(0.0) as usize,
            (y1.ceil().max(0.0) as usize).min(self.h),
        )
    }

    fn disc(&mut self, cx: f32, cy: f32, r: f32, col: u32) {
        let (cx, cy, r, c) = (cx * self.k, cy * self.k, r * self.k, rgb(col));
        let (xs, xe, ys, ye) = self.bounds(cx - r - 2.0, cy - r - 2.0, cx + r + 2.0, cy + r + 2.0);
        for y in ys..ye {
            for x in xs..xe {
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                let a = (r - d + 0.5).clamp(0.0, 1.0);
                if a > 0.0 {
                    self.put(x, y, c, a);
                }
            }
        }
    }

    fn poly(&mut self, pts: &[(f32, f32)], col: u32) {
        let p: Vec<(f32, f32)> = pts.iter().map(|&(x, y)| (x * self.k, y * self.k)).collect();
        let c = rgb(col);
        let (mut x0, mut x1, mut y0, mut y1) = (f32::MAX, 0.0f32, f32::MAX, 0.0f32);
        for &(x, y) in &p {
            x0 = x0.min(x);
            x1 = x1.max(x);
            y0 = y0.min(y);
            y1 = y1.max(y);
        }
        let (xs, xe, ys, ye) = self.bounds(x0, y0, x1, y1);
        for y in ys..ye {
            for x in xs..xe {
                let mut n = 0;
                for sy in 0..3 {
                    for sx in 0..3 {
                        if inside(&p, x as f32 + (sx as f32 + 0.5) / 3.0, y as f32 + (sy as f32 + 0.5) / 3.0) {
                            n += 1;
                        }
                    }
                }
                if n > 0 {
                    self.put(x, y, c, n as f32 / 9.0);
                }
            }
        }
    }

    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, col: u32) {
        self.poly(&[(x, y), (x + w, y), (x + w, y + h), (x, y + h)], col);
    }

    fn rrect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, col: u32) {
        let k = self.k;
        let (x, y, w, h, r, c) = (x * k, y * k, w * k, h * k, r * k, rgb(col));
        let (xs, xe, ys, ye) = self.bounds(x - 2.0, y - 2.0, x + w + 2.0, y + h + 2.0);
        for yy in ys..ye {
            for xx in xs..xe {
                let (px, py) = (xx as f32 + 0.5 - (x + w / 2.0), yy as f32 + 0.5 - (y + h / 2.0));
                let (qx, qy) = (px.abs() - (w / 2.0 - r), py.abs() - (h / 2.0 - r));
                let sd = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - r;
                let a = (0.5 - sd).clamp(0.0, 1.0);
                if a > 0.0 {
                    self.put(xx, yy, c, a);
                }
            }
        }
    }

    /// To'lqinsimon tepalik (barxan): `base` atrofida sinus chizig'i, pastiga to'ldiriladi.
    fn wave(&mut self, base: f32, amp: f32, freq: f32, phase: f32, col: u32) {
        let mut pts = Vec::new();
        let mut x = 0.0;
        while x <= 400.0 {
            pts.push((x, base + amp * (x * freq + phase).sin()));
            x += 8.0;
        }
        pts.push((400.0, 225.0));
        pts.push((0.0, 225.0));
        self.poly(&pts, col);
    }
}

fn paint(a: Art) -> Image {
    let tall = a == Art::Dawn;
    let (w, h, vw) = if tall { (234, 506, 390.0) } else { (360, 203, 400.0) };
    let mut p = Pix::new(w, h, vw);
    match a {
        Art::Mountain => {
            p.grad(&[(0.0, 0x2B1B3D), (1.0, 0xF2A65A)]);
            p.disc(270.0, 120.0, 30.0, 0xFFE2A8);
            p.poly(&[(0.0, 170.0), (70.0, 100.0), (120.0, 150.0), (180.0, 85.0), (250.0, 160.0), (320.0, 110.0), (400.0, 165.0), (400.0, 225.0), (0.0, 225.0)], 0x5B3358);
            p.poly(&[(0.0, 195.0), (60.0, 140.0), (130.0, 185.0), (210.0, 130.0), (300.0, 190.0), (360.0, 160.0), (400.0, 185.0), (400.0, 225.0), (0.0, 225.0)], 0x2A1A30);
            p.poly(&[(0.0, 215.0), (90.0, 180.0), (190.0, 210.0), (300.0, 175.0), (400.0, 210.0), (400.0, 225.0), (0.0, 225.0)], 0x100A14);
        }
        Art::City => {
            p.grad(&[(0.0, 0x0B1A33), (1.0, 0x3A66A3)]);
            p.disc(320.0, 50.0, 18.0, 0xF4F1E8);
            p.poly(&[(0.0, 225.0), (0.0, 150.0), (40.0, 150.0), (40.0, 110.0), (80.0, 110.0), (80.0, 140.0), (110.0, 140.0), (110.0, 85.0), (155.0, 85.0), (155.0, 130.0), (185.0, 130.0), (185.0, 95.0), (225.0, 95.0), (225.0, 120.0), (265.0, 120.0), (265.0, 75.0), (310.0, 75.0), (310.0, 125.0), (345.0, 125.0), (345.0, 100.0), (400.0, 100.0), (400.0, 225.0)], 0x070F1E);
            for &(x, y) in &[(30.0, 160.0), (60.0, 120.0), (90.0, 150.0), (120.0, 100.0), (135.0, 135.0), (200.0, 105.0), (240.0, 130.0), (280.0, 90.0), (295.0, 120.0), (320.0, 140.0), (360.0, 115.0)] {
                p.rect(x, y, 4.0, 5.0, 0xFFD27A);
            }
        }
        Art::Dunes => {
            p.grad(&[(0.0, 0xF8D9A0), (1.0, 0xE8A44F)]);
            p.disc(90.0, 70.0, 26.0, 0xFFF3D6);
            p.wave(150.0, 20.0, 0.016, 0.0, 0xC97A30);
            p.wave(185.0, 18.0, 0.02, 1.5, 0x93501C);
            p.wave(212.0, 12.0, 0.025, 3.0, 0x5A2C10);
        }
        Art::Food => {
            p.grad(&[(0.0, 0x4A2416), (1.0, 0x1B0C07)]);
            p.disc(200.0, 118.0, 88.0, 0xEFE5D6);
            p.disc(200.0, 118.0, 66.0, 0xC2410C);
            p.disc(178.0, 100.0, 16.0, 0x7BA84A);
            p.disc(222.0, 130.0, 14.0, 0xF4D35E);
            p.disc(205.0, 96.0, 9.0, 0xFFF3D6);
            p.disc(185.0, 138.0, 10.0, 0x7BA84A);
        }
        Art::Tech => {
            p.grad(&[(0.0, 0x0F1826), (1.0, 0x27507F)]);
            p.disc(200.0, 112.0, 80.0, 0x1C3D66);
            p.rrect(160.0, 30.0, 80.0, 165.0, 14.0, 0xE6E6E6);
            p.rrect(166.0, 38.0, 68.0, 149.0, 9.0, 0x0A0A0A);
            p.rrect(172.0, 50.0, 56.0, 34.0, 6.0, 0xE2620F);
            p.rrect(172.0, 92.0, 56.0, 8.0, 4.0, 0x333333);
            p.rrect(172.0, 106.0, 40.0, 8.0, 4.0, 0x333333);
        }
        Art::Forest => {
            p.grad(&[(0.0, 0x0C2A22), (1.0, 0x2D8068)]);
            for &(x, w, h, c) in &[(10.0, 70.0, 120.0, 0x0A2A20), (70.0, 90.0, 150.0, 0x0E3A2C), (150.0, 60.0, 110.0, 0x0A2A20), (210.0, 100.0, 160.0, 0x0E3A2C), (300.0, 80.0, 130.0, 0x0A2A20), (350.0, 70.0, 100.0, 0x0E3A2C)] {
                p.poly(&[(x, 225.0), (x + w / 2.0, 225.0 - h), (x + w, 225.0)], c);
            }
            p.wave(205.0, 5.0, 0.02, 0.0, 0x061A14);
        }
        Art::Dawn => {
            p.grad(&[(0.0, 0x1B1030), (0.55, 0xB8441B), (1.0, 0xF2A65A)]);
            p.disc(195.0, 430.0, 62.0, 0xFFE2A8);
            p.poly(&[(0.0, 560.0), (90.0, 450.0), (150.0, 520.0), (230.0, 420.0), (310.0, 510.0), (390.0, 440.0), (390.0, 844.0), (0.0, 844.0)], 0x4B2A52);
            p.poly(&[(0.0, 640.0), (70.0, 560.0), (140.0, 620.0), (230.0, 540.0), (320.0, 620.0), (390.0, 580.0), (390.0, 844.0), (0.0, 844.0)], 0x241430);
            p.poly(&[(0.0, 730.0), (100.0, 670.0), (200.0, 720.0), (300.0, 660.0), (390.0, 710.0), (390.0, 844.0), (0.0, 844.0)], 0x0F0A14);
        }
    }
    Image { w: w as u32, h: h as u32, rgba: p.px }
}
