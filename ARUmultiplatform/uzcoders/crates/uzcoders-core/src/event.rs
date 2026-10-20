//! Kiritish hodisalari.

use crate::geometry::Point;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerKind {
    Down,
    Move,
    Up,
    Cancel,
}

/// Teginish yoki sichqoncha hodisasi (`id` — ko'p barmoqli tegish uchun).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerEvent {
    pub kind: PointerKind,
    pub pos: Point,
    pub id: u32,
    /// Hodisa vaqti (ms, monoton). 0 — qobiq vaqti ishlatiladi.
    pub time_ms: u64,
}

impl PointerEvent {
    pub fn at(kind: PointerKind, x: f32, y: f32, time_ms: u64) -> Self {
        Self {
            kind,
            pos: Point::new(x, y),
            id: 0,
            time_ms,
        }
    }

    pub fn new(kind: PointerKind, x: f32, y: f32) -> Self {
        Self {
            kind,
            pos: Point::new(x, y),
            id: 0,
            time_ms: 0,
        }
    }
}
