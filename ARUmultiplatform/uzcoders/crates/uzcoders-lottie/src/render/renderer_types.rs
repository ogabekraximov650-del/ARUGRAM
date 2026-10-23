use std::{cell::RefCell, rc::Rc};

use rustc_hash::FxHashMap;
use crate::raster::{
    Color, GradientStop, LineCap as TinyLineCap, LineJoin as TinyLineJoin, LinearGradient, Paint,
    Path, Point, RadialGradient, Shader, SpreadMode, Stroke as TinyStroke,
    StrokeDash as TinyStrokeDash, Transform as PixmapTransform,
};

use super::{assets::ImageAssetStore, sample_cache::TimelineSampleCache, values::distance};
use crate::{Animation, Asset, Layer, LottieError, SupportProfile};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Rgba8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// UZ: Bitta kadrni chizish sozlamalari.
/// EN: Configuration for rasterizing a single frame.
/// RU: Параметры отрисовки одного кадра.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct RenderConfig {
    /// UZ: Chizishdan oldin kanvas to'ldiriladigan fon rangi.
    /// EN: Background color used to clear the canvas before rendering.
    /// RU: Цвет фона, которым заливается холст перед отрисовкой.
    pub background: Rgba8,
    /// UZ: Animatsiya kanvasiga nisbatan chiqish masshtabi.
    /// EN: Output scale relative to the animation canvas.
    /// RU: Масштаб вывода относительно холста анимации.
    pub scale: f32,
}

/// UZ: Bitta chizilgan kadrning RGBA piksellari.
/// EN: Raw RGBA pixels of one rendered frame.
/// RU: Пиксели RGBA одного отрисованного кадра.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct RasterFrame {
    pub width: u32,
    pub height: u32,
    /// UZ: RGBA8 piksellar, qator-qator; ranglar alfaga OLDINDAN KO'PAYTIRILGAN (premultiplied).
    /// EN: Packed RGBA8 pixels in row-major order; colors are PREMULTIPLIED by alpha.
    /// RU: Пиксели RGBA8 по строкам; цвета ПРЕДУМНОЖЕНЫ на альфу (premultiplied).
    pub pixels: Vec<u8>,
}

pub trait ImageAssetResolver {
    fn resolve_image_asset(&self, asset: &Asset) -> Result<Option<Vec<u8>>, LottieError>;
}

/// UZ: Animatsiyalarni tahlil qilish va chizish kirish nuqtasi (holatsiz).
/// EN: Stateless entry point for analyzing and rendering animations.
/// RU: Точка входа для анализа и отрисовки анимаций (без состояния).
#[derive(Debug, Clone, Copy)]
pub struct Renderer {
    pub(super) profile: SupportProfile,
}

/// UZ: Qayta-qayta chizish uchun oldindan tayyorlangan animatsiya. Ifodalar, tahlil va asset tayyorlash bir marta bajariladi.
/// EN: A preprocessed animation that can be rendered repeatedly. Expression resolution, analysis and asset preparation run once.
/// RU: Предварительно подготовленная анимация для многократной отрисовки. Выражения, анализ и подготовка ассетов выполняются один раз.
#[derive(Debug)]
pub struct PreparedAnimation {
    pub(super) renderer: Renderer,
    pub(super) animation: Animation,
    pub(super) image_assets: ImageAssetStore,
    pub(super) static_path_cache: StaticPathCache,
    pub(super) layer_hierarchy_cache: LayerHierarchyCache,
    pub(super) shape_plan_cache: ShapePlanCache,
    pub(super) timeline_sample_cache: TimelineSampleCache,
}

#[derive(Debug, Default)]
pub(super) struct StaticPathCache {
    pub(super) paths: RefCell<FxHashMap<usize, Option<Rc<Path>>>>,
}

#[derive(Debug, Default)]
pub(super) struct LayerHierarchyCache {
    pub(super) slices: FxHashMap<usize, LayerSliceCache>,
}

#[derive(Debug)]
pub(super) struct LayerSliceCache {
    pub(super) positions_by_ptr: FxHashMap<usize, usize>,
    pub(super) parent_lineages: Vec<Box<[usize]>>,
    pub(super) matte_sources: Vec<Option<usize>>,
    pub(super) static_transforms: Vec<Option<RenderTransform>>,
}

#[derive(Debug, Default)]
pub(super) struct ShapePlanCache {
    pub(super) groups: FxHashMap<usize, ShapeGroupPlan>,
}

#[derive(Debug)]
pub(super) struct ShapeGroupPlan {
    pub(super) local_transform: Option<usize>,
    pub(super) static_local_transform: Option<RenderTransform>,
    pub(super) trim: Option<usize>,
    pub(super) static_trim: Option<TrimStyle>,
    pub(super) repeater: Option<usize>,
    pub(super) static_repeater: Option<RepeaterStyle>,
    pub(super) fill: Option<usize>,
    pub(super) static_fill: Option<FillStyle>,
    pub(super) stroke: Option<usize>,
    pub(super) static_stroke: Option<StrokeStyle>,
    pub(super) merge_index: Option<usize>,
    pub(super) renderables: Box<[ShapeRenderableItem]>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ShapeRenderableItem {
    pub(super) index: usize,
    pub(super) kind: ShapeRenderableKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ShapeRenderableKind {
    Group,
    Geometry,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct RenderTransform {
    pub(super) matrix: PixmapTransform,
    pub(super) opacity: f32,
}

#[derive(Debug, Clone, Default)]
pub(super) struct ShapeStyles {
    pub(super) fill: Option<FillStyle>,
    pub(super) stroke: Option<StrokeStyle>,
}

#[derive(Debug, Clone)]
pub(super) struct FillStyle {
    pub(super) brush: BrushStyle,
    pub(super) opacity: f32,
}

#[derive(Debug, Clone)]
pub(super) struct StrokeStyle {
    pub(super) brush: BrushStyle,
    pub(super) opacity: f32,
    pub(super) stroke: TinyStroke,
}

#[derive(Debug, Clone)]
pub(super) enum BrushStyle {
    Solid(Rgba8),
    Gradient(GradientStyle),
}

#[derive(Debug, Clone)]
pub(super) struct GradientStyle {
    pub(super) kind: GradientKind,
    pub(super) start: [f32; 2],
    pub(super) end: [f32; 2],
    pub(super) highlight_length: f32,
    pub(super) highlight_angle: f32,
    pub(super) stops: Vec<GradientStopStyle>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GradientKind {
    Linear,
    Radial,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct GradientStopStyle {
    pub(super) offset: f32,
    pub(super) color: Rgba8,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct TrimStyle {
    pub(super) start: f32,
    pub(super) end: f32,
    pub(super) offset_degrees: f32,
    pub(super) mode: u8,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct RepeaterStyle {
    pub(super) copies: f32,
    pub(super) offset: f32,
    pub(super) mode: u8,
    pub(super) anchor: [f32; 2],
    pub(super) position: [f32; 2],
    pub(super) scale: [f32; 2],
    pub(super) rotation: f32,
    pub(super) start_opacity: f32,
    pub(super) end_opacity: f32,
}

pub(super) struct LayerRenderContext<'a> {
    pub(super) animation: &'a Animation,
    pub(super) layers: &'a [Layer],
    pub(super) image_assets: &'a ImageAssetStore,
    pub(super) static_path_cache: Option<&'a StaticPathCache>,
    pub(super) layer_hierarchy_cache: Option<&'a LayerHierarchyCache>,
    pub(super) shape_plan_cache: Option<&'a ShapePlanCache>,
    pub(super) timeline_sample_cache: Option<&'a TimelineSampleCache>,
    pub(super) frame_cache: &'a FrameRenderCache,
    pub(super) canvas_width: u32,
    pub(super) canvas_height: u32,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ShapeRenderState<'a> {
    pub(super) styles: &'a ShapeStyles,
    pub(super) trim: Option<&'a TrimStyle>,
    pub(super) static_path_cache: Option<&'a StaticPathCache>,
    pub(super) shape_plan_cache: Option<&'a ShapePlanCache>,
    pub(super) timeline_sample_cache: Option<&'a TimelineSampleCache>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ShapeCaches<'a> {
    pub(super) static_paths: Option<&'a StaticPathCache>,
    pub(super) plans: Option<&'a ShapePlanCache>,
    pub(super) timeline_samples: Option<&'a TimelineSampleCache>,
}

#[derive(Clone, Copy)]
pub(super) struct PreparedResources<'a> {
    pub(super) image_assets: &'a ImageAssetStore,
    pub(super) shape_caches: ShapeCaches<'a>,
    pub(super) layer_hierarchy_cache: Option<&'a LayerHierarchyCache>,
}

#[derive(Debug, Default)]
pub(super) struct FrameRenderCache {
    pub(super) layer_transforms: RefCell<FxHashMap<(usize, u32), RenderTransform>>,
}

impl Rgba8 {
    pub const TRANSPARENT: Self = Self::new(0, 0, 0, 0);

    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub(super) fn with_alpha_factor(self, factor: f32) -> Self {
        let scaled = ((self.a as f32) * factor.clamp(0.0, 1.0)).round() as u8;
        Self { a: scaled, ..self }
    }
}

impl From<Rgba8> for Color {
    fn from(value: Rgba8) -> Self {
        Self::from_rgba8(value.r, value.g, value.b, value.a)
    }
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            background: Rgba8::TRANSPARENT,
            scale: 1.0,
        }
    }
}

impl RenderConfig {
    #[must_use]
    pub const fn new(background: Rgba8, scale: f32) -> Self {
        Self { background, scale }
    }
}

impl RasterFrame {
    #[must_use]
    pub const fn new(width: u32, height: u32, pixels: Vec<u8>) -> Self {
        Self {
            width,
            height,
            pixels,
        }
    }
}

impl Default for RenderTransform {
    fn default() -> Self {
        Self {
            matrix: PixmapTransform::identity(),
            opacity: 1.0,
        }
    }
}

impl<F> ImageAssetResolver for F
where
    F: Fn(&Asset) -> Result<Option<Vec<u8>>, LottieError>,
{
    fn resolve_image_asset(&self, asset: &Asset) -> Result<Option<Vec<u8>>, LottieError> {
        self(asset)
    }
}

impl FillStyle {
    pub(super) fn paint(&self, inherited_opacity: f32) -> Option<Paint> {
        self.brush
            .paint((self.opacity * inherited_opacity).clamp(0.0, 1.0))
    }
}

impl StrokeStyle {
    pub(super) const fn new(
        brush: BrushStyle,
        opacity: f32,
        width: f32,
        line_cap: TinyLineCap,
        line_join: TinyLineJoin,
        miter_limit: f32,
        dash: Option<TinyStrokeDash>,
    ) -> Self {
        Self {
            brush,
            opacity,
            stroke: TinyStroke {
                width: width.max(0.0),
                line_cap,
                line_join,
                miter_limit: miter_limit.max(1.0),
                dash,
            },
        }
    }

    pub(super) fn paint(&self, inherited_opacity: f32) -> Option<Paint> {
        self.brush
            .paint((self.opacity * inherited_opacity).clamp(0.0, 1.0))
    }

    pub(super) const fn stroke(&self) -> &TinyStroke {
        &self.stroke
    }
}

impl BrushStyle {
    pub(super) fn paint(&self, opacity: f32) -> Option<Paint> {
        let mut paint = Paint {
            anti_alias: true,
            ..Paint::default()
        };

        match self {
            Self::Solid(color) => {
                let color = color.with_alpha_factor(opacity);
                paint.set_color_rgba8(color.r, color.g, color.b, color.a);
            }
            Self::Gradient(gradient) => {
                paint.shader = gradient.shader(opacity)?;
            }
        }

        Some(paint)
    }
}

impl GradientStyle {
    pub(super) fn shader(&self, opacity: f32) -> Option<Shader> {
        let stops = self
            .stops
            .iter()
            .map(|stop| {
                GradientStop::new(
                    stop.offset.clamp(0.0, 1.0),
                    Color::from(stop.color.with_alpha_factor(opacity)),
                )
            })
            .collect::<Vec<_>>();

        match self.kind {
            GradientKind::Linear => LinearGradient::new(
                Point::from_xy(self.start[0], self.start[1]),
                Point::from_xy(self.end[0], self.end[1]),
                stops,
                SpreadMode::Pad,
                PixmapTransform::identity(),
            ),
            GradientKind::Radial => {
                let center = self.start;
                let edge = self.end;
                let radius = distance(center, edge);
                if radius <= f32::EPSILON {
                    return None;
                }

                let angle = (edge[1] - center[1]).atan2(edge[0] - center[0]);
                let percent = self.highlight_length.clamp(-0.99, 0.99);
                let focal_distance = radius * percent;
                let focal = [
                    (angle + self.highlight_angle)
                        .cos()
                        .mul_add(focal_distance, center[0]),
                    (angle + self.highlight_angle)
                        .sin()
                        .mul_add(focal_distance, center[1]),
                ];

                RadialGradient::new(
                    Point::from_xy(focal[0], focal[1]),
                    0.0,
                    Point::from_xy(center[0], center[1]),
                    radius,
                    stops,
                    SpreadMode::Pad,
                    PixmapTransform::identity(),
                )
            }
        }
    }
}
