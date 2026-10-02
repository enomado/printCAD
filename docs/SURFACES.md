# Surfaces

The Surface workbench makes sheets: surfaces with no inside, built from
curves, joined into shells, and closed into solids. Use it for shapes a
solid feature cannot reach: a car body panel, a mouse shell, a duct, or a
lid whose edge follows a free-form curve.

## Surface bodies

Surface steps go into a surface body. The first step goes into the body
of the selected sketch when that body holds only sketches and datums, and
into a new body named Surface otherwise. A Design feature asked for on a
surface body goes into a new body of its own, so the two never build over
each other. To build solid features on a surface model, sew it closed
into a solid and use the body in Design (a body boolean, or borrowing its
shape).

## Curves

A step is built from curves:

- **Sketches.** Every chain of a sketch's curves counts, open or closed.
  An open chain runs from one loose end to the other. A point where three
  curves meet is refused, since no single chain passes it.
- **Edges.** Edges of the body's own surfaces, picked in the view (Ctrl
  adds to the pick). A fill can close the gap between surfaces this way.

Select a sketch in the tree, or edges in the view, before clicking a tool,
and the step takes them. In the task, "Add the selection" takes what is
selected now, and the cross by a row takes it out.

## Making surfaces

| Tool | What it makes |
| --- | --- |
| Extruded surface | Each curve swept straight: square to its sketch, or along X, Y or Z; one way, reversed, or half each way. |
| Revolved surface | Each curve turned about an axis: the sketch's vertical or horizontal axis, or X, Y or Z. |
| Planar surface | The flat face closed loops bound, a loop inside another a hole. |
| Filling | The surface four curves meeting end to end bound. |
| Ruled surface | Straight lines between two curves, end to end. |
| Lofted surface | A surface through section curves, one per sketch, in order. |
| Swept surface | A profile moved along a straight path. |

Extrusions, revolutions and planar surfaces are exact. A ruled surface, a
loft and a fill are B-spline surfaces fitted through the curves to within
a thousandth of a millimetre.

## Working on surfaces

| Tool | What it does |
| --- | --- |
| Sew | Joins the body's surfaces where their edges meet. A shell that closes becomes a solid. |
| Mirror | Adds the body's reflection in the YZ, XZ or XY plane, moved by an offset. |

## Checking how faces meet

Check continuity measures every edge where two faces of the selected body
meet, a surface body or a solid: the gap between them, and the angle of
the crease (none where they meet tangent). Each edge is labelled in the
view: G1 for a tangent join, the crease angle, or the gap where the faces
are apart; the task lists them from the sharpest crease down. Close puts
the labels away.

## Waiting on the geometry kernel

These tools show in the toolbar, dim, until the geometry kernel has their
operation. A script can make their steps already: each keeps its settings
and says in its task why it does not build yet.

- Offset surface, Extend surface, Blend surface, Split surface, Thicken,
  Trim by plane.
- A fill of three sides or more than four, or one tangent (G1) or
  curvature continuous (G2) to its neighbours.
- A sweep along a curved path, and a loft closed back on itself.
- Sewing a fill, a ruled surface or a loft to its neighbours: these come
  without boundary edges to join.
- Exporting a surface body as STEP.

## Scripting

Every tool that builds has a command: `pc.surface.extrude`,
`pc.surface.revolve`, `pc.surface.planar`, `pc.surface.fill`,
`pc.surface.ruled`, `pc.surface.loft`, `pc.surface.sweep`,
`pc.surface.sew` and `pc.surface.mirror`. They take `sketches` (a list,
in order), `body` (a body, or any feature in it) and any field of the step
by name.

```lua
local s = pc.sketch.new{plane = "XY"}
pc.sketch.polyline{sketch = s, points = {{0, 0}, {10, 0}, {10, 10}, {0, 10}, {0, 0}}}
local walls = pc.surface.extrude{sketches = {s}, length = 5}
pc.surface.planar{body = walls, sketches = {s}}
pc.surface.sew{body = walls}
```
