//! Android bo'lmagan muhitda pleyer yo'q: har doim xato holati.

#[derive(Clone)]
pub enum Source {
    Aru(String),
    Http(String),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum State {
    Opening,
    Ready,
    Ended,
    Error,
}

#[derive(Clone)]
pub struct Status {
    pub state: State,
    pub pos_ms: i64,
    pub dur_ms: i64,
    pub playing: bool,
    pub buffering: bool,
    pub error: String,
    pub width: i32,
    pub height: i32,
    pub speed: f32,
}

pub struct Player;

impl Player {
    pub fn open(_source: Source, _start_ms: i64, _auto_play: bool) -> Player {
        Player
    }
    pub fn status(&self) -> Status {
        Status { state: State::Error, pos_ms: 0, dur_ms: 0, playing: false, buffering: false, error: "Pleyer faqat telefonda ishlaydi".into(), width: 0, height: 0, speed: 1.0 }
    }
    pub fn play(&self) {}
    pub fn set_speed(&self, _x: f32) {}
    pub fn pause(&self) {}
    pub fn seek_ms(&self, _ms: i64) {}
    pub fn close(&self) {}
}
