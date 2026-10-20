//! uzcoders-android — Android `NativeActivity` qobig'i (tashqi kutubxonasiz, faqat tizim `libandroid`/`liblog`).
//!
//! Ilova `cdylib` bo'lib, `android_app!(boshlash)` makrosini chaqiradi; `boshlash` — `fn(zichlik: f32) -> Ui`.
//! Qobiq oynani, teginishlarni va kadr tsiklini o'zi boshqaradi: holat o'zgarsa qayta chizadi, bo'lmasa uxlaydi.

#[cfg(target_os = "android")]
pub mod glue;

#[cfg(target_os = "android")]
pub use glue::{on_create, ANativeActivity};

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

#[cfg(test)]
mod tests {
    use super::*;

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
