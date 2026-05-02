pub(crate) use std::mem;

use glam::{Mat4, Vec4};

use crate::graphics_context::GraphicsContext;
use crate::resources::instancing_geometry::InstancingGeometry;
use crate::resources::vertex::TexturedVertex;

use crate::memory::{BumpInstances, InstanceId, Instances};

#[derive(Default, Copy, Clone)]
pub struct ColoredPlaneInstanceId {
    value: u32,
}

impl InstanceId for ColoredPlaneInstanceId {
    fn index(&self) -> usize {
        self.value as usize
    }
}

impl PartialEq for ColoredPlaneInstanceId {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

#[repr(C)]
#[derive(Default, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ColoredPlaneInstance {
    mvp_matrix: [[f32; 4]; 4],
    color: [f32; 4],
}

impl ColoredPlaneInstance {
    pub fn new(mvp_matrix: &Mat4, color: &Vec4) -> Self {
        ColoredPlaneInstance {
            mvp_matrix: mvp_matrix.to_cols_array_2d(),
            color: color.to_array(),
        }
    }

    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<ColoredPlaneInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 16]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

pub struct ColoredPlaneRenderer<
    I: Instances<ColoredPlaneInstanceId, ColoredPlaneInstance> = BumpInstances<
        ColoredPlaneInstanceId,
        ColoredPlaneInstance,
    >,
> {
    render_pipeline: wgpu::RenderPipeline,
    instances: I,
}

impl<I: Instances<ColoredPlaneInstanceId, ColoredPlaneInstance>> ColoredPlaneRenderer<I> {
    pub fn new(context: &GraphicsContext<'_, '_>, mut instances: I) -> Self {
        instances.create_buffer(context, |index, _| ColoredPlaneInstanceId {
            value: index as u32,
        });

        let shader = context
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Color Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/colored_plane.wgsl").into()),
            });

        let render_pipeline_layout =
            context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Render Pipeline Layout"),
                    bind_group_layouts: &[],
                    immediate_size: 0,
                });

        let render_pipeline =
            context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Render Pipeline"),
                    layout: Some(&render_pipeline_layout),
                    cache: None,
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs_main"),
                        buffers: &[TexturedVertex::desc(), ColoredPlaneInstance::desc()],
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
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Cw,
                        cull_mode: Some(wgpu::Face::Back),
                        polygon_mode: wgpu::PolygonMode::Fill,
                        unclipped_depth: false,
                        conservative: false,
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState {
                        count: 1,
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
        id: ColoredPlaneInstanceId,
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
