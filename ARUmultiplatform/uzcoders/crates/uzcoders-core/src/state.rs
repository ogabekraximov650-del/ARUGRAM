//! Reaktiv holat. `State<T>` o'zgarsa, global "versiya" oshadi; UI shu orqali qayta qurilishi kerakligini biladi.
//! Bitta oqimli (thread-local) — mobil UI oqimi uchun yetarli va juda tez.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

thread_local! {
    static REVISION: Cell<u64> = const { Cell::new(0) };
}

/// Har qanday `State` o'zgarganda oshadigan global hisoblagich.
pub fn global_revision() -> u64 {
    REVISION.with(|r| r.get())
}

/// UI'ni qo'lda qayta qurishga majbur qiladi (masalan, tashqi ma'lumot kelganda).
pub fn invalidate() {
    REVISION.with(|r| r.set(r.get() + 1));
}

struct Inner<T> {
    value: RefCell<T>,
    version: Cell<u64>,
}

/// Arzon nusxalanadigan (Rc) o'zgaruvchan qiymat.
pub struct State<T> {
    inner: Rc<Inner<T>>,
}

impl<T> Clone for State<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
        }
    }
}

impl<T> State<T> {
    pub fn new(value: T) -> Self {
        Self {
            inner: Rc::new(Inner {
                value: RefCell::new(value),
                version: Cell::new(0),
            }),
        }
    }
    /// Shu holatning o'z versiyasi (har `set` / `update` da oshadi).
    pub fn version(&self) -> u64 {
        self.inner.version.get()
    }
    pub fn set(&self, v: T) {
        *self.inner.value.borrow_mut() = v;
        self.touch();
    }
    pub fn update(&self, f: impl FnOnce(&mut T)) {
        f(&mut self.inner.value.borrow_mut());
        self.touch();
    }
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        f(&self.inner.value.borrow())
    }
    fn touch(&self) {
        self.inner.version.set(self.inner.version.get() + 1);
        invalidate();
    }
}

impl<T: Clone> State<T> {
    pub fn get(&self) -> T {
        self.inner.value.borrow().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_bumps_versions() {
        let s = State::new(1);
        let r0 = global_revision();
        s.set(2);
        assert_eq!(s.get(), 2);
        assert_eq!(s.version(), 1);
        assert!(global_revision() > r0);
        let c = s.clone();
        c.update(|v| *v += 5);
        assert_eq!(s.get(), 7);
    }
}
