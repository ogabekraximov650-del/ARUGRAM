//! UZ: Fon vazifalari: og'ir ishni (tarmoq, fayl) alohida oqimda bajarib, natijani UI oqimida qabul qilish.
//! `State` bir oqimli bo'lgani uchun `done` har doim UI oqimida chaqiriladi.
//! EN: Background tasks: run heavy work (network, files) on a separate thread and receive the result on the UI thread.
//! Because `State` is single-threaded, `done` is always called on the UI thread.
//! RU: Фоновые задачи: тяжёлая работа (сеть, файлы) выполняется в отдельном потоке, результат принимается в UI-потоке.
//! Поскольку `State` однопоточный, `done` всегда вызывается в UI-потоке.

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

/// UZ: Istalgan oqimdan chaqiriladi: UI oqimini uyg'otib, bitta kadrni qayta chizishni so'raydi (masalan, video kadri tayyor bo'lganda).
/// EN: Callable from any thread: wakes the UI thread and asks for one frame to be redrawn (for example, when a video frame is ready).
/// RU: Можно вызывать из любого потока: будит UI-поток и просит перерисовать один кадр (например, когда готов кадр видео).
pub fn request_frame() {
    FRAME_REQ.store(true, Ordering::SeqCst);
    if let Some(w) = *WAKER.lock().unwrap() {
        w();
    }
}

/// UZ: Kadr so'rovi borligini tekshiradi (so'rovni o'chirmaydi).
/// EN: Checks whether a frame request is pending (does not clear it).
/// RU: Проверяет, есть ли запрос на кадр (запрос не сбрасывается).
pub fn frame_requested() -> bool {
    FRAME_REQ.load(Ordering::SeqCst)
}

/// UZ: Kadr so'rovini tozalaydi (kadr chizilganda chaqiriladi).
/// EN: Clears the frame request (called when a frame has been drawn).
/// RU: Сбрасывает запрос на кадр (вызывается после отрисовки кадра).
pub fn clear_frame_request() {
    FRAME_REQ.store(false, Ordering::SeqCst);
}

static VIDEO_REQ: AtomicBool = AtomicBool::new(false);

/// UZ: Istalgan oqimdan: faqat video kadrini qayta chizishni so'raydi. Interfeys qayta qurilmaydi — oxirgi chizish buyruqlari qayta ishlatiladi (arzon).
/// EN: Callable from any thread: asks to redraw only the video frame. The UI is not rebuilt — the last draw commands are reused (cheap).
/// RU: Можно вызывать из любого потока: просит перерисовать только кадр видео. UI не перестраивается — повторно используются последние команды рисования (дёшево).
pub fn request_video_frame() {
    VIDEO_REQ.store(true, Ordering::SeqCst);
    if let Some(w) = *WAKER.lock().unwrap() {
        w();
    }
}

/// UZ: Faqat video kadrini qayta chizish so'rovi bormi (o'chirmaydi).
/// EN: Whether a video-only redraw is requested (does not clear it).
/// RU: Есть ли запрос на перерисовку только кадра видео (не сбрасывает его).
pub fn video_frame_requested() -> bool {
    VIDEO_REQ.load(Ordering::SeqCst)
}

/// UZ: Video kadri so'rovini tozalaydi (kadr chizilganda chaqiriladi).
/// EN: Clears the video frame request (called when the frame has been drawn).
/// RU: Сбрасывает запрос на кадр видео (вызывается после отрисовки кадра).
pub fn clear_video_frame_request() {
    VIDEO_REQ.store(false, Ordering::SeqCst);
}

type Done = Box<dyn FnOnce(Box<dyn Any + Send>)>;

thread_local! {
    static DONE: RefCell<HashMap<u64, Done>> = RefCell::new(HashMap::new());
}

/// UZ: Qobiqni o'rnatadi: fon vazifasi tugaganda UI oqimini uyg'otadigan funksiya.
/// EN: Installs the shell hook: a function that wakes the UI thread when a background task finishes.
/// RU: Устанавливает хук оболочки: функцию, которая будит UI-поток по завершении фоновой задачи.
pub fn set_waker(f: fn()) {
    *WAKER.lock().unwrap() = Some(f);
}

/// UZ: UI oqimidan chaqiriladi: `work` yangi oqimda bajariladi, natijasi UI oqimida `done` ga beriladi.
/// EN: Call from the UI thread: `work` runs on a new thread and its result is passed to `done` on the UI thread.
/// RU: Вызывается из UI-потока: `work` выполняется в новом потоке, а результат передаётся в `done` в UI-потоке.
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

/// UZ: Tugagan vazifalarning `done` funksiyalarini chaqiradi (UI oqimida, har tsiklda). Biror ish bajarilsa `true` qaytadi.
/// EN: Calls the `done` callbacks of finished tasks (on the UI thread, every loop iteration). Returns `true` if any work was done.
/// RU: Вызывает `done` у завершённых задач (в UI-потоке, на каждой итерации цикла). Возвращает `true`, если что-то было выполнено.
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

/// UZ: Hali tugamagan vazifalar soni (asosan testlar uchun).
/// EN: Number of tasks that have not finished yet (mainly for tests).
/// RU: Количество ещё не завершённых задач (в основном для тестов).
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
                // UZ: ish boshqa oqimda bajariladi
                // EN: the work runs on another thread.
                // RU: работа выполняется в другом потоке.
                assert_ne!(wid, tid);
                // UZ: natija shu (UI) oqimiga qaytadi
                // EN: the result comes back on this (UI) thread.
                // RU: результат возвращается в этот (UI) поток.
                assert_eq!(std::thread::current().id(), tid);
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
