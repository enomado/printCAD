# A hinged arm, swung through a motion

Two bodies, each a plate padded with a bore: a base, grounded, and an
arm held to it by a hinge through both bores. The hinge's faces are the
bores as `doc.faces` lists them: a round face carries its `axis`, which
is what the hinge reads. `offset` sets the arm 5 mm up the axis, on top
of the base. A motion turns the hinge 90 degrees a second, and
`asm.motion_frames` and `asm.trace` work it out frame by frame without
moving anything, so a script can check a mechanism without a window.

The test suite runs this block from an empty document, and its asserts
check what it made.

```lua
-- A plate with a 5 mm bore; a sketch made without `body` starts a body.
local function plate(name, w, h, bx, by)
  local s = pc.sketch.new{plane = "XY", name = name}
  pc.sketch.rect{sketch = s, x = 0, y = 0, width = w, height = h}
  pc.sketch.circle{sketch = s, x = bx, y = by, radius = 2.5}
  return pc.doc.feature{id = pc.design.pad{sketch = s, length = 5}}.body
end
local base = plate("Base", 40, 40, 20, 20)
local arm = plate("Arm", 60, 10, 5, 5)
assert(#pc.doc.rebuild() == 0, "both plates build")

local function bore(body)
  for _, face in ipairs(pc.doc.faces{body = body}) do
    if face.kind == "cylinder" then return face end
  end
end

pc.asm.ground{body = base}
local hinge = pc.asm.hinge{body = arm, face = bore(arm),
  other = base, other_face = bore(base), offset = 5}

-- The arm's bore, at (5, 5) of its own, now stands on the base's, at (20, 20), 5 up.
local at = pc.asm.placement{body = arm}.translation
assert(math.abs(at[1] - 15) < 1e-6 and math.abs(at[2] - 15) < 1e-6 and math.abs(at[3] - 5) < 1e-6)

local free = pc.asm.freedom{body = arm}[1]
assert(free.free == 1 and free.motions[1].turn ~= nil, "it can only turn")
assert(#pc.asm.interference{}.clashes == 0, "the arm rests on the base")

-- A quarter turn in a second, in half-second frames.
local study = pc.asm.motion{drives = {{joint = hinge, formula = "90 * t"}}, ["end"] = 1, step = 0.5}
local frames = pc.asm.motion_frames{study = study}
assert(#frames == 3 and frames[3].t == 1)

-- The arm's far top corner, in its own frame, swings from +X to +Y about the bores.
local path = pc.asm.trace{study = study, body = arm, point = {60, 5, 5}}
local function near(p, x, y, z)
  return math.abs(p[1] - x) < 1e-3 and math.abs(p[2] - y) < 1e-3 and math.abs(p[3] - z) < 1e-3
end
assert(near(path[1].point, 75, 20, 10), "out along X at the start")
assert(near(path[3].point, 20, 75, 10), "out along Y a quarter turn on")

-- Working the motion out moved nothing.
assert(math.abs(pc.asm.travel{joint = hinge}) < 1e-6)
```
