use std::f32::consts::PI;
use std::sync::{Arc, LazyLock};

use glam::{Mat4, Vec2, Vec3, Vec4};
use parking_lot::Mutex;
use rand::RngExt;
use sumi::prelude::*;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

pub struct State {
    window: Arc<Window>,
    gapi: *mut sumi::Graphics,
}

pub struct Application {
    state: Option<State>,
}

const MAIN_RENDERER_ID: u64 = 0;

impl ApplicationHandler for Application {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attributes = Window::default_attributes().with_active(true);
        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
        self.state = Some(pollster::block_on(State::new(window)).unwrap());
        // self.window = Some(
        //     event_loop
        //         .create_window(Window::default_attributes())
        //         .unwrap(),
        // );
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let state = match &mut self.state {
            Some(canvas) => canvas,
            None => return,
        };

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(size) => state.resize(size.width, size.height),
            WindowEvent::RedrawRequested => {
                state.render();
            }
            _ => {}
        }
    }
}

impl State {
    pub async fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        // let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        //     backends: wgpu::Backends::PRIMARY,
        //     ..Default::default()
        // });
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());

        let surface = instance.create_surface(window.clone()).unwrap();

        #[cfg(target_os = "macos")]
        #[allow(invalid_reference_casting)]
        unsafe {
            surface.as_hal::<wgpu::hal::metal::Api, _, ()>(|surface| {
                if let Some(surface_ref) = surface {
                    let surface_mut = &mut *(surface_ref as *const wgpu::hal::metal::Surface
                        as *mut wgpu::hal::metal::Surface);
                    surface_mut.present_with_transaction = true;
                }
            });
        }

        let (adapter, device, queue) =
            sumi::Graphics::wgpu_request_device(&instance, &surface).await;

        let size = window.inner_size();
        let view_size = Vec2::new(size.width as f32, size.height as f32);

        let gapi_params = sumi::GraphicsCreateParams {
            id: MAIN_RENDERER_ID,
            view_size,
            device,
            queue,
            adapter,
            surface,
        };

        let gapi = unsafe { sumi::Graphics::new(gapi_params) };

        unsafe {
            (&mut *gapi).set_view_parameters(window.scale_factor() as f32, view_size);
        }

        Ok(Self { window, gapi })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let view_size = Vec2::new(width as f32, height as f32);

        unsafe {
            let gapi = &mut *self.gapi;

            gapi.set_view_parameters(self.window.scale_factor() as f32, view_size);
        }
    }

    pub fn render(&mut self) {
        self.window.request_redraw();

        unsafe {
            let gapi = &mut *self.gapi;
            gapi.render();
        }
    }
}

static STATE: LazyLock<Mutex<RenderDemoState>> =
    LazyLock::new(|| Mutex::new(RenderDemoState::default()));

const PARTICLES_COUNT: usize = 100;

#[derive(Default)]
struct RenderDemoState {
    tick: u64,
    quad_model_matrix: Mat4,
    rotation: f32,
    grid_mvp_matrix: Mat4,
    big_grid_mvp_matrix: Mat4,
    grid_angle: f32,
    sin_amp: f32,
    sin_amp_angle: f32,
    outlined_circle_radius: f32,
    outlined_circle_amp_angle: f32,
    outlined_circle_mvp_matrix: Mat4,
    particles: Vec<Particle>,
    tiger: sumi::SvgMesh,
    goose: sumi::SvgMesh,
    tiger_mvp_matrix: Mat4,
    tiger2_mvp_matrix: Mat4,
    goose_mvp_matrix: Mat4,
    goose_position: Vec2,
    quad_instance_id: sumi::ColoredPlaneInstanceId,
    fps_text: sumi::TextId,
    demo_text: sumi::TextId,
}

#[derive(Default, Clone, Copy)]
struct Particle {
    alive: bool,
    pos: Vec2,
    angle: f32,
    acc: f32,
    radius: f32,
    color: Vec4,
    mvp_matrix: Mat4,
}

struct RenderDemoRenderer<'a> {
    resources: Resources<'a>,
    renderers: Renderers,
    fonts: sumi::FontResources,
}

struct Resources<'a> {
    centered_plane: sumi::CenteredPlaneResources,
    mesh_2d: sumi::Mesh2DResources,

    big_grid: sumi::PolylineResources,
    small_grid: sumi::PolylineResources,
    sin_wave: sumi::PolylineResources,
    text: sumi::TextsResources<'a>,
}

struct Renderers {
    colored_plane: sumi::ColoredPlaneRenderer,
    filled_circle: sumi::FilledCircleRenderer,
    outlined_circle: sumi::OutlinedCircleRenderer,
    colored_svg: sumi::ColoredSvgRenderer,
    svg: sumi::SvgRenderer,
    polyline: sumi::PolylineRenderer,
    text: sumi::TextRenderer,
}

impl RenderDemoRenderer<'_> {
    fn new(context: &mut sumi::GraphicsContext<'_, '_>, view: &sumi::GraphicsView) -> Self {
        let mut state = STATE.lock();
        let mut resources = Resources {
            centered_plane: sumi::CenteredPlaneResources::new(context),
            mesh_2d: sumi::Mesh2DResources::new(),
            big_grid: sumi::PolylineResources::new(context),
            small_grid: sumi::PolylineResources::new(context),
            sin_wave: sumi::PolylineResources::new(context),
            text: sumi::TextsResources::new(),
        };

        let mut fonts = sumi::FontResources::new();

        fonts.load_font("Inter", include_bytes!("assets/fonts/Inter.ttf"));
        fonts.load_font(
            "Source Han Serif",
            include_bytes!("assets/fonts/SourceHanSerif-Regular.otf"),
        );
        fonts.load_font(
            "Noto Emoji",
            include_bytes!("assets/fonts/NotoEmoji-VariableFont_wght.ttf"),
        );

        state.demo_text =
            resources
                .text
                .add_text(view.scale_factor as f64, &mut fonts, 20., 20., |_, text| {
                    text.set_text("Hello World!\nFantôm\n今日は🦅🦁", None);
                });

        state.fps_text =
            resources
                .text
                .add_text(view.scale_factor as f64, &mut fonts, 14., 14., |_, text| {
                    text.set_text("Frame Time: 0ms, FPS: 0", None);
                });

        resources
            .text
            .shape_as_needed(state.demo_text, &mut fonts.font_system, true);

        resources.sin_wave.set_color(Vec4::new(0.8, 0.1, 0.1, 1.));
        resources.sin_wave.set_line_width(5. * view.scale_factor);

        resources.small_grid.set_line_width(1. * view.scale_factor);
        resources.big_grid.set_line_width(5. * view.scale_factor);

        state.tiger = resources
            .mesh_2d
            .load_svg_to_gpu(context, include_bytes!("assets/ghostscript_tiger.svg"));

        state.goose = resources
            .mesh_2d
            .load_svg_to_gpu(context, include_bytes!("assets/goose.svg"));

        let mut renderers = Renderers {
            colored_plane: sumi::ColoredPlaneRenderer::new(context, sumi::BumpInstances::new(10)),
            filled_circle: sumi::FilledCircleRenderer::new(
                context,
                sumi::BumpInstances::new(PARTICLES_COUNT),
            ),
            outlined_circle: sumi::OutlinedCircleRenderer::new(
                context,
                sumi::BumpInstances::new(100),
            ),
            colored_svg: sumi::ColoredSvgRenderer::new(
                context,
                &resources.mesh_2d,
                sumi::BumpInstances::new(10),
            ),
            svg: sumi::SvgRenderer::new(context, &resources.mesh_2d, sumi::BumpInstances::new(10)),
            polyline: sumi::PolylineRenderer::new(context),
            text: sumi::TextRenderer::new(context),
        };

        state.goose_position = Vec2::new(0., 128.);

        spawn_particles(view, &mut state);
        update(view, &mut state);

        let mvp_matrix = view.screen_camera_matrix * state.quad_model_matrix;

        state.quad_instance_id =
            renderers
                .colored_plane
                .instances()
                .insert(sumi::ColoredPlaneInstance::new(
                    &mvp_matrix,
                    &Vec4::new(1.0, 1.0, 0.0, 0.8),
                ));

        renderers
            .colored_plane
            .instances()
            .load_all_instances_to_gpu(context, sumi::LoadToGPUSchedule::NextFrame);

        Self {
            resources,
            renderers,
            fonts,
        }
    }
}

impl sumi::GraphicsViewRenderer for RenderDemoRenderer<'_> {
    fn render(&mut self, context: &mut sumi::GraphicsContext<'_, '_>, view: &sumi::GraphicsView) {
        let mut state = STATE.lock();

        update(view, &mut state);
        render_demo(context, view, &state, self);
    }
}

fn spawn_particles(view: &sumi::GraphicsView, state: &mut RenderDemoState) {
    let mut rng = rand::rng();
    state.particles = vec![Particle::default(); PARTICLES_COUNT];

    for particle in state.particles.iter_mut() {
        particle.pos = Vec2::new(
            rng.random_range(-10.0..view.size.x + 10.),
            rng.random_range(-10.0..view.size.y + 10.),
        );
        spawn_particle(&mut rng, particle);
    }
}

fn spawn_particle(rng: &mut impl rand::Rng, particle: &mut Particle) {
    particle.radius = rng.random_range(5.0..20.0);
    particle.acc = 0.1 + rng.random::<f32>() / 2.;
    particle.color = Vec4::new(rng.random(), rng.random(), rng.random(), 1.0);
    particle.angle = 0.;
    particle.alive = true;
}

fn update(view: &sumi::GraphicsView, state: &mut RenderDemoState) {
    // Centered quad
    let transforms = sumi::Transforms2D {
        position: Vec2::new(view.size.x / 2., view.size.y / 2.),
        // position: Vec2::new(0., 0.),
        scaling: Vec2::new(
            430. * view.scale_factor * 0.8,
            600. * view.scale_factor * 0.8,
        ),
        rotation: state.rotation,
    };

    state.rotation -= 0.1 * view.delta_time_f32;
    state.quad_model_matrix = sumi::transforms_create_2d_model_matrix(&transforms);

    // Grid
    state.grid_angle += view.delta_time_f32 / 2.;

    let translation = Vec3::new(
        f32::sin(state.grid_angle) * 100.,
        f32::cos(state.grid_angle) * 100.,
        0.,
    );
    state.grid_mvp_matrix = view.screen_camera_matrix * Mat4::from_translation(translation);

    let translation = Vec3::new(
        -f32::sin(state.grid_angle) * 100.,
        -f32::cos(state.grid_angle) * 100.,
        0.,
    );
    state.big_grid_mvp_matrix = view.screen_camera_matrix * Mat4::from_translation(translation);

    // Outlined circle
    state.outlined_circle_amp_angle += view.delta_time_f32 * 10. / 2.;
    state.outlined_circle_radius = 100. + f32::sin(state.sin_amp_angle) * 50.;

    let transforms = sumi::Transforms2D {
        position: Vec2::new(view.size.x / 2., view.size.y / 2.),
        scaling: Vec2::new(
            state.outlined_circle_radius * 2.,
            state.outlined_circle_radius * 2.,
        ),
        rotation: 0.,
    };

    state.outlined_circle_mvp_matrix =
        view.screen_camera_matrix * sumi::transforms_create_2d_model_matrix(&transforms);

    // Sin wave
    state.sin_amp_angle += view.delta_time_f32 * 10. / 2.;
    state.sin_amp = f32::sin(state.sin_amp_angle) * 100.;

    // Particles
    update_particles(view, state);

    // Tiger
    let scale = 1. + f32::cos(state.sin_amp_angle / 2.) / 2.;
    let transforms = sumi::Transforms2D {
        position: Vec2::new(view.size.x / 2., view.size.y / 2.),
        scaling: Vec2::new(-400. * state.tiger.aspect_ratio, 400.) * Vec2::new(scale, scale),
        rotation: f32::sin(state.sin_amp_angle / 2.) / 2.,
    };

    state.tiger_mvp_matrix =
        view.screen_camera_matrix * sumi::transforms_create_2d_model_matrix(&transforms);

    // Tiger 2
    let transforms = sumi::Transforms2D {
        position: Vec2::new(view.size.x - 400., view.size.y - 400.),
        scaling: Vec2::new(600. * state.tiger.aspect_ratio, 600.),
        rotation: 0.,
    };
    state.tiger2_mvp_matrix =
        view.screen_camera_matrix * sumi::transforms_create_2d_model_matrix(&transforms);

    // Goose

    state.goose_position.x += view.delta_time_f32 * 200.;
    state.goose_position.y = 128. + state.sin_amp / 2.;

    if state.goose_position.x > view.size.x + 64. {
        state.goose_position.x = -64.;
    }

    let transforms = sumi::Transforms2D {
        position: state.goose_position,
        scaling: Vec2::new(128., 128.),
        rotation: 0.,
    };

    state.goose_mvp_matrix =
        view.screen_camera_matrix * sumi::transforms_create_2d_model_matrix(&transforms);
}

fn update_particles(view: &sumi::GraphicsView, state: &mut RenderDemoState) {
    let mut rng = rand::rng();

    for particle in state.particles.iter_mut() {
        if !particle.alive {
            particle.pos = Vec2::new(rng.random_range(0.0..view.size.x), 0.);
            spawn_particle(&mut rng, particle);
        } else {
            particle.pos.y += particle.acc * view.delta_time_f32 * 1000.;
            particle.angle += particle.acc * view.delta_time_f32 * 1000. / 50.;

            if particle.pos.y > view.size.y {
                particle.alive = false;
            }
        }
    }

    for particle in state.particles.iter_mut() {
        let transforms = sumi::Transforms2D {
            position: particle.pos + Vec2::new(particle.angle.sin() * 10., 0.),
            scaling: Vec2::new(particle.radius * 2., particle.radius * 2.),
            rotation: 0.,
        };

        particle.mvp_matrix =
            view.screen_camera_matrix * sumi::transforms_create_2d_model_matrix(&transforms);
    }
}

pub struct RenderGridInput {
    camera_pos: Vec2,
    viewport_size: Vec2,
    mvp_matrix: Mat4,
    step: i32,
    color: Vec4,
}

fn render_demo(
    context: &mut sumi::GraphicsContext<'_, '_>,
    view: &sumi::GraphicsView,
    state: &RenderDemoState,
    graphics_state: &mut RenderDemoRenderer,
) {
    let resources = &mut graphics_state.resources;
    let renderers = &mut graphics_state.renderers;
    let fonts = &mut graphics_state.fonts;

    renderers.polyline.update_uniforms(context, view.size);
    renderers.text.update_viewport(context);

    render_grid(
        context,
        &mut resources.small_grid,
        &mut renderers.polyline,
        RenderGridInput {
            camera_pos: Vec2::new(-100., -100.),
            viewport_size: Vec2::new(view.size.x + 200., view.size.y + 200.),
            mvp_matrix: state.grid_mvp_matrix,
            step: 32,
            color: Vec4::new(0.0, 0.0, 0.0, 1.0),
        },
    );

    render_grid(
        context,
        &mut resources.big_grid,
        &mut renderers.polyline,
        RenderGridInput {
            camera_pos: Vec2::new(-100., -100.),
            viewport_size: Vec2::new(view.size.x + 200., view.size.y + 200.),
            mvp_matrix: state.big_grid_mvp_matrix,
            step: 64,
            color: Vec4::new(0.0, 0.0, 0.0, 1.0),
        },
    );

    // Centered Plane
    let mvp_matrix = view.screen_camera_matrix * state.quad_model_matrix;

    renderers.colored_plane.instances().update_instance(
        state.quad_instance_id,
        sumi::ColoredPlaneInstance::new(&mvp_matrix, &Vec4::new(1.0, 1.0, 0.0, 0.8)),
    );

    renderers.colored_plane.instances().load_instance_to_gpu(
        context,
        sumi::LoadToGPUSchedule::NextFrame,
        state.quad_instance_id,
    );

    renderers.colored_plane.render_instance(
        context,
        &resources.centered_plane,
        state.quad_instance_id,
    );

    render_sin_wave_2(context, view, resources, renderers, state);

    // Particles
    let instances = renderers.filled_circle.instances();
    instances.clear();

    for particle in state.particles.iter() {
        instances.insert(sumi::FilledCircleInstance::new(
            &particle.mvp_matrix,
            &particle.color,
        ));
    }

    instances.load_all_instances_to_gpu(context, sumi::LoadToGPUSchedule::NextFrame);

    renderers
        .filled_circle
        .render_all_instances(context, &resources.centered_plane);

    // Outlined Circle
    let instances = renderers.outlined_circle.instances();
    instances.clear();

    instances.insert(sumi::OutlinedCircleInstance::new(
        5. * view.scale_factor,
        state.outlined_circle_radius,
        &state.outlined_circle_mvp_matrix,
        &Vec4::new(0.1, 0.8, 0.1, 1.0),
    ));

    instances.load_all_instances_to_gpu(context, sumi::LoadToGPUSchedule::NextFrame);

    renderers
        .outlined_circle
        .render_all_instances(context, &resources.centered_plane);

    // Svg
    let svg_renderer = &mut renderers.svg;

    let instances = svg_renderer.instances();
    instances.clear();

    instances.insert(sumi::SvgMeshInstance::new(
        state.tiger.mesh_id,
        &state.tiger_mvp_matrix,
    ));
    instances.insert(sumi::SvgMeshInstance::new(
        state.tiger.mesh_id,
        &state.tiger2_mvp_matrix,
    ));

    instances.load_all_instances_to_gpu(context, sumi::LoadToGPUSchedule::NextFrame);
    svg_renderer.render_all_instances(context, &resources.mesh_2d);

    // Colored Svg
    let colored_svg_renderer = &mut renderers.colored_svg;
    let instances = colored_svg_renderer.instances();

    instances.clear();
    instances.insert(sumi::ColoredSvgMeshInstance::new(
        state.goose.mesh_id,
        &state.goose_mvp_matrix,
        &Vec4::new(1., 0.5, 0., 1.),
    ));

    instances.load_all_instances_to_gpu(context, sumi::LoadToGPUSchedule::NextFrame);
    colored_svg_renderer.render_all_instances(context, &resources.mesh_2d);

    // Text
    let instances = renderers.text.instances();

    resources.text.update_text(state.fps_text, |text| {
        text.set_text(
            &format!(
                "Frame Time: {}ms\nFPS: {}",
                (view.delta_time * 1000.).floor(),
                view.fps
            ),
            None,
        );
    });

    resources
        .text
        .shape_as_needed(state.fps_text, &mut fonts.font_system, true);

    instances.clear();
    instances.insert(sumi::TextInstance::new(
        state.fps_text,
        Vec2::new(
            view.safe_area.left as f32 + 10.,
            view.safe_area.top as f32 + 0.,
        ),
        Vec4::new(1., 1., 1., 1.),
    ));

    let (_, fps_text_size_height) = resources.text.get_mut(state.fps_text).calculate_size();

    instances.insert(sumi::TextInstance::new(
        state.demo_text,
        Vec2::new(
            view.safe_area.left as f32 + 10.,
            view.safe_area.top as f32 + 10. + fps_text_size_height as f32,
        ),
        Vec4::new(1., 0., 0., 1.),
    ));

    renderers
        .text
        .render_all_instances(context, fonts, &resources.text);
}

pub fn render_grid(
    context: &mut sumi::GraphicsContext<'_, '_>,
    polyline: &mut sumi::PolylineResources,
    renderer: &mut sumi::PolylineRenderer,
    input: RenderGridInput,
) {
    polyline.set_color(input.color);
    polyline.set_mvp_matrix(input.mvp_matrix);

    polyline.clear();

    let camera_pos = input.camera_pos;
    let camera_x_round = camera_pos.x.round() as i32;
    let camera_y_round = camera_pos.y.round() as i32;

    // Vertical lines
    let from = camera_x_round - camera_x_round % input.step;
    let to = camera_x_round + input.viewport_size.x as i32;

    for i in (from..to).step_by(input.step as usize) {
        let x = i as f32;
        polyline.add_line(
            Vec3::new(x, camera_pos.y, 0.),
            Vec3::new(x, input.viewport_size.y + camera_pos.y, 0.),
        );
    }

    // Horizontal lines
    let from = camera_y_round - camera_y_round % input.step;
    let to = camera_y_round + input.viewport_size.y as i32;

    for i in (from..to).step_by(input.step as usize) {
        let y = i as f32;
        polyline.add_line(
            Vec3::new(camera_pos.x, y, 0.),
            Vec3::new(input.viewport_size.x + camera_pos.x, y, 0.),
        );
    }

    polyline.load_to_gpu(context);
    renderer.render(context, polyline);
}

fn render_sin_wave_2(
    context: &mut sumi::GraphicsContext<'_, '_>,
    view: &sumi::GraphicsView,
    geometries: &mut Resources,
    renderers: &mut Renderers,
    state: &RenderDemoState,
) {
    geometries.sin_wave.clear();
    geometries
        .sin_wave
        .set_mvp_matrix(view.screen_camera_matrix);

    let mut angle: f32 = -state.sin_amp_angle;
    let step: usize = 10;
    let end = view.size.x as usize + step * 2;

    for x in (0..end).step_by(step) {
        angle += PI / 30.;
        geometries.sin_wave.add_point(Vec3::new(
            x as f32,
            view.size.y / 2. + f32::sin(angle) * state.sin_amp,
            0.,
        ));
    }

    geometries.sin_wave.load_to_gpu(context);
    renderers.polyline.render(context, &geometries.sin_wave);
}

fn main() {
    let event_loop = EventLoop::builder().build().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut application = Application { state: None };

    unsafe { sumi::Graphics::init_memory() };
    sumi::Graphics::register_renderer(0, RenderDemoRenderer::new);

    log::info!("Init: Render Demo");

    event_loop.run_app(&mut application).unwrap();
}
