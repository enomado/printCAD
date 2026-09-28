#!/usr/bin/env bash
# Renders the application icon from its two drawings into the files the
# builds carry: printcad-small.svg (no layer lines, a bigger block) for 48 px
# and below, printcad.svg above. Needs rsvg-convert and ImageMagick.
#
#   scripts/app-icon.sh
set -euo pipefail
dir="$(cd "$(dirname "$0")/.." && pwd)/crates/app_shell/assets/icon"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

render() { # size -> $work/<size>.png
  local size=$1 source=printcad.svg
  [ "$size" -le 48 ] && source=printcad-small.svg
  rsvg-convert -w "$size" -h "$size" "$dir/$source" -o "$work/$size.png"
}

for size in 16 24 32 48 64 128 256 512 1024; do render "$size"; done
# The window's icon and the Linux desktop entry's.
cp "$work/256.png" "$dir/printcad-256.png"
# Windows: every size a shell asks for, in one file.
magick "$work"/{16,24,32,48,64,128,256}.png "$dir/printcad.ico"
# macOS: the sizes an application bundle carries, each a PNG under its
# four-letter type (a Retina size is the next one up).
python3 - "$work" "$dir/printcad.icns" <<'PY'
import struct, sys
work, out = sys.argv[1], sys.argv[2]
types = [("icp4", 16), ("icp5", 32), ("icp6", 64), ("ic07", 128), ("ic08", 256),
         ("ic09", 512), ("ic10", 1024), ("ic11", 32), ("ic12", 64), ("ic13", 256),
         ("ic14", 512)]
body = b""
for kind, size in types:
    png = open(f"{work}/{size}.png", "rb").read()
    body += kind.encode() + struct.pack(">I", 8 + len(png)) + png
open(out, "wb").write(b"icns" + struct.pack(">I", 8 + len(body)) + body)
PY
ls -l "$dir"
