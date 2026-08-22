//! rdm — Rust Download Manager (binary entry point).
//!
//! CLI-only, multi-threaded, resumable HTTP download engine inspired by IDM.
//! All logic lives in the `rdm` library crate.

use clap::Parser;
use std::process::ExitCode;

fn main() -> ExitCode {
    match std::panic::catch_unwind(|| {
        let opts = rdm::cli::commands::Opts::parse();
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("rdm")
            .build()?;
        rt.block_on(rdm::cli::commands::run(opts))
    }) {
        Ok(Ok(code)) => ExitCode::from(code),
        Ok(Err(err)) => {
            eprintln!("rdm: {err:#}");
            ExitCode::from(1)
        }
        Err(_) => {
            eprintln!("rdm: fatal internal error");
            ExitCode::from(2)
        }
    }
}
