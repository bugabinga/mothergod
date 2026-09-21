// Fixtures for the completion-opener predicate. Every `text` here is the
// opening of a real audited final response from 2026-09-19 to 2026-09-21,
// lightly shortened; the expectations pin the docstring's edges: content
// marks protect a sentence, a bare label goes with its announcement, and
// trimming never empties the response.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;

const driver = `
import json, sys
sys.path.insert(0, sys.argv[1])
import opener
text, removed = opener.trim(sys.argv[2])
print(json.dumps({"text": text, "removed": removed}))
`;

const fixtures = [
  {
    name: "the maintainer's 'Heartbeat done. Summary:' loses both lines",
    text: "Heartbeat done. Summary:\n\n- Checked the priority queue: no red PRs.",
    expect: { text: "- Checked the priority queue: no red PRs.", removed: ["Heartbeat done."] },
  },
  {
    name: "an eleven-word announcement is still an announcement",
    text: "Heartbeat's one substantial unit of work is done and posted. Summary:\n\n- Opened PR #643.",
    expect: {
      text: "- Opened PR #643.",
      removed: ["Heartbeat's one substantial unit of work is done and posted."],
    },
  },
  {
    name: "the curator's 'Run complete.'",
    text: "Run complete. Triaged #637 and #636.",
    expect: { text: "Triaged #637 and #636.", removed: ["Run complete."] },
  },
  {
    name: "the BDFL's 'Run's done.'",
    text: "Run's done. Delta core came back clean, but retrospect surfaced a real gap.",
    expect: {
      text: "Delta core came back clean, but retrospect surfaced a real gap.",
      removed: ["Run's done."],
    },
  },
  {
    name: "'Status line sent.' says nothing to the reader who just got it",
    text: "Status line sent. No-op run: sweep, inbox and PRs were all clean.",
    expect: { text: "No-op run: sweep, inbox and PRs were all clean.", removed: ["Status line sent."] },
  },
  {
    name: "a colon-labelled announcement trims to the colon and recapitalizes",
    text: "This run's work is done: filed **#636** for the operator.",
    expect: { text: "Filed **#636** for the operator.", removed: ["This run's work is done:"] },
  },
  {
    name: "two announcements in a row: the second stays rather than emptying the text",
    text: "Ops-log posted. This heartbeat's one unit of work is done.",
    expect: { text: "This heartbeat's one unit of work is done.", removed: ["Ops-log posted."] },
  },
  {
    name: "an announcement that is the whole response stays",
    text: "Heartbeat complete for this cycle.",
    expect: { text: "Heartbeat complete for this cycle.", removed: [] },
  },
  {
    name: "a PR reference is content, even beside 'passes'",
    text: "PR #641 passes. Verified independently.",
    expect: { text: "PR #641 passes. Verified independently.", removed: [] },
  },
  {
    name: "'posted' beside an issue reference is content",
    text: "Review posted and PR #639 labeled `changes-requested`.",
    expect: { text: "Review posted and PR #639 labeled `changes-requested`.", removed: [] },
  },
  {
    name: "a bold verdict label is not an announcement",
    text: "**Verdict: changes-requested** on PR #630.",
    expect: { text: "**Verdict: changes-requested** on PR #630.", removed: [] },
  },
  {
    name: "the documented miss: no keyword, so it reads as content",
    text: "Inbox still empty. That closes out this wake.",
    expect: { text: "Inbox still empty. That closes out this wake.", removed: [] },
  },
  {
    name: "a one-word response survives",
    text: "Done.",
    expect: { text: "Done.", removed: [] },
  },
  {
    name: "empty in, empty out",
    text: "",
    expect: { text: "", removed: [] },
  },
];

for (const { name, text, expect } of fixtures) {
  test(name, () => {
    const run = spawnSync("python3", ["-c", driver, scriptsDir, text], { encoding: "utf-8" });
    assert.equal(run.status, 0, run.stderr);
    assert.deepEqual(JSON.parse(run.stdout), expect);
  });
}
