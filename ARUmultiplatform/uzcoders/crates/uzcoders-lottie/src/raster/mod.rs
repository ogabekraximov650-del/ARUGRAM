//! UZ: Kichik dasturiy rasterizator: yo'l to'ldirish va chiziq (qirralari yumshatilgan), gradient, maska va alfa aralashtirish.
//! Piksellar — oldindan ko'paytirilgan (premultiplied) RGBA8. Tashqi grafik kutubxona ishlatilmaydi.
//! EN: A small software rasterizer: anti-aliased path fill and stroke, gradients, masks and alpha blending.
//! Pixels are premultiplied RGBA8. No external graphics crate is used.
//! RU: Небольшой программный растеризатор: заливка и обводка контуров со сглаживанием, градиенты, маски и альфа-смешивание.
//! Пиксели — RGBA8 с предумноженной альфой. Внешняя графическая библиотека не используется.

mod path;
mod pixmap;
mod stroke;
#[cfg(test)]
mod tests;

pub use path::{Path, PathBuilder, PathSegment};
pub use pixmap::{Mask, MaskType, Pixmap};

/// UZ: Nuqta. EN: A point. RU: Точка.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    /// UZ: X koordinata. EN: X coordinate. RU: Координата X.
    pub x: f32,
    /// UZ: Y koordinata. EN: Y coordinate. RU: Координата Y.
    pub y: f32,
}

impl Point {
    /// UZ: Koordinatalardan nuqta. EN: A point from coordinates. RU: Точка по координатам.
    #[must_use]
    pub const fn from_xy(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// UZ: Haqiqiy sonli to'rtburchak (chap, tepa, o'ng, past). EN: A float rectangle (left, top, right, bottom). RU: Прямоугольник (лево, верх, право, низ).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

impl Rect {
    /// UZ: Chap-tepa va o'lchamdan; noto'g'ri qiymatda `None`. EN: From origin and size; `None` for invalid values. RU: По углу и размеру; `None` при неверных значениях.
    #[must_use]
    pub fn from_xywh(x: f32, y: f32, w: f32, h: f32) -> Option<Self> {
        let (r, b) = (x + w, y + h);
        if x.is_finite() && y.is_finite() && w.is_finite() && h.is_finite() && w >= 0.0 && h >= 0.0 && r.is_finite() && b.is_finite() {
            Some(Self { left: x, top: y, right: r, bottom: b })
        } else {
            None
        }
    }

    /// UZ: Chap chegara. EN: Left edge. RU: Левый край.
    #[must_use]
    pub const fn left(&self) -> f32 {
        self.left
    }
    /// UZ: Tepa chegara. EN: Top edge. RU: Верхний край.
    #[must_use]
    pub const fn top(&self) -> f32 {
        self.top
    }
    /// UZ: O'ng chegara. EN: Right edge. RU: Правый край.
    #[must_use]
    pub const fn right(&self) -> f32 {
        self.right
    }
    /// UZ: Past chegara. EN: Bottom edge. RU: Нижний край.
    #[must_use]
    pub const fn bottom(&self) -> f32 {
        self.bottom
    }
}

/// UZ: Butun sonli to'rtburchak. EN: An integer rectangle. RU: Целочисленный прямоугольник.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntRect {
    x: i32,
    y: i32,
    w: u32,
    h: u32,
}

impl IntRect {
    /// UZ: Chap-tepa va o'lchamdan; o'lcham 0 bo'lsa `None`. EN: From origin and size; `None` for zero size. RU: По углу и размеру; `None` при нулевом размере.
    #[must_use]
    pub const fn from_xywh(x: i32, y: i32, w: u32, h: u32) -> Option<Self> {
        if w == 0 || h == 0 { None } else { Some(Self { x, y, w, h }) }
    }
    /// UZ: Chap chegara. EN: Left edge. RU: Левый край.
    #[must_use]
    pub const fn left(&self) -> i32 {
        self.x
    }
    /// UZ: Tepa chegara. EN: Top edge. RU: Верхний край.
    #[must_use]
    pub const fn top(&self) -> i32 {
        self.y
    }
    /// UZ: Kenglik. EN: Width. RU: Ширина.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.w
    }
    /// UZ: Balandlik. EN: Height. RU: Высота.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.h
    }
}

/// UZ: 2D affin almashtirish: `x' = sx*x + kx*y + tx`, `y' = ky*x + sy*y + ty`.
/// EN: A 2D affine transform: `x' = sx*x + kx*y + tx`, `y' = ky*x + sy*y + ty`.
/// RU: 2D аффинное преобразование: `x' = sx*x + kx*y + tx`, `y' = ky*x + sy*y + ty`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    /// UZ: X bo'yicha masshtab. EN: X scale. RU: Масштаб по X.
    pub sx: f32,
    /// UZ: Y ning X ga ta'siri (qiyshayish). EN: Skew of X by Y. RU: Влияние Y на X (скос).
    pub kx: f32,
    /// UZ: X ning Y ga ta'siri (qiyshayish). EN: Skew of Y by X. RU: Влияние X на Y (скос).
    pub ky: f32,
    /// UZ: Y bo'yicha masshtab. EN: Y scale. RU: Масштаб по Y.
    pub sy: f32,
    /// UZ: X siljish. EN: X translation. RU: Сдвиг по X.
    pub tx: f32,
    /// UZ: Y siljish. EN: Y translation. RU: Сдвиг по Y.
    pub ty: f32,
}

impl Default for Transform {
    fn default() -> Self {
        Self::identity()
    }
}

impl Transform {
    /// UZ: Birlik almashtirish. EN: The identity transform. RU: Единичное преобразование.
    #[must_use]
    pub const fn identity() -> Self {
        Self { sx: 1.0, kx: 0.0, ky: 0.0, sy: 1.0, tx: 0.0, ty: 0.0 }
    }

    /// UZ: Birlik almashtirishmi. EN: Is it the identity. RU: Единичное ли.
    #[must_use]
    pub fn is_identity(&self) -> bool {
        *self == Self::identity()
    }

    /// UZ: `self ∘ other`: avval `other`, keyin `self` qo'llanadi. EN: `self ∘ other`: `other` is applied first, then `self`. RU: `self ∘ other`: сначала `other`, затем `self`.
    #[must_use]
    pub fn pre_concat(self, o: Self) -> Self {
        Self {
            sx: self.sx * o.sx + self.kx * o.ky,
            kx: self.sx * o.kx + self.kx * o.sy,
            ky: self.ky * o.sx + self.sy * o.ky,
            sy: self.ky * o.kx + self.sy * o.sy,
            tx: self.sx * o.tx + self.kx * o.ty + self.tx,
            ty: self.ky * o.tx + self.sy * o.ty + self.ty,
        }
    }

    /// UZ: Avval siljitadi (nuqta almashtirishdan oldin). EN: Translates first (before the existing transform). RU: Сначала сдвиг (до существующего преобразования).
    #[must_use]
    pub fn pre_translate(self, tx: f32, ty: f32) -> Self {
        self.pre_concat(Self { tx, ty, ..Self::identity() })
    }

    /// UZ: Avval masshtablaydi. EN: Scales first. RU: Сначала масштабирует.
    #[must_use]
    pub fn pre_scale(self, sx: f32, sy: f32) -> Self {
        self.pre_concat(Self { sx, sy, ..Self::identity() })
    }

    /// UZ: Avval buradi (gradus, soat mili yo'nalishida ekranda). EN: Rotates first (degrees, clockwise on screen). RU: Сначала поворот (градусы, по часовой на экране).
    #[must_use]
    pub fn pre_rotate(self, degrees: f32) -> Self {
        let (s, c) = degrees.to_radians().sin_cos();
        self.pre_concat(Self { sx: c, kx: -s, ky: s, sy: c, tx: 0.0, ty: 0.0 })
    }

    /// UZ: Nuqtani almashtiradi. EN: Maps a point. RU: Преобразует точку.
    #[must_use]
    pub fn map(&self, x: f32, y: f32) -> (f32, f32) {
        (self.sx * x + self.kx * y + self.tx, self.ky * x + self.sy * y + self.ty)
    }

    /// UZ: Teskari almashtirish; o'zgarmas bo'lsa `None`. EN: The inverse; `None` if singular. RU: Обратное; `None`, если вырожденное.
    #[must_use]
    pub fn invert(&self) -> Option<Self> {
        let det = self.sx * self.sy - self.kx * self.ky;
        if !det.is_finite() || det.abs() < 1e-12 {
            return None;
        }
        let id = 1.0 / det;
        let sx = self.sy * id;
        let kx = -self.kx * id;
        let ky = -self.ky * id;
        let sy = self.sx * id;
        Some(Self { sx, kx, ky, sy, tx: -(sx * self.tx + kx * self.ty), ty: -(ky * self.tx + sy * self.ty) })
    }

    /// UZ: Chiziqli masshtab (qirralarni yumshatish uchun o'rtacha). EN: Mean linear scale (for flattening tolerance). RU: Средний линейный масштаб (для допуска спрямления).
    #[must_use]
    pub fn mean_scale(&self) -> f32 {
        ((self.sx * self.sy - self.kx * self.ky).abs()).sqrt().max(1e-6)
    }
}

/// UZ: Oldindan ko'paytirilmagan RGBA rang, har kanal 0..1. EN: A non-premultiplied RGBA color, each channel 0..1. RU: Цвет RGBA без предумножения, каналы 0..1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    /// UZ: Qizil. EN: Red. RU: Красный.
    pub r: f32,
    /// UZ: Yashil. EN: Green. RU: Зелёный.
    pub g: f32,
    /// UZ: Ko'k. EN: Blue. RU: Синий.
    pub b: f32,
    /// UZ: Alfa. EN: Alpha. RU: Альфа.
    pub a: f32,
}

impl Color {
    /// UZ: 8 bitli kanallardan. EN: From 8-bit channels. RU: Из 8-битных каналов.
    #[must_use]
    pub fn from_rgba8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r: f32::from(r) / 255.0, g: f32::from(g) / 255.0, b: f32::from(b) / 255.0, a: f32::from(a) / 255.0 }
    }
}

/// UZ: Aralashtirish rejimi (Porter-Duff). EN: A blend mode (Porter-Duff). RU: Режим смешивания (Porter-Duff).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlendMode {
    /// UZ: Manba ustiga. EN: Source over destination. RU: Источник поверх.
    #[default]
    SourceOver,
    /// UZ: Manba ichida (manba * maqsad alfasi). EN: Source in destination. RU: Источник внутри приёмника.
    SourceIn,
    /// UZ: Maqsaddan manbani ayirish. EN: Destination out. RU: Приёмник вне источника.
    DestinationOut,
}

/// UZ: To'ldirish qoidasi. EN: A fill rule. RU: Правило заливки.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillRule {
    /// UZ: Nolga teng bo'lmagan o'rash. EN: Non-zero winding. RU: Ненулевая намотка.
    Winding,
    /// UZ: Juft-toq. EN: Even-odd. RU: Чётно-нечётное.
    EvenOdd,
}

/// UZ: Chiziq uchi. EN: A line cap. RU: Окончание линии.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineCap {
    /// UZ: Tekis. EN: Butt. RU: Плоское.
    #[default]
    Butt,
    /// UZ: Yumaloq. EN: Round. RU: Круглое.
    Round,
    /// UZ: Kvadrat. EN: Square. RU: Квадратное.
    Square,
}

/// UZ: Chiziq burchagi. EN: A line join. RU: Соединение линий.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineJoin {
    /// UZ: O'tkir. EN: Miter. RU: Острое.
    #[default]
    Miter,
    /// UZ: Chegaralangan o'tkir (limitdan oshsa kesiladi). EN: Miter, clipped at the limit. RU: Острое с отсечением по лимиту.
    MiterClip,
    /// UZ: Yumaloq. EN: Round. RU: Круглое.
    Round,
    /// UZ: Kesilgan. EN: Bevel. RU: Скошенное.
    Bevel,
}

/// UZ: Punktir naqshi: `[chiziq, bo'shliq, ...]` va siljish. EN: A dash pattern: `[on, off, ...]` and an offset. RU: Штриховой узор: `[линия, пропуск, ...]` и смещение.
#[derive(Debug, Clone, PartialEq)]
pub struct StrokeDash {
    pub(crate) array: Vec<f32>,
    pub(crate) offset: f32,
}

impl StrokeDash {
    /// UZ: Naqsh yasaydi; juft uzunlik, manfiy yoki cheksiz qiymat yoki nol yig'indida `None`. EN: Builds a pattern; `None` for odd length, negative/infinite values or a zero sum. RU: Создаёт узор; `None` при нечётной длине, отрицательных/бесконечных значениях или нулевой сумме.
    #[must_use]
    pub fn new(array: Vec<f32>, offset: f32) -> Option<Self> {
        if array.len() < 2 || array.len() % 2 != 0 || !offset.is_finite() {
            return None;
        }
        if array.iter().any(|v| !v.is_finite() || *v < 0.0) || array.iter().sum::<f32>() <= 0.0 {
            return None;
        }
        Some(Self { array, offset })
    }
}

/// UZ: Chiziq sozlamalari. EN: Stroke settings. RU: Параметры обводки.
#[derive(Debug, Clone, PartialEq)]
pub struct Stroke {
    /// UZ: Qalinlik. EN: Width. RU: Толщина.
    pub width: f32,
    /// UZ: O'tkir burchak chegarasi. EN: Miter limit. RU: Лимит острого угла.
    pub miter_limit: f32,
    /// UZ: Uchi. EN: Cap. RU: Окончание.
    pub line_cap: LineCap,
    /// UZ: Burchagi. EN: Join. RU: Соединение.
    pub line_join: LineJoin,
    /// UZ: Punktir. EN: Dash. RU: Штрих.
    pub dash: Option<StrokeDash>,
}

impl Default for Stroke {
    fn default() -> Self {
        Self { width: 1.0, miter_limit: 4.0, line_cap: LineCap::Butt, line_join: LineJoin::Miter, dash: None }
    }
}

/// UZ: Gradient chegarasidan tashqari. EN: Gradient spread outside its range. RU: Поведение градиента за пределами.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpreadMode {
    /// UZ: Chekka rang davom etadi. EN: The edge color continues. RU: Крайний цвет продолжается.
    Pad,
}

/// UZ: Gradient to'xtash nuqtasi. EN: A gradient stop. RU: Остановка градиента.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GradientStop {
    pub(crate) pos: f32,
    pub(crate) color: Color,
}

impl GradientStop {
    /// UZ: Joyi (0..1) va rangdan. EN: From a position (0..1) and a color. RU: По позиции (0..1) и цвету.
    #[must_use]
    pub fn new(pos: f32, color: Color) -> Self {
        Self { pos, color }
    }
}

/// UZ: Chiziqli gradient. EN: A linear gradient. RU: Линейный градиент.
#[derive(Debug, Clone, PartialEq)]
pub struct LinearGradient {
    pub(crate) start: Point,
    pub(crate) end: Point,
    pub(crate) stops: Vec<GradientStop>,
}

impl LinearGradient {
    /// UZ: Gradient shaderini yasaydi; to'xtash nuqtasi yo'q bo'lsa `None`. EN: Builds a gradient shader; `None` without stops. RU: Создаёт шейдер градиента; `None` без остановок.
    #[must_use]
    pub fn new(start: Point, end: Point, stops: Vec<GradientStop>, _mode: SpreadMode, _t: Transform) -> Option<Shader> {
        if stops.is_empty() || !start.x.is_finite() || !end.x.is_finite() || !start.y.is_finite() || !end.y.is_finite() {
            return None;
        }
        Some(Shader::LinearGradient(Self { start, end, stops: sorted(stops) }))
    }
}

/// UZ: Radial (fokusli) gradient. EN: A radial (focal) gradient. RU: Радиальный (фокальный) градиент.
#[derive(Debug, Clone, PartialEq)]
pub struct RadialGradient {
    pub(crate) focal: Point,
    pub(crate) center: Point,
    pub(crate) radius: f32,
    pub(crate) stops: Vec<GradientStop>,
}

impl RadialGradient {
    /// UZ: Fokus, boshlang'ich radius (e'tiborga olinmaydi, 0), markaz, radius va to'xtash nuqtalari. EN: Focal point, start radius (ignored, 0), center, radius and stops. RU: Фокус, начальный радиус (игнорируется, 0), центр, радиус и остановки.
    #[must_use]
    pub fn new(focal: Point, _start_radius: f32, center: Point, radius: f32, stops: Vec<GradientStop>, _mode: SpreadMode, _t: Transform) -> Option<Shader> {
        if stops.is_empty() || !(radius.is_finite() && radius > 0.0) {
            return None;
        }
        Some(Shader::RadialGradient(Self { focal, center, radius, stops: sorted(stops) }))
    }
}

fn sorted(mut stops: Vec<GradientStop>) -> Vec<GradientStop> {
    stops.sort_by(|a, b| a.pos.total_cmp(&b.pos));
    stops
}

/// UZ: Bo'yoq manbai. EN: A paint source. RU: Источник краски.
#[derive(Debug, Clone, PartialEq)]
pub enum Shader {
    /// UZ: Bir xil rang. EN: A solid color. RU: Сплошной цвет.
    SolidColor(Color),
    /// UZ: Chiziqli gradient. EN: A linear gradient. RU: Линейный градиент.
    LinearGradient(LinearGradient),
    /// UZ: Radial gradient. EN: A radial gradient. RU: Радиальный градиент.
    RadialGradient(RadialGradient),
}

/// UZ: Chizish bo'yog'i. EN: A paint. RU: Краска для рисования.
#[derive(Debug, Clone, PartialEq)]
pub struct Paint {
    /// UZ: Bo'yoq manbai. EN: Paint source. RU: Источник.
    pub shader: Shader,
    /// UZ: Aralashtirish rejimi. EN: Blend mode. RU: Режим смешивания.
    pub blend_mode: BlendMode,
    /// UZ: Qirralarni yumshatish (har doim yoqilgan). EN: Anti-aliasing (always on). RU: Сглаживание (всегда включено).
    pub anti_alias: bool,
}

impl Default for Paint {
    fn default() -> Self {
        Self { shader: Shader::SolidColor(Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }), blend_mode: BlendMode::SourceOver, anti_alias: false }
    }
}

impl Paint {
    /// UZ: Bir xil rang beradi (8 bitli kanallar). EN: Sets a solid color (8-bit channels). RU: Задаёт сплошной цвет (8-битные каналы).
    pub fn set_color_rgba8(&mut self, r: u8, g: u8, b: u8, a: u8) {
        self.shader = Shader::SolidColor(Color::from_rgba8(r, g, b, a));
    }
}

/// UZ: Pixmapni chizish sozlamalari. EN: Settings for drawing a pixmap. RU: Параметры рисования pixmap.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PixmapPaint {
    /// UZ: Shaffoflik 0..1. EN: Opacity 0..1. RU: Непрозрачность 0..1.
    pub opacity: f32,
    /// UZ: Aralashtirish rejimi. EN: Blend mode. RU: Режим смешивания.
    pub blend_mode: BlendMode,
}

impl Default for PixmapPaint {
    fn default() -> Self {
        Self { opacity: 1.0, blend_mode: BlendMode::SourceOver }
    }
}
