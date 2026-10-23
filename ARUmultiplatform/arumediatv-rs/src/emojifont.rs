//! Telegram uslubidagi emoji shrifti (`TgEmoji.ttf`). Fayl yopiq kanalda turadi (worker `/api/emojifont`), ilova uni
//! bir marta yuklab `files/emoji_font/<versiya>.ttf` ga saqlaydi (Kotlin `EmojiFonts`). Yuklanguncha tizim emojisi ishlatiladi.

use std::sync::Mutex;

static PATH: Mutex<Option<String>> = Mutex::new(None);
static BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn dir() -> String {
    let d = format!("{}/emoji_font", uzcoders::android::files_dir());
    let _ = std::fs::create_dir_all(&d);
    d
}

fn ver_of(name: &str) -> Option<u64> {
    name.strip_suffix(".ttf")?.parse().ok()
}

fn looks_like_font(b: &[u8]) -> bool {
    b.len() > 100_000 && matches!(&b[..4], [0, 1, 0, 0] | b"true" | b"OTTO" | b"ttcf")
}

/// Diskdagi eng yangi shrift (bo'lsa).
fn latest() -> Option<(u64, String)> {
    std::fs::read_dir(dir()).ok()?.flatten().filter_map(|e| {
        let n = e.file_name().to_string_lossy().into_owned();
        ver_of(&n).map(|v| (v, e.path().to_string_lossy().into_owned()))
    }).max()
}

/// Shrift fayli yo'li (matn rasmlariga beriladi). Bo'sh — tizim shrifti.
pub fn path() -> String {
    let mut g = PATH.lock().unwrap_or_else(|e| e.into_inner());
    if g.is_none() {
        *g = latest().map(|(_, p)| p);
    }
    g.clone().unwrap_or_default()
}

/// Fonda: shrift yo'q yoki eski bo'lsa kanaldan yuklaydi. `true` — yangi shrift tayyor bo'ldi.
pub fn ensure(session: &str) -> bool {
    use std::sync::atomic::Ordering;
    if BUSY.swap(true, Ordering::SeqCst) {
        return false;
    }
    let ok = ensure_inner(session);
    BUSY.store(false, Ordering::SeqCst);
    ok
}

fn ensure_inner(session: &str) -> bool {
    for attempt in 1..=4u64 {
        let Ok(j) = crate::api::get_json("/api/emojifont", "", Some(session)) else { return false };
        if j["missing"].as_bool() == Some(true) {
            std::thread::sleep(std::time::Duration::from_secs(4 * attempt));
            continue;
        }
        let (ver, name) = (j["ver"].as_u64().unwrap_or(0), j["name"].as_str().unwrap_or("").to_string());
        if ver == 0 || name.is_empty() {
            return false;
        }
        if latest().map(|(v, _)| v) == Some(ver) {
            return false;
        }
        let got = (|| {
            let ok = crate::data::tg_deliver(session, &[name.clone()]).ok()?;
            if !ok.contains(&name) {
                return None;
            }
            let port = crate::data::tg_port();
            for _ in 0..20 {
                let r = ureq::get(&format!("http://127.0.0.1:{port}/tg/0/{name}")).timeout(std::time::Duration::from_secs(120)).call();
                if let Ok(r) = r {
                    let mut b = Vec::new();
                    std::io::Read::read_to_end(&mut std::io::Read::take(r.into_reader(), 24 << 20), &mut b).ok()?;
                    return Some(b);
                }
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
            None
        })();
        if let Some(b) = got.filter(|b| looks_like_font(b)) {
            let f = format!("{}/{ver}.ttf", dir());
            if std::fs::write(&f, &b).is_ok() {
                if let Ok(rd) = std::fs::read_dir(dir()) {
                    for e in rd.flatten() {
                        if e.path().to_string_lossy() != f {
                            let _ = std::fs::remove_file(e.path());
                        }
                    }
                }
                *PATH.lock().unwrap_or_else(|e| e.into_inner()) = Some(f);
                return true;
            }
        }
        let _ = crate::api::request("POST", "/api/emojifont/fail", "", Some(&serde_json::json!({ "ver": ver })), Some(session));
        std::thread::sleep(std::time::Duration::from_secs(2 * attempt));
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versiya_nomdan_olinadi() {
        assert_eq!(ver_of("12.ttf"), Some(12));
        assert_eq!(ver_of("a.ttf"), None);
        assert!(!looks_like_font(b"short"));
    }
}
