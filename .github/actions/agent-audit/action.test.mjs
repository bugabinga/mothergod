import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const action = readFileSync(new URL("action.yml", import.meta.url), "utf8");
const marker = "      run: |\n";
const start = action.indexOf(marker);
const end = action.indexOf("\n\n    - if:", start);
assert.notEqual(start, -1, "extract run block must exist");
assert.notEqual(end, -1, "extract run block must end before upload");
const extract = action
  .slice(start + marker.length, end)
  .split("\n")
  .map((line) => line.slice(8))
  .join("\n");

const fixtures = [
  {
    name: "rounds an authoritative seven-day observation",
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
    output: "allowance_index=-u1235-r1800000000\nutilization=0.12345\nresets_at=1800000000\n",
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
    output: "allowance_index=\nutilization=\nresets_at=\n",
  },
  {
    name: "omits the suffix when no observation exists",
    input: [{ type: "result", result: "done" }],
    output: "allowance_index=\nutilization=\nresets_at=\n",
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
    output: "allowance_index=-u4000-r1800000000\nutilization=0.4\nresets_at=1800000000\n",
  },
  {
    name: "omits the suffix for malformed execution data",
    input: "{not json",
    raw: true,
    output: "allowance_index=\nutilization=\nresets_at=\n",
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
    output: "allowance_index=-u2200-r1788386400\nutilization=0.22\nresets_at=1788386400\n",
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
    output: "allowance_index=\nutilization=\nresets_at=\n",
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
    output: "allowance_index=-u2000-r1800000000\nutilization=0.2\nresets_at=1800000000\n",
  },
];

test("agent-audit allowance index fixtures", async (t) => {
  for (const fixture of fixtures) {
    await t.test(fixture.name, () => {
      const directory = mkdtempSync(join(tmpdir(), "agent-audit-test-"));
      try {
        const execution = join(directory, "execution.json");
        const output = join(directory, "github-output");
        writeFileSync(execution, fixture.raw ? fixture.input : JSON.stringify(fixture.input));
        const result = spawnSync("bash", ["-c", extract], {
          cwd: new URL("../../../", import.meta.url),
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
