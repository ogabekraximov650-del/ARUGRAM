//! # uzcoders-core
//! Asosiy turlar: geometriya, rang, reaktiv holat, animatsiya va kiritish hodisalari. Hech qanday tashqi bog'liqlik yo'q.

pub mod anim;
pub mod color;
pub mod env;
pub mod event;
pub mod geometry;
pub mod state;

pub use anim::{Easing, Spring, Tween};
pub use color::Color;
pub use env::{safe_insets, set_safe_insets};
pub use event::{PointerEvent, PointerKind};
pub use geometry::{EdgeInsets, Point, Rect, Size};
pub use state::{global_revision, invalidate, State};
