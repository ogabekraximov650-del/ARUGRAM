//! UZ: Elementlar daraxti: tavsif (qiymat) ko'rinishida, har kadrda qaytadan quriladi.
//! EN: The element tree: a description (plain values) that is rebuilt on every frame.
//! RU: Дерево элементов: описание (обычные значения), которое перестраивается на каждом кадре.

use crate::scroll::ScrollState;
use std::rc::Rc;
use uzcoders_core::{Color, EdgeInsets, State};
use uzcoders_layout::{Align, Axis, Dim, Justify, Style};
use uzcoders_render::{Image, Path};

/// UZ: Klaviatura turi (Android `inputType` ga aylanadi).
/// EN: Keyboard kind (converted to Android `inputType`).
/// RU: Вид клавиатуры (преобразуется в Android `inputType`).
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
    /// UZ: Android `InputType` qiymati.
    /// EN: The Android `InputType` value.
    /// RU: Значение Android `InputType`.
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
    Sprite {
        sheet: Rc<Image>,
        cols: u32,
        count: u32,
        fps: f32,
        playing: bool,
        w: f32,
        h: f32,
    },
    Field(Box<FieldCfg>),
}

/// UZ: Uzoq bosish bosqichi (`Element::on_hold`).
/// EN: A long-press phase (`Element::on_hold`).
/// RU: Фаза долгого нажатия (`Element::on_hold`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Hold {
    /// UZ: Uzoq bosish boshlandi. EN: The hold started. RU: Удержание началось.
    Start,
    /// UZ: Barmoq harakatda; boshlang'ich nuqtadan vertikal siljish (dp, tepaga manfiy). EN: The finger moved; vertical offset from the start point (dp, negative is up). RU: Палец движется; вертикальное смещение от начальной точки (dp, вверх — отрицательное).
    Move(f32),
    /// UZ: Barmoq qo'yib yuborildi (yoki bekor qilindi). EN: The finger was released (or cancelled). RU: Палец отпущен (или отменено).
    End(f32),
}

/// UZ: UI daraxti elementi: uslub, bolalar va chizish xossalari to'plami.
/// EN: A UI tree element: a bundle of style, children and drawing properties.
/// RU: Элемент дерева UI: набор стиля, детей и свойств рисования.
pub struct Element {
    pub(crate) kind: Kind,
    pub(crate) style: Style,
    pub(crate) bg: Option<Color>,
    pub(crate) radius: f32,
    pub(crate) border: Option<(f32, Color)>,
    pub(crate) glass: Option<Color>,
    pub(crate) backdrop: Option<(Color, f32, f32)>,
    pub(crate) gradient: Option<(Color, Color)>,
    /// UZ: Yumshoq yorug'lik dog'lari: (x ulushi, y ulushi, radius dp, rang) — element qutisiga nisbatan.
    /// EN: Soft glows: (x fraction, y fraction, radius in dp, color) relative to the element's box.
    /// RU: Мягкие световые пятна: (доля x, доля y, радиус в dp, цвет) относительно рамки элемента.
    pub(crate) glows: Vec<(f32, f32, f32, Color)>,
    pub(crate) on_click: Option<Callback>,
    pub(crate) on_swipe: Option<Rc<dyn Fn(i32)>>,
    pub(crate) on_hold: Option<(u64, Rc<dyn Fn(Hold)>)>,
    pub(crate) video: bool,
    pub(crate) on_drag: Option<Rc<dyn Fn(f32, bool)>>,
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
            on_hold: None,
            video: false,
            on_drag: None,
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
    /// UZ: Mavjud padding ustiga qo'shadi (almashtirmaydi). Scroll'da padding kontent bilan birga suriladi, shuning uchun
    /// ro'yxatni to'liq ekranga yoyib, tepa/pastga sarlavha va panel balandligicha qo'shsangiz, kontent ularning ostidan o'tadi.
    /// EN: Adds to the existing padding (does not replace it). In a scroll the padding moves with the content, so stretching a
    /// list full-screen and adding the header/bar heights at top/bottom lets the content pass under them.
    /// RU: Добавляет к существующему padding (не заменяет). В прокрутке padding двигается вместе с содержимым, поэтому список
    /// на весь экран с добавкой высот шапки/панели сверху/снизу позволяет содержимому проходить под ними.
    pub fn padding_add(mut self, left: f32, top: f32, right: f32, bottom: f32) -> Self {
        let p = self.style.padding;
        self.style.padding = EdgeInsets::new(p.left + left, p.top + top, p.right + right, p.bottom + bottom);
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
    /// UZ: "Shisha" fon (iOS uslubi): shaffof `tint`, yorug' chegara va ichki yaltirash.
    /// EN: A "glass" background (iOS style): translucent `tint`, a light border and an inner sheen.
    /// RU: Фон «стекло» (в стиле iOS): полупрозрачный `tint`, светлая граница и внутренний блеск.
    pub fn glass(mut self, tint: Color) -> Self {
        self.glass = Some(tint);
        self
    }
    /// UZ: HAQIQIY shisha: element ostidagi tasvir `blur` dp radiusda xiralashadi, ustiga `tint` va yorug' chegara qo'yiladi.
    /// EN: REAL glass: the image under the element is blurred with radius `blur` dp, with a `tint` and a light border on top.
    /// RU: НАСТОЯЩЕЕ стекло: изображение под элементом размывается с радиусом `blur` dp, сверху накладываются `tint` и светлая граница.
    pub fn backdrop(mut self, tint: Color, blur: f32) -> Self {
        self.backdrop = Some((tint, blur, 1.0));
        self
    }
    /// UZ: "Lupali" shisha (iOS Liquid Glass): deyarli shaffof; ostidagi tasvir panel markazidan `zoom` marta kattalashib ko'rinadi, chetida sinish bor.
    /// EN: "Lens" glass (iOS Liquid Glass): almost transparent; the image underneath is magnified `zoom` times from the panel's center, with refraction at the edge.
    /// RU: Стекло-«лупа» (iOS Liquid Glass): почти прозрачное; изображение под панелью увеличено в `zoom` раз от её центра, у края — преломление.
    pub fn lens(mut self, tint: Color, zoom: f32) -> Self {
        self.backdrop = Some((tint, 0.0, zoom));
        self
    }
    /// UZ: Element orqasiga yumshoq yorug'lik dog'lari qo'yadi: `(x ulushi, y ulushi, radius dp, rang)`.
    /// EN: Adds soft glows behind the element: `(x fraction, y fraction, radius in dp, color)`.
    /// RU: Добавляет мягкие световые пятна позади элемента: `(доля x, доля y, радиус в dp, цвет)`.
    pub fn glows(mut self, g: Vec<(f32, f32, f32, Color)>) -> Self {
        self.glows = g;
        self
    }
    /// UZ: Maydon sarlavhasi chegarada emas, maydon ichida tepada kichik bo'lib turadi (shaffof / shisha fon uchun qulay).
    /// EN: The field label sits small at the top inside the field instead of on the border (convenient for transparent / glass backgrounds).
    /// RU: Подпись поля располагается маленькой вверху внутри поля, а не на границе (удобно для прозрачного / стеклянного фона).
    pub fn label_inside(mut self) -> Self {
        if let Some(f) = self.field_cfg() {
            f.label_inside = true;
        }
        self
    }
    /// UZ: Vertikal gradient fon: tepadan pastga `top` -> `bottom` ranglari.
    /// EN: A vertical gradient background: colors `top` -> `bottom`, from top to bottom.
    /// RU: Вертикальный градиентный фон: цвета `top` -> `bottom` сверху вниз.
    pub fn gradient(mut self, top: Color, bottom: Color) -> Self {
        self.gradient = Some((top, bottom));
        self
    }
    /// UZ: Balandlik = kenglik x `ratio` (masalan, poster kartasi uchun 1.5).
    /// EN: Height = width x `ratio` (for example 1.5 for a poster card).
    /// RU: Высота = ширина x `ratio` (например, 1.5 для карточки-постера).
    pub fn aspect(mut self, ratio: f32) -> Self {
        self.style.aspect = ratio;
        self
    }
    /// UZ: Yumaloq burchakli kontur chizadi.
    /// EN: Draws a rounded-corner border.
    /// RU: Рисует контур со скруглёнными углами.
    pub fn border(mut self, width: f32, c: Color) -> Self {
        self.border = Some((width, c));
        self
    }
    /// UZ: Konturni butunlay olib tashlaydi (fokus bo'lganda ham chizilmaydi).
    /// EN: Removes the border entirely (it is not drawn even when focused).
    /// RU: Полностью убирает контур (не рисуется даже при фокусе).
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
    /// UZ: Qolgan bo'sh joyni `v` ulushda bo'lishadi (Compose'dagi `weight` kabi): bolalar kontentidan qat'i nazar.
    /// EN: Shares the remaining free space by the fraction `v` (like `weight` in Compose), regardless of the children's content.
    /// RU: Делит оставшееся свободное место в доле `v` (как `weight` в Compose), независимо от содержимого детей.
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
    /// UZ: Surish / bosish: barmoq elementning gorizontal qismida (0..1) qayerdaligi bilan chaqiriladi; ikkinchi argument — barmoq ko'tarildi.
    /// Sirg'anish chiziqlari (slider) uchun mo'ljallangan; bunday teginish ro'yxat scrollini yoki oddiy bosishni ishga tushirmaydi.
    /// EN: Drag / press: called with where the finger is along the element's width (0..1); the second argument is true when the finger is lifted.
    /// Meant for sliders; such a touch does not trigger list scrolling or a normal click.
    /// RU: Перетаскивание / нажатие: вызывается с положением пальца по ширине элемента (0..1); второй аргумент — палец поднят.
    /// Предназначено для слайдеров; такое касание не запускает прокрутку списка и обычный клик.
    pub fn on_drag(mut self, f: impl Fn(f32, bool) + 'static) -> Self {
        self.on_drag = Some(Rc::new(f));
        self
    }
    /// UZ: Dekoder video kadrlarini shu element maydoniga (GPU orqali) chizadi.
    /// EN: Draws decoder video frames into this element's area (via the GPU).
    /// RU: Рисует кадры видео от декодера в области этого элемента (через GPU).
    pub fn video(mut self) -> Self {
        self.video = true;
        self
    }
    /// UZ: Gorizontal surish: chapga (keyingi) `1`, o'ngga (oldingi) `-1` bilan chaqiriladi.
    /// EN: Horizontal swipe: called with `1` for a swipe left (next) and `-1` for right (previous).
    /// RU: Горизонтальный свайп: вызывается с `1` при свайпе влево (следующий) и `-1` при свайпе вправо (предыдущий).
    pub fn on_swipe(mut self, f: impl Fn(i32) + 'static) -> Self {
        self.on_swipe = Some(Rc::new(f));
        self
    }
    /// UZ: Uzoq bosish: barmoq `ms` millisekund siljimay tursa `Hold::Start`; keyin barmoq harakatida `Hold::Move(dy)` (boshlang'ich nuqtadan dp), qo'yib yuborilganda `Hold::End(dy)`.
    /// Uzoq bosish boshlangach oddiy bosish (`on_click`) va ro'yxat surilishi ishlamaydi. Barmoq `ms` dan oldin siljisa — oddiy surish/bosish.
    /// EN: Long press: `Hold::Start` when the finger stays still for `ms` milliseconds; then `Hold::Move(dy)` as it moves (dp from the start point) and `Hold::End(dy)` on release.
    /// Once the hold starts, the normal click (`on_click`) and list scrolling are suppressed. If the finger moves before `ms`, it is an ordinary scroll/tap.
    /// RU: Долгое нажатие: `Hold::Start`, когда палец неподвижен `ms` миллисекунд; затем `Hold::Move(dy)` при движении (dp от начальной точки) и `Hold::End(dy)` при отпускании.
    /// После начала удержания обычный клик (`on_click`) и прокрутка списка отключаются. Если палец сдвинулся раньше `ms` — обычная прокрутка/касание.
    pub fn on_hold(mut self, ms: u64, f: impl Fn(Hold) + 'static) -> Self {
        self.on_hold = Some((ms, Rc::new(f)));
        self
    }
    pub fn on_click(mut self, f: impl Fn() + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
    /// UZ: Asosiy o'q bo'yicha scroll qilinadigan qiladi. Balandlik cheklangan bo'lishi kerak
    /// (masalan, `.grow(1.0)` yoki `.height(..)`).
    /// EN: Makes the element scrollable along the main axis. Its height must be bounded
    /// (for example `.grow(1.0)` or `.height(..)`).
    /// RU: Делает элемент прокручиваемым по главной оси. Высота должна быть ограничена
    /// (например, `.grow(1.0)` или `.height(..)`).
    pub fn scroll(mut self, state: &ScrollState) -> Self {
        self.style.scroll = true;
        self.scroll = Some(state.clone());
        self
    }

    /// UZ: Matn yoki kiritish maydoni uchun shrift o'lchami.
    /// EN: Font size for text or an input field.
    /// RU: Размер шрифта для текста или поля ввода.
    pub fn size(mut self, s: f32) -> Self {
        match &mut self.kind {
            Kind::Text { size, .. } | Kind::Icon { size, .. } => *size = s,
            Kind::Image { .. } | Kind::Spinner { .. } | Kind::Sprite { .. } => {}
            Kind::Field(f) => f.size = s,
            Kind::Container => {}
        }
        self
    }
    /// UZ: Matn yoki kiritish maydoni matn rangi.
    /// EN: Text color for text or an input field.
    /// RU: Цвет текста для текста или поля ввода.
    pub fn color(mut self, c: Color) -> Self {
        match &mut self.kind {
            Kind::Text { color, .. } | Kind::Icon { color, .. } | Kind::Spinner { color, .. } => {
                *color = c
            }
            Kind::Field(f) => f.color = c,
            Kind::Container | Kind::Image { .. } | Kind::Sprite { .. } => {}
        }
        self
    }

    /// UZ: Shrift qalinligi (400 oddiy, 700 qalin, 900 qora); ilovada shunday yuz qo'shilgan bo'lsa ishlaydi.
    /// EN: Font weight (400 regular, 700 bold, 900 black); works if the app has added such a face.
    /// RU: Насыщенность шрифта (400 — обычный, 700 — жирный, 900 — чёрный); работает, если приложение добавило такое начертание.
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
    /// UZ: Harflar orasidagi qo'shimcha masofa (dp), Compose'dagi `letterSpacing` kabi.
    /// EN: Extra spacing between letters (dp), like `letterSpacing` in Compose.
    /// RU: Дополнительный интервал между буквами (dp), как `letterSpacing` в Compose.
    pub fn letter_spacing(mut self, sp: f32) -> Self {
        match &mut self.kind {
            Kind::Text { spacing, .. } => *spacing = sp,
            Kind::Field(f) => f.spacing = sp,
            _ => {}
        }
        self
    }
    /// UZ: Qator qutisi balandligi (dp). Matn qutida Compose'dagi kabi proporsional taqsimlanadi.
    /// EN: Line box height (dp). Text is distributed inside the box proportionally, as in Compose.
    /// RU: Высота строки (dp). Текст распределяется внутри строки пропорционально, как в Compose.
    pub fn line_height(mut self, h: f32) -> Self {
        match &mut self.kind {
            Kind::Text { line_height, .. } => *line_height = Some(h),
            Kind::Field(f) => f.line_height = Some(h),
            _ => {}
        }
        self
    }

    // UZ: ── Kiritish maydoni sozlamalari ──
    // EN: ── Input field settings ──
    // RU: ── Настройки поля ввода ──
    /// UZ: Suzuvchi sarlavha (Material "outlined" uslubi): bo'sh va fokussiz maydonda ichkarida, aks holda chegarada turadi.
    /// EN: A floating label (Material "outlined" style): inside the field when it is empty and unfocused, otherwise on the border.
    /// RU: Плавающая подпись (стиль Material «outlined»): внутри поля, когда оно пусто и без фокуса, иначе — на границе.
    pub fn label(mut self, l: impl Into<String>) -> Self {
        if let Some(f) = self.field_cfg() {
            f.label = l.into();
        }
        self
    }
    /// UZ: Suzuvchi sarlavha chegarani qirqib ko'rsatishi uchun orqa fon rangi.
    /// EN: Background color that lets the floating label cut through the border.
    /// RU: Цвет фона, чтобы плавающая подпись «прорезала» границу.
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
    /// UZ: Bo'sh maydonda ko'rinadigan maslahat matni (hint).
    /// EN: Hint text shown in an empty field.
    /// RU: Текст-подсказка (hint), видимый в пустом поле.
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
    /// UZ: Fokusdagi kontur va kursor rangi.
    /// EN: Color of the focused border and the cursor.
    /// RU: Цвет контура в фокусе и курсора.
    pub fn accent(mut self, c: Color) -> Self {
        if let Some(f) = self.field_cfg() {
            f.accent = c;
        }
        self
    }
    /// UZ: Ochiladigan klaviatura turi (raqamli, telefon va boshqalar).
    /// EN: The kind of keyboard to open (numeric, phone, and so on).
    /// RU: Вид открываемой клавиатуры (цифровая, телефонная и т. д.).
    pub fn keyboard(mut self, k: KeyboardKind) -> Self {
        if let Some(f) = self.field_cfg() {
            f.keyboard = k;
        }
        self
    }
    /// UZ: Matn nuqtalar bilan yashiriladi (parol maydoni).
    /// EN: The text is hidden behind dots (a password field).
    /// RU: Текст скрывается за точками (поле пароля).
    pub fn password(mut self) -> Self {
        if let Some(f) = self.field_cfg() {
            f.password = true;
            f.keyboard = KeyboardKind::Password;
        }
        self
    }
    /// UZ: Eng ko'p belgi soni (0 — cheklanmagan; `format` bo'lsa probellar sanalmaydi).
    /// EN: Maximum number of characters (0 means unlimited; with `format`, spaces are not counted).
    /// RU: Максимальное число символов (0 — без ограничения; при `format` пробелы не считаются).
    pub fn max_len(mut self, n: usize) -> Self {
        if let Some(f) = self.field_cfg() {
            f.max_len = n;
        }
        self
    }
    /// UZ: Faqat shu shartga mos belgilar kiritiladi.
    /// EN: Only characters that satisfy this predicate are accepted.
    /// RU: Принимаются только символы, удовлетворяющие этому условию.
    pub fn filter(mut self, p: fn(char) -> bool) -> Self {
        if let Some(f) = self.field_cfg() {
            f.filter = Some(p);
        }
        self
    }
    /// UZ: Har tahrirdan keyin matnni qayta formatlaydi (masalan, `901234567` -> `90 123 45 67`).
    /// EN: Reformats the text after every edit (for example `901234567` -> `90 123 45 67`).
    /// RU: Переформатирует текст после каждой правки (например, `901234567` -> `90 123 45 67`).
    pub fn format(mut self, fm: fn(&str) -> String) -> Self {
        if let Some(f) = self.field_cfg() {
            f.format = Some(fm);
        }
        self
    }
    /// UZ: Klaviaturada "Enter" bosilganda chaqiriladi.
    /// EN: Called when "Enter" is pressed on the keyboard.
    /// RU: Вызывается при нажатии «Enter» на клавиатуре.
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

/// UZ: Qatlam: bolalar bir-birining ustiga qo'yiladi (oxirgisi eng yuqorida), hammasi tugun maydonini to'ldiradi.
/// EN: A layer stack: children are placed on top of each other (the last one on top); all fill the node's area.
/// RU: Стек слоёв: дети накладываются друг на друга (последний сверху), все заполняют область узла.
pub fn stack(children: Vec<Element>) -> Element {
    let mut e = Element::container(Axis::Column, children);
    e.style.stack = true;
    e
}

/// UZ: Vertikal scroll qilinadigan ustun: `.grow(1.0)` bilan qolgan joyni egallaydi.
/// EN: A vertically scrollable column: with `.grow(1.0)` it takes the remaining space.
/// RU: Вертикально прокручиваемая колонка: с `.grow(1.0)` занимает оставшееся место.
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

/// UZ: Vektor ikonka: `d` — SVG yo'li (24x24 qutida, Material ikonkalari kabi), `size` — dp o'lchami. Rang `.color()` bilan beriladi.
/// EN: A vector icon: `d` is an SVG path (in a 24x24 box, like Material icons), `size` is in dp. The color is set with `.color()`.
/// RU: Векторная иконка: `d` — SVG-контур (в рамке 24x24, как иконки Material), `size` — размер в dp. Цвет задаётся через `.color()`.
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

/// UZ: Virtual ro'yxat: `count` ta bir xil balandlikdagi (`item_height`, dp) element, lekin faqat ko'rinadiganlari quriladi.
/// `build(i)` — i-elementni qaytaradi. Juda uzun ro'yxatlar ham tez ishlaydi.
/// EN: A virtualized list: `count` items of equal height (`item_height`, dp), but only the visible ones are built.
/// `build(i)` returns the i-th item. Even very long lists stay fast.
/// RU: Виртуальный список: `count` элементов одинаковой высоты (`item_height`, dp), но строятся только видимые.
/// `build(i)` возвращает i-й элемент. Даже очень длинные списки работают быстро.
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

/// UZ: Virtual ro'yxat, elementlar balandligi har xil (`heights`, dp): faqat ko'rinadiganlari (va 100 dp zaxira) quriladi.
/// `build(i)` — i-elementni qaytaradi (balandligi `heights[i]` ga majburlanadi).
/// EN: A virtualized list with items of different heights (`heights`, dp): only the visible ones (plus a 100 dp margin) are built.
/// `build(i)` returns the i-th item (its height is forced to `heights[i]`).
/// RU: Виртуальный список с элементами разной высоты (`heights`, dp): строятся только видимые (и запас 100 dp).
/// `build(i)` возвращает i-й элемент (его высота принудительно равна `heights[i]`).
pub fn lazy_column_var(state: &ScrollState, heights: &[f32], build: impl Fn(usize) -> Element) -> Element {
    let d = uzcoders_core::density().max(0.1);
    let off = state.offset() / d;
    let vp = if state.viewport() > 0.0 { state.viewport() / d } else { 3000.0 };
    let mut prefix: Vec<f32> = Vec::with_capacity(heights.len() + 1);
    prefix.push(0.0);
    for h in heights {
        prefix.push(prefix.last().copied().unwrap_or(0.0) + h);
    }
    let total = prefix.last().copied().unwrap_or(0.0);
    let lo = (off - 100.0).max(0.0);
    let hi = off + vp + 100.0;
    // UZ: Birinchi ko'rinadigan element: oxiri `lo` dan keyin bo'lgan eng birinchisi. EN: First visible item: the first whose end is past `lo`. RU: Первый видимый элемент: первый, чей конец за `lo`.
    let first = prefix.partition_point(|p| *p <= lo).saturating_sub(1).min(heights.len());
    let last = prefix.partition_point(|p| *p < hi).min(heights.len());
    let last = last.max(first);
    let mut kids = Vec::with_capacity(last - first + 2);
    if first > 0 {
        kids.push(gap_box(0.0, prefix[first]));
    }
    for i in first..last {
        kids.push(build(i).height(heights[i]));
    }
    if last < heights.len() {
        kids.push(gap_box(0.0, total - prefix[last]));
    }
    column(kids).scroll(state)
}

/// UZ: Aylanuvchi yuklanish ko'rsatkichi (Material `CircularProgressIndicator` kabi): `size` dp, chiziq qalinligi `width` dp.
/// EN: A rotating loading spinner (like Material `CircularProgressIndicator`): `size` dp, stroke width `width` dp.
/// RU: Вращающийся индикатор загрузки (как Material `CircularProgressIndicator`): `size` dp, толщина линии `width` dp.
pub fn spinner(size: f32, width: f32, color: Color) -> Element {
    let mut e = Element::new(
        Kind::Spinner { size, width, color },
        Style::default(),
        Vec::new(),
    );
    e.style.shrink = 0.0;
    e
}

/// UZ: Rasm: `w` x `h` dp o'lchamida (rasm shu qutiga cho'ziladi).
/// EN: An image of `w` x `h` dp (the image is stretched into this box).
/// RU: Изображение размером `w` x `h` dp (изображение растягивается на эту рамку).
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

/// UZ: Animatsiyali rasm: `sheet` — kadrlar yonma-yon turgan varaq (`cols` ustun, jami `count` kadr, qatorma-qator), `fps` — tezlik.
/// `playing = false` bo'lsa birinchi kadr turadi (animatsiya yurmaydi, qayta chizish talab qilinmaydi). `w` x `h` dp.
/// EN: An animated image: `sheet` holds the frames side by side (`cols` columns, `count` frames in total, row by row), `fps` is the speed.
/// With `playing = false` the first frame is shown (no animation, no redraw requests). `w` x `h` dp.
/// RU: Анимированное изображение: `sheet` — лист с кадрами рядом (`cols` столбцов, всего `count` кадров, по строкам), `fps` — скорость.
/// При `playing = false` показывается первый кадр (без анимации и без запросов перерисовки). `w` x `h` dp.
pub fn sprite(sheet: &Rc<Image>, cols: u32, count: u32, fps: f32, playing: bool, w: f32, h: f32) -> Element {
    let mut e = Element::new(
        Kind::Sprite {
            sheet: sheet.clone(),
            cols: cols.max(1),
            count: count.max(1),
            fps,
            playing,
            w,
            h,
        },
        Style::default(),
        Vec::new(),
    );
    e.style.shrink = 0.0;
    e
}

/// UZ: Rasm, ota-elementning berilgan maydonini to'ldiradi (`stack` ichida yoki `.aspect()` bilan ishlatiladi).
/// EN: An image that fills the parent's given area (used inside a `stack` or with `.aspect()`).
/// RU: Изображение, заполняющее отведённую область родителя (используется внутри `stack` или с `.aspect()`).
pub fn image_fill(img: &Rc<Image>) -> Element {
    let mut e = image(img, 0.0, 0.0);
    e.style.shrink = 0.0;
    e
}

/// UZ: Bo'sh joyni to'ldiradigan elastik element.
/// EN: An elastic element that fills the free space.
/// RU: Эластичный элемент, заполняющий свободное место.
pub fn spacer() -> Element {
    Element::container(Axis::Column, Vec::new()).grow(1.0)
}

/// UZ: Qat'iy o'lchamli bo'sh joy (padding o'rniga oraliq uchun).
/// EN: A fixed-size empty gap (for spacing, instead of padding).
/// RU: Пустой промежуток фиксированного размера (для интервалов вместо padding).
pub fn gap_box(w: f32, h: f32) -> Element {
    Element::container(Axis::Column, Vec::new())
        .width(w)
        .height(h)
}

/// UZ: Tayyor tugma: yumaloq fon, o'rtada matn; bosilganda `f` chaqiriladi.
/// EN: A ready-made button: rounded background, centered text; `f` is called when pressed.
/// RU: Готовая кнопка: скруглённый фон, текст по центру; при нажатии вызывается `f`.
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

/// UZ: Matn kiritish maydoni: qiymat `state` da saqlanadi. Bosilsa fokus oladi va klaviatura ochiladi.
/// EN: A text input field: its value is stored in `state`. When tapped it gets focus and opens the keyboard.
/// RU: Поле ввода текста: значение хранится в `state`. При нажатии получает фокус и открывает клавиатуру.
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

/// UZ: Teng ustunli to'r: `columns` ustun, bolalar qatorma-qator joylashadi (oxirgi qator bo'sh kataklar bilan to'ldiriladi).
/// EN: An equal-column grid: `columns` columns, children laid out row by row (the last row is padded with empty cells).
/// RU: Сетка с равными колонками: `columns` колонок, дети раскладываются по строкам (последняя строка добивается пустыми ячейками).
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
