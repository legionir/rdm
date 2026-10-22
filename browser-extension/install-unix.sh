#!/usr/bin/env bash
# Per-user native-host registration for Chrome and Firefox on Linux/macOS.
set -euo pipefail

host_name="com.legionir.rdm"
chrome_id="addehhgmjlamplfjohdpjmpfjpdnjonb"
firefox_id="rdm@legionir"
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
host_path=${1:-"$script_dir/rdm-native-host"}

if [[ ! -f "$host_path" ]]; then
  printf 'Cannot find the native host at %s\nPass its full path as the first argument.\n' "$host_path" >&2
  exit 1
fi
chmod +x "$host_path"
host_dir=$(CDPATH= cd -- "$(dirname -- "$host_path")" && pwd)
host_path="$host_dir/$(basename -- "$host_path")"
json_host=$(printf '%s' "$host_path" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g')

case "$(uname -s)" in
  Linux)
    chrome_dir="${XDG_CONFIG_HOME:-$HOME/.config}/google-chrome/NativeMessagingHosts"
    firefox_dir="$HOME/.mozilla/native-messaging-hosts"
    ;;
  Darwin)
    chrome_dir="$HOME/Library/Application Support/Google/Chrome/NativeMessagingHosts"
    firefox_dir="$HOME/Library/Application Support/Mozilla/NativeMessagingHosts"
    ;;
  *)
    printf 'This installer supports Linux and macOS; use install-windows.ps1 on Windows.\n' >&2
    exit 1
    ;;
esac
mkdir -p "$chrome_dir" "$firefox_dir"

cat >"$chrome_dir/$host_name.json" <<EOF
{
  "name": "$host_name",
  "description": "RDM browser integration",
  "path": "$json_host",
  "type": "stdio",
  "allowed_origins": ["chrome-extension://$chrome_id/"]
}
EOF
cat >"$firefox_dir/$host_name.json" <<EOF
{
  "name": "$host_name",
  "description": "RDM browser integration",
  "path": "$json_host",
  "type": "stdio",
  "allowed_extensions": ["$firefox_id"]
}
EOF

printf 'RDM native messaging is registered for Chrome and Firefox.\n'
printf 'Keep rdm-native-host and rdm-gui together so the host can open RDM.\n'
