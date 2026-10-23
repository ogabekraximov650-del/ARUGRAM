//! UZ: Emoji klasterlarini (bitta ko'rinadigan belgi: bayroq, ZWJ ketma-ketligi, teri rangi, shaxsiy belgi) ajratish — matn maydonida emoji rasm bilan chizilishi uchun.
//! EN: Splitting text into emoji clusters (one visible symbol: a flag, a ZWJ sequence, a skin tone, a private-use char) so the input field can draw emoji as images.
//! RU: Разбиение текста на кластеры эмодзи (один видимый символ: флаг, ZWJ-последовательность, оттенок кожи, символ частного использования), чтобы поле ввода рисовало эмодзи картинками.

fn base_emoji(c: char) -> bool {
    matches!(c as u32,
        0x00A9 | 0x00AE | 0x203C | 0x2049 | 0x2122 | 0x2139 | 0x2194..=0x21AA | 0x231A..=0x23FF | 0x24C2 | 0x25AA..=0x25FE
        | 0x2600..=0x27BF | 0x2934 | 0x2935 | 0x2B05..=0x2B55 | 0x3030 | 0x303D | 0x3297 | 0x3299
        | 0x1F000..=0x1F1FF | 0x1F300..=0x1FAFF
        | 0xE000..=0xF8FF)
}

fn extender(c: char) -> bool {
    matches!(c as u32, 0xFE0F | 0x20E3 | 0x1F3FB..=0x1F3FF | 0xE0020..=0xE007F)
}

/// UZ: Matnni (boshlanish, tugash, emojimi) bo'laklarga ajratadi; bo'laklar butun matnni qoplaydi.
/// EN: Splits the text into (start, end, is_emoji) pieces; the pieces cover the whole text.
/// RU: Разбивает текст на куски (начало, конец, эмодзи ли); куски покрывают весь текст.
pub fn clusters(chars: &[char]) -> Vec<(usize, usize, bool)> {
    let mut out: Vec<(usize, usize, bool)> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let keycap = matches!(c, '0'..='9' | '#' | '*') && {
            let mut j = i + 1;
            if chars.get(j) == Some(&'\u{FE0F}') {
                j += 1;
            }
            chars.get(j) == Some(&'\u{20E3}')
        };
        if base_emoji(c) || keycap {
            let mut j = i + 1;
            loop {
                while j < chars.len() && extender(chars[j]) {
                    j += 1;
                }
                // UZ: Bayroq: ikkita hudud belgisi. EN: A flag: two regional indicators. RU: Флаг: два региональных индикатора.
                if (0x1F1E6..=0x1F1FF).contains(&(chars[i] as u32)) && j == i + 1 && j < chars.len() && (0x1F1E6..=0x1F1FF).contains(&(chars[j] as u32)) {
                    j += 1;
                    continue;
                }
                if j + 1 < chars.len() && chars[j] == '\u{200D}' && base_emoji(chars[j + 1]) {
                    j += 2;
                    continue;
                }
                break;
            }
            out.push((i, j, true));
            i = j;
        } else {
            let s = i;
            i += 1;
            while i < chars.len() && !base_emoji(chars[i]) && !matches!(chars[i], '0'..='9' | '#' | '*') {
                i += 1;
            }
            // UZ: Raqam/belgi keycap emas edi: oddiy matn sifatida qo'shamiz. EN: The digit/symbol was not a keycap: kept as plain text. RU: Цифра/символ оказались не keycap: оставляем как обычный текст.
            while i < chars.len() && matches!(chars[i], '0'..='9' | '#' | '*') && !clusters_keycap(chars, i) {
                i += 1;
                while i < chars.len() && !base_emoji(chars[i]) && !matches!(chars[i], '0'..='9' | '#' | '*') {
                    i += 1;
                }
            }
            out.push((s, i, false));
        }
    }
    out
}

fn clusters_keycap(chars: &[char], i: usize) -> bool {
    let mut j = i + 1;
    if chars.get(j) == Some(&'\u{FE0F}') {
        j += 1;
    }
    chars.get(j) == Some(&'\u{20E3}')
}

/// UZ: `caret` dan oldingi klasterning boshlanishi (Backspace butun emojini o'chirishi uchun).
/// EN: The start of the cluster before `caret` (so Backspace removes a whole emoji).
/// RU: Начало кластера перед `caret` (чтобы Backspace удалял эмодзи целиком).
pub fn cluster_start_before(chars: &[char], caret: usize) -> usize {
    for (s, e, emoji) in clusters(&chars[..caret.min(chars.len())]) {
        if e == caret && emoji {
            return s;
        }
    }
    caret.saturating_sub(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cl(s: &str) -> Vec<(String, bool)> {
        let v: Vec<char> = s.chars().collect();
        clusters(&v).into_iter().map(|(a, b, e)| (v[a..b].iter().collect(), e)).collect()
    }

    #[test]
    fn klasterlar_togri_ajraladi() {
        assert_eq!(cl("salom"), vec![("salom".to_string(), false)]);
        assert_eq!(cl("a😀b"), vec![("a".into(), false), ("😀".into(), true), ("b".into(), false)]);
        assert_eq!(cl("👍🏽"), vec![("👍🏽".into(), true)]);
        assert_eq!(cl("🇺🇿🇰🇿"), vec![("🇺🇿".into(), true), ("🇰🇿".into(), true)]);
        assert_eq!(cl("👨‍👩‍👧"), vec![("👨‍👩‍👧".into(), true)]);
        assert_eq!(cl("❤️x"), vec![("❤️".into(), true), ("x".into(), false)]);
        assert_eq!(cl("a1b"), vec![("a1b".to_string(), false)]);
        assert_eq!(cl("1️⃣"), vec![("1️⃣".into(), true)]);
        assert_eq!(cl("\u{E001}"), vec![("\u{E001}".into(), true)]);
        assert_eq!(cl("").len(), 0);
    }

    #[test]
    fn orqaga_ochirish_butun_klasterni_oladi() {
        let v: Vec<char> = "ab👍🏽".chars().collect();
        assert_eq!(cluster_start_before(&v, v.len()), 2);
        assert_eq!(cluster_start_before(&v, 2), 1);
    }
}
