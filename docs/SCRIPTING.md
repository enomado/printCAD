# Scripting

printCAD runs Lua 5.4 scripts. Everything the application and its
workbenches can do by command is a function under `pc`, called with named
arguments in braces:

```lua
local s = pc.sketch.new{plane = "XY"}
pc.sketch.rect{sketch = s, x = 0, y = 0, width = 30, height = 20}
local pad = pc.design.pad{sketch = s, length = 12}
pc.design.set{feature = pad, length = 20}
```

The Design workbench's commands were once under `part` (`pc.part.pad`,
`part.set`); those names still run the same commands, so older scripts,
recordings and key settings keep working.

## Where scripts run

- **The console.** Windows › Console, Scripts › Console, or the toolbar's
  Scripts button. Enter runs what is typed, Shift+Enter starts another
  line, Tab completes a command name, Up and Down go back through what was
  run. An expression shows its value. Globals stay set between runs, and
  `local` names last one run only. Save as script writes everything run
  in the console since the start as a new script in the scripts folder.
- **Script files.** Scripts › Run script… runs any `.lua` file. Every
  `.lua` file in the scripts folder (`~/.config/printcad/scripts`) is also
  a command of its own: it shows in the Scripts menu, the toolbar's Scripts
  button and the command palette, and takes a key in Preferences ›
  Keyboard. Its first comment line is its description. Scripts › New
  script starts one from a template.
- **The command line.** No window opens:

  ```sh
  printcad --script build.lua --open part.prtcad --save out.prtcad -- 40 20
  ```

  `--open` and `--save` are optional. The words after `--` are the
  script's `arg` table. Output goes to stdout and log lines to stderr. The
  exit code is 0 when the script finishes and 1 when it stops on an error.
  Commands that need a window (the view, the selection, tools) are not
  available there.

## Working with commands

- `help()` lists every command; `help("sketch")` those starting with
  `sketch`. `show(value)` prints a table.
- Ids of bodies, features, sketch elements and constraints are strings.
  Commands that make something answer its id.
- An empty `{}` in a command's arguments is the empty list. `array()`
  says the same anywhere else, and `array(1, 2)` is the same as `{1, 2}`.
- A feature's fields are what `pc.doc.feature{id = f}.fields` shows, set
  by name (`pc.design.set{feature = f, length = 25}`). A field holding no
  value is nil in Lua, so `fields` leaves it out and `unset` names it. A
  choice is its name as a string (`axis = "SketchY"`); a choice that
  carries values is a table under its name:
  `axis = {Custom = {origin = {0, 5}, dir = {0, 1}}}` turns a revolution
  about the line through (0, 5) along the sketch's Y, in sketch
  coordinates. A datum is `{Datum = id}` wherever a feature takes one (a
  revolution's axis, a mirror's plane, a pattern's direction), and an
  empty `{}` stands for a table of settings where one is wanted
  (`params = {}`).
- A command that fails raises a Lua error with the reason, which stops the
  script. `pcall(pc.design.pad, {sketch = s})` catches it instead.
- Every change is an ordinary edit, so Undo takes it back. A console line
  or a script run is one undo step, and so is anything you edit by hand
  while it runs.
- Solids rebuild after a script, as they do after a click. To read a solid
  in the same script, call `pc.doc.rebuild()` first: it waits and answers
  the features that failed.
- `pc.doc.faces{body = b}` lists a solid's faces with a point on each and
  a flat face's normal or a round face's axis. Assembly joints take those
  faces as they are listed.
- Scripts run on a thread of their own, so the window stays live while
  one runs. The status bar and the console show it with a Stop button.
  Lines and scripts started meanwhile wait their turn. On the command line
  a run stops after an hour.

## Recording

Scripts › Record… (or the toolbar's Scripts button) records what you do
from then on; Stop saves it as `recording_<n>.lua` in the scripts folder,
where the Scripts menu lists it. The status bar shows the recording with a
Stop button.

A click and a script run the same code: every tool ends in the command it
stands for, and the recording is the list of those commands.

- A sketch tool records one `pc.sketch.draw` call per shape: the tool, the
  points its clicks landed on in the sketch, the values typed at them, and
  its settings. A replay runs the same tool over the same points, so it
  snaps and constrains the same way.
- Constraint tools, dimension edits, drags, deletes and construction record
  as `sketch.constrain`, `sketch.set_value`, `sketch.drag`, `sketch.delete`
  and `sketch.construction`.
- A Design feature records when its task closes with OK, as the
  command that makes it with the fields that differ from what that command
  makes on its own. An edit records as `design.set` with the fields changed.
- A joint records with its faces where the bodies were before it moved
  them; a move records the placement it ended at.
- The sketcher's other actions record too: arrays, cut and paste (the
  pasted geometry goes into the script), mirrored and merged sketches,
  carbon copies, external geometry, a new plane, driving and active flags.
- Renaming, showing or hiding, suppressing, reordering, moving the tip of
  and deleting tree rows record as `doc.*`, as do Repair shape and Convert
  to solid; an import records as `file.import` with its path; the Solve
  button as `asm.solve`.
- What a recording makes is named (`pad1`, `rect2.elements[3]`), and later
  lines use the name, so a replay works on the things it makes. Things
  that were there before the recording started are named by their id: the
  script expects the same document.
- Undo and Redo are not recorded; what they take back stays in the
  recording. Nor are the view, the selection or a script run meanwhile.

## Variables and formulas

Any number of a feature can be set by a formula over variables and other
objects' dimensions, and follows when they change:

```lua
pc.var.new{name = "Printer"}
pc.var.set{set = "Printer", name = "nozzle", formula = "0.4 mm"}
pc.var.set{set = "Printer", name = "wall", formula = "3 * Printer.nozzle"}
local pad = pc.design.pad{sketch = s, length = 5}
pc.doc.set_formula{id = pad, parameter = "length", formula = "Printer.wall * 10"}
print(pc.var.eval{formula = "Pad.length"}.text)  -- 12 mm
```

`pc.doc.parameters{id = ...}` lists a feature's numbers and what formulas
call them. See [Variables and formulas](VARIABLES.md) for what a formula
can say.

## Examples

A plate with a centred hole, sized from the command line, written as 3MF:

```lua
-- A plate with a hole, sized from the command line
local w, h = tonumber(arg[1]) or 40, tonumber(arg[2]) or 30
local s = pc.sketch.new{plane = "XY"}
pc.sketch.rect{sketch = s, x = 0, y = 0, width = w, height = h}
local pad = pc.design.pad{sketch = s, length = 4}
local body = pc.doc.feature{id = pad}.body
local hole = pc.sketch.new{body = body, plane = "XY", offset = 4}
pc.sketch.circle{sketch = hole, x = w / 2, y = h / 2, radius = 3}
pc.design.pocket{sketch = hole, through_all = true}
pc.doc.rebuild()
print(pc.doc.measure{body = body}.volume)
pc.file.export{path = "plate.3mf"}
```

A constrained sketch:

```lua
local s = pc.sketch.new{}
local line = pc.sketch.line{sketch = s, x1 = 0, y1 = 0, x2 = 30, y2 = 2}
pc.sketch.constrain{sketch = s, kind = "horizontal", items = {line}}
pc.sketch.constrain{sketch = s, kind = "dimension", items = {line}, value = 25}
print(show(pc.sketch.status{sketch = s}))
```

One body on another:

```lua
local top = function(body)
  for _, f in ipairs(pc.doc.faces{body = body}) do
    if f.normal and f.normal[3] > 0.99 then return f end
  end
end
local bottom = function(body)
  for _, f in ipairs(pc.doc.faces{body = body}) do
    if f.normal and f.normal[3] < -0.99 then return f end
  end
end
pc.asm.mate{body = lid, face = bottom(lid), other = box, other_face = top(box)}
```

## Commands

<!-- commands: generated from the registered commands -->
### doc

`pc.doc.info`: The document's name, file and unit.

- Returns {name, file, unit, modified}

`pc.doc.agent_rules`: The rules an AI agent working on this document keeps to, beside the ones in Preferences for every document.

- Returns the rules, as text

`pc.doc.set_agent_rules`: Set the rules an AI agent working on this document keeps to; they are saved with the document.

- `rules` (string): The rules, as plain text

`pc.doc.bodies`: List the bodies.

- Returns a list of {id, name, visible, frozen, selectable, material, features}

`pc.doc.features`: List the features in build order.

- `body` (id, optional): Only this body's
- Returns a list of {id, name, kind, body, visible, suppressed, error}

`pc.doc.feature`: A feature with its fields.

- `id` (id)
- Returns {id, name, kind, body, visible, suppressed, error, fields, unset, values}: unset names the fields holding no value, which fields leaves out; values is the data as the feature is built now (formulas worked out, a plane following what it stands on), given when it differs from fields

`pc.doc.selection`: What is selected.

- Returns {item, body, feature}, each an id or nil

`pc.doc.select`: Select a body or a feature, as a click on its row.

- `id` (id)

`pc.doc.new_body`: Make an empty body.

- `name` (string, optional)
- Returns the body's id

`pc.doc.rename`: Rename a body or a feature.

- `id` (id)
- `name` (string)

`pc.doc.set_body`: Change a body: its colour, how much shows through, material, placement, whether it is frozen or clicks pick it.

- `body` (id)
- `color` (any, optional): {r, g, b} from 0 to 1, or nil for the colour it came with
- `opacity` (number, optional): 1 solid, less to see through
- `material` (any, optional): {name, density} with the density in g/cm³, or nil for none
- `frozen` (boolean, optional): Keep it as it stands: its features are not rebuilt until it thaws
- `selectable` (boolean, optional): false lets clicks pass through it
- `face_colors` (list, optional): An empty list gives every face the body's colour again
- `translation` (any, optional): {x, y, z}: where its origin goes; bodies moving as one with it follow
- `rotation` (any, optional): {x, y, z, w}: its turn as a quaternion

`pc.doc.set_face_color`: Colour one face of a body, over the body's colour.

- `body` (id)
- `face` (integer): The face, as doc.faces numbers it from 0
- `color` (any, optional): {r, g, b} from 0 to 1, or nil for the body's colour

`pc.doc.linked_copy`: A linked copy of a body beside it: the same shape, following every change.

- `body` (id)
- Returns the copy's id

`pc.doc.move_after`: Move a feature in its body's history to just after another.

- `id` (id): The feature
- `after` (id, optional): The feature it goes after; first in its body when left out

`pc.doc.recompute`: Build a body again from its history.

- `body` (id)

`pc.doc.set_visible`: Show or hide a body or a feature.

- `id` (id)
- `visible` (boolean)

`pc.doc.delete`: Delete a body or a feature.

- `id` (id)

`pc.doc.repair`: Repair the shapes the kernel's checker calls broken.

- `bodies` (list): The bodies
- Returns nothing; pc.doc.rebuild() waits for the repair

`pc.doc.convert_to_solid`: Turn mesh bodies into solids.

- `bodies` (list): The mesh bodies
- Returns nothing; pc.doc.rebuild() waits for the conversion

`pc.doc.replace_shape`: Read a body's shape from another file: its first solid becomes the shape the body's features build on.

- `body` (id): An imported or converted body, or one with a base shape
- `path` (string): A STEP, IGES or mesh file
- Returns nothing; pc.doc.rebuild() waits for the new shape

`pc.doc.suppress`: Leave a feature out of its body's solid, or back in.

- `id` (id): The feature
- `suppressed` (boolean, optional): true (the default) or false

`pc.doc.move`: Move a feature one step in its body's history.

- `id` (id): The feature
- `up` (boolean): true: earlier, false: later
- Returns true; a move past the end of the history, or past a feature one of the two is built from, fails saying so

`pc.doc.set_tip`: Build a body only up to a feature, or all of it again.

- `id` (id): A feature of the body
- `clear` (boolean, optional): true: build the whole history again

`pc.doc.rebuild`: Rebuild every solid that changed, repair or convert what was asked, and wait.

- `timeout` (number, optional): Seconds to wait at most (60)
- Returns a list of {feature, error} for every feature that failed

`pc.doc.faces`: The faces of a body's solid, where it sits.

- `body` (id)
- Returns a list of {index, kind, point, area, normal?, axis?, radius?}: point lies on the face, normal is a flat face's outward one, axis a turned face's {point, direction}

`pc.doc.measure`: A body's volume, surface area, centre and bounds.

- `body` (id)
- Returns {volume, area, centre, min, max, approximate}

`pc.doc.parameters`: A feature's numbers that formulas set and read.

- `id` (id): The feature
- Returns a list of {name, key, label, kind, value, text, formula, error}: name is what formulas call it (nil when they cannot), value in mm or degrees

`pc.doc.set_formula`: Set one of a feature's numbers by a formula, or take the formula away.

- `id` (id): The feature
- `parameter` (string): Its name or key, as doc.parameters lists them
- `formula` (string, optional): Such as "Printer.wall * 2"; nil takes it away
- Returns {value, text, error}: what it comes to

`pc.doc.set_value`: Set one of a feature's numbers, taking away any formula on it.

- `id` (id): The feature
- `parameter` (string): Its name or key, as doc.parameters lists them
- `value` (number): In millimetres or degrees

### var

`pc.var.new`: Make a variable set.

- `name` (string): What formulas call it: Printer.nozzle
- Returns the set's id

`pc.var.set`: Set a variable to a formula, adding it when new.

- `set` (string): The set, by name or id
- `name` (string)
- `formula` (string): Such as "0.4 mm" or "3 * Printer.nozzle"
- `comment` (string, optional)
- Returns {value, text, error}: what it comes to

`pc.var.remove`: Take a variable out of its set.

- `set` (string): The set, by name or id
- `name` (string)

`pc.var.rename`: Rename a variable, and every formula that reads it.

- `set` (string): The set, by name or id
- `name` (string)
- `to` (string)

`pc.var.list`: The variable sets and what each variable comes to.

- `set` (string, optional): Only this set, by name or id
- Returns a list of {id, name, variables}, each variable {name, formula, value, text, kind, error, comment}

`pc.var.eval`: What a formula comes to in this document.

- `formula` (string)
- Returns {value, kind, text}: value in mm or degrees

### config

`pc.config.list`: The configurations: the variables they set, each row, and which is in effect.

- Returns {columns, rows: [{name, values}], active}

`pc.config.new`: Add a configuration.

- `name` (string): Such as "Large"
- `like` (string, optional): Start from this configuration's values

`pc.config.remove`: Remove a configuration.

- `name` (string)

`pc.config.rename`: Rename a configuration.

- `name` (string)
- `to` (string)

`pc.config.add_variable`: Let the configurations set a variable: it becomes a column.

- `variable` (string): As formulas read it: Size.width

`pc.config.remove_variable`: Take a variable's column away.

- `variable` (string): As formulas read it: Size.width

`pc.config.set`: What a configuration gives a variable: a formula, or empty for its own.

- `name` (string): The configuration
- `variable` (string): As formulas read it: Size.width
- `value` (string): Such as "60 mm"

`pc.config.leave_out`: The bodies a configuration leaves out: not drawn, picked, exported or checked.

- `name` (string): The configuration
- `bodies` (list): The bodies' ids; an empty list leaves none out

`pc.config.activate`: Put a configuration in effect.

- `name` (string, optional): Nil leaves every variable its own

### app

`pc.app.workbenches`: The workbenches, in the order they load.

- Returns a list of {id, label, active}

`pc.app.workbench`: Switch to a workbench.

- `id` (string): As app.workbenches lists it

`pc.app.tool`: Start a toolbar tool, as a click on it does.

- `id` (string): The tool's id, as Preferences › Keyboard lists it

`pc.app.tools`: The tools of a workbench.

- `workbench` (string, optional): The active one when left out
- Returns a list of {id, label, keys}

`pc.app.quit`: Quit.

`pc.app.log`: Log panel.

### file

`pc.file.new`: New.

`pc.file.open`: Open.

- `path` (string, optional): The document to open; the dialog when left out
- Returns nothing; the document opens in its tab after the script

`pc.file.save`: Save.

`pc.file.save_as`: Save as.

- `path` (string, optional): Where to save; the dialog when left out

`pc.file.import`: Import.

- `path` (string, optional): The STEP, IGES, STL, OBJ, 3MF, PLY, glTF or VRML file; the dialog when left out
- Returns nothing; pc.doc.rebuild() waits for the import

`pc.file.export`: Export.

- `path` (string, optional): Where to write; the dialog when left out
- `format` (string, optional): step, step_nurbs (every surface a spline), stl or 3mf; from the path's extension when left out
- `bodies` (list, optional): The bodies to write; every visible one when left out
- `tolerance` (number, optional): The mesh formats' distance to the true surface, mm (0.01)
- Returns {path, written, skipped, triangles}

`pc.file.send_to_slicer`: Send to slicer.

### edit

`pc.edit.undo`: Undo.

`pc.edit.redo`: Redo.

`pc.edit.cut`: Cut.

`pc.edit.copy`: Copy.

`pc.edit.paste`: Paste.

`pc.edit.rename`: Rename the selected item.

`pc.edit.properties`: Properties of the selected item.

`pc.edit.recompute`: Recompute all.

### view

`pc.view.fit_all`: Fit all.

`pc.view.fit_selection`: Fit selection.

`pc.view.isolate`: Show only the selected body.

`pc.view.show_all`: Show every body.

`pc.view.isometric`: Isometric view.

`pc.view.front`: Front view.

`pc.view.top`: Top view.

`pc.view.right`: Right view.

`pc.view.rear`: Rear view.

`pc.view.bottom`: Bottom view.

`pc.view.left`: Left view.

`pc.view.orthographic`: Orthographic.

`pc.view.perspective`: Perspective.

`pc.view.shaded_edges`: Shaded with edges.

`pc.view.shaded`: Shaded.

`pc.view.wireframe`: Wireframe.

`pc.view.clipping_plane`: Clipping plane.

`pc.view.measure`: Measure.

`pc.view.print_bed`: Print bed.

`pc.view.annotations`: Annotations.

### tab

`pc.tab.new`: New tab.

`pc.tab.close`: Close tab.

`pc.tab.next`: Next tab.

`pc.tab.previous`: Previous tab.

### sketch

`pc.sketch.new`: Make an empty sketch on a base plane.

- `body` (id, optional): The body it belongs to; the selected body, else a new one
- `plane` (string, optional): XY (the default), XZ or YZ
- `offset` (number, optional): How far along the plane's normal it sits
- `name` (string, optional): Its name in the tree
- `attachment` (any, optional): Attached as a datum plane is, the attachment as a datum keeps it (doc.feature on a datum shows it): {Face = {face = {point = {x, y, z}, normal = {x, y, z}}}}, {ThreePoints = {points = {{At = {point = {x, y, z}}}, ...}}} and the like, faces and edges on the body's own solid; the sketch follows what it stands on. design.datum makes the same from plainer arguments, and `on` takes that datum
- `attachment_offset` (any, optional): The attachment's offset, as a datum's
- `on` (id, optional): A datum plane, or a coordinate system whose XY, XZ or YZ plane (see plane) it takes
- `normal` (list, optional): A plane of its own instead: its normal as {x, y, z}
- `origin` (list, optional): With normal: where the plane's origin sits, {x, y, z}
- `x_axis` (list, optional): With normal: the sketch's X direction, {x, y, z}
- Returns the sketch's id

`pc.sketch.import_dxf`: Make a sketch of a DXF drawing: its lines, arcs, circles, ellipses and polylines as sketch curves, splines as lines through points on them, hidden ones as construction, ends that meet sharing one point.

- `body` (id, optional): The body it belongs to; the selected body, else a new one
- `plane` (string, optional): XY (the default), XZ or YZ
- `offset` (number, optional): How far along the plane's normal it sits
- `name` (string, optional): Its name in the tree
- `attachment` (any, optional): Attached as a datum plane is, the attachment as a datum keeps it (doc.feature on a datum shows it): {Face = {face = {point = {x, y, z}, normal = {x, y, z}}}}, {ThreePoints = {points = {{At = {point = {x, y, z}}}, ...}}} and the like, faces and edges on the body's own solid; the sketch follows what it stands on. design.datum makes the same from plainer arguments, and `on` takes that datum
- `attachment_offset` (any, optional): The attachment's offset, as a datum's
- `on` (id, optional): A datum plane, or a coordinate system whose XY, XZ or YZ plane (see plane) it takes
- `normal` (list, optional): A plane of its own instead: its normal as {x, y, z}
- `origin` (list, optional): With normal: where the plane's origin sits, {x, y, z}
- `x_axis` (list, optional): With normal: the sketch's X direction, {x, y, z}
- `path` (string): The DXF file
- `scale` (number, optional): Millimetres per drawing unit; the drawing's own unit when left out, else 1
- Returns the sketch's id

`pc.sketch.point`: Add a point.

- `sketch` (id): The sketch to draw in
- `x` (number)
- `y` (number)
- Returns the point's id

`pc.sketch.line`: Add a line from (x1, y1) to (x2, y2).

- `sketch` (id): The sketch to draw in
- `x1` (number)
- `y1` (number)
- `x2` (number)
- `y2` (number)
- Returns the line's id

`pc.sketch.polyline`: Add lines through a list of points, each ending where the next starts; a level or upright one is held so.

- `sketch` (id): The sketch to draw in
- `points` (list): Points as {x, y} pairs
- `closed` (boolean, optional): Join the last point to the first
- Returns the lines' ids

`pc.sketch.rect`: Add a rectangle from its corner (x, y), its width and its height, its sides held level and upright.

- `sketch` (id): The sketch to draw in
- `x` (number)
- `y` (number)
- `width` (number)
- `height` (number)
- Returns the four lines' ids

`pc.sketch.circle`: Add a circle.

- `sketch` (id): The sketch to draw in
- `x` (number): The centre
- `y` (number): The centre
- `radius` (number)
- Returns the circle's id

`pc.sketch.arc`: Add an arc, counter-clockwise from the start angle to the end angle.

- `sketch` (id): The sketch to draw in
- `x` (number): The centre
- `y` (number): The centre
- `radius` (number)
- `start` (number): Degrees from the sketch's X axis
- `end` (number): Degrees from the sketch's X axis
- Returns the arc's id

`pc.sketch.geometry`: List the sketch's elements with their points.

- `sketch` (id): The sketch to draw in
- Returns a list of {id, kind, points, radius?, construction}

`pc.sketch.constrain`: Constrain elements, as the constraint's toolbar button does for a selection.

- `sketch` (id): The sketch to draw in
- `kind` (string): coincident, point_on_object, midpoint, horizontal, vertical, horizontal_vertical, parallel, perpendicular, tangent, equal, symmetric, block, lock, dimension, distance, distance_x, distance_y, gap, arc_length, radius, diameter, radius_diameter, angle, angle_x, angle_y, angle_at_point, arc_angle, angle_three_points (items: arm, corner, arm), ellipse_minor or refraction; radius on an ellipse is its major radius, arc_length on a spline or conic its length
- `items` (list): Element ids, or "origin", "x_axis" and "y_axis"
- `value` (number, optional): A dimension's value (mm, degrees for an angle, the ratio of indices for a refraction); the measured one when left out
- `remove_redundant` (boolean, optional): Take away the older constraints the new ones make redundant
- Returns the new constraints' ids

`pc.sketch.set_value`: Change a dimension's value.

- `sketch` (id): The sketch to draw in
- `constraint` (id)
- `value` (number): mm, or degrees for an angle
- `driving` (boolean, optional): false makes it a reference dimension that only measures

`pc.sketch.draw`: Run a drawing or editing tool over points of the sketch, as clicks there would.

- `sketch` (id): The sketch to draw in
- `tool` (string): line, polyline, rect, rect_center, rect_rounded, rect3, rect_center3, rect_frame, circle, circle3, arc, arc3, ellipse, ellipse3, ellipse_arc, parabola, hyperbola, bspline, polygon, slot, arc_slot, point, fillet, chamfer, trim, extend, split, bspline_knot, offset, translate, rotate, scale or mirror
- `points` (list): The clicks, each {x, y}, or {x = , y = , typed = {length = 20}, constrain = true} with values typed at it; "arc" and "line" switch a polyline, "finish" ends a spline
- `tolerance` (number, optional): How close a click snaps onto points and curves, mm (0.001)
- `params` (any, optional): Tool settings: polygon_sides, slot_width, fillet_radius, chamfer_length, corner_keep, offset_distance, offset_round, offset_both, offset_delete, offset_linked, copies, copies_linked, bspline_periodic, bspline_degree, bspline_interpolate, auto_constraints, mirror_keep, mirror_linked, mirror_center
- `construction` (boolean, optional): What it makes is construction geometry
- `avoid_redundant` (boolean, optional): Drop auto constraints that add nothing (true)
- `selection` (list, optional): The elements offset, translate, rotate, scale and mirror act on
- Returns {elements, constraints}: what it made

`pc.sketch.drag`: Drag elements by a step, the rest of the sketch following its constraints.

- `sketch` (id): The sketch to draw in
- `items` (list): The elements to drag
- `by` (list): The step, {x, y}

`pc.sketch.attachment`: Move a sketch on the datum it is attached to: along its normal, across it, turned about it.

- `offset` (number, optional): Along the normal, mm
- `shift` (list, optional): Across the plane, {x, y} in mm
- `turn` (number, optional): About the normal, degrees
- `sketch` (id): The sketch to draw in

`pc.sketch.external_from`: Bring another sketch's curves and points, or a datum, into this sketch as external geometry that follows them.

- `from` (id): A sketch or a datum
- `sketch` (id): The sketch to draw in
- Returns the external elements made

`pc.sketch.external_defining`: Count external geometry in the sketch's profiles, or leave it only guiding.

- `items` (list): External elements' ids
- `on` (boolean, optional): true counts them (the default), false stops
- `sketch` (id): The sketch to draw in

`pc.sketch.solver_settings`: How far the solver goes on this sketch.

- `iterations` (number, optional): The most steps it takes (100 when never set)
- `tolerance` (number, optional): How small what is left must be, against the sketch's size (1e-9 when never set)
- `sketch` (id): The sketch to draw in

`pc.sketch.repair`: Join ends of curves that nearly meet, and remove curves of no size, doubled curves and constraints left naming nothing.

- `tolerance` (number, optional): How near two ends must be to join, mm (0.01 when left out)
- `sketch` (id): The sketch to draw in
- Returns what was repaired, in words

`pc.sketch.restore`: Put the sketch back as `data` holds it: an editing session cancelled.

- `data` (any): The sketch as doc.feature lists its data
- `sketch` (id): The sketch to draw in

`pc.sketch.set_plane`: Move the sketch onto another plane, its geometry kept in its own coordinates.

- `sketch` (id): The sketch to draw in
- `normal` (list): The plane's normal, {x, y, z}
- `origin` (list, optional): Its origin, {x, y, z}
- `x_axis` (list, optional): The sketch's X direction, {x, y, z}

`pc.sketch.array`: Repeat elements in rows and columns.

- `sketch` (id): The sketch to draw in
- `items` (list): The elements to repeat
- `rows` (integer)
- `cols` (integer)
- `dx` (number): The step between columns, mm
- `dy` (number): The step between rows, mm
- `linked` (boolean, optional): Copies stay the originals' size, spaced by one pitch along the rows and one down the columns (false)
- Returns {elements}: what it made

`pc.sketch.text`: Lay out text as closed outlines standing on a new point: the start of its first line on the baseline.

- `sketch` (id): The sketch to draw in
- `text` (string): What it says; a new line starts a line
- `at` (list): Where its point goes, {x, y}
- `font` (string, optional): IBM Plex Sans, IBM Plex Sans SemiBold, IBM Plex Mono, or a font file's path (IBM Plex Sans)
- `size` (number, optional): The font's em, mm (10)
- `spacing` (number, optional): Added between letters, mm (0)
- `angle` (number, optional): Degrees it turns about its point (0)
- Returns {text, point}: the block and the point it stands on

`pc.sketch.text_edit`: Change a text block, its outlines made again where its point stands.

- `sketch` (id): The sketch to draw in
- `block` (id): The text block, or its point
- `text` (string, optional)
- `font` (string, optional)
- `size` (number, optional): mm
- `spacing` (number, optional): mm
- `angle` (number, optional): degrees

`pc.sketch.to_bspline`: Make lines, arcs, circles, ellipses and conics into splines that are exactly them.

- `sketch` (id): The sketch to draw in
- `items` (list): The curves to make splines of
- Returns {elements}: what it made

`pc.sketch.spline_degree`: Raise or lower the degree of splines: raising keeps the curve, lowering fits the nearest one.

- `sketch` (id): The sketch to draw in
- `items` (list): The splines
- `by` (integer): 1 to raise, -1 to lower

`pc.sketch.insert_knot`: Insert a knot into a spline where it passes nearest a point, the curve unchanged.

- `sketch` (id): The sketch to draw in
- `spline` (id): The spline
- `at` (list): A point near the curve, {x, y}

`pc.sketch.knot_multiplicity`: Set how many times a spline's knot stands (1 up to the degree), or remove it with 0.

- `sketch` (id): The sketch to draw in
- `spline` (id): The spline
- `knot` (number): The knot's value, as sketch.spline_knots lists it
- `multiplicity` (integer)

`pc.sketch.spline_knots`: A spline's knots inside its ends and how many times each stands.

- `sketch` (id): The sketch to draw in
- `spline` (id): The spline
- Returns {{knot, multiplicity}}

`pc.sketch.spline_weight`: Weigh a spline's control point: more pulls the curve toward it.

- `sketch` (id): The sketch to draw in
- `spline` (id): The spline
- `point` (id): One of its control points
- `weight` (number): More than 0; 1 is plain

`pc.sketch.join`: Merge curves that meet end to end into one B-spline following them.

- `sketch` (id): The sketch to draw in
- `items` (list): The lines, arcs, arcs of ellipses, parabolas and hyperbolas, and open splines to merge
- `tolerance` (number, optional): How far the spline may stray from the curves, mm (0.01)
- Returns {elements}: what it made

`pc.sketch.set_constraint`: Make constraints driving or reference, active or not, parked or not.

- `sketch` (id): The sketch to draw in
- `items` (list): The constraints
- `driving` (boolean, optional): false: a reference dimension that only measures
- `active` (boolean, optional): false: kept but not solved
- `parked` (boolean, optional): true: its symbol moves to the parked layer, drawn only while that layer shows; it still solves

`pc.sketch.mirror_sketch`: A new sketch on the same plane: this one's geometry mirrored across its Y axis.

- `sketch` (id): The sketch to draw in
- Returns the new sketch's id

`pc.sketch.merge`: A new sketch holding this one's geometry and other sketches', mapped onto its plane.

- `sketch` (id): The sketch to draw in
- `with` (list): The other sketches
- Returns the new sketch's id

`pc.sketch.carbon_copy`: Copy another sketch's geometry into this one, mapped onto its plane.

- `sketch` (id): The sketch to draw in
- `from` (id): The sketch to copy
- Returns {elements, constraints}: what it made

`pc.sketch.paste`: Add geometry held as a sketch of its own, moved by a step.

- `sketch` (id): The sketch to draw in
- `clipboard` (any): The geometry, as a sketch's fields (what copying in the sketcher holds)
- `by` (list): The step, {x, y}
- Returns {elements}: what it made

`pc.sketch.external`: Project edges of solids into the sketch as fixed references.

- `sketch` (id): The sketch to draw in
- `edges` (list): Each {body, point, direction}: a point on the edge and its direction, in the body's own frame
- Returns {elements}: what it made

`pc.sketch.intersection`: Add where faces of solids cross the sketch plane, as fixed references.

- `sketch` (id): The sketch to draw in
- `faces` (list): Each {body, point, normal}: a point on the face and its normal there, in the body's own frame
- Returns {elements}: what it made

`pc.sketch.constraints`: List the sketch's constraints.

- `sketch` (id): The sketch to draw in
- Returns a list of {id, kind, items, value?}

`pc.sketch.wall_thickness`: How thin the sketch's closed profile gets, for printing.

- `sketch` (id): The sketch to draw in
- `minimum` (number, optional): The thinnest wall that prints, mm; the Sketcher preference when left out
- Returns {thinnest, where = {x, y}, minimum, thin, regions}: the thinnest wall in mm, where it is, whether it is under the minimum, and each region's own

`pc.sketch.status`: How constrained the sketch is, and what conflicts.

- `sketch` (id): The sketch to draw in
- Returns {dof, solved, redundant, conflicting}

`pc.sketch.delete`: Delete elements or constraints, and what depends on them.

- `sketch` (id): The sketch to draw in
- `items` (list): Element or constraint ids

`pc.sketch.construction`: Make elements construction geometry, or normal again.

- `sketch` (id): The sketch to draw in
- `items` (list): Element ids
- `on` (boolean, optional): true (the default) or false

`pc.sketch.internal_geometry`: Show or hide curves' internal geometry: an ellipse's axes and foci, a parabola's or hyperbola's axis and focus, a B-spline's control polygon, as construction held to its curve.

- `sketch` (id): The sketch to draw in
- `items` (list): The curves, or pieces of their internal geometry
- `show` (boolean, optional): true makes what is missing, false takes away the pieces nothing else holds; left out, it shows when a piece is missing and hides otherwise
- Returns {shown, elements}: whether it showed, and what it made or took away

`pc.sketch.section_view`: Cut away everything on the viewer's side of the sketch plane while it is edited.

- `sketch` (id): The sketch to draw in
- `on` (boolean, optional): true (the default) or false

`pc.sketch.remove_axis_alignment`: Turn the horizontal and vertical constraints of lines into parallel and perpendicular ones among them, so the group keeps its shape and turns as a whole.

- `sketch` (id): The sketch to draw in
- `items` (list): The lines
- Returns how many constraints changed

`pc.sketch.generator`: Change the numbers a generated sketch (a gear, a sprocket, a shaft) is made from, or detach it into a plain sketch.

- `sketch` (id): The generated sketch
- `detach` (boolean, optional): Keep the curves as they are and forget the numbers: a plain sketch to edit by hand
- Other arguments: The numbers to change, such as teeth = 24 or module = 1.5; a shaft takes sections = {{length = 20, diameter = 10, chamfer = 0.5, fillet = 0}, ...}
- Returns what the numbers come to: its diameters, or its length

### design

`pc.design.pad`: Pad a sketch.

- `sketch` (id, optional): The sketch it uses
- `body` (id, optional): The body it goes in; the sketch's body when left out
- `name` (string, optional): Its name in the tree
- `face_point` (list, optional): A face it takes as the viewport's picked face (a thickness's opening, a draft's neutral plane, a mirror's plane, the profile of a pad or a pocket given no sketch): a point of it, {x, y, z}, in the body's own frame
- `face_normal` (list, optional): With face_point: the face's outward normal, {x, y, z}
- Other arguments: Any field of the feature, such as length = 20 or reversed = true
- Returns the feature's id

`pc.design.pocket`: Cut a sketch into the body.

- `sketch` (id, optional): The sketch it uses
- `body` (id, optional): The body it goes in; the sketch's body when left out
- `name` (string, optional): Its name in the tree
- `face_point` (list, optional): A face it takes as the viewport's picked face (a thickness's opening, a draft's neutral plane, a mirror's plane, the profile of a pad or a pocket given no sketch): a point of it, {x, y, z}, in the body's own frame
- `face_normal` (list, optional): With face_point: the face's outward normal, {x, y, z}
- Other arguments: Any field of the feature, such as length = 20 or reversed = true
- Returns the feature's id

`pc.design.revolve`: Turn a sketch about an axis.

- `sketch` (id, optional): The sketch it uses
- `body` (id, optional): The body it goes in; the sketch's body when left out
- `name` (string, optional): Its name in the tree
- `face_point` (list, optional): A face it takes as the viewport's picked face (a thickness's opening, a draft's neutral plane, a mirror's plane, the profile of a pad or a pocket given no sketch): a point of it, {x, y, z}, in the body's own frame
- `face_normal` (list, optional): With face_point: the face's outward normal, {x, y, z}
- Other arguments: Any field of the feature, such as length = 20 or reversed = true
- Returns the feature's id

`pc.design.groove`: Cut a sketch turned about an axis.

- `sketch` (id, optional): The sketch it uses
- `body` (id, optional): The body it goes in; the sketch's body when left out
- `name` (string, optional): Its name in the tree
- `face_point` (list, optional): A face it takes as the viewport's picked face (a thickness's opening, a draft's neutral plane, a mirror's plane, the profile of a pad or a pocket given no sketch): a point of it, {x, y, z}, in the body's own frame
- `face_normal` (list, optional): With face_point: the face's outward normal, {x, y, z}
- Other arguments: Any field of the feature, such as length = 20 or reversed = true
- Returns the feature's id

`pc.design.loft`: Loft through sketches.

- `sketch` (id, optional): The sketch it uses
- `body` (id, optional): The body it goes in; the sketch's body when left out
- `name` (string, optional): Its name in the tree
- `face_point` (list, optional): A face it takes as the viewport's picked face (a thickness's opening, a draft's neutral plane, a mirror's plane, the profile of a pad or a pocket given no sketch): a point of it, {x, y, z}, in the body's own frame
- `face_normal` (list, optional): With face_point: the face's outward normal, {x, y, z}
- Other arguments: Any field of the feature, such as length = 20 or reversed = true
- Returns the feature's id

`pc.design.subtractive_loft`: Cut a loft through sketches.

- `sketch` (id, optional): The sketch it uses
- `body` (id, optional): The body it goes in; the sketch's body when left out
- `name` (string, optional): Its name in the tree
- `face_point` (list, optional): A face it takes as the viewport's picked face (a thickness's opening, a draft's neutral plane, a mirror's plane, the profile of a pad or a pocket given no sketch): a point of it, {x, y, z}, in the body's own frame
- `face_normal` (list, optional): With face_point: the face's outward normal, {x, y, z}
- Other arguments: Any field of the feature, such as length = 20 or reversed = true
- Returns the feature's id

`pc.design.pipe`: Sweep a sketch along a path.

- `sketch` (id, optional): The sketch it uses
- `body` (id, optional): The body it goes in; the sketch's body when left out
- `name` (string, optional): Its name in the tree
- `face_point` (list, optional): A face it takes as the viewport's picked face (a thickness's opening, a draft's neutral plane, a mirror's plane, the profile of a pad or a pocket given no sketch): a point of it, {x, y, z}, in the body's own frame
- `face_normal` (list, optional): With face_point: the face's outward normal, {x, y, z}
- Other arguments: Any field of the feature, such as length = 20 or reversed = true
- Returns the feature's id

`pc.design.subtractive_pipe`: Cut a sketch swept along a path.

- `sketch` (id, optional): The sketch it uses
- `body` (id, optional): The body it goes in; the sketch's body when left out
- `name` (string, optional): Its name in the tree
- `face_point` (list, optional): A face it takes as the viewport's picked face (a thickness's opening, a draft's neutral plane, a mirror's plane, the profile of a pad or a pocket given no sketch): a point of it, {x, y, z}, in the body's own frame
- `face_normal` (list, optional): With face_point: the face's outward normal, {x, y, z}
- Other arguments: Any field of the feature, such as length = 20 or reversed = true
- Returns the feature's id

`pc.design.helix`: Sweep a sketch along a helix.

- `sketch` (id, optional): The sketch it uses
- `body` (id, optional): The body it goes in; the sketch's body when left out
- `name` (string, optional): Its name in the tree
- `face_point` (list, optional): A face it takes as the viewport's picked face (a thickness's opening, a draft's neutral plane, a mirror's plane, the profile of a pad or a pocket given no sketch): a point of it, {x, y, z}, in the body's own frame
- `face_normal` (list, optional): With face_point: the face's outward normal, {x, y, z}
- Other arguments: Any field of the feature, such as length = 20 or reversed = true
- Returns the feature's id

`pc.design.subtractive_helix`: Cut a sketch swept along a helix.

- `sketch` (id, optional): The sketch it uses
- `body` (id, optional): The body it goes in; the sketch's body when left out
- `name` (string, optional): Its name in the tree
- `face_point` (list, optional): A face it takes as the viewport's picked face (a thickness's opening, a draft's neutral plane, a mirror's plane, the profile of a pad or a pocket given no sketch): a point of it, {x, y, z}, in the body's own frame
- `face_normal` (list, optional): With face_point: the face's outward normal, {x, y, z}
- Other arguments: Any field of the feature, such as length = 20 or reversed = true
- Returns the feature's id

`pc.design.primitive`: Add a box, cylinder, sphere, cone, torus or wedge.

- `sketch` (id, optional): The sketch it uses
- `body` (id, optional): The body it goes in; the sketch's body when left out
- `name` (string, optional): Its name in the tree
- `face_point` (list, optional): A face it takes as the viewport's picked face (a thickness's opening, a draft's neutral plane, a mirror's plane, the profile of a pad or a pocket given no sketch): a point of it, {x, y, z}, in the body's own frame
- `face_normal` (list, optional): With face_point: the face's outward normal, {x, y, z}
- `variant` (string, optional): box (the default), cylinder, sphere, cone, torus or wedge
- Other arguments: Any field of the feature, such as length = 20 or reversed = true
- Returns the feature's id

`pc.design.subtractive_primitive`: Cut a box, cylinder, sphere, cone, torus or wedge.

- `sketch` (id, optional): The sketch it uses
- `body` (id, optional): The body it goes in; the sketch's body when left out
- `name` (string, optional): Its name in the tree
- `face_point` (list, optional): A face it takes as the viewport's picked face (a thickness's opening, a draft's neutral plane, a mirror's plane, the profile of a pad or a pocket given no sketch): a point of it, {x, y, z}, in the body's own frame
- `face_normal` (list, optional): With face_point: the face's outward normal, {x, y, z}
- `variant` (string, optional): box (the default), cylinder, sphere, cone, torus or wedge
- Other arguments: Any field of the feature, such as length = 20 or reversed = true
- Returns the feature's id

`pc.design.hole`: Drill holes at a sketch's circles and points.

- `sketch` (id, optional): The sketch it uses
- `body` (id, optional): The body it goes in; the sketch's body when left out
- `name` (string, optional): Its name in the tree
- `face_point` (list, optional): A face it takes as the viewport's picked face (a thickness's opening, a draft's neutral plane, a mirror's plane, the profile of a pad or a pocket given no sketch): a point of it, {x, y, z}, in the body's own frame
- `face_normal` (list, optional): With face_point: the face's outward normal, {x, y, z}
- Other arguments: Any field of the feature, such as length = 20 or reversed = true
- Returns the feature's id

`pc.design.fillet`: Round edges.

- `sketch` (id, optional): The sketch it uses
- `body` (id, optional): The body it goes in; the sketch's body when left out
- `name` (string, optional): Its name in the tree
- `face_point` (list, optional): A face it takes as the viewport's picked face (a thickness's opening, a draft's neutral plane, a mirror's plane, the profile of a pad or a pocket given no sketch): a point of it, {x, y, z}, in the body's own frame
- `face_normal` (list, optional): With face_point: the face's outward normal, {x, y, z}
- Other arguments: Any field of the feature, such as length = 20 or reversed = true
- Returns the feature's id

`pc.design.chamfer`: Bevel edges.

- `sketch` (id, optional): The sketch it uses
- `body` (id, optional): The body it goes in; the sketch's body when left out
- `name` (string, optional): Its name in the tree
- `face_point` (list, optional): A face it takes as the viewport's picked face (a thickness's opening, a draft's neutral plane, a mirror's plane, the profile of a pad or a pocket given no sketch): a point of it, {x, y, z}, in the body's own frame
- `face_normal` (list, optional): With face_point: the face's outward normal, {x, y, z}
- Other arguments: Any field of the feature, such as length = 20 or reversed = true
- Returns the feature's id

`pc.design.draft`: Tilt faces.

- `sketch` (id, optional): The sketch it uses
- `body` (id, optional): The body it goes in; the sketch's body when left out
- `name` (string, optional): Its name in the tree
- `face_point` (list, optional): A face it takes as the viewport's picked face (a thickness's opening, a draft's neutral plane, a mirror's plane, the profile of a pad or a pocket given no sketch): a point of it, {x, y, z}, in the body's own frame
- `face_normal` (list, optional): With face_point: the face's outward normal, {x, y, z}
- Other arguments: Any field of the feature, such as length = 20 or reversed = true
- Returns the feature's id

`pc.design.thickness`: Hollow the solid.

- `sketch` (id, optional): The sketch it uses
- `body` (id, optional): The body it goes in; the sketch's body when left out
- `name` (string, optional): Its name in the tree
- `face_point` (list, optional): A face it takes as the viewport's picked face (a thickness's opening, a draft's neutral plane, a mirror's plane, the profile of a pad or a pocket given no sketch): a point of it, {x, y, z}, in the body's own frame
- `face_normal` (list, optional): With face_point: the face's outward normal, {x, y, z}
- Other arguments: Any field of the feature, such as length = 20 or reversed = true
- Returns the feature's id

`pc.design.delete_faces`: Delete faces and close the openings from their neighbours.

- `sketch` (id, optional): The sketch it uses
- `body` (id, optional): The body it goes in; the sketch's body when left out
- `name` (string, optional): Its name in the tree
- `face_point` (list, optional): A face it takes as the viewport's picked face (a thickness's opening, a draft's neutral plane, a mirror's plane, the profile of a pad or a pocket given no sketch): a point of it, {x, y, z}, in the body's own frame
- `face_normal` (list, optional): With face_point: the face's outward normal, {x, y, z}
- Other arguments: Any field of the feature, such as length = 20 or reversed = true
- Returns the feature's id

`pc.design.offset_faces`: Push or pull faces along their normals, their neighbours following.

- `sketch` (id, optional): The sketch it uses
- `body` (id, optional): The body it goes in; the sketch's body when left out
- `name` (string, optional): Its name in the tree
- `face_point` (list, optional): A face it takes as the viewport's picked face (a thickness's opening, a draft's neutral plane, a mirror's plane, the profile of a pad or a pocket given no sketch): a point of it, {x, y, z}, in the body's own frame
- `face_normal` (list, optional): With face_point: the face's outward normal, {x, y, z}
- Other arguments: Any field of the feature, such as length = 20 or reversed = true
- Returns the feature's id

`pc.design.move_faces`: Move or turn faces, their neighbours following.

- `sketch` (id, optional): The sketch it uses
- `body` (id, optional): The body it goes in; the sketch's body when left out
- `name` (string, optional): Its name in the tree
- `face_point` (list, optional): A face it takes as the viewport's picked face (a thickness's opening, a draft's neutral plane, a mirror's plane, the profile of a pad or a pocket given no sketch): a point of it, {x, y, z}, in the body's own frame
- `face_normal` (list, optional): With face_point: the face's outward normal, {x, y, z}
- Other arguments: Any field of the feature, such as length = 20 or reversed = true
- Returns the feature's id

`pc.design.mirror`: Mirror the last feature.

- `sketch` (id, optional): The sketch it uses
- `body` (id, optional): The body it goes in; the sketch's body when left out
- `name` (string, optional): Its name in the tree
- `face_point` (list, optional): A face it takes as the viewport's picked face (a thickness's opening, a draft's neutral plane, a mirror's plane, the profile of a pad or a pocket given no sketch): a point of it, {x, y, z}, in the body's own frame
- `face_normal` (list, optional): With face_point: the face's outward normal, {x, y, z}
- Other arguments: Any field of the feature, such as length = 20 or reversed = true
- Returns the feature's id

`pc.design.linear_pattern`: Repeat the last feature along a line.

- `sketch` (id, optional): The sketch it uses
- `body` (id, optional): The body it goes in; the sketch's body when left out
- `name` (string, optional): Its name in the tree
- `face_point` (list, optional): A face it takes as the viewport's picked face (a thickness's opening, a draft's neutral plane, a mirror's plane, the profile of a pad or a pocket given no sketch): a point of it, {x, y, z}, in the body's own frame
- `face_normal` (list, optional): With face_point: the face's outward normal, {x, y, z}
- Other arguments: Any field of the feature, such as length = 20 or reversed = true
- Returns the feature's id

`pc.design.polar_pattern`: Repeat the last feature about an axis.

- `sketch` (id, optional): The sketch it uses
- `body` (id, optional): The body it goes in; the sketch's body when left out
- `name` (string, optional): Its name in the tree
- `face_point` (list, optional): A face it takes as the viewport's picked face (a thickness's opening, a draft's neutral plane, a mirror's plane, the profile of a pad or a pocket given no sketch): a point of it, {x, y, z}, in the body's own frame
- `face_normal` (list, optional): With face_point: the face's outward normal, {x, y, z}
- Other arguments: Any field of the feature, such as length = 20 or reversed = true
- Returns the feature's id

`pc.design.scaled`: Scale the last feature.

- `sketch` (id, optional): The sketch it uses
- `body` (id, optional): The body it goes in; the sketch's body when left out
- `name` (string, optional): Its name in the tree
- `face_point` (list, optional): A face it takes as the viewport's picked face (a thickness's opening, a draft's neutral plane, a mirror's plane, the profile of a pad or a pocket given no sketch): a point of it, {x, y, z}, in the body's own frame
- `face_normal` (list, optional): With face_point: the face's outward normal, {x, y, z}
- Other arguments: Any field of the feature, such as length = 20 or reversed = true
- Returns the feature's id

`pc.design.boolean`: Combine with another body.

- `sketch` (id, optional): The sketch it uses
- `body` (id, optional): The body it goes in; the sketch's body when left out
- `name` (string, optional): Its name in the tree
- `face_point` (list, optional): A face it takes as the viewport's picked face (a thickness's opening, a draft's neutral plane, a mirror's plane, the profile of a pad or a pocket given no sketch): a point of it, {x, y, z}, in the body's own frame
- `face_normal` (list, optional): With face_point: the face's outward normal, {x, y, z}
- Other arguments: Any field of the feature, such as length = 20 or reversed = true
- Returns the feature's id

`pc.design.set`: Change fields of a Design feature or a datum.

- `feature` (id): The feature to change
- Other arguments: The fields to change, such as length = 25; a datum takes offset {x, y, z}, rotation and flip as design.datum does

`pc.design.datum`: Add a datum plane, line, point or coordinate system.

- `kind` (string): plane, line, point or coordinate_system
- `body` (id): The body it belongs to
- `mode` (string, optional): What it attaches to: base_plane (the default), face, three_points, normal_to_edge, along_edge, two_points, plane_intersection, curve_centre or inertia; references are in the body's own frame and follow its solid
- `plane` (string, optional): base_plane: the base plane it sits on, XY (the default), XZ or YZ
- `face_point` (list, optional): face: a point of the face, {x, y, z}; without a mode, a flat face kept as given
- `face_normal` (list, optional): With face_point: the face's outward normal, {x, y, z}
- `edge_point` (list, optional): normal_to_edge, along_edge, curve_centre: a point of the edge, {x, y, z}
- `edge_direction` (list, optional): With edge_point: the way the edge runs there, {x, y, z}
- `spot` (string, optional): normal_to_edge: where on the edge, picked (the default), start, end, middle or centre
- `points` (list, optional): three_points, two_points: each {x, y, z}, or {face_point, face_normal}, or {edge_point, edge_direction, spot}
- `planes` (list, optional): plane_intersection: two of XY, XZ, YZ, a datum's id, or {face_point, face_normal}
- `offset` (list, optional): Moved along its own x, y and normal, {x, y, z} in mm
- `rotation` (number, optional): Turned about its normal, degrees
- `flip` (boolean, optional): Turned to face the other way
- `tilt` (list, optional): Tilted about its own x-axis, then its y-axis, {x, y} in degrees
- `size` (number, optional): How large it draws, mm
- `name` (string, optional): Its name in the tree
- Returns the datum's id

`pc.design.borrow`: Borrow another body's sketch, or faces and edges of its solid.

- `body` (id): The body that borrows
- `sketch` (id, optional): A sketch of another body: its profile, for this body's features
- `from` (id, optional): Or the body whose solid lends faces and edges
- `faces` (list, optional): With from: faces it lends, each {point, normal} in that body's own frame
- `edges` (list, optional): With from: edges it lends, each {point, direction} in that body's own frame
- `frozen` (boolean, optional): Keep the geometry as it is now rather than follow the source
- `options` (any, optional): How it lends: {offset = {translation = {x, y, z}, rotation_deg, tilt = {x, y}, flip}} moves and turns it along and about this body's axes; fill = true makes closed borrowed edges a face features take as a profile; whole = true lends the whole solid's edges as reference
- `name` (string, optional): Its name in the tree
- Returns the borrow's id

`pc.design.recognize_holes`: Make the round holes of a body's solid Hole features: their faces deleted, and each set of alike holes drilled again from a sketch of their centres.

- `body` (id): The body
- Returns {holes, left, features}: the holes made features, the bores left as they are (counterbores, slots) and the features added

`pc.design.freeze`: Freeze borrowed geometry as it is now, or let it follow its source again.

- `feature` (id): The borrow
- `frozen` (boolean, optional): true (the default) takes the source as it is now; false follows it again

`pc.design.move_to_body`: Move a feature into another body's history, with the sketch and datums only it uses.

- `feature` (id): The feature
- `body` (id): The body it goes to, in at its tip
- Returns the ids of the features moved, the given one last

`pc.design.duplicate`: Make a copy of a feature, with its own copies of the sketches and datums it reads.

- `feature` (id): The feature
- `body` (id, optional): The body the copy goes in, at its tip (the feature's own when left out)
- Returns the ids of the features made, the copy of the given one last

`pc.design.centre_line`: Measure the centre line of a tube-like solid between two of its faces.

- `body` (id): The body whose solid it runs through
- `from_point` (list): A point of the face it starts at, {x, y, z}, in the body's own frame
- `from_normal` (list): That face's outward normal, {x, y, z}
- `to_point` (list): A point of the face it ends at, {x, y, z}
- `to_normal` (list): That face's outward normal, {x, y, z}
- `tolerance` (number, optional): How closely it follows the sections' centres, mm (0.02 when left out)
- Returns {length, points, deviation, straight}: its length in mm, points along it in the body's frame, the largest distance measured from a section's centre to it, and whether it is one straight segment

`pc.design.gear`: Make an involute spur gear's profile, outer or internal (ring): a sketch to pad.

- `body` (id, optional): The body it goes in; the selected one, else a new one
- `plane` (string, optional): The base plane it lies on: XY, XZ or YZ (a gear and a sprocket take XY, a shaft XZ)
- `face_point` (list, optional): Or a face it lies on, centred at this point of it, {x, y, z}, in the body's own frame
- `face_normal` (list, optional): With face_point: the face's outward normal, {x, y, z}
- `name` (string, optional): Its name in the tree
- Other arguments: module, teeth, pressure_angle_deg, profile_shift, addendum and dedendum (in modules), backlash, root_fillet (in modules), bore, internal (true for a ring), rim (a ring's outside diameter)
- Returns the sketch's id

`pc.design.sprocket`: Make a roller chain sprocket's profile (ISO 606 teeth): a sketch to pad.

- `body` (id, optional): The body it goes in; the selected one, else a new one
- `plane` (string, optional): The base plane it lies on: XY, XZ or YZ (a gear and a sprocket take XY, a shaft XZ)
- `face_point` (list, optional): Or a face it lies on, centred at this point of it, {x, y, z}, in the body's own frame
- `face_normal` (list, optional): With face_point: the face's outward normal, {x, y, z}
- `name` (string, optional): Its name in the tree
- Other arguments: pitch, roller (the roller's diameter), teeth, bore
- Returns the sketch's id

`pc.design.shaft`: Make a stepped shaft's half section: a sketch to revolve about its vertical axis.

- `body` (id, optional): The body it goes in; the selected one, else a new one
- `plane` (string, optional): The base plane it lies on: XY, XZ or YZ (a gear and a sprocket take XY, a shaft XZ)
- `face_point` (list, optional): Or a face it lies on, centred at this point of it, {x, y, z}, in the body's own frame
- `face_normal` (list, optional): With face_point: the face's outward normal, {x, y, z}
- `name` (string, optional): Its name in the tree
- Other arguments: sections = {{length, diameter, chamfer, fillet}, ...}, start_chamfer, and loads = {bearings = {a, b}, forces = {{at, force, angle_deg}, ...}, torque (N·m), torque_from, torque_to, modulus (GPa)} for its stresses and deflection
- Returns the sketch's id

### asm

`pc.asm.mate`: Put two flat faces against each other.

- `body` (id): The body that moves
- `face` (any): A flat face, {point, normal}, as pc.doc.faces lists it
- `other` (id): The body it is held against; the nil id (all zeros) for the world origin, its faces then in world space
- `other_face` (any): A flat face, {point, normal}, as pc.doc.faces lists it
- `name` (string, optional): Its name in the tree
- `offset` (number, optional): The gap between them, mm
- `flip` (boolean, optional): Face the same way instead of at each other
- Returns the joint's id

`pc.asm.align`: Put two round faces on one axis.

- `body` (id): The body that moves
- `face` (any): A round face, {axis = {point, direction}}, as pc.doc.faces lists it; an edge's line or circle axis goes the same way
- `other` (id): The body it is held against; the nil id (all zeros) for the world origin, its faces then in world space
- `other_face` (any): A round face, {axis = {point, direction}}, as pc.doc.faces lists it; an edge's line or circle axis goes the same way
- `name` (string, optional): Its name in the tree
- `turn_drive` (any, optional): An alignment's turn (degrees from where it was made) to hold it at; false lets it turn
- `turn_limits` (any, optional): {low, high}: the range an alignment's turn stays in; false takes it away
- `slide_drive` (any, optional): How far along the axis (mm) to hold an alignment; false lets it slide
- `slide_limits` (any, optional): {low, high}: the range an alignment's slide stays in, mm; false takes it away
- Returns the joint's id

`pc.asm.angle`: Hold two faces or axes at an angle.

- `body` (id): The body that moves
- `face` (any): A flat face {point, normal} or a round face or edge {axis = {point, direction}}
- `other` (id): The body it is held against; the nil id (all zeros) for the world origin, its faces then in world space
- `other_face` (any): A flat face {point, normal} or a round face or edge {axis = {point, direction}}
- `name` (string, optional): Its name in the tree
- `degrees` (number, optional): Between their outward normals; the angle they make now when left out
- Returns the joint's id

`pc.asm.hinge`: Put two axes on one line: the body can only turn about it.

- `body` (id): The body that moves
- `face` (any): A round face, {axis = {point, direction}}, as pc.doc.faces lists it; an edge's line or circle axis goes the same way
- `other` (id): The body it is held against; the nil id (all zeros) for the world origin, its faces then in world space
- `other_face` (any): A round face, {axis = {point, direction}}, as pc.doc.faces lists it; an edge's line or circle axis goes the same way
- `name` (string, optional): Its name in the tree
- `offset` (number, optional): How far along the axis the first sits from the second, mm
- `drive` (any, optional): A hinge's angle (degrees from where it was made) or a slider's position (mm) to hold it at; false lets it move again
- `limits` (any, optional): {low, high}: the range a hinge's angle or a slider's position stays in while not driven; false takes the limits away
- Returns the joint's id

`pc.asm.slider`: Put two axes on one line without turning: the body can only slide along it.

- `body` (id): The body that moves
- `face` (any): A round face, {axis = {point, direction}}, as pc.doc.faces lists it; an edge's line or circle axis goes the same way
- `other` (id): The body it is held against; the nil id (all zeros) for the world origin, its faces then in world space
- `other_face` (any): A round face, {axis = {point, direction}}, as pc.doc.faces lists it; an edge's line or circle axis goes the same way
- `name` (string, optional): Its name in the tree
- `drive` (any, optional): A hinge's angle (degrees from where it was made) or a slider's position (mm) to hold it at; false lets it move again
- `limits` (any, optional): {low, high}: the range a hinge's angle or a slider's position stays in while not driven; false takes the limits away
- Returns the joint's id

`pc.asm.fix`: Hold a body to another where it sits.

- `body` (id): The body that moves
- `face` (any, optional): Any face, as pc.doc.faces lists it; the body's origin when left out
- `other` (id): The body it is held against
- `other_face` (any, optional): Any face, as pc.doc.faces lists it; the body's origin when left out
- `name` (string, optional): Its name in the tree
- Returns the joint's id

`pc.asm.parallel`: Keep two faces or axes parallel.

- `body` (id): The body that moves
- `face` (any): A flat face {point, normal} or a round face or edge {axis = {point, direction}}
- `other` (id): The body it is held against; the nil id (all zeros) for the world origin, its faces then in world space
- `other_face` (any): A flat face {point, normal} or a round face or edge {axis = {point, direction}}
- `name` (string, optional): Its name in the tree
- Returns the joint's id

`pc.asm.perpendicular`: Keep two faces or axes square to each other.

- `body` (id): The body that moves
- `face` (any): A flat face {point, normal} or a round face or edge {axis = {point, direction}}
- `other` (id): The body it is held against; the nil id (all zeros) for the world origin, its faces then in world space
- `other_face` (any): A flat face {point, normal} or a round face or edge {axis = {point, direction}}
- `name` (string, optional): Its name in the tree
- Returns the joint's id

`pc.asm.distance`: Keep two faces, axes or points a distance apart: along a face, from an axis, between axes or points.

- `body` (id): The body that moves
- `face` (any): A flat face {point, normal}, a round face or edge {axis}, or a point ({centre} of a ball, or {point} alone)
- `other` (id): The body it is held against; the nil id (all zeros) for the world origin, its faces then in world space
- `other_face` (any): A flat face {point, normal}, a round face or edge {axis}, or a point ({centre} of a ball, or {point} alone)
- `name` (string, optional): Its name in the tree
- `offset` (number, optional): Along the second face's normal, mm; the distance they are now when left out
- Returns the joint's id

`pc.asm.tangent`: Rest a round face on a flat one.

- `body` (id): The body that moves
- `face` (any): A flat face {point, normal} on one body and a round face {axis, radius} on the other, either way round
- `other` (id): The body it is held against; the nil id (all zeros) for the world origin, its faces then in world space
- `other_face` (any): A flat face {point, normal} on one body and a round face {axis, radius} on the other, either way round
- `name` (string, optional): Its name in the tree
- `radius` (number, optional): The round face's radius, mm; the face's own when left out
- Returns the joint's id

`pc.asm.ball`: Put two points together: the body can turn every way about them.

- `body` (id): The body that moves
- `face` (any): A point: a ball's {centre}, or {point} alone, as pc.doc.faces lists them
- `other` (id): The body it is held against; the nil id (all zeros) for the world origin, its faces then in world space
- `other_face` (any): A point: a ball's {centre}, or {point} alone, as pc.doc.faces lists them
- `name` (string, optional): Its name in the tree
- Returns the joint's id

`pc.asm.universal`: Cross two yokes' pins at one point, square to each other: the body turns about either.

- `body` (id): The body that moves
- `face` (any): A round face, {axis = {point, direction}}, as pc.doc.faces lists it; an edge's line or circle axis goes the same way
- `other` (id): The body it is held against; the nil id (all zeros) for the world origin, its faces then in world space
- `other_face` (any): A round face, {axis = {point, direction}}, as pc.doc.faces lists it; an edge's line or circle axis goes the same way
- `name` (string, optional): Its name in the tree
- Returns the joint's id

`pc.asm.slot`: Keep a point on a line: a pin sliding in a slot.

- `body` (id): The body that moves
- `face` (any): The pin: a point ({centre} or {point}) on the moving body; the slot: a line {axis = {point, direction}} on the other
- `other` (id): The body it is held against; the nil id (all zeros) for the world origin, its faces then in world space
- `other_face` (any): The pin: a point ({centre} or {point}) on the moving body; the slot: a line {axis = {point, direction}} on the other
- `name` (string, optional): Its name in the tree
- Returns the joint's id

`pc.asm.path`: Keep a point on an edge of any shape: it runs along it.

- `body` (id): The body that moves
- `face` (any): The point that runs ({centre} or {point}); on the other body, a {point} on the edge it runs along
- `other` (id): The body it is held against; the nil id (all zeros) for the world origin, its faces then in world space
- `other_face` (any): The point that runs ({centre} or {point}); on the other body, a {point} on the edge it runs along
- `name` (string, optional): Its name in the tree
- Returns the joint's id

`pc.asm.cam`: Keep a follower on a cam's face, a roller's radius off it.

- `body` (id): The body that moves
- `face` (any): The follower ({centre} or {point}); on the other body, a {point} on the cam's face
- `other` (id): The body it is held against; the nil id (all zeros) for the world origin, its faces then in world space
- `other_face` (any): The follower ({centre} or {point}); on the other body, a {point} on the cam's face
- `name` (string, optional): Its name in the tree
- `radius` (number, optional): The follower's roller radius, mm; 0 for a point follower
- Returns the joint's id

`pc.asm.width`: Centre a tab's two faces between a slot's two walls.

- `body` (id): The body that moves
- `face` (any): A flat face, {point, normal}, as pc.doc.faces lists it
- `other` (id): The body it is held against; the nil id (all zeros) for the world origin, its faces then in world space
- `other_face` (any): A flat face, {point, normal}, as pc.doc.faces lists it
- `name` (string, optional): Its name in the tree
- `face2` (any): The tab's other flat face
- `other_face2` (any): The slot's other wall
- Returns the joint's id

`pc.asm.couple`: Tie two joints' motions together: gears or a belt between two hinges, a rack and pinion or a screw between a hinge and a slider.

- `driver` (id): The hinge or slider that leads
- `driven` (id): The hinge or slider that follows
- `gearing` (string, optional): gears (hinges turning opposite ways), belt (the same way), rack (a hinge and a slider, by the pinion's pitch radius) or screw (by the lead); the first that suits the two joints when left out
- `ratio` (number, optional): Turns of the driven hinge per turn of the driver for gears and a belt, the pitch radius in mm for a rack, the lead in mm a turn for a screw
- `reverse` (boolean, optional): The driven joint moves the other way
- `name` (string, optional): Its name in the tree
- Returns the coupling's id

`pc.asm.set`: Change a joint's gap, side, angle or radius, or a coupling's joints and ratio.

- `joint` (id): A joint or a coupling
- `gearing` (string, optional): gears (hinges turning opposite ways), belt (the same way), rack (a hinge and a slider, by the pinion's pitch radius) or screw (by the lead); the first that suits the two joints when left out
- `ratio` (number, optional): Turns of the driven hinge per turn of the driver for gears and a belt, the pitch radius in mm for a rack, the lead in mm a turn for a screw
- `reverse` (boolean, optional): A coupling's driven joint moves the other way
- `driver` (id, optional): A coupling's leading joint
- `driven` (id, optional): A coupling's following joint
- `offset` (number, optional): A mate's gap, a hinge's height or a distance, mm
- `flip` (boolean, optional): A mate's side
- `degrees` (number, optional): An angle joint's angle
- `radius` (number, optional): A tangent's radius, mm
- `drive` (any, optional): A hinge's angle (degrees from where it was made) or a slider's position (mm) to hold it at; false lets it move again
- `limits` (any, optional): {low, high}: the range a hinge's angle or a slider's position stays in while not driven; false takes the limits away
- `turn_drive` (any, optional): An alignment's turn (degrees from where it was made) to hold it at; false lets it turn
- `turn_limits` (any, optional): {low, high}: the range an alignment's turn stays in; false takes it away
- `slide_drive` (any, optional): How far along the axis (mm) to hold an alignment; false lets it slide
- `slide_limits` (any, optional): {low, high}: the range an alignment's slide stays in, mm; false takes it away

`pc.asm.copy`: Insert linked copies of a body: each takes its shape and follows it, placed on its own.

- `body` (id): The body to copy
- `count` (number, optional): How many (1 when left out)
- `step` (list, optional): {x, y, z}: how far each copy sits from the one before, mm; beside it along X when left out
- `around` (any, optional): {point = {x, y, z}, direction = {x, y, z}, angle}: the copies turned about this axis instead, spread evenly over `angle` degrees (360 when left out)
- Returns the copies' ids

`pc.asm.mirror`: Insert a linked copy that is a body's mirror image, following every change to it.

- `body` (id): The body to mirror
- `point` (list): A point of the mirror plane, {x, y, z}, in the world
- `normal` (list): The plane's normal, {x, y, z}
- Returns the mirrored copy's id

`pc.asm.replace`: Put another body in a body's place, with its joints found again on the new body's faces.

- `body` (id): The body to replace; it is hidden
- `with` (id): The body that takes its place
- Returns {kept, unmatched}: the joints whose ends were found on the new body, and those that were not

`pc.asm.group`: Lock bodies together where they sit, in one rigid group.

- `bodies` (list): Two bodies or more; the first the one the rest hold to
- `group` (id, optional): A group to change to these bodies, rather than a new one
- `name` (string, optional): A new group's name in the tree
- Returns the group's id

`pc.asm.component`: Put bodies in a new component: one row in the tree that moves as one, or, flexible, keeps the joints inside it live; components nest.

- `bodies` (list): The bodies it holds, taken out of any other
- `name` (string, optional): Its name in the tree
- `parent` (id, optional): The component it sits in; the top if left out
- `flexible` (boolean, optional): The joints inside it move (false: rigid)
- Returns the component's id

`pc.asm.component_set`: Rename a component, move it, or make it rigid or flexible.

- `component` (id): The component
- `name` (string, optional): A new name
- `flexible` (boolean, optional): The joints inside it move
- `parent` (any, optional): The component it goes in, or null for the top

`pc.asm.component_add`: Put bodies in a component, or take them out.

- `bodies` (list): The bodies
- `component` (id, optional): The component; left out, the bodies go to the top

`pc.asm.component_remove`: Take a component apart: its bodies and components go one level up.

- `component` (id): The component

`pc.asm.motion`: Keep a motion over time: hinges and sliders each driven by a formula of t, seconds.

- `drives` (list): {{joint = id, formula = "90 * t"}, ...}: a hinge's angle in degrees, a slider's position in mm
- `start` (number, optional): When it starts, s (0 when left out)
- `end` (number, optional): When it ends, s (2 when left out)
- `step` (number, optional): The time between frames, s (0.05 when left out)
- `study` (id, optional): A motion to change, rather than a new one
- `name` (string, optional): A new motion's name in the tree
- Returns the motion's id

`pc.asm.motion_frames`: Every body's placement at each frame of a motion; nothing is moved.

- `study` (id): The motion
- Returns a list of {t, bodies = {{body, translation, rotation}, ...}}

`pc.asm.trace`: Follow a point of a body through a motion: where it is and how fast at each frame.

- `study` (id): The motion
- `body` (id): The body
- `point` (list): {x, y, z} in the body's own frame
- Returns a list of {t, point, speed (mm/s)}

`pc.asm.exploded_view`: Keep an exploded view: steps, each moving some bodies by a shift, played in order.

- `steps` (list): {{bodies = {ids}, shift = {x, y, z}}, ...}, in the order they play
- `view` (id, optional): A view to change, rather than a new one
- `name` (string, optional): A new view's name in the tree
- Returns the view's id

`pc.asm.explode_at`: Where an exploded view puts every body, part way through its steps.

- `view` (id): The exploded view
- `at` (number): How many steps in: 1.5 is half way through the second
- Returns a list of {body, translation, rotation}; nothing is moved

`pc.asm.save_state`: Save where every body sits, which are hidden and where drives hold, under a name.

- `name` (string, optional): A new state's name in the tree
- `state` (id, optional): A saved state to keep the assembly in instead
- Returns the state's id

`pc.asm.restore_state`: Put the assembly back as a saved state has it.

- `state` (id): The saved state

`pc.asm.redundant`: The joints that hold nothing a body's other joints do not.

- Returns a list of {joint, name}

`pc.asm.motion_clashes`: Step a hinge's or a slider's drive through a range and find where bodies collide.

- `joint` (id): The hinge or slider
- `low` (number): Where the steps start: degrees or mm
- `high` (number): Where they end
- `steps` (number, optional): How many steps (24 when left out)
- Returns a list of {at, a, b, volume (mm³)}: each step and pair sharing more material than where the joint stands

`pc.asm.turn`: Turn a joint's body about the joint's axis or normal, the joint keeping it there.

- `joint` (id): The joint
- `degrees` (number): How far, degrees

`pc.asm.flip`: Turn a joint's body over, half a turn across the joint's axis or normal.

- `joint` (id): The joint

`pc.asm.interference`: Where solid bodies share material: each pair that clashes, how much and where.

- `bodies` (list, optional): Only these bodies; every visible one when left out
- `clearance` (number, optional): Look instead for pairs nearer than this many mm
- Returns {checked, skipped, clashes}, each clash {a, b, volume (mm³), centre}; skipped counts visible bodies with no solid. With a clearance, {checked, skipped, near}, each {a, b, distance (mm), on_a, on_b}, nearest first

`pc.asm.mass`: The mass and centre of mass of the solid bodies at one density.

- `bodies` (list, optional): Only these bodies; every visible one when left out
- `density` (number, optional): g/cm³ for bodies without a material (1 when left out)
- Returns {mass (g), volume (mm³), centre = {x, y, z} or nil, bodies = {{body, mass, volume, centre}, ...}, skipped}

`pc.asm.parts`: Every part: bodies of the same shape counted together.

- `by_component` (boolean, optional): Each component's parts under it: every entry gains a depth, and components come as {component, name, depth}
- Returns a list of {name, quantity, bodies, size = {x, y, z} in mm or nil, mesh, number or nil, bought, values = {column = text}}, numbered parts first by number, then by name

`pc.asm.part`: Set what the parts list keeps for a part: its number, whether it is bought, its values in the added columns.

- `body` (id): Any body of the part
- `number` (number, optional): Its item number
- `bought` (boolean, optional): Bought rather than made: left out of exports and the slicer
- `values` (any, optional): {column = text}: its values, a column not yet in the list added to it

`pc.asm.parts_table`: Replace what the parts list keeps, whole.

- `table` (any): {columns = {...}, entries = {[body id] = {number, bought, values}}}

`pc.asm.travel`: Where a hinge or a slider has got to: the hinge's angle in degrees, the slider's position in mm.

- `joint` (id)
- Returns a number

`pc.asm.ground`: Keep a body where it is: the bodies joined to it are placed against it.

- `body` (id)
- `grounded` (boolean, optional): false lets it move again (true by default)
- Returns the ground joint's id, or nil when it was taken away

`pc.asm.freedom`: What each jointed body may still do: the motions its joints leave open.

- `body` (id, optional): Only this body
- Returns a list of {body, free, motions}, each motion {turn = {axis, through}} or {slide = direction}, with at_limit true where a limit lets it go one way only

`pc.asm.solve`: Place every body its joints hold.

- Returns what moved, in words

`pc.asm.placement`: Where a body sits.

- `body` (id)
- Returns {translation, rotation}, rotation a quaternion {x, y, z, w}

`pc.asm.place`: Put a body at a placement.

- `body` (id)
- `translation` (list, optional): {x, y, z} in mm
- `rotation` (list, optional): A quaternion {x, y, z, w}

`pc.asm.move`: Move a body by a step and a turn.

- `body` (id)
- `by` (list, optional): {x, y, z} in mm
- `turn` (number, optional): Degrees about `axis`
- `axis` (list, optional): {x, y, z}; Z when left out
- `about` (list, optional): The point the turn is about, {x, y, z}; the origin when left out
<!-- /commands -->
