# Solver problem and migration contract

This specifies milestone 1 of [RFC 0003](0003-standalone-sketch-solver.md).
The first copy of the library is `crates/sketch_solver`. Its modules are
`problem`, `compile`, `curves`, `spline`, `residual`, `solve`, `freedom`, and
`diagnosis`. The module paths, rather than re-exports, identify public types.

## Identity and ordering

`PointId(u128)`, `CurveId(u128)` and `ConstraintId(u128)` are distinct
caller-supplied identities. `VariableIndex(usize)` and `EquationIndex(usize)`
address numerical arrays. None implements `Deref` or generates identities.
The adapter maps a UUID through `as_u128()` and reconstructs it through
`Uuid::from_u128()`. The inverse is exact, including UUID ordering used to
order ellipse focus rules. Point and curve identities remain separate even
if they carry the same number; duplicate identities within a domain fail
validation. `ItemId` explicitly distinguishes a point from a curve.

Origin and coordinate axes use `PointReference::Origin` and
`CurveReference::{XAxis,YAxis}`; ordinary references carry a typed ID.
They are immutable, contribute no free variables, and are inserted only
when an active relation uses any reference. All three reference points
precede shape variables, as in the application compiler.

`Problem.geometry: Vec<Geometry>` preserves the application's interleaving
of points and curves. The compiler first visits shape-bearing curves,
then geometry again for points and radii, then constraints for contact
parameters. It never sorts geometry, constraints, residuals or summations
through a lookup map. Maps provide lookup only. Focus rules retain their
explicit curve-ID sort. IDs in the result always name the caller's input.

## Public records

`problem::Problem` owns:

- `geometry: Vec<Geometry>`: points with `[f64; 2]` coordinates; lines
  with typed endpoints; circles and arcs with centre, radius and arc
  endpoints; ellipses with centre, major-vector and initial minor radius,
  optional arc endpoints; hyperbolas/parabolas with centre or vertex,
  axis, minor and endpoints; splines with typed control points, degree,
  knots, rational weights and periodicity.
- `constraints: Vec<Constraint>`: ordered `ConstraintId` plus one of the
  42 semantic `Relation` variants in the equation inventory. Only active,
  driving application relations enter this vector. Internal alignment
  includes a typed role and item. Generic gap/contact operands use typed
  item references, rather than an untyped ID with guessed interpretation.
- `external: Vec<ItemId>`: explicitly fixed geometry and its points;
  external shape degrees of freedom never count as free.
- `held_points: Vec<PointId>`: the current gesture's held points, used in
  focus classification as well as variable pinning.
- `application_held_points: Vec<PointId>`: fit-spline controls and text
  outlines that the application updates after applying a result. They
  have no font, image, fit-point or document dependencies in the core.
- `settings: Settings`: effective positive finite tolerance, positive
  iteration limit, LM retry limit, damping limits and floor, finite
  difference step, direction floor, step cap, rank tolerance and diagnosis
  limit. `Settings::default()` uses the current numerical constants;
  replay records every value. The adapter resolves the application's
  zero-iteration and nonpositive-tolerance fallback before construction.

`solve::solve(&Problem) -> Result<Solution, InputError>` returns:

- `Outcome`: `Converged`, `NotConverged` or `NothingToSolve`;
  the iteration count is explicit even for an unsuccessful attempt.
- Ordered solved point positions, curve radii and participating shapes,
  plus solved auxiliary parameters. Untouched geometry remains addressable
  by the same IDs. Held values stay unchanged.
- The effective convergence threshold, maximum residual and ordered
  residual rows, each with `EquationIndex`, value, unit and origin.
- A compilation trace: initial variable values, free indices, contact
  parameters, selected contact/gap branches, ray endpoint ordering and
  the exact ordered residual specifications. This trace supports replay
  of iteration without reselecting branches.
- Per-constraint compilation reports, including `NoEquations` when a
  valid application relation naturally contributes no rows (control
  polygon, derived focus or a pitch with no step).

`freedom::degrees_of_freedom(&Problem) -> Result<Freedom, InputError>`
returns the free-variable count, Jacobian rank, uncompiled free-shape
count and their difference. It compiles circular-arc endpoint rules even
when ordinary solve has no user residual. A hidden parabola minor slot is
pinned. `Freedom` must not substitute for the application's nudge-based
`free_points` policy.

`diagnosis::diagnose(&Problem) -> Result<Diagnosis, InputError>` returns
DoF, ordered redundant/conflicting `ConstraintId`s and `analyzed`. It
counts semantic constraints, including valid zero-row constraints, towards
the limit (60 by default). Each exclusion probe recompiles shape variables
and derived foci from the remaining constraints and the probe's held set.
Numerical diagnosis has no callback into application post-processing;
the corpus must expose any difference this boundary introduces.

`problem::InputError` identifies the record and reason: duplicate ID,
missing or wrong-kind reference, nonfinite number, unsupported geometry
or relation domain, invalid settings, or invalid spline data. Finite
zero-length lines and coincident centres remain admissible and use the
existing numerical floors; inability to solve them is a numerical outcome.
Unsupported fixture schemas are rejected by the replay reader before
constructing a problem. No malformed input silently loses a relation.

## Adapter contract

`prepare` builds the ordered problem and `IdMap`, plus a report of every
excluded application constraint: inactive, reference dimension, missing
geometry or unsupported operand kind. Partial composite relations report
which members were excluded; they cannot vanish behind a successful solve.
An exhaustive match over `ConstraintKind` enforces the 42-variant mapping.

`apply` writes f32 positions and radii, normalizes ellipse axes in the
current f32 operation order, renames major/minor dimensions and internal
roles, reverses affected axis line endpoints, and places derived foci.
It updates `unsolved` and fully constrained flags, refits splines, then
follows text outlines. A failed held attempt restores geometry and runs
an ordinary solve from the pre-attempt gesture position. Constraint role
changes and the existing retry order are checked by application fixtures.

The adapter promotes stored numbers without changing their precision.
In particular, ellipse minor radii are computed with the existing f32
length and multiplication before promotion, and pitch directions are
normalized in f32 before promotion. The core receives these initial
values explicitly; it does not know f32 storage or normalize them again.

## Domains and compatibility

Lengths share the caller's unit; angular quantities are radians.
`DistanceX/Y >= 0` measure absolute separation, negative values signed
separation. A missing second point measures the first from zero.
`Angle`, `AngleToAxis`, `AngleAtPoint`, `AngleThreePoints` and `PolarPitch`
are periodic directions with current wrapped errors. `ArcAngle` and
`ArcLength` use a CCW sweep in `(0, 2π]`, with equal endpoints denoting a
full turn. New standalone inputs outside this sweep domain fail explicitly;
the adapter reports unsupported legacy domains without silently replacing
them. This does not introduce a signed-sweep or multi-turn arc model.

Tangency selects the initial external/internal branch once; equal errors
select external. A shared line/arc or arc/arc endpoint uses its endpoint
tangent relation. Gap inside/nested selection and the refraction ray's
near/far endpoints are also fixed at compilation. Curve contacts add
nearest parameters from the current coarse/fine searches. Curve lengths
keep 128 chord intervals; spline basis normalization retains its current
degree, knots and rational/periodic evaluation contracts.

## Captured corpus and checks

`wb_sketch/src/solver/migration.rs` builds 24 deterministic input scenes;
`migration.json` stores the complete inputs and application answers.
It records ordinary/held/freedom compilation, held-attempt outcome,
final outcome, iterations when available, stored geometry and constraint
roles, DoF and diagnosis coverage/IDs. A fixed floating tolerance of
`1e-6 * max(1, |expected|)` is declared before refactoring. IDs, topology,
enum tags, order, integer metadata and iteration counts compare exactly.
This is migration equivalence, not a mathematical oracle.

Separate assertions check line lengths, fixed and held points, ordinary
fallback, contact branch distances and translated text outlines. Controlled
changes of a coordinate, DoF and iteration count must fail comparison.
Existing solver and internal-geometry tests supply the broader independent
geometric assertions listed in the equation inventory.

Capture is an explicit test-only maintenance action, never a production
fallback. Capture once before changing the solver and review the diff;
do not recapture to make a changed solver pass.

```sh
PRINTCAD_CAPTURE_SOLVER_CORPUS=1 cargo test -p wb_sketch --release \
  --no-default-features --locked --lib \
  solver::migration::captured_application_corpus -- --exact
cargo test -p wb_sketch --release --no-default-features --locked --lib solver::
cargo test -p wb_sketch --release --no-default-features --locked --lib \
  solver::migration::measure_migration_baseline -- --ignored --exact --nocapture
```

The timing test separates compilation and diagnosis. Its solve phase
includes current write-back, refit and text following; milestone 2 adds a
separate pure-iteration measurement. Clean and incremental library-test
builds use an isolated target directory and the same toolchain, profile,
features and lockfile. No speedup is inferred from cached workspace builds.

Captured before numerical refactoring, Rust 1.98, release, locked,
`wb_sketch --no-default-features --lib`: a clean library-test build took
52.151 s; a rebuild triggered by touching only `solver.rs` took 20.845 s.
Five timing trials, each 50 repetitions of the 24-case corpus, gave these
medians (microseconds per whole corpus): compilation 77.076, solve including
input clone/application updates 226.028, diagnosis 1036.967. Ranges were
69.222–93.797, 221.468–241.323 and 1014.077–1066.131 respectively. These
measurements describe this machine and test harness, not a CI budget or
a standalone-core speedup. The engine is the solver at `566bf70` with
test-only observation and corpus modules attached; its numerical code is
unchanged.
