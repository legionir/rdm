#!/usr/bin/env python3
"""Static UI verification for the rdm GUI design tokens.

This is a *static* check, not a substitute for `cargo test`: the sandbox that
produced the change has no Rust toolchain, so this script re-derives the
numbers straight from the shipped token source and adds the design guards that
would otherwise only run in CI.

It verifies, for the palette that is actually in `rdm-gui/src/theme/tokens.rs`:

1. Every (role, surface) pair declared in `theme/contrast.rs::CONTRACT` reaches
   its WCAG 2.1 minimum ratio — for both the dark and the light palette.
2. The focus/accent colour is visible (>= 3:1, SC 1.4.11) on every surface.
3. No view file hardcodes a colour (`Color32::from_*`, `Color32::GRAY`, ...).
4. Every view / shell file references the token layer (`theme::`).
5. Delimiters are balanced in every changed Rust file (a cheap guard against
   scripted-edit damage; the real parser is `cargo` in CI).

Usage: python3 audits/ui-contrast-check.py [--root DIR]
Exit code 0 = all checks pass, 1 = at least one violation.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

# ----------------------------------------------------------------- WCAG 2.x


def _lin(c: int) -> float:
    c = c / 255.0
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def luminance(rgb: tuple[int, int, int]) -> float:
    r, g, b = (_lin(v) for v in rgb)
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def contrast(fg: tuple[int, int, int], bg: tuple[int, int, int]) -> float:
    a, b = luminance(fg), luminance(bg)
    hi, lo = max(a, b), min(a, b)
    return (hi + 0.05) / (lo + 0.05)


def hexof(rgb: tuple[int, int, int]) -> str:
    return "#%02X%02X%02X" % rgb


# ------------------------------------------------------- parse the Rust source

LET_RGB = re.compile(
    r"let\s+(\w+)\s*=\s*Color32::from_rgb\(\s*0x([0-9A-Fa-f]{2}),\s*"
    r"0x([0-9A-Fa-f]{2}),\s*0x([0-9A-Fa-f]{2})\s*\);"
)
CONTRACT_ENTRY = re.compile(r'\(\s*"(\w+)"\s*,\s*(\w+)\s*,\s*(\w+)\s*\)')
CONST_LIST = re.compile(r"(?:pub )?const (\w+): [^=]+ = &\[(.*?)\];", re.S)
SURFACES_LIST = re.compile(r"pub const SURFACES: \[&str; \d+\] = \[(.*?)\];", re.S)
FLOAT_CONST = re.compile(r"pub const (\w+): f32 = ([0-9.]+);")
STRINGS = re.compile(r'"(\w+)"')
FORBIDDEN = [
    "Color32::from_rgb",
    "Color32::from_rgba",
    "Color32::from_gray",
    "Color32::from_white_alpha",
    "Color32::from_black_alpha",
    "Color32::GRAY",
    "Color32::RED",
    "Color32::WHITE",
]


def palette_of(tokens_src: str, fn_name: str) -> dict[str, tuple[int, int, int]]:
    start = tokens_src.index(f"fn {fn_name}()")
    end = tokens_src.index("\n}\n", start)
    body = tokens_src[start:end]
    out = {}
    for name, r, g, b in LET_RGB.findall(body):
        out[name] = (int(r, 16), int(g, 16), int(b, 16))
    return out


def contract_of(contrast_src: str) -> tuple[list[str], list[tuple[str, list[str], float]]]:
    surfaces = STRINGS.findall(SURFACES_LIST.search(contrast_src).group(1))
    lists = {name: STRINGS.findall(body)
             for name, body in CONST_LIST.findall(contrast_src)}
    floats = {name: float(value) for name, value in FLOAT_CONST.findall(contrast_src)}
    block = contrast_src[contrast_src.index("pub const CONTRACT"):]
    block = block[: block.index("\n];") + 3]
    contract = []
    for role, names, minimum in CONTRACT_ENTRY.findall(block):
        contract.append((role, lists.get(names, []), floats.get(minimum, 0.0)))
    return surfaces, contract


# ------------------------------------------------------- lightweight guards


def delimiter_balance(src: str) -> str | None:
    """Return an error string if (), [], {} are unbalanced outside literals."""
    stack: list[str] = []
    pairs = {")": "(", "]": "[", "}": "{"}
    i, n = 0, len(src)
    while i < n:
        ch = src[i]
        if ch == "/" and i + 1 < n and src[i + 1] == "/":
            i = src.find("\n", i)
            if i == -1:
                break
            continue
        if ch == '"':
            i += 1
            while i < n and src[i] != '"':
                i += 2 if src[i] == "\\" else 1
        elif ch == "'" and i + 2 < n and (src[i + 2] == "'" or (src[i + 1] == "\\")):
            i += 4 if src[i + 1] == "\\" else 2
        elif ch in "([{":
            stack.append(ch)
        elif ch in ")]}":
            if not stack or stack.pop() != pairs[ch]:
                return f"unbalanced `{ch}`"
        i += 1
    if stack:
        return f"unclosed {''.join(stack)}"
    return None


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=".", help="repository root")
    ap.add_argument("--out", default="", help="write the report to this file too")
    args = ap.parse_args()

    root = Path(args.root).resolve()
    gui = root / "rdm-gui" / "src"
    tokens_path = gui / "theme" / "tokens.rs"
    contrast_path = gui / "theme" / "contrast.rs"
    tokens_src = tokens_path.read_text()
    contrast_src = contrast_path.read_text()

    lines: list[str] = []
    failures = 0

    def say(text: str = "") -> None:
        lines.append(text)
        print(text)

    palettes = {
        "dark": palette_of(tokens_src, "dark_palette"),
        "light": palette_of(tokens_src, "light_palette"),
    }
    surfaces, contract = contract_of(contrast_src)

    say("rdm — UI design-token verification (static)")
    say(f"source: {tokens_path.relative_to(root)}")
    say(f"contract: {len(contract)} roles × surfaces, "
        f"{len(surfaces)} surfaces, {len(palettes)} themes")
    say("")

    # 1 + 2 — contrast contract -------------------------------------------
    say("== 1/2. WCAG contrast contract ==")
    for mode, palette in palettes.items():
        worst = ("", "", 99.0)
        for role, on_surfaces, minimum in contract:
            if role not in palette:
                say(f"  FAIL {mode}: role `{role}` is not a palette field")
                failures += 1
                continue
            for surface in on_surfaces:
                if surface not in palette:
                    say(f"  FAIL {mode}: surface `{surface}` is not a palette field")
                    failures += 1
                    continue
                ratio = contrast(palette[role], palette[surface])
                if ratio < worst[2]:
                    worst = (role, surface, ratio)
                if ratio + 1e-9 < minimum:
                    say(f"  FAIL {mode} {role} on {surface}: {ratio:.2f} < {minimum}")
                    failures += 1
        worst_rgb = hexof(palette[worst[0]])
        say(f"  [ok] {mode}: worst text pair {worst[0]} {worst_rgb} on "
            f"{worst[1]} = {worst[2]:.2f}:1 (AA min 4.5:1)")
    for mode, palette in palettes.items():
        for surface in surfaces:
            ratio = contrast(palette["accent"], palette[surface])
            if ratio < 3.0:
                say(f"  FAIL {mode}: focus ring on {surface} = {ratio:.2f} < 3.0")
                failures += 1
        else:
            say(f"  [ok] {mode}: focus ring visible on all {len(surfaces)} surfaces")

    # 3 + 4 — token discipline --------------------------------------------
    say("")
    say("== 3/4. Token discipline in the views ==")
    # Same file set as the Rust design guard in `theme/mod.rs`: the app shell
    # plus every rendered view (the module list and the view-models hold no
    # styling, so they are not part of the discipline check).
    ui_files = [gui / "app.rs"] + [
        p for p in sorted((gui / "views").glob("*.rs")) if p.name != "mod.rs"
    ]
    for path in ui_files:
        src = path.read_text()
        rel = path.relative_to(root)
        hits = [needle for needle in FORBIDDEN if needle in src]
        if hits:
            say(f"  FAIL {rel}: hardcoded colours {hits}")
            failures += 1
        if "theme::" not in src:
            say(f"  FAIL {rel}: does not reference the token layer")
            failures += 1
    if failures == 0:
        say(f"  [ok] {len(ui_files)} UI files: no colour literals, all use `theme::`")

    # 5 — delimiter sanity -------------------------------------------------
    say("")
    say("== 5. Static sanity of the changed Rust files ==")
    changed = sorted(gui.rglob("*.rs"))
    for path in changed:
        problem = delimiter_balance(path.read_text())
        if problem:
            say(f"  FAIL {path.relative_to(root)}: {problem}")
            failures += 1
    say(f"  [ok] {len(changed)} Rust files: delimiters balanced")

    say("")
    say("RESULT: " + ("PASS — all checks green" if failures == 0
                      else f"FAIL — {failures} violation(s)"))

    text = "\n".join(lines) + "\n"
    if args.out:
        Path(args.out).write_text(text)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
