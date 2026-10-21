//! UZ: uzcoders-android — Android `NativeActivity` qobig'i (tashqi kutubxonasiz; faqat tizim `libandroid` / `liblog`).
//!
//! Ilova `cdylib` bo'lib, `android_app!(boshlash)` makrosini chaqiradi; `boshlash` — `fn(zichlik: f32) -> Ui`.
//! Qobiq oyna, teginishlar va kadr tsiklini o'zi boshqaradi: holat o'zgarsa qayta chizadi, bo'lmasa uxlaydi.
//! EN: uzcoders-android — the Android `NativeActivity` shell (no external libraries; only the system `libandroid` / `liblog`).
//!
//! The app is a `cdylib` that invokes the `android_app!(start)` macro; `start` is `fn(density: f32) -> Ui`.
//! The shell manages the window, touches and the frame loop itself: it redraws when state changes and sleeps otherwise.
//! RU: uzcoders-android — оболочка Android `NativeActivity` (без внешних библиотек; только системные `libandroid` / `liblog`).
//!
//! Приложение — это `cdylib`, вызывающий макрос `android_app!(start)`; `start` — `fn(density: f32) -> Ui`.
//! Оболочка сама управляет окном, касаниями и циклом кадров: перерисовывает при изменении состояния, иначе спит.

#[cfg(target_os = "android")]
pub(crate) mod egl;
#[cfg(target_os = "android")]
pub mod glue;
#[cfg(target_os = "android")]
pub(crate) mod jni;

#[cfg(target_os = "android")]
pub use glue::{activity_call, activity_call_ret, video_stats, frame_to_file, keep_screen_on, set_fullscreen, video_release, video_window, copy_text, files_dir, ime_event, jstring, on_create, open_url, pick_file, take_picked, toast, ANativeActivity, ImeEvent};

/// UZ: Ilova kirish nuqtasini e'lon qiladi: `uzcoders_android::android_app!(my_app);`
/// bu yerda `fn my_app(scale: f32) -> uzcoders_ui::Ui`.
/// EN: Declares the app entry point: `uzcoders_android::android_app!(my_app);`
/// where `fn my_app(scale: f32) -> uzcoders_ui::Ui`.
/// RU: Объявляет точку входа приложения: `uzcoders_android::android_app!(my_app);`
/// где `fn my_app(scale: f32) -> uzcoders_ui::Ui`.
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

        // UZ: Java yelimi (`uz.uzcoders.UzActivity`) chaqiradigan JNI funksiyalari.
        // EN: JNI functions called by the Java glue (`uz.uzcoders.UzActivity`).
        // RU: JNI-функции, которые вызывает Java-прослойка (`uz.uzcoders.UzActivity`).
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

/// UZ: Piksel buferini (RGBA8, `w*h*4`) qabul qiluvchi manzil buferga qatorma-qator nusxalaydi
/// (`stride` — manzil qator uzunligi, piksellarda). Platformadan mustaqil, shuning uchun testlanadi.
/// EN: Copies a pixel buffer (RGBA8, `w*h*4`) row by row into the destination buffer
/// (`stride` is the destination row length in pixels). Platform-independent, so it is unit-tested.
/// RU: Построчно копирует буфер пикселей (RGBA8, `w*h*4`) в буфер назначения
/// (`stride` — длина строки назначения в пикселях). Не зависит от платформы, поэтому покрыт тестами.
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

/// UZ: Android `KEYCODE_*` va `META_*` qiymatidan ASCII belgi (Latin harflar, raqamlar va keng tarqalgan belgilar) hosil qiladi.
/// Boshqa tugmalar (strelkalar, funksiya tugmalari) uchun `None` qaytadi.
/// EN: Produces an ASCII character from Android `KEYCODE_*` and `META_*` values (Latin letters, digits and common symbols).
/// Returns `None` for other keys (arrows, function keys).
/// RU: Получает ASCII-символ из значений Android `KEYCODE_*` и `META_*` (латинские буквы, цифры и распространённые знаки).
/// Для остальных клавиш (стрелки, функциональные) возвращает `None`.
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
        // UZ: yuqoriga strelka
        // EN: Up arrow key.
        // RU: Стрелка вверх.
        assert_eq!(keycode_to_char(19, 0), None);
        // UZ: orqaga (Back tugmasi)
        // EN: Back key.
        // RU: Кнопка «Назад».
        assert_eq!(keycode_to_char(4, 0), None);
    }

    #[test]
    fn qatorlar_stride_bilan() {
        // UZ: 2x2 piksel (16 bayt)
        // EN: 2x2 pixels (16 bytes).
        // RU: 2x2 пикселя (16 байт).
        let src: Vec<u8> = (0..16).collect();
        // UZ: stride = 3 piksel (manzil qatori kengroq)
        // EN: stride = 3 pixels (the destination row is wider).
        // RU: stride = 3 пикселя (строка назначения шире).
        let mut dst = vec![0u8; 3 * 2 * 4];
        blit_rows(&src, 2, 2, &mut dst, 3);
        assert_eq!(&dst[0..8], &src[0..8]);
        assert_eq!(&dst[8..12], &[0; 4]);
        assert_eq!(&dst[12..20], &src[8..16]);
    }
}

/// UZ: Android bo'lmagan muhitda (testlar): vaqtinchalik papka.
/// EN: Outside Android (tests): a temporary directory.
/// RU: Вне Android (тесты): временная папка.
#[cfg(not(target_os = "android"))]
pub fn files_dir() -> String {
    std::env::temp_dir().to_string_lossy().into_owned()
}

// UZ: Android bo'lmagan muhitda (testlar): platforma xizmatlari bo'sh (hech narsa qilmaydi).
// EN: Outside Android (tests): platform services are empty stubs (they do nothing).
// RU: Вне Android (тесты): платформенные службы — пустые заглушки (ничего не делают).
#[cfg(not(target_os = "android"))]
pub fn open_url(_url: &str) {}
#[cfg(not(target_os = "android"))]
pub fn copy_text(_t: &str) {}
#[cfg(not(target_os = "android"))]
pub fn toast(_t: &str) {}

/// UZ: Activity metodini chaqirish (faqat Android; bu yerda bo'sh).
/// EN: Calling an Activity method (Android only; an empty stub here).
/// RU: Вызов метода Activity (только Android; здесь пустая заглушка).
#[cfg(not(target_os = "android"))]
pub fn activity_call(_name: &str, _arg: &str) {}

#[cfg(not(target_os = "android"))]
pub fn activity_call_ret(_name: &str, _arg: &str) -> String {
    String::new()
}

/// UZ: Video diagnostikasi (faqat Android; bu yerda bo'sh).
/// EN: Video diagnostics (Android only; an empty stub here).
/// RU: Диагностика видео (только Android; здесь пустая заглушка).
#[cfg(not(target_os = "android"))]
pub fn video_stats() -> (u32, bool, u64) {
    (0, false, 0)
}

/// UZ: Videodan bitta kadr (faqat Android; bu yerda bo'sh).
/// EN: Extracting one frame from a video (Android only; an empty stub here).
/// RU: Извлечение одного кадра из видео (только Android; здесь пустая заглушка).
#[cfg(not(target_os = "android"))]
pub fn frame_to_file(_arg: &str) {}
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
