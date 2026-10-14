// Pins for memory-check, the recipe seven by-hand measurements shared
// (#1109, #1145): the footer prints once both children ran, a peak at or
// over the bound fails and names the side, a decode that differs fails
// under the bound, and a child that does not exit 0 is a failure, never a
// reading. The subprocess pins use a stub compressor (cat both ways, or a
// liar), because the script's claims are about the measurement, not the
// codec; the boundary is pinned on `verdict` directly, because a stub's
// peak is whatever the kernel reports and a bound equal to it is a race
// (#1148).
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const script = join(new URL(".", import.meta.url).pathname, "memory-check");

function stub(body) {
  const dir = mkdtempSync(join(tmpdir(), "memory-check-"));
  const path = join(dir, "mothergod");
  writeFileSync(path, `#!/bin/sh\n${body}\n`, { mode: 0o755 });
  return path;
}

const run = (binary, ...args) => spawnSync(script, [...args, binary], { encoding: "utf8" });

const footer = (out) => out.split("\n").find((line) => line.startsWith("memory-check: bytes"));

const driver = `
import importlib.machinery, importlib.util, json, sys
loader = importlib.machinery.SourceFileLoader("memory_check", sys.argv[1])
spec = importlib.util.spec_from_loader("memory_check", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
print(json.dumps(mod.verdict(*json.loads(sys.argv[2]))))
`;

function verdict(encode, decode, boundKib, lossless) {
  const r = spawnSync("python3", ["-I", "-c", driver, script, JSON.stringify([encode, decode, boundKib, lossless])], {
    encoding: "utf8",
  });
  assert.equal(r.status, 0, r.stderr);
  return JSON.parse(r.stdout);
}

test("under is strict: a peak one below the bound holds, at the bound fails, each side named", () => {
  assert.deepEqual(verdict(1023, 1023, 1024, true), []);
  assert.deepEqual(verdict(1024, 1023, 1024, true), ["encode peak 1024 KiB is not under the bound"]);
  assert.deepEqual(verdict(1023, 1024, 1024, true), ["decode peak 1024 KiB is not under the bound"]);
  assert.deepEqual(verdict(5, 5, 1024, false), ["round trip differs"]);
});

test("a stub that copies both ways holds: exit 0, the footer carries both peaks and the size", () => {
  const r = run(stub("exec cat"), "--bytes", "4096");
  assert.equal(r.status, 0, r.stdout + r.stderr);
  assert.match(
    footer(r.stdout),
    /^memory-check: bytes 4096 \| encode peak \d+ KiB \| decode peak \d+ KiB \| bound 1048576 KiB \| compressed 4096 B \| round trip ok$/,
  );
});

test("a peak at or over the bound fails and names the side", () => {
  const r = run(stub("exec cat"), "--bytes", "4096", "--bound-kib", "1");
  assert.equal(r.status, 1, r.stdout + r.stderr);
  assert.match(r.stdout, /encode peak \d+ KiB is not under the bound/);
  assert.match(r.stdout, /decode peak \d+ KiB is not under the bound/);
});

test("a decode that differs fails under the bound, and the footer says so", () => {
  const liar = "if [ \"$1\" = -d ]; then head -c 4095 /dev/zero; printf x; else cat; fi";
  const r = run(stub(liar), "--bytes", "4096");
  assert.equal(r.status, 1, r.stdout + r.stderr);
  assert.match(footer(r.stdout), /round trip DIFFERS$/);
  assert.match(r.stdout, /^memory-check: round trip differs$/m);
});

test("a child that does not exit 0 is a failure, never a reading", () => {
  const r = run(stub("exit 3"), "--bytes", "4096");
  assert.equal(r.status, 1, r.stdout + r.stderr);
  assert.match(r.stdout, /did not exit 0/);
  assert.equal(footer(r.stdout), undefined);
});
