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

pub(crate) fn wake() {
    if let Some(w) = *WAKER.lock().unwrap() {
        w();
    }
}

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

static CRUMBS: Mutex<std::collections::VecDeque<String>> = Mutex::new(std::collections::VecDeque::new());

/// UZ: So'nggi amallar yozuvi (ko'pi bilan 24 ta): yiqilish hisobotiga kiradi ("nima qilinayotgan edi").
/// EN: Records a recent action (at most 24 kept): included in the crash report ("what was happening").
/// RU: Записывает недавнее действие (хранится не более 24): попадает в отчёт о сбое («что происходило»).
pub fn breadcrumb(msg: &str) {
    if let Ok(mut c) = CRUMBS.try_lock() {
        if c.len() >= 24 {
            c.pop_front();
        }
        c.push_back(msg.chars().take(120).collect());
    }
}

/// UZ: Yiqilish hisoboti matni: so'nggi amallar, oqimlar va xotira (Linux/Android `/proc/self/status`).
/// EN: Crash report text: recent actions, thread count and memory (Linux/Android `/proc/self/status`).
/// RU: Текст отчёта о сбое: недавние действия, число потоков и память (Linux/Android `/proc/self/status`).
pub fn crash_context() -> String {
    let mut out = String::new();
    if let Ok(st) = std::fs::read_to_string("/proc/self/status") {
        for l in st.lines() {
            if l.starts_with("Threads:") || l.starts_with("VmRSS:") || l.starts_with("VmSize:") {
                out.push_str(l.split_whitespace().collect::<Vec<_>>().join(" ").as_str());
                out.push('\n');
            }
        }
    }
    out.push_str(&format!("Fon vazifalari kutilmoqda: {}\n", pending_tasks()));
    if let Ok(c) = CRUMBS.try_lock() {
        out.push_str("So'nggi amallar:\n");
        for m in c.iter() {
            out.push_str("  - ");
            out.push_str(m);
            out.push('\n');
        }
    }
    out
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
    submit(Box::new(move || {
        let r = work();
        QUEUE.lock().unwrap().push((id, Box::new(r)));
        if let Some(w) = *WAKER.lock().unwrap() {
            w();
        }
    }));
}

type Job = Box<dyn FnOnce() + Send>;

/// UZ: Oqimlar puli chegarasi: ko'p vazifa bir vaqtda kelsa, yangi oqim ochilmaydi, navbatda kutadi (aks holda "failed to spawn thread").
/// EN: Thread pool limit: when many tasks arrive at once no new threads are created, they wait in a queue (otherwise "failed to spawn thread").
/// RU: Предел пула потоков: при множестве задач новые потоки не создаются, а ждут в очереди (иначе "failed to spawn thread").
const MAX_WORKERS: usize = 12;

struct Pool {
    jobs: std::collections::VecDeque<Job>,
    workers: usize,
    idle: usize,
}

static POOL: Mutex<Pool> = Mutex::new(Pool { jobs: std::collections::VecDeque::new(), workers: 0, idle: 0 });
static POOL_CV: std::sync::Condvar = std::sync::Condvar::new();

fn submit(job: Job) {
    let mut p = POOL.lock().unwrap();
    p.jobs.push_back(job);
    if p.idle > 0 {
        POOL_CV.notify_one();
        return;
    }
    // UZ: 4 MB stek: chuqur rekursiyali chizuvchi (Lottie) uchun yetarli. EN: 4 MB stack: enough for the deeply recursive renderer (Lottie). RU: Стек 4 МБ: достаточно для глубоко рекурсивного рендера (Lottie).
    if p.workers < MAX_WORKERS && std::thread::Builder::new().stack_size(4 << 20).spawn(worker).is_ok() {
        p.workers += 1;
    }
    // UZ: Oqim ochib bo'lmasa ham ish navbatda qoladi: mavjud oqimlar uni oladi.
    // EN: Even if a thread cannot be created the job stays queued: existing workers pick it up.
    // RU: Даже если поток создать нельзя, задача остаётся в очереди: её заберут существующие потоки.
}

fn worker() {
    loop {
        let job = {
            let mut p = POOL.lock().unwrap();
            loop {
                if let Some(j) = p.jobs.pop_front() {
                    break Some(j);
                }
                p.idle += 1;
                let (g, t) = POOL_CV.wait_timeout(p, std::time::Duration::from_secs(5)).unwrap();
                p = g;
                p.idle -= 1;
                if t.timed_out() && p.jobs.is_empty() {
                    p.workers -= 1;
                    break None;
                }
            }
        };
        match job {
            Some(j) => {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(j));
            }
            None => return,
        }
    }
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

#[cfg(test)]
mod pool_tests {
    use super::*;

    #[test]
    fn kop_vazifa_cheklangan_oqimlarda_bajariladi() {
        let n = std::sync::Arc::new(AtomicUsize::new(0));
        for _ in 0..500 {
            let n = n.clone();
            submit(Box::new(move || {
                std::thread::sleep(std::time::Duration::from_millis(2));
                n.fetch_add(1, Ordering::SeqCst);
            }));
        }
        assert!(POOL.lock().unwrap().workers <= MAX_WORKERS);
        for _ in 0..500 {
            if n.load(Ordering::SeqCst) == 500 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_eq!(n.load(Ordering::SeqCst), 500);
    }
}
