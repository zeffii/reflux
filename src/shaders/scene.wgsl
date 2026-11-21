// Raymarched demo scene: a ribbed, twisting tunnel around a pulsing core object.
// Every animated value arrives through `u`, so animation never recompiles the shader.
// common.wgsl is prepended. Writes depth so rasterised meshes intersect correctly.

const TUNNEL_RADIUS: f32 = 6.0;
const RIB_SPACING: f32 = 1.5;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4f {
    // One triangle covering the whole screen.
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    return vec4f(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
}

// ---- small sdf toolkit -------------------------------------------------------

fn rotate2(p: vec2f, angle: f32) -> vec2f {
    let c = cos(angle);
    let s = sin(angle);
    return vec2f(c * p.x - s * p.y, s * p.x + c * p.y);
}

fn sd_torus(p: vec3f, major: f32, minor: f32) -> f32 {
    let q = vec2f(length(p.xz) - major, p.y);
    return length(q) - minor;
}

fn sd_box(p: vec3f, half_size: vec3f, rounding: f32) -> f32 {
    let q = abs(p) - half_size + rounding;
    return length(max(q, vec3f(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0) - rounding;
}

fn smooth_union(a: f32, b: f32, k: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
    return mix(b, a, h) - k * h * (1.0 - h);
}

// ---- the scene ---------------------------------------------------------------

fn tunnel(p_in: vec3f) -> f32 {
    var p = p_in;
    let twisted = rotate2(p.xy, p.z * u.twist * 0.15);
    p = vec3f(twisted, p.z);
    let wall = TUNNEL_RADIUS - length(p.xy);
    // Ribs: rings at regular spacing along z, with an octagonal profile.
    let z = p.z - RIB_SPACING * round(p.z / RIB_SPACING);
    let angle = atan2(p.y, p.x);
    let facet = cos(angle * 8.0) * 0.08;
    let rib = max(abs(z) - 0.12, (TUNNEL_RADIUS - 0.35 + facet) - length(p.xy));
    return min(wall, rib) * 0.8;
}

fn core(p_in: vec3f) -> f32 {
    var p = p_in;
    let spin = u.seconds * 0.6;
    let xz = rotate2(p.xz, spin);
    p = vec3f(xz.x, p.y, xz.y);
    let yz = rotate2(p.yz, spin * 0.7);
    p = vec3f(p.x, yz.x, yz.y);
    let ring = sd_torus(p, 1.0, 0.18);
    let block = sd_box(p, vec3f(0.45), 0.08);
    return smooth_union(ring, block, 0.35);
}

// The core can be hidden (e.g. while an imported mesh takes its place).
fn core_distance(p: vec3f) -> f32 {
    if (u.core_visible < 0.5) {
        return 1e5;
    }
    return core(p);
}

// Returns (distance, material): 0 = tunnel, 1 = core.
fn map(p: vec3f) -> vec2f {
    let t = tunnel(p);
    let c = core_distance(p);
    if (c < t) {
        return vec2f(c, 1.0);
    }
    return vec2f(t, 0.0);
}

fn normal_at(p: vec3f) -> vec3f {
    let e = vec2f(0.001, -0.001);
    return normalize(
        e.xyy * map(p + e.xyy).x + e.yyx * map(p + e.yyx).x +
        e.yxy * map(p + e.yxy).x + e.xxx * map(p + e.xxx).x
    );
}

fn soft_shadow(origin: vec3f, direction: vec3f) -> f32 {
    var result = 1.0;
    var t = 0.02;
    for (var i = 0; i < 48; i++) {
        let d = map(origin + direction * t).x;
        result = min(result, 8.0 * d / t);
        t += clamp(d, 0.01, 0.25);
        if (result < 0.01 || t > 8.0) {
            break;
        }
    }
    return clamp(result, 0.0, 1.0);
}

fn ambient_occlusion(p: vec3f, n: vec3f) -> f32 {
    var occlusion = 0.0;
    var weight = 1.0;
    for (var i = 1; i <= 5; i++) {
        let h = 0.06 * f32(i);
        occlusion += (h - map(p + n * h).x) * weight;
        weight *= 0.7;
    }
    return clamp(1.0 - 2.5 * occlusion, 0.0, 1.0);
}

fn camera_ray(frag: vec2f) -> vec3f {
    let uv = (frag - 0.5 * u.resolution) / u.resolution.y * vec2f(1.0, -1.0);
    let forward = normalize(u.camera_target - u.camera_position);
    let right = normalize(cross(forward, vec3f(0.0, 1.0, 0.0)));
    let up = cross(right, forward);
    let focal = 0.5 / tan(radians(u.fov_degrees) * 0.5);
    return normalize(forward * focal + right * uv.x + up * uv.y);
}

struct SceneOut {
    @location(0) colour: vec4f,
    @builtin(frag_depth) depth: f32,
};

@fragment
fn fs_main(@builtin(position) frag: vec4f) -> SceneOut {
    let origin = u.camera_position;
    let direction = camera_ray(frag.xy);
    let accent = palette(u.hue);

    var t = 0.0;
    var hit = vec2f(0.0, -1.0);
    var core_glow = 0.0;
    let max_steps = i32(u.quality_steps);
    for (var i = 0; i < max_steps; i++) {
        let sample = map(origin + direction * t);
        core_glow += exp(-core_distance(origin + direction * t) * 4.0) * 0.02;
        if (sample.x < 0.001 * t) {
            hit = vec2f(t, sample.y);
            break;
        }
        t += sample.x;
        if (t > 40.0) {
            break;
        }
    }

    var colour = FOG_COLOUR;
    var depth = 1.0;

    if (hit.y >= 0.0) {
        let p = origin + direction * hit.x;
        let n = normal_at(p);
        let to_light = normalize(LIGHT_POSITION - p);
        let diffuse = max(dot(n, to_light), 0.0) * soft_shadow(p + n * 0.01, to_light);
        let ao = ambient_occlusion(p, n);
        let rim = pow(1.0 - max(dot(n, -direction), 0.0), 3.0);

        var albedo = vec3f(0.07, 0.08, 0.09);
        var emissive = vec3f(0.0);
        if (hit.y > 0.5) {
            albedo = mix(vec3f(0.5), accent, 0.6);
            emissive = accent * rim * u.glow;
        } else {
            // A pulse ring travels down the tunnel along z and lights the ribs it passes.
            let rib_z = abs(p.z - RIB_SPACING * round(p.z / RIB_SPACING));
            // Only the rib edges glow: thin lines rather than whole bands.
            let on_rib = 1.0 - smoothstep(0.0, 0.025, abs(rib_z - 0.12));
            let wave = exp(-pow(abs(p.z) - u.ring_travel, 2.0) * 0.5);
            emissive = accent * on_rib * (0.25 + wave * u.ring_pulse * 2.0);
        }

        let lit = albedo * (0.06 + diffuse * 0.9) * ao + albedo * rim * 0.15;
        colour = lit + emissive;
        colour = apply_fog(colour, hit.x);
        depth = depth_of(p);
    }

    colour += accent * core_glow * u.glow * 0.6;
    colour += vec3f(1.0, 0.96, 0.9) * u.flash * 0.6;
    return SceneOut(vec4f(colour, 1.0), depth);
}
