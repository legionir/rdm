//! Native desktop UI for rdm (`eframe` / `egui`).
//!
//! `rdm-gui` is a standalone application: it links the `rdm` library directly
//! and runs the download engine in-process, so the `rdm` command-line binary
//! does not need to be installed alongside it.
//!
//! ```text
//! rdm-gui [--data-dir <DIR>]
//! ```

mod app;
mod backend;
mod settings;
mod state;
mod util;
mod views;

use std::path::PathBuf;
use std::process::ExitCode;

const HELP: &str = "\
rdm-gui — native desktop UI for the Rust Download Manager

USAGE:
    rdm-gui [OPTIONS]

OPTIONS:
    -d, --data-dir <DIR>    Directory holding metadata.db (default: .rdm)
    -h, --help              Show this message
    -V, --version           Show the version
";

fn main() -> ExitCode {
    let mut data_dir = PathBuf::from(".rdm");
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                println!("{HELP}");
                return ExitCode::SUCCESS;
            }
            "-V" | "--version" => {
                println!("rdm-gui {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            "-d" | "--data-dir" => match args.next() {
                Some(dir) => data_dir = PathBuf::from(dir),
                None => {
                    eprintln!("rdm-gui: --data-dir needs a value");
                    return ExitCode::from(2);
                }
            },
            other => {
                if let Some(dir) = other.strip_prefix("--data-dir=") {
                    data_dir = PathBuf::from(dir);
                } else {
                    eprintln!("rdm-gui: unexpected argument {other:?}\n\n{HELP}");
                    return ExitCode::from(2);
                }
            }
        }
    }

    if let Err(err) = run(data_dir) {
        eprintln!("rdm-gui: {err}");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

fn run(data_dir: PathBuf) -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([860.0, 560.0])
            .with_title("rdm — Rust Download Manager"),
        ..Default::default()
    };
    eframe::run_native(
        "rdm",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            match app::RdmGuiApp::new(data_dir.clone()) {
                Ok(app) => Ok(Box::new(app) as Box<dyn eframe::App>),
                Err(err) => Err(Box::<dyn std::error::Error + Send + Sync>::from(format!(
                    "{err:#}"
                ))),
            }
        }),
    )
}
