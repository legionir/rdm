# rdm — User Flows, IA & Navigation

- **Artifact type:** UX Designs (flows and information architecture)
- **Owner:** UX Designer (EXE-043) · **Approvers:** Design Manager, Product Manager (PM)
- **Consumers:** UI, Product, Frontend
- **Basis:** verified behaviour of `rdm-gui` and the `rdm` CLI on branch `arena/01a10cef-rdm`
  (every step carries file/line evidence); terminology from `audits/ux-glossary.md`
- **Notation:** `[Step]` screen or affordance · `→` transition · `*` new/changed in this increment
  · `✖` error point · `✓` exit point

## 1. Information architecture

```
rdm (product)
├── CLI: rdm <command>                     ← scriptable, full parity
│   ├── download · pause · resume · cancel · list · info · remove
│   └── global: --data-dir DIR · -v/-vv/-vvv
└── Window (rdm-gui)                        ← same engine, same metadata
    ├── Toolbar
    │   ├── New download *dialog*           (URL, output, connections, retries,
    │   │                                    chunk size, speed, timeout, checksum,
    │   │                                    user agent, resume/force)
    │   ├── Bulk: Pause all · Resume all · Remove completed…* · Refresh
    │   └── Search box · state filter · show completed · legend ⓘ*
    ├── Download list (table)               ← the product's home
    │   └── Row actions: ⏸ ▶ ⏹ ⟲ 📂 🗑       (+ tooltips naming each action)
    ├── Details modal  (double-click / Enter)
    │   └── Tabs: Overview · Chunks · JSON  (+ state meaning*)
    ├── Queue sidebar (☰)                   ← downloads waiting for a slot
    ├── Settings sidebar (⚙)                ← defaults, application, apply data dir
    └── Status bar (footer)
        ├── one-line status + counters (hover = state legend*)
        └── expandable panes: Events · App log

Desktop surface (outside the window — increment 2)
├── Tray icon (notification area)
│   ├── left click → restore the window
│   └── menu: Show rdm · New download (from clipboard) · Pause all · Resume all ·
│             Floating drop target ✓ · Quit rdm
├── Window ✕ → hides in the tray when *Keep running in the tray* is on and a tray
│   host exists (Quit rdm there really exits); without a tray host it quits as before
├── Floating drop target (opt-in)  ← small always-on-top box above the taskbar clock;
│   drop a link from the browser → New download opens pre-filled; ✕ hides it + turns the switch off
└── Drop anywhere on the window   ← same path as the target (text, uri-list, .url/.txt files)
```

**Navigation model.** One window, no page stack: the list is always visible and everything else
opens *around* it (modal, sidebars, footer pane) and closes back to it. `Esc` walks the layers
outward — New download → confirmation → details → footer pane → sidebars (implemented in the UI
increment) — so the user can always retreat one level without aiming at a ✕.

**Content hierarchy.** The list answers “is my download all right?”: state → file → progress →
size → speed/ETA, with identity (id/added) and controls at the edges. Details answer “what exactly
is happening?” (per-chunk progress, events, raw JSON). Settings hold the defaults that the New
download dialog inherits, so the frequent path never needs a form.

## 2. Primary flow — “download a file”

```
[Start]  user has a URL
   ↓
[Toolbar] New download ───────────────► [Dialog: New download]
   ↓                                          ↓ Start
   ↓                                     ✖ empty/invalid URL → inline danger banner,
   ↓                                        nothing created, dialog stays open
   ↓                                          ↓ valid
   ↓                                     ✓ dialog closes
[Download list]  row appears: Queued (waiting for a free slot)
   ↓ slot free
                 Running  ──► progress %, size, speed, ETA live
                   ├─ ✖ link drops  → Interrupted (progress kept) → ▶ Resume → Running
                   ├─ ✖ gives up    → Failed  (progress kept)      → ⟲ Restart → Running
                   └─ ⏸ Pause        → Paused  (progress kept)     → ▶ Resume → Running
   ↓ 100%
                 Merging (writing the last parts into the file)
   ↓
                 Completed ✔  ──► [Row] 📂 Open folder  ✓ done
                                └─ [Details] Events/JSON for the record
```

Error points and their recovery are listed in the feedback/error/undo policy §2–4; every one of
them ends in a state that still holds the progress.

## 3. Key task flows (entry → steps → exit)

| # | Task | Entry | Steps | Exit / success signal | Error points |
| --- | --- | --- | --- | --- | --- |
| T1 | Start a download | toolbar *New download*, `rdm download <URL>` | fill URL (defaults inherited from Settings) → *Start* | row appears as Queued → Running | invalid URL, bad checksum, unknown flag (banner / CLI error) |
| T2 | Check progress | list row / `rdm list` | read state chip, %, size, speed, ETA | values update every `refresh_ms` (default 600 ms) | — |
| T3 | Pause and continue later | row ⏸ / *Pause all* / `rdm pause <ID>` | pause → (app closed, machine restarted) → ▶ Resume | state back to Running at the same byte offset | resume of a `Completed`/`Failed` download is refused by the engine; the row no longer offers it* |
| T4 | Recover a broken transfer | row ▶ (Interrupted) / ⟲ (Failed) | Resume / Restart | Running again | Failed cannot be resumed → ⟲ Restart offered instead* |
| T5 | Inspect one download | double-click row / `Enter` / `rdm info <ID>` | read Overview → Chunks → JSON | modal shows the same numbers as `rdm info --json` | record removed while open → modal closes itself |
| T6 | Stop a download and keep the data | row ⏹ / `rdm cancel <ID>` | Cancel → row shows Cancelled | partial data kept; ▶ Resume or 🗑 Remove | cancel of a completed download reports “already completed” |
| T7 | Clean up the list | *Remove completed…* / row 🗑 / `rdm remove <ID>` | dialog (count + file decision) → Remove | row disappears; status reports the count* | 🗑 while running is disabled with the reason* |
| T8 | Re-download a file | row ⟲ / `rdm download --force` | dialog (progress + overwrite) → Restart | new transfer from byte 0 | output file locked by another process → engine error in status + Events |
| T9 | Limit concurrency | toolbar chip → ☰ Queue; Settings → *Max concurrent downloads* | raise/lower limit → queued items start | queue drains; chip disappears when empty | `0` = unlimited (documented in the label tooltip) |
| T11 | Learn the product in the app | `F1` / toolbar ⓘ / the empty-list hint | read states → keyboard → vocabulary | help closes with `Esc`/`Close` and leaves the list untouched | — (help is read-only) |
| T12 | Start from a link you already have | toolbar *New download* (clipboard pre-fill*) · tray *New download (from clipboard)* · drop the link on the floating target* or on the window* | copy/drag the link → *New download* fills and selects the URL (or the drop opens the form) → *Start* | row appears as Queued → Running; the log names the link a drop accepted | clipboard empty / not a link → form opens empty + log line “no link on the clipboard — opening the form empty”; dropping something that is not a link → a sentence in the target and the status bar (never silent) |
| T10 | Diagnose a failure | status bar → **Events** / **App log** | read the machine-level messages | the exact engine error text | App log empty until something is logged (hint says so) |

## 4. Entry points, exits and error points per screen

| Screen | Entry points | Exits (✓) | Error points (✖) |
| --- | --- | --- | --- |
| Download list | app start (empty state*), toolbar Refresh/F5 | every row action, double-click/Enter → details | “nothing here yet” hint; `cannot read downloads: …` in the status bar |
| New download dialog | toolbar button, `Ctrl+N`-free (mouse/`Tab`), CLI-equivalent flags | *Start* ✓ / *Cancel* ✓ / `Esc` ✓ / window ✕ | inline banner per field validation; disabled nothing — *Start* always attemptable and answers |
| Details modal | row double-click / `Enter` | ✕ ✓ / `Esc` ✓ | removed record → modal self-closes; JSON pane shows `// <error>` instead of failing |
| Queue sidebar | toolbar ☰, queued chip | ✕ per item ✓ / *Drop all* ✓ / ☰ ✓ | “that download already started — it is no longer in the queue”* |
| Settings sidebar | toolbar ⚙ | ☰/⚙ ✓, *Save* ✓ / *Reload* ✓ | `unsaved changes` banner*; save failure reported in the status bar; *Apply data directory* failure reported with the path |
| Footer panes | status bar *Events* / *App log* | ✕ ✓ / `Esc` ✓ / button again ✓ | “select a download to see its events”, “no events recorded”, “nothing logged yet*” |
| Confirm dialog | destructive row/toolbar actions | *Remove/Restart* ✓ / *Keep* ✓ / `Esc` ✓ | — (the dialog is itself the error-prevention step) |
| Tray menu (increment 2) | tray icon right click; icon left click restores the window | *Show rdm* ✓ / *New download* ✓ (opens the form) / *Quit rdm* ✓ (real exit, 5 s graceful pause) | no tray host → the entry does not exist and closing quits (logged); a build failure of the tray keeps the window reachable |
| Floating drop target (increment 2) | Settings → *Desktop integration* → *Floating drop target*; tray menu item | ✕ on the target ✓ (hides it and turns the switch off, nothing destroyed) / switch off ✓ | drop that is not a link → explanation where the drop happened + status bar; setting cannot be saved → error logged, the target still follows the setting in memory |

## 5. Bulk-action flows (safety-critical)

```
Toolbar: [⏸ Pause all]      → asks nothing (reversible) → “Pause requested for N download(s).”
                                                       → “Nothing to pause — no download is running.”*
Toolbar: [▶ Resume all]     → asks nothing (reversible) → “Continuing N download(s).”
                                                       → “Nothing to continue — no download is
                                                          paused, interrupted or cancelled.”*
Toolbar: [🗑 Remove completed…] → 0 completed?        → “Nothing to remove — no completed download
                                                          in the list.”*  (no dialog)
                               → otherwise            → [Confirm dialog: N downloads + file checkbox]
                                                      → “Removed N completed download(s) from the list.”*
Queue:   [Drop all]         → < 5 queued? asks nothing (nothing downloaded yet)
                               → “Dropped N queued download(s).”*
                            → ≥ 5 queued (DROP_ALL_CONFIRM_THRESHOLD)?
                               [Confirm dialog: N queued, “no partial data is lost”]
                               → “Dropped N queued download(s).”*
```

The three bulk actions differ deliberately: *pause/resume* are reversible and act at once;
*remove* destroys data and therefore asks with the count in the dialog; *drop* destroys nothing, so
it acts at once for short queues and asks only when a long queue (≥ 5) would be emptied in one
click — reparsing a long queue by hand is the real cost there. Before this increment *Remove
completed* acted immediately, included the file deletion setting, and reported “removed N
record(s)”.

## 6. Journeys (persona-free, behaviour-based)

1. **The interrupted evening.** A 4 GB ISO starts; the laptop sleeps; the next morning the row
   reads *Interrupted* with the tooltip “the transfer stopped unexpectedly; the progress is kept”.
   One click on ▶ continues; no bytes are re-downloaded. (T3/T4)
2. **The wrong file.** The user spots a wrong row and presses 🗑. The dialog names the download and
   says the partial data goes with it; the finished file stays unless the box is ticked. `Keep`
   and `Esc` are both available. (T7)
3. **The clean-up.** Ten finished downloads clutter the list; the user presses *Remove completed…*,
   sees “This removes 10 completed download(s)…”, confirms, and reads back “Removed 10 completed
   download(s) from the list.” (T7, bulk)
4. **The diagnosis.** A download fails at 87 %; **Events** shows the last engine reason, the
   tooltip on *Failed* says the progress is kept, and ⟲ Restart (or, for a transient link failure,
   waiting for the next retry) resolves it. (T4/T10)
5. **The script user.** The same job is driven from the terminal with `rdm`; the words in
   `--help` and in the window now match, so reading either one teaches the other. (T1–T7)

## 7. Open UX questions (for Product, not silently decided here)

| # | Question | Current behaviour | Decision needed from |
| --- | --- | --- | --- |
| Q1 | Should `Failed` downloads be resumable instead of requiring *Restart*? **Open — escalated (`UX-ESC-004`).** | engine refuses (`resume()` rejects terminal states except `Cancelled`); the UI offers ⟲ and the CLI summary now points at the working command (PH-6.5) | Product + engineering (changes an engine contract — out of UX scope; see `UX-ESC-002`) |
| Q2 | ~~Should *Drop all* ask when the queue is long?~~ **Resolved (PH-6.1):** asks at ≥ 5 queued (`ux::drop_all_confirm`); a user study can revisit the threshold, but the safe default is in place. | dialog above the threshold, immediate below it | *closed* |
| Q3 | Is a global “undo” for file deletion worth keeping detached files? | no undo; confirmation + explicit checkbox instead | Product (cost/benefit; `RISK-UX-002`) |
| Q4 | Should the window offer a “retry automatically when the network is back” policy? | manual Resume/Restart only; the app now also *tells* the user which failed downloads need ⟲ Restart when they use *Resume all* (PH-6.3) | Product (roadmap) |
| Q5 | Can the engine accept `resume` on a `Failed` download (making the partial data recoverable instead of requiring a restart)? | engine refuses it; UI offers ⟲ Restart | Product + engineering (`UX-ESC-002`/`UX-ESC-004`, Q1) |
