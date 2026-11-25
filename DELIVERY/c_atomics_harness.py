#!/usr/bin/env python3
"""C backend atomics cluster evidence (108_atomic_spin_lock,
109_atomic_fetch_add, 363_fence_noop).

Usage: python3 c_atomics_harness.py <out.tsv>

For every sci rosetta demo, run `sars build-exe` and, on success, run the
produced native exe. Write one TSV line per name:

    name<TAB>build_rc<TAB>run_rc<TAB>stdout_escaped

`build_rc=0` plus `run_rc=0` is the pass criterion (stdout is also recorded so
that a silent semantic regression cannot hide behind a zero exit code).
"""
import os
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SARS = os.path.join(ROOT, "target", "release", "sars")
DEMOS = os.path.normpath(os.path.join(ROOT, "..", "sci", "demos", "rosetta"))
TMP = "/tmp/opencode/c_atomics"


def esc(b):
    return (
        b.decode("utf-8", "replace")
        .replace("\\", "\\\\")
        .replace("\n", "\\n")
        .replace("\r", "\\r")
    )


def one(name):
    exe = os.path.join(TMP, name + ".out")
    src = os.path.join(DEMOS, name, "main.sa")
    b = subprocess.run(
        [SARS, "build-exe", src, "-o", exe],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=90,
    )
    if b.returncode != 0:
        return "%s\t%d\t-\t-\n" % (name, b.returncode)
    try:
        r = subprocess.run(
            [exe], stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=20
        )
        return "%s\t0\t%d\t%s\n" % (name, r.returncode, esc(r.stdout)[:120])
    except subprocess.TimeoutExpired:
        return "%s\t0\t124\t<timeout>\n" % name


def main():
    out = sys.argv[1]
    os.makedirs(TMP, exist_ok=True)
    names = sorted(
        n for n in os.listdir(DEMOS) if os.path.isfile(os.path.join(DEMOS, n, "main.sa"))
    )
    t0 = time.time()
    lines = []
    with ThreadPoolExecutor(max_workers=8) as pool:
        for line in pool.map(one, names):
            lines.append(line)
    with open(out, "w") as f:
        f.writelines(lines)
    print("wrote %d names to %s in %.1fs" % (len(lines), out, time.time() - t0))


if __name__ == "__main__":
    main()
