//! Android NativeActivity qobig'i: oyna, ko'p barmoqli teginish, EGL va kadr tsikli (alohida o'yin oqimida).
use crate::app::{App, TouchEv};
use crate::egl::{eglGetConfigAttrib, Egl};
use std::ffi::c_void;
use std::ptr::null_mut;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

#[repr(C)]
pub struct ANativeActivityCallbacks {
    on_start: Option<unsafe extern "C" fn(*mut ANativeActivity)>,
    on_resume: Option<unsafe extern "C" fn(*mut ANativeActivity)>,
    on_save_instance_state: Option<unsafe extern "C" fn(*mut ANativeActivity, *mut usize) -> *mut c_void>,
    on_pause: Option<unsafe extern "C" fn(*mut ANativeActivity)>,
    on_stop: Option<unsafe extern "C" fn(*mut ANativeActivity)>,
    on_destroy: Option<unsafe extern "C" fn(*mut ANativeActivity)>,
    on_window_focus_changed: Option<unsafe extern "C" fn(*mut ANativeActivity, i32)>,
    on_native_window_created: Option<unsafe extern "C" fn(*mut ANativeActivity, *mut c_void)>,
    on_native_window_resized: Option<unsafe extern "C" fn(*mut ANativeActivity, *mut c_void)>,
    on_native_window_redraw_needed: Option<unsafe extern "C" fn(*mut ANativeActivity, *mut c_void)>,
    on_native_window_destroyed: Option<unsafe extern "C" fn(*mut ANativeActivity, *mut c_void)>,
    on_input_queue_created: Option<unsafe extern "C" fn(*mut ANativeActivity, *mut c_void)>,
    on_input_queue_destroyed: Option<unsafe extern "C" fn(*mut ANativeActivity, *mut c_void)>,
    on_content_rect_changed: Option<unsafe extern "C" fn(*mut ANativeActivity, *const c_void)>,
    on_configuration_changed: Option<unsafe extern "C" fn(*mut ANativeActivity)>,
    on_low_memory: Option<unsafe extern "C" fn(*mut ANativeActivity)>,
}

#[repr(C)]
pub struct ANativeActivity {
    callbacks: *mut ANativeActivityCallbacks,
    vm: *mut c_void,
    env: *mut c_void,
    clazz: *mut c_void,
    internal_data_path: *const u8,
    external_data_path: *const u8,
    sdk_version: i32,
    instance: *mut c_void,
    asset_manager: *mut c_void,
    obb_path: *const u8,
}

#[link(name = "android")]
extern "C" {
    fn ALooper_prepare(opts: i32) -> *mut c_void;
    fn ALooper_pollOnce(timeout: i32, fd: *mut i32, events: *mut i32, data: *mut *mut c_void) -> i32;
    fn ALooper_wake(looper: *mut c_void);
    fn AInputQueue_attachLooper(q: *mut c_void, looper: *mut c_void, ident: i32, cb: *mut c_void, data: *mut c_void);
    fn AInputQueue_detachLooper(q: *mut c_void);
    fn AInputQueue_getEvent(q: *mut c_void, out: *mut *mut c_void) -> i32;
    fn AInputQueue_preDispatchEvent(q: *mut c_void, ev: *mut c_void) -> i32;
    fn AInputQueue_finishEvent(q: *mut c_void, ev: *mut c_void, handled: i32);
    fn AInputEvent_getType(ev: *const c_void) -> i32;
    fn AMotionEvent_getAction(ev: *const c_void) -> i32;
    fn AMotionEvent_getPointerCount(ev: *const c_void) -> usize;
    fn AMotionEvent_getPointerId(ev: *const c_void, idx: usize) -> i32;
    fn AMotionEvent_getX(ev: *const c_void, idx: usize) -> f32;
    fn AMotionEvent_getY(ev: *const c_void, idx: usize) -> f32;
    fn AKeyEvent_getAction(ev: *const c_void) -> i32;
    fn AKeyEvent_getKeyCode(ev: *const c_void) -> i32;
    fn ANativeWindow_getWidth(w: *mut c_void) -> i32;
    fn ANativeWindow_getHeight(w: *mut c_void) -> i32;
    fn ANativeWindow_setBuffersGeometry(w: *mut c_void, width: i32, height: i32, format: i32) -> i32;
    fn ANativeActivity_setWindowFlags(a: *mut ANativeActivity, add: u32, remove: u32);
}

#[link(name = "log")]
extern "C" {
    fn __android_log_write(prio: i32, tag: *const u8, text: *const u8) -> i32;
}

const INPUT_IDENT: i32 = 1000;

extern "C" {
    fn signal(sig: i32, handler: usize) -> usize;
    fn raise(sig: i32) -> i32;
}

static CRASH_PATH: std::sync::OnceLock<String> = std::sync::OnceLock::new();

extern "C" fn native_crash(sig: i32) {
    let name = match sig {
        11 => "SIGSEGV",
        6 => "SIGABRT (xotira tugashi yoki ichki xato)",
        7 => "SIGBUS",
        4 => "SIGILL",
        _ => "SIGFPE",
    };
    if let Some(p) = CRASH_PATH.get() {
        let t = std::thread::current();
        let _ = std::fs::write(p, format!("Native {name} ({})", t.name().unwrap_or("?")));
    }
    unsafe {
        signal(sig, 0);
        raise(sig);
    }
}

fn log(msg: &str) {
    let t = std::ffi::CString::new(msg.replace('\0', " ")).unwrap_or_default();
    unsafe { __android_log_write(4, b"GigaCraft\0".as_ptr(), t.as_ptr() as *const u8) };
}

struct Shared {
    window: *mut c_void,
    queue: *mut c_void,
    thread_window: *mut c_void,
    quit: bool,
    paused: bool,
    save_req: bool,
    looper: *mut c_void,
}

unsafe impl Send for Shared {}

struct Sync_ {
    m: Mutex<Shared>,
    cv: Condvar,
}

static SHARED: Mutex<Option<Arc<Sync_>>> = Mutex::new(None);

fn shared() -> Option<Arc<Sync_>> {
    SHARED.lock().ok()?.clone()
}

fn wake(g: &Shared) {
    if !g.looper.is_null() {
        unsafe { ALooper_wake(g.looper) };
    }
}

unsafe extern "C" fn window_created(_: *mut ANativeActivity, w: *mut c_void) {
    if let Some(s) = shared() {
        let mut g = s.m.lock().unwrap();
        g.window = w;
        wake(&g);
    }
}

unsafe extern "C" fn window_destroyed(_: *mut ANativeActivity, _: *mut c_void) {
    // Oyna qaytgach yaroqsiz bo'ladi: o'yin oqimi uni qo'yib yuborguncha kutamiz.
    if let Some(s) = shared() {
        let mut g = s.m.lock().unwrap();
        g.window = null_mut();
        wake(&g);
        while !g.thread_window.is_null() && !g.quit {
            g = s.cv.wait(g).unwrap();
        }
    }
}

unsafe extern "C" fn queue_created(_: *mut ANativeActivity, q: *mut c_void) {
    if let Some(s) = shared() {
        let mut g = s.m.lock().unwrap();
        g.queue = q;
        wake(&g);
    }
}

unsafe extern "C" fn queue_destroyed(_: *mut ANativeActivity, _: *mut c_void) {
    if let Some(s) = shared() {
        let mut g = s.m.lock().unwrap();
        g.queue = null_mut();
        wake(&g);
    }
}

unsafe extern "C" fn on_pause(_: *mut ANativeActivity) {
    if let Some(s) = shared() {
        let mut g = s.m.lock().unwrap();
        g.paused = true;
        g.save_req = true;
        wake(&g);
    }
}

unsafe extern "C" fn on_resume(_: *mut ANativeActivity) {
    if let Some(s) = shared() {
        let mut g = s.m.lock().unwrap();
        g.paused = false;
        wake(&g);
    }
}

unsafe extern "C" fn on_destroy(_: *mut ANativeActivity) {
    if let Some(s) = shared() {
        let mut g = s.m.lock().unwrap();
        g.quit = true;
        wake(&g);
        // O'yin oqimi saqlab bo'lishini kutamiz.
        let (g2, _) = s.cv.wait_timeout_while(g, std::time::Duration::from_secs(3), |g| !g.window.is_null() || !g.thread_window.is_null()).unwrap();
        drop(g2);
    }
}

#[no_mangle]
pub unsafe extern "C" fn ANativeActivity_onCreate(activity: *mut ANativeActivity, _state: *mut u8, _n: usize) {
    let a = &mut *activity;
    let dir = if a.internal_data_path.is_null() {
        String::new()
    } else {
        std::ffi::CStr::from_ptr(a.internal_data_path as *const std::ffi::c_char).to_string_lossy().into_owned()
    };
    let d2 = dir.clone();
    std::panic::set_hook(Box::new(move |i| {
        let t = std::thread::current();
        let msg = format!("Rust panic ({}): {i}", t.name().unwrap_or("nomsiz"));
        log(&msg);
        if !d2.is_empty() {
            let _ = std::fs::write(format!("{d2}/crash.txt"), &msg);
        }
    }));
    if !dir.is_empty() {
        let _ = CRASH_PATH.set(format!("{dir}/crash.txt"));
        for sg in [11, 6, 7, 4, 8] {
            signal(sg, native_crash as usize);
        }
    }
    // To'liq ekran + ekran o'chmasin.
    ANativeActivity_setWindowFlags(activity, 0x400 | 0x80, 0);
    let s = Arc::new(Sync_ {
        m: Mutex::new(Shared {
            window: null_mut(),
            queue: null_mut(),
            thread_window: null_mut(),
            quit: false,
            paused: false,
            save_req: false,
            looper: null_mut(),
        }),
        cv: Condvar::new(),
    });
    *SHARED.lock().unwrap() = Some(s.clone());
    let cb = &mut *a.callbacks;
    cb.on_native_window_created = Some(window_created);
    cb.on_native_window_destroyed = Some(window_destroyed);
    cb.on_input_queue_created = Some(queue_created);
    cb.on_input_queue_destroyed = Some(queue_destroyed);
    cb.on_pause = Some(on_pause);
    cb.on_resume = Some(on_resume);
    cb.on_destroy = Some(on_destroy);
    let _ = std::thread::Builder::new().name("gigacraft".into()).spawn(move || run(s, dir));
}

fn handle_input(queue: *mut c_void, app: &mut Option<App>) {
    unsafe {
        let mut ev: *mut c_void = null_mut();
        while AInputQueue_getEvent(queue, &mut ev) >= 0 {
            if AInputQueue_preDispatchEvent(queue, ev) != 0 {
                continue;
            }
            let mut handled = 0;
            match AInputEvent_getType(ev) {
                1 => {
                    // Tugma: Orqaga
                    if AKeyEvent_getKeyCode(ev) == 4 {
                        if AKeyEvent_getAction(ev) == 0 {
                            if let Some(a) = app.as_mut() {
                                a.back();
                            }
                        }
                        handled = 1;
                    }
                }
                2 => {
                    let raw = AMotionEvent_getAction(ev);
                    let act = raw & 0xff;
                    let idx = ((raw >> 8) & 0xff) as usize;
                    if let Some(a) = app.as_mut() {
                        let n = AMotionEvent_getPointerCount(ev);
                        match act {
                            0 | 5 => {
                                let i = if act == 0 { 0 } else { idx };
                                a.touch(TouchEv::Down(AMotionEvent_getPointerId(ev, i), AMotionEvent_getX(ev, i), AMotionEvent_getY(ev, i)));
                            }
                            1 | 6 => {
                                let i = if act == 1 { 0 } else { idx };
                                a.touch(TouchEv::Up(AMotionEvent_getPointerId(ev, i)));
                            }
                            2 => {
                                for i in 0..n {
                                    a.touch(TouchEv::Move(AMotionEvent_getPointerId(ev, i), AMotionEvent_getX(ev, i), AMotionEvent_getY(ev, i)));
                                }
                            }
                            3 => a.touch(TouchEv::Cancel),
                            _ => {}
                        }
                    }
                    handled = 1;
                }
                _ => {}
            }
            AInputQueue_finishEvent(queue, ev, handled);
        }
    }
}

fn run(s: Arc<Sync_>, dir: String) {
    let looper = unsafe { ALooper_prepare(1) };
    s.m.lock().unwrap().looper = looper;
    let mut egl: Option<Egl> = None;
    let mut app: Option<App> = None;
    let mut window: *mut c_void = null_mut();
    let mut queue: *mut c_void = null_mut();
    let mut failed = false;
    let mut marked = false;
    let mut last = Instant::now();
    loop {
        let paused;
        let save_req;
        {
            let mut g = s.m.lock().unwrap();
            if g.quit {
                break;
            }
            if g.window != window {
                window = g.window;
                if let Some(e) = &mut egl {
                    if window.is_null() {
                        e.release_surface();
                    } else {
                        let _ = attach(e, window);
                    }
                }
                g.thread_window = window;
                s.cv.notify_all();
            }
            if g.queue != queue {
                if !queue.is_null() {
                    unsafe { AInputQueue_detachLooper(queue) };
                }
                queue = g.queue;
                if !queue.is_null() {
                    unsafe { AInputQueue_attachLooper(queue, looper, INPUT_IDENT, null_mut(), null_mut()) };
                }
            }
            paused = g.paused;
            save_req = std::mem::take(&mut g.save_req);
        }
        if save_req {
            if let Some(a) = app.as_mut() {
                a.save();
                a.mark(false);
                marked = false;
            }
        }
        if !paused && !marked {
            if let Some(a) = app.as_ref() {
                a.mark(true);
                marked = true;
            }
        }
        // Hodisalarni o'qiymiz (oyna bor va pauzada bo'lmasa kutmaymiz)
        let mut timeout = if window.is_null() || paused || failed { -1 } else { 0 };
        loop {
            let r = unsafe { ALooper_pollOnce(timeout, null_mut(), null_mut(), null_mut()) };
            if r == INPUT_IDENT {
                if !queue.is_null() {
                    handle_input(queue, &mut app);
                }
            } else if r != -2 {
                break;
            }
            timeout = 0;
        }
        if window.is_null() || paused || failed {
            continue;
        }
        if egl.is_none() {
            match Egl::new(false) {
                Ok(mut e) => {
                    if let Err(er) = attach(&mut e, window) {
                        log(&er);
                        fail(&dir, &er);
                        failed = true;
                        continue;
                    }
                    egl = Some(e);
                }
                Err(er) => {
                    log(&er);
                    fail(&dir, &er);
                    failed = true;
                    continue;
                }
            }
        }
        if app.is_none() {
            match App::new(&dir) {
                Ok(a) => app = Some(a),
                Err(er) => {
                    log(&er);
                    fail(&dir, &er);
                    failed = true;
                    continue;
                }
            }
            last = Instant::now();
        }
        let (w, h) = unsafe { (ANativeWindow_getWidth(window), ANativeWindow_getHeight(window)) };
        if w <= 0 || h <= 0 {
            continue;
        }
        let now = Instant::now();
        let dt = (now - last).as_secs_f32().min(0.1);
        last = now;
        if let (Some(a), Some(e)) = (app.as_mut(), egl.as_mut()) {
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| a.frame(w, h, dt)));
            if r.is_err() {
                // Panika (xabar crash.txt ga yozilgan): kadrni o'tkazib yuboramiz, ilova yopilmasin.
                continue;
            }
            if !e.swap() {
                // Kontekst yo'qoldi: hammasini qaytadan quramiz
                a.save();
                app = None;
                egl = None;
            }
        }
    }
    if let Some(a) = app.as_mut() {
        a.save();
    }
    drop(app);
    drop(egl);
    let mut g = s.m.lock().unwrap();
    g.thread_window = null_mut();
    s.cv.notify_all();
}

fn fail(dir: &str, msg: &str) {
    if !dir.is_empty() {
        let _ = std::fs::write(format!("{dir}/crash.txt"), msg);
    }
}

fn attach(e: &mut Egl, window: *mut c_void) -> Result<(), String> {
    let mut fmt = 0;
    unsafe {
        eglGetConfigAttrib(e.dpy, e.cfg, 0x302E, &mut fmt);
        ANativeWindow_setBuffersGeometry(window, 0, 0, fmt);
    }
    e.set_window(window)
}
