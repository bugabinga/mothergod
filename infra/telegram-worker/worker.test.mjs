// Full webhook routing at the two real boundaries: fake KV and fake fetch.
// No command test calls GitHub or Telegram, and ordinary chat is exercised
// through the same exported fetch handler Cloudflare invokes.
//
// Run: node --test infra/telegram-worker/*.test.mjs
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { after, test } from "node:test";
import worker, { CLOCK, Typing } from "./worker.js";

const originalFetch = globalThis.fetch;
const originalNow = Date.now;
after(() => {
  globalThis.fetch = originalFetch;
  Date.now = originalNow;
});

const json = (body, status = 200, headers = {}) =>
  new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json", ...headers },
  });

function harness(github = () => json({})) {
  const calls = [];
  const kv = new Map();
  const typing = [];
  const objects = new Map();
  globalThis.fetch = async (input, init = {}) => {
    const url = String(input);
    calls.push({ url, init });
    if (url.startsWith("https://api.telegram.org/")) return json({ ok: true });
    return github(new URL(url), init);
  };
  const env = {
    WEBHOOK_SECRET: "webhook-secret",
    OPERATOR_CHAT_ID: "7",
    BOT_TOKEN: "bot-token",
    GITHUB_PAT: "github-token",
    GITHUB_REPO: "owner/repo",
    INBOX: {
      async get(key) {
        return kv.get(key) ?? null;
      },
      async put(key, value) {
        kv.set(key, value);
      },
    },
    TYPING: {
      idFromName(name) {
        return name;
      },
      get(id) {
        if (!objects.has(id)) {
          const values = new Map();
          const storage = {
            async get(key) {
              return values.get(key);
            },
            async put(key, value) {
              if (typeof key === "object") {
                for (const [name, entry] of Object.entries(key)) values.set(name, entry);
              } else {
                values.set(key, value);
              }
            },
            async deleteAll() {
              values.clear();
            },
            async setAlarm() {},
            async deleteAlarm() {},
          };
          objects.set(id, new Typing({ storage }, env));
        }
        return {
          async fetch(input, init) {
            if (id === "operator") typing.push(String(input));
            const request = input instanceof Request ? input : new Request(input, init);
            return objects.get(id).fetch(request);
          },
        };
      },
    },
  };
  return { calls, env, kv, typing };
}

async function invoke(text, setup = harness(), options = {}) {
  const update = {
    update_id: options.updateId ?? 42,
    message: {
      message_id: options.messageId ?? 9,
      date: 1_700_000_000,
      chat: { id: options.chat ?? 7 },
      text,
    },
  };
  const request = new Request("https://bot.mothergod.dev/", {
    method: "POST",
    headers: {
      "content-type": "application/json",
      "x-telegram-bot-api-secret-token": options.secret ?? "webhook-secret",
    },
    body: options.body ?? JSON.stringify(update),
  });
  const response = await worker.fetch(request, setup.env);
  return { ...setup, response };
}

function githubPaths(calls) {
  return calls
    .filter((call) => call.url.startsWith("https://api.github.com/"))
    .map((call) => new URL(call.url).pathname + new URL(call.url).search);
}

function sent(calls) {
  const call = calls.find((entry) => entry.url.endsWith("/sendMessage"));
  assert.ok(call, "the command must reply through sendMessage");
  return JSON.parse(call.init.body);
}

function run(overrides = {}) {
  return {
    name: "agent-bdfl",
    run_number: 12,
    status: "completed",
    conclusion: "success",
    created_at: "2026-08-24T12:30:00Z",
    ...overrides,
  };
}

test("Telegram command and prose routes", async (t) => {
  await t.test("help and unknown slash commands reply without KV, typing, or GitHub", async () => {
    for (const text of ["/help", "/unknown anything", "/help@mothergod_bot"]) {
      const result = await invoke(text);
      assert.equal(result.response.status, 200);
      const reply = sent(result.calls);
      assert.match(reply.text, /mothergod commands/);
      assert.match(reply.text, /\/digest/);
      assert.deepEqual(reply.reply_parameters, { message_id: 9 });
      assert.deepEqual(reply.link_preview_options, { is_disabled: true });
      assert.equal(result.kv.size, 0);
      assert.deepEqual(result.typing, []);
      assert.deepEqual(githubPaths(result.calls), []);
    }
  });

  await t.test("invalid secret and non-operator requests remain silent", async () => {
    const badSecret = await invoke("/help", harness(), { secret: "wrong" });
    assert.equal(badSecret.response.status, 401);
    assert.equal(badSecret.calls.length, 0);
    assert.equal(badSecret.kv.size, 0);

    const stranger = await invoke("/help", harness(), { chat: 8 });
    assert.equal(stranger.response.status, 200);
    assert.equal(stranger.calls.length, 0);
    assert.equal(stranger.kv.size, 0);
  });

  await t.test("ordinary prose keeps the KV, reaction, typing, memory, and BDFL dispatch path", async () => {
    const result = await invoke("please inspect this");
    assert.equal(result.response.status, 200);
    assert.equal(result.typing.length, 1);
    assert.equal(result.typing[0], "https://typing.invalid/start");
    assert.equal(JSON.parse(result.kv.get("u:000000000042")).message.text, "please inspect this");
    assert.deepEqual(JSON.parse(result.kv.get("chatlog")), [
      {
        from: "operator",
        message_id: 9,
        date: 1_700_000_000,
        text: "please inspect this",
      },
    ]);
    assert.ok(result.calls.some((call) => call.url.endsWith("/setMessageReaction")));
    assert.deepEqual(githubPaths(result.calls), [
      "/repos/owner/repo/actions/workflows/agent-bdfl.yml/dispatches",
    ]);
    assert.ok(!result.calls.some((call) => call.url.endsWith("/sendMessage")));
  });

  await t.test("command argument validation never reaches GitHub", async () => {
    const cases = [
      ["/pause 0", /1–168/],
      ["/pause 1.5", /1–168/],
      ["/run nobody", /Usage: \/run/],
      ["/run reviewer", /event-driven/],
      ["/runs nobody", /Usage: \/runs/],
      ["/diff zero", /Usage: \/diff/],
      ["/status extra", /Usage: \/status/],
      ["/digest extra", /Usage: \/digest/],
    ];
    for (const [command, expected] of cases) {
      const result = await invoke(command);
      assert.match(sent(result.calls).text, expected);
      assert.deepEqual(githubPaths(result.calls), []);
      assert.equal(result.kv.size, 0);
    }
  });

  await t.test("status combines pause, queue, and each workflow's latest run", async () => {
    const setup = harness((url, init) => {
      assert.equal(init.headers.authorization, "Bearer github-token");
      if (url.pathname.endsWith("/issues") && url.searchParams.get("labels") === "agents-paused") {
        return json([{ number: 22 }]);
      }
      if (url.pathname.endsWith("/pulls")) return json([{ number: 1 }, { number: 2 }]);
      if (url.pathname.endsWith("/issues")) {
        return json([{ number: 3 }, { number: 4, pull_request: {} }]);
      }
      if (url.pathname.endsWith("/actions/workflows/ci.yml/runs")) {
        return json({ workflow_runs: [run()] });
      }
      if (url.pathname.includes("/actions/workflows/")) {
        const active = url.pathname.includes("agent-research.yml");
        return json({
          workflow_runs: active
            ? [run({ name: "agent-research", status: "in_progress", conclusion: null })]
            : [],
        });
      }
      return json({}, 404);
    });
    const result = await invoke("/status", setup);
    const body = sent(result.calls).text;
    assert.match(body, /Agents paused by .*#22/);
    assert.match(body, /CI: ✅ green/);
    assert.match(body, /Open: 1 issues, 2 PRs/);
    assert.match(body, /Running: agent-research/);
    assert.equal(githubPaths(result.calls).length, 9);
    assert.equal(
      githubPaths(result.calls).filter(
        (path) => path.includes("/actions/workflows/") && !path.includes("/ci.yml/"),
      ).length,
      5,
    );
  });

  await t.test("status reports every CI state and rejects malformed run data", async () => {
    const cases = [
      [{ workflow_runs: [run()] }, /CI: ✅ green/],
      [{ workflow_runs: [run({ conclusion: "failure" })] }, /CI: ❌ failing \(failure\)/],
      [
        { workflow_runs: [run({ status: "queued", conclusion: null })] },
        /CI: ⏳ pending \(queued\)/,
      ],
      [{ workflow_runs: [] }, /CI: ○ absent/],
      [{ workflow_runs: [{}] }, /\/status unavailable/],
    ];
    for (const [ci, expected] of cases) {
      const result = await invoke(
        "/status",
        harness((url) => {
          if (url.pathname.endsWith("/actions/workflows/ci.yml/runs")) return json(ci);
          if (url.pathname.includes("/actions/workflows/")) {
            return json({ workflow_runs: [] });
          }
          return json([]);
        }),
      );
      assert.match(sent(result.calls).text, expected);
    }
  });

  await t.test("pause validates existing state and creates the guard-compatible issue", async () => {
    Date.now = () => Date.parse("2026-08-24T10:00:00Z");
    let created;
    const setup = harness((url, init) => {
      if (url.pathname.endsWith("/issues") && init.method === "GET") return json([]);
      if (url.pathname.endsWith("/issues") && init.method === "POST") {
        created = JSON.parse(init.body);
        return json({ number: 41 }, 201);
      }
      return json({}, 404);
    });
    const result = await invoke("/pause 6", setup);
    assert.deepEqual(created.labels, ["agents-paused"]);
    assert.match(created.body, /^Paused by the operator through Telegram\.\n\nRESUME-AT: 2026-08-24T16:00:00Z$/m);
    assert.match(sent(result.calls).text, /paused for 6h.*#41/);

    const existing = await invoke(
      "/pause 6",
      harness((url, init) => {
        assert.equal(init.method, "GET");
        return json([{ number: 41 }]);
      }),
    );
    assert.match(sent(existing.calls).text, /Already paused by .*#41/);
    assert.equal(githubPaths(existing.calls).length, 1);
  });

  await t.test("resume closes the existing pause and is idempotent", async () => {
    let patch;
    const setup = harness((url, init) => {
      if (init.method === "GET") return json([{ number: 41 }]);
      patch = JSON.parse(init.body);
      return json({ number: 41, state: "closed" });
    });
    const result = await invoke("/resume", setup);
    assert.deepEqual(patch, { state: "closed", state_reason: "completed" });
    assert.match(sent(result.calls).text, /closing .*#41/);
    assert.deepEqual(githubPaths(result.calls), [
      "/repos/owner/repo/issues?state=open&labels=agents-paused&per_page=1",
      "/repos/owner/repo/issues/41",
    ]);

    const active = await invoke("/resume", harness(() => json([])));
    assert.match(sent(active.calls).text, /already active/);
    assert.equal(githubPaths(active.calls).length, 1);
  });

  await t.test("run dispatches only a manually runnable agent workflow", async () => {
    let dispatch;
    const result = await invoke(
      "/run maintainer",
      harness((url, init) => {
        dispatch = { path: url.pathname, method: init.method, body: JSON.parse(init.body) };
        return new Response(null, { status: 204 });
      }),
    );
    assert.deepEqual(dispatch, {
      path: "/repos/owner/repo/actions/workflows/agent-heartbeat.yml/dispatches",
      method: "POST",
      body: { ref: "main" },
    });
    assert.match(sent(result.calls).text, /Dispatched maintainer/);
  });

  await t.test("duplicate run delivery dispatches once and replays its reply", async () => {
    const setup = harness(() => new Response(null, { status: 204 }));
    await Promise.all([
      invoke("/run maintainer", setup, { updateId: 51 }),
      invoke("/run maintainer", setup, { updateId: 51 }),
    ]);
    assert.equal(
      githubPaths(setup.calls).filter((path) => path.endsWith("/dispatches")).length,
      1,
    );
    const replies = setup.calls
      .filter((call) => call.url.endsWith("/sendMessage"))
      .map((call) => JSON.parse(call.init.body).text);
    assert.deepEqual(replies, ["▶️ Dispatched maintainer.", "▶️ Dispatched maintainer."]);
  });

  await t.test("concurrent duplicate pause delivery creates one issue", async () => {
    let creates = 0;
    const setup = harness((url, init) => {
      if (init.method === "GET") return json([]);
      if (init.method === "POST" && url.pathname.endsWith("/issues")) {
        creates += 1;
        return json({ number: 41 }, 201);
      }
      return json({}, 404);
    });
    await Promise.all([
      invoke("/pause 6", setup, { updateId: 52 }),
      invoke("/pause 6", setup, { updateId: 52 }),
    ]);
    assert.equal(creates, 1);
    const replies = setup.calls
      .filter((call) => call.url.endsWith("/sendMessage"))
      .map((call) => JSON.parse(call.init.body).text);
    assert.equal(replies.length, 2);
    assert.equal(replies[0], replies[1]);
  });

  await t.test("budget derives recent burn from the immediately preceding observation", async () => {
    const artifacts = [
      {
        name: "audit-bdfl-1-1-u8300-r1800000000",
        created_at: "2026-08-24T02:00:00Z",
        expired: false,
      },
      {
        name: "audit-reviewer-3-1-u8100-r1800000000",
        created_at: "2026-08-24T01:00:00Z",
        expired: false,
      },
      {
        name: "audit-maintainer-2-1-u8000-r1800000000",
        created_at: "2026-08-24T00:00:00Z",
        expired: false,
      },
      { name: "audit-bad-u99999-r1800000000", created_at: "not-a-date" },
      { name: "not-an-audit-u100-r1800000000", created_at: "2026-08-23T00:00:00Z" },
    ];
    const result = await invoke(
      "/budget",
      harness((url) => {
        assert.equal(url.pathname, "/repos/owner/repo/actions/artifacts");
        assert.equal(url.searchParams.get("per_page"), "100");
        return json({ artifacts });
      }),
    );
    const body = sent(result.calls).text;
    assert.match(body, /83\.00% used, 17\.00% remaining/);
    assert.match(body, /Recent burn: 2\.00%\/h over 1\.0h/);
    assert.match(body, /Projected exhaustion: 2026-08-24 10:30 UTC \(before reset\)/);
    assert.ok(githubPaths(result.calls).every((path) => !path.includes("/zip")));
  });

  await t.test("budget reports missing observations and missing movement honestly", async () => {
    const none = await invoke(
      "/budget",
      harness(() => json({ artifacts: [{ name: "audit-bdfl-1-1", created_at: "2026-08-24T00:00:00Z" }] })),
    );
    assert.match(sent(none.calls).text, /no indexed allowance observations/);

    const one = await invoke(
      "/budget",
      harness(() =>
        json({
          artifacts: [
            {
              name: "audit-bdfl-1-1-u5000-r1800000000",
              created_at: "2026-08-24T00:00:00Z",
            },
          ],
        })
      ),
    );
    assert.match(sent(one.calls).text, /Recent burn: no measurable indexed movement/);
    assert.match(sent(one.calls).text, /Projected exhaustion: unavailable/);
  });

  await t.test("runs merges per-workflow history or lists one role", async () => {
    const all = await invoke(
      "/runs",
      harness((url) => {
        const observations = {
          "agent-bdfl.yml": run({ created_at: "2026-08-24T12:00:00Z" }),
          "agent-heartbeat.yml": run({
            name: "agent-heartbeat",
            created_at: "2026-08-24T11:00:00Z",
          }),
          "agent-review.yml": run({ name: "agent-review", created_at: "2026-08-24T10:00:00Z" }),
          "agent-research.yml": run({
            name: "agent-research",
            created_at: "2026-08-24T14:00:00Z",
          }),
          "agent-deslop.yml": run({ name: "agent-deslop", created_at: "2026-08-24T13:00:00Z" }),
        };
        const workflow = url.pathname.split("/").at(-2);
        return json({ workflow_runs: [observations[workflow]] });
      }),
    );
    const allBody = sent(all.calls).text;
    assert.match(allBody, /Recent agent runs/);
    assert.ok(allBody.indexOf("researcher") < allBody.indexOf("deslopper"));
    assert.deepEqual(githubPaths(all.calls), [
      "/repos/owner/repo/actions/workflows/agent-bdfl.yml/runs?per_page=5",
      "/repos/owner/repo/actions/workflows/agent-heartbeat.yml/runs?per_page=5",
      "/repos/owner/repo/actions/workflows/agent-review.yml/runs?per_page=5",
      "/repos/owner/repo/actions/workflows/agent-research.yml/runs?per_page=5",
      "/repos/owner/repo/actions/workflows/agent-deslop.yml/runs?per_page=5",
    ]);

    const role = await invoke(
      "/runs reviewer",
      harness(() => json({ workflow_runs: [run({ name: "agent-review" })] })),
    );
    assert.match(sent(role.calls).text, /Recent reviewer runs/);
    assert.deepEqual(githubPaths(role.calls), [
      "/repos/owner/repo/actions/workflows/agent-review.yml/runs?per_page=5",
    ]);
  });

  await t.test("blocked lists at most ten items and reports overflow exactly", async () => {
    const result = await invoke(
      "/blocked",
      harness((url) => {
        assert.equal(url.searchParams.get("per_page"), "11");
        return json([{ number: 197, title: "Choose allowance policy <now>" }]);
      }),
    );
    const body = sent(result.calls).text;
    assert.match(body, /href="https:\/\/github.com\/owner\/repo\/issues\/197"/);
    assert.match(body, /Choose allowance policy &lt;now&gt;/);

    const ten = await invoke(
      "/blocked",
      harness(() => json(Array.from({ length: 10 }, (_, index) => ({ number: index + 1, title: "x" })))),
    );
    assert.match(sent(ten.calls).text, /Blocked on human \(10\):/);

    const eleven = await invoke(
      "/blocked",
      harness(() => json(Array.from({ length: 11 }, (_, index) => ({ number: index + 1, title: "x" })))),
    );
    assert.match(sent(eleven.calls).text, /Blocked on human \(10\+\):/);
    assert.doesNotMatch(sent(eleven.calls).text, /issues\/11/);
  });

  await t.test("diff validates the PR and summarizes API-authored totals and files", async () => {
    const result = await invoke(
      "/diff 42",
      harness((url) => {
        if (url.pathname.endsWith("/pulls/42")) {
          return json({ title: "Mechanical commands", changed_files: 2, additions: 30, deletions: 4 });
        }
        return json([
          { filename: "worker.js", status: "modified", additions: 20, deletions: 4 },
          { filename: "worker.test.mjs", status: "added", additions: 10, deletions: 0 },
        ]);
      }),
    );
    const body = sent(result.calls).text;
    assert.match(body, /PR .*#42<\/a>: Mechanical commands/);
    assert.match(body, /\+30 −4 across 2 files/);
    assert.match(body, /M worker\.js \+20 −4/);
    assert.equal(githubPaths(result.calls).length, 2);
  });

  await t.test("agents reads each workflow's actual latest run", async () => {
    const result = await invoke(
      "/agents",
      harness((url) => {
        const observations = {
          "agent-bdfl.yml": run(),
          "agent-heartbeat.yml": run({ name: "agent-heartbeat", run_number: 13 }),
          "agent-review.yml": run({ name: "agent-review", run_number: 14, conclusion: "failure" }),
          "agent-research.yml": run({ name: "agent-research", run_number: 15 }),
          "agent-deslop.yml": run({ name: "agent-deslop", run_number: 16 }),
        };
        return json({ workflow_runs: [observations[url.pathname.split("/").at(-2)]] });
      }),
    );
    const body = sent(result.calls).text;
    for (const role of ["bdfl", "maintainer", "reviewer", "researcher", "deslopper"]) {
      assert.match(body, new RegExp(`^${role} run \\d+:`, "m"));
    }
    assert.doesNotMatch(body, /no run found/);
    assert.ok(githubPaths(result.calls).every((path) => path.endsWith("/runs?per_page=1")));
    assert.equal(githubPaths(result.calls).length, 5);
  });

  await t.test("digest reads the latest comments page and strips the generated footer", async () => {
    const result = await invoke(
      "/digest",
      harness((url) => {
        if (url.pathname.endsWith("/issues")) return json([{ number: 8 }]);
        if (!url.searchParams.has("page")) {
          return json(
            [{ body: "old", created_at: "2026-08-23T00:00:00Z" }],
            200,
            {
              link: "<https://api.github.com/repos/owner/repo/issues/8/comments?per_page=100&page=2>; rel=\"last\"",
            },
          );
        }
        return json([
          {
            body: "Shipped #42.\nNext: verify.\n\n---\n_Generated by [Claude Code](https://claude.ai/code)_",
            created_at: "2026-08-24T12:00:00Z",
          },
        ]);
      }),
    );
    const body = sent(result.calls).text;
    assert.match(body, /Latest digest/);
    assert.match(body, /Shipped .*#42/);
    assert.doesNotMatch(body, /Generated by/);
    assert.deepEqual(githubPaths(result.calls), [
      "/repos/owner/repo/issues?state=open&labels=ops-log&per_page=1",
      "/repos/owner/repo/issues/8/comments?per_page=100",
      "/repos/owner/repo/issues/8/comments?per_page=100&page=2",
    ]);
  });

  await t.test("GitHub HTTP, transport, and malformed-data failures become replies, never BDFL wakes", async () => {
    const cases = [
      () => harness(() => json({ message: "down" }, 503)),
      () => harness(() => json({ not_workflow_runs: [] })),
      () =>
        harness(() => {
          throw new Error("network down");
        }),
    ];
    for (const makeSetup of cases) {
      const result = await invoke("/runs", makeSetup());
      assert.equal(result.response.status, 200);
      assert.match(sent(result.calls).text, /\/runs unavailable/);
      assert.equal(result.kv.size, 0);
      assert.deepEqual(result.typing, []);
      assert.ok(!githubPaths(result.calls).some((path) => path.endsWith("/dispatches")));
    }
  });

  await t.test("a failed Telegram command reply does not trigger webhook retries or the BDFL", async () => {
    const setup = harness();
    globalThis.fetch = async (input, init = {}) => {
      const url = String(input);
      setup.calls.push({ url, init });
      if (url.endsWith("/sendMessage")) return json({ ok: false }, 500);
      return json({});
    };
    const result = await invoke("/help", setup);
    assert.equal(result.response.status, 200);
    assert.equal(result.kv.size, 0);
    assert.deepEqual(githubPaths(result.calls), []);
  });
});

// Cadence is a budget-governed quantity (ADR-0027) that moves without
// warning, so no test may spell a cron expression: a test that hardcodes
// one is a third copy of a value that already lives in wrangler.toml and
// CLOCK, and it fails on the next allowance lever instead of on a defect.
// Fixtures name a seat and look up its expression.
const cronFor = (workflow) => Object.keys(CLOCK).find((cron) => CLOCK[cron].some((seat) => seat.workflow === workflow));

test("Clock ticks (ADR-0035)", async (t) => {
  const tick = (setup, cron) => worker.scheduled({ cron, scheduledTime: 1_700_000_000_000 }, setup.env);
  const clocklog = (setup) => JSON.parse(setup.kv.get("clocklog"));

  await t.test("every wrangler trigger has a CLOCK entry, and none is orphaned", () => {
    // The one invariant the CLOCK comment states and nothing enforced: a
    // cron in wrangler.toml with no CLOCK key wakes nobody, silently, and
    // a CLOCK key with no trigger never fires. Both halves are the same
    // typo, found here rather than by a seat that stops running.
    const wrangler = readFileSync(new URL("./wrangler.toml", import.meta.url), "utf8");
    // Pull the quoted strings, never split on commas ("49 6,18 * * *"
    // carries one inside its quotes) and never assume one line: the
    // formatter reflows the array once it outgrows the line width.
    const crons = [
      ...wrangler.match(/crons\s*=\s*\[([^\]]*)\]/)[1]
        .matchAll(/"([^"]*)"/g),
    ].map((m) => m[1]);
    assert.deepEqual(crons.slice().sort(), Object.keys(CLOCK).sort());
    // The Workers Free plan caps cron triggers at 5 per account (API
    // error 10072). A sixth line deploys the script, then fails the
    // schedule update, leaving the new seat clock-dead (deploy run
    // 33727271158, 2026-09-03). Seats share ticks via CLOCK lists.
    assert.ok(crons.length <= 5, `Workers Free allows 5 cron triggers; got ${crons.length}`);
  });

  await t.test("a tick that wakes a governed seat says it was the clock", async () => {
    // `source: cron` is load-bearing twice over: the seat reports
    // TRIGGER_EVENT=schedule downstream, and agent-guard reads it to decide
    // whether the allowance governor may skip this wake (ADR-0039). Both
    // seats the governor throttles must carry it, or the second gear is
    // wired to a lever nothing pulls.
    for (const workflow of ["agent-bdfl.yml", "agent-heartbeat.yml", "agent-herald.yml", "agent-research.yml"]) {
      const setup = harness(() => new Response(null, { status: 204 }));
      const cron = cronFor(workflow);
      await tick(setup, cron);
      const dispatch = setup.calls.find((call) => call.url.includes("/dispatches"));
      assert.ok(dispatch.url.includes(workflow), `${cron} must wake ${workflow}`);
      assert.deepEqual(JSON.parse(dispatch.init.body), {
        ref: "main",
        inputs: { source: "cron" },
      });
      assert.deepEqual(clocklog(setup), [
        { cron, at: "2023-11-14T22:13:20.000Z", woke: [workflow], failed: [] },
      ]);
    }
  });

  await t.test("the shared deslop/curator tick wakes both, each with its own inputs", async () => {
    // One expression, two seats (Workers Free caps crons at 5). The
    // deslopper stays ungoverned by choice: two wakes a day is not
    // where the allowance goes, and a seat with no `source` input
    // would reject one (ADR-0039). The curator is governed like any
    // discretionary wake.
    const setup = harness(() => new Response(null, { status: 204 }));
    const cron = cronFor("agent-deslop.yml");
    await tick(setup, cron);
    const dispatches = setup.calls.filter((call) => call.url.includes("/dispatches"));
    const bodyFor = (workflow) => JSON.parse(dispatches.find((call) => call.url.includes(workflow)).init.body);
    assert.deepEqual(bodyFor("agent-deslop.yml"), { ref: "main" });
    assert.deepEqual(bodyFor("agent-curator.yml"), {
      ref: "main",
      inputs: { source: "cron" },
    });
    assert.deepEqual(clocklog(setup)[0].woke, ["agent-deslop.yml", "agent-curator.yml"]);
  });

  await t.test("a failed dispatch is logged as failed, never thrown", async () => {
    const setup = harness(() => json({ message: "down" }, 503));
    await tick(setup, cronFor("agent-bdfl.yml"));
    assert.deepEqual(clocklog(setup)[0].failed, ["agent-bdfl.yml"]);
    assert.deepEqual(clocklog(setup)[0].woke, []);
  });

  await t.test("an expression missing from CLOCK wakes nothing but leaves evidence", async () => {
    const setup = harness(() => new Response(null, { status: 204 }));
    await tick(setup, "59 13 * * *");
    assert.ok(!setup.calls.some((call) => call.url.includes("/dispatches")));
    assert.deepEqual(clocklog(setup)[0], {
      cron: "59 13 * * *",
      at: "2023-11-14T22:13:20.000Z",
      woke: [],
      failed: [],
    });
  });

  await t.test("the log trims to 48 ticks and a corrupt log restarts", async () => {
    const setup = harness(() => new Response(null, { status: 204 }));
    setup.kv.set(
      "clocklog",
      JSON.stringify(Array.from({ length: 48 }, (_, i) => ({ cron: String(i) }))),
    );
    const cron = cronFor("agent-bdfl.yml");
    await tick(setup, cron);
    const log = clocklog(setup);
    assert.equal(log.length, 48);
    assert.equal(log[0].cron, "1");
    assert.equal(log.at(-1).cron, cron);

    setup.kv.set("clocklog", "not json");
    await tick(setup, cron);
    assert.equal(clocklog(setup).length, 1);
  });
});
