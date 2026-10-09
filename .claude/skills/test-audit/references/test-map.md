# printCAD test map

The right harness per layer. Each contract has one owner at the strongest
boundary that is cheap and deterministic enough.

| Layer | Owner | Harness |
|---|---|---|
| Solver and geometry math (sketch constraints, 2D intersections, joints, textures) | Unit test beside the code asserting a geometric property | `#[cfg(test)]` in the module; a property test on the invariant where one exists (add the dev-dependency in the commit that adds the first property) |
| Document ops, inverses and replay | Replay the recorded ops and compare documents | `crates/core_document/tests/op_replay.rs`, the `undo.rs` tests (they pin that `Document::clone` keeps the sidecars) |
| Sketcher interaction (tools, snapping, drags) | Viewport-pixel clicks through `on_input` | the `Harness` in `crates/workbenches/wb_sketch/tests/interaction.rs` |
| Sketch to feature to solid | The real benches building a real solid | `crates/kernel_ogeom/tests/design_stack.rs`; a script end to end in `scripted_part.rs` |
| Kernel ops and import | The adapter against the kernel on the bundled fixture | `crates/kernel_ogeom/tests/` with `tests/data/box_native.step`; `PRINTCAD_TEST_STEP_FILE` for a richer model; gaps under `#[ignore = "kernel: …"]` |
| The document server | The real spawned daemon over its socket | `crates/doc_server/tests/daemon.rs` |
| Workbench packages | The SDK packages built by cargo and run through the host | `cargo test -p wb_wasm` (`tests/packages.rs`, builds `sdk/`); the `rogue` package misbehaves on request |
| Generated artifacts and indexes | A test that fails on drift | the icon table test in `ui_kit/src/icon.rs`, the command reference in `docs/SCRIPTING.md` (`PRINTCAD_WRITE_DOCS=1` rewrites), the release notes entry for the running version |
| The host-bench seam | A lint over the host's sources | `crates/app_shell/src/app/seam_lint.rs` and the CI grep |
| Rendering and the whole app | A headless run with the bench hooks | `PRINTCAD_BENCH_SKETCH=pad PRINTCAD_EXIT_AFTER_MS=…`, `PRINTCAD_BENCH_PICTURE`, `grim`; watch for `printcad.gpu` output; on a small STEP, never a huge assembly |
| Windows and macOS | CI only | the `platforms` job of `ci.yml`; nobody has run the app there on real hardware |

## Rules

- A user's file is never a fixture in place: copy it to `/tmp` first. A
  test helper once deleted a user's STL.
- Quality signal is whether the owning test goes red under a deliberate
  mutation of the owner, not line coverage. When a keeper's value is in
  doubt, make one hand mutation, confirm the red, restore the source byte
  for byte.
- No real timers or sleeps. Wait on a condition with a deadline (see
  systematic-debugging).
- `cargo test -p wb_wasm` builds the SDK with cargo and is slow; keep its
  cases to what only the real host can prove.
- Time nothing in a debug build: our crates are unoptimized there.
- Flake fixes follow ci-iterate: measure the rate from `gh run` history,
  reproduce, fix the cause, prove with an N-run loop.
