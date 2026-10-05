# UX Designer — Execution Plan (EXE-043)

- **Plan path:** `audits/ux-designer-execution-plan.md`
- **Persona:** UX Designer (EXE-043), type EXECUTOR, domain Design
- **Primary goal:** create an appropriate user experience
- **Expected outcome:** UX Designs, Flows
- **Scope:** `UX` — user flows, interaction, feedback/error/undo policy, IA, navigation, terminology, a11y & task completion for the `rdm` product surface (CLI + `rdm-gui`). Engine/storage/architecture/contracts are **out of scope** and untouched.
- **Supervisors / decision owners:** Design Manager, Product Manager (PM)
- **Consumers / handoff targets:** UI, Product
- **Status legend:** `[🔴]` Not Implemented · `[🟡]` Partially Implemented · `[🟢]` Fully Implemented

## Preconditions & input provenance (honest recording)

| Input | Type | Source | Required | Validation | Freshness |
| --- | --- | --- | --- | --- | --- |
| Requirements (documented product behaviour: commands, states, flags, GUI parity table) | Requirements | `README.md` §Run / §GUI, `src/cli/commands.rs` clap help, `TEST_INVENTORY.md` | yes | read, cross-checked against the implementation | this branch |
| Shipped product behaviour (the actual flows the user experiences) | Requirements / observed behaviour | `rdm-gui/src/**`, `src/**` at `f528faf` + this branch | yes | inspected file-by-file; every flow claim carries file/line evidence | this branch |
| UI design tokens & accessibility contract (input from the UI Designer handoff) | Design System | `rdm-gui/src/theme/**`, `audits/ui-designer-design-spec.md` | yes | reviewed; used as the surface for UX copy/interaction work | commit `aad936e` |
| User Research (interviews, surveys, personas, task studies) | User Research | **Unknown / Requires Verification — MISSING**: no research artifact exists in the repository | yes | not found → not invented | n/a |
| Analytics (funnels, task success rates, error rates) | Analytics (optional) | **Unknown / Requires Verification — MISSING**: no telemetry/analytics data exists (the product has no telemetry by design) | no | not found | n/a |
| "User Problem Defined" precondition | Precondition | Derived, with explicit assumption: *a user who starts large downloads over unreliable links needs to see what is happening, stop/resume without losing progress, and delete data only on purpose* — grounded in the CLI/GUI feature set (resume, pause, cancel-with-retained-chunks, purge) | yes | recorded in §7 of the handoff as an assumption | this branch |

> **Consequence (recorded, not hidden):** the role's required evidence type is *User Test
> Evidence*. Without research participants and without a runnable GUI in this environment, no
> user-test evidence can be produced. Therefore this increment uses **verifiable substitutes**:
> code-level flow analysis, a heuristic evaluation, a task-completion walkthrough on the real
> interaction code, and automated policy/terminology checks. Everything that would require real
> users is marked **MISSING / UNVERIFIED** and escalated as `UX-ESC-001` (`MISSING_REQUIRED_INPUT`).
> No usability claim is recorded as verified without such evidence.

## Environment constraints (verified)

| Fact | Evidence | Consequence |
| --- | --- | --- |
| No Rust toolchain and no network in the sandbox | EVIDENCE-001/002 (UI increment) | Rust behaviour is verified through CI (`build.yml` on `arena/*` pushes); logs are read through check-run annotations (the diagnostic step was fixed in the UI increment, `ESC-001`) |
| GitHub CLI authenticated; CI green on the branch | EVIDENCE-007 (UI increment) | The UX increment's tests run in the same CI jobs (`test-windows`, `build-gui-windows → Test GUI crate`) |
| No display / no GPU | earlier session check | No interactive prototype or screenshot-based evaluation; the "Prototype" step produces a **specified interaction prototype** (state machine + copy) verified by unit tests, not a clickable build |

## PH-0 — Inspection & baseline `[🟢]`
| # | Step | Status |
| --- | --- | --- |
| 0.1 | Inventory every user-facing string across CLI, GUI and README; define the terminology baseline | 🟢 |
| 0.2 | Verify the semantics of every destructive/recoverable action in the code (remove/purge/restart/cancel/pause/resume/clear queue) | 🟢 |
| 0.3 | Map entry points, exits and error points for every user path; record dead-end affordances | 🟢 |
| 0.4 | Record the baseline terminology/UX findings with file:line evidence | 🟢 |

## PH-1 — UX policy artifacts `[🟢]`
| # | Step | Status |
| --- | --- | --- |
| 1.1 | Glossary & terminology rules (`audits/ux-glossary.md`) | 🟢 |
| 1.2 | Feedback, error and undo policy (`audits/ux-feedback-error-undo-policy.md`) | 🟢 |
| 1.3 | Flows, IA, navigation and journeys (`audits/ux-flows-and-ia.md`) | 🟢 |

## PH-2 — Remediation (bounded to UX scope) `[🟢]`
| # | Step | Status |
| --- | --- | --- |
| 2.1 | Machine-checkable UX policy module `rdm-gui/src/ux.rs` (confirmation policy, consequence copy, status legend, bulk microcopy) + unit tests | 🟢 |
| 2.2 | Confirmation for the bulk destructive action ("Clear completed" → "Remove completed…"), naming the count and the file-deletion consequence | 🟢 |
| 2.3 | Confirmation for "Restart from scratch" (discards partial progress) | 🟢 |
| 2.4 | Honest consequences in the remove dialog (partial data is always discarded; file only with the checkbox; no undo) | 🟢 |
| 2.5 | Remove the dead-end affordance: 🗑 disabled while running, with a next-step tooltip | 🟢 |
| 2.6 | Terminology consistency: status bar (record/done/waiting → download/completed/queued), bulk labels, job/drop wording, CLI help, README | 🟢 |
| 2.7 | Feedback gaps: zero-count bulk actions state "nothing to do" instead of "0 download(s)" | 🟢 |

## PH-3 — Usability evaluation `[🟢]`
| # | Step | Status |
| --- | --- | --- |
| 3.1 | Heuristic evaluation (Nielsen's 10, each with evidence or explicit "no finding") | 🟢 |
| 3.2 | Task-completion walkthrough for the 8 key tasks, mapped to code affordances | 🟢 |
| 3.3 | Accessibility review against the UI Designer's contrast/focus contract | 🟢 |
| 3.4 | User testing | **NOT_APPLICABLE (reason: no participants/environment)** — recorded as `UX-ESC-001`, no user-test claims made |

## PH-4 — Verification `[🟢]`
| # | Step | Status |
| --- | --- | --- |
| 4.1 | Unit tests for the UX policy (confirmation scope, copy content, glossary compliance, legend/microcopy) | 🟢 |
| 4.2 | Static terminology check over CLI + GUI + README with an explicit rule set and exceptions (`audits/ux-terminology-check.py`) → report | 🟢 |
| 4.3 | CI: compile + test the GUI crate; CLI regression | 🟢 |
| 4.4 | Record `TEST-###` / `EVIDENCE-###` and the Change Manifest | 🟢 |

## PH-5 — Review & handoff `[🟢]`
| # | Step | Status |
| --- | --- | --- |
| 5.1 | Usability evaluation report (`audits/ux-usability-evaluation.md`) | 🟢 |
| 5.2 | Handoff + Execution Result (`audits/ux-designer-handoff.md`) for UI/Product and the Design Manager/PM | 🟢 |
| 5.3 | Consolidate findings, risks, recommendations, DoD | 🟢 |

## PH-6 — Closure of the open items (follow-up increment) `[🟢]`
Requested after review: “close the items that can be closed”. Items that need
outside input (user testing, product decisions) stay open **by design** and keep
their escalations.

| # | Item | Type | Status |
| --- | --- | --- | --- |
| 6.1 | Q2 (flows §7): a long queue dropped with one click | Open product/UX question, closeable in UX scope | 🟢 asks when ≥ 5 queued downloads (`ux::drop_all_confirm`, `Confirm::DropAll`) |
| 6.2 | RISK-UX-003: the state legend existed only on hover | Risk, closeable | 🟢 in-app help window (`F1` / ⓘ) with states, keyboard map and vocabulary; the empty list also names the first step and `F1` in text |
| 6.3 | Q4/§4: `Resume all` skipped failed downloads silently | Feedback gap | 🟢 the outcome message names them and points at ⟲ Restart (`ux::resume_all_outcome`) |
| 6.4 | GUI copy quoted CLI syntax (`Remove ▸ purge`) and the remove outcome hid a silent no-op | Consistency/honesty defect | 🟢 GUI copy rewritten; `remove` reports “the file was already gone” when there was nothing to delete |
| 6.5 | CLI failed summary pointed at `rdm resume`, which refuses a failed download | Recovery-path defect (same class as FIND-UX-003) | 🟢 the summary names the continuation command that works |
| 6.6 | `TEST_INVENTORY.md` GUI section and the copy rules were not updated for the closure | Documentation | 🟢 updated (51 GUI tests, ux.rs: 12) |
| 6.7 | Q1 (make `Failed` resumable in the engine?) and Q3 (undo for file deletion?) | **Require product/engineering decisions** | 🔴 open, escalated as `UX-ESC-004` (owner: Product + engineering) |
| 6.8 | User testing (PH-3.4) | **Requires participants/display** | 🔴 open by design, `UX-ESC-001` |

Closure verification: CI run 37357552644 on `15cf0ed` (build ✓, `Test GUI crate` ✓ with the
12 UX policy tests, CLI regression ✓) + `audits/ux-terminology-check.py` PASS +
`audits/ui-contrast-check.py` PASS (21 Rust files incl. the new `help_overlay.rs`).

## Discovered work (added with a reason, never silently)

| # | Discovery | Reason it was added | Status |
| --- | --- | --- | --- |
| D-1 | No execution plan existed at the mandated path | Role §26 requires reading/executing the plan; `audits/` had none for this persona → created here as authoritative | 🟢 |
| D-2 | The bulk action "Clear completed" removed records **and could delete files** with a single click and **no confirmation** | Found in PH-0.2; violates the role acceptance criterion "destructive actions have confirmation and recovery" → remediated in PH-2.2 | 🟢 |
| D-3 | "Restart from scratch" discarded partial progress with one click, no confirmation | PH-0.2/0.3; same criterion → PH-2.3 | 🟢 |
| D-4 | 🗑 was offered for running downloads although the backend rejects it ("still running; cancel it first") | PH-0.3 dead-end affordance → PH-2.5 | 🟢 |
| D-5 | Status bar used database jargon ("record(s)", "done", "waiting") inconsistent with the state names used everywhere else | PH-0.1 terminology inventory → PH-2.6 | 🟢 |
| D-6 | No user research/analytics artifacts exist in the repository, while the role requires *User Test Evidence* | PH-0 provenance check; cannot be fabricated → recorded as MISSING and escalated (`UX-ESC-001`); verification uses documented substitutes | 🟢 (recorded) |
| D-7 | The GUI carried CLI-only syntax in its own copy (`Remove ▸ purge`) and `remove` reported success even when there was no file to delete | Found while closing the open items; a user cannot execute CLI syntax from the window, and an unreported no-op reads as a deletion | 🟢 PH-6.4 |
| D-8 | The CLI's failed summary told users to run `rdm resume <ID>`, which the same CLI refuses for a failed download | Same defect class as FIND-UX-003 (dead-end recovery path), found by re-reading the summary against `run_resume` | 🟢 PH-6.5 |
| D-9 | The two CI-diagnostic steps are load-bearing: the closure increment's three compile issues (module wiring, token names, egui builder API) were found **only** through check-run annotations | Confirms `UX-ESC-003`: without the steps, GUI regressions in this environment are undiagnosable | 🟢 (recorded) |
| D-10 | The bulk `resume_all` scope fix (FIND-UX-004) changes behaviour documented in the README | Requirement-owner decision → escalated, not decided here | 🟢 (recorded, `UX-ESC-002`) |

## Verification results (final)

| Gate | Result | Evidence |
| --- | --- | --- |
| UX policy unit tests (confirmations, copy, glossary compliance, legend/microcopy) | PASS | CI run `build-gui-windows` → `Test GUI crate` |
| Terminology check (rules + exceptions) | PASS | `audits/evidence/ux-terminology-report.txt` |
| Rust compile + GUI tests | PASS | CI runs 37350073234 (`3eb2def`) and 37350848407 (`47253af`); CLI regression job `test-windows` ✓ |
| CLI/engine regression | PASS | CI run → job `test-windows` |
| Baseline (before) evidence | recorded | `audits/evidence/ux-baseline-*.txt` |
| User testing | **MISSING — NOT_APPLICABLE in this environment** | `UX-ESC-001` |

## Rule compliance notes

- Completed steps are preserved; nothing is deleted or silently rewritten.
- A phase is 🟢 only when all its steps are 🟢 and the acceptance criteria pass.
- Every `NOT_APPLICABLE` carries a reason; every escalation names its target.
