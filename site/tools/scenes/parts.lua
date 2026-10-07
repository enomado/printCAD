local gear = pc.design.gear{plane = "XY", teeth = 18, module = 2, bore = 10, keyway = {on = true}}
pc.design.pad{sketch = gear, length = 8}
assert(#pc.doc.rebuild() == 0)
pc.file.export{path = "/var/tmp/meshes/gear.stl", tolerance = 0.1}
