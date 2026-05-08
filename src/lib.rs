pub mod graphics_context;
pub mod instances;
pub mod lazy_graphics_resource;
pub mod math;
pub mod memory;
pub mod memory_new;
pub mod renderer;
pub mod resources;
pub mod svg;
pub mod view;

pub use graphics_context::*;
pub use lazy_graphics_resource::*;
pub use math::*;
pub use memory::*;
pub use memory_new::*;
pub use view::*;

pub use cosmic_text_kv::FontId;
pub use cosmic_text_kv::FontResources;
pub use cosmic_text_kv::TextId;
pub use cosmic_text_kv::TextsResources;

pub use resources::centered_plane::*;
pub use resources::instancing_geometry::*;
pub use resources::mesh_2d::*;
pub use resources::plane::*;
pub use resources::polyline::*;
pub use resources::vertex::*;

pub use renderer::colored_plane::*;
pub use renderer::filled_circle::*;
pub use renderer::outlined_circle::*;
pub use renderer::polyline::*;
pub use renderer::rounded_rect::*;
pub use renderer::svg::*;
pub use renderer::text::*;

pub mod prelude {
    pub use crate::instances::{BumpInstances, PoolInstances, RenderInstances};
    pub use crate::memory::Instances;
}
