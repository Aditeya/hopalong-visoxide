use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use crate::sim::{HopalongSim, FOG_DENSITY, SCALE_FACTOR, SPRITE_SIZE};

// ── Vertex Types ───────────────────────────────────────────────────────────────

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct QuadVertex {
    pub position: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct ParticleInstance {
    pub world_pos: [f32; 3],
    pub _pad: f32, // align to 16 bytes
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct Uniforms {
    pub view_proj: [f32; 16],
    pub camera_pos: [f32; 3],
    pub sprite_size: f32,
    pub fog_density: f32,
    pub _pad0: f32,
    pub _pad1: f32,
    pub _pad2: f32,
}

// ── Quad Geometry ──────────────────────────────────────────────────────────────

const QUAD_VERTICES: &[QuadVertex] = &[
    QuadVertex {
        position: [-0.5, -0.5],
    },
    QuadVertex {
        position: [0.5, -0.5],
    },
    QuadVertex {
        position: [-0.5, 0.5],
    },
    QuadVertex {
        position: [0.5, 0.5],
    },
];

const QUAD_INDICES: &[u16] = &[0, 1, 2, 2, 1, 3];

// ── Renderer Resources ────────────────────────────────────────────────────────

/// Stored in `egui_wgpu::CallbackResources` so the paint callback can access them.
pub struct HopalongRendererResources {
    pub pipeline: wgpu::RenderPipeline,
    pub bind_group: wgpu::BindGroup,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub uniform_buffer: wgpu::Buffer,
    pub instance_buffer: wgpu::Buffer,
    pub instance_count: u32,
    pub max_instances: usize,
}

impl HopalongRendererResources {
    pub fn new(render_state: &egui_wgpu::RenderState, sim: &HopalongSim) -> Self {
        let device = &render_state.device;
        let queue = &render_state.queue;

        // ── Load galaxy sprite texture ──
        let sprite_bytes = include_bytes!("../assets/galaxy.png");
        let mut sprite_image = image::load_from_memory(sprite_bytes)
            .expect("Failed to load galaxy.png")
            .to_rgba8();
        // The galaxy.png has no alpha channel (RGB only). Generate alpha from
        // luminance so the dark edges become transparent — this is how Three.js
        // PointsMaterial effectively treats the sprite map with additive blending.
        for pixel in sprite_image.pixels_mut() {
            let lum = pixel[0].max(pixel[1]).max(pixel[2]);
            pixel[3] = lum;
        }
        let (tex_w, tex_h) = sprite_image.dimensions();

        let mip_level_count = (tex_w.max(tex_h).ilog2() + 1).min(8) as u32;
        let texture_size = wgpu::Extent3d {
            width: tex_w,
            height: tex_h,
            depth_or_array_layers: 1,
        };
        let sprite_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("galaxy_sprite"),
            size: texture_size,
            mip_level_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let mut mip_width = tex_w;
        let mut mip_height = tex_h;
        let mut mip_data = sprite_image.clone().into_raw();

        for mip in 0..mip_level_count {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &sprite_texture,
                    mip_level: mip,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &mip_data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * mip_width),
                    rows_per_image: Some(mip_height),
                },
                wgpu::Extent3d {
                    width: mip_width,
                    height: mip_height,
                    depth_or_array_layers: 1,
                },
            );

            if mip + 1 < mip_level_count {
                let next_width = (mip_width + 1) / 2;
                let next_height = (mip_height + 1) / 2;
                let mut next_data = Vec::with_capacity((next_width * next_height * 4) as usize);

                let src_pixels = &mip_data;
                for y in 0..next_height {
                    for x in 0..next_width {
                        let sx = x * 2;
                        let sy = y * 2;
                        let i00 = ((sy * mip_width) + sx) as usize * 4;
                        let i10 = ((sy * mip_width) + (sx + 1).min(mip_width)) as usize * 4;
                        let i01 = (((sy + 1).min(mip_height) * mip_width) + sx) as usize * 4;
                        let i11 = (((sy + 1).min(mip_height) * mip_width) + (sx + 1).min(mip_width))
                            as usize
                            * 4;

                        for c in 0..4 {
                            let val = (src_pixels[i00 + c] as u32
                                + src_pixels[i10 + c] as u32
                                + src_pixels[i01 + c] as u32
                                + src_pixels[i11 + c] as u32)
                                / 4;
                            next_data.push(val as u8);
                        }
                    }
                }

                mip_data = next_data;
                mip_width = next_width;
                mip_height = next_height;
            }
        }

        let texture_view = sprite_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sprite_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // ── Uniform buffer ──
        let uniforms = Uniforms {
            view_proj: Mat4::IDENTITY.to_cols_array(),
            camera_pos: [0.0, 0.0, SCALE_FACTOR / 2.0],
            sprite_size: SPRITE_SIZE,
            fog_density: FOG_DENSITY,
            _pad0: 0.0,
            _pad1: 0.0,
            _pad2: 0.0,
        };
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("uniforms"),
            contents: bytemuck::bytes_of(&uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // ── Bind group layout ──
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("particle_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("particle_bind_group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // ── Quad vertex / index buffers ──
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("quad_vertex"),
            contents: bytemuck::cast_slice(QUAD_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("quad_index"),
            contents: bytemuck::cast_slice(QUAD_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        // ── Instance buffer (pre-allocate for max particles) ──
        let max_instances = sim.total_particles();
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("instance_buffer"),
            size: (max_instances * std::mem::size_of::<ParticleInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // ── Shader ──
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("particle_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/particle.wgsl").into()),
        });

        // ── Pipeline layout ──
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("particle_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // ── Render pipeline ──
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("particle_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[
                    // Vertex buffer (quad corners)
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<QuadVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        }],
                    },
                    // Instance buffer (per-particle data)
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<ParticleInstance>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &[
                            // world_pos: vec3
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x3,
                                offset: 0,
                                shader_location: 1,
                            },
                            // color: vec4 (offset past world_pos + padding = 16 bytes)
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x4,
                                offset: 16,
                                shader_location: 2,
                            },
                        ],
                    },
                ],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // no culling for billboards
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None, // no depth testing (additive blending)
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: render_state.target_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::One, // additive
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            bind_group,
            vertex_buffer,
            index_buffer,
            uniform_buffer,
            instance_buffer,
            instance_count: 0,
            max_instances,
        }
    }
}

// ── Build Instance Data ────────────────────────────────────────────────────────

/// Build particle instances into a pre-allocated buffer.
///
/// This is more efficient than `build_instances` when the buffer can be reused
/// across frames, avoiding per-frame allocation.
#[inline]
pub fn build_instances_into(sim: &HopalongSim, buffer: &mut Vec<ParticleInstance>) {
    buffer.clear();
    let num_points = sim.settings.points_per_subset;
    buffer.reserve(sim.total_particles());

    let cam_z = sim.camera_z;
    let far_clip = 3.0 * SCALE_FACTOR;
    let fog_cull_dist = -(0.01f32).ln() / FOG_DENSITY;

    for ps in &sim.particle_sets {
        let dz = cam_z - ps.z_position;

        // Cull sets entirely behind the camera or beyond the far clip / fog threshold.
        if dz < 0.0 || dz > far_clip.max(fog_cull_dist) {
            continue;
        }

        let color = ps.cached_color;

        let (sin_r, cos_r) = ps.z_rotation.sin_cos();

        for point in ps.points.iter().take(num_points) {
            let rx = point[0] * cos_r - point[1] * sin_r;
            let ry = point[0] * sin_r + point[1] * cos_r;

            buffer.push(ParticleInstance {
                world_pos: [rx, ry, ps.z_position],
                _pad: 0.0,
                color,
            });
        }
    }
}

/// Build the view-projection uniform data.
#[inline]
pub fn build_uniforms(sim: &HopalongSim, aspect: f32) -> Uniforms {
    let cam_pos = Vec3::new(sim.camera_x, sim.camera_y, sim.camera_z);
    let view = Mat4::look_at_rh(cam_pos, Vec3::ZERO, Vec3::Y);
    let proj = Mat4::perspective_rh(
        sim.settings.camera_fov.to_radians(),
        aspect,
        1.0,
        3.0 * SCALE_FACTOR,
    );
    let view_proj = proj * view;

    Uniforms {
        view_proj: view_proj.to_cols_array(),
        camera_pos: cam_pos.to_array(),
        sprite_size: SPRITE_SIZE,
        fog_density: FOG_DENSITY,
        _pad0: 0.0,
        _pad1: 0.0,
        _pad2: 0.0,
    }
}

// ── Paint Callback ─────────────────────────────────────────────────────────────

/// Data passed to the paint callback each frame.
pub struct HopalongPaintCallback {
    pub uniforms: Uniforms,
    pub instances: Arc<Vec<ParticleInstance>>,
}

impl egui_wgpu::CallbackTrait for HopalongPaintCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let res: &mut HopalongRendererResources = resources.get_mut().unwrap();

        // Update uniforms.
        queue.write_buffer(&res.uniform_buffer, 0, bytemuck::bytes_of(&self.uniforms));

        // Update instance buffer — reallocate if needed.
        let instance_count = self.instances.len();
        if instance_count > res.max_instances {
            res.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("instance_buffer_resized"),
                size: (instance_count * std::mem::size_of::<ParticleInstance>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            res.max_instances = instance_count;
        }

        if instance_count > 0 {
            queue.write_buffer(
                &res.instance_buffer,
                0,
                bytemuck::cast_slice(&self.instances),
            );
        }
        res.instance_count = instance_count as u32;

        Vec::new()
    }

    fn paint(
        &self,
        info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        let res: &HopalongRendererResources = resources.get().unwrap();

        if res.instance_count == 0 {
            return;
        }

        let viewport = info.viewport_in_pixels();
        render_pass.set_viewport(
            viewport.left_px as f32,
            viewport.top_px as f32,
            viewport.width_px as f32,
            viewport.height_px as f32,
            0.0,
            1.0,
        );

        let clip = info.clip_rect_in_pixels();
        render_pass.set_scissor_rect(
            clip.left_px as u32,
            clip.top_px as u32,
            clip.width_px as u32,
            clip.height_px as u32,
        );

        render_pass.set_pipeline(&res.pipeline);
        render_pass.set_bind_group(0, &res.bind_group, &[]);
        render_pass.set_vertex_buffer(0, res.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, res.instance_buffer.slice(..));
        render_pass.set_index_buffer(res.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..6, 0, 0..res.instance_count);
    }
}
