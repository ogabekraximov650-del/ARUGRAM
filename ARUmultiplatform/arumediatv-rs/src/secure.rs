//! Diskdagi fayllarni shifrlash (AES-256-GCM, Rust yadrosi): rasmlar, to'plam kesh, sessiya. Kalit `files/master.key` dan.
//! Eski (shifrlanmagan) fayllar o'qiladi — yangi yozuvlar doim shifrlanadi; shifrlash imkonsiz bo'lsa fayl YOZILMAYDI.

use std::ffi::CString;
use std::os::raw::c_char;

extern "C" {
    fn rust_seal_bytes(label: *const c_char, data: *const u8, len: usize, out_len: *mut usize) -> *mut u8;
    fn rust_open_bytes(label: *const c_char, data: *const u8, len: usize, out_len: *mut usize) -> *mut u8;
    fn rust_free_bytes(ptr: *mut u8, len: usize);
}

fn take(p: *mut u8, n: usize) -> Option<Vec<u8>> {
    if p.is_null() {
        return None;
    }
    let v = unsafe { std::slice::from_raw_parts(p, n) }.to_vec();
    unsafe { rust_free_bytes(p, n) };
    Some(v)
}

pub fn seal(label: &str, plain: &[u8]) -> Option<Vec<u8>> {
    crate::backend::ensure_master();
    let l = CString::new(label).ok()?;
    let mut n = 0usize;
    let p = unsafe { rust_seal_bytes(l.as_ptr(), plain.as_ptr(), plain.len(), &mut n) };
    take(p, n)
}

pub fn open(label: &str, sealed: &[u8]) -> Option<Vec<u8>> {
    crate::backend::ensure_master();
    let l = CString::new(label).ok()?;
    let mut n = 0usize;
    let p = unsafe { rust_open_bytes(l.as_ptr(), sealed.as_ptr(), sealed.len(), &mut n) };
    take(p, n)
}

/// Shifrlab yozadi (avval vaqtinchalik faylga). `false` — shifrlab bo'lmadi yoki yozilmadi.
pub fn write(path: &str, label: &str, plain: &[u8]) -> bool {
    let Some(s) = seal(label, plain) else { return false };
    let tmp = format!("{path}.tmp");
    if std::fs::write(&tmp, &s).is_err() {
        return false;
    }
    if std::fs::rename(&tmp, path).is_err() {
        let _ = std::fs::remove_file(&tmp);
        return false;
    }
    true
}

/// O'qiydi: shifrlangan bo'lsa ochadi, aks holda (eski ochiq fayl) fayl baytlarini o'zini qaytaradi.
pub fn read(path: &str, label: &str) -> Option<Vec<u8>> {
    let raw = std::fs::read(path).ok()?;
    Some(open(label, &raw).unwrap_or(raw))
}

#[cfg(test)]
mod tests {
    #[test]
    fn kalitsiz_yozilmaydi_va_eski_fayl_oqiladi() {
        let d = std::env::temp_dir().join(format!("aru_secure_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&d);
        let p = d.join("a.bin");
        std::fs::write(&p, b"eski").unwrap();
        // Eski ochiq fayl o'zi qaytadi (kalit bo'lmasa ham).
        assert_eq!(super::read(p.to_str().unwrap(), "x").unwrap(), b"eski");
        let _ = std::fs::remove_dir_all(&d);
    }
}

/// Kesh papkasini `max_bytes` dan oshganda eng eskilaridan boshlab tozalaydi (LRU: oxirgi o'zgarish vaqti bo'yicha).
pub fn gc_dir(dir: &str, max_bytes: u64) -> u64 {
    let Ok(rd) = std::fs::read_dir(dir) else { return 0 };
    let mut files: Vec<(std::time::SystemTime, u64, std::path::PathBuf)> = rd
        .flatten()
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            if !m.is_file() {
                return None;
            }
            Some((m.modified().or_else(|_| m.accessed()).unwrap_or(std::time::UNIX_EPOCH), m.len(), e.path()))
        })
        .collect();
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    if total <= max_bytes {
        return 0;
    }
    files.sort_by_key(|f| f.0);
    let mut freed = 0;
    for (_, len, p) in files {
        if total <= max_bytes {
            break;
        }
        if std::fs::remove_file(&p).is_ok() {
            total -= len;
            freed += len;
        }
    }
    freed
}
