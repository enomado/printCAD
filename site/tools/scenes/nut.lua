local s = pc.sketch.new{plane = "XY"}
pc.sketch.rect{sketch = s, x = 0, y = 0, width = 30, height = 20}
local body = pc.doc.feature{id = pc.design.pad{sketch = s, length = 12}}.body
local at = pc.sketch.new{body = body, plane = "XY", offset = 12}
pc.sketch.point{sketch = at, x = 15, y = 10}
pc.design.hole{sketch = at, thread = "M5", through_all = true, nut_trap = true}
assert(#pc.doc.rebuild() == 0)
pc.file.export{path = "/var/tmp/meshes/nut.stl", tolerance = 0.02}
