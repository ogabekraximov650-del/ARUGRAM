//! UZ: Matn qatlamlari qo'llab-quvvatlanmaydi (rad etiladi); qatlam effektlari tekshiruvi.
//! EN: Text layers are not supported (rejected); the layer-effect check.
//! RU: Текстовые слои не поддерживаются (отклоняются); проверка эффектов слоя.

use super::analyzer::{SupportProfile, SupportReport, UnsupportedFeature, UnsupportedKind};
use crate::{
    effects::supported_layer_effects,
    model::{Animation, Layer, LayerType},
};

pub(super) fn visit_text_layer(_animation: &Animation, _layer: &Layer, path: &str, report: &mut SupportReport) {
    report.push(UnsupportedFeature {
        path: path.to_string(),
        kind: UnsupportedKind::Text,
        detail: "text layers are not supported".to_string(),
    });
}

pub(super) fn layer_effects_are_supported(layer: &Layer, profile: &SupportProfile) -> bool {
    profile.allow_effects || layer.layer_type == LayerType::NULL || supported_layer_effects(layer).is_some()
}
