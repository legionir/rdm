#!/usr/bin/env python3
"""Build unpacked Chrome/Firefox extensions and zip packages (stdlib only)."""

from __future__ import annotations

import base64
import hashlib
import json
from pathlib import Path
import shutil
import zipfile

ROOT = Path(__file__).resolve().parent
DIST = ROOT / "dist"
EXPECTED_CHROME_ID = "addehhgmjlamplfjohdpjmpfjpdnjonb"


def chrome_id(public_key: str) -> str:
    digest = hashlib.sha256(base64.b64decode(public_key)).digest()[:16]
    return "".join(chr(ord("a") + (byte >> 4)) + chr(ord("a") + (byte & 15)) for byte in digest)


def build(target: str) -> None:
    manifest = json.loads((ROOT / target / "manifest.json").read_text(encoding="utf-8"))
    if target == "chrome":
        actual = chrome_id(manifest["key"])
        if actual != EXPECTED_CHROME_ID:
            raise RuntimeError(f"Chrome key derives {actual}, expected {EXPECTED_CHROME_ID}")
    elif manifest["browser_specific_settings"]["gecko"]["id"] != "rdm@legionir":
        raise RuntimeError("Firefox add-on ID changed without updating the native-host installer")

    destination = DIST / target
    destination.mkdir(parents=True)
    shutil.copy2(ROOT / target / "manifest.json", destination / "manifest.json")
    for filename in ("background.js", "content.js"):
        shutil.copy2(ROOT / "common" / filename, destination / filename)
    shutil.copy2(ROOT.parent / "rdm-gui" / "assets" / "icon.png", destination / "icon.png")

    archive = DIST / f"rdm-{target}.zip"
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as output:
        for path in sorted(destination.rglob("*")):
            if path.is_file():
                output.write(path, path.relative_to(destination))
    print(f"built {destination} and {archive}")


def main() -> None:
    if DIST.exists():
        shutil.rmtree(DIST)
    DIST.mkdir()
    build("chrome")
    build("firefox")


if __name__ == "__main__":
    main()
