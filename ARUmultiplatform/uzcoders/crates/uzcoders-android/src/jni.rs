//! Minimal JNI: faqat tizim klaviaturasini ko'rsatish/yashirish uchun (tashqi kutubxonasiz).
//! Funksiya jadvali indekslari Android `jni.h` dan olingan.

use super::ANativeActivity;
use std::ffi::c_void;
use std::ptr::null_mut;

const I_EXCEPTION_CLEAR: usize = 17;
const I_DELETE_LOCAL_REF: usize = 23;
const I_GET_OBJECT_CLASS: usize = 31;
const I_GET_METHOD_ID: usize = 33;
const I_CALL_OBJECT_METHOD_A: usize = 36;
const I_CALL_VOID_METHOD_A: usize = 63;
const I_NEW_STRING_UTF: usize = 167;
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

    unsafe fn call_obj(&self, o: Obj, m: *mut c_void, args: &[u64]) -> Obj {
        if m.is_null() {
            return null_mut();
        }
        let f: extern "C" fn(*mut c_void, Obj, *mut c_void, *const u64) -> Obj =
            std::mem::transmute(self.f(I_CALL_OBJECT_METHOD_A));
        f(self.0, o, m, args.as_ptr())
    }

    unsafe fn call_void(&self, o: Obj, m: *mut c_void, args: &[u64]) {
        if m.is_null() {
            return;
        }
        let f: extern "C" fn(*mut c_void, Obj, *mut c_void, *const u64) =
            std::mem::transmute(self.f(I_CALL_VOID_METHOD_A));
        f(self.0, o, m, args.as_ptr())
    }

    unsafe fn string(&self, s: &[u8]) -> Obj {
        let f: extern "C" fn(*mut c_void, *const u8) -> Obj =
            std::mem::transmute(self.f(I_NEW_STRING_UTF));
        f(self.0, s.as_ptr())
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

unsafe fn imm(env: &Env, activity: *mut ANativeActivity) -> Obj {
    let clazz = (*activity).clazz;
    let cls = env.class_of(clazz);
    let m = env.method(
        cls,
        b"getSystemService\0",
        b"(Ljava/lang/String;)Ljava/lang/Object;\0",
    );
    let name = env.string(b"input_method\0");
    let imm = env.call_obj(clazz, m, &[name as usize as u64]);
    env.delete(name);
    env.delete(cls);
    env.clear_exception();
    imm
}

/// Tizim klaviaturasini ko'rsatadi (`toggleSoftInput(SHOW_FORCED, 0)`).
///
/// # Safety
/// `activity` yaroqli bo'lishi kerak; chaqiruvchi oqim JVM'ga ulanishi mumkin bo'lgan oqim.
pub unsafe fn show_keyboard(activity: *mut ANativeActivity) {
    let Some(env) = attach(activity) else { return };
    let imm = imm(&env, activity);
    if imm.is_null() {
        return;
    }
    let cls = env.class_of(imm);
    let m = env.method(cls, b"toggleSoftInput\0", b"(II)V\0");
    env.call_void(imm, m, &[2, 0]);
    env.clear_exception();
    env.delete(cls);
    env.delete(imm);
}

/// Klaviaturani yashiradi (`hideSoftInputFromWindow(decorView.getWindowToken(), 0)`).
///
/// # Safety
/// `show_keyboard` bilan bir xil.
pub unsafe fn hide_keyboard(activity: *mut ANativeActivity) {
    let Some(env) = attach(activity) else { return };
    let imm = imm(&env, activity);
    if imm.is_null() {
        return;
    }
    let clazz = (*activity).clazz;
    let act_cls = env.class_of(clazz);
    let get_window = env.method(act_cls, b"getWindow\0", b"()Landroid/view/Window;\0");
    let window = env.call_obj(clazz, get_window, &[]);
    let win_cls = env.class_of(window);
    let get_decor = env.method(win_cls, b"getDecorView\0", b"()Landroid/view/View;\0");
    let decor = env.call_obj(window, get_decor, &[]);
    let view_cls = env.class_of(decor);
    let get_token = env.method(view_cls, b"getWindowToken\0", b"()Landroid/os/IBinder;\0");
    let token = env.call_obj(decor, get_token, &[]);
    let imm_cls = env.class_of(imm);
    let hide = env.method(
        imm_cls,
        b"hideSoftInputFromWindow\0",
        b"(Landroid/os/IBinder;I)Z\0",
    );
    // Boolean natija kerak emas: Object chaqiruvi bilan bir xil ABI (ro'yxatdan qiymat qaytadi).
    env.call_obj(imm, hide, &[token as usize as u64, 0]);
    env.clear_exception();
    for o in [
        token, view_cls, decor, win_cls, window, act_cls, imm_cls, imm,
    ] {
        env.delete(o);
    }
}
