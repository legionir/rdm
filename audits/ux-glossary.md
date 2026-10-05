# rdm — UX Glossary & Terminology Rules

- **Artifact type:** UX Designs (terminology / IA input for UI and Product)
- **Owner:** UX Designer (EXE-043) · **Approvers:** Design Manager, Product Manager (PM)
- **Consumers:** UI, Product, Frontend (copy in code: CLI help, GUI strings, README)
- **Enforcement:** `rdm-gui/src/ux.rs::GLOSSARY` (in code, unit-tested) and
  `audits/ux-terminology-check.py` (static rule set over CLI + GUI + README)
- **Status:** implemented and verified — see `audits/evidence/ux-terminology-report.txt`

## 1. Why a glossary

The product speaks to users in three places — the terminal (`rdm …`), the window, and the
README — and before this increment they disagreed: the same thing was a *download*, a *record*,
a *job*, a *transfer* and a *row* depending on where you looked. A user who reads “removed
download” in the CLI and “Clear completed” in the window cannot tell whether the same thing
happened. The glossary fixes one preferred term per concept and forbids the rest in user copy.

## 2. Core vocabulary

| Preferred term | Meaning (plain words) | Do **not** use | Where it appears |
| --- | --- | --- | --- |
| **download** | one file the user asked rdm to fetch; it survives restarts of the app | job, record, row, item, task | everywhere (list, queue, status bar, CLI, README) |
| **queue / queued** | downloads that are accepted but not started yet | pending list, waiting list | toolbar chip, Queue sidebar, status bar |
| **partial data** | the bytes already fetched for an unfinished download | temp files, part files | Remove dialog, details overview, Chunks tab (kept as the technical view name) |
| **finished file** | the assembled output file on disk | output artifact, result file | Remove dialog, details, README |
| **Remove** | take a download out of the list; the finished file stays unless the user says otherwise | Delete (for downloads), Clear | row 🗑, *Remove completed…*, CLI `rdm remove` |
| **Drop** | take a *queued* download out of the queue before it starts (nothing has been downloaded) | Cancel, Clear | Queue sidebar ✕, *Drop all*, toolbar chip |
| **Pause** | stop a running transfer, keeping its progress | Suspend, Hold | row ⏸, *Pause all*, CLI `rdm pause` |
| **Resume** | continue a download where it stopped | Continue (as a button label), Retry | row ▶, *Resume all*, CLI `rdm resume` |
| **Restart** | download again from the beginning (progress is discarded, the file is overwritten) | Retry, Redo, Reload | row ⟲, dialog *Restart from scratch* |
| **Details** | the technical view of one download (state, sizes, partial data, events, JSON) | Info (as a window title), Properties | row double-click / Enter, `rdm info` |

### State vocabulary (the same words everywhere)

`Queued · Running · Merging · Paused · Interrupted · Completed · Cancelled · Failed`

The definitions and their next step live in `rdm-gui/src/ux.rs::LEGEND` and are surfaced as
tooltips on every state chip, on the details header and on the status-bar counters. Two pairs are
deliberately distinguished, because the user's next action differs:

* **Paused** (the user asked for it) vs **Interrupted** (something went wrong, progress is kept);
* **Cancelled** (the user stopped it; the partial data is kept and *Resume* continues it) vs
  **Failed** (the transfer gave up; only *Restart* can recover it — `Backend::resume` rejects
  terminal states except `Cancelled`).

## 3. Copy rules (enforced)

| # | Rule | Checked by |
| --- | --- | --- |
| R1 | Name the action; never “OK”/“Yes”. Buttons read `Remove`, `Drop`, `Restart`, `Keep`, `Keep the progress`. | `ux.rs::the_safe_option_is_named_after_what_it_keeps` |
| R2 | State the consequence, not a question: no “Are you sure?” anywhere. | `ux-terminology-check.py` (banned phrase) |
| R3 | Every destructive dialog ends with the undo status (“Cannot be undone.”) or says the action is recoverable. | `ux.rs::confirmations_state_the_consequence_and_the_undo_status` |
| R4 | A count is always reported (“Removed 3 completed download(s)…”); a zero result says what was missing, never “0 download(s)”. | `ux.rs::bulk_actions_name_the_count_and_explain_the_empty_case` |
| R5 | Never expose implementation vocabulary (record, row, job, database, sqlite) in user copy. | `ux.rs::no_user_facing_copy_uses_implementation_jargon`, `ux-terminology-check.py` |
| R6 | The same concept uses the same word in CLI help, GUI and README. | `ux-terminology-check.py` §3 |
| R7 | Every icon-only control has a tooltip that names the action (and the CLI equivalent where useful). | UI increment (`components::icon_button`) + review |
| R8 | Errors say what happened, what was kept, and what to do next. | §4 of the feedback/error/undo policy |

## 4. Documented exceptions (terms that are allowed to differ)

| Term used elsewhere | Where | Why it is allowed |
| --- | --- | --- |
| `chunk`, `Chunks` tab | details modal, `TEST_INVENTORY.md`, debug output | the technical view of *partial data*; the tab is explicitly the low-level view. The Remove dialog and overview use *partial data*. |
| `metadata.db`, `SQLite`, `--data-dir` | README (developer sections), CLI error messages for advanced flags | developer documentation and advanced options; not part of the everyday user flow. |
| `pending` | `UiAction::CancelPending`, internal queue API | code identifier, never rendered. |
| `record` | `rdm info --json` payload fields, engine logs | the JSON payload mirrors the database schema (machine-readable contract); the GUI never shows it as prose. |

## 5. Baseline → after

The static check over CLI help, the GUI strings and the README reported **15 violations** before
this increment and **0** after (`audits/evidence/ux-terminology-baseline.txt` vs
`audits/evidence/ux-terminology-report.txt`). The violations were not stylistic: three of them
(record/row/job) appeared in the *same sentence* as state counters, and two (`Clear completed`,
`Clear queue`) described a data-destroying action with a label that hid its consequence.
