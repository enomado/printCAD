# Local kernel operations

Making the kernel's operations cost what an edit touches rather than what
the solid holds. The rebuild already skips the history an edit did not
change (`kernel_ogeom::ChainCache`, `reuse.rs`); this is what is left: the
ops themselves.

## Where the kernel stands

- A boolean splits, classifies and rebuilds every face of both solids, the
  ones the tool never touches too, and sews the whole result again. The
  one shortcut is a bounding-box test over every face pair, each face's box
  worked out afresh on every call.
- A fillet or chamfer applies its blends as whole-solid booleans, one per
  edge piece.
- Refine (`unify_same_domain`) walks every face of the solid; nothing
  narrows it to the faces an op just made.
- The history calls every kept face modified, untouched ones included, so
  printCAD tells faces that came through unchanged by their geometry (face
  naming, about 60 ms an op on a 400-face part; the mesh reuse's keys).

## Kernel work, in order of payoff

- [ ] **A. Local boolean.** Faces whose box misses the tool's and that no
  section crosses pass through as they are: the same face, not split, not
  rebuilt. A connected region of such faces is classified once, by one
  probe, not face by face. Only the region near the tool is sewn again. A
  face tree replaces the all-pairs box test. Cost then follows what the
  tool touches.
- [ ] **B. "Unchanged" in the history.** A face that came through untouched
  is reported as the same face, not as modified.
- [ ] **C. Local refine.** `unify_same_domain` restricted to given faces and
  their neighbours.
- [ ] **D. Fillets in one pass.** All of a fillet's blends applied in one
  boolean, or by replacing faces locally, not one whole-solid boolean per
  edge piece.
- [ ] **E. Face bounds in the model.** Each face's box kept in the model,
  read by the booleans, `tight_bounds` and printCAD alike.

## printCAD work, as each lands

- [ ] With B: face naming carries an unchanged face's names straight
  across; mesh, outline and bounds reuse key on the face itself instead of
  hashing its geometry.
- [ ] With C: the refine after a feature looks only at the faces the
  feature made.
- [ ] A "large solid, small edit" case in `rebuild_bench` (the imported part
  with a pocket cut into it, not a boss beside it), measured before and
  after each item.

## Process

The kernel gap protocol: an ogeom-rs issue for each of A to E, with the
API and its semantics, a repro in ogeom terms, the bench numbers and an
acceptance test (the time follows the touched region, the result the same
as now), and a waiting `#[ignore]` test here where one fits. A and B
first: together they are most of the cost.
