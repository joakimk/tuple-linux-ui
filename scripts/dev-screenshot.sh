#!/usr/bin/env bash
# Launch a debug build, wait for its window(s) on Hyprland, screenshot each with grim, quit it.
# Usage: scripts/dev-screenshot.sh [output.png] [seconds-to-wait-before-shot]
# Extra windows (incoming call, dialogs are in-window) are saved as output-2.png, output-3.png…
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

windows() {
  hyprctl clients -j | python3 -c '
import json, sys
for c in json.load(sys.stdin):
    if c.get("class") == "dev.joakimk.TupleUi" and c.get("mapped"):
        x, y = c["at"]; w, h = c["size"]
        print(f"{x},{y} {w}x{h}")
' 2>/dev/null || true
}

for _ in $(seq 1 40); do
  [ -n "$(windows)" ] && break
  python3 -c 'import time; time.sleep(0.25)'
done
if [ -z "$(windows)" ]; then
  echo "window did not appear; log:" >&2
  cat "$log" >&2
  exit 1
fi
python3 -c "import time; time.sleep($settle)"
n=0
while IFS= read -r geometry; do
  n=$((n + 1))
  file="$out"
  [ "$n" -gt 1 ] && file="${out%.png}-$n.png"
  grim -g "$geometry" "$file"
  echo "screenshot: $file ($geometry)"
done <<<"$(windows)"
echo "--- tray items ---"
busctl --user list 2>/dev/null | grep -o 'org.kde.StatusNotifierItem-[0-9]*-[0-9]*' | while read -r name; do
  echo "$name (pid $(echo "$name" | cut -d- -f2))"
done
echo "--- app log ---"
cat "$log"
