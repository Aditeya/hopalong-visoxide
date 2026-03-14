mod app;
mod renderer;
mod sim;

fn main() -> eframe::Result {
    env_logger::init();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Hopalong Orbits Visualizer")
            .with_inner_size([1280.0, 720.0]),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    eframe::run_native(
        "Hopalong Orbits Visualizer",
        native_options,
        Box::new(|cc| Ok(Box::new(app::HopalongApp::new(cc)))),
    )
}
