//! ARUTUBE Rust yadrosi (`libtube_core.so`): tarmoq va fayl yuklash. Kotlin JNA orqali chaqiradi.
//! Hamma chaqiruv BLOKLAYDI — faqat `Dispatchers.IO` da. Natija — JSON matn (`tube_free` bilan bo'shatiladi).
use std::ffi::{CStr, CString};
use std::fs::File;
use std::io::{Cursor, Read};
use std::os::raw::c_char;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn cstr(p: *const c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
}

fn out(s: String) -> *mut c_char {
    CString::new(s.replace('\0', " ")).unwrap_or_default().into_raw()
}

pub fn err_json(m: &str) -> String {
    serde_json::json!({ "error": m }).to_string()
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(20))
        .timeout_read(Duration::from_secs(300))
        .timeout_write(Duration::from_secs(900))
        .build()
}

/// Oddiy JSON so'rov (worker bilan gaplashish). `body` bo'sh bo'lmasa — JSON tana.
pub fn call(method: &str, url: &str, token: &str, body: &str) -> String {
    let mut rq = agent().request(method, url);
    if !token.is_empty() {
        rq = rq.set("X-App-Token", token);
    }
    let res = if body.is_empty() {
        rq.call()
    } else {
        rq.set("Content-Type", "application/json").send_string(body)
    };
    match res {
        Ok(r) => r.into_string().unwrap_or_else(|e| err_json(&e.to_string())),
        Err(ureq::Error::Status(c, r)) => {
            let t = r.into_string().unwrap_or_default();
            if t.trim_start().starts_with('{') {
                t
            } else {
                err_json(&format!("HTTP {c}"))
            }
        }
        Err(e) => err_json(&e.to_string()),
    }
}

/// multipart/form-data boshi: `api_key` maydoni va `file` maydonining sarlavhasi.
pub fn multipart_head(boundary: &str, api_key: &str, filename: &str) -> String {
    let safe: String = filename.chars().filter(|c| *c != '"' && *c != '\r' && *c != '\n').collect();
    format!(
        "--{b}\r\nContent-Disposition: form-data; name=\"api_key\"\r\n\r\n{k}\r\n--{b}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{f}\"\r\nContent-Type: application/octet-stream\r\n\r\n",
        b = boundary,
        k = api_key,
        f = safe
    )
}

pub fn multipart_tail(boundary: &str) -> String {
    format!("\r\n--{boundary}--\r\n")
}

type ProgressFn = Option<extern "C" fn(u64, u64)>;

struct Prog<R: Read> {
    inner: R,
    done: u64,
    total: u64,
    last: u64,
    cb: ProgressFn,
}

impl<R: Read> Read for Prog<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.done += n as u64;
        if let Some(cb) = self.cb {
            // Kotlin'ga har ~256 KB da bir marta xabar (har o'qishda emas).
            if n == 0 || self.done - self.last >= 256 * 1024 {
                self.last = self.done;
                cb(self.done, self.total);
            }
        }
        Ok(n)
    }
}

/// Faylni DoodStream yuklash serveriga oqim bilan yuboradi (xotiraga to'liq o'qilmaydi). `file` — ochiq fayl.
pub fn upload(url: &str, api_key: &str, file: File, filename: &str, cb: ProgressFn) -> String {
    let total = match file.metadata() {
        Ok(m) => m.len(),
        Err(e) => return err_json(&e.to_string()),
    };
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(1);
    let boundary = format!("----arutube{nanos:x}");
    let head = multipart_head(&boundary, api_key, filename).into_bytes();
    let tail = multipart_tail(&boundary).into_bytes();
    let len = head.len() as u64 + total + tail.len() as u64;
    let body = Cursor::new(head)
        .chain(Prog { inner: file, done: 0, total, last: 0, cb })
        .chain(Cursor::new(tail));
    let res = agent()
        .post(url)
        .set("Content-Type", &format!("multipart/form-data; boundary={boundary}"))
        .set("Content-Length", &len.to_string())
        .send(body);
    match res {
        Ok(r) => r.into_string().unwrap_or_else(|e| err_json(&e.to_string())),
        Err(ureq::Error::Status(c, r)) => {
            let t = r.into_string().unwrap_or_default();
            if t.trim_start().starts_with('{') {
                t
            } else {
                err_json(&format!("DoodStream HTTP {c}"))
            }
        }
        Err(e) => err_json(&e.to_string()),
    }
}

// ── C ABI (JNA) ─────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn tube_version() -> *mut c_char {
    out(env!("CARGO_PKG_VERSION").to_string())
}

#[no_mangle]
pub extern "C" fn tube_free(p: *mut c_char) {
    if !p.is_null() {
        drop(unsafe { CString::from_raw(p) });
    }
}

#[no_mangle]
pub extern "C" fn tube_call(method: *const c_char, url: *const c_char, token: *const c_char, body: *const c_char) -> *mut c_char {
    out(call(&cstr(method), &cstr(url), &cstr(token), &cstr(body)))
}

/// `fd` — Kotlin `detachFd()` bergan fayl deskriptori; yadro uni yopadi.
#[no_mangle]
pub extern "C" fn tube_upload(url: *const c_char, api_key: *const c_char, fd: i32, filename: *const c_char, cb: ProgressFn) -> *mut c_char {
    use std::os::unix::io::FromRawFd;
    if fd < 0 {
        return out(err_json("fayl ochilmadi"));
    }
    let file = unsafe { File::from_raw_fd(fd) };
    out(upload(&cstr(url), &cstr(api_key), file, &cstr(filename), cb))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multipart_tuzilishi() {
        let h = multipart_head("B", "KEY", "a\"b.mp4");
        assert!(h.starts_with("--B\r\n"));
        assert!(h.contains("name=\"api_key\"\r\n\r\nKEY\r\n"));
        assert!(h.contains("filename=\"ab.mp4\""));
        assert!(h.ends_with("\r\n\r\n"));
        assert_eq!(multipart_tail("B"), "\r\n--B--\r\n");
    }

    #[test]
    fn xato_json() {
        assert_eq!(err_json("x"), "{\"error\":\"x\"}");
    }

    #[test]
    fn progress_hisoblaydi() {
        let mut p = Prog { inner: Cursor::new(vec![0u8; 1000]), done: 0, total: 1000, last: 0, cb: None };
        let mut b = Vec::new();
        p.read_to_end(&mut b).unwrap();
        assert_eq!(p.done, 1000);
    }

    #[test]
    fn bosh_url_xatosi_json_qaytaradi() {
        let r = call("GET", "http://127.0.0.1:1/x", "", "");
        assert!(r.contains("error"));
    }
}
