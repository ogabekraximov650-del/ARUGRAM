//! uzcoders-ui — deklarativ elementlar daraxti, joylashtirish, chizish, scroll, matn kiritish va navigatsiya.

mod element;
mod nav;
mod scroll;

use element::{Callback, FieldCfg, Kind};
use std::rc::Rc;
use uzcoders_core::{
    global_revision, Color, EdgeInsets, Point, PointerEvent, PointerKind, Rect, Size,
};
use uzcoders_layout::{Dim, Tree};
use uzcoders_render::{Canvas, DrawCmd, Path};
use uzcoders_text::Font;

pub use element::{
    button, column, field, gap_box, grid, icon, lazy_column, row, scroll_column, spacer, text,
    Element,
};
pub use nav::Navigator;
pub use scroll::ScrollState;
pub use uzcoders_layout::{Align as Alignment, Justify as Arrange};

const BLINK_MS: u64 = 500;
const SLOP_DP: f32 = 8.0;

enum ItemKind {
    Container,
    Text(String, f32, Color, u16, Option<f32>, f32),
    Icon(Rc<Path>, Color),
    Field(Box<FieldCfg>),
    ScrollEnd,
}

struct Item {
    node: usize,
    kind: ItemKind,
    bg: Option<Color>,
    radius: f32,
    border: Option<(f32, Color)>,
    hit: Option<usize>,
    scroll: Option<ScrollState>,
}

struct Hit {
    rect: Rect,
    cb: Callback,
}

struct Region {
    rect: Rect,
    state: ScrollState,
}

struct FieldInfo {
    rect: Rect,
    cfg: Box<FieldCfg>,
    shown: String,
    text_x: f32,
}

struct Touch {
    start: Point,
    last: Point,
    field: Option<usize>,
    region: Option<usize>,
    dragging: bool,
    /// So'nggi (vaqt ms, y) namunalari: tezlikni baholash uchun.
    samples: Vec<(u64, f32)>,
}

/// Ilova ish vaqti: elementlarni quradi, joylaydi, chizish ro'yxatini beradi va kiritishni yo'naltiradi.
pub struct Ui {
    font: Rc<Font>,
    faces: Vec<(u16, Rc<Font>)>,
    insets_px: EdgeInsets,
    kb_serial: u64,
    build: Box<dyn FnMut() -> Element>,
    hits: Vec<Hit>,
    fields: Vec<FieldInfo>,
    regions: Vec<Region>,
    scrolls: Vec<ScrollState>,
    pressed: Option<usize>,
    last_rev: Option<u64>,
    dirty: bool,
    scale: f32,
    now: u64,
    last_frame_now: Option<u64>,
    touch: Option<Touch>,
    focus: Option<usize>,
    caret: usize,
    caret_epoch: u64,
    last_blink: u64,
    back: Option<Rc<dyn Fn() -> bool>>,
}

impl Ui {
    pub fn new(font: Rc<Font>, build: impl FnMut() -> Element + 'static) -> Ui {
        Ui {
            font,
            faces: Vec::new(),
            insets_px: EdgeInsets::ZERO,
            kb_serial: 0,
            build: Box::new(build),
            hits: Vec::new(),
            fields: Vec::new(),
            regions: Vec::new(),
            scrolls: Vec::new(),
            pressed: None,
            last_rev: None,
            dirty: true,
            scale: 1.0,
            now: 0,
            last_frame_now: None,
            touch: None,
            focus: None,
            caret: 0,
            caret_epoch: 0,
            last_blink: 0,
            back: None,
        }
    }

    pub fn font(&self) -> Rc<Font> {
        self.font.clone()
    }

    /// Barcha shrift yuzlari: (qalinlik, shrift); birinchisi — asosiy (400).
    pub fn faces(&self) -> Vec<(u16, Rc<Font>)> {
        let mut v = vec![(400, self.font.clone())];
        v.extend(self.faces.iter().cloned());
        v
    }

    /// Qo'shimcha shrift yuzi (masalan `add_font(700, bold)`). Oddiy (400) yuz `Ui::new` da beriladi.
    pub fn add_font(&mut self, weight: u16, font: Rc<Font>) {
        self.faces.push((weight, font));
        self.dirty = true;
    }

    fn face(&self, weight: u16) -> Rc<Font> {
        let mut best = (&self.font, (400i32 - weight as i32).abs());
        for (w, f) in &self.faces {
            let d = (*w as i32 - weight as i32).abs();
            if d < best.1 {
                best = (f, d);
            }
        }
        best.0.clone()
    }

    /// Shu `Ui` shriftlari bilan tayyor canvas.
    pub fn canvas(&self, width: usize, height: usize) -> Canvas {
        let mut c = Canvas::new(width, height, self.font.clone());
        for (w, f) in &self.faces {
            c.add_face(*w, f.clone());
        }
        c
    }

    /// Tizim panellari chekinishlari (piksel): status bar tepada, navigatsiya paneli pastda. Ilova ularni
    /// `uzcoders_core::safe_insets()` (dp) orqali oladi.
    pub fn set_insets(&mut self, insets_px: EdgeInsets) {
        if insets_px != self.insets_px {
            self.insets_px = insets_px;
            self.dirty = true;
        }
    }

    /// Har safar kiritish maydoniga bosilganda oshadi: qobiq klaviaturani qayta ko'rsatishi uchun.
    pub fn keyboard_serial(&self) -> u64 {
        self.kb_serial
    }

    /// Zichlik koeffitsienti (dp -> piksel). Barcha o'lchamlar shunga ko'paytiriladi.
    pub fn set_scale(&mut self, scale: f32) {
        if (scale - self.scale).abs() > f32::EPSILON {
            self.scale = scale.max(0.1);
            self.dirty = true;
        }
    }

    /// Joriy vaqt (millisekund, monoton). Kursor miltillashi va inersiya shunga tayanadi; qobiq har tsiklda chaqiradi.
    pub fn set_time(&mut self, ms: u64) {
        self.now = ms;
    }

    /// "Orqaga" bosilganda chaqiriladi; `true` — qayta ishlandi (ilova yopilmaydi).
    pub fn set_back_handler(&mut self, f: impl Fn() -> bool + 'static) {
        self.back = Some(Rc::new(f));
    }

    fn blink_phase(&self) -> u64 {
        self.now.saturating_sub(self.caret_epoch) / BLINK_MS
    }

    /// Qayta chizish kerakmi (holat o'zgardi, inersiya, kursor miltillashi)?
    pub fn needs_frame(&self) -> bool {
        self.dirty
            || self.last_rev != Some(global_revision())
            || self.scrolls.iter().any(|s| s.velocity() != 0.0)
            || (self.focus.is_some() && self.blink_phase() != self.last_blink)
    }

    /// Keyingi uyg'onishgacha necha ms kutish mumkin (`None` — hodisagacha cheksiz).
    pub fn next_wakeup_ms(&self) -> Option<u64> {
        if self.scrolls.iter().any(|s| s.velocity() != 0.0) {
            return Some(0);
        }
        self.focus
            .map(|_| BLINK_MS - self.now.saturating_sub(self.caret_epoch) % BLINK_MS)
    }

    /// Klaviatura ko'rsatilishi kerakmi (kiritish maydoni fokusda).
    pub fn wants_keyboard(&self) -> bool {
        self.focus.is_some()
    }

    /// Kadrni quradi: `size` — oyna o'lchami (piksel). Chizish buyruqlari qaytadi.
    pub fn frame(&mut self, size: Size) -> Vec<DrawCmd> {
        self.last_rev = Some(global_revision());
        self.dirty = false;
        let dt = match self.last_frame_now {
            Some(t) => (self.now.saturating_sub(t) as f32 / 1000.0).min(0.05),
            None => 0.0,
        };
        self.last_frame_now = Some(self.now);
        self.step_flings(dt);
        self.last_blink = self.blink_phase();
        uzcoders_core::set_density(self.scale);
        let k = self.scale;
        let i = self.insets_px;
        uzcoders_core::set_safe_insets(EdgeInsets::new(
            i.left / k,
            i.top / k,
            i.right / k,
            i.bottom / k,
        ));

        let root = (self.build)();
        let mut tree = Tree::new();
        let mut items = Vec::new();
        let mut cbs = Vec::new();
        let root_id = self.add(root, &mut tree, &mut items, &mut cbs);
        tree.compute(root_id, size);

        self.hits.clear();
        self.fields.clear();
        self.regions.clear();
        self.scrolls.clear();
        let mut cmds = Vec::new();
        let mut shift = 0.0f32;
        let mut shifts: Vec<f32> = Vec::new();
        let mut clips: Vec<Rect> = Vec::new();
        let caret_on = self.blink_phase() % 2 == 0;

        for it in items {
            if let ItemKind::ScrollEnd = it.kind {
                cmds.push(DrawCmd::PopClip);
                clips.pop();
                shift = shifts.pop().unwrap_or(0.0);
                continue;
            }
            let r = tree.rect(it.node).translate(0.0, shift);
            let clip = clips.last().copied();
            let eff = clip.map_or(r, |c| r.intersect(&c));
            let visible = !eff.is_empty();
            if let Some(h) = it.hit {
                self.hits.push(Hit {
                    rect: eff,
                    cb: cbs[h].clone(),
                });
            }
            if visible {
                if let Some(mut c) = it.bg {
                    if it.hit.is_some() && it.hit == self.pressed {
                        c = c.darken(0.2);
                    }
                    cmds.push(DrawCmd::RoundRect {
                        rect: r,
                        radius: it.radius,
                        color: c,
                    });
                }
            }
            if visible && !matches!(it.kind, ItemKind::Field(_)) {
                if let Some((w, c)) = it.border {
                    cmds.push(DrawCmd::RoundRectStroke {
                        rect: r,
                        radius: it.radius,
                        width: w,
                        color: c,
                    });
                }
            }
            match it.kind {
                ItemKind::Text(t, sz, color, weight, lh, sp) if visible => {
                    let font = self.face(weight);
                    let lines = font.wrap_spaced(&t, sz, Some(r.w + 0.5), sp);
                    let (lhp, base) = line_metrics(&font, sz, lh);
                    for (i, l) in lines.into_iter().enumerate() {
                        cmds.push(DrawCmd::Text {
                            x: r.x,
                            baseline: r.y + base + lhp * i as f32,
                            text: l,
                            size: sz,
                            color,
                            weight,
                            spacing: sp,
                        });
                    }
                }
                ItemKind::Icon(path, color) if visible => {
                    cmds.push(DrawCmd::Path {
                        path,
                        view: 24.0,
                        rect: r,
                        color,
                    });
                }
                ItemKind::Field(cfg) => {
                    let pad = it_padding(&tree, it.node);
                    let focused = self.focus == Some(cfg.state.id());
                    let raw = cfg.state.get();
                    let shown = if cfg.password {
                        "\u{2022}".repeat(raw.chars().count())
                    } else {
                        raw
                    };
                    let inner_x = r.x + pad.left;
                    let inner_w = (r.w - pad.horizontal()).max(0.0);
                    let caret_idx = self.caret.min(shown.chars().count());
                    let prefix: String = shown.chars().take(caret_idx).collect();
                    let caret_x = self.font.text_width_spaced(&prefix, cfg.size, cfg.spacing);
                    let shift_x = if focused {
                        (caret_x - inner_w + 2.0 * self.scale).max(0.0)
                    } else {
                        0.0
                    };
                    let text_x = inner_x - shift_x;
                    if visible {
                        if let Some((w, c)) = it.border {
                            let c = if focused { cfg.accent } else { c };
                            let w = if focused { w * 2.0 } else { w };
                            cmds.push(DrawCmd::RoundRectStroke {
                                rect: r,
                                radius: it.radius,
                                width: w,
                                color: c,
                            });
                        }
                        let (lh, base) = line_metrics(&self.font, cfg.size, cfg.line_height);
                        let top = r.y + pad.top;
                        let has_label = !cfg.label.is_empty();
                        let floated = has_label && (focused || !shown.is_empty());
                        let label_color = if focused { cfg.accent } else { cfg.hint_color };
                        if floated {
                            // Chegarada turgan kichik sarlavha: ostidagi chiziq qirqiladi.
                            let ls = cfg.size * 0.75;
                            let (llh, lbase) = line_metrics(&self.font, ls, None);
                            let lw = self
                                .font
                                .text_width_spaced(&cfg.label, ls, cfg.spacing * 0.8);
                            let ltop = r.y - llh * 0.5;
                            let pad_x = 4.0 * self.scale;
                            cmds.push(DrawCmd::RoundRect {
                                rect: Rect::new(inner_x - pad_x, ltop, lw + 2.0 * pad_x, llh),
                                radius: 0.0,
                                color: cfg.label_bg,
                            });
                            cmds.push(DrawCmd::Text {
                                x: inner_x,
                                baseline: ltop + lbase,
                                text: cfg.label.clone(),
                                size: ls,
                                color: label_color,
                                weight: 400,
                                spacing: cfg.spacing * 0.8,
                            });
                        }
                        cmds.push(DrawCmd::PushClip(Rect::new(inner_x, r.y, inner_w, r.h)));
                        if shown.is_empty() {
                            let (t, c) = if has_label && !floated {
                                (cfg.label.clone(), cfg.hint_color)
                            } else if !has_label || focused {
                                (cfg.hint.clone(), cfg.hint_color)
                            } else {
                                (String::new(), cfg.hint_color)
                            };
                            if !t.is_empty() {
                                cmds.push(DrawCmd::Text {
                                    x: inner_x,
                                    baseline: top + base,
                                    text: t,
                                    size: cfg.size,
                                    color: c,
                                    weight: 400,
                                    spacing: cfg.spacing,
                                });
                            }
                        } else {
                            cmds.push(DrawCmd::Text {
                                x: text_x,
                                baseline: top + base,
                                text: shown.clone(),
                                size: cfg.size,
                                color: cfg.color,
                                weight: 400,
                                spacing: cfg.spacing,
                            });
                        }
                        if focused && caret_on {
                            cmds.push(DrawCmd::RoundRect {
                                rect: Rect::new(
                                    text_x + caret_x,
                                    top,
                                    (1.5 * self.scale).max(1.0),
                                    lh,
                                ),
                                radius: 0.0,
                                color: cfg.accent,
                            });
                        }
                        cmds.push(DrawCmd::PopClip);
                    }
                    self.fields.push(FieldInfo {
                        rect: eff,
                        cfg,
                        shown,
                        text_x,
                    });
                }
                _ => {}
            }
            if let Some(sc) = it.scroll {
                let pad = it_padding(&tree, it.node);
                // Pastdagi padding ham ko'rinishi uchun: kontent uchi = padding.top + extent + padding.bottom.
                let max = tree.content_extent(it.node) + pad.vertical() - r.h;
                sc.set_max(max);
                if sc.set_viewport(r.h) {
                    self.dirty = true;
                }
                self.regions.push(Region {
                    rect: eff,
                    state: sc.clone(),
                });
                let off = sc.offset();
                self.scrolls.push(sc);
                cmds.push(DrawCmd::PushClip(r));
                clips.push(eff);
                shifts.push(shift);
                shift -= off;
            }
        }
        if let Some(id) = self.focus {
            if !self.fields.iter().any(|f| f.cfg.state.id() == id) {
                self.focus = None;
            }
        }
        cmds
    }

    fn step_flings(&mut self, dt: f32) {
        if dt <= 0.0 {
            return;
        }
        for s in &self.scrolls {
            let v = s.velocity();
            if v == 0.0 {
                continue;
            }
            let before = s.offset();
            s.raw_set(before + v * dt);
            let after = s.offset();
            let hit_edge = (after <= 0.0 && v < 0.0) || (after >= s.max_offset() && v > 0.0);
            let nv = v * (-dt * 3.0).exp();
            s.set_velocity(if hit_edge || nv.abs() < 20.0 { 0.0 } else { nv });
        }
    }

    fn scale_element(&self, el: &mut Element) {
        let k = self.scale;
        if k == 1.0 {
            return;
        }
        let ins = |e: EdgeInsets| EdgeInsets::new(e.left * k, e.top * k, e.right * k, e.bottom * k);
        let dim = |d: Dim| {
            if let Dim::Px(v) = d {
                Dim::Px(v * k)
            } else {
                d
            }
        };
        el.style.padding = ins(el.style.padding);
        el.style.margin = ins(el.style.margin);
        el.style.gap *= k;
        el.style.width = dim(el.style.width);
        el.style.height = dim(el.style.height);
        el.radius *= k;
        if let Some((w, _)) = &mut el.border {
            *w *= k;
        }
        match &mut el.kind {
            Kind::Text {
                size,
                line_height,
                spacing,
                ..
            } => {
                *spacing *= k;
                *size *= k;
                *line_height = line_height.map(|h| h * k);
            }
            Kind::Icon { size, .. } => *size *= k,
            Kind::Field(f) => {
                f.spacing *= k;
                f.size *= k;
                f.line_height = f.line_height.map(|h| h * k);
            }
            Kind::Container => {}
        }
    }

    fn add(
        &self,
        el: Element,
        tree: &mut Tree,
        items: &mut Vec<Item>,
        cbs: &mut Vec<Callback>,
    ) -> usize {
        let mut el = el;
        self.scale_element(&mut el);
        let slot = items.len();
        let hit = el.on_click.map(|cb| {
            cbs.push(cb);
            cbs.len() - 1
        });
        let has_scroll = el.scroll.is_some();
        items.push(Item {
            node: 0,
            kind: ItemKind::Container,
            bg: el.bg,
            radius: el.radius,
            border: el.border,
            hit,
            scroll: el.scroll,
        });
        let node = match el.kind {
            Kind::Container => {
                let ids: Vec<usize> = el
                    .children
                    .into_iter()
                    .map(|c| self.add(c, tree, items, cbs))
                    .collect();
                tree.add(el.style, ids)
            }
            Kind::Text {
                text,
                size,
                color,
                weight,
                line_height,
                spacing,
            } => {
                items[slot].kind =
                    ItemKind::Text(text.clone(), size, color, weight, line_height, spacing);
                let font = self.face(weight);
                // Joylashtirish bir tugunni bir necha marta o'lchaydi: oxirgi natija eslab qolinadi.
                let memo: std::cell::Cell<Option<(f32, Size)>> = std::cell::Cell::new(None);
                tree.add_leaf(el.style, move |w| {
                    if let Some((mw, sz)) = memo.get() {
                        if mw == w {
                            return sz;
                        }
                    }
                    let max = if w.is_finite() { Some(w) } else { None };
                    let (tw, _, lines) = font.measure_spaced(&text, size, max, spacing);
                    let (lh, _) = line_metrics(&font, size, line_height);
                    let sz = Size::new(tw.ceil(), (lh * lines.len() as f32).ceil());
                    memo.set(Some((w, sz)));
                    sz
                })
            }
            Kind::Icon { path, size, color } => {
                items[slot].kind = ItemKind::Icon(path, color);
                tree.add_leaf(el.style, move |_| Size::new(size, size))
            }
            Kind::Field(cfg) => {
                let font = self.font.clone();
                let (size, minw) = (cfg.size, 100.0 * self.scale);
                let hint_w = font
                    .text_width_spaced(&cfg.hint, size, cfg.spacing)
                    .max(font.text_width_spaced(&cfg.label, size, cfg.spacing));
                let lh = cfg.line_height;
                items[slot].kind = ItemKind::Field(cfg);
                tree.add_leaf(el.style, move |_| {
                    let (h, _) = line_metrics(&font, size, lh);
                    Size::new(hint_w.max(minw).ceil(), h.ceil())
                })
            }
        };
        items[slot].node = node;
        if has_scroll {
            items.push(Item {
                node,
                kind: ItemKind::ScrollEnd,
                bg: None,
                radius: 0.0,
                border: None,
                hit: None,
                scroll: None,
            });
        }
        node
    }

    fn hit_test(&self, p: Point) -> Option<usize> {
        (0..self.hits.len())
            .rev()
            .find(|&i| self.hits[i].rect.contains(p))
    }

    fn field_at(&self, p: Point) -> Option<usize> {
        (0..self.fields.len())
            .rev()
            .find(|&i| self.fields[i].rect.contains(p))
    }

    fn region_at(&self, p: Point) -> Option<usize> {
        (0..self.regions.len())
            .rev()
            .find(|&i| self.regions[i].rect.contains(p))
    }

    fn caret_from_x(&self, fi: usize, x: f32) -> usize {
        let f = &self.fields[fi];
        let mut acc = f.text_x;
        for (i, ch) in f.shown.chars().enumerate() {
            let a = self.font.char_advance(ch, f.cfg.size) + f.cfg.spacing;
            if x < acc + a * 0.5 {
                return i;
            }
            acc += a;
        }
        f.shown.chars().count()
    }

    fn focus_field(&mut self, fi: usize, x: f32) {
        let id = self.fields[fi].cfg.state.id();
        self.caret = self.caret_from_x(fi, x);
        self.focus = Some(id);
        self.kb_serial += 1;
        self.caret_epoch = self.now;
        self.dirty = true;
    }

    /// Teginish/sichqoncha hodisasini qayta ishlaydi.
    pub fn pointer(&mut self, e: PointerEvent) {
        let p = e.pos;
        match e.kind {
            PointerKind::Down => {
                self.pressed = self.hit_test(p);
                let region = self.region_at(p);
                if let Some(ri) = region {
                    self.regions[ri].state.set_velocity(0.0);
                }
                self.touch = Some(Touch {
                    start: p,
                    last: p,
                    field: self.field_at(p),
                    region,
                    dragging: false,
                    samples: vec![(if e.time_ms > 0 { e.time_ms } else { self.now }, p.y)],
                });
                self.dirty = true;
            }
            PointerKind::Move => {
                let slop = SLOP_DP * self.scale;
                let mut unpress = false;
                if let Some(t) = &mut self.touch {
                    if let Some(ri) = t.region {
                        let dy = p.y - t.last.y;
                        if !t.dragging && (p.y - t.start.y).abs() > slop {
                            t.dragging = true;
                            unpress = true;
                        }
                        if t.dragging {
                            let st = &self.regions[ri].state;
                            st.raw_set(st.offset() - dy);
                            let tm = if e.time_ms > 0 { e.time_ms } else { self.now };
                            t.samples.push((tm, p.y));
                            if t.samples.len() > 24 {
                                t.samples.remove(0);
                            }
                            self.dirty = true;
                        }
                    }
                    t.last = p;
                }
                if unpress {
                    self.pressed = None;
                    self.dirty = true;
                }
            }
            PointerKind::Up => {
                let t = self.touch.take();
                self.pressed = None;
                self.dirty = true;
                let Some(t) = t else { return };
                if t.dragging {
                    if let Some(ri) = t.region {
                        // Tezlik: oxirgi 100 ms dagi namunalar bo'yicha (VelocityTracker kabi).
                        // Barmoq to'xtab turib qo'yib yuborilsa — inersiya yo'q.
                        let up_t = if e.time_ms > 0 { e.time_ms } else { self.now };
                        let v = match t.samples.last() {
                            Some(&(lt, ly)) if up_t.saturating_sub(lt) <= 80 => {
                                let first = t
                                    .samples
                                    .iter()
                                    .find(|(tm, _)| lt.saturating_sub(*tm) <= 100)
                                    .copied()
                                    .unwrap_or((lt, ly));
                                let dt = lt.saturating_sub(first.0) as f32 / 1000.0;
                                if dt > 0.0 {
                                    -(ly - first.1) / dt
                                } else {
                                    0.0
                                }
                            }
                            _ => 0.0,
                        };
                        let v = v.clamp(-12000.0, 12000.0);
                        self.regions[ri]
                            .state
                            .set_velocity(if v.abs() > 50.0 { v } else { 0.0 });
                    }
                    return;
                }
                if let Some(fi) = self.field_at(p).filter(|&f| Some(f) == t.field) {
                    self.focus_field(fi, p.x);
                    return;
                }
                self.focus = None;
                if let Some(i) = self.hit_test(p) {
                    let cb = self.hits[i].cb.clone();
                    cb();
                }
            }
            PointerKind::Cancel => {
                self.touch = None;
                self.pressed = None;
                self.dirty = true;
            }
        }
    }

    fn focused_index(&self) -> Option<usize> {
        let id = self.focus?;
        self.fields.iter().position(|f| f.cfg.state.id() == id)
    }

    /// Fokusdagi maydon matnini tahrirlaydi; `f` belgilar va kursor indeksini o'zgartiradi.
    fn edit(&mut self, f: impl FnOnce(&mut Vec<char>, &mut usize, &FieldCfg) -> bool) -> bool {
        let Some(fi) = self.focused_index() else {
            return false;
        };
        let cfg = &self.fields[fi].cfg;
        let mut chars: Vec<char> = cfg.state.get().chars().collect();
        let mut caret = self.caret.min(chars.len());
        if !f(&mut chars, &mut caret, cfg) {
            return false;
        }
        let mut text: String = chars.iter().collect();
        if let Some(fmt) = cfg.format {
            let significant = chars[..caret].iter().filter(|c| **c != ' ').count();
            text = fmt(&text);
            let mut seen = 0;
            caret = text.chars().count();
            for (i, c) in text.chars().enumerate() {
                if seen == significant {
                    caret = i;
                    break;
                }
                if c != ' ' {
                    seen += 1;
                }
            }
            if seen == significant && significant > 0 && caret == text.chars().count() {
                caret = text.chars().count();
            }
        }
        cfg.state.set(text);
        self.caret = caret;
        self.caret_epoch = self.now;
        self.dirty = true;
        true
    }

    /// Fokusdagi maydonga belgi kiritadi. Maydon bo'lmasa `false`.
    pub fn key_char(&mut self, c: char) -> bool {
        if c.is_control() {
            return false;
        }
        self.edit(|chars, caret, cfg| {
            if let Some(p) = cfg.filter {
                if !p(c) {
                    return true; // belgi rad etildi, lekin klaviatura hodisasi qayta ishlangan
                }
            }
            if cfg.max_len > 0 {
                let n = if cfg.format.is_some() {
                    chars.iter().filter(|x| **x != ' ').count()
                } else {
                    chars.len()
                };
                if n >= cfg.max_len {
                    return true;
                }
            }
            chars.insert(*caret, c);
            *caret += 1;
            true
        })
    }

    /// Kursor oldidagi belgini o'chiradi.
    pub fn key_backspace(&mut self) -> bool {
        self.edit(|chars, caret, _| {
            // Formatlovchi probelni emas, undan oldingi belgini o'chiramiz.
            while *caret > 0 && chars[*caret - 1] == ' ' {
                *caret -= 1;
            }
            if *caret > 0 {
                chars.remove(*caret - 1);
                *caret -= 1;
            }
            true
        })
    }

    /// "Enter": `on_submit` chaqiriladi.
    pub fn key_enter(&mut self) -> bool {
        let Some(fi) = self.focused_index() else {
            return false;
        };
        if let Some(cb) = self.fields[fi].cfg.on_submit.clone() {
            cb();
        }
        true
    }

    /// "Orqaga": avval fokus olinadi, keyin ilovaning `set_back_handler`i chaqiriladi.
    pub fn key_back(&mut self) -> bool {
        if self.focus.take().is_some() {
            self.dirty = true;
            return true;
        }
        match self.back.clone() {
            Some(b) => b(),
            None => false,
        }
    }
}

/// (qator balandligi, qator qutisi tepasidan asos chizig'igacha masofa). `lh` berilsa Compose kabi proporsional bo'linadi.
fn line_metrics(font: &Font, size: f32, lh: Option<f32>) -> (f32, f32) {
    let asc = font.ascent_px(size);
    match lh {
        None => (font.line_height(size), asc),
        Some(h) => {
            let desc = -(font.descent as f32) * size / font.units_per_em as f32;
            let total = (asc + desc).max(0.001);
            (h, (h - total) * asc / total + asc)
        }
    }
}

fn it_padding(tree: &Tree, node: usize) -> EdgeInsets {
    tree.padding(node)
}

#[cfg(test)]
mod tests;
