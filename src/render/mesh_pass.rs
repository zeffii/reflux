//! Rasterises meshes into the scene texture, depth-tested against the raymarch:
//! the imported .obj (fitted and turned by its model matrix) and the graph's
//! procedural geo (already in world space), as faces and as edge lines.

use glam::Mat4;

use super::scene_pass::{scene_shader, SceneUniformBinding, DEPTH_FORMAT, SCENE_FORMAT};
use crate::geo::Geo;
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

/// A model matrix uniform and its bind group (group 1 in mesh.wgsl).
struct ModelBinding {
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl ModelBinding {
    fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, label: &str) -> ModelBinding {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: std::mem::size_of::<MeshUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        ModelBinding { buffer, bind_group }
    }

    fn write(&self, queue: &wgpu::Queue, model: Mat4) {
        let uniforms = MeshUniforms {
            model: model.to_cols_array_2d(),
        };
        queue.write_buffer(&self.buffer, 0, bytemuck::bytes_of(&uniforms));
    }
}

/// A vertex buffer that is refilled every frame and only reallocated when it must grow.
struct DynamicVertices {
    label: &'static str,
    buffer: Option<wgpu::Buffer>,
    capacity: u64,
    count: u32,
}

impl DynamicVertices {
    fn new(label: &'static str) -> DynamicVertices {
        DynamicVertices {
            label,
            buffer: None,
            capacity: 0,
            count: 0,
        }
    }

    fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, vertices: &[Vertex]) {
        self.count = vertices.len() as u32;
        if vertices.is_empty() {
            return;
        }
        let bytes: &[u8] = bytemuck::cast_slice(vertices);
        if bytes.len() as u64 > self.capacity {
            let capacity = (bytes.len() as u64).next_power_of_two();
            self.buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(self.label),
                size: capacity,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.capacity = capacity;
        }
        queue.write_buffer(self.buffer.as_ref().expect("allocated above"), 0, bytes);
    }

    fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        if let (Some(buffer), true) = (&self.buffer, self.count > 0) {
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..self.count, 0..1);
        }
    }
}

pub struct MeshPass {
    triangle_pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    obj_model: ModelBinding,
    world_model: ModelBinding,
    obj: Option<(wgpu::Buffer, u32)>,
    procedural_triangles: DynamicVertices,
    procedural_lines: DynamicVertices,
}

impl MeshPass {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, scene_uniforms: &SceneUniformBinding) -> MeshPass {
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
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mesh pipeline layout"),
            bind_group_layouts: &[Some(&scene_uniforms.layout), Some(&layout)],
            immediate_size: 0,
        });

        let make_pipeline = |label: &str, topology: wgpu::PrimitiveTopology, fragment_entry: &str| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
                    })],
                },
                // No culling: imported meshes are often open or inconsistently wound.
                primitive: wgpu::PrimitiveState {
                    topology,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fragment_entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: SCENE_FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };

        let world_model = ModelBinding::new(device, &layout, "world model");
        world_model.write(queue, Mat4::IDENTITY);

        MeshPass {
            triangle_pipeline: make_pipeline("mesh triangles", wgpu::PrimitiveTopology::TriangleList, "fs_main"),
            line_pipeline: make_pipeline("mesh lines", wgpu::PrimitiveTopology::LineList, "fs_line"),
            obj_model: ModelBinding::new(device, &layout, "obj model"),
            world_model,
            obj: None,
            procedural_triangles: DynamicVertices::new("procedural triangles"),
            procedural_lines: DynamicVertices::new("procedural lines"),
        }
    }

    /// Replaces the imported mesh (or removes it with `None`).
    pub fn set_obj(&mut self, device: &wgpu::Device, mesh: Option<&MeshData>) {
        self.obj = mesh.map(|data| {
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
                    label: Some("obj vertices"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            );
            (buffer, vertices.len() as u32)
        });
    }

    pub fn write_obj_model(&self, queue: &wgpu::Queue, model: Mat4) {
        self.obj_model.write(queue, model);
    }

    /// Uploads this frame's procedural geo: `solid` as faces, `wire` as edges.
    pub fn set_procedural(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, solid: &Geo, wire: &Geo) {
        let triangles: Vec<Vertex> = solid
            .iter()
            .flat_map(|mesh| mesh.triangles())
            .map(|(position, normal)| Vertex { position, normal })
            .collect();
        let lines: Vec<Vertex> = wire
            .iter()
            .flat_map(|mesh| mesh.lines())
            .map(|position| Vertex {
                position,
                normal: [0.0, 1.0, 0.0],
            })
            .collect();
        self.procedural_triangles.upload(device, queue, &triangles);
        self.procedural_lines.upload(device, queue, &lines);
    }

    /// Draws into the scene pass, after the raymarch has written colour and depth.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, scene_uniforms: &SceneUniformBinding) {
        pass.set_bind_group(0, &scene_uniforms.bind_group, &[]);

        pass.set_pipeline(&self.triangle_pipeline);
        if let Some((buffer, count)) = &self.obj {
            pass.set_bind_group(1, &self.obj_model.bind_group, &[]);
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..*count, 0..1);
        }
        pass.set_bind_group(1, &self.world_model.bind_group, &[]);
        self.procedural_triangles.draw(pass);

        pass.set_pipeline(&self.line_pipeline);
        pass.set_bind_group(1, &self.world_model.bind_group, &[]);
        self.procedural_lines.draw(pass);
    }
}
