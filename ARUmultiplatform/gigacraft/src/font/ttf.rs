//! UZ: TrueType (`.ttf`) shrift o'quvchisi — noldan yozilgan.
//! Qo'llab-quvvatlanadi: head, maxp, hhea, hmtx, cmap (format 4 va 12), loca, glyf (oddiy va murakkab shakllar).
//! EN: A TrueType (`.ttf`) font reader — written from scratch.
//! Supported tables: head, maxp, hhea, hmtx, cmap (formats 4 and 12), loca, glyf (simple and composite glyphs).
//! RU: Читатель шрифтов TrueType (`.ttf`) — написан с нуля.
//! Поддерживаются таблицы: head, maxp, hhea, hmtx, cmap (форматы 4 и 12), loca, glyf (простые и составные глифы).

const MAX_DEPTH: u32 = 6;

/// UZ: Kontur nuqtasi: (x, y, chiziq ustidami) — shrift birliklarida.
/// EN: Contour point: (x, y, on-curve flag), in font units.
/// RU: Точка контура: (x, y, лежит ли на кривой) — в единицах шрифта.
#[derive(Clone, Copy, Debug)]
pub struct Pt {
    pub x: f32,
    pub y: f32,
    pub on: bool,
}

pub type Contour = Vec<Pt>;

#[derive(Clone)]
pub struct Font {
    data: Vec<u8>,
    glyf: usize,
    loca: usize,
    hmtx: usize,
    cmap: usize,
    cmap_format: u16,
    loca_long: bool,
    num_glyphs: u16,
    num_hmetrics: u16,
    pub units_per_em: u16,
    pub ascent: i16,
    pub descent: i16,
    pub line_gap: i16,
}

fn u16_at(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*d.get(o)?, *d.get(o + 1)?]))
}
fn i16_at(d: &[u8], o: usize) -> Option<i16> {
    u16_at(d, o).map(|v| v as i16)
}
fn u32_at(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes([
        *d.get(o)?,
        *d.get(o + 1)?,
        *d.get(o + 2)?,
        *d.get(o + 3)?,
    ]))
}

impl Font {
    pub fn parse(data: &[u8]) -> Option<Font> {
        let d = data;
        let tag = u32_at(d, 0)?;
        if tag != 0x0001_0000 && tag != 0x7472_7565 {
            // UZ: faqat TrueType qo'llab-quvvatlanadi
            // EN: only TrueType is supported.
            // RU: поддерживается только TrueType.
            return None;
        }
        let n = u16_at(d, 4)? as usize;
        let find = |name: &[u8; 4]| -> Option<usize> {
            for i in 0..n {
                let r = 12 + i * 16;
                if d.get(r..r + 4)? == name {
                    return Some(u32_at(d, r + 8)? as usize);
                }
            }
            None
        };
        let head = find(b"head")?;
        let maxp = find(b"maxp")?;
        let hhea = find(b"hhea")?;
        let hmtx = find(b"hmtx")?;
        let cmap = find(b"cmap")?;
        let loca = find(b"loca")?;
        let glyf = find(b"glyf")?;
        let units_per_em = u16_at(d, head + 18)?;
        if units_per_em == 0 {
            return None;
        }
        let loca_long = i16_at(d, head + 50)? != 0;
        let num_glyphs = u16_at(d, maxp + 4)?;
        let ascent = i16_at(d, hhea + 4)?;
        let descent = i16_at(d, hhea + 6)?;
        let line_gap = i16_at(d, hhea + 8)?;
        let num_hmetrics = u16_at(d, hhea + 34)?;

        // UZ: cmap: 12-format (to'liq Unicode) afzal ko'riladi; bo'lmasa 4-format (faqat BMP) ishlatiladi.
        // EN: cmap: format 12 (full Unicode) is preferred; otherwise format 4 (BMP only) is used.
        // RU: cmap: предпочтителен формат 12 (полный Unicode); иначе используется формат 4 (только BMP).
        let ntab = u16_at(d, cmap + 2)? as usize;
        let mut best: Option<(usize, u16)> = None;
        for i in 0..ntab {
            let r = cmap + 4 + i * 8;
            let plat = u16_at(d, r)?;
            let enc = u16_at(d, r + 2)?;
            let off = cmap + u32_at(d, r + 4)? as usize;
            let fmt = u16_at(d, off)?;
            let unicode = plat == 0 || (plat == 3 && (enc == 1 || enc == 10));
            if !unicode || (fmt != 4 && fmt != 12) {
                continue;
            }
            if best.map_or(true, |(_, f)| fmt == 12 && f != 12) {
                best = Some((off, fmt));
            }
        }
        let (cmap_off, cmap_format) = best?;
        Some(Font {
            data: data.to_vec(),
            glyf,
            loca,
            hmtx,
            cmap: cmap_off,
            cmap_format,
            loca_long,
            num_glyphs,
            num_hmetrics,
            units_per_em,
            ascent,
            descent,
            line_gap,
        })
    }

    pub fn num_glyphs(&self) -> u16 {
        self.num_glyphs
    }

    /// UZ: Unicode belgi -> glif raqami (0 — topilmadi).
    /// EN: Unicode character -> glyph index (0 means not found).
    /// RU: Символ Unicode -> номер глифа (0 — не найден).
    pub fn glyph_id(&self, ch: char) -> u16 {
        self.lookup(ch as u32).unwrap_or(0)
    }

    fn lookup(&self, cp: u32) -> Option<u16> {
        let d = &self.data;
        let b = self.cmap;
        if self.cmap_format == 12 {
            let groups = u32_at(d, b + 12)? as usize;
            let (mut lo, mut hi) = (0usize, groups);
            while lo < hi {
                let mid = (lo + hi) / 2;
                let g = b + 16 + mid * 12;
                let (s, e, id) = (u32_at(d, g)?, u32_at(d, g + 4)?, u32_at(d, g + 8)?);
                if cp < s {
                    hi = mid;
                } else if cp > e {
                    lo = mid + 1;
                } else {
                    return Some((id + (cp - s)) as u16);
                }
            }
            return None;
        }
        if cp > 0xFFFF {
            return None;
        }
        let seg2 = u16_at(d, b + 6)? as usize;
        let ends = b + 14;
        let starts = b + 16 + seg2;
        let deltas = starts + seg2;
        let ranges = deltas + seg2;
        for i in 0..seg2 / 2 {
            let end = u16_at(d, ends + i * 2)? as u32;
            if cp > end {
                continue;
            }
            let start = u16_at(d, starts + i * 2)? as u32;
            if cp < start {
                return None;
            }
            let delta = u16_at(d, deltas + i * 2)?;
            let ro = u16_at(d, ranges + i * 2)? as usize;
            if ro == 0 {
                return Some((cp as u16).wrapping_add(delta));
            }
            let at = ranges + i * 2 + ro + (cp - start) as usize * 2;
            let g = u16_at(d, at)?;
            return Some(if g == 0 { 0 } else { g.wrapping_add(delta) });
        }
        None
    }

    /// UZ: Glifning gorizontal siljishi (advance), shrift birliklarida.
    /// EN: The glyph's horizontal advance, in font units.
    /// RU: Горизонтальный шаг глифа (advance) в единицах шрифта.
    pub fn advance_units(&self, gid: u16) -> u16 {
        if self.num_hmetrics == 0 {
            return 0;
        }
        let i = (gid.min(self.num_hmetrics - 1)) as usize;
        u16_at(&self.data, self.hmtx + i * 4).unwrap_or(0)
    }

    fn glyph_range(&self, gid: u16) -> Option<(usize, usize)> {
        if gid >= self.num_glyphs {
            return None;
        }
        let g = gid as usize;
        let (a, b) = if self.loca_long {
            (
                u32_at(&self.data, self.loca + g * 4)? as usize,
                u32_at(&self.data, self.loca + g * 4 + 4)? as usize,
            )
        } else {
            (
                u16_at(&self.data, self.loca + g * 2)? as usize * 2,
                u16_at(&self.data, self.loca + g * 2 + 2)? as usize * 2,
            )
        };
        if b <= a {
            // UZ: bo'sh shakl (masalan, probel)
            // EN: an empty glyph (for example, a space).
            // RU: пустой глиф (например, пробел).
            return None;
        }
        Some((self.glyf + a, self.glyf + b))
    }

    /// UZ: Glifning kontur nuqtalari (shrift birliklarida, y o'qi yuqoriga qaragan).
    /// EN: The glyph's outline points (in font units, y axis pointing up).
    /// RU: Точки контура глифа (в единицах шрифта, ось y направлена вверх).
    pub fn outline(&self, gid: u16) -> Vec<Contour> {
        let mut out = Vec::new();
        self.collect(gid, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], 0, &mut out);
        out
    }

    fn collect(&self, gid: u16, m: [f32; 6], depth: u32, out: &mut Vec<Contour>) -> Option<()> {
        if depth > MAX_DEPTH {
            return None;
        }
        let (start, end) = match self.glyph_range(gid) {
            Some(r) => r,
            None => return Some(()),
        };
        let d = &self.data[..end.min(self.data.len())];
        let nc = i16_at(d, start)?;
        let mut p = start + 10;
        if nc >= 0 {
            let nc = nc as usize;
            let mut ends = Vec::with_capacity(nc);
            for i in 0..nc {
                ends.push(u16_at(d, p + i * 2)? as usize);
            }
            p += nc * 2;
            let npts = ends.last().map_or(0, |e| e + 1);
            let ilen = u16_at(d, p)? as usize;
            p += 2 + ilen;
            let mut flags = Vec::with_capacity(npts);
            while flags.len() < npts {
                let f = *d.get(p)?;
                p += 1;
                flags.push(f);
                if f & 8 != 0 {
                    let rep = *d.get(p)?;
                    p += 1;
                    for _ in 0..rep {
                        if flags.len() < npts {
                            flags.push(f);
                        }
                    }
                }
            }
            let read = |short: u8, same: u8, p: &mut usize| -> Option<Vec<i32>> {
                let mut v = Vec::with_capacity(npts);
                let mut cur = 0i32;
                for &f in &flags {
                    if f & short != 0 {
                        let b = *d.get(*p)? as i32;
                        *p += 1;
                        cur += if f & same != 0 { b } else { -b };
                    } else if f & same == 0 {
                        cur += i16_at(d, *p)? as i32;
                        *p += 2;
                    }
                    v.push(cur);
                }
                Some(v)
            };
            let xs = read(2, 16, &mut p)?;
            let ys = read(4, 32, &mut p)?;
            let mut s = 0;
            for &e in &ends {
                if e < s || e >= npts {
                    return None;
                }
                let mut c = Vec::with_capacity(e + 1 - s);
                for i in s..=e {
                    let (x, y) = (xs[i] as f32, ys[i] as f32);
                    c.push(Pt {
                        x: m[0] * x + m[2] * y + m[4],
                        y: m[1] * x + m[3] * y + m[5],
                        on: flags[i] & 1 != 0,
                    });
                }
                out.push(c);
                s = e + 1;
            }
            return Some(());
        }
        // UZ: Murakkab shakl: bir nechta glifni bitta qilib birlashtiradi (masalan, harf + urg'u belgisi).
        // EN: Composite glyph: combines several glyphs into one (for example, a letter plus an accent).
        // RU: Составной глиф: объединяет несколько глифов в один (например, буква + акцент).
        loop {
            let flags = u16_at(d, p)?;
            let child = u16_at(d, p + 2)?;
            p += 4;
            let (dx, dy);
            if flags & 1 != 0 {
                dx = i16_at(d, p)? as f32;
                dy = i16_at(d, p + 2)? as f32;
                p += 4;
            } else {
                dx = *d.get(p)? as i8 as f32;
                dy = *d.get(p + 1)? as i8 as f32;
                p += 2;
            }
            // UZ: nuqta-moslash (point matching) qo'llab-quvvatlanmaydi
            // EN: point matching is not supported.
            // RU: сопоставление точек (point matching) не поддерживается.
            let (dx, dy) = if flags & 2 != 0 { (dx, dy) } else { (0.0, 0.0) };
            let f2 = |o: usize| i16_at(d, o).map(|v| v as f32 / 16384.0);
            let (mut a, mut b, mut c, mut dd) = (1.0, 0.0, 0.0, 1.0);
            if flags & 8 != 0 {
                a = f2(p)?;
                dd = a;
                p += 2;
            } else if flags & 0x40 != 0 {
                a = f2(p)?;
                dd = f2(p + 2)?;
                p += 4;
            } else if flags & 0x80 != 0 {
                a = f2(p)?;
                b = f2(p + 2)?;
                c = f2(p + 4)?;
                dd = f2(p + 6)?;
                p += 8;
            }
            // UZ: Bola glif nuqtasi = [a c; b d] * (x, y) + (dx, dy); so'ng ota matritsa `m` qo'llanadi.
            // EN: Child glyph point = [a c; b d] * (x, y) + (dx, dy); then the parent matrix `m` is applied.
            // RU: Точка дочернего глифа = [a c; b d] * (x, y) + (dx, dy); затем применяется родительская матрица `m`.
            let cm = [a, b, c, dd, dx, dy];
            let comb = [
                m[0] * cm[0] + m[2] * cm[1],
                m[1] * cm[0] + m[3] * cm[1],
                m[0] * cm[2] + m[2] * cm[3],
                m[1] * cm[2] + m[3] * cm[3],
                m[0] * cm[4] + m[2] * cm[5] + m[4],
                m[1] * cm[4] + m[3] * cm[5] + m[5],
            ];
            self.collect(child, comb, depth + 1, out);
            if flags & 0x20 == 0 {
                break;
            }
        }
        Some(())
    }
}
