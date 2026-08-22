//! rdm — Rust Download Manager (library crate).
//!
//! Layering:
//! * [`cli`] — command surface (clap).
//! * [`network`] — HTTP client + Range semantics.
//! * [`storage`] — SQLite state + metadata sidecars.
//! * [`downloader`] — chunk planning, scheduling, workers, engine.
//! * [`filesystem`] — assembly and checksum verification.
//! * [`models`] — shared domain types.
//! * [`utils`] — formatting, path safety, rate limiting.
//! * [`console`] — terminal progress rendering.

pub mod cli;
pub mod console;
pub mod downloader;
pub mod filesystem;
pub mod models;
pub mod network;
pub mod storage;
pub mod utils;

#[cfg(feature = "gui")]
pub mod gui;
