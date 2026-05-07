use std::ops::Range;

use wgpu::util::DeviceExt;

use crate::graphics_context::GraphicsContext;

use super::{instancing_geometry::InstancingGeometry, vertex::TexturedVertex};

pub const CENTERED_PLANE_VERTICES: &[TexturedVertex] = &[
    TexturedVertex {
        position: [-0.5, -0.5, 0.0],
        tex_coords: [0.0, 1.0],
    },
    TexturedVertex {
        position: [0.5, -0.5, 0.0],
        tex_coords: [1.0, 1.0],
    },
    TexturedVertex {
        position: [0.5, 0.5, 0.0],
        tex_coords: [1.0, 0.0],
    },
    TexturedVertex {
        position: [-0.5, 0.5, 0.0],
        tex_coords: [0.0, 0.0],
    },
];

pub const CENTERED_PLANE_INDICES: &[u16] = &[0, 3, 1, 2];

pub struct CenteredPlaneResources {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
}

impl CenteredPlaneResources {
    pub fn new(context: &GraphicsContext<'_, '_>) -> Self {
        let vertex_buffer = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Centered Plane Vertex Buffer"),
                contents: bytemuck::cast_slice(CENTERED_PLANE_VERTICES),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let index_buffer = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Centered Plane Index Buffer"),
                contents: bytemuck::cast_slice(CENTERED_PLANE_INDICES),
                usage: wgpu::BufferUsages::INDEX,
            });

        Self {
            vertex_buffer,
            index_buffer,
        }
    }
}

impl InstancingGeometry for CenteredPlaneResources {
    fn render_instances(&self, context: &mut GraphicsContext<'_, '_>, instances: Range<u32>) {
        context
            .render_pass()
            .set_vertex_buffer(0, self.vertex_buffer.slice(..));
        context
            .render_pass()
            .set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        context
            .render_pass()
            .draw_indexed(0..CENTERED_PLANE_INDICES.len() as u32, 0, instances);
    }

    fn primitive() -> wgpu::PrimitiveState {
        wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            strip_index_format: Some(wgpu::IndexFormat::Uint16),
            front_face: wgpu::FrontFace::Cw,
            cull_mode: Some(wgpu::Face::Back),
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        }
    }
}
