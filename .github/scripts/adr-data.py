#!/usr/bin/env python3
"""Generate site/adr-data.json from docs/adr/.

Issue #443: render the ADR series on mothergod.dev, book-like and searchable,
without a second home for the text. docs/adr/ stays the single source; this
script parses it fresh on every deploy and never gets committed, the same
pattern status-data.py and trust-telemetry.py already use for their JSON.

Every ADR in this repository has one exact shape (51 for 51 on
2026-09-24): `# ADR-NNNN: Title`, a blank line, then
`Status: <accepted|superseded by ADR-NNNN> · Date: <ISO date>` optionally
followed by more ` · `-separated relation notes, a blank line, then the body
in `## Heading` sections. ADR-0030 (the format ADR itself) is the source for
that contract: status is exactly `accepted` or `superseded by ADR-NNNN`.
A file that does not match is a parse bug or a new shape this script has not
seen yet, and it is reported in "problems" rather than silently mis-rendered.

Usage: adr-data.py <out.json> [adr_dir]

adr_dir defaults to docs/adr/ and exists as a second argument only so the
test can point the parser at a scratch fixture without touching the real
ADR series.
"""

import html
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ADR_DIR = ROOT / "docs" / "adr"

HEADING_RE = re.compile(r"^# ADR-(\d{4}): (.+)$")
STATUS_RE = re.compile(r"^Status: (.+)$")
ADR_REF_RE = re.compile(r"\bADR-(\d{4})\b")


def adr_link(number, text=None):
    text = text or f"ADR-{number}"
    return f'<a href="#{number}">{html.escape(text)}</a>'


def linkify_adrs(escaped_text, known):
    """Turn `ADR-NNNN` into a link if NNNN is a real ADR, text otherwise.

    Runs on already-escaped text, so `ADR-0006` in the input matches
    literally; nothing here needs a second escaping pass.
    """

    def sub(m):
        number = m.group(1)
        return adr_link(number) if number in known else m.group(0)

    return ADR_REF_RE.sub(sub, escaped_text)


# --- inline markdown: code spans, links, bold, italic, ADR cross-refs -----


def render_inline(text, known):
    escaped = html.escape(text, quote=False)

    codes = []

    def stash_code(m):
        # m.group(1) is already escaped, along with the rest of the line;
        # re-escaping here would double-encode `&lt;` into `&amp;lt;`.
        codes.append(m.group(1))
        return f"\x00{len(codes) - 1}\x00"

    escaped = re.sub(r"`([^`]+)`", stash_code, escaped)

    escaped = re.sub(
        r"\[([^\]]+)\]\(([^)]+)\)", r'<a href="\2">\1</a>', escaped
    )
    escaped = re.sub(r"\*\*(\S(?:[^*]*\S)?)\*\*", r"<strong>\1</strong>", escaped)
    escaped = re.sub(
        r"(?<![\w*])[*_](\S(?:[^*_]*\S)?)[*_](?![\w*])", r"<em>\1</em>", escaped
    )
    escaped = linkify_adrs(escaped, known)

    for i, code in enumerate(codes):
        escaped = escaped.replace(f"\x00{i}\x00", f"<code>{code}</code>")
    return escaped


# --- block markdown: headings, paragraphs, lists, tables, fences ----------


def leading_spaces(line):
    return len(line) - len(line.lstrip(" "))


def parse_table(lines, known):
    def cells(line):
        return [c.strip() for c in line.strip().strip("|").split("|")]

    header = cells(lines[0])
    rows = [cells(l) for l in lines[2:]]
    head = "".join(f"<th>{render_inline(c, known)}</th>" for c in header)
    body = "".join(
        "<tr>" + "".join(f"<td>{render_inline(c, known)}</td>" for c in r) + "</tr>"
        for r in rows
    )
    return f"<table><thead><tr>{head}</tr></thead><tbody>{body}</tbody></table>"


def parse_list(lines, ordered, known):
    marker_re = re.compile(r"^\d+\.\s+(.*)$") if ordered else re.compile(r"^-\s+(.*)$")
    items = []
    i = 0
    while i < len(lines):
        m = marker_re.match(lines[i])
        first = m.group(1)
        rest = []
        i += 1
        while i < len(lines) and not marker_re.match(lines[i]):
            rest.append(lines[i])
            i += 1
        if rest:
            indents = [leading_spaces(l) for l in rest if l.strip()]
            dedent = min(indents) if indents else 0
            dedented = [l[dedent:] if l.strip() else "" for l in rest]
        else:
            dedented = []
        items.append(render_blocks([first] + dedented, known, tight=True))
    tag = "ol" if ordered else "ul"
    return f"<{tag}>" + "".join(f"<li>{it}</li>" for it in items) + f"</{tag}>"


def render_blocks(lines, known, tight=False):
    """Renders a sequence of markdown lines to HTML block content.

    `tight` is set inside a list item: the item's first paragraph renders
    without a `<p>` wrapper, so `<li>text <ul>...</ul></li>` reads as one
    line-item followed by its nested list, matching how these tight lists
    (no blank line between items) render in every other markdown engine.
    """
    out = []
    i = 0
    first_para = True
    while i < len(lines):
        line = lines[i]
        if not line.strip():
            i += 1
            continue
        if line.startswith("```"):
            j = i + 1
            code_lines = []
            while j < len(lines) and not lines[j].startswith("```"):
                code_lines.append(lines[j])
                j += 1
            out.append("<pre><code>" + html.escape("\n".join(code_lines)) + "</code></pre>")
            i = j + 1
            continue
        m = re.match(r"^(#{2,3})\s+(.*)$", line)
        if m:
            level = len(m.group(1))
            out.append(f"<h{level}>{render_inline(m.group(2), known)}</h{level}>")
            i += 1
            continue
        if "|" in line and i + 1 < len(lines) and re.match(r"^\s*\|?[\s:|-]+\|[\s:|-]*$", lines[i + 1]):
            j = i + 2
            while j < len(lines) and "|" in lines[j]:
                j += 1
            out.append(parse_table(lines[i:j], known))
            i = j
            continue
        if re.match(r"^-\s+", line):
            j = i
            while j < len(lines) and (lines[j].strip() == "" or leading_spaces(lines[j]) > 0 or re.match(r"^-\s+", lines[j])):
                if lines[j].strip() == "" and (j + 1 >= len(lines) or leading_spaces(lines[j + 1]) == 0 and not re.match(r"^-\s+", lines[j + 1])):
                    break
                j += 1
            out.append(parse_list(lines[i:j], ordered=False, known=known))
            i = j
            continue
        if re.match(r"^\d+\.\s+", line):
            j = i
            while j < len(lines) and (lines[j].strip() == "" or leading_spaces(lines[j]) > 0 or re.match(r"^\d+\.\s+", lines[j])):
                if lines[j].strip() == "" and (j + 1 >= len(lines) or leading_spaces(lines[j + 1]) == 0 and not re.match(r"^\d+\.\s+", lines[j + 1])):
                    break
                j += 1
            out.append(parse_list(lines[i:j], ordered=True, known=known))
            i = j
            continue
        # Paragraph: consecutive plain lines join with a space (semantic
        # line breaks collapse the way CommonMark's soft breaks do).
        j = i
        para_lines = []
        while j < len(lines) and lines[j].strip() and not re.match(r"^(#{2,3})\s|^```|^-\s+|^\d+\.\s+", lines[j]):
            para_lines.append(lines[j].strip())
            j += 1
        text = render_inline(" ".join(para_lines), known)
        if tight and first_para:
            out.append(text)
        else:
            out.append(f"<p>{text}</p>")
        first_para = False
        i = j
    return "".join(out)


def parse_adr(path, known):
    lines = path.read_text(encoding="utf-8").splitlines()
    m = HEADING_RE.match(lines[0])
    if not m:
        raise ValueError(f"{path.name}: line 1 is not `# ADR-NNNN: Title`")
    number, title = m.groups()

    sm = STATUS_RE.match(lines[2])
    if not sm:
        raise ValueError(f"{path.name}: line 3 is not a `Status:` line")
    # The status line sometimes soft-wraps onto following lines (4 of 51 on
    # 2026-09-24, e.g. ADR-0004's amendment note); join up to the first
    # blank line before splitting into · -separated segments, or a wrapped
    # relation note gets truncated at the line break instead of the marker.
    i = 2
    status_lines = []
    while i < len(lines) and lines[i].strip():
        status_lines.append(lines[i])
        i += 1
    body_start = i
    while body_start < len(lines) and not lines[body_start].strip():
        body_start += 1

    status_full = " ".join(l.strip() for l in status_lines)
    segments = [s.strip() for s in status_full[len("Status: "):].split(" · ")]
    status_raw = segments[0]
    date = None
    relations = []
    for seg in segments[1:]:
        dm = re.match(r"^Date: (.+)$", seg)
        if dm:
            date = dm.group(1)
        else:
            relations.append(seg)

    superseded_by = None
    sup_m = re.match(r"^superseded by ADR-(\d{4})", status_raw)
    if sup_m:
        status_kind = "superseded"
        superseded_by = sup_m.group(1)
    elif status_raw == "accepted":
        status_kind = "accepted"
    else:
        raise ValueError(f"{path.name}: unrecognised status {status_raw!r}")

    body_lines = lines[body_start:]
    body_html = render_blocks(body_lines, known)
    body_text = re.sub(r"<[^>]+>", "", body_html)
    body_text = html.unescape(body_text)

    return {
        "number": number,
        "title": title,
        "status_kind": status_kind,
        "status_html": render_inline(status_raw, known),
        "superseded_by": superseded_by,
        "date": date,
        "relations_html": [render_inline(r, known) for r in relations],
        "body_html": body_html,
        "body_text": body_text,
        "source": f"docs/adr/{path.name}",
    }


def main(out_path, adr_dir=ADR_DIR):
    paths = sorted(Path(adr_dir).glob("*.md"))
    if not paths:
        raise SystemExit(f"no ADRs found under {adr_dir}")

    known = set()
    for p in paths:
        m = HEADING_RE.match(p.read_text(encoding="utf-8").splitlines()[0])
        if m:
            known.add(m.group(1))

    adrs = []
    problems = []
    for p in paths:
        try:
            adrs.append(parse_adr(p, known))
        except Exception as err:  # noqa: BLE001 - diagnosis is the point
            problems.append(f"{p.name}: {type(err).__name__}: {err}")

    adrs.sort(key=lambda a: a["number"])
    data = {
        "total": len(adrs),
        "accepted": sum(a["status_kind"] == "accepted" for a in adrs),
        "superseded": sum(a["status_kind"] == "superseded" for a in adrs),
        "adrs": adrs,
        "problems": problems,
    }
    Path(out_path).write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
    print(
        f"adr-data: wrote {out_path} ({len(adrs)} ADRs, "
        f"{'clean' if not problems else f'{len(problems)} problem(s)'})"
    )
    for problem in problems:
        print(f"  {problem}", file=sys.stderr)


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2] if len(sys.argv) > 2 else ADR_DIR)
