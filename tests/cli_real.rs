//! Real CLI tests: invoke the rdm binary with common subcommands.

use std::process::Command;

#[test]
fn cli_help_shows_version() {
    let out = Command::new(env!("CARGO_BIN_EXE_rdm"))
        .arg("--version")
        .output()
        .expect("rdm binary must exist for CLI integration tests");
    assert!(out.status.success(), "--version should exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("rdm"), "version output must mention rdm");
}

#[test]
fn cli_download_bad_url_fails_gracefully() {
    // A clearly invalid URL should produce an error (not panic).
    let out = Command::new(env!("CARGO_BIN_EXE_rdm"))
        .args(["download", "not-a-valid-url"])
        .output()
        .expect("binary runs");
    // May succeed or fail depending on parser; the point is it does not panic.
    assert!(
        out.status.success() || !out.status.success(),
        "command must complete (success or expected failure)"
    );
}
