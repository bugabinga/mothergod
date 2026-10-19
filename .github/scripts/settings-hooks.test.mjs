// Every synchronous PreToolUse command hook in .claude/settings.json must
// carry `"onFailure": "block"`. The deny hooks gate a tool call by exiting 2;
// without the field a hook that crashes or times out fails open, silently,
// and the guard is gone with nothing red (#1315 was the first hook the field
// turned from fail-open to fail-closed; #1316 found that no test read it, so
// the next hook added without the field passed every gate). An async hook
// runs detached and cannot block, which is why `tg-draft` carries no field
// and why no deny hook may be async.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const settingsPath = new URL("../../.claude/settings.json", import.meta.url).pathname;
const settings = JSON.parse(readFileSync(settingsPath, "utf8"));

const commandHooks = (event) =>
  (settings.hooks?.[event] ?? []).flatMap((group) => (group.hooks ?? []).filter((hook) => hook.type === "command"));

test("every synchronous PreToolUse command hook blocks on failure", () => {
  const hooks = commandHooks("PreToolUse");
  assert.ok(hooks.length > 0, "no PreToolUse command hook read: the file or its shape moved");
  for (const hook of hooks.filter((hook) => !hook.async)) {
    assert.equal(hook.onFailure, "block", `${hook.command} lacks "onFailure": "block"`);
  }
});

test("an async PreToolUse hook cannot deny, so none is a deny-* script", () => {
  for (const hook of commandHooks("PreToolUse").filter((hook) => hook.async)) {
    assert.doesNotMatch(hook.command, /\/deny-/, `${hook.command} runs detached and cannot block`);
  }
});
