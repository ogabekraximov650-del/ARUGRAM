//! WebP dekoderi (`image-webp`, sof Rust) va umumiy rasm ochuvchi: JPEG/PNG — uzcoders, WebP — shu yerda.

use uzcoders::prelude::{decode_image, Image};

fn is_webp(b: &[u8]) -> bool {
    b.len() > 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP"
}

/// WebP (statik yoki animatsiyaning birinchi kadri) -> RGBA.
pub fn decode_webp(bytes: &[u8]) -> Option<Image> {
    let mut d = image_webp::WebPDecoder::new(std::io::Cursor::new(bytes)).ok()?;
    let (w, h) = d.dimensions();
    if w == 0 || h == 0 || w as u64 * h as u64 > 64_000_000 {
        return None;
    }
    let alpha = d.has_alpha();
    let size = d.output_buffer_size()?;
    let mut buf = vec![0u8; size];
    d.read_image(&mut buf).ok()?;
    let rgba = if alpha {
        buf
    } else {
        let mut o = Vec::with_capacity(w as usize * h as usize * 4);
        for p in buf.chunks_exact(3) {
            o.extend_from_slice(&[p[0], p[1], p[2], 255]);
        }
        o
    };
    if rgba.len() != w as usize * h as usize * 4 {
        return None;
    }
    Some(Image { w, h, rgba })
}

/// JPEG, PNG yoki WebP.
pub fn decode_any(bytes: &[u8]) -> Option<Image> {
    if is_webp(bytes) {
        decode_webp(bytes)
    } else {
        decode_image(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webp_sarlavhasi_taniladi() {
        assert!(is_webp(b"RIFF\x00\x00\x00\x00WEBPVP8 "));
        assert!(!is_webp(b"\x89PNG\r\n\x1a\n\0\0\0\0\0\0"));
        assert!(decode_webp(b"RIFF\x00\x00\x00\x00WEBPVP8 ").is_none());
    }
}
