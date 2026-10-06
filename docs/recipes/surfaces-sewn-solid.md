# A solid sewn from surfaces

A closed outline extruded as walls, capped at both ends with flat
surfaces and sewn: a closed shell sewn becomes a solid, which measures
and takes features as any other. The walls go in the outline sketch's
body; the top cap's sketch is started in that body with
`pc.sketch.new{body = ...}`, as the Surface bench's Create sketch does.

The test suite runs this block from an empty document, and its asserts
check what it made.

```lua
local s = pc.sketch.new{plane = "XY", name = "Outline"}
pc.sketch.polyline{sketch = s, points = {{0, 0}, {10, 0}, {10, 10}, {0, 10}}, closed = true}

local walls = pc.surface.extrude{sketches = {s}, length = 5}
local body = pc.doc.feature{id = walls}.body

-- The bottom cap from the outline itself, the top from a copy at the walls' top.
pc.surface.planar{body = body, sketches = {s}}
local lid = pc.sketch.new{body = body, plane = "XY", offset = 5, name = "Lid"}
pc.sketch.polyline{sketch = lid, points = {{0, 0}, {10, 0}, {10, 10}, {0, 10}}, closed = true}
pc.surface.planar{body = body, sketches = {lid}}

pc.surface.sew{body = body}
assert(#pc.doc.rebuild() == 0, "every step builds")

assert(#pc.doc.faces{body = body} == 6, "four walls and two caps")
-- A sewn shell measures close to, not exactly, its volume.
local volume = pc.doc.measure{body = body}.volume
assert(math.abs(volume - 500) < 5, volume)
```
