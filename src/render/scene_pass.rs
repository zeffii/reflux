//! The raymarch pass: draws the SDF scene into an HDR texture.

use crate::scene::SceneState;

/// Rendering quality. The default keeps the full look; Eco is opt-in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quality {
    Standard,
    Eco,
}

impl Quality {
    fn march_steps(self) -> f32 {
        match self {
            Quality::Standard => 128.0,
            Quality::Eco => 64.0,
        }
    }

    /// Fraction of the window resolution the scene is rendered at.
    pub fn resolution_scale(self) -> f32 {
        match self {
            Quality::Standard => 1.0,
            Quality::Eco => 0.5,
        }
    }
}

/// Matches `SceneUniforms` in scene.wgsl field for field (80 bytes).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SceneUniforms {
    resolution: [f32; 2],
    seconds: f32,
    beat: f32,
    camera_position: [f32; 3],
    fov_degrees: f32,
    camera_target: [f32; 3],
    hue: f32,
    glow: f32,
    flash: f32,
    twist: f32,
    ring_pulse: f32,
    ring_travel: f32,
    quality_steps: f32,
    _pad: [f32; 2],
}

pub const SCENE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

pub struct ScenePass {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl ScenePass {
    pub fn new(device: &wgpu::Device) -> ScenePass {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/scene.wgsl").into()),
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene uniforms"),
            size: std::mem::size_of::<SceneUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene bind layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene bind group"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });

        let pipeline = super::fullscreen_pipeline(device, "scene pipeline", &pipeline_layout, &shader, SCENE_FORMAT);

        ScenePass {
            pipeline,
            uniform_buffer,
            bind_group,
        }
    }

    pub fn draw(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: [u32; 2],
        state: &SceneState,
        quality: Quality,
    ) {
        let uniforms = SceneUniforms {
            resolution: [size[0] as f32, size[1] as f32],
            seconds: state.seconds,
            beat: state.beat,
            camera_position: state.camera_position,
            fov_degrees: state.fov_degrees,
            camera_target: state.camera_target,
            hue: state.hue,
            glow: state.glow,
            flash: state.flash,
            twist: state.twist,
            ring_pulse: state.ring_pulse,
            ring_travel: state.ring_travel,
            quality_steps: quality.march_steps(),
            _pad: [0.0; 2],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        let mut pass = super::begin_pass(encoder, "scene pass", target);
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
