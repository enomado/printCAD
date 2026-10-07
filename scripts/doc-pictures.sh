#!/usr/bin/env bash
# Draw the guides' pictures of models into docs/images/, each from a
# script run headless: a recipe's own Lua, or a few lines here, ending in
# pc.doc.picture. The same arguments draw the same picture.
#
#   scripts/doc-pictures.sh [path/to/printcad]
#
# Pictures of the app's panels and tools are captured from the running
# app instead (see docs/images/README.md).
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
printcad="${1:-$root/target/release/printcad}"
out="$root/docs/images"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$out"

# The Lua of a recipe: every ```lua block of docs/recipes/<name>.md.
recipe() {
  awk '/^```lua$/ {on = 1; next} /^```$/ {on = 0} on' "$root/docs/recipes/$1.md"
}

# Run Lua from stdin headless, a picture call at its end writing $out/...
run() {
  local name="$1"
  cat > "$work/$name.lua"
  "$printcad" --script "$work/$name.lua" >/dev/null 2>"$work/$name.log" || {
    cat "$work/$name.log" >&2
    echo "$name failed" >&2
    exit 1
  }
  echo "drew docs/images/$name.png"
}

size='size = {960, 600}'

# Editing: the bracket recipe, its fillet painted.
{
  recipe bracket-fillet-holes
  cat <<EOF
local round
for i, face in ipairs(pc.doc.faces{body = body}) do
  if face.kind == "cylinder" and math.abs(face.radius - r) < 1e-6 then round = i - 1 end
end
pc.doc.picture{path = "$out/bracket-fillet.png", view = {azimuth = 35, elevation = 25}, $size,
  highlight = {{body = body, faces = {round}}}}
EOF
} | run bracket-fillet

# Holes: a plate drilled four ways, cut through their centres.
run holes-section <<EOF
local s = pc.sketch.new{plane = "XY", name = "Plate"}
pc.sketch.rect{sketch = s, x = 0, y = 0, width = 100, height = 30}
local body = pc.doc.feature{id = pc.design.pad{sketch = s, length = 14}}.body
local function hole(x, fields)
  local at = pc.sketch.new{body = body, plane = "XY", offset = 14}
  pc.sketch.point{sketch = at, x = x, y = 15}
  fields.sketch = at
  pc.design.hole(fields)
end
hole(15, {diameter = 6, through_all = true})
hole(40, {thread = "M6", through_all = true, cut = {Seat = {seat = "SocketHead"}}})
hole(65, {thread = "M6", through_all = true, cut = {Seat = {seat = "Countersunk"}}})
hole(88, {thread = "M8", depth = 12, threaded = true, modeled_thread = true, thread_depth = 10,
  drill_point = {Angled = {}}})
assert(#pc.doc.rebuild() == 0)
pc.doc.picture{path = "$out/holes-section.png", view = {azimuth = 0, elevation = 18},
  size = {1100, 300},
  section = {origin = {0, 15, 0}, normal = {0, -1, 0}}}
EOF

# Printing: a nut trap under an M5 hole, cut through its centre.
run nut-trap <<EOF
local s = pc.sketch.new{plane = "XY"}
pc.sketch.rect{sketch = s, x = 0, y = 0, width = 30, height = 20}
local body = pc.doc.feature{id = pc.design.pad{sketch = s, length = 12}}.body
local at = pc.sketch.new{body = body, plane = "XY", offset = 12}
pc.sketch.point{sketch = at, x = 15, y = 10}
pc.design.hole{sketch = at, thread = "M5", through_all = true, nut_trap = true}
assert(#pc.doc.rebuild() == 0)
pc.doc.picture{path = "$out/nut-trap.png", view = {azimuth = 30, elevation = 40}, $size,
  section = {origin = {0, 10, 0}, normal = {0, -1, 0}}}
EOF

# Generators: a gear and a sprocket, each a sketch padded.
run generators <<EOF
local gear = pc.design.gear{plane = "XY", teeth = 18, module = 2, bore = 10,
  keyway = {on = true}}
local gear_body = pc.doc.feature{id = gear}.body
pc.design.pad{sketch = gear, length = 8}
local sprocket = pc.design.sprocket{plane = "XY", teeth = 15, bore = 10}
local sprocket_body = pc.doc.feature{id = sprocket}.body
pc.design.pad{sketch = sprocket, length = 5}
pc.asm.place{body = sprocket_body, translation = {58, 0, 0}}
assert(#pc.doc.rebuild() == 0)
pc.doc.picture{path = "$out/generators.png", view = {azimuth = 20, elevation = 45}, $size}
EOF

# Surfaces: the shade recipe.
{
  recipe surfaces-trimmed-shade
  echo "pc.doc.picture{path = \"$out/surface-shade.png\", view = {azimuth = 30, elevation = 30}, $size}"
} | run surface-shade

# Assembly: the hinged arm recipe.
{
  recipe hinged-arm
  echo "pc.doc.picture{path = \"$out/hinged-arm.png\", view = {azimuth = 35, elevation = 30}, $size}"
} | run hinged-arm
