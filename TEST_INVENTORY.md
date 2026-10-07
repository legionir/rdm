# Test Inventory — rdm (CLI + GUI)

## End-to-end engine (tests/integration.rs) — 8 tests
- segmented_download_matches_payload
- resume_after_premature_disconnect
- pause_preserves_chunks_and_resume_continues
- single_stream_fallback_when_server_has_no_ranges
- checksum_verification
- zero_byte_file
- pause_resume_mid_transfer_preserves_content (real mid-transfer pause via the control channel + resume with dynamic splits; guards the append-on-resume and merge-order fixes)
- split_chunks_merge_in_byte_order (dynamic split via a stalled first chunk; assembly must order chunks by byte range)

## Library unit (src/) — 18 tests
- src/network/range.rs: 8 (plan, classify, header, range, chunk, +2 new)
- src/storage/database.rs: 2
- src/utils/human.rs: 3
- src/utils/path.rs: 4
- src/utils/rate.rs: 1
- src/filesystem/merger.rs: 0 (tests embedded elsewhere)

## CLI tests
- src/cli/commands.rs: 7 new (parse_checksum ×3, opts_parse ×3, connections_range)
- tests/cli_real.rs: 2 new (help/version, bad-url graceful)

## GUI tests (rdm-gui/src/) — 119 tests
- app.rs: 3 (transparent clear colour for the transparent viewports; the tray hide path
  never blanks **or minimizes** the window — source guard against `ViewportCommand::Visible(false)`
  and `ViewportCommand::Minimized(true)`; the repaint heartbeat is short enough to feel instant,
  and slow enough in the tray to sit well inside the liveness window)
- backend.rs: 10
- logging.rs: 2
- settings.rs: 9 (4 + 5 for the path work: five save/load cycles of `C:\download\rdm`,
  a legacy doubled value preserved with no further growth, the new switches'
  defaults/parsing, `--data-dir` precedence)
- util.rs: 4 (3 + dropped files/links → `DropReport`)
- platform.rs: 10 (import bus, drop interpretation: text, uri-list, `.url`/`.txt` files,
  non-links never silent)
- clipboard.rs: 2 (what counts as a link / what does not)
- dropzone.rs: 8 (round 6: the mark is a small square window between 40 and 64 pt whose circle
  fills 88% of it, with the app mark inset from the ring; the logo mesh is exactly one quad per
  grid cell of the embedded asset, never leaves its rect, keeps the asset's transparent corners
  and its blue/white, and can never mix the channels up; **the drop policy is a pure function** —
  a browser's link read from the text, a Qt/GTK link read from the uri-list, a `.url` file's link
  from inside the file, a file that holds none answered with a sentence (never with a silent
  clipboard import), and the clipboard only as the announced last resort, with an empty one still
  explained; drain and activation are one-shot)
- oledrop.rs: 5 (Windows-only COM drop target: the vtable layouts are the documented 3/7/6 slots
  and `Target::interface` sits at offset 0 — a wrong layout is a crash, not a bug report; the two
  IIDs are the documented ones and differ; UTF-16 text stops at its terminator; whitespace-only
  text counts as nothing; a payload is “readable” only when something survived — with the format
  ids for files, text, `text/uri-list` and the URL spellings all drawn from the same constants
  the target asks for)
- icon.rs: 2 (the embedded 64×64 RGBA buffer is complete and not transparent; window icon size)
- windows.rs: 4 (the work-area anchor is either absent or on screen; an unknown window handle
  is a refusal, not a guess; **hiding never minimizes** — no `SW_MINIMIZE` anywhere, comments
  stripped; only the main window’s title wins the fallback search, and the drop target now says
  who it is in one constant — `DROP_TARGET_TITLE` ranks 0 like `rdm — Settings`/`Help`, and the
  two searches can never trade places)
- frames.rs: 2 (a fresh frame is never stale, an old one is; the liveness window is
  comfortably longer than both heartbeats — 250 ms on screen, 1 s in the tray)
- tray.rs: 7 (every menu id maps to its command and nothing else does; only `Show` /
  `New download` need the window — the list is a contract; the inbox keeps order and is
  handed over exactly once; the Quit deadline outlives the graceful shutdown and stays
  inside what a user will wait; the drop-target item says its state in words, no ✓ glyph;
  **a right click opens the menu and nothing else** — right/down, right/up and the right
  double click never restore the window, which is what used to dismiss that menu;
  only a *finished* left click (or the left double click) is `Show`, so one click is one
  command)
- logging.rs: 4 (level splitting, buffer order/drain, every line also lands in the log file,
  a missing data directory disables the file without failing)
- theme/tokens.rs: 5 (state/chunk/log coverage, state distinguishability, zebra tint, breakpoints)
- theme/contrast.rs: 5 (WCAG maths, contract in both themes, worst-pair margin, focus ring, name resolution)
- theme/icons.rs: 3 (drawn glyph geometry: closed shapes, stroke inside the rect, state mapping)
- theme/mod.rs: 6 (design guards: no colour literals in views, every view uses `theme::`,
  sidebar cap, theme reversibility, and the control-height contract — button, icon button,
  text input, combo box and labelled button all measure `Spacing::control_height` (24 pt);
  a window panel insets its content by `Spacing::window_padding`, measured in a real
  layout pass — the Help/Settings “content glued to the edges” fix)
- theme/components.rs: 4 (row-background precedence, banner severities; the labelled
  button's geometry — the icon keeps `Spacing::icon_gap` from the label for every label
  width, and icon and label sit on the control's midline)
- views/download_list.rs: 4 (responsive column fitting, optional-column drop order, width invariant)
- views/help_overlay.rs: 2 (the help window closes through one flag; its sections stay in sync
  with the glossary)
- views/settings_view.rs: 6 (the window's tabs keep their state across frames, the save/reload
  footer reports what changed, the folder pickers match the text inputs)
- ux.rs: 14 (UX policy: confirmation scope and consequence copy, safe-option naming, bulk
  count/zero-case copy, drop-all threshold, resume-all outcome naming failed downloads,
  state legend completeness, help sections — now four, including the desktop-integration
  section — Failed/Completed never offer Resume, remove-while-running explanation,
  jargon-free copy, glossary synonyms, the doubled-separator hint for legacy values —
  which must never fire for a UNC share or an extended-length path)
- backend.rs: 10
- logging.rs: 2
- settings.rs: 9 (4 + 5 for the path work: five save/load cycles of `C:\download\rdm`,
  a legacy doubled value preserved with no further growth, the new switches'
  defaults/parsing, `--data-dir` precedence)
- util.rs: 4 (3 + dropped files/links → `DropReport`)
- platform.rs: 10 (import bus, drop interpretation: text, uri-list, `.url`/`.txt` files,
  non-links never silent)
- clipboard.rs: 2 (what counts as a link / what does not)
- dropzone.rs: 8 (round 6: the mark is a small square window between 40 and 64 pt whose circle
  fills 88% of it, with the app mark inset from the ring; the logo mesh is exactly one quad per
  grid cell of the embedded asset, never leaves its rect, keeps the asset's transparent corners
  and its blue/white, and can never mix the channels up; **the drop policy is a pure function** —
  a browser's link read from the text, a Qt/GTK link read from the uri-list, a `.url` file's link
  from inside the file, a file that holds none answered with a sentence (never with a silent
  clipboard import), and the clipboard only as the announced last resort, with an empty one still
  explained; drain and activation are one-shot)
- oledrop.rs: 5 (Windows-only COM drop target: the vtable layouts are the documented 3/7/6 slots
  and `Target::interface` sits at offset 0 — a wrong layout is a crash, not a bug report; the two
  IIDs are the documented ones and differ; UTF-16 text stops at its terminator; whitespace-only
  text counts as nothing; a payload is “readable” only when something survived — with the format
  ids for files, text, `text/uri-list` and the URL spellings all drawn from the same constants
  the target asks for)
- icon.rs: 2 (the embedded 64×64 RGBA buffer is complete and not transparent; window icon size)
- windows.rs: 1 (the work-area anchor is either absent or on screen)
- theme/tokens.rs: 5 (state/chunk/log coverage, state distinguishability, zebra tint, breakpoints)
- theme/contrast.rs: 5 (WCAG maths, contract in both themes, worst-pair margin, focus ring, name resolution)
- theme/mod.rs: 4 (design guards: no colour literals in views, every view uses `theme::`, sidebar cap, theme reversibility)
- theme/components.rs: 4 (row-background precedence, banner severities; the labelled
  button's geometry — the icon keeps `Spacing::icon_gap` from the label for every label
  width, and icon and label sit on the control's midline)
- views/download_list.rs: 4 (responsive column fitting, optional-column drop order, width invariant)
- ux.rs: 14 (UX policy: confirmation scope and consequence copy, safe-option naming, bulk
  count/zero-case copy, drop-all threshold, resume-all outcome naming failed downloads,
  state legend completeness, help sections — now four, including the desktop-integration
  section — Failed/Completed never offer Resume, remove-while-running explanation,
  jargon-free copy, glossary synonyms, the doubled-separator hint for legacy values —
  which must never fire for a UNC share or an extended-length path)

Design layer: `rdm-gui/src/theme/` is the single source of truth for colour,
spacing, radii, typography, sizes and breakpoints; the column-fitting tests
were extended and are kept green by the same CI job.

UX layer: `rdm-gui/src/ux.rs` is the single source of truth for confirmation
policy, consequence copy, the state legend, bulk-action microcopy and the
glossary; `audits/ux-terminology-check.py` enforces the same rules statically
over CLI help, the GUI strings and the README.

Round 5 (the tray right-click, the window padding and the icon/label gap) keeps the
same shape: the tray decision is a pure function (`command_for_icon_event`), the button
geometry is a pure function (`icon_text_layout`) and the window padding is measured in a
real `Context` layout pass, so all three are ordinary unit tests rather than a screenshot
review.

Round 6 (the mark is the app logo in a small circle, no text — and a dropped link now
actually reaches the app) keeps it too. The two halves of “dropping a link did nothing”
are covered separately, because they are different failures: the *policy* (`resolve_drop`)
is a pure function tested with every payload a drag can carry, and the *transport*
(`oledrop`) is pinned by the COM layout tests — the vtable shapes, the IIDs and the offset
of the interface inside the object — since the reason a browser's link never arrived was
that the drop handler answered `DROPEFFECT_NONE` for it, which no screenshot could show.
The mark's drawing is likewise a pure function (`logo_mesh`), so “is it the app's own
logo, with nothing written on it” is a test and not a hope.

## CI (after ci/ci-tests.patch)
- test-windows (cargo test --all-targets) — the CLI + engine suites
- build-gui-windows: `cargo build --release`, then `cargo test --release` in `rdm-gui`,
  then the staged artifact `rdm-gui-windows-x86_64.exe`

Last verified counts, Windows runner: `test-windows` ✓ and `build-gui-windows` ✓ on run
**37553244501** (`a859ebc`, 2026-10-07, plus the `pull_request` twin 37553248764) — the GUI job
built the crate, ran the **119** cases listed above (0 failed: the *Surface test failures* step
stayed skipped) and uploaded the binary (5,821,355 bytes); the `release` job skipped itself,
as it only runs for `v*` tags. Round 6 needed three attempts to get there, each caught by this
job and each recorded in the run list: a `windows-sys` path that does not exist
(37551901919), a test target that needed `Guid: Debug` (37552333201), and two test failures
that were real — the COM object missing `#[repr(C)]` and a wrapping `u8` comparison in the
colour test (37552733634). Before that, 109 cases were green on **37546378552** (`ce38af6`),
104 on **37535678990** (`668166d`) and 99 on **37522424155** (`8b03d25`); every round and its
cause is recorded in `audits/evidence/ux-feature-pack-ci-runs.json`.
