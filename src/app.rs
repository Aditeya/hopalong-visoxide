use std::sync::Arc;
use std::time::Instant;

use eframe::egui;

use crate::renderer::{self, HopalongPaintCallback, HopalongRendererResources};
use crate::sim::HopalongSim;

// ── Theme Colors ───────────────────────────────────────────────────────────────

/// Accent blue for interactive elements and section headers.
const ACCENT: egui::Color32 = egui::Color32::from_rgb(100, 120, 220);
const ACCENT_HOVER: egui::Color32 = egui::Color32::from_rgb(130, 150, 255);
const ACCENT_DIM: egui::Color32 = egui::Color32::from_rgb(70, 85, 160);

/// Section header color (brighter accent).
const SECTION_HEADER: egui::Color32 = egui::Color32::from_rgb(150, 170, 255);

/// Text hierarchy.
const TEXT_PRIMARY: egui::Color32 = egui::Color32::from_rgb(210, 210, 225);
const TEXT_SECONDARY: egui::Color32 = egui::Color32::from_rgb(130, 130, 155);

/// Panel background (semi-transparent deep dark blue).
const PANEL_BG: egui::Color32 = egui::Color32::from_rgba_premultiplied(10, 10, 20, 210);
const PANEL_STROKE: egui::Color32 = egui::Color32::from_rgba_premultiplied(70, 70, 130, 50);

/// Widget fills.
const WIDGET_BG: egui::Color32 = egui::Color32::from_rgba_premultiplied(30, 30, 50, 180);
const WIDGET_BG_HOVER: egui::Color32 = egui::Color32::from_rgba_premultiplied(45, 45, 75, 200);
const WIDGET_BG_ACTIVE: egui::Color32 = egui::Color32::from_rgba_premultiplied(60, 60, 105, 220);

/// Separator color.
const SEPARATOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(70, 70, 130, 40);

/// Button accents.
const RESET_COLOR: egui::Color32 = egui::Color32::from_rgb(210, 180, 60);
const QUIT_COLOR: egui::Color32 = egui::Color32::from_rgb(170, 65, 65);

/// FPS overlay.
const FPS_GREEN: egui::Color32 = egui::Color32::from_rgb(80, 240, 120);
const FPS_BG: egui::Color32 = egui::Color32::from_rgba_premultiplied(0, 0, 0, 160);

// ── App State ──────────────────────────────────────────────────────────────────

pub struct HopalongApp {
    sim: HopalongSim,
    show_settings: bool,
    last_frame: Instant,
    frame_times: Vec<f32>,
    show_fps: bool,
    should_quit: bool,
    theme_applied: bool,
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
            show_fps: false,
            should_quit: false,
            theme_applied: false,
        }
    }

    // ── Theme ──────────────────────────────────────────────────────────────────

    /// Apply the custom cosmic theme once at startup.
    fn apply_theme(&mut self, ctx: &egui::Context) {
        if self.theme_applied {
            return;
        }
        self.theme_applied = true;

        ctx.style_mut(|style| {
            let v = &mut style.visuals;

            // Base
            v.dark_mode = true;
            v.panel_fill = PANEL_BG;
            v.window_fill = PANEL_BG;
            v.extreme_bg_color = egui::Color32::from_rgb(8, 8, 16);
            v.faint_bg_color = egui::Color32::from_rgba_premultiplied(20, 20, 35, 100);

            // Selection accent
            v.selection.bg_fill = ACCENT;
            v.selection.stroke = egui::Stroke::new(1.0, ACCENT_HOVER);

            // ── Widget states ──

            // Inactive
            v.widgets.inactive.bg_fill = WIDGET_BG;
            v.widgets.inactive.weak_bg_fill = WIDGET_BG;
            v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, TEXT_PRIMARY);
            v.widgets.inactive.bg_stroke = egui::Stroke::new(0.5, PANEL_STROKE);
            v.widgets.inactive.corner_radius = egui::CornerRadius::same(4);

            // Hovered
            v.widgets.hovered.bg_fill = WIDGET_BG_HOVER;
            v.widgets.hovered.weak_bg_fill = WIDGET_BG_HOVER;
            v.widgets.hovered.fg_stroke = egui::Stroke::new(1.5, egui::Color32::WHITE);
            v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, ACCENT_DIM);
            v.widgets.hovered.corner_radius = egui::CornerRadius::same(4);

            // Active (pressed)
            v.widgets.active.bg_fill = WIDGET_BG_ACTIVE;
            v.widgets.active.weak_bg_fill = WIDGET_BG_ACTIVE;
            v.widgets.active.fg_stroke = egui::Stroke::new(2.0, egui::Color32::WHITE);
            v.widgets.active.bg_stroke = egui::Stroke::new(1.0, ACCENT);
            v.widgets.active.corner_radius = egui::CornerRadius::same(4);

            // Open (expanded collapsing headers)
            v.widgets.open.bg_fill = egui::Color32::from_rgba_premultiplied(35, 35, 60, 140);
            v.widgets.open.weak_bg_fill = egui::Color32::from_rgba_premultiplied(35, 35, 60, 140);
            v.widgets.open.fg_stroke = egui::Stroke::new(1.0, SECTION_HEADER);
            v.widgets.open.corner_radius = egui::CornerRadius::same(4);

            // Non-interactive (labels, separators)
            v.widgets.noninteractive.bg_fill = egui::Color32::TRANSPARENT;
            v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, TEXT_SECONDARY);
            v.widgets.noninteractive.bg_stroke = egui::Stroke::new(0.5, SEPARATOR);
            v.widgets.noninteractive.corner_radius = egui::CornerRadius::same(4);

            // Slider handle
            v.handle_shape = egui::style::HandleShape::Rect { aspect_ratio: 0.5 };

            // Window chrome
            v.window_corner_radius = egui::CornerRadius::same(8);
            v.window_stroke = egui::Stroke::new(1.0, PANEL_STROKE);

            // ── Spacing ──
            style.spacing.item_spacing = egui::vec2(8.0, 5.0);
            style.spacing.slider_width = 170.0;
        });
    }

    // ── Input ──────────────────────────────────────────────────────────────────

    fn handle_input(&mut self, ctx: &egui::Context) {
        ctx.input(|input| {
            // Mouse position (offset from screen centre).
            if let Some(pos) = input.pointer.latest_pos() {
                let screen = input.viewport_rect();
                let center_x = (screen.min.x + screen.max.x) / 2.0;
                let center_y = (screen.min.y + screen.max.y) / 2.0;
                self.sim.mouse_x = pos.x - center_x;
                self.sim.mouse_y = pos.y - center_y;
            }

            // Keyboard shortcuts.
            for event in &input.events {
                if let egui::Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                    ..
                } = event
                {
                    match key {
                        egui::Key::ArrowUp => {
                            self.sim.settings.speed += 0.25;
                        }
                        egui::Key::ArrowDown => {
                            self.sim.settings.speed = (self.sim.settings.speed - 0.25).max(0.0);
                        }
                        egui::Key::ArrowLeft => {
                            self.sim.settings.rotation_speed += 0.0005;
                        }
                        egui::Key::ArrowRight => {
                            self.sim.settings.rotation_speed -= 0.0005;
                        }
                        egui::Key::R => {
                            self.sim.reset_defaults();
                        }
                        egui::Key::L => {
                            self.sim.settings.mouse_locked = !self.sim.settings.mouse_locked;
                        }
                        egui::Key::C => {
                            self.sim.center_camera();
                        }
                        egui::Key::Tab => {
                            self.show_settings = !self.show_settings;
                        }
                        egui::Key::Q => {
                            self.should_quit = true;
                        }
                        _ => {}
                    }
                }
            }
        });

        if self.should_quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    // ── Settings Panel ─────────────────────────────────────────────────────────

    fn ui_settings(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
        }

        let viewport = ctx.input(|i| i.viewport_rect());
        let padding = 8.0;
        let frame_margin: i8 = 14;
        let content_width = 260.0;
        let panel_height = viewport.height() - padding * 2.0;

        let panel_frame = egui::Frame::default()
            .fill(PANEL_BG)
            .inner_margin(egui::Margin::same(frame_margin))
            .stroke(egui::Stroke::new(1.0, PANEL_STROKE))
            .corner_radius(egui::CornerRadius::same(8));

        egui::Area::new(egui::Id::new("settings_panel"))
            .fixed_pos(egui::pos2(padding, padding))
            .show(ctx, |ui| {
                panel_frame.show(ui, |ui| {
                    ui.set_width(content_width);
                    let inner_height = panel_height - (frame_margin as f32) * 2.0;
                    ui.set_min_height(inner_height);
                    ui.set_max_height(inner_height);

                    // ── Title (pinned above scroll) ──
                    ui.heading(
                        egui::RichText::new("Hopalong Orbits")
                            .color(egui::Color32::WHITE)
                            .size(18.0),
                    );
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(6.0);

                    // ── Scrollable body (fills remaining height) ──
                    let remaining = ui.available_height();
                    egui::ScrollArea::vertical()
                        .max_height(remaining)
                        .auto_shrink(false)
                        .show(ui, |ui| {
                            ui.set_width(content_width);

                            // ── Simulation ──
                            self.ui_section_simulation(ui);
                            ui.add_space(2.0);

                            // ── Camera ──
                            self.ui_section_camera(ui);
                            ui.add_space(2.0);

                            // ── Particles ──
                            self.ui_section_particles(ui);
                            ui.add_space(2.0);

                            // ── Info ──
                            self.ui_section_info(ui);

                            ui.add_space(6.0);
                            ui.separator();
                            ui.add_space(6.0);

                            // ── Action Buttons ──
                            self.ui_action_buttons(ui);

                            ui.add_space(6.0);

                            // ── Keyboard Shortcuts ──
                            self.ui_section_shortcuts(ui);
                        });
                });
            });
    }

    fn ui_section_simulation(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new(
            egui::RichText::new("Simulation")
                .color(SECTION_HEADER)
                .size(14.0),
        )
        .default_open(true)
        .show(ui, |ui| {
            ui.add_space(2.0);

            ui.label(egui::RichText::new("Speed").color(TEXT_PRIMARY).size(12.0));
            ui.add(egui::Slider::new(&mut self.sim.settings.speed, 0.0..=50.0).step_by(0.25))
                .on_hover_text("Arrow Up / Down");

            ui.add_space(4.0);

            ui.label(
                egui::RichText::new("Rotation Speed")
                    .color(TEXT_PRIMARY)
                    .size(12.0),
            );
            let mut rot_display = self.sim.settings.rotation_speed * -2000.0;
            if ui
                .add(egui::Slider::new(&mut rot_display, -100.0..=100.0).step_by(1.0))
                .on_hover_text("Arrow Left / Right")
                .changed()
            {
                self.sim.settings.rotation_speed = rot_display / -2000.0;
            }

            ui.add_space(2.0);
        });
    }

    fn ui_section_camera(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new(
            egui::RichText::new("Camera")
                .color(SECTION_HEADER)
                .size(14.0),
        )
        .default_open(true)
        .show(ui, |ui| {
            ui.add_space(2.0);

            ui.label(
                egui::RichText::new("Field of View")
                    .color(TEXT_PRIMARY)
                    .size(12.0),
            );
            let mut fov = self.sim.settings.camera_fov;
            if ui
                .add(
                    egui::Slider::new(&mut fov, 10.0..=120.0)
                        .step_by(1.0)
                        .suffix("\u{00b0}"),
                )
                .changed()
            {
                self.sim.settings.camera_fov = fov;
            }

            ui.add_space(4.0);

            ui.checkbox(&mut self.sim.settings.mouse_locked, "Mouse Locked")
                .on_hover_text("L to toggle");

            let is_centered = self.sim.settings.mouse_locked
                && self.sim.camera_x.abs() < 1.0
                && self.sim.camera_y.abs() < 1.0;
            let mut center_checked = is_centered;
            if ui
                .checkbox(&mut center_checked, "Center Camera")
                .on_hover_text("C to toggle")
                .changed()
            {
                if center_checked {
                    self.sim.camera_x = 0.0;
                    self.sim.camera_y = 0.0;
                    self.sim.mouse_x = 0.0;
                    self.sim.mouse_y = 0.0;
                    self.sim.settings.mouse_locked = true;
                } else {
                    self.sim.settings.mouse_locked = false;
                }
            }

            ui.add_space(2.0);
        });
    }

    fn ui_section_particles(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new(
            egui::RichText::new("Particles")
                .color(SECTION_HEADER)
                .size(14.0),
        )
        .default_open(false)
        .show(ui, |ui| {
            ui.add_space(2.0);

            ui.label(
                egui::RichText::new("Points per Subset (thousands)")
                    .color(TEXT_PRIMARY)
                    .size(12.0),
            );
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

            ui.add_space(4.0);

            ui.label(
                egui::RichText::new("Subset Count")
                    .color(TEXT_PRIMARY)
                    .size(12.0),
            );
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

            ui.add_space(4.0);

            ui.label(
                egui::RichText::new("Level Count")
                    .color(TEXT_PRIMARY)
                    .size(12.0),
            );
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

            ui.add_space(4.0);

            ui.label(
                egui::RichText::new(format!(
                    "Total: {}",
                    format_number(self.sim.total_particles())
                ))
                .color(TEXT_SECONDARY)
                .size(11.0),
            );

            ui.add_space(2.0);
        });
    }

    fn ui_section_info(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new(egui::RichText::new("Info").color(SECTION_HEADER).size(14.0))
            .default_open(false)
            .show(ui, |ui| {
                ui.add_space(2.0);

                ui.checkbox(&mut self.show_fps, "Show FPS Counter");

                ui.label(
                    egui::RichText::new(format!(
                        "Particles: {}",
                        format_number(self.sim.total_particles())
                    ))
                    .color(TEXT_SECONDARY)
                    .size(12.0),
                );

                ui.add_space(2.0);
            });
    }

    fn ui_action_buttons(&mut self, ui: &mut egui::Ui) {
        // Reset — amber accent, full width, taller target.
        let reset_btn = egui::Button::new(
            egui::RichText::new("Reset Defaults")
                .color(RESET_COLOR)
                .size(13.0),
        )
        .min_size(egui::vec2(ui.available_width(), 28.0));
        if ui.add(reset_btn).on_hover_text("R").clicked() {
            self.sim.reset_defaults();
        }

        ui.add_space(4.0);

        // Quit — subdued red, slightly smaller.
        let quit_btn = egui::Button::new(egui::RichText::new("Quit").color(QUIT_COLOR).size(12.0))
            .min_size(egui::vec2(ui.available_width(), 24.0));
        if ui.add(quit_btn).on_hover_text("Q").clicked() {
            self.should_quit = true;
        }
    }

    fn ui_section_shortcuts(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new(
            egui::RichText::new("Keyboard Shortcuts")
                .color(SECTION_HEADER)
                .size(14.0),
        )
        .default_open(false)
        .show(ui, |ui| {
            ui.add_space(2.0);

            let shortcuts = [
                ("Tab", "Toggle this panel"),
                ("Up / Down", "Adjust speed"),
                ("Left / Right", "Adjust rotation"),
                ("L", "Lock mouse"),
                ("C", "Centre + lock"),
                ("R", "Reset defaults"),
                ("Q", "Quit"),
            ];

            egui::Grid::new("shortcuts_grid")
                .num_columns(2)
                .spacing([12.0, 4.0])
                .show(ui, |ui| {
                    for (key, desc) in shortcuts {
                        ui.label(
                            egui::RichText::new(key)
                                .color(ACCENT)
                                .size(12.0)
                                .monospace(),
                        );
                        ui.label(egui::RichText::new(desc).color(TEXT_SECONDARY).size(12.0));
                        ui.end_row();
                    }
                });

            ui.add_space(2.0);
        });
    }

    // ── FPS Overlay ────────────────────────────────────────────────────────────

    fn ui_fps_overlay(&self, ctx: &egui::Context) {
        if !self.show_fps || self.frame_times.is_empty() {
            return;
        }

        let avg_dt: f32 = self.frame_times.iter().sum::<f32>() / self.frame_times.len() as f32;
        let fps = if avg_dt > 0.0 { 1.0 / avg_dt } else { 0.0 };

        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("fps_paint"),
        ));
        let screen = ctx.input(|i| i.viewport_rect());

        // Monospace font for stable width (tabular-nums equivalent).
        let text = format!("{:.0} FPS", fps);
        let font_id = egui::FontId::monospace(13.0);
        let anchor = egui::pos2(screen.max.x - 12.0, 10.0);

        // Measure text to draw background pill.
        let galley = painter.layout_no_wrap(text.clone(), font_id.clone(), FPS_GREEN);
        let text_rect = egui::Align2::RIGHT_TOP.anchor_size(anchor, galley.size());
        let pill_rect = text_rect.expand2(egui::vec2(6.0, 3.0));
        painter.rect_filled(pill_rect, 4.0, FPS_BG);

        // Draw text over pill.
        painter.text(anchor, egui::Align2::RIGHT_TOP, text, font_id, FPS_GREEN);
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────────

/// Format a number with comma separators for readability (e.g. 196000 -> "196,000").
fn format_number(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}

// ── eframe::App ────────────────────────────────────────────────────────────────

impl eframe::App for HopalongApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 1.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Continuous repaint for animation.
        ctx.request_repaint();

        // Apply custom cosmic theme (once).
        self.apply_theme(ctx);

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
