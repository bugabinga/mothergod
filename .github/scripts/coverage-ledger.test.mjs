// The parse path (real cargo-llvm-cov shape), the two bail paths (missing
// report, unreadable report), and the worst-N selection, since those are
// the shapes `coverage-check.yml`'s `if: always()` step relies on never
// throwing.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const scriptPath = new URL("coverage-ledger.py", import.meta.url).pathname;

function run(reportPath, outPath) {
  const result = spawnSync("python3", [scriptPath, reportPath, outPath], { encoding: "utf8" });
  return { status: result.status, stdout: result.stdout, stderr: result.stderr };
}

function withTmpDir(fn) {
  const dir = mkdtempSync(join(tmpdir(), "coverage-ledger-"));
  try {
    return fn(dir);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

test("a missing report writes a null-coverage entry and exits 0", () => {
  withTmpDir((dir) => {
    const out = join(dir, "entry.json");
    const result = run(join(dir, "absent.json"), out);
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stderr + result.stdout, /does not exist/);
    const entry = JSON.parse(readFileSync(out, "utf8"));
    assert.equal(entry.coverage_region_pct, null);
    assert.equal(entry.role, "coverage");
  });
});

test("a malformed report writes a null-coverage entry and exits 0", () => {
  withTmpDir((dir) => {
    const report = join(dir, "coverage.json");
    const out = join(dir, "entry.json");
    writeFileSync(report, "not json");
    const result = run(report, out);
    assert.equal(result.status, 0, result.stderr);
    const entry = JSON.parse(readFileSync(out, "utf8"));
    assert.equal(entry.coverage_region_pct, null);
  });
});

test("a report missing the data/totals shape bails, not throws", () => {
  withTmpDir((dir) => {
    const report = join(dir, "coverage.json");
    const out = join(dir, "entry.json");
    writeFileSync(report, JSON.stringify({ data: [] }));
    const result = run(report, out);
    assert.equal(result.status, 0, result.stderr);
    const entry = JSON.parse(readFileSync(out, "utf8"));
    assert.equal(entry.coverage_region_pct, null);
  });
});

test("a real-shaped report writes the crate percent and names the worst files", () => {
  withTmpDir((dir) => {
    const report = join(dir, "coverage.json");
    const out = join(dir, "entry.json");
    writeFileSync(
      report,
      JSON.stringify({
        data: [
          {
            totals: { regions: { percent: 91.2345 } },
            files: [
              { filename: "src/lib.rs", summary: { regions: { count: 10, percent: 94.67 } } },
              { filename: "src/codec.rs", summary: { regions: { count: 20, percent: 96.56 } } },
              { filename: "src/bin/mothergod.rs", summary: { regions: { count: 5, percent: 82.44 } } },
              { filename: "src/empty.rs", summary: { regions: { count: 0, percent: 0 } } },
            ],
          },
        ],
      }),
    );
    const result = run(report, out);
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /mothergod\.rs at 82\.44% region coverage/);
    assert.doesNotMatch(result.stdout, /empty\.rs/);
    const entry = JSON.parse(readFileSync(out, "utf8"));
    assert.equal(entry.coverage_region_pct, 91.23);
    assert.equal(entry.mutation_score, null);
    assert.equal(entry.fuzz_cpu_s, null);
  });
});
