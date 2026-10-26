// token-sidecar.py against a fake runner and exchange on one local server,
// and app_token.prefer_file, the reader the writers call (#1687). Pins the
// contract both halves share: the OIDC request carries the action's audience
// and the bearer from stdin alone, the exchange gets the OIDC token and
// nothing else, the newest app token is the whole file at mode 0600, a
// failed mint keeps the last file, and no token ever reaches stdout or
// stderr. Every loop test ends on --rounds or --max-minutes, never on a
// count a loaded runner could miss (the reviewer's repro on #1666).
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;
const script = `${scriptsDir}token-sidecar.py`;

// `scenario` scripts the fake: "ok"; "down-once" (the first exchange answers
// 503, the rest mint); "refused" (every exchange answers 401 with the
// action's validation body); "oidc-refused" (the runner answers 401).
function serve(scenario) {
  const seen = { oidc: [], exchange: [] };
  let minted = 0;
  const server = createServer((req, res) => {
    const url = new URL(req.url, "http://x");
    const auth = req.headers.authorization || "";
    let body = "";
    req.on("data", (chunk) => (body += chunk));
    req.on("end", () => {
      if (req.method === "GET" && url.pathname === "/oidc") {
        seen.oidc.push({ audience: url.searchParams.get("audience"), auth });
        if (scenario === "oidc-refused") {
          res.writeHead(401, { "content-type": "application/json" });
          res.end(JSON.stringify({ message: "bad bearer" }));
          return;
        }
        res.writeHead(200, { "content-type": "application/json" });
        res.end(JSON.stringify({ value: `oidc-${seen.oidc.length}` }));
      } else if (req.method === "POST" && url.pathname === "/exchange") {
        seen.exchange.push({ auth, body });
        if (scenario === "down-once" && seen.exchange.length === 1) {
          res.writeHead(503, { "content-type": "application/json" });
          res.end(JSON.stringify({ error: { message: "upstream unavailable" } }));
          return;
        }
        if (scenario === "refused") {
          res.writeHead(401, { "content-type": "application/json" });
          res.end(
            JSON.stringify({
              error: {
                message: "Workflow validation failed.",
                details: { error_code: "workflow_not_found_on_default_branch" },
              },
            }),
          );
          return;
        }
        minted += 1;
        res.writeHead(201, { "content-type": "application/json" });
        const key = minted === 1 ? "token" : "app_token";
        res.end(JSON.stringify({ [key]: `ghs_secret${minted}`, expires_at: `2026-10-26T19:0${minted}:00Z` }));
      } else {
        res.writeHead(404);
        res.end();
      }
    });
  });
  return { server, seen };
}

async function withServer(scenario, fn) {
  const { server, seen } = serve(scenario);
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const base = `http://127.0.0.1:${server.address().port}`;
  const dir = mkdtempSync(join(tmpdir(), "token-sidecar-"));
  try {
    await fn({ base, seen, path: join(dir, "app-token") });
  } finally {
    server.close();
  }
}

// Async, never spawnSync: the fake endpoints serve from this process's
// event loop. The bearer rides stdin, as the workflow step feeds it; the
// environment carries the request URL and a decoy bearer that must not be
// used.
function run(args, { base, env = {}, stdin = "runner-bearer\n" }) {
  return new Promise((resolve) => {
    const child = spawn("python3", [script, "--exchange-url", `${base}/exchange`, ...args], {
      env: {
        PATH: process.env.PATH,
        ACTIONS_ID_TOKEN_REQUEST_URL: `${base}/oidc?api-version=2`,
        ACTIONS_ID_TOKEN_REQUEST_TOKEN: "env-bearer",
        ...env,
      },
    });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (c) => (stdout += c));
    child.stderr.on("data", (c) => (stderr += c));
    child.on("close", (status) => resolve({ status, stdout, stderr }));
    child.stdin.end(stdin);
  });
}

function assertNoSecret(r) {
  for (const needle of ["ghs_secret", "oidc-1", "oidc-2", "runner-bearer", "env-bearer"]) {
    assert.ok(!r.stdout.includes(needle), `stdout leaked ${needle}: ${r.stdout}`);
    assert.ok(!r.stderr.includes(needle), `stderr leaked ${needle}: ${r.stderr}`);
  }
}

test("one round: the action's audience, stdin's bearer, the token whole in a 0600 file, nothing leaked", async () => {
  await withServer("ok", async ({ base, seen, path }) => {
    const r = await run(["--path", path, "--interval", "0", "--rounds", "1"], { base });
    assert.equal(r.status, 0, r.stdout + r.stderr);
    assert.equal(r.stderr, "");
    assertNoSecret(r);
    assert.deepEqual(seen.oidc, [{ audience: "claude-code-github-action", auth: "bearer runner-bearer" }]);
    assert.deepEqual(seen.exchange, [{ auth: "Bearer oidc-1", body: "" }]);
    assert.equal(readFileSync(path, "utf8"), "ghs_secret1\n");
    assert.equal(statSync(path).mode & 0o777, 0o600);
    assert.ok(!existsSync(`${path}.tmp`), "the temporary is renamed away");
    assert.match(r.stdout, /^token-sidecar: started, first mint in 0 min, then every 0$/m);
    assert.match(r.stdout, /^token-sidecar: minted 1 at 0 min: exchange 201, expires_at 2026-10-26T19:01:00Z$/m);
    assert.match(r.stdout, /^token-sidecar: done, 1 mints, --rounds reached$/m);
  });
});

test("a second round replaces the file with the newer token", async () => {
  await withServer("ok", async ({ base, seen, path }) => {
    const r = await run(["--path", path, "--interval", "0", "--rounds", "2"], { base });
    assert.equal(r.status, 0, r.stdout + r.stderr);
    assertNoSecret(r);
    assert.equal(seen.exchange.length, 2);
    assert.equal(seen.exchange[1].auth, "Bearer oidc-2");
    assert.equal(readFileSync(path, "utf8"), "ghs_secret2\n");
    assert.match(r.stdout, /^token-sidecar: minted 2 at 0 min: exchange 201, expires_at 2026-10-26T19:02:00Z$/m);
  });
});

test("an exchange that answers 5xx is one line of statuses, then the retry mints", async () => {
  await withServer("down-once", async ({ base, seen, path }) => {
    const r = await run(["--path", path, "--interval", "0", "--retry", "0", "--rounds", "1"], { base });
    assert.equal(r.status, 0, r.stdout + r.stderr);
    assertNoSecret(r);
    assert.equal(seen.exchange.length, 2);
    assert.match(
      r.stdout,
      /^token-sidecar: mint failed at 0 min: oidc 200, exchange 503, no error code: upstream unavailable; next try in 0 s$/m,
    );
    assert.match(r.stdout, /^token-sidecar: minted 1 at 0 min: exchange 201/m);
    assert.equal(readFileSync(path, "utf8"), "ghs_secret1\n");
  });
});

test("an exchange that refuses keeps the last file; --max-minutes ends the loop, exit 0", async () => {
  await withServer("refused", async ({ base, seen, path }) => {
    writeFileSync(path, "ghs_earlier\n");
    const r = await run(["--path", path, "--interval", "0", "--retry", "0.05", "--max-minutes", "0.003"], { base });
    assert.equal(r.status, 0, r.stdout + r.stderr);
    assertNoSecret(r);
    assert.ok(seen.exchange.length >= 1, `exchanges ${seen.exchange.length}`);
    assert.match(
      r.stdout,
      /^token-sidecar: mint failed at 0 min: oidc 200, exchange 401, workflow_not_found_on_default_branch: Workflow validation failed\.; next try in 0\.05 s$/m,
    );
    assert.match(r.stdout, /^token-sidecar: done, 0 mints, --max-minutes reached$/m);
    assert.equal(readFileSync(path, "utf8"), "ghs_earlier\n");
  });
});

test("a runner that refuses the OIDC request writes no file and names the status", async () => {
  await withServer("oidc-refused", async ({ base, seen, path }) => {
    const r = await run(["--path", path, "--interval", "0", "--retry", "0.05", "--max-minutes", "0.003"], { base });
    assert.equal(r.status, 0, r.stdout + r.stderr);
    assertNoSecret(r);
    assert.equal(seen.exchange.length, 0);
    assert.match(
      r.stdout,
      /^token-sidecar: mint failed at 0 min: oidc 401, no token in the response; next try in 0\.05 s$/m,
    );
    assert.ok(!existsSync(path), "no file without a mint");
  });
});

test("no bearer on stdin, or no request URL: one line, no request, exit 0", async () => {
  await withServer("ok", async ({ base, seen, path }) => {
    const silent = await run(["--path", path, "--interval", "0", "--rounds", "1"], { base, stdin: "" });
    assert.equal(silent.status, 0);
    assert.equal(silent.stdout.trim(), "token-sidecar: skipped, no request token on stdin");
    const noUrl = await run(["--path", path, "--interval", "0", "--rounds", "1"], {
      base,
      env: { ACTIONS_ID_TOKEN_REQUEST_URL: "" },
    });
    assert.equal(noUrl.status, 0);
    assert.match(noUrl.stdout, /^token-sidecar: skipped, no ACTIONS_ID_TOKEN_REQUEST_URL/);
    assert.equal(seen.oidc.length, 0);
    assert.ok(!existsSync(path));
  });
});

// The reader: GH_TOKEN_FILE names the sidecar's file, and the writers point
// GH_TOKEN at it before each gh call. Unset, missing or empty leaves the
// environment exactly as it was.
const readerDriver = `
import json, os, sys
sys.path.insert(0, sys.argv[1])
import app_token

def case(file):
    env = {"GH_TOKEN": "started-with"}
    if file is not None:
        env["GH_TOKEN_FILE"] = file
    got = app_token.prefer_file(env)
    return [got, env["GH_TOKEN"]]

d = sys.argv[2]
print(json.dumps({
    "unset": case(None),
    "missing": case(os.path.join(d, "absent")),
    "empty": case(os.path.join(d, "empty")),
    "minted": case(os.path.join(d, "minted")),
}))
`;

test("prefer_file: the file's token replaces GH_TOKEN; unset, missing or empty changes nothing", () => {
  const dir = mkdtempSync(join(tmpdir(), "prefer-file-"));
  writeFileSync(join(dir, "empty"), "\n");
  writeFileSync(join(dir, "minted"), "ghs_fresh\n");
  const proc = spawnSync("python3", ["-c", readerDriver, scriptsDir, dir], { encoding: "utf8" });
  assert.equal(proc.status, 0, proc.stderr);
  assert.deepEqual(JSON.parse(proc.stdout), {
    unset: [null, "started-with"],
    missing: [null, "started-with"],
    empty: [null, "started-with"],
    minted: ["ghs_fresh", "ghs_fresh"],
  });
});
