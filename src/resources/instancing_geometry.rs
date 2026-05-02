use std::ops::Range;

use crate::graphics_context::GraphicsContext;

pub trait InstancingGeometry {
    fn render_instances(&self, context: &mut GraphicsContext<'_, '_>, instances: Range<u32>);
}
