use std::{collections::HashMap, ptr, time::Instant};

use crate::graphics_context::GraphicsContext;
use crate::math::{OthroCameraTransforms, create_ortho_camera_matrices};
use glam::{Mat4, Vec2};
use lattice::EdgeInsets;

pub(crate) static mut GAPI_STATE: *mut GraphicsState = ptr::null_mut();

#[derive(Default)]
pub(crate) struct GraphicsState {
    pub(crate) api: HashMap<u64, Graphics>,
    pub(crate) renderer_factories: HashMap<u64, Box<RendererFactory>>,
    pub(crate) renderers: HashMap<u64, Box<dyn GraphicsViewRenderer>>,
}

type RendererFactory =
    dyn Fn(&mut GraphicsContext<'_, '_>, &GraphicsView) -> Box<dyn GraphicsViewRenderer + 'static>;

pub trait GraphicsViewRenderer {
    fn render(&mut self, context: &mut GraphicsContext<'_, '_>, view: &GraphicsView);

    fn should_render(&self) -> bool {
        true
    }
}

#[derive(Debug, Default, Clone)]
pub struct GraphicsView {
    pub size_unscaled: Vec2,
    pub size: Vec2,
    pub delta_time: f64,
    pub delta_time_f32: f32,
    pub scale_factor: f32,
    pub fps: u32,
    pub safe_area: EdgeInsets,
    pub screen_camera_matrix: Mat4,
}

struct RenderStats {
    fps_timer: Instant,
    delta_timer: Instant,
    rendered_frames: u32,
}

pub struct Graphics {
    pub id: u64,

    suspend: bool,
    wake: bool,
    view: GraphicsView,
    stats: RenderStats,
    surface_texture_format: wgpu::TextureFormat,

    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) config: wgpu::SurfaceConfiguration,
}

#[rustfmt::skip]
pub fn opengl_to_wgpu_matrix() -> Mat4 {
    Mat4::from_cols_array(&[
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 0.5, 0.0,
        0.0, 0.0, 0.5, 1.0,
    ])
}

pub struct GraphicsCreateParams {
    pub id: u64,
    pub view_size: Vec2,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter: wgpu::Adapter,
    pub surface: wgpu::Surface<'static>,
}

impl Graphics {
    /// # Safety
    ///
    /// Should be called once when app is initialized.
    pub unsafe fn init_memory() {
        let gapi_state = Box::new(GraphicsState::default());

        unsafe {
            GAPI_STATE = Box::into_raw(gapi_state);
        }
    }

    /// # Safety
    ///
    /// The [`Self::init_memory`] method should be called before calling this methd.
    ///
    /// - [`GAPI_STATE`] must be initialized and not concurrently accessed by other threads
    /// - `params.id` should be unique to avoid overwriting existing entries
    /// - Returned pointer is only valid while the entry remains in the global HashMap
    /// - Caller must ensure proper lifetime management of the returned pointer
    pub unsafe fn new(params: GraphicsCreateParams) -> *mut Graphics {
        let caps = params.surface.get_capabilities(&params.adapter);
        let surface_texture_format = caps.formats[0];
        // let surface_texture_format = caps.formats[0].remove_srgb_suffix();

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_texture_format,
            width: params.view_size.x as u32,
            height: params.view_size.y as u32,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            desired_maximum_frame_latency: 2,
            view_formats: vec![],
        };

        params.surface.configure(&params.device, &config);

        let gapi = Self {
            surface_texture_format,
            suspend: false,
            wake: false,

            id: params.id,
            view: GraphicsView::default(),
            surface: params.surface,
            device: params.device,
            queue: params.queue,
            config,
            stats: RenderStats {
                fps_timer: Instant::now(),
                delta_timer: Instant::now(),
                rendered_frames: 0,
            },
        };

        let gapi_state = unsafe { GAPI_STATE.as_mut().unwrap() };
        gapi_state.api.insert(params.id, gapi);
        let gapi_ref = gapi_state.api.get_mut(&params.id).unwrap();

        gapi_ref as *mut Graphics
    }

    pub(crate) unsafe fn raw_new(params: GraphicsCreateParams) -> *mut u8 {
        let gapi = unsafe { Self::new(params) };

        gapi as *mut u8
    }

    pub fn on_destroy(&mut self) {
        let gapi_state = unsafe { GAPI_STATE.as_mut().unwrap() };

        gapi_state.api.remove(&self.id);
        gapi_state.renderers.remove(&self.id);
    }

    pub fn register_renderer<T>(
        id: u64,
        create: fn(context: &mut GraphicsContext<'_, '_>, view: &GraphicsView) -> T,
    ) where
        T: GraphicsViewRenderer + 'static,
    {
        let gapi_state = unsafe { GAPI_STATE.as_mut().unwrap() };

        gapi_state.renderer_factories.insert(
            id,
            Box::new(move |context, view| Box::new(create(context, view))),
        );
    }

    pub async fn wgpu_request_device(
        instance: &wgpu::Instance,
        surface: &wgpu::Surface<'_>,
    ) -> (wgpu::Adapter, wgpu::Device, wgpu::Queue) {
        // let adapter = wgpu::util::initialize_adapter_from_env_or_default(instance, Some(surface))
        //     .await
        //     .expect("No suitable GPU adapters found on the system!");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::from_env()
                    .unwrap_or(wgpu::PowerPreference::HighPerformance),
                force_fallback_adapter: false,
                compatible_surface: Some(surface),
            })
            .await
            .expect("No suitable GPU adapters found on the system!");

        let adapter_info = adapter.get_info();

        println!("Using {} ({:?})", adapter_info.name, adapter_info.backend);

        let base_dir = std::env::var("CARGO_MANIFEST_DIR");
        let _trace_path = if let Ok(base_dir) = base_dir {
            Some(std::path::PathBuf::from(&base_dir).join("WGPU_TRACE_ERROR"))
        } else {
            None
        };

        let res = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: adapter.features(),
                required_limits: adapter.limits(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
            })
            .await;
        match res {
            Err(err) => {
                panic!("request_device failed: {err:?}");
            }
            Ok((device, queue)) => (adapter, device, queue),
        }
    }

    pub fn set_view_parameters(&mut self, scale_factor: f32, new_size: Vec2) {
        self.view.scale_factor = scale_factor;

        if new_size == self.view.size_unscaled {
            return;
        }

        if new_size.x > 0. && new_size.y > 0. {
            self.view.size_unscaled = new_size;
            self.view.size = new_size;
            self.config.width = new_size.x as u32;
            self.config.height = new_size.y as u32;
            self.surface.configure(&self.device, &self.config);
            self.queue.submit([]);
            self.wake = true;

            self.view.screen_camera_matrix = create_ortho_camera_matrices(&OthroCameraTransforms {
                viewport_size: self.view.size,
                position: Vec2::ZERO,
                zoom: 1.0,
            })
            .mvp_matrix;
        }
    }

    pub fn should_render(&self) -> bool {
        let gapi_state = unsafe { GAPI_STATE.as_ref().unwrap() };
        let renderer = gapi_state.renderers.get(&self.id);

        if let Some(renderer) = renderer {
            renderer.should_render() || self.wake
        } else {
            true
        }
    }

    pub fn render(&mut self) {
        let gapi_state = unsafe { GAPI_STATE.as_mut().unwrap() };
        let renderer = gapi_state.renderers.get_mut(&self.id);

        let renderer = if let Some(renderer) = renderer {
            renderer
        } else {
            let factory = gapi_state.renderer_factories.get(&self.id).unwrap();
            let mut context = GraphicsContext {
                surface_texture_format: self.surface_texture_format,
                render_pass: ptr::null_mut(),
                device: &self.device,
                queue: &self.queue,
                view_size: self.view.size,
                scale_factor: self.view.scale_factor,
            };

            gapi_state
                .renderers
                .insert(self.id, factory(&mut context, &self.view.clone()));

            gapi_state.renderers.get_mut(&self.id).unwrap()
        };

        if !renderer.should_render() && !self.wake {
            self.suspend = true;

            self.stats.fps_timer = Instant::now();
            self.stats.delta_timer = Instant::now();
            self.stats.rendered_frames = 0;

            return;
        }

        if self.wake {
            self.wake = false;
        }

        self.suspend = false;
        let elapsed = self.stats.fps_timer.elapsed().as_millis();

        if elapsed >= 1000 {
            self.view.fps = self.stats.rendered_frames;
            self.stats.rendered_frames = 0;
            self.stats.fps_timer = Instant::now();
        }

        let delta_time = self.stats.delta_timer.elapsed().as_secs_f64();
        self.view.delta_time = delta_time;
        self.view.delta_time_f32 = delta_time as f32;
        self.stats.delta_timer = Instant::now();

        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(surface_texture) => surface_texture,
            wgpu::CurrentSurfaceTexture::Suboptimal(surface_texture) => {
                log::warn!("Get current surface texture: suboptimal");

                surface_texture
            }
            wgpu::CurrentSurfaceTexture::Timeout => {
                panic!("Get current surface texture: timout");
            }
            wgpu::CurrentSurfaceTexture::Occluded => {
                panic!("Get current surface texture: occluded");
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                panic!("Get current surface texture: outdate");
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                panic!("Get current surface texture: lost");
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                panic!("Get current surface texture: validation error")
            }
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            {
                let mut context = GraphicsContext {
                    surface_texture_format: self.surface_texture_format,
                    render_pass: &mut render_pass,
                    device: &self.device,
                    queue: &self.queue,
                    view_size: self.view.size,
                    scale_factor: self.view.scale_factor,
                };

                renderer.render(&mut context, &self.view.clone());
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        self.stats.rendered_frames += 1;
    }
}
