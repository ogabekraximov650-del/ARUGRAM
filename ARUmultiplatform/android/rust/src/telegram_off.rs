//! UZ: `telegram` xususiyati O'CHIQ bo'lganda (Rust ilova): MTProto yo'q, bu funksiyalar pleyer/keshga "Telegram yo'li yo'q" deydi.
//! EN: stand-in used when the `telegram` feature is off: there is no MTProto path, callers fall back to the worker.
//! RU: заглушка при выключенной функции `telegram`: пути MTProto нет, вызывающие переходят на worker.

pub(crate) const NET_ERR: &str = "tarmoq: ";

pub(crate) fn is_net_err(e: &str) -> bool {
    e.starts_with(NET_ERR)
}

pub(crate) fn mime_of_name(name: &str) -> &'static str {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "mp4" => "video/mp4",
        "mkv" => "video/x-matroska",
        "m4a" => "audio/mp4",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    }
}

pub fn route_url(_key: &str) -> Option<String> {
    None
}

pub fn is_origin_url(_url: &str) -> bool {
    false
}

pub(crate) fn origin_name(_url: &str) -> Option<String> {
    None
}

pub(crate) fn note_failure(_key: &str) {}

pub(crate) fn doc_size(_name: &str) -> Result<(u64, String), String> {
    Err("Telegram yo'li o'chirilgan".into())
}

pub(crate) fn cached_doc_size(_name: &str) -> Option<(u64, String)> {
    None
}

pub(crate) fn fetch_range(_name: &str, _offset: u64, _len: u64) -> Result<Vec<u8>, String> {
    Err("Telegram yo'li o'chirilgan".into())
}
