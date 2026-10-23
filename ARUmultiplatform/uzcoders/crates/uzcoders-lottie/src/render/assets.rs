//! UZ: Rasm assetlari qo'llab-quvvatlanmaydi (Telegram stikerlarida ular yo'q): rasm asseti bor animatsiya rad etiladi.
//! EN: Image assets are not supported (Telegram stickers have none): an animation with an image asset is rejected.
//! RU: Растровые ассеты не поддерживаются (в стикерах Telegram их нет): анимация с растровым ассетом отклоняется.

use super::renderer::ImageAssetResolver;
use crate::{Animation, LottieError};

#[derive(Debug, Default)]
pub(super) struct ImageAssetStore;

pub(super) fn resolve_image_assets(animation: &Animation, resolver: Option<&dyn ImageAssetResolver>) -> Result<ImageAssetStore, LottieError> {
    if let Some(asset) = animation.assets.iter().find(|asset| asset.is_image_asset()) {
        return Err(LottieError::InvalidImageAsset { id: asset.id.clone(), detail: "image assets are not supported".to_string() });
    }
    if resolver.is_some() {
        return Err(LottieError::InvalidImageAsset { id: "<resolver>".to_string(), detail: "image assets are not supported".to_string() });
    }
    Ok(ImageAssetStore)
}
