#!/usr/bin/env python3
"""C backend fallible ABI cluster evidence (fallible signatures + `?` unpack).

Usage: python3 c_fallible_harness.py <before_binary> <after_binary> <out.log>

For every sci rosetta demo, run `build-exe` with both binaries and, on success,
run the produced native exe. Emits a summary plus one line per name:

    name before_rc=<rc> after_build_rc=<rc> run_rc=<rc> stdout='<escaped>'

`before_rc=0` and `after_build_rc=0`+`run_rc=0` are the pass criteria; stdout is
recorded so a silent semantic regression cannot hide behind a zero exit code.
"""
import os
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DEMOS = os.path.normpath(os.path.join(ROOT, "..", "sci", "demos", "rosetta"))
TMP = "/tmp/opencode/c_fallible"


def esc(b):
    return (
        b.decode("utf-8", "replace")
        .replace("\\", "\\\\")
        .replace("\n", "\\n")
        .replace("\r", "\\r")
    )


def run_one(binary, src, exe):
    b = subprocess.run(
        [binary, "build-exe", src, "-o", exe],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=90,
    )
    if b.returncode != 0:
        return b.returncode, None, ""
    try:
        r = subprocess.run(
            [exe], stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=20
        )
        return 0, r.returncode, esc(r.stdout)
    except subprocess.TimeoutExpired:
        return 0, 124, "<timeout>"


def one(name, before, after):
    src = os.path.join(DEMOS, name, "main.sa")
    bexe = os.path.join(TMP, "before_" + name + ".out")
    aexe = os.path.join(TMP, "after_" + name + ".out")
    brc, _, bout = run_one(before, src, bexe)
    arc, rrc, out = run_one(after, src, aexe)
    return "%s before_rc=%d before_stdout='%s' after_build_rc=%d run_rc=%s stdout='%s'\n" % (
        name, brc, bout[:120], arc, "-" if rrc is None else rrc, out[:120],
    )


def main():
    before, after, outpath = sys.argv[1:4]
    os.makedirs(TMP, exist_ok=True)
    names = sorted(
        n for n in os.listdir(DEMOS) if os.path.isfile(os.path.join(DEMOS, n, "main.sa"))
    )
    t0 = time.time()
    lines = []
    with ThreadPoolExecutor(max_workers=8) as pool:
        for line in pool.map(lambda n: one(n, before, after), names):
            lines.append(line)
    def rc0(line, key):
        for part in line.split():
            if part.startswith(key):
                return int(part.split("=")[1]) == 0
        return False
    before_rc0 = sum(rc0(l, "before_rc=") for l in lines)
    after_rc0 = sum(rc0(l, "after_build_rc=") and "run_rc=0" in l for l in lines)
    def field(line, key):
        for part in line.split():
            if part.startswith(key):
                return part[len(key):].rstrip("'")
        return ""
    regressions = [
        l for l in lines
        if rc0(l, "before_rc=")
        and not (rc0(l, "after_build_rc=") and "run_rc=0" in l)
    ]
    stdout_regressions = [
        l for l in lines
        if rc0(l, "before_rc=")
        and rc0(l, "after_build_rc=")
        and "run_rc=0" in l
        and field(l, "before_stdout='") != field(l, "stdout='")
    ]
    summary = (
        "total=%d\nbefore_rc0=%d\nafter_rc0=%d\n"
        "pass_to_fail=%d\nstdout_regressions=%d\n"
        % (len(lines), before_rc0, after_rc0, len(regressions), len(stdout_regressions))
    )
    with open(outpath, "w") as f:
        f.write(summary)
        f.writelines(lines)
    print(summary.strip() + " wrote %s in %.1fs" % (outpath, time.time() - t0))


if __name__ == "__main__":
    main()
