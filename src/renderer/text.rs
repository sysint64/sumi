use glam::{Vec2, Vec4};
use slotmap::{DenseSlotMap, new_key_type};

use crate::graphics_context::GraphicsContext;

new_key_type! {
    pub struct TextInstanceId;
}

pub struct TextRenderer {
    swash_cache: glyphon::SwashCache,
    viewport: glyphon::Viewport,
    atlas: glyphon::TextAtlas,
    glyphon_renderer: glyphon::TextRenderer,
    instances: DenseSlotMap<TextInstanceId, TextInstance>,
}

#[derive(Copy, Clone)]
pub struct TextInstance {
    text_id: cosmic_text_kv::TextId,
    position: Vec2,
    color: glyphon::Color,
    bounds: glyphon::TextBounds,
}

impl TextInstance {
    pub fn new(text_id: cosmic_text_kv::TextId, position: Vec2, color: Vec4) -> Self {
        Self {
            text_id,
            position,
            color: glyphon::Color::rgba(
                (color[0] * 255.).round().clamp(0., 255.) as u8,
                (color[1] * 255.).round().clamp(0., 255.) as u8,
                (color[2] * 255.).round().clamp(0., 255.) as u8,
                (color[3] * 255.).round().clamp(0., 255.) as u8,
            ),
            bounds: Default::default(),
        }
    }

    pub fn new_bounded(
        text_id: cosmic_text_kv::TextId,
        position: Vec2,
        color: Vec4,
        bounds: lattice::Rect<f32>,
    ) -> Self {
        let bounds = glyphon::TextBounds {
            left: bounds.left() as i32,
            top: bounds.top() as i32,
            right: bounds.right() as i32,
            bottom: bounds.bottom() as i32,
        };

        Self {
            text_id,
            position,
            bounds,
            color: glyphon::Color::rgba(
                (color[0] * 255.).round().clamp(0., 255.) as u8,
                (color[1] * 255.).round().clamp(0., 255.) as u8,
                (color[2] * 255.).round().clamp(0., 255.) as u8,
                (color[3] * 255.).round().clamp(0., 255.) as u8,
            ),
        }
    }

    pub fn set_bounds(&mut self, bounds: lattice::Rect<f32>) {
        self.bounds = glyphon::TextBounds {
            left: bounds.left() as i32,
            top: bounds.top() as i32,
            right: bounds.right() as i32,
            bottom: bounds.bottom() as i32,
        };
    }
}

impl TextRenderer {
    pub fn new(context: &GraphicsContext<'_, '_>) -> Self {
        let swash_cache = glyphon::SwashCache::new();
        let cache = glyphon::Cache::new(context.device);
        let mut viewport = glyphon::Viewport::new(context.device, &cache);

        viewport.update(
            context.queue,
            glyphon::Resolution {
                width: context.view_size.x as u32,
                height: context.view_size.y as u32,
            },
        );

        let mut atlas = glyphon::TextAtlas::new(
            context.device,
            context.queue,
            &cache,
            context.surface_texture_format,
        );

        let glyphon_renderer = glyphon::TextRenderer::new(
            &mut atlas,
            context.device,
            wgpu::MultisampleState::default(),
            None,
        );

        Self {
            swash_cache,
            viewport,
            atlas,
            glyphon_renderer,
            instances: DenseSlotMap::default(),
        }
    }

    pub fn instances(&mut self) -> &mut DenseSlotMap<TextInstanceId, TextInstance> {
        &mut self.instances
    }

    pub fn update_viewport(&mut self, context: &GraphicsContext<'_, '_>) {
        self.viewport.update(
            context.queue,
            glyphon::Resolution {
                width: context.view_size.x as u32,
                height: context.view_size.y as u32,
            },
        );
    }

    pub fn render_all_instances(
        &mut self,
        context: &mut GraphicsContext<'_, '_>,
        font_resources: &mut cosmic_text_kv::FontResources,
        text_resources: &cosmic_text_kv::TextsResources,
    ) {
        let mut text_areas = Vec::with_capacity(self.instances.len());

        for item in self.instances.values() {
            let text = text_resources.get(item.text_id);
            let area = glyphon::TextArea {
                buffer: text.buffer(),
                top: item.position.y,
                left: item.position.x,
                scale: 1.0,
                bounds: item.bounds,
                default_color: item.color,
                custom_glyphs: &[],
            };
            text_areas.push(area);
        }

        self.glyphon_renderer
            .prepare(
                context.device,
                context.queue,
                &mut font_resources.font_system,
                &mut self.atlas,
                &self.viewport,
                text_areas,
                &mut self.swash_cache,
            )
            .unwrap();

        self.glyphon_renderer
            .render(&self.atlas, &self.viewport, context.render_pass())
            .unwrap();

        self.atlas.trim();
    }
}
