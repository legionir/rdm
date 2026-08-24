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

## GUI tests (rdm-gui/src/) — 19 tests
- backend.rs: 10
- logging.rs: 2
- settings.rs: 4
- util.rs: 3

## CI (after ci/ci-tests.patch)
- test-linux (cargo test --locked --all-targets)
- test-windows (cargo test --all-targets)
- build-gui-linux / build-gui-windows now include `cargo test --release`
