"""One parse for the API's rate-limit payloads; every consumer imports this.

Imported, not run, like `anchor.py`. Two consumers read the same
`rate_limit_events` out of audit artifacts: `audit-extract.py` derives the
allowance index that names each artifact, `retrospect` prints the budget
footer the BDFL reads every wake. They used to carry twin copies of this
parse kept aligned by comment, and both #308 defects lived in exactly that
gap (issue #310), so the shape rule lives here once.

Shape rule, two payloads on the record: until 2026-08-26 an event carried
one window flat -- rateLimitType, utilization, resetsAt. Since then (run
33171829189's artifact is the reference) the flat fields only name the
event's trigger window and carry no utilization; both windows ride nested
under `unifiedWindows`, keyed by window name. A nested form present means
the flat fields are ignored, because a payload carrying both describes the
same windows twice.

Validation stays with the consumers because they need different halves,
stated here once so neither invents its own: the allowance index encodes
utilization AND reset, so it demands `valid_fraction` and `valid_reset`
both; the budget footer informs with the fraction alone, so it demands
only `valid_fraction` and degrades a bad reset in display. `window_readings`
itself filters on structure only, yielding invalid windows too, so a
consumer can NAME the window kinds that fail its validation instead of
dropping them silently (issue #310).

Projection lives here too, for the same reason the parse does. `project`
is the week-average arithmetic the allowance governor throttles on
(guard-decide.py, ADR-0039). The budget footer used to carry its own
projection, a two-reading delta over the audited minutes, and the two
disagreed at the same instant (issue #533; retrospect's docstring has the
numbers). One arithmetic, imported by both, cannot disagree with itself.
"""

import math

WEEK = 604800


def project(observed, resets, used):
    """Week-average seven-day burn against the next reset, or None if unusable.

    One reading's utilization over the window elapsed so far, never a
    two-reading delta: back-to-back sessions space the readings under a
    minute apart, the utilization delta falls below reporting precision,
    and the governor goes blind exactly when burn peaks (#369). The same
    delta over-reads in the other direction when the readings straddle a
    burst (#533). Elapsed time since the window opened is the only
    denominator that does neither.

    None means "no usable reading", which is also what a projection that
    reaches the reset returns: both leave every caller in its normal tier.
    Epoch seconds and a fraction in [0, 1], all three; the caller parses.
    """
    try:
        observed, resets, used = float(observed), float(resets), float(used)
    except (TypeError, ValueError):
        return None
    if not all(math.isfinite(v) for v in (observed, resets, used)):
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
        # Negative when the allowance is already spent; the governor's floor
        # turns that into "keep a quarter", not "keep none".
        "sustainable": max((1.0 - used) / remaining, 0.0),
        "resets": resets,
        "exhausts_at": exhausts_at,
    }


def window_readings(info):
    """Yield (window kind, window dict) for every window one payload carries.

    Structural filtering only: a non-dict payload or window is unusable by
    any consumer, everything else is yielded raw, valid or not.
    """
    if not isinstance(info, dict):
        return
    nested = info.get("unifiedWindows")
    windows = (nested.items() if isinstance(nested, dict)
               else [(info.get("rateLimitType", "unknown"), info)])
    for kind, window in windows:
        if isinstance(window, dict):
            yield kind, window


def valid_fraction(utilization):
    """A utilization safe to print and compare: finite number in [0, 1], not bool."""
    return (isinstance(utilization, (int, float))
            and not isinstance(utilization, bool)
            and math.isfinite(utilization)
            and 0 <= utilization <= 1)


def valid_reset(resets_at):
    """A reset instant safe to encode: positive integer epoch seconds, not bool."""
    return (isinstance(resets_at, int)
            and not isinstance(resets_at, bool)
            and resets_at > 0)
