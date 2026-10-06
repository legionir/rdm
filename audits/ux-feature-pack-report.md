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

* `escape()` now escapes only what `parse()` understands: `\n`, `\t`, `"`. A stray
  backslash — the normal case for a Windows path — is written verbatim, so `C:\download\rdm` is
  stored and read back unchanged. **The doubling stops.**
* `download_dir = D:\iso\files` and `--data-dir` values stay byte-identical through any
  number of save/load cycles.
* **Values already doubled by the old version are preserved, not rewritten.** In a saved
  value the pair `\\` cannot be told apart from a legitimate UNC share (`\\server\share`) or
  an extended-length path (`\\?\C:\…`); an automatic repair could corrupt those, and
  rewriting a user's saved data without asking is not acceptable. Windows treats duplicate
  separators as one, so such a path still resolves to the same folder. What changed is that
  the window now *says* it: a hint under *Download directory* and *Metadata directory* in
  Settings — “Doubled separators — written by an older rdm build. Windows normally treats
  them as one; pick the folder again to clean it up.”
  (`ux::DOUBLED_SEPARATOR_HINT` + `ux::doubled_separator_hint`, unit-tested; it never fires
  for a UNC share or an extended-length path).

**Evidence.** `rdm-gui/src/settings.rs`:
`windows_paths_survive_any_number_of_round_trips` (five save/load cycles of
`C:\download\rdm`; the value is unchanged and no doubled pair appears after any round) and
`a_legacy_doubled_path_is_preserved_and_no_longer_grows` (a file written by the buggy
version: the value is kept verbatim, a save does not double it again, and the Settings hint
fires). `rdm-gui/src/ux.rs::the_legacy_separator_hint_fires_for_corrupted_values_and_never_for_unc`
pins the hint rule. CI job `build-gui-windows` → `Test GUI crate` (run IDs in
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
| Bug A fixed (no doubling on any save/load; a legacy doubled value is preserved, explained in Settings, and never grows further) | `settings.rs` round-trip + legacy tests, `ux.rs` hint test; CI `Test GUI crate` | ✅ proven |
| Bug B fixed (no console child) | `CREATE_NO_WINDOW` on the only spawn; grep shows no other process spawn in `rdm-gui` | ✅ proven in code |
| Icon pipeline (exe, window, tray) | `assets/generate_icon.py` re-runs deterministically; `icon.rs` tests; `build.rs` embeds the `.ico` on Windows | ✅ proven |
| All GUI unit tests pass (75 `#[test]`, was 51) | CI run 37369744644 (`b478f4b`): job `build-gui-windows` → **Build GUI binary ✓ · Test GUI crate ✓ · Stage GUI binary ✓ · Upload GUI artifact ✓** (the exe, icon embedded, was built and uploaded) | ✅ **proven** |
| CLI/engine regression untouched | CI run 37369744644 → job `test-windows` ✓ | ✅ **proven** |
| Copy rules and contrast still hold | `audits/ux-terminology-check.py` (15 user-facing files), `audits/ui-contrast-check.py` (27 Rust files) | ✅ PASS |
| **Tray icon appears, menu works while hidden, close really hides** | needs a Windows desktop | ❌ **NOT VERIFIED here → UX-ESC-005 (QA smoke test)** |
| **Dragging a real browser link onto the target** | needs a browser + Windows desktop | ❌ **NOT VERIFIED here → UX-ESC-005** |
| **Anchor exactly above the clock at 100 %/125 %/150 % DPI, taskbar left/right** | needs a desktop | ❌ **NOT VERIFIED here → UX-ESC-005** |
| **Clipboard pre-fill with the real Windows clipboard** | needs a desktop session | ❌ **NOT VERIFIED here → UX-ESC-005** |
| **Explorer shows the new icon for the built exe** | needs the built artefact on Windows | ❌ **NOT VERIFIED here → UX-ESC-005** |

No usability claim is made for the unverified rows. A checklist for the smoke test is in the
escalation (§11).

**CI availability, recorded honestly.** Four runs in this increment never reached a runner:

| Runs | sha | What happened |
| --- | --- | --- |
| 37362918254, 37364680078 / 37364684573, 37368129718 / 37368133806 | `eeb9880`, `3e0cbfb`, `680d5f3` | “The job was not acquired by Runner of type hosted even after multiple attempts” — the `build-gui-windows` job queued for 12–15 min and was cancelled without running a single step. The `test-windows` job (CLI/engine) passed in the same runs, and run 37366518834 did reach a GUI runner and compiled the **whole** feature pack (tray, drop target, icon build script) — its only error was the duplicate test module fixed in `680d5f3`. |

That is an infrastructure failure on the GitHub side, not a code failure. It resolved on the
push run **37369744644** (`b478f4b`) — `build-gui-windows` ✓ (build · 75 GUI tests · stage ·
upload) and `test-windows` ✓ — while the `pull_request` run for the *same* commit
(37369748949) was still stuck in the queue. The workflow file is out of scope (`UX-ESC-003`) and
was not touched to work around the queue; runs are simply re-triggered by pushing.

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
* The pre-existing `float_literal_f32_fallback` warnings (`theme/mod.rs:96,101,102`,
  `views/download_list.rs:223`) come from the toolchain the runner uses and are **not** part of
  this increment; the one in `dropzone.rs` was fixed while it was being reviewed. They are
  future-incompatible (“will become a hard error”), so the owning increment should pick them
  up — recorded here rather than changed silently: **REC-UX-006** (owner: UI increment).
* The same warning class in `theme/mod.rs:19` (`state_glyph` imported but unused) is also
  pre-existing and left alone for the same reason.
* Not done here (out of this increment's request): keyboard shortcut to open the drop target,
  a settings control for the tray icon itself, and moving the drop target by dragging.
* **Correction (2026-10-06).** An earlier draft of §2 and the commit message of `472414b`
  claimed the loader decoded a doubled backslash back into a single one for files from the
  buggy version. The code never did that: it preserves the stored value. The false sentence
  is removed, the real behaviour is documented above and pinned by
  `settings.rs::a_legacy_doubled_path_is_preserved_and_no_longer_grows`, and the user-facing
  consequence (a hint in Settings) was added in `cf7c211`+ instead of leaving a silent
  surprise.

## 13. Round 3 (2026-10-06) — the tray freeze and the standard control heights

### 13.1 What was reported

| # | Report (intent) | Class |
| --- | --- | --- |
| R1 | the footer **Clear/Copy** buttons are the wrong size | defect — layout |
| R2 | no ✕ close icon renders correctly; the Clear / ⓘ Help / ↑↓ glyphs show as empty boxes | defect — icons/fonts |
| R3 | the toolbar row (search field, Clear, status filter, Help) must share one height | requirement |
| R4 | the Settings folder-picker button must be as tall as its input | defect — layout |
| R5 | Settings must become a **separate window with tabs**, and Settings + Help must carry a corner ✕ “like real windows” — not modal-like | requirement |
| R6 | standardise the height of text inputs, dropdowns, buttons — everything | requirement |
| R7 | **tray freeze**: with the app in the tray no menu item works and the window never comes back | defect — blocker |
| R8 | the floating target must be a **small circle with only the logo**, no explanation text, and must accept a dropped link **even while the app is in the tray** | requirement |

### 13.2 R7 — root cause (verified against the sources, not guessed)

`app.rs` hid the window on close with `CancelClose` + `ViewportCommand::Visible(false)`. That
clears `WS_VISIBLE`; **Windows sends no `WM_PAINT` to a window that is not visible**, and eframe
paints only on `RedrawRequested`. Tray commands were read inside `App::update`, so after the hide
the app never ran another frame: the menu was dead, and the very command that should bring the
window back was waiting for a frame that could never come. Minimizing is no escape either —
nothing visible means nothing painted (MSDN, *Drawing a Minimized Window*; winit 0.30 keeps its
own helper window `WS_VISIBLE` with the layered style for exactly this reason).

### 13.3 The fix (three layers)

1. **Hide the way winit hides its helper window.** The window keeps `WS_VISIBLE` and is taken off
   the screen with the layered style — alpha 0, `WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW`,
   `WS_EX_APPWINDOW` removed — so it is invisible, click-through, out of the taskbar and out of
   Alt-Tab while still receiving paints. `windows::reveal_main_window` restores it from any thread.
2. **The tray no longer waits for a frame.** Two relay threads block on the menu and icon
   channels, queue the command and wake the UI. The commands that are *about* the window — and
   any command at all when `frames::is_stale()` (≈1 s) says the UI has stopped delivering frames —
   restore the window through the OS. `poll()` remains the in-frame fallback.
3. **A transparent clear colour** for every viewport, so the floating target is a mark on the
   desktop instead of a dark square, and the frame clock is noted at the top of `update()`.

### 13.4 R1/R3/R4/R6 — one control height, measured

One token (`Spacing::control_height`, 24 pt) and one test that runs the real layout pass: button,
icon button, text input, combo box and labelled button all measure **24.00 pt**. That took one
real fix and one measurement fix: `components::text_edit` now pins the height with
`.min_size(0, control_height)`, and the test reads the rect the **control occupies** — egui 0.29's
`TextEdit` deliberately shrinks its own reply to the inner text rect
(`text_edit/builder.rs:416`, “TODO: return full outer_rect”), which is what the earlier red run
was measuring at 16 pt. The quirk is documented on `components::text_edit` so no caller aligns to
the wrong rect again.

### 13.5 R2/R5/R8 — carried by the earlier steps

* **R2** — the glyphs are *drawn* (`theme::icons`), not font codepoints; the audit bans the
  codepoints that produced the boxes (PH-2/PH-3).
* **R5** — Settings and Help are deferred, decorated viewports with their own ✕; Settings has tabs
  and a Save/Reload footer (`6a64ead`, extended in `ee8a8ab` / `a783a46`).
* **R8** — the target is a 76 pt transparent circle carrying only the rdm mark; a drop reports its
  outcome in the status bar, opens the pre-filled form and reveals the window (PH-7.6).

### 13.6 Verification, and what is still missing

| Gate | Result | Evidence |
| --- | --- | --- |
| The tray fix compiles on Windows | PASS | 37520509055/37520515850 → 37521061557/37521068081 (two annotation-driven rounds) |
| Every GUI test (99) + the CLI/engine suites | PASS | runs **37522424155** / **37522433273** (`8b03d25`): `test-windows` ✓, `build-gui-windows` ✓ |
| The control-height contract | PASS | same run; the red run 37521816137 carries the message that named the text input |
| Tray round trip, and a real drag onto the target | **NOT RUN — NOT_APPLICABLE in this environment** | `UX-ESC-005` (QA); artifact `rdm-gui-windows-x86_64.exe` (5,808,400 bytes) is the hand-off |

### 13.7 Change manifest (round 3)

| File | Action | Scope | Reason | Requirement | Test status | Evidence |
| --- | --- | --- | --- | --- | --- | --- |
| `rdm-gui/src/windows.rs` | changed | UX / desktop integration | layered hide, OS-level reveal, handle capture | R7 | 2 unit tests + Windows compile | run 37522424155 |
| `rdm-gui/src/tray.rs` | rewritten | UX / desktop integration | blocking relay threads, command mapping, window escape hatch | R7 | 4 unit tests | run 37522424155 |
| `rdm-gui/src/frames.rs` | **new** | UX / desktop integration | frame-liveness clock the relay threads consult | R7 | 2 unit tests | run 37522424155 |
| `rdm-gui/src/app.rs` | changed | UX / desktop integration | transparent clear colour, frame clock, hide/reveal routing, regression guard | R7 | 3 unit tests | run 37522424155 |
| `rdm-gui/src/main.rs` | changed | UX / desktop integration | remember the window handle before the first frame | R7 | Windows compile | run 37522424155 |
| `rdm-gui/Cargo.toml` | changed | build (GUI only) | `raw-window-handle = "0.6"` — Windows-only use, already in the tree through `winit` | R7 | Windows compile | run 37522424155 |
| `rdm-gui/src/theme/components.rs` | changed | UX | the text input pins the shared control height | R1/R3/R6 | the layout test | run 37522424155 |
| `rdm-gui/src/theme/mod.rs` | changed | UX | the height test measures the control rect and reports the numbers | R3/R6 | the test itself | 37521816137 → 37522424155 |
| `TEST_INVENTORY.md` | changed | docs | 77 → 99 GUI tests, CI counts and evidence | — | n/a | this file |
| `audits/ux-designer-execution-plan.md` | changed | docs | PH-8, D-17…D-22, verification rows | — | n/a | this file |
| `audits/ux-feature-pack-report.md` | changed | docs | this section | — | n/a | this file |
| `audits/evidence/ux-feature-pack-ci-runs.json` | changed | docs | round-3 run ledger | — | n/a | run ids above |

Nothing outside the UX scope was touched: `.github/workflows/build.yml` is unchanged
(`UX-ESC-003` still stands for the annotation steps) and no engine, storage or API code moved.

## 14. Round 4 (2026-10-07) — the tray freeze, second attempt: what the first fix got wrong

### 14.1 The report

The product owner re-reported the identical symptom after `df2a4eb` (green CI): “when the app goes
to the tray none of its menu items work; it is as if it has hung completely, and it has to be ended
from the Task Manager”. PH-8 had fixed the *hide* call but left the freeze in place.

### 14.2 What was wrong with the first fix (honest accounting)

`windows::hide_main_window` kept `WS_VISIBLE` (correct) **and then called `SW_MINIMIZE`** “to give
the keyboard back”. A minimized window has nothing on screen to paint, so `WM_PAINT` stops —
exactly the state the function was written to avoid. The menu still opened, because the OS draws
it on the app’s message pump; the commands were read inside `App::update`, which never ran again.
Round 3’s *cause* was understood correctly and the *remedy* smuggled the same trap back in. CI
cannot see this: it compiles and tests, it does not sit in a notification area. The missing gate
was always PH-8.11 (desktop smoke test), and it is still open.

There was a second, independent weakness: the relay threads only **queued** the commands. With no
frames, a queue is read by nobody — including the command that would have restored the frames.

### 14.3 The fix

| Layer | Behaviour |
| --- | --- |
| hide | Alpha 0, click-through, tool-window, `WS_EX_NOACTIVATE`, z-order to the bottom — **never minimized, `WS_VISIBLE` never cleared**. The window stays “on screen” to Windows, so it keeps painting; if it ever stops, nothing below depends on it |
| tray relay | *Show* and *New download* restore the window through the OS from the relay thread itself; when `frames::is_stale()` (2 s) says the UI stopped delivering frames, *every* command gets that rescue; the state-dependent commands (*Pause all*, *Resume all*, the drop-target toggle) are still applied in the app |
| Quit | Restores the window **and arms a deadline** (`FORCE_QUIT_MS` = 8 s > the 5 s graceful shutdown): if the app has not exited by then, the process ends itself and logs why. The tray’s *Quit* is always true to its name, so “end it from the Task Manager” is never required again |
| handle | Captured twice (start-up context, first frame) and, failing both, searched for among this process’s own unowned top-level windows by title (`RDM` vs `rdm — …`), validated with `IsWindow` before every use |
| diagnosis | Every line also appended to `rdm-gui.log` in the data directory, one line per open; the relays log each command and whether they armed; the hide decision logs its result |

### 14.4 Verification, and the honest gap

| Gate | Result | Evidence |
| --- | --- | --- |
| Windows compile + 104 GUI tests | PASS | run **37535678990** (`668166d`), also the `pull_request` twin 37535686161 |
| Guards against the whole trap class | PASS | no `SW_MINIMIZE` / `Minimized(true)` / `Visible(false)` in the tray path (comments stripped), hiding never minimizes, only the main title wins the window search, the Quit deadline outlives the graceful shutdown |
| The tray on a real desktop, with the log file returned as evidence | **NOT RUN — NOT_APPLICABLE in this environment** | `UX-ESC-005`; artifact `rdm-gui-windows-x86_64.exe` (5,819,368 bytes) from run 37535678990 |

### 14.5 Change manifest (round 4)

| File | Action | Scope | Reason | Requirement | Test status | Evidence |
| --- | --- | --- | --- | --- | --- | --- |
| `rdm-gui/src/windows.rs` | changed | UX / desktop integration | no minimize, no blanking; window search; validated handle; style restore on refusal | R7 | 4 unit tests | run 37535678990 |
| `rdm-gui/src/tray.rs` | changed | UX / desktop integration | relays act (reveal) instead of only queueing; Quit deadline; logging of every command and of whether the relays armed | R7 | 5 unit tests | run 37535678990 |
| `rdm-gui/src/app.rs` | changed | UX / desktop integration | handle capture from the frame loop, hidden heartbeat, diagnosis lines | R7 | 3 unit tests | run 37535678990 |
| `rdm-gui/src/frames.rs` | changed | UX / desktop integration | liveness window 1 s → 2 s with the slower hidden heartbeat | R7 | 2 unit tests | run 37535678990 |
| `rdm-gui/src/logging.rs` | changed | UX (diagnostics) | `rdm-gui.log` in the data directory, every captured line | R7 | 4 unit tests | run 37535678990 |
| `rdm-gui/src/main.rs` | changed | UX (diagnostics) | pass the data directory to the logger | R7 | Windows compile | run 37535678990 |

Nothing outside the UX scope moved; the workflow file is untouched (`UX-ESC-003`).
