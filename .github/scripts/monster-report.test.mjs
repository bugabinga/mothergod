import assert from "node:assert/strict";
import { test } from "node:test";

import {
  ISSUE_MARKER,
  buildReport,
  failedMatrixJobs,
  firstUsefulFailure,
  selectIssue,
} from "./monster-report.mjs";

test("failure parsing skips runner noise and keeps the first useful diagnostic", () => {
  const log = [
    "2026-08-29T03:20:00.000Z ##[group]Run cargo test --all-targets",
    "2026-08-29T03:20:01.000Z    Compiling mothergod v0.0.1",
    "2026-08-29T03:20:02.000Z test codec::roundtrip ... FAILED",
    "2026-08-29T03:20:02.001Z",
    "2026-08-29T03:20:02.002Z failures:",
    "2026-08-29T03:20:02.003Z ---- codec::roundtrip stdout ----",
    "2026-08-29T03:20:02.004Z thread 'codec::roundtrip' panicked at src/codec.rs:7:5:",
    "2026-08-29T03:20:03.000Z ##[error]Process completed with exit code 101.",
  ].join("\n");

  assert.deepEqual(firstUsefulFailure(log), [
    "test codec::roundtrip ... FAILED",
    "failures:",
    "---- codec::roundtrip stdout ----",
    "thread 'codec::roundtrip' panicked at src/codec.rs:7:5:",
  ]);
});

test("failure parsing falls back deterministically when no diagnostic exists", () => {
  assert.deepEqual(firstUsefulFailure("setup only\n", "timed_out"), [
    "Job ended with timed_out; no diagnostic line was available.",
  ]);
});

test("report generation includes each failed lane once and excludes green lanes", () => {
  const jobs = [
    {
      id: 2,
      name: "windows-11-arm | ARM64 | GNU-LLVM | msrv",
      conclusion: "timed_out",
      html_url: "https://example.test/jobs/2",
    },
    {
      id: 1,
      name: "ubuntu-24.04 | x64 | glibc | stable",
      conclusion: "failure",
      html_url: "https://example.test/jobs/1",
    },
    {
      id: 3,
      name: "macos-26 | ARM64 | Darwin-libc | stable",
      conclusion: "success",
      html_url: "https://example.test/jobs/3",
    },
    {
      id: 4,
      name: "report",
      conclusion: "failure",
      html_url: "https://example.test/jobs/4",
    },
  ];
  const logs = new Map([
    ["1", "error[E0308]: mismatched types\n  --> src/lib.rs:1:1\n"],
    ["2", "runner stopped\n"],
  ]);

  const body = buildReport(jobs, logs, "https://example.test/runs/9");
  assert.ok(body.startsWith(`${ISSUE_MARKER}\n`));
  assert.match(body, /ubuntu-24\.04 \| x64 \| glibc \| stable/);
  assert.match(body, /windows-11-arm \| ARM64 \| GNU-LLVM \| msrv/);
  assert.match(body, /error\[E0308\]: mismatched types/);
  assert.match(body, /Job ended with timed_out/);
  assert.doesNotMatch(body, /macos-26/);
  assert.doesNotMatch(body, /jobs\/4/);
  assert.equal((body.match(/## Failed lanes/g) ?? []).length, 1);
});

test("failed matrix detection ignores cancellation without a failed lane", () => {
  const jobs = [
    {
      id: 1,
      name: "ubuntu-24.04 | x64 | glibc | stable",
      conclusion: "cancelled",
    },
    { id: 2, name: "report", conclusion: "failure" },
  ];

  assert.deepEqual(failedMatrixJobs(jobs), []);
});

test("failed matrix detection includes failures and timeouts", () => {
  const jobs = [
    {
      id: 1,
      name: "ubuntu-24.04 | x64 | glibc | stable",
      conclusion: "failure",
    },
    {
      id: 2,
      name: "Android-15 | x86-64 | Bionic | msrv",
      conclusion: "timed_out",
    },
    {
      id: 3,
      name: "macos-26 | ARM64 | Darwin-libc | stable",
      conclusion: "success",
    },
  ];

  assert.deepEqual(
    failedMatrixJobs(jobs).map(({ job }) => job.id),
    [1, 2],
  );
});

test("dedup selects one bot-authored marked issue deterministically", () => {
  const bot = { login: "github-actions[bot]" };
  const issues = [
    { number: 30, state: "CLOSED", title: "renamed", body: ISSUE_MARKER, author: bot },
    { number: 32, state: "OPEN", title: "renamed", body: ISSUE_MARKER, author: bot },
    { number: 42, state: "OPEN", title: "unrelated", body: "none", author: bot },
  ];

  assert.deepEqual(selectIssue(issues), { number: 32, state: "OPEN" });
});

test("dedup ignores title-only issues", () => {
  assert.equal(
    selectIssue([
      {
        number: 41,
        state: "OPEN",
        title: "🐛 Monster CI failure",
        body: "legacy",
        author: { login: "github-actions[bot]" },
      },
    ]),
    null,
  );
});

test("dedup ignores markers spoofed by non-bot authors", () => {
  assert.equal(
    selectIssue([
      {
        number: 40,
        state: "OPEN",
        title: "renamed",
        body: ISSUE_MARKER,
        author: { login: "octocat" },
      },
    ]),
    null,
  );
});

test("dedup ignores labeled non-bot issues", () => {
  assert.equal(
    selectIssue([
      {
        number: 39,
        state: "OPEN",
        title: "🐛 Monster CI failure",
        body: ISSUE_MARKER,
        author: { login: "octocat" },
        labels: [{ name: "bug" }, { name: "agent-system" }],
      },
    ]),
    null,
  );
});
