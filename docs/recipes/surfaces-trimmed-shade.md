# A shade from a revolved line, cut and thickened

A line turned about the vertical axis makes a cone's side, an open
sheet. A trim by a tilted plane cuts its rim at a slant, and Thicken gives
the sheet a wall: a solid ready to print. The sheet measures an area and
no volume until it is thickened.

The test suite runs this block from an empty document, and its asserts
check what it made.

```lua
local profile = pc.sketch.new{plane = "XZ", name = "Profile"}
pc.sketch.line{sketch = profile, x1 = 30, y1 = 0, x2 = 15, y2 = 40}

local cone = pc.surface.revolve{sketches = {profile}, name = "Shade"}
local body = pc.doc.feature{id = cone}.body
assert(#pc.doc.rebuild() == 0, "the cone builds")

local slant = math.sqrt(15 ^ 2 + 40 ^ 2)
local sheet = pc.doc.measure{body = body}
assert(sheet.volume == nil, "an open sheet has an area and no volume")
assert(math.abs(sheet.area - math.pi * (30 + 15) * slant) < 1e-3, sheet.area)

-- Keep what lies below a plane through z = 30, tilted along X: the
-- side its normal points to.
pc.surface.trim{body = body, plane = {Custom = {origin = {0, 0, 30}, normal = {0.3, 0, -1}}}}
assert(#pc.doc.rebuild() == 0, "the trim keeps the lower part")
local trimmed = pc.doc.measure{body = body}
assert(trimmed.area < sheet.area and trimmed.max[3] < 40, "the top is cut away")

pc.surface.thicken{body = body, thickness = 1.5}
assert(#pc.doc.rebuild() == 0, "the wall builds")
local solid = pc.doc.measure{body = body}
-- About the sheet's area times the wall, a little less on the inside.
assert(math.abs(solid.volume / (trimmed.area * 1.5) - 1) < 0.05, solid.volume)
local cones = 0
for _, face in ipairs(pc.doc.faces{body = body}) do
  if face.kind == "cone" then cones = cones + 1 end
end
assert(cones == 2, "the wall's inside and outside")
```
