//! Headless rendering: one frame straight to a PNG, no window needed.
//!
//! `reflux --shot out.png --time 6.5 --size 1280x720 --overlay graph --dim 2 --soft`

use crate::events::song::Song;
use crate::overlay::{self, graph::Graph, OverlayFrame};
use crate::render::{FrameRenderer, OverlayPaint};
use crate::scene::SceneState;
use crate::view::{DimSetting, Overlay, ViewState};

pub struct ShotOptions {
    pub path: String,
    pub seconds: f64,
    pub size: [u32; 2],
    pub overlay: Option<Overlay>,
    pub dim: Option<DimSetting>,
    pub show_status: bool,
}

const SHOT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

pub fn run(options: ShotOptions) {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let gpu = crate::gpu::Gpu::new(instance, None);
    eprintln!("rendering on {}", gpu.describe());

    let song = Song::demo();
    let mut view = ViewState::default();
    view.overlay = options.overlay;
    if let Some(dim) = options.dim {
        view.set_dim(dim);
    }

    let scene = SceneState::sample(&song, options.seconds);
    let mut renderer = FrameRenderer::new(&gpu.device, SHOT_FORMAT);

    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("shot target"),
        size: wgpu::Extent3d {
            width: options.size[0],
            height: options.size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: SHOT_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target.create_view(&Default::default());

    // Build the overlay exactly as the window would, at 1 pixel per point.
    let ctx = egui::Context::default();
    let mut graph = Graph::demo();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(options.size[0] as f32, options.size[1] as f32),
        )),
        ..Default::default()
    };
    let frame = OverlayFrame {
        view: &view,
        song: &song,
        seconds: options.seconds,
        show_status: options.show_status,
    };
    let output = ctx.run_pass(input, |ctx| overlay::build(ctx, &mut graph, &frame));
    let primitives = ctx.tessellate(output.shapes, output.pixels_per_point);

    let commands = renderer.render(
        &gpu.device,
        &gpu.queue,
        &target_view,
        options.size,
        &scene,
        view.quality,
        false,
        view.backdrop(),
        Some(OverlayPaint {
            primitives: &primitives,
            textures_delta: &output.textures_delta,
            pixels_per_point: output.pixels_per_point,
        }),
    );
    gpu.queue.submit([commands]);

    let pixels = read_back(&gpu, &target, options.size);
    image::save_buffer(&options.path, &pixels, options.size[0], options.size[1], image::ColorType::Rgba8)
        .expect("could not write the PNG");
    eprintln!("wrote {}", options.path);
}

/// Copies the target texture into CPU memory as tightly packed RGBA rows.
fn read_back(gpu: &crate::gpu::Gpu, texture: &wgpu::Texture, size: [u32; 2]) -> Vec<u8> {
    let unpadded_row = size[0] * 4;
    let padded_row = unpadded_row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("shot readback"),
        size: (padded_row * size[1]) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_row),
                rows_per_image: Some(size[1]),
            },
        },
        wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit([encoder.finish()]);

    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |result| result.expect("readback mapping failed"));
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("GPU did not finish");

    let data = slice.get_mapped_range().expect("readback range");
    let mut pixels = Vec::with_capacity((unpadded_row * size[1]) as usize);
    for row in data.chunks(padded_row as usize) {
        pixels.extend_from_slice(&row[..unpadded_row as usize]);
    }
    pixels
}
