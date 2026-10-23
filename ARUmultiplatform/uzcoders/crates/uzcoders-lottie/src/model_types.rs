use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

/// UZ: Lottie animatsiyasining ildiz hujjati.
/// EN: Root Lottie animation document.
/// RU: Корневой документ Lottie-анимации.
#[derive(Debug, Clone, Deserialize)]
pub struct Animation {
    #[serde(default, rename = "v")]
    pub version: String,
    #[serde(rename = "fr")]
    pub frame_rate: f32,
    #[serde(rename = "ip")]
    pub in_point: f32,
    #[serde(rename = "op")]
    pub out_point: f32,
    #[serde(rename = "w")]
    pub width: u32,
    #[serde(rename = "h")]
    pub height: u32,
    #[serde(default)]
    pub layers: Vec<Layer>,
    #[serde(default)]
    pub assets: Vec<Asset>,
    #[serde(default)]
    pub fonts: FontList,
    #[serde(default)]
    pub chars: Vec<FontCharacter>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct FontList {
    #[serde(default, rename = "list")]
    pub list: Vec<Font>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Font {
    #[serde(default, rename = "fName")]
    pub name: String,
    #[serde(default, rename = "fFamily")]
    pub family: String,
    #[serde(default, rename = "fStyle")]
    pub style: String,
    #[serde(default)]
    pub ascent: f32,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct FontCharacter {
    #[serde(default, rename = "ch")]
    pub character: String,
    #[serde(default, rename = "size")]
    pub size: f32,
    #[serde(default, rename = "style")]
    pub style: String,
    #[serde(default, rename = "w")]
    pub width: f32,
    #[serde(default, rename = "data")]
    pub data: FontCharacterData,
    #[serde(default, rename = "fFamily")]
    pub family: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct FontCharacterData {
    #[serde(default, rename = "shapes")]
    pub shapes: Vec<ShapeItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Asset {
    #[serde(default, rename = "id")]
    pub id: String,
    #[serde(default, rename = "nm")]
    pub name: Option<String>,
    #[serde(default, rename = "w")]
    pub width: Option<u32>,
    #[serde(default, rename = "h")]
    pub height: Option<u32>,
    #[serde(default, rename = "u")]
    pub base_path: Option<String>,
    #[serde(default, rename = "p")]
    pub path: Option<String>,
    #[serde(default, rename = "e")]
    pub embedded: Option<u8>,
    #[serde(default)]
    pub layers: Vec<Layer>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Layer {
    #[serde(default, rename = "nm")]
    pub name: String,
    #[serde(rename = "ty")]
    pub layer_type: LayerType,
    #[serde(default, rename = "ind")]
    pub index: Option<i64>,
    #[serde(default, rename = "parent")]
    pub parent: Option<i64>,
    #[serde(default, rename = "hd")]
    pub hidden: bool,
    #[serde(default, rename = "ip")]
    pub in_point: Option<f32>,
    #[serde(default, rename = "op")]
    pub out_point: Option<f32>,
    #[serde(default, rename = "refId")]
    pub ref_id: Option<String>,
    #[serde(default, rename = "st")]
    pub start_time: f32,
    #[serde(default = "default_stretch", rename = "sr")]
    pub stretch: f32,
    #[serde(default, rename = "ks")]
    pub transform: Option<Transform>,
    #[serde(default, rename = "tm")]
    pub time_remap: Option<AnimatedValue>,
    #[serde(default, rename = "shapes")]
    pub shapes: Vec<ShapeItem>,
    #[serde(default, rename = "masksProperties")]
    pub masks: Vec<Mask>,
    #[serde(default, rename = "tt")]
    pub track_matte: Option<u8>,
    #[serde(default, rename = "tp")]
    pub matte_parent: Option<i64>,
    #[serde(default, rename = "td")]
    pub matte_source: Option<u8>,
    #[serde(default, rename = "ef")]
    pub effects: Vec<Value>,
    #[serde(default, rename = "t")]
    pub text: Option<TextData>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct LayerType(#[doc = "Raw Lottie layer type tag."] pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TrackMatteMode {
    Alpha,
    AlphaInverted,
    Luma,
    LumaInverted,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TextData {
    #[serde(default, rename = "d")]
    pub document: TextDocumentCollection,
    #[serde(default, rename = "a")]
    pub animators: Vec<Value>,
    #[serde(default, rename = "p")]
    pub path: Value,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TextDocumentCollection {
    #[serde(default, rename = "k")]
    pub keyframes: Vec<TextDocumentKeyframe>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TextDocumentKeyframe {
    #[serde(default, rename = "s")]
    pub document: TextDocument,
    #[serde(default, rename = "t")]
    pub time: f32,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TextDocument {
    #[serde(default, rename = "t")]
    pub text: String,
    #[serde(default, rename = "f")]
    pub font: String,
    #[serde(default, rename = "s")]
    pub size: f32,
    #[serde(default, rename = "j")]
    pub justify: u8,
    #[serde(default, rename = "tr")]
    pub tracking: f32,
    #[serde(default, rename = "lh")]
    pub line_height: f32,
    #[serde(default, rename = "ls")]
    pub baseline_shift: f32,
    #[serde(default, rename = "fc")]
    pub fill_color: Vec<f32>,
    #[serde(default, rename = "sc")]
    pub stroke_color: Vec<f32>,
    #[serde(default, rename = "sw")]
    pub stroke_width: f32,
    #[serde(default, rename = "ps")]
    pub position: Option<[f32; 2]>,
    #[serde(default, rename = "sz")]
    pub box_size: Option<[f32; 2]>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Transform {
    #[serde(default, rename = "a")]
    pub anchor: Option<AnimatedValue>,
    #[serde(default, rename = "p")]
    pub position: Option<PositionValue>,
    #[serde(default, rename = "s")]
    pub scale: Option<AnimatedValue>,
    #[serde(default, rename = "r")]
    pub rotation: Option<AnimatedValue>,
    #[serde(default, rename = "o")]
    pub opacity: Option<AnimatedValue>,
    #[serde(default, rename = "sk")]
    pub skew: Option<AnimatedValue>,
    #[serde(default, rename = "sa")]
    pub skew_axis: Option<AnimatedValue>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Mask {
    #[serde(default, rename = "mode")]
    pub mode: Option<String>,
    #[serde(default, rename = "pt")]
    pub path: Option<ShapePathValue>,
    #[serde(default, rename = "o")]
    pub opacity: Option<AnimatedValue>,
    #[serde(default, rename = "inv")]
    pub inverted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MaskMode {
    Add,
    Subtract,
    Intersect,
    None,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct AnimatedValue {
    #[serde(default, rename = "a")]
    pub animated: Option<u8>,
    #[serde(default, rename = "k")]
    pub keyframes: Option<Value>,
    #[serde(default, rename = "x")]
    pub expression: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum PositionValue {
    Split(SplitPosition),
    Combined(AnimatedValue),
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct SplitPosition {
    #[serde(default, rename = "s")]
    pub split: Option<u8>,
    #[serde(default, rename = "x")]
    pub x: Option<AnimatedValue>,
    #[serde(default, rename = "y")]
    pub y: Option<AnimatedValue>,
    #[serde(default, rename = "z")]
    pub z: Option<AnimatedValue>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BezierPath {
    pub closed: bool,
    pub vertices: Vec<BezierVertex>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BezierVertex {
    pub vertex: [f32; 2],
    pub in_tangent: [f32; 2],
    pub out_tangent: [f32; 2],
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ShapePathValue {
    #[serde(default, rename = "a")]
    pub animated: Option<u8>,
    #[serde(default, rename = "k")]
    pub keyframes: Option<Value>,
    #[serde(default, rename = "x")]
    pub expression: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ShapeItem {
    #[serde(default, rename = "nm")]
    pub name: String,
    #[serde(default, rename = "ty")]
    pub item_type: String,
    #[serde(default, rename = "it")]
    pub items: Vec<Self>,
    #[serde(default, rename = "hd")]
    pub hidden: bool,
    #[serde(default, rename = "x")]
    pub expression: Option<Value>,
    #[serde(default, rename = "ks")]
    pub path: Option<ShapePathValue>,
    #[serde(default, rename = "p")]
    pub position: Option<PositionValue>,
    #[serde(default, rename = "s")]
    pub size: Option<AnimatedValue>,
    #[serde(default, rename = "c")]
    pub color: Option<AnimatedValue>,
    #[serde(default, rename = "o")]
    pub opacity: Option<AnimatedValue>,
    #[serde(default, rename = "w")]
    pub width: Option<AnimatedValue>,
    #[serde(default, rename = "a")]
    pub anchor: Option<AnimatedValue>,
    #[serde(default, rename = "sk")]
    pub skew: Option<AnimatedValue>,
    #[serde(default, rename = "sa")]
    pub skew_axis: Option<AnimatedValue>,
    #[serde(default, rename = "lc")]
    pub line_cap: Option<u8>,
    #[serde(default, rename = "lj")]
    pub line_join: Option<u8>,
    #[serde(default, rename = "ml")]
    pub miter_limit: Option<f32>,
    #[serde(default, rename = "ml2")]
    pub miter_limit_value: Option<AnimatedValue>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GradientData {
    #[serde(rename = "p")]
    pub point_count: usize,
    #[serde(rename = "k")]
    pub colors: AnimatedValue,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DashPatternEntry {
    #[serde(default, rename = "n")]
    pub name: String,
    #[serde(default, rename = "v")]
    pub value: AnimatedValue,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RepeaterTransform {
    #[serde(default, rename = "a")]
    pub anchor: Option<AnimatedValue>,
    #[serde(default, rename = "p")]
    pub position: Option<PositionValue>,
    #[serde(default, rename = "s")]
    pub scale: Option<AnimatedValue>,
    #[serde(default, rename = "r")]
    pub rotation: Option<AnimatedValue>,
    #[serde(default, rename = "o")]
    pub opacity: Option<AnimatedValue>,
    #[serde(default, rename = "so")]
    pub start_opacity: Option<AnimatedValue>,
    #[serde(default, rename = "eo")]
    pub end_opacity: Option<AnimatedValue>,
}

const fn default_stretch() -> f32 {
    1.0
}
