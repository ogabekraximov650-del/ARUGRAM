//! Fon vazifalari: og'ir ishni (tarmoq, fayl) alohida oqimda bajarib, natijani UI oqimida qabul qilish.
//! `State` bir oqimli bo'lgani uchun `done` har doim UI oqimida chaqiriladi.

use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;

static QUEUE: Mutex<Vec<(u64, Box<dyn Any + Send>)>> = Mutex::new(Vec::new());
static NEXT: AtomicU64 = AtomicU64::new(1);
static PENDING: AtomicUsize = AtomicUsize::new(0);
static WAKER: Mutex<Option<fn()>> = Mutex::new(None);

static FRAME_REQ: AtomicBool = AtomicBool::new(false);

/// Istalgan oqimdan: UI oqimini uyg'otib, bitta kadrni qayta chizishni so'raydi (video kadri tayyor bo'lganda).
pub fn request_frame() {
    FRAME_REQ.store(true, Ordering::SeqCst);
    if let Some(w) = *WAKER.lock().unwrap() {
        w();
    }
}

/// Kadr so'rovi bormi (o'chirmaydi).
pub fn frame_requested() -> bool {
    FRAME_REQ.load(Ordering::SeqCst)
}

/// Kadr so'rovini tozalaydi (kadr chizilganda).
pub fn clear_frame_request() {
    FRAME_REQ.store(false, Ordering::SeqCst);
}

type Done = Box<dyn FnOnce(Box<dyn Any + Send>)>;

thread_local! {
    static DONE: RefCell<HashMap<u64, Done>> = RefCell::new(HashMap::new());
}

/// Qobiq o'rnatadi: fon vazifasi tugaganda UI oqimini uyg'otadi.
pub fn set_waker(f: fn()) {
    *WAKER.lock().unwrap() = Some(f);
}

/// UI oqimidan chaqiriladi: `work` yangi oqimda bajariladi, natijasi `done` ga UI oqimida beriladi.
pub fn spawn_bg<R: Send + 'static>(
    work: impl FnOnce() -> R + Send + 'static,
    done: impl FnOnce(R) + 'static,
) {
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    DONE.with(|d| {
        d.borrow_mut().insert(
            id,
            Box::new(move |b: Box<dyn Any + Send>| {
                if let Ok(r) = b.downcast::<R>() {
                    done(*r);
                }
            }),
        )
    });
    PENDING.fetch_add(1, Ordering::SeqCst);
    std::thread::spawn(move || {
        let r = work();
        QUEUE.lock().unwrap().push((id, Box::new(r)));
        if let Some(w) = *WAKER.lock().unwrap() {
            w();
        }
    });
}

/// Tugagan vazifalarning `done` larini chaqiradi (UI oqimida, har tsiklda). Biror ish bajarilsa `true`.
pub fn drain_tasks() -> bool {
    let items: Vec<_> = std::mem::take(&mut *QUEUE.lock().unwrap());
    let any = !items.is_empty();
    for (id, res) in items {
        let cb = DONE.with(|d| d.borrow_mut().remove(&id));
        PENDING.fetch_sub(1, Ordering::SeqCst);
        if let Some(cb) = cb {
            cb(res);
        }
    }
    any
}

/// Hali tugamagan vazifalar soni (testlar uchun).
pub fn pending_tasks() -> usize {
    PENDING.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::State;

    #[test]
    fn natija_ui_oqimiga_qaytadi() {
        let s = State::new(0);
        let s2 = s.clone();
        let tid = std::thread::current().id();
        spawn_bg(
            || {
                std::thread::sleep(std::time::Duration::from_millis(20));
                (std::thread::current().id(), 7)
            },
            move |(wid, v)| {
                assert_ne!(wid, tid); // ish boshqa oqimda
                assert_eq!(std::thread::current().id(), tid); // natija shu oqimda
                s2.set(v);
            },
        );
        assert_eq!(pending_tasks(), 1);
        for _ in 0..200 {
            drain_tasks();
            if pending_tasks() == 0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(s.get(), 7);
    }
}
