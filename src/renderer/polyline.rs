use glam::Vec2;
use wgpu::util::DeviceExt;

use crate::{
    graphics_context::GraphicsContext,
    resources::polyline::{PolylineResources, PolylineVertex},
};

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct PolylineUniforms {
    viewport_size: [f32; 2],
}

pub struct PolylineRenderer {
    render_pipeline: wgpu::RenderPipeline,
    uniforms: PolylineUniforms,
    uniforms_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl PolylineRenderer {
    pub fn new(context: &GraphicsContext<'_, '_>) -> Self {
        let shader = context
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/polyline.wgsl").into()),
            });

        let uniforms = PolylineUniforms::default();
        let uniforms_buffer =
            context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Polyline Pipeline Buffer"),
                    contents: bytemuck::cast_slice(&[uniforms]),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });

        let bind_group_layout =
            context
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                    label: Some("polyline_pipeline_bind_group_layout"),
                });

        let bind_group = context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniforms_buffer.as_entire_binding(),
                }],
                label: Some("polyline_pipeline_bind_group"),
            });

        let render_pipeline_layout =
            context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Polyline Render Pipeline Layout"),
                    bind_group_layouts: &[Some(&bind_group_layout)],
                    immediate_size: 0,
                });

        let render_pipeline =
            context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Polyline Render Pipeline"),
                    layout: Some(&render_pipeline_layout),
                    cache: None,
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vertex"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        buffers: &[PolylineVertex::desc()],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some("fragment"),
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
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: None,
                        unclipped_depth: false,
                        polygon_mode: wgpu::PolygonMode::Fill,
                        conservative: false,
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        strip_index_format: None,
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
            uniforms,
            uniforms_buffer,
            bind_group,
        }
    }

    pub fn update_uniforms(&mut self, context: &GraphicsContext<'_, '_>, viewport_size: Vec2) {
        let new_viewport_size = [viewport_size.x, viewport_size.y];

        if self.uniforms.viewport_size != new_viewport_size {
            self.uniforms.viewport_size = new_viewport_size;

            context.queue.write_buffer(
                &self.uniforms_buffer,
                0,
                bytemuck::cast_slice(&[self.uniforms]),
            );
        }
    }

    pub fn render(&mut self, context: &mut GraphicsContext<'_, '_>, geometry: &PolylineResources) {
        if geometry.is_empty() {
            return;
        }

        context.render_pass().set_pipeline(&self.render_pipeline);
        context
            .render_pass()
            .set_bind_group(0, &self.bind_group, &[]);

        context
            .render_pass()
            .set_vertex_buffer(0, geometry.gpu_vertex_buffer());
        context.render_pass().draw(0..6, 0..geometry.len() as u32);
    }
}
