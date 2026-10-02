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

Create sketch starts a sketch in the selected surface body (or a new
one); finishing it comes back to Surface with the sketch selected, ready
for a tool. The tools that build from curves stay dim until a sketch is
selected in the tree or edges are picked; Sew and Mirror until a surface
body with a shape is selected. A tool takes what is selected when it is
clicked; in its task, "Add the selection" takes what is selected now, and
the cross by a row takes it out.

## Making surfaces

| Tool | What it makes |
| --- | --- |
| Extruded surface | Each curve swept straight: square to its sketch, or along X, Y or Z; one way, reversed, or half each way. |
| Revolved surface | Each curve turned about an axis: the sketch's vertical or horizontal axis, or X, Y or Z. |
| Planar surface | The flat face closed loops bound, a loop inside another a hole. |
| Filling | The surface curves meeting end to end bound, three or more. A side picked on a surface meets that surface touching (G0), tangent (G1) or curvature continuous (G2); a sketch's curve is touched. |
| Ruled surface | Straight lines between two curves, end to end, one face per pair of pieces. |
| Lofted surface | A surface through section curves, one per sketch, in order; closed back to the first if asked. |
| Swept surface | A profile moved along a path, straight or curved, turning with it. |
| Offset surface | Picked faces copied at a distance along their normals. |
| Blend surface | A surface bridging two picked edges, meeting each edge's face touching, tangent or curvature continuous. |

Every surface is bounded by edges: a later step can pick them, and Sew
joins it to its neighbours. A filling is bounded by the very edges it was
given.

## Working on surfaces

| Tool | What it does |
| --- | --- |
| Sew | Joins the body's surfaces where their edges meet, or come within a gap you set. A shell that closes becomes a solid. |
| Surface fillet | Rounds picked edges where two faces of a surface meet. |
| Extend surface | Grows faces past picked edges: on their own surface (G2), straight on tangent (G1), or straight on (G0). |
| Split surface | Cuts picked faces along curves projected onto them. |
| Trim by plane | Keeps what of the body lies on one side of a plane. |
| Thicken | Gives each of the body's sheets a thickness, one way or half each way: a solid apiece. |
| Mirror | Adds the body's reflection in the YZ, XZ or XY plane, moved by an offset. |

## Checking how faces meet

Check continuity measures every edge where two faces of the selected body
meet, a surface body or a solid: the gap between them, and the angle of
the crease (none where they meet tangent). Each edge is labelled in the
view: G1 for a tangent join, the crease angle, or the gap where the faces
are apart; the task lists them from the sharpest crease down. Close puts
the labels away.

## Waiting on the geometry kernel

A few cases still fail, each with the kernel's reason in its task:

- Filling, blending, extending or rounding from an extruded surface's far
  edge (its near edge, the sketch's own curve, works).
- Filling between edges of separate surfaces that meet only at a point:
  sew them first where they share an edge.
- A ruled surface or a loft between curves cut into different numbers of
  pieces.
- Offsetting or thickening a free-form surface (a filling, a loft); flat
  and round faces offset.

## Scripting

Every tool that builds has a command: `pc.surface.extrude`,
`pc.surface.revolve`, `pc.surface.planar`, `pc.surface.fill`,
`pc.surface.ruled`, `pc.surface.loft`, `pc.surface.sweep`,
`pc.surface.offset`, `pc.surface.blend`, `pc.surface.sew`,
`pc.surface.fillet`, `pc.surface.extend`, `pc.surface.split`,
`pc.surface.trim`, `pc.surface.thicken` and `pc.surface.mirror`. They
take `sketches` (a list, in order), `body` (a body, or any feature in it)
and any field of the step by name.

```lua
local s = pc.sketch.new{plane = "XY"}
pc.sketch.polyline{sketch = s, points = {{0, 0}, {10, 0}, {10, 10}, {0, 10}, {0, 0}}}
local walls = pc.surface.extrude{sketches = {s}, length = 5}
pc.surface.planar{body = walls, sketches = {s}}
pc.surface.sew{body = walls}
```
