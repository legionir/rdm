# rdm — Usability Evaluation

- **Artifact type:** User Test Evidence — **partial** (see §5, honesty statement)
- **Owner:** UX Designer (EXE-043) · **Reviewers:** Design Manager, Product Manager (PM)
- **Methods used:** heuristic evaluation (Nielsen), task-completion walkthrough on the interaction
  code, accessibility cross-check against the UI Designer's contrast/focus contract, automated
  policy and terminology checks
- **Method NOT used:** moderated usability testing with real participants — no participants, no
  display and no runnable build in this environment (`UX-ESC-001`). **No claim in this document
  depends on user-test data**; everything is traceable to code, static checks or CI tests.

## 1. Method & scope

| Item | Value |
| --- | --- |
| Product surface | `rdm-gui` window (7 screens) + `rdm` CLI + README copy |
| Build evaluated | branch `arena/01a10cef-rdm`, commits `3eb2def`→`15cf0ed` (CI runs [37350073234](https://github.com/legionir/rdm/actions/runs/37350073234), [37357552644](https://github.com/legionir/rdm/actions/runs/37357552644)) |
| Evaluators | UX Designer (EXE-043); design intent reviewed against the UI Designer's token/contrast contract (`audits/ui-designer-design-spec.md`) |
| Tasks walked through | T1–T10 of `audits/ux-flows-and-ia.md` §3, plus the 4 bulk-action flows |
| Evidence channels | code (file/line), static checks (`audits/evidence/`), CI test results, source-level copy inventory |

## 2. Findings (before → after)

| ID | Finding (baseline, with evidence) | Heuristic | Severity | Resolution | Evidence after |
| --- | --- | --- | --- | --- | --- |
| FIND-UX-001 | *Remove completed* removed all completed downloads — **including deleting their files** when the saved setting said so — with a single click and no dialog. Toolbar tooltip described it as “rdm remove <ID> for every completed record”. | H5 error prevention · acceptance criterion “destructive actions have confirmation and recovery” | **High** | Confirmation with the count, the file checkbox and “Cannot be undone”; zero-case short-circuit | `app.rs::AskRemoveCompleted/remove_completed_now`, `ux.rs::Confirm::RemoveCompleted`, TEST-UX-001/002/004 |
| FIND-UX-002 | *Restart from scratch* discarded partial progress (and overwrote an existing output file) with one click. | H5 | **High** | Confirmation naming progress loss and overwrite | `app.rs::AskRestart/restart_now`, `ux.rs::Confirm::Restart`, TEST-UX-001/003 |
| FIND-UX-003 | The row offered **▶ Resume for a `Failed` download**, although `Backend::resume` rejects terminal states except `Cancelled`; the click produced an error instead of progress. | H4 consistency & standards · H9 help users recover | **High** | `Failed` and `Completed` now offer ⟲ Restart (with the reason in the tooltip); ▶ is offered only where the engine accepts it | `download_list.rs` row actions, `ux.rs::failed_and_completed_never_offer_resume` (TEST-UX-006) |
| FIND-UX-004 | *Resume all* included `Failed` rows; the first failed download made the whole bulk action return an error and **left the remaining paused downloads untouched**. | H5 · H9 | **High** | The bulk action now covers exactly the states `resume()` accepts (paused/interrupted/cancelled) and reports the count | `backend.rs::resume_all`, `ux.rs::BulkAction::ResumeAll` (TEST-UX-005) |
| FIND-UX-005 | The remove dialog asked “Remove … **from the database**?” — implementation vocabulary, no statement of what is lost. | H2 match with the real world | Medium | Dialog now names the download, what is discarded, the file option and the undo status | `app.rs::confirm_dialog` + `ux.rs::Confirm::body` (TEST-UX-002) |
| FIND-UX-006 | Terminology drift across surfaces: *record / job / row / download* for the same concept; *Clear completed* / *Clear queue* hid their consequence; the status bar reported “record(s) · done · waiting”. | H4 · terminology criterion | Medium | Glossary + automated rules; status bar now “download(s) · completed · running · queued · failed · cancelled”; CLI help and README aligned | `audits/ux-glossary.md`, `ux-terminology-check.py` (15 → 0 violations), TEST-UX-008/009 |
| FIND-UX-007 | Bulk actions with nothing to do reported “0 download(s)” / “paused 0 downloads”, which reads like a failure. | H1 visibility of system status | Low | Zero-case copy says what was missing (“Nothing to pause — no download is running.”) | `ux.rs::BulkAction::nothing_to_do` (TEST-UX-005) |
| FIND-UX-008 | No explanation anywhere of what `Interrupted` vs `Paused` vs `Cancelled` vs `Failed` mean or what to do next — the four states require *different* user actions. | H2 · H6 recognition rather than recall | Medium | State legend in code, surfaced as tooltips on every state chip, the details header and the status-bar counters | `ux.rs::LEGEND`, `download_list.rs`, `details_modal.rs`, `footer.rs` (TEST-UX-007) |
| FIND-UX-009 | 🗑 was offered while a download was running although removal is refused in that state — a control that can only fail. | H5 | Medium | Disabled with `on_disabled_hover_text` explaining the order of operations | `download_list.rs`, `ux.rs::remove_tooltip` (TEST-UX-009) |
| FIND-UX-010 | `Resume all` tooltip promised “paused/interrupted/**failed**”, contradicting the CLI help (“Resume a paused/interrupted download”) and the engine. | H4 | Medium | One tooltip, matching the engine and the CLI | `ux.rs::resume_all_tooltip`, CLI help update |
| FIND-UX-011 | Details said “accept ranges: true/false” and “chunks dir”, which only the engine author understands. | H2 | Low | “server supports ranges — yes/no (what it means for resuming)”, “partial data folder” | `details_modal.rs` |
| FIND-UX-012 | Technical vocabulary in the finish gate: the details “Copy CLI command” had no explanation; the App-log pane offered no hint before the first line. | H10 help & documentation | Low | Tooltips added; empty states explain themselves | `details_modal.rs`, `footer.rs` |
| FIND-UX-013 | The GUI quoted CLI-only syntax in its own messages (“use Remove ▸ purge to delete its files”) and `remove` reported success even when there was no file left to delete. | H2 · H1 | Medium | GUI copy rewritten for the window; the outcome reports “the file was already gone” when nothing was deleted | `backend.rs` (`cancel`, `remove`), PH-6.4 |
| FIND-UX-014 | The CLI's failed summary told the user to run `rdm resume <ID>`, which the same CLI refuses for a failed download — a dead-end recovery path in the terminal. | H9 · H4 | Medium | The summary names the working continuation command and says why `resume` refuses | `src/cli/commands.rs` (`report`, Failed arm), PH-6.5 |

## 3. Heuristic evaluation summary

| # | Heuristic | Verdict after this increment | Note / residual |
| --- | --- | --- | --- |
| H1 | Visibility of system status | **PASS** | live counters, progress, speed/ETA, spinner, queue chip; zero-cases now speak |
| H2 | Match between system and the real world | **PASS** | glossary enforced; states explained in plain words |
| H3 | User control and freedom | **PASS** | `Esc` retreats through every layer; *Keep*/*Keep the progress* name the safe exit; no action hides the way back |
| H4 | Consistency and standards | **PASS** | one term per concept across CLI/GUI/README; one confirmation component |
| H5 | Error prevention | **PASS** | destructive actions ask and name the consequence; impossible controls are disabled with a reason |
| H6 | Recognition rather than recall | **PASS** | legend on every state; tooltips on every icon-only control; defaults inherited from Settings |
| H7 | Flexibility and efficiency of use | **PASS** | keyboard map (↑/↓, Enter, Esc, Ctrl+F, F5), bulk actions, tooltips naming the CLI equivalent, `Copy CLI command` |
| H8 | Aesthetic and minimalist design | **PASS** | UI increment: token-driven palette/hierarchy; optional columns drop before the file name |
| H9 | Help users recognise, diagnose and recover from errors | **PASS** | error policy §4: message + what was kept + next step; Failed→Restart path fixed |
| H10 | Help and documentation | **PASS (note resolved)** | in-context tooltips **and** an in-app help window (`F1`/ⓘ: states, keyboard, vocabulary) + README; an external manual remains out of scope (`RISK-UX-004`, downgraded to Low) |

No heuristic has an open “no finding” row: each was examined and produced either a finding
(FIND-UX-001…012) or a documented pass with the evidence above.

## 4. Accessibility cross-check (with the UI Designer's contract)

| Check | Result | Evidence |
| --- | --- | --- |
| Contrast of every user-facing state/copy colour on every surface (both themes) | PASS — worst pair 5.01:1 | `audits/evidence/ui-contrast-check.txt` |
| Focus ring visible on all surfaces (≥ 3:1, SC 1.4.11) | PASS | same |
| Colour is never the only signal for a state | PASS — glyph + label + tooltip on every chip | `ux.rs::LEGEND`, `theme::status_chip` |
| Every icon-only control has an accessible name (tooltip) | PASS | row actions, sidebars, dialogs (`components::icon_button`) |
| Keyboard-only path for every task | PASS — ↑/↓ select, Enter details, Esc retreat, Ctrl+F search, F5 refresh, Tab order native | `app.rs::handle_shortcuts`, `toolbar.rs` |
| Selected vs hovered distinguishable without colour alone | PASS — 3 px accent bar | `download_list.rs`, UI increment |
| Error messages are text, not colour-only | PASS | `palette.status_color` + sentence in the status bar |
| Premature-content checks (SC 2.2.1 timing) | **NOT_APPLICABLE** — no time limits are imposed; refresh is a passive poll (reason recorded) | `app.rs::request_repaint_after` |
| Help reachable without hovering | PASS (closes `RISK-UX-003`) | `F1` / toolbar ⓘ → `views/help_overlay.rs`; the empty list also names the first step and `F1` in text (`download_list.rs`) |
| Help content complete (every state, every shortcut, every glossary term) | PASS | `ux::help_covers_states_shortcuts_and_vocabulary` (unit test) |

## 5. Honesty statement: what is verified and what is not

| Evidence class | Status | Consequence for claims |
| --- | --- | --- |
| Code-level flow analysis, task walkthrough, heuristic evaluation | **VERIFIED** (file/line evidence) | findings may be stated as design facts |
| Policy/copy checks (unit tests + static rules) | **VERIFIED** (CI run 37350073234; `audits/evidence/ux-terminology-report.txt`) | the policy holds as written |
| Contrast/focus contract | **VERIFIED** (UI increment, re-checked here) | accessibility claims restricted to contrast/focus/semantics that are machine-checked |
| **User Test Evidence** (real participants: task success, error rate, time on task, quotes) | **MISSING — not obtainable in this environment** | **no usability claim is made on these metrics**; `KPI: task success = Unknown` |
| Visual/pixel-level review | **MISSING** — no display, no build | layout claims rest on the responsive tests, not on a screenshot |

`UX-ESC-001` requests the environment/participants needed to close this gap; `UX-ESC-004` carries the
two remaining product decisions (Q1 failed-resume policy, Q3 undo for file deletion), which UX cannot
close on its own because they change product/engine behaviour. Recommended (not
executed here): a 5-participant remote study on T1/T3/T7, plus task-success metrics; the flows are
written so such a study can be scripted from them directly.

## 6. KPIs (evaluation only — no artificial targets)

| KPI | Baseline | After | Status |
| --- | --- | --- | --- |
| Terminology violations across CLI + GUI + README (static) | 15 | **0** | verified, `audits/evidence/ux-terminology-baseline.txt` → `ux-terminology-report.txt` |
| Destructive actions without confirmation (policy) | 2 (bulk remove, restart) | **0** | verified, `ux.rs` tests |
| Affordances that can only fail (dead ends) | 2 (Resume on Failed, Remove while running) | **0** | verified, `ux.rs` tests + row-action code |
| States without documented meaning/next step | 8 | **0** | verified, `ux.rs::every_state_has_a_legend_entry_with_a_next_step` |
| Task success rate with real users | Unknown (no data) | **Unknown** | `UX-ESC-001` |
| Help unavailable without hover (RISK-UX-003) | 1 (hover-only legend) | **0** | verified, `help_overlay.rs` + unit test |
| Destructive-action coverage incl. bulk queue drop | *Drop all* unconfirmed at any length | **0** above the 5-item threshold | verified, `ux::a_short_queue_is_dropped_at_once_a_long_one_asks` |
| Dead-end recovery paths (failed → `resume`) | 2 (GUI + CLI) | **0** | verified, `ux.rs::failed_and_completed_never_offer_resume`, `cli/commands.rs` Failed arm |
