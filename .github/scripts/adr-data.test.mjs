// adr-data.py publishes /adr-data.json for site/decisions.html (issue #443):
// the ADR series rendered from docs/adr/ at deploy time, docs/adr/ staying
// the single source. Runs against the real repository tree, the same
// pattern status-data.test.mjs uses against the real ROADMAP.md, plus a
// scratch-directory pass for the failure contract a real tree can't exercise
// on demand (a malformed status line).
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const script = new URL("adr-data.py", import.meta.url).pathname;
const repoRoot = new URL("../../", import.meta.url).pathname;

function generate(...args) {
  const dir = mkdtempSync(join(tmpdir(), "adr-data-"));
  const out = join(dir, "adr-data.json");
  execFileSync("python3", [script, out, ...args], { encoding: "utf-8" });
  return JSON.parse(readFileSync(out, "utf-8"));
}

const data = generate();
const realCount = readdirSync(join(repoRoot, "docs", "adr")).filter((f) => f.endsWith(".md")).length;
const byNumber = Object.fromEntries(data.adrs.map((a) => [a.number, a]));

test("parses every file in docs/adr/ with no problems", () => {
  assert.equal(data.total, realCount);
  assert.deepEqual(data.problems, []);
  assert.equal(data.accepted + data.superseded, data.total);
});

test("sorted ascending by number, the series' own order", () => {
  const numbers = data.adrs.map((a) => a.number);
  assert.deepEqual(numbers, [...numbers].sort());
});

test("every ADR carries the fields the page renders", () => {
  // A stray "<ISO-8601 UTC>" or similar in an ADR's own prose (ADR-0004) is
  // not an HTML tag; only real markup element names are disqualifying.
  const tagLike = /<\/?(p|li|ul|ol|table|thead|tbody|tr|td|th|code|pre|strong|em|a|h[1-6])[ >]/i;
  for (const a of data.adrs) {
    assert.ok(a.title, a.number);
    assert.ok(["accepted", "superseded"].includes(a.status_kind), a.number);
    assert.ok(a.date, a.number);
    assert.ok(a.body_html.length > 0, a.number);
    assert.ok(!tagLike.test(a.body_text), `${a.number} body_text carries a tag`);
  }
});

test("a superseded ADR names the ADR that superseded it, both linkable", () => {
  const a1 = byNumber["0001"];
  assert.equal(a1.status_kind, "superseded");
  assert.equal(a1.superseded_by, "0030");
  assert.ok(byNumber[a1.superseded_by], "superseded_by must be a known ADR");
});

test("a status line wrapped across source lines is not truncated at the break", () => {
  // ADR-0004's Status line wraps onto a second physical line in the file;
  // the continuation ("RESUME-AT clause amended...") must survive as its
  // own relation, not get dropped or fused into the wrong segment.
  const a4 = byNumber["0004"];
  assert.equal(a4.relations_html.length, 2);
  assert.match(a4.relations_html[1], /RESUME-AT clause amended/);
});

test("inline code inside a status-adjacent code span is escaped exactly once", () => {
  // Regression: escaping the whole line before extracting code spans, then
  // escaping the span's own content again, turned `<` into `&amp;lt;`.
  const a4 = byNumber["0004"];
  assert.match(a4.body_html, /<code>RESUME-AT: &lt;ISO-8601 UTC&gt;<\/code>/);
  assert.doesNotMatch(a4.body_html, /&amp;(lt|gt|quot|#x27);/);
});

test("a GFM table becomes a real table, inline formatting included", () => {
  const a18 = byNumber["0018"];
  assert.match(a18.body_html, /<table>/);
  assert.match(a18.body_html, /<strong>No global pause\.<\/strong>/);
});

test("a nested bullet list inside an ordered item renders as a real nested list", () => {
  const a4 = byNumber["0004"];
  const olStart = a4.body_html.indexOf("<ol>");
  const nestedUl = a4.body_html.indexOf("<ul>");
  assert.ok(olStart >= 0 && nestedUl > olStart, "expected <ul> nested inside <ol>");
});

test("an ADR-NNNN mention in prose is linked only when NNNN is a real ADR", () => {
  const a18 = byNumber["0018"];
  assert.match(a18.body_html, /<a href="#0004">ADR-0004<\/a>/);
});

test("a malformed status line is reported as a problem, not a crash", () => {
  const dir = mkdtempSync(join(tmpdir(), "adr-data-fixture-"));
  writeFileSync(
    join(dir, "0001-bad.md"),
    "# ADR-0001: Broken fixture\n\nStatus: pending review · Date: 2026-01-01\n\n## Context\n\nText.\n",
  );
  const out = generate(dir);
  assert.equal(out.total, 0);
  assert.equal(out.problems.length, 1);
  assert.match(out.problems[0], /unrecognised status/);
});

function fixture(files) {
  const dir = mkdtempSync(join(tmpdir(), "adr-data-fixture-"));
  for (const [name, text] of Object.entries(files)) writeFileSync(join(dir, name), text);
  return generate(dir);
}

const adr = (body) => `# ADR-0001: Fixture\n\nStatus: accepted · Date: 2026-01-01\n\n## Context\n\n${body}\n`;

test("a link target cannot end the href attribute or carry a script scheme", () => {
  // The page assigns body_html through innerHTML on the site origin, so a
  // merged ADR's link target is stored script injection unless confined.
  const out = fixture({
    "0001-evil.md": adr(
      "[a](x\" onmouseover=\"alert(1)) [b](javascript:alert(1)) [c](https://ok.example/p?q=1&r=2) [d](`\" onmouseover=\"`)",
    ),
  });
  const html = out.adrs[0].body_html;
  assert.doesNotMatch(html, /<a [^>]*onmouseover/);
  assert.doesNotMatch(html, /href="javascript:/);
  assert.match(html, /<a href="https:\/\/ok\.example\/p\?q=1&amp;r=2">c<\/a>/);
});

test("a link target is not rewritten by the emphasis and ADR-reference passes", () => {
  const out = fixture({
    "0001-a.md": adr("[x](https://ok.example/_a_/ADR-0001/_b_) and ADR-0001"),
  });
  assert.match(out.adrs[0].body_html, /<a href="https:\/\/ok\.example\/_a_\/ADR-0001\/_b_">x<\/a>/);
  assert.match(out.adrs[0].body_html, /and <a href="#0001">ADR-0001<\/a>/);
});

test("an empty file is reported as a problem and the rest still render", () => {
  const out = fixture({
    "0001-ok.md": adr("Text."),
    "0002-empty.md": "",
  });
  assert.equal(out.total, 1);
  assert.equal(out.problems.length, 1);
  assert.match(out.problems[0], /0002-empty\.md/);
});
