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
mod clipboard;
mod dropzone;
mod frames;
mod icon;
mod logging;
mod platform;
mod settings;
mod state;
mod theme;
mod tray;
mod util;
mod ux;
mod views;
mod windows;

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
    // `--data-dir` outranks the saved value, so remember that it was given.
    let mut data_dir_explicit = false;
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
                Some(dir) => {
                    data_dir = PathBuf::from(dir);
                    data_dir_explicit = true;
                }
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
                    data_dir_explicit = true;
                } else {
                    eprintln!("rdm-gui: unexpected argument {other:?}\n\n{HELP}");
                    return ExitCode::from(2);
                }
            }
        }
    }

    let logging = logging::install(verbosity.unwrap_or("info"));
    if let Err(err) = run(data_dir, data_dir_explicit, logging, verbosity) {
        eprintln!("rdm-gui: {err}");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

fn run(
    data_dir: PathBuf,
    data_dir_explicit: bool,
    logging: Option<logging::LogControl>,
    forced_level: Option<&'static str>,
) -> Result<(), eframe::Error> {
    // Window geometry comes from the design tokens, like every other size.
    let sizes = theme::Sizes::default();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(sizes.window_default)
            .with_min_inner_size(sizes.window_min)
            .with_title("RDM")
            // The same asset Explorer shows for rdm-gui.exe (build.rs) appears
            // in the title bar, the taskbar and Alt-Tab.
            .with_icon(std::sync::Arc::new(icon::window_icon())),
        ..Default::default()
    };
    eframe::run_native(
        "RDM",
        options,
        Box::new(move |cc| {
            // The tray's escape hatch needs the window handle *before* the first
            // frame, and the app needs the context to ask for one.
            windows::remember_main_window_from(cc);
            match app::RdmGuiApp::new(
                cc,
                data_dir.clone(),
                data_dir_explicit,
                logging.clone(),
                forced_level,
            ) {
                Ok(app) => Ok(Box::new(app) as Box<dyn eframe::App>),
                Err(err) => Err(Box::<dyn std::error::Error + Send + Sync>::from(format!(
                    "{err:#}"
                ))),
            }
        }),
    )
}
