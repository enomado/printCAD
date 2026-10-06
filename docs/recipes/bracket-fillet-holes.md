# An angle bracket with holes and a fillet

An L-shaped bracket: its side drawn on XZ and padded across, two holes
down through the foot, one through the upright, and the inside corner
rounded. XZ faces -Y, so the pad is `reversed` to grow toward +Y. Each
hole's sketch is started in the bracket's body on the face it drills
from: the foot's top is XY at the wall's thickness, the upright's inside
face YZ at the same offset (YZ faces +X, and a hole drills against its
sketch's normal, into the wall). The fillet goes last and finds its edge
by a point on it and the way it runs.

The test suite runs this block from an empty document, and its asserts
check what it made.

```lua
local t, w = 5, 30           -- wall thickness, bracket width
local foot, height = 40, 30  -- the legs' lengths
local r, d = 4, 5            -- fillet radius, hole diameter

local side = pc.sketch.new{plane = "XZ", name = "Side"}
pc.sketch.polyline{sketch = side, closed = true, points = {
  {0, 0}, {foot, 0}, {foot, t}, {t, t}, {t, height}, {0, height},
}}
local pad = pc.design.pad{sketch = side, length = w, reversed = true}
local body = pc.doc.feature{id = pad}.body

-- Two holes down through the foot; a circle gives only its centre.
local down = pc.sketch.new{body = body, plane = "XY", offset = t, name = "Foot holes"}
pc.sketch.circle{sketch = down, x = 27, y = 8, radius = 1}
pc.sketch.circle{sketch = down, x = 27, y = w - 8, radius = 1}
pc.design.hole{sketch = down, diameter = d, through_all = true}

-- One through the upright, at a point of a sketch on its inside face.
-- On YZ the sketch's x runs along world Y and its y up world Z.
local across = pc.sketch.new{body = body, plane = "YZ", offset = t, name = "Wall hole"}
pc.sketch.point{sketch = across, x = w / 2, y = 20}
pc.design.hole{sketch = across, diameter = d, through_all = true}

-- The inside corner's edge runs along Y at x = t, z = t.
pc.design.fillet{body = body, radius = r,
  edges = {Edges = {{point = {t, w / 2, t}, direction = {0, 1, 0}}}}}

assert(#pc.doc.rebuild() == 0, "every feature builds")

local section = foot * t + (height - t) * t
local rounded = r * r - math.pi * r * r / 4  -- what the fillet adds in the corner
local drilled = 3 * math.pi * (d / 2) ^ 2 * t
local m = pc.doc.measure{body = body}
assert(not m.approximate, "every face measured in closed form")
assert(math.abs(m.volume - ((section + rounded) * w - drilled)) < 0.01, m.volume)

local bores, rounds = 0, 0
for _, face in ipairs(pc.doc.faces{body = body}) do
  if face.kind == "cylinder" then
    if math.abs(face.radius - d / 2) < 1e-6 then bores = bores + 1 end
    if math.abs(face.radius - r) < 1e-6 then rounds = rounds + 1 end
  end
end
assert(bores == 3 and rounds == 1, "three bores and the round")
```
