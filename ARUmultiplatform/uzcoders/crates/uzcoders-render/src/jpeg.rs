//! UZ: JPEG dekoderi (noldan yozilgan): baseline va progressive, 1 yoki 3 komponent, har qanday diskretlash (4:4:4, 4:2:0 ...) va restart intervallari.
//! EN: A JPEG decoder (written from scratch): baseline and progressive, 1 or 3 components, any chroma subsampling (4:4:4, 4:2:0 ...) and restart intervals.
//! RU: Декодер JPEG (написан с нуля): baseline и progressive, 1 или 3 компонента, любое субдискретизирование цвета (4:4:4, 4:2:0 ...) и интервалы restart.

use crate::image::Image;

const ZZ: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

#[derive(Clone, Default)]
struct Huff {
    maxcode: [i32; 18],
    valptr: [i32; 17],
    mincode: [i32; 17],
    vals: Vec<u8>,
}

fn build_huff(counts: &[u8; 16], vals: Vec<u8>) -> Huff {
    let mut h = Huff {
        vals,
        ..Huff::default()
    };
    let (mut code, mut k) = (0i32, 0i32);
    for l in 1..=16 {
        h.valptr[l] = k;
        h.mincode[l] = code;
        code += counts[l - 1] as i32;
        k += counts[l - 1] as i32;
        h.maxcode[l] = if counts[l - 1] > 0 { code - 1 } else { -1 };
        code <<= 1;
    }
    h.maxcode[17] = i32::MAX;
    h
}

struct Bits<'a> {
    d: &'a [u8],
    pos: usize,
    buf: u32,
    cnt: u32,
    marker: bool,
}

impl<'a> Bits<'a> {
    fn new(d: &'a [u8], pos: usize) -> Self {
        Bits {
            d,
            pos,
            buf: 0,
            cnt: 0,
            marker: false,
        }
    }

    fn bit(&mut self) -> u32 {
        if self.cnt == 0 {
            let mut b = 0u8;
            if !self.marker {
                match self.d.get(self.pos) {
                    Some(&0xFF) => match self.d.get(self.pos + 1) {
                        Some(&0) => {
                            b = 0xFF;
                            self.pos += 2;
                        }
                        // UZ: marker: bit oqimi tugadi, nollar beramiz
                        // EN: a marker: the bit stream ended, so we return zeros.
                        // RU: маркер: битовый поток закончился, возвращаем нули.
                        _ => self.marker = true,
                    },
                    Some(&v) => {
                        b = v;
                        self.pos += 1;
                    }
                    None => self.marker = true,
                }
            }
            self.buf = b as u32;
            self.cnt = 8;
        }
        self.cnt -= 1;
        (self.buf >> self.cnt) & 1
    }

    fn bits(&mut self, n: u32) -> i32 {
        let mut v = 0i32;
        for _ in 0..n {
            v = (v << 1) | self.bit() as i32;
        }
        v
    }

    fn extend(&mut self, n: u32) -> i32 {
        if n == 0 {
            return 0;
        }
        let v = self.bits(n);
        if v < (1 << (n - 1)) {
            v - (1 << n) + 1
        } else {
            v
        }
    }

    fn decode(&mut self, h: &Huff) -> u8 {
        let mut code = 0i32;
        for l in 1..=16 {
            code = (code << 1) | self.bit() as i32;
            if h.maxcode[l] >= 0 && code <= h.maxcode[l] {
                let i = (h.valptr[l] + code - h.mincode[l]) as usize;
                return h.vals.get(i).copied().unwrap_or(0);
            }
        }
        0
    }

    /// UZ: Restart: bit oqimini bayt chegarasiga tekislaydi va `RSTn` markerini o'tkazib yuboradi.
    /// EN: Restart: aligns the bit stream to a byte boundary and skips the `RSTn` marker.
    /// RU: Restart: выравнивает битовый поток по границе байта и пропускает маркер `RSTn`.
    fn restart(&mut self) {
        self.cnt = 0;
        self.marker = false;
        while self.pos + 1 < self.d.len() {
            if self.d[self.pos] == 0xFF && (0xD0..=0xD7).contains(&self.d[self.pos + 1]) {
                self.pos += 2;
                return;
            }
            if self.d[self.pos] == 0xFF && self.d[self.pos + 1] != 0 && self.d[self.pos + 1] != 0xFF
            {
                return;
            }
            self.pos += 1;
        }
    }
}

struct Comp {
    id: u8,
    h: usize,
    v: usize,
    tq: usize,
    bw: usize,
    bh: usize,
    /// UZ: Haqiqiy (to'ldirilmagan) bloklar soni: gorizontal / vertikal.
    /// EN: Number of real (non-padding) blocks: horizontal / vertical.
    /// RU: Число реальных (без добивки) блоков: по горизонтали / по вертикали.
    rw: usize,
    rh: usize,
    coef: Vec<i16>,
    dc: usize,
    ac: usize,
}

/// UZ: JPEG ni RGBA8 ga ochadi. Qo'llab-quvvatlanmaydi: arifmetik kodlash, 12 bit, CMYK.
/// EN: Decodes JPEG into RGBA8. Not supported: arithmetic coding, 12-bit, CMYK.
/// RU: Декодирует JPEG в RGBA8. Не поддерживается: арифметическое кодирование, 12 бит, CMYK.
pub fn decode_jpeg(d: &[u8]) -> Option<Image> {
    if d.get(..2)? != [0xFF, 0xD8] {
        return None;
    }
    let mut qt = [[0u16; 64]; 4];
    let mut dc_t: Vec<Huff> = vec![Huff::default(); 4];
    let mut ac_t: Vec<Huff> = vec![Huff::default(); 4];
    let mut comps: Vec<Comp> = Vec::new();
    let (mut w, mut h) = (0usize, 0usize);
    let (mut hmax, mut vmax) = (1usize, 1usize);
    let mut progressive = false;
    let mut restart = 0usize;
    let mut p = 2;
    let mut got_scan = false;
    while p + 4 <= d.len() {
        if d[p] != 0xFF {
            p += 1;
            continue;
        }
        let m = d[p + 1];
        if m == 0xFF || m == 0 || (0xD0..=0xD8).contains(&m) || m == 0x01 {
            p += if m == 0xFF { 1 } else { 2 };
            continue;
        }
        if m == 0xD9 {
            break;
        }
        let len = u16::from_be_bytes([d[p + 2], d[p + 3]]) as usize;
        let body = d.get(p + 4..p + 2 + len)?;
        match m {
            0xDB => {
                let mut i = 0;
                while i < body.len() {
                    let (pq, tq) = (body[i] >> 4, (body[i] & 15) as usize);
                    i += 1;
                    for k in 0..64 {
                        let v = if pq == 0 {
                            let v = *body.get(i)? as u16;
                            i += 1;
                            v
                        } else {
                            let v = u16::from_be_bytes([*body.get(i)?, *body.get(i + 1)?]);
                            i += 2;
                            v
                        };
                        *qt.get_mut(tq)?.get_mut(ZZ[k])? = v;
                    }
                }
            }
            0xC4 => {
                let mut i = 0;
                while i + 17 <= body.len() {
                    let (tc, th) = (body[i] >> 4, (body[i] & 15) as usize);
                    let mut counts = [0u8; 16];
                    counts.copy_from_slice(&body[i + 1..i + 17]);
                    let n: usize = counts.iter().map(|&c| c as usize).sum();
                    let vals = body.get(i + 17..i + 17 + n)?.to_vec();
                    i += 17 + n;
                    let hf = build_huff(&counts, vals);
                    if th > 3 {
                        return None;
                    }
                    if tc == 0 {
                        dc_t[th] = hf;
                    } else {
                        ac_t[th] = hf;
                    }
                }
            }
            0xC0 | 0xC1 | 0xC2 => {
                progressive = m == 0xC2;
                if body[0] != 8 {
                    return None;
                }
                h = u16::from_be_bytes([body[1], body[2]]) as usize;
                w = u16::from_be_bytes([body[3], body[4]]) as usize;
                let n = body[5] as usize;
                if w == 0 || h == 0 || (n != 1 && n != 3) || w * h > 64 << 20 {
                    return None;
                }
                comps.clear();
                for c in 0..n {
                    let b = body.get(6 + c * 3..9 + c * 3)?;
                    comps.push(Comp {
                        id: b[0],
                        h: (b[1] >> 4) as usize,
                        v: (b[1] & 15) as usize,
                        tq: (b[2] & 3) as usize,
                        bw: 0,
                        bh: 0,
                        rw: 0,
                        rh: 0,
                        coef: Vec::new(),
                        dc: 0,
                        ac: 0,
                    });
                }
                hmax = comps.iter().map(|c| c.h).max()?.max(1);
                vmax = comps.iter().map(|c| c.v).max()?.max(1);
                let (mcux, mcuy) = (w.div_ceil(8 * hmax), h.div_ceil(8 * vmax));
                for c in comps.iter_mut() {
                    if c.h == 0 || c.v == 0 {
                        return None;
                    }
                    c.bw = mcux * c.h;
                    c.bh = mcuy * c.v;
                    c.rw = (w * c.h).div_ceil(hmax).div_ceil(8);
                    c.rh = (h * c.v).div_ceil(vmax).div_ceil(8);
                    c.coef = vec![0i16; c.bw * c.bh * 64];
                }
            }
            0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF => return None,
            0xDD => restart = u16::from_be_bytes([body[0], body[1]]) as usize,
            0xDA => {
                if comps.is_empty() {
                    return None;
                }
                let ns = body[0] as usize;
                let mut sc: Vec<usize> = Vec::new();
                for i in 0..ns {
                    let (cid, t) = (body[1 + i * 2], body[2 + i * 2]);
                    let ci = comps.iter().position(|c| c.id == cid)?;
                    comps[ci].dc = (t >> 4) as usize & 3;
                    comps[ci].ac = (t & 15) as usize & 3;
                    sc.push(ci);
                }
                let (ss, se, a) = (
                    body[1 + ns * 2] as usize,
                    body[2 + ns * 2] as usize,
                    body[3 + ns * 2],
                );
                let (ah, al) = ((a >> 4) as u32, (a & 15) as u32);
                let mut br = Bits::new(d, p + 2 + len);
                scan(
                    &mut br,
                    &mut comps,
                    &sc,
                    &dc_t,
                    &ac_t,
                    w,
                    h,
                    hmax,
                    vmax,
                    restart,
                    progressive,
                    ss,
                    se.min(63),
                    ah,
                    al,
                );
                p = br.pos;
                got_scan = true;
                continue;
            }
            _ => {}
        }
        p += 2 + len;
    }
    if !got_scan || comps.is_empty() {
        return None;
    }
    // UZ: Dekvantlash + teskari kosinus almashtirish (IDCT) -> rang tekisliklari (komponentlar).
    // EN: Dequantization + inverse DCT (IDCT) -> color planes (components).
    // RU: Деквантование + обратное ДКП (IDCT) -> цветовые плоскости (компоненты).
    let mut planes: Vec<(Vec<u8>, usize)> = Vec::new();
    let mut cost = [[0f32; 8]; 8];
    for (x, row) in cost.iter_mut().enumerate() {
        for (u, c) in row.iter_mut().enumerate() {
            let cu = if u == 0 {
                std::f32::consts::FRAC_1_SQRT_2
            } else {
                1.0
            };
            *c = 0.5 * cu * (((2 * x + 1) as f32 * u as f32 * std::f32::consts::PI) / 16.0).cos();
        }
    }
    for c in &comps {
        let stride = c.bw * 8;
        let mut plane = vec![0u8; stride * c.bh * 8];
        for by in 0..c.bh {
            for bx in 0..c.bw {
                let blk = &c.coef[(by * c.bw + bx) * 64..(by * c.bw + bx) * 64 + 64];
                let mut f = [0f32; 64];
                for i in 0..64 {
                    f[i] = blk[i] as f32 * qt[c.tq][i] as f32;
                }
                let mut tmp = [0f32; 64];
                for v in 0..8 {
                    for x in 0..8 {
                        let mut s = 0.0;
                        for u in 0..8 {
                            s += cost[x][u] * f[v * 8 + u];
                        }
                        tmp[v * 8 + x] = s;
                    }
                }
                for x in 0..8 {
                    for y in 0..8 {
                        let mut s = 0.0;
                        for v in 0..8 {
                            s += cost[y][v] * tmp[v * 8 + x];
                        }
                        plane[(by * 8 + y) * stride + bx * 8 + x] =
                            (s + 128.5).clamp(0.0, 255.0) as u8;
                    }
                }
            }
        }
        planes.push((plane, stride));
    }
    let mut rgba = vec![255u8; w * h * 4];
    let sample = |ci: usize, x: usize, y: usize| -> f32 {
        let c = &comps[ci];
        let (pl, st) = &planes[ci];
        pl[(y * c.v / vmax) * st + x * c.h / hmax] as f32
    };
    for y in 0..h {
        for x in 0..w {
            let o = (y * w + x) * 4;
            if comps.len() == 1 {
                let g = sample(0, x, y) as u8;
                rgba[o..o + 3].copy_from_slice(&[g, g, g]);
            } else {
                let (yy, cb, cr) = (
                    sample(0, x, y),
                    sample(1, x, y) - 128.0,
                    sample(2, x, y) - 128.0,
                );
                rgba[o] = (yy + 1.402 * cr).clamp(0.0, 255.0) as u8;
                rgba[o + 1] = (yy - 0.344136 * cb - 0.714136 * cr).clamp(0.0, 255.0) as u8;
                rgba[o + 2] = (yy + 1.772 * cb).clamp(0.0, 255.0) as u8;
            }
        }
    }
    Some(Image {
        w: w as u32,
        h: h as u32,
        rgba,
    })
}

#[allow(clippy::too_many_arguments)]
fn scan(
    br: &mut Bits,
    comps: &mut [Comp],
    sc: &[usize],
    dc_t: &[Huff],
    ac_t: &[Huff],
    w: usize,
    h: usize,
    hmax: usize,
    vmax: usize,
    restart: usize,
    progressive: bool,
    ss: usize,
    se: usize,
    ah: u32,
    al: u32,
) {
    let mut pred = [0i32; 4];
    let mut eobrun = 0i32;
    let mut count = 0usize;
    let single = sc.len() == 1;
    let (mcux, mcuy) = if single {
        (comps[sc[0]].rw, comps[sc[0]].rh)
    } else {
        (w.div_ceil(8 * hmax), h.div_ceil(8 * vmax))
    };
    for my in 0..mcuy {
        for mx in 0..mcux {
            if restart > 0 && count > 0 && count % restart == 0 {
                br.restart();
                pred = [0; 4];
                eobrun = 0;
            }
            count += 1;
            for (si, &ci) in sc.iter().enumerate() {
                let (bh, bv) = if single {
                    (1, 1)
                } else {
                    (comps[ci].h, comps[ci].v)
                };
                for vy in 0..bv {
                    for hx in 0..bh {
                        let c = &mut comps[ci];
                        let (bx, by) = if single {
                            (mx, my)
                        } else {
                            (mx * bh + hx, my * bv + vy)
                        };
                        let off = (by * c.bw + bx) * 64;
                        let blk = &mut c.coef[off..off + 64];
                        let (dct, act) = (&dc_t[c.dc], &ac_t[c.ac]);
                        if !progressive {
                            let t = br.decode(dct) as u32;
                            pred[si] += br.extend(t);
                            blk[0] = pred[si] as i16;
                            let mut k = 1;
                            while k < 64 {
                                let rs = br.decode(act);
                                let (s, r) = ((rs & 15) as u32, (rs >> 4) as usize);
                                if s == 0 {
                                    if r == 15 {
                                        k += 16;
                                        continue;
                                    }
                                    break;
                                }
                                k += r;
                                if k > 63 {
                                    break;
                                }
                                blk[ZZ[k]] = br.extend(s) as i16;
                                k += 1;
                            }
                        } else if ss == 0 {
                            if ah == 0 {
                                let t = br.decode(dct) as u32;
                                pred[si] += br.extend(t);
                                blk[0] = (pred[si] << al) as i16;
                            } else if br.bit() == 1 {
                                blk[0] |= 1 << al;
                            }
                        } else if ah == 0 {
                            if eobrun > 0 {
                                eobrun -= 1;
                                continue;
                            }
                            let mut k = ss;
                            while k <= se {
                                let rs = br.decode(act);
                                let (s, r) = ((rs & 15) as u32, (rs >> 4) as usize);
                                if s == 0 {
                                    if r < 15 {
                                        eobrun = (1 << r) - 1;
                                        if r > 0 {
                                            eobrun += br.bits(r as u32);
                                        }
                                        break;
                                    }
                                    k += 16;
                                    continue;
                                }
                                k += r;
                                if k > 63 {
                                    break;
                                }
                                blk[ZZ[k]] = (br.extend(s) << al) as i16;
                                k += 1;
                            }
                        } else {
                            let (p1, m1) = (1i16 << al, -1i16 << al);
                            let mut k = ss;
                            if eobrun == 0 {
                                while k <= se {
                                    let rs = br.decode(act);
                                    let (s0, mut r) = ((rs & 15) as i32, (rs >> 4) as i32);
                                    let mut val = 0i16;
                                    if s0 != 0 {
                                        val = if br.bit() == 1 { p1 } else { m1 };
                                    } else if r != 15 {
                                        eobrun = 1 << r;
                                        if r > 0 {
                                            eobrun += br.bits(r as u32);
                                        }
                                        break;
                                    }
                                    while k <= se {
                                        let cf = &mut blk[ZZ[k]];
                                        if *cf != 0 {
                                            if br.bit() == 1 && (*cf & p1) == 0 {
                                                *cf += if *cf >= 0 { p1 } else { m1 };
                                            }
                                        } else {
                                            r -= 1;
                                            if r < 0 {
                                                break;
                                            }
                                        }
                                        k += 1;
                                    }
                                    if val != 0 && k <= se {
                                        blk[ZZ[k]] = val;
                                    }
                                    k += 1;
                                }
                            }
                            if eobrun > 0 {
                                while k <= se {
                                    let cf = &mut blk[ZZ[k]];
                                    if *cf != 0 && br.bit() == 1 && (*cf & p1) == 0 {
                                        *cf += if *cf >= 0 { p1 } else { m1 };
                                    }
                                    k += 1;
                                }
                                eobrun -= 1;
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::decode_png;

    /// UZ: Dekoder natijasini PIL (libjpeg) natijasi bilan solishtiradi: o'rtacha farq kichik bo'lishi kerak.
    /// EN: Compares the decoder output with PIL (libjpeg) output: the mean difference must be small.
    /// RU: Сравнивает результат декодера с результатом PIL (libjpeg): средняя разница должна быть мала.
    fn check(jpg: &[u8], png: &[u8], tol: f32) {
        let a = decode_jpeg(jpg).expect("jpeg ochilmadi");
        let b = decode_png(png).unwrap();
        assert_eq!((a.w, a.h), (b.w, b.h));
        let mut sum = 0f64;
        let mut n = 0usize;
        for (x, y) in a.rgba.chunks(4).zip(b.rgba.chunks(4)) {
            for k in 0..3 {
                sum += (x[k] as f64 - y[k] as f64).abs();
                n += 1;
            }
        }
        let mean = (sum / n as f64) as f32;
        assert!(mean < tol, "o'rtacha farq {mean}");
    }

    #[test]
    fn baseline_420() {
        check(
            include_bytes!("../testdata/j_base420.jpg"),
            include_bytes!("../testdata/j_base420.png"),
            3.5,
        );
    }

    #[test]
    fn baseline_444() {
        check(
            include_bytes!("../testdata/j_base444.jpg"),
            include_bytes!("../testdata/j_base444.png"),
            2.0,
        );
    }

    #[test]
    fn progressive() {
        check(
            include_bytes!("../testdata/j_prog.jpg"),
            include_bytes!("../testdata/j_prog.png"),
            3.5,
        );
    }

    #[test]
    fn kulrang() {
        check(
            include_bytes!("../testdata/j_gray.jpg"),
            include_bytes!("../testdata/j_gray.png"),
            2.0,
        );
    }

    #[test]
    fn buzuq_fayl_yiqilmaydi() {
        let j = include_bytes!("../testdata/j_prog.jpg");
        for cut in [10, 100, 600, 1500] {
            let _ = decode_jpeg(&j[..cut.min(j.len())]);
        }
        assert!(decode_jpeg(b"salom").is_none());
    }
}
