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

## PH-7 — Reported defects + desktop integration (2nd follow-up increment) `[🟡]`

Trigger: the product owner reported two defects (a Windows path that doubles its backslashes,
a terminal window that appears with the app) and asked for five Windows-facing features (icon,
clipboard pre-fill, tray, tray menu, floating drop target) on 2026-10-05.

| Step | Work | Status |
| --- | --- | --- |
| 7.1 | Bug A — `escape()`/`parse()` symmetry (only what the loader understands), five-round round-trip test, legacy over-escaped files still load; `--data-dir` no longer loses to the saved value | 🟢 |
| 7.2 | Bug B — `CREATE_NO_WINDOW` on the only process spawn in the GUI; grep-verified that no other spawn exists | 🟢 |
| 7.3 | F1 — deterministic icon generator (stdlib only), `.ico` embedded by `build.rs`, one RGBA buffer for window + tray, tests that fail on a broken asset | 🟢 |
| 7.4 | F2 — clipboard pre-fill (toolbar + tray), URL field focused and selected, switch and tooltip that follow the setting, empty/invalid clipboard explained in the log | 🟢 |
| 7.5 | F3/F4 — tray icon and menu (Show · New download · Pause all · Resume all · drop target · Quit); Close hides and Quit exits; no-tray fallback keeps the normal close | 🟢 code-complete; desktop behaviour pending `UX-ESC-005` |
| 7.6 | F5 — floating drop target above the clock area (work-area + DPI), drop interpretation (text, uri-list, `.url`/`.txt`), never-silent feedback, ✕ hides and turns the setting off | 🟢 code-complete; desktop behaviour pending `UX-ESC-005` |
| 7.7 | Flows, IA, glossary, feedback/undo policy and the in-app help updated for the new surface (entry points, exits, error points) | 🟢 |
| 7.8 | Audits re-run and extended; a real defect in the delimiter guard fixed (escaped char literals) | 🟢 |
| 7.9 | CI: compile + 75 GUI unit tests (was 51) + CLI regression | 🟢 run **37369744644** (`b478f4b`): `build-gui-windows` ✓ (Build GUI binary · Test GUI crate · Stage · Upload) and `test-windows` ✓, after four annotation-driven fix rounds (37361180868, 37361634987, 37362408563, 37366518834) and three runs that never reached a runner (37362918254, 37364680078/37364684573, 37368129718/37368133806 — GitHub could not acquire a hosted Windows runner); the `pull_request` duplicate of the green commit stayed queued |
| 7.10 | Windows-desktop smoke test (tray, target anchor under scaling, real clipboard, exe icon, no console window) | 🔴 open — `UX-ESC-005` (QA) |

## PH-8 — Round-3 report: the tray freeze and the “standard heights” pass (3rd follow-up) `[🟡]`

Trigger: the product owner reported on 2026-10-06, with three screenshots: Clear/Copy button
sizes, ✕/ⓘ/↑↓ glyphs rendering as empty boxes, unequal heights across the toolbar row and
between the Settings folder-picker button and its input, Settings needing to be a **real window
with tabs** plus a corner ✕ (Help too), a general “one height for text inputs, dropdowns, buttons
and everything else”, the tray going dead (“no menu item works and the window will not come
back”), and the floating target needing to be a **small circle carrying only the logo** that still
accepts a dropped link while the app sits in the tray.

| Step | Work | Status |
| --- | --- | --- |
| 8.1 | Drawn icons instead of font glyphs (✕, ⓘ, ✓, ↑↓, ▾) everywhere, with the audit rule that bans exotic codepoints; footer Clear/Copy and the toolbar row follow the same control height | 🟢 (PH-2/PH-3 work, re-verified) |
| 8.2 | One control height (**24 pt**) for buttons, icon buttons, text inputs, combo boxes and the folder pickers; `components::text_edit` pins it with `min_size(0, control_height)`; the layout test measures the rect the control *occupies* | 🟢 run **37522424155** |
| 8.3 | Settings and Help are real windows (decorated, resizable, their own ✕), Settings with tabs and a Save/Reload footer | 🟢 code-complete; appearance pending `UX-ESC-005` |
| 8.4 | Floating target: a small transparent circle carrying **the app's own embedded icon** and nothing else — no label, no tooltip (52 pt since round 6; 76 pt and a drawn glyph before); a drop opens the pre-filled New-download form | 🟢 code-complete; desktop drop pending `UX-ESC-005`, and a real **browser-link drag** is `UX-ESC-006` |
| 8.5 | Tray freeze — root cause: `ViewportCommand::Visible(false)` clears `WS_VISIBLE`, Windows then sends no `WM_PAINT`, the frame loop stops for good, and every tray command is read inside a frame that can never come; minimizing is no escape either (nothing visible → nothing painted) | 🟢 (source-diagnosed: egui #5127/#737, winit 0.30 event-loop note, MSDN) |
| 8.6 | Fix, layer 1 — hide the window the way winit hides its own helper window: keep `WS_VISIBLE` and take it off screen with the layered style (alpha 0, click-through, out of taskbar and Alt-Tab) via `windows::hide_main_window`; `reveal_main_window` undoes it from any thread | 🟢 |
| 8.7 | Fix, layer 2 — the tray owns two blocking relay threads (menu, icon) that queue commands, wake the UI, and restore the window through the OS when the command is about the window or when `frames::is_stale()` (≈1 s) says the UI stopped delivering frames | 🟢 |
| 8.8 | Fix, layer 3 — every viewport is cleared transparent, so the floating target is a mark instead of a dark square; the frame clock is noted at the top of `update()` | 🟢 |
| 8.9 | CI round 1 — two annotation-driven fixes: `HWND` is an `isize` in `windows-sys` 0.52 (not a pointer), and `&Arc<Wake>` does not coerce to `&Wake` in argument position | 🟢 runs 37520509055 / 37520515850 → 37521061557 / 37521068081 |
| 8.10 | CI round 2 — the failing height test now reports **which** control and by how much (the annotation filter needed the message to start with `assertion:`); it named the text input at 16 pt, which is `TextEdit` shrinking its reply to the inner rect (egui 0.29 `text_edit/builder.rs:416`) — the control itself is 24 pt | 🟢 runs 37521816137 / 37521825816 → **37522424155 / 37522433273** green (`8b03d25`) |
| 8.11 | Windows-desktop smoke test of the round trip (close → tray → Show, a drop on the target while the window is hidden, the heights on a real desktop) | 🔴 open — `UX-ESC-005` (QA) |

## PH-9 — Round-3 re-report: the tray freeze was NOT fixed, and why `[🟡]`

Trigger: the product owner re-reported on 2026-10-07, with the same symptom and a new detail —
“when the app goes to the tray none of its menu items work; it is as if it has hung completely,
and it has to be ended from the Task Manager”. `df2a4eb` (PH-8) had shipped as green CI, so a
green build is not evidence here: the report is about a Windows desktop, which this environment
does not have. The honest reading is that PH-8’s fix addressed the *hide* technique but left the
freeze in place, and PH-8.11 (the desktop smoke test) would have caught it.

| Step | Work | Status |
| --- | --- | --- |
| 9.1 | Re-open PH-8 on the evidence: the fix kept `WS_VISIBLE` but called `SW_MINIMIZE` in the same function (“give the keyboard back”) — and a **minimized window has nothing to paint**, so `WM_PAINT` stops, `App::update` stops, and the tray commands that were only *queued* wait forever. The menu still opens because the OS draws it on the app’s message pump: the app looks alive and does nothing, which is exactly the report | 🟢 root cause |
| 9.2 | `windows::hide_main_window` no longer minimizes and no longer clears `WS_VISIBLE`: alpha 0, click-through, tool-window, `WS_EX_NOACTIVATE`, z-order to the bottom. The window stays “on screen” to Windows, so paints keep coming — and nothing below depends on them | 🟢 |
| 9.3 | The relay threads now **act** instead of queueing: *Show* and *New download* restore the window through the OS themselves; the state-dependent commands stay in the app and are rescued with a reveal when `frames::is_stale()` | 🟢 |
| 9.4 | *Quit* always quits: the relay restores the window and arms a deadline (`FORCE_QUIT_MS` = 8 s, longer than the 5 s graceful shutdown) after which the process ends itself, logging why — “end it from the Task Manager” is no longer the only way out of a wedged app | 🟢 |
| 9.5 | The window handle is captured twice (start-up context, then the first frame) and, failing both, **searched for** among this process’s own unowned top-level windows, ranked by title (`RDM` = 2, `rdm — …` = 0), validated with `IsWindow` before every use | 🟢 |
| 9.6 | Diagnostics, because the report has to be answerable from a frozen desktop: `rdm-gui.log` in the data directory receives every line (the window that shows the app log is the window a freeze takes away), the tray relays log every command and whether it was armed, and the hide decision logs its result | 🟢 |
| 9.7 | The hidden-window heartbeat drops to 1 s and `frames::STALE_MS` rises 1 s → 2 s, so a healthy hidden app can never be mistaken for a dead one (which would pop the window up on every click) | 🟢 |
| 9.8 | Guards: no `SW_MINIMIZE` / `Minimized(true)` / `Visible(false)` may appear in the tray path (comments stripped before searching), hiding never minimizes, only the main title wins the window search, the Quit deadline outlives the graceful shutdown, and the log file receives every captured line | 🟢 |
| 9.9 | CI — three annotation-driven rounds: `SWP_NOZORDER` missing from an import list, `Option<Arc<PathBuf>>` returned where `Option<&Path>` was promised, and two failing tests (the file write was in the wrong function; the `SW_MINIMIZE` guard matched its own prose) | 🟢 runs 37534564654 / 37535011455 → **37535678990 / 37535686161** green (`668166d`), 104 tests |
| 9.10 | Windows-desktop smoke test with the new binary, **including the log file checkout** (close → tray → each menu item → Quit; then read `rdm-gui.log` for the tray lines) | 🔴 open — `UX-ESC-005` (QA / product owner) |

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
| D-11 | Every save/load cycle doubled the backslashes of a Windows path (`download_dir = C:\download\rdm` → `C:\\download\\rdm` → …) | Reported by the product owner; root-caused in PH-7.1: `escape()` had no matching decoder in `parse()` | 🟢 |
| D-12 | `--data-dir DIR` was silently replaced by the value stored in the settings file (and thus lost across reloads) | Found while fixing D-11 and testing the parser; the flag describes the run, so it now wins and the file keeps its own directory | 🟢 |
| D-13 | The first version of the tray read its events only inside a frame — a window hidden in the tray runs a frame only when asked, so *Show* / *New download* would have reacted late or never | Found by reviewing the tray code before CI; fixed with two relay threads that map events to commands and wake the window (`Context::request_repaint_of(ROOT)`) | 🟢 |
| D-14 | The delimiter guard in `audits/ui-contrast-check.py` skipped one character too many for escaped char literals (`'\n'`), reporting `{`/`}` as unbalanced in files that use them | Found by running the guard on the new files; the guard is itself evidence, so a false FAIL would have been a false alarm on a green tree | 🟢 |
| D-15 | Two CI runs never started: “The job was not acquired by Runner of type hosted even after multiple attempts” (GitHub infrastructure) | Recorded so the red run list is not read as a code failure; re-triggered by pushing (the workflow file is out of scope, `UX-ESC-003`) | 🟢 (recorded) |
| D-16 | A root `.gitignore` was added to keep `audits/__pycache__/*.pyc` (produced by running the audit scripts) out of the tree | Scope note: a new file at the repository root, flagged for approval; nothing existing was changed and no bytecode was ever committed | 🟢 (flagged) |
| D-17 | Hiding the window with `ViewportCommand::Visible(false)` (and equally with `Minimized(true)`) stops `WM_PAINT`, so `App::update` never runs again — and `App::update` is where tray commands are applied. That is the whole round-3 freeze, and no heartbeat inside `update()` could have fixed it | Reported by the product owner; root-caused in PH-8.5 against egui/winit sources and MSDN before touching code | 🟢 PH-8.5-8.8 |
| D-18 | A tray relay that only *queues* is not enough: the two commands that are about the window must work with no frame at all, so the relay thread itself restores the window through the OS (`windows::reveal_main_window`), and `frames::is_stale()` extends that to every command if the UI ever stops | Found while designing the fix; without it “Show rdm” would still depend on the frame loop it is supposed to rescue | 🟢 |
| D-19 | `windows-sys` 0.52 defines `pub type HWND = isize`, not a pointer — the first CI compile failed on every cast | Annotation from run 37520509055; fixed with the raw `isize` and `0` for `SetWindowPos`'s insert-after | 🟢 8.9 |
| D-20 | `deliver(cmd, &inbox, &menu_wake)` does not coerce `&Arc<Wake>` to `&Wake` in argument position; the relay threads now pass `&*menu_wake` | Annotation from the same run; the test’s `&*wake` form was already correct | 🟢 8.9 |
| D-21 | egui 0.29’s `TextEdit` deliberately shrinks its `Response::rect` to the **inner** text rect (`text_edit/builder.rs:416`, with a TODO to return the outer rect), so the height test was measuring 16 pt while the control on screen was 24 pt | The failing assertion in run 37521816137 named it with numbers once the message was readable; documented on `components::text_edit` so no caller aligns to the wrong rect | 🟢 8.2 / 8.10 |
| D-22 | The Windows job surfaces a failing test as a workflow annotation filtered by `^assertion`/`^left:`/`^right:`, so the *numbers* in a panic message were invisible; the assertion now starts with `assertion:` and lists every measurement | Two runs were spent guessing which control was off; the diagnostic change is what turned the third run into a one-line answer | 🟢 8.10 |
| D-23 | **A fix can be green in CI and still not fix the report.** PH-8 kept the window visible to Windows but minimized it in the same function; CI compiles and tests, it does not sit in a tray | The product owner re-reported the identical freeze after `df2a4eb`; the desktop smoke test (PH-8.11/9.10) is the gate that was missing, and it stays open | 🟢 recorded |
| D-24 | Queueing a command is not the same as applying it: with no frames, the queue is read by nobody — including the command that would restore the frames | Re-read of `tray.rs` against the report: the relay threads *received* the clicks and *wrote* them where only a dead thread would read them | 🟢 9.3 |
| D-25 | The tray’s own *Quit* could not save the user either, for the same reason, so “end it from the Task Manager” was the only exit | The report names that workaround explicitly; a tray app must never require it | 🟢 9.4 |
| D-26 | The App-log tab is not a diagnostic channel for this class of defect — the freeze is what makes that window unreachable | The reason `rdm-gui.log` now exists, one line per open, flushed as written | 🟢 9.6 |
| D-27 | `GuiState::push_log` never reached the tracing buffer, so the app’s own status lines (including “window hidden…”) were absent from any log file; the tray decisions now log through `tracing` as well | Found while wiring the log file: a diagnostic file that misses the decisive line is not a diagnostic file | 🟢 9.6 |
| D-28 | The `SW_MINIMIZE` guard failed on its own prose (the module *documents* the trap) — the same self-match class as the glyph audit; comments are stripped before searching | Annotation from run 37535011455 | 🟢 9.8/9.9 |

## Verification results (final)

| Gate | Result | Evidence |
| --- | --- | --- |
| UX policy unit tests (confirmations, copy, glossary compliance, legend/microcopy) | PASS | CI run `build-gui-windows` → `Test GUI crate` |
| Terminology check (rules + exceptions) | PASS | `audits/evidence/ux-terminology-report.txt` |
| Rust compile + GUI tests | PASS | CI runs 37350073234 (`3eb2def`) and 37350848407 (`47253af`); CLI regression job `test-windows` ✓ |
| CLI/engine regression | PASS | CI run → job `test-windows` |
| Baseline (before) evidence | recorded | `audits/evidence/ux-baseline-*.txt` |
| Desktop integration (increment 2): flows, copy, help and README | PASS (static) | `audits/ux-feature-pack-report.md`, `audits/evidence/ux-feature-pack-checks.txt` |
| Bug A round-trip (5 save/load cycles, legacy files) | PASS | `settings.rs` unit tests → CI `Test GUI crate` |
| Tray / floating target / real clipboard / exe icon on a Windows desktop | **NOT RUN — NOT_APPLICABLE in this environment** | `UX-ESC-005` (QA smoke-test checklist) |
| Control heights (button · icon button · text input · combo box · labelled button = 24 pt) | PASS | CI run **37522424155** (`build-gui-windows` → `Test GUI crate`); the red run 37521816137 carries the message that named the text input at 16 pt |
| Tray-freeze fix compiles and every GUI test passes on Windows | PASS | CI runs **37522424155** / **37522433273** (`8b03d25`): `test-windows` ✓, `build-gui-windows` ✓, artifact `rdm-gui-windows-x86_64.exe` (5,808,400 bytes) |
| Tray freeze fix (round 4) compiles and all 104 GUI tests pass on Windows | PASS | CI runs **37535678990** / **37535686161** (`668166d`): `test-windows` ✓, `build-gui-windows` ✓, artifact `rdm-gui-windows-x86_64.exe` (5,819,368 bytes) |
| The tray works with no frames at all (Show / New download / Quit from the relay threads) | **NOT RUN — NOT_APPLICABLE in this environment** | `UX-ESC-005`: the desktop test, with `rdm-gui.log` as the evidence to return |
| Tray round trip (close → tray → Show / New download, drop on the target while hidden) on a real desktop | **NOT RUN — NOT_APPLICABLE in this environment** | `UX-ESC-005` (QA smoke test; the artifact above is the hand-off) |
| A link dragged from a browser onto the mark (round 6) | **NOT RUN — NOT_APPLICABLE in this environment** | `UX-ESC-006`: the transport is proven from `winit`'s own drop handler (one format only), pinned by the COM layout tests, and the run's exe is the hand-off |
| The mark's circle and the drop target compile, and all **119** GUI tests pass on Windows (round 6) | PASS | CI runs **37553244501** / **37553248764** (`a859ebc`): `test-windows` ✓, `build-gui-windows` ✓, artifact `rdm-gui-windows-x86_64.exe` (5,821,355 bytes) |
| User testing | **MISSING — NOT_APPLICABLE in this environment** | `UX-ESC-001` |

## Rule compliance notes

- Completed steps are preserved; nothing is deleted or silently rewritten.
- A phase is 🟢 only when all its steps are 🟢 and the acceptance criteria pass.
- Every `NOT_APPLICABLE` carries a reason; every escalation names its target.
