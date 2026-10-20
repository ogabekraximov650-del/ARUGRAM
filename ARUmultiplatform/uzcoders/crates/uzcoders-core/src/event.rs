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
}

impl PointerEvent {
    pub fn new(kind: PointerKind, x: f32, y: f32) -> Self {
        Self {
            kind,
            pos: Point::new(x, y),
            id: 0,
        }
    }
}
