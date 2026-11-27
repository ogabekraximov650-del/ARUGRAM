//! Foydalanuvchi (kanal) kimligi: qurilmada saqlanadi, serverda `users` jadvalida.

#[derive(Clone)]
pub struct Me {
    pub id: String,
    pub name: String,
}

fn path() -> String {
    format!("{}/aru_me.txt", uzcoders::android::files_dir())
}

pub fn load() -> Option<Me> {
    let t = std::fs::read_to_string(path()).ok()?;
    let mut l = t.lines();
    let (id, name) = (l.next()?.trim().to_string(), l.next()?.trim().to_string());
    if id.len() >= 8 && name.chars().count() >= 2 {
        Some(Me { id, name })
    } else {
        None
    }
}

pub fn save(m: &Me) {
    let _ = std::fs::write(path(), format!("{}\n{}\n", m.id, m.name));
}

/// 16 belgili tasodifiy id (vaqt va xotira manzilidan; kriptografik kuch kerak emas).
pub fn new_id() -> String {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(1);
    let x = Box::new(0u8);
    let mut h = (t as u64) ^ ((&*x as *const u8 as u64) << 20) ^ ((t >> 64) as u64);
    h = h.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (h >> 29);
    let g = h.wrapping_mul(0xBF58_476D_1CE4_E5B9) ^ (t as u64).rotate_left(17);
    format!("{:08x}{:08x}", (h >> 16) as u32, (g >> 11) as u32)
}
