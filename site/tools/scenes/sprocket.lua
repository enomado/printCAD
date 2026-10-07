local sprocket = pc.design.sprocket{plane = "XY", teeth = 15, bore = 10}
pc.design.pad{sketch = sprocket, length = 5}
assert(#pc.doc.rebuild() == 0)
pc.file.export{path = "/var/tmp/meshes/sprocket.stl", tolerance = 0.1}
