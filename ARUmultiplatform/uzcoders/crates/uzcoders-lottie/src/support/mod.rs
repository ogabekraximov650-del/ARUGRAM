//! UZ: Qo'llab-quvvatlash tahlili: animatsiyada chizib bo'lmaydigan imkoniyatlar bormi (rad etish sababi bilan).
//! EN: Support analysis: whether an animation uses features that cannot be drawn (with the reason).
//! RU: Анализ поддержки: использует ли анимация возможности, которые нельзя нарисовать (с причиной).

mod analyzer;
mod analyzer_shapes;
mod analyzer_text;

pub use analyzer::{
    SupportProfile, SupportReport, UnsupportedFeature, UnsupportedKind, analyze_animation,
    analyze_animation_with_profile,
};
