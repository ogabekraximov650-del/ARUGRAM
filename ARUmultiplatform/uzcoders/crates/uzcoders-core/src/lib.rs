//! UZ: # uzcoders-core
//! Asosiy turlar: geometriya, rang, reaktiv holat, animatsiya va kiritish hodisalari. Hech qanday tashqi bog'liqlik yo'q.
//! EN: # uzcoders-core
//! Core types: geometry, color, reactive state, animation and input events. No external dependencies.
//! RU: # uzcoders-core
//! Базовые типы: геометрия, цвет, реактивное состояние, анимация и события ввода. Без внешних зависимостей.

pub mod anim;
pub mod color;
pub mod env;
pub mod event;
pub mod geometry;
pub mod state;
pub mod task;

pub use anim::{Easing, Spring, Tween};
pub use color::Color;
pub use env::{density, safe_insets, set_density, set_safe_insets, set_window_dp, window_dp};
pub use event::{PointerEvent, PointerKind};
pub use geometry::{EdgeInsets, Point, Rect, Size};
pub use state::{global_revision, invalidate, State};
pub mod errlog;
pub use errlog::{clear_errors, error_entries, errors_text, report_error, set_error_file, take_error_toast, ErrEntry};
pub use task::{clear_frame_request, clear_video_frame_request, request_video_frame, video_frame_requested, drain_tasks, frame_requested, request_frame, pending_tasks, breadcrumb, crash_context, set_waker, spawn_bg};
