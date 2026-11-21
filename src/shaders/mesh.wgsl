// Rasterised meshes (imported .obj and procedural geo), lit like the raymarched scene.
// common.wgsl is prepended; the depth test runs against the raymarcher's depth.

struct MeshUniforms {
    model: mat4x4f,
};

@group(1) @binding(0) var<uniform> m: MeshUniforms;

struct VertexIn {
    @location(0) position: vec3f,
    @location(1) normal: vec3f,
};

struct VertexOut {
    @builtin(position) clip: vec4f,
    @location(0) world: vec3f,
    @location(1) normal: vec3f,
};

@vertex
fn vs_main(v: VertexIn) -> VertexOut {
    let world = m.model * vec4f(v.position, 1.0);
    var out: VertexOut;
    out.clip = u.view_projection * world;
    out.world = world.xyz;
    // The model matrix is rotation + uniform scale, so it can transform normals directly.
    out.normal = normalize((m.model * vec4f(v.normal, 0.0)).xyz);
    return out;
}

@fragment
fn fs_main(in: VertexOut, @builtin(front_facing) front: bool) -> @location(0) vec4f {
    let accent = palette(u.hue);
    var n = normalize(in.normal);
    if (!front) {
        n = -n; // open meshes show their inside lit too
    }
    let to_camera = normalize(u.camera_position - in.world);
    let to_light = normalize(LIGHT_POSITION - in.world);
    let diffuse = max(dot(n, to_light), 0.0);
    let half_vector = normalize(to_light + to_camera);
    let specular = pow(max(dot(n, half_vector), 0.0), 48.0) * 0.35;
    let rim = pow(1.0 - max(dot(n, to_camera), 0.0), 3.0);

    let albedo = mix(vec3f(0.55), accent, 0.35);
    var colour = albedo * (0.07 + diffuse * 0.9) + vec3f(specular) + accent * rim * u.glow * 0.8;
    colour = apply_fog(colour, length(u.camera_position - in.world));
    colour += vec3f(1.0, 0.96, 0.9) * u.flash * 0.6;
    return vec4f(colour, 1.0);
}

// Edges of procedural meshes: thin glowing lines in the accent colour.
@fragment
fn fs_line(in: VertexOut) -> @location(0) vec4f {
    let accent = palette(u.hue);
    var colour = accent * (0.7 + u.glow * 0.6);
    colour = apply_fog(colour, length(u.camera_position - in.world));
    colour += vec3f(1.0, 0.96, 0.9) * u.flash * 0.6;
    return vec4f(colour, 1.0);
}
