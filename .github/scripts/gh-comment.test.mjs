// gh-comment assembles two things a human keeps getting wrong by hand: the
// role footer the self-trigger guard matches on (issue #138, #206) and the
// credential the write must ride (issue #50, #81). Issue #489 added a third
// verb, `--new`, because the app token expires an hour into a session and a
// long run could not file what it learned. All three are argv-shaped, so a
// stub `gh` on PATH is the whole harness: it records what it was called with.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdtempSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const script = join(new URL(".", import.meta.url).pathname, "gh-comment");

// The stub records every gh call in order. `issue list`, which `--new` sends
// first (#1374), answers with `issues` as JSON, or exits 1 when `issues` is
// "FAIL"; every other verb prints a URL. `called` is the last call, the one
// that posts, so the assertions written before the list existed still read
// the create or the comment.
function run(args, { body = "hello", env = {}, issues = [] } = {}) {
  const stub = mkdtempSync(join(tmpdir(), "gh-comment-"));
  const log = join(stub, "calls.json");
  const recorder = join(stub, "record.mjs");
  writeFileSync(
    recorder,
    `import { existsSync, readFileSync, writeFileSync } from "node:fs";
const log = ${JSON.stringify(log)};
const calls = existsSync(log) ? JSON.parse(readFileSync(log, "utf8")) : [];
const argv = process.argv.slice(2);
calls.push({ argv, token: process.env.GH_TOKEN ?? null, admin: process.env.GH_ADMIN_TOKEN ?? null });
writeFileSync(log, JSON.stringify(calls));
const verb = argv.slice(0, 2).join(" ");
const flaky = process.env.GH_STUB_FLAKY ?? "";
if (flaky && verb === flaky && calls.filter((c) => c.argv.slice(0, 2).join(" ") === flaky).length === 1) {
  console.error(process.env.GH_STUB_FLAKY_ERROR ?? "HTTP 502: Server Error (https://api.github.com/graphql)");
  process.exit(1);
}
if (argv[0] === "api" && argv.some((a) => a.endsWith("/comments"))) {
  console.log(process.env.GH_STUB_COMMENTS ?? "[]");
} else if (argv[0] === "api" && argv.includes("--jq")) {
  console.log(process.env.GH_STUB_STATE ?? "closed");
} else if (verb === "issue list") {
  if (process.env.GH_STUB_ISSUES === "FAIL") {
    console.error("HTTP 500: edge down");
    process.exit(1);
  }
  console.log(process.env.GH_STUB_ISSUES);
} else {
  console.log("https://github.com/o/r/issues/1");
}
`,
  );
  writeFileSync(join(stub, "gh"), `#!/bin/sh\nexec node ${recorder} "$@"\n`);
  chmodSync(join(stub, "gh"), 0o755);
  const proc = spawnSync("python3", [script, ...args], {
    encoding: "utf8",
    input: body,
    env: {
      PATH: `${stub}:${process.env.PATH}`,
      ROLE: "bdfl",
      GH_WORKFLOW_TOKEN: "workflow-token",
      GH_ADMIN_TOKEN: "admin-token",
      GH_STUB_ISSUES: issues === "FAIL" ? "FAIL" : JSON.stringify(issues),
      GH_RETRY_DELAY: "0",
      ...env,
    },
  });
  let calls = [];
  try {
    calls = JSON.parse(readFileSync(log, "utf8"));
  } catch {
    // gh was never reached; the refusal tests assert exactly that.
  }
  const verbs = calls.map((c) => c.argv.slice(0, 2).join(" "));
  return { ...proc, called: calls.at(-1) ?? null, calls, verbs };
}

test("--new creates an issue with the labels, the footer, and the workflow token", () => {
  const { status, called } = run(["--new", "Late writes die", "--label", "bug", "--label", "agent-system"]);
  assert.equal(status, 0);
  assert.deepEqual(called.argv.slice(0, 4), ["issue", "create", "--title", "Late writes die"]);
  assert.match(called.argv[5], /^hello\n\n---\n_bdfl · \[Claude Code\]/);
  assert.deepEqual(called.argv.slice(6), ["--label", "bug", "--label", "agent-system"]);
  assert.equal(called.token, "workflow-token");
  // The admin PAT is stripped, not merely unused: an ambient one is the bug.
  assert.equal(called.admin, null);
});

// Issue #1374: the deslopper filed one decodes_incrementally finding four
// times in 40 hours, and each copy cost a triage. The guard holds for every
// seat because it is in the one path an issue is filed through, not in a
// prompt each seat has to remember.
test("--new refuses a title whose identifier and claim an open issue's title already names, and lists it", () => {
  const issues = [
    {
      number: 1297,
      title: "decodes_incrementally restates codec::decode_to_writer's streaming dispatch as a second matches!",
    },
    { number: 1326, title: "gzip reference size varies by a few bytes between runs" },
  ];
  const { status, stderr, verbs } = run(
    [
      "--new",
      "lib.rs decodes_incrementally restates codec::decode_to_writer's streaming dispatch",
      "--label",
      "product",
    ],
    { issues },
  );
  assert.equal(status, 1);
  assert.match(stderr, /already names `codec::decode_to_writer`, `decode_to_writer`, `decodes_incrementally`:/);
  assert.match(stderr, /#1297  decodes_incrementally restates/);
  assert.doesNotMatch(stderr, /#1326/);
  assert.match(stderr, /--anyway/);
  assert.deepEqual(verbs, ["issue list"]);
});

// Issue #1427: a name alone flagged 66 distinct defects among 71 pairs on
// 200 closed titles, seven titles on baseline_gate alone, and a seat that
// learns the retry is routine stops reading the hits. A copy restates the
// claim; a second defect in the same symbol does not.
test("a shared name alone is an area; a copy shares two words of the claim too", () => {
  const issues = [
    { number: 1434, title: "baseline_gate check sees no input past 50,000 bytes" },
    { number: 1346, title: "baseline_gate stale-finals stderr cites README for 35 minutes it does not state" },
  ];
  const copy = run(
    ["--new", "baseline_gate's stale-finals stderr sends the fixer into a 35 minute foreground run"],
    { issues },
  );
  assert.equal(copy.status, 1);
  assert.match(copy.stderr, /#1346  baseline_gate stale-finals/);
  assert.doesNotMatch(copy.stderr, /#1434/);
  const other = run(
    ["--new", "baseline_gate write raises a case's baseline when the case grew within tolerance"],
    { issues },
  );
  assert.equal(other.status, 0, other.stderr);
  assert.deepEqual(other.verbs, ["issue list", "issue create"]);
});

test("a file name with an underscore is an area, bare or backticked, even when the claim matches", () => {
  const issues = [{ number: 1354, title: "bench_report.rs reads stale rows from the frontier table" }];
  for (
    const title of [
      "bench_report.rs writes stale rows to the frontier table",
      "`bench_report.rs` writes stale rows to the frontier table",
    ]
  ) {
    const { status, stderr, verbs } = run(["--new", title], { issues });
    assert.equal(status, 0, `${title}: ${stderr}`);
    assert.deepEqual(verbs, ["issue list", "issue create"], title);
  }
});

// Issue #1505: identifiers() dropped the file name, claim() did not, so a
// file name of five letters or more plus one shared word made the two the
// copy rule asks for, and a second defect in one symbol and one file paid
// the --anyway retry #1504 set out to end.
test("a file name is not a claim word either: one shared word beside it files", () => {
  const issues = [{ number: 1469, title: "baseline_gate drops cases in bench_report.rs" }];
  const { status, stderr, verbs } = run(
    ["--new", "baseline_gate drops frontier rows in bench_report.rs"],
    { issues },
  );
  assert.equal(status, 0, stderr);
  assert.deepEqual(verbs, ["issue list", "issue create"]);
});

test("--anyway files past the hit without reading the list, on the same credential", () => {
  const issues = [{ number: 1281, title: "decodes_incrementally's matches! drifts silently" }];
  const { status, verbs, called } = run(
    ["--new", "decodes_incrementally restates the dispatch", "--anyway"],
    { issues },
  );
  assert.equal(status, 0);
  assert.deepEqual(verbs, ["issue create"]);
  assert.equal(called.token, "workflow-token");
});

test("a shared file name, script name or type name is an area, not a duplicate", () => {
  const issues = [
    { number: 1354, title: "literal.rs ALPHABET_U32 restates 256 beside a derived ALPHABET" },
    { number: 1418, title: "bench-report --publish leaves the body's moved columns to the seat" },
  ];
  const { status, stderr, verbs } = run(
    ["--new", "literal.rs and codec.rs: bench-report helpers shared by the PPM expert have one caller"],
    { issues },
  );
  assert.equal(status, 0, stderr);
  assert.deepEqual(verbs, ["issue list", "issue create"]);
});

test("a :: path meets its bare segment, a backticked token meets itself, a flag meets a flag", () => {
  const issues = [
    { number: 1297, title: "codec::decode_to_writer restates the streaming dispatch" },
    { number: 1301, title: "merge-pr --wait reads the latest review run on a head" },
    { number: 1192, title: "`mutants` is advisory and pending on every push" },
  ];
  const titles = [
    "decode_to_writer's doc restates the streaming dispatch",
    "merge-pr --wait reads the latest review run twice",
    "the `mutants` job stays advisory and pending",
  ];
  for (const title of titles) {
    const { status, stderr } = run(["--new", title], { issues });
    assert.equal(status, 1, title);
    assert.match(stderr, /already names/, title);
  }
});

test("an open-issue list gh cannot read files with a warning, never silently and never a refusal", () => {
  const { status, stderr, verbs } = run(["--new", "decodes_incrementally restates the dispatch"], { issues: "FAIL" });
  assert.equal(status, 0);
  assert.match(stderr, /filing without the duplicate check/);
  assert.deepEqual(verbs, ["issue list", "issue create"]);
});

test("--anyway without --new is refused rather than silently dropped", () => {
  const { status, stderr, called } = run(["489", "--anyway"]);
  assert.equal(status, 1);
  assert.equal(called, null);
  assert.match(stderr, /--anyway applies to --new only/);
});

test("a completion-announcement opener is trimmed before the body is posted", () => {
  const { status, called, stderr } = run(["489"], {
    body: "Ops-log posted. PR #641 passed review and merged.",
  });
  assert.equal(status, 0);
  assert.match(called.argv[4], /^PR #641 passed review and merged\.\n\n---/);
  assert.match(stderr, /trimmed 1 completion-announcement opener/);
});

test("a body with no opener reaches gh unchanged", () => {
  const { called, stderr } = run(["489"], { body: "PR #641 passed review and merged." });
  assert.match(called.argv[4], /^PR #641 passed review and merged\.\n\n---/);
  assert.equal(stderr, "");
});

test("commenting and closing keep their shapes", () => {
  assert.deepEqual(run(["489"]).called.argv.slice(0, 2), ["issue", "comment"]);
  assert.deepEqual(run(["489", "--close"]).called.argv.slice(0, 2), ["issue", "close"]);
});

// Issue #772's own close: `gh issue close` has no --delete-branch, and the
// session that wanted one hand-rolled `gh pr close` instead, losing the
// footer. This is the path that should have existed.
test("--close --delete-branch closes through gh pr, not gh issue", () => {
  const { status, called } = run(["772", "--close", "--delete-branch"]);
  assert.equal(status, 0);
  assert.deepEqual(called.argv.slice(0, 2), ["pr", "close"]);
  assert.ok(called.argv.includes("--delete-branch"));
  assert.match(called.argv[called.argv.indexOf("--comment") + 1], /^hello\n\n---\n_bdfl ·/);
});

test("--delete-branch without --close is refused rather than silently dropped", () => {
  const { status, stderr, called } = run(["772", "--delete-branch"]);
  assert.equal(status, 1);
  assert.equal(called, null);
  assert.match(stderr, /--delete-branch needs --close/);
});

test("--new refuses a number or a close, which would mean two verbs at once", () => {
  const { status, stderr, called } = run(["--new", "t", "489", "--close"]);
  assert.equal(status, 1);
  assert.equal(called, null);
  assert.match(stderr, /takes no number/);
});

// Issue #1621's handoff: the maintainer's token died at the hour, the diff
// posted through this script, and `gh issue edit` for ready-to-ship
// answered 401. The label is the queue's signal, so it rides the same
// credential as the comment, after it: a label on a thread with no diff
// behind it is a claim the queue ranks first.
test("--label on a number posts the comment first, then the labels through the issues endpoint", () => {
  const { status, calls, verbs } = run(["1621", "--label", "ready-to-ship", "--label", "product"]);
  assert.equal(status, 0);
  assert.deepEqual(verbs, ["issue comment", "api -X"]);
  const [comment, labels] = calls;
  assert.deepEqual(comment.argv.slice(0, 3), ["issue", "comment", "1621"]);
  assert.deepEqual(labels.argv, [
    "api",
    "-X",
    "POST",
    "repos/{owner}/{repo}/issues/1621/labels",
    "-f",
    "labels[]=ready-to-ship",
    "-f",
    "labels[]=product",
  ]);
  assert.equal(labels.token, "workflow-token");
  assert.equal(labels.admin, null);
});

// The refusal removed with `gh issue edit` was the only gate on the verdict:
// `merge-pr` merges on the `agent-approved` label and never asks who set it.
test("--label refuses the reviewer's verdict labels on a number, whatever the case or padding", () => {
  for (
    const label of ["agent-approved", "changes-requested", "Agent-Approved", " agent-approved", "changes-requested "]
  ) {
    const { status, stderr, calls } = run(["1621", "--label", "ready-to-ship", "--label", label]);
    assert.equal(status, 1, label);
    assert.match(stderr, /reviewer's verdict/);
    assert.equal(calls.length, 0, "nothing posts when a label is refused");
  }
});

// An empty name clears the verdict guard, so the comment posts and the
// labels call dies, the two halves apart (#1637): refused before either.
test("--label refuses an empty or blank name before anything posts", () => {
  for (const args of [["1621", "--label", ""], ["1621", "--label", "  "], ["--new", "t", "--label", ""]]) {
    const { status, stderr, calls } = run(args);
    assert.equal(status, 1, JSON.stringify(args));
    assert.match(stderr, /--label needs a name/);
    assert.equal(calls.length, 0, "nothing posts when a label is empty");
  }
});

test("--close with --label closes first, then labels", () => {
  const { status, verbs, calls } = run(["489", "--close", "--label", "wontfix"]);
  assert.equal(status, 0);
  assert.deepEqual(verbs, ["issue close", "api -X"]);
  assert.deepEqual(calls[1].argv.slice(3), ["repos/{owner}/{repo}/issues/489/labels", "-f", "labels[]=wontfix"]);
});

// Three reviewer runs on 2026-10-26 each lost a verdict post to a GitHub
// 5xx and each retried by hand after reading the thread; gh_retry.once is
// that procedure. The stub's GH_STUB_FLAKY names the verb whose first call
// answers 502; GH_STUB_COMMENTS is what the thread read returns.
test("a 5xx on the post is tried once more, after a look at the thread finds nothing", () => {
  const { status, stderr, verbs } = run(["950"], { env: { GH_STUB_FLAKY: "issue comment" } });
  assert.equal(status, 0, stderr);
  assert.deepEqual(verbs, ["issue comment", "api -X", "issue comment"]);
  assert.match(stderr, /gh issue comment on #950 answered a 5xx \(HTTP 502: Server Error.*\); one more try in 0 s/);
});

test("a 5xx whose comment landed is the post, printed and never repeated", () => {
  const sent = run(["950"]).called.argv[4];
  const url = "https://github.com/o/r/issues/950#issuecomment-7";
  const { status, stdout, stderr, verbs } = run(["950"], {
    env: { GH_STUB_FLAKY: "issue comment", GH_STUB_COMMENTS: JSON.stringify([{ body: sent, html_url: url }]) },
  });
  assert.equal(status, 0, stderr);
  assert.deepEqual(verbs, ["issue comment", "api -X"]);
  assert.equal(stdout.trim(), url);
  assert.match(stderr, /but had landed: https:\/\/github.com\/o\/r\/issues\/950#issuecomment-7/);
});

test("a close whose comment landed on a thread still open is tried once more", () => {
  const sent = run(["489", "--close"]).called.argv[4];
  const landed = JSON.stringify([{ body: sent, html_url: "https://github.com/o/r/issues/489#issuecomment-7" }]);
  const open = run(["489", "--close"], {
    env: { GH_STUB_FLAKY: "issue close", GH_STUB_COMMENTS: landed, GH_STUB_STATE: "open" },
  });
  assert.equal(open.status, 0, open.stderr);
  assert.deepEqual(open.verbs, ["issue close", "api -X", "api repos/{owner}/{repo}/issues/489", "issue close"]);
  const closed = run(["489", "--close"], { env: { GH_STUB_FLAKY: "issue close", GH_STUB_COMMENTS: landed } });
  assert.deepEqual(closed.verbs, ["issue close", "api -X", "api repos/{owner}/{repo}/issues/489"]);
});

test("a 5xx on the labels call gets one more try, with no look first", () => {
  const { status, stderr, verbs } = run(["1621", "--label", "ready-to-ship"], { env: { GH_STUB_FLAKY: "api -X" } });
  assert.equal(status, 0, stderr);
  assert.deepEqual(verbs, ["issue comment", "api -X", "api -X"]);
  assert.match(stderr, /the labels call on #1621 answered a 5xx/);
});

test("a 5xx on --new is tried once more unless an open issue already carries the title", () => {
  const url = "https://github.com/o/r/issues/7";
  const found = run(["--new", "Late writes die", "--anyway"], {
    env: { GH_STUB_FLAKY: "issue create" },
    issues: [{ number: 7, title: "Late writes die", url }],
  });
  assert.equal(found.status, 0, found.stderr);
  assert.deepEqual(found.verbs, ["issue create", "issue list"]);
  assert.equal(found.stdout.trim(), url);
  const missing = run(["--new", "Late writes die", "--anyway"], { env: { GH_STUB_FLAKY: "issue create" } });
  assert.deepEqual(missing.verbs, ["issue create", "issue list", "issue create"]);
});

test("a 4xx is the caller's and is not tried again", () => {
  const { status, stderr, verbs } = run(["950"], {
    env: { GH_STUB_FLAKY: "issue comment", GH_STUB_FLAKY_ERROR: "HTTP 422: Validation Failed" },
  });
  assert.equal(status, 1);
  assert.deepEqual(verbs, ["issue comment"]);
  assert.match(stderr, /gh issue comment on #950 failed: HTTP 422/);
});

test("no verb reaches gh without GH_WORKFLOW_TOKEN", () => {
  const { status, called } = run(["--new", "t"], { env: { GH_WORKFLOW_TOKEN: "" } });
  assert.equal(status, 1);
  assert.equal(called, null);
});

test("an empty --new is a typo, and says so instead of asking for a number", () => {
  const { status, stderr, called } = run(["--new", ""]);
  assert.equal(status, 1);
  assert.equal(called, null);
  assert.match(stderr, /needs a title/);
});

// The two env vars above are one capability, not two knobs: GH_WORKFLOW_TOKEN
// is the credential the post rides and ROLE is the seat it is attributed to,
// and gh-comment refuses without either. Four seats exported the token and not
// the role (issue #505), so every one of their posts died on the second check
// after the agent had already done the work. The roster is the authority for
// the footer's name, so the seat list is read from agents/personas/ rather
// than restated here. Scope: presence per workflow file, which is the mistake
// that happened; two agent steps in one file with mismatched env blocks would
// slip through, and no workflow has ever had two.
test("every workflow that can post also says which seat is posting", () => {
  const root = join(new URL("../..", import.meta.url).pathname);
  const seats = readdirSync(join(root, "agents/personas"))
    .filter((f) => f.endsWith(".md") && f !== "README.md")
    .map((f) => f.slice(0, -3));
  const workflows = readdirSync(join(root, ".github/workflows")).filter((f) => f.endsWith(".yml"));
  for (const file of workflows) {
    const text = readFileSync(join(root, ".github/workflows", file), "utf8");
    if (!text.includes("GH_WORKFLOW_TOKEN:")) continue;
    const role = text.match(/^\s*ROLE:\s*(\S+)\s*$/m);
    assert.ok(role, `${file} exports GH_WORKFLOW_TOKEN but no ROLE; gh-comment dies late (issue #505)`);
    assert.ok(
      seats.includes(role[1]),
      `${file} claims ROLE: ${role[1]}, which is no seat in agents/personas/ (${seats.join(", ")})`,
    );
  }
});

// Issue #590: the footer is assembled here, so a body that arrives carrying
// one, in any of the census shapes, leaves with exactly one, the role's.
test("a typed footer is stripped before the role footer goes on", () => {
  const body = "PR #641 passed review.\n\n---\n_Generated by [Claude Code](https://claude.ai/code)_\n\n"
    + "🤖 Generated with [Claude Code](https://claude.com/claude-code)";
  const { status, stderr, called } = run(["683"], { body });
  assert.equal(status, 0);
  assert.equal(called.argv[4].split("[Claude Code]").length - 1, 1);
  assert.match(called.argv[4], /^PR #641 passed review\.\n\n---\n_bdfl · \[Claude Code\]/);
  assert.match(stderr, /stripped 2 typed attribution line/);
});

test("a PR number is a comment target like any issue: gh takes it (#589, #683)", () => {
  assert.deepEqual(run(["683"]).called.argv.slice(0, 3), ["issue", "comment", "683"]);
});

// Issue #771: a review scoped to #768 commented on and merged #770 instead.
// A review session's environment carries PR_NUMBER; nothing outside that
// scope may reach gh, no matter how the wrong number was resolved.
test("PR_NUMBER in the environment refuses a write to any other number", () => {
  const { status, called, stderr } = run(["770"], { env: { PR_NUMBER: "768" } });
  assert.equal(status, 1);
  assert.equal(called, null);
  assert.match(stderr, /does not match PR_NUMBER=768/);
});

test("PR_NUMBER matching the target is unaffected", () => {
  const { status, called } = run(["768"], { env: { PR_NUMBER: "768" } });
  assert.equal(status, 0);
  assert.deepEqual(called.argv.slice(0, 3), ["issue", "comment", "768"]);
});

test("no PR_NUMBER in the environment (every non-review seat) is unaffected", () => {
  const { status, called } = run(["770"]);
  assert.equal(status, 0);
  assert.deepEqual(called.argv.slice(0, 3), ["issue", "comment", "770"]);
});
