//! UZ: Reaktiv holat. `State<T>` o'zgarganda global "versiya" oshadi; UI shu orqali qayta qurilishi kerakligini biladi.
//! Bitta oqimli (thread-local): mobil UI oqimi uchun yetarli va juda tez.
//! EN: Reactive state. When a `State<T>` changes, a global "revision" counter grows; the UI uses it to know it must rebuild.
//! Single-threaded (thread-local): enough for a mobile UI thread and very fast.
//! RU: Реактивное состояние. При изменении `State<T>` растёт глобальный счётчик «ревизии»; по нему UI понимает, что нужно перестроиться.
//! Однопоточное (thread-local): достаточно для UI-потока мобильного приложения и очень быстро.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

thread_local! {
    static REVISION: Cell<u64> = const { Cell::new(0) };
}

/// UZ: Har qanday `State` o'zgarganda oshadigan global hisoblagich (faqat UI oqimi uchun).
/// EN: Global counter that grows whenever any `State` changes (UI thread only).
/// RU: Глобальный счётчик, растущий при изменении любого `State` (только для UI-потока).
pub fn global_revision() -> u64 {
    REVISION.with(|r| r.get())
}

/// UZ: UI'ni qo'lda qayta qurishga majbur qiladi (masalan, tashqi ma'lumot kelganda). Faqat UI oqimidan chaqiriladi.
/// EN: Forces the UI to rebuild manually (for example, when external data arrives). Call it from the UI thread only.
/// RU: Принудительно перестраивает UI вручную (например, когда пришли внешние данные). Вызывать только из UI-потока.
pub fn invalidate() {
    REVISION.with(|r| r.set(r.get() + 1));
}

struct Inner<T> {
    value: RefCell<T>,
    version: Cell<u64>,
}

/// UZ: Arzon nusxalanadigan (Rc) o'zgaruvchan qiymat: nusxalar bir xil ma'lumotni ko'rsatadi.
/// EN: A cheaply cloneable (Rc) mutable value: clones share the same data.
/// RU: Дёшево клонируемое (Rc) изменяемое значение: клоны указывают на одни и те же данные.
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
    /// UZ: Shu holatni boshqalaridan ajratuvchi barqaror identifikator (nusxalar bir xil id beradi).
    /// EN: A stable identifier that tells this state apart from others (clones return the same id).
    /// RU: Стабильный идентификатор, отличающий это состояние от других (клоны возвращают тот же id).
    pub fn id(&self) -> usize {
        Rc::as_ptr(&self.inner) as *const () as usize
    }
    /// UZ: Shu holatning o'z versiyasi (har `set` / `update` da oshadi).
    /// EN: This state's own version (increases on every `set` / `update`).
    /// RU: Собственная версия этого состояния (растёт при каждом `set` / `update`).
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
