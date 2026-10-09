local s = pc.sketch.new{plane = "XZ", name = "Profile"}
pc.sketch.polyline{sketch = s, points = {{16, 72}, {0, 72}, {0, 0}, {22, 0}}}
pc.sketch.draw{sketch = s, tool = "bspline", params = {bspline_interpolate = true},
  points = {{22, 0}, {31, 16}, {27, 34}, {16, 50}, {13, 62}, {16, 72}, "finish"}}
pc.design.revolve{sketch = s}
assert(#pc.doc.rebuild() == 0)
pc.file.export{path = "/var/tmp/meshes/vase.stl", tolerance = 0.03}
