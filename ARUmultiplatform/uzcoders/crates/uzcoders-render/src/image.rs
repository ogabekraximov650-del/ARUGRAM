//! Rasm: PNG dekoderi (noldan: inflate + filtrlar) va RGBA bitmap.

/// RGBA8 bitmap (alfa to'g'ri, premultiplied emas).
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

impl Image {
    /// Aspektni saqlab `max_w x max_h` ga sig'adigan qilib kichraytiradi (maydon o'rtachasi); kichik bo'lsa o'zgarmaydi.
    pub fn resized(self, max_w: u32, max_h: u32) -> Image {
        let k = (max_w as f32 / self.w as f32).min(max_h as f32 / self.h as f32);
        if k >= 1.0 {
            return self;
        }
        let (nw, nh) = (
            ((self.w as f32 * k) as u32).max(1),
            ((self.h as f32 * k) as u32).max(1),
        );
        let mut out = vec![0u8; (nw * nh * 4) as usize];
        for y in 0..nh {
            let (y0, y1) = (
                (y as f32 / k) as u32,
                (((y + 1) as f32 / k).ceil() as u32)
                    .min(self.h)
                    .max((y as f32 / k) as u32 + 1),
            );
            for x in 0..nw {
                let (x0, x1) = (
                    (x as f32 / k) as u32,
                    (((x + 1) as f32 / k).ceil() as u32)
                        .min(self.w)
                        .max((x as f32 / k) as u32 + 1),
                );
                let mut acc = [0u32; 4];
                let mut n = 0u32;
                for yy in y0..y1.min(self.h) {
                    for xx in x0..x1.min(self.w) {
                        let i = ((yy * self.w + xx) * 4) as usize;
                        for c in 0..4 {
                            acc[c] += self.rgba[i + c] as u32;
                        }
                        n += 1;
                    }
                }
                let o = ((y * nw + x) * 4) as usize;
                for c in 0..4 {
                    out[o + c] = (acc[c] / n.max(1)) as u8;
                }
            }
        }
        Image {
            w: nw,
            h: nh,
            rgba: out,
        }
    }

    /// Burchaklarni `r` piksel radiusda yumaloqlaydi (alfa kanali silliqlanadi).
    pub fn round_corners(mut self, r: f32) -> Image {
        let (w, h) = (self.w as f32, self.h as f32);
        let (hw, hh) = (w * 0.5, h * 0.5);
        let r = r.min(hw).min(hh);
        for y in 0..self.h {
            for x in 0..self.w {
                let (px, py) = (x as f32 + 0.5 - hw, y as f32 + 0.5 - hh);
                let (qx, qy) = (px.abs() - (hw - r), py.abs() - (hh - r));
                let sd =
                    (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - r;
                let k = (0.5 - sd).clamp(0.0, 1.0);
                let i = ((y * self.w + x) * 4 + 3) as usize;
                self.rgba[i] = (self.rgba[i] as f32 * k + 0.5) as u8;
            }
        }
        self
    }
}

// ── inflate (RFC 1951) ──────────────────────────────────────────────

struct Bits<'a> {
    d: &'a [u8],
    pos: usize,
    bit: u32,
    cnt: u32,
}

impl Bits<'_> {
    fn need(&mut self, n: u32) -> Option<u32> {
        while self.cnt < n {
            self.bit |= (*self.d.get(self.pos)? as u32) << self.cnt;
            self.pos += 1;
            self.cnt += 8;
        }
        let v = self.bit & ((1u32 << n) - 1);
        self.bit >>= n;
        self.cnt -= n;
        Some(v)
    }
}

struct Huff {
    count: [u16; 16],
    symbol: Vec<u16>,
}

fn build(lengths: &[u8]) -> Huff {
    let mut count = [0u16; 16];
    for &l in lengths {
        count[l as usize] += 1;
    }
    let mut offs = [0u16; 16];
    for i in 1..15 {
        offs[i + 1] = offs[i] + count[i];
    }
    let mut symbol = vec![0u16; lengths.len()];
    for (s, &l) in lengths.iter().enumerate() {
        if l != 0 {
            symbol[offs[l as usize] as usize] = s as u16;
            offs[l as usize] += 1;
        }
    }
    Huff { count, symbol }
}

fn decode(b: &mut Bits, h: &Huff) -> Option<u16> {
    let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
    for len in 1..16 {
        code |= b.need(1)? as i32;
        let c = h.count[len] as i32;
        if code - c < first {
            return h.symbol.get((index + (code - first)) as usize).copied();
        }
        index += c;
        first = (first + c) << 1;
        code <<= 1;
    }
    None
}

const LBASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LEXT: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DBASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DEXT: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

/// Xom deflate oqimini ochadi (zlib sarlavhasiz).
pub fn inflate(data: &[u8]) -> Option<Vec<u8>> {
    let mut b = Bits {
        d: data,
        pos: 0,
        bit: 0,
        cnt: 0,
    };
    let mut out: Vec<u8> = Vec::new();
    loop {
        let last = b.need(1)?;
        match b.need(2)? {
            0 => {
                b.bit = 0;
                b.cnt = 0;
                let len = u16::from_le_bytes([*data.get(b.pos)?, *data.get(b.pos + 1)?]) as usize;
                b.pos += 4;
                out.extend_from_slice(data.get(b.pos..b.pos + len)?);
                b.pos += len;
            }
            t @ (1 | 2) => {
                let (lit, dist) = if t == 1 {
                    let mut l = [0u8; 288];
                    for (i, v) in l.iter_mut().enumerate() {
                        *v = match i {
                            0..=143 => 8,
                            144..=255 => 9,
                            256..=279 => 7,
                            _ => 8,
                        };
                    }
                    (build(&l), build(&[5u8; 30]))
                } else {
                    let nlen = b.need(5)? as usize + 257;
                    let ndist = b.need(5)? as usize + 1;
                    let ncode = b.need(4)? as usize + 4;
                    const ORDER: [usize; 19] = [
                        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
                    ];
                    let mut cl = [0u8; 19];
                    for &o in ORDER.iter().take(ncode) {
                        cl[o] = b.need(3)? as u8;
                    }
                    let ch = build(&cl);
                    let mut lengths = vec![0u8; nlen + ndist];
                    let mut i = 0;
                    while i < nlen + ndist {
                        let sym = decode(&mut b, &ch)?;
                        if sym < 16 {
                            lengths[i] = sym as u8;
                            i += 1;
                        } else {
                            let (prev, rep) = match sym {
                                16 => (*lengths.get(i.checked_sub(1)?)?, 3 + b.need(2)? as usize),
                                17 => (0, 3 + b.need(3)? as usize),
                                _ => (0, 11 + b.need(7)? as usize),
                            };
                            if i + rep > nlen + ndist {
                                return None;
                            }
                            for _ in 0..rep {
                                lengths[i] = prev;
                                i += 1;
                            }
                        }
                    }
                    (build(&lengths[..nlen]), build(&lengths[nlen..]))
                };
                loop {
                    let sym = decode(&mut b, &lit)? as usize;
                    if sym < 256 {
                        out.push(sym as u8);
                    } else if sym == 256 {
                        break;
                    } else {
                        let s = sym - 257;
                        let len = *LBASE.get(s)? as usize + b.need(*LEXT.get(s)? as u32)? as usize;
                        let ds = decode(&mut b, &dist)? as usize;
                        let d = *DBASE.get(ds)? as usize + b.need(*DEXT.get(ds)? as u32)? as usize;
                        if d > out.len() {
                            return None;
                        }
                        for _ in 0..len {
                            out.push(out[out.len() - d]);
                        }
                    }
                }
            }
            _ => return None,
        }
        if last == 1 {
            return Some(out);
        }
    }
}

/// PNG yoki JPEG ni formatiga qarab ochadi.
pub fn decode_image(d: &[u8]) -> Option<Image> {
    if d.starts_with(&[0x89, b'P', b'N', b'G']) {
        decode_png(d)
    } else if d.starts_with(&[0xFF, 0xD8]) {
        crate::jpeg::decode_jpeg(d)
    } else {
        None
    }
}

// ── PNG ─────────────────────────────────────────────────────────────

fn paeth(a: i32, b: i32, c: i32) -> i32 {
    let p = a + b - c;
    let (pa, pb, pc) = ((p - a).abs(), (p - b).abs(), (p - c).abs());
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

/// PNG ni RGBA8 ga ochadi. Qo'llab-quvvatlanadi: kulrang/RGB/palitra/+alfa, chuqurlik 1–16, interlace'siz.
pub fn decode_png(d: &[u8]) -> Option<Image> {
    if d.get(..8)? != [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A] {
        return None;
    }
    let (mut w, mut h, mut depth, mut ctype, mut interlace) = (0usize, 0usize, 0u8, 0u8, 0u8);
    let mut plte: Vec<u8> = Vec::new();
    let mut trns: Vec<u8> = Vec::new();
    let mut idat: Vec<u8> = Vec::new();
    let mut p = 8;
    while p + 8 <= d.len() {
        let len = u32::from_be_bytes(d[p..p + 4].try_into().ok()?) as usize;
        let kind = &d[p + 4..p + 8];
        let body = d.get(p + 8..p + 8 + len)?;
        match kind {
            b"IHDR" => {
                w = u32::from_be_bytes(body[0..4].try_into().ok()?) as usize;
                h = u32::from_be_bytes(body[4..8].try_into().ok()?) as usize;
                depth = body[8];
                ctype = body[9];
                interlace = body[12];
            }
            b"PLTE" => plte = body.to_vec(),
            b"tRNS" => trns = body.to_vec(),
            b"IDAT" => idat.extend_from_slice(body),
            b"IEND" => break,
            _ => {}
        }
        p += 12 + len;
    }
    if w == 0 || h == 0 || interlace != 0 || w * h > 64 << 20 {
        return None;
    }
    let ch = match ctype {
        0 | 3 => 1,
        2 => 3,
        4 => 2,
        6 => 4,
        _ => return None,
    };
    let bpp_bits = ch * depth as usize;
    let stride = (w * bpp_bits).div_ceil(8);
    let bpp = bpp_bits.div_ceil(8).max(1);
    let raw = inflate(idat.get(2..)?)?;
    if raw.len() < (stride + 1) * h {
        return None;
    }
    let mut px = vec![0u8; stride * h];
    for y in 0..h {
        let f = raw[y * (stride + 1)];
        let line = &raw[y * (stride + 1) + 1..(y + 1) * (stride + 1)];
        for x in 0..stride {
            let a = if x >= bpp {
                px[y * stride + x - bpp] as i32
            } else {
                0
            };
            let b = if y > 0 {
                px[(y - 1) * stride + x] as i32
            } else {
                0
            };
            let c = if y > 0 && x >= bpp {
                px[(y - 1) * stride + x - bpp] as i32
            } else {
                0
            };
            let add = match f {
                0 => 0,
                1 => a,
                2 => b,
                3 => (a + b) / 2,
                4 => paeth(a, b, c),
                _ => return None,
            };
            px[y * stride + x] = (line[x] as i32 + add) as u8;
        }
    }
    // Bir pikselning `ch` kanalini 0..255 ga keltirib oladi.
    let sample = |row: &[u8], i: usize| -> u8 {
        match depth {
            8 => row[i],
            16 => row[i * 2],
            _ => {
                let per = 8 / depth as usize;
                let byte = row[i / per];
                let shift = 8 - depth as usize * (i % per + 1);
                let v = (byte >> shift) & ((1u16 << depth) - 1) as u8;
                if ctype == 3 {
                    v
                } else {
                    (v as u32 * 255 / ((1u32 << depth) - 1)) as u8
                }
            }
        }
    };
    let mut rgba = vec![255u8; w * h * 4];
    for y in 0..h {
        let row = &px[y * stride..(y + 1) * stride];
        for x in 0..w {
            let o = (y * w + x) * 4;
            let rgb: [u8; 4] = match ctype {
                0 => {
                    let g = sample(row, x);
                    [g, g, g, 255]
                }
                2 => [
                    sample(row, x * 3),
                    sample(row, x * 3 + 1),
                    sample(row, x * 3 + 2),
                    255,
                ],
                3 => {
                    let i = sample(row, x) as usize;
                    let c = plte.get(i * 3..i * 3 + 3)?;
                    [c[0], c[1], c[2], trns.get(i).copied().unwrap_or(255)]
                }
                4 => {
                    let g = sample(row, x * 2);
                    [g, g, g, sample(row, x * 2 + 1)]
                }
                _ => [
                    sample(row, x * 4),
                    sample(row, x * 4 + 1),
                    sample(row, x * 4 + 2),
                    sample(row, x * 4 + 3),
                ],
            };
            rgba[o..o + 4].copy_from_slice(&rgb);
        }
    }
    Some(Image {
        w: w as u32,
        h: h as u32,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inflate_saqlangan_va_dinamik() {
        // "stored" blok.
        assert_eq!(
            inflate(&[1, 3, 0, 0xFC, 0xFF, b'a', b'b', b'c']).unwrap(),
            b"abc"
        );
        // zlib(python) "hello hello hello hello" (fixed Huffman).
        let z = [203, 72, 205, 201, 201, 87, 200, 64, 39, 1];
        assert_eq!(inflate(&z).unwrap(), b"hello hello hello hello");
    }

    #[test]
    fn oz_png_yozuvchimiz_bilan_aylanish() {
        let rgba: Vec<u8> = (0..4 * 3 * 4).map(|i| (i * 7) as u8).collect();
        let png = crate::png::encode_rgba(4, 3, &rgba);
        let img = decode_png(&png).unwrap();
        assert_eq!((img.w, img.h), (4, 3));
        assert_eq!(img.rgba, rgba);
    }

    #[test]
    fn palitrali_4_bit_siqilgan_png() {
        // Haqiqiy flagcdn PNG (4 bit palitra, deflate) — sinov rasmi.
        let img = decode_png(include_bytes!("../testdata/flag_uz.png")).unwrap();
        assert_eq!((img.w, img.h), (80, 40));
        // Yuqori chiziq ko'k, o'rtasi oq (O'zbekiston bayrog'i).
        let px = |x: usize, y: usize| &img.rgba[(y * 80 + x) * 4..(y * 80 + x) * 4 + 3];
        assert!(px(40, 2)[2] > px(40, 2)[0]);
        assert_eq!(px(40, 20), &[255, 255, 255]);
    }

    #[test]
    fn kichraytirish() {
        let img = Image {
            w: 100,
            h: 50,
            rgba: vec![200; 100 * 50 * 4],
        }
        .resized(20, 20);
        assert_eq!((img.w, img.h), (20, 10));
        assert!(img.rgba.iter().all(|&v| v == 200));
        let same = Image {
            w: 4,
            h: 4,
            rgba: vec![1; 64],
        }
        .resized(10, 10);
        assert_eq!(same.w, 4);
        assert!(decode_image(include_bytes!("../testdata/j_prog.jpg")).is_some());
        assert!(decode_image(b"xx").is_none());
    }

    #[test]
    fn yumaloq_burchak() {
        let img = Image {
            w: 10,
            h: 10,
            rgba: vec![255; 400],
        }
        .round_corners(4.0);
        assert!(img.rgba[3] < 40); // chap-yuqori burchak shaffof
        assert_eq!(img.rgba[(5 * 10 + 5) * 4 + 3], 255);
    }
}
