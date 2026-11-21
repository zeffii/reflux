//! Rasterises imported meshes into the scene texture, depth-tested against the raymarch.

use glam::Mat4;

use super::scene_pass::{scene_shader, SceneUniformBinding, DEPTH_FORMAT, SCENE_FORMAT};
use crate::mesh::MeshData;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct MeshUniforms {
    model: [[f32; 4]; 4],
}

/// One mesh uploaded to the GPU.
struct GpuMesh {
    vertices: wgpu::Buffer,
    vertex_count: u32,
}

pub struct MeshPass {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    mesh: Option<GpuMesh>,
}

impl MeshPass {
    pub fn new(device: &wgpu::Device, scene_uniforms: &SceneUniformBinding) -> MeshPass {
        let shader = scene_shader(device, "mesh shader", include_str!("../shaders/mesh.wgsl"));

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mesh bind layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mesh uniforms"),
            size: std::mem::size_of::<MeshUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mesh bind group"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mesh pipeline layout"),
            bind_group_layouts: &[Some(&scene_uniforms.layout), Some(&layout)],
            immediate_size: 0,
        });

        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mesh pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(vertex_layout)],
            },
            // No culling: imported meshes are often open or inconsistently wound.
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: SCENE_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        MeshPass {
            pipeline,
            uniform_buffer,
            bind_group,
            mesh: None,
        }
    }

    /// Replaces the uploaded mesh (or removes it with `None`).
    pub fn set_mesh(&mut self, device: &wgpu::Device, mesh: Option<&MeshData>) {
        self.mesh = mesh.map(|data| {
            let vertices: Vec<Vertex> = data
                .positions
                .iter()
                .zip(&data.normals)
                .map(|(position, normal)| Vertex {
                    position: *position,
                    normal: *normal,
                })
                .collect();
            let buffer = wgpu::util::DeviceExt::create_buffer_init(
                device,
                &wgpu::util::BufferInitDescriptor {
                    label: Some("mesh vertices"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            );
            GpuMesh {
                vertices: buffer,
                vertex_count: vertices.len() as u32,
            }
        });
    }

    pub fn write_uniforms(&self, queue: &wgpu::Queue, model: Mat4) {
        let uniforms = MeshUniforms {
            model: model.to_cols_array_2d(),
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    /// Draws into the scene pass, after the raymarch has written colour and depth.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, scene_uniforms: &SceneUniformBinding) {
        let Some(mesh) = &self.mesh else {
            return;
        };
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &scene_uniforms.bind_group, &[]);
        pass.set_bind_group(1, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, mesh.vertices.slice(..));
        pass.draw(0..mesh.vertex_count, 0..1);
    }
}
