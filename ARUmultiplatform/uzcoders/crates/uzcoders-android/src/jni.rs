//! Minimal JNI: faqat tizim klaviaturasini ko'rsatish/yashirish uchun (tashqi kutubxonasiz).
//! Funksiya jadvali indekslari Android `jni.h` dan olingan.

use super::ANativeActivity;
use std::ffi::c_void;
use std::ptr::null_mut;

const I_EXCEPTION_CLEAR: usize = 17;
const I_DELETE_LOCAL_REF: usize = 23;
const I_GET_OBJECT_CLASS: usize = 31;
const I_GET_METHOD_ID: usize = 33;
const I_CALL_VOID_METHOD_A: usize = 63;
const I_NEW_STRING: usize = 163;
const I_CALL_OBJECT_METHOD_A: usize = 36;
const I_EXCEPTION_CHECK: usize = 228;
const VM_ATTACH: usize = 4;
const VM_DETACH: usize = 5;

type Obj = *mut c_void;

struct Env(*mut c_void);

impl Env {
    unsafe fn f(&self, idx: usize) -> usize {
        let table = *(self.0 as *mut *const usize);
        *table.add(idx)
    }

    unsafe fn class_of(&self, o: Obj) -> Obj {
        let f: extern "C" fn(*mut c_void, Obj) -> Obj =
            std::mem::transmute(self.f(I_GET_OBJECT_CLASS));
        f(self.0, o)
    }

    unsafe fn method(&self, cls: Obj, name: &[u8], sig: &[u8]) -> *mut c_void {
        let f: extern "C" fn(*mut c_void, Obj, *const u8, *const u8) -> *mut c_void =
            std::mem::transmute(self.f(I_GET_METHOD_ID));
        f(self.0, cls, name.as_ptr(), sig.as_ptr())
    }

    unsafe fn call_void(&self, o: Obj, m: *mut c_void, args: &[u64]) {
        if m.is_null() {
            return;
        }
        let f: extern "C" fn(*mut c_void, Obj, *mut c_void, *const u64) =
            std::mem::transmute(self.f(I_CALL_VOID_METHOD_A));
        f(self.0, o, m, args.as_ptr())
    }

    unsafe fn delete(&self, o: Obj) {
        if !o.is_null() {
            let f: extern "C" fn(*mut c_void, Obj) =
                std::mem::transmute(self.f(I_DELETE_LOCAL_REF));
            f(self.0, o)
        }
    }

    unsafe fn clear_exception(&self) {
        let chk: extern "C" fn(*mut c_void) -> u8 = std::mem::transmute(self.f(I_EXCEPTION_CHECK));
        if chk(self.0) != 0 {
            let clr: extern "C" fn(*mut c_void) = std::mem::transmute(self.f(I_EXCEPTION_CLEAR));
            clr(self.0);
        }
    }
}

/// Chaqiruvchi oqimni JVM'ga ulaydi (bir marta).
unsafe fn attach(activity: *mut ANativeActivity) -> Option<Env> {
    let vm = (*activity).vm;
    if vm.is_null() {
        return None;
    }
    let table = *(vm as *mut *const usize);
    let f: extern "C" fn(*mut c_void, *mut *mut c_void, *mut c_void) -> i32 =
        std::mem::transmute(*table.add(VM_ATTACH));
    let mut env: *mut c_void = null_mut();
    if f(vm, &mut env, null_mut()) != 0 || env.is_null() {
        return None;
    }
    Some(Env(env))
}

/// Oqim tugashidan oldin JVM'dan ajratadi.
///
/// # Safety
/// `activity` yaroqli bo'lishi kerak.
pub unsafe fn detach(activity: *mut ANativeActivity) {
    let vm = (*activity).vm;
    if vm.is_null() {
        return;
    }
    let table = *(vm as *mut *const usize);
    let f: extern "C" fn(*mut c_void) -> i32 = std::mem::transmute(*table.add(VM_DETACH));
    f(vm);
}

/// `UzActivity.showKeyboard(inputType)` (Java yelimi) ni chaqiradi.
///
/// # Safety
/// `activity` yaroqli bo'lishi kerak; chaqiruvchi oqim JVM'ga ulanishi mumkin bo'lgan oqim.
pub unsafe fn show_keyboard(activity: *mut ANativeActivity, input_type: i32) {
    let Some(env) = attach(activity) else { return };
    let clazz = (*activity).clazz;
    let cls = env.class_of(clazz);
    let m = env.method(cls, b"showKeyboard\0", b"(I)V\0");
    env.call_void(clazz, m, &[input_type as u32 as u64]);
    env.clear_exception();
    env.delete(cls);
}

/// `UzActivity.hideKeyboard()` ni chaqiradi.
///
/// # Safety
/// `show_keyboard` bilan bir xil.
pub unsafe fn hide_keyboard(activity: *mut ANativeActivity) {
    let Some(env) = attach(activity) else { return };
    let clazz = (*activity).clazz;
    let cls = env.class_of(clazz);
    let m = env.method(cls, b"hideKeyboard\0", b"()V\0");
    env.call_void(clazz, m, &[]);
    env.clear_exception();
    env.delete(cls);
}

/// `UzActivity.<name>(String)` — bitta matn argumentli void metodni chaqiradi (UTF-16, emoji ham to'g'ri).
///
/// # Safety
/// `show_keyboard` bilan bir xil.
pub unsafe fn call_str(activity: *mut ANativeActivity, name: &[u8], s: &str) {
    let Some(env) = attach(activity) else { return };
    let clazz = (*activity).clazz;
    let cls = env.class_of(clazz);
    let m = env.method(cls, name, b"(Ljava/lang/String;)V\0");
    let u: Vec<u16> = s.encode_utf16().collect();
    let f: extern "C" fn(*mut c_void, *const u16, i32) -> Obj = std::mem::transmute(env.f(I_NEW_STRING));
    let js = f(env.0, u.as_ptr(), u.len() as i32);
    env.call_void(clazz, m, &[js as usize as u64]);
    env.clear_exception();
    env.delete(js);
    env.delete(cls);
}

/// `UzActivity.<name>(String) -> String`: bitta matn argumentli, matn qaytaradigan metod.
///
/// # Safety
/// `show_keyboard` bilan bir xil.
pub unsafe fn call_str_ret(activity: *mut ANativeActivity, name: &[u8], s: &str) -> String {
    let Some(env) = attach(activity) else { return String::new() };
    let clazz = (*activity).clazz;
    let cls = env.class_of(clazz);
    let m = env.method(cls, name, b"(Ljava/lang/String;)Ljava/lang/String;\0");
    let mut out = String::new();
    if !m.is_null() {
        let u: Vec<u16> = s.encode_utf16().collect();
        let nf: extern "C" fn(*mut c_void, *const u16, i32) -> Obj = std::mem::transmute(env.f(I_NEW_STRING));
        let js = nf(env.0, u.as_ptr(), u.len() as i32);
        let f: extern "C" fn(*mut c_void, Obj, *mut c_void, *const u64) -> Obj = std::mem::transmute(env.f(I_CALL_OBJECT_METHOD_A));
        let o = f(env.0, clazz, m, [js as usize as u64].as_ptr());
        env.clear_exception();
        if !o.is_null() {
            out = jstring(env.0, o);
            env.delete(o);
        }
        env.delete(js);
    }
    env.clear_exception();
    env.delete(cls);
    out
}

#[link(name = "android")]
extern "C" {
    fn ANativeWindow_fromSurface(env: *mut c_void, surface: *mut c_void) -> *mut c_void;
}

/// `UzActivity.newVideoSurface(tex)` — Surface yaratadi va uning `ANativeWindow*` ini qaytaradi (dekoder shunga yozadi).
///
/// # Safety
/// `show_keyboard` bilan bir xil; GL oqimida chaqiriladi.
pub unsafe fn video_surface(activity: *mut ANativeActivity, tex: u32) -> *mut c_void {
    let Some(env) = attach(activity) else { return null_mut() };
    let clazz = (*activity).clazz;
    let cls = env.class_of(clazz);
    let m = env.method(cls, b"newVideoSurface\0", b"(I)Landroid/view/Surface;\0");
    let mut out: Obj = null_mut();
    if !m.is_null() {
        let f: extern "C" fn(*mut c_void, Obj, *mut c_void, *const u64) -> Obj = std::mem::transmute(env.f(I_CALL_OBJECT_METHOD_A));
        out = f(env.0, clazz, m, [tex as u64].as_ptr());
    }
    env.clear_exception();
    let win = if out.is_null() { null_mut() } else { ANativeWindow_fromSurface(env.0, out) };
    env.delete(out);
    env.delete(cls);
    win
}

/// `UzActivity.videoUpdate()` — yangi kadr bo'lsa teksturaga o'tkazadi va 4x4 matritsani qaytaradi.
///
/// # Safety
/// GL oqimida chaqiriladi.
pub unsafe fn video_update(activity: *mut ANativeActivity) -> Option<[f32; 16]> {
    let env = attach(activity)?;
    let clazz = (*activity).clazz;
    let cls = env.class_of(clazz);
    let m = env.method(cls, b"videoUpdate\0", b"()Ljava/lang/String;\0");
    let mut res = None;
    if !m.is_null() {
        let f: extern "C" fn(*mut c_void, Obj, *mut c_void, *const u64) -> Obj = std::mem::transmute(env.f(I_CALL_OBJECT_METHOD_A));
        let o = f(env.0, clazz, m, [].as_ptr());
        env.clear_exception();
        if !o.is_null() {
            let s = jstring(env.0, o);
            env.delete(o);
            let v: Vec<f32> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            if v.len() == 16 {
                let mut a = [0f32; 16];
                a.copy_from_slice(&v);
                res = Some(a);
            }
        }
    }
    env.clear_exception();
    env.delete(cls);
    res
}

/// `UzActivity.<name>(boolean)` — bitta mantiqiy argumentli void metod.
///
/// # Safety
/// `show_keyboard` bilan bir xil.
pub unsafe fn call_bool(activity: *mut ANativeActivity, name: &[u8], on: bool) {
    let Some(env) = attach(activity) else { return };
    let clazz = (*activity).clazz;
    let cls = env.class_of(clazz);
    let m = env.method(cls, name, b"(Z)V\0");
    env.call_void(clazz, m, &[on as u64]);
    env.clear_exception();
    env.delete(cls);
}

/// `UzActivity.videoRelease()`.
///
/// # Safety
/// `show_keyboard` bilan bir xil.
pub unsafe fn video_release(activity: *mut ANativeActivity) {
    let Some(env) = attach(activity) else { return };
    let clazz = (*activity).clazz;
    let cls = env.class_of(clazz);
    let m = env.method(cls, b"videoRelease\0", b"()V\0");
    env.call_void(clazz, m, &[]);
    env.clear_exception();
    env.delete(cls);
}

/// Java `String` ni Rust `String` ga o'giradi (UTF-16 -> UTF-8, emoji ham to'g'ri).
///
/// # Safety
/// `env` va `s` JNI chaqiruvidan kelgan yaroqli ko'rsatkichlar.
pub unsafe fn jstring(env: *mut c_void, s: *mut c_void) -> String {
    if s.is_null() {
        return String::new();
    }
    let env = Env(env);
    let len: extern "C" fn(*mut c_void, Obj) -> i32 = std::mem::transmute(env.f(164));
    let chars: extern "C" fn(*mut c_void, Obj, *mut u8) -> *const u16 =
        std::mem::transmute(env.f(165));
    let release: extern "C" fn(*mut c_void, Obj, *const u16) = std::mem::transmute(env.f(166));
    let n = len(env.0, s).max(0) as usize;
    let p = chars(env.0, s, null_mut());
    if p.is_null() {
        return String::new();
    }
    let out = String::from_utf16_lossy(std::slice::from_raw_parts(p, n));
    release(env.0, s, p);
    out
}
