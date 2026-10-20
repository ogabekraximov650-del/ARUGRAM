//! Elementlar daraxti: tavsif (qiymat), har kadrda qaytadan quriladi.

use crate::scroll::ScrollState;
use std::rc::Rc;
use uzcoders_core::{Color, EdgeInsets, State};
use uzcoders_layout::{Align, Axis, Dim, Justify, Style};
use uzcoders_render::{Image, Path};

/// Klaviatura turi (Android `inputType` ga aylanadi).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum KeyboardKind {
    #[default]
    Text,
    Number,
    Phone,
    Password,
    Email,
}

impl KeyboardKind {
    /// Android `InputType` qiymati.
    pub fn android_input_type(self) -> i32 {
        match self {
            KeyboardKind::Text => 1,
            KeyboardKind::Number => 2,
            KeyboardKind::Phone => 3,
            KeyboardKind::Password => 1 | 0x80,
            KeyboardKind::Email => 1 | 0x20,
        }
    }
}

pub(crate) type Callback = Rc<dyn Fn()>;

pub(crate) struct FieldCfg {
    pub state: State<String>,
    pub hint: String,
    pub size: f32,
    pub color: Color,
    pub hint_color: Color,
    pub accent: Color,
    pub password: bool,
    pub max_len: usize,
    pub filter: Option<fn(char) -> bool>,
    pub format: Option<fn(&str) -> String>,
    pub on_submit: Option<Callback>,
    pub label: String,
    pub label_bg: Color,
    pub line_height: Option<f32>,
    pub spacing: f32,
    pub keyboard: KeyboardKind,
    pub label_inside: bool,
}

pub(crate) enum Kind {
    Container,
    Text {
        text: String,
        size: f32,
        color: Color,
        weight: u16,
        line_height: Option<f32>,
        spacing: f32,
    },
    Icon {
        path: Rc<Path>,
        size: f32,
        color: Color,
    },
    Spinner {
        size: f32,
        width: f32,
        color: Color,
    },
    Image {
        image: Rc<Image>,
        w: f32,
        h: f32,
        opacity: f32,
    },
    Field(Box<FieldCfg>),
}

/// UI daraxti elementi.
pub struct Element {
    pub(crate) kind: Kind,
    pub(crate) style: Style,
    pub(crate) bg: Option<Color>,
    pub(crate) radius: f32,
    pub(crate) border: Option<(f32, Color)>,
    pub(crate) glass: Option<Color>,
    pub(crate) backdrop: Option<(Color, f32, f32)>,
    pub(crate) gradient: Option<(Color, Color)>,
    /// Yumshoq yorug'lik dog'lari: (x ulushi, y ulushi, radius dp, rang) — element qutisiga nisbatan.
    pub(crate) glows: Vec<(f32, f32, f32, Color)>,
    pub(crate) on_click: Option<Callback>,
    pub(crate) on_swipe: Option<Rc<dyn Fn(i32)>>,
    pub(crate) scroll: Option<ScrollState>,
    pub(crate) children: Vec<Element>,
}

impl Element {
    fn new(kind: Kind, style: Style, children: Vec<Element>) -> Element {
        Element {
            kind,
            style,
            bg: None,
            radius: 0.0,
            border: None,
            glass: None,
            backdrop: None,
            gradient: None,
            glows: Vec::new(),
            on_click: None,
            on_swipe: None,
            scroll: None,
            children,
        }
    }

    fn container(direction: Axis, children: Vec<Element>) -> Element {
        Element::new(
            Kind::Container,
            Style {
                direction,
                ..Style::default()
            },
            children,
        )
    }

    pub fn padding(mut self, v: f32) -> Self {
        self.style.padding = EdgeInsets::all(v);
        self
    }
    pub fn padding_xy(mut self, horizontal: f32, vertical: f32) -> Self {
        self.style.padding = EdgeInsets::symmetric(horizontal, vertical);
        self
    }
    pub fn padding_each(mut self, left: f32, top: f32, right: f32, bottom: f32) -> Self {
        self.style.padding = EdgeInsets::new(left, top, right, bottom);
        self
    }
    pub fn margin(mut self, v: f32) -> Self {
        self.style.margin = EdgeInsets::all(v);
        self
    }
    pub fn margin_each(mut self, left: f32, top: f32, right: f32, bottom: f32) -> Self {
        self.style.margin = EdgeInsets::new(left, top, right, bottom);
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
    /// "Shisha" fon (iOS uslubi): shaffof `tint`, yorug' chegara va ichki yaltirash.
    pub fn glass(mut self, tint: Color) -> Self {
        self.glass = Some(tint);
        self
    }
    /// HAQIQIY shisha: element ostidagi tasvir `blur` dp radiusda xiralashadi, ustiga `tint` va yorug' chegara.
    pub fn backdrop(mut self, tint: Color, blur: f32) -> Self {
        self.backdrop = Some((tint, blur, 1.0));
        self
    }
    /// "Lupali" shisha (iOS Liquid Glass): deyarli shaffof, ostidagi tasvir panel markazidan `zoom` marta kattalashib ko'rinadi, chetida sinish.
    pub fn lens(mut self, tint: Color, zoom: f32) -> Self {
        self.backdrop = Some((tint, 0.0, zoom));
        self
    }
    /// Element orqasiga yumshoq yorug'lik dog'lari: `(x ulushi, y ulushi, radius dp, rang)`.
    pub fn glows(mut self, g: Vec<(f32, f32, f32, Color)>) -> Self {
        self.glows = g;
        self
    }
    /// Maydon sarlavhasi chegarada emas, maydon ichida tepada kichik bo'lib ko'tariladi (shaffof/shisha fon uchun).
    pub fn label_inside(mut self) -> Self {
        if let Some(f) = self.field_cfg() {
            f.label_inside = true;
        }
        self
    }
    /// Vertikal gradient fon: tepadan pastga `top` -> `bottom`.
    pub fn gradient(mut self, top: Color, bottom: Color) -> Self {
        self.gradient = Some((top, bottom));
        self
    }
    /// Balandlik = kenglik x `ratio` (masalan poster kartasi uchun 1.5).
    pub fn aspect(mut self, ratio: f32) -> Self {
        self.style.aspect = ratio;
        self
    }
    /// Yumaloq burchakli kontur.
    pub fn border(mut self, width: f32, c: Color) -> Self {
        self.border = Some((width, c));
        self
    }
    /// Konturni butunlay olib tashlaydi (fokusda ham chizilmaydi).
    pub fn no_border(mut self) -> Self {
        self.border = None;
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
    /// Qolgan joyni `v` ulushda teng bo'lishadi (Compose `weight`): bolalar kontentidan qat'i nazar.
    pub fn weight(mut self, v: f32) -> Self {
        self.style.grow = v;
        self.style.basis0 = true;
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
    /// Gorizontal surish: chapga (keyingi) `1`, o'ngga (oldingi) `-1` bilan chaqiriladi.
    pub fn on_swipe(mut self, f: impl Fn(i32) + 'static) -> Self {
        self.on_swipe = Some(Rc::new(f));
        self
    }
    pub fn on_click(mut self, f: impl Fn() + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
    /// Asosiy o'q bo'yicha scroll qilinadigan qiladi. Balandlik cheklangan bo'lishi kerak
    /// (masalan `.grow(1.0)` yoki `.height(..)`).
    pub fn scroll(mut self, state: &ScrollState) -> Self {
        self.style.scroll = true;
        self.scroll = Some(state.clone());
        self
    }

    /// Matn yoki kiritish maydoni uchun shrift o'lchami.
    pub fn size(mut self, s: f32) -> Self {
        match &mut self.kind {
            Kind::Text { size, .. } | Kind::Icon { size, .. } => *size = s,
            Kind::Image { .. } | Kind::Spinner { .. } => {}
            Kind::Field(f) => f.size = s,
            Kind::Container => {}
        }
        self
    }
    /// Matn yoki kiritish maydoni matn rangi.
    pub fn color(mut self, c: Color) -> Self {
        match &mut self.kind {
            Kind::Text { color, .. } | Kind::Icon { color, .. } | Kind::Spinner { color, .. } => {
                *color = c
            }
            Kind::Field(f) => f.color = c,
            Kind::Container | Kind::Image { .. } => {}
        }
        self
    }

    /// Shrift qalinligi (400 oddiy, 700 qalin, 900 qora); ilovada shunday yuz qo'shilgan bo'lsa.
    pub fn font_weight(mut self, w: u16) -> Self {
        if let Kind::Text { weight, .. } = &mut self.kind {
            *weight = w;
        }
        self
    }
    pub fn bold(self) -> Self {
        self.font_weight(700)
    }
    pub fn black(self) -> Self {
        self.font_weight(900)
    }
    /// Harflar orasidagi qo'shimcha masofa (dp), Compose `letterSpacing` kabi.
    pub fn letter_spacing(mut self, sp: f32) -> Self {
        match &mut self.kind {
            Kind::Text { spacing, .. } => *spacing = sp,
            Kind::Field(f) => f.spacing = sp,
            _ => {}
        }
        self
    }
    /// Qator qutisi balandligi (dp). Belgilar qutida Compose'dagi kabi proporsional tarqatiladi.
    pub fn line_height(mut self, h: f32) -> Self {
        match &mut self.kind {
            Kind::Text { line_height, .. } => *line_height = Some(h),
            Kind::Field(f) => f.line_height = Some(h),
            _ => {}
        }
        self
    }

    // ── Kiritish maydoni sozlamalari ──
    /// Suzuvchi sarlavha (Material "outlined" uslubi): bo'sh va fokussiz maydonda ichkarida, aks holda chegarada.
    pub fn label(mut self, l: impl Into<String>) -> Self {
        if let Some(f) = self.field_cfg() {
            f.label = l.into();
        }
        self
    }
    /// Suzuvchi sarlavha chegarani qirqib ko'rsatishi uchun orqa fon rangi.
    pub fn label_bg(mut self, c: Color) -> Self {
        if let Some(f) = self.field_cfg() {
            f.label_bg = c;
        }
        self
    }
    fn field_cfg(&mut self) -> Option<&mut FieldCfg> {
        match &mut self.kind {
            Kind::Field(f) => Some(f),
            _ => None,
        }
    }
    /// Bo'sh maydonda ko'rinadigan maslahat matni.
    pub fn hint(mut self, h: impl Into<String>) -> Self {
        if let Some(f) = self.field_cfg() {
            f.hint = h.into();
        }
        self
    }
    pub fn hint_color(mut self, c: Color) -> Self {
        if let Some(f) = self.field_cfg() {
            f.hint_color = c;
        }
        self
    }
    /// Fokusdagi kontur va kursor rangi.
    pub fn accent(mut self, c: Color) -> Self {
        if let Some(f) = self.field_cfg() {
            f.accent = c;
        }
        self
    }
    /// Ochiladigan klaviatura turi (raqamli, telefon, ...).
    pub fn keyboard(mut self, k: KeyboardKind) -> Self {
        if let Some(f) = self.field_cfg() {
            f.keyboard = k;
        }
        self
    }
    /// Matn nuqtalar bilan yashiriladi.
    pub fn password(mut self) -> Self {
        if let Some(f) = self.field_cfg() {
            f.password = true;
            f.keyboard = KeyboardKind::Password;
        }
        self
    }
    /// Eng ko'p belgi soni (0 — cheklanmagan; `format` bo'lsa probellar sanalmaydi).
    pub fn max_len(mut self, n: usize) -> Self {
        if let Some(f) = self.field_cfg() {
            f.max_len = n;
        }
        self
    }
    /// Faqat shu shartga mos belgilar kiritiladi.
    pub fn filter(mut self, p: fn(char) -> bool) -> Self {
        if let Some(f) = self.field_cfg() {
            f.filter = Some(p);
        }
        self
    }
    /// Har tahrirdan keyin matnni qayta formatlaydi (masalan `901234567` -> `90 123 45 67`).
    pub fn format(mut self, fm: fn(&str) -> String) -> Self {
        if let Some(f) = self.field_cfg() {
            f.format = Some(fm);
        }
        self
    }
    /// Klaviaturada "Enter" bosilganda.
    pub fn on_submit(mut self, cb: impl Fn() + 'static) -> Self {
        if let Some(f) = self.field_cfg() {
            f.on_submit = Some(Rc::new(cb));
        }
        self
    }
}

pub fn column(children: Vec<Element>) -> Element {
    Element::container(Axis::Column, children)
}

pub fn row(children: Vec<Element>) -> Element {
    Element::container(Axis::Row, children)
}

/// Qatlam: bolalar bir-birining ustiga qo'yiladi (oxirgisi eng yuqorida), hammasi tugun maydonini to'ldiradi.
pub fn stack(children: Vec<Element>) -> Element {
    let mut e = Element::container(Axis::Column, children);
    e.style.stack = true;
    e
}

/// Vertikal scroll qilinadigan ustun: `.grow(1.0)` bilan qolgan joyni egallaydi.
pub fn scroll_column(state: &ScrollState, children: Vec<Element>) -> Element {
    column(children).scroll(state)
}

pub fn text(s: impl Into<String>) -> Element {
    Element::new(
        Kind::Text {
            text: s.into(),
            size: 16.0,
            color: Color::BLACK,
            weight: 400,
            line_height: None,
            spacing: 0.0,
        },
        Style::default(),
        Vec::new(),
    )
}

thread_local! {
    static PATHS: std::cell::RefCell<std::collections::HashMap<&'static str, Rc<Path>>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Vektor ikonka: `d` — SVG yo'li (24x24 qutida, Material ikonkalari kabi), `size` — dp. Rang `.color()` bilan.
pub fn icon(d: &'static str, size: f32) -> Element {
    let path = PATHS.with(|m| {
        m.borrow_mut()
            .entry(d)
            .or_insert_with(|| Rc::new(Path::parse(d).unwrap_or_default()))
            .clone()
    });
    let mut e = Element::new(
        Kind::Icon {
            path,
            size,
            color: Color::BLACK,
        },
        Style::default(),
        Vec::new(),
    );
    e.style.shrink = 0.0;
    e
}

/// Virtual ro'yxat: `count` ta bir xil balandlikdagi (`item_height`, dp) element, lekin faqat ko'rinadiganlari quriladi.
/// `build(i)` — i-elementni qaytaradi. Juda uzun ro'yxatlar ham tez ishlaydi.
pub fn lazy_column(
    state: &ScrollState,
    count: usize,
    item_height: f32,
    build: impl Fn(usize) -> Element,
) -> Element {
    let ihp = (item_height * uzcoders_core::density()).max(1.0);
    let vp = if state.viewport() > 0.0 {
        state.viewport()
    } else {
        3000.0
    };
    let off = state.offset();
    let first = (((off / ihp).floor() as isize) - 2).max(0) as usize;
    let last = (((off + vp) / ihp).ceil() as usize + 2).min(count);
    let first = first.min(last);
    let mut kids = Vec::with_capacity(last - first + 2);
    if first > 0 {
        kids.push(gap_box(0.0, first as f32 * item_height));
    }
    for i in first..last {
        kids.push(build(i).height(item_height));
    }
    if last < count {
        kids.push(gap_box(0.0, (count - last) as f32 * item_height));
    }
    column(kids).scroll(state)
}

/// Aylanuvchi yuklanish ko'rsatkichi (Material `CircularProgressIndicator` kabi): `size` dp, chiziq `width` dp.
pub fn spinner(size: f32, width: f32, color: Color) -> Element {
    let mut e = Element::new(
        Kind::Spinner { size, width, color },
        Style::default(),
        Vec::new(),
    );
    e.style.shrink = 0.0;
    e
}

/// Rasm: `w` x `h` dp o'lchamda (rasm shu qutiga cho'ziladi).
pub fn image(img: &Rc<Image>, w: f32, h: f32) -> Element {
    let mut e = Element::new(
        Kind::Image {
            image: img.clone(),
            w,
            h,
            opacity: 1.0,
        },
        Style::default(),
        Vec::new(),
    );
    e.style.shrink = 0.0;
    e
}

/// Rasm, ota-elementning berilgan maydonini to'ldiradi (`stack` ichida yoki `.aspect()` bilan).
pub fn image_fill(img: &Rc<Image>) -> Element {
    let mut e = image(img, 0.0, 0.0);
    e.style.shrink = 0.0;
    e
}

/// Bo'sh joyni to'ldiradigan elastik element.
pub fn spacer() -> Element {
    Element::container(Axis::Column, Vec::new()).grow(1.0)
}

/// Qat'iy o'lchamli bo'sh joy (padding o'rniga oraliq uchun).
pub fn gap_box(w: f32, h: f32) -> Element {
    Element::container(Axis::Column, Vec::new())
        .width(w)
        .height(h)
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

/// Matn kiritish maydoni: qiymat `state` da saqlanadi. Bosilsa fokus oladi va klaviatura ochiladi.
pub fn field(state: &State<String>) -> Element {
    let mut e = Element::new(
        Kind::Field(Box::new(FieldCfg {
            state: state.clone(),
            hint: String::new(),
            size: 16.0,
            color: Color::BLACK,
            hint_color: Color::hex(0x8A8A8E),
            accent: Color::hex(0x2F80ED),
            password: false,
            max_len: 0,
            filter: None,
            format: None,
            on_submit: None,
            label: String::new(),
            label_bg: Color::WHITE,
            line_height: None,
            spacing: 0.0,
            keyboard: KeyboardKind::Text,
            label_inside: false,
        })),
        Style::default(),
        Vec::new(),
    );
    e.style.padding = EdgeInsets::symmetric(14.0, 14.0);
    e.radius = 12.0;
    e.border = Some((1.0, Color::hex(0xBDBDBD)));
    e
}

/// Teng ustunli to'r: `columns` ustun, bolalar qatorma-qator joylashadi (so'nggi qator bo'sh kataklar bilan to'ldiriladi).
pub fn grid(columns: usize, gap: f32, children: Vec<Element>) -> Element {
    let columns = columns.max(1);
    let mut rows = Vec::new();
    let mut it = children.into_iter().peekable();
    while it.peek().is_some() {
        let mut cells = Vec::with_capacity(columns);
        for _ in 0..columns {
            cells.push(match it.next() {
                Some(c) => c.weight(1.0),
                None => spacer().weight(1.0),
            });
        }
        rows.push(row(cells).gap(gap));
    }
    column(rows).gap(gap)
}
