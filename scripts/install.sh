#!/usr/bin/env bash
# Install tuple-ui from a release tarball into ~/.local (no root needed).
# Run from the extracted tarball directory: ./install.sh   (or ./install.sh --uninstall)
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
bin_dir="${HOME}/.local/bin"
app_dir="${HOME}/.local/share/applications"
desktop="dev.joakimk.TupleUi.desktop"

if [ "${1:-}" = "--uninstall" ]; then
  rm -f "$bin_dir/tuple-ui" "$app_dir/$desktop"
  update-desktop-database "$app_dir" 2>/dev/null || true
  echo "tuple-ui removed from $bin_dir"
  exit 0
fi

for lib in libgtk-4.so.1 libadwaita-1.so.0; do
  if ! ldconfig -p 2>/dev/null | grep -q "$lib"; then
    echo "warning: $lib not found; install GTK 4 and libadwaita from your distribution" >&2
  fi
done
if ! command -v tuple >/dev/null 2>&1; then
  echo "warning: the 'tuple' CLI is not on your PATH; install Tuple for Linux first" >&2
fi

mkdir -p "$bin_dir" "$app_dir"
install -m 755 "$here/tuple-ui" "$bin_dir/tuple-ui"
install -m 644 "$here/$desktop" "$app_dir/$desktop"
update-desktop-database "$app_dir" 2>/dev/null || true

echo "installed $bin_dir/tuple-ui and the desktop entry"
case ":$PATH:" in
  *":$bin_dir:"*) ;;
  *) echo "note: add $bin_dir to your PATH to run 'tuple-ui' from a terminal" ;;
esac
echo "run: tuple-ui        (or tuple-ui --tray to start hidden)"
