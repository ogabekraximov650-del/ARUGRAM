//! Eng sodda PNG yozuvchi (siqishsiz "stored" bloklar): testlardagi ekran rasmlari uchun.

fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for n in 0..256u32 {
        let mut c = n;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        table[n as usize] = c;
    }
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c = table[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let mut body = kind.to_vec();
    body.extend_from_slice(data);
    out.extend_from_slice(&body);
    out.extend_from_slice(&crc32(&body).to_be_bytes());
}

/// RGBA (w*h*4) ni RGB PNG baytlariga aylantiradi.
pub fn encode_rgba(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity((w * h * 3 + h) as usize);
    for y in 0..h as usize {
        raw.push(0);
        for x in 0..w as usize {
            let i = (y * w as usize + x) * 4;
            raw.extend_from_slice(&rgba[i..i + 3]);
        }
    }
    let mut z = vec![0x78, 0x01];
    let mut i = 0;
    while i < raw.len() {
        let n = (raw.len() - i).min(65535);
        z.push(if i + n == raw.len() { 1 } else { 0 });
        z.extend_from_slice(&(n as u16).to_le_bytes());
        z.extend_from_slice(&(!(n as u16)).to_le_bytes());
        z.extend_from_slice(&raw[i..i + n]);
        i += n;
    }
    z.extend_from_slice(&adler32(&raw).to_be_bytes());
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_va_adler_ma_lum_qiymatlar() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    }

    #[test]
    fn png_tuzilishi_va_piksellar() {
        let (w, h) = (3u32, 2u32);
        let rgba: Vec<u8> = (0..(w * h * 4) as usize).map(|i| (i * 7 % 256) as u8).collect();
        let p = encode_rgba(w, h, &rgba);
        assert_eq!(&p[0..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        // Chunklarni yurib CRC tekshiramiz va IDAT ni "stored" sifatida ochamiz
        let mut o = 8;
        let mut idat = Vec::new();
        let mut kinds = Vec::new();
        while o < p.len() {
            let n = u32::from_be_bytes(p[o..o + 4].try_into().unwrap()) as usize;
            let body = &p[o + 4..o + 8 + n];
            let crc = u32::from_be_bytes(p[o + 8 + n..o + 12 + n].try_into().unwrap());
            assert_eq!(crc32(body), crc);
            kinds.push(body[0..4].to_vec());
            if &body[0..4] == b"IDAT" {
                idat.extend_from_slice(&body[4..]);
            }
            o += 12 + n;
        }
        assert_eq!(kinds, vec![b"IHDR".to_vec(), b"IDAT".to_vec(), b"IEND".to_vec()]);
        // zlib: 2 bayt sarlavha, keyin stored bloklar
        let mut raw = Vec::new();
        let mut i = 2;
        loop {
            let last = idat[i] & 1 == 1;
            let n = u16::from_le_bytes([idat[i + 1], idat[i + 2]]) as usize;
            raw.extend_from_slice(&idat[i + 5..i + 5 + n]);
            i += 5 + n;
            if last {
                break;
            }
        }
        assert_eq!(adler32(&raw).to_be_bytes(), idat[i..i + 4]);
        assert_eq!(raw.len(), (h * (1 + w * 3)) as usize);
        assert_eq!(&raw[1..4], &rgba[0..3]);
        assert_eq!(&raw[1 + 3 * 3 + 1..1 + 3 * 3 + 1 + 3], &rgba[(w * 4) as usize..(w * 4) as usize + 3]);
    }
}
