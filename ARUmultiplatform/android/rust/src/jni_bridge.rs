//! Kotlin ↔ Rust ko'prigi (JNI).
//!
//! Flutter ilovada Dart `dart:ffi` orqali `rust_cache_*` funksiyalarini
//! chaqiradi (`lib/services/rust_bridge.dart`). Kotlin'da FFI yo'q — shu
//! sabab bu yerda o'sha funksiyalarning JNI o'ramlari: mantiq
//! (`cache.rs` — shifrlash, 5 daqiqalik tozalik) BITTA, ikkala ilova
//! uchun bir xil.
//!
//! Kotlin tomoni: `uz.arugram.soft.rust.RustCore`.

use crate::cache::{rust_cache_clear, rust_cache_get, rust_cache_is_fresh, rust_cache_save};
use crate::ffi_utils::rust_free_string;
use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jstring, JNI_FALSE, JNI_TRUE};
use jni::JNIEnv;
use std::ffi::{CStr, CString};

/// Java satrini C satriga aylantiradi (ichida `\0` bo'lsa — `None`).
fn to_c(env: &mut JNIEnv, s: &JString) -> Option<CString> {
    let rs: String = env.get_string(s).ok()?.into();
    CString::new(rs).ok()
}

#[no_mangle]
pub extern "system" fn Java_uz_arugram_soft_rust_RustCore_nativeVersion<'l>(
    env: JNIEnv<'l>,
    _c: JClass<'l>,
) -> jstring {
    env.new_string(env!("CARGO_PKG_VERSION"))
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// Keshdagi JSON (yoki `null`).
#[no_mangle]
pub extern "system" fn Java_uz_arugram_soft_rust_RustCore_nativeCacheGet<'l>(
    mut env: JNIEnv<'l>,
    _c: JClass<'l>,
    path: JString<'l>,
) -> jstring {
    let Some(p) = to_c(&mut env, &path) else {
        return std::ptr::null_mut();
    };
    let raw = rust_cache_get(p.as_ptr());
    if raw.is_null() {
        return std::ptr::null_mut();
    }
    let text = unsafe { CStr::from_ptr(raw) }.to_string_lossy().into_owned();
    rust_free_string(raw);
    env.new_string(text)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "system" fn Java_uz_arugram_soft_rust_RustCore_nativeCacheIsFresh<'l>(
    mut env: JNIEnv<'l>,
    _c: JClass<'l>,
    path: JString<'l>,
) -> jboolean {
    let Some(p) = to_c(&mut env, &path) else {
        return JNI_FALSE;
    };
    if rust_cache_is_fresh(p.as_ptr()) == 1 { JNI_TRUE } else { JNI_FALSE }
}

#[no_mangle]
pub extern "system" fn Java_uz_arugram_soft_rust_RustCore_nativeCacheSave<'l>(
    mut env: JNIEnv<'l>,
    _c: JClass<'l>,
    path: JString<'l>,
    json: JString<'l>,
) -> jboolean {
    let (Some(p), Some(j)) = (to_c(&mut env, &path), to_c(&mut env, &json)) else {
        return JNI_FALSE;
    };
    if rust_cache_save(p.as_ptr(), j.as_ptr()) == 1 { JNI_TRUE } else { JNI_FALSE }
}

#[no_mangle]
pub extern "system" fn Java_uz_arugram_soft_rust_RustCore_nativeCacheClear<'l>(
    mut env: JNIEnv<'l>,
    _c: JClass<'l>,
    path: JString<'l>,
) {
    if let Some(p) = to_c(&mut env, &path) {
        rust_cache_clear(p.as_ptr());
    }
}
