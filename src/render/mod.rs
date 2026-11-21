//! Drawing one frame: scene (raymarch) -> composite (backdrop treatment) -> overlays (egui).

pub mod composite_pass;
pub mod scene_pass;

use composite_pass::CompositePass;
use scene_pass::{Quality, ScenePass, SCENE_FORMAT};

use crate::scene::SceneState;
use crate::view::Backdrop;

/// egui output ready to be drawn on top of the composite.
pub struct OverlayPaint<'a> {
    pub primitives: &'a [egui::ClippedPrimitive],
    pub textures_delta: &'a egui::TexturesDelta,
    pub pixels_per_point: f32,
}

pub struct FrameRenderer {
    scene_pass: ScenePass,
    composite_pass: CompositePass,
    overlay_renderer: egui_wgpu::Renderer,
    scene_texture: Option<(wgpu::Texture, wgpu::TextureView, [u32; 2])>,
}

impl FrameRenderer {
    pub fn new(device: &wgpu::Device, output_format: wgpu::TextureFormat) -> FrameRenderer {
        FrameRenderer {
            scene_pass: ScenePass::new(device),
            composite_pass: CompositePass::new(device, output_format),
            overlay_renderer: egui_wgpu::Renderer::new(device, output_format, egui_wgpu::RendererOptions::default()),
            scene_texture: None,
        }
    }

    /// Draws a frame into `target`. When `frozen` is set the previous scene image is
    /// reused, so a frozen backdrop costs no raymarching at all.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        target_size: [u32; 2],
        scene: &SceneState,
        quality: Quality,
        frozen: bool,
        backdrop: Backdrop,
        overlay: Option<OverlayPaint<'_>>,
    ) -> wgpu::CommandBuffer {
        let scale = quality.resolution_scale();
        let scene_size = [
            ((target_size[0] as f32 * scale) as u32).max(1),
            ((target_size[1] as f32 * scale) as u32).max(1),
        ];
        let recreated = self.ensure_scene_texture(device, scene_size);

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("frame encoder"),
        });

        let (_, scene_view, _) = self.scene_texture.as_ref().expect("scene texture exists");
        if !frozen || recreated {
            self.scene_pass
                .draw(queue, &mut encoder, scene_view, scene_size, scene, quality);
        }
        self.composite_pass
            .draw(device, queue, &mut encoder, scene_view, target, backdrop);

        if let Some(overlay) = overlay {
            self.draw_overlay(device, queue, &mut encoder, target, target_size, overlay);
        }
        encoder.finish()
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

    /// Returns true when the texture had to be (re)created.
    fn ensure_scene_texture(&mut self, device: &wgpu::Device, size: [u32; 2]) -> bool {
        if let Some((_, _, current)) = &self.scene_texture {
            if *current == size {
                return false;
            }
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scene texture"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SCENE_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        self.scene_texture = Some((texture, view, size));
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
        depth_stencil: None,
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
