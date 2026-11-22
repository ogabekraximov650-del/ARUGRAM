//! UZ: Scroll holati: ofset, inersiya tezligi va oxirgi hisoblangan chegara.
//! EN: Scroll state: offset, fling (inertia) velocity and the last computed bounds.
//! RU: Состояние прокрутки: смещение, скорость инерции и последние вычисленные границы.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use uzcoders_core::invalidate;

struct Inner {
    offset: Cell<f32>,
    velocity: Cell<f32>,
    max: Cell<f32>,
    viewport: Cell<f32>,
    /// UZ: Tepada pastga tortish (piksel, >= 0) — "tortib yangilash" uchun. EN: pull distance past the top (pixels) for pull-to-refresh.
    pull: Cell<f32>,
    on_refresh: RefCell<Option<Rc<dyn Fn()>>>,
}

/// UZ: `scroll_column` / `.scroll(&state)` bilan ishlatiladigan scroll holati. Arzon nusxalanadi (Rc).
/// EN: Scroll state used with `scroll_column` / `.scroll(&state)`. Cheap to clone (Rc).
/// RU: Состояние прокрутки для `scroll_column` / `.scroll(&state)`. Дёшево клонируется (Rc).
#[derive(Clone)]
pub struct ScrollState {
    inner: Rc<Inner>,
}

impl Default for ScrollState {
    fn default() -> Self {
        Self::new()
    }
}

impl ScrollState {
    pub fn new() -> Self {
        ScrollState {
            inner: Rc::new(Inner {
                offset: Cell::new(0.0),
                velocity: Cell::new(0.0),
                max: Cell::new(0.0),
                viewport: Cell::new(0.0),
                pull: Cell::new(0.0),
                on_refresh: RefCell::new(None),
            }),
        }
    }

    /// UZ: Hozirgi ofset (piksel, 0 = eng tepa).
    /// EN: The current offset (pixels, 0 = the very top).
    /// RU: Текущее смещение (пиксели, 0 = самый верх).
    pub fn offset(&self) -> f32 {
        self.inner.offset.get()
    }

    pub fn set_offset(&self, v: f32) {
        self.inner
            .offset
            .set(v.clamp(0.0, self.inner.max.get().max(0.0)));
        invalidate();
    }

    /// UZ: "Tortib yangilash": ro'yxat eng tepada turganda pastga tortilib qo'yib yuborilsa `f` chaqiriladi. Tortilgan masofa `pull()` da (piksel).
    /// EN: Pull-to-refresh: when the list is at the top and pulled down far enough then released, `f` runs. The pull distance is in `pull()` (pixels).
    /// RU: «Потяни, чтобы обновить»: если список вверху и его потянули вниз достаточно далеко, при отпускании вызывается `f`; расстояние — в `pull()`.
    pub fn on_refresh(&self, f: impl Fn() + 'static) {
        *self.inner.on_refresh.borrow_mut() = Some(Rc::new(f));
    }

    /// UZ: Hozirgi tortish masofasi (piksel).
    pub fn pull(&self) -> f32 {
        self.inner.pull.get()
    }

    pub(crate) fn has_refresh(&self) -> bool {
        self.inner.on_refresh.borrow().is_some()
    }

    pub(crate) fn set_pull(&self, v: f32) {
        self.inner.pull.set(v.max(0.0));
        invalidate();
    }

    pub(crate) fn fire_refresh(&self) {
        let cb = self.inner.on_refresh.borrow().clone();
        if let Some(cb) = cb {
            cb();
        }
    }

    pub fn scroll_to_top(&self) {
        self.inner.velocity.set(0.0);
        self.set_offset(0.0);
    }

    /// UZ: Oxirgi kadrda hisoblangan eng katta ofset (kontent balandligi - ko'rinish balandligi).
    /// EN: The largest offset computed on the last frame (content height - viewport height).
    /// RU: Наибольшее смещение, вычисленное на последнем кадре (высота содержимого - высота видимой области).
    pub fn max_offset(&self) -> f32 {
        self.inner.max.get()
    }

    /// UZ: Oxirgi kadrdagi ko'rinish balandligi (piksel; hali chizilmagan bo'lsa 0).
    /// EN: Viewport height on the last frame (pixels; 0 if not drawn yet).
    /// RU: Высота видимой области на последнем кадре (пиксели; 0, если ещё не рисовалось).
    pub fn viewport(&self) -> f32 {
        self.inner.viewport.get()
    }

    pub(crate) fn set_viewport(&self, v: f32) -> bool {
        let changed = (self.inner.viewport.get() - v).abs() > 1.0;
        self.inner.viewport.set(v);
        changed
    }

    pub(crate) fn velocity(&self) -> f32 {
        self.inner.velocity.get()
    }
    pub(crate) fn set_velocity(&self, v: f32) {
        self.inner.velocity.set(v);
    }
    pub(crate) fn set_max(&self, m: f32) {
        self.inner.max.set(m.max(0.0));
        let o = self.inner.offset.get();
        if o > m.max(0.0) {
            self.inner.offset.set(m.max(0.0));
        }
    }
    pub(crate) fn raw_set(&self, v: f32) {
        self.inner
            .offset
            .set(v.clamp(0.0, self.inner.max.get().max(0.0)));
    }
}
