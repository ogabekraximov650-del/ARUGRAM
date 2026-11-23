//! Java `VdDataSource` (ExoPlayer manbasi) uchun JNI: pleyer baytlarni mahalliy serversiz, to'g'ridan-to'g'ri shu yerdan oladi
//! (Kotlin `EncCacheDataSource` kabi). Qaytish kodlari: o'qilgan bayt; 0 — fayl oxiri; -1 — xato; -2 — qayta urinish.

use jni::objects::{JByteArray, JClass, JString};
use jni::sys::{jint, jlong};
use jni::JNIEnv;

const RETRY: jint = -2;

/// Fayl hajmi; 0 — ma'lumot yo'q (Java IOException tashlaydi).
#[no_mangle]
pub extern "system" fn Java_uz_arumediatv_rs_VdDataSource_nativeSize<'l>(mut env: JNIEnv<'l>, _c: JClass<'l>, name: JString<'l>) -> jlong {
    let Ok(name) = env.get_string(&name).map(String::from) else { return 0 };
    crate::vdsrv::size_of(&name).map(|s| s as jlong).unwrap_or(0)
}

#[no_mangle]
pub extern "system" fn Java_uz_arumediatv_rs_VdDataSource_nativeRead<'l>(env: JNIEnv<'l>, _c: JClass<'l>, name: JString<'l>, pos: jlong, buf: JByteArray<'l>, off: jint, len: jint) -> jint {
    let mut env = env;
    if pos < 0 || off < 0 || len <= 0 {
        return 0;
    }
    let Ok(name) = env.get_string(&name).map(String::from) else { return -1 };
    let mut tmp = vec![0u8; len as usize];
    match crate::vdsrv::read_at(&name, pos as u64, &mut tmp) {
        Ok(0) => 0,
        Ok(n) => {
            let signed: &[i8] = unsafe { std::slice::from_raw_parts(tmp.as_ptr() as *const i8, n) };
            if env.set_byte_array_region(&buf, off, signed).is_err() {
                return -1;
            }
            n as jint
        }
        Err(_) => RETRY,
    }
}
