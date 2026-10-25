use serde_json::Value;

use super::{
    LottieError,
    model_parse::{color_components_to_rgba, f32_unit_to_u8, parse_bezier_path},
};
pub use super::{model_parse::parse_bezier_keyframe_value, model_types::*};

impl Animation {
    /// UZ: Nomi `layer` bo'lgan qatlamdagi barcha to'ldirish (`fl`) va chiziq (`st`) ranglarini `rgb` (0..1) ga almashtiradi (RLottie `setLayerColor` kabi). Almashtirilgan rang soni qaytariladi.
    /// EN: Replaces every fill (`fl`) and stroke (`st`) color of the layer named `layer` with `rgb` (0..1), like RLottie's `setLayerColor`. Returns how many colors were replaced.
    /// RU: Заменяет все цвета заливки (`fl`) и обводки (`st`) слоя с именем `layer` на `rgb` (0..1), как `setLayerColor` в RLottie. Возвращает число заменённых цветов.
    pub fn set_layer_color(&mut self, layer: &str, rgb: [f32; 3]) -> usize {
        self.recolor(Some(layer), rgb)
    }

    /// UZ: Barcha qatlamlardagi to'ldirish va chiziq ranglarini `rgb` ga almashtiradi. EN: Replaces the fill and stroke colors of all layers with `rgb`. RU: Заменяет цвета заливки и обводки всех слоёв на `rgb`.
    pub fn set_all_colors(&mut self, rgb: [f32; 3]) -> usize {
        self.recolor(None, rgb)
    }

    fn recolor(&mut self, layer: Option<&str>, rgb: [f32; 3]) -> usize {
        fn walk(items: &mut [ShapeItem], rgb: [f32; 3]) -> usize {
            let mut n = 0;
            for it in items {
                if it.item_type == "fl" || it.item_type == "st" {
                    it.color = Some(AnimatedValue {
                        animated: Some(0),
                        keyframes: Some(serde_json::json!([rgb[0], rgb[1], rgb[2], 1.0])),
                        expression: None,
                    });
                    n += 1;
                }
                n += walk(&mut it.items, rgb);
            }
            n
        }
        let mut n: usize = self.layers.iter_mut().filter(|l| layer.is_none_or(|nm| l.name == nm)).map(|l| walk(&mut l.shapes, rgb)).sum();
        // UZ: precomp (assets) ichidagi qatlamlar ham. EN: Layers inside precomps (assets) too. RU: Слои внутри precomp (assets) тоже.
        for a in self.assets.iter_mut() {
            n += a.layers.iter_mut().filter(|l| layer.is_none_or(|nm| l.name == nm)).map(|l| walk(&mut l.shapes, rgb)).sum::<usize>();
        }
        n
    }

    /// UZ: JSON matnidan Lottie animatsiyasini o'qiydi. JSON yaroqsiz bo'lsa xato qaytaradi.
    /// EN: Parses a Lottie animation from a JSON string. Returns an error when `json` is not valid Lottie JSON for this model.
    /// RU: Разбирает Lottie-анимацию из JSON-строки. Возвращает ошибку, если `json` не является допустимым Lottie JSON.
    pub fn from_json_str(json: &str) -> Result<Self, LottieError> {
        Ok(serde_json::from_str(json)?)
    }

    #[must_use]
    pub fn duration_frames(&self) -> f32 {
        (self.out_point - self.in_point).max(0.0)
    }

    #[must_use]
    pub fn duration_seconds(&self) -> f32 {
        if self.frame_rate <= f32::EPSILON {
            0.0
        } else {
            self.duration_frames() / self.frame_rate
        }
    }

    #[must_use]
    pub fn lookup_font(&self, name: &str) -> Option<&Font> {
        self.fonts.list.iter().find(|font| font.name == name)
    }

    #[must_use]
    pub fn lookup_glyph(&self, grapheme: &str, font: &Font) -> Option<&FontCharacter> {
        self.chars
            .iter()
            .find(|glyph| glyph.matches(grapheme, font))
    }
}

impl FontCharacter {
    #[must_use]
    pub fn matches(&self, grapheme: &str, font: &Font) -> bool {
        self.character == grapheme
            && (self.style.is_empty() || font.style.is_empty() || self.style == font.style)
            && self
                .family
                .as_deref()
                .is_none_or(|family| family.is_empty() || family == font.family)
    }

    #[must_use]
    pub fn advance_for_size(&self, size: f32) -> Option<f32> {
        (self.size > f32::EPSILON).then_some((self.width * size) / self.size)
    }
}

impl Asset {
    #[must_use]
    pub const fn is_image_asset(&self) -> bool {
        self.path.is_some()
    }

    #[must_use]
    pub fn is_embedded_image_asset(&self) -> bool {
        self.is_image_asset() && self.embedded.unwrap_or(0) != 0
    }

    #[must_use]
    pub fn image_data_url(&self) -> Option<&str> {
        self.is_embedded_image_asset()
            .then_some(self.path.as_deref())
            .flatten()
    }
}

impl LayerType {
    pub const IMAGE: Self = Self(2);
    pub const NULL: Self = Self(3);
    pub const PRECOMP: Self = Self(0);
    pub const SHAPE: Self = Self(4);
    pub const SOLID: Self = Self(1);
    pub const TEXT: Self = Self(5);

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self.0 {
            0 => "precomp",
            1 => "solid",
            2 => "image",
            3 => "null",
            4 => "shape",
            5 => "text",
            _ => "unknown",
        }
    }
}

impl Layer {
    #[must_use]
    pub fn track_matte_mode(&self) -> Option<TrackMatteMode> {
        match self.track_matte? {
            1 => Some(TrackMatteMode::Alpha),
            2 => Some(TrackMatteMode::AlphaInverted),
            3 => Some(TrackMatteMode::Luma),
            4 => Some(TrackMatteMode::LumaInverted),
            _ => None,
        }
    }

    #[must_use]
    pub fn is_matte_source_layer(&self) -> bool {
        self.matte_source == Some(1)
    }
}

impl TextData {
    #[must_use]
    pub fn document_at(&self, frame: f32) -> Option<&TextDocument> {
        let mut current = self.document.keyframes.first()?;
        for keyframe in &self.document.keyframes[1..] {
            if keyframe.time > frame {
                break;
            }
            current = keyframe;
        }
        Some(&current.document)
    }

    #[must_use]
    pub const fn has_animators(&self) -> bool {
        !self.animators.is_empty()
    }

    #[must_use]
    pub fn has_path(&self) -> bool {
        match &self.path {
            Value::Null => false,
            Value::Object(object) => !object.is_empty(),
            Value::Array(items) => !items.is_empty(),
            _ => true,
        }
    }
}

impl TextDocument {
    #[must_use]
    pub fn fill_color_rgba(&self) -> Option<[u8; 4]> {
        color_components_to_rgba(&self.fill_color)
    }

    #[must_use]
    pub fn stroke_color_rgba(&self) -> Option<[u8; 4]> {
        color_components_to_rgba(&self.stroke_color)
    }

    #[must_use]
    pub fn effective_line_height(&self) -> f32 {
        if self.line_height > f32::EPSILON {
            self.line_height
        } else {
            self.size * 1.2
        }
    }
}

impl Mask {
    #[must_use]
    pub fn mask_mode(&self) -> Option<MaskMode> {
        let mode = self.mode.as_deref()?.trim().to_ascii_lowercase();
        match mode.as_str() {
            "a" | "add" => Some(MaskMode::Add),
            "s" | "subtract" => Some(MaskMode::Subtract),
            "i" | "intersect" => Some(MaskMode::Intersect),
            "n" | "none" => Some(MaskMode::None),
            _ => None,
        }
    }
}

impl AnimatedValue {
    #[must_use]
    pub const fn has_expression(&self) -> bool {
        self.expression.is_some()
    }

    #[must_use]
    pub fn is_static(&self) -> bool {
        let Some(keyframes) = self.keyframes.as_ref() else {
            return false;
        };

        if self.animated == Some(1) {
            return false;
        }

        match keyframes {
            Value::Number(_) => true,
            Value::Array(items) => items.iter().all(Value::is_number),
            _ => false,
        }
    }

    #[must_use]
    pub fn as_scalar(&self) -> Option<f32> {
        let values = self.as_vec()?;
        (values.len() == 1).then(|| values[0])
    }

    #[must_use]
    pub fn as_vec2(&self) -> Option<[f32; 2]> {
        let values = self.as_vec()?;
        (values.len() >= 2).then(|| [values[0], values[1]])
    }

    #[must_use]
    pub fn as_vec3(&self) -> Option<[f32; 3]> {
        let values = self.as_vec()?;
        (values.len() >= 3).then(|| [values[0], values[1], values[2]])
    }

    #[must_use]
    pub fn as_color_rgba(&self) -> Option<[u8; 4]> {
        let values = self.as_vec()?;
        if values.len() < 3 {
            return None;
        }

        let alpha = values.get(3).copied().unwrap_or(1.0);
        Some([
            f32_unit_to_u8(values[0]),
            f32_unit_to_u8(values[1]),
            f32_unit_to_u8(values[2]),
            f32_unit_to_u8(alpha),
        ])
    }

    fn as_vec(&self) -> Option<Vec<f32>> {
        if !self.is_static() {
            return None;
        }

        match self.keyframes.as_ref()? {
            Value::Number(number) => Some(vec![number.as_f64()? as f32]),
            Value::Array(items) => items
                .iter()
                .map(|item| item.as_f64().map(|value| value as f32))
                .collect(),
            _ => None,
        }
    }
}

impl PositionValue {
    #[must_use]
    pub const fn combined(&self) -> Option<&AnimatedValue> {
        match self {
            Self::Combined(value) => Some(value),
            Self::Split(_) => None,
        }
    }

    #[must_use]
    pub const fn split(&self) -> Option<&SplitPosition> {
        match self {
            Self::Combined(_) => None,
            Self::Split(value) => Some(value),
        }
    }
}

impl SplitPosition {
    #[must_use]
    pub fn is_split(&self) -> bool {
        self.split.unwrap_or(0) != 0
    }
}

impl ShapePathValue {
    #[must_use]
    pub const fn has_expression(&self) -> bool {
        self.expression.is_some()
    }

    #[must_use]
    pub fn is_static(&self) -> bool {
        let Some(keyframes) = self.keyframes.as_ref() else {
            return false;
        };

        if self.animated == Some(1) {
            return false;
        }

        keyframes.is_object()
    }

    #[must_use]
    pub fn as_bezier_path(&self) -> Option<BezierPath> {
        if !self.is_static() {
            return None;
        }

        parse_bezier_path(self.keyframes.as_ref()?)
    }
}

impl ShapeItem {
    #[must_use]
    pub const fn has_expression(&self) -> bool {
        self.expression.is_some()
    }

    #[must_use]
    pub fn gradient_data(&self) -> Option<GradientData> {
        self.extra
            .get("g")
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok())
    }

    pub fn gradient_type(&self) -> Option<u8> {
        self.extra
            .get("t")
            .and_then(Value::as_u64)
            .and_then(|value| u8::try_from(value).ok())
    }

    #[must_use]
    pub const fn gradient_start_point(&self) -> Option<&AnimatedValue> {
        self.size.as_ref()
    }

    #[must_use]
    pub fn gradient_end_point(&self) -> Option<AnimatedValue> {
        self.extra
            .get("e")
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok())
    }

    #[must_use]
    pub fn gradient_highlight_length(&self) -> Option<AnimatedValue> {
        self.extra
            .get("h")
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok())
    }

    #[must_use]
    pub const fn gradient_highlight_angle(&self) -> Option<&AnimatedValue> {
        self.anchor.as_ref()
    }

    #[must_use]
    pub fn raw_r_value(&self) -> Option<AnimatedValue> {
        self.extra
            .get("r")
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok())
    }

    #[must_use]
    pub fn rectangle_roundness(&self) -> Option<AnimatedValue> {
        self.raw_r_value()
    }

    #[must_use]
    pub fn transform_rotation(&self) -> Option<AnimatedValue> {
        self.raw_r_value()
    }

    #[must_use]
    pub fn dash_pattern(&self) -> Option<Vec<DashPatternEntry>> {
        self.extra
            .get("d")
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok())
    }

    #[must_use]
    pub fn trim_end(&self) -> Option<AnimatedValue> {
        self.extra
            .get("e")
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok())
    }

    pub fn merge_mode(&self) -> Option<u8> {
        self.extra
            .get("mm")
            .and_then(Value::as_u64)
            .and_then(|value| u8::try_from(value).ok())
    }

    pub fn trim_mode(&self) -> Option<u8> {
        self.extra
            .get("m")
            .and_then(Value::as_u64)
            .and_then(|value| u8::try_from(value).ok())
    }

    #[must_use]
    pub const fn trim_start(&self) -> Option<&AnimatedValue> {
        self.size.as_ref()
    }

    #[must_use]
    pub const fn trim_offset(&self) -> Option<&AnimatedValue> {
        self.opacity.as_ref()
    }

    pub fn polystar_type(&self) -> Option<u8> {
        self.extra
            .get("sy")
            .and_then(Value::as_u64)
            .and_then(|value| u8::try_from(value).ok())
    }

    #[must_use]
    pub fn polystar_points(&self) -> Option<AnimatedValue> {
        self.extra
            .get("pt")
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok())
    }

    #[must_use]
    pub fn polystar_outer_radius(&self) -> Option<AnimatedValue> {
        self.extra
            .get("or")
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok())
    }

    #[must_use]
    pub fn polystar_outer_roundness(&self) -> Option<AnimatedValue> {
        self.extra
            .get("os")
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok())
    }

    #[must_use]
    pub fn polystar_inner_radius(&self) -> Option<AnimatedValue> {
        self.extra
            .get("ir")
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok())
    }

    #[must_use]
    pub fn polystar_inner_roundness(&self) -> Option<AnimatedValue> {
        self.extra
            .get("is")
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok())
    }

    pub fn shape_direction(&self) -> Option<u8> {
        self.extra
            .get("d")
            .and_then(Value::as_u64)
            .and_then(|value| u8::try_from(value).ok())
    }

    #[must_use]
    pub const fn repeater_copies(&self) -> Option<&AnimatedValue> {
        self.color.as_ref()
    }

    #[must_use]
    pub fn polystar_rotation(&self) -> Option<AnimatedValue> {
        self.raw_r_value()
    }

    #[must_use]
    pub const fn repeater_offset(&self) -> Option<&AnimatedValue> {
        self.opacity.as_ref()
    }

    pub fn repeater_composite_mode(&self) -> Option<u8> {
        self.extra
            .get("m")
            .and_then(Value::as_u64)
            .and_then(|value| u8::try_from(value).ok())
    }

    #[must_use]
    pub fn repeater_transform(&self) -> Option<RepeaterTransform> {
        self.extra
            .get("tr")
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok())
    }
}
