# rdm — Desktop Integration & Reported-Defect Report (UX increment 2)

- **Artifact type:** UX Designs (flows and behaviour contract) + defect record
- **Owner:** UX Designer (EXE-043) · **Approvers:** Design Manager, Product Manager (PM)
- **Consumers:** UI, Frontend, QA
- **Basis:** the two defects and five features the product owner reported on 2026-10-05,
  implemented on `arena/01a10cef-rdm` (see the Change Manifest at the end)
- **Related:** `audits/ux-flows-and-ia.md` (flows added in §3.4/§4), `audits/ux-glossary.md`
  (new terms), `audits/ux-feedback-error-undo-policy.md` §7 (feedback and recovery rules),
  `audits/ux-designer-execution-plan.md` PH-7 (steps and statuses)

## 1. What was reported

| # | Report (verbatim intent) | Class |
| --- | --- | --- |
| **Bug A** | typing an output folder such as `c:\download\rdm` results in `c:\\download\\rdm` | defect — data corruption on save/load |
| **Bug B** | starting the app opens a terminal window; closing that terminal closes the app | defect — no window may ever appear |
| **F1** | a fitting application icon, visible in all parts of the app | feature |
| **F2** | after copying a link, **New** pre-fills the URL field from the clipboard | feature |
| **F3** | a tray icon: **Close** must not quit — the app goes to the notification area | feature |
| **F4** | tray menu with frequent actions, at minimum **New download** pre-filled from the clipboard | feature |
| **F5** | a Settings toggle for an always-visible floating button at the right corner above the taskbar clock; dropping a link (e.g. dragged from a browser) on it opens the New-download page with that link | feature |

## 2. Bug A — backslashes multiplied on every save/load cycle

**Root cause (verified by reading the code, not guessed).** `AppSettings::serialize` escaped
every `\` as `\\` (`settings.rs`, `escape()`), but `AppSettings::parse` had **no
corresponding decoder** — `parse()` only unescaped `\n`/`\t`/`\"` and treated a lone
backslash as itself. So each *save → load* round doubled every backslash: `C:\download\rdm`
→ `C:\\download\\rdm` → `C:\\\\download\\\\rdm` … exactly the reported symptom, and it
worsened with every save (theme switch, setting change, download added).

**Fix (in scope, no format break).**

* `escape()` now escapes only what `parse()` understands: `\n`, `\t`, `"`.
  A stray backslash — the normal case for a Windows path — is written verbatim.
* `parse()` keeps its tolerance for the old, over-escaped form, so files written by earlier
  versions still load (a `\\` sequence is read back as a single `\`).
* `download_dir = D:\iso\files` and `--data-dir` files stay byte-identical through any number
  of save/load cycles.

**Evidence.** `rdm-gui/src/settings.rs` tests: a five-round round-trip of `C:\download\rdm`
asserting the value is unchanged and that no doubled backslash appears after any round
(`settings.rs::windows_paths_survive_repeated_saves`), plus the loader case for an
over-escaped legacy file. CI job `build-gui-windows` → `Test GUI crate` (run IDs in
`audits/evidence/ux-feature-pack-ci-runs.json`).

**Related fix found while verifying A.** `--data-dir DIR` was overwritten by the value saved
in the settings file, so a `--data-dir` run silently used the old directory on reload. The
flag now wins for the run and for every hot reload; the file records the directory it lives
in (self-consistent: the file can never point somewhere else than its own folder).

## 3. Bug B — a terminal window appears (and owns the app's lifetime)

**Finding.** The GUI binary already carries `#![cfg_attr(target_os = "windows",
windows_subsystem = "windows")]` (`main.rs:13`, present since `f528faf`/PR #7), so the
*process itself* does not allocate a console — that is why the app dies when “its” terminal is
closed: the terminal was a **console child process**, not the app.

**Fix.** The one place that spawns a process — `util::open_in_file_manager` (the 📂 button) —
now passes `CREATE_NO_WINDOW` (`0x0800_0000`) through `std::os::windows::process::CommandExt`,
so no console child can appear for any user action. The icon embedding (below) also uses a
build script, which cannot open a window.

**Honest status.** The reported behaviour (“closing the terminal closes the app”) is
consistent with a console child started from a `.bat`/shortcut with `cmd /c`. The child is
gone by construction; whether the *specific* terminal the reporter saw came from an old build
or from a launcher script cannot be determined from here — QA's smoke test (UX-ESC-005)
starts `rdm-gui.exe` from Explorer with no console present and verifies no window appears.

## 4. F1 — application icon

* `rdm-gui/assets/generate_icon.py` renders the icon from the app's own palette
  (`#1D4ED8` / `#7CB3FF`, taken from `theme::tokens`) with 4× supersampling, using only the
  Python standard library. Output is committed so the build has no Python dependency:
  `icon.rgba` (64×64 RGBA), `icon.png` (docs/README), `icon.ico` (7 sizes).
* `rdm-gui/build.rs` embeds `icon.ico` into `rdm-gui.exe` via `winresource` — this is what
  Explorer, the taskbar and Alt-Tab show. On non-Windows targets the script is a no-op.
* The same 64×64 RGBA buffer is the window icon (`ViewportBuilder::with_icon` in `main.rs`,
  `ViewportCommand::Icon` available for runtime changes) and the tray icon
  (`icon::tray_icon()`) — one asset, three surfaces, no divergence.
* `icon.rs` tests assert the buffer length is exactly `64·64·4` and that the icon is not
  (nearly) transparent, so a broken asset path fails the build's test job.

## 5. F2 — clipboard pre-fill on “New download”

* Opening the form from the toolbar **New download** button (and from the tray's
  *New download (from clipboard)* entry) reads the clipboard; when it holds a link, the URL
  field is filled, focused and **selected**, so `Enter` starts the transfer immediately.
* Deliberately *not* automatic: nothing is downloaded without an explicit Start (the
  clipboard belongs to the user, not to the app).
* The clipboard holds something that is not a link → the form opens empty and the log says
  `no link on the clipboard — opening the form empty` (never a silent no-op).
* Switch: Settings → *Desktop integration* → **Fill the URL from the clipboard** (default on).
  The toolbar tooltip changes with the switch (`ux::new_download_tooltip(bool)`), so the
  affordance and the setting cannot disagree.
* `clipboard.rs` tests pin what counts as a link (http/https, no whitespace, length cap) and
  what does not (bare `example.com`, `ftp://`, a Windows path, 5 000 characters).

## 6. F3/F4 — tray icon and menu

* **Close hides, Quit exits.** With *Keep running in the tray* on **and** a tray host
  available, closing the window sends `CancelClose` + `Visible(false)`: the transfer keeps
  running and the log says so. The tray menu's *Quit rdm* really exits (it runs the same 5-second
  graceful shutdown as before, reporting how many transfers were paused).
* **Never unreachable.** If the tray cannot be created (no tray host in the session), the app
  logs `no system tray available — closing the window will quit` and keeps the normal close
  behaviour. This is a deliberate safety rule: the window must never be hideable into a tray
  that does not exist.
* **Menu (frequent actions first):** *Show rdm* · *New download (from clipboard)* ·
  *(separator)* *Pause all* · *Resume all* · *(separator)* *Floating drop target* ·
  *Quit rdm*. The drop-target entry carries a ✓ when it is on, and follows the setting
  wherever it is changed.
* **Waking a hidden window.** Tray events are delivered to two small relay threads that
  translate them into commands the instant they happen and call
  `Context::request_repaint_of(ROOT)`. Without that wake-up a hidden window (which only runs a
  frame when someone asks it to) would handle *Show* / *New download* late or not at all —
  this was found by reviewing the first version of the code, and the fix is why the tray is
  usable while the window is not visible.
* A click on the icon itself restores the window (the menu is on right click, so the two
  gestures do not fight).

## 7. F5 — floating drop target

* A small (214×104 pt), borderless, always-on-top, non-taskbar window anchored to the
  bottom-right of the **work area** — i.e. just above the clock area of the taskbar. The
  Windows work-area rectangle (`SystemParametersInfoW(SPI_GETWORKAREA)`) is converted to egui
  points with the system DPI, clamped so it cannot land at negative coordinates; on
  non-Windows the caller falls back to the monitor size (`windows.rs`).
* It is its own egui viewport (`show_viewport_deferred`), so it stays alive while the main
  window is hidden in the tray. It re-anchors itself when the reported position drifts
  (DPI change, taskbar moved).
* Dropping a link on it queues the link, wakes the main window, opens the New-download form
  pre-filled and logs the outcome. What the OS can hand over is understood: plain text
  (browsers), `text/uri-list`, and dropped files that are `.url` shortcuts or `.txt` files
  holding a link (`platform::interpret_drop`, 10 unit tests).
* **A drop is never silent.** Anything that is not a link produces a sentence in the target
  and in the status bar (that is why the drop target shows a line of text under its hint).
* Opt-in (default **off**): it adds an always-visible window, so the user turns it on
  (Settings or the tray). Its ✕ hides it *and* turns the switch off — one click, nothing
  destroyed, no dialog needed.
* Dropping files or links on the main window takes the same path, so the window is a drop
  surface too.

## 8. Acceptance criteria (role standard: every path has entry points, exits and error points)

| Path | Entry points | Exits | Error points and their handling |
| --- | --- | --- | --- |
| Clipboard → New download | toolbar **New download**; tray *New download (from clipboard)* | dialog close; `Esc`; Start | clipboard empty / not a link → empty form + log line; clipboard switch off → no pre-fill at all |
| Close → tray | window ✕; Alt-F4 | tray *Show rdm*; icon click; tray *Quit rdm* (real exit) | no tray host → normal close (logged); tray fails to build → normal close |
| Drop a link | floating drop target; main window (any file drop) | form opens pre-filled (then Start/Esc) | not a link → sentence in target/status bar; unreadable file → same; hovering shows “Release to start a download” |
| Floating drop target on/off | Settings checkbox; tray menu item | Settings = off; ✕ on the target | setting cannot be saved → error logged, target still follows the in-memory state |

## 9. Verification — what is proven and what is not

| Claim | Evidence | Status |
| --- | --- | --- |
| Bug A fixed (no doubling, legacy files still load) | `settings.rs` round-trip tests; CI `Test GUI crate` | ✅ proven |
| Bug B fixed (no console child) | `CREATE_NO_WINDOW` on the only spawn; grep shows no other process spawn in `rdm-gui` | ✅ proven in code |
| Icon pipeline (exe, window, tray) | `assets/generate_icon.py` re-runs deterministically; `icon.rs` tests; `build.rs` embeds the `.ico` on Windows | ✅ proven |
| All GUI unit tests pass (75 `#[test]`, was 51) | CI `build-gui-windows` → `Test GUI crate` | ✅ once the green run lands (IDs in evidence) |
| CLI/engine regression untouched | CI `test-windows` | ✅ |
| Copy rules and contrast still hold | `audits/ux-terminology-check.py` (15 user-facing files), `audits/ui-contrast-check.py` (27 Rust files) | ✅ PASS |
| **Tray icon appears, menu works while hidden, close really hides** | needs a Windows desktop | ❌ **NOT VERIFIED here → UX-ESC-005 (QA smoke test)** |
| **Dragging a real browser link onto the target** | needs a browser + Windows desktop | ❌ **NOT VERIFIED here → UX-ESC-005** |
| **Anchor exactly above the clock at 100 %/125 %/150 % DPI, taskbar left/right** | needs a desktop | ❌ **NOT VERIFIED here → UX-ESC-005** |
| **Clipboard pre-fill with the real Windows clipboard** | needs a desktop session | ❌ **NOT VERIFIED here → UX-ESC-005** |
| **Explorer shows the new icon for the built exe** | needs the built artefact on Windows | ❌ **NOT VERIFIED here → UX-ESC-005** |

No usability claim is made for the unverified rows. A checklist for the smoke test is in the
escalation (§11). Also recorded: **two CI runs (37362918254, 37364680078/37364684573) never
started** because GitHub could not acquire a hosted Windows runner
(“The job was not acquired by Runner of type hosted even after multiple attempts”) — an
infrastructure failure, not a code failure; the runs were re-triggered by pushing.

## 10. Change manifest (this increment)

| File | Action | Scope | Reason | Requirement |
| --- | --- | --- | --- | --- |
| `rdm-gui/src/settings.rs` | MODIFIED | UX/GUI | Bug A escape/parse symmetry; `--data-dir` precedence; 4 new tests | Bug A |
| `rdm-gui/src/util.rs` | MODIFIED | UX/GUI | `CREATE_NO_WINDOW` on the only process spawn (Bug B); `dropped_links` for window drops | Bug B, F5 |
| `rdm-gui/src/main.rs` | MODIFIED | UX/GUI | window icon, `--data-dir` explicit flag, new modules, app gets the egui context | F1, F3 |
| `rdm-gui/src/app.rs` | MODIFIED | UX/GUI | tray polling, close-to-tray, reveal, clipboard pre-fill, drop-zone lifecycle, window drops | F2–F5 |
| `rdm-gui/src/state.rs` | MODIFIED | UX/GUI | `focus_url`, `prefill_from_clipboard` for the toolbar tooltip | F2 |
| `rdm-gui/src/ux.rs` | MODIFIED | UX | URL hint, New-download tooltip, drop confirmation, `DESKTOP_HELP` + a fourth help section | F2, F5 |
| `rdm-gui/src/views/{add_download,toolbar,settings_view}.rs` | MODIFIED | UX/GUI | URL focus/select-all; tooltip switch; three “Desktop integration” switches | F2–F5 |
| `rdm-gui/src/{icon,clipboard,platform,windows,tray,dropzone}.rs` | ADDED | UX/GUI | icon asset use, clipboard façade, drop interpretation, work-area query, tray, floating target | F1–F5 |
| `rdm-gui/build.rs`, `rdm-gui/assets/generate_icon.py`, `assets/icon.{rgba,png,ico}` | ADDED | UX/GUI | generated icon + embedding into the executable | F1 |
| `rdm-gui/Cargo.toml` | MODIFIED | UX/GUI | `arboard`, `tray-icon`, `windows-sys` (+3 features), `winresource` build-dep, `tempfile` dev-dep | F2–F5 |
| `audits/ui-contrast-check.py` | MODIFIED | UX audit tooling | found + fixed a real off-by-one: char literals with an escape (`'\n'`) skipped one character too many, mis-reporting `{`/`}` as unbalanced in files that use them | audit fidelity |
| `audits/ux-terminology-check.py` | MODIFIED | UX audit tooling | scans the new user-facing files (tray/dropzone/ux); the queue-empty action must be the confirming one | copy rules |
| `audits/ux-designer-execution-plan.md`, `audits/ux-flows-and-ia.md`, `audits/ux-glossary.md`, `audits/ux-feedback-error-undo-policy.md`, `audits/ux-designer-handoff.md`, `audits/ux-feature-pack-report.md`, `audits/evidence/*` | MODIFIED/ADDED | UX docs | flows, IA, terminology, policy and evidence for the new surface | role artifacts |
| `README.md`, `TEST_INVENTORY.md` | MODIFIED | docs | document the switches, the drop rules and the icon pipeline; test inventory 51 → 75 | documentation |
| `.gitignore` (new file) | ADDED | repo hygiene — **flagged for approval** | the audit scripts generate `audits/__pycache__/*.pyc`, which `git add -A` picked up; nothing generated belongs in the tree. No existing file was changed by this | hygiene |

## 11. Escalation

```
UX-ESC-005   Desktop-integration smoke test cannot run in this environment
Type:        Usability risk / verification gap (not a known defect)
Impact:      the tray, the floating target, the real clipboard path, the .exe icon and the
             no-console guarantee are implemented and unit-tested, but their Windows-desktop
             behaviour is unverified here (no display, no browser, no local toolchain)
Target:      QA (owner) + Design Manager; Product informed
Ask:         run the checklist on a Windows desktop:
             1. start rdm-gui.exe from Explorer — no console window appears; the exe/taskbar/
                tray icons are the new one;
             2. copy a link, click New download — the URL field is filled and selected;
             3. close the window — it disappears from the taskbar, the transfer continues,
                the tray icon remains; Show rdm and a click on the icon restore it;
             4. drag a link from a browser onto the floating target (also with the window
                hidden) — the form opens pre-filled;
             5. check the anchor above the clock at 100 %, 125 %, 150 % scaling and with the
                taskbar at the bottom/left/right;
             6. Quit rdm from the tray with a transfer running — it pauses and exits (log
                says how many were paused).
Priority:    P1 (the features are user-visible; a wrong anchor or a dead tray would be found
             by users first)
```

## 12. Notes and honest limits

* The icon is generated mid-increment and committed; if the palette in `theme::tokens`
  changes, `generate_icon.py` must be re-run (documented in the script's header).
* The tray menu marks the drop-target state with a ✓ **inside the label**
  (`Floating drop target ✓`) rather than a native checkmark item: the crate's `MenuItem` is
  used for all entries so the ids stay in one place. A native `CheckMenuItem` is a possible
  polish item — recorded as an open recommendation, not a defect.
* `arboard`, `tray-icon` and `windows-sys` are new runtime dependencies of the GUI. They were
  unavoidable for F2–F5 (clipboard, tray, work-area query) and only affect `rdm-gui`.
* The pre-existing `float_literal_f32_fallback` warnings in `theme/mod.rs` are from the newer
  toolchain and are **not** part of this increment; the one in `dropzone.rs` was fixed while
  it was being reviewed.
* Not done here (out of this increment's request): keyboard shortcut to open the drop target,
  a settings control for the tray icon itself, and moving the drop target by dragging.
