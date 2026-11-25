#!/usr/bin/env python3
"""C backend extern/native cluster evidence (staged host bindings).

Usage: python3 c_extern_harness.py <before_binary> <after_binary> <out.log>

Runs every sci rosetta demo through `build-exe` with both binaries and, on
success, runs the produced native exe. The log records one line per name:

    name before_rc=<rc> before_stdout='<escaped>' after_build_rc=<rc> run_rc=<rc> stdout='<escaped>'

Summary reports per-category pass-after for names tagged `extern` and `native`
in build_exe_fail_classify.json, so both clusters are accounted explicitly.
"""
import json
import os
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DEMOS = os.path.normpath(os.path.join(ROOT, "..", "sci", "demos", "rosetta"))
TMP = "/tmp/opencode/c_extern"
CLASSIFY = os.path.join(ROOT, "DELIVERY", "build_exe_fail_classify.json")


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


def field(line, key):
    for part in line.split():
        if part.startswith(key):
            return part[len(key):].rstrip("'")
    return ""


def rc0(line, key):
    try:
        return int(field(line, key)) == 0
    except ValueError:
        return False


def after_pass(line):
    return rc0(line, "after_build_rc=") and field(line, "run_rc=") == "0"


def main():
    before, after, outpath = sys.argv[1:4]
    os.makedirs(TMP, exist_ok=True)
    with open(CLASSIFY) as f:
        classify = json.load(f)
    tags = {
        tag: {k for k, v in classify.items() if tag in v}
        for tag in ("extern", "native", "vtable")
    }
    names = sorted(
        n for n in os.listdir(DEMOS) if os.path.isfile(os.path.join(DEMOS, n, "main.sa"))
    )
    t0 = time.time()
    lines = []
    with ThreadPoolExecutor(max_workers=8) as pool:
        for line in pool.map(lambda n: one(n, before, after), names):
            lines.append(line)
    before_rc0 = sum(rc0(l, "before_rc=") for l in lines)
    after_rc0 = sum(after_pass(l) for l in lines)
    regressions = [
        l for l in lines if rc0(l, "before_rc=") and not after_pass(l)
    ]
    stdout_regressions = [
        l for l in lines
        if rc0(l, "before_rc=") and after_pass(l)
        and field(l, "before_stdout='") != field(l, "stdout='")
    ]
    summary = (
        "total=%d\nbefore_rc0=%d\nafter_rc0=%d\n"
        "pass_to_fail=%d\nstdout_regressions=%d\n"
        % (
            len(lines), before_rc0, after_rc0,
            len(regressions), len(stdout_regressions),
        )
    )
    for tag, tagged in tags.items():
        tagged_lines = [l for l in lines if l.split()[0] in tagged]
        summary += "%s_total=%d\n%s_pass_after=%d\n" % (
            tag, len(tagged_lines), tag, sum(after_pass(l) for l in tagged_lines),
        )
    with open(outpath, "w") as f:
        f.write(summary)
        f.writelines(lines)
    print(summary.strip() + " wrote %s in %.1fs" % (outpath, time.time() - t0))


if __name__ == "__main__":
    main()
