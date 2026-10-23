use thiserror::Error;

use crate::support::SupportReport;

/// UZ: Lottie tarkibini o'qish, tekshirish yoki chizishda chiqadigan xatolar.
/// EN: Errors that can occur while parsing, validating, or rendering Lottie content.
/// RU: Ошибки, возможные при разборе, проверке или отрисовке Lottie-содержимого.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum LottieError {
    #[error("failed to parse Lottie JSON: {0}")]
    Parse(#[from] serde_json::Error),

    #[error("internal render pipeline failure: {detail}")]
    Internal {
        detail: String,
    },

    /// UZ: Animatsiya sozlangan profil rad etadigan imkoniyatlardan foydalanadi.
    /// EN: The animation uses features that the configured support profile rejects.
    /// RU: Анимация использует возможности, которые отклоняет настроенный профиль.
    #[error("animation contains unsupported features: {report}")]
    UnsupportedFeatures {
        report: SupportReport,
    },

    #[error("invalid canvas size {width}x{height}")]
    InvalidCanvasSize {
        width: u32,
        height: u32,
    },

    #[error("invalid image asset `{id}`: {detail}")]
    InvalidImageAsset {
        id: String,
        detail: String,
    },

    #[error("invalid text layer `{layer}`: {detail}")]
    InvalidTextLayer {
        layer: String,
        detail: String,
    },
}
