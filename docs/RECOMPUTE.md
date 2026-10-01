# Faster rebuilds

How printCAD avoids rebuilding more than an edit needs. Every technique here
is automatic: nothing asks the user what to rebuild.

## Where it starts

- Dirty flags follow the feature graph: an edit marks the features that
  depend on it, and only their bodies rebuild.
- A body has one build out at a time; a plan made meanwhile waits, a newer
  one replacing it, so a drag builds only the latest shape.
- The kernel worker keeps the last few solids per body by everything they
  were built from, so undo, redo and moving the tip find them built.
- An open task previews its feature's tool over the body before it.

What it lacks: every build replays the body's whole history from its first
feature, meshes the whole result, waits for a build nobody needs any more,
meshes at full detail while a value is dragged, and builds one body at a
time.

## Plan

Each item is one milestone and one commit.

- [x] **0. Measure.** `crates/kernel_ogeom/examples/rebuild_bench.rs` times
  each feature of a long history and the meshing, so the items below are
  checked against numbers.
- [x] **1. Resume from the edited feature.** The chain keeps its state
  (model, solid, face names, pattern tools) at the start of the feature that
  changed last, keyed by everything before it. The next build whose history
  agrees up to there resumes from it instead of from the first feature.
- [x] **2. Drop a build nobody needs.** A newer plan for a body whose build
  is running cancels that build; the newer one goes at once. A build still
  in the queue is skipped; a running one stops unless it is past half its
  body's usual build time, so a stream of edits still shows a shape now and
  then.
- [x] **3. Coarse while dragging, fine at rest.** Builds that follow one
  another faster than they land mesh at a coarser tolerance; once the edits
  settle, the body is built once more at full detail (from the state item 1
  kept, so only the meshing is repeated).
- [x] **4. Stop when nothing changed.** When a rebuilt feature makes the
  same solid as before, everything after it is the same too: the previous
  result is used. Solids are compared by their geometry (every vertex,
  points along every edge, a point and normal inside every face), not by
  their snapshots, which also record parameter ranges; the comparison is
  made only where the ops after the edited one took more than twice as long
  as writing a snapshot.
- [ ] **5. The edited feature first.** While a task edits a feature with
  features after it, the body is shown at that feature first, then the rest
  of its history is built.
- [ ] **6. Mesh only what changed.** Faces a build left as they were keep
  their meshes from the build before; only new and changed faces are meshed.
- [ ] **7. Bodies in parallel.** Builds of different bodies run on several
  threads; a body still has one build at a time.
- 8. Local kernel operations (booleans and dress-ups that work only near
  the change) are kernel work, for a later pass.

## Measurements

`rebuild_bench` (release build): a 160 x 100 mm plate, 12 pockets, 6 bosses,
a slot patterned 6 times, a fillet round the bottom and a last boss, 45
kernel ops. Each line is one build after the change it names.

| Build | Before | 1 |
|---|---|---|
| From scratch | 577 ms | 578 ms |
| Last feature edited | 596 ms | 59 ms |
| Last feature edited again | 563 ms | 53 ms |
| A middle feature edited | 581 ms | 588 ms |
| The same one again | 571 ms | 447 ms |
| Nothing changed (worker cache aside) | 585 ms | 26 ms |

Every build replays all 45 ops whatever changed. Of a build: the fillet
233 ms, the pattern 43 ms, the pads, pockets and their refines 2 to 24 ms
each and growing with the solid, meshing 5 ms and the snapshot 6 ms (a
flat part: meshing grows with curved faces).

After 1, a build from scratch also keeps the state at its last feature, so
the first edit there resumes too; the first edit of a middle feature still
replays everything (nothing was kept there), the next resumes at it and
pays only for what follows, here the pattern and the fillet. Resuming
costs a clone of the kept model, 2 to 3 ms on this part.

After 3, a dragged body meshes coarse (here 1952 triangles in place of
3282) and, once it settles, is built at full detail from the state kept at
the end of the chain: 31 ms, the meshing and the snapshot alone.
