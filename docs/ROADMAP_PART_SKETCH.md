# Part Design and Sketcher: what is still to build

What the two workbenches do not do yet, grouped by area. Everything not
listed here is built; see the release notes for what is.

## Part Design

### Pad, Pocket and Revolution

- **Revolution up to a curved face:** a revolution stops on a flat face
  whose plane holds its axis; any other target waits on the kernel
  (ogeom-rs#73).
- **Direction by formula:** a custom extrusion direction's components
  are not yet numbers a formula can set.

### Loft, Pipe and Helix

- **Pipe orientation:** fixed and Frenet only; to add a second guide
  path (auxiliary) and a binormal direction.
- **Pipe corners:** how the section turns a sharp corner of the path
  (transformed, right, rounded).
- **Multisection pipe:** several sections along one path.
- **Flat spiral:** a helix of height 0 (growth per turn) is refused
  today.
- **Subtractive helix, keep inside:** keep the intersection instead of
  cutting.

### Patterns

- **Pattern axis:** a reference edge or datum line; a sketch's own axes.
- **Uneven spacing:** a spacing per occurrence.
- **Polar by step:** an angle between occurrences, beside the overall
  angle.

### Dress-ups

- **Tangent chains:** picking an edge takes the edges tangent to it.
- **Thickness joins:** arc or intersection where the walls meet.

### Datums

- **Attachment modes:** a datum attaches to a base plane or a flat face
  today. To add: tangent to a curved face at a point, through three
  points, normal to an edge, along an edge, through two points, the
  intersection of two planes, a curve's centre of curvature, a shape's
  centre of mass and inertia axes.

### References across bodies

- **Borrowed geometry:** a body using another body's faces, edges or
  sketch (a hole through two bodies, one master sketch driving several),
  live or frozen.

### Generators

- **Ring gears:** internal involute gears.
- **Undercut:** a small pinion's flank cut back as a cutter leaves it.
- **Keyways:** a keyway on a shaft or a bore.
- **Generator task panel:** generators are edited in the sketcher's
  panel today.

## Sketcher

### Drawing

- **Trimming conics:** parabola and hyperbola arcs split and join, but do
  not trim yet.

### Editing aids

- **Internal geometry:** an ellipse's axes and foci, a B-spline's control
  polygon, shown and hidden as construction.
- **Intersection references:** another body's geometry cut by the sketch
  plane, beside projected edges.
- **Section view:** everything in front of the sketch plane clipped while
  editing, kept per sketch.
- **Parked constraints:** moving constraint symbols to a second layer to
  declutter.
- **Constraint list filters:** geometric, dimensional, named, reference,
  selected, related to the selection.
- **Remove axis alignment:** turning horizontal and vertical constraints
  into parallel and perpendicular ones so a group rotates as a whole.
