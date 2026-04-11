// GPU-driven billboard particle shader for Hopalong Orbits Visualizer.
//
// Particles are drawn as instanced quads. The vertex shader reads orbit
// points and per-set metadata from storage buffers, deriving everything
// from instance_index — no per-instance vertex buffer needed.

struct Uniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    sprite_size: f32,
    fog_density: f32,
    points_per_subplot: u32,
    total_sets: u32,
    _pad2: u32,
};

struct SetMetadata {
    z_position: f32,
    sin_rotation: f32,
    cos_rotation: f32,
    subset_index: u32,
    color: vec4<f32>,
};

struct VertexInput {
    @location(0) quad_pos: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) fog_factor: f32,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var sprite_texture: texture_2d<f32>;
@group(0) @binding(2) var sprite_sampler: sampler;

@group(1) @binding(0) var<storage, read> orbit_points: array<vec2<f32>>;
@group(1) @binding(1) var<storage, read> set_metadata: array<SetMetadata>;

@vertex
fn vs_main(
    vert: VertexInput,
    @builtin(instance_index) instance_index: u32,
) -> VertexOutput {
    var out: VertexOutput;

    // Derive set and point indices from instance_index
    let set_idx = instance_index / uniforms.points_per_subplot;
    let point_idx = instance_index % uniforms.points_per_subplot;
    let meta = set_metadata[set_idx];

    // Look up orbit point from storage buffer
    let orbit_index = meta.subset_index * uniforms.points_per_subplot + point_idx;
    let point = orbit_points[orbit_index];

    // Apply Z-axis rotation (sin/cos pre-computed in metadata)
    let rx = point.x * meta.cos_rotation - point.y * meta.sin_rotation;
    let ry = point.x * meta.sin_rotation + point.y * meta.cos_rotation;
    let world_pos = vec3<f32>(rx, ry, meta.z_position);

    // Transform to clip space
    let clip_center = uniforms.view_proj * vec4<f32>(world_pos, 1.0);

    // Billboard offset in clip space (scale by w for consistent screen-space size)
    let offset = vec2<f32>(vert.quad_pos.x, vert.quad_pos.y)
                 * uniforms.sprite_size * clip_center.w * 0.004;
    out.clip_pos = vec4<f32>(clip_center.xy + offset, clip_center.z, clip_center.w);

    // UV for texture sampling
    out.uv = vert.quad_pos + vec2<f32>(0.5, 0.5);

    out.color = meta.color;

    // Exponential fog
    let dist = length(world_pos - uniforms.camera_pos);
    out.fog_factor = exp(-uniforms.fog_density * dist);

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(sprite_texture, sprite_sampler, in.uv);
    let particle_color = tex_color * in.color * in.fog_factor * 1.5;
    return particle_color;
}