//! QR kod rasmi (oq fonda qora modullar): Telegram'da "Qurilmani ulash" uchun.

use qrcodegen::{QrCode, QrCodeEcc};
use uzcoders::prelude::Image;

/// `text` ni QR rasmga aylantiradi (har modul 8 piksel, atrofida 2 modullik oq chegara).
pub fn qr_image(text: &str) -> Option<Image> {
    let qr = QrCode::encode_text(text, QrCodeEcc::Medium).ok()?;
    let (n, border, scale) = (qr.size(), 2i32, 8i32);
    let side = ((n + 2 * border) * scale) as usize;
    let mut rgba = vec![255u8; side * side * 4];
    for y in 0..n {
        for x in 0..n {
            if !qr.get_module(x, y) {
                continue;
            }
            for dy in 0..scale {
                for dx in 0..scale {
                    let (px, py) = (((x + border) * scale + dx) as usize, ((y + border) * scale + dy) as usize);
                    let i = (py * side + px) * 4;
                    rgba[i..i + 3].copy_from_slice(&[0, 0, 0]);
                }
            }
        }
    }
    Some(Image { w: side as u32, h: side as u32, rgba })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qr_rasm_kvadrat_va_qora_modulli() {
        let img = qr_image("tg://login?token=abc").unwrap();
        assert_eq!(img.w, img.h);
        assert_eq!(img.rgba.len(), (img.w * img.h * 4) as usize);
        assert!(img.rgba.chunks(4).any(|p| p[0] == 0));
        assert_eq!(&img.rgba[0..4], &[255, 255, 255, 255]);
    }
}
