#!/usr/bin/env python3
"""Decide whether an agent seat runs this cycle, on which model, at what effort.

Run by `.github/actions/agent-guard`, tested by `guard-decide.test.mjs`. It
lived as a heredoc inside that action's YAML until the allowance governor grew
a second gear (issue #375). At that point "never hidden in a YAML heredoc"
(CLAUDE.md style) stopped being a style point: a decider that can cancel a run
is a decider that has to be provable, and a heredoc cannot be called.

`decide()` is pure -- no network, no clock, no file reads -- so a test can put
it in states the live ledger reaches once a week.

Three of the four reasons a seat must not run are the caller's (global pause,
stale wake, and the exhausted-ladder half of this file). The fourth is here:

  Second gear (issue #375). Thrift alone is a discount, not a brake. It swaps
  the role to a cheaper ladder and then runs the same number of wakes, and the
  number of wakes is what costs. On 2026-08-30 the cadence lever was pulled by
  hand twice in three and a half hours (#368, #374) against a burn that rose
  anyway, each pull costing a run, a PR, a review and a worker deploy to change
  one integer this file already has the arithmetic to choose. So when the
  projection misses the reset AND the wake is discretionary, a computed share
  of those wakes is skipped. The cron becomes the responsive ceiling; this is
  the throttle underneath it.

Never a lever on the responsive path. `discretionary` is set by the calling
workflow and means the clock (or a chained wake) started this run because
nothing else did. An operator event wake, a Telegram dispatch, an alarm wake
and every reviewer run pass it as false: independent review and operator
responsiveness are not budget levers (ADR-0027, ADR-0039).

Self-restoring in both gears, like thrift: every wake re-projects from the
latest reading, so full cadence returns on its own once the allowance shows
slack. No PR either direction, and no run has to remember what the last one did.
"""

import json
import os
import re
import sys
import time

WEEK = 604800

# Never keep less than this share of the day, however badly the projection
# misses. The stall sweep, the inbox drain and the operator sweep only happen
# on a wake that runs, and a governor that starves them for days has traded a
# budget problem for a liveness problem.
KEEP_FLOOR = 0.25

# The decimation window. Wakes landing in the first KEEP fraction of each
# window run; the rest do not. One day, so the floor above is a six-hour
# window every twenty-four, which any cadence faster than six-hourly is
# guaranteed to land in at least once. The sweep-carrying seats (BDFL,
# heartbeat) tick far faster than that, and `guard-decide.test.mjs` asserts
# it against the real crons in wrangler.toml so moving the cadence lever
# cannot silently starve them. The herald and the researcher are governed
# without that guarantee, by design: their wakes carry no sweeps, and a
# floor week skipping them costs postponable work only.
WINDOW = 86400

EFFORTS = ("low", "medium", "high", "xhigh", "max")


def _fenced(body):
    """The JSON object inside a ledger issue body's first ```json fence.

    Anything unparseable reads as absent. Both ledgers fail open, for the same
    reason in two directions: a bad model ledger costs one 429, a bad
    allowance reading costs one thin cycle. Failing closed idles the fleet.
    """
    match = re.search(r"```json\s*(\{.*?\})\s*```", body or "", re.S)
    if not match:
        return None
    try:
        return json.loads(match.group(1))
    except ValueError:
        return None


def _utc(epoch):
    return time.strftime("%Y-%m-%d %H:%M", time.gmtime(epoch)) + " UTC"


def project(allowance):
    """Week-average seven-day burn against the next reset, or None if unusable.

    One reading's utilization over the window elapsed so far, never a
    two-reading delta: back-to-back sessions space the readings under a minute
    apart, the utilization delta falls below reporting precision, and the
    governor goes blind exactly when burn peaks (#369).

    None means "no usable reading", which is also what a projection that
    reaches the reset returns: both leave every caller in its normal tier.
    """
    ledger = _fenced(allowance)
    if ledger is None:
        return None
    # Pre-#369 ledgers nest the reading under "current"; the flat object IS
    # the reading now. One .get carries the transition.
    reading = ledger.get("current", ledger) if isinstance(ledger, dict) else {}
    try:
        observed = float(reading["observedAt"])
        resets = float(reading["resetsAt"])
        used = float(reading["utilization"])
    except (TypeError, KeyError, ValueError, AttributeError):
        return None

    elapsed = observed - (resets - WEEK)
    remaining = resets - observed
    # A reading from a lapsed window has no time left to spend anything over,
    # so it cannot say whether the current window is in trouble.
    if elapsed <= 0 or remaining <= 0:
        return None
    rate = used / elapsed
    if rate <= 0:
        return None
    exhausts_at = observed + (1.0 - used) / rate
    if exhausts_at >= resets:
        return None
    return {
        "rate": rate,
        # Negative when the allowance is already spent; the floor below turns
        # that into "keep a quarter", not "keep none".
        "sustainable": max((1.0 - used) / remaining, 0.0),
        "resets": resets,
        "exhausts_at": exhausts_at,
    }


def miss(projection):
    """The projection as one clause, numbers included. Both gears log it."""
    return (
        f"projected exhaustion {_utc(projection['exhausts_at'])} "
        f"misses reset {_utc(projection['resets'])} "
        f"at week-average {projection['rate'] * 3600 * 100:.2f}%/h"
    )


def keep_fraction(projection):
    """Share of discretionary wakes the allowance can still pay for."""
    return min(1.0, max(KEEP_FLOOR, projection["sustainable"] / projection["rate"]))


def keeps(now, fraction):
    """Keep every wake landing in the first `fraction` of the current window.

    Decimation in the time domain, which is the only kind that cannot be
    aliased by the shape of the wake stream, because it never looks at the
    stream. It asks one question of one wake: what time is it. Two wakes a
    second apart get the same answer and a run that never happened changes
    nothing.

    Counting wakes instead is the trap, and this function did it first. It
    decimated on `github.run_number`, reasoning that a run counter increments
    whatever the hour so no cadence could hide inside its period. But that
    counter advances on EVERY wake of the workflow, discretionary or not, so
    one interleaved operator wake per tick puts the whole cron on odd run
    numbers, where a keep-every-fourth rule keeps exactly none of them: total
    starvation of the seat, wearing the label of a 25% floor. Found in review
    of PR #383 by running it, which is the only way anyone was going to.

    The guarantee this gives instead is a hard bound on the gap rather than on
    the count: at KEEP_FLOOR the keep window is six hours wide, so no seat
    ticking faster than six-hourly goes more than a day without a wake. The
    share kept is approximate, landing near `fraction` for any cadence that
    divides the window into several ticks, and exact for none of them.
    """
    if fraction >= 1.0:
        return True
    return (now % WINDOW) / WINDOW < fraction


def decide(role, roles, ledger, allowance, now=0, discretionary=False):
    """One seat's run decision as (status, model, effort, note).

    Status is `ok` (run it), `skip` (second gear) or `exhausted` (no rung
    left). The note always says which reason fired and with what numbers,
    because the run log is the only place this decision is visible.
    """
    entry = roles.get(role) or {}
    projection = project(allowance)

    if projection and discretionary:
        fraction = keep_fraction(projection)
        if not keeps(now, fraction):
            return (
                "skip",
                "",
                "",
                f"SKIP: discretionary wake and {miss(projection)}; "
                f"keeping the first {fraction * 100:.0f}% of each day and "
                f"this wake is at {_utc(now)}",
            )

    thrift = ""
    if projection and entry.get("thrift"):
        entry = entry["thrift"]
        thrift = f" [THRIFT ({miss(projection)})]"

    # Levels per the Claude Code CLI --effort flag, verified against
    # `claude --help` on 2026-08-23. "ultracode" was on this list and is NOT
    # an --effort value, so the guard would have passed it through for the CLI
    # to reject, which is exactly the "typo takes an agent offline" case this
    # check exists to prevent.
    effort = entry.get("effort") or ""
    if effort and effort not in EFFORTS:
        print(f"unknown effort {effort!r} for {role}; ignoring it", file=sys.stderr)
        effort = ""

    ladder = entry.get("ladder") or []
    if not ladder:
        return ("ok", "", effort, f"no ladder for this role; using action default model{thrift}")

    # The model ledger maps model id -> epoch seconds at which the limit resets.
    limits = _fenced(ledger) or {}
    now = int(time.time())
    try:
        blocked = {k: int(v) for k, v in limits.items() if int(v) > now}
    except (TypeError, ValueError, AttributeError):
        blocked = {}

    for model in ladder:
        if model not in blocked:
            note = f"{model} (ladder: {' > '.join(ladder)})"
            if blocked:
                note += f"; limited: {', '.join(sorted(blocked))}"
            return ("ok", model, effort, note + thrift)

    resets = min(blocked[m] for m in ladder if m in blocked)
    return ("exhausted", "", "", f"every rung limited ({' > '.join(ladder)}); earliest reset {resets}{thrift}")


def main():
    role = os.environ["ROLE"]
    try:
        roles = json.load(open("agents/models.json"))["roles"]
    except (OSError, ValueError, KeyError) as exc:
        # A missing or broken file must not stop the fleet: fall back to the
        # action defaults, which is the pre-ADR-0018 behavior.
        print(f"ok|||models.json unreadable ({exc}); using action defaults")
        return
    print(
        "|".join(
            decide(
                role,
                roles,
                os.environ.get("LEDGER", ""),
                os.environ.get("ALLOWANCE", ""),
                time.time(),
                os.environ.get("DISCRETIONARY", "") == "true",
            )
        )
    )


if __name__ == "__main__":
    main()
