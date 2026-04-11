use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use crate::sim::{HopalongSim, SetMetadata, FOG_DENSITY, SCALE_FACTOR, SPRITE_SIZE};

// ── Vertex Types ───────────────────────────────────────────────────────────────

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct QuadVertex {
    pub position: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct Uniforms {
    pub view_proj: [f32; 16],
    pub camera_pos: [f32; 3],
    pub sprite_size: f32,
    pub fog_density: f32,
    pub points_per_subplot: u32,
    pub total_sets: u32,
    pub _pad2: u32,
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
    pub bind_group_0: wgpu::BindGroup,
    pub bind_group_1: wgpu::BindGroup,
    pub bind_group_layout_1: wgpu::BindGroupLayout,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub uniform_buffer: wgpu::Buffer,
    pub orbit_buffer: wgpu::Buffer,
    pub set_metadata_buffer: wgpu::Buffer,
    pub instance_count: u32,
    pub orbit_version: u64,
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
        for pixel in sprite_image.pixels_mut() {
            let lum = pixel[0].max(pixel[1]).max(pixel[2]);
            pixel[3] = lum;
        }
        let (tex_w, tex_h) = sprite_image.dimensions();

        let texture_size = wgpu::Extent3d {
            width: tex_w,
            height: tex_h,
            depth_or_array_layers: 1,
        };
        let sprite_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("galaxy_sprite"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &sprite_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &sprite_image,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * tex_w),
                rows_per_image: Some(tex_h),
            },
            texture_size,
        );

        let texture_view = sprite_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sprite_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // ── Uniform buffer ──
        let uniforms = Uniforms {
            view_proj: Mat4::IDENTITY.to_cols_array(),
            camera_pos: [0.0, 0.0, SCALE_FACTOR / 2.0],
            sprite_size: SPRITE_SIZE,
            fog_density: FOG_DENSITY,
            points_per_subplot: sim.settings.points_per_subset as u32,
            total_sets: sim.particle_sets.len() as u32,
            _pad2: 0,
        };
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("uniforms"),
            contents: bytemuck::bytes_of(&uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // ── Storage buffers ──
        let orbit_data = sim.build_orbit_data();
        let set_metadata = sim.build_set_metadata();

        let orbit_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("orbit_points"),
            contents: bytemuck::cast_slice(&orbit_data),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let set_metadata_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("set_metadata"),
            contents: bytemuck::cast_slice(&set_metadata),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        // ── Bind group layout 0: uniforms + texture + sampler ──
        let bind_group_layout_0 =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("particle_bind_group_layout_0"),
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

        let bind_group_0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("particle_bind_group_0"),
            layout: &bind_group_layout_0,
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

        // ── Bind group layout 1: orbit points + set metadata (storage buffers) ──
        let bind_group_layout_1 =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("particle_bind_group_layout_1"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let bind_group_1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("particle_bind_group_1"),
            layout: &bind_group_layout_1,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: orbit_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: set_metadata_buffer.as_entire_binding(),
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

        // ── Shader ──
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("particle_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/particle.wgsl").into()),
        });

        // ── Pipeline layout ──
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("particle_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout_0, &bind_group_layout_1],
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
                    // Quad vertex buffer (only buffer — instance data comes from storage buffers)
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<QuadVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        }],
                    },
                ],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
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
                            dst_factor: wgpu::BlendFactor::One,
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
            bind_group_0,
            bind_group_1,
            bind_group_layout_1,
            vertex_buffer,
            index_buffer,
            uniform_buffer,
            orbit_buffer,
            set_metadata_buffer,
            instance_count: sim.total_particles() as u32,
            orbit_version: sim.orbit_version,
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
        points_per_subplot: sim.settings.points_per_subset as u32,
        total_sets: sim.particle_sets.len() as u32,
        _pad2: 0,
    }
}

// ── Paint Callback ─────────────────────────────────────────────────────────────

/// Data passed to the paint callback each frame.
pub struct HopalongPaintCallback {
    pub uniforms: Uniforms,
    pub set_metadata: Vec<SetMetadata>,
    pub orbit_data: Vec<[f32; 2]>,
    pub orbit_version: u64,
    pub instance_count: u32,
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

        // Update set metadata (every frame, ~1.5KB).
        queue.write_buffer(
            &res.set_metadata_buffer,
            0,
            bytemuck::cast_slice(&self.set_metadata),
        );

        // Update orbit data only when version changes (~224KB every 3s).
        if self.orbit_version != res.orbit_version {
            // Resize orbit buffer if needed.
            let needed = (self.orbit_data.len() * std::mem::size_of::<[f32; 2]>()) as u64;
            if needed > res.orbit_buffer.size() {
                res.orbit_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("orbit_buffer_resized"),
                    size: needed,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                // Re-create bind group 1 with the new orbit buffer.
                res.bind_group_1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("particle_bind_group_1_updated"),
                    layout: &res.bind_group_layout_1,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: res.orbit_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: res.set_metadata_buffer.as_entire_binding(),
                        },
                    ],
                });
            }
            queue.write_buffer(&res.orbit_buffer, 0, bytemuck::cast_slice(&self.orbit_data));
            res.orbit_version = self.orbit_version;
        }

        res.instance_count = self.instance_count;

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
        render_pass.set_bind_group(0, &res.bind_group_0, &[]);
        render_pass.set_bind_group(1, &res.bind_group_1, &[]);
        render_pass.set_vertex_buffer(0, res.vertex_buffer.slice(..));
        render_pass.set_index_buffer(res.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..6, 0, 0..res.instance_count);
    }
}
