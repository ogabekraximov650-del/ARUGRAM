//! Tarmoq: so'rovlar Java tomonda (`AruActivity.http`) bajariladi, shu yerda faqat tayyorlash va javobni tahlil qilish.

use crate::json::J;

pub const BASE: &str = "https://aru-api.uzcom.workers.dev";
/// Yozish so'rovlari tokeni: CI `ARU_APP_TOKEN` bilan beradi (kodga yozilmaydi).
pub const TOKEN: &str = match option_env!("ARU_APP_TOKEN") {
    Some(t) => t,
    None => "",
};

pub fn url(path: &str) -> String {
    format!("{BASE}{path}")
}

/// Faqat testlar uchun: tarmoq o'rniga sinov javobi (so'rovlar fon oqimlarida bajariladi, shuning uchun umumiy).
#[cfg(test)]
pub static MOCK: std::sync::Mutex<Option<fn(&str, &str, &str) -> (u16, String)>> = std::sync::Mutex::new(None);

fn parse(code: u16, text: &str) -> Result<J, String> {
    if code == 0 {
        return Err("Internet yo'q".into());
    }
    let j = J::parse(text).unwrap_or(J::Null);
    if (200..300).contains(&code) {
        Ok(j)
    } else {
        let e = j.s("error");
        Err(if e.is_empty() { format!("Xato {code}") } else { e })
    }
}

static LAST: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

/// Oxirgi so'rov natijasi (ekranda tashxis uchun ko'rsatiladi).
pub fn last() -> String {
    LAST.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

fn note(method: &str, path: &str, code: &str, bytes: usize, t0: std::time::Instant) {
    let p: String = path.split('?').next().unwrap_or("").chars().take(30).collect();
    *LAST.lock().unwrap_or_else(|e| e.into_inner()) = format!("{method} {p} -> {code} · {bytes} bayt · {} ms", t0.elapsed().as_millis());
}

/// Bloklovchi so'rov (faqat fon oqimida chaqiring: `spawn_bg`).
pub fn call(method: &str, path: &str, body: &str) -> Result<J, String> {
    call_text(method, path, body).map(|(j, _)| j)
}

/// `call`, lekin javob matnini ham qaytaradi (diskka saqlash uchun).
pub fn call_text(method: &str, path: &str, body: &str) -> Result<(J, String), String> {
    #[cfg(test)]
    let mock = *MOCK.lock().unwrap_or_else(|e| e.into_inner());
    #[cfg(test)]
    if let Some(f) = mock {
        let (c, t) = f(method, path, body);
        return parse(c, &t).map(|j| (j, t));
    }
    let t0 = std::time::Instant::now();
    let spec = format!("{method}\n{BASE}{path}\n{TOKEN}\n{body}");
    let r = uzcoders::android::activity_call_ret("http", &spec);
    if r.is_empty() {
        note(method, path, "Java javob bermadi", 0, t0);
        return Err("Ilova ichki xatosi: tarmoq qatlami javob bermadi".into());
    }
    let (c, t) = r.split_once('\n').unwrap_or(("0", ""));
    note(method, path, c.trim(), t.len(), t0);
    parse(c.trim().parse().unwrap_or(0), t).map(|j| (j, t.to_string()))
}
