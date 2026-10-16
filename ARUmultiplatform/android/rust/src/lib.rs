//! ARUMEDIATV Rust yadrosi (Android, JNI).
//!
//! Hozircha: server so'rovlarini imzolash (`X-App-Sig`). Keyingi bosqichlarda Telegram (MTProto), shifrlangan kesh va
//! pleyer manbasi shu yerga qo'shiladi (Flutter loyihasidagi `rust/` dan ko'chiriladi).
//!
//! Imzo qoidasi worker bilan AYNAN bir xil (`worker/src/lib.rs` -> `verify_app_sig`):
//!   `X-App-Sig: v2.<vaqt>.<hex HMAC-SHA256>`, imzolanadigan matn `"<vaqt>.<METOD>.<yo'l>"`, kalit — `APP_SIGN_SECRET`.

use hmac::{Hmac, Mac};
use jni::objects::{JClass, JString};
use jni::sys::jstring;
use jni::JNIEnv;
use sha2::Sha256;

/// Kompilyatsiya paytida kiritilgan kalit (xom ko'rinishda — darhol niqoblanadi va binarga tushmaydi).
const RAW_SECRET: &str = match option_env!("APP_SIGN_SECRET") {
    Some(v) => v,
    None => "",
};

const SECRET_LEN: usize = RAW_SECRET.len();

/// Har bayt uchun boshqacha niqob (takrorlanuvchi naqsh chiqmasin).
const fn mask_at(i: usize) -> u8 {
    0x5Au8 ^ (i as u8).wrapping_mul(31).wrapping_add(7)
}

/// Niqoblangan kalit — binarda AYNAN shu baytlar turadi (`strings` kalitni topa olmaydi).
const OBFUSCATED: [u8; SECRET_LEN] = {
    let src = RAW_SECRET.as_bytes();
    let mut out = [0u8; SECRET_LEN];
    let mut i = 0;
    while i < SECRET_LEN {
        out[i] = src[i] ^ mask_at(i);
        i += 1;
    }
    out
};

fn app_secret() -> Vec<u8> {
    (0..SECRET_LEN).map(|i| OBFUSCATED[i] ^ mask_at(i)).collect()
}

/// `v2.<vaqt>.<hex>` yoki bo'sh satr (kalit yo'q / maydon bo'sh).
pub fn sign_with(secret: &[u8], ts: &str, method: &str, path: &str) -> String {
    if secret.is_empty() || ts.is_empty() || method.is_empty() || path.is_empty() {
        return String::new();
    }
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret) else {
        return String::new();
    };
    mac.update(format!("{ts}.{}.{path}", method.to_uppercase()).as_bytes());
    format!("v2.{ts}.{}", hex::encode(mac.finalize().into_bytes()))
}

/// Yadro kaliti bilan imzo.
pub fn sign(ts: &str, method: &str, path: &str) -> String {
    sign_with(&app_secret(), ts, method, path)
}

fn read(env: &mut JNIEnv, s: &JString) -> String {
    env.get_string(s).map(|v| v.into()).unwrap_or_default()
}

fn to_jstring(env: &mut JNIEnv, s: &str) -> jstring {
    match env.new_string(s) {
        Ok(v) => v.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Kotlin: `AruCore.nativeSign(ts, method, path): String`
#[no_mangle]
pub extern "system" fn Java_uz_arumediatv_com_core_AruCore_nativeSign<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    ts: JString<'l>,
    method: JString<'l>,
    path: JString<'l>,
) -> jstring {
    let (ts, method, path) = (read(&mut env, &ts), read(&mut env, &method), read(&mut env, &path));
    let out = sign(&ts, &method, &path);
    to_jstring(&mut env, &out)
}

/// Kotlin: `AruCore.nativeInfo(): String` — diagnostika (kalit qo'yilganmi).
#[no_mangle]
pub extern "system" fn Java_uz_arumediatv_com_core_AruCore_nativeInfo<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
) -> jstring {
    let info = format!("aru_core {} · imzo kaliti: {}", env!("CARGO_PKG_VERSION"), if SECRET_LEN > 0 { "bor" } else { "YO'Q" });
    to_jstring(&mut env, &info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imzo_worker_bilan_bir_xil() {
        // Python: hmac.new(b"test-kalit", b"1700000000.GET./api/seasons", hashlib.sha256).hexdigest()
        let s = sign_with(b"test-kalit", "1700000000", "get", "/api/seasons");
        assert_eq!(s, "v2.1700000000.f24c01e5268e904378e923f36d9ffb05896645a669da944887f22fdba91d9510");
        assert_eq!(s, sign_with(b"test-kalit", "1700000000", "GET", "/api/seasons"));
    }

    #[test]
    fn bosh_maydon_yoki_kalit_yoq_bosh_qaytadi() {
        assert_eq!(sign_with(b"", "1", "GET", "/a"), "");
        assert_eq!(sign_with(b"k", "", "GET", "/a"), "");
        assert_eq!(sign_with(b"k", "1", "", "/a"), "");
        assert_eq!(sign_with(b"k", "1", "GET", ""), "");
    }
}
