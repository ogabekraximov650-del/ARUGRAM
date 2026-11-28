//! Interfeys: tugmalar joylashuvi (barcha o'lchamlar 1080 balandlikka nisbatan o'lchanadi), teginish nuqtasini aniqlash va chizish.
use crate::blocks::{self, PLACEABLE};
use crate::items::{self, Item, Stack};
use crate::hud::*;
use crate::stream::QUALITY_NAMES;

/// Kvadrat tugma: markaz va yarim tomon.
#[derive(Clone, Copy, Debug)]
pub struct Sq {
    pub cx: f32,
    pub cy: f32,
    pub r: f32,
}

impl Sq {
    pub fn hit(&self, x: f32, y: f32) -> bool {
        (x - self.cx).abs() <= self.r && (y - self.cy).abs() <= self.r
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Btn {
    Jump,
    Fly,
    Down,
    Speed,
    Break,
    Place,
    Pause,
    Inv,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Play,
    Inv,
    Pause,
    Dead,
    /// Omon qolish inventari / ustaxona / pech oynasi.
    Craft,
}

/// Yasash oynasi turi.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Station {
    /// Inventar: 2x2.
    Hand,
    /// Ustaxona: 3x3.
    Table,
    /// Pech: bitta kirish katagi.
    Furnace,
    /// Sehr stoli: bitta asbob katagi.
    Enchant,
    /// Sandiq: 9x3 katak.
    Chest,
}

impl Station {
    /// Qator/ustun soni (kvadrat to'rlar uchun).
    pub fn grid(self) -> usize {
        match self {
            Station::Hand => 2,
            Station::Table => 3,
            _ => 1,
        }
    }

    /// Yuqoridagi to'rdagi kataklar soni.
    pub fn count(self) -> usize {
        match self {
            Station::Chest => 27,
            s => s.grid() * s.grid(),
        }
    }

    /// Natija katagi bormi.
    pub fn has_result(self) -> bool {
        self != Station::Chest
    }
}

/// Yasash oynasidagi teginish nishoni.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CraftHit {
    Close,
    Inv(usize),
    Grid(usize),
    Result,
}

/// Yasash oynasini chizish uchun nusxa.
#[derive(Clone)]
pub struct CraftView {
    pub station: Station,
    pub inv: [Stack; items::SLOTS],
    pub grid: [Stack; 9],
    pub chest: [Stack; 27],
    pub result: Stack,
    pub cursor: Stack,
    pub fuel: u32,
    /// Sehrlash narxi (oltin quyma), bo'lmasa 0.
    pub cost: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PauseAct {
    Resume,
    Mode,
    Quality,
    Day,
    Sunset,
    Night,
    SensDown,
    SensUp,
}

pub struct Layout {
    pub w: f32,
    pub h: f32,
    pub s: f32,
    pub jump: Sq,
    pub fly: Sq,
    pub down: Sq,
    pub speed: Sq,
    pub brk: Sq,
    pub place: Sq,
    pub pause: Sq,
    pub inv: Sq,
    pub slot: f32,
    pub slot_gap: f32,
    pub slot_x0: f32,
    pub slot_y: f32,
    pub joy_home: (f32, f32),
    pub joy_r: f32,
}

impl Layout {
    pub fn new(w: f32, h: f32) -> Layout {
        let s = h / 1080.0;
        let avail = w - 1165.0 * s;
        let slot = (avail / 9.9).clamp(70.0 * s, 124.0 * s);
        let gap = slot * 0.1;
        let total = 9.0 * slot + 8.0 * gap;
        let q = |cx: f32, cy: f32, r: f32| Sq { cx, cy, r: r * s };
        Layout {
            w,
            h,
            s,
            jump: q(w - 150.0 * s, h - 440.0 * s, 86.0),
            fly: q(w - 150.0 * s, h - 630.0 * s, 64.0),
            down: q(w - 350.0 * s, h - 440.0 * s, 64.0),
            speed: q(w - 350.0 * s, h - 600.0 * s, 52.0),
            brk: q(w - 440.0 * s, h - 200.0 * s, 104.0),
            place: q(w - 164.0 * s, h - 204.0 * s, 116.0),
            pause: q(w - 62.0 * s, 80.0 * s, 48.0),
            inv: q(w - 170.0 * s, 80.0 * s, 48.0),
            slot,
            slot_gap: gap,
            slot_x0: (w - total) * 0.5,
            slot_y: h - 38.0 * s - slot,
            joy_home: (330.0 * s, h - 260.0 * s),
            joy_r: 190.0 * s,
        }
    }

    pub fn slot_rect(&self, i: usize) -> (f32, f32, f32, f32) {
        (self.slot_x0 + i as f32 * (self.slot + self.slot_gap), self.slot_y, self.slot, self.slot)
    }

    pub fn slot_at(&self, x: f32, y: f32) -> Option<usize> {
        (0..9).find(|&i| {
            let (rx, ry, rw, rh) = self.slot_rect(i);
            x >= rx && x <= rx + rw && y >= ry - 10.0 * self.s && y <= ry + rh + 10.0 * self.s
        })
    }

    /// O'yin ekranidagi tugma (bor bo'lsa).
    pub fn button_at(&self, x: f32, y: f32, flying: bool, survival: bool) -> Option<Btn> {
        let all = [
            (Btn::Pause, self.pause),
            (Btn::Inv, self.inv),
            (Btn::Place, self.place),
            (Btn::Break, self.brk),
            (Btn::Jump, self.jump),
        ];
        for (b, sq) in all {
            if sq.hit(x, y) {
                return Some(b);
            }
        }
        if survival {
            return None;
        }
        if self.fly.hit(x, y) {
            return Some(Btn::Fly);
        }
        if flying {
            if self.down.hit(x, y) {
                return Some(Btn::Down);
            }
            if self.speed.hit(x, y) {
                return Some(Btn::Speed);
            }
        }
        None
    }

    // --- Pauza menyusi ---
    pub fn panel(&self) -> (f32, f32, f32, f32) {
        let (pw, ph) = (900.0 * self.s, 900.0 * self.s);
        ((self.w - pw) * 0.5, (self.h - ph) * 0.5, pw, ph)
    }

    pub fn pause_btn(&self, a: PauseAct) -> (f32, f32, f32, f32) {
        let (px, py, pw, _) = self.panel();
        let s = self.s;
        let (m, bh) = (50.0 * s, 110.0 * s);
        let inner = pw - 2.0 * m;
        let row = |i: f32| py + 170.0 * s + i * (bh + 28.0 * s);
        match a {
            PauseAct::Resume => (px + m, row(0.0), inner, bh),
            PauseAct::Mode => (px + m, row(1.0), inner, bh),
            PauseAct::Quality => (px + m, row(2.0), inner, bh),
            PauseAct::Day => (px + m, row(3.0), inner / 3.0 - 14.0 * s, bh),
            PauseAct::Sunset => (px + m + inner / 3.0 + 7.0 * s, row(3.0), inner / 3.0 - 14.0 * s, bh),
            PauseAct::Night => (px + m + 2.0 * inner / 3.0 + 14.0 * s, row(3.0), inner / 3.0 - 14.0 * s, bh),
            PauseAct::SensDown => (px + m, row(4.0), 150.0 * s, bh),
            PauseAct::SensUp => (px + m + inner - 150.0 * s, row(4.0), 150.0 * s, bh),
        }
    }

    pub fn pause_at(&self, x: f32, y: f32) -> Option<PauseAct> {
        use PauseAct::*;
        [Resume, Mode, Quality, Day, Sunset, Night, SensDown, SensUp].into_iter().find(|&a| {
            let (bx, by, bw, bh) = self.pause_btn(a);
            x >= bx && x <= bx + bw && y >= by && y <= by + bh
        })
    }

    /// O'lim ekranidagi "Qayta tug'ilish" tugmasi.
    pub fn respawn_btn(&self) -> (f32, f32, f32, f32) {
        let (bw, bh) = (620.0 * self.s, 120.0 * self.s);
        ((self.w - bw) * 0.5, self.h * 0.58, bw, bh)
    }

    // --- Blok tanlash oynasi ---
    pub fn inv_panel(&self) -> (f32, f32, f32, f32) {
        let (pw, ph) = (1500.0 * self.s, 760.0 * self.s);
        ((self.w - pw) * 0.5, (self.h - ph) * 0.5, pw, ph)
    }

    pub fn inv_card(&self, i: usize) -> (f32, f32, f32, f32) {
        let (px, py, pw, _) = self.inv_panel();
        let s = self.s;
        let cols = 6usize;
        let gap = 14.0 * s;
        let cw = (pw - 2.0 * 50.0 * s - gap * (cols as f32 - 1.0)) / cols as f32;
        let ch = 130.0 * s;
        let (c, r) = (i % cols, i / cols);
        (px + 50.0 * s + c as f32 * (cw + gap), py + 150.0 * s + r as f32 * (ch + gap), cw, ch)
    }

    // --- Yasash oynasi (omon qolish) ---
    pub fn craft_panel(&self) -> (f32, f32, f32, f32) {
        let (pw, ph) = (1360.0 * self.s, 1000.0 * self.s);
        ((self.w - pw) * 0.5, (self.h - ph) * 0.5, pw, ph)
    }

    pub fn craft_slot_size(&self) -> (f32, f32) {
        (104.0 * self.s, 114.0 * self.s)
    }

    pub fn craft_inv_slot(&self, i: usize) -> (f32, f32) {
        let (px, py, pw, _) = self.craft_panel();
        let (_, step) = self.craft_slot_size();
        let x0 = px + (pw - 9.0 * step) * 0.5;
        let (row, col) = if i < 9 { (3usize, i) } else { ((i - 9) / 9, (i - 9) % 9) };
        let extra = if row == 3 { 24.0 * self.s } else { 0.0 };
        (x0 + col as f32 * step, py + 500.0 * self.s + row as f32 * step + extra)
    }

    pub fn craft_grid_slot(&self, st: Station, i: usize) -> (f32, f32) {
        let (px, py, pw, _) = self.craft_panel();
        let (_, step) = self.craft_slot_size();
        if st == Station::Chest {
            return (px + (pw - 9.0 * step) * 0.5 + (i % 9) as f32 * step, py + 140.0 * self.s + (i / 9) as f32 * step);
        }
        let g = st.grid();
        (px + 150.0 * self.s + (i % g) as f32 * step, py + 140.0 * self.s + (i / g) as f32 * step)
    }

    pub fn craft_result_slot(&self) -> (f32, f32) {
        let (px, py, _, _) = self.craft_panel();
        let (_, step) = self.craft_slot_size();
        (px + 780.0 * self.s, py + 140.0 * self.s + step)
    }

    pub fn craft_close(&self) -> Sq {
        let (px, py, pw, _) = self.craft_panel();
        Sq { cx: px + pw - 70.0 * self.s, cy: py + 70.0 * self.s, r: 50.0 * self.s }
    }

    pub fn craft_at(&self, x: f32, y: f32, st: Station) -> Option<CraftHit> {
        if self.craft_close().hit(x, y) {
            return Some(CraftHit::Close);
        }
        let (sz, _) = self.craft_slot_size();
        let inside = |p: (f32, f32)| x >= p.0 && x <= p.0 + sz && y >= p.1 && y <= p.1 + sz;
        if st.has_result() && inside(self.craft_result_slot()) {
            return Some(CraftHit::Result);
        }
        for i in 0..st.count() {
            if inside(self.craft_grid_slot(st, i)) {
                return Some(CraftHit::Grid(i));
            }
        }
        (0..items::SLOTS).find(|&i| inside(self.craft_inv_slot(i))).map(CraftHit::Inv)
    }

    pub fn inv_at(&self, x: f32, y: f32) -> Option<usize> {
        (0..PLACEABLE.len()).find(|&i| {
            let (cx, cy, cw, ch) = self.inv_card(i);
            x >= cx && x <= cx + cw && y >= cy && y <= cy + ch
        })
    }

    pub fn inv_close(&self) -> Sq {
        let (px, py, pw, _) = self.inv_panel();
        Sq {
            cx: px + pw - 70.0 * self.s,
            cy: py + 70.0 * self.s,
            r: 50.0 * self.s,
        }
    }
}

/// Chizish uchun holat nusxasi.
pub struct UiState {
    pub mode: Mode,
    pub slots: [Stack; 9],
    pub held: Item,
    pub craft: Option<CraftView>,
    pub cursor_pos: (f32, f32),
    pub rain: f32,
    /// Faol iksir ta'sirlari (matn).
    pub status: String,
    pub sel: usize,
    pub flying: bool,
    pub fly_level: usize,
    pub joy: Option<((f32, f32), (f32, f32))>,
    pub down: Vec<Btn>,
    pub info: String,
    pub fps: f32,
    pub quality: usize,
    pub sens: f32,
    pub loading: f32,
    pub toast: Option<String>,
    pub underwater: bool,
    pub survival: bool,
    pub health: f32,
    pub hunger: f32,
    pub air: f32,
    pub flash: f32,
    /// Buzish jarayoni [0,1] (omon qolish rejimida).
    pub break_frac: f32,
}

fn plate(h: &mut Hud, sq: Sq, fill: Col, line: Col, pressed: bool) {
    let r = sq.r * 2.0 * 0.223;
    let f = if pressed { [fill[0].saturating_add(30), fill[1].saturating_add(30), fill[2].saturating_add(30), fill[3]] } else { fill };
    h.rrect(sq.cx - sq.r, sq.cy - sq.r, sq.r * 2.0, sq.r * 2.0, r, f);
    h.rrect_line(sq.cx - sq.r, sq.cy - sq.r, sq.r * 2.0, sq.r * 2.0, r, 4.0, line);
}

fn button(h: &mut Hud, x: f32, y: f32, w: f32, bh: f32, s: f32, label: &str, fill: Col, fg: Col) {
    h.rrect(x, y, w, bh, bh * 0.223, fill);
    h.text_c(x + w * 0.5, y + bh * 0.5, 44.0 * s, label, fg);
}

pub fn draw(h: &mut Hud, l: &Layout, u: &UiState) {
    let s = l.s;
    let is_down = |b: Btn| u.down.contains(&b);
    // Suv ostida ko'k parda
    if u.underwater {
        h.rect(0.0, 0.0, l.w, l.h, rgba(20, 70, 150, 90));
    }
    // Yuqori chap: nom va ma'lumot
    let pw = (h.text_width(&u.info, 30.0 * s) + 200.0 * s).max(520.0 * s);
    h.rrect(30.0 * s, 28.0 * s, pw, 122.0 * s, 30.0 * s, rgba(0, 0, 0, 150));
    h.rrect(44.0 * s, 40.0 * s, 90.0 * s, 90.0 * s, 20.0 * s, WHITE);
    h.text_c(89.0 * s, 86.0 * s, 40.0 * s, "GC", BLACK);
    h.text(156.0 * s, 40.0 * s, 46.0 * s, "GIGACRAFT", WHITE);
    h.text(158.0 * s, 96.0 * s, 30.0 * s, &u.info, rgba(235, 235, 235, 255));
    if !u.status.is_empty() {
        let tw = h.text_width(&u.status, 30.0 * s);
        h.rrect(30.0 * s, 160.0 * s, tw + 40.0 * s, 52.0 * s, 20.0 * s, rgba(60, 20, 90, 190));
        h.text(50.0 * s, 170.0 * s, 30.0 * s, &u.status, rgba(240, 210, 255, 255));
    }
    // Yuqori o'ng: tugmalar
    plate(h, Sq { ..l.inv }, rgba(0, 0, 0, 150), rgba(255, 255, 255, 70), is_down(Btn::Inv));
    for i in 0..2 {
        for j in 0..2 {
            let (x, y) = (l.inv.cx - 27.0 * s + i as f32 * 30.0 * s, l.inv.cy - 27.0 * s + j as f32 * 30.0 * s);
            h.rrect(x, y, 24.0 * s, 24.0 * s, 6.0 * s, WHITE);
        }
    }
    plate(h, Sq { ..l.pause }, rgba(0, 0, 0, 150), rgba(255, 255, 255, 70), is_down(Btn::Pause));
    h.rect(l.pause.cx - 17.0 * s, l.pause.cy - 24.0 * s, 11.0 * s, 48.0 * s, WHITE);
    h.rect(l.pause.cx + 6.0 * s, l.pause.cy - 24.0 * s, 11.0 * s, 48.0 * s, WHITE);
    // Nishon
    let (cx, cy) = (l.w * 0.5, l.h * 0.5);
    h.circle(cx, cy, 5.0 * s, WHITE);
    let (a, b) = (30.0 * s, 14.0 * s);
    let t = 4.0 * s;
    let cw = rgba(255, 255, 255, 170);
    for (sx, sy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        h.rect(cx + sx * a - if sx > 0.0 { t } else { 0.0 }, cy + sy * a - if sy > 0.0 { b } else { 0.0 }, t, b, cw);
        h.rect(cx + sx * a - if sx > 0.0 { b } else { 0.0 }, cy + sy * a - if sy > 0.0 { t } else { 0.0 }, b, t, cw);
    }
    // Blok paneli
    let tot = 9.0 * l.slot + 8.0 * l.slot_gap;
    h.rrect(l.slot_x0 - 18.0 * s, l.slot_y - 18.0 * s, tot + 36.0 * s, l.slot + 36.0 * s, 40.0 * s, rgba(0, 0, 0, 130));
    for i in 0..9 {
        let (x, y, w, _) = l.slot_rect(i);
        let on = i == u.sel;
        h.rrect(x, y, w, w, w * 0.223, if on { WHITE } else { rgba(14, 14, 14, 215) });
        item_icon(h, x + w * 0.5, y + w * 0.54 + 6.0 * s, w * 0.30, u.slots[i].item);
        if u.survival && u.slots[i].n > 1 {
            h.text_c(x + w * 0.74, y + w * 0.84, 30.0 * s, &format!("{}", u.slots[i].n), if on { BLACK } else { WHITE });
        }
        h.text(x + 12.0 * s, y + 6.0 * s, 24.0 * s, &format!("{}", i + 1), if on { BLACK } else { rgba(200, 200, 200, 255) });
    }
    let nm = items::name(u.held);
    if !nm.is_empty() {
        let nw = h.text_width(nm, 46.0 * s);
        h.rrect((l.w - nw) * 0.5 - 34.0 * s, l.slot_y - 100.0 * s - if u.survival { 56.0 * s } else { 0.0 }, nw + 68.0 * s, 66.0 * s, 26.0 * s, rgba(0, 0, 0, 170));
        h.text_c(l.w * 0.5, l.slot_y - 67.0 * s - if u.survival { 56.0 * s } else { 0.0 }, 46.0 * s, nm, WHITE);
    }
    // Joystik
    let (jc, kn) = match u.joy {
        Some((c, k)) => (c, k),
        None => (l.joy_home, l.joy_home),
    };
    h.circle(jc.0, jc.1, l.joy_r, rgba(0, 0, 0, if u.joy.is_some() { 80 } else { 50 }));
    h.ring(jc.0, jc.1, l.joy_r, 7.0 * s, rgba(255, 255, 255, if u.joy.is_some() { 210 } else { 120 }));
    h.circle(kn.0, kn.1, 84.0 * s, SIG);
    h.ring(kn.0, kn.1, 84.0 * s, 6.0 * s, WHITE);
    // Harakat tugmalari
    plate(h, l.jump, rgba(0, 0, 0, 160), rgba(255, 255, 255, 200), is_down(Btn::Jump));
    let (jx, jy, r) = (l.jump.cx, l.jump.cy, l.jump.r);
    h.tri([(jx, jy - r * 0.55), (jx - r * 0.5, jy + r * 0.05), (jx + r * 0.5, jy + r * 0.05)], WHITE);
    h.rect(jx - r * 0.2, jy, r * 0.4, r * 0.5, WHITE);
    if !u.survival {
        plate(h, l.fly, if u.flying { SIG } else { rgba(0, 0, 0, 160) }, rgba(255, 255, 255, 200), is_down(Btn::Fly));
        h.text_c(l.fly.cx, l.fly.cy, 34.0 * s, "UCH", WHITE);
    }
    if u.flying && !u.survival {
        plate(h, l.down, rgba(0, 0, 0, 160), rgba(255, 255, 255, 200), is_down(Btn::Down));
        let (dx, dy, r) = (l.down.cx, l.down.cy, l.down.r);
        h.tri([(dx, dy + r * 0.55), (dx - r * 0.5, dy - r * 0.05), (dx + r * 0.5, dy - r * 0.05)], WHITE);
        h.rect(dx - r * 0.2, dy - r * 0.5, r * 0.4, r * 0.5, WHITE);
        plate(h, l.speed, rgba(0, 0, 0, 160), rgba(255, 255, 255, 200), is_down(Btn::Speed));
        let mult = crate::player::FLY_LABELS[u.fly_level.min(crate::player::FLY_LABELS.len() - 1)];
        h.text_c(l.speed.cx, l.speed.cy, 30.0 * s, mult, WHITE);
    }
    plate(h, l.brk, WHITE, WHITE, is_down(Btn::Break));
    let (bx, by) = (l.brk.cx, l.brk.cy);
    h.quad([(bx - 40.0 * s, by + 46.0 * s), (bx - 28.0 * s, by + 54.0 * s), (bx + 44.0 * s, by - 28.0 * s), (bx + 32.0 * s, by - 38.0 * s)], BLACK);
    h.quad([(bx - 8.0 * s, by - 52.0 * s), (bx + 52.0 * s, by - 50.0 * s), (bx + 54.0 * s, by + 8.0 * s), (bx + 30.0 * s, by - 18.0 * s)], BLACK);
    plate(h, l.place, SIG, SIG, is_down(Btn::Place));
    item_icon(h, l.place.cx, l.place.cy + 6.0 * s, 60.0 * s, u.held);
    // Xabar
    if let Some(t) = &u.toast {
        let tw = h.text_width(t, 40.0 * s);
        h.rrect((l.w - tw) * 0.5 - 30.0 * s, 200.0 * s, tw + 60.0 * s, 70.0 * s, 24.0 * s, rgba(0, 0, 0, 170));
        h.text_c(l.w * 0.5, 235.0 * s, 40.0 * s, t, WHITE);
    }
    if u.rain > 0.02 {
        draw_rain(h, l, u.rain);
    }
    if u.survival {
        draw_vitals(h, l, u);
    }
    match u.mode {
        Mode::Craft => draw_craft(h, l, u),
        Mode::Pause => draw_pause(h, l, u),
        Mode::Inv => draw_inv(h, l, u),
        Mode::Dead => draw_dead(h, l),
        Mode::Play => {}
    }
    if u.loading < 1.0 {
        h.rect(0.0, 0.0, l.w, l.h, rgba(8, 6, 20, 245));
        h.text_c(l.w * 0.5, l.h * 0.42, 96.0 * s, "GIGACRAFT", WHITE);
        h.text_c(l.w * 0.5, l.h * 0.55, 40.0 * s, "DUNYO YUKLANMOQDA", rgba(220, 220, 220, 255));
        let (bw, bh) = (700.0 * s, 22.0 * s);
        let (bx, by) = ((l.w - bw) * 0.5, l.h * 0.62);
        h.rrect(bx, by, bw, bh, bh * 0.5, rgba(255, 255, 255, 40));
        h.rrect(bx, by, (bw * u.loading.clamp(0.0, 1.0)).max(bh), bh, bh * 0.5, SIG);
    }
}

/// Buyum belgisi: blok — kub; asbob va boshqalar — tekis shakllar.
pub fn item_icon(h: &mut Hud, cx: f32, cy: f32, r: f32, item: Item) {
    if item == items::NONE {
        return;
    }
    if items::is_block(item) {
        h.cube_icon(cx, cy, r, item as u8);
        return;
    }
    let c = items::color(item);
    let col = rgba((c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8, 255);
    let wood = rgba(150, 105, 70, 255);
    let k = r * 1.5;
    if items::is_tool(item) {
        use crate::blocks::Tool;
        match items::tool_kind(item) {
            Tool::Sword => {
                h.quad([(cx - k * 0.12, cy + k * 0.15), (cx + k * 0.55, cy - k * 0.55), (cx + k * 0.7, cy - k * 0.4), (cx + k * 0.0, cy + k * 0.3)], col);
                h.quad([(cx - k * 0.45, cy + k * 0.35), (cx - k * 0.15, cy + k * 0.05), (cx - k * 0.05, cy + k * 0.15), (cx - k * 0.35, cy + k * 0.45)], wood);
                h.quad([(cx - k * 0.7, cy + k * 0.7), (cx - k * 0.4, cy + k * 0.4), (cx - k * 0.3, cy + k * 0.5), (cx - k * 0.6, cy + k * 0.8)], wood);
            }
            kind => {
                h.quad([(cx - k * 0.7, cy + k * 0.6), (cx - k * 0.6, cy + k * 0.7), (cx + k * 0.5, cy - k * 0.4), (cx + k * 0.4, cy - k * 0.5)], wood);
                match kind {
                    Tool::Pick => {
                        h.quad([(cx + k * 0.0, cy - k * 0.8), (cx + k * 0.8, cy - k * 0.0), (cx + k * 0.65, cy + k * 0.05), (cx - k * 0.05, cy - k * 0.6)], col);
                        h.quad([(cx + k * 0.1, cy - k * 0.55), (cx + k * 0.55, cy - k * 0.1), (cx + k * 0.3, cy - k * 0.0), (cx + k * 0.0, cy - k * 0.3)], col);
                    }
                    Tool::Axe => {
                        h.quad([(cx + k * 0.15, cy - k * 0.75), (cx + k * 0.7, cy - k * 0.2), (cx + k * 0.3, cy + k * 0.05), (cx - k * 0.05, cy - k * 0.35)], col);
                    }
                    _ => {
                        h.circle(cx + k * 0.45, cy - k * 0.45, k * 0.28, col);
                    }
                }
            }
        }
    } else {
        h.rrect(cx - k * 0.5, cy - k * 0.5, k, k, k * 0.28, col);
        h.rrect_line(cx - k * 0.5, cy - k * 0.5, k, k, k * 0.28, 3.0, rgba(0, 0, 0, 90));
    }
}

fn slot_box(h: &mut Hud, s: f32, x: f32, y: f32, w: f32, st: &Stack, hot: bool) {
    h.rrect(x, y, w, w, w * 0.2, if hot { rgba(70, 66, 100, 255) } else { rgba(40, 38, 58, 255) });
    if !st.is_empty() {
        item_icon(h, x + w * 0.5, y + w * 0.54, w * 0.30, st.item);
        if st.n > 1 {
            h.text_c(x + w * 0.74, y + w * 0.82, 30.0 * s, &format!("{}", st.n), WHITE);
        }
        if st.ench > 0 {
            h.text(x + w * 0.08, y + w * 0.02, 30.0 * s, &"*".repeat(st.ench as usize), rgba(210, 150, 255, 255));
        }
        if items::is_tool(st.item) && st.used > 0 {
            let f = 1.0 - st.used as f32 / items::max_durability(st.item).max(1) as f32;
            h.rect(x + w * 0.12, y + w * 0.9, w * 0.76, 6.0 * s, rgba(0, 0, 0, 200));
            h.rect(x + w * 0.12, y + w * 0.9, w * 0.76 * f.clamp(0.0, 1.0), 6.0 * s, rgba((255.0 * (1.0 - f)) as u8, (230.0 * f) as u8, 60, 255));
        }
    }
}

fn draw_craft(h: &mut Hud, l: &Layout, u: &UiState) {
    let Some(c) = &u.craft else { return };
    let s = l.s;
    h.rect(0.0, 0.0, l.w, l.h, rgba(0, 0, 0, 150));
    let (px, py, pw, ph) = l.craft_panel();
    h.rrect(px, py, pw, ph, 56.0 * s, rgba(18, 16, 28, 245));
    h.rrect_line(px, py, pw, ph, 56.0 * s, 4.0, rgba(255, 255, 255, 60));
    let title = match c.station {
        Station::Hand => "INVENTAR",
        Station::Table => "USTAXONA",
        Station::Furnace => "PECH",
        Station::Enchant => "SEHR STOLI",
        Station::Chest => "SANDIQ",
    };
    h.text(px + 50.0 * s, py + 36.0 * s, 60.0 * s, title, WHITE);
    let cl = l.craft_close();
    h.rrect(cl.cx - cl.r, cl.cy - cl.r, cl.r * 2.0, cl.r * 2.0, cl.r * 0.45, rgba(40, 38, 58, 255));
    h.text_c(cl.cx, cl.cy, 56.0 * s, "X", WHITE);
    let (sz, _) = l.craft_slot_size();
    for i in 0..c.station.count() {
        let (x, y) = l.craft_grid_slot(c.station, i);
        let st = if c.station == Station::Chest { c.chest[i] } else { c.grid[i] };
        slot_box(h, s, x, y, sz, &st, false);
    }
    let (rx, ry) = l.craft_result_slot();
    let (_, step) = l.craft_slot_size();
    if c.station.has_result() {
        // O'q
        let ax = rx - 120.0 * s;
        h.tri([(ax, ry + sz * 0.5 - 34.0 * s), (ax + 70.0 * s, ry + sz * 0.5), (ax, ry + sz * 0.5 + 34.0 * s)], rgba(200, 200, 220, 255));
        slot_box(h, s, rx, ry, sz, &c.result, !c.result.is_empty());
        if !c.result.is_empty() {
            h.text_c(rx + sz * 0.5, ry + sz + 34.0 * s, 32.0 * s, items::name(c.result.item), WHITE);
        }
    }
    if c.station == Station::Enchant {
        let t = if c.cost > 0 { format!("NARX: {} OLTIN QUYMA", c.cost) } else { "ASBOBNI QO'YING".to_string() };
        h.text(px + 150.0 * s, py + 140.0 * s + step + 10.0 * s, 34.0 * s, &t, rgba(210, 160, 255, 255));
        h.text(px + 150.0 * s, py + 140.0 * s + step + 56.0 * s, 28.0 * s, "KURAK/BOLTA: TEZLIK, QILICH: ZARBA", rgba(200, 200, 215, 255));
    }
    if c.station == Station::Furnace {
        let t = format!("YOQILG'I: {}", c.fuel);
        h.text(px + 150.0 * s, py + 140.0 * s + step + 10.0 * s, 34.0 * s, &t, rgba(255, 190, 90, 255));
        h.text(px + 150.0 * s, py + 140.0 * s + step + 56.0 * s, 28.0 * s, "KO'MIR, TANA YOKI TAXTA KERAK", rgba(200, 200, 215, 255));
    }
    for i in 0..items::SLOTS {
        let (x, y) = l.craft_inv_slot(i);
        slot_box(h, s, x, y, sz, &c.inv[i], i == u.sel);
    }
    if !c.cursor.is_empty() {
        let (x, y) = u.cursor_pos;
        item_icon(h, x, y, sz * 0.34, c.cursor.item);
        if c.cursor.n > 1 {
            h.text_c(x + sz * 0.3, y + sz * 0.32, 32.0 * s, &format!("{}", c.cursor.n), WHITE);
        }
    }
}

/// Yomg'ir: qiya chiziqlar va xira parda.
fn draw_rain(h: &mut Hud, l: &Layout, k: f32) {
    let s = l.s;
    h.rect(0.0, 0.0, l.w, l.h, rgba(30, 40, 60, (k * 55.0) as u8));
    let n = (k * 120.0) as usize;
    let mut z = 0x1234_5678u32;
    for i in 0..n {
        z ^= z << 13;
        z ^= z >> 17;
        z ^= z << 5;
        let x = (z & 0xFFFF) as f32 / 65535.0 * l.w;
        let y = ((z >> 16) & 0xFFFF) as f32 / 65535.0 * l.h;
        let len = (30.0 + (i % 5) as f32 * 8.0) * s;
        h.quad([(x, y), (x + 3.0 * s, y), (x - 6.0 * s, y + len), (x - 9.0 * s, y + len)], rgba(190, 210, 240, 110));
    }
}

/// Yurak (frac: 1 — to'liq, 0.5 — yarmi, 0 — bo'sh). Markaz (cx, cy), o'lcham r.
fn heart(h: &mut Hud, cx: f32, cy: f32, r: f32, frac: f32) {
    let red = rgba(226, 40, 52, 255);
    let dark = rgba(60, 20, 30, 220);
    let (l, rt) = (if frac >= 0.5 { red } else { dark }, if frac >= 1.0 { red } else { dark });
    let k = r * 0.55;
    h.circle(cx - k, cy - r * 0.25, k, l);
    h.circle(cx + k, cy - r * 0.25, k, rt);
    h.tri([(cx - r * 1.1, cy - r * 0.05), (cx, cy - r * 0.05), (cx, cy + r * 1.0)], l);
    h.tri([(cx, cy - r * 0.05), (cx + r * 1.1, cy - r * 0.05), (cx, cy + r * 1.0)], rt);
}

/// Hayot, ochlik, havo pufakchalari, zarbadan keyingi qizil chaqnash va buzish jarayoni.
fn draw_vitals(h: &mut Hud, l: &Layout, u: &UiState) {
    let s = l.s;
    if u.flash > 0.0 {
        h.rect(0.0, 0.0, l.w, l.h, rgba(200, 0, 0, (u.flash * 90.0) as u8));
    }
    let tot = 9.0 * l.slot + 8.0 * l.slot_gap;
    let step = (tot * 0.5 - 20.0 * s) / 10.0;
    let r = (step * 0.42).min(22.0 * s);
    let y = l.slot_y - 62.0 * s;
    for i in 0..10 {
        let frac = ((u.health - i as f32 * 2.0) / 2.0).clamp(0.0, 1.0);
        let frac = if frac >= 1.0 { 1.0 } else if frac >= 0.5 { 0.5 } else { 0.0 };
        heart(h, l.slot_x0 + r + i as f32 * step, y, r, frac);
    }
    // Ochlik: o'ng tomonda, o'ngdan chapga
    for i in 0..10 {
        let f = ((u.hunger - i as f32 * 2.0) / 2.0).clamp(0.0, 1.0);
        let x = l.slot_x0 + tot - r - i as f32 * step;
        h.circle(x, y, r * 0.85, rgba(60, 40, 20, 200));
        if f > 0.0 {
            h.circle(x, y, r * 0.85 * f.max(0.5), rgba(232, 150, 40, 255));
        }
    }
    // Havo (suv ostida)
    if u.air < 9.99 {
        let n = (u.air / 10.0 * 10.0).ceil() as i32;
        for i in 0..n.clamp(0, 10) {
            let x = l.slot_x0 + tot - r - i as f32 * step;
            h.ring(x, y - 2.0 * r - 6.0 * s, r * 0.7, 4.0 * s, rgba(140, 210, 255, 255));
        }
    }
    // Buzish jarayoni
    if u.break_frac > 0.01 {
        let (cx, cy) = (l.w * 0.5, l.h * 0.5);
        h.ring(cx, cy, 52.0 * s, 8.0 * s, rgba(255, 255, 255, 60));
        let n = (u.break_frac.clamp(0.0, 1.0) * 24.0) as i32;
        for k in 0..n {
            let a = -std::f32::consts::FRAC_PI_2 + k as f32 / 24.0 * std::f32::consts::TAU;
            h.circle(cx + 48.0 * s * a.cos(), cy + 48.0 * s * a.sin(), 6.0 * s, SIG);
        }
    }
}

fn draw_dead(h: &mut Hud, l: &Layout) {
    let s = l.s;
    h.rect(0.0, 0.0, l.w, l.h, rgba(90, 0, 10, 190));
    h.text_c(l.w * 0.5, l.h * 0.36, 110.0 * s, "SIZ HALOK BO'LDINGIZ", WHITE);
    let (x, y, w, bh) = l.respawn_btn();
    button(h, x, y, w, bh, s, "QAYTA TUG'ILISH", SIG, WHITE);
}

fn draw_pause(h: &mut Hud, l: &Layout, u: &UiState) {
    let s = l.s;
    h.rect(0.0, 0.0, l.w, l.h, rgba(0, 0, 0, 150));
    let (px, py, pw, ph) = l.panel();
    h.rrect(px, py, pw, ph, 56.0 * s, rgba(18, 16, 28, 245));
    h.rrect_line(px, py, pw, ph, 56.0 * s, 4.0, rgba(255, 255, 255, 60));
    h.text_c(px + pw * 0.5, py + 85.0 * s, 70.0 * s, "PAUZA", WHITE);
    let dark = rgba(40, 38, 58, 255);
    let b = |h: &mut Hud, a: PauseAct, label: &str, fill: Col, fg: Col| {
        let (x, y, w, bh) = l.pause_btn(a);
        button(h, x, y, w, bh, s, label, fill, fg);
    };
    b(h, PauseAct::Resume, "DAVOM ETISH", SIG, WHITE);
    b(h, PauseAct::Mode, if u.survival { "REJIM: OMON QOLISH" } else { "REJIM: IJODIY" }, dark, WHITE);
    b(h, PauseAct::Quality, &format!("SIFAT: {}", QUALITY_NAMES[u.quality.min(2)]), dark, WHITE);
    b(h, PauseAct::Day, "KUN", dark, WHITE);
    b(h, PauseAct::Sunset, "KECH", dark, WHITE);
    b(h, PauseAct::Night, "TUN", dark, WHITE);
    b(h, PauseAct::SensDown, "-", dark, WHITE);
    b(h, PauseAct::SensUp, "+", dark, WHITE);
    let (sx, sy, sw, sh) = l.pause_btn(PauseAct::SensDown);
    let (ux, _, uw, _) = l.pause_btn(PauseAct::SensUp);
    h.text_c((sx + sw + ux) * 0.5 + uw * 0.0, sy + sh * 0.5, 42.0 * s, &format!("SEZGIRLIK {:.1}", u.sens), WHITE);
}

fn draw_inv(h: &mut Hud, l: &Layout, u: &UiState) {
    let s = l.s;
    h.rect(0.0, 0.0, l.w, l.h, rgba(0, 0, 0, 150));
    let (px, py, pw, ph) = l.inv_panel();
    h.rrect(px, py, pw, ph, 56.0 * s, rgba(18, 16, 28, 245));
    h.rrect_line(px, py, pw, ph, 56.0 * s, 4.0, rgba(255, 255, 255, 60));
    h.text(px + 50.0 * s, py + 40.0 * s, 64.0 * s, "BLOKLAR", WHITE);
    let c = l.inv_close();
    h.rrect(c.cx - c.r, c.cy - c.r, c.r * 2.0, c.r * 2.0, c.r * 0.45, rgba(40, 38, 58, 255));
    h.text_c(c.cx, c.cy, 56.0 * s, "X", WHITE);
    for (i, &b) in PLACEABLE.iter().enumerate() {
        let (x, y, w, ch) = l.inv_card(i);
        let cur = u.held == b as Item;
        h.rrect(x, y, w, ch, 30.0 * s, if cur { WHITE } else { rgba(40, 38, 58, 255) });
        h.cube_icon(x + w * 0.5, y + ch * 0.38, ch * 0.2, b);
        h.text_c(x + w * 0.5, y + ch * 0.84, 26.0 * s, blocks::name(b), if cur { BLACK } else { WHITE });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tugmalar_ekranga_sigadi() {
        for &(w, h) in &[(2400.0f32, 1080.0f32), (1600.0, 720.0), (1920.0, 1080.0), (2340.0, 1080.0), (1280.0, 720.0)] {
            let l = Layout::new(w, h);
            for sq in [l.jump, l.fly, l.down, l.speed, l.brk, l.place, l.pause, l.inv] {
                assert!(sq.cx - sq.r >= 0.0 && sq.cx + sq.r <= w && sq.cy - sq.r >= 0.0 && sq.cy + sq.r <= h, "{w}x{h}: {:?}", sq);
            }
            let (x9, _, w9, _) = l.slot_rect(8);
            assert!(x9 + w9 <= w);
            assert!(l.slot_x0 >= 0.0);
        }
    }

    #[test]
    fn hotbar_tugmalar_bilan_tutashmaydi_keng_ekranda() {
        for &(w, h) in &[(2400.0f32, 1080.0f32), (1600.0, 720.0), (2340.0, 1080.0)] {
            let l = Layout::new(w, h);
            let (x9, _, w9, _) = l.slot_rect(8);
            assert!(x9 + w9 < l.brk.cx - l.brk.r, "{w}x{h}: hotbar {} break {}", x9 + w9, l.brk.cx - l.brk.r);
        }
    }

    #[test]
    fn teginish_aniqlash() {
        let l = Layout::new(2400.0, 1080.0);
        assert_eq!(l.button_at(l.place.cx, l.place.cy, false, false), Some(Btn::Place));
        assert_eq!(l.button_at(l.brk.cx, l.brk.cy, false, false), Some(Btn::Break));
        assert_eq!(l.button_at(l.down.cx, l.down.cy, false, false), None);
        assert_eq!(l.button_at(l.down.cx, l.down.cy, true, false), Some(Btn::Down));
        assert_eq!(l.button_at(1200.0, 300.0, true, false), None);
        let (x, y, w, _) = l.slot_rect(3);
        assert_eq!(l.slot_at(x + w * 0.5, y + w * 0.5), Some(3));
        assert_eq!(l.slot_at(5.0, 5.0), None);
        // omon qolish rejimida uchish tugmalari yo'q
        assert_eq!(l.button_at(l.fly.cx, l.fly.cy, false, true), None);
        assert_eq!(l.button_at(l.fly.cx, l.fly.cy, false, false), Some(Btn::Fly));
        assert_eq!(l.button_at(l.brk.cx, l.brk.cy, false, true), Some(Btn::Break));
    }

    #[test]
    fn menyu_va_blok_oynasi_tugmalari() {
        for &(w, h) in &[(2400.0f32, 1080.0f32), (1600.0, 720.0)] {
            let l = Layout::new(w, h);
            for a in [PauseAct::Resume, PauseAct::Mode, PauseAct::Quality, PauseAct::Day, PauseAct::Sunset, PauseAct::Night, PauseAct::SensDown, PauseAct::SensUp] {
                let (x, y, bw, bh) = l.pause_btn(a);
                assert_eq!(l.pause_at(x + bw * 0.5, y + bh * 0.5), Some(a));
                let (px, py, pw, ph) = l.panel();
                assert!(x >= px && x + bw <= px + pw && y + bh <= py + ph, "{a:?} panelga sig'maydi");
            }
            let c = l.craft_panel();
            assert!(c.0 >= 0.0 && c.1 >= 0.0 && c.0 + c.2 <= w && c.1 + c.3 <= h, "yasash oynasi ekranga sig'maydi");
            for st in [Station::Hand, Station::Table, Station::Furnace, Station::Enchant, Station::Chest] {
                for i in 0..st.count() {
                    let (x, y) = l.craft_grid_slot(st, i);
                    assert_eq!(l.craft_at(x + 5.0, y + 5.0, st), Some(CraftHit::Grid(i)));
                }
            }
            for i in 0..items::SLOTS {
                let (x, y) = l.craft_inv_slot(i);
                assert_eq!(l.craft_at(x + 5.0, y + 5.0, Station::Table), Some(CraftHit::Inv(i)));
                assert!(x + 104.0 * l.s <= c.0 + c.2 && y + 104.0 * l.s <= c.1 + c.3);
            }
            let (rx, ry) = l.craft_result_slot();
            assert_eq!(l.craft_at(rx + 5.0, ry + 5.0, Station::Hand), Some(CraftHit::Result));
            assert_eq!(l.craft_at(l.craft_close().cx, l.craft_close().cy, Station::Hand), Some(CraftHit::Close));
            for i in 0..PLACEABLE.len() {
                let (x, y, cw, ch) = l.inv_card(i);
                assert_eq!(l.inv_at(x + cw * 0.5, y + ch * 0.5), Some(i));
                let (px, py, pw, ph) = l.inv_panel();
                assert!(x + cw <= px + pw && y + ch <= py + ph, "karta {i} panelga sig'maydi");
            }
        }
    }
}
