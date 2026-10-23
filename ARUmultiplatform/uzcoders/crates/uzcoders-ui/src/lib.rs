//! UZ: uzcoders-ui — deklarativ elementlar daraxti, joylashtirish, chizish, scroll, matn kiritish va navigatsiya.
//! EN: uzcoders-ui — a declarative element tree, layout, drawing, scrolling, text input and navigation.
//! RU: uzcoders-ui — декларативное дерево элементов, раскладка, рисование, прокрутка, ввод текста и навигация.

mod element;
mod nav;
mod scroll;

use element::{Callback, FieldCfg, Kind};
use std::rc::Rc;
use uzcoders_core::{
    global_revision, Color, EdgeInsets, Point, PointerEvent, PointerKind, Rect, Size,
};
use uzcoders_layout::{Axis, Dim, Tree};
use uzcoders_render::{Canvas, DrawCmd, Image, Path};
use uzcoders_text::Font;

pub use element::Hold;
pub use element::{
    button, column, field, gap_box, grid, icon, image, image_fill, lazy_column, lazy_column_var, row, scroll_column,
    spacer, sprite, spinner, stack, text, Element, KeyboardKind,
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
    Image(Rc<Image>, f32),
    Sprite(Rc<Image>, u32, u32, f32, bool),
    Spinner(f32, Color),
    Field(Box<FieldCfg>),
    ScrollEnd,
}

struct Item {
    node: usize,
    kind: ItemKind,
    bg: Option<Color>,
    radius: f32,
    border: Option<(f32, Color)>,
    glass: Option<Color>,
    backdrop: Option<(Color, f32, f32)>,
    gradient: Option<(Color, Color)>,
    glows: Vec<(f32, f32, f32, Color)>,
    hit: Option<usize>,
    swipe: Option<Rc<dyn Fn(i32)>>,
    hold: Option<(u64, Rc<dyn Fn(Hold)>)>,
    video: bool,
    drag: Option<Rc<dyn Fn(f32, bool)>>,
    scroll: Option<ScrollState>,
    /// UZ: Qator `.scroll()` bilan: gorizontal scroll.
    /// EN: A row with `.scroll()`: horizontal scrolling.
    /// RU: Строка с `.scroll()`: горизонтальная прокрутка.
    hscroll: bool,
}

struct Hit {
    rect: Rect,
    cb: Callback,
}

struct Region {
    rect: Rect,
    state: ScrollState,
    horizontal: bool,
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
    /// UZ: Gorizontal surish aniqlandi (`on_swipe`).
    /// EN: A horizontal swipe was detected (`on_swipe`).
    /// RU: Обнаружен горизонтальный свайп (`on_swipe`).
    swiped: bool,
    /// UZ: Sirg'anish chizig'i (`on_drag`) indeksi.
    /// EN: Index of the slider (`on_drag`).
    /// RU: Индекс слайдера (`on_drag`).
    drag: Option<usize>,
    /// UZ: Uzoq bosish (`on_hold`) holati; qayta chaqirish o'zi saqlanadi (daraxt qayta qurilganda ham to'g'ri ishlashi uchun).
    /// EN: The long-press (`on_hold`) state; the callback itself is kept (so it stays correct when the tree is rebuilt).
    /// RU: Состояние долгого нажатия (`on_hold`); сам обработчик хранится (чтобы работать верно при перестроении дерева).
    hold: Option<HoldState>,
    /// UZ: So'nggi namunalar: (vaqt ms, scroll o'qi bo'yicha koordinata) — tezlikni baholash uchun.
    /// EN: Recent samples: (time in ms, coordinate along the scroll axis), used to estimate velocity.
    /// RU: Последние отсчёты: (время в мс, координата по оси прокрутки) — для оценки скорости.
    samples: Vec<(u64, f32)>,
}

#[derive(Clone)]
struct HoldState {
    cb: Rc<dyn Fn(Hold)>,
    ms: u64,
    start: u64,
    fired: bool,
}

/// UZ: Ilova ish vaqti: elementlarni quradi, joylaydi, chizish ro'yxatini beradi va kiritishni yo'naltiradi.
/// EN: The app runtime: builds and lays out elements, produces the draw list and routes input.
/// RU: Среда выполнения приложения: строит и раскладывает элементы, выдаёт список рисования и направляет ввод.
pub struct Ui {
    font: Rc<Font>,
    faces: Vec<(u16, Rc<Font>)>,
    insets_px: EdgeInsets,
    kb_serial: u64,
    build: Box<dyn FnMut() -> Element>,
    hits: Vec<Hit>,
    swipes: Vec<(Rect, Rc<dyn Fn(i32)>)>,
    holds: Vec<(Rect, u64, Rc<dyn Fn(Hold)>)>,
    drags: Vec<(Rect, Rc<dyn Fn(f32, bool)>)>,
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
    /// UZ: IME (klaviatura) dagi tugallanmagan matn oralig'i: (boshlanish, uzunlik), belgilarda.
    /// EN: The IME (keyboard) composing-text range: (start, length), in characters.
    /// RU: Диапазон незавершённого текста IME (клавиатуры): (начало, длина) в символах.
    composing: Option<(usize, usize)>,
    /// UZ: Oxirgi kadrda animatsiyali element (spinner) bor edi: uzluksiz chizish kerak.
    /// EN: The last frame had an animated element (a spinner): continuous redrawing is needed.
    /// RU: На последнем кадре был анимированный элемент (спиннер): нужна непрерывная перерисовка.
    animating: bool,
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
            swipes: Vec::new(),
            holds: Vec::new(),
            drags: Vec::new(),
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
            composing: None,
            animating: false,
        }
    }

    pub fn font(&self) -> Rc<Font> {
        self.font.clone()
    }

    /// UZ: Barcha shrift yuzlari: (qalinlik, shrift); birinchisi — asosiy (400).
    /// EN: All font faces: (weight, font); the first is the base one (400).
    /// RU: Все начертания шрифта: (насыщенность, шрифт); первое — основное (400).
    pub fn faces(&self) -> Vec<(u16, Rc<Font>)> {
        let mut v = vec![(400, self.font.clone())];
        v.extend(self.faces.iter().cloned());
        v
    }

    /// UZ: Qo'shimcha shrift yuzini qo'shadi (masalan, `add_font(700, bold)`). Oddiy (400) yuz `Ui::new` da beriladi.
    /// EN: Adds an extra font face (for example `add_font(700, bold)`). The regular (400) face is passed to `Ui::new`.
    /// RU: Добавляет дополнительное начертание (например, `add_font(700, bold)`). Обычное (400) передаётся в `Ui::new`.
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

    /// UZ: Shu `Ui` shriftlari bilan tayyorlangan canvas.
    /// EN: A canvas prepared with this `Ui`'s fonts.
    /// RU: Canvas, подготовленный со шрифтами этого `Ui`.
    pub fn canvas(&self, width: usize, height: usize) -> Canvas {
        let mut c = Canvas::new(width, height, self.font.clone());
        for (w, f) in &self.faces {
            c.add_face(*w, f.clone());
        }
        c
    }

    /// UZ: Tizim panellari chekinishlari (piksel): status bar tepada, navigatsiya paneli pastda. Ilova ularni
    /// `uzcoders_core::safe_insets()` (dp) orqali oladi.
    /// EN: System bar insets (pixels): the status bar on top, the navigation bar at the bottom. The app reads them
    /// through `uzcoders_core::safe_insets()` (dp).
    /// RU: Отступы системных панелей (пиксели): статус-бар сверху, навигационная панель снизу. Приложение получает их
    /// через `uzcoders_core::safe_insets()` (dp).
    pub fn set_insets(&mut self, insets_px: EdgeInsets) {
        if insets_px != self.insets_px {
            self.insets_px = insets_px;
            self.dirty = true;
        }
    }

    /// UZ: Kiritish maydoniga har bosilganda oshadi: qobiq klaviaturani qayta ko'rsatishi uchun.
    /// EN: Increases every time an input field is tapped, so the shell can show the keyboard again.
    /// RU: Увеличивается при каждом нажатии на поле ввода, чтобы оболочка снова показала клавиатуру.
    pub fn keyboard_serial(&self) -> u64 {
        self.kb_serial
    }

    /// UZ: Zichlik koeffitsienti (dp -> piksel). Barcha o'lchamlar shunga ko'paytiriladi.
    /// EN: Density factor (dp -> pixel). All sizes are multiplied by it.
    /// RU: Коэффициент плотности (dp -> пиксели). На него умножаются все размеры.
    pub fn set_scale(&mut self, scale: f32) {
        if (scale - self.scale).abs() > f32::EPSILON {
            self.scale = scale.max(0.1);
            self.dirty = true;
        }
    }

    /// UZ: Joriy vaqt (millisekund, monoton). Kursor miltillashi va inersiya shunga tayanadi; qobiq uni har tsiklda chaqiradi.
    /// EN: Current time (milliseconds, monotonic). Cursor blinking and fling depend on it; the shell calls it every loop.
    /// RU: Текущее время (миллисекунды, монотонное). От него зависят мигание курсора и инерция; оболочка вызывает его в каждом цикле.
    pub fn set_time(&mut self, ms: u64) {
        self.now = ms;
        // UZ: Uzoq bosish muddati tugadimi. EN: Has the long-press time elapsed. RU: Истекло ли время долгого нажатия.
        let fire = match &mut self.touch {
            Some(t) if !t.dragging && !t.swiped && t.drag.is_none() => match &mut t.hold {
                Some(h) if !h.fired && ms >= h.start + h.ms => {
                    h.fired = true;
                    Some(h.cb.clone())
                }
                _ => None,
            },
            _ => None,
        };
        if let Some(cb) = fire {
            self.pressed = None;
            self.dirty = true;
            cb(Hold::Start);
        }
    }

    /// UZ: "Orqaga" bosilganda chaqiriladi; `true` qaytarilsa — qayta ishlandi (ilova yopilmaydi).
    /// EN: Called when "Back" is pressed; returning `true` means it was handled (the app does not close).
    /// RU: Вызывается при нажатии «Назад»; `true` — обработано (приложение не закрывается).
    pub fn set_back_handler(&mut self, f: impl Fn() -> bool + 'static) {
        self.back = Some(Rc::new(f));
    }

    fn blink_phase(&self) -> u64 {
        self.now.saturating_sub(self.caret_epoch) / BLINK_MS
    }

    /// UZ: Qayta chizish kerakmi (holat o'zgardi, inersiya, kursor miltillashi)?
    /// EN: Is a redraw needed (state changed, fling, cursor blink, a frame was requested from another thread)?
    /// RU: Нужна ли перерисовка (изменилось состояние, инерция, мигание курсора, кадр запрошен из другого потока)?
    pub fn needs_frame(&self) -> bool {
        self.dirty
            || uzcoders_core::frame_requested()
            || self.last_rev != Some(global_revision())
            || self.animating
            || self.scrolls.iter().any(|s| s.velocity() != 0.0)
            || (self.focus.is_some() && self.blink_phase() != self.last_blink)
    }

    /// UZ: Keyingi uyg'onishgacha necha millisekund kutish mumkin (`None` — hodisagacha cheksiz).
    /// EN: How many milliseconds we can sleep until the next wake-up (`None` means until an event arrives).
    /// RU: Сколько миллисекунд можно ждать до следующего пробуждения (`None` — до события, без ограничения).
    pub fn next_wakeup_ms(&self) -> Option<u64> {
        if self.animating || self.scrolls.iter().any(|s| s.velocity() != 0.0) {
            return Some(0);
        }
        // UZ: Kutilayotgan uzoq bosish muddatigacha uyg'onish. EN: Wake up for a pending long press. RU: Пробуждение к сроку ожидающего долгого нажатия.
        if let Some(Touch { hold: Some(h), .. }) = &self.touch {
            if !h.fired {
                return Some((h.start + h.ms).saturating_sub(self.now));
            }
        }
        self.focus
            .map(|_| BLINK_MS - self.now.saturating_sub(self.caret_epoch) % BLINK_MS)
    }

    /// UZ: Fokusdagi maydonning klaviatura turi.
    /// EN: The keyboard kind of the focused field.
    /// RU: Вид клавиатуры поля в фокусе.
    pub fn keyboard_kind(&self) -> KeyboardKind {
        self.focused_index()
            .map_or(KeyboardKind::Text, |i| self.fields[i].cfg.keyboard)
    }

    /// UZ: Fokusni olib tashlaydi (masalan, klaviatura tizim tomonidan yopilganda).
    /// EN: Clears focus (for example, when the system closed the keyboard).
    /// RU: Снимает фокус (например, когда система закрыла клавиатуру).
    pub fn clear_focus(&mut self) {
        if self.focus.take().is_some() {
            self.composing = None;
            self.dirty = true;
        }
    }

    /// UZ: Klaviatura ko'rsatilishi kerakmi (kiritish maydoni fokusda bo'lsa).
    /// EN: Whether the keyboard should be shown (when an input field has focus).
    /// RU: Нужно ли показывать клавиатуру (если поле ввода в фокусе).
    pub fn wants_keyboard(&self) -> bool {
        self.focus.is_some()
    }

    /// UZ: Kadrni quradi: `size` — oyna o'lchami (piksel). Chizish buyruqlari ro'yxati qaytadi.
    /// EN: Builds a frame: `size` is the window size (pixels). Returns the list of draw commands.
    /// RU: Строит кадр: `size` — размер окна (пиксели). Возвращает список команд рисования.
    pub fn frame(&mut self, size: Size) -> Vec<DrawCmd> {
        self.last_rev = Some(global_revision());
        self.dirty = false;
        uzcoders_core::clear_frame_request();
        let dt = match self.last_frame_now {
            Some(t) => (self.now.saturating_sub(t) as f32 / 1000.0).min(0.1),
            None => 0.0,
        };
        self.last_frame_now = Some(self.now);
        self.step_flings(dt);
        self.last_blink = self.blink_phase();
        uzcoders_core::set_density(self.scale);
        uzcoders_core::set_window_dp(size.w / self.scale, size.h / self.scale);
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
        check_overflow(&tree, &items, self.scale);

        self.hits.clear();
        self.swipes.clear();
        self.holds.clear();
        self.drags.clear();
        self.fields.clear();
        self.regions.clear();
        self.scrolls.clear();
        self.animating = false;
        let mut cmds = Vec::new();
        let mut shift = (0.0f32, 0.0f32);
        let mut shifts: Vec<(f32, f32)> = Vec::new();
        let mut clips: Vec<Rect> = Vec::new();
        let caret_on = self.blink_phase() % 2 == 0;

        for it in items {
            if let ItemKind::ScrollEnd = it.kind {
                cmds.push(DrawCmd::PopClip);
                clips.pop();
                shift = shifts.pop().unwrap_or((0.0, 0.0));
                continue;
            }
            let r = tree.rect(it.node).translate(shift.0, shift.1);
            let clip = clips.last().copied();
            let eff = clip.map_or(r, |c| r.intersect(&c));
            let visible = !eff.is_empty();
            if let Some(h) = it.hit {
                self.hits.push(Hit {
                    rect: eff,
                    cb: cbs[h].clone(),
                });
            }
            if let Some(cb) = &it.drag {
                self.drags.push((eff, cb.clone()));
            }
            if let Some(cb) = &it.swipe {
                self.swipes.push((eff, cb.clone()));
            }
            if let Some((ms, cb)) = &it.hold {
                self.holds.push((eff, *ms, cb.clone()));
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
            if visible && it.video {
                cmds.push(DrawCmd::Video { rect: r });
            }
            if visible {
                if let Some((top, bottom)) = it.gradient {
                    cmds.push(DrawCmd::Gradient {
                        rect: r,
                        radius: it.radius,
                        top,
                        bottom,
                    });
                }
                if let Some((tint, blur, zoom)) = it.backdrop {
                    cmds.push(DrawCmd::Backdrop {
                        rect: r,
                        radius: it.radius,
                        blur,
                        zoom,
                        tint,
                    });
                }
                if let Some(t) = it.glass {
                    cmds.push(DrawCmd::Glass {
                        rect: r,
                        radius: it.radius,
                        tint: t,
                    });
                }
                for &(fx, fy, rad, col) in &it.glows {
                    cmds.push(DrawCmd::Glow {
                        cx: r.x + fx * r.w,
                        cy: r.y + fy * r.h,
                        radius: rad,
                        color: col,
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
                ItemKind::Image(image, opacity) if visible => {
                    cmds.push(DrawCmd::Image {
                        image,
                        rect: r,
                        opacity,
                    });
                }
                ItemKind::Sprite(sheet, cols, count, fps, playing) if visible => {
                    let rows = count.div_ceil(cols).max(1);
                    let (cw, ch) = (sheet.w / cols, sheet.h / rows);
                    let frame = if playing && fps > 0.0 {
                        (((self.now as f32) * fps / 1000.0) as u32) % count
                    } else {
                        0
                    };
                    cmds.push(DrawCmd::ImageRegion {
                        src: ((frame % cols) * cw, (frame / cols) * ch, cw, ch),
                        image: sheet,
                        rect: r,
                        opacity: 1.0,
                    });
                    if playing {
                        self.animating = true;
                    }
                }
                ItemKind::Spinner(width, color) if visible => {
                    // UZ: Material indeterminate: aylanish + yoy uzunligi 10%..75% oralig'ida o'zgaradi.
                    // EN: Material indeterminate style: rotation plus an arc length that changes between 10% and 75%.
                    // RU: Material indeterminate: вращение плюс длина дуги, меняющаяся от 10% до 75%.
                    let t = self.now as f32 / 1000.0;
                    let tau = std::f32::consts::TAU;
                    let phase = 0.5 + 0.5 * (t * tau / 1.33).sin();
                    let sweep = tau * (0.10 + 0.65 * phase);
                    let start = t * tau / 1.4 - tau * 0.25 + (1.0 - phase) * tau * 0.4;
                    cmds.push(DrawCmd::Arc {
                        cx: r.x + r.w * 0.5,
                        cy: r.y + r.h * 0.5,
                        radius: r.w.min(r.h) * 0.5,
                        width,
                        start,
                        sweep,
                        color,
                    });
                    self.animating = true;
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
                        if let Some((w, c)) = it.border.filter(|(w, _)| *w > 0.0) {
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
                        if floated && cfg.label_inside {
                            // UZ: Maydon ichida tepada kichik sarlavha (shaffof / shisha fon uchun).
                            // EN: A small label at the top inside the field (for transparent / glass backgrounds).
                            // RU: Маленькая подпись вверху внутри поля (для прозрачного / стеклянного фона).
                            let ls = cfg.size * 0.75;
                            let (_, lbase) = line_metrics(&self.font, ls, None);
                            cmds.push(DrawCmd::Text {
                                x: inner_x,
                                baseline: r.y + 8.0 * self.scale + lbase,
                                text: cfg.label.clone(),
                                size: ls,
                                color: label_color,
                                weight: 400,
                                spacing: cfg.spacing * 0.8,
                            });
                        } else if floated {
                            // UZ: Chegarada turgan kichik sarlavha: ostidagi chiziq qirqiladi.
                            // EN: A small label sitting on the border: the line underneath is cut out.
                            // RU: Маленькая подпись на границе: линия под ней прорезается.
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
                // UZ: Pastdagi padding ham ko'rinishi uchun: kontent uzunligi = padding.top + extent + padding.bottom.
                // EN: So the bottom padding is visible too: content length = padding.top + extent + padding.bottom.
                // RU: Чтобы нижний padding тоже был виден: длина содержимого = padding.top + extent + padding.bottom.
                let (max, vp) = if it.hscroll {
                    (tree.content_extent(it.node) + pad.horizontal() - r.w, r.w)
                } else {
                    (tree.content_extent(it.node) + pad.vertical() - r.h, r.h)
                };
                sc.set_max(max);
                if sc.set_viewport(vp) {
                    self.dirty = true;
                }
                self.regions.push(Region {
                    rect: eff,
                    state: sc.clone(),
                    horizontal: it.hscroll,
                });
                let off = sc.offset();
                self.scrolls.push(sc);
                cmds.push(DrawCmd::PushClip(r));
                clips.push(eff);
                shifts.push(shift);
                if it.hscroll {
                    shift.0 -= off;
                } else {
                    shift.1 -= off;
                }
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
            let nv = v * (-dt * 1.1).exp();
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
        if let Some((_, b, _)) = &mut el.backdrop {
            *b *= k;
        }
        for g in &mut el.glows {
            g.2 *= k;
        }
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
            Kind::Spinner { size, width, .. } => {
                *size *= k;
                *width *= k;
            }
            Kind::Image { w, h, .. } | Kind::Sprite { w, h, .. } => {
                *w *= k;
                *h *= k;
            }
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
            glass: el.glass,
            backdrop: el.backdrop,
            gradient: el.gradient,
            glows: std::mem::take(&mut el.glows),
            hit,
            swipe: el.on_swipe.take(),
            hold: el.on_hold.take(),
            video: el.video,
            drag: el.on_drag.take(),
            hscroll: el.scroll.is_some() && el.style.direction == Axis::Row,
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
                // UZ: Joylashtirish bir tugunni bir necha marta o'lchaydi, shuning uchun oxirgi natija eslab qolinadi (memo).
                // EN: Layout measures a node several times, so the last result is remembered (memo).
                // RU: Раскладка измеряет один узел несколько раз, поэтому последний результат запоминается (memo).
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
            Kind::Image {
                image,
                w,
                h,
                opacity,
            } => {
                items[slot].kind = ItemKind::Image(image, opacity);
                tree.add_leaf(el.style, move |_| Size::new(w, h))
            }
            Kind::Sprite {
                sheet,
                cols,
                count,
                fps,
                playing,
                w,
                h,
            } => {
                items[slot].kind = ItemKind::Sprite(sheet, cols, count, fps, playing);
                tree.add_leaf(el.style, move |_| Size::new(w, h))
            }
            Kind::Spinner { size, width, color } => {
                items[slot].kind = ItemKind::Spinner(width, color);
                tree.add_leaf(el.style, move |_| Size::new(size, size))
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
                glass: None,
                backdrop: None,
                gradient: None,
                glows: Vec::new(),
                hit: None,
                swipe: None,
                hold: None,
                video: false,
                drag: None,
                hscroll: false,
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

    /// UZ: Teginish / sichqoncha hodisasini qayta ishlaydi.
    /// EN: Handles a touch / mouse event.
    /// RU: Обрабатывает событие касания / мыши.
    pub fn pointer(&mut self, e: PointerEvent) {
        let p = e.pos;
        match e.kind {
            PointerKind::Down => {
                if let Some(di) = self.drags.iter().rposition(|(r, _)| r.contains(p)) {
                    // UZ: Sirg'anish chizig'i (slider): oddiy bosish ham qiymat beradi; ro'yxat scrolli va oddiy bosish ishga tushmaydi.
                    // EN: Slider: a plain press also gives a value; list scrolling and a normal click are not triggered.
                    // RU: Слайдер: даже простое нажатие задаёт значение; прокрутка списка и обычный клик не срабатывают.
                    let (r, cb) = self.drags[di].clone();
                    let cb2 = cb.clone();
                    self.touch = Some(Touch {
                        start: p,
                        last: p,
                        field: None,
                        region: None,
                        dragging: false,
                        swiped: false,
                        drag: Some(di),
                        hold: None,
                        samples: Vec::new(),
                    });
                    self.pressed = None;
                    cb2(((p.x - r.x) / r.w.max(1.0)).clamp(0.0, 1.0), false);
                    self.dirty = true;
                    return;
                }
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
                    swiped: false,
                    drag: self.drags.iter().rposition(|(r, _)| r.contains(p)),
                    hold: self.holds.iter().rev().find(|(r, _, _)| r.contains(p)).map(|(_, ms, cb)| HoldState {
                        cb: cb.clone(),
                        ms: *ms,
                        start: if e.time_ms > 0 { e.time_ms } else { self.now },
                        fired: false,
                    }),
                    samples: vec![(
                        if e.time_ms > 0 { e.time_ms } else { self.now },
                        region.map_or(p.y, |ri| if self.regions[ri].horizontal { p.x } else { p.y }),
                    )],
                });
                self.dirty = true;
            }
            PointerKind::Move => {
                if let Some(t) = &mut self.touch {
                    if let Some(di) = t.drag {
                        if let Some((r, cb)) = self.drags.get(di) {
                            let fx = ((p.x - r.x) / r.w.max(1.0)).clamp(0.0, 1.0);
                            let cb = cb.clone();
                            t.last = p;
                            cb(fx, false);
                            self.dirty = true;
                        }
                        return;
                    }
                }
                let slop = SLOP_DP * self.scale;
                let mut unpress = false;
                // UZ: Uzoq bosish: boshlangan bo'lsa barmoq harakati unga uzatiladi (surish ishlamaydi); boshlanmagan bo'lsa, siljish uni bekor qiladi.
                // EN: Long press: once started, finger motion goes to it (no scrolling); if not started yet, moving cancels it.
                // RU: Долгое нажатие: если началось — движение пальца передаётся ему (прокрутки нет); если ещё нет — сдвиг его отменяет.
                if let Some(t) = &mut self.touch {
                    if let Some(h) = &t.hold {
                        if h.fired {
                            let cb = h.cb.clone();
                            t.last = p;
                            let dy = (p.y - t.start.y) / self.scale.max(0.01);
                            cb(Hold::Move(dy));
                            self.dirty = true;
                            return;
                        }
                        if (p.x - t.start.x).hypot(p.y - t.start.y) > slop {
                            t.hold = None;
                        }
                    }
                }
                if let Some(t) = &mut self.touch {
                    let (adx, ady) = ((p.x - t.start.x).abs(), (p.y - t.start.y).abs());
                    if !t.dragging && !t.swiped && adx > slop * 1.5 && adx > ady * 1.5 && t.region.map_or(true, |ri| !self.regions[ri].horizontal) && self.swipes.iter().any(|(r, _)| r.contains(t.start)) {
                        t.swiped = true;
                        unpress = true;
                    }
                    if t.swiped {
                        t.last = p;
                        if unpress {
                            self.pressed = None;
                            self.dirty = true;
                        }
                        return;
                    }
                    if let Some(ri) = t.region {
                        let hz = self.regions[ri].horizontal;
                        let (cur, last, start) = if hz { (p.x, t.last.x, t.start.x) } else { (p.y, t.last.y, t.start.y) };
                        let dy = cur - last;
                        if !t.dragging && (cur - start).abs() > slop {
                            t.dragging = true;
                            unpress = true;
                        }
                        if t.dragging {
                            let st = &self.regions[ri].state;
                            st.raw_set(st.offset() - dy);
                            let tm = if e.time_ms > 0 { e.time_ms } else { self.now };
                            t.samples.push((tm, cur));
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
                if let Some(h) = t.hold.as_ref().filter(|h| h.fired) {
                    let cb = h.cb.clone();
                    cb(Hold::End((p.y - t.start.y) / self.scale.max(0.01)));
                    return;
                }
                if let Some(di) = t.drag {
                    if let Some((r, cb)) = self.drags.get(di) {
                        let fx = ((p.x - r.x) / r.w.max(1.0)).clamp(0.0, 1.0);
                        let cb = cb.clone();
                        cb(fx, true);
                    }
                    return;
                }
                if t.swiped {
                    let dx = p.x - t.start.x;
                    if dx.abs() > 40.0 * self.scale {
                        if let Some((_, cb)) = self.swipes.iter().rev().find(|(r, _)| r.contains(t.start)) {
                            let cb = cb.clone();
                            cb(if dx < 0.0 { 1 } else { -1 });
                        }
                    }
                    return;
                }
                if t.dragging {
                    if let Some(ri) = t.region {
                        // UZ: Tezlik: oxirgi 100 ms dagi namunalar bo'yicha hisoblanadi (VelocityTracker kabi).
                        //     Barmoq to'xtab turib qo'yib yuborilsa — inersiya bo'lmaydi.
                        // EN: Velocity: computed from samples of the last 100 ms (like VelocityTracker).
                        //     If the finger stops and is then lifted, there is no fling.
                        // RU: Скорость: считается по отсчётам за последние 100 мс (как VelocityTracker).
                        //     Если палец остановился и затем был отпущен — инерции нет.
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
                if let Some(h) = self.touch.as_ref().and_then(|t| t.hold.as_ref()).filter(|h| h.fired) {
                    let cb = h.cb.clone();
                    cb(Hold::End(0.0));
                }
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

    /// UZ: Fokusdagi maydon matnini tahrirlaydi; `f` belgilar va kursor indeksini o'zgartiradi.
    /// EN: Edits the text of the focused field; `f` modifies the characters and the caret index.
    /// RU: Редактирует текст поля в фокусе; `f` изменяет символы и индекс курсора.
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

    /// UZ: IME: tasdiqlangan matn (`commitText`).
    /// EN: IME: committed text (`commitText`).
    /// RU: IME: подтверждённый текст (`commitText`).
    pub fn ime_commit(&mut self, s: &str) -> bool {
        self.ime_text(s, false)
    }

    /// UZ: IME: tugallanmagan matn (`setComposingText`); keyingi chaqiruv uni almashtiradi.
    /// EN: IME: composing text (`setComposingText`); the next call replaces it.
    /// RU: IME: незавершённый текст (`setComposingText`); следующий вызов заменяет его.
    pub fn ime_composing(&mut self, s: &str) -> bool {
        self.ime_text(s, true)
    }

    /// UZ: IME: kursor atrofidagi belgilarni o'chirish.
    /// EN: IME: delete characters around the cursor.
    /// RU: IME: удаление символов вокруг курсора.
    pub fn ime_delete(&mut self, before: usize, after: usize) -> bool {
        self.composing = None;
        self.edit(|chars, caret, _| {
            let b = before.min(*caret);
            chars.drain(*caret - b..*caret);
            *caret -= b;
            let a = after.min(chars.len() - *caret);
            chars.drain(*caret..*caret + a);
            true
        })
    }

    /// UZ: IME: tugallanmagan matn tasdiqlandi.
    /// EN: IME: the composing text was finalized.
    /// RU: IME: незавершённый текст подтверждён.
    pub fn ime_finish(&mut self) {
        self.composing = None;
    }

    fn ime_text(&mut self, s: &str, composing: bool) -> bool {
        let comp = self.composing.take();
        let mut new_comp = None;
        let ok = self.edit(|chars, caret, cfg| {
            let (start, len) = comp.unwrap_or((*caret, 0));
            let start = start.min(chars.len());
            let end = (start + len).min(chars.len());
            chars.drain(start..end);
            *caret = start;
            let n = insert_str(chars, caret, cfg, s);
            if composing && cfg.format.is_none() && n > 0 {
                new_comp = Some((start, n));
            }
            true
        });
        self.composing = new_comp;
        ok
    }

    /// UZ: Fokusdagi maydonga belgi kiritadi. Maydon bo'lmasa `false` qaytadi.
    /// EN: Types a character into the focused field. Returns `false` if there is no field.
    /// RU: Вводит символ в поле в фокусе. Если поля нет — возвращает `false`.
    pub fn key_char(&mut self, c: char) -> bool {
        if c.is_control() {
            return false;
        }
        self.edit(|chars, caret, cfg| {
            insert_str(chars, caret, cfg, c.encode_utf8(&mut [0; 4]));
            true
        })
    }

    /// UZ: Kursor oldidagi belgini o'chiradi (Backspace).
    /// EN: Deletes the character before the cursor (Backspace).
    /// RU: Удаляет символ перед курсором (Backspace).
    pub fn key_backspace(&mut self) -> bool {
        self.edit(|chars, caret, _| {
            // UZ: Formatlovchi probelni emas, undan oldingi belgini o'chiramiz.
            // EN: Delete the character before the formatter's space, not the space itself.
            // RU: Удаляем символ перед пробелом форматирования, а не сам пробел.
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

    /// UZ: "Enter": `on_submit` chaqiriladi.
    /// EN: "Enter": `on_submit` is called.
    /// RU: «Enter»: вызывается `on_submit`.
    pub fn key_enter(&mut self) -> bool {
        let Some(fi) = self.focused_index() else {
            return false;
        };
        if let Some(cb) = self.fields[fi].cfg.on_submit.clone() {
            cb();
        }
        true
    }

    /// UZ: "Orqaga": avval fokus olinadi, keyin ilovaning `set_back_handler` funksiyasi chaqiriladi.
    /// EN: "Back": focus is cleared first, then the app's `set_back_handler` is called.
    /// RU: «Назад»: сначала снимается фокус, затем вызывается обработчик приложения `set_back_handler`.
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

/// UZ: Qaytadi: (qator balandligi, qator qutisi tepasidan asos chizig'igacha masofa). `lh` berilsa Compose kabi proporsional bo'linadi.
/// EN: Returns: (line height, distance from the top of the line box to the baseline). If `lh` is given, it is split proportionally like Compose.
/// RU: Возвращает: (высота строки, расстояние от верха строки до базовой линии). Если задан `lh`, делится пропорционально, как в Compose.
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

/// UZ: `s` ni kursor oldiga kiritadi (filtr va uzunlik chegarasi bilan); kiritilgan belgilar soni qaytadi.
/// EN: Inserts `s` at the cursor (applying the filter and the length limit); returns the number of characters inserted.
/// RU: Вставляет `s` перед курсором (с учётом фильтра и ограничения длины); возвращает число вставленных символов.
fn insert_str(chars: &mut Vec<char>, caret: &mut usize, cfg: &FieldCfg, s: &str) -> usize {
    let mut n = 0;
    for c in s.chars() {
        if c.is_control() {
            continue;
        }
        if cfg.filter.is_some_and(|p| !p(c)) {
            continue;
        }
        if cfg.max_len > 0 {
            let used = if cfg.format.is_some() {
                chars.iter().filter(|x| **x != ' ').count()
            } else {
                chars.len()
            };
            if used >= cfg.max_len {
                break;
            }
        }
        chars.insert(*caret, c);
        *caret += 1;
        n += 1;
    }
    n
}

fn it_padding(tree: &Tree, node: usize) -> EdgeInsets {
    tree.padding(node)
}

#[cfg(test)]
mod tests;


thread_local! {
    static FIT_SEEN: std::cell::RefCell<std::collections::HashSet<String>> = std::cell::RefCell::new(std::collections::HashSet::new());
}

/// UZ: Joylashuv tekshiruvi: aylantirilmaydigan konteynerning bolasi ota qutisidan chiqib ketsa (masalan, matn/tugma sig'masa) xatolar jurnaliga yoziladi (har holat bir marta).
/// EN: Layout check: when a child of a non-scrolling container sticks out of its parent box (for example text/button does not fit) it is written to the error log (once per case).
/// RU: Проверка раскладки: если ребёнок неприкручиваемого контейнера выходит за рамку родителя (например, не помещается текст/кнопка), это записывается в журнал ошибок (по разу на случай).
fn check_overflow(tree: &Tree, items: &[Item], scale: f32) {
    let k = scale.max(0.1);
    let mut scroll = std::collections::HashSet::new();
    let mut label: std::collections::HashMap<usize, String> = std::collections::HashMap::new();
    for it in items {
        if matches!(it.kind, ItemKind::ScrollEnd) {
            continue;
        }
        if it.scroll.is_some() {
            scroll.insert(it.node);
        }
        if let ItemKind::Text(t, ..) = &it.kind {
            label.insert(it.node, format!("matn \"{}\"", t.chars().take(30).collect::<String>()));
        }
    }
    let mut stack = vec![0usize];
    // UZ: Ildiz tugun `add` ning birinchi chaqirig'idan keladi; indekslarni bolalar orqali yuramiz.
    // EN: The root node comes from the first `add` call; we walk indexes through the children.
    // RU: Корневой узел приходит из первого вызова `add`; обходим индексы через детей.
    if let Some(first) = items.iter().find(|i| !matches!(i.kind, ItemKind::ScrollEnd)) {
        stack = vec![first.node];
    }
    let mut found = 0;
    while let Some(n) = stack.pop() {
        let r = tree.rect(n);
        let skip = scroll.contains(&n);
        for &c in tree.children(n) {
            stack.push(c);
            if skip {
                continue;
            }
            let cr = tree.rect(c);
            let (ox, oy) = ((cr.x + cr.w) - (r.x + r.w), (cr.y + cr.h) - (r.y + r.h));
            let (nx, ny) = (r.x - cr.x, r.y - cr.y);
            let worst = ox.max(oy).max(nx).max(ny);
            if worst > 3.0 && cr.w > 0.0 && cr.h > 0.0 {
                let name = label.get(&c).cloned().unwrap_or_else(|| format!("element {:.0}x{:.0}", cr.w / k, cr.h / k));
                let key = format!("{name}|{:.0}|{:.0}|{:.0}", r.w / k, r.h / k, worst / k);
                let fresh = FIT_SEEN.with(|s| {
                    let mut s = s.borrow_mut();
                    s.len() < 500 && s.insert(key)
                });
                if fresh && found < 5 {
                    found += 1;
                    uzcoders_core::report_error(
                        "joylashuv",
                        &format!("{name} ota qutisidan {:.0} dp chiqib ketdi (ota {:.0}x{:.0} dp, bola {:.0}x{:.0} dp)", worst / k, r.w / k, r.h / k, cr.w / k, cr.h / k),
                    );
                }
            }
        }
    }
}
