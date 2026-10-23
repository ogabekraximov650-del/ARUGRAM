//! UZ: Telegram `.tgs` (gzip bilan siqilgan Lottie JSON) ni ochadi; oddiy JSON bo'lsa o'zini qaytaradi.
//! EN: Unpacks Telegram `.tgs` (gzip-compressed Lottie JSON); plain JSON is returned as is.
//! RU: Распаковывает Telegram `.tgs` (Lottie JSON, сжатый gzip); обычный JSON возвращается как есть.

/// UZ: Baytlardan Lottie JSON matnini oladi (`.tgs` yoki oddiy JSON). EN: Gets the Lottie JSON text from bytes (`.tgs` or plain JSON). RU: Получает текст Lottie JSON из байтов (`.tgs` или обычный JSON).
#[must_use]
pub fn to_json(raw: &[u8]) -> Option<String> {
    if raw.len() >= 18 && raw[0] == 0x1f && raw[1] == 0x8b && raw[2] == 8 {
        let flg = raw[3];
        let mut pos = 10usize;
        if flg & 4 != 0 {
            let xlen = usize::from(*raw.get(pos)?) | (usize::from(*raw.get(pos + 1)?) << 8);
            pos = pos.checked_add(2 + xlen)?;
        }
        for bit in [8u8, 16u8] {
            if flg & bit != 0 {
                while *raw.get(pos)? != 0 {
                    pos += 1;
                }
                pos += 1;
            }
        }
        if flg & 2 != 0 {
            pos += 2;
        }
        let body = raw.get(pos..raw.len().checked_sub(8)?)?;
        let out = uzcoders_render::image::inflate(body)?;
        return String::from_utf8(out).ok();
    }
    String::from_utf8(raw.to_vec()).ok()
}

#[cfg(test)]
mod tests {
    use super::to_json;

    #[test]
    fn oddiy_json_ochiq_qaytadi() {
        assert_eq!(to_json(b"{\"v\":1}").as_deref(), Some("{\"v\":1}"));
    }

    #[test]
    fn gzip_ochiladi() {
        // UZ: Python `gzip` natijasi (mtime=0). EN: Output of Python `gzip` (mtime=0). RU: Результат Python `gzip` (mtime=0).
        let gz: [u8; 51] = [
            0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0xff, 0xab, 0x56, 0x2a, 0x53, 0xb2, 0x52, 0x32, 0xd5, 0x33, 0xd5, 0x33, 0x52, 0xd2, 0x51, 0xca, 0x48, 0xcd, 0xc9,
            0xc9, 0x57, 0xb2, 0x8a, 0x36, 0xd4, 0x31, 0xd2, 0x31, 0xd6, 0xc1, 0x20, 0x63, 0x6b, 0x01, 0x35, 0xd7, 0xfd, 0x41, 0x2f, 0x00, 0x00, 0x00,
        ];
        assert_eq!(to_json(&gz).as_deref(), Some("{\"v\":\"5.5.2\",\"hello\":[1,2,3,1,2,3,1,2,3,1,2,3]}"));
    }

    #[test]
    fn buzilgan_gzip_none() {
        assert!(to_json(&[0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9]).is_none());
    }
}
