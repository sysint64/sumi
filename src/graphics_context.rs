use glam::Vec2;

pub struct GraphicsContext<'a, 'b> {
    pub(crate) render_pass: *mut wgpu::RenderPass<'b>,
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub surface_texture_format: wgpu::TextureFormat,
    pub view_size: Vec2,
    pub scale_factor: f32,
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
}
