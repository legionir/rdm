//! Embeds the application icon into the Windows executable.
//!
//! `assets/icon.ico` is produced by `assets/generate_icon.py` and checked in,
//! so this build step only needs to hand it to the linker. Without it, Explorer
//! and the taskbar show the default generic icon for `rdm-gui.exe`; the window
//! and tray use `assets/icon.rgba` directly (see `src/icon.rs`).
//!
//! On non-Windows targets this is a no-op, which keeps the Linux test job (and
//! the Cargo cache) untouched.

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=assets/icon.rgba");

    #[cfg(target_os = "windows")]
    {
        // Fail loudly: shipping an unsigned, icon-less executable is worse than
        // a broken build, and the file is part of the repository.
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "rdm");
        res.set("FileDescription", "rdm — Rust Download Manager");
        res.compile()
            .expect("embedding assets/icon.ico into the executable failed");
    }
}
