//! uzcoders-ui — deklarativ elementlar daraxti, joylashtirish, chizish va bosishlarni qayta ishlash.

use std::rc::Rc;
use uzcoders_core::{global_revision, Color, EdgeInsets, PointerEvent, PointerKind, Rect, Size};
use uzcoders_layout::{Align, Axis, Dim, Justify, Style, Tree};
use uzcoders_render::DrawCmd;
use uzcoders_text::Font;

pub use uzcoders_layout::{Align as Alignment, Justify as Arrange};

type Callback = Rc<dyn Fn()>;

enum Kind {
    Container,
    Text {
        text: String,
        size: f32,
        color: Color,
    },
}

/// UI daraxti elementi. Har kadrda (holat o'zgarganda) qaytadan quriladi — arzon, chunki faqat tavsif.
pub struct Element {
    kind: Kind,
    style: Style,
    bg: Option<Color>,
    radius: f32,
    on_click: Option<Callback>,
    children: Vec<Element>,
}

impl Element {
    fn container(direction: Axis, children: Vec<Element>) -> Element {
        Element {
            kind: Kind::Container,
            style: Style {
                direction,
                ..Style::default()
            },
            bg: None,
            radius: 0.0,
            on_click: None,
            children,
        }
    }

    pub fn padding(mut self, v: f32) -> Self {
        self.style.padding = EdgeInsets::all(v);
        self
    }
    pub fn padding_xy(mut self, horizontal: f32, vertical: f32) -> Self {
        self.style.padding = EdgeInsets::symmetric(horizontal, vertical);
        self
    }
    pub fn margin(mut self, v: f32) -> Self {
        self.style.margin = EdgeInsets::all(v);
        self
    }
    pub fn gap(mut self, v: f32) -> Self {
        self.style.gap = v;
        self
    }
    pub fn bg(mut self, c: Color) -> Self {
        self.bg = Some(c);
        self
    }
    pub fn radius(mut self, r: f32) -> Self {
        self.radius = r;
        self
    }
    pub fn width(mut self, v: f32) -> Self {
        self.style.width = Dim::Px(v);
        self
    }
    pub fn height(mut self, v: f32) -> Self {
        self.style.height = Dim::Px(v);
        self
    }
    pub fn grow(mut self, v: f32) -> Self {
        self.style.grow = v;
        self
    }
    pub fn align(mut self, a: Align) -> Self {
        self.style.align_items = a;
        self
    }
    pub fn justify(mut self, j: Justify) -> Self {
        self.style.justify = j;
        self
    }
    pub fn center(self) -> Self {
        self.align(Align::Center).justify(Justify::Center)
    }
    pub fn on_click(mut self, f: impl Fn() + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
    /// Matn elementi uchun shrift o'lchami.
    pub fn size(mut self, s: f32) -> Self {
        if let Kind::Text { size, .. } = &mut self.kind {
            *size = s;
        }
        self
    }
    /// Matn elementi uchun rang.
    pub fn color(mut self, c: Color) -> Self {
        if let Kind::Text { color, .. } = &mut self.color_target() {
            *color = c;
        }
        self
    }
    fn color_target(&mut self) -> &mut Kind {
        &mut self.kind
    }
}

pub fn column(children: Vec<Element>) -> Element {
    Element::container(Axis::Column, children)
}

pub fn row(children: Vec<Element>) -> Element {
    Element::container(Axis::Row, children)
}

pub fn text(s: impl Into<String>) -> Element {
    Element {
        kind: Kind::Text {
            text: s.into(),
            size: 16.0,
            color: Color::BLACK,
        },
        style: Style::default(),
        bg: None,
        radius: 0.0,
        on_click: None,
        children: Vec::new(),
    }
}

/// Bo'sh joyni to'ldiradigan elastik element.
pub fn spacer() -> Element {
    Element::container(Axis::Column, Vec::new()).grow(1.0)
}

/// Tayyor tugma: yumaloq fon, o'rtada matn, bosilganda `f` chaqiriladi.
pub fn button(label: impl Into<String>, f: impl Fn() + 'static) -> Element {
    Element::container(
        Axis::Column,
        vec![text(label).color(Color::WHITE).size(18.0)],
    )
    .bg(Color::hex(0x2F80ED))
    .radius(10.0)
    .padding_xy(20.0, 12.0)
    .center()
    .on_click(f)
}

struct Item {
    node: usize,
    kind: ItemKind,
    bg: Option<Color>,
    radius: f32,
    hit: Option<usize>,
    clip: bool,
    end_clip: usize,
}

enum ItemKind {
    Container,
    Text(String, f32, Color),
}

struct Hit {
    rect: Rect,
    cb: Callback,
}

/// Ilova ish vaqti: elementlarni quradi, joylaydi, chizish ro'yxatini beradi va bosishlarni yo'naltiradi.
pub struct Ui {
    font: Rc<Font>,
    build: Box<dyn FnMut() -> Element>,
    hits: Vec<Hit>,
    pressed: Option<usize>,
    last_rev: Option<u64>,
    dirty: bool,
    scale: f32,
}

impl Ui {
    pub fn new(font: Rc<Font>, build: impl FnMut() -> Element + 'static) -> Ui {
        Ui {
            font,
            build: Box::new(build),
            hits: Vec::new(),
            pressed: None,
            last_rev: None,
            dirty: true,
            scale: 1.0,
        }
    }

    pub fn font(&self) -> Rc<Font> {
        self.font.clone()
    }

    /// Zichlik koeffitsienti (dp -> piksel). Barcha o'lchamlar (padding, shrift, radius...) shunga ko'paytiriladi.
    pub fn set_scale(&mut self, scale: f32) {
        if (scale - self.scale).abs() > f32::EPSILON {
            self.scale = scale.max(0.1);
            self.dirty = true;
        }
    }

    /// Holat o'zgargani yoki bosish tufayli qayta chizish kerakmi?
    pub fn needs_frame(&self) -> bool {
        self.dirty || self.last_rev != Some(global_revision())
    }

    /// Kadrni quradi: `size` — oyna o'lchami (piksel). Chizish buyruqlari qaytadi.
    pub fn frame(&mut self, size: Size) -> Vec<DrawCmd> {
        self.last_rev = Some(global_revision());
        self.dirty = false;
        let root = (self.build)();
        let mut tree = Tree::new();
        let mut items = Vec::new();
        let mut cbs = Vec::new();
        let root_id = self.add(root, &mut tree, &mut items, &mut cbs);
        tree.compute(root_id, size);
        self.hits.clear();
        let mut cmds = Vec::new();
        for it in &items {
            let r = tree.rect(it.node);
            if let Some(h) = it.hit {
                debug_assert_eq!(h, self.hits.len());
                self.hits.push(Hit {
                    rect: r,
                    cb: cbs[h].clone(),
                });
            }
            let _ = (it.clip, it.end_clip);
            if let Some(mut c) = it.bg {
                if it.hit.is_some() && it.hit == self.pressed {
                    c = c.darken(0.8);
                }
                cmds.push(DrawCmd::RoundRect {
                    rect: r,
                    radius: it.radius,
                    color: c,
                });
            }
            if let ItemKind::Text(t, sz, color) = &it.kind {
                let lines = self.font.wrap(t, *sz, Some(r.w + 0.5));
                let lh = self.font.line_height(*sz);
                let asc = self.font.ascent_px(*sz);
                for (i, l) in lines.into_iter().enumerate() {
                    cmds.push(DrawCmd::Text {
                        x: r.x,
                        baseline: r.y + asc + lh * i as f32,
                        text: l,
                        size: *sz,
                        color: *color,
                    });
                }
            }
        }
        cmds
    }

    fn add(
        &self,
        el: Element,
        tree: &mut Tree,
        items: &mut Vec<Item>,
        cbs: &mut Vec<Callback>,
    ) -> usize {
        let slot = items.len();
        let mut el = el;
        self.scale_element(&mut el);
        let hit = el.on_click.map(|cb| {
            cbs.push(cb);
            cbs.len() - 1
        });
        // Ota-ona oldin chizilishi uchun avval joy ajratamiz, tugun raqami keyin yoziladi.
        items.push(Item {
            node: 0,
            kind: ItemKind::Container,
            bg: el.bg,
            radius: el.radius,
            hit,
            clip: false,
            end_clip: 0,
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
            Kind::Text { text, size, color } => {
                items[slot].kind = ItemKind::Text(text.clone(), size, color);
                let font = self.font.clone();
                tree.add_leaf(el.style, move |w| {
                    let max = if w.is_finite() { Some(w) } else { None };
                    let (tw, th, _) = font.measure(&text, size, max);
                    Size::new(tw.ceil(), th.ceil())
                })
            }
        };
        items[slot].node = node;
        node
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
        if let Kind::Text { size, .. } = &mut el.kind {
            *size *= k;
        }
    }

    /// Eng yuqoridagi bosiladigan element indeksi.
    fn hit_test(&self, x: f32, y: f32) -> Option<usize> {
        let p = uzcoders_core::Point::new(x, y);
        (0..self.hits.len())
            .rev()
            .find(|&i| self.hits[i].rect.contains(p))
    }

    /// Teginish/sichqoncha hodisasini qayta ishlaydi.
    pub fn pointer(&mut self, e: PointerEvent) {
        match e.kind {
            PointerKind::Down => {
                self.pressed = self.hit_test(e.pos.x, e.pos.y);
                self.dirty = true;
            }
            PointerKind::Up => {
                let was = self.pressed.take();
                self.dirty = true;
                if let Some(i) = was {
                    if self.hit_test(e.pos.x, e.pos.y) == Some(i) {
                        let cb = self.hits[i].cb.clone();
                        cb();
                    }
                }
            }
            PointerKind::Cancel => {
                self.pressed = None;
                self.dirty = true;
            }
            PointerKind::Move => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use uzcoders_core::State;

    fn font() -> Rc<Font> {
        Rc::new(Font::parse(include_bytes!("../../../assets/fonts/DejaVuSans.ttf")).unwrap())
    }

    #[test]
    fn bosish_hisoblagichni_oshiradi() {
        let count = State::new(0i32);
        let c2 = count.clone();
        let mut ui = Ui::new(font(), move || {
            let c = c2.clone();
            column(vec![
                text(format!("{}", c2.get())),
                button("+", move || c.update(|v| *v += 1)),
            ])
            .padding(10.0)
        });
        let size = Size::new(200.0, 200.0);
        ui.frame(size);
        let r = ui.hits[0].rect;
        let (x, y) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
        ui.pointer(PointerEvent::new(PointerKind::Down, x, y));
        ui.pointer(PointerEvent::new(PointerKind::Up, x, y));
        assert_eq!(count.get(), 1);
        assert!(ui.needs_frame());
        ui.frame(size);
        assert!(!ui.needs_frame());
    }

    #[test]
    fn tashqarida_qoyvorish_bosmaydi() {
        let n = Rc::new(Cell::new(0));
        let n2 = n.clone();
        let mut ui = Ui::new(font(), move || {
            let n = n2.clone();
            column(vec![button("x", move || n.set(n.get() + 1))])
        });
        ui.frame(Size::new(200.0, 200.0));
        let r = ui.hits[0].rect;
        ui.pointer(PointerEvent::new(PointerKind::Down, r.x + 2.0, r.y + 2.0));
        ui.pointer(PointerEvent::new(PointerKind::Up, 190.0, 190.0));
        assert_eq!(n.get(), 0);
    }

    #[test]
    fn chizish_buyruqlari() {
        let mut ui = Ui::new(font(), || {
            column(vec![text("Salom").size(20.0), button("OK", || {})])
        });
        let cmds = ui.frame(Size::new(200.0, 200.0));
        assert!(cmds.iter().any(|c| matches!(c, DrawCmd::RoundRect { .. })));
        assert_eq!(
            cmds.iter()
                .filter(|c| matches!(c, DrawCmd::Text { .. }))
                .count(),
            2
        );
    }

    #[test]
    fn zichlik_olchamlarni_oshiradi() {
        let mut ui = Ui::new(font(), || {
            column(vec![button("OK", || {}).width(100.0).height(40.0)])
        });
        ui.frame(Size::new(400.0, 400.0));
        let r1 = ui.hits[0].rect;
        ui.set_scale(2.0);
        assert!(ui.needs_frame());
        ui.frame(Size::new(400.0, 400.0));
        let r2 = ui.hits[0].rect;
        assert_eq!((r1.w, r1.h), (100.0, 40.0));
        assert_eq!((r2.w, r2.h), (200.0, 80.0));
    }
}
