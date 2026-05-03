use glam::{Mat4, Vec4};

use crate::graphics_context::GraphicsContext;
use crate::memory::{BumpInstances, InstanceId, Instances};
use crate::resources::instancing_geometry::InstancingGeometry;
use crate::resources::vertex::TexturedVertex;

#[derive(Default, Debug, Copy, Clone)]
pub struct FilledCircleInstanceId {
    value: u32,
}

impl InstanceId for FilledCircleInstanceId {
    fn index(&self) -> usize {
        self.value as usize
    }
}

impl PartialEq for FilledCircleInstanceId {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

#[repr(C)]
#[derive(Default, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FilledCircleInstance {
    pub color: [f32; 4],
    pub mvp_matrix: [[f32; 4]; 4],
}

impl FilledCircleInstance {
    pub fn new(mvp_matrix: &Mat4, color: &Vec4) -> Self {
        FilledCircleInstance {
            mvp_matrix: mvp_matrix.to_cols_array_2d(),
            color: color.to_array(),
        }
    }

    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;
        let size_of = mem::size_of::<FilledCircleInstance>();
        wgpu::VertexBufferLayout {
            array_stride: size_of as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 16]>() as wgpu::BufferAddress,
                    shader_location: 9,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

pub struct FilledCircleRenderer<
    I: Instances<FilledCircleInstanceId, FilledCircleInstance> = BumpInstances<
        FilledCircleInstanceId,
        FilledCircleInstance,
    >,
> {
    render_pipeline: wgpu::RenderPipeline,
    instances: I,
}

impl<I: Instances<FilledCircleInstanceId, FilledCircleInstance>> FilledCircleRenderer<I> {
    pub fn new(context: &GraphicsContext<'_, '_>, mut instances: I) -> Self {
        instances.create_buffer(context, |index, _| FilledCircleInstanceId {
            value: index as u32,
        });

        let shader = context
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Filled Circle Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/filled_circle.wgsl").into()),
            });

        let render_pipeline_layout =
            context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Filled Circles Render Pipeline Layout"),
                    bind_group_layouts: &[],
                    immediate_size: 0,
                });

        let render_pipeline =
            context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Filled Circles Render Pipeline"),
                    layout: Some(&render_pipeline_layout),
                    cache: None,
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs_main"),
                        buffers: &[TexturedVertex::desc(), FilledCircleInstance::desc()],
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
        id: FilledCircleInstanceId,
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
