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
"""

import math


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
