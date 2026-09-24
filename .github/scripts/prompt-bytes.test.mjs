// Fixtures for prompt-bytes, the one count #486's audits share.
//
// The last test is the mechanism: it runs the script against this
// repository and fails when any seat's prompt disagrees with its pin. That
// is how a PR that grows a prompt goes red in ci's worker job (the js
// filter watches .github/workflows/ and .github/scripts/) until the pin
// moves in the same diff. The fixtures before it pin the definition, and
// the first one is the 18-byte lesson from #689: bytes are UTF-8 bytes, an
// em dash is three of them, and two readers counting differently is not a
// measurement.
//
// `prompts`, `measure`, `findings` and `render` take their inputs as
// arguments (no filesystem, no cwd) precisely so these can exist.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;
const repoRoot = new URL("../..", import.meta.url).pathname;

const driver = `
import importlib.machinery, importlib.util, io, json, sys
import yaml
loader = importlib.machinery.SourceFileLoader("prompt_bytes", sys.argv[1] + "/prompt-bytes")
spec = importlib.util.spec_from_loader("prompt_bytes", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
fn, raw = sys.argv[2], json.loads(sys.argv[3])
if fn == "prompts":
    print(json.dumps(mod.prompts(yaml.safe_load(raw["yaml"]))))
elif fn == "measure":
    print(json.dumps(mod.measure(raw["text"])))
elif fn == "findings":
    print(json.dumps(mod.findings(raw["seen"], raw["pinned"])))
else:
    buf = io.StringIO()
    n = mod.render(raw["seen"], raw["pinned"], out=buf)
    print(json.dumps({"n": n, "out": buf.getvalue()}))
`;

function call(fn, payload) {
  const run = spawnSync("python3", ["-c", driver, scriptsDir, fn, JSON.stringify(payload)], {
    encoding: "utf8",
  });
  assert.equal(run.status, 0, run.stderr);
  return JSON.parse(run.stdout);
}

const seat = (bytes, over = {}) => ({ bytes, lines: 10, em_dashes: 0, ...over });

test("bytes are UTF-8 bytes, not code points: an em dash is three", () => {
  // "a—b\n" is four code points and six bytes. Counting code points is the
  // 18-byte disagreement on the reviewer (#689): nine em dashes, two bytes
  // each unaccounted for.
  assert.deepEqual(call("measure", { text: "a—b\n" }), { bytes: 6, lines: 1, em_dashes: 1 });
});

test("the parsed scalar is measured: block indent gone, one trailing newline kept", () => {
  const yaml = [
    "jobs:",
    "  seat:",
    "    steps:",
    "      - uses: some/action",
    "        with:",
    "          prompt: |",
    "            first line",
    "            second line",
    "",
  ].join("\n");
  assert.deepEqual(call("prompts", { yaml }), ["first line\nsecond line\n"]);
});

test("a workflow whose steps carry no with.prompt is not a seat", () => {
  // agent-alarm.yml and agent-model-intel.yml: nothing printed, not a zero.
  const yaml =
    "jobs:\n  alarm:\n    steps:\n      - run: echo hi\n      - uses: x\n        with:\n          token: t\n";
  assert.deepEqual(call("prompts", { yaml }), []);
  const yamlNoJobs = "on: push\n";
  assert.deepEqual(call("prompts", { yaml: yamlNoJobs }), []);
});

test("a prompt matching its pin with no em dash is no finding", () => {
  assert.deepEqual(call("findings", { seen: { bdfl: seat(100) }, pinned: { bdfl: 100 } }), []);
});

test("growth names the seat, the delta, and both ways out", () => {
  const [line] = call("findings", { seen: { bdfl: seat(1120) }, pinned: { bdfl: 1000 } });
  assert.match(line, /^bdfl: 1,120 bytes, pinned 1,000, grew 120\./);
  assert.match(line, /Move the pin to 1120 in this PR and say why in its body, or cut 120 bytes\./);
});

test("a shrink is a finding too, so the room cannot hide a regrowth", () => {
  const [line] = call("findings", { seen: { bdfl: seat(900) }, pinned: { bdfl: 1000 } });
  assert.match(line, /shrank 100\. Move the pin to 900 so the room does not hide a regrowth\./);
});

test("an unpinned seat says to add the pin; a pin without a seat says to drop it", () => {
  const lines = call("findings", { seen: { newcomer: seat(42) }, pinned: { retired: 7 } });
  assert.equal(lines.length, 2);
  assert.match(
    lines[0],
    /^newcomer: 42 bytes, no pin\. Add PINNED\['newcomer'\] = 42 in \.github\/scripts\/prompt-bytes\./,
  );
  assert.match(lines[1], /^retired: pinned 7 bytes, no agent-retired\.yml carries a prompt\. Drop its pin\./);
});

test("an em dash is a finding even at the pinned size", () => {
  const lines = call("findings", { seen: { herald: seat(100, { em_dashes: 4 }) }, pinned: { herald: 100 } });
  assert.deepEqual(lines, ["herald: 4 em dash(es). CLAUDE.md Voice: none, ever. Comma, colon, semicolon, period."]);
});

test("render prints the table largest first, the footer, then the findings", () => {
  const { n, out } = call("render", {
    seen: { small: seat(10), big: seat(2000) },
    pinned: { small: 10, big: 1999 },
  });
  assert.equal(n, 1);
  const lines = out.split("\n");
  assert.match(lines[0], /^seat\s+bytes\s+pinned\s+lines\s+em$/);
  assert.match(lines[1], /^big\s+2,000\s+1,999\s+10\s+0$/);
  assert.match(lines[2], /^small\s+10\s+10\s+10\s+0$/);
  assert.match(out, /\nprompt-bytes: seats 2 \| total 2,010 bytes \| pins 2 \| findings 1\n/);
  assert.match(out, /\n    big: 2,000 bytes, pinned 1,999, grew 1\./);
});

test("every seat's prompt on this branch matches its pin", () => {
  // The check itself. Red here means a prompt changed without its pin:
  // the script's own output below says which seat and what to do.
  const run = spawnSync(`${scriptsDir}/prompt-bytes`, [], { cwd: repoRoot, encoding: "utf8" });
  assert.equal(run.status, 0, `${run.stdout}\n${run.stderr}`);
  assert.match(run.stdout, /prompt-bytes: seats 7 \| total [\d,]+ bytes \| pins 7 \| findings 0/);
});
