#!/usr/bin/env python3
"""Write one row a minute of this runner's vitals to the vitals table while a session runs (#1665, ADR-0060).

Five heartbeats in 27 hours ended in a SIGTERM to the Claude step that
skipped every `always()` step after it (#1661, #1665): no artifact, no
session row, no notice, and no record of what the machine was doing. A
runner that is stopped takes its own post-mortem with it, so the record
has to leave the VM while the session is still alive. This is that writer.
agent-heartbeat.yml starts it in its own session before the Claude step
(setsid, stdout and stderr in a file under RUNNER_TEMP), so the step that
starts it ends at once and the job's orphan cleanup ends it with the job,
the same cleanup that names `claude` at a kill. Each minute, one row:
memory available and swap free from /proc/meminfo, space available on `/`
from statvfs, the one-minute load from /proc/loadavg, and the heaviest
process by resident size with its name from /proc/<pid>/status. The
readers are `sessions vitals <run-id>` and retrospect, which prints a
killed run's newest row under its `killed` heading.

Never fails the run and never holds it: exit 0 on every path, a row that
could not be written is one line in its own log (token-free, d1.py scrubs),
and the loop ends at --max-minutes, the job's own ceiling, should the
cleanup miss it. No CLOUDFLARE_API_TOKEN is a valid configuration and gets
one line, like guard-row.py. A field the sampler cannot read is NULL in
its column, never 0, and never the row's loss.

Usage: vitals-sidecar.py [--once] [--interval SECONDS] [--max-minutes N] [--proc DIR] [--root PATH]

--once writes one row and exits. --proc and --root point the sampler at a
fixture tree and a path to statvfs for the test suite; no workflow sets
them.
"""

import argparse
import os
import sys
import time
from datetime import datetime, timezone

import d1

COLUMNS = ("run_id", "attempt", "at", "mem_avail_mb", "swap_free_mb",
           "disk_avail_mb", "load1", "top_rss_mb", "top_name")
INSERT = (f"INSERT OR REPLACE INTO vitals ({', '.join(COLUMNS)}) "
          f"VALUES ({', '.join('?' * len(COLUMNS))})")
KIB = 1024


def meminfo(proc):
    """MemAvailable and SwapFree in MB from /proc/meminfo; None for a field it lacks."""
    fields = {}
    with open(os.path.join(proc, "meminfo"), encoding="utf-8") as f:
        for line in f:
            key, _, rest = line.partition(":")
            parts = rest.split()
            if parts and parts[0].isdigit():
                fields[key.strip()] = int(parts[0]) // KIB
    return fields.get("MemAvailable"), fields.get("SwapFree")


def load1(proc):
    """The one-minute load average, the first field of /proc/loadavg."""
    with open(os.path.join(proc, "loadavg"), encoding="utf-8") as f:
        return float(f.read().split()[0])


def disk_avail_mb(root):
    """Space available to this user on the filesystem holding `root`, in MB."""
    st = os.statvfs(root)
    return st.f_bavail * st.f_frsize // (KIB * KIB)


def heaviest(proc):
    """The process with the largest resident size as (rss_mb, name); (None, None) when none is readable.

    A process that exits between the listing and the read is skipped: a
    sampler that raised on it would lose the row to a race it cannot win.
    """
    top = (None, None)
    for entry in os.listdir(proc):
        if not entry.isdigit():
            continue
        name, rss = None, None
        try:
            with open(os.path.join(proc, entry, "status"), encoding="utf-8") as f:
                for line in f:
                    if line.startswith("Name:"):
                        name = line.split(":", 1)[1].strip()
                    elif line.startswith("VmRSS:"):
                        rss = int(line.split()[1]) // KIB
        except OSError:
            continue
        if rss is not None and (top[0] is None or rss > top[0]):
            top = (rss, name)
    return top


def read(label, fn):
    """One field, or None with a line saying which: a field lost is not a row lost."""
    try:
        return fn()
    except (OSError, ValueError, IndexError) as error:
        print(f"vitals-sidecar: {label} unread: {type(error).__name__}: {error}", flush=True)
        return None


def sample(proc="/proc", root="/"):
    """One reading, a tuple in COLUMNS order from mem_avail_mb on."""
    mem, swap = read("meminfo", lambda: meminfo(proc)) or (None, None)
    rss, name = read("heaviest", lambda: heaviest(proc)) or (None, None)
    return (mem, swap, read("disk", lambda: disk_avail_mb(root)), read("load", lambda: load1(proc)), rss, name)


def row(env, now, reading):
    """The run's identity and the reading as the table's row."""
    return (int(env["GITHUB_RUN_ID"]), int(env.get("GITHUB_RUN_ATTEMPT") or 1),
            now.strftime("%Y-%m-%dT%H:%M:%SZ"), *reading)


def parse(argv):
    parser = argparse.ArgumentParser(prog="vitals-sidecar.py", description=__doc__.splitlines()[0])
    parser.add_argument("--once", action="store_true", help="write one row and exit")
    parser.add_argument("--interval", type=float, default=60.0, help="seconds between rows (60)")
    parser.add_argument("--max-minutes", type=float, default=240.0,
                        help="stop after this long, the job's timeout-minutes (240)")
    parser.add_argument("--proc", default="/proc", help=argparse.SUPPRESS)
    parser.add_argument("--root", default="/", help=argparse.SUPPRESS)
    return parser.parse_args(argv)


def main(argv):
    args = parse(argv)
    if not os.environ.get("CLOUDFLARE_API_TOKEN"):
        print("vitals-sidecar: skipped, no CLOUDFLARE_API_TOKEN", flush=True)
        return 0
    if "GITHUB_RUN_ID" not in os.environ:
        print("vitals-sidecar: skipped, no GITHUB_RUN_ID", flush=True)
        return 0
    started = time.monotonic()
    written = 0
    while True:
        values = row(os.environ, datetime.now(timezone.utc), sample(args.proc, args.root))
        try:
            d1.query(INSERT, values)
            written += 1
            print(f"vitals-sidecar: wrote row {written} at {values[2]}: mem avail {values[3]} MB, "
                  f"disk avail {values[5]} MB, load {values[6]}, heaviest {values[8]} {values[7]} MB", flush=True)
        except Exception as error:  # noqa: BLE001, the docstring's exit-0 invariant
            print(f"vitals-sidecar: not written: {type(error).__name__}: {error}", flush=True)
        if args.once or time.monotonic() - started >= args.max_minutes * 60:
            print(f"vitals-sidecar: done, {written} rows", flush=True)
            return 0
        time.sleep(args.interval)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
