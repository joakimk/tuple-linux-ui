#!/usr/bin/env bash
# Launch a debug build, wait for its window on Hyprland, screenshot it with grim, quit it.
# Usage: scripts/dev-screenshot.sh [output.png] [seconds-to-wait-before-shot]
set -euo pipefail
cd "$(dirname "$0")/.."

out="${1:-/tmp/tuple-ui.png}"
settle="${2:-3}"
target="$(cargo metadata --format-version 1 2>/dev/null | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
nice -n 19 cargo build -q -j 2
log="$(mktemp -t tuple-ui-log.XXXXXX)"

"$target/debug/tuple-ui" >"$log" 2>&1 &
pid=$!
trap 'kill "$pid" 2>/dev/null || true' EXIT

geometry=""
for _ in $(seq 1 40); do
  geometry="$(hyprctl clients -j | python3 -c '
import json, sys
for c in json.load(sys.stdin):
    if c.get("class") == "dev.joakimk.TupleUi" and c.get("mapped"):
        x, y = c["at"]; w, h = c["size"]
        print(f"{x},{y} {w}x{h}"); break
' 2>/dev/null || true)"
  [ -n "$geometry" ] && break
  python3 -c 'import time; time.sleep(0.25)'
done
if [ -z "$geometry" ]; then
  echo "window did not appear; log:" >&2
  cat "$log" >&2
  exit 1
fi
python3 -c "import time; time.sleep($settle)"
grim -g "$geometry" "$out"
echo "screenshot: $out ($geometry)"
echo "--- app log ---"
cat "$log"
