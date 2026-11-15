// UZ: O'zgarish belgilari markazi (Cloudflare Durable Object `MarkHub` o'rnida, Ample'da): jadval nomi -> eng so'nggi o'zgarish vaqti (ms).
//     `POST /marks` (belgilash + o'qish), `POST /wait` (long-poll: o'zgarguncha ~25 s kutadi). Belgilar faqat xotirada: ishga tushganda ko'rilmagan kalit
//     "boot vaqti" deb hisoblanadi (eskirgan ilova bir marta yangilanadi — o'zgarish o'tkazib yuborilmaydi). Kalit `/setkey` bilan BIR MARTA o'rnatiladi.
// EN: Change-marks hub (on Ample, replacing the Cloudflare Durable Object `MarkHub`): table name -> latest change time (ms).
//     `POST /marks` (bump + read), `POST /wait` (long-poll until something changes, ~25 s). Marks live in memory only: after a restart an unseen key reads as the
//     boot time (a stale app refreshes once — no change is missed). The key is set ONCE via `/setkey`.
// RU: Центр отметок изменений (на Ample, вместо Durable Object `MarkHub` в Cloudflare): имя таблицы -> время последнего изменения (мс).
//     `POST /marks` (отметить + прочитать), `POST /wait` (long-poll до изменения, ~25 с). Отметки только в памяти: после перезапуска неизвестный ключ = время запуска
//     (устаревшее приложение обновится один раз — изменения не теряются). Ключ задаётся ОДИН раз через `/setkey`.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::Notify;

pub struct Hub {
    boot: i64,
    marks: Mutex<HashMap<String, i64>>,
    notify: Notify,
    key_hash: Mutex<Option<String>>,
    key_file: PathBuf,
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn sha_hex(s: &str) -> String {
    Sha256::digest(s.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

fn ct_eq(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}

impl Hub {
    pub fn new(key_file: PathBuf) -> Self {
        let saved = std::fs::read_to_string(&key_file).ok().map(|t| t.trim().to_string()).filter(|t| t.len() == 64);
        Hub { boot: now_ms(), marks: Mutex::new(HashMap::new()), notify: Notify::new(), key_hash: Mutex::new(saved), key_file }
    }

    /// UZ: kalitni BIR MARTA o'rnatadi (>= 32 belgi). `false` — allaqachon o'rnatilgan yoki yaroqsiz. EN: sets the key ONCE.
    pub fn set_key(&self, key: &str) -> bool {
        if key.len() < 32 {
            return false;
        }
        let mut g = self.key_hash.lock().unwrap_or_else(|e| e.into_inner());
        if g.is_some() {
            return false;
        }
        let h = sha_hex(key);
        if let Some(dir) = self.key_file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&self.key_file, &h);
        *g = Some(h);
        true
    }

    pub fn has_key(&self) -> bool {
        self.key_hash.lock().unwrap_or_else(|e| e.into_inner()).is_some()
    }

    pub fn authorized(&self, key: &str) -> bool {
        match self.key_hash.lock().unwrap_or_else(|e| e.into_inner()).as_deref() {
            Some(h) => ct_eq(h, &sha_hex(key)),
            None => false,
        }
    }

    /// UZ: ko'rilmagan kalit qiymati: jadval belgilari uchun ishga tushish vaqti (hub qayta ishga tushsa keshlar eskirgan hisoblansin);
    /// `vddel_` ("fayl o'chirilgan") belgilari uchun 0 — aks holda har bir fayl "o'chirilgan" bo'lib ko'rinardi.
    /// EN: value of an unseen key: boot time for table marks; 0 for `vddel_` tombstones (otherwise every file would look deleted).
    fn base(&self, k: &str) -> i64 {
        // Worker kalitni `t:` old qo'shimchasi bilan yuboradi (`t:vddel_<nom>`), shu sabab ikkala ko'rinish tekshiriladi.
        if k.starts_with("vddel_") || k.starts_with("t:vddel_") { 0 } else { self.boot }
    }

    fn value(&self, m: &HashMap<String, i64>, k: &str) -> i64 {
        m.get(k).copied().unwrap_or_else(|| self.base(k))
    }

    /// UZ: `bump` kalitlarini `at` ga ko'taradi (kamaymaydi) va `get` kalitlari qiymatini qaytaradi. EN: raises `bump` keys to `at` (never lowers) and returns `get` values.
    pub fn marks(&self, bump: &[String], at: i64, get: &[String]) -> Value {
        let mut m = self.marks.lock().unwrap_or_else(|e| e.into_inner());
        for k in bump {
            let e = m.entry(k.clone()).or_insert_with(|| self.base(k));
            if at > *e {
                *e = at;
            }
        }
        let v: serde_json::Map<String, Value> = get.iter().map(|k| (k.clone(), json!(self.value(&m, k)))).collect();
        drop(m);
        if !bump.is_empty() {
            self.notify.notify_waiters();
        }
        json!({"v": v})
    }

    fn max_of(&self, keys: &[String]) -> i64 {
        let m = self.marks.lock().unwrap_or_else(|e| e.into_inner());
        keys.iter().map(|k| self.value(&m, k)).max().unwrap_or(self.boot)
    }

    /// UZ: `keys` ichidagi eng katta belgi `since` dan farq qilguncha (yoki `timeout`) kutadi. EN: waits until the max mark differs from `since` (or `timeout`).
    pub async fn wait(&self, keys: &[String], since: i64, timeout: Duration) -> Value {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            // `notified()` bekor qilinmasdan oldin yaratiladi: tekshiruv va kutish orasidagi o'zgarish yo'qolmaydi.
            let n = self.notify.notified();
            tokio::pin!(n);
            n.as_mut().enable();
            let max = self.max_of(keys);
            if max != since {
                return json!({"max": max, "changed": true});
            }
            if tokio::time::timeout_at(deadline, n).await.is_err() {
                return json!({"max": self.max_of(keys), "changed": false});
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn hub() -> Hub {
        Hub::new(std::env::temp_dir().join(format!("hubkey-{}-{}", std::process::id(), now_ms())))
    }

    #[test]
    fn key_is_set_once_and_checked() {
        let h = hub();
        assert!(!h.authorized("x") && !h.has_key());
        assert!(!h.set_key("short"));
        let k = "k".repeat(64);
        assert!(h.set_key(&k));
        assert!(!h.set_key(&"z".repeat(64)), "ikkinchi marta o'rnatib bo'lmaydi");
        assert!(h.authorized(&k) && !h.authorized("boshqa"));
        // qayta ishga tushganda fayldan tiklanadi
        let h2 = Hub::new(h.key_file.clone());
        assert!(h2.authorized(&k));
        let _ = std::fs::remove_file(&h.key_file);
    }

    #[test]
    fn bump_get_never_lowers_and_unseen_is_boot() {
        let h = hub();
        let boot = h.boot;
        let r = h.marks(&[], 0, &["t:a".into()]);
        assert_eq!(r["v"]["t:a"], boot);
        h.marks(&["t:a".into()], boot + 500, &[]);
        h.marks(&["t:a".into()], boot + 100, &[]);
        assert_eq!(h.marks(&[], 0, &["t:a".into()])["v"]["t:a"], boot + 500);
    }

    #[test]
    fn unseen_tombstone_is_zero() {
        let h = hub();
        assert_eq!(h.marks(&[], 0, &["vddel_a.jpg".into()])["v"]["vddel_a.jpg"], 0);
        h.marks(&["vddel_a.jpg".into()], h.boot + 10, &[]);
        assert_eq!(h.marks(&[], 0, &["vddel_a.jpg".into()])["v"]["vddel_a.jpg"], h.boot + 10);
        assert_eq!(h.marks(&[], 0, &["vddel_b.jpg".into()])["v"]["vddel_b.jpg"], 0);
        // Worker haqiqatda yuboradigan kalit ko'rinishi.
        assert_eq!(h.marks(&[], 0, &["t:vddel_c.jpg".into()])["v"]["t:vddel_c.jpg"], 0);
        assert_eq!(h.marks(&[], 0, &["t:anime_db".into()])["v"]["t:anime_db"], h.boot);
    }

    #[tokio::test]
    async fn wait_wakes_on_bump_and_times_out() {
        let h = Arc::new(hub());
        let keys = vec!["t:a".to_string()];
        let since = h.max_of(&keys);
        let (h2, k2) = (Arc::clone(&h), keys.clone());
        let t = tokio::spawn(async move { h2.wait(&k2, since, Duration::from_secs(5)).await });
        tokio::time::sleep(Duration::from_millis(50)).await;
        h.marks(&keys, since + 1000, &[]);
        let r = t.await.unwrap();
        assert_eq!(r["changed"], true);
        assert_eq!(r["max"], since + 1000);
        // o'zgarish yo'q — vaqt tugaydi
        let r = h.wait(&keys, since + 1000, Duration::from_millis(80)).await;
        assert_eq!(r["changed"], false);
        // allaqachon farq qiladi — darhol
        assert_eq!(h.wait(&keys, 1, Duration::from_secs(5)).await["changed"], true);
    }
}
