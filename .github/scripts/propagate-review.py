#!/usr/bin/env python3
"""Refresh agent-review.yml on every open PR branch after it changes on main.

The class this compiles (issue #132, judged by ADR-0022): the moment a PR
touching `.github/workflows/agent-review.yml` merges, every open PR carries a
stale copy, claude-code-action refuses to start on all of them, and the
refusal exits 0. The manual rescue was an agent diagnosing the same thing
twice (#68, #126) and hand-merging main; this script is that rescue, run at
the only moment the class can begin: a push to main that touched the file.

Mechanism, per open PR:
- Skip drafts (a draft is a human's work-in-progress; pushing to it uninvited
  is interference, and review does not run on drafts anyway) and fork PRs
  (their branches live in the fork, unreachable by the merges API).
- Compare the file's blob SHA at the PR head against the triggering commit.
  Equal means fresh, skip; the trigger path filter says the file changed, but
  a branch cut after the change already carries it.
- Stale: POST /repos/{repo}/merges, base = the PR branch, head = the
  triggering SHA. The merge happens server-side; no PR tree is ever checked
  out here and no PR code runs, which is what makes the admin credential in
  this job's environment safe to hold (workflow header has the full ruling).
- 201: verify with settle-push --sha, because a push whose runs never start
  is the exact silence this exists to end. 204 (branch already contains the
  commit, e.g. a PR that itself edits agent-review.yml): nothing to push,
  note it and move on. 409: conflict, which is judgment, so it is NOT
  resolved here; the job fails red and agent-alarm wakes the fixer.

Exit nonzero on any conflict or settle failure. Silence is never success.

Env: GITHUB_REPOSITORY, GITHUB_SHA, GH_TOKEN (admin PAT; merge commits
rewrite a workflow file on the branch, which the app token cannot push,
issue #24).
"""

import json
import os
import subprocess
import sys

REVIEW_PATH = ".github/workflows/agent-review.yml"
REPO = os.environ["GITHUB_REPOSITORY"]
MAIN_SHA = os.environ["GITHUB_SHA"]


def gh(*args):
    """gh on the job's ambient credential; returns (exit code, stdout, stderr)."""
    proc = subprocess.run(("gh",) + args, capture_output=True, text=True)
    return proc.returncode, proc.stdout, proc.stderr


def blob_sha(ref):
    """Blob SHA of agent-review.yml at ref, or None where the file is absent."""
    code, out, err = gh("api", f"repos/{REPO}/contents/{REVIEW_PATH}?ref={ref}",
                        "--jq", ".sha")
    if code != 0:
        if "404" in err:
            return None
        sys.exit(f"propagate-review: contents read at {ref} failed: {err.strip()}")
    return out.strip()


def main():
    main_blob = blob_sha(MAIN_SHA)
    if main_blob is None:
        sys.exit(f"propagate-review: {REVIEW_PATH} missing at {MAIN_SHA[:9]};"
                 " the trigger path filter and reality disagree, fix that first.")

    code, out, err = gh("api", f"repos/{REPO}/pulls?state=open&per_page=100")
    if code != 0:
        sys.exit(f"propagate-review: PR list failed: {err.strip()}")

    conflicted, failed = [], []
    for pr in json.loads(out):
        number, branch = pr["number"], pr["head"]["ref"]
        if pr["draft"]:
            print(f"#{number}: draft, left alone")
            continue
        head_repo = (pr["head"]["repo"] or {}).get("full_name")
        if head_repo != REPO:
            print(f"#{number}: fork branch ({head_repo}), unreachable, left alone")
            continue
        if blob_sha(pr["head"]["sha"]) == main_blob:
            print(f"#{number}: review copy already fresh")
            continue

        code, out, err = gh("api", "-X", "POST", f"repos/{REPO}/merges",
                            "-f", f"base={branch}", "-f", f"head={MAIN_SHA}",
                            "-f", f"commit_message=merge main: refresh {REVIEW_PATH}")
        if code != 0:
            if "409" in err:
                print(f"#{number}: CONFLICT merging main into {branch};"
                      " resolving is judgment, not this script's")
                conflicted.append(number)
            else:
                print(f"#{number}: merge failed: {err.strip()}")
                failed.append(number)
            continue
        if not out.strip():
            print(f"#{number}: already contains {MAIN_SHA[:9]}, nothing to push")
            continue
        merged = json.loads(out)["sha"]
        print(f"#{number}: merged main as {merged[:9]}, settling")
        settle = subprocess.run((".github/scripts/settle-push", str(number),
                                 "--sha", merged))
        if settle.returncode != 0:
            failed.append(number)

    if conflicted or failed:
        sys.exit("propagate-review:"
                 + (f" conflicts on {conflicted} (mechanical rescue per"
                    " agents/GOVERNANCE.md 'Stalled auto-merge')" if conflicted else "")
                 + (f" failures on {failed}" if failed else ""))
    print("propagate-review: every open PR carries the current review workflow")


main()
