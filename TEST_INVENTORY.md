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

## GUI tests (rdm-gui/src/) — 99 tests
- app.rs: 3 (transparent clear colour for the transparent viewports; the tray hide path
  never blanks the window — source guard against `ViewportCommand::Visible(false)`;
  the repaint heartbeat is short enough to feel instant)
- backend.rs: 10
- logging.rs: 2
- settings.rs: 9 (4 + 5 for the path work: five save/load cycles of `C:\download\rdm`,
  a legacy doubled value preserved with no further growth, the new switches'
  defaults/parsing, `--data-dir` precedence)
- util.rs: 4 (3 + dropped files/links → `DropReport`)
- platform.rs: 10 (import bus, drop interpretation: text, uri-list, `.url`/`.txt` files,
  non-links never silent)
- clipboard.rs: 2 (what counts as a link / what does not)
- dropzone.rs: 3 (target geometry, one-shot ✕/activation flags, uri-list from dropped paths)
- icon.rs: 2 (the embedded 64×64 RGBA buffer is complete and not transparent; window icon size)
- windows.rs: 2 (the work-area anchor is either absent or on screen; an unknown window handle
  is a refusal, not a guess — the tray must not pretend to restore a window it cannot)
- frames.rs: 2 (a fresh frame is never stale, an old one is; the liveness window is
  comfortably longer than the 250 ms heartbeat the app asks for)
- tray.rs: 4 (every menu id maps to its command and nothing else does; only `Show` /
  `New download` need the window — the list is a contract; the inbox keeps order and is
  handed over exactly once; the drop-target item says its state in words, no ✓ glyph)
- theme/tokens.rs: 5 (state/chunk/log coverage, state distinguishability, zebra tint, breakpoints)
- theme/contrast.rs: 5 (WCAG maths, contract in both themes, worst-pair margin, focus ring, name resolution)
- theme/icons.rs: 3 (drawn glyph geometry: closed shapes, stroke inside the rect, state mapping)
- theme/mod.rs: 5 (design guards: no colour literals in views, every view uses `theme::`,
  sidebar cap, theme reversibility, and the control-height contract — button, icon button,
  text input, combo box and labelled button all measure `Spacing::control_height` (24 pt))
- theme/components.rs: 2 (row-background precedence, banner severities)
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
- dropzone.rs: 3 (target geometry, one-shot ✕/activation flags, uri-list from dropped paths)
- icon.rs: 2 (the embedded 64×64 RGBA buffer is complete and not transparent; window icon size)
- windows.rs: 1 (the work-area anchor is either absent or on screen)
- theme/tokens.rs: 5 (state/chunk/log coverage, state distinguishability, zebra tint, breakpoints)
- theme/contrast.rs: 5 (WCAG maths, contract in both themes, worst-pair margin, focus ring, name resolution)
- theme/mod.rs: 4 (design guards: no colour literals in views, every view uses `theme::`, sidebar cap, theme reversibility)
- theme/components.rs: 2 (row-background precedence, banner severities)
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

## CI (after ci/ci-tests.patch)
- test-windows (cargo test --all-targets) — the CLI + engine suites
- build-gui-windows: `cargo build --release`, then `cargo test --release` in `rdm-gui`,
  then the staged artifact `rdm-gui-windows-x86_64.exe`

Last verified counts, Windows runner: `test-windows` ✓ and `build-gui-windows` ✓ on run
**37522424155** (`8b03d25`, 2026-10-06) — the GUI job executed the 99 cases listed above
(0 failed) and uploaded the binary (5,808,400 bytes). Runs 37520509055/37520515850,
37521061557/37521068081 and 37521816137/37521825816 are the annotation-driven rounds that
led there; their causes are recorded in `audits/evidence/ux-feature-pack-ci-runs.json`.
