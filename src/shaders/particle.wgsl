// Billboard particle shader for Hopalong Orbits Visualizer.
//
// Renders instanced quads that always face the camera, textured with a sprite,
// using exponential fog and additive blending.

struct Uniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    sprite_size: f32,
    fog_density: f32,
    // padding to 16-byte alignment
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};

struct VertexInput {
    @location(0) quad_pos: vec2<f32>,   // quad corner [-0.5 .. 0.5]
};

struct InstanceInput {
    @location(1) world_pos: vec3<f32>,  // particle world position
    @location(2) color: vec4<f32>,      // particle RGBA color
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

@vertex
fn vs_main(vert: VertexInput, inst: InstanceInput) -> VertexOutput {
    var out: VertexOutput;

    // Transform world position to clip space.
    let clip_center = uniforms.view_proj * vec4<f32>(inst.world_pos, 1.0);

    // Billboard: offset in clip space (scale by w to keep screen-space size consistent).
    let offset = vec2<f32>(vert.quad_pos.x, vert.quad_pos.y) * uniforms.sprite_size * clip_center.w * 0.01;
    out.clip_pos = vec4<f32>(clip_center.xy + offset, clip_center.z, clip_center.w);

    // UV for texture sampling.
    out.uv = vert.quad_pos + vec2<f32>(0.5, 0.5);

    out.color = inst.color;

    // Exponential fog: fogFactor = exp(-density * distance).
    let dist = length(inst.world_pos - uniforms.camera_pos);
    out.fog_factor = exp(-uniforms.fog_density * dist);

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(sprite_texture, sprite_sampler, in.uv);
    // Brightness boost (2x) to compensate for additive blending accumulation
    // being less dense than the Three.js version at comparable particle counts.
    let particle_color = tex_color * in.color * in.fog_factor * 2.0;
    return particle_color;
}
