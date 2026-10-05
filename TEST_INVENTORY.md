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

## GUI tests (rdm-gui/src/) — 75 tests
- backend.rs: 10
- logging.rs: 2
- settings.rs: 8 (4 + 4 for the path round-trip: five save/load cycles of `C:\download\rdm`,
  legacy over-escaped files, the new switches' defaults/parsing, `--data-dir` precedence)
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
- ux.rs: 13 (UX policy: confirmation scope and consequence copy, safe-option naming, bulk
  count/zero-case copy, drop-all threshold, resume-all outcome naming failed downloads,
  state legend completeness, help sections — now four, including the desktop-integration
  section — Failed/Completed never offer Resume, remove-while-running explanation,
  jargon-free copy, glossary synonyms)

Design layer: `rdm-gui/src/theme/` is the single source of truth for colour,
spacing, radii, typography, sizes and breakpoints; the column-fitting tests
were extended and are kept green by the same CI job.

UX layer: `rdm-gui/src/ux.rs` is the single source of truth for confirmation
policy, consequence copy, the state legend, bulk-action microcopy and the
glossary; `audits/ux-terminology-check.py` enforces the same rules statically
over CLI help, the GUI strings and the README.

## CI (after ci/ci-tests.patch)
- test-linux (cargo test --locked --all-targets)
- test-windows (cargo test --all-targets)
- build-gui-linux / build-gui-windows now include `cargo test --release`
