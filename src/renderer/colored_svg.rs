use std::mem;

use glam::{Mat4, Vec4};

use crate::GraphicsContext;
use crate::memory::{BumpInstances, Instances};
use crate::resources::mesh_2d::{Mesh2DId, Mesh2DInstanceId, Mesh2DResources, Mesh2DVertex};

#[repr(C)]
#[derive(Default, Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ColoredSvgMeshInstance {
    mvp_matrix: [[f32; 4]; 4],
    color: [f32; 4],
    mesh_id: u32,
}

impl ColoredSvgMeshInstance {
    pub fn new(mesh_id: Mesh2DId, mvp_matrix: &Mat4, color: &Vec4) -> Self {
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

pub struct ColoredSvgRenderer<
    I: Instances<Mesh2DInstanceId, ColoredSvgMeshInstance> = BumpInstances<
        Mesh2DInstanceId,
        ColoredSvgMeshInstance,
    >,
> {
    shader: wgpu::ShaderModule,
    render_pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    instances: I,
}

impl<I: Instances<Mesh2DInstanceId, ColoredSvgMeshInstance>> ColoredSvgRenderer<I> {
    pub fn new(
        context: &GraphicsContext<'_, '_>,
        mesh_2d_resources: &Mesh2DResources,
        mut instances: I,
    ) -> Self {
        instances.create_buffer(context, |index, instance| Mesh2DInstanceId {
            mesh_id: Mesh2DId {
                value: instance.mesh_id as usize,
            },
            value: index as u32,
        });

        let shader = context
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("SVG Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/svg_color.wgsl").into()),
            });

        let (bind_group_layout, bind_group) =
            ColoredSvgRenderer::<I>::create_bind_group(context.device, mesh_2d_resources);

        let render_pipeline =
            ColoredSvgRenderer::<I>::create_pipeline(context, &shader, &bind_group_layout);

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
            ColoredSvgRenderer::<I>::create_bind_group(context.device, mesh_2d_resources);

        self.render_pipeline =
            ColoredSvgRenderer::<I>::create_pipeline(context, &self.shader, &bind_group_layout);

        self.bind_group_layout = bind_group_layout;
        self.bind_group = bind_group;
    }

    fn create_bind_group(
        device: &wgpu::Device,
        mesh_2d_resources: &Mesh2DResources,
    ) -> (wgpu::BindGroupLayout, wgpu::BindGroup) {
        let storage = mesh_2d_resources.storage();

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Colored SVG Bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(storage.primitive_buffer_byte_size),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(storage.transform_buffer_byte_size),
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
                            storage.mesh_sizes_buffer_byte_size,
                        ),
                    },
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Colored SVG Bind group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(
                        storage.primitives.as_entire_buffer_binding(),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(
                        storage.transforms.as_entire_buffer_binding(),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Buffer(
                        storage.mesh_sizes.as_entire_buffer_binding(),
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
                    immediate_size: 0,
                    label: None,
                });

        context
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Colored SVG Render Pipeline"),
                layout: Some(&pipeline_layout),
                cache: None,
                vertex: wgpu::VertexState {
                    module: shader,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[Mesh2DVertex::desc(), ColoredSvgMeshInstance::desc()],
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

        mesh_2d_resources.render(context, instance_id);
    }

    pub fn render_all_instances(
        &mut self,
        context: &mut GraphicsContext<'_, '_>,
        mesh_2d_resources: &Mesh2DResources,
    ) {
        for instance_id in self.instances.ids() {
            context.render_pass().set_pipeline(&self.render_pipeline);
            context
                .render_pass()
                .set_bind_group(0, &self.bind_group, &[]);
            context
                .render_pass()
                .set_vertex_buffer(1, self.instances.gpu_buffer().slice(..));

            mesh_2d_resources.render(context, *instance_id);
        }
    }
}
