use std::mem;

use glam::{Mat4, Vec4};

use crate::graphics_context::GraphicsContext;
use crate::memory_new::SlotId;
use crate::prelude::RenderInstances;
use crate::resources::mesh_2d::Mesh2DResources;
use crate::{Mesh2DId, Mesh2DVertex};

use super::bind_group::SvgBindGroup;

#[derive(Default, Copy, Clone, PartialEq)]
pub struct ColoredSvgMeshInstanceId {
    value: u32,
}

impl SlotId for ColoredSvgMeshInstanceId {
    fn from_index(index: usize) -> Self {
        ColoredSvgMeshInstanceId {
            value: index as u32,
        }
    }

    fn index(&self) -> usize {
        self.value as usize
    }
}

#[repr(C)]
#[derive(Default, Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ColoredSvgMeshInstance {
    mvp_matrix: [[f32; 4]; 4],
    color: [f32; 4],
    mesh_id: u32,
}

impl ColoredSvgMeshInstance {
    pub fn new(mesh_id: Mesh2DId, mvp_matrix: &Mat4, color: Vec4) -> Self {
        ColoredSvgMeshInstance {
            mesh_id: mesh_id.value as u32,
            mvp_matrix: mvp_matrix.to_cols_array_2d(),
            color: color.to_array(),
        }
    }

    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<ColoredSvgMeshInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 16]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 20]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Uint32,
                },
            ],
        }
    }
}

pub struct ColoredSvgRenderer {
    render_pipeline: wgpu::RenderPipeline,
    bind_group: SvgBindGroup,
}

impl ColoredSvgRenderer {
    pub fn new(context: &GraphicsContext<'_, '_>, resources: &Mesh2DResources) -> Self {
        let shader = context
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Colored SVG Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/svg_color.wgsl").into()),
            });

        let bind_group = SvgBindGroup::new(context.device, resources);

        let pipeline_layout =
            context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    bind_group_layouts: &[Some(bind_group.layout())],
                    label: None,
                    immediate_size: 0,
                });

        let render_pipeline =
            context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Colored SVG Render Pipeline"),
                    layout: Some(&pipeline_layout),
                    cache: None,
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs_main"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        buffers: &[Mesh2DVertex::desc(), ColoredSvgMeshInstance::desc()],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some("fs_main"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: context.surface_texture_format,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        polygon_mode: wgpu::PolygonMode::Fill,
                        front_face: wgpu::FrontFace::Ccw,
                        strip_index_format: None,
                        cull_mode: None,
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
            render_pipeline,
            bind_group,
        }
    }

    pub fn rebuild_bind_group(&mut self, device: &wgpu::Device, resources: &Mesh2DResources) {
        self.bind_group.rebuild(device, resources);
    }

    pub fn render_instance<I>(
        &self,
        context: &mut GraphicsContext<'_, '_>,
        resources: &Mesh2DResources,
        instance_id: ColoredSvgMeshInstanceId,
        instances: &I,
    ) where
        I: RenderInstances<ColoredSvgMeshInstanceId, ColoredSvgMeshInstance>,
    {
        context.render_pass().set_pipeline(&self.render_pipeline);
        context
            .render_pass()
            .set_bind_group(0, self.bind_group.bind_group(), &[]);
        context
            .render_pass()
            .set_vertex_buffer(1, instances.gpu_buffer().slice(..));

        let slot = instance_id.value;
        let mesh_id = Mesh2DId {
            value: instances.data()[instance_id.index()].mesh_id as usize,
        };
        let mesh = resources.mesh_ref(mesh_id);

        context
            .render_pass()
            .set_vertex_buffer(0, mesh.vertices.slice(..));
        context
            .render_pass()
            .set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
        context
            .render_pass()
            .draw_indexed(0..mesh.indices_len, 0, slot..slot + 1);
    }

    pub fn render_all_instances<I>(
        &self,
        context: &mut GraphicsContext<'_, '_>,
        resources: &Mesh2DResources,
        instances: &mut I,
    ) where
        I: RenderInstances<ColoredSvgMeshInstanceId, ColoredSvgMeshInstance>,
    {
        context.render_pass().set_pipeline(&self.render_pipeline);
        context
            .render_pass()
            .set_bind_group(0, self.bind_group.bind_group(), &[]);
        context
            .render_pass()
            .set_vertex_buffer(1, instances.gpu_buffer().slice(..));

        for range in instances.drain(context) {
            for index in range {
                let id = ColoredSvgMeshInstanceId::from_index(index as usize);
                let mesh_id = Mesh2DId {
                    value: instances.data()[id.index()].mesh_id as usize,
                };
                let mesh = resources.mesh_ref(mesh_id);

                context
                    .render_pass()
                    .set_vertex_buffer(0, mesh.vertices.slice(..));
                context
                    .render_pass()
                    .set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                context
                    .render_pass()
                    .draw_indexed(0..mesh.indices_len, 0, index..index + 1);
            }
        }
    }
}
