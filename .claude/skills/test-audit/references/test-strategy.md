# Test strategy

Use this before you add tests for a feature, and during an audit when you
decide what a test is worth. It turns risks into one decision per risk and
per affected test.

## 1. Risk map

1. State the protected outcome, the actors and the acceptance criteria.
   Stop with `BLOCKED` when there is no concrete behavior to protect.
2. Trace each critical flow from the gesture or command through the op
   and the build to a durable or user-visible outcome (the file, the
   tree, the view, an export).
3. Mark the behavior that matters most here: a document that loads back
   as saved, undo that restores exactly, replay that matches the live
   edit, a solid the kernel builds the same at any thread count, a
   package or an agent held to its grants, a file the user already has
   left untouched.
4. List plausible defect classes: wrong success, rejected valid input,
   accepted invalid input, boundary error (empty profile, zero length,
   hidden body, mesh body, linked copy), partial failure, duplicate,
   order, cancellation, race between a plan and a build, a stale cache,
   compatibility drift in a persisted type.
5. Drop behavior the kernel or a crate already guarantees and states no
   product outcome depends on. Rank the rest qualitatively; do not invent
   numbers.

## 2. One action per risk and per affected test

| Action | Use when |
|---|---|
| `KEEP` | Trusted, unique proof is still valid |
| `ADD` | A material risk has no proof |
| `UPDATE` | The intent is valuable but the boundary, setup or oracle changed |
| `MERGE` | Proof can be consolidated without losing a scenario or failure localization |
| `DELETE` | The proof is obsolete, duplicate, trivial or untrustworthy |
| `NO_TEST` | Another control or an accepted residual risk covers it; name which |

The action is separate from the run state (`PASS`, `FAIL`, `BLOCKED`,
`UNPROVEN`, `IGNORED`). A `kernel:` ignore is a run state, never an
action, and stays visible with its issue number; it is never a silent
pass. Zero selected tests prove nothing.

## 3. Deletion and merge need proof

- Delete or merge only when the basis is obsolete, or other evidence
  covers every still-required behavior and failure mode with equal or
  better trust.
- A regression guard for a fixed incident stays. So does the only proof
  of a rare critical edge.
- Coverage shows execution, not proof. A slow test that uniquely guards a
  critical journey is not low-value; a fast test that proves a crate's or
  the kernel's own behavior is.
- Test-count, pass rate and raw coverage are not quality targets.

## 4. Level, oracle and gate

- One owner per contract, at the strongest boundary that is cheap and
  deterministic enough. Another level needs its own distinct risk.
- The oracle is independent of the code under test: a returned contract,
  durable state (a file written and read back), an emitted op, a
  geometric property, or a golden artifact (the bundled STEP fixture).
  Never recompute the expected value with the implementation's own logic.
- Fakes must not bypass the boundary the test claims to prove.
- Control randomness, locale, thread count and order where behavior
  depends on them (`map_ordered` output is identical at any thread count;
  `tests/step_import.rs` asserts it).
- Say which gate owns the test (`ci.yml`'s `check` job, its `platforms`
  job, or local only) and its measured run cost.

## 5. Output

For each decision: basis, protected outcome, risk, existing test or gap,
action, level, oracle, gate, and run state. Verdict: `READY`,
`INCONCLUSIVE` (name the next evidence step) or `BLOCKED`.
