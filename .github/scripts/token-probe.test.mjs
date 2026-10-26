// token-probe.py against a fake runner, exchange and GitHub on one local
// server (#1687). Pins the three contracts the sidecar will inherit: the
// OIDC request carries the action's audience and the runner's bearer, the
// exchange gets the OIDC token and nothing else, every minted token is
// revoked at GitHub, and no token ever reaches stdout or stderr.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;
const script = `${scriptsDir}token-probe.py`;

const validation =
  "Workflow validation failed. The workflow file must exist and have identical content to the version on the repository's default branch.";

// `second` is the exchange's answer to the second round: "ok" or "refused".
function serve(second) {
  const seen = { oidc: [], exchange: [], revoke: [] };
  let minted = 0;
  const server = createServer((req, res) => {
    const url = new URL(req.url, "http://x");
    const auth = req.headers.authorization || "";
    let body = "";
    req.on("data", (chunk) => (body += chunk));
    req.on("end", () => {
      if (req.method === "GET" && url.pathname === "/oidc") {
        seen.oidc.push({ audience: url.searchParams.get("audience"), auth });
        res.writeHead(200, { "content-type": "application/json" });
        res.end(JSON.stringify({ value: `oidc-${seen.oidc.length}` }));
      } else if (req.method === "POST" && url.pathname === "/exchange") {
        seen.exchange.push({ auth, body });
        if (seen.exchange.length === 2 && second === "refused") {
          res.writeHead(401, { "content-type": "application/json" });
          res.end(
            JSON.stringify({
              error: { message: validation, details: { error_code: "workflow_not_found_on_default_branch" } },
            }),
          );
          return;
        }
        minted += 1;
        res.writeHead(201, { "content-type": "application/json" });
        const key = minted === 1 ? "token" : "app_token";
        res.end(JSON.stringify({ [key]: `ghs_secret${minted}`, expires_at: `2026-10-26T19:0${minted}:00Z` }));
      } else if (req.method === "DELETE" && url.pathname === "/installation/token") {
        seen.revoke.push(auth);
        res.writeHead(204);
        res.end();
      } else {
        res.writeHead(404);
        res.end();
      }
    });
  });
  return { server, seen };
}

async function withServer(second, fn) {
  const { server, seen } = serve(second);
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const base = `http://127.0.0.1:${server.address().port}`;
  try {
    await fn(base, seen);
  } finally {
    server.close();
  }
}

// Async, never spawnSync: the fake endpoints serve from this process's
// event loop, which a synchronous spawn would block until the probe's
// own timeout.
function run(args, env) {
  return new Promise((resolve) => {
    const child = spawn("python3", [script, ...args], {
      env: { PATH: process.env.PATH, ...env },
    });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (c) => (stdout += c));
    child.stderr.on("data", (c) => (stderr += c));
    child.on("close", (status) => resolve({ status, stdout, stderr }));
  });
}

function assertNoSecret(r) {
  for (const needle of ["ghs_secret", "oidc-1", "oidc-2", "runner-bearer"]) {
    assert.ok(!r.stdout.includes(needle), `stdout leaked ${needle}: ${r.stdout}`);
    assert.ok(!r.stderr.includes(needle), `stderr leaked ${needle}: ${r.stderr}`);
  }
}

test("two allowed rounds: exit 0, both tokens revoked, nothing leaked", async () => {
  await withServer("ok", async (base, seen) => {
    const r = await run(["--gap", "0", "--exchange-url", `${base}/exchange`, "--github-api", base], {
      ACTIONS_ID_TOKEN_REQUEST_URL: `${base}/oidc?api-version=2`,
      ACTIONS_ID_TOKEN_REQUEST_TOKEN: "runner-bearer",
    });
    assert.equal(r.status, 0, r.stdout + r.stderr);
    assertNoSecret(r);
    assert.match(r.stdout, /^round 1: oidc 200, exchange 201, expires_at 2026-10-26T19:01:00Z, revoke 204$/m);
    assert.match(
      r.stdout,
      /^round 2 \(after 0s\): oidc 200, exchange 201, expires_at 2026-10-26T19:02:00Z, revoke 204$/m,
    );
    assert.match(r.stdout, /^token-probe: rounds 2 \| minted 2 \| re-mint allowed$/m);
    assert.deepEqual(
      seen.oidc.map((o) => o.audience),
      ["claude-code-github-action", "claude-code-github-action"],
    );
    assert.deepEqual(
      seen.oidc.map((o) => o.auth),
      ["bearer runner-bearer", "bearer runner-bearer"],
    );
    assert.deepEqual(
      seen.exchange.map((e) => e.auth),
      ["Bearer oidc-1", "Bearer oidc-2"],
    );
    assert.deepEqual(seen.exchange.map((e) => e.body), ["", ""]);
    assert.deepEqual(seen.revoke, ["Bearer ghs_secret1", "Bearer ghs_secret2"]);
  });
});

test("second round refused: exit 1, the error code on the line, only the first token revoked", async () => {
  await withServer("refused", async (base, seen) => {
    const r = await run(["--gap", "0", "--exchange-url", `${base}/exchange`, "--github-api", base], {
      ACTIONS_ID_TOKEN_REQUEST_URL: `${base}/oidc`,
      ACTIONS_ID_TOKEN_REQUEST_TOKEN: "runner-bearer",
    });
    assert.equal(r.status, 1, r.stdout + r.stderr);
    assertNoSecret(r);
    assert.match(
      r.stdout,
      /^round 2 \(after 0s\): oidc 200, exchange 401, workflow_not_found_on_default_branch: Workflow validation failed/m,
    );
    assert.match(r.stdout, /^token-probe: rounds 2 \| minted 1 \| re-mint refused$/m);
    assert.deepEqual(seen.revoke, ["Bearer ghs_secret1"]);
  });
});

test("no OIDC request variables: exit 2 naming the missing permission", async () => {
  const r = await run(["--gap", "0"], {});
  assert.equal(r.status, 2, r.stdout + r.stderr);
  assert.match(r.stdout, /id-token: write/);
});
