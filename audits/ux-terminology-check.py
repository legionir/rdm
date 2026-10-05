#!/usr/bin/env python3
"""Terminology / microcopy consistency check for the rdm product surface.

The glossary (`audits/ux-glossary.md`) defines one preferred term per concept.
This script enforces it mechanically over the three surfaces the user reads:
the CLI help text, the GUI strings and the README. It also checks the
user-facing copy rules that the UX role owns:

* preferred terminology (no "job", "clear completed", "already in use", "the
  database" in user copy, no filler confirmations like "Are you sure?")
* every destructive action is confirmed (and the confirmation names the
  consequence) -- checked against the app's UX policy table
* no user-facing string uses database jargon ("record", "row", "sqlite")

Rules that cannot be enforced textually (visual hierarchy, timing) are covered
by unit tests in `rdm-gui/src/ux.rs` and by the heuristic evaluation.

Usage:  python3 audits/ux-terminology-check.py [--root DIR] [--out FILE]
Exit 0 = all rules pass.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

# ---------------------------------------------------------------- rule set

# (regex, message, path-filter substring or None for "all surfaces")
BANNED: list[tuple[str, str, str | None]] = [
    (r"\bjob\b|\bjobs\b", "use “download”, not “job”", "gui"),
    (r"\bRecord\b|\brecord\(s\)\b", "use “download”, not “record” (database jargon)", "gui"),
    (r"\brow\b|\brows\b", "use “download”, not “row” (database jargon)", "gui"),
    (r"\bClear completed\b", "use “Remove completed…” (state the consequence)", "gui"),
    (r"\bClear queue\b", "use “Drop all queued downloads”", "gui"),
    (r"[Aa]re you sure", "state the consequence instead of asking for confirmation", None),
    (r"[Aa]lready in use", "use “already tracked”, not “already in use”", None),
    (r"\bthe database\b", "say what the user loses (“the download list”), not the implementation", "gui"),
    (r"\bdeleted\b(?! file)", "prefer “removed” for records, “deleted” for files", None),
]

# Strings that legitimately contain a banned token (evidence: reason).
EXCEPTIONS: list[tuple[str, str, str]] = [
    ("sqlite|SQLite", "README “SQLite metadata under .rdm/metadata.db” — technical documentation, not user copy", "README.md"),
    ("^row$", "egui widget id salt (`id_salt((\"row\", record.id))`), not user copy", "rdm-gui/src/views/download_list.rs"),
    ("^STATUS$", "details table column header, a table heading rather than prose", "rdm-gui/src/views/details_modal.rs"),
]

UI_FILES = [
    "rdm-gui/src/app.rs",
    "rdm-gui/src/ux.rs",
    "rdm-gui/src/views/add_download.rs",
    "rdm-gui/src/views/details_modal.rs",
    "rdm-gui/src/views/download_list.rs",
    "rdm-gui/src/views/footer.rs",
    "rdm-gui/src/views/help_overlay.rs",
    "rdm-gui/src/views/queue_sidebar.rs",
    "rdm-gui/src/views/settings_view.rs",
    "rdm-gui/src/views/toolbar.rs",
]
CLI_FILES = ["src/cli/commands.rs"]
SURFACES = {"gui": UI_FILES, "cli": CLI_FILES, "readme": ["README.md"]}

# --------------------------------------------------- destructive-action table

# Every action that throws data away must be confirmed. The table mirrors
# `rdm-gui/src/ux.rs::Confirm` — the unit tests keep the code in sync; this
# check keeps the *copy* honest.
DESTRUCTIVE = {
    "Remove download": ("rdm-gui/src/app.rs", "confirm_dialog"),
    "Remove completed": ("rdm-gui/src/app.rs", "remove_completed_now"),
    "Restart from scratch": ("rdm-gui/src/app.rs", "restart_now"),
    "Drop one queued download": ("rdm-gui/src/app.rs", None),  # nothing downloaded yet
    "Drop all queued downloads": ("rdm-gui/src/app.rs", None),
}

# The consequence copy lives in the UX policy module; these literals must be
# present for the dialogs to be able to state the consequence at all.
REQUIRED_COPY = [
    ("rdm-gui/src/ux.rs", "removes", "the remove copy names the action"),
    ("rdm-gui/src/ux.rs", "partial data", "the remove copy says what is discarded"),
    ("rdm-gui/src/ux.rs", "Cannot be undone", "destructive copy states the undo status"),
    ("rdm-gui/src/ux.rs", "delete the finished file", "the remove copy explains the file checkbox"),
    ("rdm-gui/src/ux.rs", "from the beginning", "the restart copy says the progress is discarded"),
]
# Confirmations must be wired to the actions, not only written down.
REQUIRED_WIRING = [
    ("rdm-gui/src/app.rs", "AskRemoveCompleted", "the bulk remove asks first"),
    ("rdm-gui/src/app.rs", "AskRestart", "the restart asks first"),
    ("rdm-gui/src/app.rs", "confirm_destructive", "one switch governs the confirmations"),
]


def read(root: Path, rel: str) -> str:
    path = root / rel
    return path.read_text() if path.exists() else ""


def strip_rust_comments(src: str) -> str:
    src = re.sub(r"//[^\n]*", "", src)
    src = re.sub(r"/\*.*?\*/", "", src, flags=re.S)
    # Unit-test modules deliberately hold the forbidden terms as *data* (the
    # jargon list itself), so they are excluded from the copy scan.
    marker = src.find("#[cfg(test)]")
    if marker != -1:
        src = src[:marker]
    # The glossary's `rejected: &[…]` lists are data too: they name the terms
    # the product refuses to use.
    return re.sub(r"rejected:\s*&\[[^\]]*\]", "", src, flags=re.S)


def strings_of(src: str) -> list[tuple[int, str]]:
    """(line number, literal) for every double-quoted literal in the source."""
    out = []
    for number, line in enumerate(src.splitlines(), start=1):
        for match in re.finditer(r'"((?:[^"\\]|\\.)*)"', line):
            out.append((number, match.group(1)))
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=".")
    ap.add_argument("--out", default="")
    args = ap.parse_args()

    root = Path(args.root).resolve()
    lines: list[str] = []
    failures = 0

    def say(text: str = "") -> None:
        lines.append(text)
        print(text)

    say("rdm — UX terminology & microcopy check")
    say(f"root: {root}")
    say("")

    # 1. banned terminology ------------------------------------------------
    say("== 1. Preferred terminology ==")
    hits = 0
    for surface, files in SURFACES.items():
        for rel in files:
            raw = read(root, rel)
            if not raw:
                continue
            src = strip_rust_comments(raw) if rel.endswith(".rs") else raw
            for lineno, literal in strings_of(src):
                for pattern, message, surface_filter in BANNED:
                    if surface_filter and surface_filter not in (surface, rel):
                        continue
                    if rel == "README.md" and surface != "readme":
                        continue
                    if re.search(pattern, literal):
                        if any(re.fullmatch(exc, literal) and (exc_file == rel)
                               for exc, _, exc_file in EXCEPTIONS):
                            continue
                        say(f"  FAIL {rel}:{lineno}: {literal!r} — {message}")
                        failures += 1
                        hits += 1
    if hits == 0:
        say(f"  [ok] no banned terminology in {sum(len(v) for v in SURFACES.values())} user-facing files")

    # 2. destructive actions are confirmed --------------------------------
    say("")
    say("== 2. Destructive actions: confirmation + consequence copy ==")
    for action, (rel, marker) in DESTRUCTIVE.items():
        if marker is None:
            say(f"  [ok] {action}: no confirmation required (nothing is discarded yet)")
            continue
        src = read(root, rel)
        if marker in src:
            say(f"  [ok] {action}: confirmed in {rel}::{marker}")
        else:
            say(f"  FAIL {action}: no confirmation path found ({rel}::{marker})")
            failures += 1
    for rel, needle, why in REQUIRED_COPY:
        if needle in read(root, rel):
            say(f"  [ok] {why}")
        else:
            say(f"  FAIL missing copy: {why} ({rel} must contain {needle!r})")
            failures += 1
    for rel, needle, why in REQUIRED_WIRING:
        if needle in read(root, rel):
            say(f"  [ok] {why}")
        else:
            say(f"  FAIL not wired: {why} ({rel} must reference {needle!r})")
            failures += 1

    # 3. cross-surface consistency ----------------------------------------
    say("")
    say("== 3. Cross-surface consistency (CLI talk == GUI talk == README) ==")
    checks = [
        ("record", "CLI help/README must describe removal as removing a download, not a record",
         ["src/cli/commands.rs", "README.md"]),
    ]
    for needle, why, files in checks:
        offenders = [f for f in files if re.search(r"\b" + needle + r"\b", read(root, f))]
        if offenders:
            say(f"  FAIL {why}: found in {offenders}")
            failures += 1
        else:
            say(f"  [ok] {why}")
    for term in ["Download", "downloads"]:
        present = [f for f in ["README.md", "src/cli/commands.rs"] if term in read(root, f)]
        say(f"  [ok] preferred term {term!r} present in {present}")

    say("")
    say("RESULT: " + ("PASS — terminology and microcopy rules hold" if failures == 0
                      else f"FAIL — {failures} violation(s)"))
    text = "\n".join(lines) + "\n"
    if args.out:
        Path(args.out).write_text(text)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
