use glam::{Mat4, Vec3, Vec4};
use wgpu::util::DeviceExt;

use crate::graphics_context::GraphicsContext;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PolylineVertex {
    pub depth_bias: f32,
    pub width: f32,
    pub point_0: [f32; 3],
    pub point_1: [f32; 3],
    pub color: [f32; 4],
    pub mvp_matrix: [[f32; 4]; 4],
}

impl PolylineVertex {
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<PolylineVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // depth_bias
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 0,
                    shader_location: 0,
                },
                // width
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: std::mem::size_of::<f32>() as wgpu::BufferAddress,
                    shader_location: 1,
                },
                // point_0
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 2,
                },
                // point_1
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: std::mem::size_of::<[f32; 5]>() as wgpu::BufferAddress,

                    shader_location: 3,
                },
                // color
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: std::mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 4,
                },
                // mvp_matrix
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: std::mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 5,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: std::mem::size_of::<[f32; 16]>() as wgpu::BufferAddress,
                    shader_location: 6,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: std::mem::size_of::<[f32; 20]>() as wgpu::BufferAddress,
                    shader_location: 7,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: std::mem::size_of::<[f32; 24]>() as wgpu::BufferAddress,
                    shader_location: 8,
                },
            ],
        }
    }
}

pub struct PolylineResources {
    vertex_buffer: wgpu::Buffer,
    data: Vec<PolylineVertex>,

    line_width: f32,
    color: Vec4,
    mvp_matrix: Mat4,
    last_point: Option<Vec3>,
}

impl PolylineResources {
    pub fn new(context: &GraphicsContext<'_, '_>) -> Self {
        let data = Vec::<PolylineVertex>::new();

        let vertex_buffer = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Polyline Vertices"),
                contents: bytemuck::cast_slice(&data),
                usage: wgpu::BufferUsages::VERTEX,
            });

        Self {
            vertex_buffer,
            data,
            line_width: 1.,
            color: Vec4::new(0., 0., 0., 1.),
            mvp_matrix: Mat4::IDENTITY,
            last_point: None,
        }
    }

    pub fn gpu_vertex_buffer(&self) -> wgpu::BufferSlice<'_> {
        self.vertex_buffer.slice(..)
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn clear(&mut self) {
        self.data.clear();
        self.last_point = None;
    }

    pub fn set_line_width(&mut self, line_width: f32) {
        self.line_width = line_width;
    }

    pub fn set_color(&mut self, color: Vec4) {
        self.color = color;
    }

    pub fn set_mvp_matrix(&mut self, mvp_matrix: Mat4) {
        self.mvp_matrix = mvp_matrix;
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn add_line(&mut self, point1: Vec3, point2: Vec3) {
        self.data.push(PolylineVertex {
            depth_bias: 0.,
            width: self.line_width,
            point_0: point1.to_array(),
            point_1: point2.to_array(),
            color: self.color.to_array(),
            mvp_matrix: self.mvp_matrix.to_cols_array_2d(),
        });

        self.last_point = Some(point2);
    }

    pub fn add_rect(&mut self, rect: lattice::Rect<f32>) {
        let p1 = Vec3::new(rect.x, rect.y, 0.);
        let p2 = Vec3::new(rect.x + rect.width, rect.y, 0.);
        let p3 = Vec3::new(rect.x + rect.width, rect.y + rect.height, 0.);
        let p4 = Vec3::new(rect.x, rect.y + rect.height, 0.);

        self.add_line(p1, p2);
        self.add_line(p2, p3);
        self.add_line(p3, p4);
        self.add_line(p4, p1);
    }

    pub fn add_point(&mut self, point: Vec3) {
        if let Some(last_point) = self.last_point {
            self.data.push(PolylineVertex {
                depth_bias: 0.,
                width: self.line_width,
                point_0: last_point.to_array(),
                point_1: point.to_array(),
                color: self.color.to_array(),
                mvp_matrix: self.mvp_matrix.to_cols_array_2d(),
            });
        }

        self.last_point = Some(point);
    }

    pub fn load_to_gpu(&mut self, context: &GraphicsContext<'_, '_>) {
        self.vertex_buffer = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Polyline Vertices"),
                contents: bytemuck::cast_slice(&self.data),
                usage: wgpu::BufferUsages::VERTEX,
            });
    }
}
