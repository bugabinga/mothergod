// ROADMAP.md's M3 section sorts journal ids into three sentences, and a
// status word it carries is read as the verdict. Nothing compared the
// sentences with the journal's own headers: #1514 filed four `LEAD` ids as
// exhausted (#1520), and the live-thread paragraph read "no standing lead
// is open" for four days after S2-A127 shipped (#1580, #1583). This holds
// each sentence to the headers.
//
// Lives here, not in tests/claims.rs: a claims input has to be in ci.yml's
// `rust` filter or a roadmap-only PR skips the check and main goes red on
// merge (#1009), and `worker` runs on every PR with no filter (#1451).
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const repoRoot = new URL("../..", import.meta.url).pathname;

// The journal statuses each opener admits, as the first word of a header's
// status field ("ACCEPTED, unwired" and "RESOLVED 2026-08-29, closed by ..."
// both begin with a word listed here).
const SENTENCES = [
  { opener: "Resolved, by id:", admits: ["RESOLVED", "ACCEPTED"] },
  { opener: "Exhausted, each under its own id", admits: ["EXHAUSTED", "REJECTED"] },
  { opener: "Live thread:", admits: ["LEAD"], first: true },
];

// A journal id, optionally the start of a range inside one series
// ("S2-A127 to S2-A129" names S2-A128 too).
const ID = /\b(S\d-[A-Z]+)(\d+)\b(?: to \1(\d+)\b)?/g;

// M3's paragraphs as single lines, from `## M3` to the next `## `.
function m3Paragraphs(roadmap) {
  const start = roadmap.indexOf("## M3");
  assert.notEqual(start, -1, "ROADMAP.md has an `## M3` section");
  const end = roadmap.indexOf("\n## ", start + 1);
  return roadmap
    .slice(start, end === -1 ? undefined : end)
    .split(/\n\n+/)
    .map((paragraph) => paragraph.replace(/\s+/g, " "));
}

// The ids in `text`, ranges expanded. With `first`, only the ids of the
// first parenthesis group: the live thread's own lead, not the resolved or
// exhausted ids its prose cites.
function idsIn(text, first) {
  const scope = (first ? text.match(/\(([^)]*)\)/)?.[1] : text) ?? "";
  const ids = [];
  for (const [, series, from, to] of scope.matchAll(ID)) {
    for (let n = Number(from); n <= Number(to ?? from); n += 1) ids.push(`${series}${n}`);
  }
  return ids;
}

// Each sentence's ids paired with its admitted statuses. A sentence runs
// from its opener to the end of its paragraph or to the next opener.
function claims(roadmap) {
  const paragraphs = m3Paragraphs(roadmap);
  return SENTENCES.map(({ opener, admits, first }) => {
    const paragraph = paragraphs.find((candidate) => candidate.includes(opener));
    assert.ok(paragraph, `M3 has no sentence opening "${opener}"`);
    const from = paragraph.indexOf(opener);
    const next = SENTENCES.map((other) => paragraph.indexOf(other.opener, from + 1))
      .filter((at) => at !== -1)
      .sort((a, b) => a - b)[0];
    return { opener, admits, ids: idsIn(paragraph.slice(from, next), first) };
  });
}

// The first word of the status field of id's journal header
// (`- <id> | <STATUS> | ...`), or undefined without a header.
function status(journal, id) {
  return journal.match(new RegExp(`^- ${id} \\| ([A-Z]+)`, "m"))?.[1];
}

// One line per id whose status its sentence does not admit, and one per
// sentence that names no id, so a revert reports every defect at once.
function violations(roadmap, journal) {
  return claims(roadmap).flatMap(({ opener, admits, ids }) =>
    ids.length === 0
      ? [`"${opener}" names no journal id`]
      : ids
        .filter((id) => !admits.includes(status(journal, id)))
        .map(
          (id) =>
            `${id} under "${opener}" has journal status ${status(journal, id) ?? "none (no header)"}, admitted: ${
              admits.join(", ")
            }`,
        )
  );
}

const JOURNAL = `
- S1-P1 | RESOLVED 2026-08-29, closed by S2-A60 | x
- S1-P2 | EXHAUSTED 2026-09-22 | x
- S1-P4 | LEAD | x
- S2-A127 | ACCEPTED | x
- S2-A128 | ACCEPTED, unwired | x
- S2-A129 | REJECTED | x
- S2-R33 | REJECTED | x
`;

const ROADMAP = `## M3: Close

Intro text. Resolved, by id:
SSE (S1-P1), the reach (S2-A127 to
S2-A128). Exhausted, each under its own id: parse (S1-P2),
rate gating (S2-R33).

Live thread: large windows (S1-P4), the one \`LEAD\` left among S1-P1 to S1-P8.

## M4
`;

test("ids expand within a series and the live thread reads only its own parenthesis", () => {
  assert.deepEqual(
    claims(ROADMAP).map(({ ids }) => ids),
    [["S1-P1", "S2-A127", "S2-A128"], ["S1-P2", "S2-R33"], ["S1-P4"]],
  );
});

test("a roadmap whose sentences match the headers has no violations", () => {
  assert.deepEqual(violations(ROADMAP, JOURNAL), []);
});

test("a range's interior id is checked, not just its endpoints", () => {
  const journal = JOURNAL.replace("S2-A128 | ACCEPTED, unwired", "S2-A128 | REJECTED");
  assert.equal(violations(ROADMAP, journal).length, 1);
  assert.match(violations(ROADMAP, journal)[0], /^S2-A128 under "Resolved, by id:" .* REJECTED/);
});

test("an id under the wrong sentence is named with both statuses", () => {
  const roadmap = ROADMAP.replace("parse (S1-P2)", "parse (S1-P4)");
  assert.match(violations(roadmap, JOURNAL)[0], /^S1-P4 under "Exhausted.*LEAD, admitted: EXHAUSTED, REJECTED$/);
});

test("an id without a journal header is a violation, not a skip", () => {
  const roadmap = ROADMAP.replace("(S1-P4)", "(S9-P9)");
  assert.match(violations(roadmap, JOURNAL)[0], /^S9-P9 under "Live thread:" .* none/);
});

test("a live thread that names no id is a violation instead of passing empty", () => {
  const roadmap = ROADMAP.replace("large windows (S1-P4)", "large windows");
  assert.deepEqual(violations(roadmap, JOURNAL), ["\"Live thread:\" names no journal id"]);
});

test("a renamed opener fails by design", () => {
  assert.throws(() => claims(ROADMAP.replace("Resolved, by id:", "Done:")), /no sentence opening "Resolved, by id:"/);
});

test("ROADMAP.md's M3 sentences match research/JOURNAL.md's headers", () => {
  const roadmap = readFileSync(`${repoRoot}/ROADMAP.md`, "utf8");
  const journal = readFileSync(`${repoRoot}/research/JOURNAL.md`, "utf8");
  assert.deepEqual(violations(roadmap, journal), []);
});
