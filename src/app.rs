//! The interactive window: one window, render as backdrop, overlays on top.

use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::events::song::Song;
use crate::gpu::Gpu;
use crate::mesh::{self, LoadedMesh};
use crate::graph::eval;
use crate::overlay::{self, graph::GraphView, OverlayFrame};
use crate::project::Project;
use crate::render::{FrameInputs, FrameRenderer, OverlayPaint};
use crate::scene::SceneState;
use crate::view::{Overlay, ViewState};

/// Playback time that can be paused and restarted.
struct Clock {
    seconds: f64,
    last: Instant,
}

impl Clock {
    fn new() -> Clock {
        Clock {
            seconds: 0.0,
            last: Instant::now(),
        }
    }

    fn advance(&mut self, playing: bool) -> f64 {
        let now = Instant::now();
        if playing {
            self.seconds += (now - self.last).as_secs_f64();
        }
        self.last = now;
        self.seconds
    }
}

/// Everything that only exists once the window is open.
struct Running {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    gpu: Gpu,
    renderer: FrameRenderer,
    egui_ctx: egui::Context,
    egui_state: egui_winit::State,
}

struct App {
    song: Song,
    view: ViewState,
    project: Project,
    graph_view: GraphView,
    clock: Clock,
    ctrl_held: bool,
    mesh: Option<LoadedMesh>,
    running: Option<Running>,
}

pub fn run(obj_path: Option<std::path::PathBuf>, scene_path: Option<std::path::PathBuf>) {
    let event_loop = EventLoop::new().expect("could not create the event loop");
    let song = Song::demo();
    let mut project = match scene_path {
        Some(path) => Project::open(&path, &song.table),
        None => Project::empty(),
    };
    overlay::graph::auto_layout(&mut project.graph);
    if let Some(error) = &project.error {
        eprintln!("{error}");
    }
    let mut app = App {
        song,
        view: ViewState::default(),
        project,
        graph_view: GraphView::default(),
        clock: Clock::new(),
        ctrl_held: false,
        mesh: None,
        running: None,
    };
    if let Some(path) = obj_path {
        app.load_mesh(&path);
    }
    event_loop.run_app(&mut app).expect("event loop failed");
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.running.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("reflux")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0));
        let window = Arc::new(event_loop.create_window(attributes).expect("could not open a window"));

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(
            event_loop.owned_display_handle(),
        )));
        let surface = instance.create_surface(window.clone()).expect("could not create a surface");
        let gpu = Gpu::new(instance, Some(&surface));
        eprintln!("rendering on {}", gpu.describe());

        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&gpu.adapter, size.width.max(1), size.height.max(1))
            .expect("surface is not supported by the adapter");
        config.present_mode = wgpu::PresentMode::AutoVsync;
        surface.configure(&gpu.device, &config);

        let mut renderer = FrameRenderer::new(&gpu.device, &gpu.queue, config.format);
        renderer.set_mesh(&gpu.device, self.mesh.as_ref().map(|m| &m.data));
        let egui_ctx = egui::Context::default();
        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            Some(gpu.device.limits().max_texture_dimension_2d as usize),
        );

        self.running = Some(Running {
            window,
            surface,
            config,
            gpu,
            renderer,
            egui_ctx,
            egui_state,
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(running) = self.running.as_mut() else {
            return;
        };
        // egui sees every event (node dragging); Reflux's own keys are handled below regardless.
        let _ = running.egui_state.on_window_event(&running.window, &event);

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                running.config.width = size.width.max(1);
                running.config.height = size.height.max(1);
                running.surface.configure(&running.gpu.device, &running.config);
            }
            WindowEvent::KeyboardInput { event, .. } => self.handle_key(&event),
            WindowEvent::DroppedFile(path) => self.load_mesh(&path),
            WindowEvent::ModifiersChanged(modifiers) => self.ctrl_held = modifiers.state().control_key(),
            WindowEvent::RedrawRequested => self.draw_frame(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(running) = &self.running {
            running.window.request_redraw();
        }
    }
}

impl App {
    /// Loads an .obj, puts it in the scene in place of the SDF core, and names it in the graph.
    fn load_mesh(&mut self, path: &std::path::Path) {
        let is_obj = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("obj"));
        if !is_obj {
            eprintln!("not an .obj file: {}", path.display());
            return;
        }
        match mesh::load_obj(path) {
            Ok(loaded) => {
                eprintln!("loaded {} ({} triangles)", loaded.name, loaded.data.triangle_count());
                self.view.core_visible = false;
                if let Some(running) = self.running.as_mut() {
                    running.renderer.set_mesh(&running.gpu.device, Some(&loaded.data));
                }
                self.mesh = Some(loaded);
            }
            Err(message) => eprintln!("could not load mesh: {message}"),
        }
    }

    fn handle_key(&mut self, event: &KeyEvent) {
        let PhysicalKey::Code(code) = event.physical_key else {
            return;
        };
        // Peek is a hold key: it reacts to press and release.
        if code == KeyCode::KeyP {
            self.view.peeking = event.state == ElementState::Pressed;
            return;
        }
        if event.state != ElementState::Pressed || event.repeat {
            return;
        }
        match code {
            KeyCode::Tab => self.view.toggle_overlay(Overlay::Graph),
            KeyCode::KeyL => self.view.toggle_overlay(Overlay::Lane),
            KeyCode::Escape => self.view.overlay = None,
            KeyCode::KeyD => self.view.cycle_dim(),
            KeyCode::KeyS if self.ctrl_held => self.save_layout(),
            KeyCode::KeyS => self.view.toggle_soft(),
            KeyCode::KeyF => self.view.frozen = !self.view.frozen,
            KeyCode::KeyQ => self.view.toggle_quality(),
            KeyCode::KeyC => self.view.core_visible = !self.view.core_visible,
            KeyCode::Space => self.view.playing = !self.view.playing,
            KeyCode::Home => self.clock.seconds = 0.0,
            _ => {}
        }
    }

    fn save_layout(&mut self) {
        match self.project.save_layout() {
            Ok(()) => eprintln!("saved layout to {}", self.project.name()),
            Err(message) => self.project.error = Some(message),
        }
    }

    fn draw_frame(&mut self) {
        let seconds = self.clock.advance(self.view.playing);
        if self.project.poll(&self.song.table) {
            overlay::graph::auto_layout(&mut self.project.graph);
        }
        let evaluation = eval::evaluate(&self.project.graph, &self.song, seconds);
        let Some(running) = self.running.as_mut() else {
            return;
        };

        let surface_texture = match running.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture) | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                running.surface.configure(&running.gpu.device, &running.config);
                return;
            }
            _ => return, // timeout or occluded: try again next frame
        };
        let target_view = surface_texture.texture.create_view(&Default::default());
        let target_size = [running.config.width, running.config.height];

        let raw_input = running.egui_state.take_egui_input(&running.window);
        let scene_name = self.project.name();
        let scene_error = self.project.error.clone();
        let frame = OverlayFrame {
            view: &self.view,
            song: &self.song,
            evaluation: &evaluation,
            scene_name: &scene_name,
            scene_error: scene_error.as_deref(),
            seconds,
            show_status: true,
            mesh: self.mesh.as_ref().map(|m| (m.name.as_str(), m.data.triangle_count())),
        };
        let graph = &mut self.project.graph;
        let graph_view = &mut self.graph_view;
        let output = running
            .egui_ctx
            .run_pass(raw_input, |ctx| {
                overlay::build(ctx, graph_view, graph, &frame);
            });
        running
            .egui_state
            .handle_platform_output(&running.window, output.platform_output.clone());
        let primitives = running.egui_ctx.tessellate(output.shapes, output.pixels_per_point);

        let mut scene = SceneState::sample(&self.song, seconds);
        if let Some(camera) = &evaluation.camera {
            scene.apply_camera(camera);
        }
        let commands = running.renderer.render(
            &running.gpu.device,
            &running.gpu.queue,
            &target_view,
            target_size,
            &FrameInputs {
                scene: &scene,
                quality: self.view.quality,
                frozen: self.view.frozen,
                backdrop: self.view.backdrop(),
                core_visible: self.view.core_visible,
                solid: &evaluation.solid,
                wire: &evaluation.wire,
            },
            Some(OverlayPaint {
                primitives: &primitives,
                textures_delta: &output.textures_delta,
                pixels_per_point: output.pixels_per_point,
            }),
        );
        running.gpu.queue.submit([commands]);
        running.window.pre_present_notify();
        running.gpu.queue.present(surface_texture);
    }
}
