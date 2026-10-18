//! JNI ko'prigi: Kotlin (`uz.arumediatv.game.NativeBridge`) <-> dvigatel.

use crate::input::TouchAction;
use crate::renderer::{Frame, Renderer};
use crate::{world, Game};
use jni::objects::{JClass, JObject};
use jni::sys::{jfloat, jfloatArray, jint, jintArray, jlong, jshortArray};
use jni::objects::JFloatArray;
use jni::JNIEnv;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr::NonNull;

struct App {
    game: Game,
    renderer: Renderer,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    _window: ndk::native_window::NativeWindow,
}

fn init_app(env: &mut JNIEnv, surface: &JObject, density: f32) -> Result<App, String> {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("aru_game"),
    );
    let window = unsafe {
        ndk::native_window::NativeWindow::from_surface(env.get_raw() as *mut _, surface.as_raw() as *mut _)
    }
    .ok_or("NativeWindow olinmadi")?;
    let (w, h) = (window.width().max(1) as u32, window.height().max(1) as u32);

    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN | wgpu::Backends::GL,
        ..Default::default()
    });
    let handle = raw_window_handle::AndroidNdkWindowHandle::new(
        NonNull::new(window.ptr().as_ptr() as *mut std::ffi::c_void).ok_or("oyna ko'rsatkichi bo'sh")?,
    );
    let wgpu_surface = unsafe {
        instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: raw_window_handle::RawDisplayHandle::Android(
                raw_window_handle::AndroidDisplayHandle::new(),
            ),
            raw_window_handle: raw_window_handle::RawWindowHandle::AndroidNdk(handle),
        })
    }
    .map_err(|e| format!("surface: {e}"))?;

    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: Some(&wgpu_surface),
        force_fallback_adapter: false,
    }))
    .ok_or("adapter topilmadi")?;
    log::info!("GPU: {:?}", adapter.get_info());

    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("aru"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
            memory_hints: wgpu::MemoryHints::Performance,
        },
        None,
    ))
    .map_err(|e| format!("device: {e}"))?;

    let caps = wgpu_surface.get_capabilities(&adapter);
    let format = caps
        .formats
        .iter()
        .copied()
        .find(|f| !f.is_srgb())
        .or_else(|| caps.formats.first().copied())
        .ok_or("format yo'q")?;
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: w,
        height: h,
        present_mode: wgpu::PresentMode::Fifo,
        desired_maximum_frame_latency: 2,
        alpha_mode: caps.alpha_modes.first().copied().unwrap_or(wgpu::CompositeAlphaMode::Auto),
        view_formats: vec![],
    };
    wgpu_surface.configure(&device, &config);

    let renderer = Renderer::new(device, queue, format, w, h, &world::build().statics);
    Ok(App {
        game: Game::new(w, h, density),
        renderer,
        surface: wgpu_surface,
        config,
        _window: window,
    })
}

fn frame(app: &mut App, dt: f32) {
    app.game.update(dt);
    let tex = match app.surface.get_current_texture() {
        Ok(t) => t,
        Err(_) => {
            app.surface.configure(app.renderer.device(), &app.config);
            return;
        }
    };
    let view = tex.texture.create_view(&wgpu::TextureViewDescriptor::default());
    let hud = app.game.hud();
    let fd = app.game.frame();
    let frame = Frame {
        dynamic: &fd.dynamic,
        dynamic_shadow: fd.dynamic_shadow,
        outline: fd.outline,
        viewmodel: &fd.viewmodel,
        hud: &hud,
        time: app.game.time,
    };
    app.renderer.render(&view, &app.game.camera, &frame);
    tex.present();
}

#[no_mangle]
pub extern "system" fn Java_uz_arumediatv_game_NativeBridge_nativeInit(
    mut env: JNIEnv,
    _c: JClass,
    surface: JObject,
    density: jfloat,
) -> jlong {
    match catch_unwind(AssertUnwindSafe(|| init_app(&mut env, &surface, density))) {
        Ok(Ok(app)) => Box::into_raw(Box::new(app)) as jlong,
        Ok(Err(e)) => {
            log::error!("init xato: {e}");
            0
        }
        Err(_) => {
            log::error!("init panic");
            0
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_uz_arumediatv_game_NativeBridge_nativeResize(
    _env: JNIEnv,
    _c: JClass,
    ptr: jlong,
    w: jint,
    h: jint,
) {
    if ptr == 0 || w <= 0 || h <= 0 {
        return;
    }
    let app = unsafe { &mut *(ptr as *mut App) };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        app.config.width = w as u32;
        app.config.height = h as u32;
        app.surface.configure(app.renderer.device(), &app.config);
        app.renderer.resize(w as u32, h as u32);
        app.game.resize(w as u32, h as u32);
    }));
}

#[no_mangle]
pub extern "system" fn Java_uz_arumediatv_game_NativeBridge_nativeFrame(
    _env: JNIEnv,
    _c: JClass,
    ptr: jlong,
    dt: jfloat,
) {
    if ptr == 0 {
        return;
    }
    let app = unsafe { &mut *(ptr as *mut App) };
    let _ = catch_unwind(AssertUnwindSafe(|| frame(app, dt)));
}

#[no_mangle]
pub extern "system" fn Java_uz_arumediatv_game_NativeBridge_nativeTouch(
    _env: JNIEnv,
    _c: JClass,
    ptr: jlong,
    action: jint,
    id: jint,
    x: jfloat,
    y: jfloat,
) {
    if ptr == 0 {
        return;
    }
    let app = unsafe { &mut *(ptr as *mut App) };
    let a = match action {
        0 => TouchAction::Down,
        1 => TouchAction::Move,
        _ => TouchAction::Up,
    };
    let _ = catch_unwind(AssertUnwindSafe(|| app.game.touch(a, id, x, y)));
}

#[no_mangle]
pub extern "system" fn Java_uz_arumediatv_game_NativeBridge_nativeDestroy(
    _env: JNIEnv,
    _c: JClass,
    ptr: jlong,
) {
    if ptr != 0 {
        let _ = catch_unwind(AssertUnwindSafe(|| unsafe { drop(Box::from_raw(ptr as *mut App)) }));
    }
}

/// Joylashuv o'zgargan bo'lsa saqlash uchun `FloatArray` (7 ta son), bo'lmasa `null`.
#[no_mangle]
pub extern "system" fn Java_uz_arumediatv_game_NativeBridge_nativeTakeLayout(
    env: JNIEnv,
    _c: JClass,
    ptr: jlong,
) -> jfloatArray {
    if ptr == 0 {
        return std::ptr::null_mut();
    }
    let app = unsafe { &mut *(ptr as *mut App) };
    let layout = catch_unwind(AssertUnwindSafe(|| app.game.input.take_layout())).ok().flatten();
    let Some(vals) = layout else { return std::ptr::null_mut() };
    match env.new_float_array(vals.len() as i32) {
        Ok(arr) => {
            if env.set_float_array_region(&arr, 0, &vals).is_err() {
                return std::ptr::null_mut();
            }
            arr.into_raw()
        }
        Err(_) => std::ptr::null_mut(),
    }
}

/// Saqlangan joylashuvni tiklaydi (noto'g'ri qiymat e'tiborsiz qoldiriladi).
#[no_mangle]
pub extern "system" fn Java_uz_arumediatv_game_NativeBridge_nativeSetLayout(
    env: JNIEnv,
    _c: JClass,
    ptr: jlong,
    arr: JFloatArray,
) {
    if ptr == 0 {
        return;
    }
    let app = unsafe { &mut *(ptr as *mut App) };
    let Ok(len) = env.get_array_length(&arr) else { return };
    if !(7..=64).contains(&len) {
        return;
    }
    let mut buf = vec![0.0f32; len as usize];
    if env.get_float_array_region(&arr, 0, &mut buf).is_err() {
        return;
    }
    if let Some(l) = crate::input::Layout::from_slice(&buf) {
        app.game.input.set_layout(l);
    }
}

/// Shu kadrdagi ovoz hodisalari (`audio::EV_*`) yoki `null` (bo'sh).
#[no_mangle]
pub extern "system" fn Java_uz_arumediatv_game_NativeBridge_nativeTakeEvents(
    env: JNIEnv,
    _c: JClass,
    ptr: jlong,
) -> jintArray {
    if ptr == 0 {
        return std::ptr::null_mut();
    }
    let app = unsafe { &mut *(ptr as *mut App) };
    let Ok(ev) = catch_unwind(AssertUnwindSafe(|| app.game.take_events())) else { return std::ptr::null_mut() };
    if ev.is_empty() {
        return std::ptr::null_mut();
    }
    let vals: Vec<i32> = ev.iter().map(|e| *e as i32).collect();
    match env.new_int_array(vals.len() as i32) {
        Ok(arr) => {
            if env.set_int_array_region(&arr, 0, &vals).is_err() {
                return std::ptr::null_mut();
            }
            arr.into_raw()
        }
        Err(_) => std::ptr::null_mut(),
    }
}

/// `id` ovozining PCM (16 bit, mono, `audio::RATE` Hz) ma'lumoti; noto'g'ri `id` — `null`.
#[no_mangle]
pub extern "system" fn Java_uz_arumediatv_game_NativeBridge_nativeGetSound(
    env: JNIEnv,
    _c: JClass,
    id: jint,
) -> jshortArray {
    let pcm = crate::audio::render(id.max(0) as usize);
    if pcm.is_empty() {
        return std::ptr::null_mut();
    }
    match env.new_short_array(pcm.len() as i32) {
        Ok(arr) => {
            if env.set_short_array_region(&arr, 0, &pcm).is_err() {
                return std::ptr::null_mut();
            }
            arr.into_raw()
        }
        Err(_) => std::ptr::null_mut(),
    }
}

/// Chizish o'lchami (adaptiv ruxsat) o'zgarganda: HUD o'lchami uchun effektiv piksel zichligi.
#[no_mangle]
pub extern "system" fn Java_uz_arumediatv_game_NativeBridge_nativeSetDensity(
    _env: JNIEnv,
    _c: JClass,
    ptr: jlong,
    density: jfloat,
) {
    if ptr == 0 || !density.is_finite() {
        return;
    }
    let app = unsafe { &mut *(ptr as *mut App) };
    let _ = catch_unwind(AssertUnwindSafe(|| app.game.set_density(density)));
}
