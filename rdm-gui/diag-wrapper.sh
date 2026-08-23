#!/usr/bin/env bash
# Temporary rustc wrapper (see .cargo/config.toml).
# Forwards to the real rustc; when a crate fails to compile, re-emits the
# rustc diagnostics as GitHub Actions `::error::` annotations.
#
# The runner only parses workflow commands from the STEP's stdout. Cargo
# replaces this wrapper's stdout with a capture pipe, but cargo's own stdout
# is the step stdout — so annotations are written to /proc/$PPID/fd/1
# (cargo's stdout) to bypass the capture. Fallback: stderr.
set -u

RAW="/tmp/rdm-diag.$$.raw"
EMIT="/tmp/rdm-diag.emitted"

emit() {
  # $1 = full annotation line (no newline)
  local line="$1"
  if [[ -w "/proc/$PPID/fd/1" ]]; then
    printf '%s\n' "$line" > "/proc/$PPID/fd/1" 2>/dev/null || printf '%s\n' "$line" >&2
  else
    printf '%s\n' "$line" >&2
  fi
}

# Locate the real rustc (this wrapper is invoked in its place).
real="$(command -v rustc || true)"

if [[ -z "$real" ]]; then
  emit "::error title=rdm-diag::wrapper could not locate rustc in PATH"
  exit 127
fi

# Run the real rustc, teeing stderr so cargo still sees live diagnostics.
"$real" "$@" 2> >(tee "$RAW" >&2)
rc=$?

# On failure, convert rustc errors into annotations (bounded, once).
if [[ $rc -ne 0 && ! -f "$EMIT" ]]; then
  touch "$EMIT"
  n=0
  err=""
  while IFS= read -r line; do
    if [[ "$line" =~ ^error(\[[A-Za-z0-9_]+\])?:[[:space:]]*(.*)$ ]]; then
      err="${BASH_REMATCH[1]} ${BASH_REMATCH[2]}"
    elif [[ -n "$err" && "$line" =~ ^[[:space:]]*--\>[[:space:]]*([^:]+):([0-9]+) ]]; then
      f="${BASH_REMATCH[1]}"
      ln="${BASH_REMATCH[2]}"
      msg="$(printf '%s' "$err" | tr '\n' ' ' | cut -c1-400)"
      ttl="$(printf 'rdm-diag %s' "$err" | tr -d ',:"' | cut -c1-90)"
      emit "::error file=$f,line=$ln,title=$ttl::$msg"
      n=$((n+1))
      err=""
      if [[ $n -ge 8 ]]; then break; fi
    fi
  done < "$RAW"
  if [[ $n -eq 0 ]]; then
    # Failing crate produced no parseable error block; dump first lines raw.
    head -c 600 "$RAW" | while IFS= read -r l; do
      emit "::error title=rdm-diag-raw::${l}"
    done
  fi
fi

rm -f "$RAW"
exit "$rc"
