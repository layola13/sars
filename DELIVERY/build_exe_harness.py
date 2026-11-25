#!/usr/bin/env python3
"""build-exe first-increment harness: per-demo build + run evidence.

Usage: build_exe_harness.py [limit]
Records, for each sci rosetta demo, whether `sars build-exe` succeeds and,
when it does, the native exe's exit code and stdout (escaped).
"""
import os
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SARS = os.path.join(ROOT, "target", "release", "sars")
DEMOS = os.path.normpath(os.path.join(ROOT, "..", "sci", "demos", "rosetta"))
OUT = "/tmp/sars_build_exe_probe"


def esc(b):
    return b.decode("utf-8", "replace").replace("\\", "\\\\").replace("\n", "\\n").replace("\r", "\\r")


def main():
    limit = int(sys.argv[1]) if len(sys.argv) > 1 else 0
    names = sorted(
        d for d in os.listdir(DEMOS)
        if os.path.isfile(os.path.join(DEMOS, d, "main.sa"))
    )
    if limit:
        names = names[:limit]
    os.makedirs(OUT, exist_ok=True)
    built = ran = 0
    t0 = time.time()
    for d in names:
        exe = os.path.join(OUT, d + ".out")
        src = os.path.join(DEMOS, d, "main.sa")
        try:
            b = subprocess.run(
                [SARS, "build-exe", src, "-o", exe],
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=60,
            )
            if b.returncode != 0:
                reason = esc(b.stderr).strip().splitlines()[-1] if b.stderr else ""
                print("BUILD_FAIL %s %s" % (d, reason[:200]))
                continue
            built += 1
            try:
                r = subprocess.run([exe], stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=10)
                stdout, stderr, rc = esc(r.stdout), esc(r.stderr), r.returncode
            except subprocess.TimeoutExpired:
                stdout, stderr, rc = "<timeout>", "", 124
            ran += 1
            print("RUN %s rc=%d stdout=%s stderr=%s" % (d, rc, stdout[:300], stderr[:120]))
        except subprocess.TimeoutExpired:
            print("BUILD_TIMEOUT %s" % d)
    print("SUMMARY total=%d built=%d ran=%d elapsed_s=%.1f" % (len(names), built, ran, time.time() - t0))


if __name__ == "__main__":
    main()
