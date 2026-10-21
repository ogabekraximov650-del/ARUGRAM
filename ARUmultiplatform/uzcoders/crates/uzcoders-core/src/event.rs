//! UZ: Kiritish hodisalari: teginish va sichqoncha.
//! EN: Input events: touch and mouse.
//! RU: События ввода: касание и мышь.

use crate::geometry::Point;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerKind {
    Down,
    Move,
    Up,
    Cancel,
}

/// UZ: Teginish yoki sichqoncha hodisasi (`id` — ko'p barmoqli teginishda barmoq raqami).
/// EN: A touch or mouse event (`id` identifies the finger in multi-touch).
/// RU: Событие касания или мыши (`id` — номер пальца при мультитаче).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerEvent {
    pub kind: PointerKind,
    pub pos: Point,
    pub id: u32,
    /// UZ: Hodisa vaqti (ms, monoton). 0 bo'lsa qobiq vaqti ishlatiladi.
    /// EN: Event time (ms, monotonic). If 0, the shell's time is used.
    /// RU: Время события (мс, монотонное). Если 0 — используется время оболочки.
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
