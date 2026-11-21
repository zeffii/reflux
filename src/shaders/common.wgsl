// Shared by scene.wgsl and mesh.wgsl (prepended to both): the per-frame uniforms,
// the light, fog and palette, so meshes and SDFs are lit the same way.

struct SceneUniforms {
    view_projection: mat4x4f,
    resolution: vec2f,
    seconds: f32,
    beat: f32,
    camera_position: vec3f,
    fov_degrees: f32,
    camera_target: vec3f,
    hue: f32,
    glow: f32,
    flash: f32,
    twist: f32,
    ring_pulse: f32,
    ring_travel: f32,
    quality_steps: f32,
    core_visible: f32,
    _pad: f32,
};

@group(0) @binding(0) var<uniform> u: SceneUniforms;

const LIGHT_POSITION: vec3f = vec3f(0.0, 2.5, 3.5);
const FOG_COLOUR: vec3f = vec3f(0.012, 0.016, 0.022);
const FOG_DENSITY: f32 = 0.09;

// Hue 0..1 walks blue -> teal -> green -> amber -> orange, avoiding purple and pink.
fn palette(hue: f32) -> vec3f {
    let stops = array<vec3f, 5>(
        vec3f(0.10, 0.30, 0.85),
        vec3f(0.05, 0.70, 0.70),
        vec3f(0.35, 0.80, 0.30),
        vec3f(0.95, 0.62, 0.15),
        vec3f(0.95, 0.30, 0.08),
    );
    let x = clamp(hue, 0.0, 1.0) * 4.0;
    let i = min(u32(floor(x)), 3u);
    return mix(stops[i], stops[i + 1u], x - f32(i));
}

fn apply_fog(colour: vec3f, distance: f32) -> vec3f {
    return mix(colour, FOG_COLOUR, 1.0 - exp(-distance * FOG_DENSITY));
}

// Depth-buffer value for a world position, matching the rasteriser's.
fn depth_of(world: vec3f) -> f32 {
    let clip = u.view_projection * vec4f(world, 1.0);
    return clamp(clip.z / clip.w, 0.0, 0.999999);
}
