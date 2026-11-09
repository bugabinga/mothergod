// actionlint's contract (#1656, #1987): the version, checksum and ignored
// message live in .github/scripts/actionlint alone, ci.yml's `workflows` job
// runs that script under an actions/cache step keyed on the script's hash,
// and no YAML fetches the binary itself. So a pin bump is one edit, and a
// release-CDN fault costs one download per pin, not one per run: on
// 2026-11-09 a minute of 504s outlasted ten retries and reddened main. A
// pin copied back into YAML, or a cache keyed on anything but the script,
// is red here and not on the next fault.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

const root = new URL("../..", import.meta.url).pathname;
const script = readFileSync(join(root, ".github/scripts/actionlint"), "utf8");
const ci = readFileSync(join(root, ".github/workflows/ci.yml"), "utf8");
const CACHE = "key: actionlint-${{ hashFiles('.github/scripts/actionlint') }}";
const RUN = "- run: .github/scripts/actionlint\n";

test("the script pins one version with a full checksum", () => {
  assert.match(script, /^VERSION=\d+\.\d+\.\d+$/m, "VERSION is not one release");
  assert.match(script, /^SHA256=[0-9a-f]{64}$/m, "SHA256 is not 64 hex");
});

test("ci runs the script behind a cache keyed on it, and no YAML fetches actionlint", () => {
  assert.notEqual(ci.indexOf(CACHE), -1, "ci.yml: no cache keyed on the script's hash");
  assert.notEqual(ci.indexOf(RUN), -1, "ci.yml: the workflows job does not run the script");
  assert.ok(ci.indexOf(CACHE) < ci.indexOf(RUN), "ci.yml: the cache restores after the script ran");
  const dirs = [".github/workflows", ".github/actions"];
  for (const dir of dirs) {
    for (const entry of readdirSync(join(root, dir), { recursive: true })) {
      if (!/\.ya?ml$/.test(entry)) continue;
      const text = readFileSync(join(root, dir, entry), "utf8");
      assert.ok(
        !text.includes("rhysd/actionlint"),
        `${dir}/${entry}: fetches actionlint itself; the pin lives in the script`,
      );
    }
  }
});
