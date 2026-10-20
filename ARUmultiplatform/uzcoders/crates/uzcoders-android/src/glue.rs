//! NativeActivity bilan ishlash: oyna, kiritish navbati va chizish oqimi.

use std::ffi::c_void;
use std::ptr::null_mut;
use std::sync::{Arc, Condvar, Mutex};
use uzcoders_core::{PointerEvent, PointerKind, Size};
use uzcoders_render::Canvas;
use uzcoders_ui::Ui;

#[repr(C)]
pub struct ANativeActivityCallbacks {
    on_start: Option<unsafe extern "C" fn(*mut ANativeActivity)>,
    on_resume: Option<unsafe extern "C" fn(*mut ANativeActivity)>,
    on_save_instance_state:
        Option<unsafe extern "C" fn(*mut ANativeActivity, *mut usize) -> *mut c_void>,
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

#[repr(C)]
struct WindowBuffer {
    width: i32,
    height: i32,
    stride: i32,
    format: i32,
    bits: *mut u8,
    reserved: [u32; 6],
}

#[link(name = "android")]
extern "C" {
    fn ALooper_prepare(opts: i32) -> *mut c_void;
    fn ALooper_pollOnce(
        timeout_ms: i32,
        out_fd: *mut i32,
        out_events: *mut i32,
        out_data: *mut *mut c_void,
    ) -> i32;
    fn ALooper_wake(looper: *mut c_void);
    fn AInputQueue_attachLooper(
        q: *mut c_void,
        looper: *mut c_void,
        ident: i32,
        cb: *mut c_void,
        data: *mut c_void,
    );
    fn AInputQueue_detachLooper(q: *mut c_void);
    fn AInputQueue_getEvent(q: *mut c_void, out: *mut *mut c_void) -> i32;
    fn AInputQueue_preDispatchEvent(q: *mut c_void, ev: *mut c_void) -> i32;
    fn AInputQueue_finishEvent(q: *mut c_void, ev: *mut c_void, handled: i32);
    fn AInputEvent_getType(ev: *const c_void) -> i32;
    fn AMotionEvent_getAction(ev: *const c_void) -> i32;
    fn AMotionEvent_getX(ev: *const c_void, idx: usize) -> f32;
    fn AMotionEvent_getY(ev: *const c_void, idx: usize) -> f32;
    fn ANativeWindow_getWidth(w: *mut c_void) -> i32;
    fn ANativeWindow_getHeight(w: *mut c_void) -> i32;
    fn ANativeWindow_setBuffersGeometry(
        w: *mut c_void,
        width: i32,
        height: i32,
        format: i32,
    ) -> i32;
    fn ANativeWindow_lock(w: *mut c_void, out: *mut WindowBuffer, bounds: *mut c_void) -> i32;
    fn ANativeWindow_unlockAndPost(w: *mut c_void) -> i32;
    fn AConfiguration_new() -> *mut c_void;
    fn AConfiguration_fromAssetManager(cfg: *mut c_void, am: *mut c_void);
    fn AConfiguration_getDensity(cfg: *mut c_void) -> i32;
    fn AConfiguration_delete(cfg: *mut c_void);
}

#[link(name = "log")]
extern "C" {
    fn __android_log_write(prio: i32, tag: *const u8, text: *const u8) -> i32;
}

/// Logcat'ga yozadi (teg: "uzcoders").
pub fn log(msg: &str) {
    let mut m = msg.replace('\0', " ").into_bytes();
    m.push(0);
    unsafe {
        __android_log_write(4, b"uzcoders\0".as_ptr(), m.as_ptr());
    }
}

const INPUT_IDENT: i32 = 1;
const WINDOW_FORMAT_RGBA_8888: i32 = 1;

struct Shared {
    window: *mut c_void,
    queue: *mut c_void,
    thread_window: *mut c_void,
    redraw: bool,
    quit: bool,
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

fn wake(s: &Shared) {
    if !s.looper.is_null() {
        unsafe { ALooper_wake(s.looper) };
    }
}

unsafe extern "C" fn window_created(_: *mut ANativeActivity, w: *mut c_void) {
    if let Some(s) = shared() {
        let mut g = s.m.lock().unwrap();
        g.window = w;
        g.redraw = true;
        wake(&g);
    }
}

unsafe extern "C" fn window_resized(_: *mut ANativeActivity, _: *mut c_void) {
    if let Some(s) = shared() {
        let mut g = s.m.lock().unwrap();
        g.redraw = true;
        wake(&g);
    }
}

unsafe extern "C" fn window_destroyed(_: *mut ANativeActivity, _: *mut c_void) {
    // Oyna qaytgandan keyin yaroqsiz: chizish oqimi uni qo'yib yuborganini kutamiz.
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

unsafe extern "C" fn focus_or_resume(_: *mut ANativeActivity) {
    if let Some(s) = shared() {
        let mut g = s.m.lock().unwrap();
        g.redraw = true;
        wake(&g);
    }
}

unsafe extern "C" fn focus_changed(_: *mut ANativeActivity, _: i32) {
    focus_or_resume(null_mut());
}

unsafe extern "C" fn destroy(_: *mut ANativeActivity) {
    if let Some(s) = shared() {
        let mut g = s.m.lock().unwrap();
        g.quit = true;
        wake(&g);
        s.cv.notify_all();
    }
}

/// Makro chaqiradigan kirish nuqtasi.
///
/// # Safety
/// `activity` — Android bergan yaroqli ko'rsatkich.
pub unsafe fn on_create(activity: *mut ANativeActivity, make_ui: fn(f32) -> Ui) {
    std::panic::set_hook(Box::new(|i| log(&format!("PANIC: {i}"))));
    let a = &mut *activity;
    let mut density = 160;
    let cfg = AConfiguration_new();
    if !cfg.is_null() {
        AConfiguration_fromAssetManager(cfg, a.asset_manager);
        density = AConfiguration_getDensity(cfg);
        AConfiguration_delete(cfg);
    }
    let scale = if density > 0 && density < 0xfffe {
        (density as f32 / 160.0).clamp(1.0, 4.0)
    } else {
        2.0
    };

    let s = Arc::new(Sync_ {
        m: Mutex::new(Shared {
            window: null_mut(),
            queue: null_mut(),
            thread_window: null_mut(),
            redraw: true,
            quit: false,
            looper: null_mut(),
        }),
        cv: Condvar::new(),
    });
    *SHARED.lock().unwrap() = Some(s.clone());

    let cb = &mut *a.callbacks;
    cb.on_native_window_created = Some(window_created);
    cb.on_native_window_resized = Some(window_resized);
    cb.on_native_window_redraw_needed = Some(window_resized);
    cb.on_native_window_destroyed = Some(window_destroyed);
    cb.on_input_queue_created = Some(queue_created);
    cb.on_input_queue_destroyed = Some(queue_destroyed);
    cb.on_resume = Some(focus_or_resume);
    cb.on_window_focus_changed = Some(focus_changed);
    cb.on_destroy = Some(destroy);

    std::thread::spawn(move || run(s, make_ui, scale));
}

fn run(s: Arc<Sync_>, make_ui: fn(f32) -> Ui, scale: f32) {
    let looper = unsafe { ALooper_prepare(0) };
    s.m.lock().unwrap().looper = looper;
    let mut ui = make_ui(scale);
    ui.set_scale(scale);
    let mut canvas: Option<Canvas> = None;
    let mut window: *mut c_void = null_mut();
    let mut queue: *mut c_void = null_mut();
    let mut dims = (0i32, 0i32);
    let mut force = true;

    loop {
        {
            let mut g = s.m.lock().unwrap();
            if g.quit {
                g.thread_window = null_mut();
                s.cv.notify_all();
                break;
            }
            if g.window != window {
                window = g.window;
                g.thread_window = window;
                dims = (0, 0);
                force = true;
                s.cv.notify_all();
            }
            if g.queue != queue {
                if !queue.is_null() {
                    unsafe { AInputQueue_detachLooper(queue) };
                }
                queue = g.queue;
                if !queue.is_null() {
                    unsafe {
                        AInputQueue_attachLooper(queue, looper, INPUT_IDENT, null_mut(), null_mut())
                    };
                }
            }
            if g.redraw {
                g.redraw = false;
                force = true;
            }
        }

        if !window.is_null() {
            let (w, h) = unsafe {
                (
                    ANativeWindow_getWidth(window),
                    ANativeWindow_getHeight(window),
                )
            };
            if (w, h) != dims && w > 0 && h > 0 {
                dims = (w, h);
                unsafe { ANativeWindow_setBuffersGeometry(window, w, h, WINDOW_FORMAT_RGBA_8888) };
                force = true;
            }
            if (force || ui.needs_frame()) && dims.0 > 0 {
                force = false;
                draw(&mut ui, &mut canvas, window, dims);
            }
        }

        let timeout = if !window.is_null() && ui.needs_frame() {
            0
        } else {
            -1
        };
        let ident = unsafe { ALooper_pollOnce(timeout, null_mut(), null_mut(), null_mut()) };
        if ident == INPUT_IDENT && !queue.is_null() {
            drain_input(queue, &mut ui);
        }
    }
    if !queue.is_null() {
        unsafe { AInputQueue_detachLooper(queue) };
    }
}

fn draw(ui: &mut Ui, canvas: &mut Option<Canvas>, window: *mut c_void, dims: (i32, i32)) {
    let (w, h) = (dims.0 as usize, dims.1 as usize);
    let cmds = ui.frame(Size::new(w as f32, h as f32));
    let c = canvas.get_or_insert_with(|| Canvas::new(w, h, ui.font()));
    if c.width != w || c.height != h {
        c.resize(w, h);
    }
    c.clear(uzcoders_core::Color::BLACK);
    c.execute(&cmds);
    let mut buf = WindowBuffer {
        width: 0,
        height: 0,
        stride: 0,
        format: 0,
        bits: null_mut(),
        reserved: [0; 6],
    };
    unsafe {
        if ANativeWindow_lock(window, &mut buf, null_mut()) != 0 || buf.bits.is_null() {
            return;
        }
        let dst =
            std::slice::from_raw_parts_mut(buf.bits, buf.stride as usize * buf.height as usize * 4);
        crate::blit_rows(
            &c.pixels,
            (buf.width as usize).min(w),
            (buf.height as usize).min(h),
            dst,
            buf.stride as usize,
        );
        ANativeWindow_unlockAndPost(window);
    }
}

fn drain_input(queue: *mut c_void, ui: &mut Ui) {
    unsafe {
        let mut ev: *mut c_void = null_mut();
        while AInputQueue_getEvent(queue, &mut ev) >= 0 {
            if AInputQueue_preDispatchEvent(queue, ev) != 0 {
                continue;
            }
            let mut handled = 0;
            if AInputEvent_getType(ev) == 2 {
                // AINPUT_EVENT_TYPE_MOTION; faqat birinchi barmoq.
                let kind = match AMotionEvent_getAction(ev) & 0xff {
                    0 => Some(PointerKind::Down),
                    1 => Some(PointerKind::Up),
                    2 => Some(PointerKind::Move),
                    3 => Some(PointerKind::Cancel),
                    _ => None,
                };
                if let Some(kind) = kind {
                    let (x, y) = (AMotionEvent_getX(ev, 0), AMotionEvent_getY(ev, 0));
                    ui.pointer(PointerEvent::new(kind, x, y));
                    handled = 1;
                }
            }
            AInputQueue_finishEvent(queue, ev, handled);
        }
    }
}
