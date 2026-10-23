use crate::raster::{BlendMode, Pixmap, Transform as PixmapTransform};

use super::{
    super::{
        composition::{
            LayerSurface, apply_alpha_mask, apply_track_matte, build_layer_mask, composite_pixmap,
            crop_layer_surface, find_track_matte_source_index, new_pixmap,
            resolve_output_canvas_size,
        },
        drawing::render_shape_items,
        layer_effects::apply_supported_layer_effects,
        scene::{
            frame_in_range, layer_is_visible, lookup_precomp_asset,
            map_layer_frame, resolve_layer_transform_chain,
        },
    },
    FrameRenderCache, ImageAssetResolver, LayerRenderContext, PreparedAnimation, PreparedResources,
    RenderConfig, RenderTransform, Renderer, ShapeCaches, ShapeRenderState, ShapeStyles,
};
use crate::{Animation, Layer, LayerType, LottieError};

impl Renderer {
    /// UZ: Bitta kadrni chizadi (tayyorlov natijasi saqlanmaydi). Tayyorlov yoki chizish muvaffaqiyatsiz bo'lsa xato.
    /// EN: Renders a single frame without retaining preprocessing state. Errors when preprocessing or rasterization fails.
    /// RU: Рисует один кадр без сохранения подготовки. Ошибка, если подготовка или отрисовка не удалась.
    pub fn render_frame(
        &self,
        animation: &Animation,
        frame: f32,
        config: RenderConfig,
    ) -> Result<super::RasterFrame, LottieError> {
        self.prepare(animation)?.render_frame(frame, config)
    }

    pub fn render_frame_with_resolver<R: ImageAssetResolver>(
        &self,
        animation: &Animation,
        frame: f32,
        config: RenderConfig,
        resolver: &R,
    ) -> Result<super::RasterFrame, LottieError> {
        self.prepare_with_resolver(animation, resolver)?
            .render_frame(frame, config)
    }

    pub(super) fn render_frame_with_assets(
        &self,
        animation: &Animation,
        frame: f32,
        config: RenderConfig,
        resources: PreparedResources<'_>,
    ) -> Result<super::RasterFrame, LottieError> {
        let (width, height) = resolve_output_canvas_size(animation, config)?;
        let mut pixmap = new_pixmap(width, height)?;
        self.render_frame_with_assets_into(animation, frame, config, resources, &mut pixmap)?;

        Ok(super::RasterFrame {
            width: pixmap.width(),
            height: pixmap.height(),
            pixels: pixmap.data().to_vec(),
        })
    }

    pub(super) fn render_frame_with_assets_into(
        &self,
        animation: &Animation,
        frame: f32,
        config: RenderConfig,
        resources: PreparedResources<'_>,
        pixmap: &mut Pixmap,
    ) -> Result<(), LottieError> {
        span_enter!(
            tracing::Level::TRACE,
            "render_frame_with_assets_into",
            frame = frame,
            width = pixmap.width(),
            height = pixmap.height()
        );
        let (expected_width, expected_height) = resolve_output_canvas_size(animation, config)?;
        if pixmap.width() != expected_width || pixmap.height() != expected_height {
            return Err(LottieError::InvalidCanvasSize {
                width: pixmap.width(),
                height: pixmap.height(),
            });
        }

        pixmap.fill(config.background.into());

        if !frame_in_range(frame, Some(animation.in_point), Some(animation.out_point)) {
            return Ok(());
        }

        let frame_cache = FrameRenderCache::default();
        let context = LayerRenderContext {
            animation,
            layers: &animation.layers,
            image_assets: resources.image_assets,
            static_path_cache: resources.shape_caches.static_paths,
            layer_hierarchy_cache: resources.layer_hierarchy_cache,
            shape_plan_cache: resources.shape_caches.plans,
            timeline_sample_cache: resources.shape_caches.timeline_samples,
            frame_cache: &frame_cache,
            canvas_width: pixmap.width(),
            canvas_height: pixmap.height(),
        };

        self.render_layer_stack(
            &context,
            frame,
            pixmap,
            RenderTransform {
                matrix: PixmapTransform::identity().pre_scale(
                    pixmap.width() as f32 / animation.width.max(1) as f32,
                    pixmap.height() as f32 / animation.height.max(1) as f32,
                ),
                opacity: 1.0,
            },
        )
    }

    pub(super) fn render_layer_stack(
        &self,
        context: &LayerRenderContext<'_>,
        frame: f32,
        pixmap: &mut Pixmap,
        inherited_transform: RenderTransform,
    ) -> Result<(), LottieError> {
        span_enter!(
            tracing::Level::TRACE,
            "render_layer_stack",
            frame = frame,
            layer_count = context.layers.len()
        );
        let mut consumed_mattes = vec![false; context.layers.len()];

        for index in (0..context.layers.len()).rev() {
            if consumed_mattes[index] {
                continue;
            }

            let layer = &context.layers[index];
            if layer.is_matte_source_layer() || !layer_is_visible(layer, frame) {
                continue;
            }

            if layer.track_matte_mode().is_none()
                && layer.masks.is_empty()
                && layer.effects.is_empty()
            {
                self.render_layer_content(context, frame, layer, inherited_transform, pixmap)?;
                continue;
            }

            let Some(mut surface) =
                self.render_layer_surface(context, frame, layer, inherited_transform, false)?
            else {
                continue;
            };

            if let Some(matte_mode) = layer.track_matte_mode()
                && let Some(matte_index) =
                    find_track_matte_source_index(context, context.layers, index, layer)
            {
                let matte_surface = self.render_layer_surface(
                    context,
                    frame,
                    &context.layers[matte_index],
                    inherited_transform,
                    true,
                )?;
                let Some(next_surface) =
                    apply_track_matte(surface, matte_surface.as_ref(), matte_mode)?
                else {
                    consumed_mattes[matte_index] = true;
                    continue;
                };
                surface = next_surface;
                consumed_mattes[matte_index] = true;
            }

            composite_pixmap(
                pixmap,
                &surface.pixmap,
                surface.origin_x,
                surface.origin_y,
                BlendMode::SourceOver,
            );
        }

        Ok(())
    }

    fn render_layer_content(
        &self,
        context: &LayerRenderContext<'_>,
        frame: f32,
        layer: &Layer,
        inherited_transform: RenderTransform,
        pixmap: &mut Pixmap,
    ) -> Result<(), LottieError> {
        span_enter!(
            tracing::Level::TRACE,
            "render_layer_content",
            frame = frame,
            layer = layer.name.as_str(),
            layer_type = layer.layer_type.name()
        );
        let transform =
            inherited_transform.concat(resolve_layer_transform_chain(context, layer, frame));

        match layer.layer_type {
            LayerType::SHAPE => {
                let styles = ShapeStyles::default();
                render_shape_items(
                    &layer.shapes,
                    frame,
                    pixmap,
                    transform,
                    ShapeRenderState {
                        styles: &styles,
                        trim: None,
                        static_path_cache: context.static_path_cache,
                        shape_plan_cache: context.shape_plan_cache,
                        timeline_sample_cache: context.timeline_sample_cache,
                    },
                )?;
            }
            LayerType::PRECOMP => {
                let child_frame = map_layer_frame(context.animation, frame, layer);
                if let Some(asset) = lookup_precomp_asset(context.animation, layer) {
                    let child_context = LayerRenderContext {
                        animation: context.animation,
                        layers: &asset.layers,
                        image_assets: context.image_assets,
                        static_path_cache: context.static_path_cache,
                        layer_hierarchy_cache: context.layer_hierarchy_cache,
                        shape_plan_cache: context.shape_plan_cache,
                        timeline_sample_cache: context.timeline_sample_cache,
                        frame_cache: context.frame_cache,
                        canvas_width: context.canvas_width,
                        canvas_height: context.canvas_height,
                    };
                    self.render_layer_stack(&child_context, child_frame, pixmap, transform)?;
                }
            }
            // UZ: Rasm qatlamlari rad etiladi (`support` tahlilchisi), bu yerga kelmaydi. EN: Image layers are rejected by the `support` analyzer and never reach here. RU: Слои-изображения отклоняются анализатором `support` и сюда не попадают.
            LayerType::IMAGE => {}
            LayerType::TEXT => {
                Self::render_text_layer(
                    context.animation,
                    layer,
                    frame,
                    pixmap,
                    transform,
                    ShapeCaches {
                        static_paths: context.static_path_cache,
                        plans: context.shape_plan_cache,
                        timeline_samples: context.timeline_sample_cache,
                    },
                )?;
            }
            _ => {}
        }

        Ok(())
    }

    fn render_layer_surface(
        &self,
        context: &LayerRenderContext<'_>,
        frame: f32,
        layer: &Layer,
        inherited_transform: RenderTransform,
        force_visible: bool,
    ) -> Result<Option<LayerSurface>, LottieError> {
        span_enter!(
            tracing::Level::TRACE,
            "render_layer_surface",
            frame = frame,
            layer = layer.name.as_str(),
            force_visible = force_visible
        );
        if !frame_in_range(frame, layer.in_point, layer.out_point) {
            return Ok(None);
        }

        if !force_visible && layer.hidden {
            return Ok(None);
        }

        let mut pixmap = new_pixmap(context.canvas_width, context.canvas_height)?;
        self.render_layer_content(context, frame, layer, inherited_transform, &mut pixmap)?;

        apply_supported_layer_effects(&mut pixmap, layer, frame);

        if !layer.masks.is_empty() {
            let transform =
                inherited_transform.concat(resolve_layer_transform_chain(context, layer, frame));
            let mask = build_layer_mask(context, layer, frame, transform)?;
            pixmap = apply_alpha_mask(pixmap, &mask);
        }
        crop_layer_surface(&pixmap)
    }
}

impl PreparedAnimation {
    /// UZ: Tayyorlov natijasidan foydalanib bitta kadrni chizadi.
    /// EN: Renders one frame reusing the stored preprocessing results.
    /// RU: Рисует один кадр, используя сохранённые результаты подготовки.
    pub fn render_frame(
        &self,
        frame: f32,
        config: RenderConfig,
    ) -> Result<super::RasterFrame, LottieError> {
        self.renderer.render_frame_with_assets(
            &self.animation,
            frame,
            config,
            self.prepared_resources(),
        )
    }

    pub fn new_scratch_pixmap(&self) -> Result<Pixmap, LottieError> {
        new_pixmap(self.animation.width, self.animation.height)
    }

    pub fn new_scratch_pixmap_for_config(
        &self,
        config: RenderConfig,
    ) -> Result<Pixmap, LottieError> {
        let (width, height) = resolve_output_canvas_size(&self.animation, config)?;
        new_pixmap(width, height)
    }

    pub fn render_frame_into_pixmap(
        &self,
        frame: f32,
        config: RenderConfig,
        pixmap: &mut Pixmap,
    ) -> Result<(), LottieError> {
        self.renderer.render_frame_with_assets_into(
            &self.animation,
            frame,
            config,
            self.prepared_resources(),
            pixmap,
        )
    }
}

impl super::RenderTransform {
    pub(in super::super) fn concat(self, other: Self) -> Self {
        Self {
            matrix: self.matrix.pre_concat(other.matrix),
            opacity: self.opacity * other.opacity,
        }
    }
}
