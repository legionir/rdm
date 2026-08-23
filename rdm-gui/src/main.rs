//! Native desktop UI for rdm (`eframe` / `egui`).

mod app;
mod settings;
mod state;
mod views;

use std::process::ExitCode;

fn main() -> ExitCode {
    if let Err(err) = run() {
        eprintln!("rdm-gui: {err}");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

fn run() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([980.0, 640.0])
            .with_min_inner_size([720.0, 480.0])
            .with_title("rdm — Rust Download Manager"),
        ..Default::default()
    };
    eframe::run_native(
        "rdm",
        options,
        Box::new(|_cc| Box::new(app::RdmGuiApp::new())),
    )
}
