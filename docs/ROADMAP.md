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

## Faster kernel operations

The rebuild skips the history an edit did not change; the operations
themselves still cost what the whole solid holds, not what an edit
touches. The kernel work, in order of payoff (`rebuild_bench`'s holed plates: one
hole added to a 582-face plate costs 1.1 s on ogeom 0.9.8, of which the
boolean 0.14 s, meshing 0.15 s, the snapshot 0.02 s and resuming the
chain from its kept state 0.7 s, nearly all of it cloning the model):

- [ ] **A. Local boolean** (L, ogeom-rs#134, on the kernel's master after 0.9.8: the
  corner hole's boolean 0.14 s to 0.02 s). Faces whose box misses the tool's and that
  no section crosses pass through as they are, not split, classified or
  rebuilt; a connected region of them is classified once, by one probe;
  only the region near the tool is sewn again; a face tree replaces the
  all-pairs box test.
- [ ] **D. Fillets in one pass** (M, ogeom-rs#135). All of a fillet's blends applied in
  one boolean, or by replacing faces locally, not one whole-solid boolean
  per edge piece.
- [ ] **G. Compacting a model in place** (S, ogeom-rs#141). A chain's
  model keeps every intermediate result: 2.5 million nodes behind a
  582-face plate, and cloning it to resume a build costs 0.6 to 2.8 s.
  Wants the unreachable dropped with the handles printCAD holds kept.

And in printCAD, as each lands:

- [ ] With the history's exact copies (`History::copy_of`, in the
  kernel): face naming carries an unchanged face's names straight
  across, about 60 ms an op on a 400-face part, and mesh, outline and
  bounds reuse key on the face itself instead of hashing its geometry.
- [ ] With G: the chain cache keeps its states compacted, and the
  running model is compacted as it grows.

Each goes to the kernel's repository as an issue with its API, a repro, the
bench numbers and an acceptance test (the time follows the touched region,
the result the same as now).
