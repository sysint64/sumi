use std::mem::size_of;

use wgpu::util::DeviceExt;

use crate::graphics_context::GraphicsContext;
use crate::memory_new::{BumpBuffer, GpuBuffer, SlotId, SlottedBuffer};
use crate::svg;

// Re-use all shared types from mesh_2d unchanged.
pub use crate::resources::mesh_2d::{
    Mesh2DGpuData, Mesh2DGpuPrimitive, Mesh2DGpuTransform, Mesh2DId, Mesh2DSize, Mesh2DVertex,
    SvgMesh,
};

// Local instance ID — just an index into the instance buffer.
#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub struct Mesh2DInstanceId {
    pub value: u32,
}

// --- Per-mesh GPU data packing structs ---

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

// --- SlotId impls ---

impl SlotId for Mesh2DId {
    fn from_index(index: usize) -> Self {
        Self { value: index }
    }
    fn index(&self) -> usize {
        self.value
    }
}

impl SlotId for Mesh2DInstanceId {
    fn from_index(index: usize) -> Self {
        Self {
            value: index as u32,
        }
    }
    fn index(&self) -> usize {
        self.value as usize
    }
}

// --- Per-mesh vertex/index GPU buffers ---

struct GpuMesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    indices_len: u32,
}

// --- Mesh2DResources ---

pub struct Mesh2DResources {
    meshes: Vec<GpuMesh>,
    primitives_buf: BumpBuffer<Mesh2DId, GPUPrimitivesBuck>,
    transforms_buf: BumpBuffer<Mesh2DId, GPUTransformsBuck>,
    sizes_buf: BumpBuffer<Mesh2DId, Mesh2DSize>,
}

impl Default for Mesh2DResources {
    fn default() -> Self {
        Self::new()
    }
}

impl Mesh2DResources {
    pub fn new() -> Self {
        let usage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;
        Self {
            meshes: Vec::new(),
            primitives_buf: BumpBuffer::new(8, usage),
            transforms_buf: BumpBuffer::new(8, usage),
            sizes_buf: BumpBuffer::new(8, usage),
        }
    }

    pub fn primitives_buffer(&self) -> &wgpu::Buffer {
        self.primitives_buf.gpu_buffer()
    }

    pub fn transforms_buffer(&self) -> &wgpu::Buffer {
        self.transforms_buf.gpu_buffer()
    }

    pub fn mesh_sizes_buffer(&self) -> &wgpu::Buffer {
        self.sizes_buf.gpu_buffer()
    }

    pub fn primitives_byte_size(&self) -> wgpu::BufferAddress {
        (self.primitives_buf.data().len() * size_of::<GPUPrimitivesBuck>()) as wgpu::BufferAddress
    }

    pub fn transforms_byte_size(&self) -> wgpu::BufferAddress {
        (self.transforms_buf.data().len() * size_of::<GPUTransformsBuck>()) as wgpu::BufferAddress
    }

    pub fn mesh_sizes_byte_size(&self) -> wgpu::BufferAddress {
        (self.sizes_buf.data().len() * size_of::<Mesh2DSize>()) as wgpu::BufferAddress
    }

    /// Returns true if any storage buffer was reallocated since the last call.
    /// The caller should rebuild bind groups when this returns true.
    pub fn take_buffer_resized(&mut self) -> bool {
        let a = self.primitives_buf.take_buffer_resized();
        let b = self.transforms_buf.take_buffer_resized();
        let c = self.sizes_buf.take_buffer_resized();
        a || b || c
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
            data: lyon::tessellation::VertexBuffers::new(),
        };

        let mut fill_tess = lyon::tessellation::FillTessellator::new();
        let mut stroke_tess = lyon::tessellation::StrokeTessellator::new();

        svg::collect_geom(
            rtree.root(),
            &mut prev_transform,
            &mut fill_tess,
            &mut stroke_tess,
            &mut mesh_data,
        );

        let mesh_id = self.load_mesh_to_gpu(context, mesh_data);
        let aspect_ratio = rtree.size().width() / rtree.size().height();

        SvgMesh {
            mesh_id,
            aspect_ratio,
            size: glam::Vec2::new(rtree.size().width(), rtree.size().height()),
        }
    }

    pub fn load_mesh_to_gpu(
        &mut self,
        context: &GraphicsContext<'_, '_>,
        mesh: Mesh2DGpuData,
    ) -> Mesh2DId {
        // Upload per-mesh vertex/index buffers.
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

        // Pack transform and primitive data into fixed-size GPU bucks.
        let mut transforms = GPUTransformsBuck::default();
        let mut primitives = GPUPrimitivesBuck::default();

        for (idx, t) in mesh.transforms.iter().enumerate() {
            transforms.data[idx] = *t;
        }
        for (idx, p) in mesh.primitives.iter().enumerate() {
            primitives.data[idx] = *p;
        }

        // Insert into storage bump buffers and get the canonical mesh ID.
        let mesh_id = self.primitives_buf.insert(primitives);
        self.transforms_buf.insert(transforms);
        self.sizes_buf.insert(mesh.size);

        // Upload active data; handles GPU buffer creation and resize automatically.
        self.primitives_buf.flush(context);
        self.transforms_buf.flush(context);
        self.sizes_buf.flush(context);

        self.meshes.push(GpuMesh {
            vertex_buffer,
            index_buffer,
            indices_len: mesh.data.indices.len() as u32,
        });

        mesh_id
    }

    pub fn render_slot(&self, context: &mut GraphicsContext<'_, '_>, slot: u32, mesh_id: Mesh2DId) {
        let mesh = self
            .meshes
            .get(mesh_id.value)
            .expect("Mesh has not been initialized");

        context
            .render_pass()
            .set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        context
            .render_pass()
            .set_vertex_buffer(0, mesh.vertex_buffer.slice(..));

        context
            .render_pass()
            .draw_indexed(0..mesh.indices_len, 0, slot..slot + 1);
    }
}
