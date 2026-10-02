# Roadmap

Everything still open, in one place: what the workbenches lack, what waits
on the kernel, and the larger work ahead. What is built is in the release
notes (`crates/app_shell/RELEASE_NOTES.md`). Sizes are rough effort (S: a
day or less, M: a few days, L: a week or more).

A kernel capability that is missing is filed on the
[kernel's repository](https://github.com/gilbertorconde/ogeom-rs/issues),
noted on its item here, and the rest of the item is built around it, with
a test marked `#[ignore = "kernel: … (ogeom-rs#N)"]` that passes once the
kernel has it.

## Design

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

- [ ] **An ellipse with its foci shown, dragged through a circle** (M). A
  minor radius dragged past the major makes it the major, but not while
  the ellipse shows its foci: a focus stands √(a² − b²) from the centre,
  which changes infinitely fast as the radii meet, so the solve grows both
  radii into a circle instead. Wants a focus constraint with no such point
  (along · across = 0 and along² − across² = a² − b²) and one keeping the
  two foci on opposite sides as they cross.

## Waiting on the kernel

Gaps in the kernel printCAD runs into. When a fix is
released: bump `ogeom` in the workspace `Cargo.toml`, take the `#[ignore]`
off its tests, run the suite.

Not yet filed:

- [ ] **A pad on a converted solid that was not refined** (M). Fusing a
  pad into a mesh converted as it is (its curved areas still facets)
  fails where the pad's edges run within micrometres of the facets'. The
  app says to refine the body first (a FACETED badge, the conversion's
  log line, and the failed feature's task); the kernel's owner is working
  on the boolean itself.

## Surfaces

What the Surface workbench waits on in the kernel; each case fails with the
kernel's reason, and a test marked `#[ignore]` in
`kernel_ogeom/tests/kernel/surface_ops.rs` passes once it lands.

- [x] Sew, sew across a gap, trim and split sheets, N-sided and tangent
  fills, surface lofts and sweeps, blend, thicken and offset, extend, STEP
  export of sheets, fillets on sheets (ogeom-rs#104 to #114, ogeom 0.7.0).
- [ ] **Placed edges** (ogeom-rs#116). Fill, blend and extend refuse an
  extruded surface's far edges, which are placed, and a sheet cannot be
  baked.
- [ ] **Fill sides meeting at a point** (ogeom-rs#117). Edges of separate
  surfaces whose ends meet do not close a fill's loop.
- [ ] **Ruled surfaces and lofts between different piece counts**
  (ogeom-rs#118).
- [ ] **Offset of free-form faces** (ogeom-rs#119). The fit must reach
  1e-6, which a fitted surface misses.

And in printCAD:

- [ ] Curvature analysis (in ogeom 0.7.0, ogeom-rs#115): curvature and
  zebra displays, and G2 in Check continuity.
- [ ] Fillets between two separate surfaces (`fillet_faces`, in ogeom
  0.7.0), guide curves for lofts and two-rail sweeps (in ogeom 0.7.0).
- [ ] Picked edges of other bodies as curves (the body's own only today).
- [ ] A pick of a face's edges by clicking the face, as external geometry
  does in the sketcher.

## Faster kernel operations

The rebuild skips the history an edit did not change; the operations
themselves still cost what the whole solid holds, not what an edit
touches. The kernel work, in order of payoff:

- [ ] **A. Local boolean** (L). Faces whose box misses the tool's and that
  no section crosses pass through as they are, not split, classified or
  rebuilt; a connected region of them is classified once, by one probe;
  only the region near the tool is sewn again; a face tree replaces the
  all-pairs box test.
- [ ] **B. "Unchanged" in the history** (S). A face that came through
  untouched is reported as the same face, not as modified.
- [ ] **C. Local refine** (S). `unify_same_domain` restricted to given
  faces and their neighbours.
- [ ] **D. Fillets in one pass** (M). All of a fillet's blends applied in
  one boolean, or by replacing faces locally, not one whole-solid boolean
  per edge piece.
- [ ] **E. Face bounds in the model** (S). Each face's box kept in the
  model, read by the booleans, `tight_bounds` and printCAD alike.

And in printCAD, as each lands:

- [ ] With B: face naming carries an unchanged face's names straight
  across, about 60 ms an op on a 400-face part, and mesh, outline and
  bounds reuse key on the face itself instead of hashing its geometry.
- [ ] With C: the refine after a feature looks only at the faces it made.
- [ ] A "large solid, small edit" case in `rebuild_bench` (an imported
  part with a pocket cut into it), measured before and after each item.

Each goes to the kernel's repository as an issue with its API, a repro, the
bench numbers and an acceptance test (the time follows the touched region,
the result the same as now). A and B first: together they are most of the
cost.

## Printing

Features that matter only for printing, for once the modelling is
complete.

- [ ] **Printing columns** (S). Volume, filament mass from a density, and
  the count to print in the parts list (`parts.rs`).
- [ ] **Nut trap** (S). A hexagonal pocket sized from the thread's nut,
  for captive nuts.
- [ ] **Print layout** (M). Each part of the parts list laid flat on its
  best face and copies packed on the bed, for export or the slicer.
