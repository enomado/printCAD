#!/bin/sh
# Installs printCAD for this user from an unpacked Linux release: the
# programs under ~/.local/share/printcad, `printcad` in ~/.local/bin, and its
# menu entry and icon. Run it again after unpacking a newer release;
# `./install.sh --remove` takes it all away. The release carries this file as
# install.sh.
set -e
here="$(cd "$(dirname "$0")" && pwd)"
data="${XDG_DATA_HOME:-$HOME/.local/share}"
home="$data/printcad"
bin="$HOME/.local/bin"
icon="$data/icons/hicolor/256x256/apps/printcad.png"
entry="$data/applications/printcad.desktop"
if [ "${1:-}" = "--remove" ]; then
  rm -rf "$home" "$bin/printcad" "$icon" "$entry"
  echo "printCAD removed"
  exit 0
fi
mkdir -p "$home" "$bin" "$(dirname "$icon")" "$(dirname "$entry")"
cp "$here/printcad" "$here/printcad-serverd" "$home/"
ln -sf "$home/printcad" "$bin/printcad"
cp "$here/printcad.png" "$icon"
sed "s|^Exec=printcad|Exec=$home/printcad|" "$here/printcad.desktop" > "$entry"
if command -v update-desktop-database >/dev/null; then
  update-desktop-database "$data/applications" || true
fi
if command -v gtk-update-icon-cache >/dev/null; then
  gtk-update-icon-cache -q "$data/icons/hicolor" 2>/dev/null || true
fi
echo "printCAD installed: run printcad, or find it in your applications"
