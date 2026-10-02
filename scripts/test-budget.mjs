#!/usr/bin/env node
// Run the workspace's tests within a time budget.
//
// Builds every test program (`cargo test --no-run`), then runs each one
// timed, the tests inside it in parallel as `cargo test` would. Fails when
// a test fails, when one program takes longer than its budget, or when all
// of them together do: a slow test is a cost every build pays, so it is
// caught when it comes in rather than found later.
//
//   node scripts/test-budget.mjs                  every crate
//   node scripts/test-budget.mjs -p wb_sketch …   only these packages
//
// TEST_BUDGET_PROGRAM_S (default 60) and TEST_BUDGET_TOTAL_S (default 150)
// set the budgets in seconds; a slower machine raises them, the defaults
// fit a CI runner with room to spare.

import { execFileSync, spawnSync } from "node:child_process";
import { basename } from "node:path";

const programBudget = Number(process.env.TEST_BUDGET_PROGRAM_S ?? 60);
const totalBudget = Number(process.env.TEST_BUDGET_TOTAL_S ?? 150);
const packages = process.argv.slice(2);
const scope = packages.length ? packages : ["--workspace"];

const built = execFileSync(
  "cargo",
  ["test", ...scope, "--no-run", "--message-format=json"],
  { encoding: "utf8", maxBuffer: 1 << 30, stdio: ["ignore", "pipe", "inherit"] },
);
const programs = [];
for (const line of built.split("\n")) {
  if (!line.startsWith("{")) continue;
  const message = JSON.parse(line);
  if (message.reason === "compiler-artifact" && message.profile?.test && message.executable) {
    programs.push({ path: message.executable, crate: message.target.name });
  }
}

const results = [];
let failed = false;
for (const program of programs) {
  const started = process.hrtime.bigint();
  const run = spawnSync(program.path, ["-q"], { encoding: "utf8", maxBuffer: 1 << 28 });
  const seconds = Number(process.hrtime.bigint() - started) / 1e9;
  const passed = /(\d+) passed/.exec(run.stdout ?? "")?.[1] ?? "0";
  if (run.status !== 0) {
    failed = true;
    process.stdout.write(run.stdout ?? "");
    process.stderr.write(run.stderr ?? "");
    console.error(`FAILED: ${program.crate} (${basename(program.path)})`);
  }
  results.push({ ...program, seconds, passed: Number(passed) });
}

results.sort((a, b) => b.seconds - a.seconds);
const total = results.reduce((sum, r) => sum + r.seconds, 0);
const tests = results.reduce((sum, r) => sum + r.passed, 0);
console.log("slowest test programs:");
for (const r of results.slice(0, 8)) {
  const over = r.seconds > programBudget ? "  OVER BUDGET" : "";
  console.log(`  ${r.seconds.toFixed(1).padStart(6)} s  ${String(r.passed).padStart(4)} tests  ${r.crate}${over}`);
}
console.log(
  `${tests} tests in ${results.length} programs, ${total.toFixed(1)} s ` +
    `(budget ${programBudget} s a program, ${totalBudget} s in all)`,
);

const overProgram = results.filter((r) => r.seconds > programBudget);
if (overProgram.length) {
  failed = true;
  console.error(
    `over budget: ${overProgram.map((r) => r.crate).join(", ")}; make the slow tests ` +
      "smaller, or mark the ones that need a big model #[ignore] with the reason",
  );
}
if (total > totalBudget) {
  failed = true;
  console.error(`the suite took ${total.toFixed(1)} s, over its ${totalBudget} s budget`);
}
process.exit(failed ? 1 : 0);
