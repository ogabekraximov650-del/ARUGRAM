//! Scroll holati: ofset, inersiya tezligi va oxirgi hisoblangan chegara.

use std::cell::Cell;
use std::rc::Rc;
use uzcoders_core::invalidate;

struct Inner {
    offset: Cell<f32>,
    velocity: Cell<f32>,
    max: Cell<f32>,
}

/// `scroll_column`/`.scroll(&state)` bilan ishlatiladigan scroll holati. Arzon nusxalanadi (Rc).
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
            }),
        }
    }

    /// Hozirgi ofset (piksel, 0 = eng tepa).
    pub fn offset(&self) -> f32 {
        self.inner.offset.get()
    }

    pub fn set_offset(&self, v: f32) {
        self.inner
            .offset
            .set(v.clamp(0.0, self.inner.max.get().max(0.0)));
        invalidate();
    }

    pub fn scroll_to_top(&self) {
        self.inner.velocity.set(0.0);
        self.set_offset(0.0);
    }

    /// Oxirgi kadrda hisoblangan eng katta ofset (kontent - ko'rinish).
    pub fn max_offset(&self) -> f32 {
        self.inner.max.get()
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
