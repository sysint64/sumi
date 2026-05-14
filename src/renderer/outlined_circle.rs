use glam::{Mat4, Vec4};

use crate::graphics_context::GraphicsContext;
use crate::instances::RenderInstances;
use crate::memory::SlotId;
use crate::resources::instancing_geometry::InstancingGeometry;
use crate::resources::vertex::TexturedVertex;

#[derive(Default, Debug, Copy, Clone, PartialEq)]
pub struct OutlinedCircleInstanceId {
    value: u32,
}

impl SlotId for OutlinedCircleInstanceId {
    fn from_index(index: usize) -> Self {
        OutlinedCircleInstanceId { value: index as u32 }
    }

    fn index(&self) -> usize {
        self.value as usize
    }
}

#[repr(C)]
#[derive(Default, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct OutlinedCircleInstance {
    line_width: f32,
    radius: f32,
    color: [f32; 4],
    mvp_matrix: [[f32; 4]; 4],
}

impl OutlinedCircleInstance {
    pub fn new(line_width: f32, radius: f32, mvp_matrix: &Mat4, color: &Vec4) -> Self {
        OutlinedCircleInstance {
            line_width,
            radius,
            mvp_matrix: mvp_matrix.to_cols_array_2d(),
            color: color.to_array(),
        }
    }

    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;
        let size_of = mem::size_of::<OutlinedCircleInstance>();
        wgpu::VertexBufferLayout {
            array_stride: size_of as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 1]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 6]>() as wgpu::BufferAddress,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 10]>() as wgpu::BufferAddress,
                    shader_location: 9,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 14]>() as wgpu::BufferAddress,
                    shader_location: 10,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 18]>() as wgpu::BufferAddress,
                    shader_location: 11,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

pub struct OutlinedCircleRenderer {
    render_pipeline: wgpu::RenderPipeline,
}

impl OutlinedCircleRenderer {
    pub fn new(context: &GraphicsContext<'_, '_>) -> Self {
        let shader = context
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Outlined Circle Shader"),
                source: wgpu::ShaderSource::Wgsl(
                    include_str!("shaders/outlined_circle.wgsl").into(),
                ),
            });

        let render_pipeline_layout =
            context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Outlined Circles Render Pipeline Layout"),
                    bind_group_layouts: &[],
                    immediate_size: 0,
                });

        let render_pipeline =
            context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Outlined Circles Render Pipeline"),
                    layout: Some(&render_pipeline_layout),
                    cache: None,
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs_main"),
                        buffers: &[TexturedVertex::desc(), OutlinedCircleInstance::desc()],
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

        Self { render_pipeline }
    }

    pub fn render_all_instances<I, T>(
        &self,
        context: &mut GraphicsContext<'_, '_>,
        geometry: &T,
        instances: &mut I,
    ) where
        I: RenderInstances<OutlinedCircleInstanceId, OutlinedCircleInstance>,
        T: InstancingGeometry,
    {
        context.render_pass().set_pipeline(&self.render_pipeline);
        instances.bind(1, context);

        for range in instances.ranges(context) {
            geometry.render_instances(context, range);
        }
    }

    pub fn render_instance<I, T>(
        &self,
        context: &mut GraphicsContext<'_, '_>,
        geometry: &T,
        id: OutlinedCircleInstanceId,
        instances: &I,
    ) where
        I: RenderInstances<OutlinedCircleInstanceId, OutlinedCircleInstance>,
        T: InstancingGeometry,
    {
        context.render_pass().set_pipeline(&self.render_pipeline);
        context
            .render_pass()
            .set_vertex_buffer(1, instances.gpu_buffer().slice(..));
        geometry.render_instances(context, id.range());
    }
}
