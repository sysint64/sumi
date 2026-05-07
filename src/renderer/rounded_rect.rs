use std::mem;

use glam::{Mat4, Vec2, Vec4};

use crate::graphics_context::GraphicsContext;
use crate::memory::{BumpInstances, InstanceId, Instances};
use crate::resources::instancing_geometry::InstancingGeometry;
use crate::resources::vertex::TexturedVertex;

#[derive(Default, Debug, Copy, Clone)]
pub struct RoundedRectInstanceId {
    value: u32,
}

impl InstanceId for RoundedRectInstanceId {
    fn index(&self) -> usize {
        self.value as usize
    }
}

impl PartialEq for RoundedRectInstanceId {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

/// Border widths per side, in the same pixel units as `size`.
#[derive(Default, Copy, Clone)]
pub struct BorderWidths {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl BorderWidths {
    pub fn all(width: f32) -> Self {
        Self {
            top: width,
            right: width,
            bottom: width,
            left: width,
        }
    }
}

#[repr(C)]
#[derive(Default, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RoundedRectInstance {
    mvp_matrix: [[f32; 4]; 4], // offset   0, 64 bytes
    fill_color: [f32; 4],      // offset  64, 16 bytes
    border_color: [f32; 4],    // offset  80, 16 bytes
    border_widths: [f32; 4],   // offset  96, 16 bytes  (top, right, bottom, left)
    size: [f32; 2],            // offset 112,  8 bytes
    border_radius: f32,        // offset 120,  4 bytes
    _pad: f32,                 // offset 124,  4 bytes
}

impl RoundedRectInstance {
    /// - `size`          — width and height in local pixel units (should match the
    ///                     `scaling` used to build the MVP matrix).
    /// - `border_radius` — corner radius in the same units as `size`.
    /// - `border_widths` — per-side widths in the same units as `size`.
    pub fn new(
        mvp_matrix: &Mat4,
        size: Vec2,
        border_radius: f32,
        fill_color: Vec4,
        border_color: Vec4,
        border_widths: BorderWidths,
    ) -> Self {
        RoundedRectInstance {
            mvp_matrix: mvp_matrix.to_cols_array_2d(),
            fill_color: fill_color.to_array(),
            border_color: border_color.to_array(),
            border_widths: [
                border_widths.top,
                border_widths.right,
                border_widths.bottom,
                border_widths.left,
            ],
            size: size.to_array(),
            border_radius,
            _pad: 0.0,
        }
    }

    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<RoundedRectInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as u64,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as u64,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 12]>() as u64,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 16]>() as u64,
                    shader_location: 9,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 20]>() as u64,
                    shader_location: 10,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 24]>() as u64,
                    shader_location: 11,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 28]>() as u64,
                    shader_location: 12,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 30]>() as u64,
                    shader_location: 13,
                    format: wgpu::VertexFormat::Float32,
                },
            ],
        }
    }
}

pub struct RoundedRectRenderer<
    I: Instances<RoundedRectInstanceId, RoundedRectInstance> = BumpInstances<
        RoundedRectInstanceId,
        RoundedRectInstance,
    >,
> {
    render_pipeline: wgpu::RenderPipeline,
    instances: I,
}

impl<I: Instances<RoundedRectInstanceId, RoundedRectInstance>> RoundedRectRenderer<I> {
    pub fn new(context: &GraphicsContext<'_, '_>, mut instances: I) -> Self {
        instances.create_buffer(
            context,
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            |index, _| RoundedRectInstanceId {
                value: index as u32,
            },
        );

        let shader = context
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Rounded Rect Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/rounded_rect.wgsl").into()),
            });

        let pipeline_layout =
            context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Rounded Rect Pipeline Layout"),
                    bind_group_layouts: &[],
                    immediate_size: 0,
                });

        let render_pipeline =
            context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Rounded Rect Render Pipeline"),
                    layout: Some(&pipeline_layout),
                    cache: None,
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs_main"),
                        buffers: &[TexturedVertex::desc(), RoundedRectInstance::desc()],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some("fs_main"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: context.surface_texture_format,
                            blend: Some(wgpu::BlendState {
                                color: wgpu::BlendComponent {
                                    src_factor: wgpu::BlendFactor::SrcAlpha,
                                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                                    operation: wgpu::BlendOperation::Add,
                                },
                                alpha: wgpu::BlendComponent::OVER,
                            }),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleStrip,
                        strip_index_format: Some(wgpu::IndexFormat::Uint16),
                        front_face: wgpu::FrontFace::Cw,
                        cull_mode: Some(wgpu::Face::Back),
                        polygon_mode: wgpu::PolygonMode::Fill,
                        unclipped_depth: false,
                        conservative: false,
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState {
                        count: context.sample_count,
                        mask: !0,
                        alpha_to_coverage_enabled: false,
                    },
                    multiview_mask: None,
                });

        Self {
            instances,
            render_pipeline,
        }
    }

    pub fn instances(&mut self) -> &mut I {
        &mut self.instances
    }

    pub fn render_all_instances<T>(&mut self, context: &mut GraphicsContext<'_, '_>, geometry: &T)
    where
        T: InstancingGeometry,
    {
        for range in self.instances.ranges_iter() {
            context.render_pass().set_pipeline(&self.render_pipeline);
            context
                .render_pass()
                .set_vertex_buffer(1, self.instances.gpu_buffer().slice(..));
            geometry.render_instances(context, range);
        }
    }

    pub fn render_instance<T>(
        &mut self,
        context: &mut GraphicsContext<'_, '_>,
        geometry: &T,
        id: RoundedRectInstanceId,
    ) where
        T: InstancingGeometry,
    {
        debug_assert!(self.instances.contains(id), "Invalid ID");

        context.render_pass().set_pipeline(&self.render_pipeline);
        context
            .render_pass()
            .set_vertex_buffer(1, self.instances.gpu_buffer().slice(..));
        geometry.render_instances(context, id.value..id.value + 1);
    }
}
