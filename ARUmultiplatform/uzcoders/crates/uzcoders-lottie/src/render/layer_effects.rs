use crate::raster::Pixmap;

use crate::{
    Layer,
    effects::{
        FillEffect, SimpleChokerEffect, SupportedLayerEffect, fill_effect_color,
        fill_effect_opacity, simple_choker_amount, supported_layer_effects,
    },
};

pub(super) fn apply_supported_layer_effects(pixmap: &mut Pixmap, layer: &Layer, frame: f32) {
    span_enter!(
        tracing::Level::TRACE,
        "apply_supported_layer_effects",
        frame = frame,
        layer = layer.name.as_str()
    );
    let Some(effects) = supported_layer_effects(layer) else {
        return;
    };

    for effect in effects {
        match effect {
            SupportedLayerEffect::Fill(fill) => apply_fill_effect(pixmap, &fill, frame),
            SupportedLayerEffect::SimpleChoker(choker) => {
                apply_simple_choker_effect(pixmap, &choker, frame);
            }
        }
    }
}

fn apply_fill_effect(pixmap: &mut Pixmap, effect: &FillEffect, frame: f32) {
    let Some(color) = fill_effect_color(effect, frame) else {
        return;
    };
    let Some(opacity) = fill_effect_opacity(effect, frame) else {
        return;
    };
    let factor = opacity * (f32::from(color[3]) / 255.0);
    if factor <= f32::EPSILON {
        return;
    }

    for pixel in pixmap.data_mut().as_chunks_mut::<4>().0 {
        if pixel[3] == 0 {
            continue;
        }

        pixel[0] = blend_fill_channel(pixel[0], color[0], factor);
        pixel[1] = blend_fill_channel(pixel[1], color[1], factor);
        pixel[2] = blend_fill_channel(pixel[2], color[2], factor);
    }
}

fn blend_fill_channel(original: u8, target: u8, factor: f32) -> u8 {
    let original = f32::from(original);
    let target = f32::from(target);
    (target - original)
        .mul_add(factor.clamp(0.0, 1.0), original)
        .round() as u8
}

fn apply_simple_choker_effect(pixmap: &mut Pixmap, effect: &SimpleChokerEffect, frame: f32) {
    let Some(amount) = simple_choker_amount(effect, frame) else {
        return;
    };
    let rounded = amount.round();
    if !rounded.is_finite() || rounded.abs() > i32::MAX as f32 {
        return;
    }
    let radius = rounded as i32;
    if radius == 0 {
        return;
    }

    let Ok(width) = i32::try_from(pixmap.width()) else {
        return;
    };
    let Ok(height) = i32::try_from(pixmap.height()) else {
        return;
    };
    // UZ: Kvadrat oynali min/max (alfa kanali) ikki o'tishda (avval qatorlar, keyin ustunlar): natija avvalgi to'liq oynali hisob bilan bir xil,
    // lekin vaqt `O(w*h*r)` (oldin `O(w*h*r^2)` edi va katta radiusda soniyalab osilardi).
    // EN: A square-window min/max (alpha channel) in two passes (rows, then columns): the result equals the old full-window computation,
    // but the time is `O(w*h*r)` (it used to be `O(w*h*r^2)` and hung for seconds at large radii).
    // RU: Min/max по квадратному окну (альфа-канал) в два прохода (строки, затем столбцы): результат тот же, что и при полном переборе окна,
    // но время `O(w*h*r)` (раньше было `O(w*h*r^2)` и при больших радиусах висло на секунды).
    let (w, h, r) = (width as usize, height as usize, radius.unsigned_abs() as usize);
    let erode = radius > 0;
    let mut alpha: Vec<u8> = pixmap.data().chunks_exact(4).map(|p| p[3]).collect();
    let mut tmp = alpha.clone();
    for y in 0..h {
        for x in 0..w {
            let (lo, hi) = (x.saturating_sub(r), (x + r).min(w - 1));
            let row = &alpha[y * w + lo..=y * w + hi];
            tmp[y * w + x] = if erode { row.iter().copied().min().unwrap_or(0) } else { row.iter().copied().max().unwrap_or(0) };
        }
    }
    for y in 0..h {
        let (lo, hi) = (y.saturating_sub(r), (y + r).min(h - 1));
        for x in 0..w {
            let mut acc = if erode { u8::MAX } else { 0 };
            for yy in lo..=hi {
                let v = tmp[yy * w + x];
                acc = if erode { acc.min(v) } else { acc.max(v) };
            }
            alpha[y * w + x] = acc;
        }
    }
    for (p, a) in pixmap.data_mut().chunks_exact_mut(4).zip(alpha) {
        p[3] = a;
    }
}
