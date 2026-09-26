# Part Design and Sketcher: what is still to build

What the two workbenches do not do yet, grouped by area. Everything not
listed here is built; see the release notes for what is. Items marked with
a kernel issue are wired in the app and refused cleanly on their feature
until the kernel does them; each has a test that runs once it does.

## Waiting on the kernel

- **Revolution up to a curved face** (ogeom-rs#73): a revolution stops on
  a flat face whose plane holds its axis only.
- **Thread in a primitive's bore** (ogeom-rs#74): a thread of two or more
  turns does not cut into a blind bore the cylinder primitive made.
- **Thickness with arc joins** (ogeom-rs#75).
- **Pipe along a second guide path, or a free binormal** (ogeom-rs#76).
- **Right and round pipe corners on a sharp-cornered path** (ogeom-rs#77).
- **Several sections along one pipe** (ogeom-rs#78).
- **Flat spiral** (ogeom-rs#79): a helix of height 0 growing per turn.
- **Measuring a helix a boolean trimmed** (ogeom-rs#80): about 3% high.
- **A boolean with a face on a helix's axis** (ogeom-rs#81).
- **A pipe starting at the far end of an L path** (ogeom-rs#82).
- **Sections of a plane that misses the solid** (ogeom-rs#83), and
  **sections of faces and shells with no solid** (ogeom-rs#84): the
  sketcher's intersection references of open sheets.

## Part Design

- **Direction by formula:** a custom extrusion direction's components
  are not yet numbers a formula can set.
- **Borrowed faces:** as up-to-shape faces and revolution targets, and
  an existing sketch mapped onto one.
- **Sketches on faces follow them:** a sketch placed on a face (a
  body's own or a borrowed one) stays where it was put.
- **Tangent-chain chamfers by two distances:** the reference face may
  change sides along the chain.
- **Generators:** ring gears, undercut on small pinions, keyways, and a
  task panel of their own.

## Sketcher

- **Trimming conics:** parabola and hyperbola arcs split and join, but do
  not trim yet.
- **Related constraints across a corner:** the constraint list's
  "related" filter does not reach constraints through a shared corner.
- **Ellipse ratio above one:** a dragged minor radius stops at the major
  one.
