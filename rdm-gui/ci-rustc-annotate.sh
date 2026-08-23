#!/usr/bin/env bash
# Temporary CI helper.
#
# The sandbox this crate is developed in can reach api.github.com but not the
# blob storage that serves Actions logs, so a failing rustc invocation re-emits
# its diagnostics as a GitHub Actions annotation, which *is* readable through
# the API. Behaves like a plain `rustc` everywhere else.
err="$(mktemp)"
"$@" 2>"$err"
status=$?
if [ "$status" -ne 0 ] && [ -n "$GITHUB_ACTIONS" ]; then
  {
    printf '::error title=rustc::'
    tail -200 "$err" | sed 's/%/%25/g' | awk '{printf "%s%%0A", $0}'
    printf '\n'
  } >&2
fi
cat "$err" >&2
rm -f "$err"
exit "$status"
