# rdm

**rdm** (Rust Download Manager) is a multi-connection, resumable HTTP/HTTPS downloader inspired by IDM. It splits a file into ranges, fetches them in parallel, stores progress in SQLite, and can pause, resume, or verify the result.

The default build is a **CLI**. The native desktop UI lives in the `rdm-gui` crate (`egui` / `eframe`).

## Features

- Concurrent segmented downloads (`Range` requests, 1–128 connections)
- Resume after pause, interrupt, or process exit
- SQLite metadata under `.rdm/metadata.db` (override with `--data-dir`)
- Per-chunk retries, connect timeout, optional global speed cap
- Optional SHA-256 check after assembly (`--checksum sha256:<hex>`)
- Progress bars (disable with `--no-progress`)
- Offline-friendly: dependencies are vendored under `vendor/`

## Requirements

- Rust **1.82+** (stable) if you build from source
- A C toolchain only if you are not using the bundled SQLite (this project enables `rusqlite` `bundled`)
- Git (to clone)

No system OpenSSL is required; TLS uses `rustls`.

## Install / build

### From a GitHub Release

After CI publishes assets (see [Releases](https://github.com/legionir/rdm/releases)):

| Platform | Asset |
| --- | --- |
| Linux x86_64 | `rdm-linux-x86_64` |
| Windows x86_64 | `rdm-windows-x86_64.exe` |
| Linux GUI | `rdm-gui-linux-x86_64` |
| Windows GUI | `rdm-gui-windows-x86_64.exe` |

```bash
# Linux
chmod +x rdm-linux-x86_64
./rdm-linux-x86_64 --help
```

```powershell
# Windows (PowerShell)
.\rdm-windows-x86_64.exe --help
```

Rolling builds from this branch land on the **nightly** prerelease. Versioned tags (`v*`) create a normal release.

### From source (vendored, recommended)

The repo ships `vendor/` and `vendor-config.txt` so you can build without crates.io.

```bash
git clone https://github.com/legionir/rdm.git
cd rdm
mkdir -p .cargo
cp vendor-config.txt .cargo/config.toml
cargo build --release --locked
```

Binary:

- Linux/macOS: `target/release/rdm`
- Windows: `target/release/rdm.exe`

Put it on your `PATH`, or run it in place.

### Windows from source

1. Install [Rust](https://rustup.rs/) (MSVC toolchain: “Desktop development with C++” in Visual Studio, or use the GNU toolchain).
2. In **Developer PowerShell** or a normal terminal:

```powershell
git clone https://github.com/legionir/rdm.git
cd rdm
New-Item -ItemType Directory -Force .cargo | Out-Null
Copy-Item vendor-config.txt .cargo\config.toml
cargo build --release --locked
.\target\release\rdm.exe --help
```

### GUI

`egui` is not vendored. Build `rdm-gui` with crates.io (and on Linux, GTK/X11/OpenGL dev packages):

```bash
# Linux packages (Debian/Ubuntu)
sudo apt-get install -y pkg-config libgtk-3-dev libx11-dev libgl1-mesa-dev \
  libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev

# Do not use vendor/ for this crate
rm -f .cargo/config.toml
cargo build --release --manifest-path rdm-gui/Cargo.toml
```

Run:

```bash
./rdm-gui/target/release/rdm-gui                     # Linux
./rdm-gui/target/release/rdm-gui --data-dir ~/.rdm   # pick another metadata dir
.\rdm-gui\target\release\rdm-gui.exe                 # Windows
```

`rdm-gui` is **standalone**: it links the `rdm` library and runs the download
engine in-process, so the `rdm` binary does not need to be installed next to
it. Both programs share `<data-dir>/metadata.db`, which means a download
started in the terminal can be paused, resumed or removed from the window and
vice versa.

| CLI | GUI |
| --- | --- |
| `rdm download <URL> …` | **New download** dialog (output, connections, retries, chunk size, speed limit, timeout, checksum, user agent, resume/force) |
| `rdm pause / resume / cancel <ID>` | ⏸ / ▶ / ⏹ row buttons, plus *Pause all* and *Resume all* |
| `rdm download --force` | ⟲ *Restart* row button |
| `rdm list [--state …]` | Download table (full-width rows: click selects, double-click opens details) with search box and state filter |
| `rdm info <ID>` | **Overview**, **Chunks** and **Events** tabs of the details modal |
| `rdm info --json` | **JSON** tab of the details modal (with *Copy JSON*) |
| `rdm remove <ID> [--purge]` | 🗑 row button (confirmation + "delete file too") and *Clear completed* |
| `--data-dir DIR` | `--data-dir` flag and the *Metadata directory* field in Settings |

Beyond the CLI surface the window adds:

* **Sidebar panels** — *Queue* and *Settings* live in sidebars toggled from
  the top menu bar. Settings include a dark/light theme switch and 📂 buttons
  that pick directories in the native file explorer.
* **Download queue** — at most `max_concurrent` transfers run at once
  (default 3, `0` = unlimited); the rest wait in the *Queue* sidebar and can be
  dropped individually or all at once.
* **Status bar** — a one-line footer with record counters; the **Events** and
  **App log** buttons expand a box above it (wrapping long lines). The engine's
  `tracing` output is captured in-process and shown live; verbosity is a combo
  box (`off`..`trace`), also settable with `-v` / `-vv` / `-vvv`, and `RUST_LOG`
  still wins.
* **Safe exit** — closing the window pauses the transfers this window owns and
  waits for their engines to flush, exactly like Ctrl+C does for the CLI;
  anything that cannot stop within 5 s is marked `interrupted` so it offers
  *Resume*. Downloads driven by another process are never touched.

Defaults for new downloads live in `<data-dir>/settings.toml`; the window
reloads that file automatically when it changes on disk:

```toml
download_dir    = "/home/me/Downloads"
connections     = 8
retries         = 5
chunk_size      = "1MiB"
max_speed       = ""          # e.g. "5MB/s"
timeout_secs    = 30
max_concurrent  = 3           # 0 = unlimited
refresh_ms      = 600
confirm_remove  = true
purge_on_remove = false
dark_mode       = true
log_level       = "info"
```

## Run

Global options (all subcommands):

| Flag | Meaning |
| --- | --- |
| `--data-dir DIR` | Metadata directory (default: `.rdm`) |
| `-v` / `-vv` / `-vvv` | Log verbosity (info / debug / trace) |

### Download

```bash
rdm download https://example.com/file.zip
rdm download https://example.com/file.zip -o ~/Downloads/file.zip
rdm download https://example.com/file.zip -c 16 --chunk-size 4MiB
rdm download https://example.com/file.zip --resume
rdm download https://example.com/file.zip --force
rdm download https://example.com/file.zip --max-speed 5MB/s
rdm download https://example.com/file.zip --checksum sha256:0123…abcd
rdm download https://example.com/file.zip --retry 8 --timeout 90
rdm download https://example.com/file.zip --user-agent "rdm/0.1"
rdm download https://example.com/file.zip --no-progress
```

On Windows, use `rdm.exe` and Windows paths:

```powershell
rdm.exe download https://example.com/file.zip -o D:\Downloads\file.zip
```

### Control an existing job

IDs can be the numeric row or the public id (`dl-…`) printed by `list` / `download`.

```bash
rdm list
rdm list --all
rdm list --state running
rdm info dl-1a2b3c4d
rdm info dl-1a2b3c4d --json
rdm pause dl-1a2b3c4d
rdm resume dl-1a2b3c4d
rdm cancel dl-1a2b3c4d
rdm remove dl-1a2b3c4d
rdm remove dl-1a2b3c4d --purge
```

`pause` / `cancel` write state into SQLite so a running engine in another terminal can notice. `remove --purge` also deletes the assembled file and sidecar.

## How it works

1. Probe the URL (size, `Accept-Ranges`, redirects).
2. Plan chunks (default minimum size `1MiB`).
3. Download ranges concurrently into a chunk directory.
4. Persist progress in `.rdm/metadata.db`.
5. Merge chunks into the output file and optionally verify SHA-256.

## Development

```bash
cargo test --locked
```

CI (`.github/workflows/build.yml`) builds Linux and Windows CLI binaries, optionally Linux GUI, and publishes them to GitHub Releases (`nightly` on branch pushes, versioned on `v*` tags).

## License

MIT — see `Cargo.toml`.
