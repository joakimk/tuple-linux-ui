#!/usr/bin/env bash
# Launch a debug build, float its window at a fixed size on Hyprland, screenshot it with
# grim, quit it. Extra windows (the incoming-call window) land in output-2.png, output-3.png…
# With TRAY_SHOT=1 the right end of the focused monitor's top bar (where the tray icon
# lives) is captured too, upscaled, as output-tray.png.
# Usage: scripts/dev-screenshot.sh [output.png] [seconds-to-wait-before-shot]
set -euo pipefail
cd "$(dirname "$0")/.."

out="${1:-/tmp/tuple-ui.png}"
settle="${2:-3}"
width="${SHOT_WIDTH:-420}"
height="${SHOT_HEIGHT:-640}"
target="$(cargo metadata --format-version 1 2>/dev/null | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
nice -n 19 cargo build -q -j 2
log="$(mktemp -t tuple-ui-log.XXXXXX)"

"$target/debug/tuple-ui" >"$log" 2>&1 &
pid=$!
trap 'kill "$pid" 2>/dev/null || true' EXIT

clients() {  # address geometry, one per mapped tuple-ui window
  hyprctl clients -j | python3 -c '
import json, sys
for c in json.load(sys.stdin):
    if c.get("class") == "dev.joakimk.TupleUi" and c.get("mapped"):
        x, y = c["at"]; w, h = c["size"]
        print(c["address"], f"{x},{y} {w}x{h}")
' 2>/dev/null || true
}

for _ in $(seq 1 40); do
  [ -n "$(clients)" ] && break
  python3 -c 'import time; time.sleep(0.25)'
done
if [ -z "$(clients)" ]; then
  echo "window did not appear; log:" >&2
  cat "$log" >&2
  exit 1
fi
# Float the main window at a known size so screenshots are comparable.
python3 -c 'import time; time.sleep(0.6)'
first="$(clients | head -1 | cut -d' ' -f1)"
hyprctl dispatch focuswindow "address:$first" >/dev/null || true
hyprctl dispatch setfloating "address:$first" >/dev/null || true
python3 -c 'import time; time.sleep(0.3)'
hyprctl dispatch resizewindowpixel "exact $width $height,address:$first" >/dev/null || true
hyprctl dispatch centerwindow >/dev/null || true
python3 -c "import time; time.sleep($settle)"
n=0
while IFS= read -r line; do
  n=$((n + 1))
  geometry="${line#* }"
  file="$out"
  [ "$n" -gt 1 ] && file="${out%.png}-$n.png"
  grim -g "$geometry" "$file"
  echo "screenshot: $file ($geometry)"
done <<<"$(clients)"
if [ "${TRAY_SHOT:-}" = 1 ]; then
  read -r mx my mw <<<"$(hyprctl monitors -j | python3 -c 'import json,sys; m=[m for m in json.load(sys.stdin) if m["focused"]][0]; print(m["x"], m["y"], m["width"])')"
  bar_h="$(hyprctl layers -j | python3 -c 'import json,sys; d=json.load(sys.stdin); print(max((i["h"] for mon in d.values() for lvl in mon["levels"].values() for i in lvl if "bar" in i["namespace"]), default=30))')"
  tray_w="${TRAY_WIDTH:-260}"
  grim -s 3 -g "$((mx + mw - tray_w)),$my ${tray_w}x${bar_h}" "${out%.png}-tray.png"
  echo "screenshot: ${out%.png}-tray.png (bar tray area)"
fi
echo "--- tray items ---"
busctl --user list 2>/dev/null | grep -o 'org.kde.StatusNotifierItem-[0-9]*-[0-9]*' | sed "s/\$/ (ours: $pid)/"
echo "--- app log ---"
cat "$log"
