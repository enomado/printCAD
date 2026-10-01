---
name: comment-cleanup
description: "Diff-scoped cleanup before review: removes narration and history from comments, keeps the ones that state what the code cannot, and strips slop (quieted warnings, swallowed errors, one-use helpers, shims) from the same hunks. Use before committing, when the comment-rot lint fails, or when asked to deslop or trim comments. Not a correctness review (use review) and not a test-portfolio audit (use test-audit)."
---

# Comment cleanup

Clean only the diff before review. Preserve behavior absolutely.

## The rule

`CLAUDE.md`: comments describe present behaviour, never the change that
produced it. No "used to", "no longer", "since X landed", and no issue,
PR or commit references in comments. The one sanctioned place for a
kernel issue number is the `#[ignore = "kernel: … (ogeom-rs#N)"]` string;
`lint-comment-rot: ignore` on a line opts it out when a reference is
load-bearing.

This codebase explains itself in prose: module docs (`//!`) and comments
that say why a thing is shaped as it is are its style, so there is no
percentage budget. The test is what a comment states, not how many there
are.

## Measure

```bash
node scripts/lint-comment-rot.mjs                 # lines added against origin/master
node scripts/lint-comment-rot.mjs --pedantic      # plus the advisory tier
node scripts/lint-comment-rot.mjs --staged        # what the pre-commit hook sees
node scripts/lint-comment-rot.mjs --all           # the whole tree, what CI runs
```

The gating tier names constructions that cannot describe present
behaviour. The advisory tier fires on correct comments often enough that
it is a review aid, not a rule: read each hit, do not delete on sight.

## Checklist

1. Scope to `git diff origin/master` (or the range the user names). Never
   clean the whole repo in a feature commit.
2. For each added or changed comment, decide one action: KEEP, DELETE,
   UPDATE or MERGE. KEEP only when the comment states what the code
   cannot: rationale, an invariant, a seam contract, a side effect, a
   failure behaviour, a kernel limitation, a platform fact.
3. Delete on sight:
   - narration of what the next line does, and syntax explanation;
   - history: "changed from", "used to", "now", "we", issue or commit
     numbers, dates, decision logs, paths not taken;
   - banners and dividers that restate the next symbol;
   - doc comments on small private functions that restate the name;
   - promises the code does not keep, and names or types restated as
     prose;
   - any name of a tool or system the project draws on (the naming ban;
     only the README's Inspiration section may name them).
4. Keep each surviving comment timeless and direct, a why and not a what,
   in the voice of the file around it. No em-dash character.
5. In the same hunks, remove slop that is abnormal for the file:
   - an `#[allow(...)]` added to quiet a warning instead of fixing it;
   - `let _ =` on a `Result`, an `.ok()` or `unwrap_or_default()` that
     hides a failure the log should carry;
   - defensive checks for states the types already exclude;
   - `clone()`s that dodge a borrow a reference would satisfy;
   - one-use helpers and intermediate variables that add no domain
     meaning;
   - a `pub` widened only for a test (see test-audit);
   - compatibility shims, aliases and fallbacks with no shipped contract
     (`core_document::renamed` and `#[serde(default)]` are the sanctioned
     compatibility paths);
   - a colour or size literal where `ui_kit::tokens` has the value;
   - style that conflicts with the surrounding file.
6. Make no functional edit. If a cleanup could change behavior, leave it
   and report it.
7. Rerun the lint, `cargo fmt --all --check` and the owning crate's
   clippy and tests.

## Campaign

A campaign cleans the comments of one crate in one commit. Read every
comment in the crate, give each an action and a one-line reason, apply
the ledger comments only (no behavior change, no renames, no
reformatting), and run `node scripts/lint-comment-rot.mjs --all` and the
crate's tests unchanged. Have one independent reviewer read the deleted
comments for a lost contract, and restore only those with source
evidence.

## Report

One to three sentences: what changed, the lint result before and after,
and any non-trivial item left for the author. Run this skill before
`review`, never in place of it.
