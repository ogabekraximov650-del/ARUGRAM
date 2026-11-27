//! Pleyer va yuklash: Java tomondagi `AruActivity` bilan gaplashadi (MediaPlayer, bo'lakli yuklash).

use crate::net;

#[derive(Clone, Default)]
pub struct Status {
    pub ready: bool,
    pub pos_ms: i64,
    pub dur_ms: i64,
    pub playing: bool,
    pub buffering: bool,
    pub w: i64,
    pub h: i64,
    pub ended: bool,
    pub err: String,
}

pub fn status() -> Status {
    let r = uzcoders::android::activity_call_ret("playStatus", "");
    let p: Vec<&str> = r.split('|').collect();
    if p.len() < 9 {
        return Status::default();
    }
    let n = |i: usize| p[i].trim().parse::<i64>().unwrap_or(0);
    Status { ready: n(0) == 2, pos_ms: n(1), dur_ms: n(2), playing: n(3) == 1, buffering: n(4) == 1, w: n(5), h: n(6), ended: n(7) == 1, err: p[8..].join("|") }
}

pub fn open(id: &str, total: i64, start_ms: i64, auto: bool) {
    uzcoders::android::activity_call("playOpen", &format!("{}|{}|{}|{}", net::url(&format!("/api/video/{id}/stream")), total, start_ms, auto as i32));
}

pub fn cmd(c: &str) {
    uzcoders::android::activity_call("playCmd", c);
}

#[derive(Clone, Default, PartialEq)]
pub struct Up {
    /// idle | run | done | error
    pub state: String,
    pub sent: i64,
    pub total: i64,
    pub err: String,
}

pub fn up_status() -> Up {
    let r = uzcoders::android::activity_call_ret("uploadStatus", "");
    let p: Vec<&str> = r.split('|').collect();
    if p.len() < 4 {
        return Up::default();
    }
    Up { state: p[0].to_string(), sent: p[1].trim().parse().unwrap_or(0), total: p[2].trim().parse().unwrap_or(0), err: p[3].to_string() }
}

pub fn upload(uid: &str, kind: &str, title: &str, path: &str) {
    let t = title.replace('\n', " ");
    uzcoders::android::activity_call("upload", &format!("{}\n{}\n{}\n{}\n{}\n{}", net::BASE, net::TOKEN, uid, kind, t, path));
}

pub fn upload_cancel() {
    uzcoders::android::activity_call("uploadCancel", "");
}
