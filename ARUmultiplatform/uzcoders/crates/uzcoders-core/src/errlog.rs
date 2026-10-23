//! UZ: Xatolar jurnali: istalgan oqimdan `report_error` bilan yoziladi, oxirgi 100 ta (bir xillari ×N bo'lib yig'iladi), faylga ham saqlanadi.
//! EN: Error log: written from any thread with `report_error`, keeps the last 100 (identical ones are merged as ×N) and is persisted to a file.
//! RU: Журнал ошибок: пишется из любого потока через `report_error`, хранит последние 100 (одинаковые сливаются как ×N) и сохраняется в файл.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_ENTRIES: usize = 100;
const MAX_MSG: usize = 300;

/// UZ: Jurnal yozuvi. EN: A log entry. RU: Запись журнала.
#[derive(Clone, Debug, PartialEq)]
pub struct ErrEntry {
    /// UZ: Oxirgi marta yuz bergan vaqt (UNIX soniya). EN: Last occurrence (UNIX seconds). RU: Время последнего появления (UNIX-секунды).
    pub ts: u64,
    pub place: String,
    pub msg: String,
    pub count: u32,
}

static LOG: Mutex<Vec<ErrEntry>> = Mutex::new(Vec::new());
static PATH: Mutex<Option<String>> = Mutex::new(None);
static TOAST: Mutex<Option<String>> = Mutex::new(None);
static LAST_TOAST: AtomicU64 = AtomicU64::new(0);

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn clip(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// UZ: Jurnal faylini belgilaydi va undagi eski yozuvlarni o'qiydi (ilova ishga tushganda bir marta).
/// EN: Sets the log file and loads the old entries from it (once at app start).
/// RU: Задаёт файл журнала и загружает из него старые записи (один раз при запуске).
pub fn set_error_file(path: &str) {
    *PATH.lock().unwrap() = Some(path.to_string());
    if let Ok(t) = std::fs::read_to_string(path) {
        let mut l = LOG.lock().unwrap();
        for line in t.lines() {
            let mut p = line.splitn(4, '\t');
            if let (Some(ts), Some(c), Some(pl), Some(m)) = (p.next(), p.next(), p.next(), p.next()) {
                if let (Ok(ts), Ok(count)) = (ts.parse(), c.parse()) {
                    l.push(ErrEntry { ts, place: pl.to_string(), msg: m.to_string(), count });
                }
            }
        }
        let n = l.len();
        if n > MAX_ENTRIES {
            l.drain(..n - MAX_ENTRIES);
        }
    }
}

fn save(l: &[ErrEntry]) {
    let Some(p) = PATH.lock().unwrap().clone() else { return };
    let mut out = String::new();
    for e in l {
        out.push_str(&format!("{}\t{}\t{}\t{}\n", e.ts, e.count, e.place.replace(['\t', '\n'], " "), e.msg.replace(['\t', '\n'], " ")));
    }
    let _ = std::fs::write(p, out);
}

/// UZ: Xatoni jurnalga yozadi (istalgan oqimdan). Oxirgi yozuv bilan bir xil bo'lsa, sanagich oshadi.
/// Foydalanuvchiga xabar (toast) UI oqimida `take_error_toast` bilan olinadi (15 soniyada ko'pi bilan bir marta).
/// EN: Writes an error to the log (from any thread). If identical to the latest entry its counter grows.
/// A toast for the user is taken on the UI thread with `take_error_toast` (at most once per 15 seconds).
/// RU: Записывает ошибку в журнал (из любого потока). Если совпадает с последней записью, растёт счётчик.
/// Уведомление для пользователя берётся в UI-потоке через `take_error_toast` (не чаще раза в 15 секунд).
pub fn report_error(place: &str, msg: &str) {
    let (place, msg) = (clip(place, 60), clip(msg, MAX_MSG));
    let t = now();
    let short = format!("Xato: {}: {}", place, clip(&msg, 80));
    {
        let mut l = LOG.lock().unwrap();
        if let Some(i) = l.iter().rposition(|e| e.place == place && e.msg == msg) {
            let mut e = l.remove(i);
            e.count = e.count.saturating_add(1);
            e.ts = t;
            l.push(e);
        } else {
            if l.len() >= MAX_ENTRIES {
                l.remove(0);
            }
            l.push(ErrEntry { ts: t, place, msg, count: 1 });
        }
        save(&l);
    }
    let last = LAST_TOAST.load(Ordering::Relaxed);
    if t >= last + 15 && LAST_TOAST.compare_exchange(last, t, Ordering::Relaxed, Ordering::Relaxed).is_ok() {
        *TOAST.lock().unwrap() = Some(short);
        crate::task::wake();
    }
}

/// UZ: Ko'rsatilishi kutilayotgan xato xabari (UI oqimidan chaqiring; bir marta o'qiladi).
/// EN: The pending error message to show (call from the UI thread; read once).
/// RU: Ожидающее показа сообщение об ошибке (вызывайте из UI-потока; читается один раз).
pub fn take_error_toast() -> Option<String> {
    TOAST.lock().unwrap().take()
}

/// UZ: Jurnal yozuvlari, eng yangisi birinchi. EN: Log entries, newest first. RU: Записи журнала, новые первыми.
pub fn error_entries() -> Vec<ErrEntry> {
    let mut v = LOG.lock().unwrap().clone();
    v.reverse();
    v
}

/// UZ: Jurnalni tozalaydi. EN: Clears the log. RU: Очищает журнал.
pub fn clear_errors() {
    let mut l = LOG.lock().unwrap();
    l.clear();
    save(&l);
}

/// UZ: Jurnalning nusxa olish uchun matni (vaqt UTC+5, Toshkent).
/// EN: The log as copyable text (time in UTC+5, Tashkent).
/// RU: Журнал в виде текста для копирования (время UTC+5, Ташкент).
pub fn errors_text() -> String {
    let mut out = String::new();
    for e in error_entries() {
        let s = (e.ts + 5 * 3600) % 86400;
        out.push_str(&format!("[{:02}:{:02}:{:02}] {}: {}", s / 3600, s % 3600 / 60, s % 60, e.place, e.msg));
        if e.count > 1 {
            out.push_str(&format!(" (×{})", e.count));
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bir_xil_xatolar_yigiladi_va_chegara_saqlanadi() {
        clear_errors();
        for _ in 0..5 {
            report_error("api", "xato A");
        }
        for i in 0..150 {
            report_error("x", &format!("m{i}"));
        }
        let v = error_entries();
        assert_eq!(v.len(), MAX_ENTRIES);
        assert_eq!(v[0].msg, "m149");
        clear_errors();
        report_error("api", "A");
        report_error("api", "A");
        assert_eq!(error_entries()[0].count, 2);
        assert!(errors_text().contains("api: A (×2)"));
        clear_errors();
    }
}
