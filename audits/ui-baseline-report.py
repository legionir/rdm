#!/usr/bin/env python3
"""Baseline (pre-change) audit of the rdm GUI, regenerated from commit f528faf.

Produces the two evidence files referenced by the design specification:

* ``audits/evidence/baseline-hardcoded-values.txt`` — how many design values
  were inlined in the UI files before the token layer existed.
* ``audits/evidence/baseline-contrast-report.txt`` — the WCAG ratios of the
  previously hardcoded state palette.

Run: python3 audits/ui-baseline-report.py
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

BASE_COMMIT = "f528faf"
DEFAULT_ROOT = Path(__file__).resolve().parent.parent
UI_FILES = [
    "rdm-gui/src/app.rs",
    "rdm-gui/src/state.rs",
    "rdm-gui/src/main.rs",
    "rdm-gui/src/views/add_download.rs",
    "rdm-gui/src/views/details_modal.rs",
    "rdm-gui/src/views/download_list.rs",
    "rdm-gui/src/views/footer.rs",
    "rdm-gui/src/views/queue_sidebar.rs",
    "rdm-gui/src/views/settings_view.rs",
    "rdm-gui/src/views/toolbar.rs",
]

COLOUR = re.compile(r"Color32::(?:from_rgb|from_rgba|from_gray|from_white_alpha|from_black_alpha|GRAY|RED|WHITE)")
DESIGN_NUMBER = re.compile(
    r"(?:add_space|desired_width|default_width|min_width|max_width|default_height|min_height"
    r"|min_col_width|\.size|\.width|Margin::symmetric)\(\s*[0-9]"
    r"|clamp\(\s*[0-9]|Vec2::new\(\s*[0-9]|vec2\(\s*[0-9]"
)

# The previously hardcoded palette (file/line evidence is in the report).
OLD_PALETTE = {
    "success (Completed)": (22, 163, 74),
    "info (Running)": (37, 99, 235),
    "merging (Merging)": (139, 92, 246),
    "warning (Queued)": (217, 119, 6),
    "interrupted (Interrupted)": (234, 88, 12),
    "danger (Failed / errors)": (220, 38, 38),
}

# (a) egui 0.29 default surfaces the code assumed (NOT verifiable offline).
EGUI_DEFAULT_SURFACES = {
    "panel #1B1B1B": (27, 27, 27),
    "hover #464646": (70, 70, 70),
    "selected #005C80": (0, 92, 128),
    "window #FFFFFF": (255, 255, 255),
    "selected(light) #90D1FF": (144, 209, 255),
}
# (b) the surfaces this change defines as tokens (verifiable).
TOKEN_SURFACES = {
    "dark surface #1B1B1B": (0x1B, 0x1B, 0x1B),
    "dark hover #3A3A3A": (0x3A, 0x3A, 0x3A),
    "dark selected #1E3A5F": (0x1E, 0x3A, 0x5F),
    "light surface #F8F8F8": (0xF8, 0xF8, 0xF8),
    "light hover #E6E6E6": (0xE6, 0xE6, 0xE6),
    "light selected #D8EBFF": (0xD8, 0xEB, 0xFF),
}


def lin(c: int) -> float:
    c = c / 255.0
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def luminance(rgb: tuple[int, int, int]) -> float:
    r, g, b = (lin(v) for v in rgb)
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def contrast(a: tuple[int, int, int], b: tuple[int, int, int]) -> float:
    la, lb = luminance(a), luminance(b)
    hi, lo = max(la, lb), min(la, lb)
    return (hi + 0.05) / (lo + 0.05)


def hexof(rgb: tuple[int, int, int]) -> str:
    return "#%02X%02X%02X" % rgb


def git_show(root: Path, path: str) -> str:
    return subprocess.run(
        ["git", "show", f"{BASE_COMMIT}:{path}"],
        cwd=root, check=True, capture_output=True, text=True,
    ).stdout


def main() -> int:
    root = DEFAULT_ROOT
    out_dir = root / "audits" / "evidence"
    out_dir.mkdir(parents=True, exist_ok=True)

    # ---- 1. hardcoded values -------------------------------------------
    lines = [
        f"Baseline scan of the rdm GUI at commit {BASE_COMMIT} (before the design tokens).",
        "Reproduce: python3 audits/ui-baseline-report.py",
        "",
        f"{'file':44} {'colour literals':>15} {'design-value literals':>22}",
    ]
    total_colour = total_number = 0
    per_file_colour: dict[str, int] = {}
    for path in UI_FILES:
        try:
            src = git_show(root, path)
        except subprocess.CalledProcessError:
            continue
        colour = COLOUR.findall(src)
        numbers = DESIGN_NUMBER.findall(src)
        per_file_colour[path] = len(colour)
        total_colour += len(colour)
        total_number += len(numbers)
        lines.append(f"{path:44} {len(colour):>15} {len(numbers):>22}")
    lines += [
        "",
        f"TOTAL: {total_colour} hardcoded colour constructors, "
        f"{total_number} inlined design numbers across {len(per_file_colour)} UI files.",
        "",
        "Colour literals by file (the values the token layer replaced):",
    ]
    for path, count in sorted(per_file_colour.items()):
        if count:
            lines.append(f"  {path}: {count}")
    # exact file:line list for the colour constructors
    lines += ["", "file:line evidence:"]
    for path in UI_FILES:
        try:
            src = git_show(root, path)
        except subprocess.CalledProcessError:
            continue
        for number, text in enumerate(src.splitlines(), start=1):
            if COLOUR.search(text):
                lines.append(f"  {path}:{number}: {text.strip()}")
    (out_dir / "baseline-hardcoded-values.txt").write_text("\n".join(lines) + "\n")
    print(f"wrote {out_dir / 'baseline-hardcoded-values.txt'}")

    # ---- 2. baseline contrast -------------------------------------------
    rep = [
        f"Baseline WCAG contrast of the hardcoded GUI palette at commit {BASE_COMMIT}.",
        "Threshold: WCAG 2.1 SC 1.4.3 AA for normal text = 4.5:1.",
        "Reproduce: python3 audits/ui-baseline-report.py",
        "",
        "Surface assumption note: before this change the row/panel fills came from",
        "egui's defaults, which are not inspectable from this repository (egui 0.29.1",
        "is not vendored and there is no network/toolchain here). The report therefore",
        "shows the old palette against (a) the egui default values that were assumed and",
        "(b) the surfaces this change defines as tokens (deterministic, verifiable).",
        "",
    ]
    for label, surfaces in (("(a) assumed egui defaults", EGUI_DEFAULT_SURFACES),
                            ("(b) surfaces defined by the new tokens", TOKEN_SURFACES)):
        rep.append(f"== old palette vs {label} ==")
        rep.append(f"  {'role (old value)':28} " + " ".join(f"{s.split()[0]:>10}" for s in surfaces))
        worst = (99.0, "", "")
        for role, rgb in OLD_PALETTE.items():
            row = []
            for surf, bg in surfaces.items():
                ratio = contrast(rgb, bg)
                row.append(f"{ratio:>10.2f}")
                if ratio < worst[0]:
                    worst = (ratio, role, surf)
            rep.append(f"  {role + ' ' + hexof(rgb):28} " + " ".join(row))
        failed = sum(
            1 for role, rgb in OLD_PALETTE.items() for bg in surfaces.values()
            if contrast(rgb, bg) < 4.5
        )
        total = len(OLD_PALETTE) * len(surfaces)
        rep += [
            "",
            f"  failures: {failed}/{total} pairs below 4.5:1; "
            f"worst {worst[0]:.2f}:1 ({worst[1]} on {worst[2]})",
            "",
        ]
    (out_dir / "baseline-contrast-report.txt").write_text("\n".join(rep) + "\n")
    print(f"wrote {out_dir / 'baseline-contrast-report.txt'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
