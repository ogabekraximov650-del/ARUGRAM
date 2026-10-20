//! # uzcoders-layout
//! Flex joylashuv dvigateli (bir qatorli, o'ramasiz): `Row` / `Column`, `grow`, `gap`, `padding`, `margin`,
//! `align_items`, `justify`. Matn kabi o'lchami kenglikka bog'liq tugunlar uchun `measure` funksiyasi beriladi.

use uzcoders_core::{EdgeInsets, Rect, Size};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Axis {
    Row,
    #[default]
    Column,
}

/// O'lcham: kontentga qarab, aniq piksel, yoki ota-ona kengligining foizi.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Dim {
    #[default]
    Auto,
    Px(f32),
    Percent(f32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Justify {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    pub direction: Axis,
    pub width: Dim,
    pub height: Dim,
    /// Bo'sh joyni bo'lishish ulushi (0 — kengaymaydi).
    pub grow: f32,
    /// Joy yetmaganda qisqarish ulushi (Px o'lchamli tugunlar qisqarmaydi).
    pub shrink: f32,
    pub padding: EdgeInsets,
    pub margin: EdgeInsets,
    pub gap: f32,
    pub align_items: Align,
    pub justify: Justify,
    /// Asosiy o'q bo'yicha kontent sig'masa bolalar qisqarmaydi (tugun scroll qilinadi).
    pub scroll: bool,
    /// Boshlang'ich (basis) o'lcham 0 deb olinadi: `grow` bo'yicha teng bo'linadi (Compose'dagi `weight`).
    pub basis0: bool,
    /// Bolalar bir-birining ustiga (qatlam) qo'yiladi: har biri tugun maydonini to'liq egallaydi.
    pub stack: bool,
    /// Balandlik = kenglik * aspect (0 — o'chiq).
    pub aspect: f32,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            direction: Axis::Column,
            width: Dim::Auto,
            height: Dim::Auto,
            grow: 0.0,
            shrink: 1.0,
            padding: EdgeInsets::ZERO,
            margin: EdgeInsets::ZERO,
            gap: 0.0,
            align_items: Align::Stretch,
            justify: Justify::Start,
            scroll: false,
            basis0: false,
            stack: false,
            aspect: 0.0,
        }
    }
}

pub type NodeId = usize;
type MeasureFn = Box<dyn Fn(f32) -> Size>;

struct Node {
    style: Style,
    children: Vec<NodeId>,
    /// Bargi tugun: berilgan kenglikda (cheksiz bo'lsa `f32::INFINITY`) kontent o'lchamini qaytaradi.
    measure: Option<MeasureFn>,
    rect: Rect,
    /// Asosiy o'q bo'yicha bolalarning to'liq uzunligi (scroll uchun).
    extent: f32,
}

#[derive(Default)]
pub struct Tree {
    nodes: Vec<Node>,
}

impl Tree {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, style: Style, children: Vec<NodeId>) -> NodeId {
        self.nodes.push(Node {
            style,
            children,
            measure: None,
            rect: Rect::ZERO,
            extent: 0.0,
        });
        self.nodes.len() - 1
    }

    pub fn add_leaf(&mut self, style: Style, measure: impl Fn(f32) -> Size + 'static) -> NodeId {
        self.nodes.push(Node {
            style,
            children: Vec::new(),
            measure: Some(Box::new(measure)),
            rect: Rect::ZERO,
            extent: 0.0,
        });
        self.nodes.len() - 1
    }

    /// Hisoblangan joylashuv (ildizga nisbatan mutlaq koordinatalarda, `compute` dan keyin).
    pub fn rect(&self, id: NodeId) -> Rect {
        self.nodes[id].rect
    }

    /// Tugunning ichki chekinishi.
    pub fn padding(&self, id: NodeId) -> EdgeInsets {
        self.nodes[id].style.padding
    }

    /// Bolalarning asosiy o'q bo'yicha to'liq uzunligi (padding'siz).
    pub fn content_extent(&self, id: NodeId) -> f32 {
        self.nodes[id].extent
    }

    pub fn children(&self, id: NodeId) -> &[NodeId] {
        &self.nodes[id].children
    }

    /// Ildizni `available` o'lchamdagi maydonga joylashtiradi.
    pub fn compute(&mut self, root: NodeId, available: Size) {
        let s = self.nodes[root].style;
        let size = self.measure(root, available.w);
        let w = match s.width {
            Dim::Px(p) => p,
            Dim::Percent(p) => available.w * p / 100.0,
            Dim::Auto => available.w,
        };
        let h = match s.height {
            Dim::Px(p) => p,
            Dim::Percent(p) => available.h * p / 100.0,
            // Ildiz oynani to'ldiradi (sig'magan kontent qisqaradi yoki scroll qilinadi).
            Dim::Auto => {
                if available.h.is_finite() {
                    available.h
                } else {
                    size.h
                }
            }
        };
        self.place(root, Rect::new(s.margin.left, s.margin.top, w, h));
    }

    /// Tugunning tashqi (margin'siz) kontent o'lchami `avail_w` kenglik sharoitida.
    fn measure(&self, id: NodeId, avail_w: f32) -> Size {
        let n = &self.nodes[id];
        let s = &n.style;
        let explicit_w = match s.width {
            Dim::Px(p) => Some(p),
            Dim::Percent(p) if avail_w.is_finite() => Some(avail_w * p / 100.0),
            _ => None,
        };
        let explicit_h = match s.height {
            Dim::Px(p) => Some(p),
            _ => None,
        };
        let inner_avail = explicit_w.unwrap_or(avail_w) - s.padding.horizontal();
        if s.aspect > 0.0 && explicit_w.unwrap_or(avail_w).is_finite() {
            let w = explicit_w.unwrap_or(avail_w);
            return Size::new(w, explicit_h.unwrap_or(w * s.aspect));
        }

        let content = if let Some(m) = &n.measure {
            m(inner_avail.max(0.0))
        } else {
            self.measure_children(n, inner_avail)
        };
        Size::new(
            explicit_w.unwrap_or(content.w + s.padding.horizontal()),
            explicit_h.unwrap_or(content.h + s.padding.vertical()),
        )
    }

    fn measure_children(&self, n: &Node, inner_avail_w: f32) -> Size {
        let s = &n.style;
        let axis = s.direction;
        if s.stack {
            let (mut w, mut h) = (0.0f32, 0.0f32);
            for &c in &n.children {
                let cm = self.nodes[c].style.margin;
                let sz = self.measure(c, (inner_avail_w - cm.horizontal()).max(0.0));
                w = w.max(sz.w + cm.horizontal());
                h = h.max(sz.h + cm.vertical());
            }
            return Size::new(w, h);
        }
        // Qatorda `weight` bolalar o'z ulushidagi kenglikda o'lchanadi (aks holda ular to'liq kenglikda o'lchanib, balandlik noto'g'ri chiqadi).
        let total_grow: f32 = if axis == Axis::Row {
            n.children
                .iter()
                .filter(|&&c| self.nodes[c].style.basis0)
                .map(|&c| self.nodes[c].style.grow)
                .sum()
        } else {
            0.0
        };
        let gaps = s.gap * n.children.len().saturating_sub(1) as f32;
        let mut main = 0.0f32;
        let mut cross = 0.0f32;
        for (i, &c) in n.children.iter().enumerate() {
            let cs = &self.nodes[c].style;
            let cm = cs.margin;
            let avail = if axis == Axis::Row
                && cs.basis0
                && total_grow > 0.0
                && inner_avail_w.is_finite()
            {
                ((inner_avail_w - gaps) * cs.grow / total_grow - cm.horizontal()).max(0.0)
            } else {
                (inner_avail_w - cm.horizontal()).max(0.0)
            };
            let sz = self.measure(c, avail);
            let outer = Size::new(sz.w + cm.horizontal(), sz.h + cm.vertical());
            let (m, x) = split(axis, outer);
            main += m + if i > 0 { s.gap } else { 0.0 };
            cross = cross.max(x);
        }
        join(axis, main, cross)
    }

    /// `rect` — tugunga ajratilgan chegara qutisi (margin'siz). Bolalarni rekursiv joylaydi.
    fn place(&mut self, id: NodeId, rect: Rect) {
        self.nodes[id].rect = rect;
        let style = self.nodes[id].style;
        let children = self.nodes[id].children.clone();
        if children.is_empty() {
            return;
        }
        let content = rect.deflate(style.padding);
        if style.stack {
            for &c in &children {
                let m = self.nodes[c].style.margin;
                self.place(
                    c,
                    Rect::new(
                        content.x + m.left,
                        content.y + m.top,
                        (content.w - m.horizontal()).max(0.0),
                        (content.h - m.vertical()).max(0.0),
                    ),
                );
            }
            return;
        }
        let axis = style.direction;
        let (content_main, content_cross) = split(axis, content.size());

        // 1) Har bir bolaning asosiy o'qdagi boshlang'ich (basis) o'lchami (margin bilan).
        struct Item {
            id: NodeId,
            basis: f32,
            margin_main: f32,
            margin_cross: f32,
            grow: f32,
            shrinkable: f32,
        }
        let mut items: Vec<Item> = Vec::with_capacity(children.len());
        for &c in &children {
            let cs = self.nodes[c].style;
            let sz = self.measure(c, (content.w - cs.margin.horizontal()).max(0.0));
            let (mm, mc) = split(
                axis,
                Size::new(cs.margin.horizontal(), cs.margin.vertical()),
            );
            let fixed_main = match axis {
                Axis::Row => matches!(cs.width, Dim::Px(_)),
                Axis::Column => matches!(cs.height, Dim::Px(_)),
            };
            // Scroll tugun kontentiga qarab o'lchanmaydi: qolgan joyni egallaydi (o'lchami aniq bo'lmasa).
            let flexible_scroll = cs.scroll && !fixed_main;
            let basis = if cs.basis0 || flexible_scroll {
                0.0
            } else {
                split(axis, sz).0
            };
            items.push(Item {
                id: c,
                basis,
                margin_main: mm,
                margin_cross: mc,
                grow: if flexible_scroll {
                    cs.grow.max(1.0)
                } else {
                    cs.grow
                },
                shrinkable: if fixed_main { 0.0 } else { cs.shrink },
            });
        }

        // 2) Bo'sh joyni taqsimlash yoki qisqartirish.
        let gaps = style.gap * (items.len().saturating_sub(1)) as f32;
        let used: f32 = items.iter().map(|i| i.basis + i.margin_main).sum::<f32>() + gaps;
        let free = content_main - used;
        self.nodes[id].extent = used;
        let mut sizes: Vec<f32> = items.iter().map(|i| i.basis).collect();
        let mut leftover = 0.0f32;
        if free > 0.0 {
            let total_grow: f32 = items.iter().map(|i| i.grow).sum();
            if total_grow > 0.0 {
                for (k, it) in items.iter().enumerate() {
                    sizes[k] += free * it.grow / total_grow;
                }
            } else {
                leftover = free;
            }
        } else if free < 0.0 && !style.scroll {
            let total: f32 = items.iter().map(|i| i.shrinkable * i.basis).sum();
            if total > 0.0 {
                for (k, it) in items.iter().enumerate() {
                    sizes[k] = (sizes[k] + free * (it.shrinkable * it.basis) / total).max(0.0);
                }
            }
        }

        // 3) Asosiy o'q bo'yicha boshlang'ich joy (justify) va oraliq.
        let (mut cursor, extra_gap) = match style.justify {
            Justify::Start => (0.0, 0.0),
            Justify::Center => (leftover * 0.5, 0.0),
            Justify::End => (leftover, 0.0),
            Justify::SpaceBetween => {
                if items.len() > 1 {
                    (0.0, leftover / (items.len() - 1) as f32)
                } else {
                    (0.0, 0.0)
                }
            }
        };

        // 4) Joylash: har bir bolaning ko'ndalang o'lchami va o'rni.
        for (k, it) in items.iter().enumerate() {
            let cs = self.nodes[it.id].style;
            let main_size = sizes[k];
            // Matn kabi kenglikka bog'liq bolalar uchun balandlikni yakuniy kenglikda qayta o'lchaymiz.
            let child_w_for_measure = match axis {
                Axis::Row => main_size,
                Axis::Column => (content.w - cs.margin.horizontal()).max(0.0),
            };
            let measured = self.measure(it.id, child_w_for_measure);
            let measured_cross = split(axis, measured).1;
            let avail_cross = (content_cross - it.margin_cross).max(0.0);
            let explicit_cross = match axis {
                Axis::Row => matches!(cs.height, Dim::Px(_)),
                Axis::Column => matches!(cs.width, Dim::Px(_)),
            };
            let align = cs.align_items; // (bola o'z `align_items` ni ota uchun emas — quyida ota'niki)
            let _ = align;
            let parent_align = style.align_items;
            let cross_size = if parent_align == Align::Stretch && !explicit_cross {
                avail_cross
            } else {
                measured_cross.min(avail_cross.max(measured_cross))
            };
            let cross_off = match parent_align {
                Align::Start | Align::Stretch => 0.0,
                Align::Center => (avail_cross - cross_size) * 0.5,
                Align::End => avail_cross - cross_size,
            };
            let (mm_start, mc_start) = match axis {
                Axis::Row => (cs.margin.left, cs.margin.top),
                Axis::Column => (cs.margin.top, cs.margin.left),
            };
            let main_pos = cursor + mm_start;
            let cross_pos = cross_off + mc_start;
            let (x, y, w, h) = match axis {
                Axis::Row => (
                    content.x + main_pos,
                    content.y + cross_pos,
                    main_size,
                    cross_size,
                ),
                Axis::Column => (
                    content.x + cross_pos,
                    content.y + main_pos,
                    cross_size,
                    main_size,
                ),
            };
            let h = if cs.aspect > 0.0 { w * cs.aspect } else { h };
            self.place(it.id, Rect::new(x, y, w, h));
            cursor += main_size + it.margin_main + style.gap + extra_gap;
        }
    }
}

fn split(axis: Axis, s: Size) -> (f32, f32) {
    match axis {
        Axis::Row => (s.w, s.h),
        Axis::Column => (s.h, s.w),
    }
}

fn join(axis: Axis, main: f32, cross: f32) -> Size {
    match axis {
        Axis::Row => Size::new(main, cross),
        Axis::Column => Size::new(cross, main),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed(w: f32, h: f32) -> Style {
        Style {
            width: Dim::Px(w),
            height: Dim::Px(h),
            ..Style::default()
        }
    }

    #[test]
    fn stack_va_aspect_va_ulushli_qator() {
        let mut t = Tree::new();
        let img = t.add(
            Style {
                aspect: 1.5,
                ..Style::default()
            },
            vec![],
        );
        let over = t.add(Style::default(), vec![]);
        let card = t.add(
            Style {
                stack: true,
                aspect: 1.5,
                basis0: true,
                grow: 1.0,
                ..Style::default()
            },
            vec![img, over],
        );
        let card2 = t.add(
            Style {
                aspect: 1.5,
                basis0: true,
                grow: 1.0,
                ..Style::default()
            },
            vec![],
        );
        let row = t.add(
            Style {
                direction: Axis::Row,
                gap: 10.0,
                ..Style::default()
            },
            vec![card, card2],
        );
        let root = t.add(Style::default(), vec![row]);
        t.compute(root, Size::new(210.0, 600.0));
        assert_eq!(t.rect(card).w, 100.0);
        assert_eq!(t.rect(card).h, 150.0);
        assert_eq!(t.rect(row).h, 150.0);
        // Qatlam: ikkala bola karta maydonini to'ldiradi.
        assert_eq!(t.rect(over).w, 100.0);
        assert_eq!(t.rect(over).h, 150.0);
        assert_eq!(t.rect(card2).x, 110.0);
    }

    #[test]
    fn scroll_tugun_qolgan_joyni_oladi_ota_qisqarmaydi() {
        let mut t = Tree::new();
        let top = t.add(fixed(100.0, 60.0), vec![]);
        let items: Vec<NodeId> = (0..50).map(|_| t.add(fixed(100.0, 40.0), vec![])).collect();
        let list = t.add(
            Style {
                scroll: true,
                ..Style::default()
            },
            items,
        );
        let root = t.add(Style::default(), vec![top, list]);
        t.compute(root, Size::new(100.0, 500.0));
        assert_eq!(t.rect(top).h, 60.0);
        assert_eq!(t.rect(list).h, 440.0);
        assert_eq!(t.content_extent(list), 2000.0);
        assert_eq!(t.rect(t_child(&t, list, 49)).h, 40.0);
    }

    fn t_child(t: &Tree, id: NodeId, i: usize) -> NodeId {
        t.children(id)[i]
    }

    #[test]
    fn row_places_children_with_gap() {
        let mut t = Tree::new();
        let a = t.add(fixed(40.0, 20.0), vec![]);
        let b = t.add(fixed(60.0, 30.0), vec![]);
        let root = t.add(
            Style {
                direction: Axis::Row,
                gap: 10.0,
                align_items: Align::Start,
                ..Style::default()
            },
            vec![a, b],
        );
        t.compute(root, Size::new(300.0, 100.0));
        assert_eq!(t.rect(a), Rect::new(0.0, 0.0, 40.0, 20.0));
        assert_eq!(t.rect(b), Rect::new(50.0, 0.0, 60.0, 30.0));
    }

    #[test]
    fn grow_splits_free_space_by_weight() {
        let mut t = Tree::new();
        let a = t.add(
            Style {
                grow: 1.0,
                ..Style::default()
            },
            vec![],
        );
        let b = t.add(
            Style {
                grow: 3.0,
                ..Style::default()
            },
            vec![],
        );
        let root = t.add(
            Style {
                direction: Axis::Row,
                ..Style::default()
            },
            vec![a, b],
        );
        t.compute(root, Size::new(200.0, 50.0));
        assert!((t.rect(a).w - 50.0).abs() < 1e-3);
        assert!((t.rect(b).w - 150.0).abs() < 1e-3);
        assert!((t.rect(b).x - 50.0).abs() < 1e-3);
    }

    #[test]
    fn padding_and_center_alignment() {
        let mut t = Tree::new();
        let a = t.add(fixed(20.0, 20.0), vec![]);
        let root = t.add(
            Style {
                direction: Axis::Column,
                padding: EdgeInsets::all(10.0),
                align_items: Align::Center,
                justify: Justify::Center,
                width: Dim::Px(100.0),
                height: Dim::Px(100.0),
                ..Style::default()
            },
            vec![a],
        );
        t.compute(root, Size::new(400.0, 400.0));
        let r = t.rect(a);
        assert!((r.x - 40.0).abs() < 1e-3, "{r:?}");
        assert!((r.y - 40.0).abs() < 1e-3, "{r:?}");
    }

    #[test]
    fn space_between_pushes_to_edges() {
        let mut t = Tree::new();
        let a = t.add(fixed(10.0, 10.0), vec![]);
        let b = t.add(fixed(10.0, 10.0), vec![]);
        let root = t.add(
            Style {
                direction: Axis::Row,
                justify: Justify::SpaceBetween,
                align_items: Align::Start,
                ..Style::default()
            },
            vec![a, b],
        );
        t.compute(root, Size::new(100.0, 10.0));
        assert!((t.rect(a).x - 0.0).abs() < 1e-3);
        assert!((t.rect(b).x - 90.0).abs() < 1e-3);
    }

    #[test]
    fn text_like_leaf_wraps_with_width() {
        // 10 ta belgi, har biri 10 px: 50 px kenglikda 2 qatorga o'raladi.
        let mut t = Tree::new();
        let leaf = t.add_leaf(Style::default(), |w| {
            let total = 100.0f32;
            if w.is_infinite() || w >= total {
                Size::new(total, 20.0)
            } else {
                let per_line = (w / 10.0).floor().max(1.0) * 10.0;
                let lines = (total / per_line).ceil();
                Size::new(per_line, 20.0 * lines)
            }
        });
        let root = t.add(
            Style {
                width: Dim::Px(50.0),
                ..Style::default()
            },
            vec![leaf],
        );
        t.compute(root, Size::new(50.0, 400.0));
        assert_eq!(t.rect(leaf).h, 40.0);
    }

    #[test]
    fn column_stretch_fills_width_and_margin_offsets() {
        let mut t = Tree::new();
        let a = t.add(
            Style {
                height: Dim::Px(10.0),
                margin: EdgeInsets::new(5.0, 7.0, 5.0, 0.0),
                ..Style::default()
            },
            vec![],
        );
        let root = t.add(Style::default(), vec![a]);
        t.compute(root, Size::new(100.0, 100.0));
        let r = t.rect(a);
        assert_eq!((r.x, r.y, r.w), (5.0, 7.0, 90.0));
    }

    #[test]
    fn shrink_reduces_auto_children_not_fixed() {
        let mut t = Tree::new();
        let a = t.add(fixed(80.0, 10.0), vec![]);
        let b = t.add_leaf(Style::default(), |_| Size::new(80.0, 10.0));
        let root = t.add(
            Style {
                direction: Axis::Row,
                align_items: Align::Start,
                ..Style::default()
            },
            vec![a, b],
        );
        t.compute(root, Size::new(100.0, 10.0));
        assert_eq!(t.rect(a).w, 80.0);
        assert!((t.rect(b).w - 20.0).abs() < 1e-3, "{:?}", t.rect(b));
    }
}
