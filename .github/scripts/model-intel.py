#!/usr/bin/env python3
"""Compare Artificial Analysis model scores against our ladders (ADR-0019).

Reads a raw API response, writes issue.md, summary.md and verdict into an
output directory. Prints nothing that could carry a credential: the API key
never reaches this script, only the response it fetched.

Self-diagnosing by design: an unexpected envelope, missing fields, or
unreadable JSON produce a report naming what was actually seen and exit 0,
because a garbage issue is worse than no issue and a failed run is worse
than both.
"""
import hashlib, json, os, re, sys

raw_path, out_dir = sys.argv[1], sys.argv[2]
os.makedirs(out_dir, exist_ok=True)

def write(name, text):
    with open(os.path.join(out_dir, name), "w", encoding="utf-8") as fh:
        fh.write(text)

def bail(reason, detail=""):
    # Self-diagnosis over silence: an unexpected payload shape reports what
    # it actually saw instead of posting a garbage issue or failing the run.
    body = f"# Model capability: no report\n\n{reason}\n\n{detail}\n"
    write("summary.md", body)
    write("issue.md", body)
    write("verdict", "unchanged")
    sys.exit(0)

try:
    payload = json.load(open(raw_path, encoding="utf-8"))
except (OSError, ValueError) as exc:
    bail(f"Response was not readable JSON ({exc}).")

# The response envelope is not pinned by the docs, so find the model list
# rather than assume a key.
rows = None
if isinstance(payload, list):
    rows = payload
elif isinstance(payload, dict):
    for key in ("data", "models", "results", "items"):
        if isinstance(payload.get(key), list):
            rows = payload[key]
            break
if not rows:
    keys = ", ".join(sorted(payload)) if isinstance(payload, dict) else type(payload).__name__
    bail("Could not locate the model list in the response.",
         f"Top-level keys seen: `{keys}`. Adjust the extractor to match.")

INDEX = "artificial_analysis_intelligence_index"

def ident(row):
    for k in ("slug", "id", "model_slug", "name", "model"):
        v = row.get(k)
        if isinstance(v, str) and v:
            return v
    return ""

def dig(row, name):
    """Find a numeric metric by exact key, at the top level or one nesting in.

    Their live payload puts the indices under `evaluations` rather than on the
    entry, while carrying a top-level `..._index_cost` that is a different
    number entirely (run 32635384862). Matching the key exactly, and only
    then descending, means a sibling metric can never be mistaken for the
    score. Structural rather than a guess at their layout.
    """
    v = row.get(name)
    if isinstance(v, (int, float)) and not isinstance(v, bool):
        return float(v)
    for nested in row.values():
        if isinstance(nested, dict):
            v = nested.get(name)
            if isinstance(v, (int, float)) and not isinstance(v, bool):
                return float(v)
    return None

def score(row):
    return dig(row, INDEX)

models = []
for row in rows:
    if not isinstance(row, dict):
        continue
    name, s = ident(row), score(row)
    if name and s is not None:
        models.append({"id": name, "score": s,
                       "coding": dig(row, "artificial_analysis_coding_index"),
                       "agentic": dig(row, "artificial_analysis_agentic_index")})
if not models:
    sample = rows[0] if isinstance(rows[0], dict) else {}
    detail = [f"Keys on the first entry: `{', '.join(sorted(sample)) or 'none'}`."]
    for k, v in sorted(sample.items()):
        if isinstance(v, dict) and v:
            detail.append(f"Keys under `{k}`: `{', '.join(sorted(v))}`.")
    bail(f"No entry carried both an identifier and `{INDEX}`, "
         "at the top level or one nesting in.", "\n\n".join(detail))

models.sort(key=lambda m: -m["score"])

try:
    roles = json.load(open("agents/models.json"))["roles"]
except (OSError, ValueError, KeyError) as exc:
    bail(f"agents/models.json unreadable ({exc}).")

ladders = {r: (v or {}).get("ladder") or [] for r, v in roles.items()}

def norm(s):
    return re.sub(r"[^a-z0-9]", "", s.lower())

def same_model(a, b):
    """Normalized containment in EITHER direction.

    Our ids are Anthropic's, theirs are their own slugs, and neither side
    is reliably the longer one: they publish `claude-opus-5-xhigh` for our
    `claude-opus-5`, and a catalogue dropping the vendor prefix would
    publish `opus-5` instead. Every comparison goes through here so
    resolution and exclusion can never disagree. They did once: checking
    one direction only reported a ladder's own floor rung as a brand-new
    model beating its top rung (PR #112 review).
    """
    na, nb = norm(a), norm(b)
    return na == nb or na in nb or nb in na

def match(rung):
    for m in models:
        if same_model(rung, m["id"]):
            return m
    return None

findings, mapping = [], []
for role, ladder in sorted(ladders.items()):
    if not ladder:
        continue
    top = ladder[0]
    hit = match(top)
    mapping.append((role, top, hit["id"] if hit else None, hit["score"] if hit else None))
    if not hit:
        continue
    # Findings drive an issue post, so they are restricted to models this
    # project can actually call: authentication is a Claude subscription
    # (ADR-0004). Without this the job fires most weeks on a competitor
    # release the BDFL cannot act on, which is the alert fatigue ADR-0019
    # exists to avoid.
    better = [m for m in models
              if m["score"] > hit["score"]
              and "claude" in m["id"].lower()
              and not any(same_model(m["id"], r) for r in ladder)]
    for m in better:
        findings.append((role, top, hit["score"], m["id"], m["score"]))

fingerprint = hashlib.sha256(
    json.dumps(sorted((f[0], f[3], f[4]) for f in findings)).encode()
).hexdigest()[:16]

prior = os.environ.get("PRIOR", "")
seen = re.search(r"<!-- fingerprint: ([0-9a-f]+) -->", prior)
unchanged = bool(seen) and seen.group(1) == fingerprint

lines = ["# Model capability snapshot", ""]
lines.append("Source: [Artificial Analysis](https://artificialanalysis.ai/). "
             "Attribution is required on every tier of their API, including free.")
lines.append("")
lines.append("## Ladder top rungs, as resolved against their catalogue")
lines.append("")
lines.append("| Role | Our top rung | Matched their entry | Intelligence |")
lines.append("|---|---|---|---|")
for role, top, hit_id, hit_score in mapping:
    lines.append(f"| {role} | `{top}` | {f'`{hit_id}`' if hit_id else '**no match**'} "
                 f"| {hit_score if hit_score is not None else 'n/a'} |")
lines.append("")
if any(h is None for _, _, h, _ in mapping):
    lines.append("A rung showing **no match** is far more likely a naming mismatch between "
                 "their slugs and ours than a retired model. Confirm the mapping before "
                 "reading anything into it; this job never treats a miss as a retirement.")
    lines.append("")
if findings:
    lines.append("## Scoring above a ladder top rung, and not on that ladder")
    lines.append("")
    lines.append("| Role | Beats | Their score | Candidate | Its score |")
    lines.append("|---|---|---|---|---|")
    for role, top, ts, cand, cs in findings:
        lines.append(f"| {role} | `{top}` | {ts} | `{cand}` | {cs} |")
    lines.append("")
    lines.append("This is evidence, not a decision. Whether a ladder changes is the BDFL's "
                 "call (ADR-0012), including whether this project can even reach these "
                 "models on its subscription, which their index does not know.")
else:
    lines.append("No model outside a ladder scores above that ladder's top rung.")
lines.append("")
# This project authenticates against a Claude subscription (ADR-0004), so
# a leaderboard of models it cannot call is noise. The Claude family is the
# actionable list; the overall top few stay only as a "is Claude keeping up"
# signal, which is a real strategic question for the BDFL.
claude = [m for m in models if "claude" in m["id"].lower()]
lines.append("## Claude family, the only models this project can call")
lines.append("")
lines.append("| Model | Intelligence | Coding | Agentic |")
lines.append("|---|---|---|---|")
for m in claude:
    lines.append(f"| `{m['id']}` | {m['score']} | {m['coding'] if m['coding'] is not None else '-'} "
                 f"| {m['agentic'] if m['agentic'] is not None else '-'} |")
lines.append("")
lines.append("Their catalogue lists effort tiers as separate entries with separate "
             "scores, so a rung of ours may appear here several times or not at all. "
             "That is what makes a **no match** above a naming question rather than "
             "an availability one.")
lines.append("")
lines.append("## Overall top 5, for context only")
lines.append("")
lines.append("| Model | Intelligence |")
lines.append("|---|---|")
for m in models[:5]:
    lines.append(f"| `{m['id']}` | {m['score']} |")
lines.append("")
lines.append(f"{len(models)} models carried an intelligence index in this snapshot, "
             f"{len(claude)} of them Claude.")
lines.append("")
# No attribution footer here: this script writes a SECTION, and the
# workflow concatenates sections and owns the document's footer. The two
# collectors must not depend on each other, so neither may own the frame.
lines.append(f"<!-- fingerprint: {fingerprint} -->")

body = "\n".join(lines)
write("issue.md", body)
write("summary.md", body)
write("verdict", "unchanged" if (unchanged or not findings) else "changed")
print(f"models={len(models)} findings={len(findings)} fingerprint={fingerprint} "
      f"unchanged={unchanged}")
