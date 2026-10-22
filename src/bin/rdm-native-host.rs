//! Chrome/Firefox native-messaging endpoint for RDM.
//!
//! Browsers launch this executable, send one length-prefixed JSON request on
//! stdin, and wait for one response on stdout.  Requests are queued for the
//! GUI; no download engine or browser data is logged to the protocol stream.

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::Command;

use rdm::browser::{self, BrowserRequest};

const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;

fn main() {
    let result = run();
    let response = match result {
        Ok(running) => serde_json::json!({ "ok": true, "running": running }),
        Err(error) => serde_json::json!({ "ok": false, "error": error }),
    };
    // If the response itself cannot be written there is no useful recovery;
    // the browser will report its own disconnected-native-host error.
    let _ = write_frame(&mut std::io::stdout().lock(), &response.to_string());
}

fn run() -> Result<bool, String> {
    let request = read_frame(&mut std::io::stdin().lock())?;
    let request = serde_json::from_str::<BrowserRequest>(&request)
        .map_err(|err| format!("invalid native-messaging request: {err}"))?;
    browser::enqueue(request)?;

    if browser::has_running_instance() {
        return Ok(true);
    }
    launch_gui()
}

/// Launch the sibling GUI when the host was packaged beside it.  A standalone
/// host can still queue requests: the response then says `running: false` and
/// the extension tells the user that opening RDM will complete the hand-off.
fn launch_gui() -> Result<bool, String> {
    let executable = std::env::current_exe()
        .map_err(|err| format!("cannot locate the native messaging host: {err}"))?;
    let directory = executable
        .parent()
        .ok_or_else(|| "native messaging host has no parent directory".to_string())?;
    let gui_name = if cfg!(target_os = "windows") {
        "rdm-gui.exe"
    } else {
        "rdm-gui"
    };
    let gui: PathBuf = directory.join(gui_name);
    if !gui.is_file() {
        return Ok(false);
    }
    Command::new(&gui)
        .current_dir(directory)
        .spawn()
        .map_err(|err| format!("request saved, but RDM could not be opened: {err}"))?;
    Ok(true)
}

/// Read one native-messaging frame (little-endian u32 length + UTF-8 JSON).
fn read_frame(reader: &mut impl Read) -> Result<String, String> {
    let mut length = [0u8; 4];
    reader
        .read_exact(&mut length)
        .map_err(|err| format!("cannot read native-messaging frame length: {err}"))?;
    let length = u32::from_le_bytes(length) as usize;
    if length == 0 || length > MAX_FRAME_BYTES {
        return Err(format!("invalid native-messaging frame size: {length}"));
    }
    let mut body = vec![0u8; length];
    reader
        .read_exact(&mut body)
        .map_err(|err| format!("cannot read native-messaging frame: {err}"))?;
    String::from_utf8(body).map_err(|err| format!("native-messaging frame is not UTF-8: {err}"))
}

fn write_frame(writer: &mut impl Write, json: &str) -> Result<(), String> {
    let length = u32::try_from(json.len())
        .map_err(|_| "native-messaging response is too large".to_string())?;
    writer
        .write_all(&length.to_le_bytes())
        .and_then(|_| writer.write_all(json.as_bytes()))
        .and_then(|_| writer.flush())
        .map_err(|err| format!("cannot write native-messaging response: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_messaging_frames_round_trip() {
        let json = r#"{"action":"open_new","url":"https://example.com/a.zip"}"#;
        let mut framed = Vec::new();
        write_frame(&mut framed, json).unwrap();
        assert_eq!(
            u32::from_le_bytes(framed[..4].try_into().unwrap()) as usize,
            json.len()
        );
        assert_eq!(read_frame(&mut framed.as_slice()).unwrap(), json);
    }

    #[test]
    fn absurd_frame_sizes_are_rejected_before_allocation() {
        let mut framed = Vec::new();
        write_frame(&mut framed, "{}").unwrap();
        framed[..4].copy_from_slice(&(u32::MAX).to_le_bytes());
        assert!(read_frame(&mut framed.as_slice()).is_err());
    }
}
