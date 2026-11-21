//! The raymarch pass: draws the SDF scene into an HDR texture.

use crate::scene::SceneState;

/// Everything the scene shaders need for one frame besides the scene values themselves.
pub struct SceneFrame<'a> {
    pub state: &'a SceneState,
    pub size: [u32; 2],
    pub quality: Quality,
    pub core_visible: bool,
}

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

/// Matches `SceneUniforms` in common.wgsl field for field (144 bytes).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SceneUniforms {
    view_projection: [[f32; 4]; 4],
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
    core_visible: f32,
    _pad: f32,
}

pub const SCENE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// The scene's shared uniforms (bind group 0 for every scene shader).
pub struct SceneUniformBinding {
    pub layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
    buffer: wgpu::Buffer,
}

/// Prepends the shared WGSL (uniforms, light, fog, palette) to a shader's own source.
pub fn scene_shader(device: &wgpu::Device, label: &str, source: &str) -> wgpu::ShaderModule {
    let full = format!("{}\n{}", include_str!("../shaders/common.wgsl"), source);
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(full.into()),
    })
}

pub struct ScenePass {
    pipeline: wgpu::RenderPipeline,
    pub uniforms: SceneUniformBinding,
}

impl ScenePass {
    pub fn new(device: &wgpu::Device) -> ScenePass {
        let shader = scene_shader(device, "scene shader", include_str!("../shaders/scene.wgsl"));

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
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
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

        // The raymarcher writes depth for every pixel, so it always passes the test.
        let depth = wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Always),
            stencil: Default::default(),
            bias: Default::default(),
        };
        let pipeline =
            super::fullscreen_pipeline(device, "scene pipeline", &pipeline_layout, &shader, SCENE_FORMAT, Some(depth));

        ScenePass {
            pipeline,
            uniforms: SceneUniformBinding {
                layout,
                bind_group,
                buffer: uniform_buffer,
            },
        }
    }

    pub fn write_uniforms(&self, queue: &wgpu::Queue, frame: &SceneFrame<'_>) {
        let state = frame.state;
        let aspect = frame.size[0] as f32 / frame.size[1] as f32;
        let uniforms = SceneUniforms {
            view_projection: crate::camera::view_projection(state, aspect).to_cols_array_2d(),
            resolution: [frame.size[0] as f32, frame.size[1] as f32],
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
            quality_steps: frame.quality.march_steps(),
            core_visible: if frame.core_visible { 1.0 } else { 0.0 },
            _pad: 0.0,
        };
        queue.write_buffer(&self.uniforms.buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.uniforms.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
