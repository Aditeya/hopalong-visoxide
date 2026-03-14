use rand::Rng;

// ── Constants ──────────────────────────────────────────────────────────────────

pub const SCALE_FACTOR: f32 = 1500.0;
pub const CAMERA_BOUND: f32 = 200.0;
pub const LEVEL_DEPTH: f32 = 600.0;
pub const DEF_BRIGHTNESS: f32 = 1.0;
pub const DEF_SATURATION: f32 = 0.8;
pub const SPRITE_SIZE: f32 = 5.0;
pub const FOG_DENSITY: f32 = 0.001;

const A_RANGE: (f32, f32) = (-30.0, 30.0);
const B_RANGE: (f32, f32) = (0.2, 1.8);
const C_RANGE: (f32, f32) = (5.0, 17.0);
const D_RANGE: (f32, f32) = (0.0, 10.0);
const E_RANGE: (f32, f32) = (0.0, 12.0);

pub const DEFAULT_SPEED: f32 = 8.0;
pub const DEFAULT_ROTATION_SPEED: f32 = 0.005;
pub const DEFAULT_FOV: f32 = 60.0;
pub const DEFAULT_POINTS_SUBSET: usize = 4000;
pub const DEFAULT_SUBSETS: usize = 7;
pub const DEFAULT_LEVELS: usize = 7;

const ORBIT_REGEN_INTERVAL: f32 = 3.0; // seconds
const CAMERA_LERP_FACTOR: f32 = 0.05;

// ── Data Structures ────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct OrbitParams {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
}

#[derive(Clone, Debug)]
pub struct SimSettings {
    pub speed: f32,
    pub rotation_speed: f32,
    pub camera_fov: f32,
    pub points_per_subset: usize,
    pub subset_count: usize,
    pub level_count: usize,
    pub mouse_locked: bool,
}

impl Default for SimSettings {
    fn default() -> Self {
        Self {
            speed: DEFAULT_SPEED,
            rotation_speed: DEFAULT_ROTATION_SPEED,
            camera_fov: DEFAULT_FOV,
            points_per_subset: DEFAULT_POINTS_SUBSET,
            subset_count: DEFAULT_SUBSETS,
            level_count: DEFAULT_LEVELS,
            mouse_locked: false,
        }
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct ParticleSetState {
    pub z_position: f32,
    pub z_rotation: f32,
    pub needs_update: bool,
    /// Which subset index this particle set uses (for orbit data on update).
    pub subset_index: usize,
    /// Which level index this particle set belongs to.
    pub level_index: usize,
    /// Baked copy of the 2D orbit points this set is currently rendering.
    /// Only updated when the set wraps around past the camera and
    /// `needs_update` is true — this creates the gradual transition.
    pub points: Vec<[f32; 2]>,
    /// Baked hue value for this set's current color.
    pub hue: f32,
}

// ── Simulation State ───────────────────────────────────────────────────────────

pub struct HopalongSim {
    pub settings: SimSettings,
    pub orbit_params: OrbitParams,
    /// Latest 2D orbit data (staging area for next transition).
    pub orbit_subsets: Vec<Vec<[f32; 2]>>,
    /// Latest hue values (staging area for next transition).
    pub hue_values: Vec<f32>,
    /// State for each particle set (num_levels * num_subsets total).
    pub particle_sets: Vec<ParticleSetState>,
    /// Camera position.
    pub camera_x: f32,
    pub camera_y: f32,
    pub camera_z: f32,
    /// Mouse target (offset from screen centre).
    pub mouse_x: f32,
    pub mouse_y: f32,
    /// Timer for orbit regeneration.
    regen_timer: f32,
    /// Whether the simulation needs a full rebuild (settings changed structurally).
    pub needs_rebuild: bool,
    /// Whether instance buffer data needs re-uploading.
    pub instances_dirty: bool,
}

impl HopalongSim {
    pub fn new() -> Self {
        let settings = SimSettings::default();
        let mut sim = Self {
            orbit_params: OrbitParams {
                a: 0.0,
                b: 0.0,
                c: 0.0,
                d: 0.0,
                e: 0.0,
            },
            orbit_subsets: Vec::new(),
            hue_values: Vec::new(),
            particle_sets: Vec::new(),
            camera_x: 0.0,
            camera_y: 0.0,
            camera_z: SCALE_FACTOR / 2.0,
            mouse_x: 0.0,
            mouse_y: 0.0,
            regen_timer: 0.0,
            needs_rebuild: false,
            instances_dirty: true,
            settings,
        };
        sim.full_rebuild();
        sim
    }

    /// Complete rebuild: regenerate orbit, hues, and particle sets from scratch.
    pub fn full_rebuild(&mut self) {
        self.shuffle_params();
        self.orbit_subsets = generate_orbit(
            &self.orbit_params,
            self.settings.subset_count,
            self.settings.points_per_subset,
        );
        self.hue_values = generate_hues(self.settings.subset_count);
        self.init_particle_sets();
        self.regen_timer = 0.0;
        self.needs_rebuild = false;
        self.instances_dirty = true;
    }

    fn shuffle_params(&mut self) {
        let mut rng = rand::rng();
        self.orbit_params = OrbitParams {
            a: rng.random_range(A_RANGE.0..=A_RANGE.1),
            b: rng.random_range(B_RANGE.0..=B_RANGE.1),
            c: rng.random_range(C_RANGE.0..=C_RANGE.1),
            d: rng.random_range(D_RANGE.0..=D_RANGE.1),
            e: rng.random_range(E_RANGE.0..=E_RANGE.1),
        };
    }

    fn init_particle_sets(&mut self) {
        let num_levels = self.settings.level_count;
        let num_subsets = self.settings.subset_count;
        self.particle_sets.clear();

        for level in 0..num_levels {
            for subset in 0..num_subsets {
                let z = -(LEVEL_DEPTH * level as f32)
                    - (subset as f32 * LEVEL_DEPTH / num_subsets as f32)
                    + SCALE_FACTOR / 2.0;

                // Bake a copy of the orbit data for this set.
                let points = if subset < self.orbit_subsets.len() {
                    self.orbit_subsets[subset].clone()
                } else {
                    Vec::new()
                };
                let hue = if subset < self.hue_values.len() {
                    self.hue_values[subset]
                } else {
                    0.0
                };

                self.particle_sets.push(ParticleSetState {
                    z_position: z,
                    z_rotation: 0.0,
                    needs_update: false,
                    subset_index: subset,
                    level_index: level,
                    points,
                    hue,
                });
            }
        }
    }

    /// Called every frame with the elapsed dt.
    pub fn update(&mut self, dt: f32) {
        // ── Camera lerp toward mouse ──
        if !self.settings.mouse_locked {
            self.camera_x += (self.mouse_x - self.camera_x) * CAMERA_LERP_FACTOR;
            self.camera_y += (-self.mouse_y - self.camera_y) * CAMERA_LERP_FACTOR;
        }
        self.camera_x = self.camera_x.clamp(-CAMERA_BOUND, CAMERA_BOUND);
        self.camera_y = self.camera_y.clamp(-CAMERA_BOUND, CAMERA_BOUND);

        // ── Advance particle sets ──
        let speed = self.settings.speed;
        let rot = self.settings.rotation_speed;
        let cam_z = self.camera_z;
        let num_levels = self.settings.level_count;

        for ps in &mut self.particle_sets {
            ps.z_position += speed;
            ps.z_rotation += rot;

            // Wraparound: recycle behind camera.
            if ps.z_position > cam_z {
                ps.z_position = -((num_levels - 1) as f32) * LEVEL_DEPTH;

                // On wraparound, bake the latest orbit data into this set.
                // Sets that haven't wrapped yet keep their old baked data,
                // creating the gradual fly-through transition.
                if ps.needs_update {
                    let idx = ps.subset_index;
                    if idx < self.orbit_subsets.len() {
                        ps.points = self.orbit_subsets[idx].clone();
                    }
                    if idx < self.hue_values.len() {
                        ps.hue = self.hue_values[idx];
                    }
                    ps.needs_update = false;
                }
            }
        }

        self.instances_dirty = true;

        // ── Orbit regeneration timer ──
        self.regen_timer += dt;
        if self.regen_timer >= ORBIT_REGEN_INTERVAL {
            self.regen_timer = 0.0;
            self.regenerate_orbit();
        }
    }

    fn regenerate_orbit(&mut self) {
        self.shuffle_params();
        self.orbit_subsets = generate_orbit(
            &self.orbit_params,
            self.settings.subset_count,
            self.settings.points_per_subset,
        );
        self.hue_values = generate_hues(self.settings.subset_count);

        // Flag all particle sets for lazy update — each keeps rendering its
        // own baked data until it individually wraps past the camera.
        for ps in &mut self.particle_sets {
            ps.needs_update = true;
        }
    }

    /// Centre the camera and toggle mouse lock.
    pub fn center_camera(&mut self) {
        self.camera_x = 0.0;
        self.camera_y = 0.0;
        self.mouse_x = 0.0;
        self.mouse_y = 0.0;
        self.settings.mouse_locked = !self.settings.mouse_locked;
    }

    /// Reset all settings to defaults and trigger full rebuild.
    pub fn reset_defaults(&mut self) {
        self.settings = SimSettings::default();
        self.full_rebuild();
    }

    /// Total number of particles across all sets.
    pub fn total_particles(&self) -> usize {
        self.settings.level_count * self.settings.subset_count * self.settings.points_per_subset
    }
}

// ── Orbit Generation ───────────────────────────────────────────────────────────

fn generate_orbit(
    params: &OrbitParams,
    num_subsets: usize,
    num_points: usize,
) -> Vec<Vec<[f32; 2]>> {
    let mut rng = rand::rng();
    let choice: f32 = rng.random();

    let mut x_min = f32::MAX;
    let mut x_max = f32::MIN;
    let mut y_min = f32::MAX;
    let mut y_max = f32::MIN;

    let mut subsets = Vec::with_capacity(num_subsets);

    for s in 0..num_subsets {
        let mut x = s as f32 * 0.005 * (0.5 - rng.random::<f32>());
        let mut y = s as f32 * 0.005 * (0.5 - rng.random::<f32>());
        let mut points = Vec::with_capacity(num_points);

        for _ in 0..num_points {
            let z = if choice < 0.5 {
                params.d + (params.b * x - params.c).abs().sqrt()
            } else if choice < 0.75 {
                params.d + (params.b * x - params.c).abs().sqrt().sqrt()
            } else {
                params.d + (2.0 + (params.b * x - params.c).abs().sqrt()).ln()
            };

            let x1 = if x > 0.0 {
                y - z
            } else if x == 0.0 {
                y
            } else {
                y + z
            };
            y = params.a - x;
            x = x1 + params.e;

            points.push([x, y]);
            x_min = x_min.min(x);
            x_max = x_max.max(x);
            y_min = y_min.min(y);
            y_max = y_max.max(y);
        }
        subsets.push(points);
    }

    // Normalize to [-SCALE_FACTOR, +SCALE_FACTOR].
    let range_x = x_max - x_min;
    let range_y = y_max - y_min;
    if range_x > 0.0 && range_y > 0.0 {
        let scale_x = 2.0 * SCALE_FACTOR / range_x;
        let scale_y = 2.0 * SCALE_FACTOR / range_y;
        for subset in &mut subsets {
            for p in subset.iter_mut() {
                p[0] = scale_x * (p[0] - x_min) - SCALE_FACTOR;
                p[1] = scale_y * (p[1] - y_min) - SCALE_FACTOR;
            }
        }
    }

    subsets
}

// ── Color Utilities ────────────────────────────────────────────────────────────

fn generate_hues(num_subsets: usize) -> Vec<f32> {
    let mut rng = rand::rng();
    (0..num_subsets).map(|_| rng.random::<f32>()).collect()
}

/// Convert HSV (h in [0,1], s in [0,1], v in [0,1]) to RGBA [0,1].
pub fn hsv_to_rgba(h: f32, s: f32, v: f32) -> [f32; 4] {
    let c = v * s;
    let h6 = h * 6.0;
    let x = c * (1.0 - (h6 % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match h6 as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [r + m, g + m, b + m, 1.0]
}
