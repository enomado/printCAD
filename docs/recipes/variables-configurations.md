# A plate sized by variables and a configuration table

A plate whose outline and thickness come from a variable set, and a
configuration table that switches between sizes. The rectangle's two
sides are dimensioned (`sketch.constrain` with `distance` gives each
side a length and returns the dimension's id), and `doc.set_formula`
binds each dimension, by that id, and the pad's `length`, by its field
name, to a formula. The configurations set only the variables made
columns with `config.add_variable`; `Size.thick` follows `Size.width`
in every one of them. Activating none puts the variables' own formulas
back.

The test suite runs this block from an empty document, and its asserts
check what it made.

```lua
pc.var.new{name = "Size"}
pc.var.set{set = "Size", name = "width", formula = "60 mm"}
pc.var.set{set = "Size", name = "depth", formula = "40 mm"}
pc.var.set{set = "Size", name = "thick", formula = "Size.width / 20"}

-- The outline's numbers are placeholders: the formulas set them.
local s = pc.sketch.new{plane = "XY", name = "Plate"}
local sides = pc.sketch.rect{sketch = s, x = 0, y = 0, width = 10, height = 10}
local wide = pc.sketch.constrain{sketch = s, kind = "distance", items = {sides[1]}, value = 10}[1]
local deep = pc.sketch.constrain{sketch = s, kind = "distance", items = {sides[2]}, value = 10}[1]
pc.doc.set_formula{id = s, parameter = wide, formula = "Size.width"}
pc.doc.set_formula{id = s, parameter = deep, formula = "Size.depth"}

local pad = pc.design.pad{sketch = s}
pc.doc.set_formula{id = pad, parameter = "length", formula = "Size.thick"}
local body = pc.doc.feature{id = pad}.body

pc.config.add_variable{variable = "Size.width"}
pc.config.add_variable{variable = "Size.depth"}
pc.config.new{name = "Small"}
pc.config.set{name = "Small", variable = "Size.width", value = "40 mm"}
pc.config.set{name = "Small", variable = "Size.depth", value = "20 mm"}
pc.config.new{name = "Large", like = "Small"}
pc.config.set{name = "Large", variable = "Size.width", value = "100 mm"}

-- The plate's extent along X, Y and Z, built as it stands.
local function size()
  assert(#pc.doc.rebuild() == 0, "the plate builds")
  local m = pc.doc.measure{body = body}
  return {m.max[1] - m.min[1], m.max[2] - m.min[2], m.max[3] - m.min[3]}
end
local function is(got, x, y, z)
  assert(math.abs(got[1] - x) < 1e-6 and math.abs(got[2] - y) < 1e-6
    and math.abs(got[3] - z) < 1e-6, table.concat(got, " x "))
end

is(size(), 60, 40, 3)
pc.config.activate{name = "Small"}
is(size(), 40, 20, 2)
pc.config.activate{name = "Large"}   -- like Small, but 100 wide
is(size(), 100, 20, 5)
pc.config.activate{}                 -- the variables' own formulas again
is(size(), 60, 40, 3)

local configs = pc.config.list{}
assert(#configs.columns == 2 and #configs.rows == 2)
```
