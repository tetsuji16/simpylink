//! simpylink GUI: egui canvas editor for Simulink-style block models.
//! Backend (Python) owns all block math — see docs/protocol.md.
mod api_client;
mod app;
mod canvas;
mod model;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "simpylink",
        options,
        Box::new(|_cc| Ok(Box::new(app::SimpylinkApp::default()))),
    )
}
