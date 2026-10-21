//! UZ: NativeActivity bilan ishlash: oyna, kiritish navbati va chizish oqimi.
//! EN: Working with NativeActivity: the window, the input queue and the render thread.
//! RU: Работа с NativeActivity: окно, очередь ввода и поток рисования.

use crate::egl::Egl;
use crate::jni;
use std::ffi::c_void;
use std::ptr::null_mut;
use std::sync::{Arc, Condvar, Mutex};
use uzcoders_core::{EdgeInsets, PointerEvent, PointerKind, Size};
use uzcoders_gpu::{gl::GlRenderer, Batcher};
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
    on_content_rect_changed: Option<unsafe extern "C" fn(*mut ANativeActivity, *const ARect)>,
    on_configuration_changed: Option<unsafe extern "C" fn(*mut ANativeActivity)>,
    on_low_memory: Option<unsafe extern "C" fn(*mut ANativeActivity)>,
}

#[repr(C)]
pub struct ARect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
pub struct ANativeActivity {
    callbacks: *mut ANativeActivityCallbacks,
    pub(crate) vm: *mut c_void,
    env: *mut c_void,
    pub(crate) clazz: *mut c_void,
    pub(crate) internal_data_path: *const u8,
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
    fn AKeyEvent_getAction(ev: *const c_void) -> i32;
    fn AKeyEvent_getKeyCode(ev: *const c_void) -> i32;
    fn AKeyEvent_getMetaState(ev: *const c_void) -> i32;
    fn AMotionEvent_getX(ev: *const c_void, idx: usize) -> f32;
    fn AMotionEvent_getEventTime(ev: *const c_void) -> i64;
    fn AMotionEvent_getHistorySize(ev: *const c_void) -> usize;
    fn AMotionEvent_getHistoricalX(ev: *const c_void, idx: usize, pos: usize) -> f32;
    fn AMotionEvent_getHistoricalY(ev: *const c_void, idx: usize, pos: usize) -> f32;
    fn AMotionEvent_getHistoricalEventTime(ev: *const c_void, pos: usize) -> i64;
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

/// UZ: Logcat'ga yozadi (teg: "uzcoders").
/// EN: Writes to Logcat (tag: "uzcoders").
/// RU: Пишет в Logcat (тег: «uzcoders»).
pub fn log(msg: &str) {
    let mut m = msg.replace('\0', " ").into_bytes();
    m.push(0);
    unsafe {
        __android_log_write(4, b"uzcoders\0".as_ptr(), m.as_ptr());
    }
}

/// UZ: GPU bilan chizish: EGL + OpenGL ES + paketlovchi (batcher).
/// EN: GPU drawing: EGL + OpenGL ES + the batcher.
/// RU: Рисование на GPU: EGL + OpenGL ES + упаковщик (batcher).
struct Gpu {
    egl: Egl,
    gl: GlRenderer,
    batcher: Batcher,
}

impl Gpu {
    fn new(window: *mut c_void, ui: &Ui) -> Result<Gpu, String> {
        let mut egl = Egl::new()?;
        egl.set_window(window)?;
        let gl = unsafe { GlRenderer::new()? };
        Ok(Gpu {
            egl,
            gl,
            batcher: Batcher::new(ui.faces()),
        })
    }
}

static LOOPER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static ACTIVITY: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static VIDEO_WINDOW: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static PICKED: Mutex<Vec<String>> = Mutex::new(Vec::new());
static FILES_DIR: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// UZ: Ilovaning ichki fayllar papkasi (`getFilesDir`).
/// EN: The app's private files directory (`getFilesDir`).
/// RU: Папка внутренних файлов приложения (`getFilesDir`).
pub fn files_dir() -> String {
    FILES_DIR.get().cloned().unwrap_or_default()
}

fn wake_ui() {
    let l = LOOPER.load(std::sync::atomic::Ordering::SeqCst);
    if l != 0 {
        unsafe { ALooper_wake(l as *mut c_void) };
    }
}

const INPUT_IDENT: i32 = 1;
const WINDOW_FORMAT_RGBA_8888: i32 = 1;

/// UZ: Java yelimidan (IME) kelgan hodisa.
/// EN: An event coming from the Java glue (IME).
/// RU: Событие от Java-прослойки (IME).
pub enum ImeEvent {
    Commit(String),
    Composing(String),
    Delete(usize, usize),
    Finish,
    Enter,
    /// UZ: Fayl tanlagich natijasi: keshdagi fayl yo'li.
    /// EN: File picker result: the path of the file copied into the cache.
    /// RU: Результат выбора файла: путь к файлу в кэше.
    Picked(String),
}

/// UZ: Java yelimi chaqiradi (UI oqimidan): hodisani chizish oqimiga uzatadi.
/// EN: Called by the Java glue (from the UI thread): hands the event to the render thread.
/// RU: Вызывается Java-прослойкой (из UI-потока): передаёт событие потоку рисования.
pub fn ime_event(e: ImeEvent) {
    if let Some(s) = shared() {
        let mut g = s.m.lock().unwrap();
        g.ime.push(e);
        wake(&g);
    }
}

pub use crate::jni::jstring;

static VIDEO_UPLOADS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static VIDEO_TEX: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

fn video_create(tex: u32) {
    VIDEO_TEX.store(tex, std::sync::atomic::Ordering::SeqCst);
    let a = ACTIVITY.load(std::sync::atomic::Ordering::SeqCst);
    if a != 0 {
        let w = unsafe { jni::video_surface(a as *mut ANativeActivity, tex) };
        VIDEO_WINDOW.store(w as usize, std::sync::atomic::Ordering::SeqCst);
    }
}

fn video_update() -> Option<[f32; 16]> {
    let a = ACTIVITY.load(std::sync::atomic::Ordering::SeqCst);
    if a == 0 {
        return None;
    }
    // UZ: Oldingi pleyer Surface'ni bo'shatgan bo'lsa (yangi qism ochilganda), shu teksturaga yangisini yaratamiz.
    // EN: If the previous player released the Surface (when a new episode opens), a new one is created for the same texture.
    // RU: Если прежний плеер освободил Surface (при открытии новой серии), для этой же текстуры создаётся новый.
    let tex = VIDEO_TEX.load(std::sync::atomic::Ordering::SeqCst);
    if tex != 0 && VIDEO_WINDOW.load(std::sync::atomic::Ordering::SeqCst) == 0 {
        video_create(tex);
    }
    let r = unsafe { jni::video_update(a as *mut ANativeActivity) };
    if r.is_some() {
        VIDEO_UPLOADS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
    r
}

/// UZ: Dekoder yozadigan `ANativeWindow*` (video elementi birinchi marta chizilgach tayyor bo'ladi; tayyor bo'lmaguncha `null`).
/// EN: The `ANativeWindow*` the decoder writes to (ready after the video element is drawn for the first time; `null` until then).
/// RU: `ANativeWindow*`, в который пишет декодер (готов после первой отрисовки элемента видео; до этого `null`).
pub fn video_window() -> *mut c_void {
    VIDEO_WINDOW.load(std::sync::atomic::Ordering::SeqCst) as *mut c_void
}

/// UZ: Video diagnostikasi: (tekstura nomi, Surface bormi, teksturaga o'tgan kadrlar soni).
/// EN: Video diagnostics: (texture name, whether a Surface exists, number of frames moved into the texture).
/// RU: Диагностика видео: (имя текстуры, есть ли Surface, число кадров, перенесённых в текстуру).
pub fn video_stats() -> (u32, bool, u64) {
    (VIDEO_TEX.load(std::sync::atomic::Ordering::SeqCst), VIDEO_WINDOW.load(std::sync::atomic::Ordering::SeqCst) != 0, VIDEO_UPLOADS.load(std::sync::atomic::Ordering::SeqCst))
}

/// UZ: Video Surface'ni bo'shatadi (pleyer yopilganda).
/// EN: Releases the video Surface (when the player is closed).
/// RU: Освобождает видео-Surface (при закрытии плеера).
pub fn video_release() {
    VIDEO_WINDOW.store(0, std::sync::atomic::Ordering::SeqCst);
    let a = ACTIVITY.load(std::sync::atomic::Ordering::SeqCst);
    if a != 0 {
        // UZ: Pleyer oqimidan chaqiriladi: ish tugagach oqim JVM'dan ajratiladi (keyingi chaqiruvda qayta ulanadi).
        // EN: Called from a player thread: after the work the thread is detached from the JVM (it re-attaches on the next call).
        // RU: Вызывается из потока плеера: после работы поток отсоединяется от JVM (при следующем вызове подключается снова).
        unsafe {
            jni::video_release(a as *mut ANativeActivity);
            jni::detach(a as *mut ANativeActivity);
        }
    }
}

/// UZ: To'liq ekran: gorizontal holat va tizim panellari yashirin.
/// EN: Fullscreen: landscape orientation with the system bars hidden.
/// RU: Полноэкранный режим: горизонтальная ориентация, системные панели скрыты.
pub fn set_fullscreen(on: bool) {
    let a = ACTIVITY.load(std::sync::atomic::Ordering::SeqCst);
    if a != 0 {
        unsafe { jni::call_bool(a as *mut ANativeActivity, b"setFullscreen\0", on) };
    }
}

/// UZ: Ekran o'chmasin (video ijro etilayotganda).
/// EN: Keeps the screen on (while a video is playing).
/// RU: Не даёт экрану гаснуть (во время воспроизведения видео).
pub fn keep_screen_on(on: bool) {
    let a = ACTIVITY.load(std::sync::atomic::Ordering::SeqCst);
    if a != 0 {
        unsafe { jni::call_bool(a as *mut ANativeActivity, b"keepScreenOn\0", on) };
    }
}

fn call_activity(name: &[u8], s: &str) {
    let a = ACTIVITY.load(std::sync::atomic::Ordering::SeqCst);
    if a != 0 {
        unsafe { jni::call_str(a as *mut ANativeActivity, name, s) };
    }
}

/// UZ: Videoning bitta kadrini JPEG fayliga yozadi (`MediaMetadataRetriever`): `arg` = `"<kirish.mp4>|<chiqish.jpg>"`.
/// Bloklaydi; fon oqimidan chaqiriladi (oqim keyin JVM'dan ajratiladi).
/// EN: Writes one video frame to a JPEG file (`MediaMetadataRetriever`): `arg` = `"<input.mp4>|<output.jpg>"`.
/// Blocking; called from a background thread (the thread is detached from the JVM afterwards).
/// RU: Записывает один кадр видео в JPEG-файл (`MediaMetadataRetriever`): `arg` = `"<вход.mp4>|<выход.jpg>"`.
/// Блокирует; вызывается из фонового потока (затем поток отсоединяется от JVM).
pub fn frame_to_file(arg: &str) {
    let a = ACTIVITY.load(std::sync::atomic::Ordering::SeqCst);
    if a != 0 {
        unsafe {
            jni::call_str(a as *mut ANativeActivity, b"frameToFile\0", arg);
            jni::detach(a as *mut ANativeActivity);
        }
    }
}

/// UZ: Activity'ning ixtiyoriy `void <nom>(String)` metodini chaqiradi (masalan, ExoPlayer ko'prigi: `exoOpen`, `exoCmd`).
/// EN: Calls any `void <name>(String)` method of the Activity (for example the ExoPlayer bridge: `exoOpen`, `exoCmd`).
/// RU: Вызывает любой метод Activity вида `void <имя>(String)` (например, мост ExoPlayer: `exoOpen`, `exoCmd`).
pub fn activity_call(name: &str, arg: &str) {
    let mut n = name.as_bytes().to_vec();
    n.push(0);
    let a = ACTIVITY.load(std::sync::atomic::Ordering::SeqCst);
    if a != 0 {
        unsafe { jni::call_str(a as *mut ANativeActivity, &n, arg) };
    }
}

/// UZ: Activity'ning `String <nom>(String)` metodini chaqiradi va natija matnini qaytaradi.
/// EN: Calls the Activity's `String <name>(String)` method and returns the result text.
/// RU: Вызывает метод Activity вида `String <имя>(String)` и возвращает текст результата.
pub fn activity_call_ret(name: &str, arg: &str) -> String {
    let mut n = name.as_bytes().to_vec();
    n.push(0);
    let a = ACTIVITY.load(std::sync::atomic::Ordering::SeqCst);
    if a == 0 {
        return String::new();
    }
    unsafe { jni::call_str_ret(a as *mut ANativeActivity, &n, arg) }
}

/// UZ: Havolani brauzerda ochadi.
/// EN: Opens a link in the browser.
/// RU: Открывает ссылку в браузере.
pub fn open_url(url: &str) {
    call_activity(b"openUrl\0", url);
}

/// UZ: Matnni almashish buferiga (clipboard) nusxalaydi.
/// EN: Copies text to the clipboard.
/// RU: Копирует текст в буфер обмена.
pub fn copy_text(t: &str) {
    call_activity(b"copyText\0", t);
}

/// UZ: Qisqa xabar (Toast) ko'rsatadi.
/// EN: Shows a short message (Toast).
/// RU: Показывает короткое сообщение (Toast).
pub fn toast(t: &str) {
    call_activity(b"toast\0", t);
}

/// UZ: Fayl tanlagichni ochadi (`mime`: `image/*`, `*/*` ...). Natija `take_picked` bilan olinadi.
/// EN: Opens the file picker (`mime`: `image/*`, `*/*` ...). The result is read with `take_picked`.
/// RU: Открывает выбор файла (`mime`: `image/*`, `*/*` ...). Результат получается через `take_picked`.
pub fn pick_file(mime: &str) {
    call_activity(b"pickFile\0", mime);
}

/// UZ: Tanlangan fayllar yo'llari (bir marta o'qiladi: olingach ro'yxat bo'shaydi).
/// EN: Paths of the picked files (read once: the list is emptied after taking).
/// RU: Пути выбранных файлов (читаются один раз: после получения список очищается).
pub fn take_picked() -> Vec<String> {
    PICKED.lock().map(|mut v| std::mem::take(&mut *v)).unwrap_or_default()
}

struct Shared {
    window: *mut c_void,
    queue: *mut c_void,
    thread_window: *mut c_void,
    redraw: bool,
    quit: bool,
    /// UZ: Tizim panellari bo'lmagan maydon (oyna koordinatalarida).
    /// EN: The area not covered by system bars (in window coordinates).
    /// RU: Область, не занятая системными панелями (в координатах окна).
    content_rect: Option<[i32; 4]>,
    ime: Vec<ImeEvent>,
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
    // UZ: Oyna qaytgandan keyin yaroqsiz bo'ladi: chizish oqimi uni qo'yib yuborguncha kutamiz.
    // EN: The window becomes invalid after it is returned: wait until the render thread lets go of it.
    // RU: После возврата окно становится недействительным: ждём, пока поток рисования его отпустит.
    if let Some(s) = shared() {
        let mut g = s.m.lock().unwrap();
        g.window = null_mut();
        wake(&g);
        while !g.thread_window.is_null() && !g.quit {
            g = s.cv.wait(g).unwrap();
        }
    }
}

unsafe extern "C" fn content_rect_changed(_: *mut ANativeActivity, r: *const ARect) {
    if r.is_null() {
        return;
    }
    if let Some(s) = shared() {
        let mut g = s.m.lock().unwrap();
        g.content_rect = Some([(*r).left, (*r).top, (*r).right, (*r).bottom]);
        g.redraw = true;
        wake(&g);
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

/// UZ: Makro chaqiradigan kirish nuqtasi.
///
/// # Safety
/// `activity` — Android bergan yaroqli ko'rsatkich.
/// EN: The entry point called by the macro.
///
/// # Safety
/// `activity` must be a valid pointer provided by Android.
/// RU: Точка входа, вызываемая макросом.
///
/// # Safety
/// `activity` — действительный указатель, выданный Android.
pub unsafe fn on_create(activity: *mut ANativeActivity, make_ui: fn(f32) -> Ui) {
    // UZ: Rust panic: logga va `files/crash.txt` ga yoziladi (Java yelimi keyingi ishga tushganda ko'rsatadi).
    // EN: A Rust panic is written to the log and to `files/crash.txt` (the Java glue shows it on the next launch).
    // RU: Паника Rust пишется в лог и в `files/crash.txt` (Java-прослойка покажет её при следующем запуске).
    let dir = {
        let p = (*activity).internal_data_path;
        if p.is_null() {
            String::new()
        } else {
            std::ffi::CStr::from_ptr(p as *const std::ffi::c_char)
                .to_string_lossy()
                .into_owned()
        }
    };
    let _ = FILES_DIR.set(dir.clone());
    ACTIVITY.store(activity as usize, std::sync::atomic::Ordering::SeqCst);
    uzcoders_gpu::set_video_hooks(video_create, video_update);
    std::panic::set_hook(Box::new(move |i| {
        let msg = format!("Rust panic: {i}");
        log(&msg);
        if !dir.is_empty() {
            let _ = std::fs::write(format!("{dir}/crash.txt"), &msg);
        }
    }));
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
            content_rect: None,
            ime: Vec::new(),
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
    cb.on_content_rect_changed = Some(content_rect_changed);

    let act = activity as usize;
    std::thread::spawn(move || run(s, make_ui, scale, act));
}

fn run(s: Arc<Sync_>, make_ui: fn(f32) -> Ui, scale: f32, activity: usize) {
    let activity = activity as *mut ANativeActivity;
    let t0 = std::time::Instant::now();
    let mut full_h = 0i32;
    let mut last_serial = 0u64;
    let mut last_show_ms = 0u64;
    let mut shown_req = false;
    let mut back_consumed = false;
    // UZ: 1 = ALOOPER_PREPARE_ALLOW_NON_CALLBACKS: kiritish navbati callback'siz ulanadi; bu bayroqsiz ulanish rad etiladi.
    // EN: 1 = ALOOPER_PREPARE_ALLOW_NON_CALLBACKS: the input queue is attached without a callback; attaching without this flag is rejected.
    // RU: 1 = ALOOPER_PREPARE_ALLOW_NON_CALLBACKS: очередь ввода подключается без callback; без этого флага подключение отклоняется.
    let looper = unsafe { ALooper_prepare(1) };
    s.m.lock().unwrap().looper = looper;
    LOOPER.store(looper as usize, std::sync::atomic::Ordering::SeqCst);
    uzcoders_core::set_waker(wake_ui);
    let mut ui = make_ui(scale);
    ui.set_scale(scale);
    let mut canvas: Option<Canvas> = None;
    let mut gpu: Option<Gpu> = None;
    let mut gpu_failed = false;
    let mut window: *mut c_void = null_mut();
    let mut queue: *mut c_void = null_mut();
    let mut dims = (0i32, 0i32);
    let mut force = true;
    let mut rect: Option<[i32; 4]>;
    let mut ime: Vec<ImeEvent>;

    loop {
        ui.set_time(t0.elapsed().as_millis() as u64);
        uzcoders_core::drain_tasks();
        {
            let mut g = s.m.lock().unwrap();
            if g.quit {
                g.thread_window = null_mut();
                s.cv.notify_all();
                break;
            }
            if g.window != window {
                window = g.window;
                // UZ: GPU sirtini yangi oynaga ulaymiz (yoki eski oynadan ajratamiz).
                // EN: Attach the GPU surface to the new window (or detach it from the old one).
                // RU: Привязываем поверхность GPU к новому окну (или отвязываем от старого).
                if let Some(gp) = &mut gpu {
                    if let Err(e) = gp.egl.set_window(window) {
                        log(&format!("EGL: {e}"));
                        gpu = None;
                        gpu_failed = true;
                    }
                }
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
            rect = g.content_rect;
            ime = std::mem::take(&mut g.ime);
        }

        if !window.is_null() {
            let (w, h) = unsafe {
                (
                    ANativeWindow_getWidth(window),
                    ANativeWindow_getHeight(window),
                )
            };
            if gpu.is_none() && !gpu_failed && w > 0 {
                match Gpu::new(window, &ui) {
                    Ok(g) => {
                        log("GPU (OpenGL ES) ishga tushdi");
                        gpu = Some(g);
                    }
                    Err(e) => {
                        log(&format!("GPU yo'q, CPU: {e}"));
                        gpu_failed = true;
                    }
                }
            }
            if (w, h) != dims && w > 0 && h > 0 {
                if w != dims.0 {
                    full_h = 0;
                }
                full_h = full_h.max(h);
                dims = (w, h);
                if gpu.is_none() {
                    unsafe {
                        ANativeWindow_setBuffersGeometry(window, w, h, WINDOW_FORMAT_RGBA_8888)
                    };
                }
                force = true;
            }
            if (force || ui.needs_frame()) && dims.0 > 0 {
                force = false;
                if let Some(gp) = &mut gpu {
                    let cmds = ui.frame(Size::new(dims.0 as f32, dims.1 as f32));
                    let frame = gp.batcher.build(&cmds, dims.0 as u32, dims.1 as u32);
                    unsafe { gp.gl.render(&frame, dims.0, dims.1) };
                    if !gp.egl.swap() {
                        // UZ: Kontekst yo'qoldi: hammasini qaytadan yaratamiz.
                        // EN: The context was lost: recreate everything.
                        // RU: Контекст потерян: создаём всё заново.
                        log("EGL kontekst yo'qoldi, qayta yaratilmoqda");
                        gpu = None;
                        gpu_failed = false;
                        force = true;
                    }
                } else {
                    draw(&mut ui, &mut canvas, window, dims);
                }
            }
        }

        for e in ime.drain(..) {
            match e {
                ImeEvent::Commit(s) => {
                    ui.ime_commit(&s);
                }
                ImeEvent::Composing(s) => {
                    ui.ime_composing(&s);
                }
                ImeEvent::Delete(b, a) => {
                    ui.ime_delete(b, a);
                }
                ImeEvent::Finish => ui.ime_finish(),
                ImeEvent::Enter => {
                    ui.key_enter();
                }
                ImeEvent::Picked(p) => {
                    if let Ok(mut v) = PICKED.lock() {
                        v.push(p);
                    }
                    uzcoders_core::invalidate();
                }
            }
        }
        // UZ: Tizim panellari chekinishlari: status bar (tepada), navigatsiya paneli (pastda; klaviatura ochiq bo'lsa hisobga olinmaydi).
        // EN: System bar insets: the status bar (top) and the navigation bar (bottom; ignored while the keyboard is open).
        // RU: Отступы системных панелей: статус-бар (сверху) и навигационная панель (снизу; не учитывается при открытой клавиатуре).
        let kb_visible = full_h > 0 && dims.1 + full_h / 6 < full_h;
        if let Some(r) = rect {
            let bottom = if kb_visible {
                0
            } else {
                (dims.1 - r[3]).max(0)
            };
            ui.set_insets(EdgeInsets::new(
                r[0].max(0) as f32,
                r[1].max(0) as f32,
                (dims.0 - r[2]).max(0) as f32,
                bottom as f32,
            ));
        }
        // UZ: Klaviatura: maydonga bosilganda (yangi `serial`) ko'rsatiladi; fokus ketsa yashiriladi.
        // EN: Keyboard: shown when a field is tapped (a new `serial`); hidden when focus is lost.
        // RU: Клавиатура: показывается при нажатии на поле (новый `serial`); скрывается, когда фокус пропал.
        let now = t0.elapsed().as_millis() as u64;
        if ui.keyboard_serial() != last_serial {
            last_serial = ui.keyboard_serial();
            // UZ: Har maydonga bosilganda: tur o'zgargan bo'lishi mumkin, shuning uchun qayta so'raymiz (takroriy chaqiruv zararsiz).
            // EN: On every field tap the kind may have changed, so we ask again (a repeated call is harmless).
            // RU: При каждом нажатии на поле вид мог измениться, поэтому запрашиваем снова (повторный вызов безвреден).
            if ui.wants_keyboard() && now.saturating_sub(last_show_ms) > 150 {
                unsafe { jni::show_keyboard(activity, ui.keyboard_kind().android_input_type()) };
                last_show_ms = now;
                shown_req = true;
            }
        }
        if !ui.wants_keyboard() && (kb_visible || shown_req) {
            unsafe { jni::hide_keyboard(activity) };
            shown_req = false;
        }
        if kb_visible {
            shown_req = true;
        }
        let timeout = if window.is_null() {
            -1
        } else if ui.needs_frame() {
            0
        } else {
            ui.next_wakeup_ms().map_or(-1, |m| m.max(1) as i32)
        };
        let ident = unsafe { ALooper_pollOnce(timeout, null_mut(), null_mut(), null_mut()) };
        if ident == INPUT_IDENT && !queue.is_null() {
            ui.set_time(t0.elapsed().as_millis() as u64);
            drain_input(queue, &mut ui, &mut back_consumed);
        }
    }
    if !queue.is_null() {
        unsafe { AInputQueue_detachLooper(queue) };
    }
    unsafe { jni::detach(activity) };
}

fn draw(ui: &mut Ui, canvas: &mut Option<Canvas>, window: *mut c_void, dims: (i32, i32)) {
    let (w, h) = (dims.0 as usize, dims.1 as usize);
    let cmds = ui.frame(Size::new(w as f32, h as f32));
    let c = canvas.get_or_insert_with(|| ui.canvas(w, h));
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

fn drain_input(queue: *mut c_void, ui: &mut Ui, back_consumed: &mut bool) {
    unsafe {
        let mut ev: *mut c_void = null_mut();
        while AInputQueue_getEvent(queue, &mut ev) >= 0 {
            if AInputQueue_preDispatchEvent(queue, ev) != 0 {
                continue;
            }
            let mut handled = 0;
            if AInputEvent_getType(ev) == 1 {
                // UZ: AINPUT_EVENT_TYPE_KEY (klaviatura tugmasi hodisasi).
                // EN: AINPUT_EVENT_TYPE_KEY (a key event).
                // RU: AINPUT_EVENT_TYPE_KEY (событие клавиши).
                let (action, code, meta) = (
                    AKeyEvent_getAction(ev),
                    AKeyEvent_getKeyCode(ev),
                    AKeyEvent_getMetaState(ev),
                );
                if code == 4 {
                    // UZ: KEYCODE_BACK: bosilganda hal qilingan bo'lsa, qo'yib yuborilganda ham yutiladi (tizimga o'tmaydi).
                    // EN: KEYCODE_BACK: if it was handled on press, it is also swallowed on release (not passed to the system).
                    // RU: KEYCODE_BACK: если нажатие обработано, отпускание тоже поглощается (не передаётся системе).
                    if action == 0 {
                        *back_consumed = ui.key_back();
                    }
                    handled = *back_consumed as i32;
                } else if action == 0 {
                    let done = match code {
                        67 => ui.key_backspace(),
                        66 | 160 => ui.key_enter(),
                        _ => crate::keycode_to_char(code, meta).is_some_and(|c| ui.key_char(c)),
                    };
                    handled = done as i32;
                }
            } else if AInputEvent_getType(ev) == 2 {
                // UZ: AINPUT_EVENT_TYPE_MOTION (teginish hodisasi); faqat birinchi barmoq hisobga olinadi.
                // EN: AINPUT_EVENT_TYPE_MOTION (a touch event); only the first finger is used.
                // RU: AINPUT_EVENT_TYPE_MOTION (событие касания); учитывается только первый палец.
                let kind = match AMotionEvent_getAction(ev) & 0xff {
                    0 => Some(PointerKind::Down),
                    1 => Some(PointerKind::Up),
                    2 => Some(PointerKind::Move),
                    3 => Some(PointerKind::Cancel),
                    _ => None,
                };
                if let Some(kind) = kind {
                    // UZ: Paketlangan tarixiy namunalar ham beriladi: shunda tezlik aniq hisoblanadi.
                    // EN: Batched historical samples are passed too, so velocity is computed accurately.
                    // RU: Передаются и пакетированные исторические отсчёты, поэтому скорость считается точно.
                    if kind == PointerKind::Move {
                        for h in 0..AMotionEvent_getHistorySize(ev) {
                            ui.pointer(PointerEvent::at(
                                PointerKind::Move,
                                AMotionEvent_getHistoricalX(ev, 0, h),
                                AMotionEvent_getHistoricalY(ev, 0, h),
                                (AMotionEvent_getHistoricalEventTime(ev, h) / 1_000_000) as u64,
                            ));
                        }
                    }
                    let (x, y) = (AMotionEvent_getX(ev, 0), AMotionEvent_getY(ev, 0));
                    let t = (AMotionEvent_getEventTime(ev) / 1_000_000) as u64;
                    ui.pointer(PointerEvent::at(kind, x, y, t));
                    handled = 1;
                }
            }
            AInputQueue_finishEvent(queue, ev, handled);
        }
    }
}
