# A revolved bushing

A flanged bushing turned from its half section. The section is drawn on
XZ, its x running out from the axis and its y up world Z, and
`design.revolve` turns it a full turn about the sketch's y-axis, which
is what it does when given no axis. A chamfer then breaks the sleeve's
top outer edge, picked by a point on the circle and the way the circle
runs there.

The test suite runs this block from an empty document, and its asserts
check what it made.

```lua
local ri, rs, rf = 4, 7, 12     -- bore, sleeve and flange radii
local hf, h = 3, 15             -- flange thickness, overall height

local section = pc.sketch.new{plane = "XZ", name = "Section"}
pc.sketch.polyline{sketch = section, closed = true, points = {
  {ri, 0}, {rf, 0}, {rf, hf}, {rs, hf}, {rs, h}, {ri, h},
}}
local turn = pc.design.revolve{sketch = section}
local body = pc.doc.feature{id = turn}.body

-- The top outer edge is a circle of radius rs at height h: at (0, rs, h)
-- it runs along X.
pc.design.chamfer{body = body, size = 1,
  edges = {Edges = {{point = {0, rs, h}, direction = {1, 0, 0}}}}}

assert(#pc.doc.rebuild() == 0, "every feature builds")

-- The turned section, less the chamfer's triangle turned at its centroid.
local turned = math.pi * ((rf * rf - ri * ri) * hf + (rs * rs - ri * ri) * (h - hf))
local chamfer = 0.5 * 2 * math.pi * (rs - 1 / 3)
local volume = pc.doc.measure{body = body}.volume
assert(math.abs(volume - (turned - chamfer)) < 0.01, volume)

local radii, cones = {}, 0
for _, face in ipairs(pc.doc.faces{body = body}) do
  if face.kind == "cylinder" then radii[#radii + 1] = face.radius end
  if face.kind == "cone" then cones = cones + 1 end
end
table.sort(radii)
assert(#radii == 3 and radii[1] == ri and radii[2] == rs and radii[3] == rf,
  "the bore, the sleeve and the flange")
assert(cones == 1, "the chamfer is one cone")
```
