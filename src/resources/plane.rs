use std::ops::Range;

use wgpu::util::DeviceExt;

use crate::graphics_context::GraphicsContext;

use super::{instancing_geometry::InstancingGeometry, vertex::TexturedVertex};

pub const PLANE_VERTICES: &[TexturedVertex] = &[
    TexturedVertex {
        position: [0.0, 0.0, 0.0],
        tex_coords: [0.0, 1.0],
    },
    TexturedVertex {
        position: [1.0, 0.0, 0.0],
        tex_coords: [1.0, 1.0],
    },
    TexturedVertex {
        position: [1.0, 1.0, 0.0],
        tex_coords: [1.0, 0.0],
    },
    TexturedVertex {
        position: [0.0, 1.0, 0.0],
        tex_coords: [0.0, 0.0],
    },
];

pub const PLANE_INDICES: &[u16] = &[0, 3, 1, 2];

pub struct PlaneResources {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
}

impl PlaneResources {
    pub fn new(context: &GraphicsContext<'_, '_>) -> Self {
        let vertex_buffer = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Plane Vertex Buffer"),
                contents: bytemuck::cast_slice(PLANE_VERTICES),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let index_buffer = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Plane Index Buffer"),
                contents: bytemuck::cast_slice(PLANE_INDICES),
                usage: wgpu::BufferUsages::INDEX,
            });

        Self {
            vertex_buffer,
            index_buffer,
        }
    }
}

impl InstancingGeometry for PlaneResources {
    fn render_instances(&self, context: &mut GraphicsContext<'_, '_>, instances: Range<u32>) {
        context
            .render_pass()
            .set_vertex_buffer(0, self.vertex_buffer.slice(..));
        context
            .render_pass()
            .set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        context
            .render_pass()
            .draw_indexed(0..PLANE_INDICES.len() as u32, 0, instances);
    }
}
