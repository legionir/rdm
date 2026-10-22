# RDM browser integration

The extension adds two context-menu commands:

- **Download with RDM** — opens RDM's existing **New download** window with the
  selected link (or media source) filled in.
- **Download all links with RDM** — collects HTTP/HTTPS anchors on the page and
  opens RDM's checkbox window. **Check All**, **Uncheck All**, **OK**, and
  **Cancel** control the selection; only checked links are added when OK is
  pressed.

The extension itself cannot launch a desktop process. It uses the browser's
standard **native messaging** protocol to talk to `rdm-native-host`, which
queues the request for `rdm-gui`. If the two executables are in the same
folder, the host also starts the GUI when it is not running. Requests made
while the GUI is elsewhere are retained and opened the next time it starts.

## Build

```bash
# Desktop programs (run from the repository root)
cargo build --release --bin rdm-native-host
cargo build --release --manifest-path rdm-gui/Cargo.toml

# Unpacked extensions + zip files, using only Python's standard library
python3 browser-extension/build.py

# Extension behavior tests (no npm dependencies)
node --test browser-extension/tests/*.test.js
```

For manual use, copy these into one folder:

- `rdm-gui` / `rdm-gui.exe`
- `rdm-native-host` / `rdm-native-host.exe`
- `browser-extension/install-unix.sh` or `install-windows.ps1`

The Windows CI bundle already has this layout.

## 1. Register the native host

### Windows

In the folder containing the two executables:

```powershell
powershell -ExecutionPolicy Bypass -File .\install-windows.ps1
```

Or point the script at an existing host:

```powershell
powershell -ExecutionPolicy Bypass -File .\install-windows.ps1 `
  -HostPath C:\Tools\RDM\rdm-native-host.exe
```

This writes separate Chrome and Firefox manifests beneath
`%LOCALAPPDATA%\RDM\NativeMessagingHosts` and registers them under HKCU. Do not
move the executables afterwards; rerun the installer if their path changes.

### Linux / macOS

```bash
./install-unix.sh /full/path/to/rdm-native-host
```

The script installs per-user manifests in Chrome's and Mozilla's documented
native-messaging directories.

## 2. Load the extension

### Chrome

1. Open `chrome://extensions`.
2. Enable **Developer mode**.
3. Choose **Load unpacked** and select `browser-extension/dist/chrome`.
4. The fixed key gives the extension ID
   `addehhgmjlamplfjohdpjmpfjpdnjonb`, which is already authorized by the
   native-host installer.

### Firefox

For local testing, open **about:debugging#/runtime/this-firefox**, choose
**Load Temporary Add-on…**, and select
`browser-extension/dist/firefox/manifest.json`. Its ID is `rdm@legionir`, also
already authorized by the installer.

Normal Firefox releases require a Mozilla-signed add-on for permanent
installation. `dist/rdm-firefox.zip` is the package to submit for signing; this
repository does not contain or require signing credentials.

Reload an already-open web page after installing/reloading the extension so its
content script is present. Browser-internal pages (for example `chrome://` and
`about:`) do not permit content scripts and therefore cannot offer page-link
collection.
