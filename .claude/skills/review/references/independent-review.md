# Independent review panel

The lead scopes the review, picks the lenses, checks each claim and issues
the verdict. Lenses are evidence questions, not personas.

## Budget

- At most three lenses per round.
- At most two rounds for one task and one scope: one initial round and,
  only when corrections or unresolved evidence need it, one selective
  follow-up. Never a third round. After the budget, the lead checks
  directly and carries open evidence into the verdict.
- Pick only lenses whose question can change the verdict. Never launch a
  lens to fill a quota, and never defer an obviously required lens to
  round two.
- A follow-up reruns no lens only because it ran before. Pick the smallest
  set from the correction diff and the open findings, or none.

## Lenses

| Lens | Question |
|---|---|
| Facts | What changed, which outcomes and paths does it touch, and what evidence is missing? |
| Caution | How can this regress, corrupt a document, escape the sandbox, or fail at an edge or on partial failure? |
| Simplicity | Is this the smallest sufficient diff and the simplest correct algorithm, with superseded code gone? |

| Specialist | Trigger | Focus |
|---|---|---|
| Document and undo | `core_document`, any new op, persistence, the server | One op per edit, inverses, replay, barriers, serde defaults, old files and op logs |
| Kernel and geometry | `kernel_ogeom`, `kernel_api`, `SolidOp` changes, build plans | `ChainError` attribution, `ChainCache` fingerprints, face naming, probes, the gap protocol, `#[ignore = "kernel: …"]` |
| Renderer and loop | `render_vk`, `gfx.rs`, `frame.rs`, camera | Fingerprint completeness, drop order, no waits, on-demand frames, validation layers |
| Workbench seam and packages | `Workbench` trait, `bench_api`, `wb_wasm`, `sdk/` | Host names no bench, `HookOutcome` plumbing, suspend and resume, `Access`, grants, traps, package-facing type changes and the example workbench repository |
| Scripts and agents | `scripting`, `agents`, `mcp.rs`, `agent_context.rs` | Command specs, access declarations, approvals, one undo step per call, tab pinning |
| UI and design system | `ui/`, `ui_kit` | Tokens only, `UiLayer` seeding, two-phase dispatch, menus as egui buttons, planned placeholders |
| Tests and oracles | Changed tests or untested material behaviour | Oracle strength, geometric properties over implementation details, owner boundary (see test-audit) |
| Performance | Import, rebuild, frame cost, big assemblies | Measured with the benches and the frame log, `--release`, no nested `map_ordered` |
| Platforms | `local_ipc`, paths, spawning, the Vulkan loader, `.gitattributes` | Windows and macOS assumptions nobody has run on real hardware |

## Packet

Every reviewer gets the same frozen packet: the task and acceptance
criteria, base and head (or the working-tree state), changed and excluded
scope, non-goals, `CLAUDE.md`, the risk tier, the allowed read-only
commands, exactly one lens, and the result schema below. Never include
provisional or sibling findings.

Reviewers may read, search and run non-mutating checks (`cargo test`,
`cargo clippy`, the lints). They may not edit tracked files, commit, push,
file issues, or launch nested reviewers. Retry a failed lens once, in the
same round, only when a concrete cause changed.

## Result schema

Each reviewer returns: coverage (files and paths read), candidate findings
(`file:line`, rule slug, failure scenario, change-causal evidence, smallest
fix), hypotheses it rejected, and open questions. "No findings" is valid.
The lead verifies every candidate against the code before it enters the
report.
