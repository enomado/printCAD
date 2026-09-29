# Part Design and Sketcher: what is still to build

What the two workbenches do not do yet, grouped by area. Everything not
listed here is built; see the release notes for what is.

## Part Design

- **Direction by formula:** a custom extrusion direction's components
  are not yet numbers a formula can set.
- **Borrowed faces:** as up-to-shape faces and revolution targets, and
  an existing sketch mapped onto one.
- **Tangent-chain chamfers by two distances:** the reference face may
  change sides along the chain.
- **Generators:** undercut on small pinions, keyways, and a task panel of
  their own.

## Sketcher

- **Related constraints across a corner:** the constraint list's
  "related" filter does not reach constraints through a shared corner.
- **Ellipse ratio above one:** a dragged minor radius stops at the major
  one.
