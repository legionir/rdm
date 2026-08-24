//! Native desktop UI for rdm (`eframe` / `egui`).
//!
//! `rdm-gui` is a standalone application: it links the `rdm` library directly
//! and runs the download engine in-process, so the `rdm` command-line binary
//! does not need to be installed alongside it.
//!
//! ```text
//! rdm-gui [--data-dir <DIR>]
//! ```

// Native GUI app: on Windows do not open a console window alongside the app
// (and do not die when that console is closed).
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod backend;
mod logging;
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
    -v, -vv, -vvv           Engine log verbosity (info / debug / trace)
    -h, --help              Show this message
    -V, --version           Show the version

The captured log is shown in the window's \"App log\" tab; RUST_LOG overrides
these flags, exactly like the CLI.
";

fn main() -> ExitCode {
    let mut data_dir = PathBuf::from(".rdm");
    let mut verbosity: Option<&'static str> = None;
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
            "-v" | "--verbose" => verbosity = Some("info"),
            "-vv" => verbosity = Some("debug"),
            "-vvv" => verbosity = Some("trace"),
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

    let logging = logging::install(verbosity.unwrap_or("info"));
    if let Err(err) = run(data_dir, logging, verbosity) {
        eprintln!("rdm-gui: {err}");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

fn run(
    data_dir: PathBuf,
    logging: Option<logging::LogControl>,
    forced_level: Option<&'static str>,
) -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([860.0, 560.0])
            .with_title("RDM"),
        ..Default::default()
    };
    eframe::run_native(
        "RDM",
        options,
        Box::new(move |_cc| {
            match app::RdmGuiApp::new(data_dir.clone(), logging.clone(), forced_level) {
                Ok(app) => Ok(Box::new(app) as Box<dyn eframe::App>),
                Err(err) => Err(Box::<dyn std::error::Error + Send + Sync>::from(format!(
                    "{err:#}"
                ))),
            }
        }),
    )
}
