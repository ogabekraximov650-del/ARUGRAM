//! Kichik JSON o'quvchi (tashqi kutubxonasiz, APK hajmi uchun).

#[derive(Clone, Debug, PartialEq)]
pub enum J {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

static NULL: J = J::Null;

impl J {
    pub fn parse(s: &str) -> Option<J> {
        let b = s.as_bytes();
        let mut i = 0;
        let v = value(b, &mut i, 0)?;
        ws(b, &mut i);
        if i == b.len() {
            Some(v)
        } else {
            None
        }
    }

    /// Ob'ekt maydoni (bo'lmasa `Null`).
    pub fn get(&self, k: &str) -> &J {
        match self {
            J::Obj(o) => o.iter().find(|(n, _)| n == k).map(|(_, v)| v).unwrap_or(&NULL),
            _ => &NULL,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, J::Null)
    }

    pub fn s(&self, k: &str) -> String {
        match self.get(k) {
            J::Str(s) => s.clone(),
            J::Num(n) => format!("{}", *n as i64),
            _ => String::new(),
        }
    }

    pub fn n(&self, k: &str) -> i64 {
        match self.get(k) {
            J::Num(n) => *n as i64,
            J::Str(s) => s.parse().unwrap_or(0),
            _ => 0,
        }
    }

    pub fn b(&self, k: &str) -> bool {
        matches!(self.get(k), J::Bool(true))
    }

    pub fn arr(&self) -> &[J] {
        match self {
            J::Arr(a) => a,
            _ => &[],
        }
    }
}

fn ws(b: &[u8], i: &mut usize) {
    while *i < b.len() && matches!(b[*i], b' ' | b'\n' | b'\r' | b'\t') {
        *i += 1;
    }
}

fn value(b: &[u8], i: &mut usize, depth: u32) -> Option<J> {
    if depth > 40 {
        return None;
    }
    ws(b, i);
    match *b.get(*i)? {
        b'{' => {
            *i += 1;
            let mut o = Vec::new();
            ws(b, i);
            if *b.get(*i)? == b'}' {
                *i += 1;
                return Some(J::Obj(o));
            }
            loop {
                ws(b, i);
                let k = string(b, i)?;
                ws(b, i);
                if *b.get(*i)? != b':' {
                    return None;
                }
                *i += 1;
                o.push((k, value(b, i, depth + 1)?));
                ws(b, i);
                match *b.get(*i)? {
                    b',' => *i += 1,
                    b'}' => {
                        *i += 1;
                        return Some(J::Obj(o));
                    }
                    _ => return None,
                }
            }
        }
        b'[' => {
            *i += 1;
            let mut a = Vec::new();
            ws(b, i);
            if *b.get(*i)? == b']' {
                *i += 1;
                return Some(J::Arr(a));
            }
            loop {
                a.push(value(b, i, depth + 1)?);
                ws(b, i);
                match *b.get(*i)? {
                    b',' => *i += 1,
                    b']' => {
                        *i += 1;
                        return Some(J::Arr(a));
                    }
                    _ => return None,
                }
            }
        }
        b'"' => Some(J::Str(string(b, i)?)),
        b't' if b[*i..].starts_with(b"true") => {
            *i += 4;
            Some(J::Bool(true))
        }
        b'f' if b[*i..].starts_with(b"false") => {
            *i += 5;
            Some(J::Bool(false))
        }
        b'n' if b[*i..].starts_with(b"null") => {
            *i += 4;
            Some(J::Null)
        }
        _ => {
            let s = *i;
            while *i < b.len() && matches!(b[*i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                *i += 1;
            }
            std::str::from_utf8(&b[s..*i]).ok()?.parse().ok().map(J::Num)
        }
    }
}

fn hex4(b: &[u8], i: usize) -> Option<u32> {
    u32::from_str_radix(std::str::from_utf8(b.get(i..i + 4)?).ok()?, 16).ok()
}

fn string(b: &[u8], i: &mut usize) -> Option<String> {
    if *b.get(*i)? != b'"' {
        return None;
    }
    *i += 1;
    let mut out: Vec<u8> = Vec::new();
    loop {
        let c = *b.get(*i)?;
        *i += 1;
        match c {
            b'"' => return String::from_utf8(out).ok(),
            b'\\' => {
                let e = *b.get(*i)?;
                *i += 1;
                match e {
                    b'n' => out.push(b'\n'),
                    b't' => out.push(b'\t'),
                    b'r' => out.push(b'\r'),
                    b'b' => out.push(8),
                    b'f' => out.push(12),
                    b'u' => {
                        let mut cp = hex4(b, *i)?;
                        *i += 4;
                        if (0xD800..0xDC00).contains(&cp) && b.get(*i) == Some(&b'\\') && b.get(*i + 1) == Some(&b'u') {
                            let lo = hex4(b, *i + 2)?;
                            *i += 6;
                            cp = 0x10000 + ((cp - 0xD800) << 10) + (lo.wrapping_sub(0xDC00) & 0x3FF);
                        }
                        let ch = char::from_u32(cp).unwrap_or('\u{FFFD}');
                        let mut buf = [0u8; 4];
                        out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                    }
                    other => out.push(other),
                }
            }
            c => out.push(c),
        }
    }
}

/// Matnni JSON qatori (qo'shtirnoq bilan) qilib beradi.
pub fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push(' '),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oqiydi() {
        let j = J::parse(r#"{"a":1,"b":"salom \"dunyo\" Ж","c":[1,2,{"d":true}],"e":null,"f":-2.5e1}"#).unwrap();
        assert_eq!(j.n("a"), 1);
        assert_eq!(j.s("b"), "salom \"dunyo\" Ж");
        assert_eq!(j.get("c").arr().len(), 3);
        assert!(j.get("c").arr()[2].b("d"));
        assert!(j.get("e").is_null());
        assert_eq!(j.n("f"), -25);
        assert!(J::parse("{").is_none());
        assert!(J::parse("[1,2] x").is_none());
    }

    #[test]
    fn juft_belgi_va_esc() {
        assert_eq!(J::parse(r#""😀""#), Some(J::Str("😀".into())));
        assert_eq!(esc("a\"b\n"), "\"a\\\"b\\n\"");
        assert_eq!(J::parse(&esc("o'zbek \"tom\"\\")).unwrap(), J::Str("o'zbek \"tom\"\\".into()));
    }
}
