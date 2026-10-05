# UI Designer — Execution Plan (EXE-042)

- **Plan path:** `audits/ui-designer-execution-plan.md`
- **Persona:** UI Designer (EXE-042), type EXECUTOR, domain Design
- **Primary goal:** create usable and consistent UI
- **Expected outcome:** UI Designs (design tokens + UI patterns + state/responsive/accessibility design, applied to the `rdm-gui` UI layer)
- **Scope:** `UI` — the presentation layer of `rdm-gui` (`rdm-gui/src/**`). Business logic, engine, storage, contracts, schemas are **out of scope** and untouched.
- **Supervisors / decision owners:** Design Manager, Product Manager (PM) — reviewers/approvers per role data.
- **Consumers / handoff targets:** Frontend, UX.
- **Status legend:** `[🔴]` Not Implemented · `[🟡]` Partially Implemented · `[🟢]` Fully Implemented.

## Environment constraints (verified, affect the verification strategy)

| Fact | Evidence | Consequence |
| --- | --- | --- |
| No Rust toolchain (`cargo`, `rustc` absent from `PATH` and filesystem) | EVIDENCE-001 | `cargo build` / `cargo test` cannot run locally → local Rust test results are `NOT_RUN`, never claimed as passed |
| No network egress (`static.crates.io` → `SSL_ERROR_SYSCALL`, HTTP 000) | EVIDENCE-002 | `egui`/`eframe` (not vendored) cannot be fetched → GUI crate cannot be compiled locally |
| GitHub CLI authenticated (`legionir`), Actions enabled; workflow `build.yml` triggers on `arena/*` pushes and runs `cargo test --release` in `rdm-gui` plus a compiler-error annotation step | EVIDENCE-003 | The Rust compile + unit tests for this change are verified **in CI**, and the run log is the evidence |

> Because of the above, every Rust-level claim in the Review/Verification phase must cite a CI run id; static and numerical checks executed locally are clearly labelled as such.

## PH-0 — Inspection & baseline `[🟢]`
| # | Step | Status |
| --- | --- | --- |
| 0.1 | Read the role/plan requirements and the full repository UI surface (`rdm-gui/src/**`, 4050 LOC) | 🟢 |
| 0.2 | Record the baseline inventory of hardcoded values (colors, spacing, sizes) with file/line evidence | 🟢 |
| 0.3 | Measure the baseline WCAG contrast of the as-is hardcoded palette against the as-is surfaces | 🟢 |

## PH-1 — Design tokens, contrast contract, reusable components `[🟢]`
| # | Step | Status |
| --- | --- | --- |
| 1.1 | Create `rdm-gui/src/theme/tokens.rs`: palette (light/dark), spacing scale, radii, typography, sizes, breakpoints, state/chunk/log-level mappings | 🟢 |
| 1.2 | Create `rdm-gui/src/theme/contrast.rs`: WCAG 2.x relative luminance + contrast ratio, role→surface usage contract, AA assertions | 🟢 |
| 1.3 | Create `rdm-gui/src/theme/components.rs`: reusable, token-driven widgets (hint, meta, section, status chip, banner, icon/primary/danger buttons, row background) | 🟢 |
| 1.4 | Create `rdm-gui/src/theme/mod.rs`: `install(ctx, dark)` (visuals + spacing + text styles from tokens), palette accessors, source-guard test that views contain no hardcoded colors | 🟢 |
| 1.5 | Register the module in `main.rs` | 🟢 |

## PH-2 — Token adoption across every screen `[🟢]`
| # | Step | Status |
| --- | --- | --- |
| 2.1 | `views/download_list.rs` — table, headers, rows, columns, state chips, zebra/selection, action buttons | 🟢 |
| 2.2 | `views/toolbar.rs` — primary/bulk actions, search, filters, sidebar toggles | 🟢 |
| 2.3 | `views/footer.rs` — status bar, Events/App-log panes, level filters | 🟢 |
| 2.4 | `views/add_download.rs` — New download form, validation error banner | 🟢 |
| 2.5 | `views/details_modal.rs` — tabs, overview grid, chunk table, JSON pane | 🟢 |
| 2.6 | `views/settings_view.rs` — settings form, dirty indicator | 🟢 |
| 2.7 | `views/queue_sidebar.rs` — queue rows and empty state | 🟢 |
| 2.8 | `app.rs` — theme install, window/panel sizes, confirm dialog | 🟢 |
| 2.9 | Remaining literals in `state.rs` / `main.rs` that encode design values (window size, panel metrics) | 🟢 |

## PH-3 — States, responsiveness, focus & semantics `[🟢]`
| # | Step | Status |
| --- | --- | --- |
| 3.1 | State coverage: all 8 `DownloadState`s, 4 `ChunkStatus`es and 5 log levels mapped to tokens (chips/glyphs/colors) with a coverage test | 🟢 |
| 3.2 | Responsiveness: column drop/scale order tokenized, width invariants preserved (existing tests kept green) | 🟢 |
| 3.3 | Keyboard: ↑/↓ selection, `Enter` details, `Escape` closes overlays, `Ctrl+F` search focus, `F5` refresh — shortcuts only when no text field has focus | 🟢 |
| 3.4 | Focus semantics: token-driven focus ring / selected-row accent bar, tooltips on every icon-only control | 🟢 |

## PH-4 — Verification `[🟢]`
| # | Step | Status |
| --- | --- | --- |
| 4.1 | Static audit script (`audits/ui-contrast-check.py`): recompute every contract pair, count hardcoded values per file, assert zero color literals in views → evidence report | 🟢 |
| 4.2 | CI: push the branch, run `Build rdm (Windows tests + GUI)` → `test-windows` + `build-gui-windows` (`cargo build --release`, `cargo test --release`) | 🟢 |
| 4.3 | Record test results with `TEST-###`, evidence with `EVIDENCE-###`, and update the Change Manifest | 🟢 |

## PH-5 — Review & handoff `[🟢]`
| # | Step | Status |
| --- | --- | --- |
| 5.1 | Write the UI design specification (`audits/ui-designer-design-spec.md`): tokens, state matrix, responsive matrix, accessibility notes, migration map | 🟢 |
| 5.2 | Write the handoff + review package (`audits/ui-designer-handoff.md`) for Frontend/UX and the Design Manager/PM decision owners | 🟢 |
| 5.3 | Consolidate the DoD, risks (`RISK-###`), findings (`FIND-###`) and the final Execution Result | 🟢 |

## Discovered work (added with reason, not part of the original sketch)

| # | Discovery | Reason it was added | Status |
| --- | --- | --- | --- |
| D-1 | No execution plan existed at the mandated path | Role rule §26 requires reading/executing the plan; the file was absent (`audits/` did not exist), so it is created here as the authoritative plan | 🟢 |
| D-2 | No Rust toolchain / network in the sandbox | Verification must move to CI; recorded as a constraint + risk (`RISK-004`) | 🟢 |
| D-3 | As-is palette fails WCAG AA badly (worst 1.43:1) | Not visible from the requirements; discovered by measurement (PH-0.3) and drives the palette redesign | 🟢 |
| D-4 | No keyboard path to open details / select rows | Discovered while mapping states & semantics (PH-3); added as additive, non-breaking interaction design | 🟢 |

## Rule compliance notes

- Completed steps are preserved; no step is deleted or silently rewritten.
- A phase is only 🟢 when all its steps are 🟢 and the phase acceptance is PASS.
- Statuses are updated in place; failures and unfinished work stay visible.
