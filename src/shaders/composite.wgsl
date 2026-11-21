// Puts the rendered scene on screen: tone mapping, then the backdrop treatment
// (dim, desaturate, blur) used when overlays are open.

struct CompositeUniforms {
    dim: f32,         // 0 = full brightness, 1 = black
    desaturate: f32,  // 0 = full colour, 1 = grey
    blur_pixels: f32, // blur radius in pixels of the scene texture
    _pad: f32,
};

@group(0) @binding(0) var<uniform> c: CompositeUniforms;
@group(0) @binding(1) var scene_texture: texture_2d<f32>;
@group(0) @binding(2) var scene_sampler: sampler;

struct VertexOut {
    @builtin(position) position: vec4f,
    @location(0) uv: vec2f,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOut {
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    var out: VertexOut;
    out.position = vec4f(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    out.uv = vec2f(x, y);
    return out;
}

fn sample_scene(uv: vec2f) -> vec3f {
    if (c.blur_pixels < 0.5) {
        return textureSample(scene_texture, scene_sampler, uv).rgb;
    }
    // Two rings of taps around the centre: cheap and soft enough for a backdrop.
    let texel = 1.0 / vec2f(textureDimensions(scene_texture));
    var total = textureSample(scene_texture, scene_sampler, uv).rgb;
    var count = 1.0;
    for (var ring = 1; ring <= 2; ring++) {
        let radius = c.blur_pixels * f32(ring) * 0.5;
        for (var i = 0; i < 8; i++) {
            let angle = f32(i) * 0.785398 + f32(ring) * 0.39;
            let offset = vec2f(cos(angle), sin(angle)) * radius * texel;
            total += textureSample(scene_texture, scene_sampler, uv + offset).rgb;
            count += 1.0;
        }
    }
    return total / count;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4f {
    var colour = sample_scene(in.uv);
    colour = colour / (colour + vec3f(1.0)) * 1.4; // gentle Reinhard tone map
    let grey = dot(colour, vec3f(0.299, 0.587, 0.114));
    colour = mix(colour, vec3f(grey), c.desaturate);
    colour *= 1.0 - c.dim;
    return vec4f(colour, 1.0);
}
