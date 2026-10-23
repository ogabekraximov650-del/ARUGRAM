//! UZ: Matn qatlamlari qo'llab-quvvatlanmaydi (stikerlarda matn yo'q).
//! EN: Text layers are not supported (stickers contain no text).
//! RU: Текстовые слои не поддерживаются (в стикерах нет текста).

use super::renderer::{RenderTransform, Renderer, ShapeCaches};
use crate::raster::Pixmap;
use crate::{Animation, Layer, LottieError};

impl Renderer {
    pub(super) fn render_text_layer(
        _animation: &Animation,
        layer: &Layer,
        _frame: f32,
        _pixmap: &mut Pixmap,
        _inherited_transform: RenderTransform,
        _shape_caches: ShapeCaches<'_>,
    ) -> Result<(), LottieError> {
        Err(LottieError::InvalidTextLayer { layer: layer.name.clone(), detail: "text layers are not supported".to_string() })
    }
}
