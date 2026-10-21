//! uzcoders-android — Android `NativeActivity` qobig'i (tashqi kutubxonasiz, faqat tizim `libandroid`/`liblog`).
//!
//! Ilova `cdylib` bo'lib, `android_app!(boshlash)` makrosini chaqiradi; `boshlash` — `fn(zichlik: f32) -> Ui`.
//! Qobiq oynani, teginishlarni va kadr tsiklini o'zi boshqaradi: holat o'zgarsa qayta chizadi, bo'lmasa uxlaydi.

#[cfg(target_os = "android")]
pub(crate) mod egl;
#[cfg(target_os = "android")]
pub mod glue;
#[cfg(target_os = "android")]
pub(crate) mod jni;

#[cfg(target_os = "android")]
pub use glue::{keep_screen_on, set_fullscreen, video_release, video_window, copy_text, files_dir, ime_event, jstring, on_create, open_url, pick_file, take_picked, toast, ANativeActivity, ImeEvent};

/// Ilova kirish nuqtasini e'lon qiladi: `uzcoders_android::android_app!(my_app);`
/// bu yerda `fn my_app(scale: f32) -> uzcoders_ui::Ui`.
#[macro_export]
macro_rules! android_app {
    ($f:path) => {
        #[no_mangle]
        pub unsafe extern "C" fn ANativeActivity_onCreate(
            activity: *mut $crate::ANativeActivity,
            _saved_state: *mut u8,
            _saved_state_size: usize,
        ) {
            $crate::on_create(activity, $f);
        }

        // Java yelimi (`uz.uzcoders.UzActivity`) chaqiradigan JNI funksiyalari.
        #[no_mangle]
        pub unsafe extern "system" fn Java_uz_uzcoders_UzActivity_nativeCommit(
            env: *mut ::std::ffi::c_void,
            _cls: *mut ::std::ffi::c_void,
            s: *mut ::std::ffi::c_void,
        ) {
            $crate::ime_event($crate::ImeEvent::Commit($crate::jstring(env, s)));
        }

        #[no_mangle]
        pub unsafe extern "system" fn Java_uz_uzcoders_UzActivity_nativeComposing(
            env: *mut ::std::ffi::c_void,
            _cls: *mut ::std::ffi::c_void,
            s: *mut ::std::ffi::c_void,
        ) {
            $crate::ime_event($crate::ImeEvent::Composing($crate::jstring(env, s)));
        }

        #[no_mangle]
        pub unsafe extern "system" fn Java_uz_uzcoders_UzActivity_nativeDelete(
            _env: *mut ::std::ffi::c_void,
            _cls: *mut ::std::ffi::c_void,
            before: i32,
            after: i32,
        ) {
            $crate::ime_event($crate::ImeEvent::Delete(
                before.max(0) as usize,
                after.max(0) as usize,
            ));
        }

        #[no_mangle]
        pub unsafe extern "system" fn Java_uz_uzcoders_UzActivity_nativeFinish(
            _env: *mut ::std::ffi::c_void,
            _cls: *mut ::std::ffi::c_void,
        ) {
            $crate::ime_event($crate::ImeEvent::Finish);
        }

        #[no_mangle]
        pub unsafe extern "system" fn Java_uz_uzcoders_UzActivity_nativePicked(
            env: *mut ::std::ffi::c_void,
            _cls: *mut ::std::ffi::c_void,
            s: *mut ::std::ffi::c_void,
        ) {
            $crate::ime_event($crate::ImeEvent::Picked($crate::jstring(env, s)));
        }

        #[no_mangle]
        pub unsafe extern "system" fn Java_uz_uzcoders_UzActivity_nativeEnter(
            _env: *mut ::std::ffi::c_void,
            _cls: *mut ::std::ffi::c_void,
        ) {
            $crate::ime_event($crate::ImeEvent::Enter);
        }
    };
}

/// Piksel buferini (RGBA8, `w*h*4`) qabul qiluvchi manzil buferga qatorma-qator nusxalaydi
/// (`stride` — manzil qator uzunligi piksellarda). Platformadan mustaqil, shuning uchun testlanadi.
pub fn blit_rows(src: &[u8], w: usize, h: usize, dst: &mut [u8], stride: usize) {
    let row = w * 4;
    for y in 0..h {
        let (s, d) = (y * row, y * stride * 4);
        if s + row > src.len() || d + row > dst.len() {
            break;
        }
        dst[d..d + row].copy_from_slice(&src[s..s + row]);
    }
}

/// Android `KEYCODE_*` va `META_*` qiymatidan ASCII belgi (klaviaturada Latin harflar, raqamlar va keng tarqalgan belgilar).
/// Boshqa tugmalar (strelkalar, funksiya tugmalari) uchun `None`.
pub fn keycode_to_char(code: i32, meta: i32) -> Option<char> {
    let shift = meta & 0x1 != 0;
    let c = match code {
        7..=16 => {
            let d = (code - 7) as u8;
            if shift {
                b")!@#$%^&*("[d as usize] as char
            } else {
                (b'0' + d) as char
            }
        }
        29..=54 => {
            let l = (b'a' + (code - 29) as u8) as char;
            if shift {
                l.to_ascii_uppercase()
            } else {
                l
            }
        }
        144..=153 => (b'0' + (code - 144) as u8) as char,
        62 => ' ',
        55 => {
            if shift {
                '<'
            } else {
                ','
            }
        }
        56 => {
            if shift {
                '>'
            } else {
                '.'
            }
        }
        69 => {
            if shift {
                '_'
            } else {
                '-'
            }
        }
        70 => {
            if shift {
                '+'
            } else {
                '='
            }
        }
        71 => {
            if shift {
                '{'
            } else {
                '['
            }
        }
        72 => {
            if shift {
                '}'
            } else {
                ']'
            }
        }
        73 => {
            if shift {
                '|'
            } else {
                '\\'
            }
        }
        74 => {
            if shift {
                ':'
            } else {
                ';'
            }
        }
        75 => {
            if shift {
                '"'
            } else {
                '\''
            }
        }
        76 => {
            if shift {
                '?'
            } else {
                '/'
            }
        }
        77 => '@',
        81 => '+',
        155 => '*',
        157 => '-',
        158 => '.',
        _ => return None,
    };
    Some(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tugma_kodlari() {
        assert_eq!(keycode_to_char(7, 0), Some('0'));
        assert_eq!(keycode_to_char(16, 0), Some('9'));
        assert_eq!(keycode_to_char(8, 1), Some('!'));
        assert_eq!(keycode_to_char(29, 0), Some('a'));
        assert_eq!(keycode_to_char(54, 1), Some('Z'));
        assert_eq!(keycode_to_char(62, 0), Some(' '));
        assert_eq!(keycode_to_char(81, 0), Some('+'));
        assert_eq!(keycode_to_char(152, 0), Some('8'));
        assert_eq!(keycode_to_char(19, 0), None); // yuqoriga strelka
        assert_eq!(keycode_to_char(4, 0), None); // orqaga
    }

    #[test]
    fn qatorlar_stride_bilan() {
        let src: Vec<u8> = (0..16).collect(); // 2x2 piksel
        let mut dst = vec![0u8; 3 * 2 * 4]; // stride 3
        blit_rows(&src, 2, 2, &mut dst, 3);
        assert_eq!(&dst[0..8], &src[0..8]);
        assert_eq!(&dst[8..12], &[0; 4]);
        assert_eq!(&dst[12..20], &src[8..16]);
    }
}

/// Android bo'lmagan muhitda (testlar) vaqtinchalik papka.
#[cfg(not(target_os = "android"))]
pub fn files_dir() -> String {
    std::env::temp_dir().to_string_lossy().into_owned()
}

// Android bo'lmagan muhitda (testlar): platforma xizmatlari bo'sh.
#[cfg(not(target_os = "android"))]
pub fn open_url(_url: &str) {}
#[cfg(not(target_os = "android"))]
pub fn copy_text(_t: &str) {}
#[cfg(not(target_os = "android"))]
pub fn toast(_t: &str) {}
#[cfg(not(target_os = "android"))]
pub fn pick_file(_mime: &str) {}
#[cfg(not(target_os = "android"))]
pub fn take_picked() -> Vec<String> {
    Vec::new()
}
#[cfg(not(target_os = "android"))]
pub fn video_window() -> *mut std::ffi::c_void {
    std::ptr::null_mut()
}
#[cfg(not(target_os = "android"))]
pub fn video_release() {}
#[cfg(not(target_os = "android"))]
pub fn set_fullscreen(_on: bool) {}
#[cfg(not(target_os = "android"))]
pub fn keep_screen_on(_on: bool) {}
