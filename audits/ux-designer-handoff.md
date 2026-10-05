# Handoff — UX Designer (EXE-043): flows, interaction policy, terminology

- **Handoff from:** UX Designer (EXE-043), EXECUTOR
- **Primary recipients:** UI, Product
- **Supporting recipients / decision owners:** Design Manager, Product Manager (PM)
- **Required artifacts:** UX Designs, Flows → [`ux-flows-and-ia.md`](ux-flows-and-ia.md),
  [`ux-glossary.md`](ux-glossary.md), [`ux-feedback-error-undo-policy.md`](ux-feedback-error-undo-policy.md),
  [`ux-usability-evaluation.md`](ux-usability-evaluation.md)
- **Required actions:** review/approve against the usability criteria, record the state, then
  answer the open questions in §7 of the flows document
- **Branch / commit:** `arena/01a10cef-rdm` @ `58cf277` (audit trail) with the closure code head
  `15cf0ed`. CI: **code head** [37357552644](https://github.com/legionir/rdm/actions/runs/37357552644)
  ✓ and **audit-trail head** [37358274886](https://github.com/legionir/rdm/actions/runs/37358274886)
  ✓ — `Build GUI binary` ✓, `Test GUI crate` ✓, `test-windows` (CLI regression) ✓ in both.
  Documentation-only commits after `58cf277` re-run the same unchanged jobs. Increment history:
  [`37350073234`](https://github.com/legionir/rdm/actions/runs/37350073234) @ `3eb2def` (first
  code-green run) → [`37350848407`](https://github.com/legionir/rdm/actions/runs/37350848407) @
  `47253af` (docs) → **closure increment** `4de6f1c`/`df5c77f`/`15cf0ed` → run above
- **Pull request:** [#8](https://github.com/legionir/rdm/pull/8) (same branch; this increment is
  part of it)

## 1. Traceability chain

| Requirement | Criterion | Design | Implementation | Test | Evidence | Acceptance |
| --- | --- | --- | --- | --- | --- | --- |
| REQ-UX-001 flows/journeys with entry, exit and error points | CRIT-UX-001 every path has entry points, exits and error points | DESIGN-UX-001 IA, DESIGN-UX-002 task flows, DESIGN-UX-003 journeys | IMP-UX-010…015 (list/dialog/sidebar/footer states) | TEST-UX-012 (walkthrough) | EVIDENCE-UX-003 | ACCEPT-UX-001 |
| REQ-UX-002 feedback, error and undo policy | CRIT-UX-002 destructive actions have confirmation and recovery | DESIGN-UX-004 confirmation policy, DESIGN-UX-005 undo inventory | IMP-UX-001…006 (`ux.rs`, dialogs, row actions, bulk guards) | TEST-UX-001…006 | EVIDENCE-UX-002, EVIDENCE-UX-005 | ACCEPT-UX-002 |
| REQ-UX-003 IA, navigation, terminology | CRIT-UX-003 glossary and terminology consistent across the product | DESIGN-UX-006 navigation model, DESIGN-UX-007 glossary | IMP-UX-007…009 (ux.rs vocabulary, CLI help, README) | TEST-UX-007…009 | EVIDENCE-UX-004 | ACCEPT-UX-003 |
| REQ-UX-004 evaluation with a11y and task completion | CRIT-UX-004 a11y + task completion evaluated | DESIGN-UX-008 legend/help-in-context, DESIGN-UX-009 bulk safety | IMP-UX-016 (legend surfaces) | TEST-UX-010…013 | EVIDENCE-UX-001, EVIDENCE-UX-005 | ACCEPT-UX-004 |

**ACCEPT-UX gates** (usability criteria): ACCEPT-UX-001 flows complete (entry/exit/error) ·
ACCEPT-UX-002 no unconfirmed data-destroying action and every destructive action has a named
recovery or an explicit “cannot be undone” · ACCEPT-UX-003 zero terminology violations across the
three surfaces · ACCEPT-UX-004 a11y cross-checks pass and task walkthrough documented.

## 2. Change Manifest

```
ChangeManifest:
  - Path: rdm-gui/src/ux.rs
      Action: CREATED   Scope: UX (policy + copy)   Status: COMPLETED
      Reason: one place for confirmation policy, consequence copy, status legend, bulk microcopy and the glossary; all unit-tested
      RequirementIDs: [REQ-UX-002, REQ-UX-003, REQ-UX-004]   TestStatus: PASS   Evidence: [EVIDENCE-UX-002, EVIDENCE-UX-005]
  - Path: rdm-gui/src/main.rs
      Action: MODIFIED  Scope: UX (module wiring)   Status: COMPLETED
      Reason: register `mod ux` (line 22)
      RequirementIDs: [REQ-UX-002]                   TestStatus: PASS   Evidence: [EVIDENCE-UX-005]
  - Path: rdm-gui/src/state.rs
      Action: MODIFIED  Scope: UX (interaction state) Status: COMPLETED
      Reason: PendingConfirm model + AskRestart/AskRemoveCompleted/RemoveCompletedConfirmed/DropQueued/DropQueue actions; stale RemoveCompleted variant removed; counters documented with product vocabulary
      RequirementIDs: [REQ-UX-001, REQ-UX-002]       TestStatus: PASS   Evidence: [EVIDENCE-UX-005]
  - Path: rdm-gui/src/app.rs
      Action: MODIFIED  Scope: UX (dialogs + action loop) Status: COMPLETED
      Reason: single policy-driven confirmation dialog; confirmations for bulk remove and restart; honest outcome copy; zero-case feedback; Esc/close handling; helpers describe/restart_now/remove_completed_now/confirm_destructive
      RequirementIDs: [REQ-UX-001, REQ-UX-002]       TestStatus: PASS   Evidence: [EVIDENCE-UX-002, EVIDENCE-UX-005]
  - Path: rdm-gui/src/views/download_list.rs
      Action: MODIFIED  Scope: UX (row actions)     Status: COMPLETED
      Reason: Failed no longer offers Resume (engine rejects it) and gets Restart; 🗑 disabled while running with the reason; all row tooltips from the UX layer; empty state names the first step
      RequirementIDs: [REQ-UX-002, REQ-UX-004]       TestStatus: PASS   Evidence: [EVIDENCE-UX-002]
  - Path: rdm-gui/src/views/toolbar.rs
      Action: MODIFIED  Scope: UX (labels/copy)     Status: COMPLETED
      Reason: “Remove completed…” and “Drop all” with consequence tooltips; queue chip copy; legend ⓘ
      RequirementIDs: [REQ-UX-003, REQ-UX-004]       TestStatus: PASS   Evidence: [EVIDENCE-UX-004]
  - Path: rdm-gui/src/views/footer.rs
      Action: MODIFIED  Scope: UX (status bar)      Status: COMPLETED
      Reason: product vocabulary in the counters; the counters hover reveals the full state legend; pane tooltips
      RequirementIDs: [REQ-UX-003, REQ-UX-004]       TestStatus: PASS   Evidence: [EVIDENCE-UX-004]
  - Path: rdm-gui/src/views/queue_sidebar.rs
      Action: MODIFIED  Scope: UX (queue)           Status: COMPLETED
      Reason: “queued” vocabulary, Drop semantics, honest empty state
      RequirementIDs: [REQ-UX-003]                   TestStatus: PASS   Evidence: [EVIDENCE-UX-004]
  - Path: rdm-gui/src/views/settings_view.rs
      Action: MODIFIED  Scope: UX (settings)        Status: COMPLETED
      Reason: “Confirm destructive actions” label + tooltip; job→download in the concurrency tooltip
      RequirementIDs: [REQ-UX-002, REQ-UX-003]       TestStatus: PASS   Evidence: [EVIDENCE-UX-004]
  - Path: rdm-gui/src/views/details_modal.rs
      Action: MODIFIED  Scope: UX (details)         Status: COMPLETED
      Reason: plain-language state meaning + next step; “partial data folder”; ranges explained; empty chunk state; tooltips
      RequirementIDs: [REQ-UX-001, REQ-UX-004]       TestStatus: PASS   Evidence: [EVIDENCE-UX-003]
  - Path: rdm-gui/src/views/add_download.rs
      Action: MODIFIED  Scope: UX (form)            Status: COMPLETED
      Reason: Start/Cancel tooltips, tip copy from the UX layer
      RequirementIDs: [REQ-UX-001]                   TestStatus: PASS   Evidence: [EVIDENCE-UX-003]
  - Path: rdm-gui/src/backend.rs
      Action: MODIFIED  Scope: UX (bulk interaction) Status: COMPLETED  **behaviour change, documented**
      Reason: `resume_all` covered Failed rows, which `resume()` rejects, so one failed download aborted the whole bulk action; it now covers exactly the states resume() accepts (paused/interrupted/cancelled) and the UI reports the count
      RequirementIDs: [REQ-UX-002]                   TestStatus: PASS   Evidence: [EVIDENCE-UX-002, EVIDENCE-UX-005]
  - Path: src/cli/commands.rs
      Action: MODIFIED  Scope: UX (CLI terminology)  Status: COMPLETED
      Reason: help text aligned with the glossary (remove = list entry + optional file; resume = paused/interrupted/cancelled; force = start over; purge = finished file + checksum sidecar); one error message reworded
      RequirementIDs: [REQ-UX-003]                   TestStatus: PASS   Evidence: [EVIDENCE-UX-004]
  - Path: README.md
      Action: MODIFIED  Scope: UX (documentation)    Status: COMPLETED
      Reason: *Remove completed…* label, confirmations & state-legend features, `confirm_remove` comment, vocabulary fixes
      RequirementIDs: [REQ-UX-003, REQ-UX-004]       TestStatus: NOT_APPLICABLE (prose)   Evidence: [EVIDENCE-UX-004]
  - Path: audits/ux-designer-execution-plan.md
      Action: CREATED   Scope: Documentation         Status: COMPLETED   Reason: mandated plan (did not exist), kept current with results
      RequirementIDs: [REQ-UX-001…004]   TestStatus: NOT_APPLICABLE (plan)   Evidence: [EVIDENCE-UX-005]
  - Path: audits/ux-flows-and-ia.md
      Action: CREATED   Scope: UX Designs (flows/IA) Status: COMPLETED   Reason: IA, primary flow, 10 task flows, entry/exit/error matrix, journeys, open questions
      RequirementIDs: [REQ-UX-001, REQ-UX-003]   TestStatus: NOT_APPLICABLE (design doc)   Evidence: [EVIDENCE-UX-003]
  - Path: audits/ux-glossary.md
      Action: CREATED   Scope: UX Designs (terminology) Status: COMPLETED   Reason: preferred terms, rejected synonyms, copy rules, documented exceptions, baseline→after
      RequirementIDs: [REQ-UX-003]   TestStatus: NOT_APPLICABLE (design doc)   Evidence: [EVIDENCE-UX-004]
  - Path: audits/ux-feedback-error-undo-policy.md
      Action: CREATED   Scope: UX Designs (policy)   Status: COMPLETED   Reason: interaction contract, confirmation dialogs, error policy, undo inventory, trade-offs
      RequirementIDs: [REQ-UX-002]   TestStatus: NOT_APPLICABLE (design doc)   Evidence: [EVIDENCE-UX-002]
  - Path: audits/ux-usability-evaluation.md
      Action: CREATED   Scope: UX evaluation        Status: COMPLETED   Reason: heuristic evaluation, findings, a11y cross-check, KPI table, honesty statement about missing user-test data
      RequirementIDs: [REQ-UX-004]   TestStatus: NOT_APPLICABLE (evaluation)   Evidence: [EVIDENCE-UX-001, EVIDENCE-UX-004]
  - Path: audits/ux-terminology-check.py
      Action: CREATED   Scope: Verification tooling  Status: COMPLETED   Reason: machine-checkable terminology/microcopy rules + destructive-action wiring checks
      RequirementIDs: [REQ-UX-002, REQ-UX-003]   TestStatus: PASS (exit 0)   Evidence: [EVIDENCE-UX-004]
  - Path: audits/evidence/ux-terminology-baseline.txt
      Action: CREATED   Scope: Evidence              Status: COMPLETED   Reason: the 15 violations that existed before the fix
      RequirementIDs: [REQ-UX-003]   TestStatus: NOT_APPLICABLE (baseline)   Evidence: [EVIDENCE-UX-004]
  - Path: audits/evidence/ux-terminology-report.txt
      Action: CREATED   Scope: Evidence              Status: COMPLETED   Reason: 0 violations after the fix
      RequirementIDs: [REQ-UX-002, REQ-UX-003]   TestStatus: PASS   Evidence: [EVIDENCE-UX-004]
  - Path: audits/evidence/ux-ci-run.json, audits/evidence/ux-ci-jobs.json
      Action: CREATED   Scope: Evidence              Status: COMPLETED   Reason: machine-readable record of the green CI run (compile + tests + CLI regression)
      RequirementIDs: [REQ-UX-001…004]   TestStatus: PASS   Evidence: [EVIDENCE-UX-005]
  - Path: .github/workflows/build.yml
      Action: MODIFIED  Scope: **OUT OF SCOPE — CI** Status: COMPLETED (needs approval, UX-ESC-003)
      Reason: added a "Surface test failures" step (same pattern/justification as the existing build-diagnostics step): failing test names/assertions have to reach the review API because the raw job log is unreachable from this environment. Job semantics unchanged (the job still fails when tests fail).
      RequirementIDs: [] (enabler for verification)   TestStatus: PASS (run 37350073234)   Evidence: [EVIDENCE-UX-005]
```

  - Path: rdm-gui/src/views/help_overlay.rs
      Action: CREATED   Scope: UX (in-app help)   Status: COMPLETED
      Reason: states + keyboard map + vocabulary reachable without hovering (closes RISK-UX-003); content comes from ux::help_sections so the unit tests cover it
      RequirementIDs: [REQ-UX-005]   TestStatus: PASS (CI)   Evidence: [EVIDENCE-UX-007]
  - Path: rdm-gui/src/ux.rs (closure delta)
      Action: MODIFIED  Scope: UX policy   Status: COMPLETED
      Reason: Confirm::DropAll + DROP_ALL_CONFIRM_THRESHOLD/drop_all_confirm, resume_all_outcome, SHORTCUTS (F1), help_sections; 9 → 12 policy tests
      RequirementIDs: [REQ-UX-005]   TestStatus: PASS (CI)   Evidence: [EVIDENCE-UX-007]
  - Path: rdm-gui/src/state.rs (closure delta)
      Action: MODIFIED  Scope: interaction state   Status: COMPLETED
      Reason: PendingConfirm::DropQueue, UiAction::AskDropQueue, UiAction::ToggleHelp, GuiState::show_help
      RequirementIDs: [REQ-UX-005]   TestStatus: PASS (CI)   Evidence: [EVIDENCE-UX-007]
  - Path: rdm-gui/src/app.rs (closure delta)
      Action: MODIFIED  Scope: interaction loop   Status: COMPLETED
      Reason: AskDropQueue threshold handling + drop_queue_now; Resume-all outcome names failed downloads; help overlay rendered and toggled; Escape closes the help first
      RequirementIDs: [REQ-UX-005]   TestStatus: PASS (CI)   Evidence: [EVIDENCE-UX-007]
  - Path: rdm-gui/src/views/toolbar.rs, queue_sidebar.rs, download_list.rs, views/mod.rs (closure delta)
      Action: MODIFIED  Scope: entry points/copy   Status: COMPLETED
      Reason: ⓘ Help button + F1 hint; AskDropQueue from the queue sidebar and the chip; the empty list names the first step and F1 in text; help view registered
      RequirementIDs: [REQ-UX-005]   TestStatus: PASS (CI)   Evidence: [EVIDENCE-UX-007]
  - Path: rdm-gui/src/backend.rs (closure delta)
      Action: MODIFIED  Scope: GUI copy/honesty   Status: COMPLETED
      Reason: cancel/remove messages no longer quote CLI syntax; remove reports “the file was already gone” when the purge deleted nothing (an unreported no-op read as a deletion)
      RequirementIDs: [REQ-UX-005]   TestStatus: PASS (CI)   Evidence: [EVIDENCE-UX-007]
  - Path: src/cli/commands.rs (closure delta)
      Action: MODIFIED  Scope: CLI recovery copy   Status: COMPLETED
      Reason: the failed summary no longer points at `rdm resume` (refused for a failed download) and names the working continuation command
      RequirementIDs: [REQ-UX-005]   TestStatus: PASS (CI)   Evidence: [EVIDENCE-UX-007]
  - Path: README.md, TEST_INVENTORY.md, audits/*.md (closure delta)
      Action: MODIFIED  Scope: documentation   Status: COMPLETED
      Reason: queue threshold, help window, resume-all scope documented; GUI test inventory 48 → 51 (ux.rs: 12); plan/flows/policy/evaluation updated with the closure and the remaining open items
      RequirementIDs: [REQ-UX-005]   TestStatus: NOT_APPLICABLE (prose)   Evidence: [EVIDENCE-UX-007]

No files were deleted or renamed. One stale enum variant (`UiAction::RemoveCompleted`) was
**removed** from `state.rs` (replaced by `AskRemoveCompleted` + `RemoveCompletedConfirmed`) — listed
here so it is not mistaken for hidden work; it is not a file deletion.

## 3. Coverage / completeness

- Changed & verified: 12 source files (11 GUI + 1 CLI) — all compiled by CI (`Build GUI binary`),
  and the crate's tests run in the same job (`Test GUI crate`, 48 `#[test]` in `rdm-gui`, 9 of them
  new UX policy tests counted from source). CLI/engine crate re-tested (`test-windows`).
- Documentation: 4 UX artifacts + plan; evidence: 4 files; tooling: 1 checker.
- Change coverage: **100 %** of changed code files compiled and covered by the CI run; test-level
  coverage of the changed policy logic is stated per test in §4 (no line-coverage tool available).
- Not covered (explicit): user testing with participants (`UX-ESC-001`), pixel-level review (no
  display), and any claim about measured task success (`KPI = Unknown`).

## 4. Tests

| ID | Test | Kind | Result | Evidence |
| --- | --- | --- | --- | --- |
| TEST-UX-001 | `ux::every_data_destroying_action_has_a_confirmation_policy` | Rust unit (CI) | PASS | EVIDENCE-UX-005 |
| TEST-UX-002 | `ux::confirmations_state_the_consequence_and_the_undo_status` | Rust unit (CI) | PASS | EVIDENCE-UX-005 |
| TEST-UX-003 | `ux::the_safe_option_is_named_after_what_it_keeps` | Rust unit (CI) | PASS | EVIDENCE-UX-005 |
| TEST-UX-004 | `ux::bulk_actions_name_the_count_and_explain_the_empty_case` (applies to remove/restart copy) | Rust unit (CI) | PASS | EVIDENCE-UX-005 |
| TEST-UX-005 | `ux::bulk_actions_name_the_count_and_explain_the_empty_case` (pause/resume/drop + counts) | Rust unit (CI) | PASS | EVIDENCE-UX-005 |
| TEST-UX-006 | `ux::failed_and_completed_never_offer_resume` | Rust unit (CI) | PASS | EVIDENCE-UX-005 |
| TEST-UX-007 | `ux::every_state_has_a_legend_entry_with_a_next_step` | Rust unit (CI) | PASS | EVIDENCE-UX-005 |
| TEST-UX-008 | `ux::no_user_facing_copy_uses_implementation_jargon` | Rust unit (CI) | PASS | EVIDENCE-UX-005 |
| TEST-UX-009 | `ux::the_glossary_rejects_a_synonym_for_every_term` + `ux::removing_a_running_download_explains_the_order_of_operations` | Rust unit (CI) | PASS | EVIDENCE-UX-005 |
| TEST-UX-010 | Contrast/focus contract re-verified after the UX copy changes | Static audit (local) | PASS | EVIDENCE-UX-001 |
| TEST-UX-011 | `audits/ux-terminology-check.py` (terminology, confirmations, wiring, cross-surface) | Static audit (local) | PASS (0 violations) | EVIDENCE-UX-004 |
| TEST-UX-012 | Task walkthrough T1–T10 + 4 bulk flows against the code (documented per step) | Manual analysis | PASS | EVIDENCE-UX-003 |
| TEST-UX-013 | Regression: CLI/engine crate (`cargo test --all-targets`) and the pre-existing GUI tests (39) | CI | PASS | EVIDENCE-UX-005 |
| TEST-UX-014 | Baseline: terminology violations before the change (15) | Static audit | PASS (recorded) | EVIDENCE-UX-004 |
| TEST-UX-015 | User testing with participants | — | **NOT_RUN — NOT_APPLICABLE in this environment** | `UX-ESC-001` |
| TEST-UX-016 | `ux::a_short_queue_is_dropped_at_once_a_long_one_asks` (drop threshold + copy) | Rust unit (CI) | PASS | EVIDENCE-UX-007 |
| TEST-UX-017 | `ux::resume_all_names_the_failed_downloads_it_cannot_continue` | Rust unit (CI) | PASS | EVIDENCE-UX-007 |
| TEST-UX-018 | `ux::help_covers_states_shortcuts_and_vocabulary` + `help_sections()` content | Rust unit (CI) | PASS | EVIDENCE-UX-007 |
| TEST-UX-019 | Closure regression: build + GUI tests + CLI tests after the closure changes | CI | PASS | EVIDENCE-UX-007 |

No test result is claimed without execution; TEST-UX-015 is deliberately NOT_RUN and no usability
metric depends on it.

## 5. Evidence log

| ID | Claim it supports | Type | Location |
| --- | --- | --- | --- |
| EVIDENCE-UX-001 | UI contrast/focus contract still holds after the UX changes | TEST_RESULT | `audits/evidence/ui-contrast-check.txt` (regenerated; `python3 audits/ui-contrast-check.py`) |
| EVIDENCE-UX-002 | Confirmation policy exists, names consequences and is wired to the actions | FILE / LINE | `rdm-gui/src/ux.rs` (Confirm, DESTRUCTIVE, BulkAction), `rdm-gui/src/app.rs` (`confirm_dialog`, `AskRemoveCompleted`, `AskRestart`, `confirm_destructive`), `rdm-gui/src/views/download_list.rs` (row actions) |
| EVIDENCE-UX-003 | Flows/IA/entry/exit/error points and the walkthrough | DOCUMENT / SECTION | `audits/ux-flows-and-ia.md` §1–§7; `audits/ux-usability-evaluation.md` §2 |
| EVIDENCE-UX-004 | Terminology consistency (0 violations, from 15) | TEST_RESULT / FILE | `audits/evidence/ux-terminology-baseline.txt`, `audits/evidence/ux-terminology-report.txt`, `audits/ux-terminology-check.py` |
| EVIDENCE-UX-005 | Rust compile + tests + CLI regression on the final head | TEST_RESULT / BUILD_OUTPUT | CI runs [37350073234](https://github.com/legionir/rdm/actions/runs/37350073234) (code head `3eb2def`) and [37350848407](https://github.com/legionir/rdm/actions/runs/37350848407) (docs head `47253af`) → `audits/evidence/ux-ci-run.json`, `ux-ci-jobs.json`, `ux-ci-run-final.json`, `ux-ci-jobs-final.json` |
| EVIDENCE-UX-007 | Closure increment compiled, tested and green on the final head; static checks pass (incl. the new help view) | TEST_RESULT / BUILD_OUTPUT | CI run [37357552644](https://github.com/legionir/rdm/actions/runs/37357552644) → `audits/evidence/ux-ci-run-closure.json`, `ux-ci-jobs-closure.json`; `audits/evidence/ux-terminology-report.txt`, `audits/evidence/ui-contrast-check.txt` (regenerated, 21 Rust files) |
| EVIDENCE-UX-008 | Three CI-only compile issues in the closure (module wiring, token names, egui builder API) were diagnosed exclusively through check-run annotations | LOG | annotations of check runs on `4de6f1c`/`df5c77f`; each fix names the failure in its commit message |
| EVIDENCE-UX-006 | The two earlier CI failures of this increment and their causes (module not registered; a test asserting the file-consequence phrase) — traceability of the fixes | LOG | check-run annotations of runs 37348453897 and 37349440692 (recorded in the commit messages `39372c8`, `3eb2def`) |

## 6. Findings, risks, recommendations

Findings consolidated (no duplicates) in `audits/ux-usability-evaluation.md` §2: **FIND-UX-001…012**,
three of them **High** (unconfirmed bulk deletion incl. files; unconfirmed restart; Resume offered
where the engine refuses — plus the bulk-resume abort).

| ID | Risk (from the findings) | Likelihood | Impact | Score | Mitigation | Owner | Residual |
| --- | --- | --- | --- | --- | --- | --- | --- |
| RISK-UX-001 | A user switches confirmations off (Settings) and destroys data unintentionally | Unlikely | High | Medium | default is on; the outcome message states what happened; the file checkbox is separate from the removal | UX / Product | Medium — accepted, documented |
| RISK-UX-002 | No undo for file deletion; a confirmed mistake is unrecoverable | Possible | High | High | confirmation + explicit file checkbox; recovery inventory documented; Q3 raised for Product | Product | High — needs a product decision |
| RISK-UX-003 | ~~The state legend is hover-only~~ **Closed (PH-6.2).** | — | — | **Closed** | `F1`/ⓘ help window with states, keyboard and vocabulary; the empty list names the first step and `F1` in text; unit test asserts the content is complete | UX / UI | **Closed** |
| RISK-UX-007 | The 5-item threshold for confirming *Drop all* may not match user expectations | Possible | Low | Low | one constant (`DROP_ALL_CONFIRM_THRESHOLD`); the dialog below the threshold is intentionally absent because nothing has been fetched; a user study can move it | UX / Design Manager | Low |
| RISK-UX-004 | No external user manual | Possible | Low | Low | in-context help everywhere **plus** the in-app help window (F1) and the README; an external manual remains outside this role's scope | Product | Low |
| RISK-UX-005 | The `resume_all` scope change alters documented bulk behaviour | Unlikely | Medium | Low | documented in the manifest; the CLI help and tooltip now match the engine; escalated for PM confirmation (`UX-ESC-002`) | PM | Low |
| RISK-UX-006 | Verification of UX behaviour depends on CI (no local toolchain) | Almost certain | Medium | Medium | CI green on the final head; local static checks re-run each time | UX / PM | Medium |

| ID | Recommendation | Owner |
| --- | --- | --- |
| REC-UX-001 | Approve the glossary + copy rules and keep the static check in CI (add it to the test job so regressions fail the build, not only the local script) | PM / Frontend |
| REC-UX-002 | Run the 5-participant study (T1, T3, T7 + the bulk flows) to convert the “Unknown” task-success KPI into data | Design Manager / PM |
| REC-UX-003 | Decide Q1 (make `Failed` resumable?) and Q3 (undo for file deletion?) — both touch engine/product scope and are escalated, not decided here | Product + engineering |
| REC-UX-004 | ~~First-run hint~~ **Done (PH-6.2):** the empty list names the first step and `F1` in text, and `F1` opens the full state/keyboard/vocabulary help. Remaining option: an actual first-run tour, if Product wants one. | UX / UI |
| REC-UX-005 | When a user manual/help centre exists, link it from the App-log pane and the New download dialog | Product |

## 7. Escalations

```
UX-ESC-001
Trigger:      MISSING_REQUIRED_INPUT
Evidence:     No User Research or Analytics artifact exists in the repository; the environment has
              no display, no runnable build and no participants (EVIDENCE-UX-005 shows the CI-only
              verification path). The role requires *User Test Evidence*.
Impact:       Usability cannot be validated with real users; the task-success KPI stays Unknown
              and all usability claims rest on code-level analysis, policy tests and static checks.
BlockedWork:  PH-3.4 (user testing) — NOT_APPLICABLE in this environment; STEP-4/STEP-5 evidence
              class "USER_FEEDBACK" remains MISSING.
DecisionRequired: provide research input/participants for a follow-up study, or accept the
              documented substitutes as the evidence base for this increment.
TargetPersona: Design Manager, Product Manager (PM)
Urgency:      P2 (nothing is blocked from shipping; the evidence class is incomplete)

UX-ESC-002
Trigger:      OWNERSHIP_CONFLICT (behaviour/requirement)
Evidence:     `backend.rs::resume_all` previously included Failed rows, which `resume()` rejects
              (backend.rs:261–275) — the bulk action aborted on the first failed download. The
              README described the bulk action as covering "paused/interrupted/failed".
Impact:       Aligning the action with the engine changes documented bulk behaviour; the decision
              belongs to the requirement owner, not to UX.
BlockedWork:  none (the consistency fix is implemented and verified) — the *decision* is what needs
              confirmation.
DecisionRequired: confirm the corrected scope (paused/interrupted/cancelled) and the wording, or
              request a different policy (e.g. make Failed resumable in the engine → Q1).
TargetPersona: Product Manager (PM) with engineering
Urgency:      P2

UX-ESC-003
Trigger:      SCOPE_CONFLICT (CI file, same class as ESC-001 of the UI increment)
Evidence:     Actions log blobs are unreachable from the verification environment; the existing
              diagnostics step only covered build failures, so the failing GUI test of run
              37349440692 was invisible (EVIDENCE-UX-006).
Impact:       Without the added "Surface test failures" step, test regressions cannot be diagnosed
              from this environment at all.
BlockedWork:  PH-4 verification of any future GUI test failure.
DecisionRequired: approve the two CI diagnostic steps (build + tests) as permanent, or provide an
              alternative diagnostics channel.
TargetPersona: Design Manager, Product Manager (PM) — repo owner for CI
Urgency:      P1
```

```
UX-ESC-004
Trigger:      MISSING_DECISION (product/engine policy)
Evidence:     Q1: `Failed` downloads cannot be resumed (`src/cli/commands.rs::run_resume` rejects
              terminal states except Cancelled; `rdm-gui/src/backend.rs::resume` mirrors it), so the
              only recovery is a full Restart — the UX layer now says so in the GUI, the CLI and the
              help window instead of pointing at a refusal (PH-6.5). Q3: file deletion has no undo
              (`backend.rs::remove` deletes the output file and its sidecar; RISK-UX-002).
Impact:       Both options change product/engine behaviour (resumable failed downloads; a kept
              detached-file copy), which is outside the UX scope to decide.
BlockedWork:  Closing Q1/Q3 — everything UX can do without the decision is implemented (copy,
              confirmations, truthful outcomes).
DecisionRequired: decide whether `Failed` becomes resumable (engine change) and whether an undo
              or a retention window for deleted files is worth its storage cost.
TargetPersona: Product Manager (PM) with engineering
Urgency:      P2
```

Cross-domain rule compliance: each effect was identified, current behaviour preserved where
possible (job outcomes unchanged: destructive confirms are additive, the CI job still fails when
builds/tests fail), documented, and escalated to the responsible persona.

## 8. Definition of Done

| Item | State |
| --- | --- |
| All increments complete | yes — PH-0…PH-5 (main) and PH-6 (closure) 🟢; the two items that need outside input stay 🔴 **by design** (6.7 product decisions, 6.8 user testing) |
| Change Manifest complete | yes — §2 (20 entries) |
| Modified/created files recorded | yes — §2 (one stale enum variant removed, recorded) |
| Tests executed | yes — CI run 37350073234 (Rust) + static audits; user testing explicitly NOT_RUN |
| Regression checked | yes — CLI/engine crate and the 39 pre-existing GUI tests still pass |
| Evidence recorded | yes — EVIDENCE-UX-001…006 |
| No blocking issue | yes — the only open items are outside-input items with named owners and triggers |
| Handoff complete | yes — this document + 4 UX artifacts |
| Execution Result complete | yes — §10 |

## 9. Pull request

PR [#8](https://github.com/legionir/rdm/pull/8) (`arena/01a10cef-rdm` → `main`) carries the UI and
UX increments; the closure increment is pushed to the same branch. CI on the current heads:
run [37357552644](https://github.com/legionir/rdm/actions/runs/37357552644) (`15cf0ed`) and run
[37358274886](https://github.com/legionir/rdm/actions/runs/37358274886) (`58cf277`) — all jobs ✓.

## 10. Execution Result

```
Status: PASS (with one evidence class MISSING and two escalations recorded)
Verdict: The rdm interaction surface now follows an explicit, machine-checked UX policy:
  every data-destroying action asks first and names the consequence; the UI no longer
  offers actions the engine refuses (Failed→Restart, remove-while-running disabled);
  bulk actions are safe and honest about their outcome; one glossary governs the CLI,
  the GUI and the README (15 → 0 violations); every state explains itself and its next
  step. Flows, error points and undo inventory are documented with file/line evidence.
  User testing with real participants was not possible in this environment and is
  recorded as MISSING (UX-ESC-001) - no usability claim depends on it.
State: REVIEW_PENDING   (decision owner: Design Manager / PM)
Closure note: this Handoff was updated by the follow-up closure increment (PH-6). Items that could
  be closed without outside input are closed and verified (Q2 drop threshold, RISK-UX-003 in-app
  help, Q4 honest Resume-all outcome, GUI/CLI recovery copy, inventory). Q1 and Q3 remain open with
  named owners (UX-ESC-004); user testing remains open (UX-ESC-001).
Coverage: 12/12 changed code files compiled and covered by the CI test run (100%);
  4 UX artifacts + 6 evidence files + 1 checker; 12 new UX policy tests (51 #[test] counted
  in the GUI crate after the closure; 48 at the first code-green run)
Coverage Manifest: audits/ux-designer-handoff.md §2 (manifest), §3 (coverage), §4 (tests)
Decomposition: PH-0 inspection/baseline → PH-1 policy artifacts → PH-2 remediation →
  PH-3 usability evaluation → PH-4 verification (static + CI) → PH-5 handoff
Findings: FIND-UX-001..014 (3 High: unconfirmed bulk delete incl. files; unconfirmed
  restart; Resume offered where the engine refuses + bulk resume abort; plus two Medium
  closure findings: CLI-only syntax in GUI copy, and the CLI's dead-end `rdm resume`
  advice on failure). All remediated. Full list: audits/ux-usability-evaluation.md §2
Changes: created rdm-gui/src/ux.rs + 4 UX artifacts + checker + evidence; modified 11 GUI
  files, the CLI help text, the README and (out of scope, escalated) the CI workflow;
  removed one stale UiAction variant
Tests: TEST-UX-001..014 PASS (CI 37350073234 / 37350848407 + static audits), TEST-UX-016..019
  closure PASS (CI 37357552644), TEST-UX-015 user testing NOT_RUN (MISSING, UX-ESC-001)
Evidence: EVIDENCE-UX-001..006
ExecutionPlan: audits/ux-designer-execution-plan.md
Affected Locations: rdm-gui/src/{ux,app,state,backend,main}.rs, rdm-gui/src/views/*.rs,
  src/cli/commands.rs, README.md, audits/**, .github/workflows/build.yml
Critical/High Findings: FIND-UX-001, FIND-UX-002, FIND-UX-003, FIND-UX-004 (all High, remediated and re-verified)
Required Decisions: confirm the corrected bulk-resume scope (UX-ESC-002, PM);
  approve the CI diagnostics steps (UX-ESC-003, P1); decide Q1/Q3 of the flows document
  (Failed-resume policy, undo for file deletion)
Assumptions: a user downloading large files over unreliable links needs to see what is
  happening, stop/resume without losing progress, and delete data only on purpose -
  derived from the CLI/GUI feature set, since no research artifact exists (recorded)
Unknowns: official UX research/analytics artifacts (absent); measured task success;
  pixel-level review; whether users hover tooltips (RISK-UX-003)
Risks: RISK-UX-001 (confirmations off, Medium), RISK-UX-002 (no undo for file deletion,
  High - product decision), RISK-UX-003 (hover-only legend, Low), RISK-UX-004 (no manual,
  Medium), RISK-UX-005 (bulk-resume scope change, Low), RISK-UX-006 (CI-dependent
  verification, Medium)
Traceability: REQ-UX-001 → CRIT-UX-001 → DESIGN-UX-001/002/003 → IMP-UX-010…015 → TEST-UX-012 → EVIDENCE-UX-003 → ACCEPT-UX-001
              REQ-UX-002 → CRIT-UX-002 → DESIGN-UX-004/005 → IMP-UX-001…006 → TEST-UX-001…006 → EVIDENCE-UX-002/005 → ACCEPT-UX-002
              REQ-UX-003 → CRIT-UX-003 → DESIGN-UX-006/007 → IMP-UX-007…009 → TEST-UX-007…009 → EVIDENCE-UX-004 → ACCEPT-UX-003
              REQ-UX-004 → CRIT-UX-004 → DESIGN-UX-008/009 → IMP-UX-016 → TEST-UX-010…013 → EVIDENCE-UX-001/005 → ACCEPT-UX-004
Handoff: UI, Product (primary); Design Manager, PM (approval). Artifacts:
  audits/ux-flows-and-ia.md, ux-glossary.md, ux-feedback-error-undo-policy.md,
  ux-usability-evaluation.md, this document
Escalation: UX-ESC-001 (missing user-test evidence, P2), UX-ESC-002 (bulk-resume scope
  decision, P2), UX-ESC-003 (CI diagnostics step outside scope, P1 - confirmed load-bearing
  by the closure increment: three compile issues were diagnosable only through it),
  UX-ESC-004 (product decisions: resumable Failed downloads, undo for file deletion, P2)
Next Action: review/approve on PR #8; decide UX-ESC-002/003/004; then REC-UX-001…005 —
  starting with the research study, which is the only way to turn the task-success KPI
  from Unknown into data. Nothing else remains open that UX can close on its own.
```

---

## Increment 2 — desktop integration and reported defects (2026-10-05)

Trigger: the product owner reported two defects (a Windows path that doubles its backslashes on
every save; a terminal window that appears with the app) and ordered five Windows-facing features
(icon, clipboard pre-fill, tray + menu, floating drop target). Full record:
`audits/ux-feature-pack-report.md`; plan phase PH-7; flows T12 + §4 rows; glossary R10.

```
Status: PARTIAL - implemented, compiled and unit-tested; desktop behaviour unverified here
Verdict: Both defects fixed in the code and pinned by tests; F1-F5 implemented and wired into
  the flows (entry points, exits, error points documented). No usability claim is made for the
  tray, the floating target, the real clipboard path or the exe icon until QA runs the smoke
  test (UX-ESC-005).
State:   Branch arena/01a10cef-rdm, HEAD 3e0cbfb + this documentation commit; PR #8 updated.
Coverage: Bug A (escape/parse symmetry + 5-round round-trip test), Bug B (CREATE_NO_WINDOW on
  the only spawn; the windows_subsystem attribute was already correct), F1 icon pipeline,
  F2 clipboard pre-fill (+switch/tooltip), F3/F4 tray + menu + close-to-tray + Quit + no-tray
  fallback, F5 floating drop target (+drop interpretation, never-silent feedback), help
  overlay + README + flows/IA/glossary/policy.
Findings: D-11 (backslash doubling, root cause: escape without decoder), D-12 (--data-dir lost
  to the saved value), D-13 (tray events were only polled inside a frame - would have made the
  tray useless while hidden), D-14 (the audit tool's delimiter guard mis-skipped escaped char
  literals), D-15 (two CI runs never started: hosted runner unavailable), D-16 (root
  .gitignore added to keep audit bytecode out of the tree - flagged for approval).
Changes: 23 source/asset files for the features + the audit tooling/doc set; all in the Change
  Manifest of the report (no silent change).
Tests:  GUI unit tests 51 -> 75 (all compile in CI); audits/ui-contrast-check.py PASS (27
  files), audits/ux-terminology-check.py PASS (15 user-facing files).
Evidence: audits/evidence/ux-feature-pack-checks.txt, ux-feature-pack-ci-runs.json.
ExecutionPlan: audits/ux-designer-execution-plan.md - PH-7 (7.1-7.8 done, 7.9 depends on CI,
  7.10 open with UX-ESC-005).
Escalation: UX-ESC-005 (P1) - Windows-desktop smoke test for tray/target/clipboard/exe icon;
  owner QA, Design Manager informed. UX-ESC-001...004 remain open as recorded above.
Next Action: QA runs the six-step checklist in the report; Design Manager approves the new
  surface against the flows document; Product decides whether the drop target should also be
  offered on first run (today it is opt-in).
```
