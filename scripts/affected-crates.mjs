#!/usr/bin/env node
// The crates a change can break: those whose files changed, and every
// workspace crate that depends on them, directly or not. Prints them as
// `-p <crate>` arguments for cargo (or for scripts/test-budget.mjs), or
// `--workspace` when the change reaches everything (the workspace's
// Cargo.toml or Cargo.lock), or nothing when no crate is touched.
//
//   node scripts/affected-crates.mjs            changes against HEAD, untracked files too
//   node scripts/affected-crates.mjs <ref>      changes since <ref> (origin/master)
//
// CI runs every test; this is for the edit-test loop, where testing only
// what a change reaches saves building and running the rest.

import { execFileSync } from "node:child_process";
import { relative, dirname } from "node:path";

const base = process.argv[2] ?? "HEAD";
const git = (...args) => execFileSync("git", args, { encoding: "utf8" });
const root = git("rev-parse", "--show-toplevel").trim();
const changed = new Set(
  [
    ...git("diff", "--name-only", base).split("\n"),
    ...git("ls-files", "--others", "--exclude-standard").split("\n"),
  ].filter(Boolean),
);

const everything = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo/config.toml"];
if ([...changed].some((f) => everything.includes(f))) {
  console.log("--workspace");
  process.exit(0);
}

const metadata = JSON.parse(
  execFileSync("cargo", ["metadata", "--format-version", "1", "--no-deps"], {
    encoding: "utf8",
    maxBuffer: 1 << 28,
  }),
);
const members = metadata.packages.map((p) => ({
  name: p.name,
  dir: relative(root, dirname(p.manifest_path)),
  deps: p.dependencies.filter((d) => d.path).map((d) => d.name),
}));

// Files whose test lives in another crate than their own directory says.
const elsewhere = [
  // The scripting guide is generated, and app_shell's test checks it.
  ["docs/SCRIPTING.md", "app_shell"],
  // The guest SDK and its examples are built by wb_wasm's package tests.
  ["sdk/", "wb_wasm"],
];

const touched = new Set();
for (const file of changed) {
  // The deepest crate directory holding the file owns it.
  const owner = members
    .filter((m) => file === m.dir || file.startsWith(`${m.dir}/`))
    .sort((a, b) => b.dir.length - a.dir.length)[0];
  if (owner) touched.add(owner.name);
  for (const [prefix, crate] of elsewhere) {
    if (file.startsWith(prefix)) touched.add(crate);
  }
}

// Everything that depends on a touched crate, transitively.
const affected = new Set(touched);
let grew = true;
while (grew) {
  grew = false;
  for (const m of members) {
    if (!affected.has(m.name) && m.deps.some((d) => affected.has(d))) {
      affected.add(m.name);
      grew = true;
    }
  }
}
console.log([...affected].sort().map((name) => `-p ${name}`).join(" "));
