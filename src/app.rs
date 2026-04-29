// stub — full implementation coming next
use eframe::egui;

pub struct DiskChurnApp;

impl Default for DiskChurnApp {
    fn default() -> Self {
        Self
    }
}

impl eframe::App for DiskChurnApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("DiskChurn — loading...");
        });
    }
}
