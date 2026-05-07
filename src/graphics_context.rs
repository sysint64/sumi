use std::num::NonZeroU32;

use glam::Vec2;

use crate::GraphicsView;

pub struct GraphicsContext<'a, 'b> {
    pub(crate) render_pass: *mut wgpu::RenderPass<'b>,
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub surface_texture_format: wgpu::TextureFormat,
    pub view_size: Vec2,
    pub scale_factor: f32,
    pub sample_count: u32,
    pub view: &'a GraphicsView,
}

#[derive(PartialEq)]
pub enum LoadToGPUSchedule {
    Immediately,
    NextFrame,
}

impl<'a, 'b> GraphicsContext<'a, 'b> {
    #[allow(clippy::mut_from_ref)]
    pub fn render_pass(&'a self) -> &'a mut wgpu::RenderPass<'b> {
        if self.render_pass.is_null() {
            panic!("No Render Pass")
        } else {
            unsafe { &mut *self.render_pass }
        }
    }

    #[inline]
    pub fn default_depth_stencil(&self) -> Option<wgpu::DepthStencilState> {
        None
    }

    #[inline]
    pub fn default_multisample(&self) -> wgpu::MultisampleState {
        wgpu::MultisampleState {
            count: self.sample_count,
            mask: !0,
            alpha_to_coverage_enabled: false,
        }
    }

    #[inline]
    pub fn default_multiview_mask(&self) -> Option<NonZeroU32> {
        None
    }
}
