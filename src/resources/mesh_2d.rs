use glam::Vec2;
use lyon::tessellation;
use wgpu::util::DeviceExt;

use crate::{graphics_context::GraphicsContext, memory::instances::InstanceId, svg};

#[derive(Clone, Copy, Default)]
pub struct SvgMesh {
    pub mesh_id: Mesh2DId,
    pub aspect_ratio: f32,
    pub size: Vec2,
}

#[derive(Copy, Clone, Default, Debug)]
pub struct Mesh2DId {
    pub value: usize,
}

#[derive(Debug, Default, Copy, Clone)]
pub struct Mesh2DInstanceId {
    pub mesh_id: Mesh2DId,
    pub value: u32,
}

impl PartialEq for Mesh2DInstanceId {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl InstanceId for Mesh2DInstanceId {
    fn index(&self) -> usize {
        self.value as usize
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Mesh2DVertex {
    pub position: [f32; 2],
    pub prim_id: u32,
}

impl Mesh2DVertex {
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Mesh2DVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    format: wgpu::VertexFormat::Float32x2,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    offset: 8,
                    format: wgpu::VertexFormat::Uint32,
                    shader_location: 1,
                },
            ],
        }
    }
}

// A 2x3 matrix (last two members of data1 unused).
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Mesh2DGpuTransform {
    pub data0: [f32; 4],
    pub data1: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Mesh2DGpuPrimitive {
    pub transform: u32,
    pub color: u32,
    pub _pad: [u32; 2],
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Mesh2DSize {
    width: f32,
    height: f32,
    _pad: [u32; 2],
}

impl Mesh2DSize {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            width,
            height,
            _pad: [0; 2],
        }
    }
}

pub struct Mesh2DGpuData {
    pub transforms: Vec<Mesh2DGpuTransform>,
    pub primitives: Vec<Mesh2DGpuPrimitive>,
    pub size: Mesh2DSize,
    pub data: tessellation::VertexBuffers<Mesh2DVertex, u32>,
}

struct GPUMesh2DData {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    indices_len: u32,
}

pub struct GPUMesh2DStorage {
    pub primitive_buffer_byte_size: wgpu::BufferAddress,
    pub transform_buffer_byte_size: wgpu::BufferAddress,
    pub mesh_sizes_buffer_byte_size: wgpu::BufferAddress,
    pub primitives: wgpu::Buffer,
    pub transforms: wgpu::Buffer,
    pub mesh_sizes: wgpu::Buffer,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GPUTransformsBuck {
    data: [Mesh2DGpuTransform; 1024 * 8],
}

impl Default for GPUTransformsBuck {
    fn default() -> Self {
        Self {
            data: [Mesh2DGpuTransform::default(); 1024 * 8],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GPUPrimitivesBuck {
    data: [Mesh2DGpuPrimitive; 1024],
}

impl Default for GPUPrimitivesBuck {
    fn default() -> Self {
        Self {
            data: [Mesh2DGpuPrimitive::default(); 1024],
        }
    }
}

#[derive(Default)]
pub struct Mesh2DResources {
    meshes: Vec<GPUMesh2DData>,
    transforms: Vec<GPUTransformsBuck>,
    primitives: Vec<GPUPrimitivesBuck>,
    mesh_sizes: Vec<Mesh2DSize>,
    storage: Option<GPUMesh2DStorage>,
}

impl Mesh2DResources {
    pub fn new() -> Self {
        Self {
            meshes: Vec::new(),
            transforms: Vec::new(),
            primitives: Vec::new(),
            mesh_sizes: Vec::new(),
            storage: None,
        }
    }

    pub fn storage(&self) -> &GPUMesh2DStorage {
        self.storage
            .as_ref()
            .expect("Storage has not been initialized")
    }

    pub fn load_svg_to_gpu(&mut self, context: &GraphicsContext<'_, '_>, data: &[u8]) -> SvgMesh {
        let opt = usvg::Options::default();
        let rtree = usvg::Tree::from_data(data, &opt).expect("Invalid SVG");

        let mut prev_transform = usvg::Transform {
            sx: f32::NAN,
            kx: f32::NAN,
            ky: f32::NAN,
            sy: f32::NAN,
            tx: f32::NAN,
            ty: f32::NAN,
        };

        let mut mesh_data = Mesh2DGpuData {
            transforms: vec![],
            primitives: vec![],
            size: Mesh2DSize::new(rtree.size().width(), rtree.size().height()),
            data: tessellation::VertexBuffers::new(),
        };

        let mut fill_tess = tessellation::FillTessellator::new();
        let mut stroke_tess = tessellation::StrokeTessellator::new();

        svg::collect_geom(
            rtree.root(),
            &mut prev_transform,
            &mut fill_tess,
            &mut stroke_tess,
            &mut mesh_data,
        );

        let id = self.load_mesh_to_gpu(context, mesh_data);

        let aspect_ratio = rtree.size().width() / rtree.size().height();

        SvgMesh {
            mesh_id: id,
            aspect_ratio,
            size: Vec2::new(rtree.size().width(), rtree.size().height()),
        }
    }

    pub fn load_mesh_to_gpu(
        &mut self,
        context: &GraphicsContext<'_, '_>,
        mesh: Mesh2DGpuData,
    ) -> Mesh2DId {
        let vertex_buffer = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&mesh.data.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let index_buffer = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&mesh.data.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

        let mut transforms = GPUTransformsBuck::default();
        let mut primitives = GPUPrimitivesBuck::default();

        for (idx, transform) in mesh.transforms.iter().enumerate() {
            transforms.data[idx] = *transform;
        }

        for (idx, primitive) in mesh.primitives.iter().enumerate() {
            primitives.data[idx] = *primitive;
        }

        self.transforms.push(transforms);
        self.primitives.push(primitives);
        self.mesh_sizes.push(mesh.size);

        let primitive_buffer_byte_size = (self.primitives.len()
            * std::mem::size_of::<GPUPrimitivesBuck>())
            as wgpu::BufferAddress;
        let transform_buffer_byte_size = (self.transforms.len()
            * std::mem::size_of::<GPUTransformsBuck>())
            as wgpu::BufferAddress;
        let mesh_sizes_buffer_byte_size =
            (self.mesh_sizes.len() * std::mem::size_of::<Mesh2DSize>()) as wgpu::BufferAddress;

        let primitives_ssbo = context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Svg Primitives SSBO"),
            size: primitive_buffer_byte_size,
            usage: wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let transforms_ssbo = context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Svg Transforms SSBO"),
            size: transform_buffer_byte_size,
            usage: wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let sizes_ssbo = context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Svg Transforms SSBO"),
            size: mesh_sizes_buffer_byte_size,
            usage: wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        context
            .queue
            .write_buffer(&transforms_ssbo, 0, bytemuck::cast_slice(&self.transforms));
        context
            .queue
            .write_buffer(&primitives_ssbo, 0, bytemuck::cast_slice(&self.primitives));
        context
            .queue
            .write_buffer(&sizes_ssbo, 0, bytemuck::cast_slice(&self.mesh_sizes));

        {
            self.storage.replace(GPUMesh2DStorage {
                primitives: primitives_ssbo,
                transforms: transforms_ssbo,
                mesh_sizes: sizes_ssbo,
                primitive_buffer_byte_size,
                transform_buffer_byte_size,
                mesh_sizes_buffer_byte_size,
            });
        }

        let mesh = GPUMesh2DData {
            vertex_buffer,
            index_buffer,
            indices_len: mesh.data.indices.len() as u32,
        };

        self.meshes.push(mesh);

        Mesh2DId {
            value: self.meshes.len() - 1,
        }
    }

    pub fn render(&self, context: &mut GraphicsContext<'_, '_>, instance_id: Mesh2DInstanceId) {
        let mesh = self
            .meshes
            .get(instance_id.mesh_id.value)
            .expect("Mesh has not been initialized");

        context
            .render_pass()
            .set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        context
            .render_pass()
            .set_vertex_buffer(0, mesh.vertex_buffer.slice(..));

        let instance_id = instance_id.value;

        context
            .render_pass()
            .draw_indexed(0..mesh.indices_len, 0, instance_id..instance_id + 1);
    }
}
