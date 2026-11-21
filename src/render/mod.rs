//! Drawing one frame: scene (raymarch) -> composite (backdrop treatment) -> overlays (egui).

pub mod composite_pass;
pub mod mesh_pass;
pub mod scene_pass;

use composite_pass::CompositePass;
use mesh_pass::MeshPass;
use scene_pass::{Quality, SceneFrame, ScenePass, DEPTH_FORMAT, SCENE_FORMAT};

use crate::mesh::MeshData;
use crate::scene::SceneState;
use crate::view::Backdrop;

/// What to draw this frame.
pub struct FrameInputs<'a> {
    pub scene: &'a SceneState,
    pub quality: Quality,
    /// Reuse the previous scene image: a frozen backdrop costs no raymarching.
    pub frozen: bool,
    pub backdrop: Backdrop,
    pub core_visible: bool,
}

/// The scene's colour and depth targets, recreated when the size changes.
struct SceneTargets {
    size: [u32; 2],
    colour: wgpu::TextureView,
    depth: wgpu::TextureView,
}

/// egui output ready to be drawn on top of the composite.
pub struct OverlayPaint<'a> {
    pub primitives: &'a [egui::ClippedPrimitive],
    pub textures_delta: &'a egui::TexturesDelta,
    pub pixels_per_point: f32,
}

pub struct FrameRenderer {
    scene_pass: ScenePass,
    mesh_pass: MeshPass,
    /// Centre and radius of the loaded mesh, used to fit it into the scene.
    mesh_bounds: ([f32; 3], f32),
    composite_pass: CompositePass,
    overlay_renderer: egui_wgpu::Renderer,
    targets: Option<SceneTargets>,
}

impl FrameRenderer {
    pub fn new(device: &wgpu::Device, output_format: wgpu::TextureFormat) -> FrameRenderer {
        let scene_pass = ScenePass::new(device);
        let mesh_pass = MeshPass::new(device, &scene_pass.uniforms);
        FrameRenderer {
            scene_pass,
            mesh_pass,
            mesh_bounds: ([0.0; 3], 1.0),
            composite_pass: CompositePass::new(device, output_format),
            overlay_renderer: egui_wgpu::Renderer::new(device, output_format, egui_wgpu::RendererOptions::default()),
            targets: None,
        }
    }

    pub fn set_mesh(&mut self, device: &wgpu::Device, mesh: Option<&MeshData>) {
        if let Some(data) = mesh {
            self.mesh_bounds = data.bounds();
        }
        self.mesh_pass.set_mesh(device, mesh);
        self.targets = None; // forces a fresh scene image even while frozen
    }


    /// Draws a frame into `target` and returns the commands to submit.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        target_size: [u32; 2],
        inputs: &FrameInputs<'_>,
        overlay: Option<OverlayPaint<'_>>,
    ) -> wgpu::CommandBuffer {
        let scale = inputs.quality.resolution_scale();
        let scene_size = [
            ((target_size[0] as f32 * scale) as u32).max(1),
            ((target_size[1] as f32 * scale) as u32).max(1),
        ];
        let recreated = self.ensure_targets(device, scene_size);

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("frame encoder"),
        });

        if !inputs.frozen || recreated {
            self.draw_scene(queue, &mut encoder, scene_size, inputs);
        }
        let targets = self.targets.as_ref().expect("scene targets exist");
        self.composite_pass
            .draw(device, queue, &mut encoder, &targets.colour, target, inputs.backdrop);

        if let Some(overlay) = overlay {
            self.draw_overlay(device, queue, &mut encoder, target, target_size, overlay);
        }
        encoder.finish()
    }

    /// Raymarch first (writing colour and depth), then meshes depth-tested against it.
    fn draw_scene(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        scene_size: [u32; 2],
        inputs: &FrameInputs<'_>,
    ) {
        self.scene_pass.write_uniforms(
            queue,
            &SceneFrame {
                state: inputs.scene,
                size: scene_size,
                quality: inputs.quality,
                core_visible: inputs.core_visible,
            },
        );
        let (centre, radius) = self.mesh_bounds;
        self.mesh_pass
            .write_uniforms(queue, crate::camera::mesh_model(centre, radius, inputs.scene.seconds));

        let targets = self.targets.as_ref().expect("scene targets exist");
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("scene pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &targets.colour,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &targets.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });
        self.scene_pass.draw(&mut pass);
        self.mesh_pass.draw(&mut pass, &self.scene_pass.uniforms);
    }

    fn draw_overlay(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        target_size: [u32; 2],
        overlay: OverlayPaint<'_>,
    ) {
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: target_size,
            pixels_per_point: overlay.pixels_per_point,
        };
        for (id, deltas) in &overlay.textures_delta.set {
            for delta in deltas {
                self.overlay_renderer.update_texture(device, queue, *id, delta);
            }
        }
        let extra = self
            .overlay_renderer
            .update_buffers(device, queue, encoder, overlay.primitives, &screen);
        queue.submit(extra);

        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("overlay pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
            let mut pass = pass.forget_lifetime();
            self.overlay_renderer.render(&mut pass, overlay.primitives, &screen);
        }

        for id in &overlay.textures_delta.free {
            self.overlay_renderer.free_texture(id);
        }
    }

    /// Returns true when the targets had to be (re)created.
    fn ensure_targets(&mut self, device: &wgpu::Device, size: [u32; 2]) -> bool {
        if self.targets.as_ref().is_some_and(|t| t.size == size) {
            return false;
        }
        let make = |label: &str, format: wgpu::TextureFormat, usage: wgpu::TextureUsages| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: size[0],
                        height: size[1],
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        self.targets = Some(SceneTargets {
            size,
            colour: make(
                "scene colour",
                SCENE_FORMAT,
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            ),
            depth: make("scene depth", DEPTH_FORMAT, wgpu::TextureUsages::RENDER_ATTACHMENT),
        });
        true
    }
}

/// A pipeline that draws one full-screen triangle (`vs_main` / `fs_main`).
fn fullscreen_pipeline(
    device: &wgpu::Device,
    label: &str,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    depth_stencil: Option<wgpu::DepthStencilState>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn begin_pass<'e>(
    encoder: &'e mut wgpu::CommandEncoder,
    label: &str,
    target: &wgpu::TextureView,
) -> wgpu::RenderPass<'e> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
            depth_slice: None,
        })],
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        timestamp_writes: None,
        multiview_mask: None,
    })
}
