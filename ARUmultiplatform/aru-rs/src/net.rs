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

/// Bloklovchi so'rov (faqat fon oqimida chaqiring: `spawn_bg`).
pub fn call(method: &str, path: &str, body: &str) -> Result<J, String> {
    #[cfg(test)]
    let mock = *MOCK.lock().unwrap_or_else(|e| e.into_inner());
    #[cfg(test)]
    if let Some(f) = mock {
        let (c, t) = f(method, path, body);
        return parse(c, &t);
    }
    let spec = format!("{method}\n{BASE}{path}\n{TOKEN}\n{body}");
    let r = uzcoders::android::activity_call_ret("http", &spec);
    let (c, t) = r.split_once('\n').unwrap_or(("0", ""));
    parse(c.trim().parse().unwrap_or(0), t)
}
