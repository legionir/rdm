//! Small, dependency-free helpers: byte formatting, path safety, rate limiting,
//! identifiers and timestamps.

pub mod human;
pub mod path;
pub mod rate;

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Monotonic process-local sequence used to build public download ids.
static ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generate a short, human-friendly public id such as `dl-a1b2c3d4`.
pub fn new_public_id() -> String {
    let seq = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mix = (now_ms() as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(seq.wrapping_mul(0xBF58_476D_1CE4_E5B9));
    format!("dl-{:08x}", (mix >> 16) as u32)
}

/// Simple arithmetic mean of an iterator of `f64` samples.
pub fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}
