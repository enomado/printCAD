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

- [ ] **Ellipse ratio above one** (S). A dragged minor radius stops at the
  major one.

## Waiting on the kernel

Fixed in the kernel's own repository, not yet in a release printCAD
builds against. When one is released: bump `ogeom` in the workspace
`Cargo.toml`, take the `#[ignore]` off its tests, run the suite.

- [ ] **A void written the wrong way out is read inside out**
  (ogeom-rs#102). Some exporters write a solid's cavities reversed twice;
  the reader should orient a void by its geometry. Twenty-two parts of the
  Voron Doom 350 assembly read as broken because of it. Test:
  `kernel_ogeom/tests/shape_health.rs`
  `a_step_void_written_the_wrong_way_out_imports_as_a_void`.
- [ ] **The repair moves a placed void out of its solid** (ogeom-rs#101).
  `fix_shape` turns an inside-out face without its placement. Test:
  `a_located_void_inside_out_is_turned_where_it_stands`.
- [ ] **STEP export drops a solid's voids** (ogeom-rs#103). Only a solid's
  first shell is written, so an internal cavity is lost on export. Test:
  `a_void_survives_a_step_export`.

Not yet filed:

- [ ] **A pad on a converted solid that was not refined** (M). Fusing a
  pad into a mesh converted as it is (its curved areas still facets)
  fails where the pad's edges run within micrometres of the facets'. The
  app says to refine the body first (a FACETED badge, the conversion's
  log line, and the failed feature's task); the kernel's owner is working
  on the boolean itself.

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

## Workbench store

- [ ] **Who may list a package** (S, to decide). Anyone can propose an
  entry, and whoever adds it must be among its maintainers; nothing yet
  ties them to the package's repository. Options: the check notes when
  the submitter is not the repository's owner, for the reviewer; or the
  owner must be among the maintainers.

## Printing

Features that matter only for printing, for once the modelling is
complete.

- [ ] **Printing columns** (S). Volume, filament mass from a density, and
  the count to print in the parts list (`parts.rs`).
- [ ] **Nut trap** (S). A hexagonal pocket sized from the thread's nut,
  for captive nuts.
- [ ] **Print layout** (M). Each part of the parts list laid flat on its
  best face and copies packed on the bed, for export or the slicer.
