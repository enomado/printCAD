# What is left to build

An audit of the Sketcher, Part Design and Assembly workbenches against the
granular feature set of a mature parametric CAD, feature by feature, each
checked in the code. This is the working list: an item is ticked when it
is built, with its tests. Sizes are rough effort (S: a day or less, M: a
few days, L: a week or more). Features that matter only for printing wait
until the application is complete; they are listed at the end.

| Workbench | Checked | Built | Partial | Missing |
| --- | --- | --- | --- | --- |
| Sketcher | 136 | 98 | 15 | 22 |
| Part Design | about 105 | about 76 | 25 | 5 |
| Assembly | 111 | 35 | 25 | 51 |

(The counts are those of the audit; the ticks below are the progress
since.)

A kernel capability that is missing is filed on the kernel's repository
and noted on its item, and the rest of the item is built around it.

## Sketcher

### Geometry

- [ ] **Text** (missing, L). A string in a chosen font as closed outlines
  (lines, arcs, splines) that can be padded or pocketed, with size,
  spacing and an anchor point constraints can hold, editable afterwards. A
  generated sketch element (`src/generator/`) regenerated from its
  parameters.
- [x] **Polyline segment modes** (partial, S). Only straight and tangent
  arc; missing a perpendicular arc and a free or reversed arc
  (`tools.rs::toggle_polyline_arc`).

### Editing

- [ ] **Fillet and chamfer between any curves** (partial, M). Only at a
  vertex two lines share (`tools/modify.rs`). Missing line and arc, arc and
  arc, two curves that do not meet yet (trimmed or extended to fit), and a
  keep-the-corner option leaving a construction point with the corner's
  constraints.
- [ ] **Offset options** (partial, M). Lines, arcs and circles with
  sharp joins only. Missing rounded joins at convex corners, ellipses and
  B-splines, both sides, deleting the original, and an offset dimension that
  keeps the copy parametric.
- [ ] **Trim, extend and split on every curve** (partial, M). Trim and
  extend skip ellipses, arcs of ellipse, conics and B-splines; split skips
  ellipses and B-splines. Needs their intersections in `geom2d.rs`.
- [ ] **Linked copies** (missing, M). Copies and arrays keep their own
  constraints but nothing ties them to the original: equal size to the
  original and one editable pitch between array members
  (`tools/transform.rs`).
- [x] **Symmetry options** (partial, S). Mirror about a point, delete
  the original, and symmetric constraints so the copy follows edits.
- [x] **Scale options** (partial, S). A keep-the-original copy, and
  scaling the selection's dimensions with it.
- [x] **Cancel a sketch session** (missing, S). Cancel closes like Close;
  it should put the sketch back as the session found it.

### External references

- [x] **External geometry in profiles** (missing, M). Projected curves are
  always left out of profiles; a per-element switch lets one close a
  profile (`Sketch::external`, `profile::extract_wires`).
- [x] **External geometry from sketches and datums** (missing, M).
  Another sketch's curves and points, datum lines and points, a datum
  plane's crossing, followed as they change (`ExternalSource`).

### Constraints

- [x] **Smooth join** (missing, M). A point and two curves: the curves
  meet at the point with matching direction, as one constraint.
- [x] **Tangent and perpendicular for every curve kind** (partial, L).
  Tangent is line and circle or two circles; perpendicular is two lines.
  Missing ellipses, conics and B-splines, and a line square to a circle
  (`solver.rs` `Tangent`).
- [x] **Point on B-splines and conics** (partial, M).
- [x] **Horizontal or vertical between two points** (partial, S). The
  tools take lines only; two hole centres level is a common step.
- [x] **One constraint across many elements** (partial, S). Equal on
  six holes in one click; coincident, parallel and equal take exactly two.
- [x] **Horizontal-or-vertical in one command** (missing, S).
- [x] **Equal for ellipses** (missing, S).
- [x] **Lock as two editable dimensions** (partial, S). Shown and driven
  as distances from the origin.
- [x] **Coincident falling back to point on curve** (missing, S).

### Dimensions

- [x] **Distance between two parallel lines** (missing, S).
- [x] **An arc's sweep angle** (missing, S).
- [x] **Angle by three points** (missing, S).
- [x] **Dimensions on ellipses, conics and splines** (partial, M). Directly
  on the curve, and a spline's length.
- [x] **Label shows name, value, or both** (missing, S).

### Solver and selection

- [x] **Validate that repairs** (partial, M). It reports and selects
  stray points, malformed constraints and open or branching profiles;
  missing joining endpoints within a tolerance, removing duplicate and
  zero-length geometry, and fixing degenerate arcs.
- [x] **From elements to their constraints and back** (missing, S). The
  main way to untangle a conflicting sketch.
- [x] **Solver settings** (missing, M). Iterations, tolerance, output for a
  hard sketch.

### B-splines

- [ ] **Convert a curve to a B-spline** (missing, M).
- [ ] **Raise or lower the degree** (missing, S).
- [ ] **Knot multiplicity and knot insertion** (missing, M).
- [ ] **Weights (rational splines)** (missing, M).
- [ ] **Curvature comb and knot display** (missing, M).

### Placement and input

- [x] **Attachment modes of its own** (partial, M). Three points,
  normal to an edge (pipe profiles), tangent to a curved face, concentric
  with a circular edge, without a datum first.
- [x] **Offset within the plane** (partial, S). Shift, turn and flip
  against the support.
- [x] **Angle snap while drawing** (missing, S). Steps of 15 degrees
  with a modifier.
- [x] **Rendering order in the options panel** (placeholder, S). The
  dropdown is drawn but disabled; the toolbar toggle already does it.

## Part Design

### Datums and attachment

- [ ] **More attachment modes** (partial, M). Ten exist. Missing: a
  datum on another datum (a plane offset or turned from a datum plane or a
  coordinate system's plane), another body's origin planes as the frame, a
  line normal to a face, a line tangent to an edge, planes along a curve,
  a plane through a line and a point, a point where a line meets a plane or
  two lines cross, and sketch vertices and lines as references
  (`core_document/src/datum.rs`, `wb_part/src/datum_panel.rs`).
- [ ] **Attachment offset tilt** (partial, S). The offset turns only about
  the normal.
- [ ] **Borrowed geometry options** (partial, M). An offset of the
  borrowed geometry, a face from borrowed closed edges, a whole solid as
  reference only (`borrow.rs`).

### Features

- [ ] **Datum and base planes as targets** (partial, S). Up to a datum,
  base or coordinate system plane (`ExtrudeMode::UpToFace`).
- [ ] **Extrusion direction** (partial, S). A datum line, a sketch
  line, a base axis, and length measured along the sketch normal for a
  slanted direction (`ExtrudeDirection`).
- [ ] **Start offset** (missing, S). Start the extrusion away from the
  profile plane.
- [ ] **A taper per side** (partial, S). Needs a second taper in the
  kernel's extrude.
- [ ] **Revolution and helix about the base axes** (partial, S), and a
  helix about the sketch normal.
- [ ] **Loft to a point, faces as sections** (partial, M).
- [ ] **Pipe along solid or borrowed edges, a face as the profile, a point at
  the end** (partial, M).
- [ ] **Fixed pipe orientation** (partial, S). The section keeps its
  orientation in space.
- [ ] **Attachable primitives** (partial, M). Placed on a face, datum
  or edge and following it (`DatumAttachment` on `PartFeature::Primitive`).
- [ ] **Primitive parameters** (partial, S). The ellipsoid's angular
  cut-outs and the prism's skew angles.

### Hole

- [ ] **Custom clearance** (partial, S). A number beside the close,
  normal and loose fits (`HoleFit`, `hole_tables.rs`).
- [ ] **More screw seats** (partial, S each). Button head, slotted and
  cross countersunk, low head cap, cap screw with washer, hex head.
- [ ] **Thread length choices** (partial, S). The whole hole depth, or by
  the thread's run-out.

### Dress-ups, patterns and booleans

- [ ] **Mirror plane** (partial, S). A datum plane, a coordinate
  system's plane, a sketch's axis or plane (`MirrorPlane`).
- [ ] **Draft references** (partial, S). A datum or base plane as the
  neutral plane, an edge or datum line as the pull direction.
- [ ] **Thickness modes** (partial, S). Pipe and both sides.
- [ ] **Scale centre by pick** (partial, S).
- [ ] **Several tool bodies in one boolean** (partial, S).

### Body and history

- [ ] **Move a feature to another body** (missing, M).
- [ ] **Duplicate, copy and paste features** (missing, M).
- [ ] **Drag handles in the view** (missing, L). Drag a pad's end or a
  fillet's radius.

### Generators and measuring

- [ ] **Internal (ring) gear** (partial, S), and addendum and dedendum
  coefficients (`wb_sketch/src/generator/gear.rs`).
- [ ] **Shaft loads and stresses** (partial, L).
- [ ] **Measure angles, radii and face areas** (partial, M). The tool
  measures distance only (`app_shell/src/app/input.rs::measure_click`).

## Assembly

### Components and structure

- [ ] **Linked copies of a body** (missing, L). Several placements
  sharing one definition: edit once, every copy follows, and the parts
  list counts them. A body kind in `core_document` referring to another
  body's solid, picked up by `parts.rs`, `solve.rs` and export.
- [ ] **Parts from other printCAD files** (missing, L). Inserted linked,
  with an out-of-date mark, reload and open source; today outside parts
  come in only as frozen STEP, IGES or mesh imports (File › Insert).
- [ ] **Sub-assemblies** (missing, L). Bodies grouped into a component
  that moves as one, or keeps its own joints live inside the parent;
  nested. A group node in `core_document` and a group-aware solver.
- [ ] **Rigid group** (partial, S). Several bodies locked in one
  action, not pairwise fixed joints.
- [ ] **Patterns and mirrors of components** (missing, M). Arrays and
  mirrored copies of bodies in the assembly, built on linked copies.
- [ ] **Insert several copies, place by dragging** (missing, S).
- [ ] **Replace a component** (missing, M), keeping its joints where the
  new faces allow.
- [ ] **Ground the first component** (partial, S), so the free motions
  read true from the start.

### Joints

- [ ] **Joints that follow part edits** (missing, M). Anchors keep face
  and edge names (`FaceRef::name`, `EdgeRef::faces`) and are found again
  on rebuild.
- [ ] **Point anchors and a ball joint** (missing, M). A vertex, a face
  centre, a sphere's centre; three turns free about it.
- [ ] **Distance, parallel, perpendicular and angle on more than flat faces**
  (partial, M). Point to point, point to plane, point to line,
  axis to axis (a centre distance, for gear spacing); edges and axes for
  parallel, perpendicular and angle.
- [ ] **Joints to the origin and datums** (missing, S). The world
  planes and datums as the other end.
- [ ] **Align with offset, drive and limits** (partial, S). The
  cylindrical joint's turn and slide driven or limited.
- [ ] **A turn offset and a flip on every joint** (partial, S), and an
  editable shift for a fixed joint.
- [ ] **Re-pick faces and change the kind** (missing, S). From the
  joint's settings, without deleting it.
- [ ] **Joints drawn in the view** (missing, S). Anchor frames, axes and
  the link between them.
- [ ] **Other bodies faded while picking** (missing, S).
- [ ] **A separate offset per end** (missing, M).
- [ ] **More kinds** (missing, M). Cam and follower, slot, path, width,
  universal joint.
- [ ] **Every joint touching a body listed** (partial, S). Joints where
  the body is the other end are not shown under it.

### Solving and moving

- [ ] **Move handles** (missing, M). Arrows and rings on a body, where
  today an unjointed body moves by numbers.
- [ ] **Redundant joints reported** (partial, M). Consistent but
  overdefined joints flagged.

### Motion

- [ ] **Motion over time** (partial, M). Several joints at once, each a
  formula of time, with start, end and step, a frame scrubber and step
  play; built on `sweep_frames`.
- [ ] **Collisions during a motion** (missing, S). The frames and pairs
  where bodies share material (`collide.rs` measures it for drags).
- [ ] **GIF and frame sequence export** (partial, S). Recording writes an
  animated PNG only.
- [ ] **Traces, speeds, plots** (missing, L).

### Exploded views and states

- [ ] **Saved, stepped exploded views** (partial, M). Per-body moves and
  turns as ordered steps, several named views kept in the document,
  explode lines, animation. Today one radial spread, dropped on close.
- [ ] **Saved assembly states** (missing, M). Placements and visibility to
  return to: a print-in-place hinge folded and open.

### Parts list

- [ ] **Bought parts** (missing, S). Marked apart from printed ones and
  left out of export.
- [ ] **Kept in the document, numbered** (missing, S).
- [ ] **Custom columns** (missing, M). Part number, supplier, any property.
- [ ] **Sub-assembly levels** (missing, low, S, after sub-assemblies).

### Checks and display

- [ ] **Clearance check** (missing, M). Pairs closer than a given gap,
  and where. Needs a minimum distance query in `KernelQueries`, then a mode
  of `interference.rs`.
- [ ] **Interference among the selected bodies from the toolbar** (partial,
  medium, S). The command takes a list; the tool takes every visible body.
- [ ] **Total mass and centre of mass** (partial, S). For tip-over checks.
- [ ] **Isolate, show only the selection** (missing, S).
- [ ] **Configurations that suppress or swap bodies** (partial, M).

### Interchange

- [ ] **STEP export with the assembly's structure** (partial, M).
  Identical bodies as shared instances in a product tree; likely a kernel
  addition.

## Deferred: printing

- [ ] **Printing columns** (missing, S). Volume, filament mass from a
  density, count to print (`parts.rs`).
- [ ] **Nut trap** (missing, S). A hexagonal pocket sized from the
  thread's nut, for captive nuts.
- [ ] **Print layout** (missing, M). Each part of the parts list laid
  flat on its best face and copies packed on the bed, for export or the
  slicer.
