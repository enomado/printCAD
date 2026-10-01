---
name: release-publisher
description: "Prepare a printCAD release: bump the app's version, write its release-notes section, run the gates, and hand the exact proposal to the owner, who tags and pushes. Use when asked to cut, prepare or check a release. Not for a red CI run (use ci-iterate) and not for kernel or sixdof releases, which live in their own repositories."
---

# Release publisher

A release is three things that must agree: the app's version in
`crates/app_shell/Cargo.toml`, a `## <version>` section in
`crates/app_shell/RELEASE_NOTES.md`, and a tag `vX.Y.Z`. The tag starts
`.github/workflows/release.yml`, which checks the first two, builds
Linux, Windows and macOS through `scripts/package-release.sh`, and
publishes them as a GitHub release whose text is the notes section.
Prepare the proposal, show it, and wait for explicit approval of that
exact proposal before you commit a version change. The owner tags and
pushes; never tag, push or publish yourself.

## 1. Scope and evidence

1. Confirm the user asked for a release. Require a clean tree; fetch
   `master` and the tags and confirm the branch is current.
2. Find the previous release boundary: `git describe --tags --abbrev=0`
   and `gh release list --limit 5`.
3. Read every commit and full commit body from that tag to `HEAD`.
   Commits and diffs are the evidence; old notes are context only.
4. Note what the release depends on outside the repo: the kernel and
   `sixdof` versions `Cargo.lock` pins (an active `[patch.crates-io]` is
   a blocker: a release builds from the published crates), and whether
   package-facing types in `bench_api` or `sdk/` changed, since the
   example workbench repository builds the SDK from `master`.

## 2. Version

- Bump `version` in `crates/app_shell/Cargo.toml` only; a `cargo check
  -p app_shell` refreshes `Cargo.lock`. Patch for fixes, minor for new
  features or anything a user has to relearn; under `0.x` a breaking
  change is a minor bump.
- Check the tag does not already exist (`git tag -l`, `gh release
  view`); a collision is a blocker, never an overwrite.

## 3. Notes

The file's own rule: each release is a `## <version>` heading, its topics
`### <topic>`, one bullet per change. The start page's What's new shows
these, the running version first, and a test fails when the running
version has no entry.

- Group by user outcome, not by file or commit order. Lead with what
  users get, then the concrete behavior, in the voice of the existing
  sections.
- State removed or renamed behavior plainly (ids that changed names are
  read either way through `core_document::renamed`; say so when it
  matters to scripts).
- Say when a document saved by this version does not open in the last
  one.
- Plain language, short bullets, no em-dash character, no name of any
  tool or system the project draws on.
- No unreproducible claims about performance or compatibility. Windows
  and macOS builds are CI-verified only; do not claim more.

## 4. Validate and propose

Run the gates the release depends on, after the last edit:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
node scripts/lint-comment-rot.mjs --all
grep -n $'\u2014' crates/app_shell/RELEASE_NOTES.md
```

Then the workflow's own checks by hand: `v$(sed -n 's/^version = "\(.*\)"/\1/p' crates/app_shell/Cargo.toml | head -1)`
is the tag you will propose, and `grep -q "^## <version>$"
crates/app_shell/RELEASE_NOTES.md` finds the section. When
`scripts/package-release.sh` or an icon changed, `cargo build --release`
the workspace and run the script for `linux` to see the archive is
whole.

Present the exact version change, the notes section, the gate results
and the commands the owner will run:

```bash
git tag vX.Y.Z && git push origin master vX.Y.Z
```

Then stop and wait.

## 5. After approval

- Commit the bump and the notes as one commit (`release: X.Y.Z`), no
  co-author lines, and say it is ready to push.
- Once the owner has tagged, follow the run: `gh run list
  --workflow=release.yml`, `gh run view <id>`. A run that stopped
  part-way is finished by dispatching the workflow with the tag, which
  the owner does; it uploads over an existing release rather than
  making a second one.
- Verify through the hosting API: `gh release view vX.Y.Z` shows the
  three archives and the notes. The app's update check reads this
  release, so every user sees it: a bad release is superseded by a
  patch, never deleted or moved.

Verdict: `RELEASED`, `READY` (awaiting the owner's tag), `PARTIAL` (some
remote state exists but a step failed; name it), or `BLOCKED`.
