# rdm — Feedback, Error & Undo Policy

- **Artifact type:** UX Designs (interaction policy)
- **Owner:** UX Designer (EXE-043) · **Approvers:** Design Manager, Product Manager (PM)
- **Consumers:** UI, Frontend, Product
- **Implementation:** `rdm-gui/src/ux.rs` (policy data + unit tests), `rdm-gui/src/app.rs`
  (dialogs and action loop), `rdm-gui/src/views/*.rs` (tooltips, empty states, disabled states)
- **Verification:** `audits/ux-terminology-check.py`, `ux.rs` unit tests, CI run 37350073234

## 1. Principles

1. **Every action answers.** No click is silent: each one either changes the screen, or writes a
   sentence to the status bar.
2. **Nothing destructive without a named decision.** Removing or restarting asks first, names what
   is lost and whether it can be undone.
3. **Undo by design where undo exists.** rdm has real recovery paths; they are offered next to the
   destructive action instead of a “Cancel” that hides them: *Pause* keeps the transfer, *Cancel*
   keeps the partial data, *Resume* continues, *Restart* starts over.
4. **The UI never offers an action the engine will refuse.** Affordances match the verified
   behaviour of the bridge (`backend.rs`) — a disabled button explains the order of operations
   instead of failing after the click.
5. **Hover is help — but not only hover.** Every state, every icon and every counter has a
   one-line explanation, and the same content is reachable without hovering through the in-app
   help (`F1` / ⓘ: states, keyboard map, vocabulary).

## 2. Interaction contract (action → feedback → recovery)

| Action | Entry points | What happens | Feedback | Recovery / undo |
| --- | --- | --- | --- | --- |
| Add download | *New download* (dialog) | validated against the same rules as `rdm download` | inline banner with the exact reason; status bar `queued <url>` | close the dialog, nothing is created on failure |
| Pause | row ⏸, *Pause all* | state → `Paused`, transfer stops | `Pause requested for N download(s).` / *Nothing to pause — no download is running.* | **Resume** continues from the same byte offset |
| Resume | row ▶, *Resume all* | continues where it stopped | `Continuing N download(s).` / *Nothing to continue — no download is paused, interrupted or cancelled.* | **Pause** again; progress is durable |
| Cancel | row ⏹ | state → `Cancelled`, partial data kept | `Cancel — stop the transfer; the partial data is kept` (tooltip) + status message from the bridge | **Resume** continues it; **Remove** discards it |
| Restart | row ⟲ → dialog | chunks wiped, re-download from the beginning | dialog: *“discards the progress … overwrites the file at the output path. Cannot be undone.”* | none (by nature) → hence the confirmation |
| Remove | row 🗑 → dialog | list entry removed, partial data discarded | dialog names the download, the consequence and the checkbox for the file | none → the file is only deleted when explicitly ticked |
| Remove completed | *Remove completed…* → dialog | bulk removal | dialog names the **count**; outcome `Removed N completed download(s) from the list.` / *Nothing to remove — no completed download in the list.* | none → confirmation, and the file checkbox defaults to the saved setting |
| Drop (queue) | queue ✕, *Drop all*, toolbar chip | queued item removed before it starts; *Drop all* asks when ≥ 5 are queued (`DROP_ALL_CONFIRM_THRESHOLD`) | `Dropped <url> from the queue.` / *Nothing to drop — the queue is empty.* | re-add the download; nothing was downloaded yet |
| Open folder | row 📂, details | OS file manager opens the output directory | `opened <dir>` or a clear error (`cannot open folder: …`) | — |
| Apply data directory | Settings | re-opens `metadata.db` from another folder | `using metadata in <dir>` | the previous directory is untouched on disk |

The policy table lives in `ux.rs::DESTRUCTIVE` and is asserted by
`every_data_destroying_action_has_a_confirmation_policy`; the confirmation content is asserted by
`confirmations_state_the_consequence_and_the_undo_status`.

## 3. Confirmation dialogs

One dialog component, driven by `ux::Confirm` (title, consequence text, destructive label, safe
label, whether the file checkbox applies):

| Policy | Title | Consequence text (essence) | Destructive / safe action | File checkbox |
| --- | --- | --- | --- | --- |
| `RemoveOne` | Remove download | removes it from the list, discards the partial data, ticking the box also deletes the finished file, *cannot be undone* | `Remove` / `Keep` | yes |
| `RemoveCompleted` | Remove completed downloads | same for **N** completed downloads, names the count, *cannot be undone* | `Remove` / `Keep` | yes |
| `Restart` | Restart from scratch | discards the progress, downloads from the beginning, overwrites the file, *cannot be undone* | `Restart` / `Keep the progress` | no (nothing is deleted) |
| `DropAll` | Drop all queued downloads | drops **N** queued downloads before they start, states that no partial data exists, *cannot be undone* | `Drop them` / `Keep them` | no |

Rules applied to all of them: the destructive button is coloured `danger`, the consequence is shown
as a `warning` banner, the safe option is named after what it keeps, `Esc` closes the dialog
(the safe path, added in the UI increment), and the download is identified by `dl-… (filename)` so
a mis-click on the wrong row is visible before confirming.

**Settings switch.** `confirm_remove` (Settings → Application, label *Confirm destructive
actions*) governs the dialogs (all except the bulk queue drop, which is governed by the queue
length — it discards nothing that was fetched). Default **on**. Switching it off makes the same actions act
immediately — the consequences are then reported as the action's status message. The TOML key is
unchanged (`settings.toml` stays compatible).

## 4. Error policy

| Class | Example trigger | User sees | Kept state | Next step offered |
| --- | --- | --- | --- | --- |
| Validation (before any work) | empty or non-http URL, bad checksum, unparsable size/speed | inline `danger` banner in the dialog with the engine's own message | nothing was created | correct the field, press `Start` again |
| Runtime, recoverable | connection dropped, timeout, checksum mismatch, server without ranges | state `Interrupted`/`Failed` in the list; engine message in the status bar and in **Events**; the details `last error` field carries the full text | partial data is kept | *Resume* (interrupted) / *Restart* (failed) |
| Configuration | invalid log level, unwritable settings file, unreachable metadata directory | status bar in `danger` colour + entry in **App log** | previous configuration stays in effect | fix the value (settings are re-read every 600 ms, so saving in an editor is enough) |
| Blocked action | 🗑 while running, ▶ on a failed download | the button is absent or disabled, tooltip names the order of operations | — | *pause/cancel first*, or *Restart* |
| Bulk action with nothing to do | *Pause all* with no active transfer | `Nothing to pause — no download is running.` | — | — |

Error copy rules: no stack traces, no `Err(...)`/`anyhow` chain in the primary line (the full
chain is kept in **App log** for bug reports), always the *what* + *what was kept* + *what to do*.
Errors never appear only as a colour: every one is a sentence (`palette.status_color` sets the
tone, the text carries the meaning).

## 5. Undo inventory (what is recoverable, honestly)

| Recoverable | Mechanism | Evidence |
| --- | --- | --- |
| Stopping a transfer | *Pause* / *Cancel* keep the partial data; *Resume* continues | `backend.rs::pause/cancel/resume`, engine chunk persistence |
| Closing the window mid-transfer | owned transfers are paused before exit (5 s grace, then `Interrupted`) | `app.rs::on_exit`, `backend.rs::shutdown` |
| Deleting data by accident | prevented: remove/restart ask first; the file is deleted only with an explicit checkbox; emptying a long queue asks too | `ux.rs` policy + dialogs |
| Losing a queued download | nothing was downloaded; re-add it (above the threshold the dialog warns first) | `backend.rs::clear_queue`, `ux::drop_all_confirm` |

| Not recoverable (and therefore confirmed) | Why |
| --- | --- |
| Removing a list entry and its partial data | chunk files are deleted with the entry (`Storage::delete_download` + chunk dir removal) |
| Deleting the finished file | removal from disk |
| A restart's overwrite of the output path | the file is replaced by the new download |

## 6. Deliberate trade-offs

1. **No global “Undo” toast.** An undo stack for file deletion would require keeping detached
   files; the cheaper and more honest protection is the confirmation plus an explicit file
   checkbox (see `RISK-UX-002` for the residual risk).
2. **Queue drops are confirmed only when the queue is long.** Nothing has been downloaded yet and
   the queue item can be recreated by pasting the URL again, so a dialog on every ✕ would punish
   the common case; but emptying *five or more* queued downloads in one click is an easy mistake
   with a real cost, so that case asks (`ux::drop_all_confirm`). The threshold is one constant;
   a user study can move it. If regret shows up on single drops, raising them to a confirmation is
   a one-line policy change (`DESTRUCTIVE`, `Confirm::None` → `Confirm::DropOne`).
3. **Confirmations can be switched off** for expert users; the outcome message then carries the
   consequence.

## 7. Desktop integration (increment 2): hide, quit and drop

| Action | Confirmation? | Feedback | Recovery |
| --- | --- | --- | --- |
| Window ✕ / Alt-F4 with *Keep running in the tray* on and a tray host present | **no dialog** — nothing is destroyed; the transfer keeps running | window disappears from the taskbar; App log: `window hidden — rdm keeps running in the tray (Quit there to exit)`; the tray icon remains | *Show rdm*, a click on the icon, or the tray's *New download* brings the window back (focused, with a taskbar flash) |
| Window ✕ with **no** tray host (or the setting off) | as before: the app quits | App log on start-up: `no system tray available — closing the window will quit` | start rdm again — nothing is lost: downloads resume from the metadata database |
| Tray → *Quit rdm* with transfers running | no dialog — but it is the *explicit* exit, and it is the only place that exits while hidden | App log: `paused N running download(s) on exit` (5 s graceful shutdown) | restart rdm; paused downloads continue with ▶ Resume |
| Tray → *New download (from clipboard)* with an empty / non-link clipboard | no dialog | form opens empty; App log: `no link on the clipboard — opening the form empty` | paste the link by hand — the dialog behaves exactly like the toolbar entry |
| Drop a link on the target / the window | no dialog (a drop is an intent, not a destructive act) | target shows *Link accepted — the New download form opened with <link>*; window opens with the URL filled and selected; App log names the link | `Esc` closes the form; nothing was queued |
| Drop something that is **not** a link | — | a sentence where the drop happened **and** in the status bar (“that drop was not a link…”) | nothing to recover — nothing happened, and the user was told so (never a silent no-op) |
| ✕ on the floating target | none needed: it hides a helper window | target disappears; App log: `floating drop target hidden`; the Settings switch follows | Settings → *Desktop integration* → *Floating drop target* (or the tray menu item) |

**Policy decisions recorded here.** (a) Closing never *asks* because it is not destructive when the
tray is there — and when the tray is not there it is a normal exit, which the user asked for.
(b) *Quit rdm* is deliberately the only explicit exit visible while the window is hidden, so
“close” and “quit” can never be confused. (c) A drop always produces a sentence: silence would
make a failed drag look like a broken app, which is the same defect class as FIND-UX-003.
