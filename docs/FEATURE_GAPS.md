# What is left to build

What the Sketcher, Part Design and Assembly workbenches still lack, from a
feature-by-feature check against a mature parametric CAD. Only open items
are listed; what is built is in the release notes. Sizes are rough effort
(S: a day or less, M: a few days, L: a week or more).

A kernel capability that is missing is filed on the kernel's repository
and noted on its item, and the rest of the item is built around it.

## Part Design

- [ ] **Direction by formula** (S). A custom extrusion direction's
  components are not numbers a formula can set.
- [ ] **Borrowed faces in more places** (M). As up-to-shape faces and
  revolution targets, and an existing sketch mapped onto one.
- [ ] **Tangent-chain chamfers by two distances** (S). The reference face
  may change sides along the chain.
- [ ] **Thickness pipe mode** (S). How it should differ from the kernel's
  hollowing, which already ends walls flush with the openings, is still
  to be settled.
- [ ] **Generators** (M). Undercut on small pinions, keyways, and a task
  panel of their own.

## Sketcher

- [ ] **Related constraints across a corner** (S). The constraint list's
  "related" filter does not reach constraints through a shared corner.
- [ ] **Ellipse ratio above one** (S). A dragged minor radius stops at the
  major one.

## Deferred: printing

Features that matter only for printing, for once the application is
complete.

- [ ] **Printing columns** (S). Volume, filament mass from a density,
  count to print in the parts list (`parts.rs`).
- [ ] **Nut trap** (S). A hexagonal pocket sized from the thread's nut,
  for captive nuts.
- [ ] **Print layout** (M). Each part of the parts list laid flat on its
  best face and copies packed on the bed, for export or the slicer.
