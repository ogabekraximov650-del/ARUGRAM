//! Server ma'lumotlari va ularni ko'rsatish yordamchilari.

use crate::json::J;

#[derive(Clone, Default)]
pub struct Video {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub uid: String,
    pub channel: String,
    pub size: i64,
    pub duration: i64,
    pub views: i64,
    pub created: i64,
    pub has_thumb: bool,
    pub likes: i64,
    pub notes: i64,
    pub liked: bool,
}

impl Video {
    pub fn from(j: &J) -> Video {
        Video {
            id: j.s("id"),
            title: j.s("title"),
            kind: j.s("kind"),
            uid: j.s("uid"),
            channel: j.s("channel"),
            size: j.n("size"),
            duration: j.n("duration"),
            views: j.n("views"),
            created: j.n("created"),
            has_thumb: j.b("has_thumb"),
            likes: j.n("likes"),
            notes: j.n("notes"),
            liked: j.b("liked"),
        }
    }
}

#[derive(Clone, Default)]
pub struct Note {
    pub at: i64,
    pub body: String,
    pub name: String,
}

impl Note {
    pub fn from(j: &J) -> Note {
        Note { at: j.n("at"), body: j.s("body"), name: j.s("name") }
    }
}

#[derive(Clone, Default)]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub videos: i64,
    pub views: i64,
    pub followers: i64,
    pub following: bool,
}

impl Channel {
    pub fn from(j: &J) -> Channel {
        Channel { id: j.s("id"), name: j.s("name"), videos: j.n("videos"), views: j.n("views"), followers: j.n("followers"), following: j.b("following") }
    }
}

/// `1 284 512` -> `1,3 mln`, `12 400` -> `12,4 ming`.
pub fn compact(n: i64) -> String {
    let cut = |v: f64| format!("{v:.1}").replace('.', ",").trim_end_matches(",0").to_string();
    if n < 1000 {
        n.to_string()
    } else if n < 1_000_000 {
        format!("{} ming", cut(n as f64 / 1000.0))
    } else {
        format!("{} mln", cut(n as f64 / 1_000_000.0))
    }
}

/// Davomiylik: `24:01` yoki `1:02:03`.
pub fn dur(secs: i64) -> String {
    let s = secs.max(0);
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s % 3600 / 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// "3 kun oldin" ko'rinishi.
pub fn ago(created: i64, now: i64) -> String {
    let s = ((now - created) / 1000).max(0);
    let (n, unit) = match s {
        0..=59 => return "hozirgina".into(),
        60..=3599 => (s / 60, "daqiqa"),
        3600..=86399 => (s / 3600, "soat"),
        86400..=2_591_999 => (s / 86400, "kun"),
        2_592_000..=31_535_999 => (s / 2_592_000, "oy"),
        _ => (s / 31_536_000, "yil"),
    };
    format!("{n} {unit} oldin")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatlar() {
        assert_eq!(compact(999), "999");
        assert_eq!(compact(12_400), "12,4 ming");
        assert_eq!(compact(2_000_000), "2 mln");
        assert_eq!(dur(1441), "24:01");
        assert_eq!(dur(3723), "1:02:03");
        assert_eq!(ago(0, 30_000), "hozirgina");
        assert_eq!(ago(0, 3 * 86_400_000), "3 kun oldin");
        assert_eq!(ago(0, 90_000), "1 daqiqa oldin");
    }
}
