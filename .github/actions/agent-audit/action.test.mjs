import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

// The extraction under test is the real file the action runs
// (issue #310 ended the era of slicing it back out of the YAML),
// invoked as the action's bash step invokes it: output directory as
// argv[1], everything else by environment.
const script = new URL("../../scripts/audit-extract.py", import.meta.url).pathname;

const fixtures = [
  {
    name: "writes an authoritative seven-day observation",
    input: [
      {
        type: "rate_limit_event",
        rate_limit_info: {
          rateLimitType: "seven_day",
          utilization: 0.12345,
          resetsAt: 1_800_000_000,
        },
      },
    ],
    output: "utilization=0.12345\nresets_at=1800000000\n",
  },
  {
    name: "ignores malformed rate-limit events",
    input: [
      { type: "rate_limit_event", rate_limit_info: "bad" },
      {
        type: "rate_limit_event",
        rate_limit_info: { rateLimitType: "seven_day", utilization: true, resetsAt: 1_800_000_000 },
      },
      {
        type: "rate_limit_event",
        rate_limit_info: { rateLimitType: "seven_day", utilization: 1.1, resetsAt: 1_800_000_000 },
      },
      {
        type: "rate_limit_event",
        rate_limit_info: { rateLimitType: "seven_day", utilization: 0.5, resetsAt: false },
      },
    ],
    output: "utilization=\nresets_at=\n",
  },
  {
    name: "writes empty outputs when no observation exists",
    input: [{ type: "result", result: "done" }],
    output: "utilization=\nresets_at=\n",
  },
  {
    name: "does not let an overage limit replace the shared allowance",
    input: [
      {
        type: "rate_limit_event",
        rate_limit_info: {
          rateLimitType: "seven_day",
          utilization: 0.4,
          resetsAt: 1_800_000_000,
        },
      },
      {
        type: "rate_limit_event",
        rate_limit_info: {
          rateLimitType: "seven_day_overage_included",
          utilization: 0.9,
          resetsAt: 1_900_000_000,
        },
      },
    ],
    output: "utilization=0.4\nresets_at=1800000000\n",
  },
  {
    name: "writes empty outputs for malformed execution data",
    input: "{not json",
    raw: true,
    output: "utilization=\nresets_at=\n",
  },
  {
    name: "reads the nested unifiedWindows shape (post-2026-08-26 payloads)",
    input: [
      {
        type: "rate_limit_event",
        rate_limit_info: {
          rateLimitType: "five_hour",
          resetsAt: 1_787_933_400,
          unifiedWindows: {
            five_hour: { utilization: 0.17, resetsAt: 1_787_933_400 },
            seven_day: { utilization: 0.22, resetsAt: 1_788_386_400 },
          },
        },
      },
    ],
    output: "utilization=0.22\nresets_at=1788386400\n",
  },
  {
    name: "nested shape present means flat fields are ignored",
    input: [
      {
        type: "rate_limit_event",
        rate_limit_info: {
          rateLimitType: "seven_day",
          utilization: 0.9,
          resetsAt: 1_800_000_000,
          unifiedWindows: {
            seven_day: { utilization: true, resetsAt: 1_800_000_000 },
          },
        },
      },
    ],
    output: "utilization=\nresets_at=\n",
  },
  {
    name: "a healthy sibling window cannot stand in for a malformed seven_day",
    input: [
      {
        type: "rate_limit_event",
        rate_limit_info: {
          rateLimitType: "five_hour",
          unifiedWindows: {
            five_hour: { utilization: 0.5, resetsAt: 1_800_000_000 },
            seven_day: { utilization: true, resetsAt: 1_800_000_000 },
          },
        },
      },
    ],
    output: "utilization=\nresets_at=\n",
  },
  {
    name: "last valid seven-day reading wins across mixed shapes",
    input: [
      {
        type: "rate_limit_event",
        rate_limit_info: { rateLimitType: "seven_day", utilization: 0.1, resetsAt: 1_800_000_000 },
      },
      {
        type: "rate_limit_event",
        rate_limit_info: {
          rateLimitType: "five_hour",
          unifiedWindows: {
            seven_day: { utilization: 0.2, resetsAt: 1_800_000_000 },
          },
        },
      },
    ],
    output: "utilization=0.2\nresets_at=1800000000\n",
  },
];

// Liveness for the pattern scrub (issue #425): plant token-shaped
// fakes in the transcript and prompt, assert none survive into the
// published files. Fakes are built by concatenation so no
// token-shaped literal lives in the repo to trip secret scanning.
test("token-shaped strings are scrubbed from every audit file", () => {
  const directory = mkdtempSync(join(tmpdir(), "agent-audit-test-"));
  try {
    const out = join(directory, "agent-audit");
    mkdirSync(out);
    const execution = join(directory, "execution.json");
    const output = join(directory, "github-output");
    const fakes = [
      "ghs_" + "A1".repeat(18),
      "github_pat_" + "B2".repeat(15),
      "sk-ant-" + "c3".repeat(15),
      "123456789:" + "Dd_-".repeat(9),
    ];
    const transcript = `pushed via https://x-access-token:${fakes[0]}@github.com/x/y, `
      + `called https://api.telegram.org/bot${fakes[3]}/sendMessage, `
      + `saw ${fakes[1]} and ${fakes[2]} in the environment`;
    writeFileSync(execution, JSON.stringify([{ type: "result", result: transcript }]));
    mkdirSync(join(directory, "claude-prompts"));
    writeFileSync(
      join(directory, "claude-prompts", "claude-prompt.txt"),
      `prompt carrying ${fakes[0]} too`,
    );
    const result = spawnSync("python3", [script, out], {
      encoding: "utf8",
      env: {
        ...process.env,
        EXEC_FILE: execution,
        GITHUB_OUTPUT: output,
        GITHUB_WORKSPACE: new URL("../../../", import.meta.url).pathname,
        RUNNER_TEMP: directory,
        ROLE: "bdfl",
      },
    });
    assert.equal(result.status, 0, result.stderr || result.stdout);
    for (const name of ["output-response.md", "input-prompt.md"]) {
      const body = readFileSync(join(out, name), "utf8");
      for (const fake of fakes) {
        assert.ok(!body.includes(fake), `${name} leaks ${fake}`);
      }
    }
    assert.ok(
      readFileSync(join(out, "output-response.md"), "utf8").includes("***REDACTED***"),
      "scrub marker missing: the patterns matched nothing",
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

// Run 35842553330 (2026-09-23): the retrospect read "denied 1 (Edit)" and
// nothing else, for a session refused a protected-path write it then died
// asking about. The typed system entry carries the wall; this pins that
// the audit keeps it, once per distinct wall, capped, beside the count
// the result entry already gave.
test("a denial's typed reason reaches metadata.json, distinct and capped", () => {
  const directory = mkdtempSync(join(tmpdir(), "agent-audit-test-"));
  try {
    const out = join(directory, "agent-audit");
    mkdirSync(out);
    const execution = join(directory, "execution.json");
    const wall = {
      type: "system",
      subtype: "permission_denied",
      tool_name: "Edit",
      tool_use_id: "toolu_1",
      decision_reason_type: "protectedPath",
      decision_reason: "x".repeat(300),
    };
    writeFileSync(
      execution,
      JSON.stringify([
        wall,
        { ...wall, tool_use_id: "toolu_2" },
        { ...wall, tool_use_id: "toolu_3", decision_reason: { hook: "deny-em-dash" }, decision_reason_type: "hook" },
        {
          type: "result",
          result: "done",
          permission_denials: [{ tool_name: "Edit", tool_input: {} }, { tool_name: "Edit", tool_input: {} }, {
            tool_name: "Edit",
            tool_input: {},
          }],
        },
      ]),
    );
    const result = spawnSync("python3", [script, out], {
      encoding: "utf8",
      env: {
        ...process.env,
        EXEC_FILE: execution,
        GITHUB_OUTPUT: join(directory, "github-output"),
        GITHUB_WORKSPACE: new URL("../../../", import.meta.url).pathname,
        RUNNER_TEMP: directory,
        ROLE: "maintainer",
      },
    });
    assert.equal(result.status, 0, result.stderr || result.stdout);
    const denials = JSON.parse(readFileSync(join(out, "metadata.json"), "utf8")).telemetry.permission_denials;
    assert.equal(denials.count, 3, "the result entry's count stays authoritative");
    assert.deepEqual(denials.tools, ["Edit"]);
    assert.equal(denials.reasons.length, 2, "one wall hit twice is one line");
    assert.equal(denials.reasons[0].type, "protectedPath");
    assert.equal(denials.reasons[0].reason.length, 240, "capped");
    assert.deepEqual(denials.reasons[1], { tool: "Edit", type: "hook", reason: "{\"hook\": \"deny-em-dash\"}" });
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

// Issue #725: `--allowedTools` pre-approves and does not hide, so a seat is
// offered Write and Edit it may not use, and the record that could prove a
// `--tools` trim did what it says did not exist. The init entry's `tools`
// is the offered set; tool_use blocks are the uses, counted once per block
// id because stream-json may repeat an assistant message per block. Names
// and counts only, never inputs.
test("offered tools and tool-use counts reach metadata.json, names only", () => {
  const directory = mkdtempSync(join(tmpdir(), "agent-audit-test-"));
  try {
    const out = join(directory, "agent-audit");
    mkdirSync(out);
    const execution = join(directory, "execution.json");
    const use = (id, name) => ({
      type: "assistant",
      message: { content: [{ type: "tool_use", id, name, input: { command: "secret-looking input" } }] },
    });
    writeFileSync(
      execution,
      JSON.stringify([
        { type: "system", subtype: "init", tools: ["Write", "Bash", "Read", "Bash"] },
        use("toolu_1", "Bash"),
        use("toolu_1", "Bash"),
        use("toolu_2", "Bash"),
        { type: "assistant", message: { content: [{ type: "text", text: "reading" }] } },
        { ...use("toolu_3", "Read"), parent_tool_use_id: "toolu_2" },
        { type: "result", result: "done" },
      ]),
    );
    const result = spawnSync("python3", [script, out], {
      encoding: "utf8",
      env: {
        ...process.env,
        EXEC_FILE: execution,
        GITHUB_OUTPUT: join(directory, "github-output"),
        GITHUB_WORKSPACE: new URL("../../../", import.meta.url).pathname,
        RUNNER_TEMP: directory,
        ROLE: "reviewer",
      },
    });
    assert.equal(result.status, 0, result.stderr || result.stdout);
    const metadata = readFileSync(join(out, "metadata.json"), "utf8");
    assert.deepEqual(JSON.parse(metadata).telemetry.tools, {
      offered: ["Bash", "Read", "Write"],
      used: { Bash: 2, Read: 1 },
      writes: 0,
    });
    assert.ok(!metadata.includes("secret-looking"), "a tool input leaked into the record");
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("an execution log without an init entry records the offered set as unknown", () => {
  const directory = mkdtempSync(join(tmpdir(), "agent-audit-test-"));
  try {
    const out = join(directory, "agent-audit");
    mkdirSync(out);
    const execution = join(directory, "execution.json");
    writeFileSync(execution, JSON.stringify([{ type: "result", result: "done" }]));
    const result = spawnSync("python3", [script, out], {
      encoding: "utf8",
      env: {
        ...process.env,
        EXEC_FILE: execution,
        GITHUB_OUTPUT: join(directory, "github-output"),
        GITHUB_WORKSPACE: new URL("../../../", import.meta.url).pathname,
        RUNNER_TEMP: directory,
        ROLE: "reviewer",
      },
    });
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.deepEqual(JSON.parse(readFileSync(join(out, "metadata.json"), "utf8")).telemetry.tools, {
      offered: null,
      used: {},
      writes: 0,
    });
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

// Issue #896 item 5: a run that changed nothing is a count of zero, so a
// seat's no-op share is a query over the session table and not a posture.
// A write is one of the four posting and shipping scripts, or `gh issue
// edit` or `gh pr edit` (the curator and the reviewer act by label and
// milestone), at command position, or a github_file_ops commit or delete,
// whose result was not an error. Reading a script, naming its test file, a
// denied call and a non-zero exit are not writes; a call that never got a
// result is counted, because it may have landed. The command text never
// reaches the record.
test("write-shaped tool calls are counted, errored and denied ones are not, inputs never leak", () => {
  const directory = mkdtempSync(join(tmpdir(), "agent-audit-test-"));
  try {
    const out = join(directory, "agent-audit");
    mkdirSync(out);
    const execution = join(directory, "execution.json");
    const bash = (id, command) => ({
      type: "assistant",
      message: { content: [{ type: "tool_use", id, name: "Bash", input: { command } }] },
    });
    const result = (id, is_error) => ({
      type: "user",
      message: { content: [{ type: "tool_result", tool_use_id: id, content: "secret-looking output", is_error }] },
    });
    writeFileSync(
      execution,
      JSON.stringify([
        { type: "system", subtype: "init", tools: ["Bash", "mcp__github_file_ops__commit_files"] },
        bash("w1", "cd /r && .github/scripts/gh-comment 3 <<'EOF'\nsecret-looking body\nEOF"),
        result("w1", false),
        bash("w2", "GH_TOKEN=\"$T\" timeout 1500 .github/scripts/merge-pr 12 --wait --sha abc"),
        result("w2", false),
        bash("w3", "git add -A\n.github/scripts/push-branch"),
        result("w3", false),
        bash("w4", "$(.github/scripts/gh-pr claude/x \"t\" <<EOF\nbody\nEOF\n)"),
        {
          type: "assistant",
          message: {
            content: [{
              type: "tool_use",
              id: "w5",
              name: "mcp__github_file_ops__commit_files",
              input: { message: "secret-looking message" },
            }],
          },
        },
        result("w5", false),
        bash("e1", ".github/scripts/gh-comment 3 <<'EOF'\nx\nEOF"),
        result("e1", true),
        bash("d1", ".github/scripts/gh-pr claude/y \"t\""),
        {
          type: "system",
          subtype: "permission_denied",
          tool_name: "Bash",
          tool_use_id: "d1",
          decision_reason_type: "hook",
          decision_reason: "deny-em-dash",
        },
        bash("r1", "sed -n '1,80p' .github/scripts/gh-comment"),
        result("r1", false),
        bash("r2", "node --test .github/scripts/gh-pr.test.mjs .github/scripts/merge-pr.test.mjs"),
        result("r2", false),
        bash("r3", ".github/scripts/tg-send <<'EOF'\nstatus\nEOF"),
        result("r3", false),
        bash("r4", "gh issue list --label ledger"),
        result("r4", false),
        bash("l1", "gh issue edit 3 --add-label ready-to-ship"),
        result("l1", false),
        bash("l2", "gh pr edit 4 --milestone M --title 'sharper'"),
        result("l2", false),
        bash("c1", "if true; then GH_TOKEN=\"a b\" .github/scripts/gh-comment 3; fi"),
        result("c1", false),
        bash("c2", "timeout -k 5 10 .github/scripts/merge-pr 1"),
        result("c2", false),
        bash("c3", "bash .github/scripts/push-branch"),
        result("c3", false),
        bash("c4", "env X=1 .github/scripts/gh-comment 3"),
        result("c4", false),
        bash("k1", ".github/scripts/push-branch"),
        { type: "result", result: "done", permission_denials: [{ tool_name: "Bash" }] },
      ]),
    );
    const r = spawnSync("python3", [script, out], {
      encoding: "utf8",
      env: {
        ...process.env,
        EXEC_FILE: execution,
        GITHUB_OUTPUT: join(directory, "github-output"),
        GITHUB_WORKSPACE: new URL("../../../", import.meta.url).pathname,
        RUNNER_TEMP: directory,
        ROLE: "bdfl",
      },
    });
    assert.equal(r.status, 0, r.stderr || r.stdout);
    const metadata = readFileSync(join(out, "metadata.json"), "utf8");
    const tools = JSON.parse(metadata).telemetry.tools;
    assert.equal(tools.writes, 12, JSON.stringify(tools));
    assert.deepEqual(tools.used, { Bash: 17, mcp__github_file_ops__commit_files: 1 });
    assert.ok(!metadata.includes("secret-looking"), "a tool input or result leaked into the record");
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("agent-audit ledger output fixtures", async (t) => {
  for (const fixture of fixtures) {
    await t.test(fixture.name, () => {
      const directory = mkdtempSync(join(tmpdir(), "agent-audit-test-"));
      try {
        const out = join(directory, "agent-audit");
        mkdirSync(out);
        const execution = join(directory, "execution.json");
        const output = join(directory, "github-output");
        writeFileSync(execution, fixture.raw ? fixture.input : JSON.stringify(fixture.input));
        const result = spawnSync("python3", [script, out], {
          encoding: "utf8",
          env: {
            ...process.env,
            EXEC_FILE: execution,
            GITHUB_OUTPUT: output,
            GITHUB_WORKSPACE: new URL("../../../", import.meta.url).pathname,
            RUNNER_TEMP: directory,
            ROLE: "bdfl",
          },
        });
        assert.equal(result.status, 0, result.stderr || result.stdout);
        assert.equal(readFileSync(output, "utf8"), fixture.output);
      } finally {
        rmSync(directory, { recursive: true, force: true });
      }
    });
  }
});
