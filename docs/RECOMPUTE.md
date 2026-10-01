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
- [x] **5. The edited feature first.** While a task edits a feature with
  features after it, the body is shown at that feature first, then the rest
  of its history is built. The task shows the body at its feature anyway;
  each edit builds only that far, and once the edits settle the whole
  history is built (from the state kept at the feature) for when the task
  closes. A task closed before that rebuilds the body.
- [x] **6. Mesh only what changed.** Faces a build left as they were keep
  their meshes from the build before; only new and changed faces are meshed.
  An op rebuilds every face of its solid, so faces are known by their
  geometry (surface, edge curves and ranges, vertices, placement, the
  deflection and their edges' chords), each surface and curve read once
  per meshing. Edges keep their outlines the same way, and the solid's
  bounds are the union of its faces', each kept with its face: measuring a
  curved solid's bounds whole cost more than meshing it.
- [x] **7. Bodies in parallel.** Builds of different bodies run on several
  threads; a body still has one build at a time. Builds go to a pool of two
  to four threads (a quarter of the machine's), sharing the solids kept and
  each body's chain states; imports, repairs and the rest keep their own
  thread. The status bar shows the first job that has something to say,
  and Cancel stops every job running.
- 8. Local kernel operations (booleans and dress-ups that work only near
  the change) are kernel work, for a later pass.

## Measurements

`rebuild_bench` (release build): a 160 x 100 mm plate, 12 pockets, 6 bosses,
a slot patterned 6 times, a fillet round the bottom and a last boss, 45
kernel ops; then an imported part (`drive_frame_upper.step`, 406 faces) as a
body's base with a boss beside it. Each line is one build after the change
it names. `--before` builds as the application did before any of this:
from the first feature every time, the whole history while a task is open,
always at full detail. The two columns were run back to back on a busy
machine (load 8 to 10), so read them against each other; on a quiet
machine the plate builds from scratch in about 580 ms.

| Build | Before | After |
|---|---|---|
| From scratch | 713 ms | 799 ms |
| Last feature edited | 688 ms | 72 ms |
| Last feature edited again | 718 ms | 78 ms |
| A middle feature edited | 743 ms | 739 ms |
| The same one again | 1196 ms | 698 ms |
| Nothing changed (worker cache aside) | 1059 ms | 16 ms |
| Dragged, coarse | 863 ms | 587 ms |
| Settled, full detail | 691 ms | 19 ms |
| Task open on the middle feature, first edit | 1223 ms | 237 ms |
| Task open, next edit | 1040 ms | 40 ms |
| Task settled, the rest of the history | 1097 ms | 510 ms |
| A pocket made through the plate | 1337 ms | 751 ms |
| Deeper, the same hole | 1030 ms | 8 ms |
| Imported part, from scratch | 173 ms | 147 ms |
| Imported part, boss edited | 180 ms | 101 ms |
| Imported part, boss edited again | 169 ms | 91 ms |

What is left: the first edit of a feature nothing was kept at replays the
history before it (the next resumes); what follows an edited feature is
built again unless it makes the same solid; on a large solid, carrying the
faces' names through each op (about 60 ms on the imported part) and
writing the snapshot are now the larger part of a small edit. Item 8 is the
way to make the ops themselves cheaper.
