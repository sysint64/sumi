use std::mem;

use glam::Mat4;

use crate::SlotId;
use crate::graphics_context::GraphicsContext;
use crate::memory_new::{BumpBuffer, GpuBuffer, SlottedBuffer};
use crate::resources::mesh_2d::Mesh2DVertex;
use crate::resources::mesh_2d_new::{Mesh2DId, Mesh2DInstanceId, Mesh2DResources};


#[repr(C)]
#[derive(Default, Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SvgMeshInstance {
    mvp_matrix: [[f32; 4]; 4],
    mesh_id: u32,
}

impl SvgMeshInstance {
    pub fn new(mesh_id: Mesh2DId, mvp_matrix: &Mat4) -> Self {
        SvgMeshInstance {
            mesh_id: mesh_id.value as u32,
            mvp_matrix: mvp_matrix.to_cols_array_2d(),
        }
    }

    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<SvgMeshInstance>() as wgpu::BufferAddress,
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
                    format: wgpu::VertexFormat::Uint32,
                },
            ],
        }
    }
}

pub struct SvgRenderer<
    I: SlottedBuffer<Mesh2DInstanceId, SvgMeshInstance> = BumpBuffer<
        Mesh2DInstanceId,
        SvgMeshInstance,
    >,
> {
    shader: wgpu::ShaderModule,
    render_pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    instances: I,
}

impl<I: SlottedBuffer<Mesh2DInstanceId, SvgMeshInstance>> SvgRenderer<I> {
    pub fn new(
        context: &GraphicsContext<'_, '_>,
        mesh_2d_resources: &Mesh2DResources,
        instances: I,
    ) -> Self {
        let shader = context
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("SVG Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/svg.wgsl").into()),
            });

        let (bind_group_layout, bind_group) =
            Self::create_bind_group(context.device, mesh_2d_resources);

        let render_pipeline = Self::create_pipeline(context, &shader, &bind_group_layout);

        Self {
            shader,
            instances,
            render_pipeline,
            bind_group_layout,
            bind_group,
        }
    }

    pub fn instances(&mut self) -> &mut I {
        &mut self.instances
    }

    pub fn on_gpu_storage_update(
        &mut self,
        context: &GraphicsContext<'_, '_>,
        mesh_2d_resources: &Mesh2DResources,
    ) {
        let (bind_group_layout, bind_group) =
            Self::create_bind_group(context.device, mesh_2d_resources);

        self.render_pipeline =
            Self::create_pipeline(context, &self.shader, &bind_group_layout);

        self.bind_group_layout = bind_group_layout;
        self.bind_group = bind_group;
    }

    fn create_bind_group(
        device: &wgpu::Device,
        mesh_2d_resources: &Mesh2DResources,
    ) -> (wgpu::BindGroupLayout, wgpu::BindGroup) {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SVG Bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            mesh_2d_resources.primitives_byte_size(),
                        ),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            mesh_2d_resources.transforms_byte_size(),
                        ),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            mesh_2d_resources.mesh_sizes_byte_size(),
                        ),
                    },
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SVG Bind group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(
                        mesh_2d_resources.primitives_buffer().as_entire_buffer_binding(),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(
                        mesh_2d_resources.transforms_buffer().as_entire_buffer_binding(),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Buffer(
                        mesh_2d_resources.mesh_sizes_buffer().as_entire_buffer_binding(),
                    ),
                },
            ],
        });

        (bind_group_layout, bind_group)
    }

    fn create_pipeline(
        context: &GraphicsContext<'_, '_>,
        shader: &wgpu::ShaderModule,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> wgpu::RenderPipeline {
        let pipeline_layout =
            context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    bind_group_layouts: &[Some(bind_group_layout)],
                    label: None,
                    immediate_size: 0,
                });

        context
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("SVG Render Pipeline"),
                layout: Some(&pipeline_layout),
                cache: None,
                vertex: wgpu::VertexState {
                    module: shader,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[Mesh2DVertex::desc(), SvgMeshInstance::desc()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: shader,
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
            })
    }

    pub fn render_instance(
        &mut self,
        context: &mut GraphicsContext<'_, '_>,
        mesh_2d_resources: &Mesh2DResources,
        instance_id: Mesh2DInstanceId,
    ) {
        context.render_pass().set_pipeline(&self.render_pipeline);
        context
            .render_pass()
            .set_bind_group(0, &self.bind_group, &[]);
        context
            .render_pass()
            .set_vertex_buffer(1, self.instances.gpu_buffer().slice(..));

        let slot = instance_id.value;
        let mesh_id = Mesh2DId {
            value: self.instances.data()[instance_id.index()].mesh_id as usize,
        };
        mesh_2d_resources.render_slot(context, slot, mesh_id);
    }

    pub fn render_all_instances(
        &mut self,
        context: &mut GraphicsContext<'_, '_>,
        mesh_2d_resources: &Mesh2DResources,
    ) {
        context.render_pass().set_pipeline(&self.render_pipeline);
        context
            .render_pass()
            .set_bind_group(0, &self.bind_group, &[]);
        context
            .render_pass()
            .set_vertex_buffer(1, self.instances.gpu_buffer().slice(..));

        for &instance_id in self.instances.ids() {
            let slot = instance_id.value;
            let mesh_id = Mesh2DId {
                value: self.instances.data()[instance_id.index()].mesh_id as usize,
            };
            mesh_2d_resources.render_slot(context, slot, mesh_id);
        }
    }
}
