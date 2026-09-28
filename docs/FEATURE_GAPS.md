# What is left to build

An audit of the Sketcher, Part Design and Assembly workbenches against the
granular feature set of a mature parametric CAD, feature by feature, each
checked in the code. It lists what is missing or partly built. Priority is
for a CAD aimed at FDM and SLA printing (high, medium, low); size is rough
effort (S: a day or less, M: a few days, L: a week or more).

| Workbench | Checked | Built | Partial | Missing |
| --- | --- | --- | --- | --- |
| Sketcher | 136 | 98 | 15 | 22 |
| Part Design | about 105 | about 76 | 25 | 5 |
| Assembly | 111 | 35 | 25 | 51 |

Part Design and the Sketcher are close to complete: what is missing is
mostly options and input kinds on tools that exist. No tool in either is a
placeholder, and nothing waits on the geometry kernel. The Assembly's
joints and solver are complete for joining bodies (ten joint kinds, ground,
gears, belts, racks and screws, drives and limits, dragging that stops at
collisions, the free motions of each body, interference, a parts list), but
the way components are held is flat: every component is a body in one
document, with no sub-assemblies, no linked copies of a part, no parts
brought in from other printCAD files, and joints that do not follow a face
when its part is edited. Exploded views, motion playback and the parts list
are first versions.

## Next, across the workbenches

The gaps worth most to printing first. Everything else follows in the
workbench sections.

1. **Custom hole clearance** (Part Design, high, S). Printed holes come out
   undersized; a clearance number per hole, beside the close, normal and
   loose fits.
2. **Distance between two parallel lines** (Sketcher, high, S). How walls
   and slot widths are dimensioned.
3. **Printing columns in the parts list** (Assembly, high, S). Volume,
   filament mass from a density, and parts marked bought (screws, bearings,
   magnets) apart from parts printed.
4. **Clearance check between parts** (Assembly, high, M). Interference
   finds shared material only; print-in-place joints, snap fits and press
   fits depend on the gap between parts. Needs a minimum distance query
   from the kernel.
5. **Joints that follow part edits** (Assembly, high, M). Joint anchors
   keep the face and edge names a pick carries and find them again after a
   rebuild, as sketches on faces do.
6. **Text in sketches** (Sketcher, high, L). A string as closed outlines,
   for embossed and debossed labels.
7. **Validate that repairs** (Sketcher, high, M). Join endpoints that
   nearly touch and remove zero-length and duplicate geometry: what stops
   imported outlines from padding.
8. **Fillet and chamfer between any curves** (Sketcher, high, M).
9. **Offset options** (Sketcher, high, M). Rounded joins, ellipses and
   splines, both sides, and an offset dimension that stays live.
10. **External geometry in profiles** (Sketcher, high, M). Pad straight up
    to a face's outline.
11. **Smooth join at a shared endpoint** (Sketcher, high, M).
12. **Linked copies of a body, and parts from other printCAD files**
    (Assembly, high, L). Edit a part once and every copy follows.
13. **Measure angles, radii and face areas** (Part Design and Assembly,
    medium, M).
14. **Datum and base planes as targets** (Part Design, medium, S). Up to a
    datum plane, mirror about one, extrude along a datum or sketch line.
15. **Internal (ring) gear** (Part Design, medium, S). Printed planetary
    gearboxes.

## Sketcher

### Geometry

- **Text** (missing, high, L). A string in a chosen font as closed outlines
  (lines, arcs, splines) that can be padded or pocketed, with size,
  spacing and an anchor point constraints can hold, editable afterwards. A
  generated sketch element (`src/generator/`) regenerated from its
  parameters.
- **Polyline segment modes** (partial, low, S). Only straight and tangent
  arc; missing a perpendicular arc and a free or reversed arc
  (`tools.rs::toggle_polyline_arc`).

### Editing

- **Fillet and chamfer between any curves** (partial, high, M). Only at a
  vertex two lines share (`tools/modify.rs`). Missing line and arc, arc and
  arc, two curves that do not meet yet (trimmed or extended to fit), and a
  keep-the-corner option leaving a construction point with the corner's
  constraints.
- **Offset options** (partial, high, M). Lines, arcs and circles with
  sharp joins only. Missing rounded joins at convex corners, ellipses and
  B-splines, both sides, deleting the original, and an offset dimension that
  keeps the copy parametric.
- **Trim, extend and split on every curve** (partial, medium, M). Trim and
  extend skip ellipses, arcs of ellipse, conics and B-splines; split skips
  ellipses and B-splines. Needs their intersections in `geom2d.rs`.
- **Linked copies** (missing, medium, M). Copies and arrays keep their own
  constraints but nothing ties them to the original: equal size to the
  original and one editable pitch between array members
  (`tools/transform.rs`).
- **Symmetry options** (partial, medium, S). Mirror about a point, delete
  the original, and symmetric constraints so the copy follows edits.
- **Scale options** (partial, low, S). A keep-the-original copy, and
  scaling the selection's dimensions with it.
- **Cancel a sketch session** (missing, low, S). Cancel closes like Close;
  it should put the sketch back as the session found it.

### External references

- **External geometry in profiles** (missing, high, M). Projected curves are
  always left out of profiles; a per-element switch lets one close a
  profile (`Sketch::external`, `profile::extract_wires`).
- **External geometry from sketches and datums** (missing, medium, M).
  Another sketch's curves and points, datum lines and points, a datum
  plane's crossing, followed as they change (`ExternalSource`).

### Constraints

- **Smooth join** (missing, high, M). A point and two curves: the curves
  meet at the point with matching direction, as one constraint.
- **Tangent and perpendicular for every curve kind** (partial, medium, L).
  Tangent is line and circle or two circles; perpendicular is two lines.
  Missing ellipses, conics and B-splines, and a line square to a circle
  (`solver.rs` `Tangent`).
- **Point on B-splines and conics** (partial, medium, M).
- **Horizontal or vertical between two points** (partial, medium, S). The
  tools take lines only; two hole centres level is a common step.
- **One constraint across many elements** (partial, medium, S). Equal on
  six holes in one click; coincident, parallel and equal take exactly two.
- **Horizontal-or-vertical in one command** (missing, low, S).
- **Equal for ellipses** (missing, low, S).
- **Lock as two editable dimensions** (partial, low, S). Shown and driven
  as distances from the origin.
- **Coincident falling back to point on curve** (missing, low, S).

### Dimensions

- **Distance between two parallel lines** (missing, high, S).
- **An arc's sweep angle** (missing, medium, S).
- **Angle by three points** (missing, low, S).
- **Dimensions on ellipses, conics and splines** (partial, low, M). Directly
  on the curve, and a spline's length.
- **Label shows name, value, or both** (missing, low, S).

### Solver and selection

- **Validate that repairs** (partial, high, M). It reports and selects
  stray points, malformed constraints and open or branching profiles;
  missing joining endpoints within a tolerance, removing duplicate and
  zero-length geometry, and fixing degenerate arcs.
- **From elements to their constraints and back** (missing, medium, S). The
  main way to untangle a conflicting sketch.
- **Solver settings** (missing, low, M). Iterations, tolerance, output for a
  hard sketch.

### B-splines

- **Convert a curve to a B-spline** (missing, low, M).
- **Raise or lower the degree** (missing, low, S).
- **Knot multiplicity and knot insertion** (missing, low, M).
- **Weights (rational splines)** (missing, low, M).
- **Curvature comb and knot display** (missing, low, M).

### Placement and input

- **Attachment modes of its own** (partial, medium, M). Three points,
  normal to an edge (pipe profiles), tangent to a curved face, concentric
  with a circular edge, without a datum first.
- **Offset within the plane** (partial, medium, S). Shift, turn and flip
  against the support.
- **Angle snap while drawing** (missing, medium, S). Steps of 15 degrees
  with a modifier.
- **Rendering order in the options panel** (placeholder, low, S). The
  dropdown is drawn but disabled; the toolbar toggle already does it.

## Part Design

### Datums and attachment

- **More attachment modes** (partial, medium, M). Ten exist. Missing: a
  datum on another datum (a plane offset or turned from a datum plane or a
  coordinate system's plane), another body's origin planes as the frame, a
  line normal to a face, a line tangent to an edge, planes along a curve,
  a plane through a line and a point, a point where a line meets a plane or
  two lines cross, and sketch vertices and lines as references
  (`core_document/src/datum.rs`, `wb_part/src/datum_panel.rs`).
- **Attachment offset tilt** (partial, low, S). The offset turns only about
  the normal.
- **Borrowed geometry options** (partial, low, M). An offset of the
  borrowed geometry, a face from borrowed closed edges, a whole solid as
  reference only (`borrow.rs`).

### Features

- **Datum and base planes as targets** (partial, medium, S). Up to a datum,
  base or coordinate system plane (`ExtrudeMode::UpToFace`).
- **Extrusion direction** (partial, medium, S). A datum line, a sketch
  line, a base axis, and length measured along the sketch normal for a
  slanted direction (`ExtrudeDirection`).
- **Start offset** (missing, low, S). Start the extrusion away from the
  profile plane.
- **A taper per side** (partial, low, S). Needs a second taper in the
  kernel's extrude.
- **Revolution and helix about the base axes** (partial, low, S), and a
  helix about the sketch normal.
- **Loft to a point, faces as sections** (partial, medium, M).
- **Pipe along solid or borrowed edges, a face as the profile, a point at
  the end** (partial, medium, M).
- **Fixed pipe orientation** (partial, low, S). The section keeps its
  orientation in space.
- **Attachable primitives** (partial, medium, M). Placed on a face, datum
  or edge and following it (`DatumAttachment` on `PartFeature::Primitive`).
- **Primitive parameters** (partial, low, S). The ellipsoid's angular
  cut-outs and the prism's skew angles.

### Hole

- **Custom clearance** (partial, high, S). A number beside the close,
  normal and loose fits (`HoleFit`, `hole_tables.rs`).
- **More screw seats** (partial, medium, S each). Button head, slotted and
  cross countersunk, low head cap, cap screw with washer, hex head.
- **Nut trap** (missing, medium, S). A hexagonal pocket sized from the
  thread's nut, for captive nuts.
- **Thread length choices** (partial, low, S). The whole hole depth, or by
  the thread's run-out.

### Dress-ups, patterns and booleans

- **Mirror plane** (partial, medium, S). A datum plane, a coordinate
  system's plane, a sketch's axis or plane (`MirrorPlane`).
- **Draft references** (partial, low, S). A datum or base plane as the
  neutral plane, an edge or datum line as the pull direction.
- **Thickness modes** (partial, low, S). Pipe and both sides.
- **Scale centre by pick** (partial, low, S).
- **Several tool bodies in one boolean** (partial, low, S).

### Body and history

- **Move a feature to another body** (missing, medium, M).
- **Duplicate, copy and paste features** (missing, low, M).
- **Drag handles in the view** (missing, medium, L). Drag a pad's end or a
  fillet's radius.

### Generators and measuring

- **Internal (ring) gear** (partial, medium, S), and addendum and dedendum
  coefficients (`wb_sketch/src/generator/gear.rs`).
- **Shaft loads and stresses** (partial, low, L).
- **Measure angles, radii and face areas** (partial, medium, M). The tool
  measures distance only (`app_shell/src/app/input.rs::measure_click`).

## Assembly

### Components and structure

- **Linked copies of a body** (missing, high, L). Several placements
  sharing one definition: edit once, every copy follows, and the parts
  list counts them. A body kind in `core_document` referring to another
  body's solid, picked up by `parts.rs`, `solve.rs` and export.
- **Parts from other printCAD files** (missing, high, L). Inserted linked,
  with an out-of-date mark, reload and open source; today outside parts
  come in only as frozen STEP, IGES or mesh imports (File › Insert).
- **Sub-assemblies** (missing, medium, L). Bodies grouped into a component
  that moves as one, or keeps its own joints live inside the parent;
  nested. A group node in `core_document` and a group-aware solver.
- **Rigid group** (partial, medium, S). Several bodies locked in one
  action, not pairwise fixed joints.
- **Patterns and mirrors of components** (missing, medium, M). Arrays and
  mirrored copies of bodies in the assembly, built on linked copies.
- **Insert several copies, place by dragging** (missing, low, S).
- **Replace a component** (missing, low, M), keeping its joints where the
  new faces allow.
- **Ground the first component** (partial, low, S), so the free motions
  read true from the start.

### Joints

- **Joints that follow part edits** (missing, high, M). Anchors keep face
  and edge names (`FaceRef::name`, `EdgeRef::faces`) and are found again
  on rebuild.
- **Point anchors and a ball joint** (missing, medium, M). A vertex, a face
  centre, a sphere's centre; three turns free about it.
- **Distance, parallel, perpendicular and angle on more than flat faces**
  (partial, medium, M). Point to point, point to plane, point to line,
  axis to axis (a centre distance, for gear spacing); edges and axes for
  parallel, perpendicular and angle.
- **Joints to the origin and datums** (missing, medium, S). The world
  planes and datums as the other end.
- **Align with offset, drive and limits** (partial, medium, S). The
  cylindrical joint's turn and slide driven or limited.
- **A turn offset and a flip on every joint** (partial, medium, S), and an
  editable shift for a fixed joint.
- **Re-pick faces and change the kind** (missing, medium, S). From the
  joint's settings, without deleting it.
- **Joints drawn in the view** (missing, medium, S). Anchor frames, axes and
  the link between them.
- **Other bodies faded while picking** (missing, low, S).
- **A separate offset per end** (missing, low, M).
- **More kinds** (missing, low, M). Cam and follower, slot, path, width,
  universal joint.
- **Every joint touching a body listed** (partial, low, S). Joints where
  the body is the other end are not shown under it.

### Solving and moving

- **Move handles** (missing, medium, M). Arrows and rings on a body, where
  today an unjointed body moves by numbers.
- **Redundant joints reported** (partial, low, M). Consistent but
  overdefined joints flagged.

### Motion

- **Motion over time** (partial, medium, M). Several joints at once, each a
  formula of time, with start, end and step, a frame scrubber and step
  play; built on `sweep_frames`.
- **Collisions during a motion** (missing, medium, S). The frames and pairs
  where bodies share material (`collide.rs` measures it for drags).
- **GIF and frame sequence export** (partial, low, S). Recording writes an
  animated PNG only.
- **Traces, speeds, plots** (missing, low, L).

### Exploded views and states

- **Saved, stepped exploded views** (partial, medium, M). Per-body moves and
  turns as ordered steps, several named views kept in the document,
  explode lines, animation. Today one radial spread, dropped on close.
- **Saved assembly states** (missing, low, M). Placements and visibility to
  return to: a print-in-place hinge folded and open.

### Parts list

- **Printing columns** (missing, high, S). Volume, filament mass from a
  density, count to print (`parts.rs`).
- **Bought parts** (missing, medium, S). Marked apart from printed ones and
  left out of export.
- **Kept in the document, numbered** (missing, low, S).
- **Custom columns** (missing, low, M). Part number, supplier, any property.
- **Sub-assembly levels** (missing, low, S, after sub-assemblies).

### Checks and display

- **Clearance check** (missing, high, M). Pairs closer than a given gap,
  and where. Needs a minimum distance query in `KernelQueries`, then a mode
  of `interference.rs`.
- **Interference among the selected bodies from the toolbar** (partial,
  medium, S). The command takes a list; the tool takes every visible body.
- **Total mass and centre of mass** (partial, low, S). For tip-over checks.
- **Isolate, show only the selection** (missing, medium, S).
- **Configurations that suppress or swap bodies** (partial, low, M).

### Interchange

- **STEP export with the assembly's structure** (partial, medium, M).
  Identical bodies as shared instances in a product tree; likely a kernel
  addition.
- **Print layout** (missing, medium, M). Each part of the parts list laid
  flat on its best face and copies packed on the bed, for export or the
  slicer.
