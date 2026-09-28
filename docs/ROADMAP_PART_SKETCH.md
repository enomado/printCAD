# Part Design and Sketcher: what is still to build

What the two workbenches do not do yet, grouped by area. Everything not
listed here is built; see the release notes for what is.

## Part Design

- **Direction by formula:** a custom extrusion direction's components
  are not yet numbers a formula can set.
- **Borrowed faces:** as up-to-shape faces and revolution targets, and
  an existing sketch mapped onto one.
- **Sketches on borrowed faces follow them:** a sketch placed on a face
  another body lends stays where it was put (one on its own body's face
  follows it).
- **Draft and thickness faces by name:** their faces are found by the
  point they were picked at; the rest of the references find theirs by
  name.
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
