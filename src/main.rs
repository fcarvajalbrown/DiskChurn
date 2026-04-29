mod app;
mod classifier;
mod entropy;
mod scanner;
mod treemap;
mod types;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("DiskChurn")
            .with_inner_size([1200.0, 700.0]),
        ..Default::default()
    };
    eframe::run_native(
        "DiskChurn",
        options,
        Box::new(|_cc| Ok(Box::new(app::DiskChurnApp::default()))),
    )
}
