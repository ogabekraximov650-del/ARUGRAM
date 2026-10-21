//! UZ: Chizish buyruqlarini GPU uchun paketlaydi: bitta uchlar oqimi (vertex stream) + operatsiyalar ro'yxati
//! (atlasga yuklash, qirqish bilan chizish). Platformadan mustaqil, shuning uchun kompyuterda sinaladi.
//! EN: Packs draw commands for the GPU: a single vertex stream plus a list of operations
//! (atlas uploads, scissored draws). Platform-independent, so it is tested on a desktop computer.
//! RU: Упаковывает команды рисования для GPU: единый поток вершин и список операций
//! (загрузка в атлас, рисование с отсечением). Не зависит от платформы, поэтому тестируется на компьютере.

use crate::atlas::Atlas;
use std::collections::HashMap;
use std::rc::Rc;
use uzcoders_core::{Color, Rect};
use uzcoders_render::{DrawCmd, Image, Path};
use uzcoders_text::{Font, GlyphCache};

pub const ATLAS_SIZE: u32 = 1024;

/// UZ: Bitta uch (vertex): joy (piksel), uv / lokal koordinata, rang va parametrlar.
/// `p.x < 0` — atlasdan alfa olinadi; aks holda yumaloq to'rtburchak SDF: `p = (yarim_kenglik, yarim_balandlik, radius, kontur_qalinligi)`.
/// EN: One vertex: position (pixels), uv / local coordinate, color and parameters.
/// `p.x < 0` means alpha is sampled from the atlas; otherwise a rounded-rectangle SDF is used: `p = (half_width, half_height, radius, stroke_width)`.
/// RU: Одна вершина: положение (в пикселях), uv / локальная координата, цвет и параметры.
/// `p.x < 0` — альфа берётся из атласа; иначе используется SDF скруглённого прямоугольника: `p = (полуширина, полувысота, радиус, толщина_контура)`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vertex {
    pub x: f32,
    pub y: f32,
    pub u: f32,
    pub v: f32,
    pub color: [u8; 4],
    pub p: [f32; 4],
    /// UZ: Qo'shimcha parametr (backdrop uchun linza kattalashtirishi; boshqa elementlarda 0).
    /// EN: Extra parameter (lens magnification for backdrop; 0 for other elements).
    /// RU: Дополнительный параметр (увеличение линзы для backdrop; для остальных элементов 0).
    pub q: f32,
}

#[derive(Debug)]
pub enum Op {
    /// UZ: Atlasning bir qismini yangilash (A8 piksellar, ya'ni bir kanalli alfa).
    /// EN: Update part of the atlas (A8 pixels, i.e. single-channel alpha).
    /// RU: Обновить часть атласа (пиксели A8, то есть одноканальная альфа).
    Upload {
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        data: Vec<u8>,
    },
    /// UZ: Atlas tozalandi: eski joylashuvlar endi yaroqsiz.
    /// EN: The atlas was cleared: old placements are no longer valid.
    /// RU: Атлас очищен: прежние положения больше недействительны.
    ClearAtlas,
    /// UZ: RGBA rasm atlasining bir qismini yangilash.
    /// EN: Update part of the RGBA image atlas.
    /// RU: Обновить часть атласа RGBA-изображений.
    UploadImage {
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        data: Vec<u8>,
    },
    /// UZ: Rasm atlasi tozalandi.
    /// EN: The image atlas was cleared.
    /// RU: Атлас изображений очищен.
    ClearImages,
    /// UZ: Katta rasm uchun alohida tekstura yaratish (bir marta yuklanadi).
    /// EN: Create a separate texture for a large image (uploaded once).
    /// RU: Создать отдельную текстуру для большого изображения (загружается один раз).
    NewImage {
        id: u32,
        w: u32,
        h: u32,
        data: Vec<u8>,
    },
    /// UZ: Alohida rasm teksturasini o'chirish (xotira chegarasi oshganda).
    /// EN: Delete a separate image texture (when the memory budget is exceeded).
    /// RU: Удалить отдельную текстуру изображения (при превышении лимита памяти).
    FreeImage(u32),
    /// UZ: Shu paytgacha chizilgan tasvirning `[x, y, w, h]` (boshlang'ich nuqta yuqori-chapda) hududini xiralashtirib, keyingi chizishlar uchun tayyorlaydi.
    /// EN: Blurs the `[x, y, w, h]` region (origin at top-left) of what has been drawn so far and prepares it for subsequent draws.
    /// RU: Размывает область `[x, y, w, h]` (начало в левом верхнем углу) уже нарисованного и готовит её для следующих отрисовок.
    Blur { region: [i32; 4], blur: f32 },
    /// UZ: Video kadrini (tashqi tekstura) `[x, y, w, h]` piksel to'rtburchagiga chizadi.
    /// EN: Draws the video frame (external texture) into the `[x, y, w, h]` pixel rectangle.
    /// RU: Рисует кадр видео (внешняя текстура) в прямоугольник `[x, y, w, h]` пикселей.
    Video { rect: [f32; 4] },
    /// UZ: `verts[first..first+count]` uchlarini (qirqish bilan) chizadi. `scissor` — `[x, y, w, h]`, boshlang'ich nuqta yuqori-chapda.
    /// EN: Draws vertices `verts[first..first+count]` (with scissoring). `scissor` is `[x, y, w, h]` with the origin at top-left.
    /// RU: Рисует вершины `verts[first..first+count]` (с отсечением). `scissor` — `[x, y, w, h]`, начало в левом верхнем углу.
    Draw {
        scissor: Option<[i32; 4]>,
        first: u32,
        count: u32,
        /// UZ: Berilgan bo'lsa — shu alohida rasm teksturasi bilan chiziladi (atlas o'rniga).
        /// EN: If set, drawing uses this separate image texture (instead of the atlas).
        /// RU: Если задано — рисование идёт с этой отдельной текстурой изображения (вместо атласа).
        image: Option<u32>,
    },
}

#[derive(Default)]
pub struct Frame {
    pub verts: Vec<Vertex>,
    pub ops: Vec<Op>,
}

#[derive(Clone, Copy)]
struct Entry {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

#[derive(Hash, PartialEq, Eq)]
enum Key {
    Glyph(usize, char, u32),
    Icon(usize, u32, u32),
}

pub struct Batcher {
    faces: Vec<(u16, GlyphCache)>,
    atlas: Atlas,
    entries: HashMap<Key, Option<Entry>>,
    img_atlas: Atlas,
    /// UZ: Atlasdagi rasmlar (`Rc` ushlab turiladi, shunda ko'rsatkich o'zgarmaydi).
    /// EN: Images in the atlas (an `Rc` is kept so the pointer stays stable).
    /// RU: Изображения в атласе (хранится `Rc`, чтобы указатель не менялся).
    images: HashMap<usize, (Rc<Image>, Option<Entry>)>,
    /// UZ: Katta rasmlar (posterlar): har biriga alohida tekstura beriladi, LRU (eng kam ishlatilgani chiqariladi) bilan boshqariladi.
    /// EN: Large images (posters): each gets its own texture, managed with an LRU (least recently used is evicted first).
    /// RU: Большие изображения (постеры): у каждого своя текстура, управление по LRU (первым вытесняется наименее используемое).
    big: HashMap<usize, BigImg>,
    next_id: u32,
    frame_no: u64,
    big_bytes: usize,
}

struct BigImg {
    id: u32,
    bytes: usize,
    last_used: u64,
    _keep: Rc<Image>,
}

/// UZ: Atlasga sig'maydigan (katta) rasmlar shu hajmdan (piksel) boshlanadi.
/// EN: Images from this size (in pixels) up are considered large and do not go into the atlas.
/// RU: Изображения от этого размера (в пикселях) считаются большими и в атлас не попадают.
const BIG_PIXELS: u32 = 128 * 128;
/// UZ: Katta rasm teksturalarining umumiy xotira chegarasi (bayt).
/// EN: Total memory budget for large-image textures (bytes).
/// RU: Общий лимит памяти для текстур больших изображений (в байтах).
const BIG_BUDGET: usize = 48 << 20;

fn rgba(c: Color) -> [u8; 4] {
    [c.r, c.g, c.b, c.a]
}

impl Batcher {
    /// UZ: `faces` — (qalinlik, shrift) juftliklari; birinchisi 400 (asosiy, oddiy) bo'lishi shart.
    /// EN: `faces` are (weight, font) pairs; the first must be 400 (the regular base face).
    /// RU: `faces` — пары (насыщенность, шрифт); первой должна быть 400 (основное обычное начертание).
    pub fn new(faces: Vec<(u16, Rc<Font>)>) -> Batcher {
        Batcher {
            faces: faces
                .into_iter()
                .map(|(w, f)| (w, GlyphCache::new(f)))
                .collect(),
            atlas: Atlas::new(ATLAS_SIZE),
            entries: HashMap::new(),
            img_atlas: Atlas::new(ATLAS_SIZE),
            images: HashMap::new(),
            big: HashMap::new(),
            next_id: 1,
            frame_no: 0,
            big_bytes: 0,
        }
    }

    fn face_index(&self, weight: u16) -> usize {
        let mut best = 0;
        let mut d = i32::MAX;
        for (i, (w, _)) in self.faces.iter().enumerate() {
            let dd = (*w as i32 - weight as i32).abs();
            if dd < d {
                d = dd;
                best = i;
            }
        }
        best
    }

    /// UZ: Atlas yo'qolgan (masalan, GL konteksti qayta yaratilgan): hammasi boshidan boshlanadi.
    /// EN: The atlas is lost (for example, the GL context was recreated): start everything over.
    /// RU: Атлас потерян (например, GL-контекст пересоздан): всё начинается заново.
    pub fn reset(&mut self) {
        self.atlas.reset();
        self.entries.clear();
        self.img_atlas.reset();
        self.images.clear();
        self.big.clear();
        self.big_bytes = 0;
    }

    pub fn build(&mut self, cmds: &[DrawCmd], width: u32, height: u32) -> Frame {
        self.frame_no += 1;
        let mut b = Build {
            frame: Frame::default(),
            batch_start: 0,
            clips: Vec::new(),
            scissor: None,
            empty: false,
            full: [0, 0, width as i32, height as i32],
        };
        for cmd in cmds {
            match cmd {
                DrawCmd::RoundRect {
                    rect,
                    radius,
                    color,
                } => {
                    b.rrect(*rect, *radius, 0.0, *color);
                }
                DrawCmd::RoundRectStroke {
                    rect,
                    radius,
                    width,
                    color,
                } => {
                    b.rrect(*rect, *radius, *width, *color);
                }
                DrawCmd::PushClip(r) => {
                    b.flush();
                    let cur = b.clips.last().copied().unwrap_or(b.full);
                    let x0 = (r.x.floor() as i32).max(cur[0]);
                    let y0 = (r.y.floor() as i32).max(cur[1]);
                    let x1 = (r.right().ceil() as i32).min(cur[0] + cur[2]);
                    let y1 = (r.bottom().ceil() as i32).min(cur[1] + cur[3]);
                    b.clips.push([x0, y0, (x1 - x0).max(0), (y1 - y0).max(0)]);
                    b.update_scissor();
                }
                DrawCmd::PopClip => {
                    b.flush();
                    b.clips.pop();
                    b.update_scissor();
                }
                DrawCmd::Text {
                    x,
                    baseline,
                    text,
                    size,
                    color,
                    weight,
                    spacing,
                } => {
                    let fi = self.face_index(*weight);
                    let mut pen = *x;
                    for ch in text.chars() {
                        let g = self.faces[fi].1.get(ch, *size);
                        if g.width > 0 && !b.empty {
                            if let Some(e) = self.glyph_entry(&mut b, fi, ch, *size, &g) {
                                let gx = pen.round() + g.left as f32;
                                let gy = baseline.round() - g.top as f32;
                                b.tex_quad(gx, gy, g.width as f32, g.height as f32, e, *color);
                            }
                        }
                        pen += g.advance + spacing;
                    }
                }
                DrawCmd::Gradient {
                    rect,
                    radius,
                    top,
                    bottom,
                } => {
                    b.gradient(*rect, *radius, *top, *bottom);
                }
                DrawCmd::Backdrop {
                    rect,
                    radius,
                    blur,
                    zoom,
                    tint,
                } => {
                    if !b.empty {
                        b.flush();
                        let m = (*blur * 2.0).ceil() as i32 + 2;
                        let cur = b.clips.last().copied().unwrap_or(b.full);
                        let _ = cur;
                        let region = [
                            (rect.x.floor() as i32 - m).max(0),
                            (rect.y.floor() as i32 - m).max(0),
                            (rect.w.ceil() as i32 + 2 * m).min(width as i32),
                            (rect.h.ceil() as i32 + 2 * m).min(height as i32),
                        ];
                        b.frame.ops.push(Op::Blur {
                            region,
                            blur: *blur,
                        });
                        b.rrect_q(*rect, *radius, -2.0, *tint, *zoom);
                    }
                }
                DrawCmd::Video { rect } => {
                    b.flush();
                    b.frame.ops.push(Op::Video {
                        rect: [rect.x, rect.y, rect.w, rect.h],
                    });
                }
                DrawCmd::Glass { rect, radius, tint } => {
                    b.rrect(*rect, *radius, -1.0, *tint);
                }
                DrawCmd::Arc {
                    cx,
                    cy,
                    radius,
                    width,
                    start,
                    sweep,
                    color,
                } => {
                    b.arc(*cx, *cy, *radius, *width, *start, *sweep, *color);
                }
                DrawCmd::Glow {
                    cx,
                    cy,
                    radius,
                    color,
                } => {
                    b.glow(*cx, *cy, *radius, *color);
                }
                DrawCmd::Image {
                    image,
                    rect,
                    opacity,
                } => {
                    if !b.empty {
                        self.draw_image(&mut b, image, *rect, *opacity);
                    }
                }
                DrawCmd::Path {
                    path,
                    view,
                    rect,
                    color,
                } => {
                    if !b.empty {
                        self.draw_path(&mut b, path, *view, *rect, *color);
                    }
                }
            }
        }
        b.flush();
        b.frame
    }

    fn alloc(&mut self, b: &mut Build, w: u32, h: u32, data: Vec<u8>) -> Option<Entry> {
        let pos = match self.atlas.alloc(w, h) {
            Some(p) => p,
            None => {
                // UZ: Atlas to'ldi: shu paytgacha yig'ilganini chizib, atlasni tozalab, qayta urinamiz.
                // EN: The atlas is full: flush what has been collected so far, clear the atlas and try again.
                // RU: Атлас заполнен: отрисовываем накопленное, очищаем атлас и пробуем снова.
                b.flush();
                b.frame.ops.push(Op::ClearAtlas);
                self.atlas.reset();
                self.entries.clear();
                self.atlas.alloc(w, h)?
            }
        };
        b.frame.ops.push(Op::Upload {
            x: pos.0,
            y: pos.1,
            w,
            h,
            data,
        });
        Some(Entry {
            x: pos.0,
            y: pos.1,
            w,
            h,
        })
    }

    fn glyph_entry(
        &mut self,
        b: &mut Build,
        fi: usize,
        ch: char,
        size: f32,
        g: &uzcoders_text::GlyphBitmap,
    ) -> Option<Entry> {
        let key = Key::Glyph(fi, ch, (size * 4.0).round() as u32);
        if let Some(e) = self.entries.get(&key) {
            return *e;
        }
        let e = self.alloc(b, g.width as u32, g.height as u32, g.data.clone());
        // UZ: `alloc` atlasni tozalagan bo'lishi mumkin, shuning uchun kalitni faqat shundan keyin yozamiz.
        // EN: `alloc` may have cleared the atlas, so the key is recorded only after it.
        // RU: `alloc` мог очистить атлас, поэтому ключ записываем только после этого.
        self.entries.insert(key, e);
        e
    }

    fn draw_image(&mut self, b: &mut Build, img: &Rc<Image>, rect: Rect, opacity: f32) {
        let key = Rc::as_ptr(img) as usize;
        if img.w * img.h > BIG_PIXELS {
            let id = match self.big.get_mut(&key) {
                Some(e) => {
                    e.last_used = self.frame_no;
                    e.id
                }
                None => {
                    let id = self.next_id;
                    self.next_id += 1;
                    let bytes = (img.w * img.h * 4) as usize;
                    b.flush();
                    b.frame.ops.push(Op::NewImage {
                        id,
                        w: img.w,
                        h: img.h,
                        data: img.rgba.clone(),
                    });
                    self.big.insert(
                        key,
                        BigImg {
                            id,
                            bytes,
                            last_used: self.frame_no,
                            _keep: img.clone(),
                        },
                    );
                    self.big_bytes += bytes;
                    // UZ: Chegara oshsa, shu kadrda ishlatilmagan eng eski rasmlar o'chiriladi.
                    // EN: When the budget is exceeded, the oldest images not used in this frame are deleted.
                    // RU: При превышении лимита удаляются самые старые изображения, не использованные в этом кадре.
                    while self.big_bytes > BIG_BUDGET {
                        let victim = self
                            .big
                            .iter()
                            .filter(|(_, e)| e.last_used < self.frame_no)
                            .min_by_key(|(_, e)| e.last_used)
                            .map(|(k, _)| *k);
                        let Some(k) = victim else { break };
                        if let Some(e) = self.big.remove(&k) {
                            self.big_bytes -= e.bytes;
                            b.frame.ops.push(Op::FreeImage(e.id));
                        }
                    }
                    id
                }
            };
            b.flush();
            b.image_quad_uv(rect, (0.0, 0.0, 1.0, 1.0), opacity);
            b.flush_with(Some(id));
            return;
        }
        let entry = match self.images.get(&key) {
            Some((_, e)) => *e,
            None => {
                let e = match self.img_atlas.alloc(img.w, img.h) {
                    Some(p) => Some(p),
                    None => {
                        b.flush();
                        b.frame.ops.push(Op::ClearImages);
                        self.img_atlas.reset();
                        self.images.clear();
                        self.img_atlas.alloc(img.w, img.h)
                    }
                }
                .map(|(x, y)| Entry {
                    x,
                    y,
                    w: img.w,
                    h: img.h,
                });
                if let Some(e) = e {
                    b.frame.ops.push(Op::UploadImage {
                        x: e.x,
                        y: e.y,
                        w: e.w,
                        h: e.h,
                        data: img.rgba.clone(),
                    });
                }
                self.images.insert(key, (img.clone(), e));
                e
            }
        };
        if let Some(e) = entry {
            b.image_quad(rect, e, opacity);
        }
    }

    fn draw_path(&mut self, b: &mut Build, path: &Rc<Path>, view: f32, rect: Rect, color: Color) {
        let (w, h) = (
            rect.w.round().max(1.0) as u32,
            rect.h.round().max(1.0) as u32,
        );
        if view <= 0.0 {
            return;
        }
        let key = Key::Icon(Rc::as_ptr(path) as usize, w, h);
        let entry = match self.entries.get(&key) {
            Some(e) => *e,
            None => {
                let (sx, sy) = (w as f32 / view, h as f32 / view);
                let segs: uzcoders_text::raster::Segs = path
                    .segs
                    .iter()
                    .map(|&((x0, y0), (x1, y1))| ((x0 * sx, y0 * sy), (x1 * sx, y1 * sy)))
                    .collect();
                let data = uzcoders_text::raster::fill(&segs, w as usize, h as usize);
                let e = self.alloc(b, w, h, data);
                self.entries.insert(key, e);
                e
            }
        };
        if let Some(e) = entry {
            b.tex_quad(rect.x.round(), rect.y.round(), w as f32, h as f32, e, color);
        }
    }
}

struct Build {
    frame: Frame,
    batch_start: usize,
    clips: Vec<[i32; 4]>,
    scissor: Option<[i32; 4]>,
    /// UZ: Joriy qirqish (scissor) maydoni bo'sh: hech narsa chizilmaydi.
    /// EN: The current scissor area is empty: nothing is drawn.
    /// RU: Текущая область отсечения (scissor) пуста: ничего не рисуется.
    empty: bool,
    full: [i32; 4],
}

impl Build {
    fn update_scissor(&mut self) {
        self.scissor = self.clips.last().copied();
        self.empty = matches!(self.scissor, Some(c) if c[2] <= 0 || c[3] <= 0);
    }

    fn flush(&mut self) {
        self.flush_with(None);
    }

    fn flush_with(&mut self, image: Option<u32>) {
        let n = self.frame.verts.len();
        if n > self.batch_start {
            self.frame.ops.push(Op::Draw {
                scissor: self.scissor,
                first: self.batch_start as u32,
                count: (n - self.batch_start) as u32,
                image,
            });
            self.batch_start = n;
        }
    }

    fn quad(&mut self, v: [Vertex; 4]) {
        // UZ: Ikki uchburchak: 0-1-2 va 2-1-3 (uchlar: chap-yuqori, o'ng-yuqori, chap-past, o'ng-past).
        // EN: Two triangles: 0-1-2 and 2-1-3 (vertices: top-left, top-right, bottom-left, bottom-right).
        // RU: Два треугольника: 0-1-2 и 2-1-3 (вершины: левая-верхняя, правая-верхняя, левая-нижняя, правая-нижняя).
        for i in [0, 1, 2, 2, 1, 3] {
            self.frame.verts.push(v[i]);
        }
    }

    fn rrect(&mut self, r: Rect, radius: f32, stroke: f32, c: Color) {
        self.rrect_q(r, radius, stroke, c, 0.0);
    }

    fn rrect_q(&mut self, r: Rect, radius: f32, stroke: f32, c: Color, q: f32) {
        if self.empty || c.a == 0 || r.w <= 0.0 || r.h <= 0.0 {
            return;
        }
        let (hw, hh) = (r.w * 0.5, r.h * 0.5);
        let rad = radius.min(hw).min(hh).max(0.0);
        // UZ: chekkani silliqlash uchun 1 piksel chegara
        // EN: a 1 pixel margin for anti-aliasing the edge.
        // RU: запас в 1 пиксель для сглаживания края.
        let m = 1.0;
        let (x0, y0, x1, y1) = (r.x - m, r.y - m, r.right() + m, r.bottom() + m);
        let (cx, cy) = (r.x + hw, r.y + hh);
        let p = [hw, hh, rad, stroke];
        let col = rgba(c);
        let mk = |x: f32, y: f32| Vertex {
            x,
            y,
            u: x - cx,
            v: y - cy,
            color: col,
            p,
            q,
        };
        self.quad([mk(x0, y0), mk(x1, y0), mk(x0, y1), mk(x1, y1)]);
    }

    fn gradient(&mut self, r: Rect, radius: f32, top: Color, bottom: Color) {
        if self.empty || (top.a == 0 && bottom.a == 0) || r.w <= 0.0 || r.h <= 0.0 {
            return;
        }
        let (hw, hh) = (r.w * 0.5, r.h * 0.5);
        let rad = radius.min(hw).min(hh).max(0.0);
        let m = 1.0;
        let (x0, y0, x1, y1) = (r.x - m, r.y - m, r.right() + m, r.bottom() + m);
        let (cx, cy) = (r.x + hw, r.y + hh);
        let p = [hw, hh, rad, 0.0];
        let (ct, cb) = (rgba(top), rgba(bottom));
        let mk = |x: f32, y: f32, c: [u8; 4]| Vertex {
            x,
            y,
            u: x - cx,
            v: y - cy,
            color: c,
            p,
            q: 0.0,
        };
        self.quad([
            mk(x0, y0, ct),
            mk(x1, y0, ct),
            mk(x0, y1, cb),
            mk(x1, y1, cb),
        ]);
    }

    #[allow(clippy::too_many_arguments)]
    fn arc(&mut self, cx: f32, cy: f32, r: f32, w: f32, start: f32, sweep: f32, c: Color) {
        if self.empty || c.a == 0 || r <= 0.0 {
            return;
        }
        let m = r + 1.0;
        let col = rgba(c);
        // UZ: `p.w = -4 - sweep`: "yoy" rejimi va yoy uzunligi bitta qiymatda kodlanadi.
        // EN: `p.w = -4 - sweep`: the "arc" mode and the arc length are encoded in one value.
        // RU: `p.w = -4 - sweep`: режим «дуга» и длина дуги кодируются одним значением.
        let p = [r, w, start, -4.0 - sweep];
        let mk = |x: f32, y: f32| Vertex {
            x,
            y,
            u: x - cx,
            v: y - cy,
            color: col,
            p,
            q: 0.0,
        };
        self.quad([
            mk(cx - m, cy - m),
            mk(cx + m, cy - m),
            mk(cx - m, cy + m),
            mk(cx + m, cy + m),
        ]);
    }

    fn glow(&mut self, cx: f32, cy: f32, r: f32, c: Color) {
        if self.empty || c.a == 0 || r <= 0.0 {
            return;
        }
        let col = rgba(c);
        let p = [r, r, r, -3.0];
        let mk = |x: f32, y: f32| Vertex {
            x,
            y,
            u: x - cx,
            v: y - cy,
            color: col,
            p,
            q: 0.0,
        };
        self.quad([
            mk(cx - r, cy - r),
            mk(cx + r, cy - r),
            mk(cx - r, cy + r),
            mk(cx + r, cy + r),
        ]);
    }

    fn image_quad(&mut self, r: Rect, e: Entry, opacity: f32) {
        let s = ATLAS_SIZE as f32;
        let uv = (
            e.x as f32 / s,
            e.y as f32 / s,
            (e.x + e.w) as f32 / s,
            (e.y + e.h) as f32 / s,
        );
        self.image_quad_uv(r, uv, opacity);
    }

    fn image_quad_uv(&mut self, r: Rect, uv: (f32, f32, f32, f32), opacity: f32) {
        let (u0, v0, u1, v1) = uv;
        let col = rgba(Color::rgba(
            255,
            255,
            255,
            (opacity.clamp(0.0, 1.0) * 255.0) as u8,
        ));
        let p = [-2.0, 0.0, 0.0, 0.0];
        let mk = |x: f32, y: f32, u: f32, v: f32| Vertex {
            x,
            y,
            u,
            v,
            color: col,
            p,
            q: 0.0,
        };
        self.quad([
            mk(r.x, r.y, u0, v0),
            mk(r.right(), r.y, u1, v0),
            mk(r.x, r.bottom(), u0, v1),
            mk(r.right(), r.bottom(), u1, v1),
        ]);
    }

    fn tex_quad(&mut self, x: f32, y: f32, w: f32, h: f32, e: Entry, c: Color) {
        let s = ATLAS_SIZE as f32;
        let (u0, v0, u1, v1) = (
            e.x as f32 / s,
            e.y as f32 / s,
            (e.x + e.w) as f32 / s,
            (e.y + e.h) as f32 / s,
        );
        let col = rgba(c);
        let p = [-1.0, 0.0, 0.0, 0.0];
        let mk = |x: f32, y: f32, u: f32, v: f32| Vertex {
            x,
            y,
            u,
            v,
            color: col,
            p,
            q: 0.0,
        };
        self.quad([
            mk(x, y, u0, v0),
            mk(x + w, y, u1, v0),
            mk(x, y + h, u0, v1),
            mk(x + w, y + h, u1, v1),
        ]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn batcher() -> Batcher {
        let f = Font::parse(include_bytes!("../../../assets/fonts/DejaVuSans.ttf")).unwrap();
        Batcher::new(vec![(400, Rc::new(f))])
    }

    fn rr(x: f32, y: f32, w: f32, h: f32) -> DrawCmd {
        DrawCmd::RoundRect {
            rect: Rect::new(x, y, w, h),
            radius: 4.0,
            color: Color::WHITE,
        }
    }

    fn draws(f: &Frame) -> Vec<(Option<[i32; 4]>, u32, u32)> {
        f.ops
            .iter()
            .filter_map(|o| match o {
                Op::Draw {
                    scissor,
                    first,
                    count,
                    ..
                } => Some((*scissor, *first, *count)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn bitta_chaqiruv_va_olti_uch() {
        let mut b = batcher();
        let f = b.build(
            &[rr(0.0, 0.0, 10.0, 10.0), rr(20.0, 0.0, 10.0, 10.0)],
            100,
            100,
        );
        assert_eq!(f.verts.len(), 12);
        assert_eq!(draws(&f), vec![(None, 0, 12)]);
        // UZ: SDF parametrlari: yarim o'lchamlar va radius.
        // EN: SDF parameters: half sizes and radius.
        // RU: Параметры SDF: половинные размеры и радиус.
        assert_eq!(f.verts[0].p, [5.0, 5.0, 4.0, 0.0]);
        assert_eq!(f.verts[0].u, -(5.0 + 1.0));
    }

    #[test]
    fn klip_paketni_ajratadi() {
        let mut b = batcher();
        let f = b.build(
            &[
                rr(0.0, 0.0, 10.0, 10.0),
                DrawCmd::PushClip(Rect::new(5.0, 5.0, 20.0, 20.0)),
                rr(6.0, 6.0, 5.0, 5.0),
                DrawCmd::PopClip,
                rr(0.0, 20.0, 10.0, 10.0),
            ],
            100,
            100,
        );
        assert_eq!(
            draws(&f),
            vec![(None, 0, 6), (Some([5, 5, 20, 20]), 6, 6), (None, 12, 6)]
        );
    }

    #[test]
    fn ichma_ich_klip_kesishadi_va_bosh_klip_chizmaydi() {
        let mut b = batcher();
        let f = b.build(
            &[
                DrawCmd::PushClip(Rect::new(0.0, 0.0, 10.0, 10.0)),
                // UZ: kesishmaydi: qirqish maydoni bo'sh
                // EN: does not intersect: the clip area is empty.
                // RU: не пересекается: область отсечения пуста.
                DrawCmd::PushClip(Rect::new(20.0, 20.0, 5.0, 5.0)),
                rr(0.0, 0.0, 50.0, 50.0),
                DrawCmd::PopClip,
                rr(1.0, 1.0, 2.0, 2.0),
                DrawCmd::PopClip,
            ],
            100,
            100,
        );
        assert_eq!(draws(&f), vec![(Some([0, 0, 10, 10]), 0, 6)]);
    }

    #[test]
    fn matn_atlasga_bir_marta_yuklanadi() {
        let mut b = batcher();
        let t = DrawCmd::Text {
            x: 5.0,
            baseline: 20.0,
            text: "aaa".into(),
            size: 16.0,
            color: Color::BLACK,
            weight: 400,
            spacing: 0.0,
        };
        let f = b.build(&[t.clone()], 100, 100);
        let uploads = f
            .ops
            .iter()
            .filter(|o| matches!(o, Op::Upload { .. }))
            .count();
        assert_eq!(uploads, 1);
        assert_eq!(f.verts.len(), 18);
        // UZ: atlas rejimi (alfa atlasdan olinadi)
        // EN: atlas mode (alpha is taken from the atlas).
        // RU: режим атласа (альфа берётся из атласа).
        assert!(f.verts[0].p[0] < 0.0);
                                        // UZ: Keyingi kadrda yuklash (upload) yo'q.
                                        // EN: No upload on the next frame.
                                        // RU: В следующем кадре загрузки (upload) нет.
        let f2 = b.build(&[t], 100, 100);
        assert_eq!(
            f2.ops
                .iter()
                .filter(|o| matches!(o, Op::Upload { .. }))
                .count(),
            0
        );
        assert_eq!(f2.verts.len(), 18);
    }

    #[test]
    fn probel_chizilmaydi_lekin_joy_oladi() {
        let mut b = batcher();
        let t = |s: &str| DrawCmd::Text {
            x: 0.0,
            baseline: 20.0,
            text: s.into(),
            size: 16.0,
            color: Color::BLACK,
            weight: 400,
            spacing: 0.0,
        };
        let f = b.build(&[t("a b")], 100, 100);
        assert_eq!(f.verts.len(), 12);
        let a = f.verts[0].x;
        let bx = f.verts[6].x;
        assert!(bx - a > 8.0);
    }

    #[test]
    fn atlas_toldirilsa_tozalanadi() {
        let mut b = batcher();
        let mut cmds = Vec::new();
        // UZ: Juda ko'p turli o'lchamli harflar atlasni to'ldirib yuboradi.
        // EN: Very many glyphs of different sizes fill up the atlas.
        // RU: Очень много глифов разных размеров заполняют атлас.
        for i in 0..400 {
            cmds.push(DrawCmd::Text {
                x: 0.0,
                baseline: 50.0,
                text: "ABCDEFGHIJKLMNOPQRSTUVWXYZ".into(),
                size: 30.0 + (i as f32) * 0.25,
                color: Color::BLACK,
                weight: 400,
                spacing: 0.0,
            });
        }
        let f = b.build(&cmds, 400, 400);
        assert!(f.ops.iter().any(|o| matches!(o, Op::ClearAtlas)));
        // UZ: Tozalashdan oldin eski atlas bilan chizish chaqiruvi bo'lishi kerak.
        // EN: A draw call using the old atlas must come before the clear.
        // RU: Перед очисткой должен быть вызов рисования со старым атласом.
        let first_clear = f
            .ops
            .iter()
            .position(|o| matches!(o, Op::ClearAtlas))
            .unwrap();
        assert!(f.ops[..first_clear]
            .iter()
            .any(|o| matches!(o, Op::Draw { .. })));
    }

    #[test]
    fn ikonka_kesh() {
        let mut b = batcher();
        let p = Rc::new(Path::parse("M4 4H20V20H4z").unwrap());
        let cmd = DrawCmd::Path {
            path: p,
            view: 24.0,
            rect: Rect::new(10.0, 10.0, 24.0, 24.0),
            color: Color::WHITE,
        };
        let f = b.build(&[cmd.clone()], 100, 100);
        assert_eq!(
            f.ops
                .iter()
                .filter(|o| matches!(o, Op::Upload { .. }))
                .count(),
            1
        );
        let f2 = b.build(&[cmd], 100, 100);
        assert_eq!(
            f2.ops
                .iter()
                .filter(|o| matches!(o, Op::Upload { .. }))
                .count(),
            0
        );
        assert_eq!(f2.verts.len(), 6);
    }

    #[test]
    fn shisha_va_yorug_dog() {
        let mut b = batcher();
        let f = b.build(
            &[
                DrawCmd::Glow {
                    cx: 50.0,
                    cy: 50.0,
                    radius: 40.0,
                    color: Color::rgba(255, 100, 0, 90),
                },
                DrawCmd::Glass {
                    rect: Rect::new(10.0, 10.0, 60.0, 30.0),
                    radius: 8.0,
                    tint: Color::rgba(255, 255, 255, 20),
                },
            ],
            100,
            100,
        );
        assert_eq!(f.verts.len(), 12);
        assert_eq!(f.verts[0].p[3], -3.0);
        assert_eq!(f.verts[6].p[3], -1.0);
        // UZ: bitta chaqiruvda chiziladi
        // EN: drawn in a single call.
        // RU: рисуется одним вызовом.
        assert_eq!(draws(&f).len(), 1);
    }

    #[test]
    fn backdrop_blur_amali_va_rejim() {
        let mut b = batcher();
        let f = b.build(
            &[DrawCmd::Backdrop {
                rect: Rect::new(10.0, 10.0, 60.0, 30.0),
                radius: 8.0,
                blur: 12.0,
                zoom: 1.0,
                tint: Color::rgba(255, 255, 255, 20),
            }],
            100,
            100,
        );
        assert!(matches!(f.ops[0], Op::Blur { .. }));
        assert!(matches!(f.ops[1], Op::Draw { .. }));
        assert_eq!(f.verts[0].p[3], -2.0);
    }

    #[test]
    fn yoy_parametrlari() {
        let mut b = batcher();
        let f = b.build(
            &[DrawCmd::Arc {
                cx: 20.0,
                cy: 20.0,
                radius: 10.0,
                width: 2.0,
                start: 1.0,
                sweep: 2.0,
                color: Color::WHITE,
            }],
            100,
            100,
        );
        assert_eq!(f.verts.len(), 6);
        assert_eq!(f.verts[0].p, [10.0, 2.0, 1.0, -6.0]);
    }

    #[test]
    fn rasm_atlasga_bir_marta() {
        let mut b = batcher();
        let img = Rc::new(Image {
            w: 8,
            h: 6,
            rgba: vec![255; 8 * 6 * 4],
        });
        let cmd = DrawCmd::Image {
            image: img,
            rect: Rect::new(5.0, 5.0, 16.0, 12.0),
            opacity: 1.0,
        };
        let f = b.build(&[cmd.clone()], 100, 100);
        assert_eq!(
            f.ops
                .iter()
                .filter(|o| matches!(o, Op::UploadImage { .. }))
                .count(),
            1
        );
        assert_eq!(f.verts[0].p[0], -2.0);
        let f2 = b.build(&[cmd], 100, 100);
        assert_eq!(
            f2.ops
                .iter()
                .filter(|o| matches!(o, Op::UploadImage { .. }))
                .count(),
            0
        );
        assert_eq!(f2.verts.len(), 6);
    }

    #[test]
    fn katta_rasm_alohida_tekstura_va_lru() {
        let mut b = batcher();
        let big = |v: u8| {
            Rc::new(Image {
                w: 480,
                h: 720,
                rgba: vec![v; 480 * 720 * 4],
            })
        };
        let imgs: Vec<Rc<Image>> = (0..40).map(|i| big(i as u8)).collect();
        // UZ: Birinchi kadr: 3 ta poster — har biri alohida chaqiruv va alohida yuklash.
        // EN: First frame: 3 posters — each with its own draw call and its own upload.
        // RU: Первый кадр: 3 постера — у каждого свой вызов рисования и своя загрузка.
        let cmds: Vec<DrawCmd> = imgs[..3]
            .iter()
            .map(|i| DrawCmd::Image {
                image: i.clone(),
                rect: Rect::new(0.0, 0.0, 100.0, 150.0),
                opacity: 1.0,
            })
            .collect();
        let f = b.build(&cmds, 400, 400);
        assert_eq!(
            f.ops
                .iter()
                .filter(|o| matches!(o, Op::NewImage { .. }))
                .count(),
            3
        );
        assert_eq!(
            f.ops
                .iter()
                .filter(|o| matches!(o, Op::Draw { image: Some(_), .. }))
                .count(),
            3
        );
        // UZ: Ikkinchi kadr: qayta yuklash yo'q.
        // EN: Second frame: no re-upload.
        // RU: Второй кадр: повторной загрузки нет.
        let f2 = b.build(&cmds, 400, 400);
        assert_eq!(
            f2.ops
                .iter()
                .filter(|o| matches!(o, Op::NewImage { .. }))
                .count(),
            0
        );
        // UZ: Ko'p rasm: 48 MB dan oshgach eskilari o'chiriladi.
        // EN: Many images: once 48 MB is exceeded, the older ones are deleted.
        // RU: Много изображений: после превышения 48 МБ старые удаляются.
        let all: Vec<DrawCmd> = imgs
            .iter()
            .map(|i| DrawCmd::Image {
                image: i.clone(),
                rect: Rect::new(0.0, 0.0, 100.0, 150.0),
                opacity: 1.0,
            })
            .collect();
        let _ = b.build(&all, 400, 400);
        // UZ: Keyingi kadrda yangi rasm kelganda, shu kadrda ishlatilmagan eskilari o'chiriladi.
        // EN: When a new image arrives in the next frame, older ones not used in that frame are deleted.
        // RU: Когда в следующем кадре приходит новое изображение, старые, не использованные в этом кадре, удаляются.
        let fresh = DrawCmd::Image {
            image: big(99),
            rect: Rect::new(0.0, 0.0, 100.0, 150.0),
            opacity: 1.0,
        };
        let f4 = b.build(&[fresh], 400, 400);
        assert!(f4.ops.iter().any(|o| matches!(o, Op::FreeImage(_))));
    }

    #[test]
    fn kontur_parametri() {
        let mut b = batcher();
        let f = b.build(
            &[DrawCmd::RoundRectStroke {
                rect: Rect::new(0.0, 0.0, 20.0, 10.0),
                radius: 3.0,
                width: 2.0,
                color: Color::WHITE,
            }],
            100,
            100,
        );
        assert_eq!(f.verts[0].p[3], 2.0);
    }
}
