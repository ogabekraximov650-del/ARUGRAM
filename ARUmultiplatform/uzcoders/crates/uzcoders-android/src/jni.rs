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
