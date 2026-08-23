#!/usr/bin/env bash
# Temporary CI helper.
#
# The sandbox this crate is developed in can reach api.github.com but not the
# blob storage that serves Actions logs, so a failing rustc invocation re-emits
# a compact form of its diagnostics as a GitHub Actions annotation, which *is*
# readable through the API. Behaves like a plain `rustc` otherwise.
err="$(mktemp)"
"$@" 2>"$err"
status=$?
if [ "$status" -ne 0 ] && [ -n "$GITHUB_ACTIONS" ]; then
  python3 - "$err" >&2 <<'PY'
import json, sys

lines = []
for raw in open(sys.argv[1], errors="replace"):
    raw = raw.strip()
    if not raw.startswith("{"):
        continue
    try:
        diag = json.loads(raw)
    except Exception:
        continue
    if diag.get("$message_type") != "diagnostic" or diag.get("level") != "error":
        continue
    span = (diag.get("spans") or [{}])[0]
    lines.append(
        "{}:{}:{} {} {}".format(
            span.get("file_name", "?"),
            span.get("line_start", "?"),
            span.get("column_start", "?"),
            (diag.get("code") or {}).get("code", ""),
            diag.get("message", ""),
        )
    )
    for child in diag.get("children", []):
        if child.get("level") in ("help", "note") and child.get("message"):
            lines.append("    " + child["message"][:160].replace("\n", " "))

text = "%0A".join(line.replace("%", "%25") for line in lines[:40])
print("::error title=rustc::" + (text or "no json diagnostics captured"))
PY
fi
cat "$err" >&2
rm -f "$err"
exit "$status"
