// The notice is the only view of an agent run the operator actually reads,
// and for a month it opened with "green, 12 turns, 4m" and ended 500 chars
// in, mid-sentence. Run 34727798310 lost its "needs the BDFL's credentials"
// blocker that way (operator report, 2026-09-13). These pin what survives.
import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { test } from "node:test";

const script = new URL("run-notice.py", import.meta.url).pathname;

// spawnSync is fine here, unlike in tg-send.test.mjs: nothing answers this
// process, so there is no event loop to starve.
function notice(response, meta = {}, label = "maintainer") {
  const dir = mkdtempSync(join(tmpdir(), "notice-"));
  writeFileSync(join(dir, "metadata.json"), JSON.stringify(meta));
  if (response !== null) writeFileSync(join(dir, "output-response.md"), response);
  const { stdout, status } = spawnSync("python3", [script, dir, label], {
    encoding: "utf8",
    env: {
      PATH: process.env.PATH,
      GITHUB_SERVER_URL: "https://github.com",
      GITHUB_REPOSITORY: "bugabinga/mothergod",
      GITHUB_RUN_ID: "99",
    },
  });
  assert.equal(status, 0, "observability never breaks the thing it observes");
  return stdout;
}

const RUN_URL = "https://github.com/bugabinga/mothergod/actions/runs/99";

test("a green run leads with the agent, not with telemetry that never varies", () => {
  const out = notice("Shipped the thing.", { telemetry: { num_turns: 12, duration_ms: 240000 } });
  assert.equal(out, "**maintainer**\nShipped the thing.\n");
  assert.doesNotMatch(out, /green|turns|4m/);
});

test("a red run says so first and carries its telemetry and run link", () => {
  const out = notice("Died.", { is_error: true, telemetry: { num_turns: 3, duration_ms: 120000 } });
  assert.equal(out, `**maintainer: RED** (3 turns, 2m)\nDied.\n${RUN_URL}\n`);
});

test("markdown reaches tg-send intact, because tg-send is what renders it", () => {
  const body = "Opened **PR #545**, see `src/lib.rs`.\n- one";
  assert.match(notice(body), /\*\*PR #545\*\*, see `src\/lib\.rs`\.\n- one/);
});

test("an over-long response loses its middle, keeping what shipped and what is owed", () => {
  const head = "OPENED PR 545. ";
  const tail = " NEXT: needs the BDFL credentials.";
  const out = notice(head + "filler ".repeat(900) + tail);
  assert.match(out, /^\*\*maintainer\*\*\nOPENED PR 545\./);
  assert.match(out, /NEXT: needs the BDFL credentials\.\n/);
  assert.match(out, /chars omitted, full run log linked below/);
  // The elision promises the full text is one link away; a green run has no
  // other reason to carry the link, so the promise has to add it.
  assert.match(out, new RegExp(`${RUN_URL}\n$`));
});

test("a response under the clip is never elided", () => {
  assert.doesNotMatch(notice("short"), /omitted/);
});

test("an absent or placeholder response degrades to the run link", () => {
  assert.equal(notice(null), `**maintainer**\n${RUN_URL}\n`);
  assert.equal(notice("(no result)"), `**maintainer**\n${RUN_URL}\n`);
});

test("a run that died before the audit wrote anything still reports", () => {
  const dir = mkdtempSync(join(tmpdir(), "notice-"));
  const { stdout, status } = spawnSync("python3", [script, dir, "herald"], {
    encoding: "utf8",
    env: { PATH: process.env.PATH, GITHUB_REPOSITORY: "bugabinga/mothergod", GITHUB_RUN_ID: "99" },
  });
  assert.equal(status, 0);
  assert.match(stdout, /^herald: finished, no audit record\n/);
});
