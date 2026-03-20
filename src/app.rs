use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;

use eframe::egui;

use crate::renderer::{self, HopalongPaintCallback, HopalongRendererResources};
use crate::sim::HopalongSim;

// ── Adaptive Theme Colors ──────────────────────────────────────────────────────

/// Theme-specific color palette for adaptive dark/light mode support.
struct ThemeColors {
    accent: egui::Color32,
    accent_hover: egui::Color32,
    accent_dim: egui::Color32,
    section_header: egui::Color32,
    text_primary: egui::Color32,
    text_secondary: egui::Color32,
    panel_bg: egui::Color32,
    panel_stroke: egui::Color32,
    widget_bg: egui::Color32,
    widget_bg_hover: egui::Color32,
    widget_bg_active: egui::Color32,
    separator: egui::Color32,
    reset_color: egui::Color32,
    quit_color: egui::Color32,
    fps_green: egui::Color32,
    fps_bg: egui::Color32,
    extreme_bg: egui::Color32,
    faint_bg: egui::Color32,
    title_color: egui::Color32,
}

impl ThemeColors {
    /// Dark mode color palette - cosmic theme with deep blues.
    fn dark() -> Self {
        Self {
            accent: egui::Color32::from_rgb(100, 120, 220),
            accent_hover: egui::Color32::from_rgb(130, 150, 255),
            accent_dim: egui::Color32::from_rgb(70, 85, 160),
            section_header: egui::Color32::from_rgb(150, 170, 255),
            text_primary: egui::Color32::from_rgb(210, 210, 225),
            text_secondary: egui::Color32::from_rgb(130, 130, 155),
            panel_bg: egui::Color32::from_rgba_premultiplied(10, 10, 20, 210),
            panel_stroke: egui::Color32::from_rgba_premultiplied(70, 70, 130, 50),
            widget_bg: egui::Color32::from_rgba_premultiplied(30, 30, 50, 180),
            widget_bg_hover: egui::Color32::from_rgba_premultiplied(45, 45, 75, 200),
            widget_bg_active: egui::Color32::from_rgba_premultiplied(60, 60, 105, 220),
            separator: egui::Color32::from_rgba_premultiplied(70, 70, 130, 40),
            reset_color: egui::Color32::from_rgb(210, 180, 60),
            quit_color: egui::Color32::from_rgb(170, 65, 65),
            fps_green: egui::Color32::from_rgb(80, 240, 120),
            fps_bg: egui::Color32::from_rgba_premultiplied(0, 0, 0, 160),
            extreme_bg: egui::Color32::from_rgb(8, 8, 16),
            faint_bg: egui::Color32::from_rgba_premultiplied(20, 20, 35, 100),
            title_color: egui::Color32::WHITE,
        }
    }

    /// Light mode color palette - clean, high contrast design with solid panels.
    fn light() -> Self {
        Self {
            accent: egui::Color32::from_rgb(60, 90, 200),
            accent_hover: egui::Color32::from_rgb(80, 115, 235),
            accent_dim: egui::Color32::from_rgb(45, 70, 170),
            section_header: egui::Color32::from_rgb(50, 75, 180),
            text_primary: egui::Color32::from_rgb(30, 30, 45),
            text_secondary: egui::Color32::from_rgb(100, 100, 120),
            panel_bg: egui::Color32::from_rgb(245, 245, 250),
            panel_stroke: egui::Color32::from_rgba_premultiplied(180, 180, 200, 150),
            widget_bg: egui::Color32::from_rgb(255, 255, 255),
            widget_bg_hover: egui::Color32::from_rgb(240, 240, 245),
            widget_bg_active: egui::Color32::from_rgb(230, 230, 240),
            separator: egui::Color32::from_rgb(220, 220, 230),
            reset_color: egui::Color32::from_rgb(180, 140, 20),
            quit_color: egui::Color32::from_rgb(160, 50, 50),
            fps_green: egui::Color32::from_rgb(40, 160, 60),
            fps_bg: egui::Color32::from_rgba_premultiplied(255, 255, 255, 200),
            extreme_bg: egui::Color32::from_rgb(240, 240, 245),
            faint_bg: egui::Color32::from_rgb(235, 235, 240),
            title_color: egui::Color32::from_rgb(20, 20, 35),
        }
    }
}

// ── App State ──────────────────────────────────────────────────────────────────

pub struct HopalongApp {
    sim: HopalongSim,
    show_settings: bool,
    last_frame: Instant,
    frame_times: VecDeque<f32>,
    show_fps: bool,
    should_quit: bool,
    theme_applied: bool,
    current_dark_mode: Option<bool>, // Track current theme to detect changes
    // Pre-allocated buffer for particle instances to avoid per-frame allocation
    instance_buffer: Vec<crate::renderer::ParticleInstance>,
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

        let max_instances = sim.total_particles();

        Self {
            sim,
            show_settings: false,
            last_frame: Instant::now(),
            frame_times: VecDeque::with_capacity(120),
            show_fps: false,
            should_quit: false,
            theme_applied: false,
            current_dark_mode: None,
            instance_buffer: Vec::with_capacity(max_instances),
        }
    }

    // ── Theme ──────────────────────────────────────────────────────────────────

    /// Apply adaptive theme based on system preference (dark/light mode).
    /// Re-applies when theme changes.
    fn apply_theme(&mut self, ctx: &egui::Context) {
        // Detect current theme from egui's resolved visuals
        let system_dark_mode = ctx.style().visuals.dark_mode;

        // Only reapply if theme changed or first run
        if self.theme_applied && self.current_dark_mode == Some(system_dark_mode) {
            return;
        }

        self.theme_applied = true;
        self.current_dark_mode = Some(system_dark_mode);

        // Select appropriate color palette
        let colors = if system_dark_mode {
            ThemeColors::dark()
        } else {
            ThemeColors::light()
        };

        ctx.style_mut(|style| {
            let v = &mut style.visuals;

            // Base mode
            v.dark_mode = system_dark_mode;
            v.panel_fill = colors.panel_bg;
            v.window_fill = colors.panel_bg;
            v.extreme_bg_color = colors.extreme_bg;
            v.faint_bg_color = colors.faint_bg;

            // Selection accent
            v.selection.bg_fill = colors.accent;
            v.selection.stroke = egui::Stroke::new(1.0, colors.accent_hover);

            // ── Widget states ──

            // Inactive
            v.widgets.inactive.bg_fill = colors.widget_bg;
            v.widgets.inactive.weak_bg_fill = colors.widget_bg;
            v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, colors.text_primary);
            v.widgets.inactive.bg_stroke = egui::Stroke::new(0.5, colors.panel_stroke);
            v.widgets.inactive.corner_radius = egui::CornerRadius::same(4);

            // Hovered
            v.widgets.hovered.bg_fill = colors.widget_bg_hover;
            v.widgets.hovered.weak_bg_fill = colors.widget_bg_hover;
            v.widgets.hovered.fg_stroke = egui::Stroke::new(1.5, colors.text_primary);
            v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, colors.accent_dim);
            v.widgets.hovered.corner_radius = egui::CornerRadius::same(4);

            // Active (pressed)
            v.widgets.active.bg_fill = colors.widget_bg_active;
            v.widgets.active.weak_bg_fill = colors.widget_bg_active;
            v.widgets.active.fg_stroke = egui::Stroke::new(2.0, colors.accent);
            v.widgets.active.bg_stroke = egui::Stroke::new(1.0, colors.accent);
            v.widgets.active.corner_radius = egui::CornerRadius::same(4);

            // Open (expanded collapsing headers)
            let open_bg = if system_dark_mode {
                egui::Color32::from_rgba_premultiplied(35, 35, 60, 140)
            } else {
                egui::Color32::from_rgba_premultiplied(230, 230, 245, 150)
            };
            v.widgets.open.bg_fill = open_bg;
            v.widgets.open.weak_bg_fill = open_bg;
            v.widgets.open.fg_stroke = egui::Stroke::new(1.0, colors.section_header);
            v.widgets.open.corner_radius = egui::CornerRadius::same(4);

            // Non-interactive (labels, separators)
            v.widgets.noninteractive.bg_fill = egui::Color32::TRANSPARENT;
            v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, colors.text_secondary);
            v.widgets.noninteractive.bg_stroke = egui::Stroke::new(0.5, colors.separator);
            v.widgets.noninteractive.corner_radius = egui::CornerRadius::same(4);

            // Slider handle - ensure visibility in both modes
            v.handle_shape = egui::style::HandleShape::Rect { aspect_ratio: 0.5 };

            // Window chrome
            v.window_corner_radius = egui::CornerRadius::same(8);
            v.window_stroke = egui::Stroke::new(1.0, colors.panel_stroke);

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

        // Get current theme colors
        let system_dark_mode = self.current_dark_mode.unwrap_or(true);
        let colors = if system_dark_mode {
            ThemeColors::dark()
        } else {
            ThemeColors::light()
        };

        let panel_frame = egui::Frame::default()
            .fill(colors.panel_bg)
            .inner_margin(egui::Margin::same(frame_margin))
            .stroke(egui::Stroke::new(1.0, colors.panel_stroke))
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
                            .color(colors.title_color)
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
        let system_dark_mode = self.current_dark_mode.unwrap_or(true);
        let colors = if system_dark_mode {
            ThemeColors::dark()
        } else {
            ThemeColors::light()
        };

        egui::CollapsingHeader::new(
            egui::RichText::new("Simulation")
                .color(colors.section_header)
                .size(14.0),
        )
        .default_open(true)
        .show(ui, |ui| {
            ui.add_space(2.0);

            ui.label(
                egui::RichText::new("Speed")
                    .color(colors.text_primary)
                    .size(12.0),
            );
            ui.add(egui::Slider::new(&mut self.sim.settings.speed, 0.0..=50.0).step_by(0.25))
                .on_hover_text("Arrow Up / Down");

            ui.add_space(4.0);

            ui.label(
                egui::RichText::new("Rotation Speed")
                    .color(colors.text_primary)
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
        let system_dark_mode = self.current_dark_mode.unwrap_or(true);
        let colors = if system_dark_mode {
            ThemeColors::dark()
        } else {
            ThemeColors::light()
        };

        egui::CollapsingHeader::new(
            egui::RichText::new("Camera")
                .color(colors.section_header)
                .size(14.0),
        )
        .default_open(true)
        .show(ui, |ui| {
            ui.add_space(2.0);

            ui.label(
                egui::RichText::new("Field of View")
                    .color(colors.text_primary)
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
        let system_dark_mode = self.current_dark_mode.unwrap_or(true);
        let colors = if system_dark_mode {
            ThemeColors::dark()
        } else {
            ThemeColors::light()
        };

        egui::CollapsingHeader::new(
            egui::RichText::new("Particles")
                .color(colors.section_header)
                .size(14.0),
        )
        .default_open(false)
        .show(ui, |ui| {
            ui.add_space(2.0);

            ui.label(
                egui::RichText::new("Points per Subset (thousands)")
                    .color(colors.text_primary)
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
                    .color(colors.text_primary)
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
                    .color(colors.text_primary)
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
                .color(colors.text_secondary)
                .size(11.0),
            );

            ui.add_space(2.0);
        });
    }

    fn ui_section_info(&mut self, ui: &mut egui::Ui) {
        let system_dark_mode = self.current_dark_mode.unwrap_or(true);
        let colors = if system_dark_mode {
            ThemeColors::dark()
        } else {
            ThemeColors::light()
        };

        egui::CollapsingHeader::new(
            egui::RichText::new("Info")
                .color(colors.section_header)
                .size(14.0),
        )
        .default_open(false)
        .show(ui, |ui| {
            ui.add_space(2.0);

            ui.checkbox(&mut self.show_fps, "Show FPS Counter");

            ui.label(
                egui::RichText::new(format!(
                    "Particles: {}",
                    format_number(self.sim.total_particles())
                ))
                .color(colors.text_secondary)
                .size(12.0),
            );

            ui.add_space(2.0);
        });
    }

    fn ui_action_buttons(&mut self, ui: &mut egui::Ui) {
        let system_dark_mode = self.current_dark_mode.unwrap_or(true);
        let colors = if system_dark_mode {
            ThemeColors::dark()
        } else {
            ThemeColors::light()
        };

        // Reset — amber accent, full width, taller target.
        let reset_btn = egui::Button::new(
            egui::RichText::new("Reset Defaults")
                .color(colors.reset_color)
                .size(13.0),
        )
        .min_size(egui::vec2(ui.available_width(), 28.0));
        if ui.add(reset_btn).on_hover_text("R").clicked() {
            self.sim.reset_defaults();
        }

        ui.add_space(4.0);

        // Quit — subdued red, slightly smaller.
        let quit_btn = egui::Button::new(
            egui::RichText::new("Quit")
                .color(colors.quit_color)
                .size(12.0),
        )
        .min_size(egui::vec2(ui.available_width(), 24.0));
        if ui.add(quit_btn).on_hover_text("Q").clicked() {
            self.should_quit = true;
        }
    }

    fn ui_section_shortcuts(&mut self, ui: &mut egui::Ui) {
        let system_dark_mode = self.current_dark_mode.unwrap_or(true);
        let colors = if system_dark_mode {
            ThemeColors::dark()
        } else {
            ThemeColors::light()
        };

        egui::CollapsingHeader::new(
            egui::RichText::new("Keyboard Shortcuts")
                .color(colors.section_header)
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
                                .color(colors.accent)
                                .size(12.0)
                                .monospace(),
                        );
                        ui.label(
                            egui::RichText::new(desc)
                                .color(colors.text_secondary)
                                .size(12.0),
                        );
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

        // Get current theme colors
        let system_dark_mode = self.current_dark_mode.unwrap_or(true);
        let colors = if system_dark_mode {
            ThemeColors::dark()
        } else {
            ThemeColors::light()
        };

        // Monospace font for stable width (tabular-nums equivalent).
        let text = format!("{:.0} FPS", fps);
        let font_id = egui::FontId::monospace(13.0);
        let anchor = egui::pos2(screen.max.x - 12.0, 10.0);

        // Measure text to draw background pill.
        let galley = painter.layout_no_wrap(text.clone(), font_id.clone(), colors.fps_green);
        let text_rect = egui::Align2::RIGHT_TOP.anchor_size(anchor, galley.size());
        let pill_rect = text_rect.expand2(egui::vec2(6.0, 3.0));
        painter.rect_filled(pill_rect, 4.0, colors.fps_bg);

        // Draw text over pill.
        painter.text(
            anchor,
            egui::Align2::RIGHT_TOP,
            text,
            font_id,
            colors.fps_green,
        );
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
        self.frame_times.push_back(dt);
        if self.frame_times.len() > 60 {
            self.frame_times.pop_front();
        }

        // ── Input ──
        self.handle_input(ctx);

        // ── Update simulation ──
        self.sim.update(dt);

        // ── UI overlays ──
        self.ui_settings(ctx);
        self.ui_fps_overlay(ctx);

        // ── Central panel: custom wgpu rendering ──
        // Only rebuild instances if dirty (optimization)
        if self.sim.instances_dirty {
            renderer::build_instances_into(&self.sim, &mut self.instance_buffer);
            self.sim.instances_dirty = false;
        }

        let instances = Arc::new(self.instance_buffer.clone());

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
                // instances are already built above, but we need to recalc uniforms with correct aspect

                let callback = egui_wgpu::Callback::new_paint_callback(
                    rect,
                    HopalongPaintCallback {
                        uniforms,
                        instances: instances.clone(),
                    },
                );
                ui.painter().add(callback);
            });
    }
}
