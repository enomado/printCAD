local t, w = 5, 30
local foot, height = 40, 30
local side = pc.sketch.new{plane = "XZ", name = "Side"}
pc.sketch.polyline{sketch = side, closed = true, points = {
  {0, 0}, {foot, 0}, {foot, t}, {t, t}, {t, height}, {0, height},
}}
pc.design.pad{sketch = side, length = w, reversed = true}
assert(#pc.doc.rebuild() == 0)
pc.file.export{path = "/var/tmp/meshes/pad.stl", tolerance = 0.02}
