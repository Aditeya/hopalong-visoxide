mod app;
mod renderer;
mod sim;

fn main() -> eframe::Result {
    env_logger::init();

    let mut wgpu_options = egui_wgpu::WgpuConfiguration::default();
    wgpu_options.present_mode = wgpu::PresentMode::Mailbox;

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Hopalong Orbits Visualizer")
            .with_inner_size([1280.0, 720.0])
            .with_min_inner_size([800.0, 500.0]),
        renderer: eframe::Renderer::Wgpu,
        vsync: false,
        wgpu_options,
        ..Default::default()
    };

    eframe::run_native(
        "Hopalong Orbits Visualizer",
        native_options,
        Box::new(|cc| Ok(Box::new(app::HopalongApp::new(cc)))),
    )
}
