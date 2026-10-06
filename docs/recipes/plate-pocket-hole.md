# A plate with a pocket and a hole

A rectangle padded into a plate, a recess pocketed into its top and a
hole drilled through it. The three sketches go in one body: a sketch made
without `body` starts a body of its own, where a pocket has nothing to
cut. The top's sketches sit at the pad's height, so the pocket and the
hole cut down into the plate, against their sketches' normal.

The test suite runs this block from an empty document, and its asserts
check what it made.

```lua
local w, h, t = 60, 40, 6

-- The plate.
local base = pc.sketch.new{plane = "XY", name = "Outline"}
pc.sketch.rect{sketch = base, x = 0, y = 0, width = w, height = h}
local pad = pc.design.pad{sketch = base, length = t}
local body = pc.doc.feature{id = pad}.body

-- A recess 2 mm deep in the top's left half.
local recess = pc.sketch.new{body = body, plane = "XY", offset = t, name = "Recess"}
pc.sketch.rect{sketch = recess, x = 5, y = 5, width = 20, height = 30}
pc.design.pocket{sketch = recess, depth = 2}

-- A 6 mm hole through the right half; the circle gives only its centre.
local drill = pc.sketch.new{body = body, plane = "XY", offset = t, name = "Drill"}
pc.sketch.circle{sketch = drill, x = 45, y = 20, radius = 1}
pc.design.hole{sketch = drill, diameter = 6, through_all = true}

assert(#pc.doc.rebuild() == 0, "every feature builds")
assert(#pc.doc.bodies() == 1, "one body holds it all")

local volume = pc.doc.measure{body = body}.volume
local expected = w * h * t - 20 * 30 * 2 - math.pi * 3 * 3 * t
assert(math.abs(volume - expected) < 0.01, volume)

local bores = 0
for _, face in ipairs(pc.doc.faces{body = body}) do
  if face.kind == "cylinder" then
    bores = bores + 1
    assert(math.abs(face.radius - 3) < 1e-6)
  end
end
assert(bores == 1, "one bore")
```
