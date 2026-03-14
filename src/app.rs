use std::sync::Arc;
use std::time::Instant;

use eframe::egui;

use crate::renderer::{self, HopalongPaintCallback, HopalongRendererResources};
use crate::sim::HopalongSim;

pub struct HopalongApp {
    sim: HopalongSim,
    show_settings: bool,
    last_frame: Instant,
    frame_times: Vec<f32>,
    show_fps: bool,
    should_quit: bool,
}

impl HopalongApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let sim = HopalongSim::new();

        // Initialise wgpu renderer resources and store in CallbackResources.
        let render_state = cc
            .wgpu_render_state
            .as_ref()
            .expect("wgpu backend required");
        let resources = HopalongRendererResources::new(render_state, &sim);
        render_state
            .renderer
            .write()
            .callback_resources
            .insert(resources);

        Self {
            sim,
            show_settings: false,
            last_frame: Instant::now(),
            frame_times: Vec::with_capacity(120),
            show_fps: true,
            should_quit: false,
        }
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        ctx.input(|input| {
            // ── Mouse position (offset from screen centre) ──
            if let Some(pos) = input.pointer.latest_pos() {
                let screen = input.viewport_rect();
                let center_x = (screen.min.x + screen.max.x) / 2.0;
                let center_y = (screen.min.y + screen.max.y) / 2.0;
                self.sim.mouse_x = pos.x - center_x;
                self.sim.mouse_y = pos.y - center_y;
            }

            // ── Keyboard shortcuts (minimal set) ──
            for event in &input.events {
                if let egui::Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                    ..
                } = event
                {
                    match key {
                        // Speed
                        egui::Key::ArrowUp => {
                            self.sim.settings.speed += 0.25;
                        }
                        egui::Key::ArrowDown => {
                            self.sim.settings.speed = (self.sim.settings.speed - 0.25).max(0.0);
                        }
                        // Rotation
                        egui::Key::ArrowLeft => {
                            self.sim.settings.rotation_speed += 0.0005;
                        }
                        egui::Key::ArrowRight => {
                            self.sim.settings.rotation_speed -= 0.0005;
                        }
                        // Reset
                        egui::Key::R => {
                            self.sim.reset_defaults();
                        }
                        // Mouse lock
                        egui::Key::L => {
                            self.sim.settings.mouse_locked = !self.sim.settings.mouse_locked;
                        }
                        // Centre camera
                        egui::Key::C => {
                            self.sim.center_camera();
                        }
                        // Toggle settings panel
                        egui::Key::Tab => {
                            self.show_settings = !self.show_settings;
                        }
                        // Quit
                        egui::Key::Q => {
                            self.should_quit = true;
                        }
                        _ => {}
                    }
                }
            }
        });

        // Handle quit outside the input closure.
        if self.should_quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn ui_settings(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
        }

        egui::SidePanel::left("settings_panel")
            .resizable(false)
            .default_width(260.0)
            .show(ctx, |ui| {
                ui.heading("Settings");
                ui.separator();

                ui.label("Speed");
                ui.add(egui::Slider::new(&mut self.sim.settings.speed, 0.0..=50.0).step_by(0.25));

                ui.label("Rotation Speed");
                let mut rot_display = self.sim.settings.rotation_speed * -2000.0;
                if ui
                    .add(egui::Slider::new(&mut rot_display, -100.0..=100.0).step_by(1.0))
                    .changed()
                {
                    self.sim.settings.rotation_speed = rot_display / -2000.0;
                }

                ui.label("Camera FOV");
                let mut fov = self.sim.settings.camera_fov;
                if ui
                    .add(egui::Slider::new(&mut fov, 10.0..=120.0).step_by(1.0))
                    .changed()
                {
                    self.sim.settings.camera_fov = fov;
                }

                ui.label("Points per Subset (thousands)");
                let mut pts_k = self.sim.settings.points_per_subset as f32 / 1000.0;
                if ui
                    .add(egui::Slider::new(&mut pts_k, 1.0..=50.0).step_by(1.0))
                    .changed()
                {
                    let new_pts = (pts_k * 1000.0) as usize;
                    if new_pts != self.sim.settings.points_per_subset {
                        self.sim.settings.points_per_subset = new_pts;
                        self.sim.full_rebuild();
                    }
                }

                ui.label("Subset Count");
                let mut subsets = self.sim.settings.subset_count as f32;
                if ui
                    .add(egui::Slider::new(&mut subsets, 1.0..=10.0).step_by(1.0))
                    .changed()
                {
                    let new_sub = subsets as usize;
                    if new_sub != self.sim.settings.subset_count {
                        self.sim.settings.subset_count = new_sub;
                        self.sim.full_rebuild();
                    }
                }

                ui.label("Level Count");
                let mut levels = self.sim.settings.level_count as f32;
                if ui
                    .add(egui::Slider::new(&mut levels, 1.0..=10.0).step_by(1.0))
                    .changed()
                {
                    let new_lev = levels as usize;
                    if new_lev != self.sim.settings.level_count {
                        self.sim.settings.level_count = new_lev;
                        self.sim.full_rebuild();
                    }
                }

                ui.separator();

                ui.checkbox(&mut self.sim.settings.mouse_locked, "Mouse Locked (L)");

                // Center checkbox: centres camera and enables lock.
                let is_centered = self.sim.settings.mouse_locked
                    && self.sim.camera_x.abs() < 1.0
                    && self.sim.camera_y.abs() < 1.0;
                let mut center_checked = is_centered;
                if ui.checkbox(&mut center_checked, "Center (C)").changed() {
                    if center_checked {
                        // Centre and lock.
                        self.sim.camera_x = 0.0;
                        self.sim.camera_y = 0.0;
                        self.sim.mouse_x = 0.0;
                        self.sim.mouse_y = 0.0;
                        self.sim.settings.mouse_locked = true;
                    } else {
                        // Unlock.
                        self.sim.settings.mouse_locked = false;
                    }
                }

                ui.separator();

                if ui.button("Reset Defaults (R)").clicked() {
                    self.sim.reset_defaults();
                }

                if ui.button("Quit (Q)").clicked() {
                    self.should_quit = true;
                }

                ui.separator();
                ui.checkbox(&mut self.show_fps, "Show FPS");

                ui.separator();
                ui.label(format!("Total particles: {}", self.sim.total_particles()));

                ui.separator();
                ui.label(egui::RichText::new("Tab: toggle panel").size(13.0));
                ui.label(egui::RichText::new("Arrows: speed / rotation").size(13.0));
                ui.label(egui::RichText::new("L: lock mouse").size(13.0));
                ui.label(egui::RichText::new("C: centre + lock").size(13.0));
                ui.label(egui::RichText::new("R: reset defaults").size(13.0));
                ui.label(egui::RichText::new("Q: quit").size(13.0));
            });
    }

    fn ui_fps_overlay(&self, ctx: &egui::Context) {
        if !self.show_fps || self.frame_times.is_empty() {
            return;
        }

        let avg_dt: f32 = self.frame_times.iter().sum::<f32>() / self.frame_times.len() as f32;
        let fps = if avg_dt > 0.0 { 1.0 / avg_dt } else { 0.0 };

        egui::Area::new(egui::Id::new("fps_overlay"))
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-8.0, 8.0))
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(format!("{:.0}", fps))
                        .color(egui::Color32::from_rgb(0, 255, 0))
                        .size(14.0)
                        .background_color(egui::Color32::from_black_alpha(160)),
                );
            });
    }
}

impl eframe::App for HopalongApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // Black background.
        [0.0, 0.0, 0.0, 1.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Continuous repaint for animation.
        ctx.request_repaint();

        // Set slider handle shape globally.
        ctx.style_mut(|style| {
            style.visuals.handle_shape = egui::style::HandleShape::Rect { aspect_ratio: 0.5 };
        });

        // ── Delta time ──
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32();
        self.last_frame = now;

        // Track frame times for FPS counter (rolling window of 60 frames).
        self.frame_times.push(dt);
        if self.frame_times.len() > 60 {
            self.frame_times.remove(0);
        }

        // ── Input ──
        self.handle_input(ctx);

        // ── Update simulation ──
        self.sim.update(dt);

        // ── UI overlays ──
        self.ui_settings(ctx);
        self.ui_fps_overlay(ctx);

        // ── Central panel: custom wgpu rendering ──
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::BLACK))
            .show(ctx, |ui| {
                let available = ui.available_size();
                let (rect, _response) =
                    ui.allocate_exact_size(available, egui::Sense::click_and_drag());

                let aspect = if rect.height() > 0.0 {
                    rect.width() / rect.height()
                } else {
                    1.0
                };

                // Build rendering data.
                let uniforms = renderer::build_uniforms(&self.sim, aspect);
                let instances = Arc::new(renderer::build_instances(&self.sim));

                let callback = egui_wgpu::Callback::new_paint_callback(
                    rect,
                    HopalongPaintCallback {
                        uniforms,
                        instances,
                    },
                );
                ui.painter().add(callback);
            });
    }
}
