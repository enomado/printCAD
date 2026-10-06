# A flange with a bolt circle

A ring padded from two circles (the inner one is a hole in the outer),
one bolt hole drilled from a sketch on its top face, and a polar pattern
repeating that hole round the ring. A pattern repeats the last feature
of the body when it is not told which (`originals`); it turns about Z,
the body's own axis, a full turn by default, and makes 4 of it unless
`occurrences` says otherwise, the first being the original.

The test suite runs this block from an empty document, and its asserts
check what it made.

```lua
local ro, ri, t = 30, 10, 6   -- outer and inner radius, thickness
local pcd, d, n = 40, 5, 6    -- bolt circle diameter, hole diameter, holes

local ring = pc.sketch.new{plane = "XY", name = "Ring"}
pc.sketch.circle{sketch = ring, x = 0, y = 0, radius = ro}
pc.sketch.circle{sketch = ring, x = 0, y = 0, radius = ri}
local pad = pc.design.pad{sketch = ring, length = t}
local body = pc.doc.feature{id = pad}.body

local top = pc.sketch.new{body = body, plane = "XY", offset = t, name = "Bolt hole"}
pc.sketch.point{sketch = top, x = pcd / 2, y = 0}
local hole = pc.design.hole{sketch = top, diameter = d, through_all = true}

local pattern = pc.design.polar_pattern{body = body, occurrences = n}
assert(pc.doc.feature{id = pattern}.fields.PolarPattern.originals[1] == hole,
  "it repeats the hole, the body's last feature")

assert(#pc.doc.rebuild() == 0, "every feature builds")

local volume = pc.doc.measure{body = body}.volume
local expected = math.pi * (ro * ro - ri * ri) * t - n * math.pi * (d / 2) ^ 2 * t
assert(math.abs(volume - expected) < 0.01, volume)

-- Every bolt hole sits on the bolt circle.
local holes = 0
for _, face in ipairs(pc.doc.faces{body = body}) do
  if face.kind == "cylinder" and math.abs(face.radius - d / 2) < 1e-6 then
    holes = holes + 1
    local at = face.axis.point
    assert(math.abs(math.sqrt(at[1] ^ 2 + at[2] ^ 2) - pcd / 2) < 1e-6)
  end
end
assert(holes == n, holes)
```
