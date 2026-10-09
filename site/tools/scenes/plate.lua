local s = pc.sketch.new{plane = "XY", name = "Plate"}
pc.sketch.rect{sketch = s, x = 0, y = 0, width = 100, height = 30}
local body = pc.doc.feature{id = pc.design.pad{sketch = s, length = 14}}.body
local function hole(x, fields)
  local at = pc.sketch.new{body = body, plane = "XY", offset = 14}
  pc.sketch.point{sketch = at, x = x, y = 15}
  fields.sketch = at
  pc.design.hole(fields)
end
hole(15, {diameter = 6, through_all = true})
hole(40, {thread = "M6", through_all = true, cut = {Seat = {seat = "SocketHead"}}})
hole(65, {thread = "M6", through_all = true, cut = {Seat = {seat = "Countersunk"}}})
hole(88, {thread = "M8", depth = 12, threaded = true, modeled_thread = true, thread_depth = 10,
  drill_point = {Angled = {}}})
assert(#pc.doc.rebuild() == 0)
pc.file.export{path = "/var/tmp/meshes/plate.stl", tolerance = 0.1}
